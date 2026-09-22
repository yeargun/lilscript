//! Discovery regressions share the real source, artifact and budget harness.
use super::*;

#[test]
fn private_product_transport_search_is_independent_and_retries_under_stronger_caps() {
    let text = "struct Point{int x;int y;}int step(Point p){p.x+=3;return p.x+p.y;}Point state=Point{1,2};print(step(state));print(state.x);";
    for (enabled, proof_work, expected_proofs, structures, truncated) in [
        (false, 1_000_000, 0, 1, 0),
        (true, 0, 1, 1, 1),
        (true, 1_000_000, 1, 2, 0),
    ] {
        let policy = policy(
            "candidate_proposal_limit=8\nterminal_codec_probe_limit=24\ncandidate_beam_width=4",
            &format!(
                "call-specialization='{}'\nscalar-replacement='off'\ninlining='off'\nconstant-folding='off'\nstring-pooling='off'\nidentifier-mangling='off'\nnaming-search='off'",
                if enabled { "on" } else { "off" },
            ),
        );
        with_source(text, true, WORK, MEMORY, |compiler, source| {
            let mut plans = request();
            plans.scalar.max_work = proof_work;
            plans.facts_cache = CacheLimits {
                entries: 0,
                bytes: 0,
                result_bytes: 0,
            };
            let mut measured = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &policy, plans, |entry| {
                    run(entry.javascript, "", "", "6\n1\n");
                    measured.push(observe(entry));
                })
                .unwrap();
            assert!(search.stopped().is_none(), "{:?}", search.stopped());
            let counts = search.counters();
            assert_eq!(counts.proof_queries, expected_proofs, "{counts:?}");
            assert_eq!(counts.structures, structures, "{counts:?}");
            assert_eq!(counts.truncated_proofs, truncated, "{counts:?}");
            assert_eq!(counts.unknown_proofs, 0, "{counts:?}");
            assert_eq!(measured.len(), structures);
            winners(&search, &measured);
        });
    }
}

#[test]
fn one_closed_product_proof_covers_its_copy_hints_without_a_facts_cache() {
    let policy = policy(
        "candidate_proposal_limit=16\nterminal_codec_probe_limit=48\ncandidate_beam_width=8",
        "scalar-replacement='on'\ninlining='off'\nconstant-folding='off'\nstring-pooling='off'\nidentifier-mangling='off'\nnaming-search='off'",
    );
    for (text, expected, components, equivalents, structures) in [
        ("struct Point{int x;int y;}Point a=Point{1,2};Point b=a;Point c=b;b.x=9;c.y=7;print(a.x);print(b.x);print(c.x);print(c.y);", "1\n9\n1\n7\n", 1, 2, 2),
        ("struct Point{int x;int y;}Point a=Point{1,2};Point b=Point{3,4};Point c=a;Point d=b;c.x=9;d.y=7;print(a.x);print(c.x);print(b.y);print(d.y);", "1\n9\n4\n7\n", 2, 2, 4),
    ] {
    with_source(text, true, WORK, MEMORY, |compiler, source| {
        let mut plans = request();
        plans.facts_cache = CacheLimits {
            entries: 0,
            bytes: 0,
            result_bytes: 0,
        };
        let mut measured = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, plans, |entry| {
                run(entry.javascript, "", "", expected);
                measured.push(observe(entry));
            })
            .unwrap();
        assert!(search.stopped().is_none(), "{:?}", search.stopped());
        let counters = search.counters();
        assert_eq!(counters.proof_queries, components, "{counters:?}");
        assert_eq!(counters.equivalent_product_hints, equivalents);
        assert_eq!(counters.unknown_proofs, 0);
        assert_eq!(counters.truncated_proofs, 0);
        assert_eq!(counters.structures, structures);
        assert_eq!(counters.proposals, structures - 1);
        assert_eq!(measured.len(), structures);
        winners(&search, &measured);
    });
    }
}

