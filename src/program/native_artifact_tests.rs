use super::*;
use crate::compilation_policy::{
    AdmissionError, BudgetError, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::program::CellId;
use crate::program::publication::{
    CheckpointLimit, Compilation, NativeHostBinding, NativeHostBindings, SemanticId,
};
use crate::js::selection::Style;

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;
const SOURCE: &str = "int twice(int value){return value*2;}print(twice(21));";

fn policy(settings: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(settings).unwrap();
    config.resolve_policy(CompilationRequest::Native).unwrap()
}

fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 8 },
    )
    .unwrap()
}

fn with_source(source: &str, inspect: impl FnOnce(&mut Compilation<'_>, SemanticId)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = crate::program::from_checked_source(&syntax, &semantics).unwrap();
    let mut compilation = compilation();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    inspect(&mut compilation, source);
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

#[test]
fn native_qualification_rejects_unknown_constraints_but_preserves_inspection() {
    with_source(SOURCE, |compilation, source| {
        let before = compilation.ledger().retained_bytes();
        for (field, evidence) in [
            ("max_startup_work", "startup work"),
            ("max_recurring_work", "recurring work"),
            ("max_runtime_memory_bytes", "runtime memory"),
            ("max_performance_regression_percent", "performance estimate"),
        ] {
            let policy = policy(&format!("[policy.constraints]\n{field}=0\n"));
            assert!(matches!(
                compilation.retain_native_c(source, &policy, ArtifactRuntimeEvidence::default(), WorkDomain::Baseline),
                Err(NativeError::Admission(AdmissionError::MissingCostEvidence(actual))) if actual == evidence
            ));
            assert_eq!(compilation.ledger().retained_bytes(), before);
            compilation
                .with_native_c(source, &policy, WorkDomain::Baseline, |output| {
                    assert!(!output.as_str().is_empty());
                })
                .unwrap();
            assert_eq!(compilation.ledger().retained_bytes(), before);
        }
        let policy = policy("[policy.constraints]\nmax_startup_work=4\n");
        let evidence = ArtifactRuntimeEvidence {
            startup_work: Some(5),
            ..Default::default()
        };
        assert!(matches!(
            compilation.retain_native_c(source, &policy, evidence, WorkDomain::Baseline),
            Err(NativeError::Admission(AdmissionError::Constraint(
                "startup work"
            )))
        ));
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let receipt = compilation
            .retain_native_c(
                source,
                &policy,
                ArtifactRuntimeEvidence {
                    startup_work: Some(4),
                    ..Default::default()
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        assert_eq!(receipt.cost().startup_work, Some(4));
        assert_eq!(receipt.cost().recurring_work, None);
        assert_eq!(receipt.cost().performance_score, None);
        assert_eq!(receipt.cost().runtime_memory_bytes, None);
        assert_eq!(receipt.policy_fingerprint(), policy.fingerprint());
        // finish, not a terminal handoff, releases this retained native record.
    });
}

#[test]
fn native_handoff_is_typed_zero_copy_and_rejects_foreign_consumed_and_stale_receipts() {
    with_source(SOURCE, |owner, source| {
        let policy = policy("");
        let before = owner.ledger().retained_bytes();
        let codecs = owner.ledger().work_by_kind(WorkKind::Codec);
        let receipt = owner
            .retain_native_c(
                source,
                &policy,
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
            )
            .unwrap();
        let (pointer, capacity) = owner
            .with_qualified_native_artifact(&receipt, |view| {
                assert!(view.header.is_empty());
                assert_eq!(receipt.c_bytes(), view.c.len());
                assert_eq!(receipt.header_bytes(), view.header.len());
                assert_eq!(
                    receipt.cost().transfer_bytes,
                    (view.c.len() + view.header.len()) as u64
                );
                (view.c.as_ptr(), view.retained_capacity as u64)
            })
            .unwrap();
        assert!(owner.ledger().retained_bytes() >= before + capacity);
        assert_eq!(owner.ledger().work_by_kind(WorkKind::Codec), codecs);
        let mut foreign = compilation();
        assert!(
            foreign
                .with_qualified_native_artifact(&receipt, |_| ())
                .is_err()
        );
        assert!(foreign.take_qualified_native_artifact(receipt).is_err());
        assert_eq!(foreign.finish().retained_bytes(), 0);
        let retained = owner.ledger().retained_bytes();
        let (c, header) = owner.take_qualified_native_artifact(receipt).unwrap();
        assert_eq!(pointer, c.as_ptr());
        assert!(header.is_empty());
        assert_eq!(owner.ledger().retained_bytes(), retained - capacity);
        assert!(owner.take_qualified_native_artifact(receipt).is_err());
        let next = owner
            .retain_native_c(
                source,
                &policy,
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
            )
            .unwrap();
        assert_eq!(receipt.artifact.slot, next.artifact.slot);
        assert_ne!(receipt.artifact.generation, next.artifact.generation);
        assert!(
            owner
                .with_qualified_native_artifact(&receipt, |_| ())
                .is_err()
        );
        owner.discard(source).unwrap();
        let (next_c, _) = owner.take_qualified_native_artifact(next).unwrap();
        assert_eq!(
            c, next_c,
            "native bytes no longer need their source checkpoint"
        );
    });
}

#[test]
fn native_host_header_is_qualified_retained_and_transferred_with_c() {
    with_source(
        "extern int compute(int value);print(compute(7));",
        |owner, source| {
            let cell = owner
                .with_semantic(source, |program, _, _| {
                    CellId::from_index(
                        program
                            .cells()
                            .iter()
                            .position(|cell| cell.name == "compute")
                            .unwrap(),
                    )
                    .unwrap()
                })
                .unwrap();
            let bindings = [NativeHostBinding {
                cell,
                link_name: "host_compute",
            }];
            let hosts = NativeHostBindings {
                callback_abi_version: 1,
                bindings: &bindings,
            };
            let receipt = owner
                .retain_native_c_and_hosts(
                    source,
                    &policy(""),
                    ArtifactRuntimeEvidence::default(),
                    WorkDomain::Baseline,
                    &hosts,
                )
                .unwrap();
            let (c_pointer, header_pointer, capacity) = owner
                .with_qualified_native_artifact(&receipt, |view| {
                    assert!(view.c.contains("host_compute"));
                    assert!(view.header.contains("host_compute"));
                    assert_eq!(
                        receipt.cost().transfer_bytes,
                        (view.c.len() + view.header.len()) as u64
                    );
                    assert_eq!(receipt.header_bytes(), view.header.len());
                    assert!(receipt.header_bytes() > 0);
                    (
                        view.c.as_ptr(),
                        view.header.as_ptr(),
                        view.retained_capacity as u64,
                    )
                })
                .unwrap();
            assert_eq!(receipt.callback_abi_version(), 1);
            assert_eq!(receipt.abi_version(), crate::package::LILSCRIPT_ABI_VERSION);
            let retained = owner.ledger().retained_bytes();
            let (c, header) = owner.take_qualified_native_artifact(receipt).unwrap();
            assert_eq!(c.as_ptr(), c_pointer);
            assert_eq!(header.as_ptr(), header_pointer);
            assert_eq!(owner.ledger().retained_bytes(), retained - capacity);
        },
    );
}

fn javascript_policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: false,
        })
        .unwrap()
}

#[test]
fn retained_native_files_reduce_memory_available_during_javascript_formation() {
    with_source(SOURCE, |owner, source| {
        let native = owner
            .retain_native_c(
                source,
                &policy(""),
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
            )
            .unwrap();
        let capacity = owner
            .with_qualified_native_artifact(&native, |view| view.retained_capacity as u64)
            .unwrap();
        let policy = javascript_policy();
        let candidate = owner
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let (with_native, request) = owner
            .with_javascript_output(candidate, &policy, |output| {
                output.with_allocation_budget(|budget| {
                    let live = budget.with_ledger(|ledger| ledger.unwrap().0.retained_bytes());
                    let request = MEMORY - live + 1;
                    assert!(matches!(
                        budget.retain(AllocationClass::Scratch, request),
                        Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                            WorkDomain::Baseline
                        )))
                    ));
                    (live, request)
                })
            })
            .unwrap();
        let (_c, _header) = owner.take_qualified_native_artifact(native).unwrap();
        owner
            .with_javascript_output(candidate, &policy, |output| {
                output.with_allocation_budget(|budget| {
                    let live = budget.with_ledger(|ledger| ledger.unwrap().0.retained_bytes());
                    assert_eq!(live, with_native - capacity);
                    let mut phase = budget.scope();
                    phase.retain(AllocationClass::Scratch, request).unwrap();
                })
            })
            .unwrap();
    });
}

