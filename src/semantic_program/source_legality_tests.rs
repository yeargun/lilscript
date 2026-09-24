//! Language legality cases compiled by production formation and observed by
//! running the output under each naming style the search renders.
//!
//! These cases came from the annotated-tree experiment's tests, deleted in
//! M1.6. Each expected stdout is the test's own assertion or the reference
//! interpreter's output, never another compiler's. A case that observes a
//! function's `name` compiles with `keep_function_names`, the contract that
//! promises it. Ignored cases are production gaps the harvest found; each
//! names the refusal or the wrong output.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use std::process::Command;

const CONFIG: &str = "[javascript]\nstrip_console=false\n";
const KEEP_NAMES: &str = "[javascript]\nstrip_console=false\nkeep_function_names=true\n";

/// One production formation of a checked program, rendered with `style`.
/// `library` keeps root declarations exported (the reusable-library world);
/// otherwise the program is a closed application.
fn formed(
    syntax: &crate::ast::Program<'_, '_>,
    checked: &crate::semantic::SemanticModel<'_, '_>,
    config: &str,
    style: Style,
    library: bool,
) -> String {
    let program = from_checked_source(syntax, checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}"));
    program.verify().unwrap();
    let config: crate::config::ProjectConfig = toml::from_str(config).unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: library,
        })
        .unwrap();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 400_000_000,
            optional_work: 0,
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
    let result = compilation
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(style))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result)
        .unwrap();
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

/// Runs `javascript` as an ES module after `setup`, returning stdout.
fn run(javascript: &str, setup: &str) -> String {
    let script = format!(
        "{setup}\nawait import('data:text/javascript,'+encodeURIComponent({}));",
        serde_json::to_string(javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for legality tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Every closed-application naming style, and the library world once.
fn compare_program_output(
    syntax: &crate::ast::Program<'_, '_>,
    checked: &crate::semantic::SemanticModel<'_, '_>,
    config: &str,
    setup: &str,
    expected: &str,
) {
    for (style, library) in [
        (Style::Global, false),
        (Style::Scoped, false),
        (Style::Source, false),
        (Style::Global, true),
    ] {
        let javascript = formed(syntax, checked, config, style, library);
        assert_eq!(
            run(&javascript, setup),
            expected,
            "{style:?} library={library}\n{javascript}"
        );
    }
}

fn compare_configured_output(config: &str, source: &str, setup: &str, expected: &str) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    compare_program_output(&syntax, &checked, config, setup, expected);
}

fn compare_source_output_with_setup(source: &str, setup: &str, expected: &str) {
    compare_configured_output(CONFIG, source, setup, expected);
}

/// For a case that observes a function's `name`.
fn compare_named_output(source: &str, setup: &str, expected: &str) {
    compare_configured_output(KEEP_NAMES, source, setup, expected);
}

fn compare_source_output(source: &str, expected: &str) {
    compare_source_output_with_setup(source, "", expected);
}

fn compare_source_with_interpreter(source: &str) {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let expected = crate::interpret_program(&program, &semantics).unwrap();
    compare_source_output(source, &expected);
}

#[test]
fn omitted_defaults_preserve_literals_global_bindings_and_fresh_callables() {
    compare_source_with_interpreter(
        r#"
        int withDefaults(int a,int b=10,int c=100){return a+b+c;}
        auto alias=withDefaults;
        print(withDefaults(3));print(alias(3,1));print(alias(3,1,2));
        int value=4;
        int readDefault(int first,int second=value){return first*10+second;}
        print(readDefault(value++));
        auto shadow=()=>{int value=70;return readDefault(2);};print(shadow());
        int increment(int[] values=[0]){values[0]+=1;return values[0];}
        print(increment());print(increment());
    "#,
    );
    compare_source_with_interpreter(
        r#"
        int offset=2;
        func(int)->int factory(func(int)->int value=(int n)=>{int saved=n+offset;return saved;}){return value;}
        auto first=factory();auto second=factory();
        print(first==second);print(first(2));print(second(4));
        T identity<T>(T value,func(T)->T transform=(T item)=>item){return transform(value);}
        print(identity(3));print(identity("three"));
    "#,
    );
}

#[test]
fn parameter_defaults_snapshot_arguments_inside_the_ordered_call() {
    compare_source_output(
        r#"
        int current=1;
        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++));
        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++,current++));
        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++,current++,current++));
        print(current);
    "#,
        "111\n233\n456\n7\n",
    );
    compare_source_output(
        r#"
        int trace=0;
        int index(){trace=trace*10+1;return 0;}
        int first(){trace=trace*10+2;return 7;}
        int second(){trace=trace*10+3;return 8;}
        print((if(index()==0){
            (int a,int b,int c=a)=>{print(trace);return c;}
        }else{
            (int a,int b,int c=a)=>{print(trace);return c;}
        })(first(),second()));
    "#,
        "123\n7\n",
    );
}

#[test]
#[ignore = "wrong output: production applies a default to a supplied JS.undefined() (prints 16/8 for 109/101)"]
fn defaults_distinguish_omission_supplied_undefined_and_function_arity() {
    compare_source_output_with_setup(
        r#"
        extern int tag(JsValue value);
        extern int arity(func(JsValue,int)->int callable);
        int choose(JsValue first=7,int second=9){return tag(first)+second;}
        print(choose());print(choose(JS.undefined()));print(choose(JS.undefined(),1));
        print(arity(choose));
    "#,
        "globalThis.tag=value=>value===undefined?100:Number(value);globalThis.arity=callable=>callable.length;",
        "16\n109\n101\n2\n",
    );
}

#[test]
fn source_classes_preserve_probe_inheritance_aliases_and_lexical_receivers() {
    let declarations = include_str!("../../finer/tools/fixtures/probe-classes.lil");
    let source = format!(
        "{declarations}\nprint(classes(-7));print(classes(-1));print(classes(0));\
         print(classes(3));print(classes(29));print(classes(1000000000));\
         print(classes(-2147483647-1));print(classes(2147483647));"
    );
    compare_source_output(&source, "-101\n3\n17\n59\n483\n-1179869167\n17\n3\n");
}

#[test]
fn class_calls_and_updates_capture_the_receiver_before_argument_effects() {
    compare_source_output(
        r#"
        class Cell {
            int value;
            init(int value) { this.value=value; }
            int add(int amount) { this.value+=amount;return this.value; }
            func()->int read() { return ()=>this.value; }
        }
        Cell left=new Cell(1);Cell right=new Cell(20);Cell chosen=left;
        Cell receiver(){print(10);return chosen;}
        int argument(){chosen=right;print(30);return 3;}
        print(receiver().add(argument()));print(left.value);print(right.value);
        func()->int read=left.read();Cell original=left;
        int replace(){left=right;original.value=40;return 5;}
        left.value+=replace();print(original.value);print(left.value);print(read());
        print(original.value++);print(++original.value);
        "#,
        "10\n30\n4\n4\n20\n9\n20\n9\n9\n11\n",
    );
}

#[test]
fn class_initializers_share_super_defaults_and_keep_fresh_fields_after_arguments() {
    compare_source_output_with_setup(
        r#"
        extern int argument();
        class Base {
            int value;
            int[] items;
            Map<string,int> table;
            init(int value=2) { this.value=value; }
            int add(int by=3) { return this.value+by; }
        }
        class Derived extends Base {
            init(int value=4) { super(value);if(value==4){return;}this.value+=1; }
        }
        Derived first=new Derived();Derived second=new Derived(argument());
        first.items.push(9);print(first.items.length);print(second.items.length);
        first.table.set("x",7);print(first.table.get("x"));print(second.table.get("x"));
        print(first.add());print(second.add());print(new Base().add());
        "#,
        "const NativeMap=Map;globalThis.Map=class extends NativeMap{constructor(){super();console.log('map')}};globalThis.argument=()=>{console.log('argument');return 8};",
        "map\nargument\nmap\n1\n0\n7\nnull\n7\n12\nmap\n5\n",
    );
}

#[test]
fn constructor_results_keep_allocation_identity_when_this_is_rebound() {
    compare_source_output(
        r#"
        class Cell {
            int value;
            init(int value) {
                this.value=value;
                if(value==1){this=new Cell(2);return;}
                if(value==4){try{return;}finally{this=new Cell(5);}}
                if(value==6){auto change=()=>{this=new Cell(7);};change();}
            }
            int replace(){this=new Cell(9);return this.value;}
        }
        Cell first=new Cell(1);
        print(first.value);print(new Cell(4).value);print(new Cell(6).value);
        print(first.replace());print(first.value);
        "#,
        "1\n4\n6\n9\n1\n",
    );
}

#[test]
fn class_callable_fields_remain_replaceable_and_generic_classes_erase_types() {
    compare_source_output(
        r#"
        class Holder<T> {
            T value;
            func()->int callback;
            init(T value,func()->int callback){this.value=value;this.callback=callback;}
            T get(){return this.value;}
        }
        Holder<int> first=new Holder<int>(7,()=>1);
        Holder<string> second=new Holder<string>("ok",()=>3);
        first.callback=()=>2;auto saved=first.callback;
        print(first.callback());print(saved());print(second.callback());
        print(first.get());print(second.get());
        "#,
        "2\n2\n3\n7\nok\n",
    );
}

