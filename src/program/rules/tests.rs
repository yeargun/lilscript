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
    seal: Seal::Module,
};
const SCRIPT: RuleRequest = RuleRequest {
    fold: true,
    dead_code: true,
    seal: Seal::StructuralOnly,
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
    optimized(source, MODULE, |program, _| {
        assert!(instantiated(program, "bump"));
    });
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
