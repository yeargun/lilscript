//! Program rule tests (M5.1, M6.4a, M7.8a). Each program is converted,
//! optimized, verified, and run as unconfigured JavaScript against the
//! reference interpreter's output for the original source; then the test
//! checks what the rules removed and what they kept.

use super::*;
use crate::js::PrintPolicy;
use std::process::Command;

const MODULE: RuleRequest = RuleRequest {
    unroll: true, pool: true,
    fold: true,
    dead_code: true,
    inline: true,
    scalar: false,
    native: false,
    pristine_builtins: false,
    seal: Seal::Module,
};
const SCRIPT: RuleRequest = RuleRequest {
    unroll: true, pool: true,
    fold: true,
    dead_code: true,
    inline: true,
    scalar: false,
    native: false,
    pristine_builtins: false,
    seal: Seal::StructuralOnly,
};
/// F1's rules alone, for the tests that inspect what folding keeps.
const FOLD_ONLY: RuleRequest = RuleRequest {
    inline: false,
    ..MODULE
};
const DCE_ONLY: RuleRequest = RuleRequest {
    unroll: true, pool: true,
    fold: false,
    inline: false,
    ..MODULE
};

#[test]
fn rule_views_survive_empty_commits_and_follow_every_edit_owner() {
    use crate::check::Type;
    use std::sync::Arc;

    for seal in [Seal::Module, Seal::StructuralOnly] {
        let arena = bumpalo::Bump::new();
        let syntax =
            crate::parse_source(&arena, "int value=3;print(value);print(\"same\");").unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let mut editor = edit::Editor::new(from_checked_source(&syntax, &semantics).unwrap());
        let effects = editor.program().effects(seal);
        let ranges = editor.program().ranges(seal);
        editor.commit().unwrap();
        // Looking up already interned data is not an edit either.
        editor.intern_type(Type::Int).unwrap();
        editor.intern_string(&"same".into()).unwrap();
        editor.commit().unwrap();
        assert!(Arc::ptr_eq(&effects, &editor.program().effects(seal)));
        assert!(Arc::ptr_eq(&ranges, &editor.program().ranges(seal)));

        let root = editor.program().initialization()[0];
        let operation = editor
            .unit_mut(root)
            .operations
            .iter_mut()
            .find(|operation| {
                matches!(
                    operation.kind,
                    OperationKind::Constant(Constant::Integer(3))
                )
            })
            .unwrap();
        operation.kind = OperationKind::Constant(Constant::Integer(4));
        editor.commit().unwrap();
        assert!(!effects.deps().valid_for(editor.program()));
        assert!(!ranges.deps().valid_for(editor.program()));
        assert!(!Arc::ptr_eq(&effects, &editor.program().effects(seal)));
        editor.program().verify().unwrap();
        assert_eq!(run(editor.program()), "4\nsame\n");

        // Table changes must invalidate even if no operation changed.
        for mutation in 0..4 {
            let effects = editor.program().effects(seal);
            let ranges = editor.program().ranges(seal);
            match mutation {
                0 => {
                    editor.intern_string(&"new data".into()).unwrap();
                }
                1 => {
                    editor
                        .intern_type(Type::Array(Box::new(Type::Record(Box::new(Type::Int)))))
                        .unwrap();
                }
                2 => {
                    let mut cell = editor.program().cells()[0].clone();
                    cell.name = "synthetic_local".into();
                    cell.synthetic = true;
                    cell.binding = CellBinding::Local;
                    editor.add_cell(cell).unwrap();
                }
                _ => {
                    let (_, cells) = editor.unit_and_cells(root);
                    cells[0].reassigned = true;
                }
            }
            editor.commit().unwrap();
            assert!(
                !effects.deps().valid_for(editor.program()),
                "mutation {mutation}"
            );
            assert!(
                !ranges.deps().valid_for(editor.program()),
                "mutation {mutation}"
            );
            assert!(!Arc::ptr_eq(&effects, &editor.program().effects(seal)));
            assert!(!Arc::ptr_eq(&ranges, &editor.program().ranges(seal)));
            editor.program().verify().unwrap();
        }
        let scratch = editor.program().effects(seal);
        let program = editor.finish().unwrap();
        assert!(scratch.deps().valid_for(&program));
        assert!(
            !Arc::ptr_eq(&scratch, &program.effects(seal)),
            "rule scratch must end before publication"
        );
    }
}

