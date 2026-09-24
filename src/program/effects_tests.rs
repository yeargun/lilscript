//! Effect summaries (M6.2), the `pure` contract (M6.3) and discarded
//! effect-free calls (M7.2), including the D3.6 termination refusals.
use super::call_graph::Seal;
use super::effects::{ParameterSet, Regions, UnitEffects};
use super::publication::*;
use super::views::Fact;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};

fn program<'src>(arena: &'src bumpalo::Bump, source: &'src str) -> Program<'src> {
    let syntax = crate::parse_source(arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    program.verify().unwrap();
    program
}

fn body(program: &Program<'_>, name: &str) -> UnitId {
    program
        .cells()
        .iter()
        .find_map(|cell| match cell.binding {
            CellBinding::Function(unit) if cell.name == name => Some(unit),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no function `{name}`"))
}

/// Summaries of the named functions, under module sealing.
fn summaries(source: &str, names: &[&str]) -> Vec<UnitEffects> {
    let arena = bumpalo::Bump::new();
    let program = program(&arena, source);
    let effects = program.effects(Seal::Module);
    names
        .iter()
        .map(|name| {
            effects
                .summary(body(&program, name))
                .unwrap_or_else(|| panic!("no summary for `{name}`"))
                .clone()
        })
        .collect()
}

fn summary(source: &str, name: &str) -> UnitEffects {
    summaries(source, &[name]).pop().unwrap()
}

fn compile(source: &str, module: bool) -> String {
    let arena = bumpalo::Bump::new();
    let program = program(&arena, source);
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    let policy: ResolvedPolicy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap();
    let mut compiler = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 20_000_000,
                optional_work: 20_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 32_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 4 },
    )
    .unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    compiler
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result)
        .unwrap()
}

/// The diagnostic a build reports, if the source breaks a contract: its
/// phase, message and source span.
fn build_error(source: &str) -> Option<(&'static str, String, crate::span::Span)> {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    crate::build::compile_source(source, &config, crate::build::ServiceOptions::default())
        .err()
        .map(|error| {
            let diagnostic = error.diagnostic.expect("a source diagnostic");
            (error.phase, diagnostic.message, diagnostic.span)
        })
}

#[test]
fn a_pure_arithmetic_body_is_discardable_once_its_arguments_are_primitive() {
    let twice = summary(
        "pure int twice(int x){return x+x;}print(twice(3));",
        "twice",
    );
    assert!(twice.discardable(), "{twice:?}");
    assert!(!twice.observable_effect());
    // The addition converts a raw argument: the caller must prove it.
    assert_eq!(twice.effects.assumed_primitive, ParameterSet::single(0));
    assert!(twice.declared_pure);
    // Integer arithmetic yields an int whatever its operands held.
    assert_eq!(twice.result_primitive, Some(ParameterSet::EMPTY));
}

#[test]
fn empty_and_reading_bodies_have_no_effect_or_obligation() {
    // motionlil's development-only `warning`/`invariant` shape.
    let [warning, reads] = summaries(
        "void warningImpl(bool check,string message,string? code=null){}\
         int limit=4;bool inRange(int value){int copy=limit;return true;}\
         func(bool,string,string?)->void warning=warningImpl;\
         warning(inRange(1),\"x\",null);",
        &["warningImpl", "inRange"],
    )
    .try_into()
    .unwrap();
    assert!(
        warning.discardable() && !warning.effects.obligated(),
        "{warning:?}"
    );
    // Reading module state is allowed; its temporal dead zone is a throw.
    assert!(reads.effects.reads.contains(Regions::CELLS));
    assert!(!reads.observable_effect());
    assert!(reads.effects.may_throw && !reads.discardable());
}

