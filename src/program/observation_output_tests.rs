//! Both literal output modes borrow one prepared semantic target. Runtime
//! observations and artifact provenance qualify the actual retained bytes.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, TacticId, WorkDomain, WorkKind,
};
use crate::js::selection::{Objective, Objectives, Plan, Sizes, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::process::Command;

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const MODES: [LiteralOutput; 2] = [LiteralOutput::Original, LiteralOutput::Observed];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];

#[derive(Clone, Copy)]
struct Case {
    name: &'static str,
    source: &'static str,
    setup: &'static str,
    observations: &'static str,
    expected: &'static str,
}
const WEAK: Case = Case {
    name: "weak-gates-branches-and-throw",
    source: include_str!("fixtures/observation-output/weak.lil"),
    setup: "const failure={};let fail=false;globalThis.event=label=>{events.push(label);if(fail&&label==='truth-effect')throw failure;return 0;};",
    observations: "library.run(true);library.run(false);fail=true;try{library.run(true)}catch(error){events.push(['caught',error===failure]);}",
    expected: r#"["truth-effect","empty-effect","missing-effect","branch-effect","truth-effect","empty-effect","missing-effect","branch-effect","truth-effect",["caught",true]]"#,
};
const EXACT: Case = Case {
    name: "mixed-public-shared-reference",
    source: include_str!("fixtures/observation-output/exact.lil"),
    setup: "globalThis.event=value=>{events.push(value);return 0;};",
    observations: "events.push(['public',library.exposed]);events.push(['return',library.run(true)]);events.push(['return',library.run(false)]);",
    expected: r#"[["public","public-exact"],"mixed-gate","mixed-exact","reference-before","reference-after",["return","return-yes"],"mixed-gate","mixed-exact","reference-before","reference-after",["return","return-no"]]"#,
};
const FORWARD: Case = Case {
    name: "shared-versus-inline-forwarding",
    source: include_str!("fixtures/observation-output/forward.lil"),
    setup: "globalThis.event=value=>{events.push(value);return 0;};",
    observations: "library.run();library.run();",
    expected: r#"["forward-effect","forward-effect"]"#,
};

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn policy(cap: usize, compaction: bool, inlining: bool) -> ResolvedPolicy {
    policy_with_naming(cap, compaction, inlining, true)
}
fn policy_with_naming(
    cap: usize,
    compaction: bool,
    inlining: bool,
    naming: bool,
) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\noptimization_level=15\npriority='size-first'\nstrip_console=false\ncandidate_proposal_limit={cap}\nterminal_codec_probe_limit={}\ncandidate_limit=8\ncandidate_beam_width=2\n[policy.search]\ncodec_schedule='staged'\nrender_batch=8\ndiversity_interval=4\n[policy.tactics]\ndead-code-elimination='on'\ntarget-compaction='{}'\nidentifier-mangling='on'\nnaming-search='{}'\ninlining='{}'\nscalar-replacement='off'\ncall-specialization='off'\nconstant-folding='off'\nstring-pooling='off'",
        cap * 2, if compaction { "on" } else { "off" }, if naming { "on" } else { "off" }, if inlining { "on" } else { "off" },
    )).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn request() -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    };
    SearchRequest {
        objectives: Objectives::All,
        scalar: ScalarRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
        },
        helper: HelperRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 16,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
    }
}
fn with_source<R>(
    case: Case,
    search: bool,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, Option<CellId>) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let helper = program
        .cells()
        .iter()
        .enumerate()
        .find_map(|(index, cell)| {
            (cell.name == "forward").then(|| CellId::from_index(index).unwrap())
        });
    let ledger = if search {
        BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: WORK,
                retained_bytes: MEMORY,
                terminal_work: 0,
            },
        )
    } else {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
    }
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 16 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compiler, source, helper);
    assert_eq!(compiler.finish().retained_bytes(), 0);
    result
}
fn execute(case: Case, javascript: &str) -> Json {
    let script = format!("const events=[];{}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{}\nprocess.stdout.write(JSON.stringify(events));", case.setup,serde_json::to_string(javascript).unwrap(),case.observations);
    let result = super::native_tests::execute(Command::new("node").args([
        "--input-type=module",
        "-e",
        &script,
    ]));
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        observed,
        serde_json::from_str::<Json>(case.expected).unwrap(),
        "{}\n{script}",
        case.name
    );
    observed
}
fn score(sizes: Sizes) -> [usize; 3] {
    [sizes.raw, sizes.gzip9.unwrap(), sizes.brotli11.unwrap()]
}

