//! Identity clients of the actual checked publication/render/freeze owners.
//! Kept under fixed_resources::tests to reuse that source and its private test
//! access; this adds no production snapshot or artifact inspection escape hatch.
use super::*;
use crate::compilation_policy::{BudgetError, TacticId, WorkKind};
use crate::output_budget::{AllocationClass, AllocationError};
use crate::semantic_program::implementation_identity::{
    ImplementationIdentity, ResourceDescription,
};
use std::cmp::Ordering;

fn identity(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    domain: WorkDomain,
) -> Result<ImplementationIdentity, AllocationError> {
    let slot = compiler.candidate_slot(candidate).unwrap();
    let checkpoint = compiler.slots[slot].checkpoint.as_ref().unwrap();
    let mut budget = AllocationBudget::new(Some((&mut compiler.ledger, domain)));
    let result = ImplementationIdentity::build(
        checkpoint.implementations.as_ref(),
        compiler.store,
        &mut budget,
    );
    assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
    result
}
fn compare(
    compiler: &mut Compilation<'_>,
    left: &ImplementationIdentity,
    right: &ImplementationIdentity,
) -> Ordering {
    left.compare(
        right,
        &mut AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline))),
    )
    .unwrap()
}
fn equivalent(
    compiler: &mut Compilation<'_>,
    left: &ImplementationIdentity,
    right: &ImplementationIdentity,
) -> bool {
    left.equivalent(
        right,
        &mut AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline))),
    )
    .unwrap()
}
fn release(compiler: &mut Compilation<'_>, value: ImplementationIdentity) {
    value.discard(compiler.store, &mut compiler.ledger).unwrap();
}
fn package(
    compiler: &mut Compilation<'_>,
    direct: CandidateId,
    producer: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> CandidateId {
    let artifact = render(compiler, producer, policy, style);
    compiler
        .freeze_producer_javascript(direct, artifact, policy, WorkDomain::Baseline)
        .unwrap()
}

#[test]
fn whole_keeps_format_three_words_hash_and_visit_tariff() {
    let mut compiler = compiler(4, 30_000_000);
    let owner = compiler.store;
    let start = compiler.ledger.work_used(WorkDomain::Baseline);
    let before = compiler.ledger.retained_bytes();
    let direct = ImplementationIdentity::build(
        None,
        owner,
        &mut AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline))),
    )
    .unwrap();
    let words = [3u32, 0, 0, 0, 0, 0];
    let expected = words
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .fold(0xcbf29ce484222325u64, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100000001b3)
        });
    assert_eq!(direct.words(), words);
    assert_eq!(direct.description().whole_words(), Some(words.as_slice()));
    assert_eq!(direct.fingerprint(), expected);
    assert_eq!(
        compiler.ledger.work_used(WorkDomain::Baseline) - start,
        8,
        "one query, one allocation, six recipe words; no resource work"
    );
    assert_eq!(compiler.ledger.retained_bytes() - before, 24);
    let start = compiler.ledger.work_used(WorkDomain::Baseline);
    assert_eq!(compare(&mut compiler, &direct, &direct), Ordering::Equal);
    assert_eq!(compiler.ledger.work_used(WorkDomain::Baseline) - start, 7);
    release(&mut compiler, direct);
    assert_eq!(compiler.ledger.retained_bytes(), before);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn equal_resources_ignore_handle_and_revision_order_but_own_their_payload() {
    with_source(|program, cell, body| {
        with_source(|program_again, cell_again, body_again| {
            let mut compiler = compiler(24, 30_000_000);
            let policy = policy();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let produced_a = producer(&mut compiler, direct, cell, &policy);
            let a = package(&mut compiler, direct, produced_a, &policy, Style::Global);
            for _ in 0..17 {
                let _ = RevisionId::fresh();
            }
            let produced_b = producer(&mut compiler, direct, cell, &policy);
            let b = package(&mut compiler, direct, produced_b, &policy, Style::Global);
            let ai = identity(&mut compiler, a, WorkDomain::Optional).unwrap();
            let bi = identity(&mut compiler, b, WorkDomain::Baseline).unwrap();
            let (ap, bp) = match (ai.description().resource(), bi.description().resource()) {
                (
                    ResourceDescription::Consumer { producer: ap, .. },
                    ResourceDescription::Consumer { producer: bp, .. },
                ) => (ap, bp),
                _ => panic!("complete producer descriptions"),
            };
            assert_eq!(ap.javascript(), bp.javascript());
            assert_ne!(
                ap.javascript().as_ptr(),
                bp.javascript().as_ptr(),
                "independent completed records"
            );
            let original_text = ap.javascript().to_owned();
            let original_ptr = ap.javascript().as_ptr();
            assert!(ai.whole_words().is_none());
            assert!(equivalent(&mut compiler, &ai, &bi));
            assert_eq!(ai.fingerprint(), bi.fingerprint());
            assert_eq!(compare(&mut compiler, &ai, &bi), Ordering::Equal);

            // Independent checked lowering has a different source revision. It is
            // comparable for deterministic ordering, never interchangeable semantic
            // evidence: publication retains that distinct compatibility check.
            {
                assert_eq!((cell, body), (cell_again, body_again));
                let other_source = compiler
                    .adopt_checked(program_again, WorkDomain::Baseline)
                    .unwrap();
                let first_snapshot = compiler.slots[compiler.lookup(source).unwrap()]
                    .checkpoint
                    .as_ref()
                    .unwrap()
                    .semantic
                    .identity;
                let other_snapshot = compiler.slots[compiler.lookup(other_source).unwrap()]
                    .checkpoint
                    .as_ref()
                    .unwrap()
                    .semantic
                    .identity;
                assert_ne!(first_snapshot, other_snapshot);
                let other_direct = compiler
                    .direct_javascript(other_source, &policy, WorkDomain::Baseline)
                    .unwrap();
                let other_producer = producer(&mut compiler, other_direct, cell_again, &policy);
                let other_package = package(
                    &mut compiler,
                    other_direct,
                    other_producer,
                    &policy,
                    Style::Global,
                );
                let other = identity(&mut compiler, other_package, WorkDomain::Baseline).unwrap();
                assert_eq!(compare(&mut compiler, &ai, &other), Ordering::Equal);
                assert_eq!(ai.fingerprint(), other.fingerprint());
                release(&mut compiler, other);
                for candidate in [other_package, other_producer, other_direct] {
                    compiler.discard(candidate.semantic_id()).unwrap();
                }
                compiler.discard(other_source).unwrap();
            }
            let before = compiler.ledger.retained_bytes();
            let (ai, error) = ai
                .discard(RevisionId::fresh(), &mut compiler.ledger)
                .unwrap_err();
            assert_eq!(error, AllocationError::WrongOwner);
            assert_eq!(compiler.ledger.retained_bytes(), before);
            for candidate in [a, b, produced_a, produced_b, direct] {
                compiler.discard(candidate.semantic_id()).unwrap();
            }
            compiler.discard(source).unwrap();
            let ResourceDescription::Consumer { producer, .. } = ai.description().resource() else {
                panic!()
            };
            assert_eq!(producer.javascript(), original_text);
            assert_eq!(
                producer.javascript().as_ptr(),
                original_ptr,
                "identity retained original text, not a copy"
            );
            assert_eq!(compare(&mut compiler, &ai, &bi), Ordering::Equal);
            release(&mut compiler, ai);
            release(&mut compiler, bi);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    });
}

#[test]
fn physical_contract_and_fixed_bytes_participate_in_exact_collision_checked_identity() {
    with_source(|program, cell, body| {
        let mut compiler = compiler(24, 30_000_000);
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let p = producer(&mut compiler, direct, cell, &policy);
        let a = package(&mut compiler, direct, p, &policy, Style::Global);
        let source_named = package(&mut compiler, direct, p, &policy, Style::Source);
        let renamed = match compiler
            .producer_javascript(
                direct,
                cell,
                "./different.mjs",
                "otherABI",
                request(),
                &policy,
                WorkDomain::Baseline,
            )
            .unwrap()
            .outcome
        {
            ProducerOutcome::Published(value) => value,
            other => panic!("{other:?}"),
        };
        let renamed_package = package(&mut compiler, direct, renamed, &policy, Style::Global);
        let FunctionOutcome::Published(fields) = compiler
            .scalar_function_javascript(direct, body, request(), &policy, WorkDomain::Baseline)
            .unwrap()
            .outcome
        else {
            panic!()
        };
        let fields_producer = producer(&mut compiler, fields, cell, &policy);
        let field_package = package(
            &mut compiler,
            direct,
            fields_producer,
            &policy,
            Style::Global,
        );
        let direct_id = identity(&mut compiler, direct, WorkDomain::Baseline).unwrap();
        let producer_id = identity(&mut compiler, p, WorkDomain::Baseline).unwrap();
        assert_eq!(
            direct_id.description().recipe_words(),
            producer_id.description().recipe_words()
        );
        assert_ne!(
            compare(&mut compiler, &direct_id, &producer_id),
            Ordering::Equal,
            "producer tag cannot collapse into Whole"
        );
        let ai = identity(&mut compiler, a, WorkDomain::Baseline).unwrap();
        let mut alternatives = [renamed_package, source_named, field_package]
            .map(|candidate| identity(&mut compiler, candidate, WorkDomain::Baseline).unwrap());
        for alternative in &mut alternatives {
            let order = compare(&mut compiler, &ai, alternative);
            assert_ne!(order, Ordering::Equal);
            alternative.force_fingerprint_for_collision_test(ai.fingerprint());
            assert!(!equivalent(&mut compiler, &ai, alternative));
            assert_eq!(
                compare(&mut compiler, &ai, alternative),
                order,
                "hash is never tie order"
            );
        }
        let ResourceDescription::Consumer { producer: ap, .. } = ai.description().resource() else {
            panic!()
        };
        let ResourceDescription::Consumer { producer: sp, .. } =
            alternatives[1].description().resource()
        else {
            panic!()
        };
        assert_ne!(
            ap.javascript(),
            sp.javascript(),
            "actual different frozen bytes are observed"
        );
        release(&mut compiler, direct_id);
        release(&mut compiler, producer_id);
        release(&mut compiler, ai);
        for alternative in alternatives {
            release(&mut compiler, alternative);
        }
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn byte_identical_producers_keep_their_actual_permission_provenance() {
    with_source(|program, cell, _| {
        let mut compiler = compiler(16, 30_000_000);
        let policy = policy();
        assert!(policy.tactic(TacticId::IdentifierMangling).enabled);
        assert!(policy.tactic(TacticId::NamingSearch).enabled);
        let off: crate::config::ProjectConfig =
            toml::from_str("[policy.tactics]\nidentifier-mangling='off'\nnaming-search='off'\n")
                .unwrap();
        let off = off
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let p = producer(&mut compiler, direct, cell, &policy);
        let baseline_artifact = render(&mut compiler, p, &off, Style::Source);
        let explored_artifact = render(&mut compiler, p, &policy, Style::Source);
        assert_eq!(
            bytes(&compiler, baseline_artifact),
            bytes(&compiler, explored_artifact)
        );
        let baseline = compiler
            .freeze_producer_javascript(direct, baseline_artifact, &policy, WorkDomain::Baseline)
            .unwrap();
        let explored = compiler
            .freeze_producer_javascript(direct, explored_artifact, &policy, WorkDomain::Baseline)
            .unwrap();
        let bi = identity(&mut compiler, baseline, WorkDomain::Baseline).unwrap();
        let mut ei = identity(&mut compiler, explored, WorkDomain::Baseline).unwrap();
        let (bp, ep) = match (bi.description().resource(), ei.description().resource()) {
            (
                ResourceDescription::Consumer { producer: bp, .. },
                ResourceDescription::Consumer { producer: ep, .. },
            ) => (bp, ep),
            _ => panic!(),
        };
        assert_eq!(bp.javascript(), ep.javascript());
        assert_eq!(bp.provenance().naming(), ep.provenance().naming());
        assert_eq!(bp.provenance().output(), ep.provenance().output());
        assert!(!bp
            .provenance()
            .naming_tactics()
            .iter()
            .any(|usage| usage.tactic == TacticId::NamingSearch));
        assert!(ep
            .provenance()
            .naming_tactics()
            .iter()
            .any(|usage| usage.tactic == TacticId::NamingSearch));
        assert_ne!(compare(&mut compiler, &bi, &ei), Ordering::Equal);
        ei.force_fingerprint_for_collision_test(bi.fingerprint());
        assert!(!equivalent(&mut compiler, &bi, &ei));
        assert!(compiler
            .with_javascript_output(baseline, &off, |_| ())
            .is_ok());
        assert!(
            compiler
                .with_javascript_output(explored, &off, |_| ())
                .is_err(),
            "actual permission client distinguishes the same bytes"
        );
        let delivered = render(&mut compiler, baseline, &off, Style::Source);
        compiler
            .with_artifact(delivered, |view| {
                assert_eq!(view.dependency.unwrap().output, bp.provenance().output());
            })
            .unwrap();
        release(&mut compiler, bi);
        release(&mut compiler, ei);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn resource_visit_and_partial_share_refusals_and_unwind_preserve_original_owners() {
    const OPTIONAL: u64 = 30_000_000;
    for remaining in [0, 1] {
        with_source(|program, cell, _| {
            let mut compiler = compiler(12, OPTIONAL);
            let policy = policy();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let produced = producer(&mut compiler, direct, cell, &policy);
            let package = package(&mut compiler, direct, produced, &policy, Style::Global);
            let start = compiler.ledger.work_used(WorkDomain::Baseline);
            let saved = identity(&mut compiler, package, WorkDomain::Baseline).unwrap();
            let complete_work = compiler.ledger.work_used(WorkDomain::Baseline) - start;
            assert!(
                complete_work > 8 + 2,
                "resource bytes/ABI visits plus two real shares"
            );
            let retained = compiler.ledger.retained_bytes();
            let padding = 30_000_000 - retained - saved.retained_bytes() + 1;
            compiler
                .ledger
                .retain(WorkDomain::Optional, padding)
                .unwrap();
            assert!(matches!(
                identity(&mut compiler, package, WorkDomain::Optional),
                Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                    WorkDomain::Optional
                )))
            ));
            assert_eq!(compiler.ledger.retained_bytes(), retained + padding);
            compiler
                .ledger
                .release(WorkDomain::Optional, padding)
                .unwrap();
            let slot = compiler.candidate_slot(package).unwrap();
            let map = compiler.slots[slot]
                .checkpoint
                .as_ref()
                .unwrap()
                .implementations
                .as_ref();
            let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut budget =
                    AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
                let _ = ImplementationIdentity::with_description(
                    map,
                    compiler.store,
                    &mut budget,
                    |description| {
                        assert!(description.whole_words().is_none());
                        panic!("intentional diagnostic callback unwind");
                    },
                );
            }));
            assert!(unwound.is_err());
            assert_eq!(compiler.ledger.retained_bytes(), retained);
            // At zero, refuse before any descriptor allocation. At work-1 the
            // whole descriptor and first producer Arc share succeed; the second
            // share refuses and must undo that earlier ownership increment.
            let quota = if remaining == 0 { 0 } else { complete_work - 1 };
            let used = compiler.ledger.work_used(WorkDomain::Optional);
            compiler
                .ledger
                .charge(
                    WorkDomain::Optional,
                    WorkKind::Analysis,
                    OPTIONAL - used - quota,
                )
                .unwrap();
            assert!(matches!(
                identity(&mut compiler, package, WorkDomain::Optional),
                Err(AllocationError::Budget(BudgetError::WorkExhausted(
                    WorkDomain::Optional
                )))
            ));
            assert_eq!(compiler.ledger.retained_bytes(), retained);
            assert_eq!(compare(&mut compiler, &saved, &saved), Ordering::Equal);
            let retry = identity(&mut compiler, package, WorkDomain::Baseline).unwrap();
            assert!(equivalent(&mut compiler, &saved, &retry));
            release(&mut compiler, saved);
            release(&mut compiler, retry);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
