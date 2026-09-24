//! Real body-scoped proofs, independent storage choices and checked edits.
//! The fixed full-field cut has only one legal nondefault layout per body.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, TacticId,
    WorkDomain,
};

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}
fn compiler<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 30_000_000,
                optional_work: 30_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 30_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 16 },
    )
    .unwrap()
}
fn policy(text: &str) -> ResolvedPolicy {
    let mut config: crate::config::ProjectConfig = toml::from_str(text).unwrap();
    config.javascript.strip_console = false;
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn cell(program: &Program<'_>, name: &str) -> CellId {
    CellId::from_index(
        program
            .cells()
            .iter()
            .position(|cell| cell.name == name)
            .unwrap(),
    )
    .unwrap()
}
fn body(program: &Program<'_>, name: &str) -> UnitId {
    let CellBinding::Function(body) = program.cells()[cell(program, name).index()].binding else {
        panic!("named body")
    };
    body
}
fn request() -> FunctionRequest {
    FunctionRequest {
        max_work: 500_000,
        scratch_bytes: 800_000,
        output_bytes: 800_000,
    }
}
fn selected(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    body: UnitId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .scalar_function_javascript(base, body, request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        FunctionOutcome::Published(candidate) => candidate,
        other => panic!("{other:?}"),
    }
}
fn descriptor(compiler: &mut Compilation<'_>, candidate: CandidateId) -> Vec<u32> {
    compiler
        .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
        .unwrap()
}
const SOURCE: &str = "struct P{int x;int y;}int first(P a){return a.x;}int second(P b){return b.y;}P source=P{1,2};print(first(source));print(second(source));";

#[test]
fn body_layout_unions_are_canonical_and_independent_of_parameter_storage() {
    for reverse in [false, true] {
        checked(SOURCE, |program| {
            let first = body(&program, "first");
            let second = body(&program, "second");
            let parameter = program.unit(first).unwrap().parameters[0];
            let policy = policy("");
            let mut compiler = compiler();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Optional)
                .unwrap();
            let before = compiler.ledger().retained_bytes();
            let left = selected(&mut compiler, direct, first, &policy);
            let right = selected(&mut compiler, direct, first, &policy);
            assert_eq!(
                descriptor(&mut compiler, left),
                descriptor(&mut compiler, right)
            );
            let equal = compiler
                .combine_javascript(left, right, &policy, WorkDomain::Optional)
                .unwrap();
            assert_eq!(
                descriptor(&mut compiler, left),
                descriptor(&mut compiler, equal)
            );
            compiler
                .with_implementations(equal, |map| assert_eq!(map.functions().len(), 1))
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            assert!(matches!(
                compiler.scalar_function_javascript(
                    left,
                    first,
                    request(),
                    &policy,
                    WorkDomain::Optional
                ),
                Err(CandidateError::DuplicateRoot)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            let other = selected(&mut compiler, direct, second, &policy);
            let both = compiler
                .combine_javascript(equal, other, &policy, WorkDomain::Optional)
                .unwrap();
            compiler
                .with_implementations(both, |map| assert_eq!(map.functions().len(), 2))
                .unwrap();
            let ProductOutcome::Published(storage) = compiler
                .scalar_product_javascript(
                    direct,
                    parameter,
                    request(),
                    &policy,
                    WorkDomain::Optional,
                )
                .unwrap()
                .outcome
            else {
                panic!("parameter storage")
            };
            let a = compiler
                .combine_javascript(storage, left, &policy, WorkDomain::Optional)
                .unwrap();
            let b = compiler
                .combine_javascript(right, storage, &policy, WorkDomain::Optional)
                .unwrap();
            assert_eq!(descriptor(&mut compiler, a), descriptor(&mut compiler, b));
            compiler
                .with_implementations(a, |map| {
                    assert_eq!(map.products().len(), 1);
                    assert_eq!(map.functions().len(), 1);
                })
                .unwrap();
            let expected = descriptor(&mut compiler, a);
            let order = if reverse {
                vec![b, a, storage, both, other, equal, right, left]
            } else {
                vec![left, right, equal, other, both, storage, a, b]
            };
            for (index, candidate) in order.iter().enumerate() {
                compiler.discard(candidate.semantic_id()).unwrap();
                if order[index + 1..].contains(&b) {
                    assert_eq!(descriptor(&mut compiler, b), expected);
                }
            }
            assert_eq!(compiler.ledger().retained_bytes(), before);
            compiler.discard(direct.semantic_id()).unwrap();
            compiler.discard(source).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn checked_new_calls_and_opaque_writers_revoke_complete_layout_input_evidence() {
    for new_call in [false, true] {
        let source = if new_call {
            "struct P{int x;}extern P opaque();int first(P a){return a.x;}int second(P b){return b.x;}P safe=P{1};P bad=opaque();print(first(safe));print(second(bad));"
        } else {
            "struct P{int x;}extern P opaque();int first(P a){return a.x;}P safe=P{1};P other=P{2};P bad=opaque();other=bad;print(first(safe));"
        };
        checked(source, |program| {
            let first = body(&program, "first");
            let target = cell(&program, if new_call { "first" } else { "safe" });
            let previous = cell(&program, if new_call { "second" } else { "other" });
            let owner = program.cells()[previous.index()].owner;
            let data = program.unit(owner).unwrap();
            let place = data
                .operations
                .iter()
                .find_map(|operation| match operation.kind {
                    OperationKind::Load(place)
                        if new_call && data.places[place.index()] == Place::Cell(previous) =>
                    {
                        Some(place)
                    }
                    OperationKind::Store(place)
                        if !new_call && data.places[place.index()] == Place::Cell(previous) =>
                    {
                        Some(place)
                    }
                    _ => None,
                })
                .unwrap();
            let policy = policy("");
            let mut compiler = compiler();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Optional)
                .unwrap();
            let selected = selected(&mut compiler, direct, first, &policy);
            let identity = descriptor(&mut compiler, selected);
            let revision = compiler.view(source).unwrap().unit_revision(owner).unwrap();
            let changed = compiler
                .edit_source(
                    source,
                    &[UnitPatch {
                        unit: owner,
                        expected_revision: revision,
                        operations: &[],
                        places: &[PlacePatch {
                            place,
                            replacement: &Place::Cell(target),
                        }],
                    }],
                    WorkDomain::Optional,
                )
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            assert!(matches!(
                compiler.rebase_javascript(selected, changed, &policy, WorkDomain::Optional),
                Err(CandidateError::StaleEvidence)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            assert_eq!(descriptor(&mut compiler, selected), identity);
            let next = compiler
                .direct_javascript(changed, &policy, WorkDomain::Optional)
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            let result = compiler
                .scalar_function_javascript(next, first, request(), &policy, WorkDomain::Optional)
                .unwrap();
            assert!(
                matches!(
                    result.outcome,
                    FunctionOutcome::Unknown(FunctionUnknownReason::UnsupportedProducer)
                ),
                "new_call={new_call}: {:?}",
                result.outcome
            );
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            for candidate in [selected, direct, next] {
                compiler.discard(candidate.semantic_id()).unwrap();
            }
            compiler.discard(source).unwrap();
            compiler.discard(changed).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn function_permissions_and_refused_proofs_preserve_existing_owners() {
    checked(SOURCE, |program| {
        let first = body(&program, "first");
        let policy = policy("");
        let off = self::policy("[policy.tactics]\ncall-specialization='off'\n");
        let mut compiler = compiler();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Optional)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let work = compiler.ledger().work_used(WorkDomain::Optional);
        assert!(matches!(
            compiler.scalar_function_javascript(
                direct,
                first,
                request(),
                &off,
                WorkDomain::Optional
            ),
            Err(CandidateError::ForbiddenTactic(
                TacticId::CallSpecialization
            ))
        ));
        assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), work);
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        for axis in 0..3 {
            let mut request = request();
            match axis {
                0 => request.max_work = 0,
                1 => request.scratch_bytes = 0,
                _ => request.output_bytes = 0,
            };
            let result = compiler
                .scalar_function_javascript(direct, first, request, &policy, WorkDomain::Optional)
                .unwrap();
            assert!(
                matches!(result.outcome, FunctionOutcome::Truncated(_)),
                "{result:?}"
            );
            assert_eq!(compiler.ledger().retained_bytes(), retained);
        }
        let chosen = selected(&mut compiler, direct, first, &policy);
        let retained = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.rebase_javascript(chosen, source, &off, WorkDomain::Optional),
            Err(CandidateError::ForbiddenTactic(
                TacticId::CallSpecialization
            ))
        ));
        let mut observed = false;
        assert!(matches!(
            compiler.with_javascript_output(chosen, &off, |_| observed = true),
            Err(CandidateError::ForbiddenTactic(
                TacticId::CallSpecialization
            ))
        ));
        assert!(!observed);
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        compiler.discard(direct.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        compiler.discard(chosen.semantic_id()).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn failed_layout_publication_releases_completed_evidence_and_partial_map_storage() {
    let mut measured = None;
    checked(SOURCE, |program| {
        let first = body(&program, "first");
        let policy = policy("");
        let mut compiler = compiler();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let selected = selected(&mut compiler, direct, first, &policy);
        measured = Some((
            compiler
                .view(selected.semantic_id())
                .unwrap()
                .receipt()
                .logical_work,
            compiler.ledger().peak_retained_bytes(),
        ));
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    let (work, peak) = measured.unwrap();
    assert!(work > 1);
    // Measured replay is a test falsifier for the last admission boundary;
    // production never estimates this family with a source-size multiplier.
    for (optional, memory) in [(work - 1, 30_000_000), (30_000_000, peak - 1)] {
        checked(SOURCE, |program| {
            let first = body(&program, "first");
            let policy = policy("");
            let mut compiler = Compilation::new(
                BudgetLedger::new(
                    ResourceLimits::default(),
                    BudgetPlan {
                        baseline_work: 30_000_000,
                        optional_work: optional,
                        baseline_retained_bytes: 0,
                        retained_bytes: memory,
                    },
                )
                .unwrap(),
                CheckpointLimit { max_live: 16 },
            )
            .unwrap();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let before = compiler.ledger().retained_bytes();
            let result = compiler.scalar_function_javascript(
                direct,
                first,
                request(),
                &policy,
                WorkDomain::Optional,
            );
            assert!(
                result.is_err(),
                "optional={optional}, memory={memory}: {result:?}"
            );
            assert_eq!(compiler.ledger().retained_bytes(), before);
            assert_eq!(compiler.checkpoint_count(), 2);
            compiler.discard(source).unwrap();
            compiler.discard(direct.semantic_id()).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
