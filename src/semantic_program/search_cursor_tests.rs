//! Private scheduling checks use real admitted maps and descriptors. Runtime
//! coverage and exact winner selection live in the public fairness cohort.
use super::*;
use crate::compilation_policy::{BaselineFirstPlan, CompilationRequest, ResourceLimits};

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 64_000_000;
const RAW: Objectives = Objectives::One(Objective::Raw);

fn with_states(inspect: impl FnOnce(&mut JavaScriptSearch<'_, '_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        include_str!("fixtures/value-placement/representation-composition.lil"),
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let definitions: Vec<_> = program
        .units
        .iter()
        .enumerate()
        .flat_map(|(unit, data)| {
            let types = &program.types;
            data.data().operations.iter().filter_map(move |operation| {
                let value = operation.result?;
                (matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                    && matches!(
                        types[data.data().values[value.index()].ty.index()],
                        Type::String
                    ))
                .then_some(ValueRef {
                    unit: UnitId::from_index(unit).unwrap(),
                    value,
                })
            })
        })
        .collect();
    assert_eq!(definitions.len(), 1);
    let config: crate::config::ProjectConfig = toml::from_str(
        "[javascript]\ncandidate_proposal_limit=0\nstrip_console=false\n\
         [policy.tactics]\nconstant-folding='on'\nstring-pooling='on'",
    )
    .unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let local_facts = LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    };
    let request = SearchRequest {
        objectives: RAW,
        scalar: ScalarRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
        },
        helper: HelperRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 16,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    };
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: WORK,
            retained_bytes: MEMORY,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 32 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let mut search = compiler
        .search_javascript(source, &policy, request)
        .unwrap();
    search
        .compilation
        .enable_local_facts(request.facts_cache, WorkDomain::Optional)
        .unwrap();
    search.grow_states(3, WorkDomain::Optional).unwrap();
    let direct = search.states[0].as_ref().unwrap().candidate;
    for choice in [
        StringChoice::LiteralAtDefinition,
        StringChoice::SharedLiteral {
            activation: definitions[0].unit,
        },
    ] {
        let candidate = match search
            .compilation
            .represent_string_javascript(
                direct,
                &definitions,
                choice,
                request.string,
                &policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            StringOutcome::Published(candidate) => candidate,
            other => panic!("existing string proof fixture must qualify: {other:?}"),
        };
        search
            .insert_state(candidate, 0, WorkDomain::Optional)
            .unwrap()
            .unwrap();
    }
    let mut budget =
        AllocationBudget::new(Some((&mut search.compilation.ledger, WorkDomain::Optional)));
    search.seeds = budget.filled(Retained, 4, Seed::Unseen).unwrap();
    search.seeds_charge = Some(
        budget
            .detach_retained(
                search.owner,
                bytes::<Seed>(search.seeds.capacity()).unwrap(),
            )
            .unwrap(),
    );
    drop(budget);
    inspect(&mut search);
    drop(search);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn structural_age_turns_preserve_quality_turns_and_use_canonical_ties() {
    with_states(|search| {
        for (index, state) in search.states.iter_mut().enumerate() {
            let state = state.as_mut().unwrap();
            state.last_served = index;
            state.best = [None; 3];
            state.cheap_raw = Some(100 - index * 40);
        }
        assert_eq!(search.next_state(RAW, 4).unwrap(), Some(2));
        search.counters.structural_attempts = 3;
        assert_eq!(search.next_state(RAW, 4).unwrap(), Some(0));

        search.states[0].as_mut().unwrap().pending = false;
        search.states[1].as_mut().unwrap().last_served = 2;
        search.states[2].as_mut().unwrap().last_served = 2;
        let expected = search.states[1..]
            .iter()
            .map(|state| state.as_ref().unwrap())
            .min_by(|a, b| a.identity.words().cmp(b.identity.words()))
            .unwrap()
            .candidate;
        let first = search.next_state(RAW, 4).unwrap().unwrap();
        assert_eq!(search.states[first].as_ref().unwrap().candidate, expected);
        // These two unrendered states own no artifact entries or seed handles.
        search.states.swap(1, 2);
        let second = search.next_state(RAW, 4).unwrap().unwrap();
        assert_ne!(first, second);
        assert_eq!(search.states[second].as_ref().unwrap().candidate, expected);
    });
}

#[test]
fn width_one_keeps_a_waiting_cursor_but_never_an_exhausted_tail() {
    for exhausted in [false, true] {
        with_states(|search| {
            search.states[2].as_mut().unwrap().pending = false;
            search.states[0].as_mut().unwrap().last_served = 0;
            search.states[1].as_mut().unwrap().last_served = 1;
            search.states[1].as_mut().unwrap().next = 3;
            search.states[1].as_mut().unwrap().cheap_raw = Some(1);
            search.seeds[0] = Seed::Unknown;
            search.seeds[1] = Seed::Truncated;
            search.seeds[2] = Seed::Equivalent(0);
            if exhausted {
                search.states[0].as_mut().unwrap().next = search.seeds.len();
            }
            let proposals = search.counters.proposals;
            let before = search.ledger().work_used(WorkDomain::Optional);
            search.trim_frontier(1, RAW).unwrap();
            assert!(search.ledger().work_used(WorkDomain::Optional) > before);
            assert_eq!(search.counters.structural_attempts, 0);
            assert_eq!(search.counters.proposals, proposals);
            assert_eq!(
                search.next_state(RAW, 1).unwrap(),
                Some(usize::from(exhausted))
            );
            assert_eq!(search.counters.beam_evictions, usize::from(!exhausted));
            assert_eq!(search.counters.skipped_unknown, usize::from(!exhausted));
            assert_eq!(search.counters.skipped_truncated, usize::from(!exhausted));
        });
    }
}

