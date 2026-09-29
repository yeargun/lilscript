//! Program rule tests (M5.1, M6.4a, M7.8a). Each program is converted,
//! optimized, verified, and run as unconfigured JavaScript against the
//! reference interpreter's output for the original source; then the test
//! checks what the rules removed and what they kept.

use super::*;
use crate::js::PrintPolicy;
use std::process::Command;

const MODULE: RuleRequest = RuleRequest {
    fold: true,
    dead_code: true,
    inline: true,
    seal: Seal::Module,
};
const SCRIPT: RuleRequest = RuleRequest {
    fold: true,
    dead_code: true,
    inline: true,
    seal: Seal::StructuralOnly,
};
/// F1's rules alone, for the tests that inspect what folding keeps.
const FOLD_ONLY: RuleRequest = RuleRequest {
    inline: false,
    ..MODULE
};

fn optimized<T>(
    source: &str,
    request: RuleRequest,
    inspect: impl FnOnce(&Program<'_>, RuleReceipt) -> T,
) -> T {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source)
        .unwrap_or_else(|error| panic!("parse: {error:?}\n{source}"));
    let semantics =
        crate::analyze(&syntax).unwrap_or_else(|error| panic!("check: {error:?}\n{source}"));
    let expected = crate::interpreter::interpret_program(&syntax, &semantics).unwrap();
    let program = from_checked_source(&syntax, &semantics)
        .unwrap_or_else(|error| panic!("convert: {error:?}\n{source}"));
    let (program, receipt) =
        optimize(program, request).unwrap_or_else(|error| panic!("{error}\n{source}"));
    program.verify().unwrap();
    assert_eq!(run(&program), expected, "{source}");
    inspect(&program, receipt)
}

fn run(program: &Program<'_>) -> String {
    let module = super::super::javascript::lower(program).unwrap();
    module.verify().unwrap();
    let javascript = module
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &javascript])
        .output()
        .expect("Node is required for program rule tests");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Operations of the created units that match.
fn count(program: &Program<'_>, matches: impl Fn(&OperationKind) -> bool) -> usize {
    let created = created_units(program);
    program
        .units
        .iter()
        .filter(|unit| created[unit.id().index()])
        .flat_map(|unit| unit.data().operations.iter())
        .filter(|operation| matches(&operation.kind))
        .count()
}

/// Whether a named function is still instantiated.
fn instantiated(program: &Program<'_>, name: &str) -> bool {
    program
        .cells()
        .iter()
        .any(|cell| cell.name == name && matches!(cell.binding, CellBinding::Function(_)))
}

fn constant(program: &Program<'_>, value: i32) -> bool {
    count(
        program,
        |kind| matches!(kind, OperationKind::Constant(Constant::Integer(known)) if *known == value),
    ) > 0
}

#[test]
fn a_false_branch_leaves_with_the_function_only_it_called() {
    let source = "int unused(int value) { return value * 1000; }\nprint(4);\nif (false) { print(unused(9)); }\n";
    for request in [MODULE, SCRIPT] {
        optimized(source, request, |program, receipt| {
            assert_eq!(
                count(program, |kind| matches!(kind, OperationKind::If { .. })),
                0
            );
            assert!(receipt.folded_branches >= 1);
            if request.seal == Seal::Module {
                assert!(!instantiated(program, "unused"));
                assert!(receipt.retired_functions >= 1);
            } else {
                // A script's root function is a global other scripts can call.
                assert!(instantiated(program, "unused"));
            }
        });
    }
}

#[test]
fn a_settled_root_condition_chooses_its_branch_only_when_sealed() {
    let source = "bool debug = false;\nif (debug) { print(1); } else { print(2); }\n";
    optimized(source, MODULE, |program, _| {
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::If { .. })),
            0
        );
    });
    optimized(source, SCRIPT, |program, _| {
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::If { .. })),
            1
        );
    });
}

