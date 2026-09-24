//! The artifact loading contract governs ordinary caller reflection. Generated
//! stack text is excluded separately; it cannot waive Function.caller identity.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_contract::{JavaScriptExecution, JavaScriptWorld};
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use serde_json::{json, Value as Json};
use std::process::Command;

const COERCING: &str =
    "extern int opaque();auto helper=(int value)=>value+1;print(helper(opaque()));";

fn policy(module: bool) -> ResolvedPolicy {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap()
}
fn request() -> HelperRequest {
    HelperRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
        local_facts: LocalFactsRequest {
            work_quota: 100_000,
            result_bytes: 100_000,
        },
    }
}
fn compiled(
    source: &str,
    module: bool,
    inspect: impl FnOnce(&mut Compilation<'_>, CandidateId, CellId, &ResolvedPolicy),
) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let helper = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "helper")
            .unwrap(),
    )
    .unwrap();
    let mut compiler = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 10_000_000,
                optional_work: 10_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 4 },
    )
    .unwrap();
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
    let policy = policy(module);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    inspect(&mut compiler, direct, helper, &policy);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
fn render(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
) -> String {
    let retained = compiler
        .with_javascript_output(candidate, policy, |output| {
            let artifact = output.render(&Plan::new(Style::Source))?;
            output.with_artifact(artifact, |view| {
                assert_eq!(
                    view.execution,
                    policy.javascript_contract().unwrap().execution
                );
            })?;
            output.retain_artifact(artifact)
        })
        .unwrap()
        .unwrap();
    compiler
        .with_artifact(retained, |view| {
            assert_eq!(
                view.execution,
                policy.javascript_contract().unwrap().execution
            );
        })
        .unwrap();
    compiler.take_artifact(retained).unwrap()
}
fn execute(javascript: &str, module: bool, coercing: bool) -> Json {
    // Function construction deliberately creates a sloppy *host* callback in
    // both runs. Only the generated caller's actual execution mode differs.
    let host = if coercing {
        r#"globalThis.opaque=()=>({valueOf:Function("globalThis.events.push(['caller-visible',arguments.callee.caller!==null]);return 4")});"#
    } else {
        ""
    };
    let script=format!("globalThis.events=[];console.log=value=>events.push(['value',value]);{host}\n{javascript}\nprocess.stdout.write(JSON.stringify(events));");
    let mut command = Command::new("node");
    if module {
        command.arg("--input-type=module");
    }
    let result = command
        .args(["-e", &script])
        .output()
        .expect("Node is required for frame observation tests");
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn artifact_execution_is_independent_of_world() {
    let mut module = *policy(true).javascript_contract().unwrap();
    let mut script = *policy(false).javascript_contract().unwrap();
    assert_eq!(module.execution, JavaScriptExecution::Module);
    assert_eq!(script.execution, JavaScriptExecution::Script);
    module.world = JavaScriptWorld::ClosedApplication;
    script.world = JavaScriptWorld::ReusableLibrary;
    assert!(module.execution.guarantees_strict_execution());
    assert!(!script.execution.guarantees_strict_execution());
}

#[test]
fn script_keeps_a_coercing_private_helper_frame_and_rejects_its_inline_candidate() {
    compiled(COERCING, false, |compiler, direct, helper, policy| {
        let original = render(compiler, direct, policy);
        assert_eq!(
            execute(&original, false, true),
            json!([["caller-visible", true], ["value", 5]])
        );
        let result = compiler
            .inline_helper_javascript(direct, helper, request(), policy, WorkDomain::Optional)
            .unwrap();
        assert!(
            matches!(
                result.outcome,
                HelperOutcome::Unknown(HelperUnknownReason::ObservableFrame)
            ),
            "{result:?}"
        );
        let retained = render(compiler, direct, policy);
        assert_eq!(
            retained, original,
            "rejection must preserve the valid incumbent"
        );
        assert_eq!(
            execute(&retained, false, true),
            json!([["caller-visible", true], ["value", 5]])
        );
    });
}

#[test]
fn module_loading_hides_coercion_callers_for_both_shared_and_inlined_recipes() {
    compiled(COERCING, true, |compiler, direct, helper, policy| {
        let result = compiler
            .inline_helper_javascript(direct, helper, request(), policy, WorkDomain::Optional)
            .unwrap();
        let HelperOutcome::Published(inlined) = result.outcome else {
            panic!("module helper: {result:?}")
        };
        for candidate in [direct, inlined] {
            let javascript = render(compiler, candidate, policy);
            assert_eq!(
                execute(&javascript, true, true),
                json!([["caller-visible", false], ["value", 5]])
            );
        }
    });
}

#[test]
fn script_still_admits_a_helper_with_proved_primitive_inputs() {
    compiled(
        "auto helper=(int value)=>value+1;print(helper(4));",
        false,
        |compiler, direct, helper, policy| {
            let result = compiler
                .inline_helper_javascript(direct, helper, request(), policy, WorkDomain::Optional)
                .unwrap();
            let HelperOutcome::Published(inlined) = result.outcome else {
                panic!("pure script helper: {result:?}")
            };
            for candidate in [direct, inlined] {
                assert_eq!(
                    execute(&render(compiler, candidate, policy), false, false),
                    json!([["value", 5]])
                );
            }
        },
    );
}

#[test]
fn a_script_keeps_the_frame_of_a_coercing_forwarding_function() {
    // `+value` runs `valueOf` inside `helper` in a sloppy script, where the
    // hook sees that frame as its caller: the call stays. A module's strict
    // frames hide their callers, so there the call becomes `+opaque()`.
    let source = "extern JsValue opaque();number helper(JsValue value){return JS.number(value);}print(helper(opaque()));";
    compiled(source, false, |compiler, direct, _, policy| {
        let javascript = render(compiler, direct, policy);
        assert!(javascript.contains("helper("), "{javascript}");
        assert_eq!(
            execute(&javascript, false, true),
            json!([["caller-visible", true], ["value", 4]])
        );
    });
    compiled(source, true, |compiler, direct, _, policy| {
        let javascript = render(compiler, direct, policy);
        assert!(javascript.contains("+opaque()"), "{javascript}");
        assert_eq!(
            execute(&javascript, true, true),
            json!([["caller-visible", false], ["value", 4]])
        );
    });
}
