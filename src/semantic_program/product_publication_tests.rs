//! Public candidate ownership and checked edits exercise the same complete
//! product proof used by targets and search. No partial-family constructor.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
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
                baseline_work: 20_000_000,
                optional_work: 20_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 20_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 12 },
    )
    .unwrap()
}
fn policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
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
fn request() -> ProductRequest {
    ProductRequest {
        max_work: 200_000,
        scratch_bytes: 500_000,
        output_bytes: 500_000,
    }
}
fn selected(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    cell: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .scalar_product_javascript(base, cell, request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        ProductOutcome::Published(candidate) => candidate,
        other => panic!("{other:?}"),
    }
}
fn descriptor(compiler: &mut Compilation<'_>, candidate: CandidateId) -> Vec<u32> {
    compiler
        .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
        .unwrap()
}
const SOURCE: &str =
    "struct P{int x;}P state=P{1};P saved=state;P other=P{9};print(saved.x);print(other.x);";
#[test]
fn copied_roots_share_one_canonical_family_and_union_discard_orders_release_every_owner() {
    for reverse in [false, true] {
        checked(SOURCE, |program| {
            let [state, saved, other] = [
                cell(&program, "state"),
                cell(&program, "saved"),
                cell(&program, "other"),
            ];
            let policy = policy();
            let mut compiler = compiler();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Optional)
                .unwrap();
            let before = compiler.ledger().retained_bytes();
            let left = selected(&mut compiler, direct, state, &policy);
            let right = selected(&mut compiler, direct, saved, &policy);
            let identity = descriptor(&mut compiler, left);
            assert_eq!(identity, descriptor(&mut compiler, right));
            let union = compiler
                .combine_javascript(left, right, &policy, WorkDomain::Optional)
                .unwrap();
            assert_eq!(identity, descriptor(&mut compiler, union));
            compiler
                .with_implementations(union, |map| {
                    assert_eq!(map.products().len(), 1);
                    let family = map.products().next().unwrap();
                    assert_eq!(
                        family
                            .cells()
                            .iter()
                            .map(|entry| entry.cell)
                            .collect::<Vec<_>>(),
                        [state, saved]
                    );
                })
                .unwrap();
            // A second proof constructed for the same covered component is
            // consumed on failed insertion; the original owners remain live.
            let retained = compiler.ledger().retained_bytes();
            assert!(matches!(
                compiler.scalar_product_javascript(
                    left,
                    saved,
                    request(),
                    &policy,
                    WorkDomain::Optional
                ),
                Err(CandidateError::DuplicateRoot)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            // A genuinely separate complete component exercises union's full
            // covered-cell validation through the public producer path.
            let independent = selected(&mut compiler, direct, other, &policy);
            let disjoint = compiler
                .combine_javascript(union, independent, &policy, WorkDomain::Optional)
                .unwrap();
            compiler
                .with_implementations(disjoint, |map| assert_eq!(map.products().len(), 2))
                .unwrap();
            compiler.discard(independent.semantic_id()).unwrap();
            compiler.discard(disjoint.semantic_id()).unwrap();
            let order = if reverse {
                [union, right, left]
            } else {
                [left, right, union]
            };
            for (index, candidate) in order.iter().enumerate() {
                compiler.discard(candidate.semantic_id()).unwrap();
                for &remaining in &order[index + 1..] {
                    assert_eq!(descriptor(&mut compiler, remaining), identity);
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
fn checked_new_consumers_and_reference_exposure_invalidate_product_reuse_atomically() {
    for reference in [false, true] {
        let source = if reference {
            "struct P{int x;}void change(ref P value){value.x=8;}P state=P{1};P saved=state;P other=P{9};change(ref other);print(saved.x);"
        } else {
            "struct P{int x;}P state=P{1};P saved=state;P other=P{9};P copied=other;print(saved.x);print(copied.x);"
        };
        checked(source, |program| {
            let state = cell(&program, "state");
            let other = cell(&program, "other");
            let owner = program.cells()[state.index()].owner;
            let data = program.unit(owner).unwrap();
            let place = if reference {
                data.call_arguments
                    .iter()
                    .find_map(|argument| match argument {
                        CallArgument::Reference(place) => Some(*place),
                        _ => None,
                    })
                    .unwrap()
            } else {
                data.operations
                    .iter()
                    .find_map(|operation| match operation.kind {
                        OperationKind::Load(place)
                            if data.places[place.index()] == Place::Cell(other) =>
                        {
                            Some(place)
                        }
                        _ => None,
                    })
                    .unwrap()
            };
            let policy = policy();
            let mut compiler = compiler();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Optional)
                .unwrap();
            let selected = selected(&mut compiler, direct, state, &policy);
            let original = descriptor(&mut compiler, selected);
            let view = compiler.view(source).unwrap();
            let revision = view.unit_revision(owner).unwrap();
            let old_users = view.cell_users(state).unwrap().revision();
            assert!(!view.cell_users(state).unwrap().reference_exposed());
            let changed = compiler
                .edit_source(
                    source,
                    &[UnitPatch {
                        unit: owner,
                        expected_revision: revision,
                        operations: &[],
                        places: &[PlacePatch {
                            place,
                            replacement: &Place::Cell(state),
                        }],
                    }],
                    WorkDomain::Optional,
                )
                .unwrap();
            let view = compiler.view(changed).unwrap();
            assert_ne!(view.cell_users(state).unwrap().revision(), old_users);
            assert_eq!(
                view.cell_users(state).unwrap().reference_exposed(),
                reference
            );
            let retained = compiler.ledger().retained_bytes();
            assert!(matches!(
                compiler.rebase_javascript(selected, changed, &policy, WorkDomain::Optional),
                Err(CandidateError::StaleEvidence)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            assert_eq!(descriptor(&mut compiler, selected), original);
            let next = compiler
                .direct_javascript(changed, &policy, WorkDomain::Optional)
                .unwrap();
            let result = compiler
                .scalar_product_javascript(next, state, request(), &policy, WorkDomain::Optional)
                .unwrap();
            if reference {
                let ProductOutcome::Published(updated) = result.outcome else {
                    panic!(
                        "closed reference writer must be reproved: {:?}",
                        result.outcome
                    )
                };
                compiler
                    .with_implementations(updated, |map| {
                        let family = map.products().next().unwrap();
                        assert_eq!(
                            family.cells().len(),
                            2,
                            "reference formals borrow storage instead of joining owned value cells"
                        );
                        assert!(family.cells().iter().all(|row| row.cell != other));
                    })
                    .unwrap();
                // The old certificate stays stale even though fresh closure can
                // now prove this newly exposed root. Its snapshot is unchanged.
                assert_eq!(descriptor(&mut compiler, selected), original);
                compiler.discard(updated.semantic_id()).unwrap();
            } else {
                let ProductOutcome::Published(updated) = result.outcome else {
                    panic!("{:?}", result.outcome)
                };
                assert_ne!(
                    descriptor(&mut compiler, updated),
                    original,
                    "new whole-copy consumer changes complete component identity"
                );
                compiler
                    .with_implementations(updated, |map| {
                        assert_eq!(map.products().next().unwrap().cells().len(), 3)
                    })
                    .unwrap();
                compiler.discard(updated.semantic_id()).unwrap();
            }
            for candidate in [next, selected, direct] {
                compiler.discard(candidate.semantic_id()).unwrap();
            }
            compiler.discard(changed).unwrap();
            compiler.discard(source).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
