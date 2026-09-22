use super::*;
use crate::structured_js::PrintPolicy;
use std::process::Command;

fn checked<T>(source: &str, inspect: impl FnOnce(&Program<'_>, &str) -> T) -> T {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let expected = crate::interpreter::interpret_program(&syntax, &semantics).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    inspect(&program, &expected)
}

fn execute(program: &Program<'_>, setup: &str, mangle_bindings: bool) -> String {
    let module = super::javascript::lower(program).unwrap();
    module.verify().unwrap();
    let javascript = module.render(PrintPolicy { mangle_bindings }).unwrap();
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!("{setup}\n{javascript}"),
        ])
        .output()
        .expect("Node is required for semantic JavaScript tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn compare(source: &str) {
    checked(source, |program, expected| {
        for mangle in [false, true] {
            assert_eq!(execute(program, "", mangle), expected, "{source}");
        }
    });
}

fn host_case(source: &str, setup: &str, expected: &str) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    for mangle in [false, true] {
        assert_eq!(execute(&program, setup, mangle), expected);
    }
}

#[test]
fn direct_units_match_interpreter_for_calls_recursion_and_integer_obligations() {
    compare(
        r#"
        int odd(int n){if(n==0){return 0;}return even(n-1);}
        int even(int n){if(n==0){return 1;}return odd(n-1);}
        int add(int a,int b){return a+b;}
        print(add(2147483647,1));print(even(8));
        int minimum=-2147483647-1;print(2147483647*2147483647);print(minimum/-1);
        print(7/0);print(7%0);print(-1>>>0);
    "#,
    );
}

#[test]
fn mutable_captures_have_per_activation_and_per_iteration_cells() {
    compare(
        r#"
        func()->int factory(int start){
            int count=start;return ()=>{count+=1;return count;};
        }
        auto first=factory(2);auto second=factory(9);
        print(first());print(second());print(first());
        void bodyCaptures(){auto a=()=>0;auto b=()=>0;int i=0;
            while(i<2){int saved=i;if(i==0){a=()=>saved;}else{b=()=>saved;}i+=1;}
            print(a());print(b());
        }bodyCaptures();
    "#,
    );
}

#[test]
fn loop_tests_updates_and_finalizers_preserve_abrupt_completion() {
    // The independent interpreter explicitly rejects exceptions. These are
    // executed completion traces, including finalizers replacing returns.
    host_case(
        r#"
        int work(int n){int sum=0;while(n>0){
            try{n-=1;if(n==2){continue;}sum+=n;}finally{sum+=1;}
        }return sum;}
        print(work(4));
        int count=0;
        for(int i=0;i<4;i+=1){try{if(i==1){continue;}if(i==3){break;}count+=i;}finally{count+=10;}}
        print(count);
        int replacement(){try{return 4;}finally{return 8;}}
        print(replacement());
        try{throw 9;}catch(auto caught){print(caught);}finally{print(10);}
    "#,
        "",
        "8\n42\n8\n9\n10\n",
    );
}

#[test]
fn short_circuit_select_and_nullish_regions_execute_only_when_demanded() {
    compare(
        r#"
        int calls=0;bool yes(){calls+=1;return true;}int once(){calls+=1;return 9;}
        print(false&&yes());print(true||yes());print(true&&yes());
        int? present=0;int? missing=null;
        print(present??once());print(missing??once());print(calls);
        print(if(false){once()}else{5});print(calls);
    "#,
    );
}

#[test]
fn arrays_and_records_preserve_aliasing_absence_and_proto_data_keys() {
    compare(
        r#"
        Record<int> values=record{x:1,__proto__:4,"quoted-key":6};
        Record<int> alias=values;alias["x"]=7;
        print(values.x??0);print(values["__proto__"]??0);print(values.toString==null);
        print(values["quoted-key"]??0);
        int[] numbers=[3,5,7];int[] shared=numbers;shared[1]=11;
        print(numbers[1]);print(numbers[0]+numbers[2]);
    "#,
    );
    host_case(
        r#"int[] numbers=[7];string[] strings=["ok"];print(numbers[4]);print(strings[4]);print("abc"[8]);print("\ud800X"[0].charCodeAt(0));"#,
        "",
        "0\n\n\n55296\n",
    );
}

#[test]
fn place_updates_snapshot_receiver_key_and_old_value_before_rhs() {
    compare(
        r#"
        int[] values=[10];int rhs(){values=[90];return 2;}
        print(values[0]+=rhs());print(values[0]);
    "#,
    );
    host_case(
        r#"
        extern int[] receiver();extern int key();extern int rhs();extern string trace();
        print(receiver()[key()]+=rhs());print(trace());
    "#,
        r#"
        const events=[];const object=new Proxy([10],{
            get(target,key){events.push('get:'+key);return target[key]},
            set(target,key,value){events.push('set:'+key+':'+value);target[key]=value;return true}
        });
        globalThis.receiver=()=>{events.push('receiver');return object};
        globalThis.key=()=>{events.push('key');return 0};
        globalThis.rhs=()=>{events.push('rhs');return 2};
        globalThis.trace=()=>events.join(',');
        "#,
        "12\nreceiver,key,get:0,rhs,set:0:12\n",
    );
}

