use super::*;
use crate::compilation_policy::{BudgetError, ResourceLimits};
use crate::semantic_program::facts::{FactsCache, FactsError, RetainedFactsCache};

fn config(resources: &str, search: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='raw'\ncandidate_search='{search}'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{resources}"
    ))
    .unwrap()
}

#[test]
fn service_cache_requests_partition_metadata_and_refuse_tiny_capacity_as_resources() {
    let policy = config("", "always")
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let minimum = CacheLimits::within_budget(128, 0, 512_000).bytes;
    for memory in [0, 1, 4_096, 65_536, 1_000_000, 4_096_000, 256_000_000] {
        let request = search_request(
            ServiceOptions {
                logical_work: 73,
                retained_bytes: memory,
                ..ServiceOptions::default()
            },
            &policy,
            Objectives::One(Objective::Raw),
        );
        let limits = request.facts_cache;
        let cache = FactsCache::new(limits).unwrap();
        assert_eq!(request.scalar.max_work, 73 / 8);
        assert_eq!(request.helper.local_facts.work_quota, 73 / 8);
        assert_eq!(request.helper.local_facts.result_bytes, limits.result_bytes);
        assert_eq!(request.string.local_facts.result_bytes, limits.result_bytes);
        assert!(limits.entries > 0 && limits.entries <= 128);
        assert!(limits.result_bytes <= 512_000);
        assert!(cache.retained_bytes() + limits.result_bytes <= limits.bytes);
        assert_eq!(limits.bytes, (memory / 8).min(16_000_000).max(minimum));
    }
    let mut ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: 10,
            retained_bytes: minimum - 1,
            terminal_work: 0,
        },
    )
    .unwrap();
    ledger.seal_baseline().unwrap();
    let error = RetainedFactsCache::new(
        CacheLimits::within_budget(128, 0, 512_000),
        &mut ledger,
        WorkDomain::Optional,
    )
    .unwrap_err();
    assert_eq!(
        error,
        FactsError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional))
    );
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn small_service_options_and_policy_limits_have_the_same_search_budget() {
    let source = "int increment(int value){return(value&255)+1;}export int run(int value){return increment(value);}";
    let from_options = compile_source_semantic(
        source,
        &config("", "always"),
        ServiceOptions {
            logical_work: 2_000_000,
            retained_bytes: 1_000_000,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let from_policy = compile_source_semantic(
        source,
        &config(
            "[policy.resources]\nlogical_work=2000000\nretained_bytes=1000000",
            "always",
        ),
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(
        from_options
            .javascript(Objective::Raw)
            .unwrap()
            .javascript(),
        from_policy.javascript(Objective::Raw).unwrap().javascript(),
    );
    assert_eq!(
        from_options.report()["search"],
        from_policy.report()["search"]
    );
    let report = from_options.report();
    assert!(report["search"]["proof_queries"].as_u64().unwrap() > 0);
    assert!(
        report["search"]["structures"].as_u64().unwrap() > 1,
        "a bounded helper proof must publish an alternative: {report}"
    );
    assert!(report["resources"]["peak_retained_bytes"].as_u64().unwrap() <= 1_000_000);
    assert_eq!(report["resources"]["retained_bytes_after_handoff"], 0);
    assert!(
        !report["search"]["stop"]
            .to_string()
            .contains("InvalidLimits")
    );
}

#[test]
fn small_no_search_service_needs_no_optional_cache_admission() {
    let result = compile_source_semantic(
        "export int answer(){return 17;}",
        &config("", "off"),
        ServiceOptions {
            retained_bytes: 1_000_000,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert!(result.javascript(Objective::Raw).is_some());
    assert_eq!(result.report()["search"]["proposals"], 0);
    assert_eq!(result.report()["resources"]["optional_work"], 0);
    assert_eq!(
        result.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
}
