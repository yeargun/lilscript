//! Real refusals and complete-artifact observations through the existing owner.
//! This private location also lets the tests deny encoder memory after staging,
//! without adding public failure-injection or mutable ledger APIs.
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetPlan, CompilationRequest, ResourceLimits,
};
use std::process::Command;

const WORK: u64 = 200_000_000;
const MEMORY: u64 = 128_000_000;
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const FACTORY: &str = include_str!("fixtures/value-placement/representation-composition.lil");
const FACTORY_SETUP: &str =
    include_str!("fixtures/value-placement/representation-composition.setup.js");
const FACTORY_EXPECTED: &str =
    include_str!("fixtures/value-placement/representation-composition.expected.out");
const WORD: &str = "export string word(){string first=\"path/\"+\"ready\";string second=\"path/\"+\"ready\";return first+second;}";
// `+2-1` rather than `+1`: this spelling keeps a real codec crossover (the
// scoped naming wins gzip, the global one Brotli), which these tests need.
// Two scopes, so scoped naming reuses a name that global naming cannot.
const BYTE: &str = "int mask(int bits){return bits&255;}export int byte(int value){return mask(value)+1;}";

fn policy(schedule: &str, probes: usize, proposals: usize, structural: bool) -> ResolvedPolicy {
    let tactic = if structural { "on" } else { "off" };
    let configuration = format!(
        "[javascript]\nstrip_console=false\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit={probes}\ncandidate_limit=24\ncandidate_beam_width=12\n[policy.search]\ncodec_schedule='{schedule}'\nrender_batch=8\ndiversity_interval=4\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'\nscalar-replacement='{tactic}'\ninlining='{tactic}'\nconstant-folding='on'\nstring-pooling='on'"
    );
    let config: crate::config::ProjectConfig = toml::from_str(&configuration).unwrap();
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
            max_work: 100_000,
            scratch_bytes: 100_000,
            output_bytes: 100_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 32,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    }
}

fn with_source<R>(
    source: &str,
    memory: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: WORK,
            retained_bytes: memory,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 128 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compilation, source);
    assert_eq!(compilation.checkpoint_count(), 1);
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Observed {
    naming: Plan,
    javascript: String,
    sizes: Sizes,
}
fn observation(entry: SearchObservation<'_>) -> Observed {
    Observed {
        naming: entry.naming.clone(),
        javascript: entry.javascript.to_owned(),
        sizes: entry.sizes,
    }
}
struct Report {
    observed: Vec<Observed>,
    best: [usize; 3],
    counters: SearchCounters,
    refusal: bool,
    terminal_probes: bool,
    cache: Option<LocalFactsCacheStatus>,
}

