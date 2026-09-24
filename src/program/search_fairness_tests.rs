//! Public search observations over independently supplied helper combinations.
//! This tests skip-branch reachability and execution, not a global size optimum.
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain, WorkKind,
};
use crate::program::facts::CacheLimits;
use crate::program::publication::*;
use crate::program::*;
use crate::js::selection::{Objective, Objectives, Plan, Sizes, Style};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::process::Command;

const SOURCE: &str = r#"
int first(int value){return(value&255)+3;}
int second(int value){return(value&127)^17;}
export int run(int value){return first(value)+second(value);}
"#;
const WORK: u64 = 100_000_000;
const MEMORY: u64 = 64_000_000;
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];

fn policy(beam: usize) -> ResolvedPolicy {
    schedule_policy(beam, 48, "staged")
}

fn schedule_policy(beam: usize, probes: usize, schedule: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\noptimization_level=15\npriority='size-first'\ncost_model='brotli'\nstrip_console=false\ncandidate_proposal_limit=24\nterminal_codec_probe_limit={probes}\ncandidate_limit=8\ncandidate_beam_width={beam}\n[policy.search]\ncodec_schedule='{schedule}'\nrender_batch=8\ndiversity_interval=4\n[policy.tactics]\ninlining='on'\nscalar-replacement='off'\ncall-specialization='off'\nconstant-folding='off'\nstring-pooling='off'\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'"
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
    search: bool,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, [CellId; 2]) -> R,
) -> R {
    with_named_source(SOURCE, search, ["first", "second"], inspect)
}

fn with_named_source<R, const N: usize>(
    source: &str,
    search: bool,
    names: [&str; N],
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, [CellId; N]) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let helpers = names.map(|name| {
        let mut cells = program
            .cells()
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.name == name);
        let index = cells.next().expect("fixture helper").0;
        assert!(cells.next().is_none());
        CellId::from_index(index).unwrap()
    });
    // Inventory visits these semantic cells in order; no fixed numeric IDs or
    // emitted identifier spellings determine which recipe the test expects.
    assert!(helpers.windows(2).all(|pair| pair[0] < pair[1]));
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
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 64 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compiler, source, helpers);
    assert_eq!(compiler.finish().retained_bytes(), 0);
    result
}

