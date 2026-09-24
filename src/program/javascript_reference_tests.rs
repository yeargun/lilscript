//! Independent alias, evaluation-order and host-coercion observations for the
//! direct reference ABI. Complete artifacts execute before codec measurement.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Plan, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::process::Command;

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn execute(javascript: &str, setup: &str, observations: &str) -> Json {
    let script = format!(
        "const events=[];console.log=value=>events.push(value);\n{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{observations}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(javascript).unwrap(),
    );
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for reference execution tests");
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        result.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
fn matrix(case: &str, source: &str, setup: &str, observations: &str, expected: Json) {
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
        let checked = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &checked).unwrap();
        program.verify().unwrap();
        let ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
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
        for style in [Style::Global, Style::Scoped, Style::Source] {
            compilation
                .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
                .unwrap();
            let retained = compilation.ledger().retained_bytes();
            let (text, sizes, observed) = compilation
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&Plan::new(style))?;
                    let observed = output.with_artifact(artifact, |view| {
                        let observed = execute(view.javascript, setup, observations);
                        assert_eq!(
                            observed, expected,
                            "{case} compact={compact} style={style:?}"
                        );
                        observed
                    })?;
                    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
                        output.measure(artifact, codec)?;
                    }
                    let sizes = output.with_artifact(artifact, |view| view.sizes)?;
                    Ok::<_, CandidateError>((output.take_artifact(artifact)?, sizes, observed))
                })
                .unwrap()
                .unwrap();
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            assert_eq!(sizes.raw, text.len());
            assert!(sizes.gzip9.is_some() && sizes.brotli11.is_some());
            eprintln!(
                "javascript-reference-artifact {}",
                json!({
                    "case":case,"source":source,"source_sha256":digest(source),
                    "setup":setup,"observations":observations,"expected":expected,"observed":observed,
                    "compact":compact,"style":format!("{style:?}"),
                    "javascript_sha256":digest(&text),"javascript":text,
                    "raw":sizes.raw,"gzip9":sizes.gzip9,"brotli11":sizes.brotli11,
                })
            );
        }
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}

#[test]
fn reference_field_and_cell_integer_loads_keep_distinct_raw_contracts() {
    let source = r#"
        struct Holder { int value; }
        extern int opaque(); extern int later();
        int read(ref int value) { return value; }
        void ignore(ref int value,int next) { print(next); }
        void overwrite(ref int value) { value=7; }
        int cell=opaque(); Holder holder=Holder{cell};
        print(read(ref cell));
        try { print(read(ref holder.value)); } catch { print(99); }
        ignore(ref holder.value,later());
        overwrite(ref holder.value); print(holder.value);
    "#;
    matrix(
        "raw-cell-normalized-field",
        source,
        r#"const payload={ [Symbol.toPrimitive](hint){events.push('coerce:'+hint);return 4294967297;} };
        globalThis.opaque=()=>payload;globalThis.later=()=>{events.push('later');return 8};
        console.log=value=>events.push(value===payload?'raw-object':value);"#,
        "",
        json!(["raw-object", "coerce:number", 1, "later", 8, 7]),
    );
    matrix(
        "throwing-field-coercion",
        source,
        r#"const payload={ [Symbol.toPrimitive](hint){events.push('coerce:'+hint);throw 13;} };
        globalThis.opaque=()=>payload;globalThis.later=()=>{events.push('later');return 8};
        console.log=value=>events.push(value===payload?'raw-object':value);"#,
        "",
        json!(["raw-object", "coerce:number", 99, "later", 8, 7]),
    );
    matrix(
        "bigint-field-coercion",
        source,
        r#"globalThis.opaque=()=>1n;globalThis.later=()=>{events.push('later');return 8};
        console.log=value=>events.push(typeof value==='bigint'?'raw-bigint':value);"#,
        "",
        json!(["raw-bigint", 99, "later", 8, 7]),
    );
}

#[test]
fn reference_overlap_reloads_current_root_after_reentry_and_preserves_throwing_writes() {
    let source = r#"
        struct Pair { int left; int right; }
        extern void keep(func()->void callback); extern int rhs();
        Pair state=Pair{1,2};
        keep(()=>{state=Pair{40,50};});
        void update(ref Pair whole,ref int left) {
            whole.right=8;
            try { left+=rhs(); } finally { print(whole.right); }
        }
        try { update(ref state,ref state.left); } catch { print(77); }
        print(state.left);print(state.right);
    "#;
    matrix(
        "overlap-reentry",
        source,
        "let callback;globalThis.keep=value=>callback=value;globalThis.rhs=()=>{callback();return 3};",
        "",
        json!([50, 4, 50]),
    );
    matrix(
        "overlap-throw",
        source,
        "let callback;globalThis.keep=value=>callback=value;globalThis.rhs=()=>{callback();throw 13};",
        "",
        json!([50, 77, 40, 50]),
    );
}

