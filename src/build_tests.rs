use super::*;
use crate::compilation_policy::{PolicyConfig, TacticId, TacticPermission};
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

#[test]
fn q2_recursive_conditionals_terminate_with_each_objective_and_reuse_mode() {
    let source = "export bool even(int n){if(n==0){return true;}return odd(n-1);}bool odd(int n){if(n==0){return false;}return even(n-1);}";
    for reuse in [false, true] {
        let settings: ProjectConfig = toml::from_str(&format!("[optimization]\npreset='maximum'\n[effort]\nlevel=13\n[cache]\nnormalization_reuse={reuse}")).unwrap();
        let built = compile_source(source, &settings, ServiceOptions {
            objectives: Some(Objectives::All), ..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let js = built.javascript(codec).unwrap().javascript();
            assert_eq!(execute_javascript(js, "", "console.log(JSON.stringify([library.even(0),library.even(9),library.even(10),library.even(100)]));"), "[true,false,true,true]\n");
        }
    }
}

#[test]
fn s4_public_abi_checks_match_build_diagnostics_before_search() {
    let cases = [
        "struct P{int x;}struct Q{int y;}export P|Q copy(P|Q value){return value;}",
        "struct P{int x;}export void grow(P[] values){values.push(P{1});}",
        "struct P{int x;}extern void observe(JsValue value);export int run(func(P)->int f){observe(f);return 0;}",
        "struct P{int x;}export int read(P[] values,func()->void poke){int first=values[0].x;poke();return first*10+values[0].x;}",
        "struct P{int x;}export int read(P[] values,JsValue alias){int first=values[0].x;alias[0]=JS.object(\"x\",2);return first*10+values[0].x;}",
        "struct P{int x;}export int read(P[] values,JsValue incoming){int first=values[0].x;P p=JS.assume(incoming);return first+values[0].x+p.x;}",
        "struct P{int x;}extern Task<int> tick();export async int read(P[] values){int first=values[0].x;await tick();return first+values[0].x;}",
    ];
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        for source in cases {
            let checked=check_source(source,&settings).unwrap_err();
            let compiled=compile_source(source,&settings,ServiceOptions::default()).unwrap_err();
            assert_eq!(checked.phase,"check","{checked}\n{source}");
            assert!(checked.to_string().contains("public value-struct ABI adaptation"),"{checked}\n{source}");
            assert!(checked.diagnostic.is_some());
            assert_eq!(compiled.phase,checked.phase);
            assert_eq!(compiled.message,checked.message);
        }
    }
}

#[test]
fn s4_public_abi_checks_keep_the_reexported_declarations_source_module() {
    let scratch=Scratch::new();
    let source="struct P{int x;}struct Q{int y;}export P|Q copy(P|Q value){return value;}";
    std::fs::write(scratch.0.join("boundary.lil"),source).unwrap();
    std::fs::write(scratch.0.join("entry.lil"),"import {copy} from \"./boundary.lil\";export {copy};").unwrap();
    let settings=config("");
    let checked=check_path(&scratch.0.join("entry.lil"),None,&settings).unwrap_err();
    let compiled=compile_path(&scratch.0.join("entry.lil"),&settings,ServiceOptions::default()).unwrap_err();
    assert_eq!(checked.phase,"check","{checked}");
    assert!(checked.to_string().contains("boundary.lil"),"{checked}");
    assert!(checked.to_string().contains("public value-struct ABI adaptation"),"{checked}");
    assert_eq!(compiled.message,checked.message);
}

#[test]
fn s4_public_abi_closed_array_snapshots_and_private_bodies_remain_supported() {
    let source="struct P{int x;}export int total(P[] values){int n=0;for(P p of values){n+=p.x;}return n;}export int first(P[] values){P p=JS.assume(values[0]);return p.x;}";
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        check_source(source,&settings).unwrap();
        let built=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",r#"
                let reads=0;const values=[{get x(){reads++;return 3}},{get x(){reads++;return 4}}];
                console.log(JSON.stringify([library.total(values),reads,library.first([{x:9}])]));
            "#),"[7,2,9]\n");
        }
    }
    // Source accessibility is not a public ABI in an application request.
    let source="struct P{int x;}export void grow(P[] values){values.push(P{2});}P[] values=[];grow(values);print(values[0].x);";
    let built=compile_source(source,&config(""),ServiceOptions{preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_javascript(built.javascript(Objective::Brotli).unwrap().javascript(),"",""),"2\n");
}

#[test]
fn s4_public_abi_unused_real_decodes_still_read_the_host_fields() {
    let source="struct P{int x;}pure P same(P p){return JS.assume(p);}P decode(JsValue p){return JS.assume(p);}export void run(JsValue p){same(P{1});decode(p);}";
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",r#"
            let reads=0;library.run({get x(){reads++;return 2}});console.log(reads);
        "#),"1\n");
    }
}

#[test]
fn s4_product_unions_preserve_public_schemas_and_private_narrowing() {
    let source=r#"
        struct Point{int x;int y;}
        struct Box<T>{T value;}
        export Point|string change(Point|string value){if(value is string){return value+"!";}Point p=value;p.x+=1;return p;}
        export Box<int>|bool flip(Box<int>|bool value){if(value is bool){return !value;}Box<int> b=value;b.value+=2;return b;}
        export Point|int next(Point|int value){if(value is int){return value+3;}Point p=value;p.y+=4;return p;}
        export Point|string apply(func(Point|string)->(Point|string) f,Point|string value){return f(value);}
        export JsValue opaque(Point|string value){return value;}
        export Point|string assumed(JsValue value){Point|string item=JS.assume(value);return change(item);}
        T pick<T>(T|string value,T fallback){if(value is string){return fallback;}return value;}
        export int internal(){Point p=pick("other",Point{7,8});Point|string value=change(p);if(value is string){return 0;}Point q=value;return p.x*10+q.x;}
    "#;
    for effort in [0,13] {
        let mut settings=config("[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'");settings.effort.level=effort;
        let built=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",r#"
                const events=[],p={get x(){events.push('x');return 3},get y(){events.push('y');return 4}};
                const changed=library.change(p),opaque=library.opaque(p);
                console.log(JSON.stringify([changed,library.change('s'),library.flip({value:5}),library.flip(true),library.next({x:1,y:2}),library.next(6),library.apply(v=>typeof v==='string'?v+'?':({x:v.x+2,y:v.y}),{x:2,y:5}),opaque,library.assumed({x:8,y:9}),library.internal(),events]));
            "#),"[{\"x\":4,\"y\":4},\"s!\",{\"value\":7},false,{\"x\":1,\"y\":6},9,{\"x\":4,\"y\":5},{\"x\":3,\"y\":4},{\"x\":9,\"y\":9},78,[\"x\",\"y\",\"x\",\"y\"]]\n");
        }
        check_scores(&built);
    }
}

#[test]
fn s4_product_unions_refuse_ambiguous_codecs_and_backing_observations() {
    for source in [
        "struct P{int x;}struct Q{int y;}export P|Q copy(P|Q value){return value;}",
        "struct P{int x;}export P|int[] copy(P|int[] value){return value;}",
        "struct P{int x;}bool array(P|int[] value){return value is int[];}print(array(P{1}));",
        "struct P{int x;}extern class Host{}bool host(P|Host value){return value is Host;}print(host(P{1}));",
        "struct P{int x;}bool same(P|string a,P|string b){return a==b;}print(same(P{1},P{1}));",
    ] {
        let mut settings=config("[policy.tactics]\ninlining='off'\nconstant-folding='off'");settings.effort.level=0;
        assert!(compile_source(source,&settings,ServiceOptions::default()).is_err(),"{source}");
    }
}

#[test]
fn s4_product_unions_development_and_absence_use_the_selected_codec() {
    let source="struct Point{int x;}export Point|int read(Point|int value){return value;}export Point|string|null maybe(Point|string|null value){return value;}";
    let result=compile_source(source,&config("checks='development'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",r#"
            let reads=0;const point=library.read({get x(){reads++;return 9}}),bad=[];
            for(const value of [true,2.5,'wrong',null,{x:'wrong'}]){try{library.read(value);bad.push(false)}catch(e){bad.push(e instanceof TypeError)}}
            console.log(JSON.stringify([point,reads,library.read(3),library.maybe(null),library.maybe('s'),library.maybe({x:4}),bad]));
        "#),"[{\"x\":9},1,3,null,\"s\",{\"x\":4},[true,true,true,true,true]]\n");
    }
}

#[test]
fn s4_public_callbacks_adapt_structs_without_observing_callable_identity() {
    let source=r#"
        struct Point{int x;int y;}
        export Point apply(func(Point)->Point f,Point p){auto alias=f;Point a=alias(p);p.x=90;return alias(a);}
        export int sum(func(Point...)->Point f){Point p=f(Point{2,3},Point{5,7});return p.x+p.y;}
        Point add(Point p){p.x+=1;return p;}
        export Point defaulted(func(Point)->Point f=add){return f(Point{4,8});}
        export int internal(){Point p=apply(add,Point{1,2});return p.x+p.y;}
    "#;
    for effort in [0,13] {
        let mut settings=config("[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'");settings.effort.level=effort;
        let result=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",r#"
                const events=[],p={x:2,y:3};
                function callback(value){events.push([value.x,value.y]);value.x+=4;
                    return {get x(){events.push('x');return value.x},get y(){events.push('y');return value.y+1}};}
                const result=library.apply(callback,p);
                const total=library.sum((...values)=>({x:values.reduce((n,v)=>n+v.x,0),y:values.reduce((n,v)=>n+v.y,0)}));
                let thrown=false;try{library.apply(()=>{throw 'sentinel'},p)}catch(e){thrown=e==='sentinel'}
                console.log(JSON.stringify([result,p,events,total,library.defaulted(),library.defaulted(v=>({x:v.x+10,y:v.y})),library.internal(),thrown,library.apply.length,library.sum.length,library.defaulted.length]));
            "#),"[{\"x\":10,\"y\":5},{\"x\":2,\"y\":3},[[2,3],\"x\",\"y\",[6,4],\"x\",\"y\"],17,{\"x\":5,\"y\":8},{\"x\":14,\"y\":8},5,true,2,1,0]\n");
        }
        check_scores(&result);
    }
}

#[test]
fn s4_public_callbacks_keep_identity_and_escape_boundaries_explicit() {
    for body in [
        "observe(f);return 0;",
        "auto nested=()=>{observe(f);return 0;};return nested();",
        "auto nested=()=>f;observe(nested());return 0;",
        "auto alias=f;auto nested=()=>{auto copy=alias;observe(copy);return 0;};return nested();",
        "auto nested=()=>{JsValue raw=f;observe(raw[\"name\"]);return 0;};return nested();",
        "auto alias=f;observe(alias);return 0;",
    ] {
        let source=format!("struct Point{{int x;}}extern void observe(JsValue f);export int run(func(Point)->int f){{{body}}}");
        let mut settings=config("[policy.tactics]\ninlining='off'\nconstant-folding='off'");settings.effort.level=0;
        let error=compile_source(&source,&settings,ServiceOptions::default()).unwrap_err();
        assert!(error.to_string().contains("public value-struct ABI adaptation"),"{error}");
    }
}

#[test]
fn s4_public_callbacks_capture_private_adapters_in_escaped_nested_closures() {
    let source=r"
        struct Point{int x;}
        export int immediate(func(Point)->int f){auto nested=()=>f(Point{1});return nested();}
        export func(int)->int make(func(Point)->Point f,int offset){
            auto alias=f;
            return (int n)=>{auto nested=(int x)=>{auto local=alias;Point got=local(Point{x});return got.x;};return nested(n+offset);};
        }
        Point add(Point p){p.x+=4;return p;}
        export func()->int defaulted(func(Point)->Point f=add){return ()=>{Point got=f(Point{3});return got.x;};}
        export func()->int replace(func(Point)->Point f){auto nested=()=>{Point got=f(Point{5});return got.x;};f=add;return nested;}
    ";
    for effort in [0,13] {
        let mut settings=config("[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'");settings.effort.level=effort;
        let built=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",r#"
                const events=[];
                const first=library.make(p=>{events.push(p.x);return {get x(){events.push('x');return p.x+10}}},2);
                const second=library.make(p=>({x:p.x*2}),5);
                const delayed=library.make(()=>{throw 'delayed'},0);let thrown=false;
                try{delayed(1)}catch(e){thrown=e==='delayed'}
                console.log(JSON.stringify([library.immediate(p=>p.x+1),first(3),first(4),second(3),events,library.defaulted()(),library.defaulted(p=>({x:p.x+20}))(),library.replace(()=>{throw 'unused'})(),thrown]));
            "#),"[2,15,16,16,[5,\"x\",6,\"x\"],7,23,9,true]\n");
        }
        check_scores(&built);
    }
}

#[test]
fn s4_public_callbacks_capture_development_checks_at_invocation() {
    let source="struct Point{int x;}export func()->int make(func(Point)->Point f){auto alias=f;return ()=>{Point p=alias(Point{4});return p.x;};}";
    let built=compile_source(source,&config("checks='development'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",r#"
            let calls=0,reads=0;const f=library.make(p=>{calls++;return {get x(){reads++;return p.x+3}}});
            const before=[calls,reads],value=f(),bad=[];
            try{library.make(3);bad.push(false)}catch(e){bad.push(e instanceof TypeError)}
            const delayed=library.make(()=>({x:'wrong'}));
            try{delayed();bad.push(false)}catch(e){bad.push(e instanceof TypeError)}
            console.log(JSON.stringify([before,value,calls,reads,bad]));
        "#),"[[0,0],7,1,1,[true,true]]\n");
    }
}

#[test]
fn s4_public_callbacks_development_validates_the_original_values_once() {
    let source="struct Point{int x;}export int run(func(Point)->Point f){Point result=f(Point{3});return result.x;}export int unused(func(Point)->Point f){return 8;}";
    let result=compile_source(source,&config("checks='development'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",r#"
            let reads=0;const valid=library.run(()=>({get x(){reads++;return 7}}));
            const checks=[];for(const bad of [null,3,'wrong',{}]){try{library.unused(bad);checks.push(false)}catch(e){checks.push(e instanceof TypeError)}}
            for(const bad of [null,3,{x:'wrong'},{x:2.5}]){try{library.run(()=>bad);checks.push(false)}catch(e){checks.push(e instanceof TypeError)}}
            console.log(JSON.stringify([valid,reads,checks]));
        "#),"[7,1,[true,true,true,true,true,true,true,true]]\n");
    }
}

#[test]
fn s4_erased_transport_preserves_branch_collection_nested_and_recursive_values() {
    let source=r#"
        struct Point{int x;}
        struct Pair<T>{T first;T second;}
        T pick<T>(T a,T b,bool first){if(first){return a;}return b;}
        T recurse<T>(T value,int depth){if(depth==0){return value;}return recurse(value,depth-1);}
        T[] pair<T>(T a,T b){T[] values=[a,b];values.push(a);return values.slice(0,2);}
        Pair<T> packed<T>(T a,T b){return Pair{a,b};}
        func()->T saved<T>(T value){return ()=>value;}
        int ignore<T>(T value){return 9;}
        T? maybe<T>(T value,bool present){if(present){return value;}return null;}
        void replace<T>(T[] values,T value){values[0]=value;}
        T fromMap<T>(T value){Map<string,T> table=new Map<string,T>();table.set("v",value);return table.get("v")??value;}
        int run(int seed){
            Point a=Point{seed};Point b=Point{seed+1};
            Point p=pick(a,b,false);Point q=recurse(a,3);
            Point[] values=pair(a,b);Pair<Point> box=packed(a,b);
            auto read=saved(a);a.x=90;b.x=91;values[0].x=80;box.first.x=70;
            Point snapshot=read();
            Point[] copied=[q];replace(copied,p);Point absent=maybe(q,false)??p;
            Point present=maybe(q,true)??p;Point mapped=fromMap(q);
            return p.x+q.x+values[0].x+values[1].x+box.first.x+box.second.x+snapshot.x+ignore(a)
                +copied[0].x+absent.x+present.x+mapped.x;
        }
        print(run(3));print(run(10));
    "#;
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        let compiled=compile_source(source,&settings,ServiceOptions {
            objectives:Some(Objectives::All),..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(compiled.javascript(codec).unwrap().javascript(),"",""),"191\n254\n");
        }
        check_scores(&compiled);
    }
}

#[test]
fn s4_erased_transport_refuses_opaque_egress_and_reports_the_source_operation() {
    for body in [
        "observe(value);return value;",
        "JsValue hidden=value;observe(hidden);return value;",
        "unknown hidden=value;observe(hidden as JsValue);return value;",
        "auto bad=()=>{observe(value);};bad();return value;",
        "return JS.assume(input);",
        "if(flag){observe(value);}return value;",
    ] {
        let source=format!("struct Point{{int x;}}extern void observe(JsValue value);extern JsValue input;extern bool flag;T relay<T>(T value){{{body}}}Point p=relay(Point{{7}});print(p.x);");
        let mut settings=config("");settings.effort.level=0;
        let error=compile_source(&source,&settings,ServiceOptions::default()).unwrap_err();
        assert!(error.to_string().contains("closed typed interface"),"{error}\n{source}");
        // The refusal identifies the operation in the source generic body,
        // rather than a synthesized operation at the beginning of the file.
        assert!(!format!("{error:?}").contains("Span { start: 0, end: 0 }"),"{error:?}");
    }
}

#[test]
fn s4_erased_transport_uses_complete_concrete_callback_inputs() {
    let source=r#"
        struct Point{int x;}
        T apply<T>(func(T)->T f,T value){return f(value);}
        Point increment(Point value){value.x+=2;return value;}
        export int run(int n){Point original=Point{n};Point first=apply(increment,original);
            Point second=apply((Point value)=>{value.x+=3;return value;},original);
            return original.x*100+first.x*10+second.x;}
    "#;
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        let compiled=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(compiled.javascript(codec).unwrap().javascript(),"","console.log(library.run(4));console.log(library.run(8));"),"467\n911\n");
        }
    }
    let source="struct Point{int x;}extern Point host(Point value);T apply<T>(func(T)->T f,T value){return f(value);}Point p=apply(host,Point{7});print(p.x);";
    let mut settings=config("");settings.effort.level=0;
    assert!(compile_source(source,&settings,ServiceOptions::default()).is_err());
}

fn config(extra: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "objective.codecs='brotli'\n[javascript]\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{extra}"
    )).unwrap()
}

#[test]
fn s3_product_call_copies_preserve_waiting_values_and_mutation_snapshots() {
    let settings = config(
        "[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'",
    );
    for (source, expected) in [
        ("struct P{int x;int y;}int offset(int n){return n+5;}int sum(P p){return p.x+p.y;}P p=P{2,3};print(offset(p.x)+sum(p));", "12\n"),
        ("struct P{int x;int y;}P p=P{1,2};int change(){p.x=9;return 3;}int read(P q,int n){return q.x+q.y+n;}print(read(p,change()));print(p.x);", "6\n9\n"),
        ("struct P{int x;int y;}int update(P p){p.x=9;return p.x+p.y;}P p=P{1,2};print(update(p));print(p.x);", "11\n1\n"),
    ] {
        let result = compile_source(source, &settings, ServiceOptions { objectives: Some(Objectives::All), ..ServiceOptions::default() }).unwrap();
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(), "", ""), expected);
        }
    }
}

pub(super) fn execute_javascript(javascript: &str, setup: &str, body: &str) -> String {
    let script = format!(
        "{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{body}",
        serde_json::to_string(javascript).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn check_scores(compiled: &ServiceCompilation) {
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        if let Some(artifact) = compiled.javascript(codec) {
            match artifact.sizes().get(codec) {
                Some(size) => assert_eq!(
                    size,
                    crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap()
                ),
                // Only a build without a walk leaves a codec unmeasured
                // (M3.5).
                None => assert!(compiled.report()["search"]["terminal"]["objectives"]
                    .as_array()
                    .is_none_or(Vec::is_empty)),
            }
            assert_eq!(artifact.sha256(), digest(artifact.javascript().as_bytes()));
        }
    }
    assert!(compiled.report().get("backend").is_none());
    assert_eq!(
        compiled.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
    assert!(
        compiled.report()["resources"]["frontend_logical_work"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        compiled.report()["resources"]["frontend_allocation_accounting"],
        "partial"
    );
    let resources = &compiled.report()["resources"];
    let phases = &resources["frontend_phase_accounting"];
    assert_eq!(
        phases["semantic_verification"],
        "pre-admitted work and owned scratch"
    );
    assert_eq!(
        phases["checker"],
        "pre-admitted fixed source-node tables, module facts/interfaces, initialization order and canonical declaration vectors; model stays live through conversion"
    );
    assert_eq!(
        phases["checker_module_graph"],
        "ownership/dependency/order validation work and shared iterative schedule work/storage admitted; exact dependency/import/export capacities"
    );
    assert_eq!(
        phases["checker_analyzer"],
        "scope/narrowing, type-parameter, return, constructor and generator context vector backing admitted; dropped and released per Analyzer without releasing shared declarations"
    );
    assert_eq!(
        phases["checker_binary"],
        "existing iterative expression visits and continuation probes charged; continuation vector backing/growth admitted and released after each expression, including nested overlap and ordinary failures"
    );
    assert_eq!(
        phases["checker_narrowing"],
        "leaf queries and iterative traversal probes charged; pending/answer vector backing and growth overlap admitted and released per query; nested maps/types remain uninstrumented"
    );
    assert_eq!(
        phases["checker_narrowing_reuse"],
        "binary continuations retain syntax-only guard inputs/projections; empty branches are skipped, retained guards re-resolve in the active scope; no result cache or source reassociation"
    );
    assert!(phases["checker_remaining"]
        .as_str()
        .unwrap()
        .contains("uninstrumented"));
    assert_eq!(
        phases["conversion_construction"],
        "pre-admitted graph and temporary storage; retained charges transfer into publication"
    );
    assert_eq!(
        phases["lexer_tokens"],
        "pre-admitted Vec capacities, including overlapping nested fragments"
    );
    assert_eq!(
        phases["lexer_templates"],
        "pre-admitted scanner frames, interpolation vectors/boxed copies and template tables; scanner byte work charged"
    );
    assert_eq!(
        phases["lexer_work"],
        "full input-byte tariff admitted before each lexical stream, including nested fragments; cooperative checks after Logos calls"
    );
    assert_eq!(
        phases["parser_lookahead_work"],
        "each token probe, including EOF, in arrow and type/reference lookahead admitted before inspection"
    );
    assert_eq!(
        phases["source_identity"],
        "inline opaque stamp and node count; included in typed owner capacities, no shared heap allocation"
    );
    if resources["source_buffer_capacity"].is_u64() {
        assert_eq!(
            phases["discovery_parse_arena"],
            "one pre-admitted arena and owned program list, reused for checking"
        );
        assert_eq!(
            phases["main_parse_arena"],
            "not repeated; discovery programs reused"
        );
        assert!(compiled.report()["phases_ns"]["discovery_parse_ns"].is_u64());
        assert!(compiled.report()["phases_ns"]["parse_ns"].is_null());
    } else {
        assert_eq!(phases["discovery_parse_arena"], "not used");
        assert_eq!(phases["main_parse_arena"], "pre-admitted arena backing");
    }
}

#[test]
fn source_service_returns_independently_scored_qualified_winners() {
    let result = compile_source(
        "export int byte(int value){return(value&255)+1;}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "const events=[library.byte.name,library.byte.length,library.byte(-1)];events.push(library.byte({valueOf(){events.push('coerce');return 511;}}));console.log(JSON.stringify(events));"
            ),
            "[\"byte\",1,256,\"coerce\",256]\n"
        );
    }
}

#[test]
fn source_service_publishes_value_struct_functions_through_the_d2_adapter() {
    // Every searched winner, not only the direct artifact, carries the same
    // public object ABI: a fresh object out, one read per field in.
    let result = compile_source(
        "struct Point{int x;int y;}\
         export Point make(int x,int y){return Point{x,y};}\
         export int sum(Point p){return p.x+p.y;}\
         export Point shift(Point p,int by){return Point{p.x+by,p.y+by};}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "const reads=[];const p=library.shift({get x(){reads.push('x');return 1},get y(){reads.push('y');return 2}},3);\
                 console.log(JSON.stringify([p,Object.keys(p),library.sum(library.make(4,5)),library.make(1,2)!==library.make(1,2),reads,\
                 [library.make.name,library.sum.name,library.shift.name],[library.make.length,library.sum.length,library.shift.length]]));"
            ),
            "[{\"x\":4,\"y\":5},[\"x\",\"y\"],9,true,[\"x\",\"y\"],[\"make\",\"sum\",\"shift\"],[2,1,2]]\n"
        );
    }
}