#[test]
fn cold_and_reused_rule_views_preserve_decisions_bytes_and_execution() {
    struct ColdViews(bool);
    impl ColdViews {
        fn set(cold: bool) -> Self {
            Self(COLD_RULE_VIEWS.with(|state| state.replace(cold)))
        }
    }
    impl Drop for ColdViews {
        fn drop(&mut self) {
            COLD_RULE_VIEWS.with(|state| state.set(self.0));
        }
    }
    for request in [MODULE, SCRIPT] {
        for source in [
            "int sum(int n){int s=0;for(int i=0;i<n;i+=1){s+=i;}return s;}if(false){print(99);}print(sum(5));",
            "int current=2;int read(){return current;}int next(int n){current+=n;return read();}print(next(3));print(read());",
            "int outer(int n){int value=n;auto inner=(int k)=>{value+=k;return value;};return inner(2)+inner(3);}print(outer(7));",
            "string choose(bool first){if(first){return \"a\"+\"b\";}return \"c\";}print(choose(true));print(choose(false));",
        ] {
            let outcomes = [false, true].map(|cold| {
                let _guard = ColdViews::set(cold);
                optimized(source, request, |program, receipt| {
                    let javascript = super::super::javascript::lower(program).unwrap()
                        .render(PrintPolicy { mangle_bindings: true }).unwrap();
                    (javascript, receipt)
                })
            });
            assert_eq!(outcomes[0], outcomes[1], "{request:?}\n{source}");
        }
    }
}

#[test]
fn constant_calls_are_evaluated_for_each_argument_tuple() {
    let source = r#"
        int choose(bool doubled, int value) {
            int result=value+1;
            if(doubled){result=result*2;}else{result=result-3;}
            return result;
        }
        print(choose(true,4)); print(choose(false,7));
    "#;
    optimized(source, FOLD_ONLY, |program, receipt| {
        assert!(!instantiated(program, "choose"), "{receipt:?}");
        assert!(constant(program, 10) && constant(program, 5));
    });
}

#[test]
fn finite_call_sets_fold_a_common_branch_without_sampling_overflow() {
    fn source(arguments: &[i32]) -> String {
        let tail = (0..70).map(|_| "print(0);").collect::<String>();
        let calls = arguments
            .iter()
            .map(|value| format!("print(classify({value}));"))
            .collect::<String>();
        format!(
            "bool classify(int value){{if(value>0){{return true;}}else{{{tail}return false;}}}}{calls}"
        )
    }

    optimized(&source(&[1, 2]), FOLD_ONLY, |program, receipt| {
        assert!(!instantiated(program, "classify"), "{receipt:?}");
        assert!(receipt.folded_branches > 0, "{receipt:?}");
        assert!(receipt.set_folds > 0, "{receipt:?}");
        assert_eq!(receipt.folded_calls, 2, "{receipt:?}");
    });
    optimized(&source(&[1, 2, 3, 4, 5]), FOLD_ONLY, |program, receipt| {
        assert!(instantiated(program, "classify"), "{receipt:?}");
        assert_eq!(receipt.folded_calls, 0, "{receipt:?}");
    });
}

#[test]
fn finite_values_are_refined_on_each_branch_path() {
    let padding = (0..70).map(|_| "print(0);").collect::<String>();
    let source = format!(
        "bool classify(int value){{if(value>0){{if(value<=0){{{padding}}}return true;}}else{{if(value>=0){{{padding}}}return true;}}}}print(classify(-2));print(classify(3));"
    );
    optimized(&source, FOLD_ONLY, |program, receipt| {
        assert!(!instantiated(program, "classify"), "{receipt:?}");
        assert_eq!(receipt.folded_calls, 2, "{receipt:?}");
        assert!(receipt.path_folds > 0, "{receipt:?}");
    });
}

#[test]
fn immutable_scalar_cells_and_common_primitive_operations_forward() {
    let source = r#"
        int compute(int value) {
            int alias = value;
            int first = alias * 3;
            print(first);
            print(first);
            int second = alias * 3;
            return first + second;
        }
        for (int i = 0; i < 3; i += 1) { print(compute(i)); }
    "#;
    optimized(source, FOLD_ONLY, |_, receipt| {
        assert!(receipt.forwarded_definitions >= 3, "{receipt:?}");
        assert_eq!(receipt.common_computations, 1, "{receipt:?}");
    });
    optimized(source, DCE_ONLY, |_, receipt| {
        assert_eq!(receipt.forwarded_definitions, 0, "{receipt:?}");
        assert_eq!(receipt.common_computations, 0, "{receipt:?}");
    });
}

#[test]
fn host_annotations_do_not_authorize_common_coercions() {
    for (source, setup, expected) in [
        ("extern int input;int compute(int value){return value*3+value*3;}print(compute(input));",
         "globalThis.input={valueOf(){console.log('coerce');return 2;}};", "coerce\ncoerce\n12\n"),
        ("extern string input;int compute(string value){return value.length+value.length;}print(compute(input));",
         "globalThis.input={get length(){console.log('read');return 2;}};", "read\nread\n4\n"),
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let (program, receipt) = optimize(program, FOLD_ONLY).unwrap();
        assert_eq!(receipt.common_computations, 0, "{receipt:?}");
        let javascript = super::super::javascript::lower(&program).unwrap()
            .render(PrintPolicy { mangle_bindings: true }).unwrap();
        let script = format!("{setup}{javascript}");
        let output = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    }
}