#[test]
fn javascript_exact_score_reuse_skips_native_slots_without_consuming_them() {
    with_source(SOURCE, |owner, source| {
        let native = owner
            .retain_native_c(
                source,
                &policy(""),
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
            )
            .unwrap();
        let policy = javascript_policy();
        let candidate = owner
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let [donor, target] = std::array::from_fn(|_| {
            owner
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&Plan::new(Style::Global)).unwrap();
                    output.retain_artifact(artifact).unwrap()
                })
                .unwrap()
        });
        let size = owner
            .measure_artifact(donor, CompressionCostModel::Gzip, WorkDomain::Baseline)
            .unwrap();
        let codec_work = owner.ledger().work_by_kind(WorkKind::Codec);
        owner
            .with_javascript_output(candidate, &policy, |output| {
                output.with_retained_arena(|arena, budget| {
                    let sizes = arena
                        .reuse_scores(target, Objectives::One(CompressionCostModel::Gzip), budget)
                        .unwrap();
                    assert_eq!(sizes.gzip9, Some(size));
                    assert!(
                        arena
                            .with_artifact(ArtifactId(native.artifact), |_| ())
                            .is_err()
                    );
                    assert!(arena.take(ArtifactId(native.artifact), budget).is_err());
                })
            })
            .unwrap();
        assert_eq!(owner.ledger().work_by_kind(WorkKind::Codec), codec_work);
        owner
            .with_qualified_native_artifact(&native, |view| assert!(!view.c.is_empty()))
            .unwrap();
    });
}
