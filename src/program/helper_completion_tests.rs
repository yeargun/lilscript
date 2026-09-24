//! Normal void fallthrough is proved without inserting semantic operations.
use super::*;
use crate::compilation_policy::{BudgetPlan, ResourceLimits};
use crate::program::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};

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
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}
fn request() -> FamilyRequest {
    FamilyRequest {
        execution: JavaScriptExecution::Module,
        attempt: AnalysisAttempt {
            plan: HELPER_FAMILY_PLAN,
            algorithm_version: HELPER_FAMILY_VERSION,
            work_quota: 1_000_000,
        },
        scratch_bytes: 2_000_000,
        output_bytes: 2_000_000,
    }
}
fn root(program: &Program<'_>) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "helper")
            .unwrap(),
    )
    .unwrap()
}
fn complete(program: &Program<'_>, uses: &UseIndex, ledger: &mut BudgetLedger) -> HelperFamily {
    let result = prepare(
        program,
        uses,
        root(program),
        request(),
        ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let PreparationOutcome::Ready(mut ready) = result.outcome else {
        panic!("expected prepared normal completion: {:?}", result.outcome)
    };
    let mut cache = RetainedFactsCache::new(
        CacheLimits {
            entries: 1,
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
    let result = ready.finish(ledger).unwrap();
    cache.discard(ledger).unwrap();
    match result {
        FamilyOutcome::Complete(family) => family,
        other => panic!("expected completed helper: {other:?}"),
    }
}

#[test]
fn empty_and_mutating_void_fallthrough_have_no_fabricated_return() {
    for source in [
        "void helper(){}helper();",
        "struct Empty{}void helper(ref Empty value,int next){}Empty state=Empty{};helper(ref state,5);",
        "struct P{int value;}void helper(ref P whole,ref int leaf){whole=P{7};leaf+=2;}P state=P{1};helper(ref state,ref state.value);print(state.value);",
    ] {
        checked(source, |program| {
            let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
            let retained=ledger.retained_bytes();let family=complete(program,&uses,&mut ledger);
            assert_eq!(family.tail_return(),None);assert!(family.return_primitive());
            let body=program.unit(family.root().body).unwrap();
            assert!(!body.operations.iter().any(|op|matches!(op.kind,OperationKind::Return)));
            assert!(!family.body_effects().transfers_control);
            assert_eq!(family.body_facts.len(),body.operations.len());
            family.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),retained);
            uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
        });
    }
}
#[test]
fn explicit_void_return_remains_the_only_consumed_completion() {
    for source in [
        "void helper(){return;}helper();",
        "void helper(ref int value){value=7;return;}int state=1;helper(ref state);print(state);",
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let family = complete(program, &uses, &mut ledger);
            let tail = family.tail_return().expect("explicit return");
            let body = program.unit(family.root().body).unwrap();
            assert_eq!(
                body.regions[body.entry.index()].operations.last(),
                Some(&tail)
            );
            assert!(matches!(
                body.operations[tail.index()].kind,
                OperationKind::Return
            ));
            assert!(family.return_primitive());
            assert!(!family.body_effects().transfers_control);
            family.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}
#[test]
fn a_checked_nonvoid_body_without_its_return_is_not_void_fallthrough() {
    checked(
        "int helper(int value){return value+1;}print(helper(3));",
        |program| {
            let mut changed = program.clone();
            let CellBinding::Function(body) = changed.cells[root(&changed).index()].binding else {
                panic!("function")
            };
            let mut working = changed.units[body.index()].clone().into_working();
            let data = working.get_mut();
            let tail = data.regions[data.entry.index()].operations.pop().unwrap();
            assert_eq!(tail.index(), data.operations.len() - 1);
            assert!(matches!(
                data.operations.pop().unwrap().kind,
                OperationKind::Return
            ));
            changed.units[body.index()] = working.freeze();
            changed.verify().unwrap();
            let mut ledger = ledger();
            let uses = UseIndex::build(&changed, &mut ledger, WorkDomain::Baseline).unwrap();
            let retained = ledger.retained_bytes();
            let result = prepare(
                &changed,
                &uses,
                root(&changed),
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(matches!(
                result.outcome,
                PreparationOutcome::Unknown(UnknownReason::CompletionShape)
            ));
            assert_eq!(ledger.retained_bytes(), retained);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}
#[test]
fn void_fallthrough_does_not_admit_early_branch_or_throwing_completion() {
    for source in [
        "void helper(bool choose,ref int value){if(choose){return;}value=7;}int state=1;helper(true,ref state);",
        "void helper(){throw 7;}try{helper();}catch{}",
    ] {
        checked(source,|program|{
            let mut ledger=ledger();let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();let retained=ledger.retained_bytes();
            let result=prepare(program,&uses,root(program),request(),&mut ledger,WorkDomain::Optional).unwrap();
            assert!(matches!(result.outcome,PreparationOutcome::Unknown(UnknownReason::CompletionShape|UnknownReason::BodyOperation)),"{:?}",result.outcome);
            assert_eq!(ledger.retained_bytes(),retained);uses.discard(&mut ledger).unwrap();assert_eq!(ledger.retained_bytes(),0);
        });
    }
}
