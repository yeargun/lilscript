use super::*;
use crate::config::{CandidateSearch, CompressionCostModel};

const SOURCE: &str = include_str!("program/fixtures/search-structural-valley/entry.lil");
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];

fn configuration(level: u8, extra: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "objective.codecs='brotli'\neffort.level={level}\n\
         [javascript]\ncandidate_proposal_limit=48\nterminal_codec_probe_limit=96\n\
         [policy.tactics]\ninlining='off'\nconstant-folding='off'\n{extra}"
    ))
    .unwrap()
}

fn verify(compiled: &ServiceCompilation, codec: Objective) {
    let output = compiled.javascript(codec).unwrap();
    assert_eq!(
        tests::execute_javascript(
            output.javascript(),
            "",
            "console.log(JSON.stringify([library.run(1),library.run(511),library.run(512),library.side(0),library.side(65)]));"
        ),
        "[67,193,580,12,77]\n"
    );
    assert_eq!(
        compiled.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
}

#[test]
fn independent_objectives_match_separate_policies_searches_and_execution() {
    for level in [0, 3, 13, 14] {
        for extra in ["", "identifier-mangling='off'\nproperty-mangling='off'\n"] {
            let config = configuration(level, extra);
            let together = compile_source(
                SOURCE,
                &config,
                ServiceOptions {
                    objectives: Some(Objectives::All),
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            assert_eq!(together.report()["search"]["independent"], true);
            for (index, codec) in CODECS.into_iter().enumerate() {
                let mut one_config = config.clone();
                one_config.objective.codecs = vec![codec];
                let one = compile_source(SOURCE, &one_config, ServiceOptions::default()).unwrap();
                let lane = &together.report()["search"]["objectives"][index];
                assert_eq!(lane["policy"], one.report()["javascript_policy"]);
                assert_eq!(
                    together.report()["javascript_policies"][index],
                    lane["policy"]
                );
                assert_eq!(
                    together.javascript(codec).unwrap().javascript(),
                    one.javascript(codec).unwrap().javascript(),
                    "{codec:?}, level {level}"
                );
                assert_eq!(
                    together.javascript(codec).unwrap().sizes().get(codec),
                    one.javascript(codec).unwrap().sizes().get(codec)
                );
                assert_eq!(
                    together.javascript(codec).unwrap().details()["policy_fingerprint"],
                    one.javascript(codec).unwrap().details()["policy_fingerprint"]
                );
                for field in [
                    "proposals",
                    "structures",
                    "renders",
                    "codec_probes",
                    "proof_queries",
                    "terminal",
                ] {
                    assert_eq!(
                        lane[field],
                        one.report()["search"][field],
                        "{field}, {codec:?}, level {level}"
                    );
                }
                assert_eq!(
                    together.report()["resources"]["frontend_logical_work"],
                    one.report()["resources"]["frontend_logical_work"]
                );
                verify(&together, codec);
            }
        }
    }
}

#[test]
fn objective_override_resolves_the_requested_codec_and_encoder_settings() {
    let mut config = configuration(13, "");
    config.objective.gzip.level = 2;
    config.objective.gzip.window = 10;
    config.objective.brotli.quality = 4;
    config.objective.brotli.window = 16;
    config.objective.brotli.mode = crate::compression::BrotliMode::Text;
    let together = compile_source(
        SOURCE,
        &config,
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for configured in CODECS {
        config.objective.codecs = vec![configured];
        for (index, requested) in CODECS.into_iter().enumerate() {
            let options = ServiceOptions {
                objectives: Some(Objectives::One(requested)),
                ..ServiceOptions::default()
            };
            let policy = options
                .resolve_policy(&config, options.javascript_request().unwrap())
                .unwrap();
            assert_eq!(policy.objective().unwrap().codec, requested);
            assert_eq!(
                policy.objective().unwrap().walk.margin,
                if requested == CompressionCostModel::Brotli {
                    150
                } else {
                    0
                }
            );
            let one = compile_source(SOURCE, &config, options).unwrap();
            let lane = &together.report()["search"]["objectives"][index];
            assert_eq!(one.report()["javascript_policy"], policy.receipt());
            assert_eq!(lane["policy"], policy.receipt());
            assert_eq!(lane["terminal"], one.report()["search"]["terminal"]);
            assert_eq!(
                together.javascript(requested).unwrap().javascript(),
                one.javascript(requested).unwrap().javascript()
            );
            assert_eq!(
                together
                    .javascript(requested)
                    .unwrap()
                    .sizes()
                    .get(requested),
                one.javascript(requested).unwrap().sizes().get(requested)
            );
            let output = one.javascript(requested).unwrap();
            let score = crate::compression::measure_admitted_with(
                output.javascript().as_bytes(),
                requested,
                &config.objective.settings(),
                &mut AllocationBudget::new(None),
            )
            .unwrap();
            assert_eq!(output.sizes().get(requested), Some(score));
            verify(&one, requested);
        }
    }
}

#[test]
fn independent_baselines_precede_optional_work_and_survive_exhaustion() {
    let config = configuration(13, "");
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let mut baseline_texts = Vec::new();
    let (batch, finished) = with_checked_source(SOURCE, &config, options, |session| {
        session.search_javascript(session.source(), |observation| {
            if observation.baseline {
                assert!(baseline_texts.len() < 3);
                baseline_texts.push(observation.javascript.to_owned());
            } else {
                assert_eq!(
                    baseline_texts.len(),
                    3,
                    "all incumbents must exist before exploration"
                );
            }
        })
    })
    .unwrap();
    batch.unwrap();
    assert_eq!(baseline_texts.len(), 3);
    assert_eq!(finished.ledger.retained_bytes(), 0);
    let mandatory = finished.ledger.work_used(WorkDomain::Baseline);
    // The preparation stack is Brotli, gzip, raw; search unwinds raw first.
    baseline_texts.reverse();
    for extra in [0, 3, 100, 10_000] {
        let constrained = ServiceOptions {
            logical_work: mandatory + extra,
            ..options
        };
        let result = compile_source(SOURCE, &config, constrained).unwrap();
        assert_eq!(result.report()["resources"]["baseline_work"], mandatory);
        assert!(
            result.report()["resources"]["optional_work"]
                .as_u64()
                .unwrap()
                <= extra
        );
        for (index, codec) in CODECS.into_iter().enumerate() {
            let lane = &result.report()["search"]["objectives"][index];
            assert!(
                lane["optional_work_used"].as_u64().unwrap()
                    <= lane["optional_work_allowance"].as_u64().unwrap()
            );
            if extra == 0 {
                assert_eq!(
                    result.javascript(codec).unwrap().javascript(),
                    baseline_texts[index]
                );
            }
            verify(&result, codec);
        }
    }
    let (failed, finished) = with_checked_source(
        SOURCE,
        &config,
        ServiceOptions {
            logical_work: mandatory - 1,
            ..options
        },
        |session| session.search_javascript(session.source(), |_| {}),
    )
    .unwrap();
    assert!(
        failed.is_err(),
        "a missing mandatory result must fail the request"
    );
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

#[test]
fn independent_search_off_preserves_each_objectives_direct_spelling() {
    let mut config = configuration(13, "");
    config.javascript.candidate_search = CandidateSearch::Off;
    let result = compile_source(
        SOURCE,
        &config,
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(result.report()["resources"]["optional_work"], 0);
    assert_eq!(result.report()["search"]["proposals"], 0);
    assert!(result.report()["search"]["terminal"]["objectives"]
        .as_array()
        .unwrap()
        .is_empty());
    for codec in CODECS {
        config.objective.codecs = vec![codec];
        let single = compile_source(SOURCE, &config, ServiceOptions::default()).unwrap();
        assert_eq!(
            result.javascript(codec).unwrap().javascript(),
            single.javascript(codec).unwrap().javascript()
        );
        verify(&result, codec);
    }
}

#[test]
fn independent_objective_callback_unwind_restores_allowance_and_releases_handoffs() {
    let config = configuration(13, "");
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let (_, finished) = with_checked_source(SOURCE, &config, options, |session| {
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.search_javascript(session.source(), |observation| {
                // Raw has already transferred its completed winner when gzip
                // begins. Unwinding must also discard that prior handoff.
                if !observation.baseline && observation.sizes.gzip9.is_some() {
                    panic!("intentional independent objective observer failure");
                }
            })
        }));
        assert!(outcome.is_err());
        let ledger = session.compilation().ledger();
        assert!(ledger.baseline_is_sealed());
        assert_eq!(
            ledger.work_limit(WorkDomain::Optional),
            options.logical_work - ledger.work_used(WorkDomain::Baseline)
        );
        assert_eq!(
            ledger.optional_search_work().0,
            ledger.work_used(WorkDomain::Optional)
        );
        assert_eq!(session.compilation().checkpoint_count(), 1);
    })
    .unwrap();
    assert_eq!(finished.ledger.retained_bytes(), 0);
}
