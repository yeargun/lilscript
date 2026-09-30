use super::*;
use crate::compilation_policy::{BudgetError, ResourceLimits};
use crate::program::facts::{FactsCache, FactsError, RetainedFactsCache};

fn config(resources: &str, search: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "objective.codecs='raw'\n[javascript]\ncandidate_search='{search}'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{resources}"
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
    // Two calls of a helper too large to copy for free: the program rules
    // keep it (M7.5a), and the structural search (from the default level,
    // AM2) weighs inlining it.
    let source = "int increment(int value){return(value&255)+1;}export int run(int value){return increment(value)+increment(value+1);}";
    let from_options = compile_source(
        source,
        &config("", "always"),
        ServiceOptions {
            logical_work: 2_000_000,
            retained_bytes: 1_000_000,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let from_policy = compile_source(
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
    assert_eq!(
        from_options.report()["javascript_policy"],
        from_policy.report()["javascript_policy"]
    );
    assert_eq!(
        from_options.report()["javascript_policy"]["resources"]["logical_work"],
        2_000_000
    );
    assert_eq!(
        from_options.report()["javascript_policy"]["resources"]["retained_bytes"],
        1_000_000
    );
    let report = from_options.report();
    assert!(report["search"]["proof_queries"].as_u64().unwrap() > 0);
    assert!(
        report["search"]["structures"].as_u64().unwrap() > 1,
        "a bounded helper proof must publish an alternative: {report}"
    );
    assert!(report["resources"]["peak_retained_bytes"].as_u64().unwrap() <= 1_000_000);
    assert_eq!(report["resources"]["retained_bytes_after_handoff"], 0);
    assert!(!report["search"]["stop"]
        .to_string()
        .contains("InvalidLimits"));
}

#[test]
fn service_ceilings_participate_in_policy_identity_without_loosening_toml() {
    let plain = config("", "off");
    let capped = config(
        "[policy.resources]\nlogical_work=1000000\nretained_bytes=2000000\nwall_time_ms=5000",
        "off",
    );
    let small = ServiceOptions {
        logical_work: 800_000,
        retained_bytes: 3_000_000,
        ..ServiceOptions::default()
    };
    let large = ServiceOptions {
        logical_work: 900_000,
        ..small
    };
    for request in [
        CompilationRequest::JavaScript {
            preserve_root_exports: true,
        },
        CompilationRequest::Native,
    ] {
        let a = small.resolve_policy(&plain, request).unwrap();
        let b = large.resolve_policy(&plain, request).unwrap();
        assert_ne!(a.fingerprint(), b.fingerprint());
        let c = small.resolve_policy(&capped, request).unwrap();
        assert_eq!(
            c.resources(),
            ResourceLimits {
                logical_work: Some(800_000),
                retained_bytes: Some(2_000_000),
                wall_time_ms: Some(5000)
            }
        );
        let roomier = ServiceOptions {
            logical_work: 2_000_000,
            retained_bytes: 4_000_000,
            ..small
        };
        assert_eq!(
            roomier
                .resolve_policy(&capped, request)
                .unwrap()
                .fingerprint(),
            capped.resolve_policy(request).unwrap().fingerprint()
        );
    }
}

#[test]
fn small_no_search_service_needs_no_optional_cache_admission() {
    let result = compile_source(
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
