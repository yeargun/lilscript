//! Schedule comparisons over actual semantic candidates and canonical codecs.
//! The fixed-cap prefix checks vary only the codec allowance. They make no
//! claim about prefixes across different proposal, pool, beam or schedule caps.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Objective, Objectives, Plan, Sizes, Style};
use std::process::Command;
use std::time::Instant;

const WORK: u64 = 200_000_000;
const MEMORY: u64 = 128_000_000;
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
// `+2-1` rather than `+1`: this spelling keeps a real codec crossover (the
// scoped naming wins gzip, the global one Brotli), which these tests need.
const BYTE: &str = "export int byte(int value){return(value&255)+1;}";
// The Factory program with a long private record key used four times:
// pooling the key string is the raw winner, while Brotli prefers the repeated
// literal spelling it compresses for free. Observations are unchanged.
const KEYS: &str = "extern int argument();extern string observe(int value,string label);\nexport func()->string make(int seed){Record<int> state=record{theNumberOfEventsObservedForThisParticularLabel:seed};auto createLabel=(int unused)=>\"place/\"+\"ready\";return ()=>{int before=state.theNumberOfEventsObservedForThisParticularLabel??0;state.theNumberOfEventsObservedForThisParticularLabel=before+1;return observe(state.theNumberOfEventsObservedForThisParticularLabel??0,createLabel(argument()));};}\n";
const FACTORY: &str = include_str!("fixtures/value-placement/representation-composition.lil");
const FACTORY_SETUP: &str =
    include_str!("fixtures/value-placement/representation-composition.setup.js");
const FACTORY_EXPECTED: &str =
    include_str!("fixtures/value-placement/representation-composition.expected.out");

#[derive(Clone, Copy, Debug)]
enum Fixture {
    Byte,
    Factory,
    Keys,
}
impl Fixture {
    fn source(self) -> &'static str {
        match self {
            Self::Byte => BYTE,
            Self::Factory => FACTORY,
            Self::Keys => KEYS,
        }
    }
    fn validate(self, javascript: &str) {
        let (setup, observation, expected) = match self {
            Self::Factory | Self::Keys => (
                FACTORY_SETUP,
                "await globalThis.valuePlacementObserve(library);",
                FACTORY_EXPECTED,
            ),
            Self::Byte => (
                "",
                r#"
                const seen=[library.byte.name,library.byte.length];
                for(const value of [-1,0,-0,255,256,4294967297])seen.push(library.byte(value));
                const failure={};try{library.byte({valueOf(){seen.push('coerce');throw failure;}});}catch(error){seen.push(error===failure);}
                console.log(JSON.stringify(seen));
            "#,
                "[\"byte\",1,256,1,1,256,1,2,\"coerce\",true]\n",
            ),
        };
        let script = format!("{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{observation}", serde_json::to_string(javascript).unwrap());
        let output = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for staged search observations");
        assert!(
            output.status.success(),
            "{}\n{javascript}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{javascript}"
        );
    }
}