#[test]
fn forwarding_preserves_snapshots_writes_and_argument_evaluation() {
    for source in [
        "int count=0;int next(){count+=1;print(count);return count;}int mix(int value){return value^value<<value;}print(mix(next()));print(mix(next()));",
        "int compute(int value){int current=value;current=4;int snapshot=current;current=7;return snapshot+current;}for(int i=0;i<3;i+=1){print(compute(i));}",
        "int compute(int value){int current=value;if(value>0){current=9;}return current;}print(compute(1));print(compute(-2));",
        "int[] values=[1,2];int[] alias=values;values[0]=3;print(alias[0]);",
        "int value=2;int read(){return value;}auto alias=read;value=7;print(alias());",
    ] { optimized(source, MODULE, |_, _| {}); }
}

#[test]
fn primitive_truthiness_in_finite_paths_matches_javascript() {
    let arena = bumpalo::Bump::new();
    let source = "bool classify(JsValue value){if(!bool(value)){return true;}else{return false;}}print(classify(\"\"));print(classify(\"x\"));";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let (program, _) = optimize(program, FOLD_ONLY).unwrap();
    let reference = Command::new("node")
        .args([
            "-e",
            "const classify=value=>!value;console.log(classify(''));console.log(classify('x')); ",
        ])
        .output()
        .unwrap();
    assert!(reference.status.success());
    assert_eq!(run(&program).as_bytes(), reference.stdout);
}

#[test]
fn default_transport_preserves_callee_guards_and_argument_effects() {
    let source = r#"
        int tick(int value) { print(value); return value; }
        void report(int tag, int? value = null) { print(tag); print(value); }
        for (int i=0; i<2; i+=1) { report(tick(i)); report(i,7); }
    "#;
    optimized(source, FOLD_ONLY, |program, receipt| {
        assert!(receipt.default_arguments_omitted > 0, "{receipt:?}");
        assert!(receipt.native_defaults > 0, "{receipt:?}");
        assert!(
            count(program, |kind| matches!(
                kind,
                OperationKind::IsUndefined { .. }
            )) > 0
        );
        // Re-entering optimization must also preserve the target-entry guard.
        let (again, _) = optimize(program.clone(), FOLD_ONLY).unwrap();
        assert_eq!(run(&again), run(program));
        // Structurally valid metadata still needs a semantic proof: the tag
        // argument has no matching default and cannot join the omitted tail.
        let (unit, call) = program
            .units
            .iter()
            .find_map(|unit| {
                unit.data()
                    .calls
                    .iter()
                    .enumerate()
                    .find(|(_, site)| {
                        site.omit_trailing > 0 && site.omit_trailing < site.arguments.len
                    })
                    .map(|(call, _)| (unit.id(), call))
            })
            .unwrap();
        let mut broken = edit::Editor::new(program.clone());
        broken.unit_mut(unit).calls[call].omit_trailing += 1;
        let broken = broken.finish().unwrap();
        assert!(broken.verify().unwrap_err().contains("default transport"));
    });
}

#[test]
fn default_transport_keeps_observed_argument_count_and_signed_zero() {
    for source in [
        "extern JsValue arguments; void report(float value=-0.0){print(arguments[\"length\"]);print(1.0/value);} report();report(0.0);",
        "extern JsValue arguments; void report(float value=-0.0){auto observe=()=>arguments[\"length\"];print(observe());print(1.0/value);}report();report(0.0);",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let expected = run(&program);
        let (program, receipt) = optimize(program, FOLD_ONLY).unwrap();
        assert_eq!(receipt.default_arguments_omitted, 0, "{receipt:?}");
        assert_eq!(run(&program), expected);
    }
}

