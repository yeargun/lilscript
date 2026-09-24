//! Execute physical alternatives through the compilation owner. Expected
//! traces describe language/host observations, independently of target shape.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Plan, Style};
use std::process::Command;

struct Case {
    name: &'static str,
    source: &'static str,
    host: &'static str,
    expected: &'static str,
}
macro_rules! case {
    ($name:literal, $expected:literal) => {
        Case {
            name: $name,
            source: include_str!(concat!("fixtures/record/", $name, ".lil")),
            host: include_str!(concat!("fixtures/record/", $name, ".setup.js")),
            expected: $expected,
        }
    };
}

fn policy() -> ResolvedPolicy {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}

fn scalar_request() -> ScalarRequest {
    ScalarRequest {
        max_work: 100_000,
        scratch_bytes: 100_000,
        output_bytes: 100_000,
    }
}

fn state(program: &Program<'_>) -> CellId {
    program
        .cells
        .iter()
        .enumerate()
        .find_map(|(index, cell)| {
            let id = CellId::from_index(index).unwrap();
            (cell.name == "state" && program.unit(cell.owner).unwrap().operations.iter().any(
            |operation| matches!(operation.kind, OperationKind::Initialize(cell) if cell == id),
        )).then_some(id)
        })
        .unwrap()
}

