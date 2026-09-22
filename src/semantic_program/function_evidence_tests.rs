//! Selected proofs are retained by the physical export; zero analysis allowance
//! cannot manufacture a proof for an otherwise identical unselected candidate.
use super::*;

#[test]
fn physical_exports_reuse_selected_proofs_and_keep_original_lifetimes() {
    with_source(|program, cell, body| {
        let mut compiler = compiler(24, 30_000_000);
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let FunctionOutcome::Published(fields) = compiler
            .scalar_function_javascript(direct, body, request(), &policy, WorkDomain::Baseline)
            .unwrap()
            .outcome
        else {
            panic!("selected layout")
        };
        let query_disabled = FunctionRequest {
            max_work: 0,
            scratch_bytes: 0,
            output_bytes: 0,
        };
        let retained = compiler.ledger.retained_bytes();
        let missing = compiler
            .producer_javascript(
                direct,
                cell,
                "./producer.mjs",
                "scoreABI",
                query_disabled,
                &policy,
                WorkDomain::Optional,
            )
            .unwrap();
        assert!(matches!(
            missing.outcome,
            ProducerOutcome::Truncated(FunctionLimit::Work)
        ));
        assert!(missing.receipt.is_some());
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        let mut producers = Vec::new();
        for name in ["scoreABI", "anotherABI"] {
            let reused = compiler
                .producer_javascript(
                    fields,
                    cell,
                    "./producer.mjs",
                    name,
                    query_disabled,
                    &policy,
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(
                reused.receipt.is_none(),
                "retaining selected evidence is not a new analysis"
            );
            let ProducerOutcome::Published(candidate) = reused.outcome else {
                panic!("selected proof reuse")
            };
            producers.push(candidate);
        }
        compiler
            .with_implementations(producers[0], |first| {
                compiler
                    .with_implementations(producers[1], |second| {
                        assert!(
                            first
                                .resource()
                                .unwrap()
                                .export()
                                .shared_proof_bytes(second.resource().unwrap().export(),)
                                > 0,
                            "both physical contracts retain the original proof allocation"
                        );
                    })
                    .unwrap();
            })
            .unwrap();
        let retained = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.producer_javascript(
                fields,
                cell,
                "./producer.mjs",
                "invalid export name",
                query_disabled,
                &policy,
                WorkDomain::Optional,
            ),
            Err(CandidateError::InvalidRequest)
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        let artifact = render(&mut compiler, producers[0], &policy, Style::Global);
        let package = compiler
            .freeze_producer_javascript(direct, artifact, &policy, WorkDomain::Optional)
            .unwrap();
        for candidate in [fields, producers[0], producers[1]] {
            compiler.discard(candidate.semantic_id()).unwrap();
        }
        // A surviving package can render after every selected-layout/producer
        // candidate is gone. Its contract still owns the original proof.
        let complete = render(&mut compiler, package, &policy, Style::Global);
        compiler.discard(package.semantic_id()).unwrap();
        compiler.discard(direct.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        assert!(bytes(&compiler, complete).1.is_some());
        compiler.discard_artifact(complete).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn selected_proof_reuse_still_obeys_policy_and_original_work_domain() {
    with_source(|program, cell, body| {
        let mut compiler = compiler(16, 30_000_000);
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let FunctionOutcome::Published(fields) = compiler
            .scalar_function_javascript(direct, body, request(), &policy, WorkDomain::Optional)
            .unwrap()
            .outcome
        else {
            panic!("selected layout")
        };
        let off: crate::config::ProjectConfig =
            toml::from_str("[policy.tactics]\ncall-specialization='off'\n").unwrap();
        let off = off
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let before = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.producer_javascript(
                fields,
                cell,
                "./producer.mjs",
                "scoreABI",
                request(),
                &off,
                WorkDomain::Baseline,
            ),
            Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::CallSpecialization
            ))
        ));
        assert_eq!(compiler.ledger.retained_bytes(), before);
        let selected = producer(&mut compiler, fields, cell, &policy);
        compiler.discard(fields.semantic_id()).unwrap();
        assert!(compiler.ledger.retained_bytes_in(WorkDomain::Optional) > 0);
        compiler.discard(selected.semantic_id()).unwrap();
        assert_eq!(compiler.ledger.retained_bytes_in(WorkDomain::Optional), 0);
        compiler.discard(direct.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn refused_shared_wrapper_releases_the_completed_packed_proof() {
    use crate::semantic_program::function_layout::{self, FamilyOutcome, FamilyRequest};
    use crate::semantic_program::implementations::FunctionEvidence;
    use crate::semantic_program::uses::UseIndex;
    with_source(|program, _, body| {
        let mut compiler = compiler(8, 30_000_000);
        let uses = UseIndex::build(&program, &mut compiler.ledger, WorkDomain::Baseline).unwrap();
        let proof = function_layout::analyze_published(
            &program,
            &uses,
            body,
            FamilyRequest {
                execution: crate::compilation_contract::JavaScriptExecution::Module,
                attempt: AnalysisAttempt {
                    plan: function_layout::FUNCTION_LAYOUT_PLAN,
                    algorithm_version: function_layout::FUNCTION_LAYOUT_VERSION,
                    work_quota: 500_000,
                },
                scratch_bytes: 800_000,
                output_bytes: 800_000,
            },
            &mut compiler.ledger,
            WorkDomain::Baseline,
        )
        .unwrap();
        let FamilyOutcome::Complete(proof) = proof.outcome else {
            panic!("complete packed proof")
        };
        let proof_bytes = proof.retained_bytes();
        let retained = compiler.ledger.retained_bytes();
        let padding = 30_000_000 - retained;
        compiler
            .ledger
            .retain(WorkDomain::Optional, padding)
            .unwrap();
        let result = {
            let mut budget =
                AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Optional)));
            FunctionEvidence::from_family(proof, &mut budget)
        };
        assert!(matches!(
            result,
            Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(
            compiler.ledger.retained_bytes(),
            retained + padding - proof_bytes
        );
        compiler
            .ledger
            .release(WorkDomain::Optional, padding)
            .unwrap();
        uses.discard(&mut compiler.ledger).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
