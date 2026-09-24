//! Rendering remains inspectable; only exact, policy-qualified records deploy.
use super::*;
use crate::compilation_policy::{
    AdmissionError, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::program::publication::{
    CheckpointLimit, Compilation, OperationPatch, SemanticId, UnitPatch,
};
use crate::program::{Constant, OpId, OperationKind, UnitId};
use crate::js::selection::Style;

fn policy(extra: &str, exports: bool) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\n{extra}"
    )).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: exports,
        })
        .unwrap()
}

fn with_source(inspect: impl FnOnce(&mut Compilation<'_>, SemanticId)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "export int answer(){return 17;}").unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = crate::program::from_checked_source(&syntax, &checked).unwrap();
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 100_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 128_000_000,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 8 }).unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    inspect(&mut compilation, source);
    assert_eq!(compilation.finish().retained_bytes(), 0);
}

fn render(
    compilation: &mut Compilation<'_>,
    source: SemanticId,
    policy: &ResolvedPolicy,
) -> ArtifactId {
    let candidate = compilation
        .direct_javascript(source, policy, WorkDomain::Baseline)
        .unwrap();
    let artifact = compilation
        .with_javascript_output(candidate, policy, |output| {
            let artifact = output.render(&Plan::new(Style::Scoped))?;
            output.retain_artifact(artifact)
        })
        .unwrap()
        .unwrap();
    compilation.discard(candidate.semantic_id()).unwrap();
    artifact
}

#[test]
fn direct_artifacts_require_exact_scores_and_known_required_runtime_evidence() {
    with_source(|compilation, source| {
        let bounded = policy("[policy.constraints]\nmax_startup_work=0\n", true);
        let artifact = render(compilation, source, &bounded);
        assert!(matches!(
            compilation.qualify_artifact(
                artifact,
                &bounded,
                CompressionCostModel::Brotli,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::Artifact(_))
        ));
        compilation
            .measure_artifact(artifact, CompressionCostModel::Brotli, WorkDomain::Baseline)
            .unwrap();
        assert!(matches!(
            compilation.qualify_artifact(
                artifact,
                &bounded,
                CompressionCostModel::Brotli,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::Admission(
                AdmissionError::MissingCostEvidence("startup work")
            ))
        ));
        let known = ArtifactRuntimeEvidence {
            startup_work: Some(0),
            ..ArtifactRuntimeEvidence::default()
        };
        let qualified = compilation
            .qualify_artifact(
                artifact,
                &bounded,
                CompressionCostModel::Brotli,
                known,
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        assert_eq!(qualified.policy_fingerprint(), bounded.fingerprint());
        compilation
            .with_qualified_artifact(&qualified, |view, provenance| {
                assert_eq!(provenance.naming().style, Style::Scoped);
                assert_eq!(
                    qualified.cost().transfer_bytes as usize,
                    crate::compression::measure(
                        view.javascript.as_bytes(),
                        CompressionCostModel::Brotli
                    )
                    .unwrap()
                );
                assert_eq!(qualified.cost().startup_work, Some(0));
                assert_eq!(qualified.cost().recurring_work, None);
            })
            .unwrap();
        let text = compilation.take_qualified_artifact(qualified).unwrap();
        assert!(!text.is_empty());
        assert!(compilation.take_qualified_artifact(qualified).is_err());
    });
}

#[test]
fn qualification_rejects_changed_formation_contract_but_inspection_remains_available() {
    with_source(|compilation, source| {
        let original = policy("", true);
        let artifact = render(compilation, source, &original);
        let changed = policy("", false);
        assert!(matches!(
            compilation.qualify_artifact(
                artifact,
                &changed,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::ContractMismatch)
        ));
        let bounded = policy("[policy.constraints]\nmax_recurring_work=0\n", true);
        assert!(matches!(
            compilation.qualify_artifact(
                artifact,
                &bounded,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::Admission(
                AdmissionError::MissingCostEvidence("recurring work")
            ))
        ));
        assert!(!compilation.take_artifact(artifact).unwrap().is_empty());
    });
}

#[test]
fn baseline_receipts_survive_byte_disposal_but_cannot_cross_source_meanings() {
    with_source(|compilation, source| {
        let policy = policy("", true);
        let baseline = render(compilation, source, &policy);
        let receipt = compilation
            .qualify_artifact(
                baseline,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        compilation.discard_artifact(baseline).unwrap();
        let sibling = render(compilation, source, &policy);
        compilation
            .qualify_artifact(
                sibling,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                Some(&receipt),
                WorkDomain::Baseline,
            )
            .unwrap();
        let (unit, expected_revision, operation) = {
            let view = compilation.view(source).unwrap();
            (0..view.unit_count())
                .find_map(|index| {
                    let unit = UnitId::from_index(index).unwrap();
                    let operation = view.unit(unit)?.operations.iter().position(|operation| {
                        matches!(
                            operation.kind,
                            OperationKind::Constant(Constant::Integer(17))
                        )
                    })?;
                    Some((
                        unit,
                        view.unit_revision(unit).unwrap(),
                        OpId::from_index(operation).unwrap(),
                    ))
                })
                .unwrap()
        };
        let replacement = OperationKind::Constant(Constant::Integer(19));
        let changed = compilation
            .edit_source(
                source,
                &[UnitPatch {
                    unit,
                    expected_revision,
                    operations: &[OperationPatch {
                        operation,
                        kind: &replacement,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Baseline,
            )
            .unwrap();
        let edited = render(compilation, changed, &policy);
        assert!(matches!(
            compilation.qualify_artifact(
                edited,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                Some(&receipt),
                WorkDomain::Baseline
            ),
            Err(CandidateError::Artifact(_))
        ));
        let constrained = self::policy("[policy.constraints]\nmax_runtime_memory_bytes=0\n", true);
        assert!(matches!(
            compilation.qualify_artifact(
                edited,
                &constrained,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::Admission(
                AdmissionError::MissingCostEvidence("runtime memory")
            ))
        ));
    });
}
