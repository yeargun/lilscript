use super::facts::{returned_value_origin, ReturnedValueOrigin};
use super::raw_domains::Admission;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind,
};

#[derive(Debug)]
enum Error {
    Budget(BudgetError),
    Invalid(&'static str),
    UnexpectedAllocation,
}
struct Meter<'a> {
    ledger: &'a mut BudgetLedger,
    domain: WorkDomain,
}
impl Admission for Meter<'_> {
    type Error = Error;
    fn work(&mut self, amount: usize) -> Result<(), Error> {
        self.ledger
            .charge(self.domain, WorkKind::Analysis, amount as u64)
            .map_err(Error::Budget)
    }
    fn vector<T>(&mut self, _: usize) -> Result<Vec<T>, Error> {
        Err(Error::UnexpectedAllocation)
    }
    fn push<T>(&mut self, _: &mut Vec<T>, _: T) -> Result<(), Error> {
        Err(Error::UnexpectedAllocation)
    }
    fn release<T>(&mut self, _: Vec<T>) -> Result<(), Error> {
        Err(Error::UnexpectedAllocation)
    }
    fn invalid(&self, reason: &'static str) -> Error {
        Error::Invalid(reason)
    }
}
fn ledger(optional: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 2_000_000,
            optional_work: optional,
            baseline_retained_bytes: 0,
            retained_bytes: 4_000_000,
        },
    )
    .unwrap()
}
fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program);
}
fn body(program: &Program<'_>, name: &str) -> UnitId {
    program
        .cells
        .iter()
        .find_map(|cell| {
            if cell.name == name {
                if let CellBinding::Function(body) = cell.binding {
                    return Some(body);
                }
            }
            None
        })
        .unwrap()
}
fn origin(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<ReturnedValueOrigin, Error> {
    returned_value_origin(
        program,
        uses,
        body,
        &mut Meter { ledger, domain },
        |_, _, _| Ok(()),
    )
}

#[test]
fn original_copy_chains_report_parameter_origin_for_generic_and_ordinary_bodies() {
    checked("T relay<T>(T first,T second){T a=second;T b=a;return b;}int plain(int value){int saved=value;return saved;}", |program| {
        let mut ledger = ledger(1_000_000);
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let retained = ledger.retained_bytes();
        let generic = body(program, "relay");
        let revision = program.units[generic.index()].revision();
        let original = program.unit(generic).unwrap() as *const UnitData;
        let mut consulted = Vec::new();
        assert_eq!(returned_value_origin(program, &uses, generic, &mut Meter { ledger: &mut ledger, domain: WorkDomain::Optional }, |cell, stamp, _| { consulted.push((cell, stamp)); Ok(()) }).unwrap(), ReturnedValueOrigin::Parameter { position: 1 });
        assert_eq!(origin(program, &uses, body(program, "plain"), &mut ledger, WorkDomain::Optional).unwrap(), ReturnedValueOrigin::Parameter { position: 0 });
        assert!(!consulted.is_empty());
        for (cell, stamp) in consulted { assert_eq!(uses.cell(cell).unwrap().revision(), stamp); }
        assert_eq!(program.units[generic.index()].revision(), revision);
        assert_eq!(program.unit(generic).unwrap() as *const UnitData, original);
        assert_eq!(ledger.retained_bytes(), retained);
        assert!(ledger.work_used(WorkDomain::Optional) > 0);
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn opaque_control_capture_mutation_and_reference_uses_supply_no_forwarding_fact() {
    for source in [
        "int candidate(int value){return 7;}",
        "extern int opaque(int value);int candidate(int value){return opaque(value);}",
        "int candidate(int value){int saved=value;saved=2;return saved;}",
        "int candidate(int value){if(value>0)return value;return value;}",
        "int candidate(ref int value){return value;}",
        "int candidate(int value){auto callback=()=>value;return value;}",
        "extern void observe(int value);int candidate(int value){observe(value);return value;}",
    ] {
        checked(source, |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            assert!(
                matches!(
                    origin(
                        program,
                        &uses,
                        body(program, "candidate"),
                        &mut ledger,
                        WorkDomain::Optional
                    )
                    .unwrap(),
                    ReturnedValueOrigin::Unknown(_)
                ),
                "{source}"
            );
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn checked_return_edit_changes_original_argument_origin_and_rejects_stale_uses() {
    checked(
        "T choose<T>(T first,T second){T a=first;T b=second;return a;}",
        |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let body = body(program, "choose");
            assert_eq!(
                origin(program, &uses, body, &mut ledger, WorkDomain::Optional).unwrap(),
                ReturnedValueOrigin::Parameter { position: 0 }
            );
            let mut changed = program.clone();
            let mut working = changed.units[body.index()].clone().into_working();
            let data = working.get_mut();
            let second = data.parameters[1];
            let replacement = data.operations.iter().find_map(|op| match op.kind {
            OperationKind::Load(place) if matches!(data.places[place.index()], Place::Cell(cell) if cell == second) => op.result,
            _ => None,
        }).unwrap();
            let result = data
                .operations
                .iter()
                .find(|op| matches!(op.kind, OperationKind::Return))
                .unwrap()
                .operands;
            data.operands[result.start as usize] = replacement;
            changed.units[body.index()] = working.freeze();
            changed.verify().unwrap();
            assert!(matches!(
                origin(&changed, &uses, body, &mut ledger, WorkDomain::Optional),
                Err(Error::Invalid("return origin stale uses"))
            ));
            let updated = uses
                .updated(&changed, &[body], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert_eq!(
                origin(&changed, &updated, body, &mut ledger, WorkDomain::Optional).unwrap(),
                ReturnedValueOrigin::Parameter { position: 1 }
            );
            assert_eq!(
                origin(program, &uses, body, &mut ledger, WorkDomain::Optional).unwrap(),
                ReturnedValueOrigin::Parameter { position: 0 }
            );
            updated.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn return_origin_work_refusal_is_an_error_and_does_not_damage_retry_or_ownership() {
    checked(
        "T candidate<T>(T value){T saved=value;return saved;}",
        |program| {
            let mut ledger = ledger(1);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let retained = ledger.retained_bytes();
            let body = body(program, "candidate");
            assert!(matches!(
                origin(program, &uses, body, &mut ledger, WorkDomain::Optional),
                Err(Error::Budget(BudgetError::WorkExhausted(_)))
            ));
            assert_eq!(ledger.retained_bytes(), retained);
            assert_eq!(
                origin(program, &uses, body, &mut ledger, WorkDomain::Baseline).unwrap(),
                ReturnedValueOrigin::Parameter { position: 0 }
            );
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}
