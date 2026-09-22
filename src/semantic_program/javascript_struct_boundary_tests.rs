//! Representation boundaries are checked after source and core verification.
//! These negative cases are legal source programs; unsupported ABI adaptation
//! must fail formation before a host can observe the private product storage.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use serde_json::{json, Value as Json};
use std::process::Command;

fn output(source: &str, compact: bool, module: bool) -> Result<String, CandidateError> {
    output_with_edit(source, compact, module, |_| {})
}

fn output_with_edit(
    source: &str,
    compact: bool,
    module: bool,
    edit: impl FnOnce(&mut Program<'_>),
) -> Result<String, CandidateError> {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source)
        .unwrap_or_else(|error| panic!("source parse failed: {error:?}\n{source}"));
    let checked = crate::analyze(&syntax)
        .unwrap_or_else(|error| panic!("source check failed: {error:?}\n{source}"));
    let mut program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("core conversion failed: {error:?}\n{source}"));
    program.verify().unwrap();
    edit(&mut program);
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
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source_id = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compilation
        .direct_javascript(source_id, &policy, WorkDomain::Baseline)
        .unwrap();
    let retained = compilation.ledger().retained_bytes();
    let result = compilation
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result);
    // Formation failures must release their partial module and all validation
    // scratch. No artifact arena entry is installed on these failures.
    if result.is_err() {
        assert_eq!(compilation.ledger().retained_bytes(), retained);
    }
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

fn rejected(source: &str, feature: &str) {
    for compact in [false, true] {
        match output(source, compact, true) {
            Err(CandidateError::Unsupported(error)) => {
                assert!(error.feature.contains(feature), "{error:?}\n{source}");
            }
            result => panic!("expected target boundary rejection, got {result:?}\n{source}"),
        }
    }
}