fn policy(
    schedule: &str,
    probes: usize,
    pool: usize,
    batch: usize,
    diversity: usize,
    fixture: Fixture,
    representations: bool,
) -> ResolvedPolicy {
    let compact = if matches!(fixture, Fixture::Byte) {
        "off"
    } else {
        "on"
    };
    let structural = if representations { "on" } else { "off" };
    let text = format!(
        "[javascript]\nstrip_console=false\ncandidate_proposal_limit=384\nterminal_codec_probe_limit={probes}\ncandidate_limit={pool}\ncandidate_beam_width=12\n[policy.search]\ncodec_schedule='{schedule}'\nrender_batch={batch}\ndiversity_interval={diversity}\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='{compact}'\nscalar-replacement='{structural}'\ninlining='{structural}'\nconstant-folding='on'\nstring-pooling='on'"
    );
    let config: crate::config::ProjectConfig = toml::from_str(&text).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn request(objectives: Objectives) -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    };
    SearchRequest {
        objectives,
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
            entries: 32,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct Observed {
    recipe: u64,
    naming: Plan,
    sizes: Sizes,
    javascript: String,
    baseline: bool,
}
struct Report {
    observed: Vec<Observed>,
    winners: [Option<Observed>; 3],
    counters: SearchCounters,
    stopped: Option<String>,
}

fn run(fixture: Fixture, policy: &ResolvedPolicy, objectives: Objectives) -> Report {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, fixture.source()).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let units = (0..program.units.len())
        .map(|index| UnitId::from_index(index).unwrap())
        .collect::<Vec<_>>();
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: WORK,
            retained_bytes: MEMORY,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 128 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let request = request(objectives);
    // Both reusable owners belong to Compilation. Populate every source unit's
    // facts under the exact shared helper/string qualification, and warm
    // enough artifact slots for the whole configured pool plus one provisional
    // trial, then discard every warmup artifact before timing/search admission.
    // This makes exact post-Drop equality sensitive to leaked pending payloads
    // and provenance, without mistaking reusable metadata for a search leak.
    compiler
        .enable_local_facts(request.facts_cache, WorkDomain::Baseline)
        .unwrap();
    assert!(units.len() <= request.facts_cache.entries);
    compiler
        .with_local_facts(WorkDomain::Baseline, units.len(), |group| {
            for &unit in &units {
                drop(group.query(source, unit, request.helper.local_facts)?);
            }
            Ok::<_, CompilationFactsError>(())
        })
        .unwrap()
        .unwrap();
    let direct = compiler
        .direct_javascript(source, policy, WorkDomain::Baseline)
        .unwrap();
    let warm = compiler
        .with_javascript_output(direct, policy, |output| {
            let mut artifacts = Vec::new();
            for _ in 0..policy.objective().unwrap().retained_candidates + 1 {
                let artifact = output.render(&Plan::new(Style::Global))?;
                artifacts.push(output.retain_artifact(artifact)?);
            }
            Ok::<_, CandidateError>(artifacts)
        })
        .unwrap()
        .unwrap();
    for artifact in warm {
        compiler.discard_artifact(artifact).unwrap();
    }
    compiler.discard(direct.semantic_id()).unwrap();
    let retained = compiler.ledger().retained_bytes();
    let mut observed = Vec::new();
    let started = Instant::now();
    let search = compiler
        .search_javascript_observed(source, policy, request, |entry| {
            observed.push(Observed {
                recipe: entry.recipe_fingerprint,
                naming: entry.naming.clone(),
                sizes: entry.sizes,
                javascript: entry.javascript.into(),
                baseline: entry.baseline,
            });
        })
        .unwrap();
    let elapsed = started.elapsed();
    let counters = search.counters();
    let stopped = search.stopped().map(|reason| format!("{reason:?}"));
    let winners = CODECS.map(|codec| {
        search.with_winner(codec, |view, naming| Observed {
            recipe: 0,
            naming: naming.clone(),
            sizes: view.sizes,
            javascript: view.javascript.into(),
            baseline: false,
        })
    });
    let baseline = observed.first().expect("scored mandatory baseline");
    assert!(baseline.baseline);
    // Exact measurement happens after timing, independently of Portfolio's
    // rank/comparison machinery; missing codecs remain absent, never zero.
    for entry in &observed {
        assert_eq!(entry.sizes.raw, entry.javascript.len());
        for codec in CODECS {
            if let Some(measured) = entry.sizes.get(codec) {
                assert_eq!(
                    measured,
                    crate::compression::measure(entry.javascript.as_bytes(), codec).unwrap()
                );
            }
            if codec != Objective::Raw && !objectives.iter().any(|requested| requested == codec) {
                assert!(
                    entry.sizes.get(codec).is_none(),
                    "unrequested encoder must stay idle"
                );
            }
        }
    }
    for (index, codec) in CODECS.into_iter().enumerate() {
        if objectives.iter().any(|requested| requested == codec) {
            let winner = winners[index].as_ref().expect("requested codec winner");
            assert_eq!(
                winner.sizes.get(codec),
                observed
                    .iter()
                    .filter_map(|entry| entry.sizes.get(codec))
                    .min()
            );
            assert!(observed.iter().any(
                |entry| entry.naming == winner.naming && entry.javascript == winner.javascript
            ));
            assert!(winner.sizes.get(codec).unwrap() <= baseline.sizes.get(codec).unwrap());
            fixture.validate(&winner.javascript);
        } else {
            assert!(winners[index].is_none());
        }
    }
    let objective = policy.objective().unwrap();
    assert!(counters.codec_probes <= objective.optional_codec_probes);
    assert!(counters.proposals <= objective.optional_alternatives);
    assert!(counters.pending_peak <= objective.retained_candidates);
    assert!(search.ledger().retained_bytes() <= MEMORY);
    eprintln!(
        "staged-search-summary {}",
        serde_json::json!({
            "fixture": format!("{fixture:?}"), "source": fixture.source(),
            "schedule": format!("{:?}", objective.search.codec_schedule),
            "render_batch": objective.search.render_batch, "diversity_interval": objective.search.diversity_interval,
            "objectives": format!("{objectives:?}"), "proposal_limit": objective.optional_alternatives,
            "codec_probe_limit": objective.optional_codec_probes, "retained_limit": objective.retained_candidates,
            "retained_byte_limit": objective.retained_candidate_bytes, "beam_width": objective.beam_width,
            "proposals": counters.proposals, "proof_queries": counters.proof_queries,
            "structures": counters.structures, "renders": counters.renders, "codec_probes": counters.codec_probes,
            "admitted_artifacts": counters.admitted_artifacts, "queued_artifacts": counters.queued_artifacts,
            "pending_peak": counters.pending_peak, "pressure_scores": counters.pressure_scores,
            "diversity_scores": counters.diversity_scores, "scoring_events": counters.scoring_events,
            "measured_count": observed.len(), "stop": stopped,
        "baseline_sizes": CODECS.map(|codec| baseline.sizes.get(codec)),
        "baseline_javascript": baseline.javascript,
        "winner_sizes": std::array::from_fn::<_,3,_>(|i| winners[i].as_ref().and_then(|entry| entry.sizes.get(CODECS[i]))),
        "winner_javascript": std::array::from_fn::<_,3,_>(|i| winners[i].as_ref().map(|entry| entry.javascript.as_str())),
        "winner_naming": std::array::from_fn::<_,3,_>(|i| winners[i].as_ref().map(|entry| format!("{:?}", entry.naming.style))),
            "optional_logical_work": search.ledger().work_used(WorkDomain::Optional),
            "peak_retained_bytes": search.ledger().peak_retained_bytes(),
            "search_elapsed_seconds": elapsed.as_secs_f64(),
            "timing_scope": "debug-test search including mandatory baseline and observer copies; excludes postsearch codec/runtime oracles",
        })
    );
    drop(search);
    assert_eq!(
        compiler.ledger().retained_bytes(),
        retained,
        "search stop/Drop must release every pending artifact and proof"
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
    Report {
        observed,
        winners,
        counters,
        stopped,
    }
}
fn union(report: &Report) -> std::collections::BTreeSet<(u64, Style, Vec<usize>, String)> {
    report
        .observed
        .iter()
        .map(|entry| {
            (
                entry.recipe,
                entry.naming.style,
                entry
                    .naming
                    .source_names
                    .iter()
                    .map(|id| id.index())
                    .collect(),
                entry.javascript.clone(),
            )
        })
        .collect()
}

#[test]
fn finite_factory_union_and_exact_winners_match_immediate_and_staged_schedules() {
    let immediate = policy("immediate", 384, 24, 8, 4, Fixture::Factory, true);
    let staged = policy("staged", 384, 24, 8, 4, Fixture::Factory, true);
    assert_eq!(immediate.contract(), staged.contract());
    let eager = run(Fixture::Factory, &immediate, Objectives::All);
    let delayed = run(Fixture::Factory, &staged, Objectives::All);
    assert!(
        eager.stopped.is_none() && delayed.stopped.is_none(),
        "finite fixture should drain: {:?}/{:?}",
        eager.stopped,
        delayed.stopped
    );
    assert_eq!(
        union(&eager),
        union(&delayed),
        "same finite recipes and naming plans must reach the common exact scorer"
    );
    assert!(eager.observed.len() >= 36);
    for index in 0..3 {
        let left = eager.winners[index].as_ref().unwrap();
        let right = delayed.winners[index].as_ref().unwrap();
        assert_eq!(left.sizes, right.sizes);
        assert_eq!(
            left.javascript, right.javascript,
            "canonical ties must not depend on scoring order"
        );
    }
    assert_eq!(eager.counters.queued_artifacts, 0);
    assert_eq!(eager.counters.pending_peak, 0);
    assert!(delayed.counters.queued_artifacts > 0 && delayed.counters.pending_peak > 1);
}

#[test]
fn queue_pressure_scores_an_old_trial_without_losing_the_incoming_trial() {
    let constrained = policy("staged", 384, 2, 8, 4, Fixture::Byte, false);
    let result = run(
        Fixture::Byte,
        &constrained,
        Objectives::One(Objective::Gzip),
    );
    assert!(result.counters.pressure_scores > 0);
    // Global incumbent plus queued Scoped fills the pool. Incoming Source
    // forces Scoped's score. The incoming losing artifact must still be scored
    // later, preserving its ownership while the selected union is replaced.
    for style in [Style::Global, Style::Scoped, Style::Source] {
        assert!(
            result
                .observed
                .iter()
                .any(|entry| entry.naming.style == style),
            "incoming/queued naming plan disappeared: {style:?}"
        );
    }
    let gzip = result.winners[1].as_ref().unwrap();
    assert_eq!(gzip.naming.style, Style::Scoped);
    assert!(gzip.sizes.gzip9 < result.observed[0].sizes.gzip9);
}

#[test]
fn requested_codecs_keep_separate_winners_and_diversity_reaches_a_real_raw_loser() {
    let byte = run(
        Fixture::Byte,
        &policy("staged", 384, 24, 8, 4, Fixture::Byte, false),
        Objectives::All,
    );
    assert_eq!(
        byte.winners[1].as_ref().unwrap().naming.style,
        Style::Scoped
    );
    assert_eq!(
        byte.winners[2].as_ref().unwrap().naming.style,
        Style::Global
    );
    assert_ne!(
        byte.winners[1].as_ref().unwrap().javascript,
        byte.winners[2].as_ref().unwrap().javascript
    );
    // A shared-string tradeoff the codecs disagree about: pooling wins raw,
    // the repeated literal wins Brotli. (Factory's five-letter key stopped
    // being one once the printer spelled literal keys `o.name`.)
    let fixture = run(
        Fixture::Keys,
        &policy("staged", 384, 24, 8, 1, Fixture::Keys, false),
        Objectives::All,
    );
    let raw = fixture.winners[0].as_ref().unwrap();
    let brotli = fixture.winners[2].as_ref().unwrap();
    assert!(
        brotli.sizes.raw > raw.sizes.raw,
        "fixture must establish a genuine raw loser"
    );
    assert!(
        brotli.sizes.brotli11 < raw.sizes.brotli11,
        "the raw loser must actually win Brotli"
    );
    assert!(fixture.counters.diversity_scores > 0);
    assert!(fixture
        .observed
        .iter()
        .any(|entry| entry.javascript == brotli.javascript));
}

#[test]
fn fixed_schedule_probe_prefixes_are_deterministic_and_additional_probes_do_not_regress() {
    let mut previous: Option<Report> = None;
    for probes in [0, 1, 2, 4, 8, 16, 32, 96] {
        let configured = policy("staged", probes, 24, 8, 4, Fixture::Factory, true);
        let current = run(Fixture::Factory, &configured, Objectives::All);
        if probes < 2 {
            assert_eq!(current.observed.len(), 1);
            assert_eq!(
                current.counters.codec_probes, 0,
                "All must not publish or run a partial two-codec score"
            );
            assert_eq!(
                current.counters.queued_artifacts, 0,
                "preflight rejects before optional rendering when no complete score can fit"
            );
        } else if probes <= 8 {
            assert!(
                current.counters.queued_artifacts > current.counters.scoring_events,
                "populated pending work must be abandoned when the scoring allowance ends"
            );
        }
        if probes == 8 {
            let repeated = run(Fixture::Factory, &configured, Objectives::All);
            assert_eq!(current.observed, repeated.observed);
            assert_eq!(current.counters, repeated.counters);
            assert_eq!(current.stopped, repeated.stopped);
        }
        if let Some(previous) = previous {
            assert!(
                current.observed.starts_with(&previous.observed),
                "changing only probe capacity must preserve the committed scoring prefix"
            );
            for index in 0..3 {
                assert!(
                    current.winners[index]
                        .as_ref()
                        .unwrap()
                        .sizes
                        .get(CODECS[index])
                        <= previous.winners[index]
                            .as_ref()
                            .unwrap()
                            .sizes
                            .get(CODECS[index])
                );
            }
        }
        previous = Some(current);
    }
}

#[test]
fn raw_only_search_bypasses_the_codec_queue_under_both_schedules() {
    let mut reports = Vec::new();
    for schedule in ["immediate", "staged"] {
        let configured = policy(schedule, 0, 24, 8, 4, Fixture::Factory, true);
        let result = run(
            Fixture::Factory,
            &configured,
            Objectives::One(Objective::Raw),
        );
        assert_eq!(result.counters.codec_probes, 0);
        assert_eq!(result.counters.queued_artifacts, 0);
        assert_eq!(result.counters.pending_peak, 0);
        assert_eq!(result.counters.pressure_scores, 0);
        assert_eq!(result.counters.diversity_scores, 0);
        assert!(result
            .observed
            .iter()
            .all(|entry| entry.sizes.gzip9.is_none() && entry.sizes.brotli11.is_none()));
        reports.push(result);
    }
    assert_eq!(union(&reports[0]), union(&reports[1]));
    assert_eq!(reports[0].winners[0], reports[1].winners[0]);
}