#[test]
fn path_service_publishes_re_exported_value_struct_functions_with_source_names() {
    // A struct function declared in one module and re-exported under an
    // alias by another keeps its source name, as JavaScript does for
    // `export {translate as move}`.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/public-structs");
    let result = compile_path(
        &root.join("entry.lil"),
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "const o=library.origin();const p=library.move(o,3,-4);\
                 console.log(JSON.stringify([o,p,library.manhattan(o,p),[library.move.name,library.origin.name,library.manhattan.name],[library.move.length,library.manhattan.length],'translate' in library]));"
            ),
            "[{\"x\":0,\"y\":0},{\"x\":3,\"y\":-4},7,[\"translate\",\"origin\",\"manhattan\"],[3,2],false]\n"
        );
    }
}

#[test]
fn independent_winner_handoff_and_all_optional_off_keep_source_literals_and_api() {
    let mut config = config("");
    config.policy = Some(PolicyConfig {
        tactics: TacticId::ALL
            .into_iter()
            .map(|tactic| (tactic, TacticPermission::Off))
            .collect(),
        ..PolicyConfig::default()
    });
    let result = compile_source(
        "export string answer(){return \"kept\";}",
        &config,
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    assert_eq!(result.report()["search"]["proposals"], 0);
    // Even equal bytes have independent policy qualifications. Each codec's
    // disabled search must retain its own direct artifact and public ABI.
    for (index, codec) in [Objective::Raw, Objective::Gzip, Objective::Brotli]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            result.report()["search"]["objectives"][index]["proposals"],
            0
        );
        let mut one_config = config.clone();
        one_config.objective.codecs = vec![codec];
        let one = compile_source(
            "export string answer(){return \"kept\";}",
            &one_config,
            ServiceOptions::default(),
        )
        .unwrap();
        assert_eq!(
            result.javascript(codec).unwrap().javascript(),
            one.javascript(codec).unwrap().javascript()
        );
    }
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "console.log(library.answer());"
            ),
            "kept\n"
        );
    }
}

#[test]
fn path_service_preserves_original_module_initialization_callbacks_and_public_objects() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/modules-javascript");
    let result = compile_path(
        &root.join("entry.lil"),
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap();
    check_scores(&result);
    assert_eq!(result.report()["shape"]["modules"], 6);
    assert_eq!(
        result.report()["inputs"]["modules"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let setup = format!(
        "const events=[];{}",
        include_str!("program/fixtures/modules-javascript/setup.js")
    );
    let body = format!(
        "{}\nconsole.log(JSON.stringify(events));",
        include_str!("program/fixtures/modules-javascript/host.js")
    );
    let output = execute_javascript(
        result.javascript(Objective::Brotli).unwrap().javascript(),
        &setup,
        &body,
    );
    let actual: Value = serde_json::from_str(&output).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "program/fixtures/modules-javascript/expected.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn integrated_original_modules_execute_through_fast_and_searched_public_service() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/integrated-architecture");
    let expected: Value = serde_json::from_str(include_str!(
        "program/fixtures/integrated-architecture/expected.json"
    ))
    .unwrap();
    let setup = format!(
        "const events=[];{}",
        include_str!("program/fixtures/integrated-architecture/setup.js")
    );
    let body = format!(
        "{}\nprocess.stdout.write(JSON.stringify(events));",
        include_str!("program/fixtures/integrated-architecture/host.js")
    );
    for search in [false, true] {
        let mut config = config("");
        if !search {
            config.javascript.candidate_search = crate::config::CandidateSearch::Off;
        }
        let result = compile_path(
            &root.join("entry.lil"),
            &config,
            ServiceOptions {
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        check_scores(&result);
        assert!(result.report()["shape"]["modules"].as_u64().unwrap() > 6);
        if !search {
            assert_eq!(result.report()["search"]["proposals"], 0);
        }
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let actual: Value = serde_json::from_str(&execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                &setup,
                &body,
            ))
            .unwrap();
            assert_eq!(actual, expected, "search={search}, codec={codec:?}");
        }
    }
    let native = compile_path(
        &root.join("native-entry.lil"),
        &config(""),
        ServiceOptions {
            target: ServiceTarget::Native,
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        execute_native(native.native_c().unwrap()),
        include_str!("program/fixtures/integrated-architecture/native.expected.out")
    );
    assert_eq!(native.report()["resources"]["codec_work"], 0);
}

#[test]
fn service_rejects_unknown_runtime_cost_and_unsupported_source_without_fallback() {
    let result = compile_source(
        "export int answer(){return 17;}",
        &config("[policy.constraints]\nmax_startup_work=0"),
        ServiceOptions::default(),
    );
    assert!(result.is_err(), "unknown is not zero");
    let result = compile_source(
        "struct P{int x;}export void add(P[] items){items.push(P{1});}",
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap_err();
    // An exported array of value structs that the function mutates has no
    // D2 adapter, so the target refuses it.
    assert_eq!(result.phase, "javascript");
    assert!(
        result
            .message
            .contains("public value-struct ABI adaptation"),
        "{result}"
    );
    // The target session no longer holds source text, so a formation refusal
    // carries its span in the message but no rendered diagnostic (011 work).
    assert!(result.diagnostic.is_none());
    // Multi-file delivery of one source module is its entry file alone.
    for mode in ["preserve-modules", "split"] {
        let result = compile_source(
            "export int answer(){return 17;}",
            &config(&format!("[delivery]\nmode='{mode}'")),
            ServiceOptions::default(),
        )
        .unwrap();
        // One entry without `import()`: at most its one file (design §7.6).
        assert!(result.javascript(Objective::Brotli).unwrap().files().len() <= 1);
    }
}

#[test]
fn resource_owner_exists_before_discovery_and_parse() {
    let options = ServiceOptions {
        logical_work: 0,
        ..ServiceOptions::default()
    };
    let source = compile_source("not valid syntax", &config(""), options).unwrap_err();
    assert_eq!(source.phase, "frontend resources");
    let path = compile_path(Path::new("/does-not-exist.lil"), &config(""), options).unwrap_err();
    assert_eq!(path.phase, "frontend resources");
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-service-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn execute_native(c: &str) -> String {
    let scratch = Scratch::new();
    let executable = scratch.0.join("program");
    let mut config = ProjectConfig::default();
    config.target.native.objective = crate::config::NativeObjective::Balanced;
    config.target.native.compiler = std::env::var_os("LILSCRIPT_NATIVE_CC").map(PathBuf::from);
    crate::native_toolchain::NativeToolchain::resolve(&config, None).unwrap()
        .compile(c, None, &executable).unwrap();
    let output = Command::new(&executable).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn bounded_loop_folding_obeys_configuration_for_javascript_and_native() {
    let source = "int calculate(int seed){int result=seed;for(int i=0;i<4;i+=1){result=result*3+1;}return result;}print(calculate(1));";
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        for permission in [TacticPermission::Off, TacticPermission::On] {
            let mut config = ProjectConfig::default();
            config.objective.codecs = vec![codec];
            config.effort.level = 0;
            config.policy = Some(PolicyConfig {
                tactics: [(TacticId::ConstantFolding, permission)].into(),
                ..PolicyConfig::default()
            });
            let result = compile_source(
                source,
                &config,
                ServiceOptions {
                    target: ServiceTarget::All,
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            let folded = result.report()["phases_ns"]["rules"]["folded_calls"]
                .as_u64()
                .unwrap();
            assert_eq!(
                folded > 0,
                permission == TacticPermission::On,
                "{}",
                result.report()
            );
            assert_eq!(
                execute_javascript(result.javascript(codec).unwrap().javascript(), "", ""),
                "121\n"
            );
            assert_eq!(execute_native(result.native_c().unwrap()), "121\n");
        }
    }
}

#[test]
fn constant_scalar_methods_obey_toml_and_match_native_and_javascript() {
    let source = r#"
        int truncated(float value){return value.toInt();}
        int points(string value){return value.codePointLength();}
        print(truncated(4294967297.75));print(truncated(-4294967297.75));
        print(truncated(0.0/0.0));print(truncated(1.0/0.0));
        print(points("A😀B"));print(points(""));
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for (permission, pristine) in [("off", false), ("on", false), ("off", true), ("on", true)] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n\
                 javascript.assume_pristine_builtins={pristine}\n\
                 [policy.tactics]\nconstant-folding='{permission}'\n\
                 inlining='off'\ncall-specialization='off'"
            ))
            .unwrap();
            let result = compile_source(
                source,
                &config,
                ServiceOptions {
                    target: ServiceTarget::All,
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            let folded = result.report()["phases_ns"]["rules"]["folded_calls"]
                .as_u64()
                .unwrap();
            assert_eq!(folded > 0, permission == "on", "{}", result.report());
            let objective = config.objective.codecs[0];
            assert_eq!(
                execute_javascript(result.javascript(objective).unwrap().javascript(), "", ""),
                "1\n-1\n0\n0\n3\n0\n"
            );
            assert_eq!(
                execute_native(result.native_c().unwrap()),
                "1\n-1\n0\n0\n3\n0\n"
            );
        }
    }
}

#[test]
fn finite_values_and_default_transport_obey_toml_in_both_targets() {
    let tail = "print(99);".repeat(70);
    let source = format!("bool classify(int value){{if(value>0){{return true;}}else{{{tail}return false;}}}}print(classify(1));print(classify(2));void report(int tag,int? value=null){{print(tag);print(value);}}for(int i=0;i<2;i+=1){{report(i);report(i,7);}}int current=1;int snapshot=current;current=2;print(snapshot);print(current);");
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["on", "off"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nconstant-folding='{permission}'\ninlining='off'\ncall-specialization='off'"
            )).unwrap();
            let result = compile_source(
                &source,
                &config,
                ServiceOptions {
                    target: ServiceTarget::All,
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            let receipt = &result.report()["phases_ns"]["rules"];
            for field in [
                "set_folds",
                "default_arguments_omitted",
                "forwarded_definitions",
            ] {
                assert_eq!(
                    receipt[field].as_u64().unwrap() > 0,
                    permission == "on",
                    "{field}: {receipt}"
                );
            }
            let expected = "true\ntrue\n0\nnull\n0\n7\n1\nnull\n1\n7\n1\n2\n";
            assert_eq!(
                execute_javascript(
                    result
                        .javascript(config.objective.codecs[0])
                        .unwrap()
                        .javascript(),
                    "",
                    ""
                ),
                expected
            );
            assert_eq!(execute_native(result.native_c().unwrap()), expected);
        }
    }
}

#[test]
fn folded_nullable_literals_keep_the_native_tagged_representation() {
    let source = r#"
        T? maybe<T>(bool present,T value){if(present){return value;}return null;}
        int? integer=maybe(true,7);
        float? floating=maybe(true,-0.0);
        bool? boolean=maybe(true,true);
        string? text=maybe(true,"literal");
        string? empty=maybe(true,"");
        int? absent=maybe(false,8);
        print(integer);print(floating);print(boolean);print(text);print(empty);print(absent);
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            target: ServiceTarget::All,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert!(
        result.report()["phases_ns"]["rules"]["folded_calls"]
            .as_u64()
            .unwrap()
            > 0
    );
    let expected = "7\n-0\ntrue\nliteral\n\nnull\n";
    assert_eq!(
        execute_javascript(
            result.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            ""
        ),
        expected
    );
    assert_eq!(execute_native(result.native_c().unwrap()), expected);
}

#[test]
fn numeric_primitive_folding_keeps_host_lookups_and_argument_effects() {
    let source = r#"
        extern float next();
        int truncate(float value){return value.toInt();}
        print(truncate(4294967297.5));
        print(truncate(next()));
        print(2.5.floor());
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["off", "on"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\njavascript.assume_pristine_builtins=false\n[policy.tactics]\nconstant-folding='{permission}'\ninlining='off'\ncall-specialization='off'"
            )).unwrap();
            let result = compile_source(source, &config, ServiceOptions::default()).unwrap();
            let javascript = result
                .javascript(config.objective.codecs[0])
                .unwrap()
                .javascript();
            assert_eq!(execute_javascript(javascript,
                "let calls=0;globalThis.next=()=>{calls++;return -4294967297.5};Math.floor=()=>91;Number.prototype.valueOf=()=>{throw Error('primitive conversion must not call valueOf')};",
                "console.log(calls);"), "1\n-1\n91\n1\n");
            assert_eq!(
                result.report()["phases_ns"]["rules"]["folded_calls"]
                    .as_u64()
                    .unwrap()
                    > 0,
                permission == "on",
                "{}",
                result.report()
            );
        }
    }
}

#[test]
fn constant_scalar_code_point_method_keeps_an_observable_host_iterator() {
    let config: ProjectConfig = toml::from_str(
        "objective.codecs='raw'\neffort.level=13\n\
         javascript.assume_pristine_builtins=false\n\
         [policy.tactics]\nconstant-folding='on'",
    )
    .unwrap();
    let result = compile_source(
        "print(\"abc\".codePointLength());",
        &config,
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(result.report()["phases_ns"]["rules"]["folded_calls"], 0);
    let javascript = result.javascript(Objective::Raw).unwrap().javascript();
    assert_eq!(execute_javascript(javascript, "", ""), "3\n");
    assert_eq!(
        execute_javascript(
            javascript,
            "String.prototype[Symbol.iterator]=function*(){yield 'changed';};",
            ""
        ),
        "1\n"
    );
}

#[test]
fn strict_target_inlining_obeys_toml_for_imported_struct_helpers() {
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("helpers.lil"),
        "export struct P{int value;}int read(P p){return p.value+1;}export int sum(P p){return read(p)+2;}").unwrap();
    let entry = scratch.0.join("main.lil");
    std::fs::write(
        &entry,
        "import {P,sum} from \"./helpers\";extern int next();print(sum(P{next()}));",
    )
    .unwrap();
    for codec in ["raw", "gzip", "brotli"] {
        for module in [false, true] {
            let mut lengths = Vec::new();
            for permission in ["off", "on"] {
                let config: ProjectConfig = toml::from_str(&format!(
                    "objective.codecs='{codec}'\neffort.level=13\n\
                     [policy.tactics]\ninlining='{permission}'\nidentifier-mangling='off'"
                ))
                .unwrap();
                let result = compile_path(
                    &entry,
                    &config,
                    ServiceOptions {
                        preserve_root_exports: module,
                        ..ServiceOptions::default()
                    },
                )
                .unwrap();
                let javascript = result
                    .javascript(config.objective.codecs[0])
                    .unwrap()
                    .javascript();
                let setup = "let calls=0;globalThis.next=()=>{if(++calls!==1)throw Error('duplicate');return 7;};";
                let output = if module {
                    execute_javascript(javascript, setup, "")
                } else {
                    let output = Command::new("node")
                        .args(["-e", &format!("{setup}\n{javascript}")])
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "{}\n{javascript}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    String::from_utf8(output.stdout).unwrap()
                };
                assert_eq!(output, "10\n");
                lengths.push(javascript.len());
            }
            assert!(
                lengths[1] < lengths[0],
                "{codec} module={module}: {lengths:?}"
            );
        }
    }
}

#[test]
fn scalar_objects_public_construction_preserves_callback_initialization_errors() {
    let source = "class State{int early;int late;init(int a,int b){this.early=a;this.late=b;}}\n\
        extern int initialize(func()->int read);\n\
        int make(){State state=new State(1,initialize(()=>state.early));return state.early;}\n\
        try{print(make());}catch(auto error){print(99);}";
    let after = "try{console.log(saved())}catch(error){console.log(error.name)}";
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["off", "on"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nscalar-replacement='{permission}'"
            )).unwrap();
            let result = compile_source(source, &config, ServiceOptions::default()).unwrap();
            let javascript = result
                .javascript(config.objective.codecs[0])
                .unwrap()
                .javascript();
            assert_eq!(execute_javascript(javascript,
                "globalThis.saved=null;globalThis.initialize=read=>{saved=read;try{console.log(read())}catch(error){console.log('during:'+error.name)}return 2;};",
                after), "during:ReferenceError\n1\n1\n", "{codec}/{permission}\n{javascript}");
            assert_eq!(execute_javascript(javascript,
                "globalThis.saved=null;globalThis.initialize=read=>{saved=read;throw Error('initializer');};",
                after), "99\nReferenceError\n", "{codec}/{permission}\n{javascript}");
        }
    }
}

#[test]
fn q4_service_reports_lazy_data_availability_and_explicit_permissions() {
    let result = compile_source(
        "print(7);",
        &config("[policy.tactics]\nrecurring-reconstruction='on'\nstartup-reconstruction='on'"),
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(
        execute_javascript(
            result.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            ""
        ),
        "7\n"
    );
    let report = result.report();
    assert!(!report["policy_diagnostics"]["javascript"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap().contains("recurring-reconstruction")));
    assert!(report["javascript_policy"].get("diagnostics").is_none());
    let tactic =
        &report["javascript_policy"]["tactics"][TacticId::RecurringReconstruction as usize];
    assert_eq!(tactic["state"]["permission"], "on");
    assert_eq!(tactic["available"], true);
    assert_eq!(tactic["state"]["enabled"], true);
}

#[test]
fn application_iife_owns_private_roots_and_independent_loads_under_each_objective() {
    let source = "extern int next();extern void retain(func()->int callback);int shared=next();retain(()=>shared);";
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let mut config = config("[policy.tactics]\nidentifier-mangling='off'");
        config.objective.codecs = vec![codec];
        let result = compile_source(
            source,
            &config,
            ServiceOptions {
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        check_scores(&result);
        let artifact = result.javascript(codec).unwrap();
        assert_eq!(
            result.report()["javascript_policy"]["contract"]["delivery"]["format"],
            "iife"
        );
        assert_eq!(artifact.details()["execution"], "Script");
        let script = format!(
            r#"
            const vm=require('node:vm');const callbacks=[];let value=0;
            const context=vm.createContext({{next:()=>++value,retain:f=>callbacks.push(f)}});
            vm.runInContext('let shared=91;var a=18',context);
            const before=Object.keys(context).sort();const code={};
            vm.runInContext(code,context);vm.runInContext(code,context);
            const actual=callbacks.map(f=>f());
            if(JSON.stringify(actual)!=='[1,2]' ||
               JSON.stringify(before)!==JSON.stringify(Object.keys(context).sort()) ||
               vm.runInContext('shared',context)!==91)throw Error('private scope leaked');
            console.log(JSON.stringify(actual));
        "#,
            serde_json::to_string(artifact.javascript()).unwrap()
        );
        let oracle = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            oracle.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&oracle.stderr),
            artifact.javascript()
        );
        assert_eq!(oracle.stdout, b"[1,2]\n");
    }
}

