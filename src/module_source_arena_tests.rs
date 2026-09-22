use super::*;
use crate::compilation_policy::{BudgetError, BudgetPlan, ResourceLimits};
use crate::output_budget::{AllocationBudget, AllocationClass};
use crate::parser::{admitted_arena_activity_for_test, AdmittedArena};
use std::panic::{catch_unwind, AssertUnwindSafe};

const SENTINEL: u64 = 31;
const MEMORY: u64 = 1_000_000;
const WORK: u64 = 10_000_000;

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    let mut ledger = BudgetLedger::new(
        ResourceLimits {
            wall_time_ms: Some(100_000),
            ..ResourceLimits::default()
        },
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap();
    ledger.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    ledger
}

#[test]
fn stable_source_text_survives_later_growth_and_ledger_moves() {
    let mut original_ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let first = sources.store("first source", &mut original_ledger).unwrap();
    let first_address = first.as_ptr();
    let first_backing = sources.allocated_bytes();
    assert!(first_backing > 0);
    let mut ledger = original_ledger;
    let text = "second source".repeat(1024);
    let second = sources.store(&text, &mut ledger).unwrap();
    assert_eq!(first, "first source");
    assert_eq!(first.as_ptr(), first_address);
    assert_eq!(second, text);
    assert!(sources.allocated_bytes() > first_backing);
    assert_eq!(
        sources.bump().allocation_limit(),
        Some(sources.allocated_bytes())
    );
    assert_eq!(
        ledger.retained_bytes(),
        SENTINEL + sources.allocated_bytes() as u64
    );
    assert_eq!(
        ledger.work_by_kind(WorkKind::Render),
        (first.len() + second.len()) as u64
    );
    assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 2);
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn source_growth_refusal_keeps_prior_text_and_other_reservations() {
    let mut empty_ledger = ledger(WORK, SENTINEL);
    let empty = StableSourceArena::new(WorkDomain::Baseline);
    assert_eq!(
        empty.store("first", &mut empty_ledger),
        Err(AllocationError::Budget(BudgetError::MemoryExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(empty.allocated_bytes(), 0);
    assert_eq!(empty_ledger.peak_retained_bytes(), SENTINEL);
    empty.discard(&mut empty_ledger).unwrap();

    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let first = sources.store("kept", &mut ledger).unwrap();
    let address = first.as_ptr();
    let backing = sources.allocated_bytes();
    let other = MEMORY - ledger.retained_bytes();
    ledger.retain(WorkDomain::Baseline, other).unwrap();
    let text = "x".repeat(backing * 2);
    assert_eq!(
        sources.store(&text, &mut ledger),
        Err(AllocationError::Budget(BudgetError::MemoryExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(first, "kept");
    assert_eq!(first.as_ptr(), address);
    assert_eq!(sources.allocated_bytes(), backing);
    assert_eq!(sources.bump().allocation_limit(), Some(backing));
    assert_eq!(ledger.retained_bytes(), MEMORY);
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL + other);
    ledger.release(WorkDomain::Baseline, other).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn source_work_and_deadline_refusals_precede_copy_and_growth() {
    for work in [0, "source".len() as u64] {
        let mut ledger = ledger(work, MEMORY);
        let sources = StableSourceArena::new(WorkDomain::Baseline);
        assert_eq!(
            sources.store("source", &mut ledger),
            Err(AllocationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(sources.allocated_bytes(), 0);
        assert_eq!(ledger.retained_bytes(), SENTINEL);
        sources.discard(&mut ledger).unwrap();
    }
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
    assert_eq!(
        sources.store("source", &mut ledger),
        Err(AllocationError::Budget(BudgetError::DeadlineExceeded))
    );
    assert_eq!(sources.allocated_bytes(), 0);
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn source_initializer_panic_keeps_new_chunks_owned_until_discard() {
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let failed = catch_unwind(AssertUnwindSafe(|| {
        let admission = SourceAdmission {
            arena: &sources,
            ledger: RefCell::new(&mut ledger),
        };
        let _ = arena_budget::attempt(sources.bump(), &admission, Layout::new::<u64>(), || {
            sources
                .bump()
                .try_alloc_with(|| -> u64 { panic!("injected source arena initializer panic") })
                .map_err(|_| AllocationError::AllocationFailed)
        });
    }));
    assert!(failed.is_err());
    let backing = sources.allocated_bytes();
    assert!(backing > 0);
    assert_eq!(sources.bump().allocation_limit(), Some(backing));
    assert_eq!(ledger.retained_bytes(), SENTINEL + backing as u64);
    assert_eq!(
        sources.store("after panic", &mut ledger).unwrap(),
        "after panic"
    );
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn temporary_read_buffer_and_stable_backing_are_charged_while_both_live() {
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let temporary = budget
        .string(AllocationClass::Retained, &"source".repeat(256))
        .unwrap();
    let temporary_bytes = temporary.capacity() as u64;
    let stable = budget.with_ledger(|owner| {
        let (ledger, _) = owner.unwrap();
        let stable = sources.store(&temporary, ledger).unwrap();
        assert_eq!(
            ledger.retained_bytes(),
            SENTINEL + temporary_bytes + sources.allocated_bytes() as u64
        );
        assert!(ledger.peak_retained_bytes() >= ledger.retained_bytes());
        stable
    });
    assert_eq!(stable, temporary);
    drop(temporary);
    drop(budget);
    assert_eq!(stable.len(), 6 * 256);
    assert_eq!(
        ledger.retained_bytes(),
        SENTINEL + sources.allocated_bytes() as u64
    );
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn incremental_program_list_preserves_identities_without_reentering_parser_ledger() {
    let mut ledger = ledger(WORK, MEMORY);
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let first = sources.store("int first=7;", &mut ledger).unwrap();
    let before = admitted_arena_activity_for_test();
    let syntax = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let mut programs = syntax.parsed_sources();
    let first_program = syntax.parse(first).unwrap();
    let first_identity = first_program.source_identity().clone();
    programs.push(first_program).unwrap();
    let second = syntax.with_ledger(|ledger, _| sources.store("int second=9;", ledger).unwrap());
    let second_program = syntax.parse(second).unwrap();
    let second_identity = second_program.source_identity().clone();
    programs.push(second_program).unwrap();
    assert_eq!(programs.len(), 2);
    assert!(programs[0].source_identity().same(&first_identity));
    assert!(programs[1].source_identity().same(&second_identity));
    assert!(!programs[0]
        .source_identity()
        .same(programs[1].source_identity()));
    syntax.with_ledger(|ledger, _| {
        assert_eq!(
            ledger.retained_bytes(),
            SENTINEL + sources.allocated_bytes() as u64 + syntax.allocated_bytes() as u64
        );
    });
    drop(programs);
    drop(syntax);
    assert_eq!(admitted_arena_activity_for_test(), (before.0, before.1 + 2));
    assert_eq!((first_identity.len(), second_identity.len()), (1, 1));
    assert_eq!(first, "int first=7;");
    assert_eq!(second, "int second=9;");
    assert_eq!(
        ledger.retained_bytes(),
        SENTINEL + sources.allocated_bytes() as u64
    );
    sources.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}