fn execute(javascript: &str, host: &str, observations: &str) -> Json {
    let script = format!(
        "const events=[];console.log=value=>events.push(value);\n{host}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{observations}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(javascript).unwrap(),
    );
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for boundary observation tests");
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

/// A host that records what it receives and then tries to mutate it.
const MUTATING_HOST: &str = "const touch=o=>{if(o&&typeof o==='object'){if('x' in o)o.x=9;for(const k of Object.keys(o))if(o[k]&&typeof o[k]==='object')o[k].x=9;}};globalThis.mutate=v=>{events.push(JSON.stringify(v));touch(v)};globalThis.dynamic=globalThis.mutate;";

#[test]
fn a_struct_reaching_a_jsvalue_is_its_public_shape_and_stays_a_value() {
    // Every transfer to a `JsValue` position sees the D2 public shape: a
    // fresh plain object of the fields. The host may mutate that copy; the
    // struct value, which has no identity, is unaffected.
    for (body, seen) in [
        // Local declaration and assignment are distinct semantic transfers.
        ("P value=P{1};JsValue erased=value;mutate(erased);print(value.x);", "{\"x\":1}"),
        ("P value=P{1};JsValue erased=null;erased=value;mutate(erased);print(value.x);", "{\"x\":1}"),
        // A dynamic callee and a private function taking `JsValue`.
        ("extern JsValue dynamic;P value=P{1};dynamic(value);print(value.x);", "{\"x\":1}"),
        ("void consume(JsValue erased){mutate(erased);}P value=P{1};consume(value);print(value.x);", "{\"x\":1}"),
        // Collection and struct-field `JsValue` positions.
        ("P value=P{1};JsValue[] erased=[value];mutate(erased);print(value.x);", "[{\"x\":1}]"),
        ("P value=P{1};Record<JsValue> erased=record{item:value};mutate(erased);print(value.x);", "{\"item\":{\"x\":1}}"),
        ("P value=P{1};JsValue[] erased=[null];erased[0]=value;mutate(erased);print(value.x);", "[{\"x\":1}]"),
        ("P value=P{1};Record<JsValue> erased=record{item:null};erased.item=value;mutate(erased);print(value.x);", "{\"item\":{\"x\":1}}"),
        ("struct Holder{JsValue payload;}P value=P{1};Holder holder=Holder{value};mutate(holder.payload);print(value.x);", "{\"x\":1}"),
        ("struct Holder{JsValue payload;}P value=P{1};Holder holder=Holder{null};holder.payload=value;mutate(holder.payload);print(value.x);", "{\"x\":1}"),
        // An object literal is a host object: its entries are `JsValue`
        // positions, and the language gives no typed read back.
        ("P value=P{1};auto erased=object{item:value};mutate(erased);print(value.x);", "{\"item\":{\"x\":1}}"),
    ] {
        let source = format!("struct P{{int x;}}extern void mutate(JsValue value);{body}");
        for compact in [false, true] {
            let javascript = output(&source, compact, true).unwrap();
            assert_eq!(
                execute(&javascript, MUTATING_HOST, ""),
                json!([seen, 1]),
                "{source}"
            );
        }
    }
    let javascript = output("struct P{int x;}export JsValue expose(){return P{1};}", true, true).unwrap();
    assert_eq!(
        execute(&javascript, "", "console.log(JSON.stringify(library.expose()));"),
        json!(["{\"x\":1}"])
    );
}

#[test]
fn lazy_results_and_thrown_products_cannot_launder_their_representation() {
    for body in [
        // The JsValue alternative makes the result type erased. The P-valued
        // arm still needs the common result interface check at Select itself.
        "extern bool flag;JsValue other=null;P value=P{1};JsValue erased=if(flag){value}else{other};mutate(erased);print(value.x);",
        "JsValue? other=null;P value=P{1};JsValue erased=other??value;mutate(erased);print(value.x);",
    ] {
        rejected(
            &format!("struct P{{int x;}}extern void mutate(JsValue value);{body}"),
            "ABI adapter",
        );
    }
    rejected(
        "struct P{int x;}extern void mutate(JsValue value);P value=P{1};try{throw value;}catch(auto caught){mutate(caught);}print(value.x);",
        "thrown value-struct ABI adaptation",
    );
}

#[test]
fn struct_bearing_callable_frames_are_strict_in_scripts_and_modules() {
    let source = "struct P{int x;}extern void probe();void consume(P value){probe();print(value.x);}consume(P{1});";
    for compact in [false, true] {
        // A classic script's struct-bearing frame is printed strict, so a
        // sloppy host sees `caller === null` there as in a module.
        let script = output(source, compact, false).unwrap();
        assert!(script.contains("\"use strict\""), "{script}");
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(
                &javascript,
                // Deliberately sloppy host code: strict Module callers must
                // not be returned through Function.caller.
                "globalThis.probe=Function('events','return function probe(){events.push(probe.caller===null)}')(events);",
                "",
            ),
            json!([true, 1])
        );
    }
    // An unused declaration alone must not require a product ABI or strict
    // callable frame. No struct value reaches an executable interface here.
    assert!(output("struct P{int x;}print(2);", true, false).is_ok());
}

#[test]
fn own_and_arrow_captured_arguments_cannot_erase_struct_parameter_backing() {
    for body in [
        "void consume(P value){mutate(arguments[0]);print(value.x);}",
        "void consume(P value){auto expose=()=>arguments[0];mutate(expose());print(value.x);}",
    ] {
        rejected(
            &format!("struct P{{int x;}}extern JsValue arguments;extern void mutate(JsValue value);{body}consume(P{{1}});"),
            "arguments",
        );
    }
}

#[test]
fn exact_nullable_transfers_and_unrelated_dynamic_calls_remain_executable() {
    let source = r#"
        struct P{int x;}
        extern JsValue dynamic;
        P copy(P value){return value;}
        export void run(){
            P original=P{1};
            P? present=original;
            P? absent=null;
            P first=present??original;
            P second=absent??copy(original);
            P chosen=if(true){first}else{second};
            chosen.x=9;
            print(original.x);print(first.x);print(second.x);print(chosen.x);
            print(dynamic(7));
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(
                &javascript,
                "globalThis.dynamic=value=>{events.push('dynamic');return value+1;};",
                "library.run();",
            ),
            json!([1, 1, 1, 9, "dynamic", 8])
        );
    }
}