fn execute(javascript: &str) -> Json {
    let inputs: [i64; 12] = [
        -257,
        -129,
        -17,
        -1,
        0,
        1,
        7,
        127,
        128,
        255,
        256,
        4_294_967_297,
    ];
    let mut expected: Vec<Json> = inputs
        .iter()
        .map(|&value| json!([value, (value & 255) + 3 + ((value & 127) ^ 17)]))
        .collect();
    expected.extend([
        json!(["coerce", 1]),
        json!(["coerce", 2]),
        json!(["boxed", 35]),
        json!(["throw-coerce", 1]),
        json!(["throw-coerce", 2]),
        json!(["caught", true]),
    ]);
    let script = format!(
        r#"const library=await import('data:text/javascript,'+encodeURIComponent({}));
const events=[];
for(const value of {})events.push([value,library.run(value)]);
let calls=0;
events.push(['boxed',library.run({{valueOf(){{events.push(['coerce',++calls]);return 6+calls;}}}})]);
let throws=0;const failure={{}};
try{{library.run({{valueOf(){{events.push(['throw-coerce',++throws]);if(throws===2)throw failure;return 7;}}}});events.push(['missing-throw']);}}
catch(error){{events.push(['caught',error===failure]);}}
process.stdout.write(JSON.stringify(events));"#,
        serde_json::to_string(javascript).unwrap(),
        serde_json::to_string(&inputs).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for structural fairness execution");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(observed, json!(expected), "{javascript}");
    observed
}

fn sizes(value: Sizes) -> [usize; 3] {
    [value.raw, value.gzip9.unwrap(), value.brotli11.unwrap()]
}

#[derive(Clone, Debug)]
struct Artifact {
    descriptor: Vec<u32>,
    style: Style,
    sizes: [usize; 3],
    javascript: String,
}

fn emit(kind: &str, label: &str, beam: usize, entry: &Artifact, observed: &Json) {
    eprintln!(
        "search-fairness-artifact {}",
        json!({"fixture":"two-private-helpers","source":SOURCE,"kind":kind,"label":label,"beam":beam,
            "descriptor":entry.descriptor,"style":format!("{:?}",entry.style),"sizes":entry.sizes,
            "javascript":entry.javascript,"javascript_sha256":format!("{:x}",Sha256::digest(entry.javascript.as_bytes())),"observed":observed})
    );
}

fn inline(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    helper: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .inline_helper_javascript(base, helper, request().helper, policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("independent fixture helper must qualify: {other:?}"),
    }
}

fn oracle(policy: &ResolvedPolicy, beam: usize) -> Vec<(&'static str, Artifact)> {
    with_source(false, |compiler, source, helpers| {
        compiler
            .enable_local_facts(request().facts_cache, WorkDomain::Optional)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, policy, WorkDomain::Baseline)
            .unwrap();
        let first = inline(compiler, direct, helpers[0], policy);
        let second = inline(compiler, direct, helpers[1], policy);
        let both = inline(compiler, first, helpers[1], policy);
        let mut result = Vec::new();
        for (label, candidate) in [
            ("direct", direct),
            ("first", first),
            ("second", second),
            ("both", both),
        ] {
            let descriptor = compiler
                .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
                .unwrap();
            for style in STYLES {
                let entry = compiler
                    .with_javascript_output(candidate, policy, |output| {
                        let artifact = output.render(&Plan::new(style))?;
                        let raw = output.measure(artifact, Objective::Raw)?;
                        let gzip = output.measure(artifact, Objective::Gzip)?;
                        let brotli = output.measure(artifact, Objective::Brotli)?;
                        let javascript = output.take_artifact(artifact)?;
                        Ok::<_, CandidateError>(Artifact {
                            descriptor: descriptor.clone(),
                            style,
                            sizes: [raw, gzip, brotli],
                            javascript,
                        })
                    })
                    .unwrap()
                    .unwrap();
                assert_eq!(entry.sizes[0], entry.javascript.len());
                let observed = execute(&entry.javascript);
                emit("oracle", label, beam, &entry, &observed);
                result.push((label, entry));
            }
        }
        assert_eq!(
            result
                .iter()
                .map(|(_, entry)| &entry.descriptor)
                .collect::<std::collections::BTreeSet<_>>()
                .len(),
            4
        );
        result
    })
}

