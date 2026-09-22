//! Execute physical string choices through the public compilation owner.
//! Exact knowledge, target representation and final codec scoring stay separate.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use std::process::Command;

fn policy(preserve: bool, target: &str) -> ResolvedPolicy {
    let configuration = format!("[javascript]\nstrip_console=false\necmascript='{target}'\n[optimization]\ndead_code_elimination={}\n", !preserve);
    let config: crate::config::ProjectConfig = toml::from_str(&configuration).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn compiler<'src>() -> Compilation<'src> {
    let mut compiler = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 20 },
    )
    .unwrap();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 32,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    compiler
}

fn request() -> StringRequest {
    StringRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
        local_facts: LocalFactsRequest {
            work_quota: 100_000,
            result_bytes: 100_000,
        },
    }
}

fn definitions(program: &Program<'_>) -> Vec<ValueRef> {
    program
        .units
        .iter()
        .enumerate()
        .flat_map(|(unit, data)| {
            data.data().operations.iter().filter_map(move |operation| {
                (matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                    && operation.result.is_some_and(|value| {
                        matches!(
                            program.types[data.data().values[value.index()].ty.index()],
                            Type::String
                        )
                    }))
                .then(|| ValueRef {
                    unit: UnitId::from_index(unit).unwrap(),
                    value: operation.result.unwrap(),
                })
            })
        })
        .collect()
}

fn select(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    values: &[ValueRef],
    choice: StringChoice,
    policy: &ResolvedPolicy,
) -> CandidateId {
    let publication = compiler
        .represent_string_javascript(
            base,
            values,
            choice,
            request(),
            policy,
            WorkDomain::Optional,
        )
        .unwrap();
    match publication.outcome {
        StringOutcome::Published(candidate) => candidate,
        other => panic!("string representation was not proved: {other:?}"),
    }
}

fn render(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> String {
    compiler
        .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |output| {
            let artifact = output.render(&Plan::new(style))?;
            output.take_artifact(artifact)
        })
        .unwrap()
        .unwrap()
}

fn execute(javascript: &str, host: &str, expected: &str) {
    let script = format!("{host}\n{javascript}");
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        expected,
        "{script}"
    );
}

fn named_cell(program: &Program<'_>, name: &str) -> CellId {
    let mut matches = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let (index, _) = matches
        .next()
        .unwrap_or_else(|| panic!("missing fixture cell {name}"));
    assert!(matches.next().is_none(), "ambiguous fixture cell {name}");
    CellId::from_index(index).unwrap()
}

