//! Physical location need belongs to the selected owning activation. Complete
//! source exposure remains independent and continues to qualify semantic facts.
use super::demand::{ContextId, DemandMode, DemandPlan, LocationDemand};
use super::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};
use super::helper_family::{self, FamilyOutcome, PreparationOutcome};
use super::implementations::ImplementationMap;
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::{JavaScriptCompilationContract, JavaScriptExecution};
use crate::compilation_policy::{
    AnalysisAttempt, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
    WorkKind,
};
use std::convert::Infallible;

const WORK: u64 = 100_000_000;
fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
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
            retained_bytes: 20_000_000,
        },
    )
    .unwrap()
}
fn contract() -> JavaScriptCompilationContract {
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
fn named(program: &Program<'_>, name: &str) -> CellId {
    let mut matches = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let cell = CellId::from_index(matches.next().unwrap().0).unwrap();
    assert!(matches.next().is_none(), "unique fixture binding {name}");
    cell
}
fn selected(
    program: &Program<'_>,
    uses: &UseIndex,
    names: &[&str],
    ledger: &mut BudgetLedger,
) -> ImplementationMap {
    let mut map = ImplementationMap::direct();
    for name in names {
        let prepared = helper_family::prepare(
            program,
            uses,
            named(program, name),
            helper_family::FamilyRequest {
                execution: JavaScriptExecution::Module,
                attempt: AnalysisAttempt {
                    plan: helper_family::HELPER_FAMILY_PLAN,
                    algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                    work_quota: 1_000_000,
                },
                scratch_bytes: 2_000_000,
                output_bytes: 2_000_000,
            },
            ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let PreparationOutcome::Ready(mut ready) = prepared.outcome else {
            panic!("{prepared:?}");
        };
        let mut cache = RetainedFactsCache::new(
            CacheLimits {
                entries: 2,
                bytes: 200_000,
                result_bytes: 100_000,
            },
            ledger,
            WorkDomain::Baseline,
        )
        .unwrap();
        {
            let mut session = cache.session(ledger, WorkDomain::Baseline, 1).unwrap();
            let facts = session
                .query(
                    program,
                    ready.root().body,
                    FactRequest {
                        attempt: AnalysisAttempt {
                            plan: LOCAL_FACTS_PLAN,
                            algorithm_version: LOCAL_FACTS_VERSION,
                            work_quota: 200_000,
                        },
                        result_bytes: 100_000,
                    },
                )
                .unwrap();
            ready
                .check_body(program, facts.facts, facts.receipt)
                .unwrap();
        }
        let FamilyOutcome::Complete(family) = ready.finish(ledger).unwrap() else {
            panic!("complete helper {name}");
        };
        cache.discard(ledger).unwrap();
        let next = map
            .with_inline_helper(family, ledger, WorkDomain::Optional)
            .unwrap();
        map.discard(ledger).unwrap();
        map = next;
    }
    map
}
fn location(
    demand: &DemandPlan<'_, '_>,
    context: ContextId,
    cell: CellId,
) -> Option<LocationDemand> {
    demand
        .cell_location_visited(context, cell, |_| Ok::<_, Infallible>(()))
        .unwrap()
}
fn contexts(program: &Program<'_>, demand: &DemandPlan<'_, '_>) -> Vec<ContextId> {
    let mut pending = demand.roots().to_vec();
    let mut found = Vec::new();
    while let Some(context) = pending.pop() {
        if found.contains(&context) {
            continue;
        }
        found.push(context);
        for index in 0..program
            .unit(demand.context(context).unit)
            .unwrap()
            .operations
            .len()
        {
            if let Some(child) = demand.child(context, OpId::from_index(index).unwrap()) {
                pending.push(child);
            }
        }
    }
    found
}
const TWO: &str = "int first(ref int firstSlot){firstSlot+=1;return firstSlot;}int second(ref int secondSlot){secondSlot+=2;return secondSlot;}int left=1;int right=4;print(first(ref left));print(second(ref right));";

#[test]
fn ordinary_shared_location_depends_on_selected_calls_while_source_exposure_stays_complete() {
    checked(TWO, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let left = named(program, "left");
        let right = named(program, "right");
        for names in [&[][..], &["first"][..], &["first", "second"][..]] {
            let map = selected(program, &uses, names, &mut ledger);
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
                for (cell, helper) in [(left, "first"), (right, "second")] {
                    assert!(
                        uses.cell(cell).unwrap().reference_exposed(),
                        "source exposure is not rewritten by recipe choice"
                    );
                    let expected = if names.contains(&helper) {
                        LocationDemand::Local
                    } else {
                        LocationDemand::Shared
                    };
                    assert_eq!(location(&demand, demand.root(), cell), Some(expected));
                    assert_eq!(
                        demand
                            .product_location_visited(demand.root(), cell, |_| Ok::<_, Infallible>(
                                ()
                            ))
                            .unwrap(),
                        None,
                        "ordinary cells have no product view"
                    );
                }
                assert!(
                    !demand
                        .has_shared_product_locations_visited(|_| Ok::<_, Infallible>(()))
                        .unwrap(),
                    "ordinary shared carriers do not require Mixed bank dispatch"
                );
                demand.discard(Some(&mut ledger)).unwrap();
                assert_eq!(ledger.retained_bytes(), before);
            }
            map.discard(&mut ledger).unwrap();
        }
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn captured_parameter_and_catch_locations_use_their_actual_owning_rows() {
    checked("int bump(ref int alias){alias+=1;return alias;}void clear(ref JsValue incoming){incoming=null;}int captured=3;int run(int owned){bump(ref owned);bump(ref captured);return owned;}print(run(4));try{throw 7;}catch(auto caught){clear(ref caught);print(caught);}",|program|{
        let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
        let demand=DemandPlan::build(program,Some(&uses),None,&contract(),DemandMode::Prune,Some((&mut ledger,WorkDomain::Optional))).unwrap();
        let captured=named(program,"captured");let owned=named(program,"owned");let caught=named(program,"caught");
        let all=contexts(program,&demand);
        let run=all.iter().copied().find(|context|demand.context(*context).unit==program.cells[owned.index()].owner).unwrap();
        assert_ne!(run,demand.root());
        assert_eq!(location(&demand,run,captured),Some(LocationDemand::Shared),"capture query resolves the ancestor module cell");
        assert_eq!(location(&demand,demand.root(),captured),Some(LocationDemand::Shared));
        assert_eq!(location(&demand,run,owned),Some(LocationDemand::Shared),"by-value parameter owns a new invocation cell");
        assert_eq!(location(&demand,demand.root(),caught),Some(LocationDemand::Shared),"baseline catch entry remains a real carrier origin");
        for name in ["alias","incoming"] {
            let cell=named(program,name);let owner=all.iter().copied().find(|context|demand.context(*context).unit==program.cells[cell.index()].owner).unwrap();
            assert_eq!(location(&demand,owner,cell),Some(LocationDemand::None),"incoming reference never requests another carrier");
        }
        demand.discard(Some(&mut ledger)).unwrap();uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
    });
}

#[test]
fn location_query_and_partial_fixed_point_refusal_do_not_publish_false_absence_or_leak() {
    checked(TWO, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let map = selected(program, &uses, &["first"], &mut ledger);
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
        let measured = demand.work().steps;
        let retained = ledger.retained_bytes();
        let mut visits = 0;
        let denied = demand.cell_location_visited(demand.root(), named(program, "left"), |_| {
            visits += 1;
            Err("denied")
        });
        assert_eq!(denied, Err("denied"));
        assert_eq!(visits, 1);
        assert_eq!(ledger.retained_bytes(), retained);
        assert_eq!(
            location(&demand, demand.root(), named(program, "left")),
            Some(LocationDemand::Local)
        );
        demand.discard(Some(&mut ledger)).unwrap();
        assert_eq!(ledger.retained_bytes(), before);
        let allowance = measured / 2;
        assert!(allowance > 0);
        ledger
            .charge(
                WorkDomain::Optional,
                WorkKind::Analysis,
                WORK - ledger.work_used(WorkDomain::Optional) - allowance,
            )
            .unwrap();
        let denied = DemandPlan::build(
            program,
            Some(&uses),
            Some(&map),
            &contract(),
            DemandMode::Prune,
            Some((&mut ledger, WorkDomain::Optional)),
        );
        assert!(matches!(
            denied,
            Err(super::demand::DemandError::Budget(
                crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Optional)
            ))
        ));
        assert_eq!(ledger.retained_bytes(), before);
        map.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
