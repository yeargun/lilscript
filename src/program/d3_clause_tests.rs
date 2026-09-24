//! Executable cases for the D3 clauses 002 left without one (D3.6-D3.10).
//! Each clause has a positive case, observed by running the compiler's
//! output, and a refusal case, before any family that could affect it is
//! enabled by default.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn policy(module: bool) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap()
}

fn compilation() -> Compilation<'static> {
    Compilation::new(
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
    .unwrap()
}

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

fn compile(source: &str, module: bool) -> Result<String, CandidateError> {
    let arena = bumpalo::Bump::new();
    let program = program(&arena, source);
    let policy = policy(module);
    let mut compiler = compilation();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compiler.direct_javascript(source, &policy, WorkDomain::Baseline)?;
    let result = compiler
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result);
    assert_eq!(compiler.finish().retained_bytes(), 0);
    result
}

/// Runs a classic script through `vm` (a sloppy global frame) or a module
/// (strict), and waits at most `deadline`. Returns stdout and whether the
/// program was still running when the deadline passed.
fn run_until(javascript: &str, host: &str, module: bool, deadline: Duration) -> (String, bool) {
    let script = if module {
        format!(
            "{host}\nawait import('data:text/javascript,'+encodeURIComponent({}));",
            serde_json::to_string(javascript).unwrap()
        )
    } else {
        format!(
            "{host}\nrequire('vm').runInThisContext({});",
            serde_json::to_string(javascript).unwrap()
        )
    };
    let mut command = Command::new("node");
    if module {
        command.args(["--input-type=module", "-e", &script]);
    } else {
        command.args(["-e", &script]);
    }
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Node is required for D3 observations");
    let started = Instant::now();
    let finished = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if started.elapsed() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = String::new();
    child.stdout.take().unwrap().read_to_string(&mut stdout).unwrap();
    if let Some(status) = finished {
        let mut stderr = String::new();
        child.stderr.take().unwrap().read_to_string(&mut stderr).unwrap();
        assert!(status.success(), "{stderr}\n{javascript}");
    }
    (stdout, finished.is_none())
}

fn run(javascript: &str, host: &str, module: bool) -> String {
    let (stdout, timed_out) = run_until(javascript, host, module, Duration::from_secs(20));
    assert!(!timed_out, "{javascript}");
    stdout
}

#[test]
fn d3_6_divergence_is_preserved_and_never_evaluated_at_compile_time() {
    // A terminating loop terminates with its result.
    let javascript = compile(
        "int spin(int n){int i=0;while(i<n){i++;}return i;}print(spin(100000));",
        false,
    )
    .unwrap();
    assert_eq!(run(&javascript, "", false), "100000\n");
    // A diverging call still diverges, although its result is unused: the
    // statement after it never runs.
    let javascript = compile(
        "void hang(){while(true){}}print(\"before\");hang();print(\"after\");",
        false,
    )
    .unwrap();
    let (stdout, timed_out) = run_until(&javascript, "", false, Duration::from_millis(1500));
    assert!(timed_out, "a diverging program terminated: {stdout}\n{javascript}");
    assert_eq!(stdout, "before\n");
    // Refusal: recursion over constant input is not evaluated while
    // compiling; the result appears only when the program runs.
    let javascript = compile(
        "int fact(int n){if(n<=1){return 1;}return n*fact(n-1);}print(fact(12));",
        false,
    )
    .unwrap();
    assert!(!javascript.contains("479001600"), "{javascript}");
    assert_eq!(run(&javascript, "", false), "479001600\n");
}

#[test]
fn d3_7_initialization_runs_once_in_order_and_early_reads_are_refused() {
    // Module initialization runs each statement once, in order; a function
    // reads a binding after its initializer has run.
    let javascript = compile(
        "int calls=0;int next(){calls++;return calls;}int first=next();int second=next();\
         print(first);print(second);print(calls);",
        false,
    )
    .unwrap();
    assert_eq!(run(&javascript, "", false), "1\n2\n2\n");
    // Refusal: a read that would precede the binding's initialization is
    // refused while checking, not deferred to a runtime error.
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "int read(){return value;}int early=read();int value=4;print(early);",
    )
    .unwrap();
    // The single-source checker says "unknown identifier"; file-based module
    // checking says "cannot read `value` before its declaration". Either way
    // the refusal names the early read inside `read`.
    let error = crate::analyze(&syntax).unwrap_err();
    let text = format!("{error:?}");
    assert!(
        text.contains("before its declaration") || text.contains("unknown identifier `value`"),
        "{text}"
    );
    assert!(text.contains("start: 18"), "{text}");
}

