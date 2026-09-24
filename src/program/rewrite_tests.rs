use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, TacticId,
    WorkDomain, WorkKind,
};
use crate::config::CompressionCostModel;
use crate::js::selection::{Plan, Style};
use crate::primitive::IntBinary;
use crate::program::publication::*;
use crate::program::*;
use std::process::Command;

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;
const SOURCE: &str = "export int answer(){return 3+4;}int other(){return 11;}";

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(from_checked_source(&syntax, &semantics).unwrap());
}

fn owner<'src>(slots: usize, memory: u64) -> Compilation<'src> {
    owner_with_deadline(slots, memory, false)
}

fn owner_with_deadline<'src>(slots: usize, memory: u64, deadline: bool) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits {
                wall_time_ms: deadline.then_some(100_000),
                ..ResourceLimits::default()
            },
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: memory,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: slots },
    )
    .unwrap()
}

fn policy(folding: bool, native: bool) -> ResolvedPolicy {
    let configuration: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\nconstant-folding='{}'\ndead-code-elimination='off'\ntarget-compaction='off'\n",
        if folding { "on" } else { "off" }
    ))
    .unwrap();
    configuration
        .resolve_policy(if native {
            CompilationRequest::Native
        } else {
            CompilationRequest::JavaScript {
                preserve_root_exports: true,
            }
        })
        .unwrap()
}

fn function(program: &Program<'_>, name: &str) -> UnitId {
    let cell = program.cells.iter().find(|cell| cell.name == name).unwrap();
    let CellBinding::Function(unit) = cell.binding else {
        panic!("expected function")
    };
    unit
}