fn check_search(beam: usize, require_joint: bool) {
    let policy = policy(beam);
    let oracle = oracle(&policy, beam);
    let skipped = &oracle
        .iter()
        .find(|(label, _)| *label == "second")
        .unwrap()
        .1
        .descriptor;
    with_source(true, |compiler, source, _| {
        let mut observed = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, request(), |view| {
                let entry = Artifact {
                    descriptor: view
                        .recipe_descriptor
                        .whole_words()
                        .expect("Whole cohort")
                        .to_vec(),
                    style: view.naming.style,
                    sizes: sizes(view.sizes),
                    javascript: view.javascript.to_owned(),
                };
                assert_eq!(entry.sizes[0], entry.javascript.len());
                let actual = execute(&entry.javascript);
                emit("search", "measured", beam, &entry, &actual);
                if let Some((_, known)) = oracle.iter().find(|(_, known)| {
                    known.descriptor == entry.descriptor && known.style == entry.style
                }) {
                    assert_eq!(entry.javascript, known.javascript);
                    assert_eq!(entry.sizes, known.sizes);
                }
                observed.push(entry);
            })
            .unwrap();
        assert!(
            observed.iter().any(|entry| &entry.descriptor == skipped),
            "later helper without the first must receive service: counters={:?}; stop={:?}",
            search.counters(),
            search.stopped()
        );
        if require_joint {
            for (_, known) in &oracle {
                assert!(
                    observed
                        .iter()
                        .any(|entry| entry.descriptor == known.descriptor
                            && entry.style == known.style),
                    "two-slot fixture must retain all four compatible recipes/names: {:?}; counters={:?}; stop={:?}",
                    known.descriptor,
                    search.counters(),
                    search.stopped()
                );
            }
        }
        let mut winners = Vec::new();
        for (index, objective) in CODECS.into_iter().enumerate() {
            winners.push(search.with_winner(objective,|view,plan| {
                let score=sizes(view.sizes);
                assert!(observed.iter().any(|entry| entry.javascript==view.javascript && entry.style==plan.style && entry.sizes==score));
                assert_eq!(score[index],observed.iter().map(|entry|entry.sizes[index]).min().unwrap());
                let actual=execute(view.javascript);
                json!({"objective":format!("{objective:?}"),"style":format!("{:?}",plan.style),"sizes":score,"observed":actual})
            }).unwrap());
        }
        let counters = search.counters();
        let objective = policy.objective().unwrap();
        assert!(counters.proposals <= objective.optional_alternatives);
        assert!(counters.codec_probes <= objective.optional_codec_probes);
        eprintln!(
            "search-fairness-summary {}",
            json!({"fixture":"two-private-helpers","beam":beam,"proposals":counters.proposals,
            "structural_attempts":counters.structural_attempts,"structures":counters.structures,"beam_evictions":counters.beam_evictions,
            "proof_queries":counters.proof_queries,"codec_probes":counters.codec_probes,"measured":observed.len(),"winners":winners,
            "stopped":search.stopped().map(|reason|format!("{reason:?}")),"discovery_refusal":search.discovery_refusal().map(|reason|format!("{reason:?}")),
            "optional_work":search.ledger().work_used(WorkDomain::Optional),"peak_retained_bytes":search.ledger().peak_retained_bytes()})
        );
        drop(search);
    });
}

#[test]
fn bounded_structural_service_reaches_skipped_helper_and_independent_compatible_union() {
    check_search(2, true);
}

#[test]
fn width_one_keeps_later_skip_cursor_and_executes_its_actual_scored_winners() {
    // Width one intentionally sacrifices branch breadth. It still must serve
    // the surviving root cursor; no joint/exhaustive quality claim follows.
    check_search(1, false);
}

const VALLEY_SOURCE: &str = include_str!("fixtures/search-structural-valley/entry.lil");
const VALLEY_HELPERS: [&str; 3] = ["distractor", "first", "second"];