#[test]
fn loops_without_a_counted_bound_and_recursion_may_diverge() {
    let [spin, fact, even, forever] = summaries(
        "int spin(int x){int i=x;while(i!=0){i=i+2;}return i;}\
         int fact(int n){if(n<=1){return 1;}return n*fact(n-1);}\
         bool even(int n){if(n==0){return true;}return odd(n-1);}\
         bool odd(int n){if(n==0){return false;}return even(n-1);}\
         void forever(){while(true){}}\
         print(spin(2));print(fact(3));print(even(2));forever();",
        &["spin", "fact", "even", "forever"],
    )
    .try_into()
    .unwrap();
    for divergent in [&spin, &fact, &even, &forever] {
        assert!(divergent.effects.may_diverge, "{divergent:?}");
        assert!(!divergent.discardable());
        // Divergence is not a contract violation.
        assert!(!divergent.observable_effect());
    }
}

#[test]
fn counted_loops_terminate_and_their_bounds_are_obligations() {
    let names = [
        "below",
        "text",
        "each",
        "constant",
        "inclusive",
        "grows",
        "skips",
        "strides",
        "strideConstant",
        "down",
    ];
    let found = summaries(
        "int below(int n){int t=0;for(int i=0;i<n;i=i+1){t=t+i;}return t;}\
         int text(string s){int t=0;int i=0;while(i<s.length){t=t+1;i+=1;}return t;}\
         int each(int[] xs){int t=0;for(int x of xs){t=t+1;}return t;}\
         int constant(){int t=0;for(int i=0;i<=10;i=i+1){t=t+i;}return t;}\
         int inclusive(int n){int t=0;for(int i=0;i<=n;i=i+1){t=t+i;}return t;}\
         int grows(int[] xs){for(int x of xs){xs.push(x);}return 0;}\
         int skips(int n){int t=0;int i=0;while(i<n){if(t>3){continue;}i=i+1;t=t+1;}return t;}\
         int strides(int n){int t=0;for(int i=0;i<n;i=i+2){t=t+1;}return t;}\
         int strideConstant(){int t=0;for(int i=0;i<100;i=i+2){t=t+1;}return t;}\
         int down(int n){int t=0;for(int i=n;i>0;i=i-1){t=t+1;}return t;}\
         print(below(3));print(text(\"ab\"));print(each([1]));print(constant());\
         print(inclusive(2));print(grows([1]));print(skips(2));print(strides(4));\
         print(strideConstant());print(down(3));",
        &names,
    );
    let by = |name: &str| &found[names.iter().position(|found| *found == name).unwrap()];
    for terminating in [
        "below",
        "text",
        "each",
        "constant",
        "strideConstant",
        "down",
    ] {
        assert!(
            !by(terminating).effects.may_diverge,
            "{terminating}: {:?}",
            by(terminating)
        );
    }
    // A parameter bound must be an int32 at the call.
    assert_eq!(by("below").effects.assumed_int32, ParameterSet::single(0));
    assert!(by("below").discardable());
    // `<=` a raw bound can wrap the counter; a growing array moves its bound;
    // a `continue` can skip the step; a stride can wrap past a raw bound.
    for divergent in ["inclusive", "grows", "skips", "strides"] {
        assert!(by(divergent).effects.may_diverge, "{divergent}");
    }
}

#[test]
fn throws_escape_unless_a_try_catches_them() {
    let [throws, caught] = summaries(
        "int throws(int x){if(x>0){throw \"no\";}return x;}\
         int caught(int x){try{if(x>0){throw \"no\";}}catch(auto e){return 0;}return x;}\
         print(throws(0));print(caught(1));",
        &["throws", "caught"],
    )
    .try_into()
    .unwrap();
    assert!(throws.effects.may_throw && !throws.discardable());
    assert!(!caught.effects.may_throw, "{caught:?}");
}

