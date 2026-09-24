//! A finite naming/structure oracle over one admitted target per recipe.
//! Source-name overrides are alternatives, not assumed compressed-size wins.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
    WorkKind,
};
use crate::js::selection::{Objective, Plan, Style};
use oxc_ast::ast::{BindingPattern, Expression, Statement};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::process::Command;

const SOURCE: &str = r#"extern void observe(string value);extern void marker();
export void report(string status,string message){
    observe(("status/"+"message/")+status+message);
    marker();
    observe(("status/"+"message/")+message+status);
}"#;
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const RECIPES: [&str; 2] = ["computed", "literal-both"];

fn policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\n\
         [policy.tactics]\nscalar-replacement='off'\ninlining='off'\n\
         call-specialization='off'\nconstant-folding='on'\nstring-pooling='off'\n\
         identifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'",
    )
    .unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn counters(compiler: &Compilation<'_>) -> Value {
    let ledger = compiler.ledger();
    json!({
        "baseline_work":ledger.work_used(WorkDomain::Baseline),
        "optional_work":ledger.work_used(WorkDomain::Optional),
        "analysis_work":ledger.work_by_kind(WorkKind::Analysis),
        "render_work":ledger.work_by_kind(WorkKind::Render),
        "codec_work":ledger.work_by_kind(WorkKind::Codec),
        "retained_bytes":ledger.retained_bytes(),
        "peak_retained_bytes":ledger.peak_retained_bytes(),
    })
}

fn parameter_names(callable: &str) -> [String; 2] {
    let source = format!("({callable});");
    let arena = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&arena, &source, oxc_span::SourceType::mjs()).parse();
    assert!(
        !parsed.panicked && parsed.diagnostics.is_empty(),
        "{source}"
    );
    let [Statement::ExpressionStatement(statement)] = parsed.program.body.as_slice() else {
        panic!("expected one reflected callable expression: {source}");
    };
    let Expression::FunctionExpression(function) = statement.expression.get_inner_expression()
    else {
        panic!("expected the exported ordinary function: {source}");
    };
    assert_eq!(function.params.items.len(), 2);
    assert!(function.params.rest.is_none());
    std::array::from_fn(|index| {
        let BindingPattern::BindingIdentifier(binding) = &function.params.items[index].pattern
        else {
            panic!("fixture parameter must remain an identifier: {source}");
        };
        binding.name.to_string()
    })
}

fn observe(javascript: &str) -> (Value, [String; 2]) {
    let script = format!(
        r#"const events=[];let count=0,fail=false;
globalThis.observe=value=>events.push(['value',Array.from({{length:value.length}},(_,i)=>value.charCodeAt(i))]);
globalThis.marker=()=>{{events.push(['marker',++count]);if(fail)throw Error('stop');}};
const api=await import('data:text/javascript,'+encodeURIComponent({}));
events.push(['api',Object.keys(api),api.report.name,api.report.length]);
events.push(['return',api.report('ready','message')===undefined]);
events.push(['return',api.report('status','\ud800')===undefined]);
fail=true;try{{api.report('stop','now');}}catch(error){{events.push(['caught',error.message]);}}
process.stdout.write(JSON.stringify({{events,callable:Function.prototype.toString.call(api.report)}}));"#,
        serde_json::to_string(javascript).unwrap()
    );
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required for naming-neighborhood observations");
    assert!(
        result.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&result.stderr)
    );
    let result: Value = serde_json::from_slice(&result.stdout).unwrap();
    let units = |text: &str| text.encode_utf16().collect::<Vec<_>>();
    let second_first = "status/message/status"
        .encode_utf16()
        .chain([0xd800])
        .collect::<Vec<_>>();
    let second_second = "status/message/"
        .encode_utf16()
        .chain([0xd800])
        .chain("status".encode_utf16())
        .collect::<Vec<_>>();
    let events = result["events"].clone();
    assert_eq!(
        events,
        json!([
            ["api", ["report"], "report", 2],
            ["value", units("status/message/readymessage")],
            ["marker", 1],
            ["value", units("status/message/messageready")],
            ["return", true],
            ["value", second_first],
            ["marker", 2],
            ["value", second_second],
            ["return", true],
            ["value", units("status/message/stopnow")],
            ["marker", 3],
            ["caught", "stop"]
        ])
    );
    // Function source is inspected only to identify naming choices. It is not
    // part of the runtime equivalence observations asserted above.
    let names = parameter_names(result["callable"].as_str().unwrap());
    (events, names)
}

