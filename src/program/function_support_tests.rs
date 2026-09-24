//! Physical support is qualified by complete producer witnesses, not body name
//! or current reachability. These tests publish real maps through Compilation.
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits};
use crate::js::selection::{Plan, Style};
use crate::program::demand::{
    shared_transport_support, DemandError, DemandMode, DemandPlan, SharedTransportSupport,
};
use crate::program::facts::CacheLimits;
use crate::program::publication::*;

struct Fixture {
    source: SemanticId,
    direct: CandidateId,
    layout: CandidateId,
    root: UnitId,
    body: UnitId,
    old_second_body: UnitId,
    second_creation: OpId,
    cells: [CellId; 2],
    parameter: CellId,
    policy: ResolvedPolicy,
}

fn request() -> ScalarRequest {
    ScalarRequest {
        max_work: 2_000_000,
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
    }
}

fn with_fixture(inspect: impl FnOnce(&mut Compilation<'_>, &Fixture)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "struct P{int x;}auto first=(P value)=>value.x;auto second=(P value)=>value.x;P source=P{7};print(first(source));print(second(source));").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let root = program.initialization[0];
    let closures: Vec<_> = program
        .unit(root)
        .unwrap()
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, op)| match op.kind {
            OperationKind::Closure(body) => Some((OpId::from_index(index).unwrap(), body)),
            _ => None,
        })
        .collect();
    assert_eq!(closures.len(), 2);
    let cells = ["first", "second"].map(|name| {
        CellId::from_index(
            program
                .cells
                .iter()
                .position(|cell| cell.name == name)
                .unwrap(),
        )
        .unwrap()
    });
    let parameter = program.unit(closures[0].1).unwrap().parameters[0];
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let mut compiler = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 40_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 32 },
    )
    .unwrap();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 16,
                bytes: 2_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    let original = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let revision = compiler
        .view(original)
        .unwrap()
        .unit_revision(root)
        .unwrap();
    let shared = OperationKind::Closure(closures[0].1);
    let source = compiler
        .edit_source(
            original,
            &[UnitPatch {
                unit: root,
                expected_revision: revision,
                operations: &[OperationPatch {
                    operation: closures[1].0,
                    kind: &shared,
                    operands: &[],
                }],
                places: &[],
            }],
            WorkDomain::Optional,
        )
        .unwrap();
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Optional)
        .unwrap();
    let FunctionOutcome::Published(layout) = compiler
        .scalar_function_javascript(
            direct,
            closures[0].1,
            request(),
            &policy,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome
    else {
        panic!("complete two-producer transport")
    };
    inspect(
        &mut compiler,
        &Fixture {
            source,
            direct,
            layout,
            root,
            body: closures[0].1,
            old_second_body: closures[1].1,
            second_creation: closures[1].0,
            cells,
            parameter,
            policy,
        },
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

fn inline(
    compiler: &mut Compilation<'_>,
    fixture: &Fixture,
    base: CandidateId,
    cell: CellId,
) -> CandidateId {
    let HelperOutcome::Published(candidate) = compiler
        .inline_helper_javascript(
            base,
            cell,
            HelperRequest {
                max_work: 2_000_000,
                scratch_bytes: 2_000_000,
                output_bytes: 2_000_000,
                local_facts: LocalFactsRequest {
                    work_quota: 100_000,
                    result_bytes: 100_000,
                },
            },
            &fixture.policy,
            WorkDomain::Optional,
        )
        .unwrap()
        .outcome
    else {
        panic!("complete exact producer helper")
    };
    candidate
}

fn support(
    compiler: &Compilation<'_>,
    fixture: &Fixture,
    candidate: CandidateId,
) -> SharedTransportSupport {
    let mut work = 0;
    let result = compiler
        .with_implementations(fixture.layout, |layout| {
            let layout = layout.functions().next().unwrap();
            assert_eq!(layout.inputs().producers().len(), 2);
            compiler
                .with_implementations(candidate, |map| {
                    shared_transport_support(map, layout, JavaScriptExecution::Module, |n| {
                        work += n;
                        Ok::<_, ()>(())
                    })
                    .unwrap()
                })
                .unwrap()
        })
        .unwrap();
    assert!(work > 0);
    result
}

fn descriptor(compiler: &mut Compilation<'_>, candidate: CandidateId) -> Vec<u32> {
    compiler
        .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| words.to_vec())
        .unwrap()
}

fn emit(
    compiler: &mut Compilation<'_>,
    fixture: &Fixture,
    candidate: CandidateId,
    style: Style,
) -> String {
    compiler
        .with_javascript_output_in(candidate, &fixture.policy, WorkDomain::Optional, |output| {
            let id = output.render(&Plan::new(style)).unwrap();
            output.take_artifact(id).unwrap()
        })
        .unwrap()
}

#[test]
fn shared_transport_requires_every_exact_creator_and_survives_compatible_children() {
    with_fixture(|compiler, fixture| {
        let first = inline(compiler, fixture, fixture.direct, fixture.cells[0]);
        assert_eq!(
            support(compiler, fixture, first),
            SharedTransportSupport::MayApply
        );
        let both = inline(compiler, fixture, first, fixture.cells[1]);
        assert_eq!(
            support(compiler, fixture, both),
            SharedTransportSupport::AllCreatorsInline
        );
        let selected = compiler
            .combine_javascript(both, fixture.layout, &fixture.policy, WorkDomain::Optional)
            .unwrap();
        assert_ne!(descriptor(compiler, both), descriptor(compiler, selected));
        compiler
            .with_implementations(selected, |map| assert_eq!(map.functions().len(), 1))
            .unwrap();
        for candidate in [first, both] {
            let with_layout = compiler
                .combine_javascript(
                    candidate,
                    fixture.layout,
                    &fixture.policy,
                    WorkDomain::Optional,
                )
                .unwrap();
            let slot = compiler.candidate_slot(with_layout).unwrap();
            let checkpoint = compiler.slots[slot].checkpoint.as_ref().unwrap();
            let demand = DemandPlan::build(
                &checkpoint.semantic.program,
                Some(&checkpoint.semantic.uses),
                checkpoint.implementations.as_ref(),
                fixture.policy.javascript_contract().unwrap(),
                DemandMode::Prune,
                Some((&mut compiler.ledger, WorkDomain::Optional)),
            )
            .unwrap();
            assert_eq!(
                demand.function_layout(fixture.body).is_some(),
                candidate == first
            );
            demand.discard(Some(&mut compiler.ledger)).unwrap();
        }
        let ProductOutcome::Published(storage) = compiler
            .scalar_product_javascript(
                fixture.direct,
                fixture.parameter,
                request(),
                &fixture.policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        else {
            panic!("independent parameter storage")
        };
        let later = compiler
            .combine_javascript(both, storage, &fixture.policy, WorkDomain::Optional)
            .unwrap();
        let later_with_layout = compiler
            .combine_javascript(selected, storage, &fixture.policy, WorkDomain::Optional)
            .unwrap();
        assert_eq!(
            support(compiler, fixture, later_with_layout),
            SharedTransportSupport::AllCreatorsInline
        );
        for style in [Style::Source, Style::Global, Style::Scoped] {
            assert_eq!(
                emit(compiler, fixture, both, style),
                emit(compiler, fixture, selected, style)
            );
            assert_eq!(
                emit(compiler, fixture, later, style),
                emit(compiler, fixture, later_with_layout, style)
            );
        }
    });
}

#[test]
fn inactive_transport_keeps_stale_and_script_guards_at_their_owners() {
    with_fixture(|compiler, fixture| {
        let first = inline(compiler, fixture, fixture.layout, fixture.cells[0]);
        let both = inline(compiler, fixture, first, fixture.cells[1]);
        let original = descriptor(compiler, both);
        let slot = compiler.candidate_slot(both).unwrap();
        let checkpoint = compiler.slots[slot].checkpoint.as_ref().unwrap();
        let map = checkpoint.implementations.as_ref().unwrap();
        let layout = map.functions().next().unwrap();
        assert_eq!(
            shared_transport_support(
                map,
                layout,
                JavaScriptExecution::Script,
                |_| Ok::<_, ()>(())
            )
            .unwrap(),
            SharedTransportSupport::MayApply
        );
        let mut script = *fixture.policy.javascript_contract().unwrap();
        script.execution = JavaScriptExecution::Script;
        assert!(matches!(
            DemandPlan::build(
                &checkpoint.semantic.program,
                Some(&checkpoint.semantic.uses),
                Some(map),
                &script,
                DemandMode::Prune,
                Some((&mut compiler.ledger, WorkDomain::Optional))
            ),
            Err(DemandError::Unsupported(_))
        ));
        let revision = compiler
            .view(fixture.source)
            .unwrap()
            .unit_revision(fixture.root)
            .unwrap();
        let separate = OperationKind::Closure(fixture.old_second_body);
        let changed = compiler
            .edit_source(
                fixture.source,
                &[UnitPatch {
                    unit: fixture.root,
                    expected_revision: revision,
                    operations: &[OperationPatch {
                        operation: fixture.second_creation,
                        kind: &separate,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Optional,
            )
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.rebase_javascript(both, changed, &fixture.policy, WorkDomain::Optional),
            Err(CandidateError::StaleEvidence)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        assert_eq!(descriptor(compiler, both), original);
        assert_eq!(
            support(compiler, fixture, both),
            SharedTransportSupport::AllCreatorsInline
        );
    });
}

#[test]
fn inactive_transport_query_refusal_is_not_a_negative_proof_or_allocation() {
    with_fixture(|compiler, fixture| {
        let first = inline(compiler, fixture, fixture.layout, fixture.cells[0]);
        let both = inline(compiler, fixture, first, fixture.cells[1]);
        let identity = descriptor(compiler, both);
        let retained = compiler.ledger().retained_bytes();
        for allowance in [0usize, 1] {
            let mut used = 0;
            let result = compiler
                .with_implementations(both, |map| {
                    shared_transport_support(
                        map,
                        map.functions().next().unwrap(),
                        JavaScriptExecution::Module,
                        |n| {
                            if n > allowance - used {
                                return Err(BudgetError::WorkExhausted(WorkDomain::Optional));
                            }
                            used += n;
                            Ok(())
                        },
                    )
                })
                .unwrap();
            assert!(matches!(
                result,
                Err(BudgetError::WorkExhausted(WorkDomain::Optional))
            ));
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            assert_eq!(descriptor(compiler, both), identity);
        }
        assert_eq!(
            support(compiler, fixture, both),
            SharedTransportSupport::AllCreatorsInline
        );
    });
}