#[test]
fn d3_8_async_settlement_and_generator_interleaving_are_preserved() {
    let source = "async void worker(string name){print(name+\":start\");int got=await Task.resolve(1);print(name+\":end\"+got);}\
                  generator int count(int stop){int i=0;while(i<stop){print(\"yield \"+i);yield i;i++;}}\
                  for(int value of count(2)){print(\"got \"+value);}\
                  worker(\"a\");worker(\"b\");print(\"sync\");";
    let javascript = compile(source, true).unwrap();
    assert_eq!(
        run(&javascript, "", true),
        "yield 0\ngot 0\nyield 1\ngot 1\na:start\nb:start\nsync\na:end1\nb:end1\n"
    );
    // Refusal: native targets refuse suspension rather than reorder it.
    let arena = bumpalo::Bump::new();
    let program = program(&arena, source);
    let mut compiler = compilation();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let native = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    assert!(matches!(
        compiler.with_native_c(source, &native, WorkDomain::Baseline, |_| panic!(
            "suspension reached native output"
        )),
        Err(super::native::NativeError::Unsupported { .. })
    ));
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn d3_9_script_and_module_frames_keep_their_meaning() {
    // A plainly called function sees the global object as `this` in a
    // sloppy script frame and `undefined` in a strict module frame: a closed
    // world does not make a script strict.
    let source = "extern JsValue this;extern void invoke(JsValue callback);extern bool isGlobal(JsValue value);\
                  void probe(){print(isGlobal(this));}invoke(probe);";
    let host = "globalThis.invoke=f=>f();globalThis.isGlobal=v=>v===globalThis;";
    assert_eq!(run(&compile(source, false).unwrap(), host, false), "true\n");
    assert_eq!(run(&compile(source, true).unwrap(), host, true), "false\n");
    // Refusal: a script frame that would need strict mode for a struct
    // adapter while observing `this` is refused, not silently made strict.
    let refused = compile(
        "extern JsValue this;extern void invoke(JsValue callback);struct P{int x;}\
         void probe(P point){print(point.x);invoke(this);}probe(P{1});",
        false,
    );
    assert!(
        matches!(&refused, Err(CandidateError::Unsupported(error)) if error.feature.contains("frame")),
        "{refused:?}"
    );
}

#[test]
fn d3_10_recursion_within_limits_runs_and_inlining_refuses_recursion() {
    // A program within the engine's limits still runs within them.
    let javascript = compile(
        "int depth(int n){if(n==0){return 0;}return 1+depth(n-1);}print(depth(5000));",
        false,
    )
    .unwrap();
    assert_eq!(run(&javascript, "", false), "5000\n");
    // Refusal: inlining a recursive helper could multiply stack depth and
    // output size, so the helper family refuses it.
    let arena = bumpalo::Bump::new();
    let program = program(
        &arena,
        "int helper(int n){if(n<=0){return 0;}return helper(n-1)+1;}print(helper(3));",
    );
    let helper = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "helper")
            .unwrap(),
    )
    .unwrap();
    let mut compiler = compilation();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 4,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    let policy = policy(false);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let outcome = compiler
        .inline_helper_javascript(
            direct,
            helper,
            HelperRequest {
                max_work: 1_000_000,
                scratch_bytes: 1_000_000,
                output_bytes: 1_000_000,
                local_facts: LocalFactsRequest {
                    work_quota: 100_000,
                    result_bytes: 100_000,
                },
            },
            &policy,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome;
    assert!(
        matches!(outcome, HelperOutcome::Unknown(HelperUnknownReason::Recursion)),
        "{outcome:?}"
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
