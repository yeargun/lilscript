//! Slot demand follows the complete family while raw execution remains an
//! independent root. Assertions use semantic identities, not target spelling.
use super::demand::{DemandError, DemandMode, DemandPlan};
use super::implementations::ImplementationMap;
use super::product_family::{self, FamilyOutcome, ProductOperationKind};
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits,
    WorkDomain, WorkKind,
};
const WORK: u64 = 10_000_000;
fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let ast = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&ast).unwrap();
    let program = from_checked_source(&ast, &checked).unwrap();
    program.verify().unwrap();
    inspect(&program);
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
fn map(program: &Program<'_>, uses: &UseIndex, ledger: &mut BudgetLedger) -> ImplementationMap {
    let cell = CellId::from_index(
        program
            .cells()
            .iter()
            .position(|cell| cell.name == "p")
            .unwrap(),
    )
    .unwrap();
    let analysis = product_family::analyze_published(
        program,
        uses,
        cell,
        product_family::FamilyRequest {
            execution: crate::compilation_contract::JavaScriptExecution::Module,
            attempt: AnalysisAttempt {
                plan: product_family::PRODUCT_FAMILY_PLAN,
                algorithm_version: product_family::PRODUCT_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
        },
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let FamilyOutcome::Complete(family) = analysis.outcome else {
        panic!("{analysis:?}");
    };
    ImplementationMap::direct()
        .with_product(family, ledger, WorkDomain::Optional)
        .unwrap()
}
fn contract() -> crate::compilation_contract::JavaScriptCompilationContract {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    *config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .javascript_contract()
        .unwrap()
}
fn contexts_for_unit(
    program: &Program<'_>,
    demand: &DemandPlan<'_, '_>,
    unit: UnitId,
) -> Vec<super::demand::ContextId> {
    let mut pending = demand.roots().to_vec();
    let mut visited = Vec::new();
    let mut matching = Vec::new();
    while let Some(context) = pending.pop() {
        if visited.contains(&context) {
            continue;
        }
        visited.push(context);
        let data = program.unit(demand.context(context).unit).unwrap();
        if demand.context(context).unit == unit {
            matching.push(context);
        }
        for index in 0..data.operations.len() {
            if let Some(child) = demand.child(context, OpId::from_index(index).unwrap()) {
                pending.push(child);
            }
        }
    }
    matching
}
const SOURCE: &str = r#"extern string tick(); struct P { string a; string b; } P p=P{tick(),"kept"}; P q=p; print(q.b);"#;

#[test]
fn component_demand_keeps_raw_copy_snapshots_and_independent_unused_initializer_calls() {
    checked(SOURCE, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let map = map(program, &uses, &mut ledger);
        let before = ledger.retained_bytes();
        for mode in [DemandMode::Prune, DemandMode::Preserve] {
            let demand = DemandPlan::build(
                program,
                Some(&uses),
                Some(&map),
                &contract(),
                mode,
                Some((&mut ledger, WorkDomain::Optional)),
            )
            .unwrap();
            let context = demand.root();
            let wanted_first = mode == DemandMode::Preserve;
            let cells: Vec<_> = demand.product_cells(context).collect();
            assert_eq!(
                cells.len(),
                2,
                "the whole copy component owns separate cell banks"
            );
            for (cell, _) in cells {
                assert_eq!(demand.needs_product_slot(context, cell, 0), wanted_first);
                assert!(demand.needs_product_slot(context, cell, 1));
            }
            let snapshots: Vec<_> = demand.product_snapshots(context).collect();
            assert!(
                snapshots.len() >= 3,
                "construction, lexical snapshot and logical copy remain distinct"
            );
            for (value, _) in snapshots {
                assert_eq!(demand.needs_snapshot_slot(context, value, 0), wanted_first);
                assert!(demand.needs_snapshot_slot(context, value, 1));
            }
            let data = program.unit(demand.context(context).unit).unwrap();
            let construct = map
                .products()
                .next()
                .unwrap()
                .operations()
                .iter()
                .find(|entry| matches!(entry.kind, ProductOperationKind::Construct { .. }))
                .unwrap();
            let input = data
                .operands(data.operations[construct.operation.operation.index()].operands)
                .unwrap()[0];
            let call = data.values[input.index()].definition;
            assert!(matches!(
                data.operations[call.index()].kind,
                OperationKind::Call(_)
            ));
            assert!(
                demand.needs_execution(context, call),
                "discarded field cannot discard host invocation"
            );
            assert_eq!(demand.needs_value(context, input), wanted_first);
            demand.discard(Some(&mut ledger)).unwrap();
            assert_eq!(ledger.retained_bytes(), before);
        }
        map.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn product_demand_refusal_releases_partial_contexts_without_releasing_selected_family() {
    checked(SOURCE, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let map = map(program, &uses, &mut ledger);
        let before = ledger.retained_bytes();
        let complete = DemandPlan::build(
            program,
            Some(&uses),
            Some(&map),
            &contract(),
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Optional)),
        )
        .unwrap();
        let measured = complete.work().steps;
        complete.discard(Some(&mut ledger)).unwrap();
        // Refuse after real allocations, using a measured complete cost only
        // in this test. Production admission has no guessed source multiplier.
        let allowance = measured / 2;
        assert!(allowance > 0);
        ledger
            .charge(
                WorkDomain::Optional,
                WorkKind::Analysis,
                WORK - ledger.work_used(WorkDomain::Optional) - allowance,
            )
            .unwrap();
        let error = match DemandPlan::build(
            program,
            Some(&uses),
            Some(&map),
            &contract(),
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Optional)),
        ) {
            Ok(plan) => {
                plan.discard(Some(&mut ledger)).unwrap();
                panic!("partial quota completed");
            }
            Err(error) => error,
        };
        assert!(matches!(
            error,
            DemandError::Budget(BudgetError::WorkExhausted(WorkDomain::Optional))
        ));
        assert_eq!(ledger.retained_bytes(), before);
        assert_eq!(
            map.products().count(),
            1,
            "a failed consumer cannot discard its proof owner"
        );
        map.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn captured_writers_revoke_snapshot_reuse_per_top_slot_without_aliasing_value_copies() {
    for whole in [false, true] {
        let source = if whole {
            "struct Inner{int x;int y;}struct P{Inner left;int stable;}extern void keep(func()->void update);P p=P{Inner{1,2},7};P copied=p;keep(()=>{p=P{Inner{9,10},11};});print(copied.left.x);print(copied.stable);print(p.left.x);print(p.stable);"
        } else {
            "struct Inner{int x;int y;}struct P{Inner left;int stable;}extern void keep(func()->void update);P p=P{Inner{1,2},7};P copied=p;keep(()=>{p.left.x=9;});print(copied.left.x);print(copied.stable);print(p.left.x);print(p.stable);"
        };
        checked(source, |program| {
            let cell = |name: &str| {
                CellId::from_index(
                    program
                        .cells()
                        .iter()
                        .position(|cell| cell.name == name)
                        .unwrap(),
                )
                .unwrap()
            };
            let p = cell("p");
            let copied = cell("copied");
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let map = map(program, &uses, &mut ledger);
            let before = ledger.retained_bytes();
            let writer = map
                .products()
                .next()
                .unwrap()
                .operations()
                .iter()
                .find(|entry| {
                    entry.operation.unit != program.cells()[p.index()].owner
                        && matches!(
                            program.unit(entry.operation.unit).unwrap().operations
                                [entry.operation.operation.index()]
                            .kind,
                            OperationKind::Store(_)
                        )
                })
                .unwrap()
                .operation
                .unit;
            for mode in [DemandMode::Prune, DemandMode::Preserve] {
                let demand = DemandPlan::build(
                    program,
                    Some(&uses),
                    Some(&map),
                    &contract(),
                    mode,
                    Some((&mut ledger, WorkDomain::Optional)),
                )
                .unwrap();
                let children = contexts_for_unit(program, &demand, writer);
                assert_eq!(children.len(), 1);
                let child = children[0];
                for context in [demand.root(), child] {
                    let mut charged = 0;
                    assert!(!demand
                        .stable_product_slot_visited(context, p, 0, |n| {
                            charged += n;
                            Ok::<_, ()>(())
                        })
                        .unwrap());
                    assert_eq!(
                        demand
                            .stable_product_slot_visited(context, p, 1, |n| {
                                charged += n;
                                Ok::<_, ()>(())
                            })
                            .unwrap(),
                        !whole
                    );
                    assert!(charged > 0, "owner lookup and bank inspection are visible");
                }
                for slot in 0..2 {
                    assert!(
                        demand
                            .stable_product_slot_visited(
                                demand.root(),
                                copied,
                                slot,
                                |_| Ok::<_, ()>(())
                            )
                            .unwrap(),
                        "logical copy owns a separate stable bank"
                    );
                }
                demand.discard(Some(&mut ledger)).unwrap();
                assert_eq!(ledger.retained_bytes(), before);
            }
            map.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn unmodified_parameter_slots_are_stable_per_real_call_but_not_reused_inline_entries() {
    checked("struct P{int x;int y;}int step(P p){return p.x+p.y;}for(int i=0;i<2;i+=1){print(step(P{i,7}));}",|program|{
        let cell=CellId::from_index(program.cells().iter().position(|cell|cell.name=="p").unwrap()).unwrap();
        let body=program.cells()[cell.index()].owner;
        let helper=super::helper_family_tests::root(program,"step");
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let map=map(program,&uses,&mut ledger);
        let mut cache=super::helper_family_tests::cache(&mut ledger);
        let (super::helper_family::FamilyOutcome::Complete(family),_)=super::helper_family_tests::analyze(program,&uses,helper,&mut ledger,&mut cache) else {panic!("complete inline helper")};
        let inline=map.with_inline_helper(family,&mut ledger,WorkDomain::Optional).unwrap();
        for (selected,expected) in [(&map,true),(&inline,false)] {
            let before=ledger.retained_bytes();
            let demand=DemandPlan::build(program,Some(&uses),Some(selected),&contract(),DemandMode::Prune,Some((&mut ledger,WorkDomain::Optional))).unwrap();
            let contexts=contexts_for_unit(program,&demand,body);
            assert!(!contexts.is_empty());
            for context in contexts {
                assert_eq!(demand.context(context).kind.is_inline(),!expected);
                for slot in 0..2 {
                    assert!(demand.needs_product_slot(context,cell,slot));
                    assert_eq!(demand.stable_product_slot_visited(context,cell,slot,|_|Ok::<_,()>(())).unwrap(),expected);
                }
            }
            demand.discard(Some(&mut ledger)).unwrap();assert_eq!(ledger.retained_bytes(),before);
        }
        inline.discard(&mut ledger).unwrap();map.discard(&mut ledger).unwrap();cache.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn unused_parameter_family_still_admits_its_cell_lookup_without_operation_overlays() {
    checked("struct P{int x;}void step(P p){}step(P{1});", |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let map = map(program, &uses, &mut ledger);
        let family = map.products().next().unwrap();
        assert_eq!(family.cells().len(), 1);
        assert!(family.operations().is_empty());
        let p = family.cells()[0].cell;
        assert_eq!(
            family.cells()[0].origin,
            product_family::ProductOrigin::Parameter
        );
        let before = ledger.retained_bytes();
        let demand = DemandPlan::build(
            program,
            Some(&uses),
            Some(&map),
            &contract(),
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Optional)),
        )
        .unwrap();
        assert_eq!(demand.product_for_cell(p), Some(0));
        assert!(
            demand.product_lookup_work() > 0,
            "cell lookup is real work even when no operation recipe exists"
        );
        // The same common estimate must deny before the target's lookup when
        // that last analysis allowance has been spent. No operation count can
        // silently turn this into an unaccounted zero-work inspection.
        let allowance = demand.product_lookup_work() as u64;
        ledger
            .charge(
                WorkDomain::Optional,
                WorkKind::Analysis,
                WORK - ledger.work_used(WorkDomain::Optional),
            )
            .unwrap();
        assert!(matches!(
            ledger.charge(WorkDomain::Optional, WorkKind::Render, allowance),
            Err(BudgetError::WorkExhausted(WorkDomain::Optional))
        ));
        demand.discard(Some(&mut ledger)).unwrap();
        assert_eq!(ledger.retained_bytes(), before);
        map.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