#[test]
fn finite_valley_schedule_ablation_reports_quality_and_work_without_changing_defaults() {
    let reference_policy = policy(2);
    let oracle = valley_oracle(&reference_policy);
    let minima: [usize; 3] = std::array::from_fn(|index| {
        oracle
            .iter()
            .map(|(_, artifact)| artifact.sizes[index])
            .min()
            .unwrap()
    });
    let mut rows = 0;
    for beam in [1, 2] {
        for schedule in ["immediate", "staged"] {
            for objectives in [Objectives::One(Objective::Brotli), Objectives::All] {
                for probes in [2, 8, 24, 48] {
                    let policy = schedule_policy(beam, probes, schedule);
                    assert_eq!(policy.contract(), reference_policy.contract());
                    with_named_source(
                        VALLEY_SOURCE,
                        true,
                        VALLEY_HELPERS,
                        |compiler, source, _| {
                            let mut request = request();
                            request.objectives = objectives;
                            let mut seen = Vec::new();
                            let mut best_seen = [usize::MAX; 3];
                            let search = compiler.search_javascript_observed(source, &policy, request, |entry| {
                            let descriptor = entry.recipe_descriptor.whole_words().unwrap();
                            let (mask, known) = oracle.iter().find(|(_, known)| {
                                known.descriptor == descriptor && known.style == entry.naming.style
                            }).expect("every searched recipe belongs to the independent finite oracle");
                            assert_eq!(entry.javascript, known.javascript);
                            assert_eq!(entry.sizes.raw, known.sizes[0]);
                            for (index, codec) in CODECS.iter().enumerate() {
                                if objectives.iter().any(|requested| requested == *codec) {
                                    let size = entry.sizes.get(*codec).unwrap();
                                    assert_eq!(size, known.sizes[index]);
                                    best_seen[index] = best_seen[index].min(size);
                                }
                            }
                            if objectives == Objectives::One(Objective::Brotli) {
                                assert_eq!(entry.sizes.gzip9, None, "unrequested gzip must not run");
                            }
                            seen.push((*mask, entry.naming.style));
                        }).unwrap();
                            let counters = search.counters();
                            let budget = policy.objective().unwrap();
                            assert!(counters.proposals <= budget.optional_alternatives);
                            assert!(counters.codec_probes <= probes);
                            let mut winners = Vec::new();
                            for (index, codec) in CODECS.iter().enumerate() {
                                if !objectives.iter().any(|requested| requested == *codec) {
                                    assert!(search.with_winner(*codec, |_, _| ()).is_none());
                                    continue;
                                }
                                winners.push(search.with_winner(*codec, |artifact, naming| {
                                let score = artifact.sizes.get(*codec).unwrap();
                                assert_eq!(score, best_seen[index], "select the exact best explored score");
                                assert!(score >= minima[index], "finite oracle is a lower bound, not a search guarantee");
                                let direct = oracle.iter().filter(|(mask, known)| *mask == 0 && known.style == Style::Global)
                                    .map(|(_, known)| known.sizes[index]).next().unwrap();
                                assert!(score <= direct, "the admitted direct incumbent cannot be lost");
                                let observed = execute_valley(artifact.javascript);
                                json!({"objective":format!("{codec:?}"),"score":score,"oracle":minima[index],
                                    "regret":score-minima[index],"style":format!("{:?}",naming.style),
                                    "javascript":artifact.javascript,
                                    "sha256":format!("{:x}",Sha256::digest(artifact.javascript.as_bytes())),"observed":observed})
                            }).unwrap());
                            }
                            eprintln!(
                                "search-schedule-ablation-summary {}",
                                json!({
                                    "beam":beam,"schedule":schedule,"objectives":format!("{objectives:?}"),"probe_limit":probes,
                                    "proposal_limit":budget.optional_alternatives,"proposals":counters.proposals,
                                    "structural_attempts":counters.structural_attempts,"structures":counters.structures,
                                    "proof_queries":counters.proof_queries,"probes":counters.codec_probes,"renders":counters.renders,
                                    "measured":seen.len(),"seen":seen.iter().map(|(mask,style)|json!([mask,format!("{style:?}")])).collect::<Vec<_>>(),
                                    "beam_evictions":counters.beam_evictions,"queued":counters.queued_artifacts,
                                    "baseline_work":search.ledger().work_used(WorkDomain::Baseline),
                                    "optional_work":search.ledger().work_used(WorkDomain::Optional),
                                    "analysis_work":search.ledger().work_by_kind(WorkKind::Analysis),
                                    "edit_work":search.ledger().work_by_kind(WorkKind::Edit),
                                    "render_work":search.ledger().work_by_kind(WorkKind::Render),
                                    "codec_work":search.ledger().work_by_kind(WorkKind::Codec),
                                    "peak_retained_bytes":search.ledger().peak_retained_bytes(),"winners":winners,
                                    "stopped":search.stopped().map(|reason|format!("{reason:?}")),
                                    "discovery_refusal":search.discovery_refusal().map(|reason|format!("{reason:?}"))
                                })
                            );
                        },
                    );
                    rows += 1;
                }
            }
        }
    }
    assert_eq!(rows, 32);
}