#[derive(Clone)]
struct Row {
    style: Style,
    requested: LiteralOutput,
    output: OutputTactics,
    javascript: String,
    sizes: [usize; 3],
}
fn output_json(output: OutputTactics) -> Json {
    json!({"dead_code_elimination":output.dead_code_elimination,
        "target_compaction":output.target_compaction,"literals":format!("{:?}",output.literals)})
}
fn emit(case: Case, kind: &str, inlined: bool, cap: Option<usize>, row: &Row, observed: &Json) {
    eprintln!(
        "observation-output-artifact {}",
        json!({"schema":1,"case":case.name,"kind":kind,"inline":inlined,"search_cap":cap,
        "style":format!("{:?}",row.style),"requested":if kind == "manual" { json!(format!("{:?}",row.requested)) } else { Json::Null },"output":output_json(row.output),
        "source":case.source,"source_sha256":digest(case.source),"setup":case.setup,"observations":case.observations,
        "javascript":row.javascript,"javascript_sha256":digest(&row.javascript),"sizes":row.sizes,
        "expected":serde_json::from_str::<Json>(case.expected).unwrap(),"observed":observed})
    );
}
fn matrix(case: Case, inlined: bool, prune: bool, expected_alternative: bool) {
    let p = policy(16, true, true);
    with_source(case, false, |compiler, source, helper| {
        let direct = compiler
            .direct_javascript(source, &p, WorkDomain::Baseline)
            .unwrap();
        let candidate = if inlined {
            compiler
                .enable_local_facts(request().facts_cache, WorkDomain::Optional)
                .unwrap();
            match compiler
                .inline_helper_javascript(
                    direct,
                    helper.unwrap(),
                    request().helper,
                    &p,
                    WorkDomain::Optional,
                )
                .unwrap()
                .outcome
            {
                HelperOutcome::Published(candidate) => candidate,
                other => panic!("existing leaf forwarding must qualify: {other:?}"),
            }
        } else {
            direct
        };
        let choices = OutputTactics {
            dead_code_elimination: prune,
            literals: LiteralOutput::Original,
            ..OutputTactics::from_policy(&p)
        };
        let mut entered = 0;
        let rows = compiler
            .with_javascript_output_choices_in(
                candidate,
                &p,
                choices,
                WorkDomain::Optional,
                |output| {
                    entered += 1;
                    assert_eq!(output.has_literal_alternative()?, expected_alternative);
                    let mut rows = Vec::new();
                    for style in STYLES {
                        for requested in MODES {
                            let artifact = output.render_bounded_with_literals(
                                &Plan::new(style),
                                requested,
                                usize::MAX,
                            )?;
                            let (javascript, actual, observed) =
                                output.with_artifact(artifact, |view| {
                                    (
                                        view.javascript.to_owned(),
                                        view.output,
                                        execute(case, view.javascript),
                                    )
                                })?;
                            assert_eq!(
                                actual.literals,
                                if expected_alternative {
                                    requested
                                } else {
                                    LiteralOutput::Original
                                }
                            );
                            assert_eq!(actual.dead_code_elimination, prune);
                            assert!(actual.target_compaction);
                            let sizes = [
                                output.measure(artifact, Objective::Raw)?,
                                output.measure(artifact, Objective::Gzip)?,
                                output.measure(artifact, Objective::Brotli)?,
                            ];
                            assert_eq!(sizes[0], javascript.len());
                            let row = Row {
                                style,
                                requested,
                                output: actual,
                                javascript,
                                sizes,
                            };
                            emit(case, "manual", inlined, None, &row, &observed);
                            rows.push((output.retain_artifact(artifact)?, row));
                        }
                    }
                    assert_eq!(output.has_literal_alternative()?, expected_alternative);
                    Ok::<_, CandidateError>(rows)
                },
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            entered, 1,
            "both modes and names share one prepared callback"
        );
        for pair in rows.chunks_exact(2) {
            if expected_alternative {
                assert_ne!(pair[0].1.javascript, pair[1].1.javascript);
            } else {
                assert_eq!(pair[0].1.javascript, pair[1].1.javascript);
            }
        }
        for (artifact, row) in rows {
            compiler
                .with_artifact(artifact, |view| {
                    assert_eq!(
                        view.output, row.output,
                        "actual mode survives prepared target destruction"
                    );
                    assert_eq!(view.javascript, row.javascript);
                    assert_eq!(score(view.sizes), row.sizes);
                })
                .unwrap();
            compiler.discard_artifact(artifact).unwrap();
        }
    });
    eprintln!(
        "observation-output-summary {}",
        json!({"schema":1,"case":case.name,"inline":inlined,"prune":prune,"has_alternative":expected_alternative,"manual_artifacts":6,"released":true})
    );
}