#[test]
fn refused_cursor_scan_preserves_progress_and_exact_incumbent() {
    with_states(|search| {
        search.seeds[0] = Seed::Unknown;
        let winner = search.with_winner(Objective::Raw, |view, _| view.javascript.to_owned());
        let retained = search.ledger().retained_bytes();
        let remaining =
            search.baseline_seal().optional_work - search.ledger().work_used(WorkDomain::Optional);
        search
            .compilation
            .ledger
            .charge(WorkDomain::Optional, WorkKind::Analysis, remaining)
            .unwrap();
        assert!(search.next_state(RAW, 4).is_err());
        assert_eq!(search.ledger().retained_bytes(), retained);
        assert_eq!(search.counters.skipped_unknown, 0);
        assert_eq!(search.counters.structural_attempts, 0);
        assert!(
            search
                .states
                .iter()
                .all(|state| state.as_ref().unwrap().next == 0)
        );
        assert_eq!(
            search.with_winner(Objective::Raw, |view, _| view.javascript.to_owned()),
            winner
        );
    });
}

fn leave_optional_work(search: &mut JavaScriptSearch<'_, '_>, allowance: u64) {
    let remaining =
        search.baseline_seal().optional_work - search.ledger().work_used(WorkDomain::Optional);
    search
        .compilation
        .ledger
        .charge(
            WorkDomain::Optional,
            WorkKind::Analysis,
            remaining - allowance,
        )
        .unwrap();
}

#[test]
fn reclaim_scan_skips_active_states_and_charges_empty_state_slots() {
    with_states(|search| {
        search.states[1].as_mut().unwrap().pending = true;
        search.states[2].as_mut().unwrap().pending = false;
        assert_eq!(search.portfolio.entries.len(), 1);
        let before = search.ledger().work_used(WorkDomain::Optional);
        search.reclaim_states().unwrap();
        assert_eq!(search.ledger().work_used(WorkDomain::Optional) - before, 3);
        assert!(search.states[1].as_ref().unwrap().pending);
        assert!(search.states[2].is_none());
        let before = search.ledger().work_used(WorkDomain::Optional);
        search.reclaim_states().unwrap();
        assert_eq!(search.ledger().work_used(WorkDomain::Optional) - before, 2);
        let before = search.ledger().work_used(WorkDomain::Optional);
        search.finish_discovery().unwrap();
        assert_eq!(search.ledger().work_used(WorkDomain::Optional) - before, 5);
        assert!(search.states[0].is_some());
        assert!(search.states[1].is_none());
    });
}

#[test]
fn partial_reclaim_refusal_preserves_pins_and_unvisited_states() {
    for allowance in 0..=4 {
        with_states(|search| {
            assert_eq!(search.portfolio.entries.len(), 1);
            for state in search.states.iter_mut().flatten() {
                state.pending = false;
            }
            let candidates: Vec<_> = search
                .states
                .iter()
                .map(|state| state.as_ref().unwrap().candidate)
                .collect();
            let winner = search.with_winner(Objective::Raw, |view, _| view.javascript.to_owned());
            leave_optional_work(search, allowance);
            let result = search.reclaim_states();
            assert_eq!(result.is_ok(), allowance == 4);
            if allowance < 4 {
                assert!(matches!(
                    result,
                    Err(SearchError::Candidate(CandidateError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Optional)
                    )))
                ));
            }
            assert_eq!(search.states[0].as_ref().unwrap().candidate, candidates[0]);
            for i in 1..3 {
                assert_eq!(search.states[i].is_none(), allowance >= 2 * i as u64);
                if let Some(state) = &search.states[i] {
                    assert_eq!(state.candidate, candidates[i]);
                }
            }
            assert_eq!(
                search.ledger().work_used(WorkDomain::Optional),
                search.baseline_seal().optional_work
            );
            assert_eq!(
                search.with_winner(Objective::Raw, |view, _| view.javascript.to_owned()),
                winner
            );
        });
    }
}

#[test]
fn partial_finish_refusal_keeps_exact_incumbent_and_releases_discovery_owners() {
    for allowance in 0..=6 {
        with_states(|search| {
            assert_eq!(search.portfolio.entries.len(), 1);
            for state in search.states.iter_mut().flatten() {
                state.pending = true;
            }
            let winner = search.with_winner(Objective::Raw, |view, _| view.javascript.to_owned());
            leave_optional_work(search, allowance);
            let result = search.finish_discovery();
            assert_eq!(result.is_ok(), allowance == 6);
            if allowance < 6 {
                assert!(matches!(
                    result,
                    Err(SearchError::Candidate(CandidateError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Optional)
                    )))
                ));
            }
            assert!(search.seeds.is_empty());
            assert!(search.seeds_charge.is_none());
            assert!(search.inventory.is_none());
            assert_eq!(search.states[0].as_ref().unwrap().pending, allowance < 2);
            assert_eq!(search.states[1].is_none(), allowance >= 4);
            assert_eq!(search.states[2].is_none(), allowance >= 6);
            assert_eq!(
                search.ledger().work_used(WorkDomain::Optional),
                search.baseline_seal().optional_work
            );
            assert_eq!(
                search.with_winner(Objective::Raw, |view, _| view.javascript.to_owned()),
                winner
            );
        });
    }
}