#[test]
#[ignore = "production formation refuses a generic method (Unsupported: generic method conversion)"]
fn generic_methods_erase_types() {
    compare_source_output(
        r#"
        class Holder<T> {
            T value;
            init(T value){this.value=value;}
            U identity<U>(U value){return value;}
        }
        Holder<int> first=new Holder<int>(7);
        print(first.identity("generic"));print(first.identity(3));
        "#,
        "generic\n3\n",
    );
}

#[test]
fn shared_value_dependencies_preserve_initialization_effects_and_cells() {
    compare_source_with_interpreter(
        r#"
        int effects=0;
        int next(){effects=effects+1;return effects;}
        int f(int x) {
            int a=x+1; int b=a; int unused=next();
            42; if(false){print(999);}
            return b;
        }
        int first=next(); first; print(f(first)); print(effects);
        int cell=1; auto set=()=>{cell=3;};
        int previous=cell; set(); print(previous); print(cell);
    "#,
    );
}

#[test]
fn materialization_keeps_long_statement_chains_within_the_target_depth_budget() {
    for nesting in [0, 12] {
        let mut source = String::from("int f(int input){");
        for _ in 0..nesting {
            source.push_str("if(input>0){");
        }
        source.push_str("int v0=input;");
        for index in 1..=1200 {
            source.push_str(&format!("int v{index}=v{}+1;", index - 1));
        }
        source.push_str("return v1200;");
        for _ in 0..nesting {
            source.push('}');
        }
        if nesting > 0 {
            source.push_str("return 0;");
        }
        source.push_str("}print(f(7));print(f(2147483647));print(f(-2147483647-1));");
        compare_source_output(
            &source,
            if nesting == 0 {
                "1207\n-2147482449\n-2147482448\n"
            } else {
                "1207\n-2147482449\n0\n"
            },
        );
    }
}

#[test]
fn binding_updates_preserve_signed_arithmetic_and_rhs_observation_order() {
    compare_source_with_interpreter(
        r#"
        int value=2147483647;print(value++);print(value);print(++value);
        int minimum=-2147483647-1;print(minimum--);print(minimum);
        int cell=3;print(cell+=(cell=9));print(cell);
        cell-=2;cell*=3;cell/=4;cell%=5;
        cell|=8;cell^=3;cell&=15;cell<<=2;cell>>=1;cell>>>=0;
        print(cell);cell/=0;print(cell);
        int negative=-1;negative>>>=32;print(negative);
        float fraction=1.5;print(fraction++);fraction*=2.0;print(--fraction);
        string text="first";text+="-last";print(text);
    "#,
    );
}

#[test]
#[ignore = "production formation refuses ??= on a place (Unsupported: nullish place assignment)"]
fn nullish_binding_assignment_preserves_lazy_callable_named_evaluation() {
    compare_named_output(
        r#"
        extern string nameOf(func()->int value);
        (func()->int)? callback=null;
        func()->int first=callback??=()=>7;
        func()->int second=callback??=()=>8;
        print(nameOf(first));print(nameOf(second));print(first());print(second());
    "#,
        "globalThis.nameOf=value=>value.name;",
        "callback\ncallback\n7\n7\n",
    );
}

#[test]
fn source_records_and_arrays_preserve_aliases_keys_and_absent_values() {
    compare_source_with_interpreter(
        r#"
        Record<int> values=record{x:1,y:2,__proto__:4,"quoted-key":6};
        Record<int> alias=values;
        alias["x"]=7;
        print(values.x==null);print(values.x??0);
        print(values["__proto__"]??0);print(values.toString==null);
        print(values["quoted-key"]??0);
        values.missing=9;print(alias["missing"]??0);
        int[] array=[3,5,7];int[] shared=array;
        shared[1]=11;print(array[1]);print(array[0]+array[2]);
    "#,
    );
    // The reference interpreter rejects these out-of-range element reads.
    // The JavaScript contract uses signed normalization and the empty string.
    compare_source_output(
        r#"
        int[] numbers=[7];string[] strings=["ok"];
        print(numbers[4]);print(strings[4]);print("abc"[8]);
        print("\ud800X"[0].charCodeAt(0));
    "#,
        "0\n\n\n55296\n",
    );
}

#[test]
fn reference_construction_preserves_order_without_loading_a_write_target() {
    compare_source_output_with_setup(
        r#"
        extern Record<int> object();extern string key();extern int replacement();
        extern string trace();
        print(object()[key()]??41);
        print(object()[key()]=replacement());
        print(trace());
    "#,
        r#"
        const events=[];
        const value=new Proxy(Object.create(null),{
            get(target,key){events.push('get');return undefined},
            set(target,key,value){events.push('set:'+value);return true}
        });
        globalThis.object=()=>{events.push('object');return value};
        globalThis.key=()=>{events.push('key');return 'slot'};
        globalThis.replacement=()=>{events.push('value');return 7};
        globalThis.trace=()=>events.join(',');
    "#,
        "41\n7\nobject,key,get,object,key,value,set:7\n",
    );
}

#[test]
fn nullish_evaluation_preserves_falsy_values_and_conditional_writes() {
    compare_source_with_interpreter(
        r#"
        int effects=0;int once(){effects=effects+1;return 9;}
        int? missing=null;int? present=0;
        print(missing??once());print(present??once());print(effects);
        bool? absent=null;bool? no=false;
        print((absent??false)||true);print((no??true)&&false);
        print(absent??(false||true));print(no??(true&&false));
        int cell=3;present??(cell=7);print(cell);
        missing??(cell=11);print(cell);
    "#,
    );
}

#[test]
fn record_function_values_keep_named_creation_separate_from_assignment() {
    compare_source_output_with_setup(
        r#"
        extern string nameOf(func()->int value);
        func()->int fallback=()=>0;
        Record<func()->int> callbacks=record{handler:()=>7};
        print(nameOf(callbacks.handler??fallback));
        callbacks.handler=()=>8;
        print(nameOf(callbacks.handler??fallback));
    "#,
        "globalThis.nameOf=value=>value.name;",
        "handler\n\n",
    );
}

#[test]
fn source_functions_branches_and_loops_match_the_independent_interpreter() {
    compare_source_with_interpreter(
        r#"
        float sum(float limit) {
            float i=0.0; float total=0.0;
            while(i<limit) {
                i=i+1.0;
                if(i==2.0) { continue; }
                if(i>5.0) { break; }
                total=total+i;
            }
            return total;
        }
        print(sum(7.0));
    "#,
    );
}

#[test]
fn source_short_circuit_and_mutable_closures_match_the_interpreter() {
    compare_source_with_interpreter(
        r#"
        float count=0.0;
        bool change(){count=count+1.0;return true;}
        print(false && change()); print(true || change()); print(count);
        auto add=(float amount)=>{count=count+amount;return count;};
        print(add(3.0)); print(add(4.0));
    "#,
    );
}

#[test]
fn erased_type_parameters_preserve_callback_bindings_and_invocations() {
    compare_source_with_interpreter(
        r#"
        bool sameParity(int previous,int next){return previous%2==next%2;}
        bool compareWith(int value,func(int,int)->bool compare){return compare(value,value);}
        print(compareWith(7,sameParity));
    "#,
    );
    // The reference interpreter currently rejects generic function signatures
    // before evaluating their body. These results are independently declared;
    // non-generic callback execution remains covered by interpreter cases.
    compare_source_output(
        r#"
        bool sameParity(int previous,int next){return previous%2==next%2;}
        bool compareWith<T>(T value,func(T,T)->bool compare){return compare(value,value);}
        T identity<T>(T value){return value;}
        print(compareWith(7,sameParity));
        print(identity(42));print(identity("typed"));print(identity(false));
    "#,
        "true\n42\ntyped\nfalse\n",
    );
}

#[test]
fn source_integer_operations_follow_the_language_contract() {
    compare_source_with_interpreter(
        r#"
        int add(int x){return x+1;}
        int minimum=-2147483647-1;
        print(add(2147483647)); print(minimum-1);
        print(-1 >>> 0); print(-1 >>> 32); print(-1 >>> 1);
        print(minimum); print(-minimum);
        print(1073741825*1073741825); print(9/0); print(9%0);
        print(-9/2); print(-9%2); print(minimum/-1);
        print(1<<33); print(-8>>34); print(3&1); print(1^3); print(1|2);
        print(-0);
    "#,
    );
}