fn run(source: &str, schedule: &str, probes: usize, memory: u64, request: SearchRequest) -> Report {
    let policy = policy(schedule, probes, 384, source == FACTORY);
    with_source(source, memory, |compilation, source_id| {
        assert!(compilation.local_facts_status().is_none());
        let mut observed = Vec::new();
        let search = compilation
            .search_javascript_observed(source_id, &policy, request, |entry| {
                observed.push(observation(entry))
            })
            .unwrap();
        let best = CODECS.map(|codec| {
            search
                .with_winner(codec, |view, naming| {
                    let size = view.sizes.get(codec).unwrap();
                    assert_eq!(
                        Some(size),
                        observed
                            .iter()
                            .filter_map(|item| item.sizes.get(codec))
                            .min()
                    );
                    assert!(observed.iter().any(|item| {
                        item.javascript == view.javascript && item.naming == *naming
                    }));
                    size
                })
                .unwrap()
        });
        let refusal = search
            .discovery_refusal()
            .is_some_and(SearchError::optional_memory_refusal);
        let terminal_probes = matches!(
            search.stopped(),
            Some(SearchError::Limit(SearchLimit::CodecProbes))
        );
        let counters = search.counters();
        assert!(search.ledger().peak_retained_bytes() <= memory);
        assert!(
            search.ledger().work_used(WorkDomain::Baseline)
                + search.ledger().work_used(WorkDomain::Optional)
                <= WORK
        );
        println!(
            "memory-search-summary {}",
            serde_json::json!({
                "fixture":if source == FACTORY {"Factory"} else {"Word"},
                "schedule":schedule,"memory":memory,"probes":probes,
                "best":best,"observed":observed.len(),
                "proof_queries":counters.proof_queries,
                "unknown_proofs":counters.unknown_proofs,
                "truncated_proofs":counters.truncated_proofs,
                "queued_artifacts":counters.queued_artifacts,
                "scoring_events":counters.scoring_events,
                "codec_probes":counters.codec_probes,
                "discovery_refusal":search.discovery_refusal().map(|error|format!("{error:?}")),
                "stop":search.stopped().map(|error|format!("{error:?}")),
                "optional_work":search.ledger().work_used(WorkDomain::Optional),
                "peak_retained_bytes":search.ledger().peak_retained_bytes(),
            })
        );
        drop(search);
        let cache = compilation.local_facts_status();
        if let Some(cache) = cache {
            // An admitted cache remains Compilation-owned. Removing exactly
            // that owner releases its reservation; final finish checks all
            // artifact slots, semantic checkpoints and other retained owners.
            let retained = compilation.ledger().retained_bytes();
            compilation.discard_local_facts().unwrap();
            assert_eq!(
                retained - compilation.ledger().retained_bytes(),
                cache.reserved_bytes
            );
        }
        // Independent canonical scores and runtime observations follow the
        // compiler handoff. They do not inject measurements into the search.
        for item in &observed {
            for codec in CODECS {
                assert_eq!(
                    item.sizes.get(codec).unwrap(),
                    crate::compression::measure(item.javascript.as_bytes(), codec).unwrap()
                );
            }
            let (setup, tail, expected) = if source == FACTORY {
                (
                    FACTORY_SETUP,
                    "globalThis.valuePlacementObserve(library);",
                    FACTORY_EXPECTED,
                )
            } else {
                ("", "console.log(library.word());", "path/readypath/ready\n")
            };
            let script = format!(
                "{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{tail}",
                serde_json::to_string(&item.javascript).unwrap()
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
        }
        Report {
            observed,
            best,
            counters,
            refusal,
            terminal_probes,
            cache,
        }
    })
}

#[test]
fn cold_memory_refusal_scores_existing_names_and_preserves_fixed_probe_prefixes() {
    // Same source, cold cache, finite work/memory and discovery schedule. This
    // cap admits canonical codecs, but cannot admit the requested 2 MB cache.
    let mut previous: Option<Report> = None;
    for probes in [0, 1, 2, 4, 8] {
        let staged = run(FACTORY, "staged", probes, 1_000_000, request());
        let immediate = run(FACTORY, "immediate", probes, 1_000_000, request());
        assert_eq!(staged.observed, immediate.observed);
        assert_eq!(staged.best, immediate.best);
        assert!(staged.cache.is_none());
        assert_eq!(staged.counters.proof_queries, 0);
        assert_eq!(staged.counters.unknown_proofs, 0);
        assert_eq!(staged.counters.truncated_proofs, 0);
        if probes < 2 {
            assert!(!staged.refusal);
            assert_eq!(staged.counters.queued_artifacts, 0);
        } else {
            assert!(staged.refusal);
            assert_eq!(staged.counters.queued_artifacts, 2);
            assert_eq!(staged.counters.scoring_events, (probes / 2).min(2));
            assert_eq!(staged.terminal_probes, probes == 2);
        }
        if let Some(previous) = previous {
            assert!(staged.observed.starts_with(&previous.observed));
            for codec in 0..3 {
                assert!(staged.best[codec] <= previous.best[codec]);
            }
        }
        previous = Some(staged);
    }
}

#[test]
fn refused_facts_session_releases_prepared_proof_but_keeps_owned_cache() {
    let mut request = request();
    request.facts_cache.result_bytes = 1_000_000;
    // Cache + the canonical encoder fit. Cache + the prepared proof + the
    // explicit facts-session result reservation do not. No owner is evicted.
    let staged = run(WORD, "staged", 8, 2_900_000, request);
    let immediate = run(WORD, "immediate", 8, 2_900_000, request);
    assert!(staged.refusal && immediate.refusal);
    assert_eq!(staged.observed, immediate.observed);
    assert_eq!(staged.best, immediate.best);
    assert_eq!(staged.observed.len(), 3);
    for report in [staged, immediate] {
        assert_eq!(report.counters.proof_queries, 1);
        assert_eq!(report.counters.unknown_proofs, 0);
        assert_eq!(report.counters.truncated_proofs, 0);
        let cache = report.cache.unwrap();
        assert_eq!(cache.entries, 0);
        assert_eq!(cache.reserved_bytes, request.facts_cache.bytes);
    }
}

fn codec_peak(javascript: &str, codec: Objective) -> u64 {
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap();
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        crate::compression::measure_admitted(javascript.as_bytes(), codec, &mut budget).unwrap();
    }
    assert_eq!(ledger.retained_bytes(), 0);
    ledger.peak_retained_bytes()
}

