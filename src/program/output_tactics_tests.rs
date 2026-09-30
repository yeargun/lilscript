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
    rules: crate::js::TargetRules::SEMANTIC,
    families: crate::js::OutputFamilies::NONE,
    choices: crate::js::ChoiceMap::SEEDS,
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
        toml::from_str(&format!("[javascript]\n{settings}")).unwrap();
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
                    rules: crate::js::TargetRules::SEMANTIC,
                    families: crate::js::OutputFamilies::NONE,
                    choices: crate::js::ChoiceMap::SEEDS,
                };
                let javascript = emit(compiler, candidate, &resolved, choices.clone());
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

/// Every output choice keeps a typed builtin's arguments in order and its
/// result, an int32 by R10 (the original `Math.imul`).
#[test]
fn output_choices_preserve_builtin_arguments_and_integer_results() {
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
                        rules: crate::js::TargetRules::SEMANTIC,
                        families: crate::js::OutputFamilies::NONE,
                        choices: crate::js::ChoiceMap::SEEDS,
                    },
                );
                assert_eq!(
                    execute(
                        &javascript,
                        r#"globalThis.operand=n=>{events.push('arg:'+n);return n*65537;};"#,
                        r#"events.push(['result',library.multiply()]);"#
                    ),
                    serde_json::json!(["arg:1", "arg:2", ["result", 262146]]),
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
                    rules: crate::js::TargetRules::NONE,
                    families: crate::js::OutputFamilies::NONE,
                    choices: crate::js::ChoiceMap::SEEDS,
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

/// The `int32_hints` family prints the `|0` the compiler printed before R1:
/// after an `int` field read. The same program otherwise.
#[test]
fn int32_hints_restore_the_previous_normalizations() {
    let source = "class Counter { int count; init(int c) { this.count = c; } }\nexport int read(Counter c) { return c.count; }\n";
    let resolved = enabled();
    with_candidate(source, &resolved, |compiler, candidate| {
        let mut render = |hints: bool| {
            emit(
                compiler,
                candidate,
                &resolved,
                OutputTactics {
                    literals: LiteralOutput::Original,
                    dead_code_elimination: true,
                    target_compaction: true,
                    rules: crate::js::TargetRules::SEMANTIC,
                    families: crate::js::OutputFamilies {
                        int32_hints: hints,
                        ..crate::js::OutputFamilies::NONE
                    },
                    choices: crate::js::ChoiceMap::SEEDS,
                },
            )
        };
        let plain = render(false);
        let hinted = render(true);
        assert_eq!(
            hinted.matches("|0").count(),
            plain.matches("|0").count() + 1,
            "{plain}\n{hinted}"
        );
    });
}

#[test]
fn pooling_and_packing_are_independent_and_preserve_fresh_arrays() {
    let source = r#"
        extern void observe(string value);
        export string[] make() {
            observe("a long repeated string");
            observe("a long repeated string");
            observe("a long repeated string");
            return ["aa", "bb", "cc", "dd", "ee", "ff", "gg", "hh"];
        }
    "#;
    let resolved =
        policy("assume_pristine_builtins=true\n[policy.tactics]\nstring-array-packing='on'");
    with_candidate(source, &resolved, |compiler, candidate| {
        for string_pooling in [false, true] {
            for string_array_packing in [false, true] {
                let output = OutputTactics {
                    families: crate::js::OutputFamilies {
                        string_pooling,
                        string_array_packing,
                        ..crate::js::OutputFamilies::NONE
                    },
                    ..OutputTactics::from_policy(&resolved)
                };
                let javascript = emit(compiler, candidate, &resolved, output);
                assert_eq!(
                    javascript.contains(".split("),
                    string_array_packing,
                    "{javascript}"
                );
                assert_eq!(
                    javascript.matches("a long repeated string").count(),
                    if string_pooling { 1 } else { 3 },
                    "{javascript}"
                );
                assert_eq!(execute(&javascript,
                    "let calls=0;globalThis.observe=x=>{if(x!=='a long repeated string')throw Error(x);calls++};",
                    "const a=library.make(),b=library.make();a[0]='changed';events.push(calls,b,a!==b);"),
                    serde_json::json!([6,["aa","bb","cc","dd","ee","ff","gg","hh"],true]));
            }
        }
    });
}

#[test]
fn explicit_output_families_and_rules_cannot_bypass_vetoes() {
    let resolved = enabled();
    with_candidate(SOURCE, &resolved, |compiler, candidate| {
        for tactic in [
            TacticId::StringPooling,
            TacticId::StringArrayPacking,
            TacticId::Inlining,
            TacticId::ConstantFolding,
            TacticId::ScalarReplacement,
        ] {
            let off = policy(&format!("[policy.tactics]\n{}='off'", tactic.spec().name));
            let mut forbidden = OutputTactics::from_policy(&off);
            match tactic {
                TacticId::StringPooling => forbidden.families.string_pooling = true,
                TacticId::StringArrayPacking => forbidden.families.string_array_packing = true,
                TacticId::Inlining => forbidden.rules.inlining = true,
                TacticId::ConstantFolding => forbidden.rules.constant_folding = true,
                TacticId::ScalarReplacement => forbidden.rules.scalar_replacement = true,
                _ => unreachable!(),
            }
            let before = compiler.ledger().retained_bytes();
            let denied = compiler.with_javascript_output_choices_in(
                candidate,
                &off,
                forbidden,
                WorkDomain::Optional,
                |_| panic!("forbidden output reached formation"),
            );
            assert!(
                matches!(denied, Err(CandidateError::ForbiddenTactic(found)) if found == tactic),
                "{tactic:?}: {denied:?}"
            );
            assert_eq!(compiler.ledger().retained_bytes(), before);
        }
    });
}

#[test]
fn fixed_target_rules_respect_folding_and_inlining_switches() {
    for (tactic, source, observation) in [
        (
            "constant-folding",
            "export int go(){return 2+3;}",
            "events.push(library.go());",
        ),
        (
            "inlining",
            "int add(int x){return x+1;}export int go(int x){return add(x);}",
            "events.push(library.go(4));",
        ),
    ] {
        let enabled = policy(&format!("[policy.tactics]\n{tactic}='on'"));
        let disabled = policy(&format!("[policy.tactics]\n{tactic}='off'"));
        with_candidate(source, &enabled, |compiler, candidate| {
            let on = emit(
                compiler,
                candidate,
                &enabled,
                OutputTactics::from_policy(&enabled),
            );
            let off = emit(
                compiler,
                candidate,
                &disabled,
                OutputTactics::from_policy(&disabled),
            );
            assert_ne!(on, off, "{tactic} changed no target work: {on}");
            for javascript in [&on, &off] {
                assert_eq!(execute(javascript, "", observation), serde_json::json!([5]));
            }
        });
    }
}

#[test]
fn packing_permission_distinguishes_startup_from_callable_bodies() {
    let source = r#"
        string[] saved=["aa","bb","cc","dd","ee","ff","gg","hh"];
        export string[] initial(){return saved;}
        export string[] make(){return ["aa","bb","cc","dd","ee","ff","gg","hh"];}
    "#;
    for (level, permission, expected_splits) in [
        (0, "auto", 0),
        (13, "auto", 0),
        (16, "auto", 1),
        (16, "off", 0),
        (0, "on", 2),
        (13, "on", 2),
    ] {
        let resolved = policy(&format!("assume_pristine_builtins=true\n[effort]\nlevel={level}\n[policy.tactics]\nstring-array-packing='{permission}'"));
        with_candidate(source, &resolved, |compiler, candidate| {
            let mut choices = OutputTactics::from_policy(&resolved);
            choices.families.string_array_packing =
                choices.rules.array_packing != crate::js::ArrayPacking::Disabled;
            let javascript = emit(compiler, candidate, &resolved, choices);
            assert_eq!(
                javascript.matches(".split(").count(),
                expected_splits,
                "{level}/{permission}: {javascript}"
            );
            assert_eq!(execute(&javascript,"", "const a=library.make(),b=library.make();a[0]='changed';events.push(library.initial(),b,a!==b,library.initial()===library.initial());"),
                serde_json::json!([["aa","bb","cc","dd","ee","ff","gg","hh"],["aa","bb","cc","dd","ee","ff","gg","hh"],true,true]));
        });
    }
}

#[test]
fn constant_table_decoders_require_startup_permission_in_direct_output() {
    let source = include_str!("../../tests/cases/data_tables.lil");
    let host = include_str!("../../tests/cases/data_tables.host.js");
    let expected = include_str!("../../tests/cases/data_tables.out");
    for (level, permission, encoded) in [
        (0, "auto", false),
        (13, "auto", false),
        (16, "auto", true),
        (0, "on", true),
        (16, "off", false),
    ] {
        let resolved = policy(&format!(
            "[effort]\nlevel={level}\n[policy.tactics]\nstartup-reconstruction='{permission}'"
        ));
        with_candidate(source, &resolved, |compiler, candidate| {
            let javascript = emit(
                compiler,
                candidate,
                &resolved,
                OutputTactics::from_policy(&resolved),
            );
            assert_eq!(
                javascript.contains(".split("),
                encoded,
                "{level}/{permission}: {javascript}"
            );
            let script = format!(
                "{host}\nawait import('data:text/javascript,'+encodeURIComponent({}));",
                serde_json::to_string(&javascript).unwrap()
            );
            let output = Command::new("node")
                .args(["--input-type=module", "-e", &script])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        });
    }
}