#[test]
fn weak_literals_share_one_prepared_target_across_modes_names_and_preserve() {
    matrix(WEAK, false, true, true);
    matrix(WEAK, false, false, false);
}
#[test]
fn mixed_exact_public_and_shared_reference_literals_canonicalize_to_original() {
    matrix(EXACT, false, true, false);
}
#[test]
fn inline_forwarding_weakens_literal_output_while_shared_call_keeps_exact_bytes() {
    matrix(FORWARD, false, true, false);
    matrix(FORWARD, true, true, true);
}

#[test]
fn literal_output_refusals_preserve_prior_artifacts_and_never_return_false_availability() {
    let p = policy(16, true, false);
    with_source(WEAK, false, |compiler, source, _| {
        let candidate = compiler
            .direct_javascript(source, &p, WorkDomain::Baseline)
            .unwrap();
        let choices = OutputTactics {
            literals: LiteralOutput::Original,
            ..OutputTactics::from_policy(&p)
        };
        compiler
            .with_javascript_output_choices_in(
                candidate,
                &p,
                choices,
                WorkDomain::Optional,
                |output| {
                    assert!(output.has_literal_alternative().unwrap());
                    let original = output
                        .render_bounded_with_literals(
                            &Plan::new(Style::Scoped),
                            LiteralOutput::Original,
                            usize::MAX,
                        )
                        .unwrap();
                    let old = output
                        .with_artifact(original, |view| {
                            execute(WEAK, view.javascript);
                            view.javascript.to_owned()
                        })
                        .unwrap();
                    assert!(matches!(
                        output.render_bounded_with_literals(
                            &Plan::new(Style::Scoped),
                            LiteralOutput::Observed,
                            0
                        ),
                        Err(CandidateError::Output(
                            crate::js::extract::OutputError::ByteLimit
                        ))
                    ));
                    assert!(output.has_literal_alternative().unwrap());
                    let changed = output
                        .render_bounded_with_literals(
                            &Plan::new(Style::Scoped),
                            LiteralOutput::Observed,
                            usize::MAX,
                        )
                        .unwrap();
                    output
                        .with_artifact(changed, |view| {
                            assert_eq!(view.output.literals, LiteralOutput::Observed);
                            assert_ne!(view.javascript, old);
                            execute(WEAK, view.javascript);
                        })
                        .unwrap();
                    // Existing admission access, used only to falsify a paid lookup.
                    // No allowance reset or new test-only budget/public API is added.
                    output.with_allocation_budget(|budget| {
                        budget.with_ledger(|owner| {
                            let (ledger, domain) = owner.unwrap();
                            let remaining = WORK - ledger.work_used(domain);
                            ledger
                                .charge(domain, WorkKind::Analysis, remaining)
                                .unwrap();
                        })
                    });
                    assert!(matches!(
                        output.has_literal_alternative(),
                        Err(CandidateError::Budget(
                            crate::compilation_policy::BudgetError::WorkExhausted(
                                WorkDomain::Optional
                            )
                        ))
                    ));
                    output
                        .with_artifact(original, |view| {
                            assert_eq!(view.output.literals, LiteralOutput::Original);
                            assert_eq!(view.javascript, old);
                        })
                        .unwrap();
                },
            )
            .unwrap();
    });
    for case in [WEAK, EXACT] {
        with_source(case, false, |compiler, source, _| {
            let candidate = compiler
                .direct_javascript(source, &p, WorkDomain::Baseline)
                .unwrap();
            let off = OutputTactics {
                target_compaction: false,
                literals: LiteralOutput::Original,
                families: crate::js::OutputFamilies::NONE,
                ..OutputTactics::from_policy(&p)
            };
            compiler
                .with_javascript_output_choices_in(
                    candidate,
                    &p,
                    off,
                    WorkDomain::Optional,
                    |output| {
                        assert!(!output.has_literal_alternative().unwrap());
                        assert!(matches!(
                            output.render_bounded_with_literals(
                                &Plan::new(Style::Scoped),
                                LiteralOutput::Observed,
                                usize::MAX
                            ),
                            Err(CandidateError::ForbiddenTactic(TacticId::TargetCompaction))
                        ));
                        let original = output
                            .render_bounded_with_literals(
                                &Plan::new(Style::Scoped),
                                LiteralOutput::Original,
                                usize::MAX,
                            )
                            .unwrap();
                        output
                            .with_artifact(original, |view| {
                                assert_eq!(view.output.literals, LiteralOutput::Original);
                                execute(case, view.javascript);
                            })
                            .unwrap();
                    },
                )
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            let mut entered = false;
            let illegal = OutputTactics {
                literals: LiteralOutput::Observed,
                ..off
            };
            let result = compiler.with_javascript_output_choices_in(
                candidate,
                &p,
                illegal,
                WorkDomain::Optional,
                |_| entered = true,
            );
            assert!(matches!(
                result,
                Err(CandidateError::ForbiddenTactic(TacticId::TargetCompaction))
            ));
            assert!(!entered);
            assert_eq!(compiler.ledger().retained_bytes(), retained);
        });
    }
}

