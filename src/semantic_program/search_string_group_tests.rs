//! A finite manual oracle for an existing family that automatic discovery only
//! proposes one definition at a time. Codec results are measurements, not goldens.
use super::facts::CacheLimits;
use super::publication::*;
use super::search_opportunities::{Inventory, OpportunityView};
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
    WorkKind,
};
use crate::output_budget::AllocationBudget;
use crate::structured_js::selection::{Objective, Plan, Style};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::process::Command;

const SOURCE: &str = r#"extern void observe(string value);extern void marker();
export void run(){
    observe("counter/status/"+"ready:\ud800");
    marker();
    observe("counter/status/"+"ready:\ud800");
}"#;
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const LABELS: [&str; 6] = [
    "computed",
    "literal-both",
    "pool-first",
    "pool-second",
    "separate-pools",
    "joint-pool",
];

fn policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\n\
         [policy.tactics]\nscalar-replacement='off'\ninlining='off'\n\
         call-specialization='off'\nconstant-folding='on'\nstring-pooling='on'\n\
         identifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'",
    )
    .unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
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

fn publish(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    definitions: &[ValueRef],
    choice: StringChoice,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .represent_string_javascript(
            base,
            definitions,
            choice,
            request(),
            policy,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome
    {
        StringOutcome::Published(candidate) => candidate,
        other => panic!("manual string choice must be proved: {other:?}"),
    }
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

fn observe(javascript: &str) -> Value {
    let script = format!(
        r#"const events=[];let count=0,fail=false;
globalThis.observe=value=>events.push(['value',Array.from({{length:value.length}},(_,i)=>value.charCodeAt(i))]);
globalThis.marker=()=>{{events.push(['marker',++count]);if(fail)throw Error('stop');}};
const api=await import('data:text/javascript,'+encodeURIComponent({}));
events.push(['api',Object.keys(api),api.run.name,api.run.length]);
api.run();api.run();fail=true;try{{api.run();}}catch(error){{events.push(['caught',error.message]);}}
process.stdout.write(JSON.stringify(events));"#,
        serde_json::to_string(javascript).unwrap()
    );
    let result = Command::new("node")
        .args(["--input-type=module", "--eval", &script])
        .output()
        .expect("Node is required for string-group runtime observations");
    assert!(
        result.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&result.stderr)
    );
    let events: Value = serde_json::from_slice(&result.stdout).unwrap();
    let units = "counter/status/ready:"
        .encode_utf16()
        .chain([0xd800])
        .collect::<Vec<_>>();
    assert_eq!(
        events,
        json!([
            ["api", ["run"], "run", 0],
            ["value", units],
            ["marker", 1],
            ["value", units],
            ["value", units],
            ["marker", 2],
            ["value", units],
            ["value", units],
            ["marker", 3],
            ["caught", "stop"]
        ])
    );
    events
}

#[test]
fn manual_joint_string_pool_has_a_qualified_finite_oracle_beyond_singleton_discovery() {
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
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 32 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let (definitions, inventory_receipt) = compiler
        .with_semantic(source, |program, _, ledger| {
            let definitions = program
                .units
                .iter()
                .enumerate()
                .flat_map(|(unit, data)| {
                    data.data().operations.iter().filter_map(move |operation| {
                        let value = operation.result?;
                        (matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                            && matches!(program.types[data.data().values[value.index()].ty.index()], Type::String))
                        .then_some(ValueRef { unit: UnitId::from_index(unit).unwrap(), value })
                    })
                })
                .collect::<Vec<_>>();
            assert_eq!(definitions.len(), 2);
            assert_eq!(definitions[0].unit, definitions[1].unit);
            let owner = RevisionId::fresh();
            let mut budget = AllocationBudget::new(Some((ledger, WorkDomain::Baseline)));
            let inventory = Inventory::build(program, &policy, 16, owner, &mut budget).unwrap();
            assert!(!inventory.truncated());
            assert_eq!(inventory.len(), 4);
            let mut rows = Vec::new();
            for index in 0..inventory.len() {
                let Some(OpportunityView::String { definitions: selected, choice }) = inventory.get(index) else {
                    panic!("only the permitted string family belongs to this inventory");
                };
                assert_eq!(selected.len(), 1, "automatic discovery has no grouped recipe");
                assert!(definitions.contains(&selected[0]));
                rows.push(json!({
                    "definitions":selected.iter().map(|definition| [definition.unit.index(),definition.value.index()]).collect::<Vec<_>>(),
                    "choice":format!("{choice:?}"),
                }));
            }
            budget.with_ledger(|owner_ledger| inventory.discard(owner, owner_ledger.unwrap().0).unwrap());
            (definitions, rows)
        })
        .unwrap();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 16,
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
    let literal = publish(
        &mut compiler,
        direct,
        &definitions,
        StringChoice::LiteralAtDefinition,
        &policy,
    );
    let shared = StringChoice::SharedLiteral {
        activation: definitions[0].unit,
    };
    let first = publish(&mut compiler, direct, &definitions[..1], shared, &policy);
    let second = publish(&mut compiler, direct, &definitions[1..], shared, &policy);
    let separate = compiler
        .combine_javascript(first, second, &policy, WorkDomain::Optional)
        .unwrap();
    let joint = publish(&mut compiler, direct, &definitions, shared, &policy);
    let candidates = [direct, literal, first, second, separate, joint];
    let descriptors = candidates.map(|candidate| {
        compiler
            .with_implementation_description(candidate, WorkDomain::Optional, |view| {
                view.recipe_words().to_vec()
            })
            .unwrap()
    });
    assert_ne!(
        descriptors[4], descriptors[5],
        "two singleton pools are not one joint pool"
    );
    let after_families = counters(&compiler);
    let mut rows = Vec::new();
    let mut minima = [[usize::MAX; 3]; 6];
    for (recipe, candidate) in candidates.into_iter().enumerate() {
        for style in STYLES {
            let before = counters(&compiler);
            let artifact = compiler
                .with_javascript_output_in(candidate, &policy, WorkDomain::Optional, |output| {
                    let artifact = output.render(&Plan::new(style))?;
                    output.retain_artifact(artifact)
                })
                .unwrap()
                .unwrap();
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
                    assert_eq!(view.implementation.recipe_words(), descriptors[recipe]);
                    assert_eq!(provenance.naming().style, style);
                })
                .unwrap();
            let javascript = compiler.take_qualified_artifact(qualifications[2]).unwrap();
            assert_eq!(
                sizes,
                CODECS.map(
                    |codec| crate::compression::measure(javascript.as_bytes(), codec).unwrap()
                )
            );
            let observations = observe(&javascript);
            for codec in 0..3 {
                minima[recipe][codec] = minima[recipe][codec].min(sizes[codec]);
            }
            let row = json!({
                "schema":1,"recipe":LABELS[recipe],"recipe_words":descriptors[recipe],
                "style":format!("{style:?}"),"policy_fingerprint":policy.fingerprint(),
                "javascript":javascript,"sha256":format!("{:x}",Sha256::digest(javascript.as_bytes())),
                "sizes":{"raw":sizes[0],"gzip9":sizes[1],"brotli11":sizes[2]},
                "work_before":before,"work_after":counters(&compiler),"observations":observations,
            });
            eprintln!("string-group-artifact {row}");
            rows.push(row);
        }
    }
    assert_eq!(rows.len(), 18);
    let final_counters = counters(&compiler);
    let final_ledger = compiler.finish();
    assert_eq!(final_ledger.retained_bytes(), 0);
    let singletons_lose = minima[2][2] > minima[0][2] && minima[3][2] > minima[0][2];
    let summary = json!({
        "schema":1,"source":SOURCE,"source_sha256":format!("{:x}",Sha256::digest(SOURCE.as_bytes())),
        "scope":"manual finite oracle; automatic inventory inspected, automatic search not executed",
        "work_scope":"adoption through qualified handoff; inspection frontend and test-owned receipts/runtime/codec replay excluded; Render includes shared allocation/movement tariffs",
        "policy":policy.receipt(),"automatic_inventory":inventory_receipt,
        "recipes":LABELS,"style_minima_raw_gzip_brotli":minima,"outputs":rows.len(),
        "strict_union_brotli_valley_observed":singletons_lose && minima[4][2] < minima[0][2],
        "joint_pool_brotli_amortization_observed":singletons_lose && minima[5][2] < minima[0][2],
        "joint_pool_vs_separate_brotli":minima[5][2] as i64-minima[4][2] as i64,
        "work_before_families":before_families,"work_after_families":after_families,
        "work_after_outputs":final_counters,"retained_bytes_after_finish":final_ledger.retained_bytes(),
        "limitations":"no guaranteed pooling win, no heuristic change, no whole-library or compiler-speed claim",
    });
    eprintln!("string-group-summary {summary}");
}
