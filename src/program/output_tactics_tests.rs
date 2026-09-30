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
fn private_properties_share_one_inherited_assignment_and_release_all_heads() {
    let source = include_str!("fixtures/private-fields.lil");
    let resolved = policy(
        "[policy.tactics]\nproperty-mangling='on'\nscalar-replacement='off'\ninlining='off'",
    );
    with_candidate(source, &resolved, |compiler, candidate| {
        let output = OutputTactics::from_policy(&resolved);
        compiler
            .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        compiler.with_javascript_formations_in(candidate, &resolved,
            output.dead_code_elimination, output.target_compaction, output.rules,
            output.families.head(), WorkDomain::Optional, |formations| {
                assert!(!formations.properties_inert());
                for property_mangling in [false, true] {
                    for int32_hints in [false, true] {
                        let mut choices = output.clone();
                        choices.families.property_mangling = property_mangling;
                        choices.families.int32_hints = int32_hints;
                        let javascript = formations.form(choices, |target| {
                            let artifact = target.render(&Plan::new(Style::Global))?;
                            target.take_artifact(artifact)
                        }).unwrap().unwrap();
                        for name in ["accumulatedValue", "offsetValue", "independentValue"] {
                            assert_eq!(javascript.contains(name), !property_mangling, "{javascript}");
                        }
                        assert_eq!(execute(&javascript,
                            "Object.defineProperty(Object.prototype,'a',{configurable:true,set(){throw 'inherited setter';}});",
                            "const a=library.make(2),b=library.make(10);events.push(a(3),b(1),a(-2));"),
                            serde_json::json!([15,21,13]));
                    }
                }
                assert_eq!(formations.other_heads_formed(), 3);
            }).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), before);
    });
}

#[test]
fn private_properties_keep_reflected_shapes_and_trusted_host_keys() {
    let source = r#"
        extern JsValue hostValue;
        class Base{int baseValue;init(){this.baseValue=4;}}
        class Child extends Base{int childValue;init(){super();this.childValue=9;}}
        class HostShape{int hostField;}
        class Thrown{int thrownField;init(){this.thrownField=17;}}
        export Base make(){return new Child();}
        export int readHost(){HostShape value=JS.assume(hostValue);return value.hostField;}
        export void fail(){throw new Thrown();}
    "#;
    let resolved = enabled();
    with_candidate(source, &resolved, |compiler, candidate| {
        let mut choices = OutputTactics::from_policy(&resolved);
        choices.families.property_mangling = true;
        let javascript = emit(compiler, candidate, &resolved, choices);
        assert_eq!(execute(&javascript, "globalThis.hostValue={hostField:23};",
            "const value=library.make();events.push(Object.keys(value),value.baseValue,value.childValue,library.readHost());try{library.fail()}catch(value){events.push(Object.keys(value),value.thrownField)}"),
            serde_json::json!([["baseValue","childValue"],4,9,23,["thrownField"],17]));
    });
}

#[test]
fn private_properties_are_vetoed_before_direct_and_cached_formation() {
    let resolved = policy("[policy.tactics]\nproperty-mangling='off'");
    with_candidate(
        include_str!("fixtures/private-fields.lil"),
        &resolved,
        |compiler, candidate| {
            let output = OutputTactics::from_policy(&resolved);
            let mut mangled = output.clone();
            mangled.families.property_mangling = true;
            let before = compiler.ledger().retained_bytes();
            assert!(matches!(
                compiler.with_javascript_output_choices_in(
                    candidate,
                    &resolved,
                    mangled.clone(),
                    WorkDomain::Optional,
                    |_| ()
                ),
                Err(CandidateError::ForbiddenTactic(TacticId::PropertyMangling))
            ));
            assert!(matches!(
                compiler.with_javascript_formations_in(
                    candidate,
                    &resolved,
                    output.dead_code_elimination,
                    output.target_compaction,
                    output.rules,
                    mangled.families.head(),
                    WorkDomain::Optional,
                    |_| ()
                ),
                Err(CandidateError::ForbiddenTactic(TacticId::PropertyMangling))
            ));
            compiler
                .with_javascript_formations_in(
                    candidate,
                    &resolved,
                    output.dead_code_elimination,
                    output.target_compaction,
                    output.rules,
                    output.families.head(),
                    WorkDomain::Optional,
                    |formations| {
                        assert!(matches!(
                            formations.survey(&mangled),
                            Err(CandidateError::ForbiddenTactic(TacticId::PropertyMangling))
                        ));
                        assert!(matches!(
                            formations.form(mangled, |_| ()),
                            Err(CandidateError::ForbiddenTactic(TacticId::PropertyMangling))
                        ));
                        assert_eq!(formations.other_heads_formed(), 0);
                    },
                )
                .unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), before);
        },
    );
}

