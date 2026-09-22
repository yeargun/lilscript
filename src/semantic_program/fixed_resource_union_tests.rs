//! Independently valid physical choices can be incompatible without any source
//! evidence becoming stale. The common union must retain that distinction.
use super::*;

#[test]
fn fixed_endpoint_and_inline_helper_conflict_without_consuming_either_choice() {
    with_source(|program, cell, body| {
        with_source(|other_program, other_cell, other_body| {
            assert_eq!((cell, body), (other_cell, other_body));
            let mut compiler = compiler(24, 30_000_000);
            let policy = policy();
            compiler
                .enable_local_facts(
                    crate::semantic_program::facts::CacheLimits {
                        entries: 4,
                        bytes: 1_000_000,
                        result_bytes: 100_000,
                    },
                    WorkDomain::Baseline,
                )
                .unwrap();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let produced = producer(&mut compiler, direct, cell, &policy);
            let producer_artifact = render(&mut compiler, produced, &policy, Style::Global);
            let fixed = compiler
                .freeze_producer_javascript(
                    direct,
                    producer_artifact,
                    &policy,
                    WorkDomain::Baseline,
                )
                .unwrap();
            let inlined = match compiler
                .inline_helper_javascript(
                    direct,
                    cell,
                    HelperRequest {
                        max_work: 1_000_000,
                        scratch_bytes: 1_000_000,
                        output_bytes: 1_000_000,
                        local_facts: LocalFactsRequest {
                            work_quota: 100_000,
                            result_bytes: 100_000,
                        },
                    },
                    &policy,
                    WorkDomain::Baseline,
                )
                .unwrap()
                .outcome
            {
                HelperOutcome::Published(value) => value,
                other => panic!("independently valid whole-program inline helper: {other:?}"),
            };
            let inline_artifact = render(&mut compiler, inlined, &policy, Style::Global);
            let inline_bytes = bytes(&compiler, inline_artifact);
            for resource in [produced, fixed] {
                let retained_artifact = render(&mut compiler, resource, &policy, Style::Global);
                let before_bytes = bytes(&compiler, retained_artifact);
                for (left, right) in [(resource, inlined), (inlined, resource)] {
                    let retained = compiler.ledger.retained_bytes();
                    let live = compiler.live;
                    assert!(
                        matches!(
                            compiler.combine_javascript(left, right, &policy, WorkDomain::Optional),
                            Err(CandidateError::ConflictingChoice)
                        ),
                        "a valid fixed export cannot simultaneously be erased by inlining"
                    );
                    assert_eq!(compiler.ledger.retained_bytes(), retained);
                    assert_eq!(compiler.live, live);
                    assert_eq!(bytes(&compiler, retained_artifact), before_bytes);
                    assert_eq!(bytes(&compiler, inline_artifact), inline_bytes);
                }
                // Publishing a newly proved helper into this required resource
                // follows the same choice rule. Its temporary proof is released
                // by the map insertion owner when admission rejects it.
                let retained = compiler.ledger.retained_bytes();
                let live = compiler.live;
                let result = compiler.inline_helper_javascript(
                    resource,
                    cell,
                    HelperRequest {
                        max_work: 1_000_000,
                        scratch_bytes: 1_000_000,
                        output_bytes: 1_000_000,
                        local_facts: LocalFactsRequest {
                            work_quota: 100_000,
                            result_bytes: 100_000,
                        },
                    },
                    &policy,
                    WorkDomain::Optional,
                );
                assert!(
                    matches!(result, Err(CandidateError::ConflictingChoice)),
                    "{result:?}"
                );
                assert_eq!(compiler.ledger.retained_bytes(), retained);
                assert_eq!(compiler.live, live);
                assert_eq!(bytes(&compiler, retained_artifact), before_bytes);
                // A compatible request remains renderable through the same
                // publication/render owners after both refused operand orders.
                let retry = compiler
                    .combine_javascript(resource, direct, &policy, WorkDomain::Baseline)
                    .unwrap();
                let retry_artifact = render(&mut compiler, retry, &policy, Style::Global);
                assert_eq!(bytes(&compiler, retry_artifact), before_bytes);
                compiler.discard_artifact(retry_artifact).unwrap();
                compiler.discard(retry.semantic_id()).unwrap();
                compiler.discard_artifact(retained_artifact).unwrap();
            }

            // This is a different checked snapshot, despite identical source
            // table positions. The pre-union source guard remains authoritative.
            let other_source = compiler
                .adopt_checked(other_program, WorkDomain::Baseline)
                .unwrap();
            let other_direct = compiler
                .direct_javascript(other_source, &policy, WorkDomain::Baseline)
                .unwrap();
            let retained = compiler.ledger.retained_bytes();
            let live = compiler.live;
            assert!(matches!(
                compiler.combine_javascript(fixed, other_direct, &policy, WorkDomain::Baseline),
                Err(CandidateError::StaleEvidence)
            ));
            assert_eq!(compiler.ledger.retained_bytes(), retained);
            assert_eq!(compiler.live, live);
            assert_eq!(bytes(&compiler, inline_artifact), inline_bytes);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    });
}