fn execute(javascript: &str, host: &str) -> String {
    let script = format!("{host}\n{javascript}");
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for representation integration tests");
    assert!(
        output.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn check_case(case: Case, scalar_expected: bool) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let state = state(&program);
    let revisions: Vec<_> = program.units.iter().map(FrozenUnit::revision).collect();
    let mut compilation = Compilation::new(ledger(), CheckpointLimit { max_live: 4 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy();
    let direct = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let scalar = compilation
        .scalar_javascript(
            direct,
            state,
            scalar_request(),
            &policy,
            WorkDomain::Optional,
        )
        .unwrap();
    let mut candidates = vec![("direct", direct)];
    match scalar.outcome {
        ScalarOutcome::Published(candidate) if scalar_expected => {
            candidates.push(("scalar", candidate))
        }
        ScalarOutcome::Unknown(_) if !scalar_expected => {}
        other => panic!("{}: unexpected scalar outcome {other:?}", case.name),
    }
    for (representation, candidate) in candidates {
        let view = compilation.view(candidate.semantic_id()).unwrap();
        for (index, &revision) in revisions.iter().enumerate() {
            assert_eq!(
                view.unit_revision(UnitId::from_index(index).unwrap()),
                Some(revision)
            );
        }
        compilation
            .with_javascript_output(candidate, &policy, |output| {
                for style in [Style::Source, Style::Scoped, Style::Global] {
                    let artifact = output.render(&Plan::new(style)).unwrap();
                    let javascript = output.take_artifact(artifact).unwrap();
                    assert_eq!(
                        execute(&javascript, case.host),
                        case.expected,
                        "{} {representation} {style:?}",
                        case.name
                    );
                    if representation == "scalar" {
                        assert!(
                            !javascript.contains("__proto__:"),
                            "private record allocation survived: {javascript}"
                        );
                    }
                }
            })
            .unwrap();
        compilation.discard(candidate.semantic_id()).unwrap();
    }
    compilation.discard(source).unwrap();
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn private_record_alternatives_preserve_shared_callbacks_reentry_and_loop_activations() {
    for case in [
        case!("sharing", "3\n3\n9\n10\n4\n"),
        case!("reentry-finally", "1\n7\n7\n17\n1\n7\n99\n17\n"),
        case!("body-activation", "1\n2\n2\n"),
        Case {
            name: "nested prepared-call lookup reentry",
            source: "extern void keep(func()->int read,func()->void change);extern int combine(int a,int b);int run(){Record<int> state=record{x:1};keep(()=>state.x??0,()=>{state.x=(state.x??0)+1;});return combine(state.x??0,combine(state.x??0,3))+(state.x??0);}print(run());",
            host: "let read,change;globalThis.keep=(r,c)=>{read=r;change=c;};Object.defineProperty(globalThis,'combine',{get(){console.log('get',read());change();return (a,b)=>{console.log('call',a,b,read());return a+b;};}});",
            expected: "get 1\nget 2\ncall 3 3 3\ncall 2 6 3\n11\n",
        },
    ] {
        check_case(case, true);
    }
}

#[test]
fn scalar_record_slots_preserve_absence_and_initializer_observations() {
    for case in [
        case!("absence-special-keys", "true\n7\n10\n"),
        case!("initializer-effects", "1\n2\n99\n"),
        Case {
            name: "unobserved throwing field",
            source: "extern int value(int n);int run(){Record<int> state=record{used:value(1),unused:value(2)};return state.used??0;}try{print(run());}catch(auto error){print(99);}",
            host: "globalThis.value=n=>{console.log(n);if(n===2)throw 23;return n;};",
            expected: "1\n2\n99\n",
        },
        Case {
            name: "zero observed slots",
            source: "extern int value(int n);int run(){Record<int> state=record{unused:value(3)};return 7;}print(run());",
            host: "globalThis.value=n=>{console.log(n);return n;};",
            expected: "3\n7\n",
        },
    ] {
        check_case(case, true);
    }
}

#[test]
fn rejected_record_families_leave_the_direct_candidate_executable() {
    for case in [
        case!("early-capture", "99\n"),
        case!("dynamic-key", "1\n"),
        case!("host-escape", "9\n"),
        case!("alias", "9\n"),
        case!("reassignment", "9\n"),
    ] {
        check_case(case, false);
    }
}

#[test]
fn complete_public_record_artifacts_are_named_and_measured_for_each_codec() {
    let source = r#"
        export func()->int make(int seed){
            Record<int> state=record{count:seed};
            return ()=>{state.count=(state.count??0)+1;return state.count??0;};
        }
    "#;
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let state = state(&program);
    let mut compilation = Compilation::new(ledger(), CheckpointLimit { max_live: 3 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy();
    let direct = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let scalar = compilation
        .scalar_javascript(
            direct,
            state,
            scalar_request(),
            &policy,
            WorkDomain::Optional,
        )
        .unwrap();
    let ScalarOutcome::Published(scalar) = scalar.outcome else {
        panic!("missing complete scalar candidate")
    };
    for (representation, candidate) in [("direct", direct), ("scalar", scalar)] {
        compilation.with_javascript_output(candidate, &policy, |output| {
            // Supplied-plan admission and measurement integration. This is not
            // production structural-search certification; that remains gate 007.
            let bindings = output.source_candidates().unwrap().to_vec();
            let mut plans = vec![Plan::new(Style::Global), Plan::new(Style::Scoped), Plan::new(Style::Source)];
            'plans: for binding in bindings {
                for style in [Style::Global, Style::Scoped, Style::Source] {
                    let mut plan = Plan::new(style);
                    plan.source_names.push(binding);
                    plans.push(plan);
                    if plans.len() == 8 { break 'plans; }
                }
            }
            assert_eq!(plans.len(), 8, "fixture must exercise eight explicit naming plans");
            // Harness-owned plan/oracle vectors are outside compiler ownership.
            let mut artifacts = Vec::new();
            let mut supplied_bytes = 0usize;
            let mut winners: [Option<usize>; 3] = [None; 3];
            for plan in &plans {
                let artifact = output.render(plan).unwrap();
                let mut sizes = [0usize; 3];
                for (index, objective) in [Objective::Raw, Objective::Gzip, Objective::Brotli].into_iter().enumerate() {
                    sizes[index] = output.measure(artifact, objective).unwrap();
                }
                let javascript = output.take_artifact(artifact).unwrap();
                supplied_bytes += javascript.len();
                assert!(supplied_bytes <= 100_000, "original total candidate text bound");
                let candidate = artifacts.len();
                for index in 0..3 {
                    if winners[index].is_none_or(|winner| {
                        let (code, previous): &(String, [usize;3]) = &artifacts[winner];
                        (sizes[index], javascript.len(), javascript.as_str()) < (previous[index], code.len(), code.as_str())
                    }) { winners[index] = Some(candidate); }
                }
                artifacts.push((javascript, sizes));
            }
            assert_eq!(artifacts.len(), 8);
            let mut measurements = Vec::new();
            for (index, objective) in [Objective::Raw, Objective::Gzip, Objective::Brotli].into_iter().enumerate() {
                let (javascript, sizes) = &artifacts[winners[index].unwrap()];
                let script = format!(
                    "const library=await import('data:text/javascript,'+encodeURIComponent({}));const a=library.make(2),b=library.make(9);console.log(library.make.name,library.make.length);console.log(a(),b(),a());console.log(a===b,JSON.stringify(a.name),a.length);",
                    serde_json::to_string(javascript).unwrap(),
                );
                assert_eq!(execute(&script, ""), "make 1\n3 10 4\nfalse \"\" 0\n");
                let exact = crate::compression::measure(javascript.as_bytes(), objective).unwrap();
                assert_eq!(sizes[index], exact);
                assert_eq!(exact, artifacts.iter().map(|(_, sizes)| sizes[index]).min().unwrap());
                measurements.push(exact);
            }
            eprintln!("record-public-sizes {representation} raw/gzip9/brotli11={measurements:?}");
        }).unwrap();
    }
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn independent_record_families_compose_without_copying_shared_closure_state() {
    let source = r#"
        extern void keep(func()->int read,func()->void change);
        extern void mutate(int which);
        func()->int make(int n){
            Record<int> left=record{x:n};Record<int> right=record{x:n+10};
            auto read=()=>(left.x??0)+(right.x??0);
            keep(read,()=>{left.x=(left.x??0)+2;right.x=(right.x??0)+3;});
            return read;
        }
        auto a=make(1);auto b=make(2);print(a());mutate(0);print(a());print(b());
    "#;
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let cell = |name| {
        CellId::from_index(
            program
                .cells
                .iter()
                .position(|cell| cell.name == name)
                .unwrap(),
        )
        .unwrap()
    };
    let left = cell("left");
    let right = cell("right");
    let mut compilation = Compilation::new(ledger(), CheckpointLimit { max_live: 6 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy();
    let direct = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let add = |compilation: &mut Compilation<'_>, base, cell| {
        let result = compilation
            .scalar_javascript(base, cell, scalar_request(), &policy, WorkDomain::Optional)
            .unwrap();
        let ScalarOutcome::Published(candidate) = result.outcome else {
            panic!("family not published")
        };
        candidate
    };
    let left_only = add(&mut compilation, direct, left);
    let left_then_right = add(&mut compilation, left_only, right);
    let right_only = add(&mut compilation, direct, right);
    let right_then_left = add(&mut compilation, right_only, left);
    let host = "const changes=[];globalThis.keep=(r,c)=>changes.push(c);globalThis.mutate=i=>changes[i]();";
    let mut artifacts = Vec::new();
    for candidate in [
        direct,
        left_only,
        right_only,
        left_then_right,
        right_then_left,
    ] {
        artifacts.push(
            compilation
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&Plan::new(Style::Scoped)).unwrap();
                    let javascript = output.take_artifact(artifact).unwrap();
                    assert_eq!(execute(&javascript, host), "12\n17\n14\n");
                    javascript
                })
                .unwrap(),
        );
    }
    // Stable semantic root order, rather than discovery order, owns bindings.
    assert_eq!(artifacts[3], artifacts[4]);
    assert_eq!(compilation.finish().retained_bytes(), 0);
}