#[test]
fn public_search_skips_inactive_modes_and_keeps_actual_mode_qualified_winners() {
    for (case, cap, active) in [(EXACT, 16, false), (WEAK, 2, true), (WEAK, 16, true)] {
        let p = policy(cap, true, false);
        let summary = with_source(case, true, |compiler, source, _| {
            let mut rows = Vec::<Row>::new();
            let mut descriptor: Option<Vec<u32>> = None;
            let mut baselines = 0;
            let search = compiler
                .search_javascript_observed(source, &p, request(), |view| {
                    if let Some(previous) = &descriptor {
                        assert_eq!(
                            previous.as_slice(),
                            view.recipe_descriptor.whole_words().expect("Whole cohort")
                        );
                    } else {
                        descriptor = Some(
                            view.recipe_descriptor
                                .whole_words()
                                .expect("Whole cohort")
                                .to_vec(),
                        );
                    }
                    if view.baseline {
                        baselines += 1;
                    }
                    let observed = execute(case, view.javascript);
                    if !active {
                        assert_eq!(view.output.literals, LiteralOutput::Original);
                    }
                    let row = Row {
                        style: view.naming.style,
                        requested: view.output.literals,
                        output: view.output,
                        javascript: view.javascript.to_owned(),
                        sizes: score(view.sizes),
                    };
                    emit(case, "search", false, Some(cap), &row, &observed);
                    rows.push(row);
                })
                .unwrap();
            assert_eq!(baselines, 1);
            if !active {
                assert_eq!(
                    search.counters().renders,
                    3,
                    "inactive output must not add render attempts"
                );
                assert_eq!(rows.len(), 3);
            }
            if active && cap == 16 {
                let combinations: BTreeSet<_> = rows
                    .iter()
                    .map(|row| (format!("{:?}", row.style), row.output.literals))
                    .collect();
                assert_eq!(
                    combinations.len(),
                    6,
                    "the fixed fixture has only two output modes and three naming choices"
                );
            }
            let mut winners = Vec::new();
            for (index, objective) in CODECS.into_iter().enumerate() {
                winners.push(search.with_winner(objective,|view,plan| {
                    let sizes=score(view.sizes);
                    assert!(rows.iter().any(|row| row.style==plan.style&&row.output==view.output&&row.javascript==view.javascript&&row.sizes==sizes));
                    assert_eq!(sizes[index],rows.iter().map(|row|row.sizes[index]).min().unwrap());
                    let observed=execute(case,view.javascript);
                    json!({"objective":format!("{objective:?}"),"style":format!("{:?}",plan.style),"output":output_json(view.output),"javascript_sha256":digest(view.javascript),"sizes":sizes,"observed":observed})
                }).unwrap());
            }
            let counters = search.counters();
            let objective = p.objective().unwrap();
            assert!(counters.proposals <= objective.optional_alternatives);
            assert!(counters.codec_probes <= objective.optional_codec_probes);
            if cap == 2 {
                assert!(search.stopped().is_some());
            }
            let summary = json!({"schema":1,"case":case.name,"cap":cap,"active":active,"descriptor":descriptor,"measured":rows.len(),"winners":winners,
                "renders":counters.renders,"proposals":counters.proposals,"codec_probes":counters.codec_probes,"structures":counters.structures,
                "stopped":search.stopped().map(|error|format!("{error:?}")),"discovery_refusal":search.discovery_refusal().map(|error|format!("{error:?}")),"policy":p.receipt()});
            drop(search);
            summary
        });
        eprintln!(
            "observation-output-search-summary {}",
            json!({"released":true,"search":summary})
        );
    }
}

