//! Real complete-artifact search oracles. The independent Cartesian product
//! uses the existing family publishers, never the scheduler or its comparator.
//! Test-owned observations are copied after the compiler's explicit handoff.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Objective, Objectives, Plan, Sizes, Style};
use std::process::Command;

#[path = "search_discovery_tests.rs"]
mod discovery;

#[path = "artifact_identity_lifetime_tests.rs"]
mod artifact_identity;

const WORK: u64 = 200_000_000;
const MEMORY: u64 = 128_000_000;
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
// `+2-1` rather than `+1`: this spelling keeps a real codec crossover (the
// scoped naming wins gzip, the global one Brotli), which these tests need.
const BYTE: &str = "export int byte(int value){return(value&255)+1;}";
const FACTORY: &str = include_str!("fixtures/value-placement/representation-composition.lil");
const FACTORY_SETUP: &str =
    include_str!("fixtures/value-placement/representation-composition.setup.js");
const FACTORY_EXPECTED: &str =
    include_str!("fixtures/value-placement/representation-composition.expected.out");

fn policy(javascript: &str, tactics: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n{javascript}\n[policy.tactics]\n{tactics}"
    ))
    .unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn enabled(javascript: &str) -> ResolvedPolicy {
    policy(
        javascript,
        "scalar-replacement='on'\ninlining='on'\nconstant-folding='on'\nstring-pooling='on'\nidentifier-mangling='on'\nnaming-search='on'",
    )
}
fn byte_policy(javascript: &str) -> ResolvedPolicy {
    policy(
        javascript,
        "target-compaction='off'\nidentifier-mangling='on'\nnaming-search='on'",
    )
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
            entries: 32,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    }
}
fn with_source<R>(
    source: &str,
    baseline_first: bool,
    work: u64,
    memory: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let ledger = if baseline_first {
        BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: work,
                retained_bytes: memory,
                terminal_work: 0,
            },
        )
    } else {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: work / 2,
                optional_work: work / 2,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
    }
    .unwrap();
    // Inventory proof seeds, frontier siblings and objective incumbents coexist.
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 128 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compiler, source);
    assert_eq!(compiler.finish().retained_bytes(), 0);
    result
}

