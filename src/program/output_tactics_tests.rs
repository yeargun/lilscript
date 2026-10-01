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

#[test]
fn default_transport_metadata_obeys_the_current_formation_permission() {
    let arena = bumpalo::Bump::new();
    let source = "extern void keep(JsValue value);void report(int tag,int? value=null){keep(tag);keep(value);}for(int i=0;i<2;i+=1){report(i);report(i,7);}";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    for permission in ["on", "off"] {
        let program = from_checked_source(&syntax, &checked).unwrap();
        let (program, receipt) = super::rules::optimize(
            program,
            super::rules::RuleRequest {
                fold: true,
                dead_code: true,
                inline: false,
                scalar: false, native: false,
                pristine_builtins: false,
                seal: super::call_graph::Seal::Module,
            },
        )
        .unwrap();
        assert!(receipt.default_arguments_omitted > 0 && receipt.native_defaults > 0);
        let policy = policy(&format!("[policy.tactics]\nconstant-folding='{permission}'\ninlining='off'\ntarget-compaction='on'"));
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
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let javascript = compiler
            .with_javascript_output(candidate, &policy, |output| {
                let artifact = output.render(&Plan::new(Style::Global))?;
                output.take_artifact(artifact)
            })
            .unwrap()
            .unwrap();
        assert_eq!(
            javascript.contains("===void 0"),
            permission == "off",
            "{javascript}"
        );
        assert_eq!(
            execute(
                &javascript,
                "globalThis.keep=value=>events.push(value);",
                ""
            ),
            serde_json::json!([0, null, 0, 7, 1, null, 1, 7])
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    }
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
fn private_properties_reuse_sibling_slots_without_aliasing_descendant_fields() {
    let source = r#"
        class Base{int sharedValue=2;}
        class Left extends Base{int leftValue=3;}
        class Right extends Base{int rightValue=5;}
        class Leaf extends Left{int leafValue=7;}
        class Other{int otherValue=11;}
        export int compute(int n){
            Left l=new Left();Right r=new Right();Leaf f=new Leaf();Other o=new Other();
            Base alias=f;l.leftValue+=n;r.rightValue+=2*n;f.leftValue+=3*n;
            return l.sharedValue+l.leftValue+r.sharedValue+r.rightValue+
                alias.sharedValue+f.leftValue+f.leafValue+o.otherValue;
        }
    "#;
    let resolved = policy("[policy.tactics]\nproperty-mangling='on'\nscalar-replacement='off'\ninlining='off'");
    with_candidate(source, &resolved, |compiler, candidate| {
        for mangled in [false, true] {
            let mut choices = OutputTactics::from_policy(&resolved);
            choices.families.property_mangling = mangled;
            let javascript = emit(compiler, candidate, &resolved, choices);
            for field in ["sharedValue", "leftValue", "rightValue", "leafValue", "otherValue"] {
                assert_eq!(javascript.contains(field), !mangled, "{javascript}");
            }
            if mangled {
                // One shared slot and two descendant slots suffice for every
                // instance. Sibling-only fields must not consume a fourth key.
                assert!(javascript.contains(".b"), "{javascript}");
                assert!(javascript.contains(".c"), "{javascript}");
                assert!(!javascript.contains(".d"), "{javascript}");
            }
            assert_eq!(execute(&javascript,
                "Object.defineProperty(Object.prototype,'a',{configurable:true,set(){throw 'inherited setter';}});",
                "events.push(library.compute(4),library.compute(-2));"),
                serde_json::json!([59,23]));
        }
    });
}

#[test]
fn private_properties_observed_identity_requires_pristine_assignments() {
    let source = r#"
        class Box{int hiddenAmount;init(int n){this.hiddenAmount=n;}}
        export int compute(int n){Box b=new Box(n);if(b is Box){b.hiddenAmount+=1;return b.hiddenAmount;}return -1;}
    "#;
    for pristine in [false, true] {
        let resolved = policy(&format!("assume_pristine_builtins={pristine}\n[policy.tactics]\nproperty-mangling='on'\nscalar-replacement='off'\ninlining='off'"));
        with_candidate(source, &resolved, |compiler, candidate| {
            let mut choices = OutputTactics::from_policy(&resolved);
            choices.families.property_mangling = true;
            let javascript = emit(compiler, candidate, &resolved, choices);
            assert_eq!(javascript.contains("hiddenAmount"), !pristine, "{javascript}");
            let setup = if pristine { "" } else {
                "Object.defineProperty(Object.prototype,'a',{configurable:true,set(){throw 'inherited setter';}});"
            };
            assert_eq!(execute(&javascript, setup, "events.push(library.compute(19),library.compute(-1));"),
                serde_json::json!([20,0]));
        });
    }
}

#[test]
fn private_properties_keep_observed_proto_setter_even_when_pristine() {
    let source = r#"
        class Box{int __proto__;int hiddenAmount;init(int n){this.__proto__=n;this.hiddenAmount=n;}}
        export int compute(int n){Box b=new Box(n);if(b is Box){return b.hiddenAmount;}return -1;}
    "#;
    let resolved = policy("assume_pristine_builtins=true\n[policy.tactics]\nproperty-mangling='on'");
    with_candidate(source, &resolved, |compiler, candidate| {
        let mut choices = OutputTactics::from_policy(&resolved);
        choices.families.property_mangling = true;
        let javascript = emit(compiler, candidate, &resolved, choices);
        assert!(javascript.contains("__proto__") && javascript.contains("hiddenAmount"), "{javascript}");
        assert_eq!(execute(&javascript, "", "events.push(library.compute(19));"), serde_json::json!([19]));
    });
}

#[test]
fn private_properties_generic_storage_and_erased_payloads_keep_their_boundaries() {
    let source = r#"
        class Box<T>{T hiddenPayload;init(T value){this.hiddenPayload=value;}}
        class Payload{int publicAmount=23;}
        class Holder<T>{T retainedValue;init(T value){this.retainedValue=value;}JsValue expose(){return this.retainedValue;}}
        export func(int)->int make(int n){Box<int> b=new Box<int>(n);return (int step)=>{b.hiddenPayload+=step;return b.hiddenPayload;};}
        export JsValue expose(){Holder<Payload> h=new Holder<Payload>(new Payload());return h.expose();}
    "#;
    let resolved = policy("[policy.tactics]\nproperty-mangling='on'\nscalar-replacement='off'\ninlining='off'");
    with_candidate(source, &resolved, |compiler, candidate| {
        for mangled in [false, true] {
            let mut choices = OutputTactics::from_policy(&resolved);
            choices.families.property_mangling = mangled;
            let javascript = emit(compiler, candidate, &resolved, choices);
            assert_eq!(javascript.contains("hiddenPayload"), !mangled, "{javascript}");
            assert_eq!(execute(&javascript, "",
                "const a=library.make(10),b=library.make(1),p=library.expose();events.push(a(2),b(3),a(-1),Object.keys(p),p['public'+'Amount'],JSON.stringify(p));"),
                serde_json::json!([12,4,11,["publicAmount"],23,"{\"publicAmount\":23}"]));
        }
    });
}

#[test]
fn private_properties_published_generic_storage_retains_keys() {
    let source = r#"
        class Box<T>{T publicPayload;init(T value){this.publicPayload=value;}}
        export Box<int> make(int n){return new Box<int>(n);}
    "#;
    let resolved = enabled();
    with_candidate(source, &resolved, |compiler, candidate| {
        let mut choices = OutputTactics::from_policy(&resolved);
        choices.families.property_mangling = true;
        let javascript = emit(compiler, candidate, &resolved, choices);
        assert_eq!(execute(&javascript, "", "const b=library.make(31);events.push(Object.keys(b),JSON.stringify(b),b['publicPayload']);"),
            serde_json::json!([["publicPayload"],"{\"publicPayload\":31}",31]));
    });
}

#[test]
fn g2_adapter_fixtures_observe_anonymous_names_under_the_explicit_contract() {
    for (source, config, host, expected) in [
        (
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-aggregate.lil"),
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-aggregate.toml"),
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-aggregate.host.js"),
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-aggregate.out"),
        ),
        (
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-unused_receiver.lil"),
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-unused_receiver.toml"),
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-unused_receiver.host.js"),
            include_str!("../../tests/cases/regressions/irjs-suppresses_adapter_name_in_variable_and_aggregate_initializers-unused_receiver.out"),
        ),
    ] {
        let config: crate::config::ProjectConfig = toml::from_str(config).unwrap();
        let resolved = config.resolve_policy(CompilationRequest::JavaScript { preserve_root_exports: true }).unwrap();
        with_candidate(source, &resolved, |compiler, candidate| {
            let javascript = emit(compiler, candidate, &resolved, OutputTactics::from_policy(&resolved));
            let script = format!("{host}\nawait import('data:text/javascript,'+encodeURIComponent({}));", serde_json::to_string(&javascript).unwrap());
            let result = Command::new("node").args(["--input-type=module", "-e", &script]).output().unwrap();
            assert!(result.status.success(), "{}\n{javascript}", String::from_utf8_lossy(&result.stderr));
            assert_eq!(String::from_utf8(result.stdout).unwrap(), expected, "{javascript}");
        });
    }
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

#[test]
fn g3_mixed_spelling_sites_are_independent_stable_and_obey_vetoes() {
    use crate::representation::{AltId,ChoiceFamily,ChoiceMap};
    let source=r#"
        extern void observe(int value);
        export int first(bool flag,int n){if(flag){return n+1;}else{return n+2;}}
        export int second(bool flag,int n){if(flag){return n+3;}else{return n+4;}}
        export void run(bool flag){if(flag){observe(7);}if(!flag){observe(9);}}
    "#;
    let resolved=policy("[policy.tactics]\ninlining='off'\nconstant-folding='off'\nstatement-spellings='on'");
    with_candidate(source,&resolved,|compiler,candidate|{
        let mut output=OutputTactics::from_policy(&resolved);
        output.families=crate::js::OutputFamilies::NONE;
        compiler.with_javascript_formations_in(candidate,&resolved,output.dead_code_elimination,output.target_compaction,output.rules,output.families.head(),WorkDomain::Optional,|formations|{
            let sites=formations.survey(&output).unwrap();
            let returns:Vec<_>=sites.iter().filter(|site|site.key.family==ChoiceFamily::ConditionalReturns).collect();
            assert_eq!(returns.len(),2,"{sites:?}");
            output.choices=ChoiceMap::SEEDS.with(returns[0].key,AltId(1)).with(returns[1].key,AltId(0));
            let survey=formations.survey(&output).unwrap();
            for site in &returns {
                assert!(survey.iter().any(|other|other.key==site.key),"site identity changed");
            }
            let javascript=formations.form(output.clone(),|target|{
                let artifact=target.render(&Plan::new(Style::Global))?;target.take_artifact(artifact)
            }).unwrap().unwrap();
            assert!(javascript.contains('?') && javascript.contains("if("),"{javascript}");
            assert_eq!(execute(&javascript,"globalThis.observe=x=>events.push(x);","events.push(library.first(true,5),library.first(false,5),library.second(true,5),library.second(false,5));library.run(true);library.run(false);"),serde_json::json!([6,7,8,9,7,9]));
            let disabled=policy("[policy.tactics]\nstatement-spellings='off'");
            assert!(output.choices.check_target_policy(&disabled).is_err());
        }).unwrap();
    });
}

#[test]
fn g3_receiver_aliases_preserve_method_getters_and_derived_initialization() {
    use crate::representation::{AltId,ChoiceFamily,ChoiceMap};
    let source=r#"
        class Base{int amount;init(int n){this.amount=n;}int total(){return this.amount+this.amount+this.amount+this.amount;}}
        class Child extends Base{int extra;init(int n){super(n);this.extra=this.amount+this.amount+this.amount+this.amount;}int sum(){return this.amount+this.amount+this.extra+this.extra;}}
        export constructor Base;
        export constructor Child;
    "#;
    let resolved=policy("[policy.tactics]\nreceiver-aliases='on'\ninlining='off'\nconstant-folding='off'");
    with_candidate(source,&resolved,|compiler,candidate|{
        let mut output=OutputTactics::from_policy(&resolved);
        compiler.with_javascript_formations_in(candidate,&resolved,output.dead_code_elimination,output.target_compaction,output.rules,output.families.head(),WorkDomain::Optional,|formations|{
            let sites=formations.survey(&output).unwrap();
            let aliases:Vec<_>=sites.iter().filter(|site|site.key.family==ChoiceFamily::ReceiverAlias).collect();
            assert!(aliases.len()>=2,"{sites:?}");
            output.choices=ChoiceMap::SEEDS.with_all(aliases.iter().map(|site|(site.key,AltId(1))));
            let javascript=formations.form(output.clone(),|target|{let artifact=target.render(&Plan::new(Style::Global))?;target.take_artifact(artifact)}).unwrap().unwrap();
            assert!(javascript.contains("=this"),"{javascript}");
            assert_eq!(execute(&javascript,"","let reads=0;const b=new library.Base(3),c=new library.Child(2);Object.defineProperty(b,'amount',{get(){return ++reads;}});events.push(b.total(),reads,c.sum(),c.extra,b.total(),reads);"),serde_json::json!([10,4,20,8,26,8]));
            let disabled=policy("[policy.tactics]\nreceiver-aliases='off'");
            assert!(output.choices.check_target_policy(&disabled).is_err());
        }).unwrap();
    });
}

#[test]
fn g3_literal_locality_preserves_effect_barriers_and_explicit_permissions() {
    use crate::representation::{AltId,ChoiceFamily,ChoiceMap};
    let source=r#"
        extern void observe(string value);
        export void run(){
            string z="z-last";string a="a-first";string m="m-middle";
            observe(z);observe(a);observe(m);
            string y="y-later";string b="b-later";observe(y);observe(b);
        }
    "#;
    let resolved=policy("[policy.tactics]\ndeclaration-order='on'\nconstant-folding='off'\ninlining='off'\nstring-pooling='off'");
    with_candidate(source,&resolved,|compiler,candidate|{
        let mut output=OutputTactics::from_policy(&resolved);
        output.families=crate::js::OutputFamilies::NONE;
        compiler.with_javascript_formations_in(candidate,&resolved,output.dead_code_elimination,output.target_compaction,output.rules,output.families.head(),WorkDomain::Optional,|formations|{
            let sites=formations.survey(&output).unwrap();
            let order:Vec<_>=sites.iter().filter(|site|site.key.family==ChoiceFamily::DeclarationOrder).collect();
            assert!(!order.is_empty(),"{sites:?}");
            output.choices=ChoiceMap::SEEDS.with_all(order.iter().map(|site|(site.key,AltId(1))));
            let javascript=formations.form(output.clone(),|target|{let artifact=target.render(&Plan::new(Style::Global))?;target.take_artifact(artifact)}).unwrap().unwrap();
            assert!(javascript.find("a-first").unwrap()<javascript.find("z-last").unwrap(),"{javascript}");
            assert_eq!(execute(&javascript,"globalThis.observe=x=>events.push(x);","library.run();"),serde_json::json!(["z-last","a-first","m-middle","y-later","b-later"]));
            let disabled=policy("[policy.tactics]\ndeclaration-order='off'");
            assert!(output.choices.check_target_policy(&disabled).is_err());
        }).unwrap();
    });
}

#[test]
fn g3_modern_spellings_respect_editions_value_calls_and_lazy_writes() {
    use crate::representation::{AltId,ChoiceFamily,ChoiceMap};
    let source=r#"
        extern bool next();
        export JsValue read(JsValue obj){if(JS.strictEqual(obj,null)||JS.isUndefined(obj)){return JS.undefined();}return obj["item"];}
        export JsValue call(JsValue obj){JsValue callback;if(JS.strictEqual(obj,null)||JS.isUndefined(obj)){callback=JS.undefined();}else{callback=obj["method"];}return callback();}
        export bool ensure(bool flag){if(!flag){flag=next();}return flag;}
    "#;
    for edition in ["es2019","es2020","es2021"] {
        let resolved=policy(&format!("ecmascript='{edition}'\n[policy.tactics]\nstatement-spellings='on'\ninlining='off'\nconstant-folding='off'"));
        with_candidate(source,&resolved,|compiler,candidate|{
            let mut output=OutputTactics::from_policy(&resolved);
            output.families=crate::js::OutputFamilies::NONE;
            output.families.statements.conditional_returns=true;
            output.families.statements.conditional_values=true;
            compiler.with_javascript_formations_in(candidate,&resolved,output.dead_code_elimination,output.target_compaction,output.rules,output.families.head(),WorkDomain::Optional,|formations|{
                let sites=formations.survey(&output).unwrap();
                let optional:Vec<_>=sites.iter().filter(|site|site.key.family==ChoiceFamily::OptionalChain).collect();
                let assignments:Vec<_>=sites.iter().filter(|site|site.key.family==ChoiceFamily::LogicalAssignment).collect();
                assert_eq!(!optional.is_empty(),edition!="es2019","{edition}: {sites:?}");
                if edition!="es2019" { assert!(optional.len()>=2,"both returned and assigned conditionals need stable sites: {sites:?}"); }
                assert_eq!(!assignments.is_empty(),edition=="es2021","{edition}: {sites:?}");
                output.choices=ChoiceMap::SEEDS.with_all(optional.iter().chain(assignments.iter()).map(|site|(site.key,AltId(1))));
                let javascript=formations.form(output.clone(),|target|{let artifact=target.render(&Plan::new(Style::Global))?;target.take_artifact(artifact)}).unwrap().unwrap();
                assert_eq!(javascript.contains("?."),edition!="es2019","{javascript}");
                assert_eq!(javascript.contains("||="),edition=="es2021","{javascript}");
                assert_eq!(execute(&javascript,"globalThis.next=()=>{events.push('next');return true;};",
                    "let reads=0;const obj={get item(){return ++reads;},method:function(){return this===undefined;}};events.push(library.read(null)===undefined,library.read(undefined)===undefined,library.read(obj),reads,library.call(obj),library.ensure(true),library.ensure(false));"),
                    serde_json::json!(["next",true,true,1,1,true,true,true]));
            }).unwrap();
        });
    }
}

#[test]
fn g3_function_locality_keeps_captures_and_self_recursion() {
    use crate::representation::{AltId, ChoiceFamily, ChoiceMap};
    let source=r#"
        extern void observe(string value);
        export void run(){
            string suffix="!";
            func(int)->string zebra=(int n)=>{if(n==0){return "zebra"+suffix;}return zebra(n-1);};
            func(int)->string alpha=(int n)=>{if(n==0){return "alpha"+suffix;}return alpha(n-1);};
            observe(zebra(0));observe(alpha(0));observe(zebra(1));observe(alpha(1));
        }
    "#;
    let resolved=policy("[policy.tactics]\ndeclaration-order='on'\nconstant-folding='off'\ninlining='off'\nstring-pooling='off'");
    with_candidate(source,&resolved,|compiler,candidate|{
        let mut output=OutputTactics::from_policy(&resolved);
        output.families=crate::js::OutputFamilies::NONE;
        compiler.with_javascript_formations_in(candidate,&resolved,output.dead_code_elimination,output.target_compaction,output.rules,output.families.head(),WorkDomain::Optional,|formations|{
            let sites=formations.survey(&output).unwrap();
            let order:Vec<_>=sites.iter().filter(|site|site.key.family==ChoiceFamily::DeclarationOrder).collect();
            assert!(!order.is_empty(),"{sites:?}");
            output.choices=ChoiceMap::SEEDS.with_all(order.iter().map(|site|(site.key,AltId(1))));
            let javascript=formations.form(output.clone(),|target|{let artifact=target.render(&Plan::new(Style::Global))?;target.take_artifact(artifact)}).unwrap().unwrap();
            assert!(javascript.find("alpha").unwrap()<javascript.find("zebra").unwrap(),"{javascript}");
            assert_eq!(execute(&javascript,"globalThis.observe=x=>events.push(x);","library.run();"),serde_json::json!(["zebra!","alpha!","zebra!","alpha!"]));
        }).unwrap();
    });
}

#[test]
fn g3_loose_host_null_guards_do_not_become_optional_members() {
    let resolved=policy("ecmascript='es2021'\n[policy.tactics]\nstatement-spellings='on'\ninlining='off'");
    with_candidate("export JsValue loose(JsValue obj){if(obj==null){return JS.undefined();}return obj[\"item\"];}",&resolved,|compiler,candidate|{
        let output=OutputTactics::from_policy(&resolved);
        compiler.with_javascript_formations_in(candidate,&resolved,output.dead_code_elimination,output.target_compaction,output.rules,output.families.head(),WorkDomain::Optional,|formations|{
            let sites=formations.survey(&output).unwrap();
            assert!(!sites.iter().any(|site|site.key.family==crate::representation::ChoiceFamily::OptionalChain),"loose null includes HTMLDDA: {sites:?}");
        }).unwrap();
    });
}