#[test]
fn one_naming_seed_uses_optional_literal_work_only_when_allowed() {
    for cap in [0, 4] {
        let p = policy_with_naming(cap, true, false, false);
        assert_eq!(Plan::seeds_for_policy(&p).unwrap().len(), 1);
        let summary = with_source(WEAK, true, |compiler, source, _| {
            let mut rows = Vec::new();
            let mut descriptor = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &p, request(), |view| {
                    let observed = execute(WEAK, view.javascript);
                    if rows.is_empty() {
                        descriptor.extend_from_slice(
                            view.recipe_descriptor.whole_words().expect("Whole cohort"),
                        );
                    }
                    assert_eq!(
                        descriptor,
                        view.recipe_descriptor.whole_words().expect("Whole cohort")
                    );
                    let row = Row {
                        style: view.naming.style,
                        requested: view.output.literals,
                        output: view.output,
                        javascript: view.javascript.to_owned(),
                        sizes: score(view.sizes),
                    };
                    emit(WEAK, "search", false, Some(cap), &row, &observed);
                    rows.push(row);
                })
                .unwrap();
            assert_eq!(rows.len(), if cap == 0 { 1 } else { 2 });
            assert_eq!(rows[0].output.literals, LiteralOutput::Observed);
            if cap == 0 {
                assert_eq!(search.counters().proposals, 0);
                assert_eq!(search.ledger().work_used(WorkDomain::Optional), 0);
            } else {
                assert_eq!(rows[1].output.literals, LiteralOutput::Original);
                assert_eq!(rows[0].style, rows[1].style);
            }
            let winners = CODECS.into_iter().enumerate().map(|(index, codec)| {
                search.with_winner(codec, |view, plan| {
                    let sizes = score(view.sizes);
                    assert_eq!(sizes[index], rows.iter().map(|row| row.sizes[index]).min().unwrap());
                    assert!(rows.iter().any(|row| row.javascript == view.javascript && row.output == view.output && row.style == plan.style));
                    json!({"objective":format!("{codec:?}"),"style":format!("{:?}",plan.style),
                        "output":output_json(view.output),"javascript_sha256":digest(view.javascript),
                        "sizes":sizes,"observed":execute(WEAK,view.javascript)})
                }).unwrap()
            }).collect::<Vec<_>>();
            json!({"schema":1,"case":WEAK.name,"cap":cap,"active":true,"naming_search":false,
                "descriptor":descriptor,"measured":rows.len(),"winners":winners,
                "renders":search.counters().renders,"proposals":search.counters().proposals,
                "codec_probes":search.counters().codec_probes,"structures":search.counters().structures,
                "stopped":search.stopped().map(|error|format!("{error:?}")),
                "discovery_refusal":search.discovery_refusal().map(|error|format!("{error:?}")),
                "optional_work":search.ledger().work_used(WorkDomain::Optional),"policy":p.receipt()})
        });
        eprintln!(
            "observation-output-search-summary {}",
            json!({"released":true,"search":summary})
        );
    }
}