fn execute_valley(javascript: &str) -> Json {
    let inputs: [i64; 12] = [
        -513,
        -257,
        -65,
        -1,
        0,
        1,
        7,
        65,
        255,
        512,
        767,
        4_294_967_297,
    ];
    let mut expected = vec![json!(["api", "run", 1, "side", 1])];
    expected.extend(inputs.iter().map(|&value| {
        json!([
            value,
            (value & 512) + 3 + ((value & 255) ^ 65),
            (value & 65) ^ 12
        ])
    }));
    expected.extend([
        json!(["coerce", 1]),
        json!(["coerce", 2]),
        json!(["boxed", 76]),
        json!(["side-coerce"]),
        json!(["side-boxed", 77]),
        json!(["throw-coerce", 1]),
        json!(["throw-coerce", 2]),
        json!(["caught", true]),
    ]);
    let script = format!(
        r#"const library=await import('data:text/javascript,'+encodeURIComponent({}));
const events=[['api',library.run.name,library.run.length,library.side.name,library.side.length]];
for(const value of {})events.push([value,library.run(value),library.side(value)]);
let calls=0;
events.push(['boxed',library.run({{valueOf(){{events.push(['coerce',++calls]);return 6+calls;}}}})]);
events.push(['side-boxed',library.side({{valueOf(){{events.push(['side-coerce']);return 71;}}}})]);
let throws=0;const failure={{}};
try{{library.run({{valueOf(){{events.push(['throw-coerce',++throws]);if(throws===2)throw failure;return 7;}}}});events.push(['missing-throw']);}}
catch(error){{events.push(['caught',error===failure]);}}
process.stdout.write(JSON.stringify(events));"#,
        serde_json::to_string(javascript).unwrap(),
        serde_json::to_string(&inputs).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for structural valley observations");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(observed, json!(expected), "{javascript}");
    observed
}

fn valley_oracle(policy: &ResolvedPolicy) -> Vec<(usize, Artifact)> {
    valley_oracle_for(VALLEY_SOURCE, policy)
}
fn valley_oracle_for(fixture: &str, policy: &ResolvedPolicy) -> Vec<(usize, Artifact)> {
    with_named_source(
        fixture,
        false,
        VALLEY_HELPERS,
        |compiler, source, helpers| {
            compiler
                .enable_local_facts(request().facts_cache, WorkDomain::Optional)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, policy, WorkDomain::Baseline)
                .unwrap();
            let mut artifacts = Vec::new();
            for mask in 0..1 << helpers.len() {
                let mut candidate = direct;
                for (bit, helper) in helpers.iter().enumerate() {
                    if mask & (1 << bit) != 0 {
                        candidate = inline(compiler, candidate, *helper, policy);
                    }
                }
                let descriptor = compiler
                    .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
                    .unwrap();
                for style in STYLES {
                    let artifact = compiler
                        .with_javascript_output(candidate, policy, |output| {
                            let artifact = output.render(&Plan::new(style))?;
                            let raw = output.measure(artifact, Objective::Raw)?;
                            let gzip = output.measure(artifact, Objective::Gzip)?;
                            let brotli = output.measure(artifact, Objective::Brotli)?;
                            Ok::<_, CandidateError>(Artifact {
                                descriptor: descriptor.clone(),
                                style,
                                sizes: [raw, gzip, brotli],
                                javascript: output.take_artifact(artifact)?,
                            })
                        })
                        .unwrap()
                        .unwrap();
                    let observed = if fixture == VALLEY_SOURCE {
                        execute_valley(&artifact.javascript)
                    } else {
                        json!(null)
                    };
                    eprintln!(
                        "structural-valley-artifact {}",
                        json!({
                            "kind":"oracle", "mask":mask, "descriptor":artifact.descriptor,
                            "style":format!("{style:?}"), "sizes":artifact.sizes,
                            "javascript":artifact.javascript, "observed":observed,
                        })
                    );
                    artifacts.push((mask, artifact));
                }
            }
            artifacts
        },
    )
}