#[derive(Debug, Clone)]
struct Artifact {
    axes: Option<OracleAxes>,
    recipe: u64,
    plan: Plan,
    javascript: String,
    sizes: [usize; 3],
    baseline: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OracleAxes {
    record: &'static str,
    helper: &'static str,
    string: &'static str,
}
fn oracle_at(oracle: &[Artifact], axes: OracleAxes, style: Style) -> &Artifact {
    let mut matching = oracle
        .iter()
        .filter(|entry| entry.axes == Some(axes) && entry.plan.style == style);
    let found = matching
        .next()
        .expect("independently published oracle state");
    assert!(
        matching.next().is_none(),
        "oracle axis/plan labels must be unique"
    );
    found
}
fn exact_sizes(javascript: &str) -> [usize; 3] {
    CODECS.map(|codec| crate::compression::measure(javascript.as_bytes(), codec).unwrap())
}
fn sizes(values: Sizes) -> [usize; 3] {
    CODECS.map(|codec| values.get(codec).expect("all requested codecs are scored"))
}
fn observe(value: SearchObservation<'_>) -> Artifact {
    let measured = sizes(value.sizes);
    assert_eq!(
        measured,
        exact_sizes(value.javascript),
        "complete bytes own their scores"
    );
    Artifact {
        axes: None,
        recipe: value.recipe_fingerprint,
        plan: value.naming.clone(),
        javascript: value.javascript.into(),
        sizes: measured,
        baseline: value.baseline,
    }
}

fn receipt(case: &str, producer: &str, axes: &str, value: &Artifact) {
    eprintln!(
        "search-artifact {}",
        serde_json::json!({
            "case": case, "producer": producer, "structural_axes": axes,
            "recipe_fingerprint": value.recipe, "naming": format!("{:?}", value.plan.style),
            "javascript": value.javascript, "raw": value.sizes[0], "gzip9": value.sizes[1],
            "brotli11": value.sizes[2], "baseline": value.baseline,
        })
    );
}
fn winners(search: &JavaScriptSearch<'_, '_>, observed: &[Artifact]) -> [Artifact; 3] {
    assert!(!observed.is_empty());
    std::array::from_fn(|index| {
        let winner = search
            .with_winner(CODECS[index], |view, plan| Artifact {
                axes: None,
                recipe: 0,
                plan: plan.clone(),
                javascript: view.javascript.into(),
                sizes: sizes(view.sizes),
                baseline: false,
            })
            .expect("requested codec retains a complete winner");
        assert_eq!(winner.sizes, exact_sizes(&winner.javascript));
        assert_eq!(
            winner.sizes[index],
            observed
                .iter()
                .map(|entry| entry.sizes[index])
                .min()
                .unwrap(),
            "selection is independently checked over the eligible measured union"
        );
        assert!(observed
            .iter()
            .any(|entry| entry.plan == winner.plan && entry.javascript == winner.javascript));
        winner
    })
}
fn emit(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> Artifact {
    // The candidate owns its shared recipe backing independently of this
    // terminal output attempt; only output storage must return to this baseline.
    compiler
        .with_implementation_description(candidate, WorkDomain::Optional, |_| ())
        .unwrap();
    let before = compiler.ledger().retained_bytes();
    let (javascript, measured) = compiler
        .with_javascript_output_in(candidate, policy, WorkDomain::Optional, |output| {
            let artifact = output.render(&Plan::new(style))?;
            for codec in CODECS {
                output.measure(artifact, codec)?;
            }
            let measured = output.with_artifact(artifact, |view| sizes(view.sizes))?;
            Ok::<_, CandidateError>((output.take_artifact(artifact)?, measured))
        })
        .unwrap()
        .unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), before);
    assert_eq!(measured, exact_sizes(&javascript));
    Artifact {
        axes: None,
        recipe: 0,
        plan: Plan::new(style),
        javascript,
        sizes: measured,
        baseline: false,
    }
}
fn run(javascript: &str, setup: &str, observation: &str, expected: &str) {
    let script = format!(
        "{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{observation}",
        serde_json::to_string(javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for search observations");
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
fn run_factory(javascript: &str) {
    run(
        javascript,
        FACTORY_SETUP,
        "await globalThis.valuePlacementObserve(library);",
        FACTORY_EXPECTED,
    );
}
fn run_byte(javascript: &str) {
    run(
        javascript,
        "",
        r#"
        const trace=[library.byte.name,library.byte.length];
        for(const value of [-1,0,-0,255,256,2147483647,2147483648,4294967297])trace.push(library.byte(value));
        trace.push(library.byte({valueOf(){trace.push('coerce');return 511;}}));
        const failure={};try{library.byte({valueOf(){trace.push('throw-coerce');throw failure;}});}catch(error){trace.push(error===failure);}
        trace.push(Object.is(library.byte(-0),-0));console.log(JSON.stringify(trace));
    "#,
        "[\"byte\",1,256,1,1,256,1,256,1,2,\"coerce\",256,\"throw-coerce\",true,false]\n",
    );
}

#[test]
fn real_gzip_and_brotli_winners_survive_independently() {
    let policy = byte_policy("candidate_proposal_limit=384\nterminal_codec_probe_limit=384");
    let oracle = with_source(BYTE, false, WORK, MEMORY, |compiler, source| {
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        STYLES.map(|style| emit(compiler, direct, &policy, style))
    });
    assert!(
        oracle[1].sizes[1] < oracle[0].sizes[1],
        "fixture must retain its actual gzip crossover: {oracle:?}"
    );
    assert!(
        oracle[0].sizes[2] < oracle[1].sizes[2],
        "fixture must retain its actual Brotli crossover: {oracle:?}"
    );
    for entry in &oracle {
        receipt(
            "independent-codecs",
            "oracle",
            "direct/compaction-off",
            entry,
        );
        run_byte(&entry.javascript);
    }
    with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
        let mut measured = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, request(), |entry| {
                let entry = observe(entry);
                receipt(
                    "independent-codecs",
                    "search",
                    "direct/compaction-off",
                    &entry,
                );
                measured.push(entry)
            })
            .unwrap();
        let chosen = winners(&search, &measured);
        for index in 0..3 {
            assert_eq!(
                chosen[index].sizes[index],
                oracle.iter().map(|entry| entry.sizes[index]).min().unwrap()
            );
            run_byte(&chosen[index].javascript);
        }
        assert_eq!(chosen[1].plan.style, Style::Scoped);
        assert_eq!(chosen[2].plan.style, Style::Global);
        assert_ne!(chosen[1].javascript, chosen[2].javascript);
    });
}