#[test]
fn source_templates_keep_probe_bindings_and_string_operations_visible() {
    compare_source_output(
        r#"
        int strings(int seed) {
          string a = "probe";
          string b = a + "-lil";
          string t = `seed=${seed} len=${b.length}`;
          int code = b.charCodeAt(1);
          int found = t.indexOf("len");
          string sliced = b.slice(0, 5);
          return b.length + t.length + code + found + sliced.length;
        }
        int templateBinding(int seed) {
          int live = seed * 2;
          string rendered = `value=${live}`;
          return rendered.length;
        }
        print(strings(7));print(strings(-3));
        print(templateBinding(7));print(templateBinding(-3));
        int live=9;
        print(`${live}${true}${2.5}`);
        print(`prefix ${`inner ${live}`} suffix`);
        print(`${"}" + live /* } ` ignored inside a comment */}`);
        print(`${(() => {return "quoted }";})()}`);
        print(``);
        "#,
        "147\n149\n8\n8\n9true2.5\nprefix inner 9 suffix\n}9\nquoted }\n\n",
    );
}

#[test]
fn source_templates_cook_escapes_without_losing_surrogates_or_creating_substitutions() {
    let source = r#"
        extern void units(string value);
        int amount=7;
        units(`\ud800${amount}\udfff`);
        units(`\` \${number} \\ \u{1f600}`);
        units(`line one
line two${amount}`);
        units(`\u0024{notAnIdentifier}${amount}`);
        units(`a\
b${amount}`);
        "#
    .replace("line one\nline two", "line one\r\nline two");
    compare_source_output_with_setup(
        &source,
        "globalThis.units=value=>console.log(Array.from({length:value.length},(_,i)=>value.charCodeAt(i)).join(','));",
        "55296,55,57343\n96,32,36,123,110,117,109,98,101,114,125,32,92,32,55357,56832\n108,105,110,101,32,111,110,101,10,108,105,110,101,32,116,119,111,55\n36,123,110,111,116,65,110,73,100,101,110,116,105,102,105,101,114,125,55\n97,98,55\n",
    );
}

#[test]
fn template_conversions_preserve_string_hint_order_reentry_and_abrupt_completion() {
    compare_source_output_with_setup(
        r#"
        extern JsValue token(func()->void write);
        extern JsValue symbol();
        extern int later();
        extern string trace();
        int value=0;
        void write(){value=2147483647;}
        JsValue delayed=token(write);
        value=0;
        print(`${delayed}${value+1}`);
        value=0;
        string unused=`${delayed}`;
        print(value+1);
        try {print(`${symbol()}${later()}`);}catch{print("caught");}
        print(trace());
        "#,
        "const events=[];globalThis.token=write=>({[Symbol.toPrimitive](hint){events.push(hint);write();if(hint!=='string')throw new Error('wrong hint');return 'converted:'}});globalThis.symbol=()=>Symbol('failure');globalThis.later=()=>{events.push('later');return 9};globalThis.trace=()=>events.join(',');",
        "converted:-2147483648\n-2147483648\ncaught\nstring,string\n",
    );
}

#[test]
fn source_exception_regions_preserve_probe_recovery_and_finalizer_completions() {
    compare_source_output(
        r#"
        int exceptions(int seed) {
          int total = 0;
          try {
            if (seed >= 0) {
              throw "seeded";
            }
            total += 1;
          } catch (auto caught) {
            total += 10;
          } finally {
            total += 100;
          }
          return total;
        }
        print(exceptions(7));print(exceptions(-3));
        int changed=0;
        int saved(){try{return changed;}finally{changed=9;}}
        print(saved());print(changed);
        int replaced(){try{return 1;}finally{return 2;}}
        print(replaced());
        try {
            try {throw "original";} finally {print("inner");throw "replacement";}
        } catch (auto error) {print(error);} finally {print("outer");}
        try {print("normal");} finally {print("done");}
        try {throw null;} catch {print("caught");}
    "#,
        "110\n101\n0\n9\n2\ninner\nreplacement\nouter\nnormal\ndone\ncaught\n",
    );
}

#[test]
fn finalizers_preserve_and_override_loop_transfers_without_copying_bodies() {
    compare_source_output(
        r#"
        int total=0;
        for(int i=0;i<5;i++){
            try {
                if(i==0){continue;}
                if(i==2){break;}
                total+=10;
            } finally {total+=i+1;}
        }
        print(total);
        int visits=0;
        for(int i=0;i<3;i++){
            try {break;} finally {visits+=1;if(i<2){continue;}}
        }
        print(visits);
        int stopped=0;
        for(int i=0;i<3;i++){
            try {continue;} finally {stopped+=1;break;}
        }
        print(stopped);
        int caught=0;
        for(int i=0;i<3;i++){
            try {throw i;}catch{continue;}finally{caught+=1;}
        }
        print(caught);
    "#,
        "16\n3\n1\n3\n",
    );
}

#[test]
fn exception_predecessors_do_not_inherit_values_from_unreached_body_tails() {
    compare_source_output_with_setup(
        r#"
        int choose(bool fail){
            int value=2147483647;
            try {if(fail){throw "early";}value=0;}
            catch {print(value+1);}
            finally {print(value+1);}
            return value+1;
        }
        print(choose(true));print(choose(false));
        extern void fail();
        int value=2147483647;
        try {fail();value=0;} catch(auto error){print(error);print(value+1);}
        finally {print(value+1);}
        print(value+1);
        int stopped(){
            int result=2147483647;
            try {return result+1;result=0;}
            finally {print(result+1);}
        }
        print(stopped());
    "#,
        "globalThis.fail=()=>{throw 'foreign'};",
        "-2147483648\n-2147483648\n-2147483648\n1\n1\nforeign\n-2147483648\n-2147483648\n-2147483648\n-2147483648\n-2147483648\n",
    );
}

#[test]
fn catch_bindings_have_fresh_scope_cells_and_survive_escaping_closures() {
    compare_source_output(
        r#"
        (func()->JsValue)[] readers=[];
        (func()->void)[] writers=[];
        for(int i=0;i<3;i++){
            try {throw i;}catch(auto caught){
                readers.push(()=>caught);
                writers.push(()=>{caught=99;});
            }
        }
        print(readers[0]());print(readers[1]());print(readers[2]());
        writers[1]();print(readers[0]());print(readers[1]());print(readers[2]());
        try {throw "outer";}catch(auto caught){
            auto read=()=>caught;
            try {throw "inner";}catch(auto caught){print(caught);}
            print(read());
        }
    "#,
        "0\n1\n2\n0\n99\n2\ninner\nouter\n",
    );
}

#[test]
fn source_collections_preserve_probe_behavior_aliases_and_nullable_values() {
    // Map/Set are outside the independent interpreter's implemented subset.
    // These explicit observations cover the complete Probe collections body.
    compare_source_output_with_setup(
        r#"
        int collections(int seed) {
          Map<string, int> map = new Map<string, int>();
          map.set("a", seed);
          map.set("b", seed * 2);
          Set<int> set = new Set<int>();
          set.add(seed);
          set.add(seed);
          set.add(seed + 1);
          int total = map.size + set.size;
          if (map.has("a")) { total += map.get("a") ?? 0; }
          return total;
        }
        print(collections(7));print(collections(0));print(collections(-3));
        Map<string,int?> values=new Map();auto alias=values;
        print(values.set("zero",0).set("null",null)==values);
        print(alias.get("zero")??99);print(values.get("null")==null);
        print(values.get("missing")==null);print(values.has("null"));
        print(alias.delete("zero"));print(values.delete("zero"));print(values.size);
        values.clear();print(alias.size);
        Map<string,bool> flags=new Map();flags.set("off",false);print(flags.get("off")??true);
        Map<string,string> words=new Map();words.set("empty","");print(words.get("empty")??"missing");
        Set<float> seen=new Set();print(seen.add(0.0).add(-0.0)==seen);print(seen.size);
        extern float nan();seen.add(nan());seen.add(nan());print(seen.size);print(seen.has(nan()));
        print(seen.delete(0.0));print(seen.size);seen.clear();print(seen.size);
        Map<string,func()->int> callbacks=new Map();callbacks.set("run",()=>7);
        auto found=callbacks.get("run")??(()=>0);print(found());
        extern string nameOf(func()->int value);print(nameOf(found));
    "#,
        "globalThis.nan=()=>NaN;globalThis.nameOf=value=>value.name;",
        "11\n4\n1\ntrue\n0\ntrue\ntrue\ntrue\ntrue\nfalse\n1\n0\nfalse\n\ntrue\n1\n2\ntrue\ntrue\n1\n0\n7\n\n",
    );
}

#[test]
fn collection_receiver_lookup_arguments_and_language_results_stay_ordered() {
    compare_source_output_with_setup(
        r#"
        extern Map<string,int> receiver();extern string key();extern void report();
        extern void install(func()->void mutate);
        int edge=0;install(()=>{edge=2147483647;});edge=0;
        receiver().set(key(),edge+1);report();
        print(receiver().get(key())==null);report();
        print(receiver().size);report();
        receiver().get("unused");report();
    "#,
        r#"
        let events=[],mutate;
        const collection=new Proxy(Object.create(null),{get(target,name){
            events.push('get:'+name);
            if(name==='size')return 4294967295;
            if(name==='set'){
                mutate();
                return function(key,value){events.push('set:'+key+':'+value+':'+(this===collection));return this};
            }
            if(name==='get')return function(key){events.push('get:'+key+':'+(this===collection));return undefined};
            throw Error('unexpected member');
        }});
        globalThis.receiver=()=>{events.push('receiver');return collection};
        globalThis.key=()=>{events.push('key');return 'a'};
        globalThis.install=action=>{mutate=action};
        globalThis.report=()=>{console.log(events.join(','));events=[]};
    "#,
        "receiver,get:set,key,set:a:-2147483648:true\ntrue\nreceiver,get:get,key,get:a:true\n-1\nreceiver,get:size\nreceiver,get:get,get:unused:true\n",
    );
}