#[test]
fn exact_values_cross_settled_cells_parameters_and_results() {
    let source = "int factor = 6;\nint repeated(int value) {\n  int first = value * 7;\n  int second = 7 * value;\n  return first + second;\n}\nprint(repeated(factor));\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(constant(program, 84), "{receipt:?}");
        assert!(!instantiated(program, "repeated"), "{receipt:?}");
    });
}

#[test]
fn conditional_values_keep_the_operand_that_runs() {
    let source = "enum Mode { Off, On }\nint pick(Mode mode) { return match (mode) { Mode.Off => 1, Mode.On => 2 }; }\nbool other(bool value) { return value; }\nprint(pick(Mode.On));\nprint(false || other(true));\nprint(true && other(false));\n";
    optimized(source, MODULE, |program, _| {
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::Select { .. })),
            0
        );
    });
}

#[test]
fn a_loop_whose_test_is_false_never_runs() {
    let source = "print(1);\nwhile (false) { print(2); }\nprint(3);\n";
    optimized(source, MODULE, |program, _| {
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::Loop { .. })),
            0
        );
    });
}

#[test]
fn a_fold_never_lengthens_its_expression() {
    let source = "int big(int shift) { return 1 << shift; }\nfloat third(float value) { return value / 3.0; }\nprint(big(30));\nprint(third(1.0));\nprint(6 * 7);\n";
    optimized(source, MODULE, |program, _| {
        // `1<<30` spelled out is longer than the expression; so is 1/3.
        assert!(!constant(program, 1 << 30));
        assert!(constant(program, 42));
    });
}

#[test]
fn a_fold_counts_what_the_output_may_name_at_one_character() {
    // A repeated literal may be read from a named constant, so `-d-1` and
    // `-d` stay; so does `(1<<30)|0`, whose inner expression is not a literal.
    // `1000*1000` folds: the shortest numeral is `1e6`.
    let source = "int[] limits = [2147483647, 2147483647, -2147483647 - 1, -2147483647];\nprint(limits[2]);\nprint(limits[3]);\nprint((1 << 30) | 0);\nprint(1000 * 1000);\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!constant(program, i32::MIN), "{receipt:?}");
        assert!(!constant(program, -2147483647), "{receipt:?}");
        assert!(!constant(program, 1 << 30), "{receipt:?}");
        assert!(constant(program, 1_000_000), "{receipt:?}");
    });
}

#[test]
fn effects_and_their_order_stay() {
    let source = "int count = 0;\nint bump() { count = count + 1; return count; }\nbump();\nint seen = bump();\nprint(count);\nprint(seen);\n";
    optimized(source, FOLD_ONLY, |program, _| {
        assert!(instantiated(program, "bump"));
    });
    optimized(source, MODULE, |_, _| {});
}

#[test]
fn unread_values_and_stores_leave() {
    let source = "int unusedValue = 5 * 7;\nint local() { int scratch = 3; scratch = 4; return 1; }\nprint(local());\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(receipt.removed_stores >= 1, "{receipt:?}");
        assert!(!constant(program, 35));
    });
}

#[test]
fn a_script_keeps_its_root_bindings() {
    let source = "int factor = 6;\nprint(factor * 7);\n";
    optimized(source, SCRIPT, |program, _| {
        assert!(!constant(program, 42));
    });
    optimized(source, MODULE, |program, _| {
        assert!(constant(program, 42));
    });
}

#[test]
fn nested_constant_structures_fold_over_rounds() {
    let source = "int choose(int a) {\n  if (a > 3) {\n    if (a > 10) { return 1; }\n    if (a > 4) { return 2; }\n    return 3;\n  }\n  return 4;\n}\nprint(choose(5));\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(constant(program, 2), "{receipt:?}");
    });
}

#[test]
fn a_function_called_once_moves_into_its_caller() {
    let source = "void report(int value) {\n  int doubled = value * 2;\n  print(doubled + 1);\n}\nfor (int i = 0; i < 3; i++) { report(i); }\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "report"), "{receipt:?}");
        assert!(receipt.inlined_calls >= 1, "{receipt:?}");
        // The copy keeps its scope: its local lives in a block.
        assert!(count(program, |kind| matches!(kind, OperationKind::Block(_))) >= 1);
    });
}