#[test]
fn host_values_and_callees_run_user_code() {
    let [dynamic, host, trusted, callback] = summaries(
        "extern JsValue source;extern int host(int x);pure extern int trusted(int x);\
         int dynamic(JsValue value){JsValue item=value[\"x\"];return 0;}\
         int viaHost(int x){return host(x);}\
         int viaTrusted(int x){return trusted(x);}\
         int apply(func(int)->int f){return f(1);}\
         print(dynamic(source));print(viaHost(1));print(viaTrusted(1));\
         print(apply((int x)=>x));",
        &["dynamic", "viaHost", "viaTrusted", "apply"],
    )
    .try_into()
    .unwrap();
    // A getter on a dynamic value runs user code.
    assert!(dynamic.effects.runs_user_code && dynamic.observable_effect());
    assert!(host.effects.runs_user_code && host.observable_effect());
    // A trusted `pure extern` has no observable effect, but its host code
    // has no termination proof: its discarded call stays (D3.6).
    assert!(!trusted.observable_effect(), "{trusted:?}");
    assert!(trusted.effects.may_diverge && !trusted.discardable());
    // A parameter can hold any callable.
    assert!(callback.effects.runs_user_code);
}

#[test]
fn writes_through_parameters_land_where_the_arguments_point() {
    let [fill, local, forward, global] = summaries(
        "int[] shared=[];\
         void fill(int[] out){out.push(1);}\
         int local(){int[] mine=[];fill(mine);return mine.length;}\
         void forward(int[] out){fill(out);}\
         void global(){fill(shared);}\
         print(local());forward(shared);global();",
        &["fill", "local", "forward", "global"],
    )
    .try_into()
    .unwrap();
    assert_eq!(fill.effects.mutated, ParameterSet::single(0));
    assert!(fill.observable_effect());
    // Mutating the caller's own allocation is invisible outside it.
    assert!(local.effects.mutated.is_empty());
    assert!(local.effects.writes.observable().is_empty(), "{local:?}");
    assert!(!local.observable_effect());
    assert_eq!(forward.effects.mutated, ParameterSet::single(0));
    assert!(global.effects.writes.contains(Regions::FIELDS));
}

#[test]
fn suspending_bodies_are_not_summarized() {
    let arena = bumpalo::Bump::new();
    let program = program(
        &arena,
        "async int later(){return 1;}later().then((int v)=>print(v));",
    );
    let effects = program.effects(Seal::Module);
    assert!(matches!(
        effects.unit(body(&program, "later")),
        Some(Fact::Unknown(_))
    ));
}

#[test]
fn the_pure_contract_reports_observable_effects_with_a_span() {
    for (source, name) in [
        ("pure int bad(int value){print(value);return value;}print(bad(1));", "bad"),
        (
            "pure void mutate(int[] values){values.push(1);}int[] values=[];mutate(values);",
            "mutate",
        ),
        (
            "int counter=0;pure int bump(){counter=counter+1;return counter;}print(bump());",
            "bump",
        ),
        (
            "extern int host(int x);pure int callsHost(int x){return host(x);}print(callsHost(1));",
            "callsHost",
        ),
        (
            "extern JsValue source;pure bool loose(JsValue v){return v==1;}print(loose(source));",
            "loose",
        ),
        (
            "int total=0;void add(int x){total=total+x;}pure int indirect(){add(1);return 1;}print(indirect());",
            "indirect",
        ),
    ] {
        let (phase, message, span) =
            build_error(source).unwrap_or_else(|| panic!("no diagnostic: {source}"));
        assert_eq!(phase, "check", "{message}");
        assert_eq!(
            message,
            format!(
                "function `{name}` is declared `pure` but may perform an observable side effect"
            )
        );
        // The span names the declaration.
        let start = source.find(&format!(" {name}(")).unwrap() + 1;
        assert_eq!(&source[span.start..span.end], name, "{span:?}");
        assert_eq!(span.start, start);
    }
    for source in [
        "pure int square(int value){int copy=value;return copy*copy;}print(square(5));",
        "pure int localWork(int value){int[] work=[];work.push(value);work[0]+=1;return work[0];}print(localWork(4));",
        "int limit=3;pure int readsGlobal(){return limit+1;}print(readsGlobal());",
        "pure int throws(int x){if(x>0){throw \"no\";}return x;}print(throws(0));",
        "pure extern int trusted(int x);pure int callsTrusted(int x){return trusted(x);}",
        "pure func()->float factory(float value){return ()=>value;}print(factory(1.5)());",
        "struct Stats{int total;int count;}pure int sum(Stats s){return s.total+s.count;}print(sum(Stats{1,2}));",
        "pure bool spins(int n){int i=n;while(i!=0){i=i+2;}return true;}print(spins(0));",
        "pure int classify(string label){if(label.startsWith(\"a\")){return 1;}return label.length;}print(classify(\"ab\"));",
    ] {
        assert_eq!(build_error(source), None, "{source}");
    }
}