#[test]
fn identical_naming_outputs_keep_distinct_trials_without_repeating_codec_probes() {
    for schedule in ["immediate", "staged"] {
        let policy = policy(
            &format!(
                "candidate_proposal_limit=8\nterminal_codec_probe_limit=4\n[policy.search]\ncodec_schedule='{schedule}'"
            ),
            "target-compaction='on'\nidentifier-mangling='on'\nnaming-search='on'",
        );
        // Two locals survive compaction: global and scoped naming spell them
        // alike, while source naming keeps `total` and `step`.
        with_source(
            "export int answer(){int total=0;for(int step=0;step<17;step++){total++;}return total;}",
            true,
            WORK,
            MEMORY,
            |compiler, source| {
                let mut measured = Vec::new();
                let search = compiler
                    .search_javascript_observed(source, &policy, request(), |entry| {
                        measured.push(observe(entry));
                    })
                    .unwrap();
                assert!(
                    search.stopped().is_none(),
                    "{schedule}: {:?}; {:?}; {measured:?}",
                    search.stopped(),
                    search.counters(),
                );
                assert_eq!(search.counters().renders, 3, "{schedule}");
                assert_eq!(search.counters().codec_probes, 2, "{schedule}");
                assert_eq!(measured.len(), 3, "{schedule}");
                for style in STYLES {
                    assert_eq!(
                        measured
                            .iter()
                            .filter(|entry| entry.plan.style == style)
                            .count(),
                        1,
                        "equal bytes must not erase a trial's naming provenance"
                    );
                }
                let by_style = |style| {
                    measured
                        .iter()
                        .find(|entry| entry.plan.style == style)
                        .unwrap()
                };
                let global = by_style(Style::Global);
                let scoped = by_style(Style::Scoped);
                let source = by_style(Style::Source);
                assert_eq!(global.javascript, scoped.javascript);
                assert_eq!(global.sizes, scoped.sizes);
                assert_ne!(global.javascript, source.javascript);
                for entry in &measured {
                    receipt("exact-score-reuse", schedule, "direct", entry);
                }
                for winner in winners(&search, &measured) {
                    run(
                        &winner.javascript,
                        "",
                        "console.log(JSON.stringify([library.answer.name,library.answer.length,library.answer()]));",
                        "[\"answer\",0,17]\n",
                    );
                }
            },
        );
    }
}

#[test]
fn zero_optional_work_seals_a_completely_scored_direct_baseline() {
    for settings in ["candidate_proposal_limit=0", "candidate_search='off'"] {
        let policy = byte_policy(settings);
        with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
            let mut measured = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &policy, request(), |entry| {
                    measured.push(observe(entry))
                })
                .unwrap();
            assert_eq!(measured.len(), 1);
            assert!(measured[0].baseline);
            assert_eq!(search.counters().proposals, 0);
            assert_eq!(search.counters().proof_queries, 0);
            assert_eq!(search.counters().codec_probes, 0);
            assert!(search.stopped().is_none());
            assert!(search.ledger().baseline_is_sealed());
            assert_eq!(search.ledger().work_used(WorkDomain::Optional), 0);
            assert_eq!(search.ledger().retained_bytes_in(WorkDomain::Optional), 0);
            let seal = search.baseline_seal();
            assert!(seal.baseline_retained_bytes > 0);
            assert_eq!(
                seal.baseline_work,
                search.ledger().work_used(WorkDomain::Baseline)
            );
            for winner in winners(&search, &measured) {
                run_byte(&winner.javascript);
            }
        });
    }
}