#[test]
fn a_small_body_is_copied_to_each_call_when_the_program_does_not_grow() {
    let source = "int inc(int value) { return value + 1; }\nfor (int i = 0; i < 2; i++) {\n  print(inc(i));\n  print(inc(i * 10));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "inc"), "{receipt:?}");
        assert_eq!(receipt.inlined_calls, 2, "{receipt:?}");
    });
}

#[test]
fn recursion_and_early_returns_stay_calls() {
    let source = "int fact(int n) {\n  if (n < 2) { return 1; }\n  return n * fact(n - 1);\n}\nint sign(int n) {\n  if (n < 0) { return -1; }\n  return 1;\n}\nfor (int i = 3; i < 5; i++) {\n  print(fact(i));\n  print(sign(i - 4));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "fact"), "{receipt:?}");
        assert!(instantiated(program, "sign"), "{receipt:?}");
    });
}

#[test]
fn statements_never_enter_another_calls_arguments() {
    let source = "int twice(int value) {\n  int doubled = value * 2;\n  return doubled + 1;\n}\nfor (int i = 0; i < 3; i++) { print(twice(i)); }\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "twice"), "{receipt:?}");
    });
}

#[test]
fn statements_never_split_an_expression_that_is_still_waiting() {
    // `tick()` is evaluated before `stepped(i)` and added after it: a copy
    // of `stepped`'s statements between them would hold its value.
    let source = "int count = 0;\nint tick() {\n  count = count + 1;\n  return count;\n}\nint stepped(int value) {\n  int next = value + 1;\n  return next;\n}\nfor (int i = 0; i < 2; i++) { print(tick() + stepped(i)); }\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "stepped"), "{receipt:?}");
    });
}

#[test]
fn a_typed_caller_never_triggers_a_default() {
    let source = "int scale(int value, int factor = 3) { return value * factor; }\nfor (int i = 0; i < 2; i++) {\n  print(scale(i));\n  print(scale(i, 5));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::IsUndefined)),
            0,
            "{receipt:?}"
        );
        assert!(!instantiated(program, "scale"), "{receipt:?}");
    });
}

#[test]
fn a_copied_body_reads_the_storage_its_function_captured() {
    let source = "int base = 10;\nint addBase(int value) { return value + base; }\nint outer(int value) { return addBase(value) * 2; }\nbase = 11;\nfor (int i = 0; i < 2; i++) { print(outer(i)); }\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "addBase"), "{receipt:?}");
    });
}

#[test]
fn an_arrow_held_in_a_local_is_inlined_at_its_calls() {
    let source = "void main() {\n  auto scale = (int value) => value * 3;\n  for (int i = 0; i < 2; i++) { print(scale(i)); }\n}\nmain();\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(receipt.inlined_calls >= 1, "{receipt:?}");
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::Closure(_))),
            0,
            "{receipt:?}"
        );
    });
}

#[test]
fn a_default_only_the_callee_builds_still_applies() {
    // Callers omit a trailing arrow default: the callee creates it.
    let source = "int offset = 2;\nint apply(int value, func(int)->int transform = (int current) => current + offset) {\n  return transform(value);\n}\nprint(apply(5));\nprint(apply(5, (int current) => current - 1));\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::IsUndefined)),
            1,
            "{receipt:?}"
        );
    });
}

#[test]
fn a_parameter_cell_is_a_statement() {
    // `int` arguments to an `int?` parameter: the copy initializes a cell,
    // which cannot stand inside `print`'s arguments.
    let source = "func(int?)->int? echo = (int? value) => value;\nprint(echo(12) == 12);\nprint(echo(null) == null);\n";
    optimized(source, MODULE, |_, _| {});
}