#[test]
fn string_data_choices_compose_with_captured_record_and_inline_helper_activations() {
    let source = r#"
extern int argument();
extern string observe(int value,string label);
export func()->string make(int seed){
    Record<int> state=record{count:seed,dead:0};
    auto makeLabel=(int unused)=>"counter/"+"ready";
    return ()=>{
        state.dead=9;
        state.count=(state.count??0)+1;
        return observe(state.count??0,makeLabel(argument()));
    };
}
auto a=make(3);auto b=make(10);print(a());print(b());print(a());
"#;
    let host = "function argument(){console.log('argument');return 99}function observe(n,s){return s+':'+n}";
    let expected =
        "argument\ncounter/ready:4\nargument\ncounter/ready:11\nargument\ncounter/ready:5\n";
    for target in ["es2018", "es2022"] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let values = definitions(&program);
        assert_eq!(values.len(), 1);
        let state = named_cell(&program, "state");
        let helper = named_cell(&program, "makeLabel");
        let revisions = program
            .units
            .iter()
            .map(FrozenUnit::revision)
            .collect::<Vec<_>>();
        let mut compiler = compiler();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let selected_policy = policy(false, target);
        let direct = compiler
            .direct_javascript(source, &selected_policy, WorkDomain::Baseline)
            .unwrap();
        let scalar = match compiler
            .scalar_javascript(
                direct,
                state,
                ScalarRequest {
                    max_work: 1_000_000,
                    scratch_bytes: 1_000_000,
                    output_bytes: 1_000_000,
                },
                &selected_policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            ScalarOutcome::Published(candidate) => candidate,
            other => panic!("{other:?}"),
        };
        let mut data_last = None;
        for (has_scalar, base) in [(false, direct), (true, scalar)] {
            let inline = match compiler
                .inline_helper_javascript(
                    base,
                    helper,
                    HelperRequest {
                        max_work: 1_000_000,
                        scratch_bytes: 1_000_000,
                        output_bytes: 1_000_000,
                        local_facts: request().local_facts,
                    },
                    &selected_policy,
                    WorkDomain::Optional,
                )
                .unwrap()
                .outcome
            {
                HelperOutcome::Published(candidate) => candidate,
                other => panic!("{other:?}"),
            };
            for (has_inline, base) in [(false, base), (true, inline)] {
                let literal = select(
                    &mut compiler,
                    base,
                    &values,
                    StringChoice::LiteralAtDefinition,
                    &selected_policy,
                );
                let shared = select(
                    &mut compiler,
                    base,
                    &values,
                    StringChoice::SharedLiteral {
                        activation: values[0].unit,
                    },
                    &selected_policy,
                );
                if has_scalar && has_inline {
                    data_last = Some(shared);
                }
                for candidate in [base, literal, shared] {
                    for preserve in [false, true] {
                        let policy = policy(preserve, target);
                        for style in [Style::Source, Style::Scoped, Style::Global] {
                            let javascript = render(&mut compiler, candidate, &policy, style);
                            execute(&javascript, host, expected);
                        }
                    }
                }
            }
        }
        // Reversing publication order must retain every compatible choice.
        // Each immutable sibling still refers to the same semantic units.
        let data_first = select(
            &mut compiler,
            direct,
            &values,
            StringChoice::SharedLiteral {
                activation: values[0].unit,
            },
            &selected_policy,
        );
        let data_then_helper = match compiler
            .inline_helper_javascript(
                data_first,
                helper,
                HelperRequest {
                    max_work: 1_000_000,
                    scratch_bytes: 1_000_000,
                    output_bytes: 1_000_000,
                    local_facts: request().local_facts,
                },
                &selected_policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            HelperOutcome::Published(candidate) => candidate,
            other => panic!("{other:?}"),
        };
        let data_then_helper_then_record = match compiler
            .scalar_javascript(
                data_then_helper,
                state,
                ScalarRequest {
                    max_work: 1_000_000,
                    scratch_bytes: 1_000_000,
                    output_bytes: 1_000_000,
                },
                &selected_policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            ScalarOutcome::Published(candidate) => candidate,
            other => panic!("{other:?}"),
        };
        for style in [Style::Source, Style::Scoped, Style::Global] {
            let forward = render(&mut compiler, data_last.unwrap(), &selected_policy, style);
            let reverse = render(
                &mut compiler,
                data_then_helper_then_record,
                &selected_policy,
                style,
            );
            assert_eq!(
                forward, reverse,
                "publication order lost or changed a compatible recipe"
            );
        }
        compiler
            .with_semantic(source, |program, _, _| {
                assert_eq!(
                    program
                        .units
                        .iter()
                        .map(FrozenUnit::revision)
                        .collect::<Vec<_>>(),
                    revisions
                );
            })
            .unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    }
}