#[test]
fn unused_collection_construction_retains_lookup_allocation_and_reentry() {
    compare_source_output_with_setup(
        r#"
        extern void install(func()->void mutate);
        int changes=0;install(()=>{changes+=1;});
        new Map<string,int>();new Set<int>();print(changes);
    "#,
        r#"
        globalThis.install=mutate=>{
            for(const name of ['Map','Set']){
                const Native=globalThis[name];
                Object.defineProperty(globalThis,name,{configurable:true,get(){
                    console.log('lookup:'+name);mutate();
                    return new Proxy(Native,{construct(target,args){
                        console.log('construct:'+name+':'+args.length);mutate();return new Native();
                    }});
                }});
            }
        };
    "#,
        "lookup:Map\nconstruct:Map:0\nlookup:Set\nconstruct:Set:0\n4\n",
    );
}

#[test]
fn binary_memory_construction_preserves_probe_views_and_shared_storage() {
    compare_source_with_interpreter(
        r#"
        int memory(int seed) {
          ArrayBuffer storage = new ArrayBuffer(8);
          Uint8Array bytes = new Uint8Array(storage);
          bytes[0] = seed;
          bytes[1] = seed >>> 8;
          bytes[2] = 255;
          Uint8Array view = bytes.subarray(1, 4);
          int previous = view[0]++;
          return bytes[0] + bytes[1] + bytes[2] + previous + view.byteOffset + view.length;
        }
        print(memory(7));print(memory(0));print(memory(-3));print(memory(511));
        SharedArrayBuffer shared=new SharedArrayBuffer(8);
        Int32Array signed=new Int32Array(shared);
        Uint32Array unsigned=new Uint32Array(shared);
        signed[0]=-1;print(unsigned[0]);print(signed.byteLength);
        auto tail=unsigned.subarray(1);tail[0]=42;print(signed[1]);
        print(tail.byteOffset);print(tail.length);print(tail.buffer==shared);
        auto copied=unsigned.slice(0,1);copied[0]=9;print(signed[0]);print(copied[0]);
    "#,
    );
}

#[test]
fn binary_memory_uses_the_checked_kinds_and_keeps_store_rounding() {
    compare_source_with_interpreter(
        r#"
        Int8Array a=new Int8Array(1);a[0]=255;print(a[0]);
        Uint8Array b=new Uint8Array(1);b[0]=-1;print(b[0]);
        Uint8ClampedArray c=new Uint8ClampedArray(1);c[0]=511;print(c[0]);
        Int16Array d=new Int16Array(1);d[0]=65535;print(d[0]);
        Uint16Array e=new Uint16Array(1);e[0]=-1;print(e[0]);
        Int32Array f=new Int32Array(1);f[0]=-1;print(f[0]);
        Uint32Array g=new Uint32Array(1);g[0]=-1;print(g[0]);
        Float32Array h=new Float32Array(1);h[0]=16777217.0;print(h[0]);
        Float64Array i=new Float64Array(1);i[0]=16777217.0;print(i[0]);
        print(a.length);print(b.byteOffset);print(c.byteLength);print(d.byteLength);
        print(e.byteLength);print(f.byteLength);print(g.byteLength);print(h.byteLength);print(i.byteLength);
        a.fill(7);print(a[0]);auto copy=a.slice(0);a.set(copy);print(a[0]);
    "#,
    );
}

#[test]
fn implicit_constructor_lookup_precedes_arguments_and_preserves_throws() {
    compare_source_output_with_setup(
        r#"
        extern void install(func()->void mutate);
        extern void check(func()->void action);
        int edge=0;
        install(()=>{edge=2147483647;});
        edge=0;
        auto value=new ArrayBuffer(edge+1);
        print(value.byteLength);
        check(()=>{new Uint8Array(-1);});
    "#,
        r#"
        const NativeBuffer=globalThis.ArrayBuffer;
        globalThis.install=mutate=>Object.defineProperty(globalThis,'ArrayBuffer',{
            configurable:true,get(){
                console.log('lookup');mutate();
                return new Proxy(NativeBuffer,{construct(target,args){
                    console.log(args[0]);return new NativeBuffer(0);
                }});
            }
        });
        globalThis.check=action=>{try{action();console.log('missed')}catch(error){console.log(error.name)}};
    "#,
        "lookup\n-2147483648\n0\nRangeError\n",
    );
}

#[test]
fn indexed_compounds_capture_receiver_key_and_old_value_before_rhs() {
    compare_source_with_interpreter(
        r#"
        int[] values=[10,20];int[] original=values;int index=0;
        int rhs(){values=[90,91];index=1;return 2;}
        print(values[index]+=rhs());print(original[0]);print(values[0]);print(index);
        int[] second=values;
        int key(){values=[30,31];return 0;}
        print(values[key()]++);print(second[0]);print(values[0]);
        int[] edge=[2147483647];print(edge[0]++);print(edge[0]);
        print(--edge[0]);print(edge[0]);
        edge[0]>>>=1;print(edge[0]);
        edge[0]/=0;print(edge[0]);
        float[] fractions=[1.25];print(fractions[0]++);print(++fractions[0]);
        string[] words=["a"];print(words[0]+="b");print(words[0]);
    "#,
    );
}

#[test]
fn indexed_updates_preserve_proxy_get_set_order() {
    compare_source_output_with_setup(
        r#"
        extern int[] receiver();extern int key();extern int rhs();extern void report();
        print(receiver()[key()]+=rhs());report();
        print(receiver()[key()]++);report();
        print(++receiver()[key()]);report();
    "#,
        INDEXED_UPDATE_HOST,
        "-2147483648\nreceiver,key,get:0,rhs,set:0:-2147483648\n-2147483648\nreceiver,key,get:0,set:0:-2147483647\n-2147483646\nreceiver,key,get:0,set:0:-2147483646\n",
    );
}

#[test]
#[ignore = "production formation refuses ??= on a place (Unsupported: nullish place assignment)"]
fn indexed_nullish_writes_are_lazy() {
    compare_source_output_with_setup(
        r#"
        extern int key();extern int rhs();extern void report();
        extern (int?)[] nullable();
        print(nullable()[key()]??=rhs());report();
        print(nullable()[key()]??=rhs());report();
    "#,
        INDEXED_UPDATE_HOST,
        "1\nnullable,key,get:0,rhs,set:0:1\n1\nnullable,key,get:0\n",
    );
}

const INDEXED_UPDATE_HOST: &str = r#"
        let events=[];
        const data=new Proxy([2147483647],{
            get:(a,k)=>{events.push('get:'+k);return a[k]},
            set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}
        });
        const optional=new Proxy([null],{
            get:(a,k)=>{events.push('get:'+k);return a[k]},
            set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}
        });
        globalThis.receiver=()=>{events.push('receiver');return data};
        globalThis.nullable=()=>{events.push('nullable');return optional};
        globalThis.key=()=>{events.push('key');return 0};
        globalThis.rhs=()=>{events.push('rhs');return 1};
        globalThis.report=()=>{console.log(events.join(','));events=[]};
    "#;

#[test]
fn typed_array_stores_do_not_redefine_prefix_postfix_or_compound_results() {
    compare_source_output_with_setup(
        r#"
        extern Uint8Array bytes();
        auto values=bytes();
        print(++values[0]);print(values[0]);
        values[0]=255;print(values[0]++);print(values[0]);
        print(values[0]+=257);print(values[0]);
        extern Uint32Array words();
        auto ints=words();print(ints[0]++);print(ints[0]);
        extern Float32Array floats();
        auto fractions=floats();print(++fractions[0]);print(fractions[0]);
    "#,
        "globalThis.bytes=()=>new Uint8Array([255]);globalThis.words=()=>new Uint32Array([4294967295]);globalThis.floats=()=>new Float32Array([16777216]);",
        "256\n0\n255\n0\n257\n1\n-1\n0\n16777217\n16777216\n",
    );
}

#[test]
#[ignore = "production formation refuses ??= on a place (Unsupported: nullish place assignment)"]
fn indexed_nullish_callable_creation_keeps_its_anonymous_name() {
    compare_named_output(
        r#"
        extern string nameOf(func()->int value);
        ((func()->int)?)[] callbacks=[null];
        auto first=callbacks[0]??=()=>7;
        auto second=callbacks[0]??=()=>8;
        print(nameOf(first));print(nameOf(second));print(first());print(second());
    "#,
        "globalThis.nameOf=f=>f.name;",
        "\n\n7\n7\n",
    );
}

#[test]
fn for_control_keeps_continue_update_break_and_initializer_effects() {
    compare_source_with_interpreter(
        r#"
        int steps=0;
        int advance(int value){steps++;print(value);return value+1;}
        int walk(){
            int total=0;
            for(int i=0;i<5;i=advance(i)){
                if(i==1)continue;
                if(i==3)break;
                total+=i;
            }
            return total;
        }
        print(walk());print(steps);
        int i=7;
        for(i=2;i<4;i++)print(i);
        print(i);
        for(print(90);false;print(91))print(92);
        int stop(){for(;;){return 12;}return 90;}
        print(stop());
        int until=0;for(;until<2;)until++;print(until);
        int nested=0;
        for(int a=0;a<3;a++){
            for(int b=0;b<3;b++){
                if(b==1)continue;
                if(a==2)break;
                nested++;
            }
        }
        print(nested);
    "#,
    );
}