#[test]
fn a_duplicate_that_reads_its_argument_twice_stays_a_call() {
    // Each copy would have to name its argument: `(t=a[0]|0,t^t<<t)`.
    let source = "int mix(int value) { return value ^ value << value; }\nint[] values = [1, 2, 3];\nprint(mix(values[0]));\nprint(mix(values[1]));\nprint(mix(values[2]));\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "mix"), "{receipt:?}");
    });
}

#[test]
fn an_argument_is_not_read_after_something_observable() {
    // `run`'s copy would load `choose` after its arguments were evaluated,
    // which the target can spell only by holding them in temporaries.
    let source = "int counter = 0;\nint next() {\n  counter = counter + 1;\n  return counter;\n}\nbool choose(int value, bool enabled) {\n  if (enabled) { return value == 1; }\n  return false;\n}\nbool run(int value, bool enabled) { return choose(value, enabled); }\nprint(run(next(), true));\nprint(run(next(), false));\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "run"), "{receipt:?}");
    });
}

#[test]
fn a_result_cannot_leave_a_scoped_copy() {
    // `items` owns storage, so it is scoped to the copy, which then cannot
    // yield `result`; `doubled` owns nothing, so its copy needs no scope.
    let source = "int count(int value) {\n  int[] items = [value, value];\n  return items.length;\n}\nint twice(int value) {\n  int doubled = value * 2;\n  return doubled + 1;\n}\nfor (int i = 0; i < 3; i++) {\n  int result = count(i) + twice(i);\n  print(result);\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "count"), "{receipt:?}");
    });
    let source = "int twice(int value) {\n  int doubled = value * 2;\n  return doubled + 1;\n}\nfor (int i = 0; i < 3; i++) {\n  int result = twice(i);\n  print(result);\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "twice"), "{receipt:?}");
    });
}

#[test]
fn a_duplicate_with_typed_arithmetic_between_its_reads_is_free() {
    // `(left + right) + extra` adds before reading `extra`: typed ints run
    // no user code, so each copy is the expression the call stood for.
    let source = "int add(int left, int right = 1, int extra = 0) {\n  return left + right + extra;\n}\nfor (int i = 0; i < 2; i++) {\n  print(add(i));\n  print(add(i, 2));\n  print(add(i, 2, 3));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "add"), "{receipt:?}");
    });
}

/// The parameter count and return type of the function named `name`.
fn signature(program: &Program<'_>, name: &str) -> (usize, bool) {
    let cell = program
        .cells()
        .iter()
        .find(|cell| cell.name == name && matches!(cell.binding, CellBinding::Function(_)))
        .unwrap_or_else(|| panic!("no function {name}"));
    let CellBinding::Function(unit) = cell.binding else {
        unreachable!()
    };
    let data = program.unit(unit).unwrap();
    let Some(crate::check::Type::Function(function)) =
        data.callable_type.and_then(|ty| program.ty(ty))
    else {
        panic!("{name} has no signature");
    };
    assert_eq!(function.params.len(), data.parameters.len());
    (data.parameters.len(), function.return_type.is_void())
}

#[test]
fn an_unread_parameter_leaves_every_call() {
    let source = "int pick(int value, int unused) {\n  if (value > 2) { return 1; }\n  return 2;\n}\nfor (int i = 0; i < 4; i++) {\n  print(pick(i, i * 7));\n  print(pick(i + 1, 5));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(signature(program, "pick"), (1, false), "{receipt:?}");
        assert_eq!(receipt.dropped_parameters, 1, "{receipt:?}");
    });
}

#[test]
fn a_parameter_every_call_passes_the_same_constant_becomes_it() {
    let source = "int scale(int value, int factor) {\n  if (value > 100) { return 0; }\n  return value * factor;\n}\nfor (int i = 0; i < 4; i++) {\n  print(scale(i, 3));\n  print(scale(i + 1, 3));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(signature(program, "scale"), (1, false), "{receipt:?}");
        assert_eq!(receipt.constant_parameters, 1, "{receipt:?}");
        assert!(constant(program, 3), "{receipt:?}");
    });
}