#[test]
fn completed_constant_loops_preserve_control_flow_and_iteration_state() {
    for source in [
        // A zero-trip loop must not execute a throwing body.
        "int calculate(int n){int result=7;while(n>0){throw 99;}return result;}print(calculate(0));",
        // Continue evaluates the for-update; a fresh body local is reset.
        "int calculate(int n){int result=0;for(int i=0;i<n;i+=1){int local=10;if(i==2){continue;}local+=i;result+=local;}return result;}print(calculate(5));",
        // The inner break belongs to the inner loop, and continue to its body.
        "int calculate(int n){int result=0;int i=0;while(i<n){i+=1;int j=0;while(j<4){j+=1;if(j==2){continue;}if(j==4){break;}result+=i*j;}}return result;}print(calculate(3));",
        // Return crosses all loop/block nesting without evaluating an update.
        "int calculate(int n){for(int i=0;i<n;i+=1){if(i==3){return i*7;}}return -1;}print(calculate(5));print(calculate(0));",
        // No condition still permits a completed break and wrapping arithmetic.
        "int calculate(int n){int i=0;for(;;){n+=2147483647;i+=1;if(i==4){break;}}return n;}print(calculate(5));",
        // The condition and update are executions, including known nested calls.
        "int advance(int n){return n+1;}int calculate(int n){int result=0;for(int i=0;advance(i)<n;i=advance(i)){result+=i;}return result;}print(calculate(5));",
    ] {
        optimized(source, FOLD_ONLY, |program, receipt| {
            assert!(!instantiated(program, "calculate"), "{receipt:?}\n{source}");
            assert!(receipt.folded_calls > 0, "{receipt:?}\n{source}");
        });
    }
}

#[test]
fn constant_loops_keep_effects_and_calls_that_exhaust_evaluation() {
    for source in [
        "int calculate(int n){int result=0;while(n>0){print(n);result+=n;n-=1;}return result;}print(calculate(3));",
        "int current=0;int calculate(int n){while(n>0){current+=n;n-=1;}return current;}print(calculate(3));print(current);",
        // Small body, deliberately more executed work than the call's bound.
        "int calculate(int n){int result=0;for(int i=0;i<n;i+=1){result+=i;}return result;}print(calculate(2000));",
    ] {
        optimized(source, FOLD_ONLY, |program, receipt| {
            assert!(instantiated(program, "calculate"), "{receipt:?}\n{source}");
        });
    }
}

#[test]
fn constant_loop_evaluation_preserves_observable_exceptions() {
    // The reference interpreter has no exceptions. Use a direct JavaScript
    // oracle with the same executed throw and catch instead.
    let oracle = Command::new("node")
        .args([
            "-e",
            "try{console.log((n=>{while(n>0){throw n;}return 7;})(1));}catch{console.log(99);}",
        ])
        .output()
        .unwrap();
    assert!(oracle.status.success());
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int calculate(int n){while(n>0){throw n;}return 7;}try{print(calculate(1));}catch{print(99);}").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let (program, receipt) = optimize(program, FOLD_ONLY).unwrap();
    program.verify().unwrap();
    assert!(instantiated(&program, "calculate"), "{receipt:?}");
    assert_eq!(run(&program).as_bytes(), oracle.stdout);
}

#[test]
fn constant_call_folding_preserves_argument_effects_and_mutable_captures() {
    let source = r#"
        int current=1;
        int read(){return current;}
        int tick(){print(41);return 2;}
        int twice(int value){return value+value;}
        print(read()); current=2; print(read()); print(twice(tick()));
    "#;
    optimized(source, FOLD_ONLY, |program, receipt| {
        assert!(instantiated(program, "read"), "{receipt:?}");
        assert!(instantiated(program, "tick"), "{receipt:?}");
        assert!(!instantiated(program, "twice"), "{receipt:?}");
    });
}

#[test]
fn known_methods_fold_only_with_the_builtin_contract() {
    let source = r#"
        print("hello".indexOf("ll"));
        print("abcdef".slice(1,3));
        print(" Abc ".trim().toUpperCase());
        print("abc".startsWith("ab"));
        print("abc".endsWith("bc"));
    "#;
    let calls =
        |program: &Program<'_>| count(program, |kind| matches!(kind, OperationKind::Call(_)));
    let before = optimized(source, FOLD_ONLY, |program, _| calls(program));
    optimized(
        source,
        RuleRequest {
            pristine_builtins: true,
            ..FOLD_ONLY
        },
        |program, receipt| {
            assert_eq!(calls(program), 5, "{receipt:?}");
            assert!(before > calls(program));
        },
    );
}

#[test]
fn constant_scalar_methods_fold_through_nested_pure_calls() {
    let source = r#"
        int truncate(float n){return n.toInt();}
        int count(string s){return s.codePointLength();}
        int calculate(float n,string s){return truncate(n)+count(s);}
        print(calculate(4294967297.5,"😀x"));
        print(calculate(-4294967297.5,"A"));
    "#;
    optimized(
        source,
        RuleRequest {
            pristine_builtins: true,
            ..FOLD_ONLY
        },
        |program, receipt| {
            assert!(!instantiated(program, "calculate"), "{receipt:?}");
            assert!(!instantiated(program, "truncate"), "{receipt:?}");
            assert!(!instantiated(program, "count"), "{receipt:?}");
            assert_eq!(
                count(program, |kind| matches!(kind, OperationKind::Call(_))),
                2
            );
        },
    );
}