#[test]
fn struct_arrays_keep_value_semantics_through_indexing_methods_and_spread() {
    // A program-owned array holds products it stored itself. Element copies,
    // pushes, pops, slices, spreads, callbacks, class fields and element
    // replacement all keep value semantics.
    let source = r#"
        struct P{int x;}
        class Holder{P[] items;}
        export void run(){
            P a=P{1};
            P[] values=[];
            values.push(a);
            a.x=5;
            print(values[0].x);
            P b=values[0];
            b.x=7;
            print(values[0].x);
            values[0]=b;
            print(values[0].x);
            b.x=9;
            print(values[0].x);
            P[] more=[...values,P{3}];
            print(more.length);
            P last=more.pop();
            print(last.x);print(more.length);
            int total=0;
            more.forEach((P item)=>{total=total+item.x;});
            print(total);
            P[] copy=values.slice(0);
            P c=copy[0];
            c.x=4;
            copy[0]=c;
            print(values[0].x);print(copy[0].x);
            Holder holder=new Holder();
            holder.items.push(P{6});
            P held=holder.items[0];
            held.x=8;
            print(holder.items[0].x);
            print(values.map((P item)=>item.x*2)[0]);
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(&javascript, "", "library.run();"),
            json!([1, 1, 7, 7, 2, 3, 1, 7, 7, 4, 6, 14])
        );
    }
    // A field update through an element, a nested field or a class field
    // rebuilds the stored product from one evaluation of its array, index or
    // instance; copies taken before it keep their values.
    let source = r#"
        struct P{int x;}
        struct Q{P inner;int y;}
        class Holder{Q current;int calls;}
        int[] trace=[];
        int next(int value){trace.push(value);return value;}
        P[] pick(P[] values){trace.push(0);return values;}
        void update(Holder holder){holder.current.inner.x=holder.current.inner.x+10;}
        export void run(){
            P[] values=[P{1},P{2}];
            P before=values[1];
            pick(values)[next(1)].x=next(9);
            print(values[1].x);print(before.x);print(values[0].x);
            Q[] nested=[Q{P{3},4}];
            nested[0].inner.x=5;
            print(nested[0].inner.x);print(nested[0].y);
            Holder holder=new Holder();
            holder.current=Q{P{6},7};
            update(holder);
            print(holder.current.inner.x);print(holder.current.y);
            print(trace.length);print(trace[0]);print(trace[1]);print(trace[2]);
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(&javascript, "", "library.run();"),
            json!([9, 2, 1, 5, 4, 16, 7, 3, 0, 1, 9])
        );
    }
    // Equality would compare backing identity. (The checker already rejects
    // `join`, which would print it.)
    for body in [
        "P value=P{1};P[] values=[value];print(values.indexOf(value));",
        "P value=P{1};P[] values=[value];print(values.includes(value));",
    ] {
        rejected(&format!("struct P{{int x;}}{body}"), "value-struct");
    }
}

#[test]
fn prototype_index_hooks_are_outside_the_supported_host_model() {
    // The host model for program-owned arrays: the standard Array.prototype
    // methods are intact and no built-in prototype has accessors for array
    // index keys, as every JavaScript compiler assumes. This independently
    // executes what a host that breaks it could do to a shared backing value.
    // Literal construction defines own data properties; later Get/Set can
    // enter host prototype hooks, which mutate the original shared backing.
    for (hook, operation, expected) in [
        (
            r#"Object.defineProperty(Array.prototype,'0',{configurable:true,set(value){note('setter');value[0]=9;Object.defineProperty(this,'0',{value,writable:true,enumerable:true,configurable:true});}});"#,
            "const values=[];values[0]=seed;note(seed[0]);note(values[0][0]);",
            json!(["setter", 9, 9]),
        ),
        (
            r#"Object.defineProperty(Array.prototype,'1',{configurable:true,get(){note('getter');this[0][0]=9;return this[0];}});"#,
            "const values=[seed];note(values[1][0]);note(seed[0]);",
            json!(["getter", 9, 9]),
        ),
    ] {
        let script = format!(
            "let trace='';const note=value=>{{trace+=(trace?',':'')+JSON.stringify(value)}};const seed=[1];\n{hook}\ntry{{{operation}}}finally{{delete Array.prototype[0];delete Array.prototype[1];}}process.stdout.write('['+trace+']');"
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for prototype boundary witnesses");
        assert!(
            result.status.success(),
            "{}\n{script}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            serde_json::from_slice::<Json>(&result.stdout).unwrap(),
            expected
        );
    }
}

#[test]
fn private_record_products_and_unobserved_array_construction_keep_value_semantics() {
    let source = r#"
        struct P{int x;}
        export void run(){
            P value=P{1};
            P[] unobserved=[value];
            Record<P> values=record{item:value};
            P first=values.item??value;
            first.x=9;
            P second=values.item??value;
            print(value.x);print(first.x);print(second.x);
            values.item=P{3};
            P replaced=values.item??value;
            print(value.x);print(second.x);print(replaced.x);
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(&javascript, "", "library.run();"),
            json!([1, 9, 1, 1, 1, 3])
        );
    }
    // The admitted record above is a source-owned null-prototype allocation.
    // An external annotation is not evidence for that physical recipe.
    rejected(
        "struct P{int x;}extern Record<P> values;P fallback=P{2};P value=values.item??fallback;print(value.x);",
        "foreign value-struct storage adaptation",
    );
}