#[test]
fn source_name_overrides_have_a_qualified_finite_structural_neighborhood() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let policy = policy();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 100_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 16_000_000,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 8 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let definitions = compiler
        .with_semantic(source, |program, _, _| {
            program
                .units
                .iter()
                .enumerate()
                .flat_map(|(unit, frozen)| {
                    let data = frozen.data();
                    data.operations.iter().filter_map(move |operation| {
                        let value = operation.result?;
                        let operands = data.operands(operation.operands)?;
                        (matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                            && operands.len() == 2
                            && operands.iter().all(|operand| {
                                matches!(
                                    data.operations
                                        [data.values[operand.index()].definition.index()]
                                    .kind,
                                    OperationKind::Constant(Constant::String(_))
                                )
                            }))
                        .then_some(ValueRef {
                            unit: UnitId::from_index(unit).unwrap(),
                            value,
                        })
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap();
    assert_eq!(definitions.len(), 2);
    assert_eq!(definitions[0].unit, definitions[1].unit);
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
    let before_families = counters(&compiler);
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let literal = match compiler
        .represent_string_javascript(
            direct,
            &definitions,
            StringChoice::LiteralAtDefinition,
            StringRequest {
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
        .outcome
    {
        StringOutcome::Published(candidate) => candidate,
        other => panic!("literal alternative must be proved: {other:?}"),
    };
    let after_families = counters(&compiler);
    let candidates = [direct, literal];
    let mut scores = [[[[usize::MAX; 3]; 3]; 4]; 2];
    let mut source_candidates = Vec::new();
    let mut source_names = Vec::new();
    let mut rows = Vec::new();
    for (recipe, candidate) in candidates.into_iter().enumerate() {
        let descriptor = compiler
            .with_implementation_description(candidate, WorkDomain::Optional, |view| {
                view.recipe_words().to_vec()
            })
            .unwrap();
        let before_prepare = counters(&compiler);
        let (binding_ids, eligible_binding_ids, artifacts) = compiler
            .with_javascript_output_in(candidate, &policy, WorkDomain::Optional, |output| {
                let eligible = output.source_candidates()?;
                assert!(eligible.len() >= 2);
                // This existing list orders by descending reference frequency,
                // then target binding ID. The two parameters are used twice.
                let bindings = [eligible[0], eligible[1]];
                let eligible_binding_ids = eligible
                    .iter()
                    .map(|binding| binding.index())
                    .collect::<Vec<_>>();
                let mut artifacts = Vec::new();
                for mask in 0..4 {
                    for (style_index, style) in STYLES.into_iter().enumerate() {
                        let mut plan = Plan::new(style);
                        plan.source_names
                            .extend(bindings.iter().enumerate().filter_map(|(bit, &binding)| {
                                (mask & (1 << bit) != 0).then_some(binding)
                            }));
                        plan.source_names.sort_unstable();
                        let artifact = output.render(&plan)?;
                        artifacts.push((
                            mask,
                            style_index,
                            plan,
                            output.retain_artifact(artifact)?,
                        ));
                    }
                }
                Ok::<_, CandidateError>((
                    bindings.map(|binding| binding.index()),
                    eligible_binding_ids,
                    artifacts,
                ))
            })
            .unwrap()
            .unwrap();
        assert_eq!(artifacts.len(), 12);
        let after_prepare = counters(&compiler);
        let mut names = [String::new(), String::new()];
        let mut plain_global = None;
        let mut changed = 0;
        for (mask, style_index, plan, artifact) in artifacts {
            let before_score = counters(&compiler);
            let sizes = CODECS.map(|codec| {
                compiler
                    .measure_artifact(artifact, codec, WorkDomain::Optional)
                    .unwrap()
            });
            let qualifications = CODECS.map(|codec| {
                compiler
                    .qualify_artifact(
                        artifact,
                        &policy,
                        codec,
                        ArtifactRuntimeEvidence::default(),
                        None,
                        WorkDomain::Optional,
                    )
                    .unwrap()
            });
            for (index, receipt) in qualifications.iter().enumerate() {
                assert_eq!(receipt.cost().transfer_bytes as usize, sizes[index]);
                assert_eq!(receipt.policy_fingerprint(), policy.fingerprint());
            }
            compiler
                .with_qualified_artifact(&qualifications[2], |view, provenance| {
                    assert_eq!(view.implementation.recipe_words(), descriptor);
                    assert_eq!(provenance.naming(), &plan);
                })
                .unwrap();
            let javascript = compiler.take_qualified_artifact(qualifications[2]).unwrap();
            assert_eq!(
                sizes,
                CODECS.map(
                    |codec| crate::compression::measure(javascript.as_bytes(), codec).unwrap()
                )
            );
            let (observations, parameters) = observe(&javascript);
            if style_index == 0 {
                if mask == 0 {
                    assert!(!parameters
                        .iter()
                        .any(|name| name == "status" || name == "message"));
                    plain_global = Some(javascript.clone());
                } else {
                    changed += usize::from(plain_global.as_ref().unwrap() != &javascript);
                    if mask == 1 || mask == 2 {
                        let preserved = parameters
                            .iter()
                            .filter(|name| *name == "status" || *name == "message")
                            .collect::<Vec<_>>();
                        assert_eq!(preserved.len(), 1);
                        names[usize::from(mask == 2)] = preserved[0].clone();
                    }
                }
            }
            if style_index == 2 || mask == 3 {
                assert_eq!(parameters, ["status", "message"]);
            }
            scores[recipe][mask][style_index] = sizes;
            let row = json!({
                "schema":1,"recipe":RECIPES[recipe],"recipe_words":descriptor,
                "style":format!("{:?}",plan.style),"source_name_mask":mask,
                "prepared_binding_ids":binding_ids,
                "selected_binding_ids":plan.source_names.iter().map(|binding|binding.index()).collect::<Vec<_>>(),
                "emitted_parameter_names":parameters,"policy_fingerprint":policy.fingerprint(),
                "javascript":javascript,"sha256":format!("{:x}",Sha256::digest(javascript.as_bytes())),
                "sizes":{"raw":sizes[0],"gzip9":sizes[1],"brotli11":sizes[2]},
                "work_before_score":before_score,"work_after_handoff":counters(&compiler),
                "observations":observations,
            });
            eprintln!("naming-neighborhood-artifact {row}");
            rows.push(row);
        }
        assert_eq!(
            changed, 3,
            "each Global override must change emitted identifiers"
        );
        assert_ne!(names[0], names[1]);
        source_names.push(names.clone());
        source_candidates.push(json!({
            "recipe":RECIPES[recipe],"binding_ids":binding_ids,"source_names":names,
            "all_eligible_binding_ids":eligible_binding_ids,
            "neighborhood_selection":"first two candidates in the existing reference-frequency order, independently verified to be the two parameters",
            "candidate_origin":"queried from this recipe's single prepared target; IDs not reused across recipes",
            "name_origin":"independent parser of delivered callable parameter spellings under each singleton Global override",
            "work_before_prepare":before_prepare,"work_after_twelve_renders":after_prepare,
        }));
    }
    assert_eq!(rows.len(), 24);
    let minima: [[[usize; 3]; 4]; 2] = std::array::from_fn(|recipe| {
        std::array::from_fn(|canonical_mask| {
            let mask = ["status", "message"]
                .into_iter()
                .enumerate()
                .filter(|(bit, _)| canonical_mask & (1 << bit) != 0)
                .fold(0, |mask, (_, name)| {
                    mask | (1
                        << source_names[recipe]
                            .iter()
                            .position(|found| found == name)
                            .unwrap())
                });
            std::array::from_fn(|codec| {
                (0..3)
                    .map(|style| scores[recipe][mask][style][codec])
                    .min()
                    .unwrap()
            })
        })
    });
    let naming_valleys: [bool; 2] = std::array::from_fn(|recipe| {
        minima[recipe][1][2] > minima[recipe][0][2]
            && minima[recipe][2][2] > minima[recipe][0][2]
            && minima[recipe][3][2] < minima[recipe][0][2]
    });
    let structure_naming_valleys: [bool; 3] = std::array::from_fn(|index| {
        let mask = index + 1;
        minima[1][0][2] > minima[0][0][2]
            && minima[0][mask][2] > minima[0][0][2]
            && minima[1][mask][2] < minima[0][0][2]
    });
    let best_masks: [[usize; 3]; 2] = std::array::from_fn(|recipe| {
        std::array::from_fn(|codec| {
            (0..4)
                .min_by_key(|&mask| minima[recipe][mask][codec])
                .unwrap()
        })
    });
    let novel_override_counts: [[usize; 2]; 2] = std::array::from_fn(|recipe| {
        let recipe_rows = &rows[recipe * 12..(recipe + 1) * 12];
        let seeds = &recipe_rows[..3];
        let mut distinct = Vec::new();
        let mut attempts = 0;
        for row in &recipe_rows[3..] {
            let text = row["javascript"].as_str().unwrap();
            if !seeds
                .iter()
                .any(|seed| seed["javascript"].as_str() == Some(text))
            {
                attempts += 1;
                if !distinct.contains(&text) {
                    distinct.push(text);
                }
            }
        }
        [attempts, distinct.len()]
    });
    let zero_mask_minima: [[usize; 3]; 2] = std::array::from_fn(|recipe| minima[recipe][0]);
    let all_mask_minima: [[usize; 3]; 2] = std::array::from_fn(|recipe| {
        std::array::from_fn(|codec| {
            (0..4)
                .map(|mask| minima[recipe][mask][codec])
                .min()
                .unwrap()
        })
    });
    let final_counters = counters(&compiler);
    let final_ledger = compiler.finish();
    assert_eq!(final_ledger.retained_bytes(), 0);
    let summary = json!({
        "schema":1,"source":SOURCE,"source_sha256":format!("{:x}",Sha256::digest(SOURCE.as_bytes())),
        "scope":"manual two-recipe x four-source-name-mask x three-style oracle; no automatic search execution",
        "work_scope":"adoption through qualified handoff; frontend inspection, test-owned plans/receipts, runtime, OXC inspection and codec replay excluded; Render includes shared allocation/movement tariffs",
        "policy":policy.receipt(),"recipes":RECIPES,"source_candidates":source_candidates,
        "summary_mask_bit_source_names":["status","message"],
        "outputs":rows.len(),"prepared_targets":2,"style_minima_by_recipe_mask_raw_gzip_brotli":minima,
        "override_outputs_absent_from_zero_mask_styles_by_recipe_attempts_and_distinct":novel_override_counts,
        "zero_mask_style_minima_by_recipe_raw_gzip_brotli":zero_mask_minima,
        "all_mask_style_minima_by_recipe_raw_gzip_brotli":all_mask_minima,
        "best_source_name_mask_by_recipe_raw_gzip_brotli":best_masks,
        "strict_two_toggle_brotli_valley_by_recipe":naming_valleys,
        "strict_structure_naming_brotli_valley_by_nonempty_mask":structure_naming_valleys,
        "structure_changes_best_brotli_override_mask":best_masks[0][2] != best_masks[1][2],
        "semantic_scheduler_neighborhood":"current semantic search evaluates Plan::new(style), equivalent to mask zero; these source-name overrides are not structural family union",
        "work_before_families":before_families,"work_after_families":after_families,
        "work_after_outputs":final_counters,"retained_bytes_after_finish":final_ledger.retained_bytes(),
        "limitations":"finite style minima, not arbitrary naming optimum; no guaranteed override win or valley, no heuristic changes, no whole-library or compiler-speed claim; callable source text is inspection, not a preserved observation",
    });
    eprintln!("naming-neighborhood-summary {summary}");
}