#[test]
fn constant_numeric_conversion_needs_no_pristine_host_assumption() {
    let source = r#"
        int truncate(float n){return n.toInt();}
        int calculate(float n){return truncate(n)+1;}
        print(calculate(4294967297.5));
        print(calculate(-4294967297.5));
    "#;
    optimized(source, FOLD_ONLY, |program, receipt| {
        assert!(!instantiated(program, "calculate"), "{receipt:?}");
        assert!(!instantiated(program, "truncate"), "{receipt:?}");
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::Call(_))),
            2
        );
    });
}

#[test]
fn bounded_constant_calls_preserve_nontermination_and_throwing_methods() {
    for source in [
        "int spin(int n){return spin(n+1);} export int go(){return spin(0);}",
        "int spin(int n){while(n>0){}return 7;} export int go(){return spin(1);}",
        "int spin(){for(;;){}return 7;} export int go(){return spin();}",
        "export string go(){return \"x\".repeat(-1);}",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let (program, _) = optimize(
            program,
            RuleRequest {
                pristine_builtins: true,
                ..FOLD_ONLY
            },
        )
        .unwrap();
        assert!(
            count(&program, |kind| matches!(kind, OperationKind::Call(_))) > 0,
            "{source}"
        );
    }
}

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
fn the_default_fold_bound_keeps_large_constants_as_expressions() {
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
fn a_repeated_nonempty_body_stays_available_to_objective_judging() {
    let source = "int inc(int value) { return value + 1; }\nfor (int i = 0; i < 2; i++) {\n  print(inc(i));\n  print(inc(i * 10));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "inc"), "{receipt:?}");
        assert_eq!(receipt.inlined_calls, 0, "{receipt:?}");
    });
}

#[test]
fn recursion_stays_callable_and_terminal_returns_inline() {
    let source = "int fact(int n) {\n  if (n < 2) { return 1; }\n  return n * fact(n - 1);\n}\nint sign(int n) {\n  if (n < 0) { return -1; }\n  return 1;\n}\nfor (int i = 3; i < 5; i++) {\n  print(fact(i));\n  print(sign(i - 4));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "fact"), "{receipt:?}");
        assert!(!instantiated(program, "sign"), "{receipt:?}");
    });
}

#[test]
fn forwarded_locals_allow_an_expression_inside_another_calls_arguments() {
    let source = "int twice(int value) {\n  int doubled = value * 2;\n  return doubled + 1;\n}\nfor (int i = 0; i < 3; i++) { print(twice(i)); }\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "twice"), "{receipt:?}");
    });
}

#[test]
fn forwarded_locals_preserve_an_expression_that_is_still_waiting() {
    // Forwarding makes stepped an expression. tick still evaluates first.
    let source = "int count = 0;\nint tick() {\n  count = count + 1;\n  return count;\n}\nint stepped(int value) {\n  int next = value + 1;\n  return next;\n}\nfor (int i = 0; i < 2; i++) { print(tick() + stepped(i)); }\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "stepped"), "{receipt:?}");
    });
}

#[test]
fn a_typed_caller_never_triggers_a_default() {
    let source = "int scale(int value, int factor = 3) { return value * factor; }\nfor (int i = 0; i < 2; i++) {\n  print(scale(i));\n  print(scale(i, 5));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert_eq!(
            count(program, |kind| matches!(
                kind,
                OperationKind::IsUndefined { .. }
            )),
            0,
            "{receipt:?}"
        );
        assert!(instantiated(program, "scale"), "{receipt:?}");
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
            count(program, |kind| matches!(
                kind,
                OperationKind::IsUndefined { .. }
            )),
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
fn repeated_parameter_loads_share_one_evaluated_argument() {
    // The shared SSA operand evaluates a[0] once, including after inlining.
    let source = "int mix(int value) { return value ^ value << value; }\nint[] values = [1, 2, 3];\nprint(mix(values[0]));\nprint(mix(values[1]));\nprint(mix(values[2]));\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "mix"), "{receipt:?}");
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
fn a_result_leaves_a_scoped_copy_through_private_storage() {
    // `items` owns storage, so its copy keeps a scope and carries the result
    // across its exit; `doubled` owns nothing and needs no scope.
    let source = "int count(int value) {\n  int[] items = [value, value];\n  return items.length;\n}\nint twice(int value) {\n  int doubled = value * 2;\n  return doubled + 1;\n}\nfor (int i = 0; i < 3; i++) {\n  int result = count(i) + twice(i);\n  print(result);\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "count"), "{receipt:?}");
    });
    let source = "int twice(int value) {\n  int doubled = value * 2;\n  return doubled + 1;\n}\nfor (int i = 0; i < 3; i++) {\n  int result = twice(i);\n  print(result);\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(!instantiated(program, "twice"), "{receipt:?}");
    });
}