#[test]
fn core_verified_load_refinements_do_not_supply_a_missing_product_adapter() {
    for (kind, body) in [
        ("cell", "JsValue held=host;mutate(held);"),
        ("value", "JsValue held=host;mutate(held);"),
        (
            "field",
            "struct Holder{JsValue payload;}Holder held=Holder{host};mutate(held.payload);",
        ),
        (
            "member",
            "Record<JsValue> held=record{item:host};mutate(held.item);",
        ),
        (
            "index",
            "Record<JsValue> held=record{item:host};mutate(held[\"item\"]);",
        ),
    ] {
        let source = format!("struct P{{int x;}}extern JsValue host;extern void mutate(JsValue value);P seed=P{{1}};{body}");
        for compact in [false, true] {
            let mut edited_span = None;
            let result = output_with_edit(&source, compact, true, |program| {
                let product = TypeId::from_index(
                    program
                        .types
                        .iter()
                        .position(
                            |ty| matches!(ty, Type::Struct(declaration) if declaration.name == "P"),
                        )
                        .unwrap(),
                )
                .unwrap();
                let (unit, operation) = program
                    .units()
                    .iter()
                    .find_map(|unit| {
                        unit.data()
                            .operations
                            .iter()
                            .enumerate()
                            .find_map(|(index, operation)| {
                                let OperationKind::Load(place) = operation.kind else {
                                    return None;
                                };
                                let selected = match (kind, &unit.data().places[place.index()]) {
                                    ("cell" | "value", Place::Cell(cell)) => {
                                        program.cells[cell.index()].name == "held"
                                    }
                                    ("field", Place::Field { .. }) => true,
                                    ("member", Place::Member { .. }) => true,
                                    ("index", Place::Index { .. }) => true,
                                    _ => false,
                                };
                                selected.then_some((unit.id(), index))
                            })
                    })
                    .unwrap();
                // The edited graph is the one owned input later adopted by
                // Compilation. No source snapshot clone escapes its owner.
                let mut working = program.units[unit.index()].clone().into_working();
                let data = working.get_mut();
                if kind == "value" {
                    let initializer = data.operations.iter().find(|operation| {
                        matches!(operation.kind, OperationKind::Initialize(cell) if program.cells[cell.index()].name == "held")
                    }).unwrap();
                    let value = data.operands(initializer.operands).unwrap()[0];
                    let place = PlaceId::from_index(data.places.len()).unwrap();
                    data.places.push(Place::Value(value));
                    data.operations[operation].kind = OperationKind::Load(place);
                }
                let value = data.operations[operation].result.unwrap();
                data.values[value.index()].ty = product;
                edited_span = Some(data.operations[operation].span);
                program.units[unit.index()] = working.freeze();
                // These existing compatibility checks accept the narrowing.
                // The target must still qualify the physical load interface.
                program.verify().unwrap();
            });
            match result {
                Err(CandidateError::Unsupported(error)) => {
                    assert_eq!(Some(error.span), edited_span, "{kind}: {error:?}");
                    assert!(error.feature.contains("ABI adapter"), "{kind}: {error:?}");
                }
                result => panic!("missing {kind} load boundary qualification: {result:?}"),
            }
        }
    }
}