fn factory_oracle(policy: &ResolvedPolicy) -> Vec<Artifact> {
    factory_oracle_for(FACTORY, policy)
}
fn factory_oracle_for(fixture: &str, policy: &ResolvedPolicy) -> Vec<Artifact> {
    with_source(fixture, false, WORK, MEMORY, |compiler, source| {
        let (state, helper, definitions) = compiler
            .with_semantic(source, |program, _, _| {
                let cell = |name| {
                    CellId::from_index(
                        program
                            .cells
                            .iter()
                            .position(|cell| cell.name == name)
                            .unwrap(),
                    )
                    .unwrap()
                };
                let mut definitions = Vec::new();
                for (unit, data) in program.units.iter().enumerate() {
                    for operation in &data.data().operations {
                        if matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                            && operation.result.is_some_and(|value| {
                                matches!(
                                    program.types[data.data().values[value.index()].ty.index()],
                                    Type::String
                                )
                            })
                        {
                            definitions.push(ValueRef {
                                unit: UnitId::from_index(unit).unwrap(),
                                value: operation.result.unwrap(),
                            });
                        }
                    }
                }
                (cell("state"), cell("createLabel"), definitions)
            })
            .unwrap();
        assert_eq!(definitions.len(), 1);
        let plans = request();
        compiler
            .enable_local_facts(plans.facts_cache, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, policy, WorkDomain::Baseline)
            .unwrap();
        let scalar = match compiler
            .scalar_javascript(direct, state, plans.scalar, policy, WorkDomain::Optional)
            .unwrap()
            .outcome
        {
            ScalarOutcome::Published(candidate) => candidate,
            other => panic!("oracle record representation must be proved: {other:?}"),
        };
        let mut result = Vec::new();
        for (record, base) in [("record", direct), ("scalar", scalar)] {
            let inline = match compiler
                .inline_helper_javascript(base, helper, plans.helper, policy, WorkDomain::Optional)
                .unwrap()
                .outcome
            {
                HelperOutcome::Published(candidate) => candidate,
                other => panic!("oracle helper representation must be proved: {other:?}"),
            };
            for (helper, base) in [("callable", base), ("inline", inline)] {
                let representations = [
                    StringChoice::LiteralAtDefinition,
                    StringChoice::SharedLiteral {
                        activation: definitions[0].unit,
                    },
                ]
                .map(|choice| {
                    match compiler
                        .represent_string_javascript(
                            base,
                            &definitions,
                            choice,
                            plans.string,
                            policy,
                            WorkDomain::Optional,
                        )
                        .unwrap()
                        .outcome
                    {
                        StringOutcome::Published(candidate) => candidate,
                        other => panic!("oracle string representation must be proved: {other:?}"),
                    }
                });
                for (string, candidate) in [
                    ("computed", base),
                    ("literal", representations[0]),
                    ("shared", representations[1]),
                ] {
                    for style in STYLES {
                        let mut artifact = emit(compiler, candidate, policy, style);
                        artifact.axes = Some(OracleAxes {
                            record,
                            helper,
                            string,
                        });
                        if fixture == FACTORY {
                            run_factory(&artifact.javascript);
                        }
                        receipt(
                            "factory",
                            "oracle",
                            &format!("{record}/{helper}/{string}"),
                            &artifact,
                        );
                        result.push(artifact);
                    }
                }
            }
        }
        assert_eq!(result.len(), 36);
        result
    })
}