#[test]
fn typed_arithmetic_duplication_is_a_representation_choice() {
    // `(left + right) + extra` adds before reading `extra`: typed ints run
    // no user code, but its byte cost still belongs to objective judging.
    let source = "int add(int left, int right = 1, int extra = 0) {\n  return left + right + extra;\n}\nfor (int i = 0; i < 2; i++) {\n  print(add(i));\n  print(add(i, 2));\n  print(add(i, 2, 3));\n}\n";
    optimized(source, MODULE, |program, receipt| {
        assert!(instantiated(program, "add"), "{receipt:?}");
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
    optimized(
        source,
        RuleRequest {
            inline: false,
            ..MODULE
        },
        |program, receipt| {
            assert_eq!(signature(program, "left"), (2, false), "{receipt:?}");
            assert_eq!(signature(program, "right"), (2, false), "{receipt:?}");
            assert_eq!(signature(program, "spare"), (1, false), "{receipt:?}");
        },
    );
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
        assert_eq!(
            receipt.inlined_calls + receipt.retired_functions,
            1,
            "{receipt:?}"
        );
        assert_eq!(
            count(program, |kind| matches!(kind, OperationKind::Return)),
            0
        );
    });
}

/// A branch folded into an exit inside a loop's body leaves the rest of the
/// body unreachable (the source's own dead statements never reach the
/// program: conversion drops them).
#[test]
fn operations_after_an_exit_go_and_initializations_stay_whole() {
    let source = "int mode = 1;
int first(int n) {
  int total = 0;
  for (int i = 0; i < n; i += 1) {
    total += i;
    if (mode == 1) { return total; }
    total += 100;
    print(total);
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

#[test]
fn s4_known_nonnull_folds_keep_the_refined_result_type() {
    // The 199 literal exposed this failure in the much larger records-128
    // corpus; helper count was incidental. Exported inputs remain unknown.
    let source = r#"
        export int compute(int value){
            Record<int> item=record{count:value,offset:199,mask:713282};
            int saved=item.count??0;item.count=saved+(item.offset??0);
            return (item.count??0)^((item.mask??0)+331773)^saved;
        }
        print(compute(-11));print(compute(0));print(compute(31));
    "#;
    for scalar in [false, true] {
        optimized(
            source,
            RuleRequest {
                scalar,
                ..FOLD_ONLY
            },
            |_, receipt| {
                assert!(receipt.folded_branches > 0, "{receipt:?}");
            },
        );
    }
}

#[test]
fn s4_short_circuit_typed_views_preserve_lazy_effects_and_widening() {
    let source = r#"
        int fallback(){print(99);return 9;}
        export int choose(int n){int? present=199;return n+(present??fallback());}
        export JsValue widen(){JsValue missing=null;return missing??123;}
        print(choose(7));print(widen());
    "#;
    optimized(source, FOLD_ONLY, |_, receipt| {
        assert!(receipt.folded_branches > 0, "{receipt:?}");
    });
}

#[test]
fn s4_defaults_belong_to_the_selected_callee_and_keep_fresh_values() {
    let cases=[
        ("auto make=(int seed)=>(int value=seed)=>value;auto f=make(7);print(f());print(f(0));print(((int a,int b=a)=>b)(8));", "7\n0\n8\n"),
        ("int apply(func(int)->int f=(int x)=>x,int bias=1){return f(bias);}print(apply());", "1\n"),
        ("int first(int x=3){return x;}int second(int x=9){return x;}auto alias=first;print(alias());alias=second;print(alias());print(alias(0));", "3\n9\n0\n"),
        ("auto a=(int x=3)=>x;auto b=(int x=7)=>x;auto f=a;print(f());f=b;print(f());print(f(0));", "3\n7\n0\n"),
        ("int take(int[] xs=[4]){int x=xs[0];xs[0]=99;return x;}print(take());print(take());", "4\n4\n"),
        ("int seed=3;int take(int x=seed){return x;}print(take());seed=9;print(take());", "3\n9\n"),
    ];
    for (source, expected) in cases {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &checked).unwrap();
        assert_eq!(
            crate::interpreter::interpret_program(&syntax, &checked).unwrap(),
            expected,
            "{source}"
        );
        assert_eq!(run(&program), expected, "{source}");
        let (program, _) = optimize(program, MODULE).unwrap();
        program.verify().unwrap();
        assert_eq!(run(&program), expected, "{source}");
    }
}

#[test]
fn s4_checked_reads_and_value_updates_preserve_snapshot_and_evaluation_order() {
    for (source, expected) in [
        ("int[] xs=[4,8];int calls=0;int[] array(){calls+=1;return xs;}int key(int n){calls+=10;return n;}print(array().get(key(-1))??99);print(array().get(key(1))??99);print(array().get(key(2))??99);print(calls);int?[] ys=[null,7];print(ys.get(0)??3);print(ys.get(1)??3);", "99\n8\n99\n33\n3\n7\n"),
        ("struct Point{int x;int y;}Point p=Point{1,2};int change(){p.y=99;return 7;}Point q=p with {x:change()};q.x=8;print(p.x);print(p.y);print(q.x);print(q.y);Point r=q with {y:6,x:5};print(r.x);print(r.y);print(q.y);", "1\n99\n8\n2\n5\n6\n2\n"),
        (r#"class Box<T>{T value;init(T x){this.value=x;}U second<U>(U x){return x;}T first<U>(U x){return this.value;}}Box<int> b=new Box<int>(7);print(b.second(3));print(b.second("word"));print(b.first(false));"#, "3\nword\n7\n"),
    ] {
        let arena=bumpalo::Bump::new();
        let syntax=crate::parse_source(&arena,source).unwrap();
        let checked=crate::analyze(&syntax).unwrap();
        assert_eq!(crate::interpreter::interpret_program(&syntax,&checked).unwrap(),expected);
        let program=from_checked_source(&syntax,&checked).unwrap();
        assert_eq!(run(&program),expected);
        for scalar in [false,true] {
            let (optimized,_)=optimize(program.clone(),RuleRequest{scalar,..MODULE}).unwrap();
            optimized.verify().unwrap();assert_eq!(run(&optimized),expected,"{source}");
        }
    }
}

#[test]
fn s4_constructor_fields_run_before_declaration_defaults() {
    for (source, expected) in [
        ("int order=0;class Default{init(){order=order*10+2;}}int field(){order=order*10+1;return 0;}class C{int x=field();init(Default d=new Default()){order=order*10+3;}}C value=new C();print(order);", "123\n"),
        ("int order=0;class Default{init(){order=order*10+2;}}int field(){order=order*10+1;return 0;}export constructor C;class C{int x=field();init(Default d=new Default()){order=order*10+3;}}C value=new C();print(order);", "123\n"),
    ] {
        let arena=bumpalo::Bump::new();let syntax=crate::parse_source(&arena,source).unwrap();
        let checked=crate::analyze(&syntax).unwrap();
        assert_eq!(crate::interpreter::interpret_program(&syntax,&checked).unwrap(),expected);
        let program=from_checked_source(&syntax,&checked).unwrap();
        assert_eq!(run(&program),expected);
        let (program,_)=optimize(program,MODULE).unwrap();assert_eq!(run(&program),expected);
    }
}

#[test]
fn s4_variadic_values_keep_typed_arrays_and_call_contracts() {
    for source in [
        "int sum(int first,int... rest){int result=first;for(int i=0;i<rest.length;i++){result+=rest[i];}return result;}func(int,int...)->int f=sum;print(f(2));print(f(2,3,4));int[] xs=[5,6];print(f(2,...xs));",
        "auto sum=(int first,int... rest)=>{int result=first;for(int i=0;i<rest.length;i++){result+=rest[i];}return result;};func(int,int...)->int f=sum;print(f(2));print(f(2,3,4));int[] xs=[5,6];print(f(2,...xs));",
        "int sum<T>(T tag,int... rest){int result=2;for(int i=0;i<rest.length;i++){result+=rest[i];}return result;}print(sum(false));print(sum(false,3,4));int[] xs=[5,6];print(sum(false,...xs));",
    ] {
        let arena=bumpalo::Bump::new();let syntax=crate::parse_source(&arena,source).unwrap();
        let checked=crate::analyze(&syntax).unwrap();let program=from_checked_source(&syntax,&checked).unwrap();
        assert_eq!(crate::interpreter::interpret_program(&syntax,&checked).unwrap(),"2\n9\n13\n");
        assert_eq!(run(&program),"2\n9\n13\n");
        let (program,_)=optimize(program,MODULE).unwrap();program.verify().unwrap();assert_eq!(run(&program),"2\n9\n13\n");
    }
}

#[test]
fn s4_spread_consumption_precedes_later_mutations() {
    let source="int[] xs=[1,2];int mutate(){xs[0]=9;return 3;}int sum(int... values){return values[0]*100+values[1]*10+values[2];}print(sum(...xs,mutate()));xs[0]=1;int[] ys=[...xs,mutate()];print(ys[0]);print(xs[0]);";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    assert_eq!(
        crate::interpreter::interpret_program(&syntax, &checked).unwrap(),
        "123\n1\n9\n"
    );
    assert_eq!(run(&program), "123\n1\n9\n");
    let (program, _) = optimize(program, MODULE).unwrap();
    assert_eq!(run(&program), "123\n1\n9\n");
}

#[test]
fn s4_frozen_records_128_matches_the_independent_calibration_oracle() {
    use sha2::{Digest, Sha256};
    let frozen = include_str!("../../../benchmarks/calibration/corpus/records-128.lil");
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../benchmarks/calibration/corpus/manifest.json"
    ))
    .unwrap();
    let row = manifest["workloads"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "records-128")
        .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(frozen.as_bytes())),
        row["source_sha256"].as_str().unwrap()
    );
    let mut source = frozen.to_owned();
    for input in row["inputs"].as_array().unwrap() {
        let input = if input.as_i64() == Some(i32::MIN as i64) {
            "(-2147483647-1)".to_owned()
        } else {
            input.to_string()
        };
        source.push_str(&format!("print(probe({input}));"));
    }
    let expected = row["expected"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| format!("{value}\n"))
        .collect::<String>();
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, &source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let (program, _) = optimize(
        program,
        RuleRequest {
            scalar: true,
            ..MODULE
        },
    )
    .unwrap();
    program.verify().unwrap();
    assert_eq!(run(&program), expected);
}