#[test]
fn for_initializer_cells_are_shared_and_body_cells_are_fresh() {
    let source = r#"
        void captures(){
            (func()->int)[] shared=[];
            (func()->int)[] fresh=[];
            (func()->int)[] writers=[];
            for(int index=0;index<3;index++){
                int local=index;
                shared.push(()=>index);
                fresh.push(()=>local);
                writers.push(()=>++local);
            }
            print(shared[0]());print(shared[2]());
            print(fresh[0]());print(fresh[1]());print(fresh[2]());
            print(writers[0]());print(fresh[0]());print(fresh[1]());
            int index=90;print(index);
        }
        captures();
    "#;
    // These are source cell-lifetime checks, not a choice of JS spelling.
    compare_source_with_interpreter(source);
    compare_source_output(source, "3\n3\n0\n1\n2\n1\n1\n1\n90\n");
}

#[test]
fn for_update_facts_do_not_assume_code_after_continue_executed() {
    compare_source_with_interpreter(
        r#"
        int edge=2147483647;
        int count=0;
        for(;count<1;print(edge+1)){
            count++;
            continue;
            edge=0;
        }
        int snapshots=0;
        int checks=0;
        bool test(int previous){checks++;return previous<2;}
        for(;test(snapshots++);print(snapshots++))print(snapshots);
        print(snapshots);print(checks);
        int absent=0;
        for(;false;print(absent++))print(999);
        print(absent);
        int skip(){for(int i=0;i<3;print(999))return i;return 90;}
        print(skip());
    "#,
    );
}

#[test]
fn typed_primitive_values_keep_utf16_effects_and_mutable_collection_observations() {
    // String.length is outside the current interpreter's implemented member
    // set. These declared cases complement the independent code-unit checks.
    compare_source_output(
        r#"
        int calls=0;
        string next(){calls=calls+1;return "𝄞";}
        next().length;print(calls);
        next().charCodeAt(0);print(calls);
        print("𝄞".length);print("𝄞".charCodeAt(0));print("𝄞".charCodeAt(1));
        print("𝄞".charCodeAt(2));print("\ud800".charCodeAt(0));
        int index=0;print("".charCodeAt(index=3));print(index);
        "ab".charAt(0);"ab".indexOf("a");
        int[] values=[1];int first=values.length;values.push(2);
        print(first);print(values.length);
        print("abc".indexOf("b"));print("abcabc".indexOf("b",3));
    "#,
        "1\n2\n2\n55348\n56606\n0\n55296\n0\n3\n1\n2\n1\n4\n",
    );
}

#[test]
fn marked_bracket_scanner_preserves_its_real_control_and_utf16_operations() {
    let source = include_str!("../../finer/tools/fixtures/marked-brackets.lil");
    // The archived port body is unchanged except for removing its export
    // keyword. These declared boundary cases execute against the emitted JS;
    // the current reference interpreter does not implement string.length.
    // Explicit calls also keep the Rust test independent of an extern setup.
    let body = source
        .split("extern string sample(int index);")
        .next()
        .unwrap();
    let source = format!(
        r#"{body}
        print(findClosingBracket("))","(",")"));
        print(findClosingBracket("ab(cd)e)tail","(",")"));
        print(findClosingBracket("()","(",")"));
        print(findClosingBracket("((x)","(",")"));
        print(findClosingBracket("a\\)b)","(",")"));
        print(findClosingBracket("𝄞)text","(",")"));
    "#
    );
    compare_source_output(&source, "0\n7\n-1\n-2\n4\n2\n");
}

#[test]
fn escaped_record_keys_and_inferred_names_preserve_code_units() {
    compare_source_output_with_setup(
        r#"
        extern string nameOf(func()->int value);
        func()->int fallback=()=>0;
        Record<func()->int> callbacks=record{"\x68andler":()=>7,"\ud800":()=>8};
        print(nameOf(callbacks["handler"]??fallback));
        print(nameOf(callbacks["\ud800"]??fallback).charCodeAt(0));
        print((callbacks["handler"]??fallback)());
        print((callbacks["\ud800"]??fallback)());
        "#,
        "globalThis.nameOf=value=>value.name;",
        "handler\n55296\n7\n8\n",
    );
}

