//! Real common-demand proofs. No target quotient recipe is installed here.
use super::*;
use crate::compilation_contract::JavaScriptCompilationContract;
use crate::compilation_policy::{AnalysisAttempt, BudgetPlan, CompilationRequest, ResourceLimits};
use crate::semantic_program::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};
use crate::semantic_program::{from_checked_source, helper_family};
const WORK: u64 = 10_000_000;
fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(&from_checked_source(&syntax, &semantics).unwrap());
}
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}
fn contract(strip: bool) -> JavaScriptCompilationContract {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = strip;
    *config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .javascript_contract()
        .unwrap()
}
fn cell(program: &Program<'_>, name: &str) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == name)
            .unwrap(),
    )
    .unwrap()
}
fn facts_request() -> FactRequest {
    FactRequest {
        attempt: AnalysisAttempt {
            plan: LOCAL_FACTS_PLAN,
            algorithm_version: LOCAL_FACTS_VERSION,
            work_quota: 100_000,
        },
        result_bytes: 100_000,
    }
}
fn cache(ledger: &mut BudgetLedger) -> RetainedFactsCache {
    RetainedFactsCache::new(
        CacheLimits {
            entries: 8,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
        ledger,
        WorkDomain::Baseline,
    )
    .unwrap()
}
fn helper(
    program: &Program<'_>,
    uses: &UseIndex,
    root: CellId,
    cache: &mut RetainedFactsCache,
    ledger: &mut BudgetLedger,
) -> helper_family::HelperFamily {
    let prepared = helper_family::prepare(
        program,
        uses,
        root,
        helper_family::FamilyRequest {
            execution: crate::compilation_contract::JavaScriptExecution::Module,
            attempt: AnalysisAttempt {
                plan: helper_family::HELPER_FAMILY_PLAN,
                algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
        },
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let mut ready = match prepared.outcome {
        helper_family::PreparationOutcome::Ready(ready) => ready,
        other => panic!("helper preparation: {other:?}"),
    };
    {
        let mut session = cache.session(ledger, WorkDomain::Optional, 1).unwrap();
        let view = session
            .query(program, ready.root().body, facts_request())
            .unwrap();
        ready.check_body(program, view.facts, view.receipt).unwrap();
    }
    match ready.finish(ledger).unwrap() {
        helper_family::FamilyOutcome::Complete(family) => family,
        other => panic!("helper proof: {other:?}"),
    }
}

fn literal(plan: &DemandPlan<'_, '_>, text: &str) -> (ContextId, ValueId) {
    plan.contexts()
        .iter()
        .enumerate()
        .find_map(|(index, context)| {
            let data = plan.program.unit(context.unit).unwrap();
            data.operations.iter().find_map(|operation| {
                let OperationKind::Constant(Constant::String(id)) = operation.kind else {
                    return None;
                };
                (plan.program.strings[id.index()].as_unicode() == Some(text))
                    .then(|| (ContextId(index), operation.result.unwrap()))
            })
        })
        .expect("literal in entered context")
}
fn observed(plan: &DemandPlan<'_, '_>, text: &str) -> ObservationDemand {
    let (context, value) = literal(plan, text);
    let mut charged = 0;
    let result = plan
        .observation(context, value, |n| {
            charged += n;
            Ok::<_, ()>(())
        })
        .unwrap();
    assert_eq!(charged, 1);
    result
}
#[test]
fn incomparable_observations_join_without_a_truthiness_nullishness_order() {
    use ObservationDemand::*;
    assert_eq!(Truthy.join(Nullish), Exact);
    assert_eq!(Nullish.join(Truthy), Exact);
    for a in [Discarded, Truthy, Nullish, Exact] {
        assert_eq!(a.join(a), a);
        assert_eq!(a.join(Discarded), a);
        for b in [Discarded, Truthy, Nullish, Exact] {
            assert_eq!(a.join(b), b.join(a));
            for c in [Discarded, Truthy, Nullish, Exact] {
                assert_eq!(a.join(b).join(c), a.join(b.join(c)));
            }
        }
    }
}
#[test]
fn lazy_gates_join_through_cells_and_regions_while_effects_stay_required() {
    checked(
        r#"
        extern JsValue event(string label);
        extern bool gate();
        string truth="truth-only";
        string? nullable="nullish-only";
        string? mixed="mixed";
        string choice=if(gate()){"yes-literal"}else{"no-literal"};
        JS.and(truth,event("truth-effect"));
        nullable??event("nullish-effect");
        JS.and(mixed,event("mixed-effect"));
        mixed??event("mixed-nullish");
        JS.and(choice,event("branch-effect"));
    "#,
        |program| {
            let plan = DemandPlan::build(
                program,
                None,
                None,
                &contract(false),
                DemandMode::Prune,
                None,
            )
            .unwrap();
            assert_eq!(observed(&plan, "truth-only"), ObservationDemand::Truthy);
            assert_eq!(observed(&plan, "nullish-only"), ObservationDemand::Nullish);
            assert_eq!(observed(&plan, "mixed"), ObservationDemand::Exact);
            assert_eq!(observed(&plan, "yes-literal"), ObservationDemand::Truthy);
            assert_eq!(observed(&plan, "no-literal"), ObservationDemand::Truthy);
            for text in [
                "truth-effect",
                "nullish-effect",
                "mixed-effect",
                "mixed-nullish",
                "branch-effect",
            ] {
                assert_eq!(
                    observed(&plan, text),
                    ObservationDemand::Exact,
                    "shared call actual {text}"
                );
            }
            for (index, context) in plan.contexts().iter().enumerate() {
                for (operation, op) in program
                    .unit(context.unit)
                    .unwrap()
                    .operations
                    .iter()
                    .enumerate()
                {
                    if matches!(op.kind, OperationKind::Call(_)) {
                        assert!(plan.needs_execution(
                            ContextId(index),
                            OpId::from_index(operation).unwrap()
                        ));
                    }
                }
            }
            plan.discard(None).unwrap();
        },
    );
}
#[test]
fn processed_weak_writers_and_copy_producers_upgrade_to_exact() {
    checked(
        r#"
        extern JsValue event();
        string state="first";
        state="second";
        string copied=if(true){state}else{"branch-fallback"};
        JS.and(copied,event());
    "#,
        |program| {
            let mut plan = DemandPlan::build(
                program,
                None,
                None,
                &contract(false),
                DemandMode::Prune,
                None,
            )
            .unwrap();
            assert_eq!(observed(&plan, "first"), ObservationDemand::Truthy);
            assert_eq!(observed(&plan, "second"), ObservationDemand::Truthy);
            assert_eq!(
                observed(&plan, "branch-fallback"),
                ObservationDemand::Truthy
            );
            let root = plan.root();
            let cell = cell(program, "copied");
            let mut cursor = plan.pending.len();
            let before = cursor;
            // Resume the already-completed unaccounted inspection owner, using the
            // same queue and writer traversal as production. No ledger is reset.
            let mut budget = Budget::new(None);
            budget.retained = plan.work.retained_bytes;
            plan.observe_cell(root, cell, ObservationDemand::Exact, &mut budget)
                .unwrap();
            plan.drain_pending(&mut cursor, &mut budget).unwrap();
            assert!(cursor > before, "upgrade must process new work");
            assert_eq!(observed(&plan, "first"), ObservationDemand::Exact);
            assert_eq!(observed(&plan, "second"), ObservationDemand::Exact);
            assert_eq!(observed(&plan, "branch-fallback"), ObservationDemand::Exact);
            plan.discard(None).unwrap();
        },
    );
}
#[test]
fn inline_parameter_and_return_transport_weakness_but_shared_abi_is_exact() {
    checked(
        r#"
        extern JsValue event();
        string forward(string input){string saved=input;return saved;}
        JS.and(forward("inline-input"),event());
    "#,
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let family = helper(
                program,
                &uses,
                cell(program, "forward"),
                &mut cache,
                &mut ledger,
            );
            let body = family.root().body;
            let map = ImplementationMap::direct()
                .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
                .unwrap();
            for inline in [false, true] {
                let plan = DemandPlan::build(
                    program,
                    Some(&uses),
                    inline.then_some(&map),
                    &contract(false),
                    DemandMode::Prune,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                assert_eq!(
                    observed(&plan, "inline-input"),
                    if inline {
                        ObservationDemand::Truthy
                    } else {
                        ObservationDemand::Exact
                    }
                );
                if inline {
                    let child = plan
                        .contexts()
                        .iter()
                        .find(|context| context.unit == body)
                        .unwrap();
                    assert_eq!(child.return_observation, ObservationDemand::Truthy);
                }
                plan.discard(Some(&mut ledger)).unwrap();
            }
            map.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}
#[test]
fn reference_exposure_and_preserve_keep_storage_exact() {
    checked(
        r#"
        extern JsValue event();
        void touch(ref string input){input="changed";}
        string state="reference-initial";
        touch(ref state);
        JS.and(state,event());
        string weak="preserved";
        JS.and(weak,event());
    "#,
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            for mode in [DemandMode::Prune, DemandMode::Preserve] {
                let plan = DemandPlan::build(
                    program,
                    Some(&uses),
                    None,
                    &contract(false),
                    mode,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                assert_eq!(
                    observed(&plan, "reference-initial"),
                    ObservationDemand::Exact
                );
                assert_eq!(
                    observed(&plan, "preserved"),
                    if mode == DemandMode::Prune {
                        ObservationDemand::Truthy
                    } else {
                        ObservationDemand::Exact
                    }
                );
                plan.discard(Some(&mut ledger)).unwrap();
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}
#[test]
fn observation_build_refusal_releases_partial_queue_and_writer_storage() {
    checked(
        r#"extern JsValue event();string state="first";state="second";JS.and(state,event());"#,
        |program| {
            let mut successful = ledger();
            let plan = DemandPlan::build(
                program,
                None,
                None,
                &contract(false),
                DemandMode::Prune,
                Some((&mut successful, WorkDomain::Optional)),
            )
            .unwrap();
            let work = successful.work_used(WorkDomain::Optional);
            let bytes = plan.work.peak_bytes;
            assert!(work > 4 && bytes > 4);
            plan.discard(Some(&mut successful)).unwrap();
            assert_eq!(successful.retained_bytes(), 0);
            for (work, memory) in [(work / 2, 10_000_000), (WORK, bytes / 2)] {
                let mut denied = BudgetLedger::new(
                    ResourceLimits::default(),
                    BudgetPlan {
                        baseline_work: WORK,
                        optional_work: work,
                        baseline_retained_bytes: 0,
                        retained_bytes: memory,
                    },
                )
                .unwrap();
                let result = DemandPlan::build(
                    program,
                    None,
                    None,
                    &contract(false),
                    DemandMode::Prune,
                    Some((&mut denied, WorkDomain::Optional)),
                );
                assert!(
                    matches!(result, Err(DemandError::Budget(_))),
                    "must refuse incomplete demand"
                );
                assert_eq!(denied.retained_bytes(), 0);
            }
        },
    );
}
#[test]
fn paid_observation_query_honors_refusal_without_exposing_an_answer() {
    checked(r#"print("exact");"#, |program| {
        let plan = DemandPlan::build(
            program,
            None,
            None,
            &contract(false),
            DemandMode::Prune,
            None,
        )
        .unwrap();
        let (context, value) = literal(&plan, "exact");
        assert_eq!(
            plan.observation(context, value, |_| Err("denied")),
            Err("denied")
        );
        assert_eq!(observed(&plan, "exact"), ObservationDemand::Exact);
        plan.discard(None).unwrap();
    });
}

#[test]
fn truthiness_demand_does_not_weaken_numeric_conversion_inputs() {
    checked(
        "extern int raw();extern JsValue event();int value=raw()+1;JS.and(value,event());",
        |program| {
            let plan = DemandPlan::build(
                program,
                None,
                None,
                &contract(false),
                DemandMode::Prune,
                None,
            )
            .unwrap();
            let root = plan.root();
            let data = program.unit(plan.context(root).unit).unwrap();
            let (index, arithmetic) = data
                .operations
                .iter()
                .enumerate()
                .find(|(_, op)| matches!(op.kind, OperationKind::IntBinary(_)))
                .unwrap();
            let id = OpId::from_index(index).unwrap();
            assert!(plan.needs_operation(root, id));
            assert_eq!(
                plan.observation(root, arithmetic.result.unwrap(), |_| Ok::<_, ()>(()))
                    .unwrap(),
                ObservationDemand::Truthy
            );
            let mut inputs = 0;
            plan.visit_effective_site_uses(
                root,
                EffectiveUseSite::Operation(id),
                |_| Ok::<_, ()>(()),
                |input| {
                    inputs += 1;
                    assert_eq!(input.observation, ObservationDemand::Exact);
                    assert_eq!(
                        plan.observation(root, input.value, |_| Ok::<_, ()>(()))
                            .unwrap(),
                        ObservationDemand::Exact
                    );
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(inputs, 2);
            let raw = data.operands(arithmetic.operands).unwrap()[0];
            let call = data.values[raw.index()].definition;
            assert!(matches!(
                data.operations[call.index()].kind,
                OperationKind::Call(_)
            ));
            assert!(
                plan.needs_execution(root, call),
                "normalization/host call remains an execution obligation"
            );
            plan.discard(None).unwrap();
        },
    );
}