#[test]
fn reference_preparation_tdz_aborts_before_later_argument_effects() {
    matrix(
        "reference-tdz",
        r#"
        extern int during(func()->void callback); extern int later();
        void update(ref int value,int next){value=next;}
        int state=during(()=>{update(ref state,later());});
        print(state);
    "#,
        r#"globalThis.during=callback=>{events.push('during');try{callback()}catch(error){events.push(error.name)}return 7};
        globalThis.later=()=>{events.push('later');return 9};"#,
        "",
        json!(["during", "ReferenceError", 7]),
    );
}

#[test]
fn reference_paths_and_product_rebuilding_do_not_use_array_prototype_hooks() {
    matrix(
        "prototype-safe-paths",
        r#"
        struct Pair {int left;int right;} struct Box {Pair pair;}
        extern void poison(); extern void restore();
        void increment(ref int value){value+=1;}
        void forward(ref Pair pair){increment(ref pair.left);pair.right+=2;}
        poison();
        Box state=Box{Pair{3,4}};
        forward(ref state.pair);
        restore();print(state.pair.left);print(state.pair.right);
    "#,
        r#"const original=Object.getOwnPropertyDescriptor(Array.prototype,'0');
        const saved={slice:Array.prototype.slice,map:Array.prototype.map,push:Array.prototype.push};
        events.push('ready');console.log=value=>events[events.length]=value;
        globalThis.poison=()=>{
            Object.defineProperty(Array.prototype,'0',{configurable:true,set(){throw Error('numeric setter')}});
            for(const key of ['slice','map','push'])Array.prototype[key]=()=>{throw Error('array method')};
        };
        globalThis.restore=()=>{
            if(original)Object.defineProperty(Array.prototype,'0',original);else delete Array.prototype[0];
            for(const key of ['slice','map','push'])Array.prototype[key]=saved[key];
        };"#,
        "",
        json!(["ready", 4, 6]),
    );
}

#[test]
fn reference_value_copies_captured_cells_and_shared_array_fields_keep_their_owners() {
    matrix(
        "copies-and-reference-leaves",
        r#"
        struct Box {int value;int[] items;}
        void alter(ref Box state){state.value=9;state.items[0]=7;}
        func()->int snapshot(ref Box state){Box copy=state;return ()=>copy.value;}
        int[] items=[1,2];Box state=Box{3,items};Box copy=state;
        func()->int saved=snapshot(ref state);
        func()->int live=()=>state.value;
        alter(ref state);
        print(saved());print(live());print(copy.value);print(copy.items[0]);print(items[0]);
    "#,
        "",
        "",
        json!([3, 9, 3, 7, 7]),
    );
}

#[test]
fn reference_catch_binding_gets_one_carrier_at_its_initialization() {
    matrix(
        "catch-reference",
        r#"
        void clear(ref JsValue value){value=null;}
        try{throw 7;}catch(auto caught){clear(ref caught);print(caught);}
    "#,
        "",
        "",
        json!([null]),
    );
}

#[test]
fn reference_field_alias_checks_stale_roots_before_and_after_rhs() {
    matrix(
        "reference-stale-root",
        r#"
        struct P{int value;}
        extern int rhs();extern void keep(func()->void callback);
        void plain(ref int value,int ignored){value=rhs();}
        export void before(){
            P? saved=P{1};
            if(saved!=null){
                P current=P{3};
                auto invalidate=()=>{current=(()=>saved)();return 0;};
                saved=null;
                try{plain(ref current.value,invalidate());}catch{print("before-caught");}
            }
        }
        export void after(){
            P? saved=P{1};
            if(saved!=null){
                P current=P{3};int marker=0;
                keep(()=>{current=(()=>saved)();marker=1;});
                saved=null;
                try{plain(ref current.value,0);}catch{print("after-caught");}
                print(marker);
            }
        }
    "#,
        "let callback;globalThis.keep=value=>callback=value;globalThis.rhs=()=>{events.push('rhs');if(callback)callback();return 2};",
        "library.before();library.after();",
        json!(["before-caught", "rhs", "after-caught", 1]),
    );
}

#[test]
fn reference_transport_rejects_script_frames_and_public_carrier_exports_without_leaks() {
    for (source, execution, feature) in [
        (
            "void bump(ref int value){value+=1;}int value=1;bump(ref value);",
            "script",
            "strict module",
        ),
        (
            "void bump(ref int value){value+=1;}export int value=1;bump(ref value);",
            "module",
            "public address-taken",
        ),
    ] {
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.strip_console = false;
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: execution == "module",
            })
            .unwrap();
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &checked).unwrap();
        let ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
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
        let before = compilation.ledger().retained_bytes();
        let mut called = false;
        let result = compilation.with_javascript_output(candidate, &policy, |_| {
            called = true;
        });
        assert!(!called);
        let error = format!("{:?}", result.unwrap_err());
        assert!(error.contains(feature), "{error}");
        assert_eq!(compilation.ledger().retained_bytes(), before);
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
}