#[test]
#[ignore = "its measured Brotli interaction did not survive 008 printing; 010 re-derives the interaction trap"]
fn narrow_brotli_search_crosses_two_singleton_losses_with_a_competing_helper() {
    let policy = policy(2);
    let oracle = valley_oracle(&policy);
    let minima: [usize; 8] = std::array::from_fn(|mask| {
        oracle
            .iter()
            .filter(|(candidate, _)| *candidate == mask)
            .map(|(_, artifact)| artifact.sizes[2])
            .min()
            .unwrap()
    });
    // Compare each recipe over all three styles currently enumerated by semantic
    // search. A raw loss or one losing spelling of a profitable singleton is insufficient.
    assert!(
        minima[2] > minima[0],
        "first alone must lose Brotli: {minima:?}"
    );
    assert!(
        minima[4] > minima[0],
        "second alone must lose Brotli: {minima:?}"
    );
    assert!(
        minima[6] < minima[0],
        "their combination must win Brotli: {minima:?}"
    );
    assert!(
        minima[1] > minima[0],
        "the earlier competing helper must also lose: {minima:?}"
    );
    let descriptors = oracle
        .iter()
        .map(|(_, entry)| &entry.descriptor)
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(descriptors.len(), 8);
    with_named_source(
        VALLEY_SOURCE,
        true,
        VALLEY_HELPERS,
        |compiler, source, _| {
            let mut request = request();
            request.objectives = Objectives::One(Objective::Brotli);
            let mut observed = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &policy, request, |entry| {
                    assert_eq!(entry.sizes.gzip9, None, "Brotli search must not run gzip");
                    let descriptor = entry.recipe_descriptor.whole_words().unwrap();
                    let (mask, known) = oracle
                        .iter()
                        .find(|(_, known)| {
                            known.descriptor == descriptor && known.style == entry.naming.style
                        })
                        .expect("searched recipe must belong to the independent finite oracle");
                    assert_eq!(entry.javascript, known.javascript);
                    assert_eq!(entry.sizes.raw, known.sizes[0]);
                    assert_eq!(entry.sizes.brotli11, Some(known.sizes[2]));
                    let actual = execute_valley(entry.javascript);
                    eprintln!(
                        "structural-valley-artifact {}",
                        json!({
                            "kind":"search", "mask":mask, "descriptor":descriptor,
                            "style":format!("{:?}",entry.naming.style), "sizes":entry.sizes,
                            "javascript":entry.javascript, "observed":actual,
                        })
                    );
                    observed.push((*mask, entry.naming.style, entry.sizes.brotli11.unwrap()));
                })
                .unwrap();
            assert!(
                observed
                    .iter()
                    .any(|&(mask, _, size)| mask == 6 && size == minima[6]),
                "two-slot beam must reach the joint winner despite both singleton losses: {:?}",
                search.counters()
            );
            let winner = search
                .with_winner(Objective::Brotli, |entry, _| {
                    execute_valley(entry.javascript);
                    entry.sizes.brotli11.unwrap()
                })
                .unwrap();
            assert_eq!(winner, *minima.iter().min().unwrap());
            assert_eq!(winner, observed.iter().map(|entry| entry.2).min().unwrap());
            let counters = search.counters();
            let budget = policy.objective().unwrap();
            assert!(counters.proposals <= budget.optional_alternatives);
            assert!(counters.codec_probes <= budget.optional_codec_probes);
            assert!(
                counters.beam_evictions > 0,
                "exercise real frontier pressure"
            );
            eprintln!(
                "structural-valley-summary {}",
                json!({
                    "source":VALLEY_SOURCE, "helpers":VALLEY_HELPERS, "minima":minima,
                    "beam":budget.beam_width, "proposal_limit":budget.optional_alternatives,
                    "probe_limit":budget.optional_codec_probes, "proposals":counters.proposals,
                    "probes":counters.codec_probes, "structures":counters.structures,
                    "beam_evictions":counters.beam_evictions, "winner":winner,
                    "stopped":search.stopped().map(|reason|format!("{reason:?}")),
                })
            );
        },
    );
}