#[test]
fn private_properties_keep_requested_keys_and_avoid_reserved_short_names() {
    let source = r#"
        class Base{int retainedCounter=2;}
        class Child extends Base{int verboseOffset=4;}
        class Other{int privateOther=7;}
        export func(int)->int make(){
            Child child=new Child();Base alias=child;Other other=new Other();
            return (int n)=>{alias.retainedCounter+=n;return child.retainedCounter+child.verboseOffset+other.privateOther;};
        }
    "#;
    let resolved = policy("[mangle]\npreserve_properties=['retainedCounter','a','b']\n[policy.tactics]\nproperty-mangling='on'\nscalar-replacement='off'\ninlining='off'");
    with_candidate(source, &resolved, |compiler, candidate| {
        let output = OutputTactics::from_policy(&resolved);
        compiler
            .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        compiler
            .with_javascript_formations_in(
                candidate,
                &resolved,
                output.dead_code_elimination,
                output.target_compaction,
                output.rules,
                output.families.head(),
                WorkDomain::Optional,
                |formations| {
                    for int32_hints in [false, true] {
                        let mut choices = output.clone();
                        choices.families.property_mangling = true;
                        choices.families.int32_hints = int32_hints;
                        let javascript = formations
                            .form(choices, |target| {
                                let artifact = target.render(&Plan::new(Style::Global))?;
                                target.take_artifact(artifact)
                            })
                            .unwrap()
                            .unwrap();
                        assert!(javascript.contains("retainedCounter"), "{javascript}");
                        for removed in ["verboseOffset", "privateOther", "a:", "b:", ".a", ".b"] {
                            assert!(!javascript.contains(removed), "{removed}: {javascript}");
                        }
                        assert_eq!(
                            execute(
                                &javascript,
                                "",
                                "const step=library.make();events.push(step(3),step(-2));"
                            ),
                            serde_json::json!([16, 14])
                        );
                    }
                },
            )
            .unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), before);
        let mut choices = output;
        choices.families.property_mangling = true;
        let direct = emit(compiler, candidate, &resolved, choices);
        assert!(direct.contains("retainedCounter"));
        assert!(!direct.contains("privateOther"));
        assert_eq!(
            execute(&direct, "", "events.push(library.make()(5));"),
            serde_json::json!([18])
        );
    });
}

#[test]
fn private_properties_preserve_implicit_host_crossings_and_published_methods() {
    let source = r#"
        extern JsValue inspect;
        class Returned{int returnedField=11;}
        class Literal{int literalField=13;}
        class Spread{int spreadField=17;}
        class Payload{int payloadField=19;}
        class Argument{int argumentField;}
        class Factory{
            Payload make(){return new Payload();}
            int read(Argument value){return value.argumentField;}
        }
        export constructor Factory;
        export JsValue returned(){return new Returned();}
        export JsValue literal(){return record{item:new Literal()};}
        export void spread(){Spread[] values=[new Spread()];inspect(...values);}
    "#;
    let resolved = enabled();
    with_candidate(source, &resolved, |compiler, candidate| {
        for property_mangling in [false, true] {
            let mut choices = OutputTactics::from_policy(&resolved);
            choices.families.property_mangling = property_mangling;
            let javascript = emit(compiler, candidate, &resolved, choices);
            assert_eq!(execute(&javascript,
                "globalThis.inspect=value=>events.push(Object.keys(value),value.spreadField);",
                "const r=library.returned(),l=library.literal().item,f=new library.Factory(),p=f.make();events.push(Object.keys(r),r.returnedField,Object.keys(l),l.literalField,Object.keys(p),p.payloadField,f.read({argumentField:23}));library.spread();"),
                serde_json::json!([["returnedField"],11,["literalField"],13,["payloadField"],19,23,["spreadField"],17]));
        }
    });
}

#[test]
fn private_properties_preserve_extern_inheritance_and_observed_class_layouts() {
    let source = r#"
        extern class HostBase{int firstHostField;int secondHostField;}
        extern class HostChild extends HostBase{}
        extern HostChild host;
        class Observed{int identityField;init(){this.identityField=7;}}
        export int read(){
            Observed value=new Observed();
            if(value is Observed){return host.firstHostField+host.secondHostField+value.identityField;}
            return 0;
        }
    "#;
    let resolved = enabled();
    with_candidate(source, &resolved, |compiler, candidate| {
        let mut choices = OutputTactics::from_policy(&resolved);
        choices.families.property_mangling = true;
        let javascript = emit(compiler, candidate, &resolved, choices);
        assert_eq!(
            execute(
                &javascript,
                "globalThis.host={firstHostField:2,secondHostField:3};",
                "events.push(library.read());"
            ),
            serde_json::json!([12])
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
    for (version, level, permission, expected_splits) in [
        (2, 0, "auto", 0),
        (2, 13, "auto", 0),
        (2, 16, "auto", 1),
        (2, 16, "off", 0),
        (2, 0, "on", 2),
        (2, 13, "on", 2),
        (3, 16, "auto", 0),
        (3, 16, "off", 0),
        (3, 13, "on", 2),
    ] {
        let resolved = policy(&format!("assume_pristine_builtins=true\n[effort]\nlevel={level}\n[policy]\nversion={version}\n[policy.tactics]\nstring-array-packing='{permission}'"));
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
    for (version, level, permission, encoded) in [
        (2, 0, "auto", false),
        (2, 13, "auto", false),
        (2, 16, "auto", true),
        (2, 0, "on", true),
        (2, 16, "off", false),
        (3, 16, "auto", false),
        (3, 16, "off", false),
        (3, 13, "on", true),
    ] {
        let resolved = policy(&format!(
            "[effort]\nlevel={level}\n[policy]\nversion={version}\n[policy.tactics]\nstartup-reconstruction='{permission}'"
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