fn binary(program: &Program<'_>, unit: UnitId) -> OpId {
    OpId::from_index(
        program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .position(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
            .unwrap(),
    )
    .unwrap()
}

fn set_binary(
    program: &mut Program<'_>,
    unit: UnitId,
    operator: IntBinary,
    left: i32,
    right: i32,
) -> OpId {
    let operation = binary(program, unit);
    let data = program.units[unit.index()].unique_edit().unwrap().0;
    let mut constants = data.operations.iter_mut().filter(|operation| {
        matches!(
            operation.kind,
            OperationKind::Constant(Constant::Integer(_))
        )
    });
    constants.next().unwrap().kind = OperationKind::Constant(Constant::Integer(left));
    constants.next().unwrap().kind = OperationKind::Constant(Constant::Integer(right));
    assert!(constants.next().is_none());
    data.operations[operation.index()].kind = OperationKind::IntBinary(operator);
    operation
}

fn commit(
    compiler: &mut Compilation<'_>,
    proof: CheckedRewrite,
    policy: &ResolvedPolicy,
) -> Result<SemanticId, RewriteError> {
    let started = compiler.ledger().work_used(WorkDomain::Optional);
    compiler.commit_checked_rewrite(proof, policy, WorkDomain::Optional, started, None)
}

/// Mutable access to a literal-fold claim, for the corruption tests.
fn literal(claim: &mut CheckedRewrite) -> &mut LiteralIntFold {
    match &mut claim.rule {
        RewriteRule::LiteralIntFold(fold) => fold,
        RewriteRule::DeadValueDrop(_) => panic!("expected a literal fold claim"),
    }
}

fn replace_constant(
    compiler: &mut Compilation<'_>,
    base: SemanticId,
    unit: UnitId,
    operation: OpId,
    integer: i32,
) -> SemanticId {
    let expected_revision = compiler.view(base).unwrap().unit_revision(unit).unwrap();
    let kind = OperationKind::Constant(Constant::Integer(integer));
    compiler
        .edit_source(
            base,
            &[UnitPatch {
                unit,
                expected_revision,
                operations: &[OperationPatch {
                    operation,
                    kind: &kind,
                    operands: &[],
                }],
                places: &[],
            }],
            WorkDomain::Baseline,
        )
        .unwrap()
}

fn render(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    policy: &ResolvedPolicy,
) -> ArtifactId {
    let candidate = compiler
        .direct_javascript(source, policy, WorkDomain::Baseline)
        .unwrap();
    let artifact = compiler
        .with_javascript_output_choices_in(
            candidate,
            policy,
            OutputTactics {
                dead_code_elimination: false,
                target_compaction: false,
                literals: LiteralOutput::Original,
                families: crate::js::OutputFamilies::NONE,
            },
            WorkDomain::Baseline,
            |output| {
                let artifact = output.render(&Plan::new(Style::Source)).unwrap();
                output.retain_artifact(artifact).unwrap()
            },
        )
        .unwrap();
    compiler.discard(candidate.semantic_id()).unwrap();
    artifact
}

fn observe(compiler: &Compilation<'_>, artifact: ArtifactId, body: &str) -> String {
    let javascript = compiler
        .with_artifact(artifact, |view| view.javascript.to_owned())
        .unwrap();
    let script = format!(
        "const library=await import('data:text/javascript,'+encodeURIComponent({}));\n{body}",
        serde_json::to_string(&javascript).unwrap()
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn matches_full_index(compiler: &mut Compilation<'_>, source: SemanticId) {
    compiler
        .with_semantic(source, |program, uses, ledger| {
            assert!(uses.valid_for(program));
            let rebuilt = uses::UseIndex::build(program, ledger, WorkDomain::Baseline).unwrap();
            for unit in &program.units {
                let actual = uses.unit(unit.id()).unwrap();
                let expected = rebuilt.unit(unit.id()).unwrap();
                assert_eq!(actual.cell_uses(), expected.cell_uses());
                assert_eq!(actual.closures(), expected.closures());
                for index in 0..unit.data().values.len() {
                    let value = ValueId::from_index(index).unwrap();
                    assert_eq!(actual.value_uses(value), expected.value_uses(value));
                }
                assert_eq!(
                    uses.creators(unit.id()).unwrap().sites(),
                    rebuilt.creators(unit.id()).unwrap().sites()
                );
            }
            for index in 0..program.cells.len() {
                let cell = CellId::from_index(index).unwrap();
                assert_eq!(
                    uses.cell(cell).unwrap().sites(),
                    rebuilt.cell(cell).unwrap().sites()
                );
                assert_eq!(
                    uses.cell(cell).unwrap().reference_exposed(),
                    rebuilt.cell(cell).unwrap().reference_exposed()
                );
            }
            rebuilt.discard(ledger).unwrap();
        })
        .unwrap();
}

#[test]
fn literal_integer_fold_preserves_parent_and_freshens_only_changed_unit_facts() {
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let other = function(&program, "other");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true, false);
        let parent = render(&mut compiler, source, &policy);
        compiler
            .enable_local_facts(
                facts::CacheLimits {
                    entries: 4,
                    bytes: 100_000,
                    result_bytes: 20_000,
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        let request = LocalFactsRequest {
            work_quota: 50_000,
            result_bytes: 20_000,
        };
        compiler
            .with_local_facts(WorkDomain::Baseline, 2, |session| {
                assert!(!session.query(source, unit, request).unwrap().cache_hit());
                assert!(!session.query(source, other, request).unwrap().cache_hit());
            })
            .unwrap();
        let previous = compiler.view(source).unwrap().unit_revision(unit).unwrap();
        let unchanged = compiler.view(source).unwrap().unit_revision(other).unwrap();
        let work = compiler.ledger().work_used(WorkDomain::Optional);
        let retained = compiler.ledger().retained_bytes();
        let folded = compiler
            .fold_literal_int_binary(source, unit, operation, &policy, WorkDomain::Optional)
            .unwrap()
            .unwrap();
        let receipt = compiler.view(folded).unwrap().receipt();
        assert_eq!(
            receipt.logical_work,
            compiler.ledger().work_used(WorkDomain::Optional) - work
        );
        assert_eq!(
            receipt.allocated_bytes,
            compiler.ledger().retained_bytes() - retained
        );
        assert_ne!(
            compiler.view(folded).unwrap().unit_revision(unit),
            Some(previous)
        );
        assert_eq!(
            compiler.view(source).unwrap().unit_revision(unit),
            Some(previous)
        );
        assert_eq!(
            compiler.view(folded).unwrap().unit_revision(other),
            Some(unchanged)
        );
        let view = compiler.view(folded).unwrap();
        assert!(matches!(
            view.unit(unit).unwrap().operations[operation.index()].kind,
            OperationKind::Constant(Constant::Integer(7))
        ));
        compiler
            .with_local_facts(WorkDomain::Baseline, 2, |session| {
                assert!(!session.query(folded, unit, request).unwrap().cache_hit());
                assert!(session.query(folded, other, request).unwrap().cache_hit());
            })
            .unwrap();
        matches_full_index(&mut compiler, source);
        matches_full_index(&mut compiler, folded);
        let result = render(&mut compiler, folded, &policy);
        assert_eq!(
            observe(&compiler, parent, "console.log(library.answer());"),
            "7\n"
        );
        assert_eq!(
            observe(&compiler, result, "console.log(library.answer());"),
            "7\n"
        );
        compiler.discard(source).unwrap();
        compiler.discard(folded).unwrap();
        assert_eq!(
            observe(&compiler, parent, "console.log(library.answer());"),
            "7\n"
        );
        assert_eq!(
            observe(&compiler, result, "console.log(library.answer());"),
            "7\n"
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn integer_annotations_do_not_authorize_literal_folding_or_erase_raw_js_coercions() {
    checked("export int run(int value){return value+1;}", |program| {
        let unit = function(&program, "run");
        let operation = binary(&program, unit);
        let mut compiler = owner(4, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true, false);
        let before = compiler.ledger().retained_bytes();
        assert!(compiler
            .fold_literal_int_binary(source, unit, operation, &policy, WorkDomain::Optional)
            .unwrap()
            .is_none());
        assert_eq!(compiler.ledger().retained_bytes(), before);
        assert_eq!(compiler.checkpoint_count(), 1);
        let artifact = render(&mut compiler, source, &policy);
        assert_eq!(
            observe(
                &compiler,
                artifact,
                r#"
            const trace=[], sentinel={};
            trace.push(library.run({valueOf(){trace.push("valueOf");return 4;}}));
            trace.push(library.run({[Symbol.toPrimitive](hint){trace.push(hint);return "7";}}));
            try{library.run({[Symbol.toPrimitive](){trace.push("throw");throw sentinel;}})}catch(error){trace.push(error===sentinel)}
            try{library.run(Symbol("value"))}catch(error){trace.push(error instanceof TypeError)}
            console.log(JSON.stringify(trace));
        "#
            ),
            "[\"valueOf\",5,\"default\",71,\"throw\",true,true]\n"
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn literal_integer_fold_matches_i32_contract_and_independent_javascript_execution() {
    for (operator, left, right, expected) in [
        (IntBinary::Add, 3, 4, 7),
        (IntBinary::Add, i32::MAX, 1, i32::MIN),
        (IntBinary::Subtract, i32::MIN, 1, i32::MAX),
        (IntBinary::Multiply, i32::MAX, i32::MAX, 0),
        (IntBinary::Multiply, 0, -1, 0),
        (IntBinary::Divide, i32::MIN, -1, i32::MIN),
        (IntBinary::Divide, 7, 0, 0),
        (IntBinary::Divide, -7, 2, -3),
        (IntBinary::Remainder, i32::MIN, -1, 0),
        (IntBinary::Remainder, -7, 0, 0),
        (IntBinary::Remainder, -4, 2, 0),
        (IntBinary::UnsignedShiftRight, -1, 0, -1),
        (IntBinary::UnsignedShiftRight, -1, 32, -1),
        (IntBinary::UnsignedShiftRight, -1, 1, i32::MAX),
        (IntBinary::UnsignedShiftRight, i32::MIN, -1, 1),
    ] {
        assert_eq!(operator.evaluate(left, right), expected);
        checked(SOURCE, |mut program| {
            let unit = function(&program, "answer");
            let operation = set_binary(&mut program, unit, operator, left, right);
            let mut compiler = owner(6, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let policy = policy(true, false);
            let before = render(&mut compiler, source, &policy);
            let folded = compiler
                .fold_literal_int_binary(source, unit, operation, &policy, WorkDomain::Optional)
                .unwrap()
                .unwrap();
            let after = render(&mut compiler, folded, &policy);
            let expected = format!("[{expected},false]\n");
            for artifact in [before, after] {
                assert_eq!(observe(&compiler, artifact, "const value=library.answer();console.log(JSON.stringify([value,Object.is(value,-0)]));"), expected, "{operator:?}({left},{right})");
            }
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn source_change_can_replace_seven_with_nine_but_cannot_claim_equivalence() {
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true, false);
        let proof = compiler
            .prepare_literal_int_fold(source, unit, operation, &policy, WorkDomain::Optional)
            .unwrap()
            .unwrap();
        let folded = commit(&mut compiler, proof, &policy).unwrap();
        let baseline = render(&mut compiler, folded, &policy);
        let baseline = compiler
            .qualify_artifact(
                baseline,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        let changed = replace_constant(&mut compiler, folded, unit, operation, 9);
        assert_ne!(
            compiler.view(folded).unwrap().meaning_identity(),
            compiler.view(changed).unwrap().meaning_identity()
        );
        assert!(compiler
            .fold_literal_int_binary(folded, unit, operation, &policy, WorkDomain::Optional)
            .unwrap()
            .is_none());
        let mut false_claim = proof;
        let view = compiler.view(folded).unwrap();
        false_claim.base = folded;
        false_claim.snapshot = view.snapshot_identity();
        false_claim.meaning = view.meaning_identity();
        false_claim.unit_revision = view.unit_revision(unit).unwrap();
        literal(&mut false_claim).result = 9;
        let count = compiler.checkpoint_count();
        let bytes = compiler.ledger().retained_bytes();
        assert!(commit(&mut compiler, false_claim, &policy).is_err());
        assert_eq!(compiler.checkpoint_count(), count);
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        let changed_artifact = render(&mut compiler, changed, &policy);
        assert_eq!(
            observe(
                &compiler,
                changed_artifact,
                "console.log(library.answer());"
            ),
            "9\n"
        );
        assert!(compiler
            .qualify_artifact(
                changed_artifact,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                Some(&baseline),
                WorkDomain::Baseline
            )
            .is_err());
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn checked_fold_rejects_wrong_result_rule_version_dependencies_and_policy() {
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true, false);
        let proof = compiler
            .prepare_literal_int_fold(source, unit, operation, &policy, WorkDomain::Optional)
            .unwrap()
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        for corruption in 0..7 {
            let mut claim = proof;
            match corruption {
                0 => literal(&mut claim).result = 9,
                1 => literal(&mut claim).version += 1,
                2 => literal(&mut claim).left += 1,
                3 => claim.tables = RevisionId::fresh(),
                4 => claim.unit_revision = RevisionId::fresh(),
                5 => claim.snapshot = RevisionId::fresh(),
                6 => claim.policy_fingerprint[0] ^= 1,
                _ => unreachable!(),
            }
            assert!(
                commit(&mut compiler, claim, &policy).is_err(),
                "corruption {corruption}"
            );
            assert_eq!(compiler.ledger().retained_bytes(), before);
            assert_eq!(compiler.checkpoint_count(), 1);
            assert!(matches!(
                compiler
                    .view(source)
                    .unwrap()
                    .unit(unit)
                    .unwrap()
                    .operations[operation.index()]
                .kind,
                OperationKind::IntBinary(IntBinary::Add)
            ));
        }
        let folded = commit(&mut compiler, proof, &policy).unwrap();
        assert_eq!(compiler.view(folded).unwrap().rewrites().len(), 1);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn fold_proofs_are_owner_and_snapshot_bound_without_invalidating_live_parent_proofs() {
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true, false);
        let proof = compiler
            .prepare_literal_int_fold(source, unit, operation, &policy, WorkDomain::Optional)
            .unwrap()
            .unwrap();
        let mut foreign = owner(2, MEMORY);
        let before = foreign.ledger().retained_bytes();
        assert!(commit(&mut foreign, proof, &policy).is_err());
        assert_eq!(foreign.ledger().retained_bytes(), before);
        assert_eq!(foreign.finish().retained_bytes(), 0);
        let changed = replace_constant(&mut compiler, source, unit, operation, 9);
        let mut wrong_base = proof;
        wrong_base.base = changed;
        assert!(commit(&mut compiler, wrong_base, &policy).is_err());
        let folded = commit(&mut compiler, proof, &policy).unwrap();
        assert_eq!(
            compiler.view(source).unwrap().meaning_identity(),
            compiler.view(folded).unwrap().meaning_identity()
        );
        let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
        let kind = OperationKind::Constant(Constant::Integer(9));
        compiler
            .advance_source(
                source,
                &[UnitPatch {
                    unit,
                    expected_revision: revision,
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
        assert!(commit(&mut compiler, proof, &policy).is_err());
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn equivalent_snapshots_keep_distinct_recipe_identity_but_share_checked_baseline_meaning() {
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true, false);
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let folded = compiler
            .fold_literal_int_binary(
                direct.semantic_id(),
                unit,
                operation,
                &policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .unwrap();
        let folded_direct = compiler
            .direct_javascript(folded, &policy, WorkDomain::Baseline)
            .unwrap();
        let (snapshot, meaning) = {
            let view = compiler.view(source).unwrap();
            (view.snapshot_identity(), view.meaning_identity())
        };
        let view = compiler.view(folded).unwrap();
        assert_ne!(view.snapshot_identity(), snapshot);
        assert_eq!(view.meaning_identity(), meaning);
        let steps = view.rewrites().steps().collect::<Vec<_>>();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].before, snapshot);
        assert_eq!(steps[0].after, view.snapshot_identity());
        assert_eq!(steps[0].meaning, meaning);
        assert_eq!(steps[0].rule.literal_fold().unwrap().result, 7);
        assert!(compiler
            .combine_javascript(direct, folded_direct, &policy, WorkDomain::Optional)
            .is_err());
        let before_artifact = render(&mut compiler, source, &policy);
        let after_artifact = render(&mut compiler, folded, &policy);
        compiler
            .with_artifact(before_artifact, |view| {
                assert_eq!(view.implementation.snapshot_identity(), Some(snapshot));
                assert_eq!(view.implementation.meaning_identity(), Some(meaning));
                assert!(view.implementation.rewrites().is_empty());
                assert!(view.implementation.whole_words().is_some());
            })
            .unwrap();
        compiler
            .with_artifact(after_artifact, |view| {
                assert_ne!(view.implementation.snapshot_identity(), Some(snapshot));
                assert_eq!(view.implementation.meaning_identity(), Some(meaning));
                assert_eq!(
                    view.implementation.rewrites().steps().collect::<Vec<_>>(),
                    steps
                );
                assert!(view.implementation.whole_words().is_none());
            })
            .unwrap();
        let baseline = compiler
            .qualify_artifact(
                before_artifact,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        compiler.discard_artifact(before_artifact).unwrap();
        let qualified = compiler
            .qualify_artifact(
                after_artifact,
                &policy,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                Some(&baseline),
                WorkDomain::Baseline,
            )
            .unwrap();
        compiler.discard(source).unwrap();
        compiler.discard(direct.semantic_id()).unwrap();
        compiler.discard(folded).unwrap();
        compiler.discard(folded_direct.semantic_id()).unwrap();
        compiler
            .with_qualified_artifact(&qualified, |view, provenance| {
                assert_eq!(
                    view.implementation.rewrites().steps().collect::<Vec<_>>(),
                    steps
                );
                assert!(provenance
                    .tactics()
                    .iter()
                    .any(|usage| usage.tactic == TacticId::ConstantFolding));
            })
            .unwrap();
        assert_eq!(
            observe(&compiler, after_artifact, "console.log(library.answer());"),
            "7\n"
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn folding_permission_survives_original_literal_output_source_edits_and_artifact_retention() {
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let enabled = policy(true, false);
        let disabled = policy(false, false);
        let before = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.fold_literal_int_binary(
                source,
                unit,
                operation,
                &disabled,
                WorkDomain::Optional
            ),
            Err(RewriteError::ForbiddenTactic(TacticId::ConstantFolding))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), before);
        compiler
            .enable_local_facts(
                facts::CacheLimits {
                    entries: 2,
                    bytes: 100_000,
                    result_bytes: 20_000,
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        compiler
            .with_local_facts(WorkDomain::Baseline, 1, |session| {
                assert!(!session
                    .query(
                        source,
                        unit,
                        LocalFactsRequest {
                            work_quota: 50_000,
                            result_bytes: 20_000
                        }
                    )
                    .unwrap()
                    .cache_hit());
            })
            .unwrap();
        let proof = compiler
            .prepare_literal_int_fold(source, unit, operation, &enabled, WorkDomain::Optional)
            .unwrap()
            .unwrap();
        assert!(commit(&mut compiler, proof, &disabled).is_err());
        let folded = commit(&mut compiler, proof, &enabled).unwrap();
        assert!(matches!(
            compiler.direct_javascript(folded, &disabled, WorkDomain::Baseline),
            Err(CandidateError::ForbiddenTactic(TacticId::ConstantFolding))
        ));
        let candidate = compiler
            .direct_javascript(folded, &enabled, WorkDomain::Baseline)
            .unwrap();
        assert!(matches!(
            compiler.with_javascript_output(candidate, &disabled, |_| ()),
            Err(CandidateError::ForbiddenTactic(TacticId::ConstantFolding))
        ));
        let artifact = render(&mut compiler, folded, &enabled);
        let changed = replace_constant(&mut compiler, folded, unit, operation, 9);
        assert!(matches!(
            compiler.direct_javascript(changed, &disabled, WorkDomain::Baseline),
            Err(CandidateError::ForbiddenTactic(TacticId::ConstantFolding))
        ));
        let changed_view = compiler.view(changed).unwrap();
        assert_eq!(changed_view.rewrites().len(), 1);
        assert_ne!(
            changed_view.rewrites().steps().next().unwrap().meaning,
            changed_view.meaning_identity(),
            "inherited permission history is not a proof for the new source meaning"
        );
        compiler.discard(candidate.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        compiler.discard(folded).unwrap();
        compiler.discard(changed).unwrap();
        let qualification = compiler.qualify_artifact(
            artifact,
            &disabled,
            CompressionCostModel::Raw,
            ArtifactRuntimeEvidence::default(),
            None,
            WorkDomain::Baseline,
        );
        assert!(
            matches!(
                qualification,
                Err(CandidateError::ForbiddenTactic(TacticId::ConstantFolding))
            ),
            "retained checked-fold provenance must reject folding-off policy: {qualification:?}"
        );
        compiler
            .qualify_artifact(
                artifact,
                &enabled,
                CompressionCostModel::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn native_formation_cannot_erase_checked_fold_permission() {
    checked("int answer(){return 3+4;}print(answer());", |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(4, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let enabled = policy(true, true);
        let disabled = policy(false, true);
        let folded = compiler
            .fold_literal_int_binary(source, unit, operation, &enabled, WorkDomain::Optional)
            .unwrap()
            .unwrap();
        let bytes = compiler.ledger().retained_bytes();
        assert!(compiler
            .with_native_c(folded, &disabled, WorkDomain::Baseline, |_| ())
            .is_err());
        assert!(compiler
            .retain_native_c(
                folded,
                &disabled,
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline
            )
            .is_err());
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        let receipt = compiler
            .retain_native_c(
                folded,
                &enabled,
                ArtifactRuntimeEvidence::default(),
                WorkDomain::Baseline,
            )
            .unwrap();
        let view = compiler.view(folded).unwrap();
        let snapshot = view.snapshot_identity();
        let meaning = view.meaning_identity();
        let steps = view.rewrites().steps().collect::<Vec<_>>();
        assert_eq!(steps.len(), 1);
        let (c, header, c_pointer, header_pointer) = compiler
            .with_qualified_native_artifact(&receipt, |view| {
                assert_eq!(view.snapshot, snapshot);
                assert_eq!(view.meaning, meaning);
                assert_eq!(view.rewrites.steps().collect::<Vec<_>>(), steps);
                (
                    view.c.to_owned(),
                    view.header.to_owned(),
                    view.c.as_ptr(),
                    view.header.as_ptr(),
                )
            })
            .unwrap();
        compiler.discard(source).unwrap();
        compiler.discard(folded).unwrap();
        compiler
            .with_qualified_native_artifact(&receipt, |view| {
                assert_eq!(view.snapshot, snapshot);
                assert_eq!(view.meaning, meaning);
                assert_eq!(view.rewrites.steps().collect::<Vec<_>>(), steps);
                assert_eq!(view.c, c);
                assert_eq!(view.header, header);
            })
            .unwrap();
        let (delivered_c, delivered_header) =
            compiler.take_qualified_native_artifact(receipt).unwrap();
        assert!(!delivered_c.is_empty());
        assert_eq!(delivered_c, c);
        assert_eq!(delivered_header, header);
        assert_eq!(delivered_c.as_ptr(), c_pointer);
        assert_eq!(delivered_header.as_ptr(), header_pointer);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn checked_integer_folds_preserve_native_overflow_division_and_binary64_multiplication() {
    checked(
        "int overflow(){return 3+4;}int divide(){return 3+4;}int minimum(){return 3+4;}int multiply(){return 3+4;}int shift(){return 3+4;}int remainder(){return 3+4;}print(overflow());print(divide());print(minimum());print(multiply());print(shift());print(remainder());",
        |mut program| {
            let sites = [
                ("overflow", IntBinary::Add, i32::MAX, 1),
                ("divide", IntBinary::Divide, 7, 0),
                ("minimum", IntBinary::Divide, i32::MIN, -1),
                ("multiply", IntBinary::Multiply, i32::MAX, i32::MAX),
                ("shift", IntBinary::UnsignedShiftRight, -1, 32),
                ("remainder", IntBinary::Remainder, i32::MIN, -1),
            ].map(|(name, operator, left, right)| {
                let unit = function(&program, name);
                (unit, set_binary(&mut program, unit, operator, left, right))
            });
            let mut compiler = owner(10, MEMORY);
            let source = compiler.adopt_checked(program, WorkDomain::Baseline).unwrap();
            let policy = policy(true, true);
            let before = compiler.retain_native_c(source, &policy, ArtifactRuntimeEvidence::default(), WorkDomain::Baseline).unwrap();
            let mut folded = source;
            for (unit, operation) in sites {
                folded = compiler.fold_literal_int_binary(folded, unit, operation, &policy, WorkDomain::Optional).unwrap().unwrap();
            }
            assert_eq!(compiler.view(folded).unwrap().rewrites().len(), sites.len());
            assert_eq!(compiler.view(source).unwrap().meaning_identity(), compiler.view(folded).unwrap().meaning_identity());
            let after = compiler.retain_native_c(folded, &policy, ArtifactRuntimeEvidence::default(), WorkDomain::Baseline).unwrap();
            let expected = "-2147483648\n0\n-2147483648\n0\n-1\n0\n";
            for (name, receipt) in [("rewrite-before", before), ("rewrite-after", after)] {
                let (c, header) = compiler.take_qualified_native_artifact(receipt).unwrap();
                assert!(header.is_empty());
                let observations = crate::program::native_tests::compile_and_execute(&c, expected, name);
                assert_eq!(observations.len(), 7);
            }
            assert_eq!(compiler.finish().retained_bytes(), 0);
        },
    );
}

#[test]
fn refused_fold_work_memory_and_slots_preserve_source_and_resource_accounting() {
    let mut successful_work = 0;
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = binary(&program, unit);
        let mut compiler = owner(4, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler
            .fold_literal_int_binary(
                source,
                unit,
                operation,
                &policy(true, false),
                WorkDomain::Optional,
            )
            .unwrap()
            .unwrap();
        successful_work = compiler.ledger().work_used(WorkDomain::Optional);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    assert!(successful_work > 16);
    for mode in 0..4 {
        checked(SOURCE, |program| {
            let unit = function(&program, "answer");
            let operation = binary(&program, unit);
            let mut compiler = owner(if mode == 3 { 1 } else { 4 }, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let view = compiler.view(source).unwrap();
            let identity = (
                view.snapshot_identity(),
                view.meaning_identity(),
                view.unit_revision(unit),
            );
            let retained = compiler.ledger().retained_bytes();
            let padding = compiler
                .with_semantic(source, |_, _, ledger| {
                    if mode == 2 {
                        let padding = MEMORY - ledger.retained_bytes();
                        ledger.retain(WorkDomain::Optional, padding).unwrap();
                        padding
                    } else {
                        if mode < 2 {
                            let available = if mode == 0 { 0 } else { successful_work - 1 };
                            ledger
                                .charge(WorkDomain::Optional, WorkKind::Edit, WORK - available)
                                .unwrap();
                        }
                        0
                    }
                })
                .unwrap();
            let work = compiler.ledger().work_used(WorkDomain::Optional);
            let result = compiler.fold_literal_int_binary(
                source,
                unit,
                operation,
                &policy(true, false),
                WorkDomain::Optional,
            );
            assert!(result.is_err(), "resource refusal mode {mode}");
            if padding != 0 {
                compiler
                    .with_semantic(source, |_, _, ledger| {
                        ledger.release(WorkDomain::Optional, padding).unwrap()
                    })
                    .unwrap();
            }
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            assert!(compiler.ledger().work_used(WorkDomain::Optional) >= work);
            assert_eq!(compiler.checkpoint_count(), 1);
            let view = compiler.view(source).unwrap();
            assert_eq!(
                (
                    view.snapshot_identity(),
                    view.meaning_identity(),
                    view.unit_revision(unit)
                ),
                identity
            );
            assert!(view.rewrites().is_empty());
            assert!(matches!(
                view.unit(unit).unwrap().operations[operation.index()].kind,
                OperationKind::IntBinary(IntBinary::Add)
            ));
            matches_full_index(&mut compiler, source);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn checked_fold_unwind_and_deadline_restore_parent_indices_identities_and_artifacts() {
    for phase in 0..3 {
        for panic in [false, true] {
            checked(SOURCE, |program| {
                let unit = function(&program, "answer");
                let operation = binary(&program, unit);
                let mut compiler = owner_with_deadline(5, MEMORY, !panic);
                let source = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let policy = policy(true, false);
                let parent = render(&mut compiler, source, &policy);
                let before = compiler.ledger().retained_bytes();
                let work = compiler.ledger().work_used(WorkDomain::Optional);
                let view = compiler.view(source).unwrap();
                let identity = (
                    view.snapshot_identity(),
                    view.meaning_identity(),
                    view.unit_revision(unit),
                );
                edits::inject_rewrite_failure(phase, panic);
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    compiler.fold_literal_int_binary(
                        source,
                        unit,
                        operation,
                        &policy,
                        WorkDomain::Optional,
                    )
                }));
                if panic {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(compiler.ledger().retained_bytes(), before);
                assert!(compiler.ledger().work_used(WorkDomain::Optional) > work);
                assert_eq!(compiler.checkpoint_count(), 1);
                let view = compiler.view(source).unwrap();
                assert_eq!(
                    (
                        view.snapshot_identity(),
                        view.meaning_identity(),
                        view.unit_revision(unit)
                    ),
                    identity
                );
                assert!(view.rewrites().is_empty());
                assert!(matches!(
                    view.unit(unit).unwrap().operations[operation.index()].kind,
                    OperationKind::IntBinary(IntBinary::Add)
                ));
                // Deadline expiry remains sticky, so rebuild admission is only
                // exercised for the unwind cases; retained output needs no work.
                if panic {
                    matches_full_index(&mut compiler, source);
                }
                assert_eq!(
                    observe(&compiler, parent, "console.log(library.answer());"),
                    "7\n"
                );
                assert_eq!(compiler.finish().retained_bytes(), 0);
            });
        }
    }
}

fn dce_policy(dead_code: bool, folding: bool) -> ResolvedPolicy {
    let configuration: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\nconstant-folding='{}'\ndead-code-elimination='{}'\ntarget-compaction='off'\n",
        if folding { "on" } else { "off" },
        if dead_code { "on" } else { "off" },
    ))
    .unwrap();
    configuration
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn with_facts(compiler: &mut Compilation<'_>) -> LocalFactsRequest {
    compiler
        .enable_local_facts(
            facts::CacheLimits {
                entries: 64,
                bytes: 4_000_000,
                result_bytes: 400_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    LocalFactsRequest {
        work_quota: 1_000_000,
        result_bytes: 400_000,
    }
}

/// Retire dead values in `unit` until none remains, one checked step at a
/// time, returning the final snapshot and how many steps were published.
fn drop_to_fixpoint(
    compiler: &mut Compilation<'_>,
    mut snapshot: SemanticId,
    unit: UnitId,
    policy: &ResolvedPolicy,
    facts: LocalFactsRequest,
) -> (SemanticId, usize) {
    let mut steps = 0;
    loop {
        let count = compiler
            .view(snapshot)
            .unwrap()
            .unit(unit)
            .unwrap()
            .operations
            .len();
        let mut progressed = false;
        for index in 0..count {
            let operation = OpId::from_index(index).unwrap();
            if let Some(next) = compiler
                .drop_dead_value(
                    snapshot,
                    unit,
                    operation,
                    policy,
                    WorkDomain::Optional,
                    facts,
                )
                .unwrap()
            {
                snapshot = next;
                steps += 1;
                progressed = true;
                break;
            }
        }
        if !progressed {
            return (snapshot, steps);
        }
    }
}

fn kinds(compiler: &Compilation<'_>, snapshot: SemanticId, unit: UnitId) -> Vec<OperationKind> {
    compiler
        .view(snapshot)
        .unwrap()
        .unit(unit)
        .unwrap()
        .operations
        .iter()
        .map(|operation| operation.kind.clone())
        .collect()
}

#[test]
fn dead_local_computations_cascade_away_through_checked_steps() {
    // `b` and `c` are written and never read. Their arithmetic, and the loads
    // that fed only that arithmetic, retire one published step at a time;
    // `a`, which the function returns, is untouched.
    checked(
        "export int run(){int a=3;int b=a+4;int c=b*5;return a;}",
        |program| {
            let unit = function(&program, "run");
            let mut compiler = owner(64, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let policy = dce_policy(true, false);
            let facts = with_facts(&mut compiler);
            let before = kinds(&compiler, source, unit);
            let parent = render(&mut compiler, source, &policy);
            let (cleaned, steps) = drop_to_fixpoint(&mut compiler, source, unit, &policy, facts);
            assert!(
                steps >= 3,
                "expected the multiply, the load of b and the add to retire, got {steps}"
            );
            let after = kinds(&compiler, cleaned, unit);
            assert_eq!(
                after
                    .iter()
                    .filter(|kind| matches!(kind, OperationKind::IntBinary(_)))
                    .count(),
                0,
                "{after:?}"
            );
            // The original snapshot survives unchanged beside every step.
            assert_eq!(
                format!("{:?}", kinds(&compiler, source, unit)),
                format!("{before:?}")
            );
            assert_eq!(compiler.view(cleaned).unwrap().rewrites().len(), steps);
            assert!(compiler
                .view(cleaned)
                .unwrap()
                .rewrites()
                .steps()
                .all(|step| step.rule.dead_value_drop().is_some()));
            matches_full_index(&mut compiler, cleaned);
            let child = render(&mut compiler, cleaned, &policy);
            let probe = "console.log(library.run());";
            assert_eq!(
                observe(&compiler, child, probe),
                observe(&compiler, parent, probe)
            );
            assert_eq!(observe(&compiler, child, probe), "3\n");
        },
    );
}

#[test]
fn a_dead_value_that_can_run_a_coercion_hook_is_kept() {
    // An exported `int` parameter still receives arbitrary JavaScript values,
    // so `value*3` can call `valueOf`. The facts summarize that as a coercion,
    // which may throw and reenter: dropping it would erase an observable hook.
    checked(
        "export int run(int value){int b=value*3;return 1;}",
        |program| {
            let unit = function(&program, "run");
            let operation = binary(&program, unit);
            let mut compiler = owner(8, MEMORY);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let policy = dce_policy(true, false);
            let facts = with_facts(&mut compiler);
            assert!(compiler
                .drop_dead_value(
                    source,
                    unit,
                    operation,
                    &policy,
                    WorkDomain::Optional,
                    facts
                )
                .unwrap()
                .is_none());
            assert_eq!(compiler.checkpoint_count(), 1);
            let artifact = render(&mut compiler, source, &policy);
            assert_eq!(
                observe(
                    &compiler,
                    artifact,
                    r#"const trace=[];library.run({valueOf(){trace.push("valueOf");return 2;}});console.log(JSON.stringify(trace));"#
                ),
                "[\"valueOf\"]\n"
            );
        },
    );
}

#[test]
fn a_value_that_is_read_is_not_retired() {
    checked("export int run(){int a=3;return a+1;}", |program| {
        let unit = function(&program, "run");
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = dce_policy(true, false);
        let facts = with_facts(&mut compiler);
        let (_, steps) = drop_to_fixpoint(&mut compiler, source, unit, &policy, facts);
        assert_eq!(steps, 0);
        assert_eq!(compiler.checkpoint_count(), 1);
    });
}

#[test]
fn dead_value_drop_obeys_policy_and_records_only_its_own_tactic() {
    checked("export int run(){int a=3;int b=a*7;return a;}", |program| {
        let unit = function(&program, "run");
        let operation = binary(&program, unit);
        let mut compiler = owner(16, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let facts = with_facts(&mut compiler);
        // The tactic is required.
        assert_eq!(
            compiler.drop_dead_value(
                source,
                unit,
                operation,
                &dce_policy(false, true),
                WorkDomain::Optional,
                facts
            ),
            Err(RewriteError::ForbiddenTactic(TacticId::DeadCodeElimination))
        );
        // Folding may be forbidden: this snapshot never folded anything.
        let dropped = compiler
            .drop_dead_value(
                source,
                unit,
                operation,
                &dce_policy(true, false),
                WorkDomain::Optional,
                facts,
            )
            .unwrap()
            .expect("the unused product retires");
        let steps = compiler
            .view(dropped)
            .unwrap()
            .rewrites()
            .steps()
            .collect::<Vec<_>>();
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].rule.tactic(), TacticId::DeadCodeElimination);
        // A later rule under a policy that forbids dead-code elimination is
        // refused, because the history it would extend depends on it.
        let load = OpId::from_index(0).unwrap();
        assert_eq!(
            compiler.drop_dead_value(
                dropped,
                unit,
                load,
                &dce_policy(false, true),
                WorkDomain::Optional,
                facts
            ),
            Err(RewriteError::ForbiddenTactic(TacticId::DeadCodeElimination))
        );
    });
}

#[test]
fn a_stale_or_corrupted_dead_value_proof_is_refused_without_publication() {
    checked("export int run(){int a=3;int b=a*7;return a;}", |program| {
        let unit = function(&program, "run");
        let operation = binary(&program, unit);
        let mut compiler = owner(16, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = dce_policy(true, false);
        let facts = with_facts(&mut compiler);
        let proof = compiler
            .prepare_dead_value_drop(
                source,
                unit,
                operation,
                &policy,
                WorkDomain::Optional,
                facts,
            )
            .unwrap()
            .expect("the unused product is retirable");
        let count = compiler.checkpoint_count();
        let bytes = compiler.ledger().retained_bytes();
        for corruption in 0..4 {
            let mut claim = proof;
            match corruption {
                0 => claim.unit_revision = RevisionId::fresh(),
                1 => claim.snapshot = RevisionId::fresh(),
                2 => claim.policy_fingerprint[0] ^= 1,
                3 => {
                    if let RewriteRule::DeadValueDrop(drop) = &mut claim.rule {
                        drop.replacement = NeutralConstant::Boolean;
                    }
                }
                _ => unreachable!(),
            }
            let started = compiler.ledger().work_used(WorkDomain::Optional);
            assert!(compiler
                .commit_checked_rewrite(claim, &policy, WorkDomain::Optional, started, Some(facts))
                .is_err());
            assert_eq!(
                compiler.checkpoint_count(),
                count,
                "corruption {corruption} published"
            );
            assert_eq!(compiler.ledger().retained_bytes(), bytes);
        }
    });
}

#[test]
fn dead_value_drop_requires_local_facts() {
    checked("export int run(){int a=3;int b=a*7;return a;}", |program| {
        let unit = function(&program, "run");
        let operation = binary(&program, unit);
        let mut compiler = owner(8, MEMORY);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let facts = LocalFactsRequest {
            work_quota: 1_000,
            result_bytes: 1_000,
        };
        assert_eq!(
            compiler.drop_dead_value(
                source,
                unit,
                operation,
                &dce_policy(true, false),
                WorkDomain::Optional,
                facts
            ),
            Err(RewriteError::FactsUnavailable)
        );
    });
}
