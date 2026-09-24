//! Struct observations are specified in language terms: copied values,
//! mutable captured cells, and shared reference fields. No test observes the
//! JavaScript representation or assumes a particular allocation strategy.
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
        .expect("Node is required for struct value execution tests");
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
                "javascript-struct-artifact {}",
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
fn javascript_struct_copies_and_projected_values_are_independent() {
    matrix(
        "value-transfers",
        r#"
        struct Leaf { int value; }
        struct Pair { Leaf left; Leaf right; }
        export void run() {
            Leaf seed=Leaf{1};
            Pair original=Pair{seed,Leaf{2}};
            Pair copy=original;
            Leaf projected=original.left;
            seed.value=10;
            projected.value=11;
            copy.left.value=12;
            copy.right.value=13;
            print(seed.value);print(projected.value);
            print(original.left.value);print(original.right.value);
            print(copy.left.value);print(copy.right.value);
            Pair saved=(original=copy);
            original.left.value=14;
            copy.right.value=15;
            print(saved.left.value);print(saved.right.value);
            print(original.left.value);print(original.right.value);
            print(copy.left.value);print(copy.right.value);
        }
    "#,
        "",
        "library.run();",
        json!([10, 11, 1, 2, 12, 13, 12, 13, 14, 13, 12, 15]),
    );
}

#[test]
fn javascript_struct_field_updates_rebuild_from_current_root_after_rhs_reentry() {
    matrix(
        "field-reentry",
        r#"
        struct Pair { int left; int right; }
        struct Box { Pair pair; int sibling; }
        extern int invoke(int tag,func()->void callback,int result);
        Pair replacement(func()->void callback) {
            invoke(3,callback,0);
            return Pair{7,8};
        }
        export void run() {
            Box state=Box{Pair{1,2},3};
            state.pair.left=invoke(1,()=>{state.pair.right=20;state.sibling=30;},9);
            print(state.pair.left);print(state.pair.right);print(state.sibling);
            state.pair.left+=invoke(2,()=>{state=Box{Pair{100,200},300};},4);
            print(state.pair.left);print(state.pair.right);print(state.sibling);
            state.pair=replacement(()=>{state.sibling=400;});
            print(state.pair.left);print(state.pair.right);print(state.sibling);
            try {
                state.pair.left=invoke(4,()=>{state.pair.right=500;throw 13;},1);
            } catch(auto caught) { print(caught); }
            finally { print(state.pair.left);print(state.pair.right);print(state.sibling); }
        }
    "#,
        r#"
        globalThis.invoke=(tag,callback,result)=>{
            events.push(['invoke',tag]);callback();events.push(['return',tag]);return result;
        };
    "#,
        "library.run();",
        json!([
            ["invoke", 1],
            ["return", 1],
            9,
            20,
            30,
            ["invoke", 2],
            ["return", 2],
            13,
            200,
            300,
            ["invoke", 3],
            ["return", 3],
            7,
            8,
            400,
            ["invoke", 4],
            13,
            7,
            500,
            400
        ]),
    );
}

#[test]
fn javascript_struct_function_arguments_and_returns_are_values() {
    matrix(
        "function-values",
        r#"
        struct Pair { int left; int right; }
        Pair update(Pair value) { value.left+=100;return value; }
        int consume(Pair value) { value.right=70;return value.left+value.right; }
        Pair make(int seed) { return Pair{seed,seed+1}; }
        export void run() {
            Pair original=Pair{1,2};
            Pair changed=update(original);
            print(original.left);print(original.right);
            print(changed.left);print(changed.right);
            print(consume(changed));print(changed.right);
            Pair copy=update(changed);
            changed.left=3;
            print(copy.left);print(changed.left);
            print(make(8).right);
        }
    "#,
        "",
        "library.run();",
        json!([1, 2, 101, 2, 171, 2, 201, 3, 9]),
    );
}

#[test]
fn javascript_struct_closures_share_the_captured_cell_per_activation() {
    matrix(
        "captured-value-cells",
        r#"
        struct State { int count; int sibling; }
        extern void keep(func()->int read,func(int)->void write,func()->int snapshot);
        export void make(int seed) {
            State state=State{seed,seed+1};
            State saved=state;
            keep(()=>state.count+state.sibling,
                (int next)=>{state.count=next;},()=>saved.count+saved.sibling);
        }
    "#,
        r#"
        const callbacks=[];
        globalThis.keep=(read,write,snapshot)=>callbacks.push({read,write,snapshot});
    "#,
        r#"
        library.make(2);library.make(20);
        const a=callbacks[0],b=callbacks[1];
        events.push(a.read(),b.read(),a.snapshot(),b.snapshot());
        a.write(10);events.push(a.read(),b.read(),a.snapshot(),b.snapshot());
        b.write(40);events.push(a.read(),b.read(),a.snapshot(),b.snapshot());
    "#,
        json!([5, 41, 5, 41, 13, 41, 5, 41, 13, 61, 5, 41]),
    );
}

#[test]
fn javascript_struct_array_fields_copy_references_and_freeze_index_receivers_before_rhs() {
    matrix(
        "shared-reference-fields",
        r#"
        struct Bag { int[] items; int marker; }
        extern int invoke(int tag,func()->void callback,int result);
        export void run() {
            int[] items=[1,2];
            Bag original=Bag{items,10};
            Bag copy=original;
            copy.marker=20;
            copy.items[0]=7;
            items[1]=8;
            print(original.marker);print(copy.marker);
            print(original.items[0]);print(original.items[1]);print(copy.items[1]);
            copy.items=[9,10];
            items[0]=11;
            print(original.items[0]);print(copy.items[0]);
            Bag alias=original;
            alias.items[0]=invoke(1,()=>{alias.items=[100,200];},5);
            print(original.items[0]);print(items[0]);print(alias.items[0]);print(alias.items[1]);
            print(original.marker);print(alias.marker);
        }
    "#,
        r#"
        globalThis.invoke=(tag,callback,result)=>{
            events.push(['invoke',tag]);callback();events.push(['return',tag]);return result;
        };
    "#,
        "library.run();",
        json!([
            10,
            20,
            7,
            8,
            8,
            11,
            9,
            ["invoke", 1],
            ["return", 1],
            5,
            5,
            100,
            200,
            10,
            10
        ]),
    );
}

#[test]
fn javascript_struct_field_assignment_checks_tdz_before_rhs() {
    matrix(
        "field-tdz",
        r#"
        struct Point { int value; }
        extern int rhs();
        extern int during(func()->void callback);
        Point state=Point{during(()=>{state.value=rhs();})};
        print(state.value);
    "#,
        r#"
        globalThis.rhs=()=>{events.push('rhs');return 7;};
        globalThis.during=callback=>{
            events.push('during');
            try{callback();events.push('missing TDZ');}
            catch(error){events.push(['caught',error.name]);}
            return 1;
        };
        "#,
        "",
        json!(["during", ["caught", "ReferenceError"], 1]),
    );
}
