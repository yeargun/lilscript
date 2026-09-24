//! Public publication/search clients over the unchanged qualified reference
//! normalization fixture. The finite manual oracle is independent of discovery.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Objectives, Plan, Sizes, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::process::Command;

const SOURCE: &str = include_str!("fixtures/search-reference-runtime/opaque-int-field.lil");
const SETUP: &str = include_str!("fixtures/search-reference-runtime/opaque-int-field.setup.mjs");
const OBSERVATIONS: &str =
    include_str!("fixtures/search-reference-runtime/opaque-int-field.observations.mjs");
const EXPECTED: &str =
    include_str!("fixtures/search-reference-runtime/opaque-int-field.expected.json");
const ORIGIN: &str = include_str!("fixtures/search-reference-runtime/origin.json");
const ORIGINAL_TEST: &str = include_str!("fixtures/search-reference-runtime/original-test.rs");
const CURRENT_TEST: &str = include_str!("product_reference_javascript_tests.rs");
const HELPERS: [&str; 3] = ["read", "ignore", "overwrite"];
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const WORK: u64 = 200_000_000;
const MEMORY: u64 = 256_000_000;
const BEAM: usize = 2;

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}

fn provenance() -> Json {
    let origin: Json = serde_json::from_str(ORIGIN).unwrap();
    assert_eq!(origin["provenance"]["snapshot"], "original-test.rs");
    assert_eq!(origin["provenance"]["sha256"], digest(ORIGINAL_TEST));
    assert_eq!(origin["helpers"], json!(HELPERS));
    for (name, text) in [
        ("opaque-int-field.lil", SOURCE),
        ("opaque-int-field.setup.mjs", SETUP),
        ("opaque-int-field.observations.mjs", OBSERVATIONS),
        ("opaque-int-field.expected.json", EXPECTED),
    ] {
        let file = origin["files"]
            .as_array()
            .unwrap()
            .iter()
            .find(|file| file["file"] == name)
            .unwrap();
        assert_eq!(file["sha256"], digest(text));
        assert_eq!(file["bytes"], json!(text.len()));
        assert!(
            text.is_empty() || ORIGINAL_TEST.contains(text),
            "original {name} bytes"
        );
        assert!(
            text.is_empty() || CURRENT_TEST.contains(text),
            "current source and observation coverage for {name}"
        );
    }
    assert_eq!(
        serde_json::from_str::<Json>(EXPECTED)
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        6
    );
    origin
}

fn policy(cap: usize) -> ResolvedPolicy {
    let text = format!(
        "[javascript]\noptimization_level=15\npriority='size-first'\ncost_model='brotli'\nstrip_console=false\ncandidate_proposal_limit={cap}\nterminal_codec_probe_limit={}\ncandidate_limit=16\ncandidate_beam_width={BEAM}\n[policy.search]\ncodec_schedule='staged'\nrender_batch=8\ndiversity_interval=4\n[policy.tactics]\ninlining='on'\nscalar-replacement='on'\ncall-specialization='off'\nconstant-folding='off'\nstring-pooling='off'\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'",
        cap * 2
    );
    let config: crate::config::ProjectConfig = toml::from_str(&text).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn request() -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 200_000,
        result_bytes: 100_000,
    };
    SearchRequest {
        objectives: Objectives::All,
        scalar: ScalarRequest {
            max_work: 2_000_000,
            scratch_bytes: 2_000_000,
            output_bytes: 2_000_000,
        },
        helper: HelperRequest {
            max_work: 2_000_000,
            scratch_bytes: 2_000_000,
            output_bytes: 2_000_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 2_000_000,
            scratch_bytes: 2_000_000,
            output_bytes: 2_000_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 32,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    }
}

fn named(program: &Program<'_>, name: &str) -> CellId {
    let mut cells = program
        .cells()
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let cell = CellId::from_index(cells.next().expect(name).0).unwrap();
    assert!(cells.next().is_none(), "unique fixture name {name}");
    cell
}

fn canonical_mapping(program: &Program<'_>) -> String {
    // Caller-owned diagnostics over existing indexed owners. Full equality is
    // checked before hashes; fresh revisions/SourceIdentity stamps are not
    // a claim that two independently adopted snapshots share a production owner.
    let units: Vec<_> = program.units().iter().map(|unit| unit.data()).collect();
    let modules: Vec<_> = program
        .modules()
        .iter()
        .map(|module| {
            (
                module.initializer,
                &module.dependencies,
                &module.imports,
                &module.exports,
            )
        })
        .collect();
    format!(
        "{:?}",
        (
            &program.cells,
            &program.types,
            &program.strings,
            &program.structs,
            &program.enums,
            &program.fields,
            &program.exports,
            &program.initialization,
            program.entry,
            modules,
            units
        )
    )
}

