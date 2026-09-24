//! Explicit output alternatives use one immutable policy and the same compiler
//! owner. Permission is checked before formation, independently of cost ranking.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, TacticId,
    WorkDomain, WorkKind,
};
use crate::js::selection::{Plan, Style};
use std::process::Command;

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 100_000_000;
const ALL: OutputTactics = OutputTactics {
    literals: LiteralOutput::Original,
    dead_code_elimination: true,
    target_compaction: true,
    raw_structure: false,
};
const SOURCE: &str = r#"
    string unused="DROP_ONLY_MARKER"+"unused";
    export int compute(int input){
        int state=0;
        auto step=(int amount)=>{state=state+amount;return state;};
        try{int ignored=step(2);return (input&255)+step(3);}
        finally{print(state);}
    }
"#;

fn policy(settings: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig =
        toml::from_str(&format!("[javascript]\nstrip_console=false\n{settings}")).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn enabled() -> ResolvedPolicy {
    policy("[policy.tactics]\ndead-code-elimination='on'\ntarget-compaction='on'\nidentifier-mangling='on'")
}
fn with_candidate(
    source: &str,
    policy: &ResolvedPolicy,
    inspect: impl FnOnce(&mut Compilation<'_>, CandidateId),
) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compiler
        .direct_javascript(source, policy, WorkDomain::Baseline)
        .unwrap();
    inspect(&mut compiler, candidate);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
fn emit(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    choices: OutputTactics,
) -> String {
    compiler
        .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let baseline_work = compiler.ledger().work_used(WorkDomain::Baseline);
    let javascript = compiler
        .with_javascript_output_choices_in(
            candidate,
            policy,
            choices,
            WorkDomain::Optional,
            |output| {
                let artifact = output.render(&Plan::new(Style::Global))?;
                output.take_artifact(artifact)
            },
        )
        .unwrap()
        .unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    assert_eq!(
        compiler.ledger().work_used(WorkDomain::Baseline),
        baseline_work
    );
    javascript
}
fn execute(javascript: &str, setup: &str, observations: &str) -> serde_json::Value {
    let script = format!(
        "const events=[];{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{observations}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(javascript).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for output tactic observations");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn four_output_choices_preserve_closure_exception_and_public_observations() {
    let resolved = enabled();
    let fingerprint = resolved.fingerprint();
    with_candidate(SOURCE, &resolved, |compiler, candidate| {
        let mut artifacts = Vec::new();
        for dead_code_elimination in [false, true] {
            for target_compaction in [false, true] {
                let choices = OutputTactics {
                    literals: LiteralOutput::Original,
                    dead_code_elimination,
                    target_compaction,
                    raw_structure: false,
                };
                let javascript = emit(compiler, candidate, &resolved, choices);
                assert_eq!(
                    execute(
                        &javascript,
                        "console.log=(value)=>events.push(['log',value]);",
                        r#"
                        events.push(['meta',library.compute.name,library.compute.length]);
                        events.push(['normal',library.compute(511)]);
                        const failure={};
                        try{library.compute({valueOf(){events.push('coerce');throw failure;}});}
                        catch(error){events.push(['thrown',error===failure]);}
                        events.push(['again',library.compute(0)]);
                    "#
                    ),
                    serde_json::json!([
                        ["meta", "compute", 1],
                        ["log", 5],
                        ["normal", 260],
                        "coerce",
                        ["log", 2],
                        ["thrown", true],
                        ["log", 5],
                        ["again", 5],
                    ]),
                    "{choices:?}\n{javascript}"
                );
                assert_eq!(
                    javascript.contains("DROP_ONLY_MARKER"),
                    !dead_code_elimination,
                    "selected demand mode must reach formation: {choices:?}\n{javascript}"
                );
                artifacts.push(javascript);
            }
        }
        assert_ne!(
            artifacts[0], artifacts[1],
            "compaction subset must reach the common placement client"
        );
        assert_ne!(
            artifacts[2], artifacts[3],
            "compaction must also apply when DCE is selected"
        );
        let default = compiler
            .with_javascript_output_in(candidate, &resolved, WorkDomain::Optional, |output| {
                let artifact = output.render(&Plan::new(Style::Global))?;
                output.take_artifact(artifact)
            })
            .unwrap()
            .unwrap();
        assert_eq!(default, artifacts[3]);
    });
    assert_eq!(resolved.fingerprint(), fingerprint);
}

#[test]
fn output_choices_preserve_host_lookup_arguments_and_integer_result_coercion() {
    let source =
        "extern int operand(int n);export int multiply(){return Math.imul(operand(1),operand(2));}";
    let resolved = enabled();
    with_candidate(source, &resolved, |compiler, candidate| {
        for dead_code_elimination in [false, true] {
            for target_compaction in [false, true] {
                let javascript = emit(
                    compiler,
                    candidate,
                    &resolved,
                    OutputTactics {
                        literals: LiteralOutput::Original,
                        dead_code_elimination,
                        target_compaction,
                        raw_structure: false,
                    },
                );
                assert_eq!(
                    execute(
                        &javascript,
                        r#"
                    let result=NaN;const failure={};
                    globalThis.operand=n=>{events.push('arg:'+n);return n;};
                    Object.defineProperty(Math,'imul',{configurable:true,get(){events.push('get');return function(a,b){events.push(['call',this===Math,a,b]);return result;};}});
                "#,
                        r#"
                    events.push(['result',library.multiply()]);
                    result={valueOf(){events.push('coerce');throw failure;}};
                    try{library.multiply();}catch(error){events.push(['thrown',error===failure]);}
                "#
                    ),
                    serde_json::json!([
                        "get",
                        "arg:1",
                        "arg:2",
                        ["call", true, 1, 2],
                        ["result", 0],
                        "get",
                        "arg:1",
                        "arg:2",
                        ["call", true, 1, 2],
                        "coerce",
                        ["thrown", true],
                    ]),
                    "{javascript}"
                );
            }
        }
    });
}

#[test]
fn forbidden_output_choice_rejects_before_target_allocation_and_callback() {
    let resolved = enabled();
    with_candidate(SOURCE, &resolved, |compiler, candidate| {
        for (setting, tactic) in [
            ("dead-code-elimination", TacticId::DeadCodeElimination),
            ("target-compaction", TacticId::TargetCompaction),
        ] {
            let off = policy(&format!(
                "[policy.tactics]\n{setting}='off'\nidentifier-mangling='on'"
            ));
            let before = compiler.ledger().clone();
            let mut entered = false;
            let result = compiler.with_javascript_output_choices_in(
                candidate,
                &off,
                ALL,
                WorkDomain::Optional,
                |_| entered = true,
            );
            assert!(
                matches!(result, Err(CandidateError::ForbiddenTactic(found)) if found == tactic)
            );
            assert!(!entered);
            assert_eq!(compiler.ledger().retained_bytes(), before.retained_bytes());
            assert_eq!(
                compiler.ledger().peak_retained_bytes(),
                before.peak_retained_bytes()
            );
            assert_eq!(
                compiler.ledger().work_by_kind(WorkKind::Render),
                before.work_by_kind(WorkKind::Render)
            );
            let choices = OutputTactics::from_policy(&off);
            let javascript = emit(compiler, candidate, &off, choices);
            assert!(!javascript.is_empty());
        }
    });
}

#[test]
fn output_permission_checks_do_not_fabricate_runtime_evidence_for_rank_policy() {
    let resolved = policy("priority='balanced'\n[policy.constraints]\nmax_startup_work=0\n[policy.tactics]\ndead-code-elimination='on'\ntarget-compaction='on'\nidentifier-mangling='on'");
    with_candidate(
        "export int read(){return 3;}",
        &resolved,
        |compiler, candidate| {
            // Rendering is allowed. The artifact frontier must separately reject
            // unknown cost evidence if this priority/constraint needs an estimate.
            for choices in [
                ALL,
                OutputTactics {
                    literals: LiteralOutput::Original,
                    dead_code_elimination: false,
                    target_compaction: false,
                    raw_structure: false,
                },
            ] {
                assert_eq!(
                    execute(
                        &emit(compiler, candidate, &resolved, choices),
                        "",
                        "events.push(library.read());"
                    ),
                    serde_json::json!([3])
                );
            }
        },
    );
}
