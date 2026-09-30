//! Value ranges (M6.4b): a counting loop bounds its counter, a cell carries
//! its range, a callee's returns bound its result, and a loop that grows
//! settles by widening.
use super::super::call_graph::Seal;
use super::*;

fn program<'src>(arena: &'src bumpalo::Bump, source: &'src str) -> Program<'src> {
    let syntax = crate::parse_source(arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let checked =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let program = from_checked_source(&syntax, &checked)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    program.verify().unwrap();
    program
}

fn unit_named(program: &Program<'_>, name: &str) -> UnitId {
    let index = program
        .units
        .iter()
        .position(|unit| {
            unit.data()
                .function_name
                .is_some_and(|label| program.strings[label.index()].as_unicode() == Some(name))
        })
        .unwrap_or_else(|| panic!("no unit named {name}"));
    UnitId::from_index(index).unwrap()
}

/// The bounds of the value `function` returns (its last `return`).
fn returned(source: &str, function: &str) -> Option<(i64, i64)> {
    let arena = bumpalo::Bump::new();
    let program = program(&arena, source);
    let ranges = program.ranges(Seal::Module);
    let unit = unit_named(&program, function);
    let data = program.units[unit.index()].data();
    let value = data
        .operations
        .iter()
        .rev()
        .find(|operation| matches!(operation.kind, OperationKind::Return))
        .and_then(|operation| data.operands(operation.operands)?.first().copied())
        .expect("a returned value");
    ranges.number(unit, value).integer_bounds()
}

/// The bounds of every `int` addition's raw result in `function`, before its
/// ToInt32 (what decides whether the `|0` is needed).
fn additions(source: &str, function: &str) -> Vec<Option<(i64, i64)>> {
    let arena = bumpalo::Bump::new();
    let program = program(&arena, source);
    let ranges = program.ranges(Seal::Module);
    let unit = unit_named(&program, function);
    let data = program.units[unit.index()].data();
    data.operations
        .iter()
        .filter(|operation| {
            matches!(operation.kind, OperationKind::IntBinary(op) if op == crate::primitive::IntBinary::Add)
        })
        .map(|operation| {
            let operands = data.operands(operation.operands).unwrap();
            let left = ranges.number(unit, operands[0]);
            let right = ranges.number(unit, operands[1]);
            left.binary(crate::js::Binary::Add, right).integer_bounds()
        })
        .collect()
}

#[test]
fn conditional_results_publish_the_join_after_their_arms() {
    let source = "export int selected(bool choice){int first=if(choice){7}else{9};return first*3+1;}print(selected(true));print(selected(false));";
    assert_eq!(returned(source, "selected"), Some((22, 28)));
    let nested = "export int selected(bool a,bool b){return if(a){if(b){3}else{5}}else{7};}print(selected(true,false));";
    assert_eq!(returned(nested, "selected"), Some((3, 7)));
}

#[test]
fn forwarding_keeps_branch_local_range_observations() {
    let arena = bumpalo::Bump::new();
    let input = program(&arena, "export int clamp(int value){if(value < -120){return -120;}if(value > 120){return 120;}return value;}print(clamp(500));");
    let (program, _) = super::super::rules::optimize(input, super::super::rules::RuleRequest {
        fold: true, dead_code: true, inline: false, pristine_builtins: false,
        seal: Seal::Module,
    }).unwrap();
    let unit = unit_named(&program, "clamp");
    let data = program.units[unit.index()].data();
    let ranges = program.ranges(Seal::Module);
    let returned = data.operations.iter().filter(|operation| matches!(operation.kind, OperationKind::Return))
        .filter_map(|operation| data.operands(operation.operands).and_then(|values| values.first()))
        .map(|&value| ranges.number(unit, value).integer_bounds().unwrap())
        .collect::<Vec<_>>();
    assert!(returned.iter().all(|&(low, high)| low >= -120 && high <= 120), "{returned:?}");
}

#[test]
fn a_counting_loop_bounds_its_counter() {
    let source = "int count(int n) { int s = 0; for (int i = 0; i < n; i += 1) { s = s ^ i; } return s; } print(count(5));";
    // `i += 1` reads `i <= n - 1 <= 2^31 - 2`: the sum stays in int32.
    let sums = additions(source, "count");
    assert!(
        sums.iter()
            .any(|bounds| bounds.is_some_and(|(low, high)| low >= 0 && high <= i32::MAX as i64)),
        "{sums:?}"
    );
}

#[test]
fn a_cell_carries_its_range_and_a_callee_its_result() {
    // Exported, so its parameter is its type's (a caller outside the
    // program may pass anything).
    let source = "export int scaled(int n) { int t = n & 255; t = t * 3; return t + 1; } print(scaled(1000));";
    assert_eq!(returned(source, "scaled"), Some((1, 766)));
    let source =
        "int clamp(int v) { if (v < -120) { return -120; } if (v > 120) { return 120; } return v; }
export int twice(int a) { return clamp(a) + clamp(a); } print(twice(7));";
    assert_eq!(returned(source, "clamp"), Some((-120, 120)));
    assert_eq!(returned(source, "twice"), Some((-240, 240)));
}

#[test]
fn a_loop_that_grows_settles_by_widening() {
    let source = "int grow(int n) { int x = 1; while (x < n) { x = x * 2 + 1; } return x; } print(grow(100));";
    // Settled (the solve did not give up): the loop's exit reads `x >= n`,
    // and `x` is an int32 whatever it grew to.
    let bounds = returned(source, "grow");
    assert!(
        bounds.is_some_and(|(low, high)| low >= i32::MIN as i64 && high <= i32::MAX as i64),
        "{bounds:?}"
    );
}

#[test]
fn a_parameter_joins_what_its_complete_call_set_passes() {
    // `inc`'s only caller passes `n & 7`, `n` being any int (`f` is
    // exported): its parameter is in [0,7] from the second round on, and
    // its result in [1,8]. Unexported, `f`'s own caller passes 1000, and
    // `inc` knows its argument exactly.
    let source = "int inc(int x) { return x + 1; } export int f(int n) { return inc(n & 7); } print(f(1000));";
    assert_eq!(returned(source, "inc"), Some((1, 8)));
    let source =
        "int inc(int x) { return x + 1; } int f(int n) { return inc(n & 7); } print(f(1000));";
    assert_eq!(returned(source, "inc"), Some((1, 1)));
    // An exported body may be called by anyone: its parameter is its type's.
    let source = "export int inc(int x) { return x + 1; } int f(int n) { return inc(n & 7); } print(f(1000));";
    assert!(
        returned(source, "inc").is_some_and(|(low, high)| high - low > 1 << 31),
        "{:?}",
        returned(source, "inc")
    );
}