#[test]
fn prepared_reference_call_retains_receiver_and_lookup_before_nested_arguments() {
    host_case(
        r#"
        extern (func(int,int)->int)[] object();extern int key();
        extern int argument(int value);extern string trace();
        print(object()[key()](argument(2),argument(3)));print(trace());
    "#,
        r#"
        const events=[];const receiver={get 0(){events.push('lookup');const callable=function(a,b){
            events.push(this===receiver?'receiver-ok':'receiver-bad');return a+b
        };Object.defineProperty(callable,'call',{get(){throw Error('unexpected call adapter')}});return callable}};
        globalThis.object=()=>{events.push('object');return receiver};
        globalThis.key=()=>{events.push('key');return 0};
        globalThis.argument=value=>{events.push('arg:'+value);return value};
        globalThis.trace=()=>events.join(',');
        "#,
        "5\nobject,key,lookup,arg:2,arg:3,receiver-ok\n",
    );
}

#[test]
fn prepared_builtin_and_primitive_lookup_happens_before_argument_effects() {
    host_case(
        r#"
        extern int argument();print(argument());
        print("abc".charCodeAt(argument()));
    "#,
        r#"
        const events=[];const original=console.log.bind(console);
        let calls=0;
        Object.defineProperty(console,'log',{configurable:true,get(){events.push('print-lookup');return value=>{
            events.push('print:'+value);original(events.join(','));events.length=0
        }}});
        Object.defineProperty(String.prototype,'charCodeAt',{configurable:true,get(){
            events.push('method-lookup');return function(index){events.push('method-call');return 100+index}
        }});
        globalThis.argument=()=>{events.push('argument');return calls++};
        "#,
        "print-lookup,argument,print:0\nprint-lookup,method-lookup,argument,method-call,print:101\n",
    );
}

#[test]
fn unsupported_implementation_fails_before_printing() {
    let arena = bumpalo::Bump::new();
    // An exported array of value structs that the function mutates has no
    // D2 adapter, so this ABI shape is refused rather than printed
    // approximately.
    let syntax = crate::parse_source(
        &arena,
        "struct P{int x;}export void add(P[] items){items.push(P{1});}",
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    assert_eq!(
        super::javascript::lower(&program).unwrap_err().feature,
        "public value-struct ABI adaptation"
    );
}

#[test]
fn public_packaging_preserves_live_bindings_and_callable_observations() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        r#"
        export int counter=1;
        export int increment(int amount){counter+=amount;return counter;}
        export func()->int factory(){return ()=>counter;}
    "#,
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let module = program.to_javascript().unwrap();
    assert_eq!(module.exports.len(), 3);
    for mangle_bindings in [false, true] {
        let javascript = module.render(PrintPolicy { mangle_bindings }).unwrap();
        let script = format!(
            r#"
            const code={};
            const library=await import('data:text/javascript,'+encodeURIComponent(code));
            const one=library.factory(),two=library.factory();
            console.log(Object.keys(library).sort().join(','));
            console.log(library.increment.name,library.increment.length);
            console.log(library.counter,library.increment(2),library.counter);
            console.log(one===two,JSON.stringify(one.name),one());
            library.increment(4);console.log(one(),two(),library.counter);
        "#,
            serde_json::to_string(&javascript).unwrap()
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
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            "counter,factory,increment\nincrement 1\n1 3 3\nfalse \"\" 3\n7 7 7\n"
        );
    }
}

#[test]
fn direct_public_artifacts_are_packaged_and_named_before_codec_selection() {
    use crate::structured_js::selection::{Budget, Objective, Objectives};
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "export int publicCalculation(int input){return input*2+3;}",
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let target = program.to_javascript().unwrap();
    let output = target.prepare_output().unwrap();
    let budget = Budget {
        plans: 8,
        candidate_bytes: 100_000,
    };
    let raw_only = output
        .select(budget, Objectives::One(Objective::Raw), |_, _| {
            panic!("a raw-only build must not invoke a codec")
        })
        .unwrap();
    assert_eq!(raw_only.measurement_calls, 0);
    assert!(raw_only.winner(Objective::Gzip).is_none());
    assert!(raw_only.winner(Objective::Brotli).is_none());
    let all = output
        .select(budget, Objectives::All, |bytes, objective| {
            assert!(std::str::from_utf8(bytes).unwrap().contains("export{"));
            assert!(std::str::from_utf8(bytes)
                .unwrap()
                .contains("publicCalculation"));
            crate::compression::measure(bytes, objective)
        })
        .unwrap();
    for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let winner = all.winner(objective).unwrap();
        assert_eq!(
            winner.sizes.get(objective).unwrap(),
            crate::compression::measure(winner.javascript.as_bytes(), objective).unwrap()
        );
        assert!(all
            .candidates
            .iter()
            .all(|candidate| winner.sizes.get(objective) <= candidate.sizes.get(objective)));
        let script = format!("const library=await import('data:text/javascript,'+encodeURIComponent({}));console.log(library.publicCalculation(4),library.publicCalculation.name,library.publicCalculation.length);", serde_json::to_string(&winner.javascript).unwrap());
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "11 publicCalculation 1\n"
        );
    }
}