#[test]
fn equal_string_definitions_share_lossless_code_units_and_preserve_intervening_host_calls() {
    let source = r#"extern void observe(string value);extern void marker();
export void run(){observe("\uD83D"+"\uDE00");marker();observe("\uD83D"+"\uDE00");}
run();run();"#;
    let host = "function observe(s){console.log(s.length+':'+s.charCodeAt(0)+':'+s.charCodeAt(1))}function marker(){console.log('marker')}";
    let expected = "2:55357:56832\nmarker\n2:55357:56832\n2:55357:56832\nmarker\n2:55357:56832\n";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let values = definitions(&program);
    assert_eq!(values.len(), 2);
    let mut compiler = compiler();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let selected_policy = policy(false, "es2022");
    let direct = compiler
        .direct_javascript(source, &selected_policy, WorkDomain::Baseline)
        .unwrap();
    let literal = select(
        &mut compiler,
        direct,
        &values,
        StringChoice::LiteralAtDefinition,
        &selected_policy,
    );
    let shared = select(
        &mut compiler,
        direct,
        &values,
        StringChoice::SharedLiteral {
            activation: values[0].unit,
        },
        &selected_policy,
    );
    for candidate in [direct, literal, shared] {
        for preserve in [false, true] {
            for style in [Style::Source, Style::Scoped, Style::Global] {
                let javascript =
                    render(&mut compiler, candidate, &policy(preserve, "es2022"), style);
                execute(&javascript, host, expected);
            }
        }
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn exact_complete_artifacts_keep_computed_literal_and_shared_choices_available_to_each_codec() {
    use crate::config::CompressionCostModel::{Brotli, Gzip, Raw};
    let source = r#"export string pattern(){return "[/\\\\]"+"*((?:-\\d)?\\d*)";}"#;
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let values = definitions(&program);
    assert_eq!(values.len(), 1);
    let mut compiler = compiler();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy(false, "es2022");
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let literal = select(
        &mut compiler,
        direct,
        &values,
        StringChoice::LiteralAtDefinition,
        &policy,
    );
    let shared = select(
        &mut compiler,
        direct,
        &values,
        StringChoice::SharedLiteral {
            activation: values[0].unit,
        },
        &policy,
    );
    let mut artifacts = Vec::new();
    for (label, candidate) in [
        ("computed", direct),
        ("literal", literal),
        ("shared", shared),
    ] {
        for style in [Style::Source, Style::Scoped, Style::Global] {
            let javascript = render(&mut compiler, candidate, &policy, style);
            let validation = format!(
                "const api=await import('data:text/javascript,'+encodeURIComponent({}));console.log(Object.keys(api).join(','));console.log(api.pattern.name+':'+api.pattern.length);console.log(JSON.stringify(Array.from({{length:api.pattern().length}},(_,i)=>api.pattern().charCodeAt(i))));",
                serde_json::to_string(&javascript).unwrap()
            );
            execute(&validation, "", "pattern\npattern:0\n[91,47,92,92,93,42,40,40,63,58,45,92,100,41,63,92,100,42,41]\n");
            let sizes = [Raw, Gzip, Brotli]
                .map(|codec| crate::compression::measure(javascript.as_bytes(), codec).unwrap());
            eprintln!("string artifact {label}/{style:?}: {sizes:?} {javascript}");
            artifacts.push((label, style, javascript, sizes));
        }
    }
    // This tests real formation and canonical encoders. The handwritten design
    // probe's crossover is not a required golden: names and packaging can
    // legitimately reverse its ordering. Full structural search remains work.
    for codec in 0..3 {
        let winner = artifacts
            .iter()
            .min_by_key(|artifact| artifact.3[codec])
            .unwrap();
        eprintln!(
            "codec {codec} winner {}/{:?}: {}",
            winner.0, winner.1, winner.3[codec]
        );
    }
    assert_ne!(artifacts[0].2, artifacts[3].2);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn literal_production_keeps_lazy_argument_throws_and_finally_after_inlining() {
    let source = r#"extern int argument(int n);
string helper(int first,int second,int third){return "left"+"right";}
if(false){print(helper(argument(0),argument(0),argument(0)));}
try{print(helper(argument(1),argument(2),argument(3)));}catch(auto error){print(90);}finally{print(99);}
print(helper(argument(4),argument(5),argument(6)));"#;
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let values = definitions(&program);
    let helper = named_cell(&program, "helper");
    let mut compiler = compiler();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let selected_policy = policy(false, "es2022");
    let direct = compiler
        .direct_javascript(source, &selected_policy, WorkDomain::Baseline)
        .unwrap();
    let inline = match compiler
        .inline_helper_javascript(
            direct,
            helper,
            HelperRequest {
                max_work: 1_000_000,
                scratch_bytes: 1_000_000,
                output_bytes: 1_000_000,
                local_facts: request().local_facts,
            },
            &selected_policy,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome
    {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("{other:?}"),
    };
    for base in [direct, inline] {
        let literal = select(
            &mut compiler,
            base,
            &values,
            StringChoice::LiteralAtDefinition,
            &selected_policy,
        );
        let shared = select(
            &mut compiler,
            base,
            &values,
            StringChoice::SharedLiteral {
                activation: values[0].unit,
            },
            &selected_policy,
        );
        for candidate in [base, literal, shared] {
            for preserve in [false, true] {
                for style in [Style::Source, Style::Scoped, Style::Global] {
                    let javascript =
                        render(&mut compiler, candidate, &policy(preserve, "es2022"), style);
                    execute(&javascript, "function argument(n){console.log(n);if(n===2)throw new RangeError('explicit');return n}", "1\n2\n90\n99\n4\n5\n6\nleftright\n");
                }
            }
        }
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