fn with_source<R>(
    search: bool,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, CellId, [CellId; 3], String) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let root = named(&program, "state");
    let helpers = HELPERS.map(|name| named(&program, name));
    let mapping = canonical_mapping(&program);
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
    let result = inspect(&mut compiler, source, root, helpers, mapping);
    assert_eq!(compiler.finish().retained_bytes(), 0);
    result
}

fn execute(javascript: &str) -> Json {
    let allocator = oxc_allocator::Allocator::default();
    let parsed =
        oxc_parser::Parser::new(&allocator, javascript, oxc_span::SourceType::mjs()).parse();
    assert!(
        !parsed.panicked && parsed.diagnostics.is_empty(),
        "{:?}\n{javascript}",
        parsed.diagnostics
    );
    let script = format!(
        "const events=[];console.log=value=>events.push(value);\n{SETUP}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{OBSERVATIONS}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(javascript).unwrap()
    );
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
        serde_json::from_str::<Json>(EXPECTED).unwrap(),
        "{script}"
    );
    observed
}

#[derive(Clone)]
struct Artifact {
    fields: bool,
    helper_mask: usize,
    descriptor: Vec<u32>,
    style: Style,
    output: OutputTactics,
    sizes: [usize; 3],
    javascript: String,
}

fn output_metadata(output: OutputTactics) -> Json {
    json!({
        "dead_code_elimination": output.dead_code_elimination,
        "target_compaction": output.target_compaction,
        "literals": format!("{:?}", output.literals),
    })
}
fn score(sizes: Sizes) -> [usize; 3] {
    [sizes.raw, sizes.gzip9.unwrap(), sizes.brotli11.unwrap()]
}

fn emit(run: &str, kind: &str, baseline: bool, row: &Artifact, observed: &Json) {
    let helpers: Vec<_> = HELPERS
        .into_iter()
        .enumerate()
        .filter_map(|(bit, name)| (row.helper_mask & (1 << bit) != 0).then_some(name))
        .collect();
    eprintln!(
        "reference-search-artifact {}",
        json!({"schema":1,"run":run,"case":"opaque-int-field","kind":kind,
        "fields":row.fields,"helper_mask":row.helper_mask,"helpers":helpers,"baseline":baseline,
        "compact":true,"dead_code_elimination":true,"style":format!("{:?}",row.style),"output":output_metadata(row.output),"descriptor":row.descriptor,
        "source_sha256":digest(SOURCE),"javascript":row.javascript,"javascript_sha256":digest(&row.javascript),
        "sizes":row.sizes,"expected":serde_json::from_str::<Json>(EXPECTED).unwrap(),"observed":observed})
    );
}

fn manual(run: &str, policy: &ResolvedPolicy) -> (Vec<Artifact>, String) {
    with_source(false, |compiler, source, root, helpers, mapping| {
        compiler
            .enable_local_facts(request().facts_cache, WorkDomain::Optional)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, policy, WorkDomain::Baseline)
            .unwrap();
        let flat = match compiler
            .scalar_product_javascript(direct, root, request().scalar, policy, WorkDomain::Optional)
            .unwrap()
            .outcome
        {
            ProductOutcome::Published(candidate) => candidate,
            other => panic!("unchanged reference product must qualify: {other:?}"),
        };
        let mut artifacts = Vec::new();
        for (fields, base) in [(false, direct), (true, flat)] {
            // Enumerate every independent helper subset; each publisher adds
            // one original named helper to an already-built smaller subset.
            let mut subsets = vec![base];
            for mask in 1usize..8 {
                let bit = mask.trailing_zeros() as usize;
                let previous = mask & !(1 << bit);
                let candidate = match compiler
                    .inline_helper_javascript(
                        subsets[previous],
                        helpers[bit],
                        request().helper,
                        policy,
                        WorkDomain::Optional,
                    )
                    .unwrap()
                    .outcome
                {
                    HelperOutcome::Published(candidate) => candidate,
                    other => panic!("unchanged helper subset {mask} must qualify: {other:?}"),
                };
                subsets.push(candidate);
            }
            for (mask, candidate) in subsets.into_iter().enumerate() {
                let descriptor = compiler
                    .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
                    .unwrap();
                for style in STYLES {
                    let retained = compiler.ledger().retained_bytes();
                    let (row, observed) = compiler
                        .with_javascript_output_in(
                            candidate,
                            policy,
                            WorkDomain::Optional,
                            |output| {
                                let artifact = output.render(&Plan::new(style))?;
                                // Runtime observation precedes all manual codec claims.
                                let (observed, actual_output) = output
                                    .with_artifact(artifact, |view| {
                                        (execute(view.javascript), view.output)
                                    })?;
                                let raw = output.measure(artifact, Objective::Raw)?;
                                let gzip = output.measure(artifact, Objective::Gzip)?;
                                let brotli = output.measure(artifact, Objective::Brotli)?;
                                let javascript = output.take_artifact(artifact)?;
                                Ok::<_, CandidateError>((
                                    Artifact {
                                        fields,
                                        helper_mask: mask,
                                        descriptor: descriptor.clone(),
                                        style,
                                        output: actual_output,
                                        sizes: [raw, gzip, brotli],
                                        javascript,
                                    },
                                    observed,
                                ))
                            },
                        )
                        .unwrap()
                        .unwrap();
                    assert_eq!(compiler.ledger().retained_bytes(), retained);
                    assert_eq!(row.sizes[0], row.javascript.len());
                    emit(run, "oracle", false, &row, &observed);
                    artifacts.push(row);
                }
            }
        }
        assert_eq!(artifacts.len(), 48);
        assert_eq!(
            artifacts
                .iter()
                .map(|row| &row.descriptor)
                .collect::<BTreeSet<_>>()
                .len(),
            16
        );
        for fields in [false, true] {
            for mask in 0..8 {
                for style in STYLES {
                    assert_eq!(
                        artifacts
                            .iter()
                            .filter(|row| row.fields == fields
                                && row.helper_mask == mask
                                && row.style == style)
                            .count(),
                        1
                    );
                }
            }
        }
        (artifacts, mapping)
    })
}