#[test]
fn private_wrapper_preserves_classic_this_and_accepts_explicit_bare() {
    let source = "extern JsValue this;extern void invoke(func()->void callback);extern bool isGlobal(JsValue value);void probe(){print(isGlobal(this));}invoke(probe);";
    for format in [
        crate::config::JavaScriptFormat::Iife,
        crate::config::JavaScriptFormat::Bare,
    ] {
        let mut config = config("");
        config.target.javascript.format = format;
        let result = compile_source(
            source,
            &config,
            ServiceOptions {
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        let artifact = result.javascript(Objective::Brotli).unwrap();
        assert_eq!(
            artifact.javascript().starts_with("(()=>{"),
            format == crate::config::JavaScriptFormat::Iife
        );
        let script = format!(
            "const vm=require('node:vm');vm.runInThisContext({}+{});",
            serde_json::to_string(
                "globalThis.invoke=f=>f();globalThis.isGlobal=value=>value===globalThis;"
            )
            .unwrap(),
            serde_json::to_string(artifact.javascript()).unwrap()
        );
        let output = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"true\n");
    }
}

#[test]
fn script_containers_refuse_module_exports_and_imports() {
    for format in ["iife", "bare"] {
        let settings = config(&format!("[target.javascript]\nformat='{format}'"));
        let error = compile_source(
            "export int answer(){return 7;}",
            &settings,
            ServiceOptions::default(),
        )
        .unwrap_err();
        assert!(error.message.contains("delivery.global") || error.message.contains("application script"), "{error}");
    }
    let error = compile_source(
        "print(7);",
        &config("[target.javascript]\nformat='esm'"),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        error.message.contains("requires module execution"),
        "{error}"
    );
    let scratch = Scratch::new();
    let entry = scratch.0.join("entry.lil");
    std::fs::write(
        &entry,
        r#"import extern { next } from "host";extern int next();print(next());"#,
    )
    .unwrap();
    let error = compile_path(
        &entry,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert!(error.message.contains("static imports require"), "{error}");
}

#[test]
fn native_and_all_share_checked_meaning_without_native_codec_work() {
    let source = "int twice(int value){return value*2;}print(twice(21));";
    let javascript = compile_source(
        source,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for target in [ServiceTarget::Native, ServiceTarget::All] {
        let result = compile_source(
            source,
            &config(""),
            ServiceOptions {
                target,
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(execute_native(result.native_c().unwrap()), "42\n");
        if target == ServiceTarget::Native {
            assert_eq!(result.report()["resources"]["codec_work"], 0);
            assert!(result.javascript(Objective::Brotli).is_none());
        } else {
            check_scores(&result);
            let standalone_peak = javascript.report()["resources"]["peak_retained_bytes"]
                .as_u64()
                .unwrap();
            let combined_peak = result.report()["resources"]["peak_retained_bytes"]
                .as_u64()
                .unwrap();
            assert!(
                combined_peak >= standalone_peak + result.native_c().unwrap().len() as u64,
                "native output stays admitted throughout JavaScript search"
            );
            let output = Command::new("node")
                .args([
                    "-e",
                    result.javascript(Objective::Brotli).unwrap().javascript(),
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(String::from_utf8(output.stdout).unwrap(), "42\n");
        }
    }
}

#[test]
fn native_and_all_reject_unknown_required_runtime_evidence() {
    for target in [ServiceTarget::Native, ServiceTarget::All] {
        let error = compile_source(
            "print(42);",
            &config("[policy.constraints]\nmax_startup_work=0"),
            ServiceOptions {
                target,
                ..ServiceOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, "native");
    }
}

#[test]
fn searched_string_records_respect_scalar_permissions_under_every_objective() {
    let source = r#"
        export func(string)->string make(string seed) {
            Record<string> state=record{value:seed};
            return (string suffix)=>{
                state.value=(state.value??"")+suffix;
                return state.value??"";
            };
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        let mut sizes = Vec::new();
        for permission in ["off", "on"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nscalar-replacement='{permission}'"
            )).unwrap();
            let compiled = compile_source(source, &config, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let objective = match codec {
                "raw" => Objective::Raw,
                "gzip" => Objective::Gzip,
                _ => Objective::Brotli,
            };
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(
                execute_javascript(artifact.javascript(), "", "const a=library.make('a'),b=library.make('b');console.log(a('!'),b('?'),a('.'),a!==b,library.make.length);"),
                "a! b? a!. true 1\n"
            );
            if permission == "off" {
                assert!(
                    artifact.javascript().contains("__proto__:"),
                    "{}",
                    artifact.javascript()
                );
                assert!(!artifact.details()["output"]["rules"]["scalar_replacement"]
                    .as_bool()
                    .unwrap());
            } else {
                assert!(
                    !artifact.javascript().contains("__proto__:"),
                    "{}",
                    artifact.javascript()
                );
            }
            sizes.push(artifact.sizes().get(objective).unwrap());
        }
        assert!(
            sizes[1] <= sizes[0],
            "{codec}: scalar alternatives regressed {sizes:?}"
        );
    }
}

#[test]
fn searched_nullish_assignment_preserves_stores_and_value_copies_at_each_syntax_floor() {
    let source = r#"
        struct Point { int x; }
        export int fill((int?)[] values,int index){return values[index]??=7;}
        export int copies(){
            Point initial=Point{3};Point? slot=null;
            Point first=slot??=initial;first.x=4;initial.x=5;
            Point second=slot??=initial;
            return first.x*100+second.x*10+initial.x;
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for floor in ["es2015", "es2022"] {
            for permission in ["off", "on"] {
                let configured: ProjectConfig = toml::from_str(&format!(
                    "objective.codecs='{codec}'\n[javascript]\necmascript='{floor}'\n[policy.tactics]\ntarget-compaction='{permission}'"
                )).unwrap();
                let compiled =
                    compile_source(source, &configured, ServiceOptions::default()).unwrap();
                check_scores(&compiled);
                let objective = match codec {
                    "raw" => Objective::Raw,
                    "gzip" => Objective::Gzip,
                    _ => Objective::Brotli,
                };
                let javascript = compiled.javascript(objective).unwrap().javascript();
                if floor == "es2015" {
                    assert!(!javascript.contains("??"), "{javascript}");
                }
                assert_eq!(
                    execute_javascript(
                        javascript,
                        "",
                        r#"
                    const events=[], values=new Proxy([null,0],{
                        get:(a,k)=>{events.push('get:'+k);return a[k]},
                        set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}
                    });
                    console.log(library.fill(values,0),library.fill(values,1),library.fill(values,0));
                    console.log(events.join(','));console.log(library.copies());
                "#
                    ),
                    "7 0 7\nget:0,set:0:7,get:1,get:0\n435\n",
                    "{codec}/{floor}/{permission}"
                );
            }
        }
    }
}

#[test]
fn searched_typed_record_spread_keeps_value_copies_and_all_objective_contracts() {
    let source = r#"
        struct Point{int x;int y;}
        export string run(){
            Point seed=Point{1,2};
            Record<Point> original=record{point:seed};
            Record<Point> copy=record{...original};
            seed.x=8;Point first=original.point??Point{0,0};first.x=7;
            original.point=first;first.y=9;
            Point saved=copy.point??Point{0,0};Point changed=original.point??Point{0,0};
            return `${saved.x},${saved.y},${changed.x},${changed.y}`;
        }
        export int order(){
            Record<int> source=record{value:1};
            func()->int change=()=>{source.value=9;return 2;};
            Record<int> copy=record{...source,other:change(),...record{third:3}};
            return (copy.value??0)*100+(copy.other??0)*10+(copy.third??0);
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for floor in ["es2015", "es2022"] {
            for compact in ["on", "off"] {
                let configured: ProjectConfig = toml::from_str(&format!(
                    "objective.codecs='{codec}'\neffort.level=13\n\
                     [javascript]\necmascript='{floor}'\n\
                     [policy.tactics]\ntarget-compaction='{compact}'"
                ))
                .unwrap();
                let compiled =
                    compile_source(source, &configured, ServiceOptions::default()).unwrap();
                check_scores(&compiled);
                let objective = match codec {
                    "raw" => Objective::Raw,
                    "gzip" => Objective::Gzip,
                    _ => Objective::Brotli,
                };
                let javascript = compiled.javascript(objective).unwrap().javascript();
                assert_eq!(
                    execute_javascript(
                        javascript,
                        "",
                        "console.log(library.run(),library.order());"
                    ),
                    "1,2,7,2 123\n"
                );
            }
        }
    }
}

#[test]
fn stable_rule_scheduling_preserves_searched_artifacts_and_behavior() {
    let source = r#"
        int add(int value){return value+1;}
        export int run(int input){
            Record<int> pair=record{left:input,right:add(input)};
            int total=0;
            for(int i=0;i<4;i+=1){
                if(i==1){continue;}
                total+=(pair.left??0)+(pair.right??0);
            }
            if(input<0){return total+1;}return total;
        }
        export func(int)->int make(int initial){
            Record<int> state=record{value:initial};
            return (int step)=>{state.value=(state.value??0)+step;return state.value??0;};
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        let configured: ProjectConfig =
            toml::from_str(&format!("objective.codecs='{codec}'\neffort.level=13")).unwrap();
        let objective = match codec {
            "raw" => Objective::Raw,
            "gzip" => Objective::Gzip,
            _ => Objective::Brotli,
        };
        let runs = [false, true].map(|dense| {
            let _audit = dense.then(crate::schedule::DenseAudit::new);
            let compiled = compile_source(source, &configured, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(execute_javascript(artifact.javascript(), "",
                "const a=library.make(2),b=library.make(7);console.log(library.run(3),library.run(-1),a(3),b(4),a(1));"),
                "21 -2 5 11 6\n");
            (artifact.javascript().to_owned(), artifact.sizes, compiled.report()["phases_ns"]["rules"].clone())
        });
        assert_eq!(runs[0], runs[1], "{codec}");
    }
}

#[test]
fn check_service_diagnoses_detached_primitive_methods_before_target_selection() {
    for source in [
        "auto method=\"text\".charCodeAt;",
        "Map<string,int> values=new Map<string,int>();auto method=values.get;",
        "auto values=new Set<int>();auto method=values.has;",
        "string? value=null;auto method=value?.charCodeAt;",
    ] {
        let error = check_source(source, &ProjectConfig::default()).unwrap_err();
        assert!(
            error.message.contains("called through its receiver"),
            "{error}"
        );
        for target in [ServiceTarget::JavaScript, ServiceTarget::Native] {
            let error = compile_source(
                source,
                &ProjectConfig::default(),
                ServiceOptions {
                    target,
                    ..ServiceOptions::default()
                },
            )
            .unwrap_err();
            assert!(
                error.message.contains("called through its receiver"),
                "{error}"
            );
        }
    }
    check_source(
        "extern JsValue object;auto method=object.method;",
        &ProjectConfig::default(),
    )
    .unwrap();
    check_source(
        "struct Holder{func(int)->int callback;}Holder h=Holder{(int v)=>v+1};\
        auto callback=h.callback;print(callback(2));",
        &ProjectConfig::default(),
    )
    .unwrap();
}

#[test]
fn searched_local_read_order_is_independently_judged_and_reports_its_veto() {
    let parameters = (0..60)
        .map(|index| format!("int input{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let expression = ["input59"; 8].join("+");
    let source = format!("export int hot({parameters}){{return {expression};}}");
    for codec in ["raw", "gzip", "brotli"] {
        for allowed in [true, false] {
            let configured: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=14\n[policy.tactics]\n\
                 naming-search='{}'\nnaming-alphabet='off'\ntarget-compaction='off'\n\
                 inlining='off'\ncall-specialization='off'\nconstant-folding='off'",
                if allowed { "on" } else { "off" }
            ))
            .unwrap();
            let compiled = compile_source(&source, &configured, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let objective = match codec {
                "raw" => Objective::Raw,
                "gzip" => Objective::Gzip,
                _ => Objective::Brotli,
            };
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(execute_javascript(artifact.javascript(), "",
                "console.log(library.hot(...Array.from({length:60},(_,i)=>i)),library.hot.name,library.hot.length);"),
                "472 hot 60\n");
            let report = compiled.report();
            let objectives = report["search"]["terminal"]["objectives"]
                .as_array()
                .unwrap();
            let trials = objectives[0]["joint_trials"].as_array().unwrap();
            let local = trials
                .iter()
                .filter(|trial| trial["name"] == "naming:local-read-order")
                .collect::<Vec<_>>();
            assert!(!local.is_empty(), "{report}");
            if allowed {
                assert!(local.iter().any(|trial| matches!(
                    trial["outcome"].as_str(),
                    Some("kept" | "rejected" | "identical")
                )));
                if codec == "raw" {
                    assert_eq!(artifact.details["output"]["local_read_order"], true);
                }
            } else {
                assert!(local.iter().all(|trial| trial["outcome"] == "vetoed"));
                assert_eq!(artifact.details["output"]["local_read_order"], false);
            }
        }
    }
}

#[test]
fn searched_private_properties_are_independent_and_preserve_each_objective_winner() {
    let source = include_str!("program/fixtures/private-fields.lil");
    for codec in ["raw", "gzip", "brotli"] {
        let objective = match codec {
            "raw" => Objective::Raw,
            "gzip" => Objective::Gzip,
            _ => Objective::Brotli,
        };
        let mut sizes = Vec::new();
        for permission in ["off", "on"] {
            let configured: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=14\n[policy.tactics]\nproperty-mangling='{permission}'\nscalar-replacement='off'\ninlining='off'"
            )).unwrap();
            let compiled = compile_source(source, &configured, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(
                execute_javascript(
                    artifact.javascript(),
                    "",
                    "const a=library.make(2),b=library.make(10);console.log(a(3),b(1),a(-2));"
                ),
                "15 21 13\n"
            );
            sizes.push(artifact.sizes.get(objective).unwrap());
            let report = compiled.report();
            let trials = report["search"]["terminal"]["objectives"][0]["joint_trials"]
                .as_array()
                .unwrap();
            let properties = trials
                .iter()
                .filter(|t| t["name"] == "properties:private-fields")
                .collect::<Vec<_>>();
            // One protected final trial, plus any deferred combinations.
            assert!(!properties.is_empty(), "{report}");
            if permission == "off" {
                assert!(properties.iter().all(|trial| trial["outcome"] == "vetoed"));
                assert_eq!(artifact.details["output"]["property_mangling"], false);
            } else if codec == "raw" {
                assert_eq!(properties[0]["outcome"], "kept");
                assert_eq!(artifact.details["output"]["property_mangling"], true);
            }
        }
        assert!(sizes[1] <= sizes[0], "{codec}: {sizes:?}");
    }
    for (identifiers, compact, expected) in [("off", "on", true), ("on", "off", false)] {
        let configured: ProjectConfig = toml::from_str(&format!(
            "objective.codecs='raw'\neffort.level=14\n[policy.tactics]\nproperty-mangling='on'\nidentifier-mangling='{identifiers}'\ntarget-compaction='{compact}'\nscalar-replacement='off'\ninlining='off'"
        )).unwrap();
        let compiled = compile_source(source, &configured, ServiceOptions::default()).unwrap();
        check_scores(&compiled);
        let artifact = compiled.javascript(Objective::Raw).unwrap();
        assert_eq!(artifact.details["output"]["property_mangling"], expected);
        assert_eq!(
            execute_javascript(
                artifact.javascript(),
                "",
                "console.log(library.make(2)(3));"
            ),
            "15\n"
        );
    }
}

#[test]
fn scoped_search_and_default_service_share_winners_handoff_and_budget() {
    let source =
        "int byte(int value){return value&255;}export int answer(int value){return byte(value)+1;}";
    let config = config("");
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let default = compile_source(source, &config, options).unwrap();
    let (batch, finished) = with_checked_source(source, &config, options, |session| {
        assert!(session.phases()["frontend_release_ns"].is_u64());
        assert_eq!(session.compilation().checkpoint_count(), 1);
        let mut baselines = 0;
        let batch = session
            .search_javascript(session.source(), |observation| {
                baselines += usize::from(observation.baseline);
            })
            .unwrap();
        assert_eq!(baselines, 3);
        batch
    })
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let scoped = batch.javascript(codec).unwrap();
        assert_eq!(
            scoped.javascript(),
            default.javascript(codec).unwrap().javascript()
        );
        assert_eq!(scoped.sizes(), default.javascript(codec).unwrap().sizes());
        let canonical_details = |artifact: &ServiceJavaScript| {
            let mut details = artifact.details().clone();
            let semantic = details["semantic"].as_object_mut().unwrap();
            assert!(semantic.remove("snapshot_identity").unwrap().is_string());
            assert!(semantic.remove("meaning_identity").unwrap().is_string());
            details
        };
        assert_eq!(
            canonical_details(scoped),
            canonical_details(default.javascript(codec).unwrap())
        );
    }
    assert_eq!(batch.report(), &default.report()["search"]);
    assert_eq!(finished.report["resources"], default.report()["resources"]);
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

#[test]
fn scoped_source_edit_keeps_one_owner_and_delivers_qualified_parent_and_child() {
    use crate::program::{CellBinding, CellId, Constant, OpId, OperationKind};

    let ((baseline, changed), finished) = with_checked_source(
        "export int answer(){return 17;}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
        |session| {
            let source = session.source();
            let (unit, operation, revision) = {
                let view = session.compilation().view(source).unwrap();
                let unit = (0..view.cell_count())
                    .find_map(|index| {
                        let cell = view.cell(CellId::from_index(index).unwrap()).unwrap();
                        match cell.binding {
                            CellBinding::Function(unit) if cell.name == "answer" => Some(unit),
                            _ => None,
                        }
                    })
                    .unwrap();
                let operation = view
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .position(|op| {
                        matches!(op.kind, OperationKind::Constant(Constant::Integer(17)))
                    })
                    .unwrap();
                (
                    unit,
                    OpId::from_index(operation).unwrap(),
                    view.unit_revision(unit).unwrap(),
                )
            };
            let candidate = {
                let (owner, _, policy) = session.parts_mut();
                owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let baseline = session.render_javascript(candidate).unwrap();
            let kind = OperationKind::Constant(Constant::Integer(19));
            let operations = [OperationPatch {
                operation,
                kind: &kind,
                operands: &[],
            }];
            let patches = [UnitPatch {
                unit,
                expected_revision: revision,
                operations: &operations,
                places: &[],
            }];
            let changed = session
                .compilation_mut()
                .edit_source(source, &patches, WorkDomain::Baseline)
                .unwrap();
            let changed = {
                let (owner, _, policy) = session.parts_mut();
                owner
                    .direct_javascript(changed, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let changed = session.render_javascript(changed).unwrap();
            let recheck = session.render_javascript(candidate).unwrap();
            assert_eq!(baseline.javascript(), recheck.javascript());
            assert_ne!(baseline.javascript(), changed.javascript());
            assert_eq!(
                session
                    .compilation()
                    .view(source)
                    .unwrap()
                    .unit_revision(unit),
                Some(revision)
            );
            (baseline, changed)
        },
    )
    .unwrap();
    for (artifact, expected) in [(baseline, "17\n"), (changed, "19\n")] {
        assert_eq!(
            execute_javascript(artifact.javascript(), "", "console.log(library.answer());"),
            expected
        );
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(
                artifact.sizes().get(codec).unwrap(),
                crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap()
            );
        }
        assert!(artifact.details()["policy_fingerprint"].is_array());
        assert!(artifact.details()["recipe_words"].is_array());
    }
    assert_eq!(finished.ledger.retained_bytes(), 0);
    assert!(
        finished.report["ledger_after_finish"]["edit_work"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn scoped_direct_delivery_cannot_bypass_runtime_admission() {
    let ((), finished) = with_checked_source(
        "export int answer(){return 17;}",
        &config("[policy.constraints]\nmax_startup_work=0"),
        ServiceOptions::default(),
        |session| {
            let candidate = {
                let (owner, source, policy) = session.parts_mut();
                owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let error = session.render_javascript(candidate).unwrap_err();
            assert_eq!(error.phase, "javascript admission");
            assert!(error.message.contains("MissingCostEvidence"));
        },
    )
    .unwrap();
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

struct ReleaseObserver;

impl ReleaseObserver {
    fn new() -> Self {
        RELEASE_EVENTS.with(|events| {
            assert!(events.borrow_mut().replace(Vec::new()).is_none());
        });
        Self
    }

    fn events(&self) -> Vec<&'static str> {
        RELEASE_EVENTS.with(|events| {
            events
                .borrow()
                .as_ref()
                .unwrap()
                .iter()
                .map(|event| event.0)
                .collect()
        })
    }

    fn retained(&self, event: &str) -> u64 {
        RELEASE_EVENTS.with(|events| {
            events
                .borrow()
                .as_ref()
                .unwrap()
                .iter()
                .find(|entry| entry.0 == event)
                .unwrap()
                .1
                .unwrap()
        })
    }
}

impl Drop for ReleaseObserver {
    fn drop(&mut self) {
        RELEASE_EVENTS.with(|events| {
            events.borrow_mut().take();
        });
    }
}

fn finalization_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/program/fixtures/search-structural-valley/entry.lil")
}

#[test]
fn path_factory_finalizes_before_source_release_on_success_and_callback_error() {
    for fail in [false, true] {
        let releases = ReleaseObserver::new();
        let (outcome, finished) = with_checked_path(
            &finalization_fixture(),
            &config(""),
            ServiceOptions::default(),
            |session| {
                assert!(releases.events().is_empty());
                let (owner, source, policy) = session.parts_mut();
                owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap();
                assert!(owner.ledger().retained_bytes() > 0);
                record_release("client");
                if fail {
                    Err("client declined")
                } else {
                    Ok(17)
                }
            },
        )
        .unwrap();
        assert_eq!(outcome, if fail { Err("client declined") } else { Ok(17) });
        assert_eq!(
            releases.events(),
            [
                "client",
                "compilation",
                "source",
                "source-charge",
                "finished"
            ]
        );
        let capacity = finished.report["resources"]["source_buffer_capacity"]
            .as_u64()
            .unwrap();
        assert!(capacity > 0);
        assert_eq!(releases.retained("compilation"), capacity);
        assert_eq!(releases.retained("source"), capacity);
        assert_eq!(releases.retained("source-charge"), 0);
        assert_eq!(releases.retained("finished"), 0);
        assert!(finished.report["phases_ns"]["source_release_ns"].is_u64());
        assert_eq!(finished.ledger.retained_bytes(), 0);
        assert_eq!(finished.report["ledger_after_finish"]["retained_bytes"], 0);
    }
}

#[test]
fn path_factory_finalizes_and_releases_source_before_resuming_same_panic() {
    let releases = ReleaseObserver::new();
    let payload = std::sync::Arc::new(String::from("original client panic"));
    let outcome = std::panic::catch_unwind({
        let payload = payload.clone();
        move || {
            let _ = with_checked_path(
                &finalization_fixture(),
                &config(""),
                ServiceOptions::default(),
                |session| -> () {
                    let (owner, source, policy) = session.parts_mut();
                    owner
                        .direct_javascript(source, policy, WorkDomain::Baseline)
                        .unwrap();
                    record_release("client");
                    std::panic::panic_any(payload);
                },
            );
        }
    });
    let resumed = outcome
        .unwrap_err()
        .downcast::<std::sync::Arc<String>>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&payload, &resumed));
    assert_eq!(
        releases.events(),
        [
            "client",
            "compilation",
            "source",
            "source-charge",
            "finished"
        ]
    );
    assert!(releases.retained("compilation") > 0);
    assert_eq!(
        releases.retained("compilation"),
        releases.retained("source")
    );
    assert_eq!(releases.retained("source-charge"), 0);
    assert_eq!(releases.retained("finished"), 0);
}

#[test]
fn source_factory_finalizes_without_claiming_caller_source_release() {
    let source = String::from("export int answer(){return 17;}");
    let releases = ReleaseObserver::new();
    let (length, finished) =
        with_checked_source(&source, &config(""), ServiceOptions::default(), |session| {
            assert!(session.compilation().view(session.source()).is_ok());
            source.len()
        })
        .unwrap();
    assert_eq!(length, source.len());
    assert_eq!(source, "export int answer(){return 17;}");
    assert_eq!(releases.events(), ["compilation", "finished"]);
    assert!(finished.report["phases_ns"]["source_release_ns"].is_null());
    assert!(finished.report["resources"]["source_buffer_capacity"].is_null());
    assert_eq!(
        finished.report["resources"]["source_buffer_accounting"],
        "caller-owned text; excluded"
    );
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

#[test]
fn default_service_preserves_target_metadata_after_factory_finalization() {
    let result = compile_source(
        "print(42);",
        &config(""),
        ServiceOptions {
            target: ServiceTarget::All,
            preserve_root_exports: false,
            objectives: Some(Objectives::One(Objective::Raw)),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let report = result.report();
    let javascript = result.javascript(Objective::Raw).unwrap();
    let native = result.native_c().unwrap();
    assert_eq!(report["artifacts"][0]["sha256"], javascript.sha256());
    assert_eq!(report["artifacts"][0]["details"], *javascript.details());
    assert_eq!(report["winners"], json!([0, null, null]));
    assert_eq!(report["native_sha256"], digest(native.as_bytes()));
    assert_eq!(report["native_delivery"]["c_bytes"], native.len());
    assert_eq!(
        report["native_delivery"]["semantic"],
        javascript.details()["semantic"]
    );
    assert!(report["first_artifact_ns"].is_u64());
    assert!(report["search"].is_object());
    assert!(report.get("backend").is_none());
    assert_eq!(report["ledger_after_finish"]["retained_bytes"], 0);
    assert!(report["phases_ns"]["source_release_ns"].is_null());
}

#[test]
fn delivered_rewrite_metadata_survives_finalization_and_marks_inherited_history() {
    use crate::program::{CellBinding, CellId, Constant, OpId, OperationKind};

    // The program rules fold `3+4` before publication; this test needs the
    // publication layer's own rewrite of it.
    SKIP_PROGRAM_RULES.with(|skip| skip.set(true));
    let ((folded_output, changed_output, before, meaning), finished) = with_checked_source(
        "export int answer(){return 3+4;}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::One(Objective::Raw)),
            ..ServiceOptions::default()
        },
        |session| {
            let (unit, operation, before, meaning) = {
                let view = session.compilation().view(session.source()).unwrap();
                let unit = (0..view.cell_count())
                    .find_map(|index| {
                        let cell = view.cell(CellId::from_index(index).unwrap()).unwrap();
                        match cell.binding {
                            CellBinding::Function(unit) if cell.name == "answer" => Some(unit),
                            _ => None,
                        }
                    })
                    .unwrap();
                let operation = view
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .position(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
                    .unwrap();
                (
                    unit,
                    OpId::from_index(operation).unwrap(),
                    format!("{:?}", view.snapshot_identity()),
                    format!("{:?}", view.meaning_identity()),
                )
            };
            let (folded, candidate) = {
                let (owner, source, policy) = session.parts_mut();
                let folded = owner
                    .fold_literal_int_binary(source, unit, operation, policy, WorkDomain::Baseline)
                    .unwrap()
                    .unwrap();
                let candidate = owner
                    .direct_javascript(folded, policy, WorkDomain::Baseline)
                    .unwrap();
                (folded, candidate)
            };
            let folded_output = session.render_javascript(candidate).unwrap();
            let revision = session
                .compilation()
                .view(folded)
                .unwrap()
                .unit_revision(unit)
                .unwrap();
            let replacement = OperationKind::Constant(Constant::Integer(9));
            let operations = [OperationPatch {
                operation,
                kind: &replacement,
                operands: &[],
            }];
            let patches = [UnitPatch {
                unit,
                expected_revision: revision,
                operations: &operations,
                places: &[],
            }];
            let changed = session
                .compilation_mut()
                .edit_source(folded, &patches, WorkDomain::Baseline)
                .unwrap();
            let candidate = {
                let (owner, _, policy) = session.parts_mut();
                owner
                    .direct_javascript(changed, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let changed_output = session.render_javascript(candidate).unwrap();
            (folded_output, changed_output, before, meaning)
        },
    )
    .unwrap();
    SKIP_PROGRAM_RULES.with(|skip| skip.set(false));
    assert_eq!(finished.ledger.retained_bytes(), 0);
    let folded = &folded_output.details()["semantic"];
    let changed = &changed_output.details()["semantic"];
    assert_eq!(folded["meaning_identity"], meaning);
    assert_ne!(folded["snapshot_identity"], before);
    assert_eq!(folded["rewrite_order"], "newest-first");
    assert_eq!(folded["rewrites"].as_array().unwrap().len(), 1);
    let step = &folded["rewrites"][0];
    assert_eq!(step["meaning_identity"], meaning);
    assert_eq!(step["before_snapshot"], before);
    assert_eq!(step["after_snapshot"], folded["snapshot_identity"]);
    assert_eq!(step["inherited"], false);
    let fold = &step["fold"];
    assert_eq!(fold["rule"], "literal-int-binary");
    assert_eq!(fold["version"], LITERAL_INT_FOLD_VERSION);
    assert_eq!(fold["operator"], "Add");
    assert_eq!(fold["left"], 3);
    assert_eq!(fold["right"], 4);
    assert_eq!(fold["result"], 7);
    for field in ["unit", "operation", "left_producer", "right_producer"] {
        assert!(fold[field].is_u64());
    }
    assert_ne!(changed["meaning_identity"], meaning);
    assert_ne!(changed["snapshot_identity"], folded["snapshot_identity"]);
    assert_eq!(changed["rewrites"][0]["inherited"], true);
    assert_eq!(changed["rewrites"][0]["fold"], *fold);
    assert_eq!(changed["rewrites"][0]["meaning_identity"], meaning);
    for (artifact, expected) in [(folded_output, "7\n"), (changed_output, "9\n")] {
        assert_eq!(
            execute_javascript(artifact.javascript(), "", "console.log(library.answer());"),
            expected
        );
    }
}

#[test]
fn path_checker_refusal_releases_source_buffers_but_preserves_owned_diagnostic() {
    let scratch = Scratch::new();
    let path = scratch.0.join("bad.lil");
    let source = "export int answer(){return unknown_name;}";
    std::fs::write(&path, source).unwrap();
    let mut frontend = Frontend::new(&config(""), ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    sources.store(source, &mut frontend.ledger).unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    sources.discard(&mut frontend.ledger).unwrap();
    let releases = ReleaseObserver::new();
    let error = compile_path(&path, &config(""), ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase, "check");
    assert_eq!(error.diagnostic.unwrap().source, source);
    assert_eq!(releases.events(), ["source", "source-charge"]);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn path_source_memory_refusal_is_typed_without_diagnostic_source_copy() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(&path, "export int answer(){return 17;}").unwrap();
    let error = compile_path(
        &path,
        &config(""),
        ServiceOptions {
            retained_bytes: 1,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "discovery resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
}

#[test]
fn source_parser_memory_refusal_remains_typed_at_the_public_boundary() {
    let error = compile_source(
        "export int answer(){return 17;}",
        &config(""),
        ServiceOptions {
            retained_bytes: 1,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "parse resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
    let cause = std::error::Error::source(&error).unwrap();
    assert_eq!(
        cause.downcast_ref::<ServiceResourceError>(),
        error.resource.as_ref()
    );
}

#[test]
fn source_checker_and_conversion_work_refusals_remain_typed_at_the_public_boundary() {
    let source = "export int answer(){return 17;}";
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let syntax = arena.parse(source).unwrap();
    arena
        .with_ledger(|ledger, domain| {
            ledger.charge(domain, WorkKind::Analysis, source.len() as u64)
        })
        .unwrap();
    let checker_work = arena.with_ledger(|ledger, domain| ledger.work_used(domain));
    let conversion_work = arena.with_ledger(|ledger, domain| {
        with_analyzed_source(
            &syntax,
            &mut AllocationBudget::new(Some((ledger, domain))),
            |_, budget| {
                budget
                    .work(WorkKind::Analysis, source.len() as u64)
                    .unwrap();
                budget.with_ledger(|owner| {
                    let (ledger, domain) = owner.unwrap();
                    ledger.work_used(domain)
                })
            },
        )
        .unwrap()
    });
    drop(syntax);
    drop(arena);
    for (work, phase) in [
        (checker_work, "check resources"),
        (conversion_work, "conversion resources"),
    ] {
        let error = compile_source(
            source,
            &config,
            ServiceOptions {
                logical_work: work,
                ..ServiceOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, phase);
        assert_eq!(
            error.resource,
            Some(ServiceResourceError::Budget(
                crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
            ))
        );
        assert!(error.diagnostic.is_none());
    }
}

#[test]
fn deadline_at_adoption_discards_prepared_graph_before_releasing_its_charge() {
    let source = "export int answer(){return 17;}";
    let config = config("[policy.resources]\nwall_time_ms=60000");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    frontend
        .ledger
        .set_deadline_elapsed_for_test(std::time::Duration::ZERO);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let syntax = arena.parse(source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let prepared = arena
        .with_ledger(|ledger, domain| {
            from_checked_source_admitted(
                &syntax,
                &semantics,
                &mut AllocationBudget::new(Some((ledger, domain))),
            )
        })
        .unwrap();
    drop(semantics);
    drop(syntax);
    drop(arena);
    assert!(frontend.ledger.retained_bytes() > 0);
    frontend
        .ledger
        .set_deadline_elapsed_for_test(std::time::Duration::from_millis(60000));

    let releases = ReleaseObserver::new();
    let error = match frontend.adopt(prepared, json!({"root":0})) {
        Ok(_) => panic!("expired handoff must not adopt a graph"),
        Err((error, ledger)) => {
            assert_eq!(ledger.retained_bytes(), 0);
            error
        }
    };
    assert_eq!(error.phase, "frontend resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::DeadlineExceeded
        ))
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(releases.events(), ["prepared-discard"]);
    assert_eq!(releases.retained("prepared-discard"), 0);
}

#[test]
fn store_construction_refusal_returns_the_owner_for_prepared_graph_cleanup() {
    for memory_refusal in [false, true] {
        let source = "export int answer(){return 17;}";
        let options = ServiceOptions {
            logical_work: 1_000_000,
            retained_bytes: 1_000_000,
            ..ServiceOptions::default()
        };
        let mut frontend = Frontend::new(&config(""), options).unwrap();
        let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
        let syntax = arena.parse(source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let prepared = arena
            .with_ledger(|ledger, domain| {
                from_checked_source_admitted(
                    &syntax,
                    &semantics,
                    &mut AllocationBudget::new(Some((ledger, domain))),
                )
            })
            .unwrap();
        drop(semantics);
        drop(syntax);
        drop(arena);
        let prepared_bytes = frontend.ledger.retained_bytes();
        assert!(prepared_bytes > 0);
        let remaining_charge = if memory_refusal {
            let remaining = options.retained_bytes - prepared_bytes;
            frontend
                .ledger
                .retain(WorkDomain::Baseline, remaining)
                .unwrap();
            remaining
        } else {
            let remaining = options.logical_work - frontend.ledger.work_used(WorkDomain::Baseline);
            frontend
                .ledger
                .charge(WorkDomain::Baseline, WorkKind::Analysis, remaining)
                .unwrap();
            0
        };

        let releases = ReleaseObserver::new();
        let error = match frontend.adopt(prepared, json!({"root":0})) {
            Ok(_) => panic!("exhausted store construction must refuse"),
            Err((error, mut ledger)) => {
                assert_eq!(ledger.retained_bytes(), remaining_charge);
                ledger
                    .release(WorkDomain::Baseline, remaining_charge)
                    .unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
                error
            }
        };
        let expected = if memory_refusal {
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        } else {
            crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
        };
        assert_eq!(error.phase, "adoption");
        assert_eq!(
            error.message,
            format!("{:?}", PublicationError::Budget(expected))
        );
        assert_eq!(releases.events(), ["prepared-discard"]);
        assert_eq!(releases.retained("prepared-discard"), remaining_charge);
    }
}

#[test]
fn path_post_discovery_work_refusal_releases_sources_before_returning() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    let source = "export int answer(){return 17;}";
    std::fs::write(&path, source).unwrap();
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let (modules, syntax) = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    drop(syntax);
    drop(arena);
    let work = frontend.ledger.work_used(WorkDomain::Baseline);
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    let releases = ReleaseObserver::new();
    let error = compile_path(
        &path,
        &config,
        ServiceOptions {
            logical_work: work,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "frontend resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(releases.events(), ["source", "source-charge"]);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn path_checker_and_conversion_refusals_release_sources_without_a_diagnostic_copy() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(&path, "export int answer(){return 17;}").unwrap();
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let (modules, syntax) = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    let bytes = modules
        .modules
        .iter()
        .map(|module| module.source.len() as u64)
        .sum();
    arena
        .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
        .unwrap();
    arena
        .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
        .unwrap();
    let checker_work = arena.with_ledger(|ledger, domain| ledger.work_used(domain));
    let conversion_work = arena.with_ledger(|ledger, domain| {
        with_analyzed_modules(
            &syntax,
            &modules,
            &mut AllocationBudget::new(Some((ledger, domain))),
            |_, budget| {
                budget.work(WorkKind::Analysis, bytes).unwrap();
                budget.with_ledger(|owner| {
                    let (ledger, domain) = owner.unwrap();
                    ledger.work_used(domain)
                })
            },
        )
        .unwrap()
    });
    drop(syntax);
    drop(arena);
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    assert_eq!(frontend.ledger.retained_bytes(), 0);

    for (work, phase) in [
        (checker_work, "check resources"),
        (conversion_work, "conversion resources"),
    ] {
        let releases = ReleaseObserver::new();
        let error = compile_path(
            &path,
            &config,
            ServiceOptions {
                logical_work: work,
                ..ServiceOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, phase);
        assert_eq!(
            error.resource,
            Some(ServiceResourceError::Budget(
                crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
            ))
        );
        assert!(error.diagnostic.is_none());
        assert_eq!(releases.events(), ["source", "source-charge"]);
        assert_eq!(releases.retained("source"), source_bytes);
        assert_eq!(releases.retained("source-charge"), 0);
    }
}

#[test]
fn path_store_refusal_releases_prepared_graph_then_factory_source_buffers() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(&path, "export int answer(){return 17;}").unwrap();
    let config = config("");
    let (_, completed) =
        with_checked_path(&path, &config, ServiceOptions::default(), |_| ()).unwrap();
    let resources = &completed.report["resources"];
    let work = resources["frontend_logical_work"].as_u64().unwrap();
    let source_bytes = resources["source_buffer_capacity"].as_u64().unwrap();
    assert_eq!(completed.ledger.retained_bytes(), 0);

    let releases = ReleaseObserver::new();
    let error = compile_path(
        &path,
        &config,
        ServiceOptions {
            logical_work: work,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "adoption");
    assert_eq!(
        error.message,
        format!(
            "{:?}",
            PublicationError::Budget(crate::compilation_policy::BudgetError::WorkExhausted(
                WorkDomain::Baseline
            ))
        )
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(
        releases.events(),
        ["prepared-discard", "source", "source-charge"]
    );
    assert_eq!(releases.retained("prepared-discard"), source_bytes);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn path_factory_parses_each_canonical_module_once_and_releases_syntax_before_backend() {
    use crate::parser::admitted_arena_activity_for_test;
    use crate::program::CellId;

    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(
        &path,
        "import {twice} from \"./math.lil\";export int answer(){return twice(21);}",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("math.lil"),
        "export int twice(int value){return value*2;}",
    )
    .unwrap();
    let (live_before, parses_before) = admitted_arena_activity_for_test();
    let (result, finished) =
        with_checked_path(&path, &config(""), ServiceOptions::default(), |session| {
            let (live, parses) = admitted_arena_activity_for_test();
            assert_eq!(
                live, live_before,
                "actual syntax arena must already be dropped"
            );
            assert_eq!(parses - parses_before, 2);
            assert_eq!(session.shape["modules"], 2);
            let source = session.source();
            {
                let view = session.compilation().view(source).unwrap();
                for expected in ["answer", "twice"] {
                    assert!((0..view.cell_count()).any(|index| {
                        view.cell(CellId::from_index(index).unwrap()).unwrap().name == expected
                    }));
                }
            }
            session.compile_targets()
        })
        .unwrap();
    let output = finish_output(result, finished).unwrap();
    check_scores(&output);
    assert_eq!(
        admitted_arena_activity_for_test(),
        (live_before, parses_before + 2)
    );
    assert_eq!(
        execute_javascript(
            output.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            "console.log(library.answer());",
        ),
        "42\n"
    );
}

#[test]
fn shared_module_discovery_refusal_releases_partial_sources_on_the_same_ledger() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    let mut entry = String::new();
    for module in 0..8 {
        entry.push_str(&format!(
            "import {{value{module}}} from \"./part{module}.lil\";"
        ));
        let mut source = format!("export int value{module}(){{");
        for local in 0..64 {
            source.push_str(&format!("int local{local}={local};"));
        }
        source.push_str("return local63;}");
        std::fs::write(scratch.0.join(format!("part{module}.lil")), source).unwrap();
    }
    entry.push_str("export int answer(){return value0();}");
    std::fs::write(&path, entry).unwrap();
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let (modules, syntax) = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap();
    drop(syntax);
    drop(arena);
    let discovery_peak = frontend.ledger.peak_retained_bytes();
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    assert_eq!(frontend.ledger.retained_bytes(), 0);

    let options = ServiceOptions {
        retained_bytes: discovery_peak - 1,
        ..ServiceOptions::default()
    };
    let mut frontend = Frontend::new(&config, options).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let error = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap_err();
    assert!(matches!(error, ModuleDiscoveryError::Resources(_)));
    drop(arena);
    let source_bytes = sources.allocated_bytes() as u64;
    assert!(
        source_bytes > 0,
        "refusal must retain some discovered source storage"
    );
    assert_eq!(frontend.ledger.retained_bytes(), source_bytes);
    sources.discard(&mut frontend.ledger).unwrap();
    assert_eq!(frontend.ledger.retained_bytes(), 0);

    let releases = ReleaseObserver::new();
    let error = compile_path(&path, &config, options).unwrap_err();
    assert_eq!(error.phase, "discovery resources", "{error:?}");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(releases.events(), ["source", "source-charge"]);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn exported_defaults_keep_their_javascript_length_and_values() {
    // JavaScript's `length` stops at the first default; an omitted or
    // `undefined` argument takes the default, as with default syntax.
    let result = compile_source(
        "export int read(int value, int scale = 2, JsValue extra = JS.undefined()){return value * scale;}",
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(
        execute_javascript(
            result.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            "console.log(JSON.stringify([library.read.length,library.read(3),library.read(3,4),library.read(3,undefined)]));"
        ),
        "[1,6,12,6]\n"
    );
}

#[test]
fn foreign_imports_become_es_imports_of_their_extern_values() {
    // `import extern` binds a module's extern value declarations to a
    // JavaScript module's exports; the output imports them, spelled from
    // the root module's directory as the old route's linker spelled them.
    let scratch = Scratch::new();
    std::fs::create_dir(scratch.0.join("lib")).unwrap();
    std::fs::write(
        scratch.0.join("host.mjs"),
        "export const greeting='hi';export function twice(x){return x*2}export default {answer:42};",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("lib/greet.lil"),
        "import extern { greeting, twice, default as config } from \"../host.mjs\";\n\
         extern string greeting;\nextern JsValue config;\nextern int twice(int value);\n\
         export string greet() { return greeting + JS.string(JS.get(config, \"answer\")) + twice(1); }\n",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("main.lil"),
        "import { greet } from \"./lib/greet.lil\";\n\
         import extern { twice } from \"./host.mjs\";\nextern int twice(int value);\n\
         export string run() { return greet() + \":\" + twice(21); }\n",
    )
    .unwrap();
    let result = compile_path(
        &scratch.0.join("main.lil"),
        &config(""),
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let javascript = result.javascript(Objective::Brotli).unwrap().javascript();
    assert!(javascript.contains("from\"./host.mjs\""), "{javascript}");
    // An import's identity is `(source, imported)`: both modules' `twice`
    // is one binding, the default is the default binding, and the one
    // specifier has one declaration (C18).
    assert_eq!(javascript.matches("import").count(), 1, "{javascript}");
    assert_eq!(javascript.matches("twice").count(), 1, "{javascript}");
    assert!(
        javascript.starts_with("import ") && !javascript.contains("default as"),
        "{javascript}"
    );
    std::fs::write(scratch.0.join("out.mjs"), javascript).unwrap();
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!(
                "const m=await import({});console.log(m.run());",
                serde_json::to_string(&format!("file://{}", scratch.0.join("out.mjs").display()))
                    .unwrap()
            ),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "hi422:42\n");
}

/// Three wrong programs the harvest of the old route's tests found in the
/// searched output (plan M1.9): an async body inlined as its return value,
/// field defaults created before a constructor's arguments, and `==` on a
/// `JsValue` compiled as `===` against the language's dynamic equality.
#[test]
fn searched_output_keeps_async_calls_construction_order_and_dynamic_equality() {
    let source = r#"
        extern int read();
        extern JsValue dynamic();
        extern void note(string label);
        async int immediate() { return 1; }
        class Cache {
            Map<string, int> guard;
            int value;
            init(int value) { this.value = value; }
        }
        export void run() {
            immediate().then((int v) => print(v));
            Cache cache = new Cache(read());
            print(cache.value);
            JsValue v = dynamic();
            print(v == 0);
            print(v != 0);
        }
    "#;
    let setup = "const events=[];globalThis.Map=class{constructor(){events.push('map')}};\
        globalThis.read=()=>{events.push('read');return 7};\
        globalThis.dynamic=()=>({valueOf(){events.push('coerce');return 0}});";
    for codec in [Objective::Raw, Objective::Brotli] {
        let result = compile_source(
            source,
            &config(""),
            ServiceOptions {
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                setup,
                "library.run();await 0;console.log(events.join(','));"
            ),
            "7\ntrue\nfalse\n1\nread,map,coerce,coerce\n",
            "{}",
            result.javascript(codec).unwrap().javascript()
        );
    }
}

/// A method is its own function (plan M8.2 A1, law P1): no private callback
/// shares an adapter factory's body, and where the contract does not keep
/// names, a method may carry the name JavaScript infers (R6). This pins the
/// gained name: the object key names the method, where the adapter's result
/// was anonymous.
#[test]
fn adapter_callbacks_are_their_own_methods_and_may_gain_an_inferred_name() {
    let source = "JsValue holder = JS.object(\"greet\", JS.method1((JsValue self, JsValue who) => JS.add(self[\"p\"], who)));\n\
                  JsValue other = JS.object(\"echo\", JS.method1((JsValue self, JsValue who) => JS.add(who, self[\"p\"])));\n\
                  export JsValue make() { return JS.object(\"a\", holder, \"b\", other); }";
    for keep in [false, true] {
        let result = compile_source(
            source,
            &config(&format!("keep_function_names={keep}")),
            ServiceOptions {
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let javascript = result.javascript(codec).unwrap().javascript();
            // No shared factory: each callback reads its receiver as `this`.
            assert!(!javascript.contains("(this,"), "{javascript}");
            assert_eq!(javascript.matches("this.p").count(), 2, "{javascript}");
            let names = if keep {
                "[\"\",\"\"]"
            } else {
                "[\"greet\",\"echo\"]"
            };
            assert_eq!(
                execute_javascript(
                    javascript,
                    "",
                    "const m=library.make();const o={p:'!',greet:m.a.greet,echo:m.b.echo};console.log(JSON.stringify([m.a.greet.name,m.b.echo.name]),o.greet('x'),o.echo('y'),m.a.greet.length);"
                ),
                format!("{names} !x y! 1\n"),
                "keep_function_names={keep}\n{javascript}"
            );
        }
    }
}

/// A class body is strict. Under `--target js` a static body moves into its
/// prototype method only when strictness cannot change what it does (plan
/// M8.2 A1, architecture §10.2): a failed host write that a sloppy frame
/// ignores stays in a sloppy function; a body without one becomes the
/// method.
#[test]
fn class_bodies_in_a_classic_script_keep_sloppy_host_writes_out_of_the_class() {
    let source = "extern JsValue Object;\n\
                  export class Writer {\n\
                    string label() { return \"w\"; }\n\
                    bool poke(JsValue target) { target[\"x\"] = 1; return true; }\n\
                  }\n\
                  export constructor Writer;\n\
                  JsValue writer = new Writer();\n\
                  print(JS.invoke(writer, \"label\"));\n\
                  print(JS.invoke(writer, \"poke\", JS.invoke(Object, \"freeze\", JS.object())));";
    // A path build, as the CLI's `--target js` makes it.
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("writer.lil"), source).unwrap();
    let result = compile_path(
        &scratch.0.join("writer.lil"),
        &config(""),
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let javascript = result.javascript(Objective::Brotli).unwrap().javascript();
    // `label` is its body; `poke` forwards to a sloppy static body.
    assert!(javascript.contains("label(){return\"w\"}"), "{javascript}");
    assert!(
        javascript.contains("poke(") && javascript.contains("(this,"),
        "{javascript}"
    );
    let output = Command::new("node")
        .args(["-e", javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "w\ntrue\n");
}

#[test]
fn application_class_forwarders_keep_sloppy_assignment_and_delete_semantics() {
    // Y5 hides caller reflection, but a class is still strict. Moving one
    // of these static bodies into its forwarding method would throw on
    // the frozen property instead of leaving it unchanged.
    for (method, expected) in [
        (
            "void poke(JsValue target) { target[\"x\"] = 1; }",
            "undefined\n",
        ),
        (
            "void poke(JsValue target) { JS.delete(target, \"x\"); }",
            "undefined\n",
        ),
    ] {
        let source = format!(
            "extern JsValue Object; export class Writer {{ {method} }}\n\
             export constructor Writer; JsValue writer = new Writer();\n\
             print(JS.invoke(writer, \"poke\", JS.invoke(Object, \"freeze\", JS.object(\"x\", 0))));"
        );
        let scratch = Scratch::new();
        std::fs::write(scratch.0.join("writer.lil"), source).unwrap();
        let compiled = compile_path(
            &scratch.0.join("writer.lil"),
            &config(""),
            ServiceOptions {
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        let javascript = compiled.javascript(Objective::Brotli).unwrap().javascript();
        let output = Command::new("node")
            .args(["-e", javascript])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{javascript}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{javascript}"
        );
    }
}

#[test]
fn the_objective_codec_settings_judge_report_and_fingerprint_the_build() {
    // Law B2 (M3.5): the judge is the configured codec, and its settings are
    // objective configuration.
    use crate::compression::{BrotliMode, BrotliSettings, CodecSettings, GzipSettings};
    let source = "export int answer(int value){int total=0;for(int i=0;i<value;i++){total+=i*3+1;}return total;}";
    let config = |settings: &str| {
        crate::config::parse_project_config(&format!("[objective]\ncodecs='brotli'\n{settings}"))
            .unwrap()
            .config
    };
    let canonical = config("");
    let nine = config("[objective.brotli]\nquality=9\nwindow=18\n");
    let request = CompilationRequest::JavaScript {
        preserve_root_exports: true,
    };
    let (canonical_policy, nine_policy) = (
        canonical.resolve_policy(request).unwrap(),
        nine.resolve_policy(request).unwrap(),
    );
    assert_ne!(canonical_policy.fingerprint(), nine_policy.fingerprint());
    assert_eq!(
        nine_policy.receipt()["objective"]["codec_settings"]["brotli"]["quality"],
        9
    );
    let compiled = compile_source(source, &nine, ServiceOptions::default()).unwrap();
    let artifact = compiled.javascript(Objective::Brotli).unwrap();
    let settings = CodecSettings {
        brotli: BrotliSettings {
            quality: 9,
            window: 18,
            mode: BrotliMode::Generic,
        },
        gzip: GzipSettings::CANONICAL,
    };
    let judged = crate::compression::measure_admitted_with(
        artifact.javascript().as_bytes(),
        Objective::Brotli,
        &settings,
        &mut crate::output_budget::AllocationBudget::new(None),
    )
    .unwrap();
    assert_eq!(artifact.sizes().get(Objective::Brotli), Some(judged));
}

/// Owner answer Y5 (2026-09-29): an application script's top-level bindings
/// are the program's own unless declared global, so its roots seal as a
/// module's do. A top-level function called once with constants folds to
/// its value and goes (the catalog's `number/clamp` family, which the
/// competitor bars print as the constant).
#[test]
fn an_application_scripts_roots_are_the_programs_own() {
    let source = "int clamp(int value, int lo, int hi) { if (value < lo) { return lo; } if (value > hi) { return hi; } return value; } print(clamp(-2, 0, 10));";
    let compiled = compile_source(
        source,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let javascript = compiled
        .javascript(Objective::Brotli)
        .expect("the Brotli artifact")
        .javascript();
    assert_eq!(
        javascript.trim(),
        "(()=>{console.log(0);})();",
        "{javascript}"
    );
}

#[test]
fn s4_native_capabilities_are_reported_before_lowering_with_source_spans() {
    for (source, part) in [
        ("int prefix=1;unknown value=3;", "unknown"),
        (
            "int prefix=1;export int answer(){return prefix;}",
            "native exported ABI",
        ),
    ] {
        for target in [ServiceTarget::Native, ServiceTarget::All] {
            let error = compile_source(
                source,
                &ProjectConfig::default(),
                ServiceOptions {
                    target,
                    ..ServiceOptions::default()
                },
            )
            .unwrap_err();
            assert_eq!(error.phase, "check", "{error:?}");
            assert!(error.message.contains(part), "{error:?}");
            let diagnostic = error.diagnostic.expect("source-owned capability error");
            assert!(
                diagnostic.span.start > 0 && diagnostic.span.end > diagnostic.span.start,
                "{diagnostic:?}"
            );
        }
    }
}

#[test]
fn s4_variadic_exports_use_real_rest_arrays_and_reflected_length() {
    let source="export int sum(int first,int... rest){int result=first;for(int i=0;i<rest.length;i++){result+=rest[i];}return result;}export auto read=(int... rest)=>rest.length;";
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "console.log(library.sum.length,library.sum(2),library.sum(2,3,4),library.read.length,library.read(),library.read(1,2,3));"),"1 2 9 0 0 3\n");
    }
}

#[test]
fn s4_forwarded_host_alias_preserves_omission_and_explicit_argument_order() {
    let source = "extern int choose(int value=7);extern int next();export int run(){auto alias=choose;int a=alias();int b=alias(next());return a+b;}";
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),
            "let events=[];globalThis.choose=function(x){events.push(['choose',arguments.length,x]);return arguments.length?x:10;};globalThis.next=()=>{events.push(['next']);return 3;};",
            "console.log(library.run());console.log(JSON.stringify(events));"),
            "13\n[[\"choose\",0,null],[\"next\"],[\"choose\",1,3]]\n");
    }
}

#[test]
fn s4_assumed_generic_and_nullable_structs_snapshot_public_fields_once() {
    let source = r#"
        struct Point{int x;}
        struct Box<T>{T value;}
        export int read(JsValue input){
            Box<Point>? snapshot=JS.assume(input);
            if(snapshot==null){return -1;}
            Box<Point> copy=snapshot;copy.value.x+=1;
            return snapshot.value.x*10+copy.value.x;
        }
        export int sum(JsValue input){
            Point?[] items=JS.assume(input);int n=0;
            for(int i=0;i<items.length;i++){Point? p=items[i];if(p!=null){n+=p.x;}}
            return n;
        }
    "#;
    let result=compile_source(source,&config(""),ServiceOptions {
        objectives:Some(Objectives::All),..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "let reads=0;const p={get x(){reads++;return 3;}};console.log(JSON.stringify([library.read(null),library.read(undefined),library.read({value:p}),reads,library.sum([null,p,undefined,{x:4}]),reads]));"),
            "[-1,-1,34,1,7,2]\n");
    }
}

#[test]
fn s4_checked_binary_reads_and_development_traps_reach_public_routes() {
    let source = r#"
        export number get(int i){Float64Array a=new Float64Array(1);a[0]=2.5;return a.get(i)??-3.0;}
        export int read(int i){Uint8Array a=new Uint8Array(1);a[0]=255;return a[i];}
        export int unit(int i){return "𐐀".codeUnitAt(i);}
    "#;
    let result=compile_source(source,&config("checks='development'"),ServiceOptions {
        objectives:Some(Objectives::All),..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const outcomes=[library.get(-1),library.get(0),library.get(1),library.read(0),library.unit(0),library.unit(1)];for(const fn of [library.read,library.unit])for(const i of [-1,9]){try{fn(i);outcomes.push('missed');}catch(e){outcomes.push(e instanceof RangeError);}}console.log(JSON.stringify(outcomes));"),
            "[-3,2.5,-3,255,55297,56320,true,true,true,true]\n");
    }
}

#[test]
fn s4_host_callbacks_keep_struct_defaults_rest_and_primitive_receivers() {
    let source = r#"
        struct Point{int x;}
        extern void observe(JsValue callback,JsValue method,JsValue nullable);
        JsValue callback=(Point p=Point{4},Point... rest)=>{
            for(int i=0;i<rest.length;i++){p.x+=rest[i].x;}return p;
        };
        JsValue method=(this int self,Point p=Point{2},Point... rest)=>{
            p.x+=self;for(int i=0;i<rest.length;i++){p.x+=rest[i].x;}return p;
        };
        JsValue nullable=(Point? ... rest)=>{
            int n=0;for(int i=0;i<rest.length;i++){Point? p=rest[i];if(p!=null){n+=p.x;}}
            return Point{n};
        };
        observe(callback,method,nullable);
    "#;
    for script in [false, true] {
        let result = compile_source(source,&config(""),ServiceOptions {
            preserve_root_exports: !script,
            objectives: Some(Objectives::All),..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            let javascript = result.javascript(codec).unwrap().javascript();
            let code = format!(
                "globalThis.observe=(f,g,h)=>{{const q={{x:3}};console.log(JSON.stringify([f.length,f(),f(undefined,q),g.length,g.call(5),g.call(5,q,q),h(null,q),q]));}};{javascript}"
            );
            let mut command = Command::new("node");
            if !script {command.arg("--input-type=module");}
            let output = command.args(["-e", &code]).output().unwrap();
            assert!(output.status.success(), "{}\n{javascript}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(String::from_utf8(output.stdout).unwrap(),
                "[0,{\"x\":4},{\"x\":7},0,{\"x\":7},{\"x\":11},{\"x\":3},{\"x\":3}]\n");
        }
    }
}

#[test]
fn s4_dual_class_imports_and_reexports_preserve_runtime_identity() {
    let scratch = Scratch::new();
    for (file, source) in [
        ("entry.lil", r#"import {Crate} from "./barrel";
            export {Crate};export constructor Crate;
            export int read(){Crate box=new Crate(9);return box.value;}
            export async JsValue load(){auto ns=await import("./box");return ns.Box;}"#),
        ("barrel.lil", r#"import {Box as Crate} from "./box";export {Crate};export constructor Crate;"#),
        ("box.lil", "export class Box{int value;init(int value){this.value=value;}}export constructor Box;"),
    ] {
        std::fs::write(scratch.0.join(file), source).unwrap();
    }
    let result = compile_path(&scratch.0.join("entry.lil"), &config(""), ServiceOptions {
        objectives: Some(Objectives::All), ..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(), "",
            "const box=new library.Crate(4);console.log(JSON.stringify([box.value,box.constructor===library.Crate,box instanceof library.Crate,library.read(),await library.load()===library.Crate]));"),
            "[4,true,true,9,true]\n");
    }
}

#[test]
fn s4_generic_struct_schemas_keep_copies_and_public_shapes() {
    let source = r#"
        struct Box<T>{T value;}
        struct Point{int x;int y;}
        export Box<Point> change(Box<Point> original){
            Box<Point> copy=original;copy.value.x=8;return copy;
        }
        export Box<int> scalar(int value){return Box{value};}
        export Box<Point>? maybe(bool yes){
            if(yes){return Box{Point{1,2}};}return null;
        }
        export int shifts(int n){Box<Box<Box<int>>> box=Box{Box{Box{4}}};return box.value.value.value+(n>>2)+(n>>>3);}
        export int nested(){
            Box<Box<Point>> first=Box{Box{Point{3,4}}};
            Box<Box<Point>> copy=first;copy.value.value.y=9;
            return first.value.value.y*10+copy.value.value.y;
        }
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const p={value:{x:1,y:2}},q=library.change(p);console.log(JSON.stringify([p,q,p!==q,p.value!==q.value,library.scalar(7),library.maybe(false),library.maybe(true),library.nested(),library.shifts(-8)]));"),
            "[{\"value\":{\"x\":1,\"y\":2}},{\"value\":{\"x\":8,\"y\":2}},true,true,{\"value\":7},null,{\"value\":{\"x\":1,\"y\":2}},49,536870913]\n");
    }
}

#[test]
fn s4_struct_boundary_defaults_and_rest_keep_public_arity_and_value_copies() {
    let source = r#"
        struct Point{int x;}
        export int count(Point point=Point{7},Point... rest){
            int n=point.x;for(int i=0;i<rest.length;i++){n+=rest[i].x;rest[i].x=99;}return n;
        }
        export Point? same(Point? point){return point;}
        Point id<T>(Point value,T unused){return value;}
        export int privateCopy(){
            Point first=Point{4};Point copy=id(first,true);copy.x=8;return first.x*10+copy.x;
        }
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const p={x:2},q={x:3};console.log(JSON.stringify([library.count.length,library.count(),library.count(undefined,q),library.count(p,q),p,q,library.same(null),library.same(p),library.privateCopy()]));"),
            "[0,7,10,5,{\"x\":2},{\"x\":3},null,{\"x\":2},48]\n");
    }
}

#[test]
fn s4_struct_constructor_and_prototype_boundaries_keep_class_identity() {
    let source = r#"
        struct Point{int x;}
        class Box{
            int total;
            init(Point first=Point{3},Point... rest){
                this.total=first.x;for(int i=0;i<rest.length;i++){this.total+=rest[i].x;}
            }
            Point add(Point value=Point{4},Point... rest){
                value.x+=this.total;for(int i=0;i<rest.length;i++){value.x+=rest[i].x;}return value;
            }
        }
        class Derived extends Box{init(Point first=Point{2}){super(first);}}
        export constructor Box;export constructor Derived;
        export Box make(){return new Box(Point{5},Point{6});}
        export Derived derived(){return new Derived(Point{7});}
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "let reads=0;const p={get x(){reads++;return 8;}},q={x:2},box=new library.Box(p,q),base=new library.Box(),derived=new library.Derived();console.log(JSON.stringify([library.Box.name,library.Box.length,box.constructor===library.Box,box instanceof library.Box,box.total,base.total,box.add.length,box.add(),box.add(q,q),q,reads,derived.total,derived instanceof library.Box,derived.constructor===library.Derived,library.make().total,library.derived().total]));"),
            "[\"Box\",0,true,true,10,3,0,{\"x\":14},{\"x\":14},{\"x\":2},1,2,true,true,11,7]\n");
    }
}

#[test]
fn s4_receiver_and_class_rest_exports_preserve_public_calling_conventions() {
    let source="export constructor Box;class Box{int value;init(int x=7,int... values){this.value=x;for(int i=0;i<values.length;i++){this.value+=values[i];}}int sum(int first=2,int... values){int n=this.value+first;for(int i=0;i<values.length;i++){n+=values[i];}return n;}}export auto add=(this int self,int first=2,int... values)=>{int n=self+first;for(int i=0;i<values.length;i++){n+=values[i];}return n;};";
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const a=new library.Box(),b=new library.Box(1,2,3);console.log(library.Box.length,a.value,b.value,a.sum.length,a.sum(),a.sum(1,2,3),library.add.length,library.add.call(7),library.add.call(7,1,2,3));"),"0 7 6 0 9 13 0 9 13\n");
    }
}

#[test]
fn s4_explicit_field_contract_is_target_and_effort_independent() {
    let source="class C{int x;init(bool early){if(early){return;}this.x=3;}}print(new C(true).x);";
    for target in [ServiceTarget::JavaScript,ServiceTarget::Native,ServiceTarget::All] {
        for effort in [0,13] {
            let mut settings=ProjectConfig::default();
            settings.language.field_initialization=crate::config::FieldInitialization::Explicit;
            settings.effort.level=effort;
            let error=compile_source(source,&settings,ServiceOptions{target,preserve_root_exports:false,..ServiceOptions::default()}).unwrap_err();
            assert_eq!(error.phase,"check","{error:?}");
            assert!(error.message.contains("field `x`") && error.message.contains("R3"),"{error:?}");
            let diagnostic=error.diagnostic.unwrap();
            assert!(source[diagnostic.span.start..diagnostic.span.end].contains('x'));
        }
    }
    let valid="class C{int x;init(bool early){try{if(early){return;}}finally{this.x=3;}}}print(new C(true).x);";
    let mut settings=ProjectConfig::default();
    settings.language.field_initialization=crate::config::FieldInitialization::Explicit;
    let result=compile_source(valid,&settings,ServiceOptions{preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_javascript(result.javascript(Objective::Brotli).unwrap().javascript(),"",""),"3\n");
    let legacy=compile_source(source,&ProjectConfig::default(),ServiceOptions{preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_javascript(legacy.javascript(Objective::Brotli).unwrap().javascript(),"",""),"0\n");
}

#[test]
fn s4_explicit_field_contract_reaches_imported_modules_and_check_api() {
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("entry.lil"),"import {C} from \"./child\";print(new C().x);").unwrap();
    std::fs::write(scratch.0.join("child.lil"),"export class C{int x;}").unwrap();
    let settings:ProjectConfig=toml::from_str("[language]\nfield_initialization='explicit'").unwrap();
    let error=compile_path(&scratch.0.join("entry.lil"),&settings,ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check");
    assert!(error.diagnostic.unwrap().path.ends_with("child.lil"));
    let error=check_path(&scratch.0.join("entry.lil"),None,&settings).unwrap_err();
    assert_eq!(error.phase,"check");
    assert!(error.message.contains("field `x`"));
}

#[test]
fn s4_field_contract_is_strictly_parsed_and_fingerprinted() {
    use crate::compilation_policy::CompilationRequest;
    let legacy=ProjectConfig::default();
    let explicit:ProjectConfig=toml::from_str("[language]\nfield_initialization='explicit'").unwrap();
    for request in [CompilationRequest::Native,CompilationRequest::JavaScript{preserve_root_exports:true}] {
        let before=legacy.resolve_policy(request.clone()).unwrap();
        let after=explicit.resolve_policy(request).unwrap();
        assert_ne!(before.fingerprint(),after.fingerprint());
        assert_eq!(after.receipt()["source_contract"]["field_initialization"],"explicit");
    }
    for bad in ["[language]\nfield_initialization='auto'","[language]\nfield_initializations='explicit'"]{
        assert!(toml::from_str::<ProjectConfig>(bad).is_err());
    }
}


#[test]
fn s4_development_bounds_survive_unused_results_and_inlined_calls() {
    let source=r#"
        void local(int i){int[] a=[1];a[i];}
        void binary(int i){Uint8Array a=new Uint8Array(1);a[i];}
        void unit(int i){"x".codeUnitAt(i);}
        export void run(int kind,int index){if(kind==0){local(index);}else if(kind==1){binary(index);}else{unit(index);}}
    "#;
    let result=compile_source(source,&config("checks='development'"),ServiceOptions {
        objectives:Some(Objectives::All),..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const r=[];for(let kind=0;kind<3;kind++)for(const i of [0,-1,1]){try{library.run(kind,i);r.push('ok');}catch(e){r.push(e instanceof RangeError);}}console.log(JSON.stringify(r));"),
            "[\"ok\",true,true,\"ok\",true,true,\"ok\",true,true]\n");
    }
}

#[test]
fn s4_private_constructor_exports_dissolve_but_observed_values_keep_their_abi() {
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("box.lil"),
        "export class Box{int longPrivateField;init(int n){this.longPrivateField=n;}int read(int add=2){return this.longPrivateField+add;}}export constructor Box;"
    ).unwrap();
    std::fs::write(scratch.0.join("barrel.lil"),
        r#"import {Box as Crate} from "./box";export {Crate};export constructor Crate;"#
    ).unwrap();
    for observation in ["", "export JsValue ctor(){return Crate;}", "export constructor Crate;", "export async JsValue ctor(){auto ns=await import(\"./box\");return ns.Box;}"] {
        std::fs::write(scratch.0.join("entry.lil"),format!(
            "import {{Crate}} from \"./barrel\";export int run(int n){{Crate b=new Crate(n);return b.read();}}{observation}"
        )).unwrap();
        let compiled=compile_path(&scratch.0.join("entry.lil"),&config(""),ServiceOptions {
            objectives:Some(Objectives::All),..ServiceOptions::default()
        }).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            let javascript=compiled.javascript(objective).unwrap().javascript();
            if observation.is_empty() {
                assert!(!javascript.contains("longPrivateField"),"{javascript}");
                assert!(!javascript.contains("class "),"{javascript}");
                assert_eq!(execute_javascript(javascript,"","console.log(library.run(5));"),"7\n");
            } else {
                assert_eq!(execute_javascript(javascript,"",
                    "const C=library.Crate??await library.ctor();const b=new C(6);console.log(JSON.stringify([library.run(5),b.longPrivateField,b.read(),b.read(4),b.constructor===C,b instanceof C,Object.keys(b),C.length,C.prototype.read.length]));"),
                    "[7,6,8,10,true,true,[\"longPrivateField\"],1,0]\n");
            }
        }
    }
}

#[test]
fn s4_observed_generic_classes_preserve_one_constructor_and_typed_rest() {
    let source = r#"
        export class Box<T>{T value;int count;
            init(T value,int count=2,T... rest){this.value=value;this.count=count;for(int i=0;i<rest.length;i++){this.value=rest[i];}}
            T get(){return this.value;}
            T pick(T fallback,T... rest){if(rest.length>0){return rest[rest.length-1];}return fallback;}
            V echo<V>(V value){return value;}
        }
        export constructor Box;
        class Child<U> extends Box<U>{init(U value,U... rest){super(value,3,...rest);}}
        export constructor Child;
        export class Empty<T>{int count=7;}export constructor Empty;
        export int run(){Box<int> b=new Box<int>(5,4,7,9);Child<int> c=new Child<int>(2,8);Empty<string> e=new Empty<string>();return b.get()+b.count+c.get()+c.count+e.count+b.echo(10)+b.pick(1,2,3);}
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",
            "const a=Symbol('a'),b=Symbol('b'),box=new library.Box(a),child=new library.Child(a,b),empty=new library.Empty();console.log(JSON.stringify([library.run(),box.get()===a,box.pick(a,b)===b,box.echo(a)===a,child.get()===b,child instanceof library.Box,child.constructor===library.Child,box.constructor===library.Box,box.count,child.count,empty.count,library.Box.length,library.Child.length,library.Empty.length,box.pick.length]));"),
            "[44,true,true,true,true,true,true,true,2,3,7,1,1,0,1]\n");
    }
}

#[test]
fn s4_generic_fixed_struct_boundaries_do_not_require_opaque_transport() {
    let source = r#"
        struct Point{int x;}
        export Point copy<T>(Point p,T value){p.x+=1;return p;}
        class Box<T>{T value;init(T v){this.value=v;}
            Point change(Point p){p.x+=2;return p;}
        }
        export constructor Box;
        class Constructed{int value;init(Point p){this.value=p.x;}}
        export JsValue constructorValue(){return Constructed;}
        export int run(){Point p=Point{4};Box<int> b=new Box<int>(3);Point q=b.change(p);Point r=copy(p,true);return p.x*100+q.x*10+r.x;}
    "#;
    for script in [false, true] {
        let source = if script {
            format!("{source}extern void observe(JsValue a,JsValue b,JsValue c,JsValue d);observe(copy,Box,constructorValue,run);")
        } else {
            source.to_string()
        };
        let result = compile_source(
            &source,
            &config(""),
            ServiceOptions {
                preserve_root_exports: !script,
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let javascript = result.javascript(objective).unwrap().javascript();
            let body="const p={x:8},box=new library.Box(1),C=library.constructorValue();console.log(JSON.stringify([library.run(),library.copy(p,Symbol('v')),box.change(p),p,new C(p).value,new C(p).constructor===C,library.copy.length,box.change.length]));";
            let observed = if script {
                let code=format!("let library;globalThis.observe=(copy,Box,constructorValue,run)=>{{library={{copy,Box,constructorValue,run}};}};{javascript}{body}");
                let output = Command::new("node").args(["-e", &code]).output().unwrap();
                assert!(
                    output.status.success(),
                    "{}\n{javascript}",
                    String::from_utf8_lossy(&output.stderr)
                );
                String::from_utf8(output.stdout).unwrap()
            } else {
                execute_javascript(javascript, "", body)
            };
            assert_eq!(
                observed,
                "[465,{\"x\":9},{\"x\":10},{\"x\":8},8,true,2,1]\n"
            );
        }
    }
}

#[test]
fn s4_type_only_generic_exports_use_the_same_native_interface_as_module_graphs() {
    let source="export class Box<T>{T value;init(T v){this.value=v;}T get(){return this.value;}}Box<int> b=new Box<int>(7);print(b.get());";
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("entry.lil"), source).unwrap();
    for path in [false, true] {
        let options = ServiceOptions {
            target: ServiceTarget::All,
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        };
        let compiled = if path {
            compile_path(&scratch.0.join("entry.lil"), &config(""), options)
        } else {
            compile_source(source, &config(""), options)
        }
        .unwrap();
        assert_eq!(execute_native(compiled.native_c().unwrap()), "7\n");
        for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(
                execute_javascript(
                    compiled.javascript(objective).unwrap().javascript(),
                    "",
                    "console.log(Object.keys(library).length);"
                ),
                "7\n0\n"
            );
        }
    }
}

#[test]
fn s4_shapes_preserve_declared_spreads_accessors_and_optional_keys() {
    let source=r#"
        shape Token{data string kind;data int start;data int? end;accessor int width;}
        export Token sample(){return Token{kind:"text",start:2,width:5};}
        export Token copy(Token source,int? end){return Token{...source,end:end};}
        export void setEnd(Token source,int? value){source.end=value;}
        export void dropWidth(Token source){source.width;}
        export int touch(Token source){source.width+=1;return source.width;}
    "#;
    for checks in ["production","development"] {
        let result=compile_source(source,&config(&format!("checks='{checks}'")),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
                let reads=0,writes=0,extra=0,width=8,prototypeWrites=0;
                const input={kind:'host',start:4,get width(){reads++;return width},set width(v){writes++;width=v},get extra(){extra++;return 99}};
                const sample=library.sample(),copy=library.copy(input,null);
                library.dropWidth(input);const touched=library.touch(input);
                Object.defineProperty(Object.prototype,'end',{configurable:true,set(){prototypeWrites++}});
                library.setEnd(sample,0);const present=Object.keys(sample);library.setEnd(sample,null);
                delete Object.prototype.end;
                console.log(JSON.stringify([sample,copy,present,Object.keys(sample),touched,reads,writes,extra,prototypeWrites,Object.getPrototypeOf(copy)===Object.prototype]));
            "#),"[{\"kind\":\"text\",\"start\":2,\"width\":5},{\"kind\":\"host\",\"start\":4,\"width\":8},[\"kind\",\"start\",\"width\",\"end\"],[\"kind\",\"start\",\"width\"],9,4,1,0,0,true]\n");
        }
    }
}

#[test]
fn s4_shapes_join_views_and_keep_recursive_data_checks() {
    let source=r#"
        shape Left{data int x;}shape Right{data string label;}
        shape Node{data int value;data Node? next;}
        export Left & Right merge(Left left,Right right){return {...left,...right};}
        export int sum(Left & Right both){return both.x+both.label.length;}
        export int read(Node node){return node.value;}
    "#;
    let result=compile_source(source,&config("checks='development'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
            const both=library.merge({x:4,unused:3},{label:'hey'}),node={value:7};node.next=node;
            let access=0;const outcomes=[both,library.sum(both),library.read(node)];
            for(const value of [{get value(){access++;return 3}}, {value:'bad'}, {value:2,next:{value:false}}]){
                try{library.read(value);outcomes.push('missed')}catch(e){outcomes.push(e instanceof TypeError)}
            }
            node.value='changed';try{library.read(node);outcomes.push('missed')}catch(e){outcomes.push(e instanceof TypeError)}
            outcomes.push(access);console.log(JSON.stringify(outcomes));
        "#),"[{\"x\":4,\"label\":\"hey\"},7,7,true,true,true,true,0]\n");
    }
}

#[test]
fn s4_shapes_refuse_invalid_construction_and_admit_native_storage() {
    for (source,fragment) in [
        ("shape A{data int x;}A a=A{};","must provide"),
        ("shape A{data int x;}A a=A{wrong:1};","no field"),
        ("shape A{data int x;}A a=new A();","keyed literal"),
        ("shape A{data int x;}shape B{accessor int x;}void f(A & B value){}","same type and data/accessor"),
        ("shape A{data int x;}shape B{data string x;}void f(A & B value){}","same type and data/accessor"),
        ("shape A{data int x;}export constructor A;","constructor exports"),
        ("shape A{accessor int x;}pure int read(A a){return a.x;}","observable side effect"),
    ] {
        let error=compile_source(source,&config(""),ServiceOptions::default()).unwrap_err();
        assert_eq!(error.phase,"check","{error:?}");assert!(error.message.contains(fragment),"{error:?}");
    }
    let compiled=compile_source("shape A{data int x;}A a=A{x:1};print(a.x);",&config(""),ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap();
    assert!(compiled.native_c().is_some());
}

#[test]
fn s4_shapes_keep_generic_defaults_reference_identity_and_mangling_vetoes() {
    let source=r#"
        int count=0;int stamp(){count+=1;return count;}
        shape Box<T>{data T longPayloadName;data int createdStamp=stamp();}
        export func()->int make(int n){
            Box<int> value=Box{longPayloadName:n};Box<int> alias=value;
            return ()=>{alias.longPayloadName+=1;return value.longPayloadName*100+value.createdStamp;};
        }
    "#;
    for permission in ["off","on"] {
        let result=compile_source(source,&config(&format!("[policy.tactics]\nproperty-mangling='{permission}'\nscalar-replacement='off'\ninlining='off'\nconstant-folding='off'")),
            ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            let code=result.javascript(objective).unwrap().javascript();
            assert_eq!(execute_javascript(code,"","const first=library.make(3),second=library.make(7);console.log(JSON.stringify([first(),first(),second()]));"),"[401,501,802]\n");
            if objective==Objective::Raw {assert_eq!(code.contains("longPayloadName"),permission=="off","{code}");}
        }
    }
}

#[test]
fn s4_shapes_adapt_public_products_and_preserve_reentrant_places() {
    let source=r#"
        struct Point{int x;int y;}
        shape Holder{data Point position;accessor Point observed;data Point? spare;}
        shape Extra{data int label;}
        export Holder make(int n){Point p=Point{n,2};return Holder{position:p,observed:p};}
        export int update(Holder h,func()->int rhs){
            Point saved=h.position;h.position.x=rhs();return saved.x*100+saved.y;
        }
        export int touch(Holder h){h.observed.x+=2;return h.observed.y;}
        export Point? assign(Holder h,Point? p){return h.spare=p;}
        export Holder & Extra merge(Holder h,Extra e){return {...h,...e};}
    "#;
    for checks in ["production","development"] {
        let result=compile_source(source,&config(&format!("checks='{checks}'")),
            ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
                const h=library.make(3);let reads=0,writes=0,current={x:4,y:8};
                Object.defineProperty(h,'observed',{enumerable:true,configurable:true,
                    get(){reads++;return current},set(v){writes++;current=v}});
                const saved=library.update(h,()=>{h.position={x:20,y:30};return 7});
                const touched=library.touch(h), assigned=library.assign(h,{x:8,y:9});
                assigned.x=99;
                const copy=library.merge(h,{label:5,unused:10});
                const removed=library.assign(h,null);
                console.log(JSON.stringify([h,copy,saved,touched,reads,writes,current,removed]));
            "#),"[{\"position\":{\"x\":7,\"y\":30},\"observed\":{\"x\":6,\"y\":8}},{\"position\":{\"x\":7,\"y\":30},\"observed\":{\"x\":6,\"y\":8},\"spare\":{\"x\":8,\"y\":9},\"label\":5},302,8,4,1,{\"x\":6,\"y\":8},null]\n");
        }
    }
}

#[test]
fn s4_shapes_reject_forward_intersection_conflicts_and_malformed_product_data() {
    for source in [
        "shape Use{data A & B value;}shape A{data int x;}shape B{data string x;}",
        "struct Use{A & B value;}shape A{data int x;}shape B{accessor int x;}",
    ] {
        let error=compile_source(source,&config(""),ServiceOptions::default()).unwrap_err();
        assert_eq!(error.phase,"check","{error:?}");
        assert!(error.message.contains("same type and data/accessor"),"{error:?}");
    }
    let source="struct Point{int x;int y;}shape Holder{data Point p;}export int read(Holder h){return h.p.x;}";
    let result=compile_source(source,&config("checks='development'"),ServiceOptions{objectives:Some(Objectives::One(Objective::Raw)),..ServiceOptions::default()}).unwrap();
    let code=result.javascript(Objective::Raw).unwrap().javascript();
    assert_eq!(execute_javascript(code,"",r#"
        let getters=0;const out=[];
        for(const p of [{x:'bad',y:2},{x:3},{get x(){getters++;return 3},y:2}]){
            try{out.push(library.read({p}))}catch(e){out.push(e instanceof TypeError)}
        }
        out.push(getters);console.log(JSON.stringify(out));
    "#),"[true,true,true,0]\n");
}

#[test]
fn s4_shape_tags_and_spread_defaults_keep_their_checked_contracts() {
    let source=r#"
        int count=0;int stamp(){count+=1;return count;}
        shape Token{tag string kind="text";data int value;}
        shape Partial{data int? next;}
        shape Defaults{data int? next=stamp();}
        shape Hostile{data int __proto__;}
        export Token make(){return Token{kind:"t\u0065xt",value:7};}
        export bool test(JsValue x){return x is Token;}
        export void discard(JsValue x){x is Token;}
        export int narrow(JsValue x){Token? t=x as? Token;if(t!=null){return t.value;}return -1;}
        export Defaults copy(Partial p){return Defaults{...p};}
        export Defaults absent(Partial p){return Defaults{next:null,...p};}
        export Hostile hostile(Hostile p){return Hostile{...p};}
        export int calls(){return count;}
    "#;
    for checks in ["production","development"] {
        let result=compile_source(source,&config(&format!("checks='{checks}'")),
            ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
                let reads=0;const accessor={get kind(){reads++;return 'text'}};
                const made=library.make(),tests=[library.test(made),library.test(null),library.test(4),library.test({kind:'other'})];
                library.discard(accessor);tests.push(library.test(accessor),reads);
                const defaults=[library.copy({}),library.copy({next:9}),library.absent({}),library.calls()];
                const proto=library.hostile({['__proto__']:6,extra:7});
                console.log(JSON.stringify([made,tests,library.narrow(made),library.narrow({kind:'other'}),defaults,proto,Object.getPrototypeOf(proto)===Object.prototype]));
            "#),"[{\"kind\":\"text\",\"value\":7},[true,false,false,false,true,2],7,-1,[{\"next\":1},{\"next\":9},{},1],{\"__proto__\":6},true]\n");
        }
    }
    for (source,fragment) in [
        ("shape T{tag string kind=\"a\";}T t=T{kind:\"b\"};","declared literal"),
        ("shape T{tag string kind=\"a\";}T t=T{};t.kind=\"b\";","immutable"),
        ("shape T{tag string kind=\"a\";}shape U{tag string kind=\"b\";}void f(T & U t){}","discriminants must agree"),
        ("shape T{tag string kind=\"a\";}shape U{data string kind;}T f(U u){return T{...u};}","preserve the destination"),
        ("shape T{tag string kind;}T t=T{};","literal initializer"),
    ] {
        let error=compile_source(source,&config(""),ServiceOptions::default()).unwrap_err();
        assert_eq!(error.phase,"check","{error:?}");assert!(error.message.contains(fragment),"{error:?}");
    }
}

#[test]
fn s4_shapes_require_concrete_public_presence_and_keep_public_class_product_storage() {
    let refused="shape Box<T>{data T value;}export Box<T> make<T>(T value){return Box{value:value};}";
    let error=compile_source(refused,&config(""),ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");assert!(error.message.contains("optional-key storage"),"{error:?}");
    let source=r#"
        struct Point{int x;int y;}
        class Holder{Point p=Point{1,2};init(){}int read(){return this.p.x;}void set(Point p){this.p=p;}}
        export constructor Holder;
        shape Box<T>{data T value;}
        int calls=0;Point next(){calls+=1;return Point{calls,9};}
        export Point expression(Holder h){return h.p=next();}
        export int count(){return calls;}
        export Box<int?> optional(int? value){return Box{value:value};}
        export Box<JsValue> opaque(JsValue value){return Box{value:value};}
    "#;
    let result=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
            const h=new library.Holder(),p={x:5,y:6};h.set(p);p.x=90;
            const read=h.read(),assigned=library.expression(h);assigned.x=10;
            console.log(JSON.stringify([read,h.p,assigned,library.count(),library.optional(null),library.optional(3),library.opaque(null)]));
        "#),"[5,{\"x\":1,\"y\":9},{\"x\":10,\"y\":9},1,{},{\"value\":3},{\"value\":null}]\n");
    }
}

#[test]
fn s4_absence_unifies_defaults_and_keeps_boundary_pins() {
    let source=r#"
        int count=0;
        int next(){count+=1;return count;}
        int choose(int value=next()){return value;}
        export int exercise(){int?? a=undefined;int? b=null;int same=0;if(a==b){same=100;}return same+choose(a)*10+choose(b);}
        export int | undefined absentU(){return null;}
        export int | null absentN(){return undefined;}
        export int? inferred(){return undefined;}
        export int defaulted(int value=7){return value;}
    "#;
    let settings=config("[language]\nabsence='unified'");
    let result=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
            console.log(JSON.stringify([library.exercise(),library.absentU()===undefined,library.absentN()===null,library.inferred()===null,library.defaulted(),library.defaulted(undefined),library.defaulted(0),library.defaulted.length]));
        "#),"[112,true,true,true,7,7,0,0]\n");
    }
}

#[test]
fn s4_absence_keeps_public_optional_keys_and_pinned_fields() {
    let source=r#"
        shape S{data int? missing;data int | null n;data int | undefined u;}
        class C{int? value;init(){this.value=null;}void set(int? value){this.value=value;}}
        export constructor C;
        export S make(){return S{missing:undefined,n:undefined,u:null};}
        export void set(S s,int? value){s.missing=value;s.n=value;s.u=value;}
        struct P{int? x;int | null y;}
        export P pair(){return P{undefined,null};}
    "#;
    let result=compile_source(source,&config("[language]\nabsence='unified'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
            const s=library.make(),c=new library.C(),p=library.pair();
            const outcomes=[Object.keys(s),s.n===null,s.u===undefined,Object.keys(c),Object.keys(p),p.y===null];
            library.set(s,3);c.set(2);outcomes.push(s.missing,c.value);
            library.set(s,null);c.set(undefined);outcomes.push(Object.keys(s),Object.keys(c),s.n===null,s.u===undefined);
            console.log(JSON.stringify(outcomes));
        "#),"[[\"n\",\"u\"],true,true,[],[\"y\"],true,3,2,[\"n\",\"u\"],[],true,true]\n");
    }
}

#[test]
fn s4_absence_refuses_spelling_observations_and_keeps_dynamic_tests() {
    for source in [
        "void f(int? x){print(x);}",
        "string f(int? x){return string(x);}",
        "number f(int? x){return float(x);}",
        "JsValue f(int? x){return typeof x;}",
        "bool f(int? x){return x===null;}",
        "bool f(int?[] a,int? x){return a.includes(x);}",
        "string f(int?[] a){return JSON.stringify(a);}",
        "Set<int?> s=new Set<int?>();",
        "Map<int?,int> m=new Map<int?,int>();",
    ] {
        for target in [ServiceTarget::JavaScript,ServiceTarget::Native] {
            let error=compile_source(source,&config("[language]\nabsence='unified'"),ServiceOptions{target,..ServiceOptions::default()}).unwrap_err();
            assert_eq!(error.phase,"check","{source}: {error:?}");
            assert!(error.message.contains("R2"),"{source}: {error:?}");
        }
    }
    let result=compile_source("export bool exact(JsValue x){return x===null;}",&config("[language]\nabsence='unified'"),ServiceOptions::default()).unwrap();
    assert_eq!(execute_javascript(result.javascript(Objective::Brotli).unwrap().javascript(),"","console.log(library.exact(null),library.exact(undefined));"),"true false\n");
}

#[test]
fn s4_absence_adapts_host_defaults_callbacks_and_checks_pins() {
    let source=r#"
        extern int host(int n=9);
        extern bool inspect(func(int | undefined)->int | null callback);
        int? callback(int? x){return x;}
        export int run(int? x){return host(x);}
        export bool visit(){return inspect(callback);}
        export int take(int | null x){return x??5;}
        export int optional(int x=7){return x;}
        export int invoke(func(int | undefined)->int | null callback){return callback(null)??3;}
    "#;
    let result=compile_source(source,&config("checks='development'\n[language]\nabsence='unified'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),r#"
            globalThis.host=(n=9)=>n;
            globalThis.inspect=callback=>callback(undefined)===null;
        "#,r#"
            const outcomes=[library.run(null),library.run(0),library.visit(),library.take(null),library.optional(),library.invoke(x=>x===undefined?null:999)];
            for(const f of [()=>library.take(undefined),()=>library.optional(null)]){try{f();outcomes.push(false)}catch(e){outcomes.push(e instanceof TypeError)}}
            console.log(JSON.stringify(outcomes));
        "#),"[9,0,true,5,7,3,true,true]\n");
    }
}

#[test]
fn s4_absence_preserves_mutable_collection_aliases_and_later_writes() {
    let source=r#"
        export (int | undefined)[] values=[null,2];
        export Map<string,int | null> valuesByName=new Map<string,int | null>();
        export void change((int | undefined)[] target){target[1]=null;target.push(null);}
        export void changeMap(Map<string,int | null> target){int? missing=undefined;target.set("a",missing);}
        export (int | undefined)[] copy((int | null)[] source){return [...source];}
        export int?[] mapped(int[] source){return source.map((int value)=>{int? x=undefined;return x;});}
    "#;
    let result=compile_source(source,&config("[language]\nabsence='unified'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
            const a=library.values,m=library.valuesByName;library.change(a);library.changeMap(m);
            console.log(JSON.stringify([a===library.values,a.length,a.every(x=>x===undefined),m===library.valuesByName,m.get('a')===null,library.mapped([1,2]).every(x=>x===null),library.copy([null,null]).every(x=>x===undefined)]));
        "#),"[true,3,true,true,true,true,true]\n");
    }
    let error=compile_source("void f((int | undefined)[] a){(int | null)[] b=a;}",&config("[language]\nabsence='unified'"),ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check");
}

#[test]
fn s4_absence_config_types_and_native_omission_share_the_contract() {
    let legacy=ProjectConfig::default();
    let unified:ProjectConfig=toml::from_str("[language]\nabsence='unified'").unwrap();
    assert_ne!(legacy.language,unified.language);
    assert!(toml::from_str::<ProjectConfig>("[language]\nabsence='guess'").is_err());
    for effort in [0,13] {
        let mut settings=unified.clone();settings.effort.level=effort;
        let result=compile_source("int? pick(bool b){if(b){return undefined;}return 3;}int value(int x=9){return x;}print((pick(true)??4)+value());",
            &settings,ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap();
        assert_eq!(execute_native(result.native_c().unwrap()),"13\n");
    }
    let error=compile_source("int f(int x=1){return x;}int? x=null;print(f(x));",&unified,ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap_err();
    assert_eq!(error.phase,"check");assert!(error.message.contains("N2"),"{error:?}");
    let source="int? f(int? x){return x;}export int | undefined result(int | null x){return f(x);}export (int | undefined)[] list=[null];";
    let arena=bumpalo::Bump::new();
    let parsed=crate::parser::parse_source(&arena,source).unwrap();
    let checked=crate::check::analyze(&parsed).unwrap();
    let result=checked.symbols().iter().find(|symbol|symbol.name=="result").unwrap();
    let crate::check::Type::Function(signature)=&result.ty else{panic!("function type")};
    let crate::check::Type::Nullable(returns)=signature.return_type.as_ref() else{panic!("nullable result")};
    let crate::check::Type::Nullable(parameter)=&signature.params[0].ty else{panic!("nullable parameter")};
    assert_eq!(returns.boundary,crate::check::AbsencePin::Undefined);
    assert_eq!(parameter.boundary,crate::check::AbsencePin::Null);
}

#[test]
fn s4_absence_default_constructors_super_and_generics_share_the_call_contract() {
    let source = r#"
        class C{int value;init(int value=7){this.value=value;}}
        class D extends C{init(int? value){super(value);}}
        class Box<T>{T value;init(T fallback,T value=fallback){this.value=value;}}
        export constructor C;
        export constructor D;
        T choose<T>(T fallback,T value=fallback){return value;}
        export int legacySpelling(){C c=new C(JS.undefined());return c.value+choose(5,JS.undefined());}
        export int run(int? value){
            C c=new C(value);D d=new D(value);Box<int> b=new Box<int>(9,value);
            return c.value+d.value+b.value+choose(10,value)+choose(11,undefined);
        }
    "#;
    for checks in ["production", "development"] {
        let settings = config(&format!("checks='{checks}'\n[language]\nabsence='unified'"));
        let result = compile_source(source, &settings, ServiceOptions {
            objectives: Some(Objectives::All), ..ServiceOptions::default()
        }).unwrap();
        for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(), "", r#"
                console.log(JSON.stringify([library.run(null),library.run(2),new library.C().value,new library.D(null).value,library.legacySpelling()]));
            "#), "[44,19,7,7,12]\n");
        }
    }
    for source in [
        "class C{int value;init(int value=7){this.value=value;}}C c=new C(null);print(c.value);",
        "class C{int value;init(int value=7){this.value=value;}}class D extends C{init(int? value){super(value);}}D d=new D(null);print(d.value);",
    ] {
        let error = compile_source(source, &config("[language]\nabsence='unified'"), ServiceOptions {
            target: ServiceTarget::Native, ..ServiceOptions::default()
        }).unwrap_err();
        assert_eq!(error.phase, "check");
        assert!(error.message.contains("N2"), "{error:?}");
    }
}

#[test]
fn s4_char_code_number_keeps_nan_utf16_and_explicit_integer_conversion() {
    let source = r#"
        export number read(string value,int index){return value.charCodeAt(index);}
        export int integer(string value,int index){return value.charCodeAt(index)|0;}
        export number folded(){return "".charCodeAt(0);}
        export int unit(string value,int index){return value.codeUnitAt(index);}
    "#;
    for checks in ["production", "development"] {
        let settings = config(&format!("checks='{checks}'\n[language]\nchar_code_at='number'"));
        let result = compile_source(source, &settings, ServiceOptions {
            objectives: Some(Objectives::All), ..ServiceOptions::default()
        }).unwrap();
        for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(), "", r#"
                console.log(JSON.stringify([library.read('😀',0),library.read('😀',1),library.read('\ud800',0),Number.isNaN(library.read('a',-1)),Number.isNaN(library.read('a',1)),Number.isNaN(library.folded()),library.integer('a',9),library.unit('😀',1)]));
            "#), "[55357,56832,55296,true,true,true,0,56832]\n");
        }
    }
    let settings = config("[language]\nchar_code_at='number'");
    let error = compile_source("int code(string s){return s.charCodeAt(0);}", &settings, ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase, "check");
    assert!(toml::from_str::<ProjectConfig>("[language]\nchar_code_at='fast'").is_err());
}

#[test]
fn s4_char_code_native_uses_the_same_number_contract_before_and_after_folding() {
    let source = r#"
        number read(string value,int index){return value.charCodeAt(index);}
        number missing=read("a",3);
        print(missing!=missing);print(read("😀",1));print(read("\ud800",0));
        print(read("",0)|0);print("😀".codeUnitAt(0));
    "#;
    for (effort, folding) in [(0, false), (13, false), (13, true)] {
        let mut settings = config("[language]\nchar_code_at='number'");
        settings.effort.level = effort;
        if !folding {
            settings.policy.get_or_insert_with(PolicyConfig::default)
                .tactics.insert(TacticId::ConstantFolding, TacticPermission::Off);
        }
        let result = compile_source(source, &settings, ServiceOptions {
            target: ServiceTarget::Native, ..ServiceOptions::default()
        }).unwrap();
        if !folding { assert!(result.native_c().unwrap().contains("ls_char_code_at_number")); }
        assert_eq!(execute_native(result.native_c().unwrap()), "true\n56832\n55296\n0\n55357\n");
    }
}

#[test]
fn s4_enums_keep_abi_values_ordinals_collections_and_effect_order() {
    let source = r#"
        enum Kind:string{Text="text",Break="break",Empty=""}
        enum Code:int{A=65,B=-7}
        flags enum Access:int{Read=1,Write=4,High=-2147483648}
        shape Token{data Kind kind;data Access access;}
        extern string supply();
        export Kind? parse(string value){return Kind.from(value);}
        export Kind? effect(){return Kind.from(supply());}
        export int ordinal(Kind kind){return kind.ordinal;}
        export string abi(Kind kind){return kind.abi;}
        export int numeric(Code code){return code.ordinal*100+code.abi;}
        export Code? code(int value){return Code.from(value);}
        export Access combine(Access a,Access b){return a|b;}
        export Access? access(int value){return Access.from(value);}
        export bool has(Access a,Access b){return a.has(b);}
        export int classify(Kind kind){return match(kind){Kind.Text=>1,Kind.Break=>2,Kind.Empty=>3};}
        export string optional(string text){return Kind.from(text)?.abi??"missing";}
        export Token make(){return Token{kind:Kind.Text,access:Access.Read|Access.Write};}
        export void update(Token token,Kind[] values){token.kind=Kind.Break;values.push(Kind.Empty);}
        export int read(Token token){return token.kind.ordinal;}
        export bool same(Kind[] values){return values.includes(Kind.Text);}
    "#;
    for checks in ["production", "development"] {
        let result = compile_source(source, &config(&format!("checks='{checks}'\n[language]\nenum_abi='explicit'")), ServiceOptions {
            objectives: Some(Objectives::All), ..ServiceOptions::default()
        }).unwrap();
        for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(), "globalThis.count=0;globalThis.supply=()=>{count++;return 'break'};", r#"
                const values=['text'],token=library.make();library.update(token,values);
                console.log(JSON.stringify([library.parse('text'),library.parse('bad'),library.effect(),count,library.ordinal('break'),library.abi('text'),library.numeric(-7),library.code(65),library.code(0),library.combine(1,4),library.access(5),library.access(2),library.access(-2147483648),library.has(5,4),library.has(1,4),library.classify(''),library.optional('bad'),library.optional('text'),token,values,library.read(token),library.same(values)]));
            "#), "[\"text\",null,\"break\",1,1,\"text\",93,65,null,5,5,null,-2147483648,true,false,3,\"missing\",\"text\",{\"kind\":\"break\",\"access\":5},[\"text\",\"\"],1,true]\n");
        }
        if checks == "development" {
            assert_eq!(execute_javascript(result.javascript(Objective::Raw).unwrap().javascript(), "globalThis.supply=()=>'';", r#"
                const results=[];let hooks=0;
                for(const f of [()=>library.ordinal('bad'),()=>library.numeric(0),()=>library.combine(2,1),()=>library.combine({valueOf(){hooks++;return 1}},1),()=>library.read({kind:'bad',access:1})]){try{f();results.push(false)}catch(e){results.push(e instanceof TypeError)}}
                results.push(hooks);console.log(JSON.stringify(results));
            "#), "[true,true,true,true,true,0]\n");
        }
    }
}

#[test]
fn s4_enums_native_and_shared_folding_preserve_declared_domains() {
    let source = r#"
        enum Kind:string{Text="text",Break="break"}
        enum Code:int{A=65,B=-7}
        flags enum Access:int{Read=1,Write=4,High=-2147483648}
        int read(string text){Kind? kind=Kind.from(text);if(kind!=null){return kind.ordinal;}return -1;}
        print(read("break"));print(read("bad"));print(Code.B.abi);print(Code.B.ordinal);
        Access flags=Access.Read|Access.Write;print(flags.abi);print(flags.has(Access.Write));
        print(Access.from(2)==null);print(Access.from(-2147483648)!=null);print(Kind.Text);print(Kind.Text.abi);
    "#;
    for effort in [0, 13] {
        let mut settings = config("[language]\nenum_abi='explicit'");settings.effort.level=effort;
        let result = compile_source(source, &settings, ServiceOptions {target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap();
        assert_eq!(execute_native(result.native_c().unwrap()), "1\n-1\n-7\n1\n5\ntrue\ntrue\ntrue\ntext\ntext\n");
    }
}

#[test]
fn s4_enums_reject_invalid_domains_and_unpinned_observations() {
    for source in [
        "enum E:string{A=\"same\",B=\"s\\u0061me\"}",
        "enum E:int{A=1,B=1}",
        "enum E:int{A=2147483648}",
        "enum E{A=1}",
        "enum E:string{A}",
        "flags enum E:int{A=3}",
        "flags enum E:string{A=\"a\"}",
        "enum E{A,B}print(E.A);",
        "enum E{A,B}export E value(){return E.A;}",
        "enum E{A,B}print(JSON.stringify([E.A]));",
        "flags enum E:int{A=1,B=2}int f(E value){return match(value){E.A=>1,E.B=>2};}",
    ] {
        let error = compile_source(source, &config("[language]\nenum_abi='explicit'"), ServiceOptions::default()).unwrap_err();
        assert!(matches!(error.phase, "parse" | "check"), "{source}: {error:?}");
    }
    let result=compile_source("enum E{A,B}export int ordinal(){return E.B.ordinal;}",&config("[language]\nenum_abi='explicit'"),ServiceOptions::default()).unwrap();
    assert_eq!(execute_javascript(result.javascript(Objective::Brotli).unwrap().javascript(),"","console.log(library.ordinal());"),"1\n");
    assert!(toml::from_str::<ProjectConfig>("[language]\nenum_abi='guessed'").is_err());
    let result = compile_source("enum Kind:string{A=\"a\"}class Converter{int from(int n){return n;}}export int shadow(){Converter Kind=new Converter();return Kind.from(7);}", &config("[language]\nenum_abi='explicit'"), ServiceOptions::default()).unwrap();
    assert_eq!(execute_javascript(result.javascript(Objective::Brotli).unwrap().javascript(), "", "console.log(library.shadow());"), "7\n");
}

const S4_PAYLOAD_CLASSES: &str = r#"
    class Text{string text;init(string text){this.text=text;}}
    class Count{int count;init(int count){this.count=count;}}
    int calls=0;
    Text|Count choose(bool text){calls++;if(text){return new Text("hello");}return new Count(9);}
    export int run(bool text){return match(choose(text)){Text(item)=>item.text.length,Count(item)=>item.count};}
    export int captured(bool text){auto fn=match(choose(text)){Text(item)=>(()=>item.text.length),Count(item)=>(()=>item.count)};return fn();}
    export int count(){return calls;}
    int identity(int value){return value;}
    export int single(){return identity(match(new Count(6)){Count(item)=>item.count});}
    export int loop(){auto callbacks=[()=>0];for(int i=0;i<3;i++){Text|Count value=new Count(i);auto callback=match(value){Text(item)=>(()=>item.text.length),Count(item)=>(()=>item.count)};callbacks.push(callback);}return callbacks[1]()*100+callbacks[2]()*10+callbacks[3]();}
"#;

const S4_VIRTUAL_CLASSES: &str = r#"
    int trace=0;
    int mark(int digit){trace=trace*10+digit;return digit;}
    sealed class Base{
        virtual int value(int n,int delta=mark(3)){return n+delta;}
        virtual int apply(func(int)->int f,int delta=2){return f(10)+delta;}
        virtual int sum(int first=1,int... rest){int n=first;for(int v of rest){n+=v;}return n;}
    }
    class Derived extends Base{
        override int value(int n,int delta=mark(4)){return n*10+delta;}
        override int apply(func(int)->int f,int delta=3){return f(20)+delta;}
        override int sum(int first=2,int... rest){int n=first*10;for(int v of rest){n+=v;}return n;}
    }
    class Deep extends Derived{
        override int value(int n,int delta=mark(5)){return n*100+delta;}
        override int apply(func(int)->int f,int delta=4){return f(30)+delta;}
    }
    Base choose(int kind){mark(1);if(kind==1){return new Derived();}if(kind==2){return new Deep();}return new Base();}
    export int order(int kind){trace=0;int result=choose(kind).value(mark(2));return result*1000+trace;}
    export int captured(int kind){int n=5;int result=choose(kind).apply((int x)=>{n++;return x+n;});return result+n*100;}
    export int rest(int kind){Base value=choose(kind);return value.sum()+value.sum(3,4,5);}
"#;

#[test]
fn s4_variants_payload_scopes_closures_and_native_identity() {
    let source=format!("{S4_PAYLOAD_CLASSES}print(run(true));print(run(false));print(captured(true));print(captured(false));print(count());print(loop());print(single());");
    let expected="5\n9\n5\n9\n4\n12\n6\n";
    let arena=bumpalo::Bump::new();let parsed=crate::parser::parse_source(&arena,&source).unwrap();let checked=crate::check::analyze(&parsed).unwrap();
    assert_eq!(crate::interpreter::interpret_program(&parsed,&checked).unwrap(),expected);
    let result=compile_source(&source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",""),expected);
    }
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        let native=compile_source(&source.replace("export ",""),&settings,ServiceOptions{target:ServiceTarget::Native,preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
        assert_eq!(execute_native(native.native_c().unwrap()),expected);
    }
}

#[test]
fn s4_variants_virtual_defaults_closures_rest_and_native_dispatch() {
    let source=format!("{S4_VIRTUAL_CLASSES}print(order(0));print(order(1));print(order(2));print(captured(0));print(captured(1));print(captured(2));print(rest(0));print(rest(1));print(rest(2));");
    let expected="5123\n24124\n205125\n618\n629\n640\n13\n59\n59\n";
    let arena=bumpalo::Bump::new();let parsed=crate::parser::parse_source(&arena,&source).unwrap();let checked=crate::check::analyze(&parsed).unwrap();
    assert_eq!(crate::interpreter::interpret_program(&parsed,&checked).unwrap(),expected);
    for checks in ["production","development"] {
        let result=compile_source(&source,&config(&format!("checks='{checks}'")),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",""),expected);
        }
    }
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        let native=compile_source(&source.replace("export ",""),&settings,ServiceOptions{target:ServiceTarget::Native,preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
        assert_eq!(execute_native(native.native_c().unwrap()),expected);
    }
}

#[test]
fn s4_variants_tagged_payloads_validate_and_mutate_the_original() {
    let source=r#"
        shape Text{tag string kind="text";data string value;}
        shape Count{tag string kind="count";data int value;}
        int advance(Count count){count.value++;return count.value;}
        export int read(Text|Count value){return match(value){Text(item)=>item.value.length,Count(item)=>advance(item)};}
        export int fallback(Text|Count value){return match(value){Text(item)=>item.value.length,_=>-1};}
    "#;
    for checks in ["production","development"] {
        let result=compile_source(source,&config(&format!("checks='{checks}'")),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"",r#"
                const a={kind:'text',value:'hello'},b={kind:'count',value:8};
                console.log(JSON.stringify([library.read(a),library.read(b),b.value,library.fallback(a),library.fallback(b)]));
            "#),"[5,9,9,5,-1]\n");
        }
    }
}

#[test]
fn s4_variants_ref_arguments_keep_their_preparation_order() {
    let source=r#"
        int change(ref int n,int value){n+=value;return n;}
        sealed class Base{virtual int score(int n){return n+1;}}
        class Derived extends Base{override int score(int n){return n+2;}}
        Base choose(bool b){if(b){return new Derived();}return new Base();}
        export int run(bool b){Base value=choose(b);int n=1;int result=value.score(change(ref n,n+=2));return result*10+n;}
    "#;
    let result=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(objective).unwrap().javascript(),"","console.log(library.run(false));console.log(library.run(true));"),"76\n86\n");
    }
    let native=compile_source(&format!("{}print(run(false));print(run(true));",source.replace("export ","")),&config(""),ServiceOptions{target:ServiceTarget::Native,preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_native(native.native_c().unwrap()),"76\n86\n");
}

#[test]
fn s4_variants_refuse_ambiguous_incomplete_or_incompatible_contracts() {
    for source in [
        "class A{}class B{}int f(A|B value){return match(value){A(a)=>1};}",
        "class A{}class B{}int f(A|B value){return match(value){A(a)=>1,A(b)=>2,B(b)=>3};}",
        "class A{}class B{}int f(A|B value){return match(value){A(a)=>1,B(b)=>a.x};}",
        "shape A{tag int kind=1;}shape B{tag int kind=1;}int f(A|B value){return match(value){A(a)=>1,B(b)=>2};}",
        "shape A{tag int a=1;}shape B{tag int b=2;}int f(A|B value){return match(value){A(a)=>1,B(b)=>2};}",
        "class A{virtual int f(){return 1;}}",
        "class A{int f(){return 1;}}class B extends A{override int f(){return 2;}}",
        "sealed class A{virtual int f(){return 1;}}class B extends A{int f(){return 2;}}",
        "sealed class A{virtual int f(int n=1){return n;}}class B extends A{override int f(int n){return n;}}",
        "sealed class A{virtual pure int f(){return 1;}}class B extends A{override int f(){print(1);return 2;}}",
        "class B{override int f(){return 2;}}",
        "sealed class A{virtual int f(){return 1;}}A a=new A();auto callback=a.f;",
    ] {
        let error=compile_source(source,&config(""),ServiceOptions::default()).unwrap_err();
        assert!(matches!(error.phase,"check"|"parse"),"{source}: {error:?}");
    }
}

#[test]
fn s4_variants_reference_enum_model_uses_abi_values() {
    let source=r#"enum Kind:string{Text="text",Break="break"}flags enum Bits:int{A=1,B=4}print(Kind.Text);print(Kind.Break.ordinal);print(Kind.from("text")!=null);print(Kind.from("bad")==null);print((Bits.A|Bits.B).abi);print((Bits.A|Bits.B).has(Bits.B));"#;
    let arena=bumpalo::Bump::new();let parsed=crate::parser::parse_source(&arena,source).unwrap();let checked=crate::check::analyze(&parsed).unwrap();
    assert_eq!(crate::interpreter::interpret_program(&parsed,&checked).unwrap(),"text\n1\ntrue\ntrue\n5\ntrue\n");
}

#[test]
fn s4_variants_dispatch_before_subclass_initialization_and_public_prototypes() {
    let source=r#"
        export sealed class Base{virtual int read(int n=1){return n;}}
        export int read(Base value){return value.read();}
        print(read(new Base()));
        export class Derived extends Base{init(){super();}override int read(int n=2){return n*10;}}
        export constructor Base;export constructor Derived;
        print(read(new Derived()));
    "#;
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("types.lil"),source).unwrap();
    std::fs::write(scratch.0.join("entry.lil"),"import {Base,Derived,read} from \"./types.lil\";export {Base,Derived,read};export constructor Base;export constructor Derived;").unwrap();
    for graph in [false,true] {
        let options=ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()};
        let compiled=if graph {compile_path(&scratch.0.join("entry.lil"),&config(""),options)}else{compile_source(source,&config(""),options)}.unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(compiled.javascript(objective).unwrap().javascript(),"",r#"
                const a=new library.Base(),b=new library.Derived();
                console.log(JSON.stringify([library.read(a),library.read(b),a.read(7),b.read(7),b instanceof library.Base]));
            "#),"1\n20\n[1,20,7,70,true]\n");
        }
        check_scores(&compiled);
    }
    let native_source=source.replace("export constructor Base;export constructor Derived;","").replace("export ","");
    let native=compile_source(&native_source,&config(""),ServiceOptions{target:ServiceTarget::Native,preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_native(native.native_c().unwrap()),"1\n20\n");
}

#[test]
fn s4_erased_variants_generic_virtual_binders_defaults_and_native_boxing() {
    let source=r#"
        sealed class Base<T>{T stored;init(T value){this.stored=value;}
            virtual T pick(T value,bool stored=false){if(stored){return this.stored;}return value;}
            virtual U echo<U>(U value){return value;}
        }
        class Child<S> extends Base<S>{init(S value){super(value);}
            override S pick(S value,bool stored=true){if(stored){return this.stored;}return value;}
            override V echo<V>(V value){return value;}
        }
        class Fixed extends Base<int>{init(int value){super(value);}
            override int pick(int value,bool stored=true){return value+this.stored;}
        }
        string choose(bool child){Base<string> value=new Base<string>("base");if(child){value=new Child<string>("child");}return value.pick("given");}
        int numeric(){Base<int> value=new Fixed(40);return value.pick(8);}
        int method(){Base<string> value=new Child<string>("x");auto fn=value.echo((int n)=>n+1);return value.echo(7)+fn(2);}
        print(choose(false));print(choose(true));print(numeric());print(method());
    "#;
    let arena=bumpalo::Bump::new();let parsed=crate::parser::parse_source(&arena,source).unwrap();let checked=crate::check::analyze(&parsed).unwrap();
    let expected="given\nchild\n48\n10\n";
    assert_eq!(crate::interpreter::interpret_program(&parsed,&checked).unwrap(),expected);
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",""),expected);
    }
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        let native=compile_source(source,&settings,ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap();
        assert_eq!(execute_native(native.native_c().unwrap()),expected);
    }
}

#[test]
fn s4_erased_variants_applied_payloads_use_nominal_identity_only() {
    let source=r#"
        class Left<T>{T value;init(T value){this.value=value;}}
        class Right<T>{T value;init(T value){this.value=value;}}
        Left<int>|Right<string> choose(bool left){if(left){return new Left<int>(7);}return new Right<string>("hello");}
        int read(bool left){return match(choose(left)){Left(item)=>item.value,Right(item)=>item.value.length};}
        print(read(true));print(read(false));
    "#;
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),target:ServiceTarget::All,..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",""),"7\n5\n");
    }
    assert_eq!(execute_native(built.native_c().unwrap()),"7\n5\n");
    let source=r#"
        shape Left<T>{tag string kind="left";data T value;}
        shape Right<T>{tag string kind="right";data T value;}
        export int read(Left<int>|Right<string> value){return match(value){Left(item)=>item.value,Right(item)=>item.value.length};}
    "#;
    let built=compile_source(source,&config("checks='development'"),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"","console.log(library.read({kind:'left',value:7}));console.log(library.read({kind:'right',value:'hello'}));"),"7\n5\n");
    }
    for source in [
        "class Box<T>{T value;}int read(Box<int>|Box<string> b){return match(b){Box(v)=>1};}",
        "sealed class B{virtual T pick<T>(T v){return v;}}class C extends B{override int pick<U>(U v){return 1;}}",
        "sealed class B<T>{virtual T pick(T v){return v;}}class C extends B<int>{override string pick(string v){return v;}}",
    ] {
        let error=compile_source(source,&config(""),ServiceOptions::default()).unwrap_err();
        assert_eq!(error.phase,"check","{error:?}");
    }
}

#[test]
fn s4_erased_variants_await_keeps_the_original_promise_schedule() {
    let source=r#"
        extern Task<int> tick(int n);
        class Left{int value=7;}class Right{int value=9;}
        Left|Right choose(bool left){if(left){return new Left();}return new Right();}
        export async int run(bool left){return match(choose(left)){Left(item)=>item.value+await tick(3),Right(item)=>(await tick(item.value))+1};}
    "#;
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"globalThis.events=[];globalThis.tick=n=>{events.push('tick'+n);return Promise.resolve(n);};",r#"
            async function reference(left){return left?7+await tick(3):(await tick(9))+1;}
            async function trace(fn,left){events.length=0;const pending=fn(left);queueMicrotask(()=>events.push('queued'));pending.then(n=>events.push('done'+n));await pending;await 0;return [...events];}
            for(const left of [true,false]){const expected=await trace(reference,left);const actual=await trace(library.run,left);if(JSON.stringify(actual)!==JSON.stringify(expected))throw Error(JSON.stringify({actual,expected}));}
            console.log('same');
        "#),"same\n");
    }
}

#[test]
fn s4_erased_variants_suspending_captures_have_fresh_shared_cells() {
    let source=r#"
        extern Task<int> save(func()->int read,func()->int write,int n);
        extern Task<int> saveNested(func()->func()->int factory,int n);
        class Left{int value;init(int n){this.value=n;}}
        class Right{int value;init(int n){this.value=n;}}
        Left|Right choose(int n){if(n==1){return new Right(n);}return new Left(n);}
        export async int run(){int sum=0;for(int i=0;i<3;i++){
            sum+=match(choose(i)){
                Left(item)=>await save(()=>item.value,()=>{item=new Left(item.value+10);return item.value;},i),
                Right(item)=>await save(()=>item.value,()=>{item=new Right(item.value+10);return item.value;},i)
            };
        }return sum;}
        export async int condition(){int i=0;while(match(choose(i)){
            Left(item)=>await save(()=>item.value,()=>{item=new Left(item.value+10);return item.value;},i),
            Right(item)=>await save(()=>item.value,()=>{item=new Right(item.value+10);return item.value;},i)
        }<2){i++;}return i;}
        export async int nested(){int sum=0;for(int i=0;i<3;i++){
            sum+=match(new Left(i)){Left(item)=>await saveNested(()=>()=>item.value,i)};
        }return sum;}
    "#;
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    let setup=r#"
        globalThis.events=[];globalThis.readers=[];globalThis.writers=[];globalThis.factories=[];
        globalThis.save=(read,write,n)=>{readers.push(read);writers.push(write);events.push('save'+read());return Promise.resolve(n);};
        globalThis.saveNested=(factory,n)=>{factories.push(factory);events.push('nested'+n);return Promise.resolve(n);};
    "#;
    let oracle=r#"
        async function reference(mode){let sum=0;for(let i=0;i<3;i++){
            let item={value:i};
            if(mode==='nested'){sum+=await saveNested(()=>()=>item.value,i);continue;}
            const value=await save(()=>item.value,()=>{item={value:item.value+10};return item.value;},i);
            if(mode==='condition'&&value>=2)return i;
            sum+=value;
        }return sum;}
        async function trace(fn){events.length=readers.length=writers.length=factories.length=0;
            const pending=fn();queueMicrotask(()=>events.push('queued'));
            pending.then(n=>events.push('done'+n));await pending;await 0;
            const before=readers.map(f=>f());const changes=writers.map(f=>f());
            const after=readers.map(f=>f());const nested=factories.map(f=>f()());
            return JSON.stringify({events:[...events],before,changes,after,nested});
        }
        for(const mode of ['run','condition','nested']){
            const expected=await trace(()=>reference(mode));const actual=await trace(library[mode]);
            if(actual!==expected)throw Error(JSON.stringify({mode,actual,expected}));
        }
        console.log('same');
    "#;
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),setup,oracle),"same\n");
    }
}

#[test]
fn s4_host_catalog_generic_extern_schemas_and_subclasses() {
    let source=r#"
        extern class Host<T>{init(T value);T value;T choose(T other);U echo<U>(U other);}
        extern class Extended<T> extends Host<T>{string label;}
        extern Extended<string> existing;
        class Child extends Host<string>{init(string value){super(value);}string read(){return this.value;}}
        export string run(){Child child=new Child("child");existing.value="changed";return existing.choose("pick")+existing.label+child.read()+existing.echo(7).toString();}
    "#;
    let setup=r#"
        globalThis.Host=class {constructor(value){this.value=value;}choose(other){return this.value+other;}echo(other){return other;}};
        globalThis.Extended=class extends Host {constructor(value){super(value);this.label='label';}};
        globalThis.existing=new Extended('initial');
    "#;
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),setup,"console.log(library.run());console.log(existing.value);"),"changedpicklabelchild7\nchanged\n");
    }
}

#[test]
fn s4_host_catalog_modules_share_declarations_and_preserve_host_receivers() {
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("helper.lil"),r#"import {document} from "lil:dom";export string title(){return document.title;}"#).unwrap();
    let source=r#"
        import {document as page} from "lil:dom";
        import {console as logger, performance as clock, queueMicrotask} from "lil:ecmascript";
        import {title} from "./helper";
        export string run(){auto element=page.createElement("button");element.setAttribute("id","go");logger.info(title());return (element.getAttribute("id")??"missing")+clock.now().toInt().toString();}
        export void later(){queueMicrotask(()=>{logger.info("later");});}
    "#;
    std::fs::write(scratch.0.join("entry.lil"),source).unwrap();
    let setup=r#"
        globalThis.events=[];
        globalThis.document={title:'catalog',createElement(name){if(this!==document)throw Error('document receiver');events.push(name);return {values:{},setAttribute(k,v){this.values[k]=v;},getAttribute(k){return this.values[k]??null;}};}};
        globalThis.performance={now(){if(this!==performance)throw Error('performance receiver');return 12;}};
        globalThis.console.info=function(value){if(this!==console)throw Error('console receiver');events.push(value);};
    "#;
    let built=compile_path(&scratch.0.join("entry.lil"),&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),setup,"console.log(library.run());library.later();await 0;console.log(JSON.stringify(events));"),"go12\n[\"button\",\"catalog\",\"later\"]\n");
    }
    std::fs::write(scratch.0.join("entry.lil"),"import {document} from \"lil:dom\";print(3);").unwrap();
    let unused=compile_path(&scratch.0.join("entry.lil"),&config(""),ServiceOptions::default()).unwrap();
    let plain=compile_source("print(3);",&config(""),ServiceOptions::default()).unwrap();
    assert_eq!(unused.javascript(Objective::Brotli).unwrap().javascript(),plain.javascript(Objective::Brotli).unwrap().javascript());
    let error=compile_path(&scratch.0.join("entry.lil"),&config(""),ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");
}

#[test]
fn s4_host_catalog_invalid_names_and_eval_are_source_diagnostics() {
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("entry.lil"),"import {missing} from \"lil:missing\";print(1);").unwrap();
    let error=compile_path(&scratch.0.join("entry.lil"),&config(""),ServiceOptions::default()).unwrap_err();
    assert!(format!("{error:?}").contains("unknown platform catalog"),"{error:?}");
    let error=compile_source("extern int eval(string source);",&config(""),ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");
    assert!(format!("{error:?}").contains("direct eval"),"{error:?}");
}

#[test]
fn s4_host_catalog_toml_paths_keep_checked_identity_and_call_order() {
    let source=r#"
        extern int first(int n);extern int second(int n);extern int next();
        extern class Host{init(int n);int value;int read();}
        class Child extends Host{init(int n){super(n);}}
        export int run(){return first(next())*100+second(next())+new Child(3).read();}
    "#;
    let flags="[host.javascript]\nfirst='second'\nsecond='Runtime.second'\nnext='Runtime.next'\nHost='Runtime.Base'";
    let setup=r#"
        globalThis.trace=[];let n=0;
        Object.defineProperty(globalThis,'second',{configurable:true,get(){trace.push('first');return x=>{trace.push('call1:'+x);return x+10;};}});
        globalThis.Runtime={get second(){trace.push('second');return x=>{trace.push('call2:'+x);return x+20;};},next(){trace.push('next');return ++n;},Base:class{constructor(n){this.value=n;}read(){return this.value;}}};
    "#;
    for effort in [0,13] {
        let mut settings=config(flags);settings.effort.level=effort;
        let built=compile_source(source,&settings,ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),setup,"trace.length=0;console.log(library.run());console.log(JSON.stringify(trace));"),"1125\n[\"first\",\"next\",\"call1:1\",\"second\",\"next\",\"call2:2\"]\n");
        }
    }
    let request=CompilationRequest::JavaScript{preserve_root_exports:true};
    assert_ne!(config("").resolve_policy(request).unwrap().fingerprint(),config(flags).resolve_policy(request).unwrap().fingerprint());
}

#[test]
fn s4_host_catalog_native_provider_configuration_delivers_a_matching_header() {
    let source="extern int compute(int n);extern T identity<T>(T value);print(compute(7));print(identity(12));print(identity(\"ok\"));auto values=identity([3,4]);print(values[1]);";
    let settings=config("[host.javascript]\ncompute='Runtime.compute'\nidentity='Runtime.identity'\n[host.native]\ncompute='host_compute'\nidentity='host_identity'");
    let built=compile_source(source,&settings,ServiceOptions{target:ServiceTarget::All,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_javascript(built.javascript(Objective::Brotli).unwrap().javascript(),"globalThis.Runtime={compute:n=>n*3,identity:value=>value};",""),"21\n12\nok\n4\n");
    let scratch=Scratch::new();let source_path=scratch.0.join("main.c");
    std::fs::write(&source_path,built.native_c().unwrap()).unwrap();
    std::fs::write(scratch.0.join("main.h"),built.native_header().unwrap()).unwrap();
    std::fs::write(scratch.0.join("provider.c"),"#include \"main.h\"\nint32_t host_compute(int32_t n){return n*3;}\nhost_identity_result host_identity(host_identity_arg0 value){ls_value_retain(value);return value;}\n").unwrap();
    let executable=scratch.0.join("main");
    let cc=std::env::var_os("LILSCRIPT_NATIVE_CC").unwrap_or_else(||"cc".into());
    let output=Command::new(cc).args(["-std=c11","-O1","-fno-fast-math","-ffp-contract=off"])
        .arg(&source_path).arg(scratch.0.join("provider.c")).args(["-lm","-o"]).arg(&executable).output().unwrap();
    assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));
    let output=Command::new(executable).output().unwrap();assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(),"21\n12\nok\n4\n");
    let error=compile_source(source,&config(""),ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");assert!(format!("{error:?}").contains("host.native"));
    let error=compile_source("extern int compute(int n=1);print(compute());",&settings,ServiceOptions{target:ServiceTarget::Native,..ServiceOptions::default()}).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");
}

#[test]
fn s4_host_catalog_binding_configuration_refuses_ambiguous_or_invalid_contracts() {
    for flags in ["[host.javascript]\nf='x;evil()'", "[host.javascript]\nf='eval'", "[host.native]\nf='not_reserved'", "[host.native]\nf='host_same'\ng='host_same'"] {
        assert!(config(flags).validate().is_err(),"{flags}");
    }
    let error=compile_source("int f(){return 1;}print(f());",&config("[host.javascript]\nf='runtime.f'"),ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("entry.lil"),"import extern {f} from \"./host.js\";extern int f();print(f());").unwrap();
    std::fs::write(scratch.0.join("host.js"),"export function f(){return 1;}").unwrap();
    let error=compile_path(&scratch.0.join("entry.lil"),&config("[host.javascript]\nf='runtime.f'"),ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check","{error:?}");assert!(format!("{error:?}").contains("conflicts with its import extern"));
}


const INLINE_FOR_CAPTURES: &str = r#"
    (func()->int)[] reads=[];
    (func()->int)[] changes=[];
    inline for(int unused of []){print(999);}
    inline for(int outer of [1,2]) {
        int local=outer*10;
        inline for(int inner of [3,4]) {
            reads.push(()=>outer*100+local+inner);
            changes.push(()=>{outer+=1;local+=2;inner+=3;return outer*100+local+inner;});
        }
    }
    print(reads[0]());print(reads[1]());print(reads[2]());print(reads[3]());
    print(changes[0]());print(reads[0]());print(reads[1]());print(reads[2]());
    print(changes[3]());print(reads[2]());print(reads[3]());
    int early(){inline for(int n of [1,2,3]){if(n==2){return n;}}return 99;}
    print(early());
    inline for(float x of [1,2.5]){print(x);}
    inline for(int? x of [null,7]){print(x??9);}
    inline for(bool x of [true,false]){print(x);}
    inline for(string x of ["a","b"]){print(x);}
    (func(int)->int)[] make(int offset){
        (func(int)->int)[] results=[];
        inline for(int n of [1,2]){results.push((int x=n)=>{n+=x;return offset+n;});}
        return results;
    }
    auto made=make(100);print(made[0](1));print(made[0](3));print(made[1](2));
"#;
const INLINE_FOR_EXPECTED: &str = "113\n114\n223\n224\n218\n218\n216\n223\n329\n325\n329\n2\n1\n2.5\n9\n7\ntrue\nfalse\na\nb\n102\n105\n104\n";

#[test]
fn s4_inline_for_expands_with_independent_captures_on_both_targets() {
    for effort in [0,13] {
        let mut settings=config("[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'");
        settings.effort.level=effort;
        let built=compile_source(INLINE_FOR_CAPTURES,&settings,ServiceOptions {
            target: ServiceTarget::All, objectives:Some(Objectives::All), ..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",""),INLINE_FOR_EXPECTED);
        }
        assert_eq!(execute_native(built.native_c().unwrap()),INLINE_FOR_EXPECTED);
        check_scores(&built);
    }
}

#[test]
fn s4_inline_for_permission_conflicts_are_source_diagnostics() {
    let settings=config("[policy.tactics]\nloop-unrolling='off'");
    for target in [ServiceTarget::JavaScript,ServiceTarget::Native,ServiceTarget::All] {
        let error=compile_source("inline for(int x of [1,2]){print(x);}",&settings,ServiceOptions {
            target, ..ServiceOptions::default()
        }).unwrap_err();
        assert!(error.to_string().contains("loop-unrolling='off'"),"{error}");
    }
}


#[test]
fn s4_pool_pins_survive_folding_inlining_and_every_objective() {
    let source=r#"
        @pool string small(){return "x";}
        @pool func()->string captured(string value="default"){return ()=>`prefix:${value}`;}
        class Labels { @pool string one(){return "one";} }
        string label(bool condition){Labels labels=new Labels();if(condition){return small();}return labels.one();}
        func()->string get(){return captured();}
        @pool string lone(){return "\uD800";}
        @pool string empty(){return ``;}
        print(lone().codeUnitAt(0));print(label(true));print(label(false));print(get()());print(empty().length);
    "#;
    for effort in [0,13] {
        let mut settings=config("");settings.effort.level=effort;
        let built=compile_source(source,&settings,ServiceOptions {
            target:if effort==0 {ServiceTarget::All} else {ServiceTarget::JavaScript},
            preserve_root_exports:false, objectives:Some(Objectives::All), ..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            assert_eq!(execute_javascript(built.javascript(codec).unwrap().javascript(),"",""),"55296\nx\none\nprefix:default\n0\n");
        }
        if let Some(native)=built.native_c() {assert_eq!(execute_native(native),"55296\nx\none\nprefix:default\n0\n");}
        check_scores(&built);
    }
}

#[test]
fn s4_pool_diagnoses_source_conflicts_and_misplaced_attributes() {
    let settings=config("[policy.tactics]\nstring-pooling='off'");
    let error=compile_source("@pool string value(){return \"small\";}print(value());",&settings,ServiceOptions::default()).unwrap_err();
    assert!(error.to_string().contains("string-pooling='off'"),"{error}");
    for source in ["@pool extern string f();","@pool class C{}","@pool struct S{}","@pool enum E{A}","@pool shape S{}","class C{@pool string x;}"] {
        let arena=bumpalo::Bump::new();
        assert!(crate::parse_source(&arena,source).is_err(),"{source}");
    }
}

#[test]
fn s4_choose_keeps_neighboring_quote_pins_through_inlining_and_each_objective() {
    let source=r#"
        @choose(quote_delimiter=0) string first(string value){return "q\"a"+value;}
        @choose(quote_delimiter=1) string second(string value){return "plain"+value;}
        string both(string value){return first(value)+second(value);}
        print(both("x"));
    "#;
    for effort in [0,13] {
        let mut settings=config("[policy.tactics]\nconstant-folding='off'");settings.effort.level=effort;
        let built=compile_source(source,&settings,ServiceOptions {
            target:if effort==0 {ServiceTarget::All} else {ServiceTarget::JavaScript},
            preserve_root_exports:false,objectives:Some(Objectives::All),..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            let code=built.javascript(codec).unwrap().javascript();
            assert_eq!(execute_javascript(code,"",""),"q\"axplainx\n");
            assert!(code.contains(r#""q\"a""#),"{effort}/{codec:?}: {code}");
            assert!(code.contains("'plain'"),"{effort}/{codec:?}: {code}");
        }
        if let Some(native)=built.native_c() {assert_eq!(execute_native(native),"q\"axplainx\n");}
        check_scores(&built);
    }
}

#[test]
fn s4_choose_diagnoses_unknown_misplaced_duplicate_and_forbidden_choices() {
    for source in [
        "@choose(quote_delimiter=2) string f(){return \"x\";}",
        "@choose(quote_delimiter=0) @choose(quote_delimiter=1) string f(){return \"x\";}",
        "@choose(name_allocation=1) string f(){return \"x\";}",
        "@choose(unknown=1) string f(){return \"x\";}",
        "@choose(quote_delimiter=1) class C{}",
        "class C{@choose(quote_delimiter=1) string field;}",
    ] {
        let arena=bumpalo::Bump::new();assert!(crate::parse_source(&arena,source).is_err(),"{source}");
    }
    for flag in ["statement-spellings", "target-compaction"] {
        let settings=config(&format!("[policy.tactics]\n{flag}='off'"));
        let error=compile_source("@choose(quote_delimiter=1) string label(){return \"x\";}print(label());",&settings,ServiceOptions::default()).unwrap_err();
        assert!(error.to_string().contains("@choose conflicts"),"{error}");
        compile_source("@choose(quote_delimiter=0) string label(){return \"x\";}print(label());",&settings,ServiceOptions::default()).unwrap();
    }
}

#[test]
fn s4_choose_canonical_return_survives_shared_normalization() {
    let source=r#"
        @choose(conditional_returns=0) int first(bool flag,int n){if(flag){return n+1;}return n+2;}
        @choose(conditional_returns=1) int second(bool flag,int n){if(flag){return n+3;}return n+4;}
        export {first,second};
    "#;
    let built=compile_source(source,&config(""),ServiceOptions {objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        let code=built.javascript(codec).unwrap().javascript();
        assert!(code.contains("if(") && code.contains('?'),"{code}");
        assert_eq!(execute_javascript(code,"","console.log(library.first(true,5),library.first(false,5),library.second(true,5),library.second(false,5));"),"6 7 8 9\n");
    }
    check_scores(&built);
}

#[test]
fn s4_choose_and_pool_keep_distinct_conflicting_quote_classes() {
    let source=r#"
        @pool @choose(quote_delimiter=0) string first(string value){return "shared\"text"+value;}
        @pool @choose(quote_delimiter=1) string second(string value){return "shared\"text"+value;}
        export {first,second};
    "#;
    let built=compile_source(source,&config(""),ServiceOptions{objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        let code=built.javascript(codec).unwrap().javascript();
        assert!(code.contains(r#""shared\"text""#) && code.contains("'shared\"text'"),"{code}");
        assert_eq!(execute_javascript(code,"","console.log(library.first('a'),library.second('b'));"),"shared\"texta shared\"textb\n");
    }
    check_scores(&built);
}

#[test]
fn s4_choose_prevents_sharing_helpers_with_incompatible_pins() {
    let source=r#"
        @choose(quote_delimiter=0) string first(string x){return "same\"text"+x;}
        @choose(quote_delimiter=1) string second(string x){return "same\"text"+x;}
        export string both(string x){return first(x)+second(x)+first(x+x)+second(x+x);}
    "#;
    let built=compile_source(source,&config("[policy.tactics]\ninlining='off'\nhelper-sharing='on'"),ServiceOptions {objectives:Some(Objectives::All),..ServiceOptions::default()}).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        let code=built.javascript(codec).unwrap().javascript();
        assert!(code.contains(r#""same\"text""#) && code.contains("'same\"text'"),"{code}");
        assert_eq!(execute_javascript(code,"","console.log(library.both('z'));"),"same\"textzsame\"textzsame\"textzzsame\"textzz\n");
    }
    check_scores(&built);
}

#[path = "build_container_tests.rs"]
mod delivery_formats;

#[test]
fn n1_native_check_and_compile_share_source_qualified_representation_refusals() {
    let options = ServiceOptions { target: ServiceTarget::Native, preserve_root_exports: false, ..ServiceOptions::default() };
    let settings: ProjectConfig = toml::from_str("effort.level=0\n[target.native]\ncompiler='/no/compiler/is/needed/to/check'").unwrap();
    check_source_for_target("print(42);", &settings, options).unwrap();
    for source in [
        "struct Box<T> { T value; }\nBox<int> b=Box<int>{1};print(b.value);",
        "struct Point { int x; }\nPoint? point=null;print(point==null);",
        "int x=1;\ntry{print(x);}finally{print(2);}",
    ] {
        check_source_for_target(source, &settings, options).unwrap();
        compile_source(source, &settings, options).unwrap();
    }
    for source in [
        "export int answer(){return 42;}",
        "extern class Error{string message;init(string message);}class Problem extends Error{init(string message){super(message);}}Problem p=new Problem(\"oops\");print(p.message);",
    ] {
        let checked = check_source_for_target(source, &settings, options).unwrap_err();
        let compiled = compile_source(source, &settings, options).unwrap_err();
        assert!(matches!(checked.phase, "check" | "native capability"), "{checked:?}");
        assert_eq!(checked.message, compiled.message, "{source}");
        let diagnostic = checked.diagnostic.expect("native refusal retains original source");
        assert_eq!(diagnostic.source, source);
        assert!(diagnostic.span.end > diagnostic.span.start, "{diagnostic:?}");
        assert!(diagnostic.span.end <= source.len());
    }
    let scratch = Scratch::new();
    let entry = scratch.0.join("main.lil"); let dependency = scratch.0.join("value.lil");
    std::fs::write(&entry, "import {run} from \"./value.lil\";run();").unwrap();
    let source = r#"export void run(){JsValue value=JS.object("x",1);print(value);}"#;
    std::fs::write(&dependency, source).unwrap();
    let error = check_entries_for_target(&[EntrySource::of(&entry)], &settings, options).unwrap_err();
    let diagnostic = error.diagnostic.unwrap();
    assert_eq!(diagnostic.path, dependency.canonicalize().unwrap(), "{diagnostic:?}");
    assert_eq!(diagnostic.source, source);
    assert!(diagnostic.span.end > diagnostic.span.start);
}

#[test]
fn n1_native_check_retains_no_output_and_releases_its_borrowed_source_index() {
    let options = ServiceOptions { target: ServiceTarget::Native, preserve_root_exports: false, ..ServiceOptions::default() };
    let ((), finished) = with_checked_source("int twice(int x){return x*2;}print(twice(21));", &config(""), options, |session| {
        let before = session.compilation().ledger().retained_bytes();
        session.check_native(session.source()).unwrap();
        assert_eq!(session.compilation().ledger().retained_bytes(), before);
    }).unwrap();
    assert!(finished.report["native_sha256"].is_null());
    assert_eq!(finished.report["resources"]["optional_work"], 0);
    assert_eq!(finished.ledger.retained_bytes(), 0);
    assert_eq!(finished.report["resources"]["codec_work"], 0);
    assert!(finished.report["phases_ns"]["rules"]["folded_calls"].as_u64().unwrap() > 0);
}

#[test]
fn n1_native_global_guards_consume_shared_initialization_and_keep_reentry_hazards() {
    let options = ServiceOptions { target: ServiceTarget::Native, preserve_root_exports: false, ..ServiceOptions::default() };
    // A mutable global cannot be folded away; each call follows its initializer.
    let source = "int counter=1;int next(){counter+=1;return counter;}print(next());print(next());";
    for enabled in [false, true] {
        let settings: ProjectConfig = toml::from_str(&format!("effort.level=0\n[policy.tactics]\ndead-code-elimination='{}'\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'", if enabled {"on"} else {"off"})).unwrap();
        let built = compile_source(source, &settings, options).unwrap();
        assert_eq!(execute_native(built.native_c().unwrap()), "2\n3\n");
        assert_eq!(built.report()["native_delivery"]["initialization_guards_removed"].as_u64().unwrap() > 0, enabled);
        assert_eq!(built.native_c().unwrap().contains("ls_native_unbound"), !enabled);
    }
    // A provider may call this callback immediately. Its foreign body is not
    // assumed to defer reentry until the later module initializer completes.
    let source = "extern int install(func()->int f);int later=install(read);int read(){return later;}print(read());";
    let settings: ProjectConfig = toml::from_str("effort.level=0\n[policy.tactics]\ndead-code-elimination='on'\ninlining='off'\n[host.native]\ninstall='host_install'").unwrap();
    let built = compile_source(source, &settings, options).unwrap();
    assert!(built.native_c().unwrap().contains("ls_native_unbound"));
    assert_eq!(built.report()["native_delivery"]["initialization_guards_removed"], 0);
}