#[test]
fn exact_string_values_cross_cells_joins_and_utf16_operations() {
    compare_source_output(
        r#"
        string values(bool choice) {
            string high="\ud83d";
            string pair=high+"\ude00";
            string same="";
            if(choice){same="\x61";}else{same="a";}
            return `${pair.length},${pair.charCodeAt(1)},${pair.indexOf("\ude00")},${pair.charAt(-1).length},${pair.charAt(1).charCodeAt(0)},${same}`;
        }
        print(values(true));print(values(false));
        string changed="ab";
        print((changed="different").length);print(changed);
        print(`\u0024${"{"}identifier}${"\\"}${"`"}`);
        print(`a${true}${2147483647}`);
        float negative=-0.0;print(`zero=${negative}`);
        "#,
        "2,56832,1,0,56832,a\n2,56832,1,0,56832,a\n9\ndifferent\n${identifier}\\`\natrue2147483647\nzero=0\n",
    );
}

#[test]
fn source_operations_with_one_diagnostic_span_keep_distinct_resolution() {
    use crate::ast::{ArrayElement, ExprKind, Ident, Item, SourceNodes, Stmt};
    use crate::primitive::Intrinsic;
    use crate::primitive::ResolvedIntrinsic::Property;
    use crate::semantic::BuiltinCall;
    let arena = bumpalo::Bump::new();
    let empty = crate::parse_source(&arena, "").unwrap();
    let nodes = SourceNodes::default();
    let span = crate::span::Span::empty(0);
    let ident = |name| Ident { name, span };
    // Generated syntax can have one diagnostic location while each occurrence
    // still has its own binding, operation and result obligations.
    let array = arena.alloc(nodes.expression(ExprKind::ArrayLiteral {
        elements: arena.alloc_slice_fill_iter([ArrayElement::Value(
            nodes.expression(ExprKind::Int(2, span)),
        )]),
        span,
    }));
    let array_length = nodes.expression(ExprKind::Member {
        object: array,
        property: ident("length"),
        span,
    });
    let string_length = nodes.expression(ExprKind::Member {
        object: arena.alloc(nodes.expression(ExprKind::String("😀x", span))),
        property: ident("length"),
        span,
    });
    let (array_id, string_id) = (array_length.id, string_length.id);
    let multiplied = nodes.expression(ExprKind::Call {
        callee: arena.alloc(nodes.expression(ExprKind::Member {
            object: arena.alloc(nodes.expression(ExprKind::Ident(ident("Math")))),
            property: ident("imul"),
            span,
        })),
        args: arena.alloc_slice_fill_iter([array_length, string_length].into_iter().map(
            |expression| crate::ast::Argument {
                span: expression.span(),
                expression,
                passing: crate::primitive::ParameterPassing::Value,
            },
        )),
        span,
    });
    let multiply_id = multiplied.id;
    let printed = nodes.expression(ExprKind::Call {
        callee: arena.alloc(nodes.expression(ExprKind::Ident(ident("print")))),
        args: arena.alloc_slice_fill_iter([multiplied].into_iter().map(|expression| {
            crate::ast::Argument {
                span: expression.span(),
                expression,
                passing: crate::primitive::ParameterPassing::Value,
            }
        })),
        span,
    });
    let print_id = printed.id;
    let program = empty.with_items(
        &nodes,
        arena.alloc_slice_fill_iter([Item::Stmt(Stmt::Expr(printed))]),
    );
    let semantics = crate::analyze(&program).unwrap();
    assert_eq!(
        semantics.resolved_intrinsic(array_id),
        Some(Property(Intrinsic::ArrayLength))
    );
    assert_eq!(
        semantics.resolved_intrinsic(string_id),
        Some(Property(Intrinsic::StringLength))
    );
    assert_eq!(
        semantics.builtin_call(multiply_id),
        Some(BuiltinCall::MathImul)
    );
    assert_eq!(semantics.builtin_call(print_id), Some(BuiltinCall::Print));
    assert_eq!(
        crate::interpreter::interpret_program(&program, &semantics).unwrap(),
        "3\n"
    );
    compare_program_output(&program, &semantics, CONFIG, "", "3\n");
    // imul's exact low bits differ from binary64 multiplication followed by
    // source integer normalization. Its target call must retain that identity.
    compare_source_output("print(Math.imul(2147483647,2147483647));", "1\n");
}

#[test]
fn callable_source_occurrences_do_not_alias_at_the_same_span() {
    use crate::ast::{ExprKind, Item, SourceNodes, Stmt};
    let arena = bumpalo::Bump::new();
    let parsed = crate::parse_source(
        &arena,
        "auto first=()=>1;auto second=()=>2;print(first());print(second());",
    )
    .unwrap();
    let nodes = SourceNodes::continuing(parsed.source_identity());
    let mut items = parsed.items.to_vec();
    for item in &mut items {
        if let Item::Stmt(Stmt::VarDecl(declaration)) = item {
            if let ExprKind::ArrowFunction { span, .. } =
                &mut declaration.initializer.as_mut().unwrap().kind
            {
                *span = crate::span::Span::empty(0);
            }
        }
    }
    let program = parsed.with_items(&nodes, arena.alloc_slice_fill_iter(items));
    let semantics = crate::analyze(&program).unwrap();
    compare_program_output(&program, &semantics, CONFIG, "", "1\n2\n");
}

#[test]
fn escaping_callable_names_survive_storage_elimination_and_alias_renaming() {
    compare_named_output(
        r#"
        extern string nameOf(func()->int value);
        func()->int pass(func()->int veryLongCallbackParameter){
            auto anotherLongAlias=veryLongCallbackParameter;
            return anotherLongAlias;
        }
        int declared(){return 7;}
        auto original=()=>1;
        auto result=(original=()=>2);
        auto other=()=>3;
        auto third=(other=()=>4);
        print(nameOf(pass(result)));
        print(nameOf(pass(third)));
        print(nameOf(()=>5));
        print(nameOf(pass(declared)));
    "#,
        "globalThis.nameOf=value=>value.name;",
        "original\nother\n\ndeclared\n",
    );
}

#[test]
fn scoped_names_reuse_siblings_and_constrain_references_across_intervening_scopes() {
    compare_source_output(
        r#"
        int a(){return 2;}
        int left(int parameter){
            auto closure=(int increment)=>parameter+increment;
            return closure(a());
        }
        int right(int parameter){return parameter+1;}
        print(left(5));print(right(7));
    "#,
        "7\n8\n",
    );
    compare_source_output(
        r#"
        int outer=7;
        int independent(int reusable){return reusable+1;}
        int enclosing(int captured){
            auto nested=(int argument)=>captured+outer+argument;
            return nested(3);
        }
        print(independent(2));print(enclosing(4));print(outer);
    "#,
        "3\n14\n7\n",
    );
    compare_source_output(
        r#"
        int outer=7;
        int independent(int reusable){return reusable+1;}
        int enclosing(int captured){
            auto nested=(int argument)=>{outer=argument;return captured;};
            return nested(3);
        }
        print(independent(2));print(enclosing(4));print(outer);
    "#,
        "3\n4\n3\n",
    );
}

#[test]
fn generic_callbacks_and_host_reads_keep_their_values() {
    compare_source_output_with_setup(
        r#"
        extern int read();
        bool sameParity(int previous,int next){return previous%2==next%2;}
        bool compareWith<T>(T value,func(T,T)->bool compare){return compare(value,value);}
        print(compareWith(read(),sameParity));
    "#,
        "globalThis.read=()=>7;",
        "true\n",
    );
    compare_source_output_with_setup(
        "extern int read();int total(int first,int second){return first+second;}print(total(read(),3));",
        "globalThis.read=()=>7;",
        "10\n",
    );
}

#[test]
fn private_functions_keep_names_that_are_observed() {
    compare_source_output(
        "int privateArithmeticFunction(int value){return value+2;}print(privateArithmeticFunction(7));",
        "9\n",
    );
    compare_named_output(
        "extern string nameOf(func(int)->int value);int exposedFunction(int value){return value;}print(nameOf(exposedFunction));",
        "globalThis.nameOf=value=>value.name;",
        "exposedFunction\n",
    );
}

#[test]
fn dead_locals_and_constant_control_leave_values_and_arithmetic_intact() {
    compare_source_output(
        "int f(int x){int a=x;int b=a;int unused=7;42;if(false){print(99);}return b;}print(f(3));",
        "3\n",
    );
    compare_source_output(
        "int f(int x){int a=x+1;int b=a+2;if(false){print(99);}return b;}print(f(3));",
        "6\n",
    );
    compare_source_with_interpreter("int f(int x){return x>>>1;}print(f(2147483647));");
}

#[test]
fn shared_value_dependencies_stay_linear_and_exact() {
    let mut source = String::from("int f(int input){int v0=input;");
    for index in 1..=72 {
        source.push_str(&format!("int v{index}=v{0}+v{0};", index - 1));
    }
    source.push_str("return v72;}print(f(7));");
    compare_source_output(&source, "0\n");
}

#[test]
fn range_proofs_preserve_overflow_negative_zero_and_explicit_bitwise_operations() {
    compare_source_with_interpreter(
        r#"
        int unsigned(int x,int shift){int y=x>>>shift;return y;}
        int positive(int x){int y=x>>>1;return y;}
        print(unsigned(-1,0)); print(unsigned(-1,32)); print(positive(-1));
        int zero=-0;print(zero); print(2147483647+1);
        int explicit(int x){return x|0;}print(explicit(7));
    "#,
    );
    compare_source_with_interpreter("int f(int x){return (x>>>1)|0;}print(f(-1));");
}

#[test]
fn postfix_snapshots_survive_captured_mutation_and_short_circuiting() {
    compare_source_with_interpreter(
        r#"
        int cell=4;int temporary=99;int a=42;
        auto advance=()=>{cell+=2;return cell++;};
        print(advance());print(cell);
        print(cell++ + cell++);print(cell);
        bool skipped=false&&(cell++>0);print(cell);
        bool taken=true&&(cell++>0);print(cell);
        print(temporary);print(a);
    "#,
    );
    compare_source_output("int value=1;value++;--value;print(value);", "1\n");
}

#[test]
#[ignore = "production formation refuses ??= on a place (Unsupported: nullish place assignment)"]
fn nullish_assignment_evaluates_its_value_only_when_absent() {
    compare_source_with_interpreter(
        r#"
        int cell=4;
        int? missing=null;int? present=0;
        print(missing??=(cell++));print(present??=(cell++));print(cell);
    "#,
    );
}

#[test]
fn constant_language_operations_match_independent_evaluation_at_i32_boundaries() {
    let values = [
        i32::MIN,
        -1073741825,
        -9,
        -1,
        0,
        1,
        2,
        31,
        32,
        1073741825,
        i32::MAX,
    ];
    let literal = |value: i32| {
        if value == i32::MIN {
            "(-2147483647-1)".to_string()
        } else {
            format!("({value})")
        }
    };
    let mut source = String::new();
    for left in values {
        for right in values {
            for operator in ["+", "-", "*", "/", "%", ">>>"] {
                source.push_str(&format!(
                    "print({}{operator}{});",
                    literal(left),
                    literal(right)
                ));
            }
        }
    }
    // The interpreter does not call the compiler's constant evaluator. The
    // matrix holds binary64 multiplication, signed overflow, zero division,
    // negative remainders and masked shift counts.
    compare_source_with_interpreter(&source);
}

#[test]
fn dead_closures_and_blocks_leave_live_values() {
    compare_source_with_interpreter(
        r#"
        int outer(int x) {
            if(false){int abandoned=99;auto gone=()=>abandoned;print(gone());}
            int removed=123;
            auto reader=()=>x+1;
            return reader();
        }
        print(outer(7));
    "#,
    );
}

#[test]
fn signed_offset_reassociation_preserves_rounding_effects_and_materialization() {
    let values = [i32::MIN, -9, -1, 0, 1, 31, 32, 1073741825, i32::MAX];
    let literal = |value: i32| {
        if value == i32::MIN {
            "(-2147483647-1)".to_string()
        } else {
            format!("({value})")
        }
    };
    let mut source = String::new();
    for (index, (a, b)) in values.into_iter().zip(values.into_iter().rev()).enumerate() {
        source.push_str(&format!(
            "int f{index}(int x){{int a=x+{};int b=a-{};return 31+b;}}",
            literal(a),
            literal(b)
        ));
        for input in values {
            source.push_str(&format!("print(f{index}({}));", literal(input)));
        }
    }
    source.push_str(
        r#"
        int calls=0;
        int next(){calls=calls+1;return 2147483647;}
        print((next()+2147483647)+2147483647);print(calls);
        int cell=7; int old=cell+1; cell=99; print(old+2);print(old);
        int multiply(int x){return (x*1073741825)*1073741825;}
        print(multiply(1073741825));
        float precision(float x){return (x+10000000000000000.0)-10000000000000000.0;}
        print(precision(1.0));
    "#,
    );
    compare_source_with_interpreter(&source);
    compare_source_output(
        "int f(int x){int a=x+1;int b=a+2;return b+3;}print(f(7));",
        "13\n",
    );
}

#[test]
fn private_array_observations_keep_initialization_effects_at_their_original_site() {
    compare_source_with_interpreter(
        r#"
        int calls=0;
        int next(){calls=calls+1;print(calls);return calls;}
        int[] values=[next(),next(),7];
        print(100);print(values.length);print(values.length);print(calls);
    "#,
    );
    compare_source_with_interpreter(
        r#"
        int[] original=[1,2];int[] alias=original;original=[3];
        print(alias.length);print(original.length);
        int[] captured=[1,2];auto length=()=>captured.length;print(length());
        int count(int[] values){return values.length;}
        int[] passed=[3,4,5];print(count(passed));
    "#,
    );
}

#[test]
fn cell_values_follow_blocks_branches_loops_and_captured_writes() {
    compare_source_with_interpreter(
        r#"
        int f(int input){
            int local=input;{local=local+1;}
            int shared=1;auto write=()=>{shared=9;};write();
            return local+shared;
        }
        print(f(7));
    "#,
    );
    compare_source_with_interpreter("int f(int x){x=2;x;return x;}print(f(7));");
    compare_source_with_interpreter(
        r#"
        int values(int choice) {
            int x=2; x=5; int a=x+3;
            if(choice>0){x=9;}else{x=9;}
            int b=x+2;
            if(choice>0){x=3;}else{x=8;}
            int c=x+1;
            x=20;
            return a+b+c+x;
        }
        print(values(0));print(values(1));
    "#,
    );
    compare_source_with_interpreter(
        r#"
        int looped(int n) {
            int x=2; int i=0;
            while(i<n) {
                x=x+1;i=i+1;
                if(i==2){continue;}
                if(i==4){break;}
                print(x+1);
            }
            print(x+1);
            bool update=n>0&&(x=8)>0;
            print(x+1);
            bool keep=n>0||(x=12)>0;
            return x+1;
        }
        print(looped(0));print(looped(1));print(looped(8));
    "#,
    );
    compare_source_with_interpreter(
        r#"
        int outer(int n) {
            int cell=1;
            auto change=(int value)=>{cell=value;return cell;};
            int before=cell+1;
            int result=change(n)+cell;
            return before+result+cell;
        }
        print(outer(4));print(outer(7));
    "#,
    );
}

#[test]
fn unobserved_cells_disappear_without_losing_initializer_effects_or_assignment_values() {
    compare_source_with_interpreter(
        r#"
        int tick(int value){print(value);return value;}
        int work(){int discarded=tick(1);discarded=tick(2);int result=(discarded=tick(3))+5;return result;}
        int neverCalled(){print(99);return 1;}
        print(work());
    "#,
    );
}

#[test]
fn checked_primitive_and_collection_operations_keep_their_results() {
    use crate::primitive::Intrinsic;
    use crate::primitive::ResolvedIntrinsic::{Constructor, Property};
    compare_source_output(
        "Map<string,int> values=new Map();auto found=values.get(\"missing\");print(found==null);",
        "true\n",
    );
    let source = r#"
        ArrayBuffer storage=new ArrayBuffer(8);
        auto view=new Uint8Array(storage);
        int inspect(ArrayBuffer? value){return value?.byteLength??0;}
        print(view.length);print(view.byteLength);print(view.byteOffset);auto underlying=view.buffer;
        print(inspect(storage));print(inspect(null));
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let initializer = |index: usize| {
        let crate::ast::Item::Stmt(crate::ast::Stmt::VarDecl(declaration)) = &program.items[index]
        else {
            panic!("expected variable")
        };
        declaration.initializer.as_ref().unwrap()
    };
    let printed = |index: usize| {
        let crate::ast::Item::Stmt(crate::ast::Stmt::Expr(value)) = &program.items[index] else {
            panic!("expected print")
        };
        let crate::ast::ExprKind::Call { args, .. } = &value.kind else {
            panic!("expected call")
        };
        &args[0]
    };
    let crate::ast::Item::Function(inspect) = &program.items[2] else {
        panic!("expected inspect")
    };
    let crate::ast::Stmt::Return {
        value: Some(value), ..
    } = &inspect.body[0]
    else {
        panic!("expected return")
    };
    let crate::ast::ExprKind::Binary { lhs: optional, .. } = &value.kind else {
        panic!("expected fallback")
    };
    let operation =
        |expression: &crate::ast::Expr<'_, '_>| semantics.resolved_intrinsic(expression.id);
    assert_eq!(
        operation(initializer(0)),
        Some(Constructor(Intrinsic::ArrayBufferNew))
    );
    assert_eq!(
        operation(initializer(1)),
        Some(Constructor(Intrinsic::Uint8ArrayNew))
    );
    assert_eq!(
        operation(&printed(3).expression),
        Some(Property(Intrinsic::Uint8ArrayLength))
    );
    assert_eq!(
        operation(&printed(4).expression),
        Some(Property(Intrinsic::Uint8ArrayByteLength))
    );
    assert_eq!(
        operation(&printed(5).expression),
        Some(Property(Intrinsic::Uint8ArrayByteOffset))
    );
    assert_eq!(
        operation(initializer(6)),
        Some(Property(Intrinsic::Uint8ArrayBuffer))
    );
    assert_eq!(
        operation(optional),
        Some(Property(Intrinsic::BufferByteLength))
    );
    compare_program_output(&program, &semantics, CONFIG, "", "8\n8\n0\n8\n0\n");
    compare_source_output(
        "ArrayBuffer value=new ArrayBuffer(4);print(value.byteLength);",
        "4\n",
    );
}

#[test]
fn for_updates_keep_their_scope_and_run_after_each_iteration() {
    compare_source_output_with_setup(
        r#"
        extern int take(int value);
        int outer=take(3);
        int run(int input){
            int current=input;
            for(int counter=0;counter<2;current+=outer){
                counter++;
            }
            return current;
        }
        print(run(4));
    "#,
        "globalThis.take=x=>x;",
        "10\n",
    );
}

#[test]
fn empty_control_retains_condition_effects_and_repeated_loop_execution() {
    compare_source_with_interpreter(
        r#"
        int identity(int n){{if(n>0){int gone=n+1;}else{int gone=n+2;}}return n;}
        bool choose(){print(5);return true;}
        if(choose()){int gone=1;}else{int gone=2;}
        int count=0;
        bool next(){count=count+1;print(count);return count<3;}
        while(next()){int gone=1;}
        print(count);print(identity(7));
    "#,
    );
}

#[test]
fn primitive_integer_results_use_emitted_normalization_without_assuming_host_range() {
    // This operation is taken from Probe's real codeAt coverage. The
    // independent interpreter uses UTF-16 and zero for an absent code unit.
    compare_source_with_interpreter(
        r#"
        int codeAt(string text,int index){return text.charCodeAt(index);}
        int last(int[] values){return values.pop();}
        print(codeAt("ab",0));print(codeAt("",0));print(codeAt("ab",-1));
        print(codeAt("𝄞",0));print(codeAt("𝄞",1));print(codeAt("𝄞",2));
    "#,
    );
}

#[test]
fn doubling_string_chains_keep_their_exact_length() {
    let mut source = String::from("string part0=\"x\";");
    for i in 1..=14 {
        source.push_str(&format!("string part{i}=part{}+part{};", i - 1, i - 1));
    }
    source.push_str("print(part14.length);");
    compare_source_output(&source, "16384\n");
}

#[test]
fn string_operations_keep_host_order_and_code_units() {
    let cases: &[(&str, &str, &str)] = &[
        (
            r#"
            extern int read();
            int strings(int seed) {
                string a="probe";string b=a+"-lil";
                string t=`seed=${seed} len=${b.length}`;
                int code=b.charCodeAt(1);int found=t.indexOf("len");
                string sliced=b.slice(0,5);
                return b.length+t.length+code+found+sliced.length;
            }
            print(strings(read()));print(strings(-3));print(strings(0));
            "#,
            "globalThis.read=()=>17;",
            "149\n149\n147\n",
        ),
        (
            // Method results are mutable host operations: `code` keeps its
            // evaluation before indexOf and slice.
            r#"
            int strings(int seed) {
                string a="probe";string b=a+"-lil";
                string t=`seed=${seed} len=${b.length}`;
                int code=b.charCodeAt(1);int found=t.indexOf("len");
                string sliced=b.slice(0,5);
                return b.length+t.length+code+found+sliced.length;
            }
            print(strings(17));
            "#,
            r#"
            const events=[];const output=console.log.bind(console);
            console.log=value=>output(JSON.stringify([...events,value]));
            String.prototype.charCodeAt=function(){events.push('code');return {valueOf(){events.push('code-coerce');return 1}}};
            String.prototype.indexOf=function(){events.push('find');return 2};
            String.prototype.slice=function(){events.push('slice');return {get length(){events.push('length');return 3}}};
            "#,
            "[\"code\",\"code-coerce\",\"find\",\"slice\",\"length\",28]\n",
        ),
        (
            r#"
            extern void units(string value);
            int amount=7;
            units(`\ud800${amount}\udfff`);
            units(`\u0024{notAnIdentifier}${amount}`);
            units(`a${amount}`);
            "#,
            "globalThis.units=value=>console.log(Array.from({length:value.length},(_,i)=>value.charCodeAt(i)).join(','));",
            "55296,55,57343\n36,123,110,111,116,65,110,73,100,101,110,116,105,102,105,101,114,125,55\n97,55\n",
        ),
        (
            r#"
            extern int take();
            int source=take();
            int unused(){return source;}
            if(false){print(unused());}
            int move(int seed){int first=seed+1;int second=first+2;return second+3;}
            int outer(){int captured=take();func()->int forgotten=()=>captured;return 2;}
            int nested(){int n=7;string text=`${n}`;int size=text.length;return size+size;}
            print(move(8));print(outer());print(nested());
            "#,
            "globalThis.take=()=>{console.log('take');return 5};",
            "take\n14\ntake\n2\n2\n",
        ),
        (
            r#"
            extern int take();
            int abandoned=0;int observed=0;
            observed=(abandoned=take());
            int[] lengths=[abandoned];
            print(lengths.length);print(observed);
            "#,
            "globalThis.take=()=>{console.log('take');return 5};",
            "take\n1\n5\n",
        ),
    ];
    for &(source, setup, expected) in cases {
        compare_source_output_with_setup(source, setup, expected);
    }
}

#[test]
fn deep_initializers_keep_their_value_whatever_their_depth() {
    for (initial_depth, returned_depth, expected) in [(1, 3, "12\n"), (200, 350, "558\n")] {
        let source = format!(
            "int work(int seed){{int a=seed{};int b=a+1;func()->int abandoned=()=>a;return b{};}}print(work(7));",
            "+1".repeat(initial_depth),
            "+1".repeat(returned_depth)
        );
        compare_source_output(&source, expected);
    }
}

#[test]
fn exports_are_live_bindings_that_survive_cleanup() {
    let source = "int unused=0;export int value=1;export int advance(){value+=1;return value;}print(advance());";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        let javascript = formed(&syntax, &checked, CONFIG, style, true);
        let script = format!(
            "const lib=await import('data:text/javascript,'+encodeURIComponent({}));console.log(lib.value,lib.advance(),lib.value,lib.advance.name);",
            serde_json::to_string(&javascript).unwrap()
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{javascript}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "2\n2 3 3 advance\n",
            "{style:?}\n{javascript}"
        );
    }
}