#[test]
#[ignore = "its measured Brotli interaction did not survive 008 printing; 010 re-derives the interaction trap"]
fn structural_discovery_matches_independent_twelve_state_oracle_and_measured_union() {
    let policy = enabled(
        "candidate_proposal_limit=384\nterminal_codec_probe_limit=384\ncandidate_beam_width=12",
    );
    let oracle = factory_oracle(&policy);
    // This measured four-point local minimum couples naming to a string
    // representation. It does not prove that structural beam search expanded
    // a temporarily losing parent: the literal recipe's best naming already
    // improves its parent. No source spelling recognizer chooses these states.
    let computed = OracleAxes {
        record: "scalar",
        helper: "callable",
        string: "computed",
    };
    let literal = OracleAxes {
        string: "literal",
        ..computed
    };
    let base = oracle_at(&oracle, computed, Style::Scoped).sizes[2];
    let literal_only = oracle_at(&oracle, literal, Style::Scoped).sizes[2];
    let naming_only = oracle_at(&oracle, computed, Style::Global).sizes[2];
    let combined = oracle_at(&oracle, literal, Style::Global).sizes[2];
    assert!(
        literal_only > base && naming_only > base && combined < base,
        "real Brotli interaction must survive current formation: base={base}, literal={literal_only}, naming={naming_only}, combined={combined}"
    );
    with_source(FACTORY, true, WORK, MEMORY, |compiler, source| {
        let revisions = compiler
            .with_semantic(source, |program, _, _| {
                program
                    .units
                    .iter()
                    .map(FrozenUnit::revision)
                    .collect::<Vec<_>>()
            })
            .unwrap();
        let mut measured = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, request(), |entry| {
                let entry = observe(entry);
                // Keep the logged fixture matrix bounded even if a broader
                // inventory later discovers additional valid representations.
                if measured.len() < 36 {
                    receipt("factory", "search", "recipe-fingerprint", &entry);
                }
                measured.push(entry)
            })
            .unwrap();
        let chosen = winners(&search, &measured);
        for index in 0..3 {
            assert!(
                chosen[index].sizes[index]
                    <= oracle.iter().map(|entry| entry.sizes[index]).min().unwrap(),
                "bounded discovery lost the independent oracle optimum: {:?}",
                search.counters()
            );
            run_factory(&chosen[index].javascript);
        }
        // Byte/plan coverage is independent of scheduler-issued candidate IDs.
        for entry in &oracle {
            assert!(
                measured
                    .iter()
                    .any(|found| found.plan == entry.plan && found.javascript == entry.javascript),
                "missing oracle artifact {:?}/{:?}; counters={:?}, stop={:?}",
                entry.plan,
                entry.sizes,
                search.counters(),
                search.stopped()
            );
        }
        let unique = measured
            .iter()
            .map(|entry| entry.recipe)
            .collect::<std::collections::BTreeSet<_>>();
        assert!(
            unique.len() >= 12,
            "selected structural identity must include all recipe combinations"
        );
        assert!(
            search.counters().codec_probes <= policy.objective().unwrap().optional_codec_probes
        );
        let counters = search.counters();
        eprintln!(
            "search-summary {}",
            serde_json::json!({
                "case": "factory", "proposals": counters.proposals,
                "proof_queries": counters.proof_queries, "unknown_proofs": counters.unknown_proofs,
                "truncated_proofs": counters.truncated_proofs, "inventory_truncated": counters.inventory_truncated,
                "conflicting_choices": counters.conflicting_choices, "duplicate_states": counters.duplicate_states,
                "structures": counters.structures, "renders": counters.renders,
                "codec_probes": counters.codec_probes, "admitted_artifacts": counters.admitted_artifacts,
                "beam_evictions": counters.beam_evictions,
                "stop": search.stopped().map(|reason| format!("{reason:?}")),
                "best_sizes": [chosen[0].sizes[0], chosen[1].sizes[1], chosen[2].sizes[2]],
                "measured_count": measured.len(), "unique_recipe_hash_count": unique.len(),
                "brotli_naming_literal_local_minimum": {
                    "base": base, "literal_only": literal_only, "naming_only": naming_only, "combined": combined,
                },
            })
        );
        drop(search);
        let after = compiler
            .with_semantic(source, |program, _, _| {
                program
                    .units
                    .iter()
                    .map(FrozenUnit::revision)
                    .collect::<Vec<_>>()
            })
            .unwrap();
        assert_eq!(
            after, revisions,
            "physical exploration must preserve semantic snapshots"
        );
    });
}

#[test]
fn optional_work_memory_and_probe_exhaustion_preserve_scored_incumbents() {
    let baseline_policy = enabled("candidate_proposal_limit=0");
    let (baseline_work, baseline_peak) =
        with_source(FACTORY, true, WORK, MEMORY, |compiler, source| {
            let search = compiler
                .search_javascript(source, &baseline_policy, request())
                .unwrap();
            (
                search.baseline_seal().baseline_work,
                search.ledger().peak_retained_bytes(),
            )
        });
    for kind in ["work", "memory", "probes"] {
        let policy = enabled(if kind == "probes" {
            "candidate_proposal_limit=384\nterminal_codec_probe_limit=0"
        } else {
            "candidate_proposal_limit=384\nterminal_codec_probe_limit=384"
        });
        let work = if kind == "work" { baseline_work } else { WORK };
        let memory = if kind == "memory" {
            baseline_peak
        } else {
            MEMORY
        };
        let mut request = request();
        if kind == "memory" {
            request.facts_cache.bytes = MEMORY;
        }
        with_source(FACTORY, true, work, memory, |compiler, source| {
            let mut measured = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &policy, request, |entry| {
                    measured.push(observe(entry))
                })
                .unwrap();
            match (kind, search.stopped()) {
                (
                    "work",
                    Some(SearchError::Candidate(CandidateError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Optional),
                    ))),
                ) => (),
                (
                    "memory",
                    Some(SearchError::Candidate(CandidateError::Budget(
                        BudgetError::MemoryExhausted(WorkDomain::Optional),
                    ))),
                ) => (),
                (
                    "memory",
                    Some(SearchError::Candidate(CandidateError::LocalFacts(
                        CompilationFactsError::Facts(super::facts::FactsError::Budget(
                            BudgetError::MemoryExhausted(WorkDomain::Optional),
                        )),
                    ))),
                ) => (),
                ("probes", Some(SearchError::Limit(SearchLimit::CodecProbes))) => (),
                other => panic!("expected bounded optional {kind} stop, got {other:?}"),
            }
            assert!(search.ledger().retained_bytes() <= memory);
            assert!(
                search.ledger().work_used(WorkDomain::Baseline)
                    + search.ledger().work_used(WorkDomain::Optional)
                    <= work
            );
            assert!(measured.first().unwrap().baseline);
            for winner in winners(&search, &measured) {
                run_factory(&winner.javascript);
            }
        });
    }
}

