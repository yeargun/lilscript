//! Checked fixed-resource ownership tests. Runtime/complete-codec matrices use
//! the same fixture in the separate target client; no bytes are patched here.
use super::*;
use crate::compilation_policy::{BudgetPlan, CompilationRequest, ResourceLimits};
use crate::structured_js::selection::{Plan, Style};
use std::path::Path;

fn with_source<R>(inspect: impl FnOnce(Program<'_>, CellId, UnitId) -> R) -> R {
    with_entry("entry.lil", inspect)
}
fn with_entry<R>(entry_name: &str, inspect: impl FnOnce(Program<'_>, CellId, UnitId) -> R) -> R {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/fixed-javascript-resources")
        .join(entry_name);
    let modules = crate::module::discover_modules(&entry).unwrap();
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::semantic::analyze_modules(&syntax, &modules).unwrap();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    let cell = CellId::from_index(
        program
            .cells()
            .iter()
            .position(|cell| cell.name == "score")
            .unwrap(),
    )
    .unwrap();
    let CellBinding::Function(body) = program.cells()[cell.index()].binding else {
        panic!()
    };
    inspect(program, cell, body)
}
fn policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn compiler<'s>(slots: usize, optional_work: u64) -> Compilation<'s> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 30_000_000,
                optional_work,
                baseline_retained_bytes: 0,
                retained_bytes: 30_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: slots },
    )
    .unwrap()
}
fn request() -> FunctionRequest {
    FunctionRequest {
        max_work: 500_000,
        scratch_bytes: 800_000,
        output_bytes: 800_000,
    }
}
fn producer(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    cell: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    let result = compiler
        .producer_javascript(
            base,
            cell,
            "./producer.mjs",
            "scoreABI",
            request(),
            policy,
            WorkDomain::Baseline,
        )
        .unwrap()
        .outcome;
    match result {
        ProducerOutcome::Published(id) => id,
        other => panic!("complete producer proof: {other:?}"),
    }
}
fn render(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> ArtifactId {
    compiler
        .with_javascript_output(candidate, policy, |output| {
            let id = output.render(&Plan::new(style))?;
            output.retain_artifact(id)
        })
        .unwrap()
        .unwrap()
}
fn bytes(compiler: &Compilation<'_>, artifact: ArtifactId) -> (String, Option<String>) {
    compiler
        .with_artifact(artifact, |view| {
            (
                view.javascript.to_owned(),
                view.dependency.map(|value| value.javascript.to_owned()),
            )
        })
        .unwrap()
}

#[test]
fn physical_retarget_rejects_stale_adapter_but_explicit_rebuild_and_compatible_retarget_publish() {
    with_source(|program, cell, body| {
        let mut compiler = compiler(24, 30_000_000);
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(
                crate::semantic_program::facts::CacheLimits {
                    entries: 2,
                    bytes: 100_000,
                    result_bytes: 20_000,
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        compiler
            .with_local_facts(WorkDomain::Baseline, 1, |facts| {
                assert!(!facts
                    .query(
                        source,
                        body,
                        LocalFactsRequest {
                            work_quota: 20_000,
                            result_bytes: 20_000
                        }
                    )
                    .unwrap()
                    .cache_hit());
            })
            .unwrap();
        let packed = producer(&mut compiler, direct, cell, &policy);
        let p0 = render(&mut compiler, packed, &policy, Style::Global);
        let a = compiler
            .freeze_producer_javascript(direct, p0, &policy, WorkDomain::Baseline)
            .unwrap();
        assert!(compiler.with_artifact(p0, |_| ()).is_err());
        let a_bytes = render(&mut compiler, a, &policy, Style::Global);
        let expected_a = bytes(&compiler, a_bytes);
        assert!(expected_a.1.is_some());
        let p0_other = render(&mut compiler, packed, &policy, Style::Scoped);
        let compatible = compiler
            .freeze_producer_javascript(direct, p0_other, &policy, WorkDomain::Baseline)
            .unwrap();
        let retargeted = compiler
            .retarget_fixed_producer_javascript(a, compatible, &policy, WorkDomain::Baseline)
            .unwrap();
        let FunctionOutcome::Published(fields) = compiler
            .scalar_function_javascript(direct, body, request(), &policy, WorkDomain::Baseline)
            .unwrap()
            .outcome
        else {
            panic!()
        };
        let fields_producer = producer(&mut compiler, fields, cell, &policy);
        let p1 = render(&mut compiler, fields_producer, &policy, Style::Global);
        let b = compiler
            .freeze_producer_javascript(direct, p1, &policy, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.retarget_fixed_producer_javascript(a, b, &policy, WorkDomain::Optional),
            Err(CandidateError::StaleEvidence)
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        assert_eq!(bytes(&compiler, a_bytes), expected_a);
        compiler
            .with_local_facts(WorkDomain::Baseline, 1, |facts| {
                assert!(facts
                    .query(
                        source,
                        body,
                        LocalFactsRequest {
                            work_quota: 20_000,
                            result_bytes: 20_000
                        }
                    )
                    .unwrap()
                    .cache_hit());
            })
            .unwrap();
        let off: crate::config::ProjectConfig =
            toml::from_str("[policy.tactics]\ncall-specialization='off'\n").unwrap();
        let off = off
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        // The consumer has no selected function layout. Refusal must come
        // from the frozen producer's actual retained provenance.
        assert!(matches!(
            compiler.with_javascript_output(b, &off, |_| ()),
            Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::CallSpecialization
            ))
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        let repaired = compiler
            .rebuild_fixed_consumer_javascript(a, b, &policy, WorkDomain::Baseline)
            .unwrap();
        let repaired_bytes = render(&mut compiler, repaired, &policy, Style::Global);
        let clean_bytes = render(&mut compiler, b, &policy, Style::Global);
        assert_eq!(
            bytes(&compiler, repaired_bytes),
            bytes(&compiler, clean_bytes)
        );
        // A retained complete artifact owns both files after every candidate
        // (including the original producing candidate) has been released.
        for candidate in [
            retargeted,
            compatible,
            packed,
            fields_producer,
            fields,
            a,
            b,
            repaired,
            direct,
        ] {
            compiler.discard(candidate.semantic_id()).unwrap();
        }
        compiler.discard(source).unwrap();
        assert_eq!(bytes(&compiler, a_bytes), expected_a);
        assert!(compiler.take_artifact(a_bytes).is_err());
        assert_eq!(bytes(&compiler, a_bytes), expected_a);
        for id in [a_bytes, repaired_bytes, clean_bytes] {
            compiler.discard_artifact(id).unwrap();
        }
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn freeze_store_and_work_refusals_preserve_actual_producer_handle_for_retry() {
    with_source(|program, cell, _| {
        let mut compiler = compiler(3, 0);
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let packed = producer(&mut compiler, direct, cell, &policy);
        let artifact = render(&mut compiler, packed, &policy, Style::Global);
        let expected = bytes(&compiler, artifact);
        let retained = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.freeze_producer_javascript(direct, artifact, &policy, WorkDomain::Baseline),
            Err(CandidateError::Publication(PublicationError::StoreFull))
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        assert_eq!(bytes(&compiler, artifact), expected);
        compiler.discard(packed.semantic_id()).unwrap();
        let retained = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.freeze_producer_javascript(direct, artifact, &policy, WorkDomain::Optional),
            Err(CandidateError::Budget(_))
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        assert_eq!(bytes(&compiler, artifact), expected);
        let package = compiler
            .freeze_producer_javascript(direct, artifact, &policy, WorkDomain::Baseline)
            .unwrap();
        assert!(compiler.with_artifact(artifact, |_| ()).is_err());
        compiler.discard(package.semantic_id()).unwrap();
        compiler.discard(direct.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn whole_artifacts_cannot_acquire_producer_provenance_and_source_edit_revokes_frozen_contract() {
    with_source(|program, cell, _| {
        let entry = program.modules()[program.entry_module().index()].initializer;
        let operation = OpId::from_index(
            program
                .unit(entry)
                .unwrap()
                .operations
                .iter()
                .position(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(0))))
                .unwrap(),
        )
        .unwrap();
        let mut compiler = compiler(12, 30_000_000);
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let whole = render(&mut compiler, direct, &policy, Style::Global);
        let retained = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.freeze_producer_javascript(direct, whole, &policy, WorkDomain::Baseline),
            Err(CandidateError::Artifact(_))
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        assert!(bytes(&compiler, whole).1.is_none());
        let producer = producer(&mut compiler, direct, cell, &policy);
        let artifact = render(&mut compiler, producer, &policy, Style::Global);
        let package = compiler
            .freeze_producer_javascript(direct, artifact, &policy, WorkDomain::Baseline)
            .unwrap();
        let kind = OperationKind::Constant(Constant::Integer(1));
        let edited = compiler
            .edit_source(
                source,
                &[UnitPatch {
                    unit: entry,
                    expected_revision: compiler.view(source).unwrap().unit_revision(entry).unwrap(),
                    operations: &[OperationPatch {
                        operation,
                        kind: &kind,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Baseline,
            )
            .unwrap();
        let retained = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.rebase_javascript(package, edited, &policy, WorkDomain::Optional),
            Err(CandidateError::StaleEvidence)
        ));
        assert_eq!(compiler.ledger.retained_bytes(), retained);
        for candidate in [package, producer, direct] {
            compiler.discard(candidate.semantic_id()).unwrap();
        }
        for source in [edited, source] {
            compiler.discard(source).unwrap();
        }
        compiler.discard_artifact(whole).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[path = "fixed_resource_runtime_tests.rs"]
mod runtime;

#[path = "resource_identity_tests.rs"]
mod identity;

#[path = "function_evidence_tests.rs"]
mod function_evidence;

#[path = "fixed_resource_union_tests.rs"]
mod union;