#[test]
fn parameter_values_follow_every_actual_argument() {
    for source in [
        "int arithmetic(int value){return value+1;}print(arithmetic(3));",
        "int arithmetic(int value){return value+1;}int actual=3;print(arithmetic(actual));",
        "string arithmetic(string value){return value+\"!\";}print(arithmetic(\"x\".slice(0)));",
        "float arithmetic(float value){return value+1.0;}print(arithmetic(-0.0));",
        "float arithmetic(float value){return value+1.0;}print(arithmetic(1.0));print(arithmetic(4294967296.0));",
    ] {
        compare_source_with_interpreter(source);
    }
    compare_source_output_with_setup(
        "extern int opaque();int arithmetic(int value){return value+1;}print(arithmetic(3));print(arithmetic(opaque()));",
        "globalThis.opaque=()=>5;",
        "4\n6\n",
    );
    compare_source_output_with_setup(
        "extern void retain(func(int)->int callback);int arithmetic(int value){return value+1;}retain(arithmetic);print(arithmetic(3));",
        "globalThis.retain=callback=>{};",
        "4\n",
    );
    compare_source_output_with_setup(
        "extern int opaque();int arithmetic(int value){auto mutate=()=>{value=opaque();};mutate();return value+1;}print(arithmetic(3));",
        "globalThis.opaque=()=>5;",
        "6\n",
    );
}