#[test]
fn stale_nullable_product_reads_keep_plain_store_checks_before_and_after_rhs() {
    let source = r#"
        struct P{int x;}
        extern int forbiddenRhs();
        void update(P value){value.x=forbiddenRhs();print("unreachable");}
        export void before(){
            P? saved=P{1};
            if(saved!=null){
                auto later=()=>update(saved);
                saved=null;
                try{later();}catch(auto caught){print("before-caught");}
            }
        }
        export void after(){
            P? saved=P{1};
            if(saved!=null){
                P current=P{3};
                int marker=0;
                auto rhs=()=>{current=(()=>saved)();marker=1;print("rhs");return 2;};
                saved=null;
                try{current.x=rhs();}catch(auto caught){print("after-caught");}
                print(marker);
            }
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(
                &javascript,
                "globalThis.forbiddenRhs=()=>{events.push('forbidden-rhs');return 2;};",
                "library.before();library.after();",
            ),
            json!(["before-caught", "rhs", "after-caught", 1])
        );
    }
}

#[test]
fn integer_field_loads_normalize_but_place_checks_never_coerce_the_old_payload() {
    // The actual host value of an extern int remains unknown before the
    // selected runtime normalization. The source signature is not evidence
    // that this raw value is already a JavaScript Number.
    let source = r#"
        struct P{int x;}
        extern int input;
        extern int rhs();
        export void read(){P value=P{input};print(value.x+1);}
        export void plain(){P value=P{input};value.x=rhs();print(value.x);}
        export void compound(){P value=P{input};value.x+=rhs();print(value.x);}
    "#;
    for (input, expected) in [
        (
            "'4'",
            json!(["read", 5, "plain", "rhs", 2, "compound", "rhs", 6]),
        ),
        (
            "({valueOf(){events.push('coerce');return 4;}})",
            json!(["read", "coerce", 5, "plain", "rhs", 2, "compound", "coerce", "rhs", 6]),
        ),
        (
            "({valueOf(){events.push('coerce');throw Error('coercion');}})",
            json!(["read", "coerce", "threw", "plain", "rhs", 2, "compound", "coerce", "threw"]),
        ),
        (
            "1n",
            json!(["read", "threw", "plain", "rhs", 2, "compound", "threw"]),
        ),
    ] {
        for compact in [false, true] {
            let javascript = output(source, compact, true).unwrap();
            assert_eq!(
                execute(
                    &javascript,
                    &format!("globalThis.input={input};globalThis.rhs=()=>{{events.push('rhs');return 2;}};"),
                    "for(const method of ['read','plain','compound']){events.push(method);try{library[method]();}catch(error){events.push('threw');}}",
                ),
                expected,
                "input={input} compact={compact}",
            );
        }
    }
}

#[test]
fn exported_value_struct_functions_publish_one_object_adapter_with_source_reflection() {
    // D2: a public value struct is a plain object whose own data properties
    // are its fields in declaration order. Each call returns a fresh object;
    // an incoming object is read once per field, depth first in declaration
    // order, so later caller mutation is not observed.
    let source = r#"
        struct Point{int x;int y;}
        struct Line{Point a;Point b;string label;}
        export Point make(int x,int y){return Point{x,y};}
        export int sum(Point p){return p.x+p.y;}
        export Line span(Point from,int by){return Line{from,Point{from.x+by,from.y+by},"span"};}
        export int far(Line line){return line.b.y-line.a.x;}
        export {make as build};
    "#;
    let observations = r#"
        const p=library.make(3,4);
        console.log(Object.keys(p));console.log(p.x+p.y);
        console.log(Object.getPrototypeOf(p)===Object.prototype);
        console.log(library.make(1,2)!==library.make(1,2));
        console.log(library.build===library.make);
        console.log([library.make.name,library.sum.name,library.span.name,library.far.name]);
        console.log([library.make.length,library.sum.length,library.span.length,library.far.length]);
        const reads=[];
        const point=(tag,x,y)=>({get x(){reads.push(tag+'.x');return x},get y(){reads.push(tag+'.y');return y}});
        console.log(library.sum(point('p',7,8)));
        console.log(library.far({get a(){reads.push('a');return point('a',1,2)},get b(){reads.push('b');return point('b',3,9)},get label(){reads.push('label');return 'L'}}));
        console.log(reads);
        console.log(JSON.stringify(library.span({x:1,y:2},10)));
        const argument={x:5,y:6};const kept=library.span(argument,0);argument.x=100;console.log(kept.a.x);
        try{library.sum(null);console.log('accepted')}catch(error){console.log(error instanceof TypeError)}
        console.log(typeof new library.make(1,2));
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(&javascript, "", observations),
            json!([
                ["x", "y"],
                7,
                true,
                true,
                true,
                ["make", "sum", "span", "far"],
                [2, 1, 2, 1],
                15,
                8,
                ["p.x", "p.y", "a", "a.x", "a.y", "b", "b.x", "b.y", "label"],
                r#"{"a":{"x":1,"y":2},"b":{"x":11,"y":12},"label":"span"}"#,
                5,
                true,
                "object"
            ])
        );
    }
}