#[test]
fn panicking_observer_releases_losing_artifact_provenance_and_search_storage() {
    let policy = byte_policy("candidate_proposal_limit=384\nterminal_codec_probe_limit=384");
    for panic_at in [Style::Global, Style::Source] {
        with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
            // The JavaScript contract, retained local-facts cache and vacant slots
            // in the retained artifact arena belong to Compilation, surviving an
            // individual search. Prewarm the arena for three objective incumbents
            // plus their staged promotion before measuring search-owned cleanup.
            // with_source still requires Compilation::finish to release all bytes.
            // Staged scoring may discover proof opportunities before the losing
            // naming seed invokes this observer. Reserve that reusable owner
            // before the search-owned retention baseline as well.
            compiler
                .enable_local_facts(request().facts_cache, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let warmed = compiler
                .with_javascript_output(direct, &policy, |output| {
                    let mut retained = Vec::new();
                    for _ in 0..CODECS.len() + 1 {
                        let artifact = output.render(&Plan::new(Style::Global))?;
                        retained.push(output.retain_artifact(artifact)?);
                    }
                    Ok::<_, CandidateError>(retained)
                })
                .unwrap()
                .unwrap();
            for artifact in warmed {
                compiler.discard_artifact(artifact).unwrap();
            }
            compiler.discard(direct.semantic_id()).unwrap();
            let retained = compiler.ledger().retained_bytes();
            let mut best = [usize::MAX; 3];
            let mut reached_expected_artifact = false;
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let search = compiler
                    .search_javascript_observed(source, &policy, request(), |entry| {
                        let entry = observe(entry);
                        if entry.plan.style == panic_at {
                            if panic_at == Style::Source {
                                assert!(entry
                                    .sizes
                                    .iter()
                                    .zip(best)
                                    .all(|(size, incumbent)| *size > incumbent));
                            } else {
                                assert!(entry.baseline);
                                assert_eq!(best, [usize::MAX; 3]);
                            }
                            reached_expected_artifact = true;
                            panic!(
                                "intentional observer failure on complete {panic_at:?} artifact"
                            );
                        }
                        for index in 0..3 {
                            best[index] = best[index].min(entry.sizes[index]);
                        }
                    })
                    .unwrap();
                drop(search);
            }));
            assert!(outcome.is_err());
            assert!(reached_expected_artifact);
            assert_eq!(compiler.ledger().retained_bytes(), retained);
        });
    }
}

#[test]
fn taking_one_shared_winner_moves_its_buffer_and_consumes_all_objective_aliases() {
    let policy = byte_policy("candidate_proposal_limit=0");
    with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
        let mut search = compiler
            .search_javascript(source, &policy, request())
            .unwrap();
        let (pointer, capacity, expected) = search
            .with_winner(Objective::Raw, |view, _| {
                (
                    view.javascript.as_ptr() as usize,
                    view.retained_capacity,
                    exact_sizes(view.javascript),
                )
            })
            .unwrap();
        for codec in CODECS {
            assert_eq!(
                search.with_winner(codec, |view, _| view.javascript.as_ptr() as usize),
                Some(pointer)
            );
        }
        let retained = search.ledger().retained_bytes();
        let javascript = search.take_winner(Objective::Gzip).unwrap();
        assert_eq!(
            javascript.as_ptr() as usize,
            pointer,
            "terminal handoff must move the actual buffer"
        );
        assert_eq!(exact_sizes(&javascript), expected);
        assert_eq!(search.ledger().retained_bytes(), retained - capacity as u64);
        for codec in CODECS {
            assert!(search.with_winner(codec, |_, _| ()).is_none());
            assert!(search.take_winner(codec).is_none());
        }
        drop(search);
        run_byte(&javascript);
    });
}