#[test]
fn prepared_public_output_preserves_mutable_intrinsics_and_integer_results() {
    use crate::structured_js::selection::{Budget, Objective, Objectives};

    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        r#"
        extern int argument();
        export int counter=0;
        export int bump(){counter+=1;return counter;}
        export int read(){return "abc".charCodeAt(0);}
        export int observed(){return "abc".charCodeAt(argument());}
    "#,
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let target = program.to_javascript().unwrap();

    // This is the direct core -> typed target -> Output::from_module boundary.
    // It has no AnnotatedTree or source-derived extraction choices: a known
    // string/index cannot prove what a mutable host method will return, or
    // authorize removing the typed int normalization around that result.
    let output = target.prepare_output().unwrap();
    let selection = output
        .select(
            Budget {
                plans: 8,
                candidate_bytes: 100_000,
            },
            Objectives::All,
            crate::compression::measure,
        )
        .unwrap();
    for objective in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let javascript = &selection.winner(objective).unwrap().javascript;
        let script = format!(
            r#"
            const library=await import('data:text/javascript,'+encodeURIComponent({}));
            console.log(library.read.name,library.read.length,library.observed.name,library.observed.length);
            String.prototype.charCodeAt=function(){{return 123}};
            console.log(library.read());
            String.prototype.charCodeAt=function(){{return NaN}};
            console.log(library.read());

            const events=[];
            Object.defineProperty(String.prototype,'charCodeAt',{{configurable:true,get(){{
                events.push('lookup');events.push('lookup-reentry:'+library.bump());
                return function(index){{
                    events.push('call:'+String(this)+':'+index);
                    events.push('call-reentry:'+library.bump());return NaN
                }}
            }}}});
            globalThis.argument=()=>{{
                events.push('argument:'+library.counter);
                Object.defineProperty(String.prototype,'charCodeAt',{{
                    configurable:true,writable:true,value(){{return 777}}
                }});
                return 2
            }};
            console.log(library.observed(),library.counter,events.join(','));
            console.log(library.read());

            events.length=0;
            Object.defineProperty(String.prototype,'charCodeAt',{{configurable:true,get(){{
                events.push('throwing-lookup');throw Error('lookup stopped')
            }}}});
            try{{library.observed()}}catch(error){{console.log(error.message,events.join(','))}}
        "#,
            serde_json::to_string(javascript).unwrap()
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for semantic JavaScript tests");
        assert!(
            result.status.success(),
            "{objective:?}: {}\n{javascript}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            concat!(
                "read 0 observed 0\n123\n0\n",
                "0 2 lookup,lookup-reentry:1,argument:1,call:abc:2,call-reentry:2\n",
                "777\nlookup stopped throwing-lookup\n",
            ),
            "{objective:?}\n{javascript}"
        );
    }
}

#[test]
fn module_body_declarations_keep_fresh_cells_for_escaping_closures() {
    // decisions/006-for-control.md and the established target test
    // for_initializer_cells_are_shared_and_body_cells_are_fresh specify fresh
    // body declarations. The reference interpreter currently stores module
    // locals as global values, so it is not an oracle for this exact case.
    host_case(
        r#"
        auto a=()=>0;auto b=()=>0;int i=0;
        while(i<2){int saved=i;if(i==0){a=()=>saved;}else{b=()=>saved;}i+=1;}
        print(a());print(b());
    "#,
        "",
        "0\n1\n",
    );
}

#[test]
fn exact_callable_names_survive_temporary_formation_and_binding_mangling() {
    host_case(
        r#"
        extern string nameOf(func()->int value);
        int declared(){return 3;}
        auto assigned=()=>1;print(nameOf(assigned));
        assigned=()=>2;print(nameOf(assigned));print(nameOf(declared));
        func()->int fallback=()=>0;
        Record<func()->int> callbacks=record{handler:()=>7};
        print(nameOf(callbacks.handler??fallback));
        callbacks.handler=()=>8;print(nameOf(callbacks.handler??fallback));
    "#,
        "globalThis.nameOf=value=>value.name;",
        "assigned\nassigned\ndeclared\nhandler\n\n",
    );
}

#[test]
fn prepared_lookup_exception_prevents_all_argument_evaluation() {
    host_case(
        r#"
        extern (func(int)->int)[] object();extern int argument();extern string trace();
        try{object()[0](argument());}catch{print(trace());}
    "#,
        r#"
        const events=[];
        globalThis.object=()=>{events.push('receiver');return {get 0(){
            events.push('lookup');throw Error('stop')
        }}};
        globalThis.argument=()=>{events.push('argument');return 1};
        globalThis.trace=()=>events.join(',');
    "#,
        "receiver,lookup\n",
    );
}
