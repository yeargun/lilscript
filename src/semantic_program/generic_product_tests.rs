//! Presence follows a particular original call's returned argument. Generic
//! declarations and unrelated actuals never become scalar banks or graph clones.
use super::product_family::*;
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptExecution;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

const ENTRY: &str = include_str!("fixtures/generic-provenance/entry.lil");
const SNAPSHOT: &str = include_str!("fixtures/generic-provenance/snapshot.lil");
const POINT: &str = include_str!("fixtures/generic-provenance/point.lil");
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 20_000_000,
            optional_work: 20_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 40_000_000,
        },
    )
    .unwrap()
}
fn request() -> FamilyRequest {
    FamilyRequest {
        execution: JavaScriptExecution::Module,
        attempt: AnalysisAttempt {
            plan: PRODUCT_FAMILY_PLAN,
            algorithm_version: PRODUCT_FAMILY_VERSION,
            work_quota: 1_000_000,
        },
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
    }
}
fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}
fn named_body(program: &Program<'_>, name: &str) -> UnitId {
    program
        .cells
        .iter()
        .find_map(|cell| {
            if cell.name == name {
                if let CellBinding::Function(body) = cell.binding {
                    Some(body)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .unwrap()
}
fn score_parameter(program: &Program<'_>) -> CellId {
    program
        .unit(named_body(program, "score"))
        .unwrap()
        .parameters[0]
}
fn complete(program: &Program<'_>, uses: &UseIndex, budget: &mut BudgetLedger) -> ProductFamily {
    let result = analyze(
        program,
        uses,
        score_parameter(program),
        request(),
        budget,
        WorkDomain::Optional,
    )
    .unwrap();
    assert_eq!(result.receipt.completion, AnalysisCompletion::Complete);
    match result.outcome {
        FamilyOutcome::Complete(family) => family,
        other => panic!("expected complete original provenance: {other:?}"),
    }
}

#[test]
fn imported_forwarding_preserves_nominal_presence_for_an_independent_downstream_bank() {
    let sources = [ENTRY, SNAPSHOT, POINT];
    let graph = crate::module::ModuleSet {
        modules: sources
            .iter()
            .enumerate()
            .map(|(id, source)| crate::module::ModuleSource {
                path: format!("/generic-provenance-{id}.lil").into(),
                source: (*source).into(),
                dependencies: if id == 0 { vec![2, 1] } else { vec![] },
                foreign_dependencies: vec![],
                dynamic_dependencies: vec![],
                offset: 0,
            })
            .collect(),
        dependency_order: vec![2, 1, 0],
        root: 0,
        eager: vec![true; 3],
        for_of_specialize_family: 0,
    };
    let arena = bumpalo::Bump::new();
    let syntax: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let semantics = crate::semantic::analyze_modules(&syntax, &graph).unwrap();
    let program = from_checked_modules(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    let mut budget = ledger();
    let uses = UseIndex::build(&program, &mut budget, WorkDomain::Baseline).unwrap();
    let original: Vec<_> = program
        .units
        .iter()
        .map(|unit| {
            (
                unit.id(),
                unit.revision(),
                unit.data() as *const UnitData,
                unit.data().operations.len(),
            )
        })
        .collect();
    let family = complete(&program, &uses, &mut budget);
    assert_eq!(
        family
            .cells()
            .iter()
            .map(|cell| cell.cell)
            .collect::<Vec<_>>(),
        [score_parameter(&program)]
    );
    assert_eq!(family.schema(), program.structs()[0].identity);
    assert_eq!(
        family.fields(),
        program
            .fields()
            .iter()
            .map(|field| field.identity)
            .collect::<Vec<_>>()
    );
    let snapshot = named_body(&program, "snapshot");
    assert!(family
        .dependencies()
        .units()
        .iter()
        .any(|&(body, _)| body == snapshot));
    assert!(family
        .dependencies()
        .creators()
        .iter()
        .any(|&(body, _)| body == snapshot));
    assert!(!family
        .cells()
        .iter()
        .any(|cell| program.cells[cell.cell.index()].owner == snapshot));
    for (id, revision, address, count) in original {
        assert_eq!(program.units[id.index()].revision(), revision);
        assert_eq!(program.unit(id).unwrap() as *const UnitData, address);
        assert_eq!(program.unit(id).unwrap().operations.len(), count);
    }
    assert!(family.dependencies().valid_for(&program, &uses));
    family.discard(&mut budget).unwrap();
    uses.discard(&mut budget).unwrap();
    assert_eq!(budget.retained_bytes(), 0);
}

#[test]
fn annotations_opaque_forwarding_and_recursive_calls_cannot_seed_product_presence() {
    for source in [
        "struct P{int x;}T relay<T>(T value){return value;}extern P opaque();int score(P point){return point.x;}P value=relay(opaque());print(score(value));",
        "struct P{int x;}extern void observe(JsValue value);T relay<T>(T value){observe(value);return value;}int score(P point){return point.x;}P value=relay(P{3});print(score(value));",
        "struct P{int x;}T relay<T>(T value){return relay(value);}int score(P point){return point.x;}P value=relay(P{3});print(score(value));",
    ] {
        checked(source, |program| {
            let mut budget = ledger(); let uses = UseIndex::build(&program, &mut budget, WorkDomain::Baseline).unwrap();
            let retained = budget.retained_bytes();
            let result = analyze(&program, &uses, score_parameter(&program), request(), &mut budget, WorkDomain::Optional).unwrap();
            assert!(matches!(result.outcome, FamilyOutcome::Unknown(_)), "{source}");
            assert_eq!(budget.retained_bytes(), retained);
            uses.discard(&mut budget).unwrap(); assert_eq!(budget.retained_bytes(), 0);
        });
    }
}

#[test]
fn return_edit_follows_the_other_original_actual_and_invalidates_saved_presence() {
    checked("struct P{int x;}extern P opaque();T pick<T>(T good,T other){T a=good;T b=other;return a;}int score(P point){return point.x;}P saved=pick(P{3},opaque());print(score(saved));", |program| {
        let mut budget = ledger(); let uses = UseIndex::build(&program, &mut budget, WorkDomain::Baseline).unwrap();
        let family = complete(&program, &uses, &mut budget);
        let body = named_body(&program, "pick"); let mut changed = program.clone(); let mut working = changed.units[body.index()].clone().into_working();
        let data = working.get_mut(); let other = data.parameters[1];
        let replacement = data.operations.iter().find_map(|op| match op.kind { OperationKind::Load(place) if matches!(data.places[place.index()], Place::Cell(cell) if cell == other) => op.result, _ => None }).unwrap();
        let result = data.operations.iter().find(|op| matches!(op.kind, OperationKind::Return)).unwrap().operands;
        data.operands[result.start as usize] = replacement; changed.units[body.index()] = working.freeze(); changed.verify().unwrap();
        let updated = uses.updated(&changed, &[body], &mut budget, WorkDomain::Optional).unwrap();
        assert!(!family.dependencies().valid_for(&changed, &updated));
        let result = analyze(&changed, &updated, score_parameter(&changed), request(), &mut budget, WorkDomain::Optional).unwrap();
        assert!(matches!(result.outcome, FamilyOutcome::Unknown(_)));
        assert!(family.dependencies().valid_for(&program, &uses));
        family.discard(&mut budget).unwrap(); updated.discard(&mut budget).unwrap(); uses.discard(&mut budget).unwrap(); assert_eq!(budget.retained_bytes(), 0);
    });
}