#[test]
fn s4_receiver_functions_are_typed_and_accept_rest_without_adapter_limits() {
    for (source,expected) in [
        ("auto add=(this int self,int first,int... rest)=>{int total=self+first;for(int i=0;i<rest.length;i++){total+=rest[i];}return total;};func(this:int,int,int...)->int f=add;print(f.call(7,2));int[] xs=[3,4];print(f.call(7,2,...xs));", "9\n16\n"),
        ("class Box{int value;func(this:Box,int...)->int f;init(int x){this.value=x;this.f=(this Box self,int... values)=>{int sum=self.value;for(int i=0;i<values.length;i++){sum+=values[i];}return sum;};}}Box b=new Box(7);print(b.f(2,3));func(this:Box,int...)->int detached=b.f;print(detached.call(b,4));", "12\n11\n"),
        ("int captured=3;auto add=(this int self,int a,int b,int c,int d,int e,int f,int g,int h,int i,int j,int k)=>self+a+k+captured;print(add.call(7,1,2,3,4,5,6,7,8,9,10,11));", "22\n"),
    ] {
        let arena=bumpalo::Bump::new();let syntax=crate::parse_source(&arena,source).unwrap();
        let checked=crate::analyze(&syntax).unwrap();
        assert_eq!(crate::interpreter::interpret_program(&syntax,&checked).unwrap(),expected);
        let program=from_checked_source(&syntax,&checked).unwrap();assert_eq!(run(&program),expected);
        let (program,_)=optimize(program,MODULE).unwrap();program.verify().unwrap();assert_eq!(run(&program),expected);
    }
}