#[test]
fn opaque_host_arguments_keep_their_coercion_and_throws() {
    let source = "extern int opaque();int arithmetic(int value){int discarded=value+1;return 7;}print(arithmetic(opaque()));";
    for (host, expected) in [
        (
            "({valueOf(){events.push('coerce');throw Error('opaque')}})",
            "[\"coerce\",\"Error\"]\n",
        ),
        ("Symbol('opaque')", "[\"TypeError\"]\n"),
        ("1n", "[\"TypeError\"]\n"),
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let javascript = formed(&syntax, &checked, CONFIG, Style::Global, false);
        let script = format!(
            "const events=[];globalThis.opaque=()=>({host});try{{await import('data:text/javascript,'+encodeURIComponent({}))}}catch(error){{events.push(error.name)}}console.log(JSON.stringify(events));",
            serde_json::to_string(&javascript).unwrap()
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap(), expected, "{javascript}");
    }
}

fn refused(source: &str) -> bool {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    match crate::analyze(&syntax) {
        Err(_) => true,
        Ok(checked) => from_checked_source(&syntax, &checked).is_err(),
    }
}

#[test]
fn a_source_eval_binding_is_refused() {
    assert!(refused("extern int eval(string source);"));
}

#[test]
#[ignore = "production compiles a detached primitive method read, which throws when called; it must be refused"]
fn primitive_method_values_are_refused() {
    for source in [
        "auto method=\"text\".charCodeAt;print(method(0));",
        "auto values=new Map<string,int>();auto method=values.get;print(method(\"x\"));",
        "auto values=new Set<int>();auto method=values.has;print(method(1));",
    ] {
        assert!(refused(source), "{source}");
    }
}

#[test]
fn string_escapes_keep_their_code_units() {
    compare_source_with_interpreter(r#"print("a\n\u0041\t");print("\ud83d\ude00");"#);
}

/// Runs the program after `setup`, then `after`, and returns the `trace`
/// array the host functions fill, for every naming style.
fn compare_trace(source: &str, setup: &str, after: &str, expected: &str) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        let javascript = formed(&syntax, &checked, CONFIG, style, false);
        let script = format!(
            "const trace=[];{setup}\nawait import('data:text/javascript,'+encodeURIComponent({}));\n{after}\nprocess.stdout.write(JSON.stringify(trace));",
            serde_json::to_string(&javascript).unwrap()
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for mutable method observations");
        assert!(
            result.status.success(),
            "{style:?}: {}\n{javascript}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            expected,
            "{style:?}: {javascript}"
        );
    }
}

#[test]
fn a_module_helper_frame_stays_hidden_from_a_sloppy_host_caller() {
    // Function constructs a sloppy host callback. A module's helper frames
    // are strict, so `probe.caller` cannot reach and re-enter them.
    compare_trace(
        "extern void probe();int helper(int value){probe();int discarded=value+1;return 7;}print(helper(1));",
        "console.log=value=>trace.push(value);globalThis.saved=null;globalThis.probe=Function(\"if(!globalThis.saved)globalThis.saved=globalThis.probe.caller;\");",
        "trace.push(saved===null?'sealed':'retained');",
        "[7,\"sealed\"]",
    );
}

#[test]
fn mutable_method_lookup_and_invocation_keep_distinct_cell_snapshots() {
    compare_trace(
        r#"
            extern void install(func()->void lookup,func()->void invoke);
            extern void observe(int value);
            int state=0;
            void lookup(){state=2;}
            void invoke(){state=3;}
            int second(){state=5;return state;}
            install(lookup,invoke);state=1;
            "ab".slice(state,second());
            observe(state);
        "#,
        r#"
            const original=Object.getOwnPropertyDescriptor(String.prototype,'slice');
            globalThis.install=(lookup,invoke)=>Object.defineProperty(String.prototype,'slice',{
                configurable:true,get(){
                    if(String(this)!=='ab')return original.value;
                    trace.push('lookup');lookup();
                    return function(start,end){trace.push(['args',start,end]);invoke();return 'result';};
                }
            });
            globalThis.observe=value=>{Object.defineProperty(String.prototype,'slice',original);trace.push(['after',value]);};
        "#,
        "",
        r#"["lookup",["args",2,5],["after",3]]"#,
    );
}

#[test]
fn replaced_slice_and_split_preserve_raw_values_aliases_and_length_getters() {
    compare_trace(
        r#"
            extern void inspect(string[] first,string[] second,string value);
            string[] first="ab".split(",");
            string[] second="ab".split(",");
            inspect(first,second,"ab".slice(0));
        "#,
        r#"
            const originalSplit=String.prototype.split,originalSlice=String.prototype.slice;
            const shared=['initial'],returned={valueOf(){throw Error('unexpected coercion');}};
            String.prototype.split=function(){trace.push('split');return shared;};
            String.prototype.slice=function(){trace.push('slice');return returned;};
            globalThis.inspect=(a,b,value)=>{
                String.prototype.split=originalSplit;String.prototype.slice=originalSlice;
                trace.push([a===b,value===returned]);a[0]='changed';trace.push(b[0]);
            };
        "#,
        "",
        r#"["split","split","slice",[true,true],"changed"]"#,
    );
    compare_trace(
        r#"extern void restore();"ab".split(",").length;restore();"#,
        r#"
            const original=String.prototype.split;
            String.prototype.split=function(){trace.push('split');return {
                get length(){trace.push('length');return {valueOf(){trace.push('coerce');return 5;}};}
            };};
            globalThis.restore=()=>{String.prototype.split=original;};
        "#,
        "",
        r#"["split","length","coerce"]"#,
    );
}

#[test]
fn mutable_integer_methods_keep_normalization_and_observable_result_coercion() {
    compare_trace(
        r#"extern void observe(int first,int second);observe("ab".charCodeAt(0),"ab".indexOf("a"));"#,
        r#"
            const code=String.prototype.charCodeAt,find=String.prototype.indexOf;
            String.prototype.charCodeAt=function(){trace.push('code');return {
                valueOf(){trace.push('coerce');return 4294967297;}
            };};
            String.prototype.indexOf=function(){trace.push('find');return -2147483649;};
            globalThis.observe=(a,b)=>{String.prototype.charCodeAt=code;String.prototype.indexOf=find;trace.push([a,b]);};
        "#,
        "",
        r#"["code","coerce","find",[1,2147483647]]"#,
    );
}
