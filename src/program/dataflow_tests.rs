//! The region-structured solver (M5.5): reachability after abrupt exits,
//! the join of branches, and the bound on a lattice that never settles.
use super::*;

/// Reachability: whether any path reaches an operation.
struct Reached;

impl Forward for Reached {
    type State = bool;
    fn unreachable(&self) -> bool {
        false
    }
    fn join(&self, into: &mut bool, from: &bool) {
        *into |= *from;
    }
    fn transfer(&self, _: &UnitData, _: OpId, _: &mut bool) {}
}

/// A counter that grows at every operation: no fixed point in a loop.
struct Counting;

impl Forward for Counting {
    type State = u64;
    fn unreachable(&self) -> u64 {
        0
    }
    fn join(&self, into: &mut u64, from: &u64) {
        *into = (*into).max(*from);
    }
    fn transfer(&self, _: &UnitData, _: OpId, state: &mut u64) {
        *state += 1;
    }
}

fn unit_named<'a>(program: &'a Program<'_>, name: &str) -> &'a UnitData {
    program
        .units
        .iter()
        .find(|unit| {
            unit.data()
                .function_name
                .is_some_and(|label| program.strings[label.index()].as_unicode() == Some(name))
        })
        .unwrap_or_else(|| panic!("no unit named {name}"))
        .data()
}

fn program<'src>(arena: &'src bumpalo::Bump, source: &'src str) -> Program<'src> {
    let syntax = crate::parse_source(arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    from_checked_source(&syntax, &checked).unwrap()
}

#[test]
fn nothing_after_a_return_is_reached_and_a_branch_joins() {
    let arena = bumpalo::Bump::new();
    let program = program(
        &arena,
        "int f(bool c){if(c){return 1;}print(2);return 3;} print(f(true));",
    );
    let unit = unit_named(&program, "f");
    let solution = solve(unit, &Reached, true, |_| Ok::<_, ()>(())).unwrap();
    // Every operation of the entry region is reached: the `if` without
    // `else` joins its branch with its entry.
    for &operation in &unit.regions[unit.entry.index()].operations {
        assert!(
            *solution.before(operation),
            "{:?}",
            unit.operations[operation.index()].kind
        );
    }
}

#[test]
fn a_loop_that_never_settles_is_refused() {
    let arena = bumpalo::Bump::new();
    let program = program(
        &arena,
        "int g(int n){int t=0;while(t<n){t=t+1;}return t;} print(g(3));",
    );
    let unit = unit_named(&program, "g");
    assert!(matches!(
        solve(unit, &Counting, 0, |_| Ok::<_, ()>(())),
        Err(Stop::Unsettled)
    ));
}

#[test]
fn q2_transfer_only_flow_admits_live_states_without_operation_history() {
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
    struct Fixed;
    impl Forward for Fixed {
        type State = Vec<u64>;
        fn unreachable(&self) -> Self::State { vec![0; 128] }
        fn empty_in(&self, budget: &mut AllocationBudget<'_>) -> Result<Self::State, AllocationError> {
            budget.filled(Retained, 128, 0)
        }
        fn copy_in(&self, state: &Self::State, budget: &mut AllocationBudget<'_>) -> Result<Self::State, AllocationError> {
            budget.copy_slice(Retained, state)
        }
        fn join(&self, into: &mut Self::State, from: &Self::State) {
            for (into, from) in into.iter_mut().zip(from) { *into = (*into).max(*from); }
        }
        fn transfer(&self, _: &UnitData, _: OpId, state: &mut Self::State) { state[0] += 1; }
    }
    let arena = bumpalo::Bump::new();
    let source = "print(1);".repeat(300);
    let syntax = crate::parse_source(&arena, &source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let data = program.units[0].data();
    assert!(data.operations.len() > 300);
    for memory in [0, 1024, 2047, 2048, 4096] {
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
            baseline_work: 0, optional_work: 1_000_000, baseline_retained_bytes: 0, retained_bytes: memory,
        }).unwrap();
        let outcome = solve_discard_in(data, &Fixed, &vec![0; 128],
            &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional))));
        assert_eq!(outcome.is_ok(), memory >= 2048);
        assert_eq!(ledger.retained_bytes(), 0);
        assert!(ledger.peak_retained_bytes() <= 2048);
    }
}