#[test]
fn a_public_struct_whose_copy_would_lose_identity_or_frame_stays_refused() {
    for (source, feature) in [
        // A mutated collection, a callable or a nullable carrying a struct
        // has aliasing or identity a copying adapter cannot preserve. (A
        // read-only array parameter decodes; generic structs do not reach
        // formation yet, since conversion refuses them first.)
        (
            "struct P{int x;}export void add(P[] items){items.push(P{1});}",
            "public value-struct ABI adaptation",
        ),
        (
            "struct P{int x;}export int apply(func(P)->int f){return f(P{1});}",
            "public value-struct ABI adaptation",
        ),
        (
            "struct P{int x;}export P? maybe(int x){if(x>0){return P{x};}return null;}",
            "public value-struct ABI adaptation",
        ),
        // The wrapper supplies its own receiver; a body observing one would
        // see the wrapper's instead of the caller's.
        (
            "struct P{int x;}extern JsValue this;extern void mutate(JsValue value);export P who(){mutate(this);return P{1};}",
            "observed activation",
        ),
    ] {
        rejected(source, feature);
    }
}

#[test]
fn function_values_reaching_host_code_are_wrapped_by_a_callable_adapter() {
    // Host code calls a struct-bearing function with public shapes and gets
    // public shapes back: parameters decode, results (nullable too) encode.
    let source = r#"
        struct NV { string name; string value; }
        extern JsValue call(JsValue callback, JsValue argument);
        JsValue method = JS.method0((JsValue self) => { return NV{"self", JS.string(self)}; });
        JsValue maybe = (JsValue x) => { if (JS.isNullish(x)) { return null; } return NV{"b", "c"}; };
        JsValue takes = (NV pair) => pair.name + "=" + pair.value;
        export void run() {
            print(call(method, "s"));
            print(call(maybe, null));
            print(call(maybe, 1));
            print(call(takes, JS.object("name", "k", "value", "v")));
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(
                &javascript,
                "globalThis.call=(f,a)=>{const r=f===undefined?undefined:(typeof a==='string'?f.call(a):f(a));return typeof r==='object'&&r!==null?JSON.stringify(r):r};",
                "library.run();"
            ),
            json!(["{\"name\":\"self\",\"value\":\"s\"}", null, "{\"name\":\"b\",\"value\":\"c\"}", "k=v"])
        );
    }
}

#[test]
fn an_exported_function_may_take_a_struct_array_it_only_reads() {
    // A decoded copy of the host array is indistinguishable from the array
    // when the function only reads elements and `length`; `JS.assume` of a
    // struct to its own type is the identity.
    let source = r#"
        struct Block { bool keep; int depth; }
        export bool keepAll(Block[] stack) {
            for (int i = 0; i < stack.length; i++) {
                Block block = JS.assume(stack[i]);
                if (!block.keep) { return false; }
            }
            return true;
        }
        export int deepest(Block[] stack) {
            int best = 0;
            for (Block block of stack) { if (block.depth > best) { best = block.depth; } }
            return best;
        }
    "#;
    for compact in [false, true] {
        let javascript = output(source, compact, true).unwrap();
        assert_eq!(
            execute(
                &javascript,
                "",
                "const stack=[{keep:true,depth:1},{keep:false,depth:4}];console.log(library.keepAll(stack));console.log(library.keepAll(stack.slice(0,1)));console.log(library.deepest(stack));console.log(stack.length);"
            ),
            json!([false, true, 4, 2])
        );
    }
    // A function that mutates or retains its array would observe the copy.
    for body in [
        "export void grow(Block[] stack){stack.push(Block{true,1});}",
        "Block[] kept=[];export void keep(Block[] stack){kept=stack;}",
    ] {
        rejected(
            &format!("struct Block{{bool keep;int depth;}}{body}"),
            "public value-struct ABI adaptation",
        );
    }
}