#[test]
fn one_retained_artifact_allows_replacement_but_rejects_a_divergent_codec_union() {
    let policy = byte_policy(
        "candidate_limit=1\ncandidate_proposal_limit=384\nterminal_codec_probe_limit=384",
    );
    assert_eq!(policy.objective().unwrap().retained_candidates, 1);
    with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
        let mut request = request();
        request.objectives = Objectives::One(Objective::Gzip);
        let mut measured = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, request, |entry| {
                assert_eq!(entry.sizes.raw, entry.javascript.len());
                assert!(
                    entry.sizes.brotli11.is_none(),
                    "unrequested codec stays unknown"
                );
                let gzip =
                    crate::compression::measure(entry.javascript.as_bytes(), Objective::Gzip)
                        .unwrap();
                assert_eq!(entry.sizes.gzip9, Some(gzip));
                measured.push((entry.naming.style, gzip, entry.javascript.to_owned()));
            })
            .unwrap();
        let (style, gzip, javascript) = search
            .with_winner(Objective::Gzip, |view, plan| {
                assert!(view.sizes.brotli11.is_none());
                (
                    plan.style,
                    view.sizes.gzip9.unwrap(),
                    view.javascript.to_owned(),
                )
            })
            .unwrap();
        assert_eq!(measured[0].0, Style::Global);
        assert_eq!(style, Style::Scoped);
        assert!(
            gzip < measured[0].1,
            "replacement must actually improve the incumbent"
        );
        assert_eq!(gzip, measured.iter().map(|entry| entry.1).min().unwrap());
        assert!(!matches!(
            search.stopped(),
            Some(SearchError::Limit(SearchLimit::ArtifactCount))
        ));
        assert!(search.with_winner(Objective::Raw, |_, _| ()).is_none());
        assert!(search.with_winner(Objective::Brotli, |_, _| ()).is_none());
        run_byte(&javascript);
    });
    with_source(BYTE, true, WORK, MEMORY, |compiler, source| {
        let mut measured = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, request(), |entry| {
                measured.push(observe(entry))
            })
            .unwrap();
        assert!(matches!(
            search.stopped(),
            Some(SearchError::Limit(SearchLimit::ArtifactCount))
        ));
        assert_eq!(
            measured.len(),
            1,
            "failed divergent promotion must not commit an observation"
        );
        assert!(measured[0].baseline);
        assert_eq!(measured[0].plan.style, Style::Global);
        for winner in winners(&search, &measured) {
            assert_eq!(winner.javascript, measured[0].javascript);
            run_byte(&winner.javascript);
        }
    });
}

#[test]
fn invalid_ledger_and_missing_runtime_evidence_reject_before_search_work_or_storage() {
    use crate::compilation_policy::AdmissionError;
    let ordinary = byte_policy("");
    let constrained = byte_policy("[policy.constraints]\nmax_startup_work=0");
    assert_eq!(
        constrained.objective().unwrap().rank.priority,
        crate::config::JavaScriptPriority::SizeFirst
    );
    for (baseline_first, policy) in [(false, &ordinary), (true, &constrained)] {
        with_source(BYTE, baseline_first, WORK, MEMORY, |compiler, source| {
            let before = compiler.ledger().clone();
            let mut entered = false;
            let result = compiler
                .search_javascript_observed(source, policy, request(), |_| entered = true)
                .map(drop);
            if baseline_first {
                assert!(matches!(
                    result,
                    Err(SearchError::Admission(AdmissionError::MissingCostEvidence(
                        "startup work"
                    )))
                ));
            } else {
                assert!(matches!(
                    result,
                    Err(SearchError::Candidate(CandidateError::Budget(
                        BudgetError::InvalidBaselinePhase
                    )))
                ));
            }
            assert!(!entered);
            assert_eq!(
                compiler.ledger().peak_retained_bytes(),
                before.peak_retained_bytes()
            );
            for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
                assert_eq!(
                    compiler.ledger().retained_bytes_in(domain),
                    before.retained_bytes_in(domain)
                );
                assert_eq!(
                    compiler.ledger().work_used(domain),
                    before.work_used(domain)
                );
            }
        });
    }
}