#[test]
fn one_artifact_attempt_can_reach_a_literal_after_many_unavailable_helpers() {
    let mut text = String::new();
    for index in 0..24 {
        text.push_str(&format!("export int identity{index}(int x){{return x;}}\n"));
    }
    text.push_str("print(\"left\"+\"right\");");
    let policy = policy(
        "candidate_proposal_limit=1\nterminal_codec_probe_limit=8",
        "inlining='on'\nconstant-folding='on'\nstring-pooling='off'\nidentifier-mangling='off'\nnaming-search='off'\ntarget-compaction='off'",
    );
    // Identify and publish the choice from semantic operations. The expected
    // recipe does not come from search IDs, fingerprints or a source-text test.
    let (expected_descriptor, expected) =
        with_source(&text, false, WORK, MEMORY, |compiler, source| {
            let definition = compiler
                .with_semantic(source, |program, _, _| {
                    let mut definitions = Vec::new();
                    for (unit, body) in program.units.iter().enumerate() {
                        for (value, data) in body.data().values.iter().enumerate() {
                            if matches!(program.types[data.ty.index()], Type::String)
                                && !matches!(
                                    body.data().operations[data.definition.index()].kind,
                                    OperationKind::Constant(_)
                                )
                            {
                                definitions.push(ValueRef {
                                    unit: UnitId::from_index(unit).unwrap(),
                                    value: ValueId::from_index(value).unwrap(),
                                });
                            }
                        }
                    }
                    assert_eq!(definitions.len(), 1);
                    definitions[0]
                })
                .unwrap();
            compiler
                .enable_local_facts(request().facts_cache, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let literal = match compiler
                .represent_string_javascript(
                    direct,
                    &[definition],
                    StringChoice::LiteralAtDefinition,
                    request().string,
                    &policy,
                    WorkDomain::Optional,
                )
                .unwrap()
                .outcome
            {
                StringOutcome::Published(candidate) => candidate,
                other => panic!("known concatenation must qualify: {other:?}"),
            };
            let descriptor = compiler
                .with_recipe_descriptor(literal, WorkDomain::Optional, |words| words.to_vec())
                .unwrap();
            (descriptor, emit(compiler, literal, &policy, Style::Source))
        });
    with_source(&text, true, WORK, MEMORY, |compiler, source| {
        let mut measured = Vec::new();
        let mut descriptors = Vec::new();
        let search = compiler
            .search_javascript_observed(source, &policy, request(), |entry| {
                run(entry.javascript, "", "", "leftright\n");
                descriptors.push(
                    entry
                        .recipe_descriptor
                        .whole_words()
                        .expect("Whole cohort")
                        .to_vec(),
                );
                measured.push(observe(entry));
            })
            .unwrap();
        let counters = search.counters();
        assert!(!counters.inventory_truncated);
        assert!(counters.unknown_proofs >= 24);
        assert!(counters.proof_queries > counters.proposals);
        assert_eq!(counters.proposals, 1);
        assert_eq!(counters.renders, 2);
        assert!(
            descriptors.iter().zip(&measured).any(|(words, entry)| {
                *words == expected_descriptor
                    && entry.plan == expected.plan
                    && entry.javascript == expected.javascript
                    && entry.sizes == expected.sizes
            }),
            "one artifact slot must remain after unrelated failed proofs: {counters:?}"
        );
        winners(&search, &measured);
    });
}

#[test]
fn truncated_queries_are_skipped_within_the_request_and_retried_with_stronger_bounds() {
    let policy = enabled(
        "candidate_proposal_limit=384\nterminal_codec_probe_limit=384\ncandidate_beam_width=12",
    );
    let run_search = |truncated: bool| {
        with_source(FACTORY, true, WORK, MEMORY, |compiler, source| {
            let mut plans = request();
            if truncated {
                plans.helper.max_work = 0;
            }
            let mut measured = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &policy, plans, |entry| {
                    run_factory(entry.javascript);
                    measured.push(observe(entry));
                })
                .unwrap();
            assert!(search.stopped().is_none(), "{:?}", search.stopped());
            winners(&search, &measured);
            (search.counters(), measured)
        })
    };
    let (limited, limited_artifacts) = run_search(true);
    let (strong, strong_artifacts) = run_search(false);
    assert!(limited.truncated_proofs > 0);
    assert!(
        limited.skipped_truncated > 0,
        "retained siblings must skip already attempted bounds: {limited:?}"
    );
    assert_eq!(strong.truncated_proofs, 0);
    assert_eq!(strong.skipped_truncated, 0);
    assert!(strong.structures > limited.structures);
    assert!(strong_artifacts.iter().any(|entry| !limited_artifacts
        .iter()
        .any(|old| entry.plan == old.plan && entry.javascript == old.javascript)));
    // Search owns a fixed request. This checks a fresh stronger search, not
    // adaptive retries inside one run or across a semantic revision.
}