#[test]
fn zero_byte_and_partial_all_codec_failures_are_never_retried_or_observed() {
    for allow_gzip in [false, true] {
        with_source(BYTE, MEMORY, |compilation, source| {
            let baseline = policy("staged", 8, 0, false);
            let policy = policy("staged", 8, 384, false);
            let mut search = compilation
                .search_javascript(source, &baseline, request())
                .unwrap();
            let original = CODECS.map(|codec| {
                search
                    .with_winner(codec, |view, _| view.sizes.get(codec).unwrap())
                    .unwrap()
            });
            let mut observed = Vec::new();
            search
                .evaluate_structure(
                    0,
                    &policy,
                    Objectives::All,
                    &[Style::Scoped],
                    &mut |entry| observed.push(observation(entry)),
                )
                .unwrap();
            assert!(observed.is_empty());
            assert_eq!(search.counters.queued_artifacts, 1);
            let (position, trial) = search
                .portfolio
                .entries
                .iter()
                .find(|(_, entry)| entry.pending)
                .map(|(position, entry)| (position, entry.artifact))
                .unwrap();
            let javascript = search
                .compilation
                .artifacts
                .with_artifact(trial, |view| view.javascript.to_owned())
                .unwrap();
            let gzip_peak = codec_peak(&javascript, Objective::Gzip);
            assert!(codec_peak(&javascript, Objective::Brotli) > gzip_peak);
            let allowance = if allow_gzip { gzip_peak } else { 0 };
            let padding = MEMORY - search.ledger().retained_bytes() - allowance;
            search
                .compilation
                .ledger
                .retain(WorkDomain::Optional, padding)
                .unwrap();
            let retained = search.ledger().retained_bytes();
            let work = search.ledger().work_used(WorkDomain::Optional);
            let error = search
                .score_next(&policy, Objectives::All, &mut |entry| {
                    observed.push(observation(entry))
                })
                .unwrap_err();
            assert!(error.optional_memory_refusal(), "{error:?}");
            assert!(search.ledger().work_used(WorkDomain::Optional) > work);
            assert_eq!(search.ledger().retained_bytes(), retained);
            assert_eq!(search.counters.codec_probes, if allow_gzip { 2 } else { 1 });
            assert_eq!(search.counters.scoring_events, 0);
            assert!(!search.portfolio.entries.get(position).unwrap().pending);
            let partial = search
                .compilation
                .artifacts
                .with_artifact(trial, |view| view.sizes)
                .unwrap();
            assert_eq!(partial.gzip9.is_some(), allow_gzip);
            assert!(partial.brotli11.is_none());
            let probes = search.counters.codec_probes;
            search.discovery_refusal = Some(error);
            search.finish_discovery().unwrap();
            search
                .drain_pending(&policy, Objectives::All, &mut |entry| {
                    observed.push(observation(entry))
                })
                .unwrap();
            assert_eq!(search.counters.codec_probes, probes);
            assert!(observed.is_empty());
            assert!(search
                .compilation
                .artifacts
                .with_artifact(trial, |_| ())
                .is_err());
            assert_eq!(search.portfolio.entries.len(), 1);
            for (codec, expected) in CODECS.into_iter().zip(original) {
                assert_eq!(
                    search.with_winner(codec, |view, _| view.sizes.get(codec).unwrap()),
                    Some(expected)
                );
            }
            assert!(search.stopped().unwrap().optional_memory_refusal());
            search
                .compilation
                .ledger
                .release(WorkDomain::Optional, padding)
                .unwrap();
        });
    }
}