#[test]
fn a_result_no_call_uses_leaves() {
    let source = "int report(int value) {\n  print(value);\n  if (value > 1) { return 1; }\n  return 0;\n}\nfor (int i = 0; i < 3; i++) {\n  report(i);\n  report(i * 2);\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(signature(program, "report"), (1, true), "{receipt:?}");
        assert_eq!(receipt.unused_results, 1, "{receipt:?}");
    });
}

#[test]
fn a_published_signature_stays() {
    let source = "export int pick(int value, int unused) {\n  if (value > 2) { return 1; }\n  return 2;\n}\nprint(pick(1, 2));\nprint(pick(3, 4));\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(signature(program, "pick"), (2, false), "{receipt:?}");
    });
}

#[test]
fn a_signature_changes_for_every_function_that_shares_it() {
    // `left` and `right` share `(int, int) -> int`; only `right` leaves its
    // first parameter unread, so both keep it: one shape stays one shape.
    // `spare` has its signature to itself, and loses its unread parameter.
    let source = "int left(int a, int b) {\n  if (a > b) { return a; }\n  return b;\n}\nint right(int a, int b) {\n  if (b > 2) { return 1; }\n  return b;\n}\nbool spare(bool unused, float value) {\n  if (value > 1.5) { return true; }\n  return false;\n}\nfor (int i = 0; i < 4; i++) {\n  print(left(i, 2));\n  print(right(i, i));\n  print(spare(true, 1.0 * i));\n  print(spare(false, 2.0));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(signature(program, "left"), (2, false), "{receipt:?}");
        assert_eq!(signature(program, "right"), (2, false), "{receipt:?}");
        assert_eq!(signature(program, "spare"), (1, false), "{receipt:?}");
    });
}

/// An `int` is its own ToInt32: `x | 0` and the other identities with 0 are
/// `x`; a `float` operand keeps its conversion.
#[test]
fn bitwise_identities_of_an_int_are_the_int() {
    let source = "int keep(int x) { return (x | 0) + (0 ^ x) + (((x << 0) | 0) | 0); }\nint convert(float y) { return y | 0; }\nprint(keep(5));\nprint(convert(-2.5));\n";
    optimized(source, FOLD_ONLY, |program, _| {
        assert_eq!(
            count(program, |kind| matches!(
                kind,
                OperationKind::Binary(BinaryOp::BitOr | BinaryOp::Xor | BinaryOp::ShiftLeft)
            )),
            1
        );
    });
}

/// A folded branch that ends in an exit joins its region, and what follows
/// the exit is unreachable and goes (M7.8a's dead code after folding): the
/// body's one `return` gives the call an exact result, the call folds to it,
/// and the function is retired (the catalog's `number/clamp` family).
#[test]
fn a_folded_exit_leaves_one_return_that_inlines() {
    let source = "int clamp(int value, int lo, int hi) {
  if (value < lo) { return lo; }
  if (value > hi) { return hi; }
  return value;
}
print(clamp(-2, 0, 10));";
    optimized(source, MODULE, |program, receipt| {
        assert!(receipt.unreachable_operations >= 1, "{receipt:?}");
        assert_eq!(receipt.inlined_calls + receipt.retired_functions, 1, "{receipt:?}");
        assert_eq!(count(program, |kind| matches!(kind, OperationKind::Return)), 0);
    });
}

/// After a `return` inside a loop's body nothing of that body runs.
#[test]
fn operations_after_an_exit_go_and_initializations_stay_whole() {
    let source = "int first(int n) {
  int total = 0;
  for (int i = 0; i < n; i += 1) {
    total += i;
    return total;
    print(i);
  }
  return total;
}
print(first(3));
print(first(0));";
    // The run matches the interpreter's (`optimized` checks it).
    optimized(source, MODULE, |_, receipt| {
        assert!(receipt.unreachable_operations >= 1, "{receipt:?}");
    });
}