#[test]
fn cold_low_search_and_high_warmed_low_search_keep_exact_winners_and_request_charges() {
    use crate::compilation_policy::AnalysisCompletion;

    const SOURCE: &str = "export string word(){return \"left\"+\"right\";}";
    #[derive(Clone, Debug, PartialEq, Eq)]
    struct Row {
        descriptor: Vec<u32>,
        naming: Plan,
        output: OutputTactics,
        javascript: String,
        sizes: [usize; 3],
        baseline: bool,
    }
    let policy = policy(
        "candidate_proposal_limit=16\nterminal_codec_probe_limit=48\ncandidate_beam_width=4",
        "scalar-replacement='off'\ncall-specialization='off'\ninlining='off'\nconstant-folding='on'\nstring-pooling='off'\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='off'",
    );
    let strong = request().string.local_facts;
    let weak = LocalFactsRequest {
        work_quota: 1,
        ..strong
    };
    let run_search = |warm: bool, facts_request: LocalFactsRequest| {
        with_source(SOURCE, true, WORK, MEMORY, |compiler, source| {
            let definition = compiler
                .with_semantic(source, |program, _, _| {
                    let mut definitions = Vec::new();
                    for (unit, body) in program.units.iter().enumerate() {
                        for operation in &body.data().operations {
                            if matches!(operation.kind, OperationKind::Binary(BinaryOp::Add)) {
                                definitions.push(ValueRef {
                                    unit: UnitId::from_index(unit).unwrap(),
                                    value: operation.result.unwrap(),
                                });
                            }
                        }
                    }
                    assert_eq!(definitions.len(), 1);
                    definitions[0]
                })
                .unwrap();
            let mut plans = request();
            plans.string.local_facts = facts_request;
            plans.helper.local_facts = facts_request;
            // Both arms explicitly own the same empty cache capacity. Warming
            // never hides a cache-enable branch or its work inside subtraction.
            compiler
                .enable_local_facts(plans.facts_cache, WorkDomain::Baseline)
                .unwrap();
            assert_eq!(compiler.local_facts_status().unwrap().entries, 0);
            let common_setup = compiler.ledger().work_used(WorkDomain::Baseline);
            let warm_before = common_setup;
            if warm {
                compiler
                    .with_local_facts(WorkDomain::Baseline, 1, |facts| {
                        let result = facts.query(source, definition.unit, strong).unwrap();
                        assert!(!result.cache_hit());
                        assert_eq!(result.receipt().completion, AnalysisCompletion::Complete);
                        assert_eq!(
                            result.string(definition.value).unwrap().as_unicode(),
                            Some("leftright")
                        );
                    })
                    .unwrap();
                assert!(compiler.local_facts_status().unwrap().entries > 0);
            }
            let warm_setup = compiler.ledger().work_used(WorkDomain::Baseline) - warm_before;
            assert_eq!(warm_setup > 0, warm);
            let before = [WorkDomain::Baseline, WorkDomain::Optional]
                .map(|domain| compiler.ledger().work_used(domain));
            let mut observed = Vec::new();
            let search = compiler
                .search_javascript_observed(source, &policy, plans, |entry| {
                    assert!(entry.dependency.is_none());
                    run(
                        entry.javascript,
                        "",
                        "console.log(library.word());",
                        "leftright\n",
                    );
                    assert_eq!(sizes(entry.sizes), exact_sizes(entry.javascript));
                    observed.push((
                        entry.candidate,
                        Row {
                            descriptor: entry
                                .recipe_descriptor
                                .whole_words()
                                .expect("Whole cohort")
                                .to_vec(),
                            naming: entry.naming.clone(),
                            output: entry.output,
                            javascript: entry.javascript.to_owned(),
                            sizes: sizes(entry.sizes),
                            baseline: entry.baseline,
                        },
                    ));
                })
                .unwrap();
            assert!(search.stopped().is_none(), "{:?}", search.stopped());
            let counters = search.counters();
            assert!(counters.proof_queries > 0);
            let selected: [Row; 3] = std::array::from_fn(|index| {
                search
                    .with_winner(CODECS[index], |view, naming| {
                        assert!(view.dependency.is_none());
                        let matching: Vec<_> = observed
                            .iter()
                            .filter(|(candidate, row)| {
                                *candidate == view.candidate
                                    && row.naming == *naming
                                    && row.output == view.output
                                    && row.javascript == view.javascript
                                    && row.sizes == sizes(view.sizes)
                            })
                            .collect();
                        assert_eq!(
                            matching.len(),
                            1,
                            "winner retains its exact observed recipe"
                        );
                        assert_eq!(
                            view.sizes.get(CODECS[index]).unwrap(),
                            observed
                                .iter()
                                .map(|(_, row)| row.sizes[index])
                                .min()
                                .unwrap()
                        );
                        matching[0].1.clone()
                    })
                    .unwrap()
            });
            let charged = [WorkDomain::Baseline, WorkDomain::Optional]
                .map(|domain| search.ledger().work_used(domain));
            let request_work = [charged[0] - before[0], charged[1] - before[1]];
            drop(search);
            // This separate query verifies the semantic answer requested by
            // search. It is outside the measured search interval, with its own
            // full logical bill even when the search just cached this attempt.
            let facts_before = compiler.ledger().work_used(WorkDomain::Optional);
            let (answer, receipt, hit) = compiler
                .with_local_facts(WorkDomain::Optional, 1, |facts| {
                    let result = facts.query(source, definition.unit, facts_request).unwrap();
                    (
                        result
                            .string(definition.value)
                            .map(|value| value.as_unicode().unwrap().to_owned()),
                        result.receipt(),
                        result.cache_hit(),
                    )
                })
                .unwrap();
            assert!(hit, "search must have queried the actual qualified attempt");
            let facts_work = compiler.ledger().work_used(WorkDomain::Optional) - facts_before;
            assert_eq!(facts_work, receipt.logical_work);
            (
                observed.into_iter().map(|(_, row)| row).collect::<Vec<_>>(),
                selected,
                counters,
                request_work,
                (answer, receipt, facts_work),
                common_setup,
                warm_setup,
            )
        })
    };
    let cold = run_search(false, weak);
    let warmed = run_search(true, weak);
    assert_eq!(
        cold.0, warmed.0,
        "all complete observed recipes, outputs and scores"
    );
    assert_eq!(
        cold.1, warmed.1,
        "three independently selected complete winners"
    );
    assert_eq!(
        cold.2, warmed.2,
        "cache warmth cannot change low-effort discovery"
    );
    assert_eq!(
        cold.3, warmed.3,
        "all per-request baseline and optional work"
    );
    assert_eq!(
        cold.4, warmed.4,
        "semantic answer, attempt receipt and query charge"
    );
    assert_eq!(
        cold.5, warmed.5,
        "the common empty-owner setup is identical"
    );
    assert_eq!(cold.6, 0);
    assert!(warmed.6 > 0);
    assert_eq!(cold.4 .0, None);
    assert_eq!(cold.4 .1.completion, AnalysisCompletion::Truncated);
    assert!(cold.2.truncated_proofs > 0);
    let complete = run_search(false, strong);
    assert_eq!(complete.4 .0.as_deref(), Some("leftright"));
    assert_eq!(complete.4 .1.completion, AnalysisCompletion::Complete);
    assert_eq!(complete.2.truncated_proofs, 0);
    assert!(
        complete.2.structures > cold.2.structures,
        "strong knowledge enables a real physical choice"
    );
    assert!(complete
        .0
        .iter()
        .any(|row| !cold.0.iter().any(|old| old.descriptor == row.descriptor)));
    eprintln!("search-cache-effort-parity common_setup={} cold_request={:?} warm_setup={} warmed_request={:?} cold_query={} warmed_query={}", cold.5, cold.3, warmed.6, warmed.3, cold.4.2, warmed.4.2);
}