#[test]
fn s4_defaults_and_constructor_rest_keep_omission_and_fresh_arrays() {
    let source="int sum(int first=7,int... rest){int n=first;for(int i=0;i<rest.length;i++){n+=rest[i];}return n;}print(sum());print(sum(2,3,4));class Box{int value;init(int first=7,int... rest){this.value=first;for(int i=0;i<rest.length;i++){this.value+=rest[i];}}}Box a=new Box();int[] xs=[3,4];Box b=new Box(2,...xs);print(a.value);print(b.value);auto f=(int first=7,int... rest)=>first+rest.length;print(f());print(f(2,3,4));";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let expected = "7\n9\n7\n9\n7\n4\n";
    assert_eq!(
        crate::interpreter::interpret_program(&syntax, &checked).unwrap(),
        expected
    );
    let program = from_checked_source(&syntax, &checked).unwrap();
    assert_eq!(run(&program), expected);
    let (program, _) = optimize(program, MODULE).unwrap();
    program.verify().unwrap();
    assert_eq!(run(&program), expected);
}

#[test]
fn s4_default_expressions_execute_in_the_callee_and_capture_earlier_parameters() {
    let source="int calls=0;int next(){calls+=1;return calls;}int read(int first=next(),func()->int get=()=>first){return get();}print(read());print(read(9));print(calls);auto make=(int seed)=>{auto f=(int n=seed+next(),func()->int get=()=>n)=>get();return f;};auto f=make(10);print(f());print(f());print(calls);";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let expected = "1\n9\n1\n12\n13\n3\n";
    assert_eq!(
        crate::interpreter::interpret_program(&syntax, &checked).unwrap(),
        expected
    );
    let program = from_checked_source(&syntax, &checked).unwrap();
    assert_eq!(run(&program), expected);
    let (program, _) = optimize(program, MODULE).unwrap();
    program.verify().unwrap();
    assert_eq!(run(&program), expected);
}