#[test]
fn discarded_effect_free_calls_leave_the_output() {
    let javascript = compile(
        "pure int twice(int x){return x+x;}\
         void warningImpl(bool check,string message){}\
         func(bool,string)->void warning=warningImpl;\
         int below(int n){int t=0;for(int i=0;i<n;i=i+1){t=t+i;}return t;}\
         export void run(){twice(3);warningImpl(false,\"unseen\");below(4);print(\"ran\");}",
        true,
    );
    assert!(!javascript.contains("unseen"), "{javascript}");
    assert!(
        !javascript.contains("<4") && !javascript.contains("(4)"),
        "{javascript}"
    );
    assert!(javascript.contains("\"ran\""), "{javascript}");
}

#[test]
fn discarded_calls_with_effects_or_without_proofs_stay() {
    let javascript = compile(
        "int spin(int x){int i=x;while(i!=0){i=i+2;}return i;}\
         int fact(int n){if(n<=1){return 1;}return n*fact(n-1);}\
         int boom(int x){if(x>0){throw \"boom\";}return x;}\
         pure extern int trusted(int x);\
         extern int input();\
         pure int twice(int x){return x+x;}\
         bool loose(JsValue v){return v==1;}\
         export void run(JsValue v){spin(1);fact(7);boom(1);trusted(5);twice(input());loose(v);}",
        true,
    );
    // D3.6: an unbounded loop and recursion keep their calls.
    assert!(javascript.contains("(1)"), "spin: {javascript}");
    assert!(javascript.contains("(7)"), "fact: {javascript}");
    // A call that may throw stays.
    assert!(javascript.contains("\"boom\""), "{javascript}");
    // A trusted `pure extern` has no termination proof.
    assert!(javascript.contains("trusted(5)"), "{javascript}");
    // A raw host argument may run a conversion hook in the body.
    assert!(javascript.contains("input()"), "{javascript}");
    // A dynamic comparison may run `valueOf`.
    assert!(javascript.contains("==1"), "{javascript}");
}

#[test]
fn an_exported_function_survives_its_discarded_internal_calls() {
    let javascript = compile(
        "export int shown(int x){return x+1;}export void run(){shown(2);print(\"ran\");}",
        true,
    );
    assert!(javascript.contains("shown"), "{javascript}");
    assert!(!javascript.contains("(2)"), "{javascript}");
}

#[test]
fn a_call_whose_callee_read_must_stay_is_kept_whole() {
    // The binding is initialized by a statement after `run` is created, so
    // its read inside `run` keeps its temporal dead zone: dropping only the
    // call would leave a bare read of `warning`.
    let javascript = compile(
        "void warningImpl(bool check,string message){}\
         func(bool,string)->void warning=warningImpl;\
         export void run(){warning(true,\"through\");warningImpl(true,\"direct\");print(\"ran\");}",
        true,
    );
    assert!(javascript.contains("\"through\""), "{javascript}");
    assert!(!javascript.contains("\"direct\""), "{javascript}");
}