fn check(run: &str, cap: usize, require_mixed: bool) {
    let origin = provenance();
    let policy = policy(cap);
    assert!(OutputTactics::from_policy(&policy).dead_code_elimination);
    let (oracle, mapping) = manual(run, &policy);
    let oracle_minima: [usize; 3] =
        std::array::from_fn(|index| oracle.iter().map(|row| row.sizes[index]).min().unwrap());
    let oracle_winners: Vec<_> = CODECS.into_iter().enumerate().map(|(index, objective)| {
        let row = oracle.iter().min_by_key(|row| row.sizes[index]).unwrap();
        let observed = execute(&row.javascript);
        json!({"objective":format!("{objective:?}"),"fields":row.fields,"helper_mask":row.helper_mask,
            "descriptor":row.descriptor,"style":format!("{:?}",row.style),"output":output_metadata(row.output),"javascript_sha256":digest(&row.javascript),"sizes":row.sizes,"observed":observed})
    }).collect();
    eprintln!(
        "reference-search-source {}",
        json!({"schema":1,"run":run,"case":"opaque-int-field","source":SOURCE,
        "setup":SETUP,"observations":OBSERVATIONS,"expected":serde_json::from_str::<Json>(EXPECTED).unwrap(),
        "origin":origin,"canonical_mapping_sha256":digest(&mapping),"policy":policy.receipt(),
        "manual_maps":16,"manual_artifacts":48,"oracle_minima":oracle_minima,"oracle_winners":oracle_winners})
    );
    let summary = with_source(true, |compiler, source, _, _, search_mapping| {
        assert_eq!(
            search_mapping, mapping,
            "independent source snapshots must have identical indexed semantic owners"
        );
        let mut measured = Vec::<Artifact>::new();
        let mut baseline = None;
        let search = compiler.search_javascript_observed(source, &policy, request(), |view| {
            let known = oracle.iter().find(|row| row.descriptor == view.recipe_descriptor.whole_words().expect("Whole cohort") && row.style == view.naming.style && row.output == view.output)
                .expect("search's permitted storage/helper map, naming and actual output belong to the independent finite oracle");
            assert_eq!(known.javascript, view.javascript, "same source mapping, canonical recipe, name plan and actual output");
            assert_eq!(known.sizes, score(view.sizes));
            let observed = execute(view.javascript);
            if view.baseline {
                assert!(baseline.is_none());
                assert!(!known.fields && known.helper_mask == 0);
                baseline = Some(known.clone());
            }
            emit(run, "search", view.baseline, known, &observed);
            measured.push(known.clone());
        }).unwrap();
        let baseline = baseline.expect("mandatory executed baseline");
        let mixed = measured
            .iter()
            .any(|row| row.fields && row.helper_mask != 0);
        if require_mixed {
            assert!(
                mixed,
                "bounded discovery must reach Fields plus at least one independently selected reference helper; counters={:?}, stop={:?}",
                search.counters(),
                search.stopped()
            );
        }
        let minima: [usize; 3] =
            std::array::from_fn(|index| measured.iter().map(|row| row.sizes[index]).min().unwrap());
        let mut winners = Vec::new();
        for (index, objective) in CODECS.into_iter().enumerate() {
            winners.push(search.with_winner(objective, |view, plan| {
                let sizes = score(view.sizes);
                let row = measured.iter().find(|row| row.style == plan.style && row.output == view.output && row.javascript == view.javascript && row.sizes == sizes)
                    .expect("winner belongs to actual executed measured union");
                assert_eq!(sizes[index], minima[index]);
                assert!(sizes[index] <= baseline.sizes[index], "retained mandatory baseline");
                let observed = execute(view.javascript);
                json!({"objective":format!("{objective:?}"),"fields":row.fields,"helper_mask":row.helper_mask,
                    "descriptor":row.descriptor,"style":format!("{:?}",plan.style),"output":output_metadata(view.output),"javascript_sha256":digest(view.javascript),"sizes":sizes,"observed":observed})
            }).unwrap());
        }
        let counters = search.counters();
        let objective = policy.objective().unwrap();
        assert_eq!(objective.optional_alternatives, cap);
        assert_eq!(objective.optional_codec_probes, cap * 2);
        assert_eq!(objective.beam_width, BEAM);
        assert!(counters.proposals <= objective.optional_alternatives);
        assert!(counters.codec_probes <= objective.optional_codec_probes);
        if !require_mixed {
            assert!(
                matches!(
                    search.stopped(),
                    Some(SearchError::Limit(SearchLimit::Alternatives))
                ),
                "small fixed cap is a deliberate finalization witness: {:?}",
                search.stopped()
            );
        }
        let comparison: Vec<_> = oracle.iter().map(|row| json!({"fields":row.fields,"helper_mask":row.helper_mask,
            "descriptor":row.descriptor,"style":format!("{:?}",row.style),"output":output_metadata(row.output),"sizes":row.sizes,
            "reached":measured.iter().any(|seen| seen.descriptor==row.descriptor && seen.style==row.style && seen.output==row.output)})).collect();
        let gaps: [i64; 3] =
            std::array::from_fn(|index| minima[index] as i64 - oracle_minima[index] as i64);
        let summary = json!({"schema":1,"run":run,"case":"opaque-int-field","schedule_version":crate::compilation_policy::SEARCH_SCHEDULE_VERSION,
            "canonical_mapping_sha256":digest(&mapping),"require_mixed":require_mixed,"reached_mixed":mixed,
            "caps":{"alternatives":objective.optional_alternatives,"codec_probes":objective.optional_codec_probes,"beam":objective.beam_width,"retained_candidates":objective.retained_candidates,"logical_work":WORK,"retained_bytes":MEMORY},
            "measured":measured.len(),"search_minima":minima,"oracle_minima":oracle_minima,"search_minus_oracle":gaps,"oracle_comparison":comparison,"oracle_winners":oracle_winners,"winners":winners,
            "counters":{"proposals":counters.proposals,"structural_attempts":counters.structural_attempts,"structures":counters.structures,"proof_queries":counters.proof_queries,"renders":counters.renders,"codec_probes":counters.codec_probes,"unknown_proofs":counters.unknown_proofs,"truncated_proofs":counters.truncated_proofs,"conflicting_choices":counters.conflicting_choices,"duplicate_states":counters.duplicate_states,"beam_evictions":counters.beam_evictions,"inventory_truncated":counters.inventory_truncated},
            "stopped":search.stopped().map(|value|format!("{value:?}")),"discovery_refusal":search.discovery_refusal().map(|value|format!("{value:?}")),
            "work_baseline":search.ledger().work_used(WorkDomain::Baseline),"work_optional":search.ledger().work_used(WorkDomain::Optional),"peak_retained_bytes":search.ledger().peak_retained_bytes(),
            "comparison_scope":"all16 independently published local-storage/helper subsets under three names with each policy-default actual output; matching includes actual DCE/compaction/literal mode; fixed-cap search coverage can be incomplete; no cross-cap winner monotonicity claim"});
        drop(search);
        summary
    });
    // Both source owners, their proof caches, artifacts and candidates have
    // passed the real finish().retained_bytes()==0 check before this marker.
    eprintln!(
        "reference-search-summary {}",
        json!({"released":true,"manual_released":true,"search":summary})
    );
}

#[test]
fn reference_oracle_and_small_cap_finalization_execute_exact_observations() {
    check("small", 12, false);
}

#[test]
fn reference_search_reaches_mixed_fields_and_inline_under_larger_fixed_cap() {
    check("large", 96, true);
}
