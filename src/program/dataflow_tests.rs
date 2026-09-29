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
        assert!(*solution.before(operation), "{:?}", unit.operations[operation.index()].kind);
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
