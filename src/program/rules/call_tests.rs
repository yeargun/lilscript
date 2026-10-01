//! S3 batches compare explicit semantic observations, with the tactic disabled
//! as a second lane where applicable. These are not whole-library reruns.
use super::aggregate_tests::checked;
use super::*;

const CALLS: RuleRequest = RuleRequest {
    fold: false,
    dead_code: true,
    inline: true,
    scalar: false,
    native: false,
    pristine_builtins: false,
    seal: Seal::Module,
};

#[test]
fn s3_terminal_branches_preserve_lazy_effects_and_return_types() {
    let source = "int mark(int n){print(n);return n;}int choose(bool c,int n){if(c){return mark(n+1);}else{return mark(n+2);}}int a=choose(true,4);int b=choose(false,7);print(a);print(b);";
    checked(source, "5\n9\n5\n9\n", CALLS, |_, r| {
        assert!(r.normalized_returns > 0, "{r:?}")
    });
    checked(
        source,
        "5\n9\n5\n9\n",
        RuleRequest {
            inline: false,
            ..CALLS
        },
        |_, r| assert_eq!(r.normalized_returns, 0),
    );
}

#[test]
fn s3_tail_guard_return_keeps_the_untaken_suffix_lazy() {
    checked("int next(int n){print(n);return n;}int select(bool c){if(c){return next(1);}return next(2);}int a=select(true);int b=select(false);print(a);print(b);", "1\n2\n1\n2\n", CALLS,
        |_, r| assert!(r.normalized_returns > 0, "{r:?}"));
}

#[test]
fn s3_return_normalization_does_not_keep_a_range_after_mutation() {
    checked("int step(int n){if(n>0){n=-2147483647-1;return n-1;}return 0;}int a=step(1);int b=step(-1);print(a);print(b);", "2147483647\n0\n", CALLS,
        |_, r| assert!(r.normalized_returns > 0, "{r:?}"));
}

#[test]
fn s3_scoped_result_survives_value_product_parameter_mutation() {
    checked("struct P{int x;int y;}int update(P p){p.x=7;return p.x+p.y;}P p=P{1,2};int result=update(p);print(result);print(p.x);", "9\n1\n", CALLS,
        |_, r| assert!(r.inlined_calls > 0, "{r:?}"));
}

#[test]
fn s3_field_written_reference_parameter_does_not_copy_its_handle() {
    checked(
        "class P{int x=1;}void update(P p){p.x=7;}P p=new P();update(p);print(p.x);",
        "7\n",
        CALLS,
        |p, r| {
            assert!(r.inlined_calls > 0, "{r:?}");
            assert!(
                !p.cells.iter().any(|c| c.synthetic && c.name == "p"),
                "reference parameter unnecessarily cloned"
            );
        },
    );
}

const CAPTURES: &str = "func()->int make(int n){return ()=>{n+=1;return n;};}func()->int first=()=>0;func()->int last=()=>0;for(int i=0;i<2;i+=1){auto next=make(i*10);if(i==0){first=next;}else{last=next;}}print(first());print(last());print(first());";

#[test]
fn s3_returned_closures_keep_distinct_loop_activations() {
    checked(CAPTURES, "1\n11\n2\n", CALLS, |_, r| {
        assert!(r.cloned_closure_units > 0, "{r:?}")
    });
    checked(
        CAPTURES,
        "1\n11\n2\n",
        RuleRequest {
            inline: false,
            ..CALLS
        },
        |_, r| assert_eq!(r.cloned_closure_units, 0),
    );
}

#[test]
fn s3_nested_closure_subtree_remaps_every_capture() {
    checked("func()->int make(int n){auto next=()=>{auto leaf=()=>n;return leaf();};return next;}auto read=make(7);print(read());", "7\n", CALLS,
        |_, r| assert!(r.cloned_closure_units > 0, "{r:?}"));
}

#[test]
fn s3_inlined_argument_mutation_keeps_earlier_snapshot() {
    checked("int state=1;int change(){state=9;return 2;}int combine(int a,int b){int[] hold=[a,b];return hold[0]+hold[1];}int result=combine(state,change());print(result);print(state);", "3\n9\n", CALLS,
        |_, r| assert!(r.inlined_calls > 0, "{r:?}"));
}

#[test]
fn s3_cross_module_inlining_keeps_global_capture_and_initialization_order() {
    super::super::module_helper_tests::modules(
        &[
            "import {add} from \"./math\";int answer=add(5);print(answer);",
            "int base=7;export int add(int n){return base+n;}",
        ],
        &[&[1], &[]],
        &[1, 0],
        |program| {
            let (program, receipt) = optimize(program.clone(), CALLS).unwrap();
            program.verify().unwrap();
            assert!(receipt.inlined_calls > 0, "{receipt:?}");
            let text = super::super::javascript::lower(&program)
                .unwrap()
                .render(crate::js::PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap();
            let result = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &text])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{text}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(String::from_utf8(result.stdout).unwrap(), "12\n", "{text}");
        },
    );
}

#[test]
fn s3_cross_module_cycle_keeps_captured_tdz_observable() {
    super::super::module_helper_tests::modules(
        &[
            r#"import {read} from "./dep";export int call(){return read();}print(call());"#,
            r#"import {call} from "./entry";extern void seen(JsValue error);try{print(call());}catch(auto error){seen(error);}int base=7;export int read(){return base;}"#,
        ],
        &[&[1], &[0]],
        &[1, 0],
        |program| {
            let (program, receipt) = optimize(program.clone(), CALLS).unwrap();
            assert!(receipt.inlined_calls > 0, "{receipt:?}");
            let text = program
                .to_javascript()
                .unwrap()
                .render(crate::js::PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap();
            let code = format!("globalThis.seen=e=>console.log(e.name);{text}");
            let observed = std::process::Command::new("node")
                .args(["--input-type=module", "-e", &code])
                .output()
                .unwrap();
            assert!(
                observed.status.success(),
                "{}\n{code}",
                String::from_utf8_lossy(&observed.stderr)
            );
            assert_eq!(
                String::from_utf8(observed.stdout).unwrap(),
                "ReferenceError\n7\n"
            );
        },
    );
}

#[test]
fn s3_native_inlined_closure_owns_its_captured_bank_after_return() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, CAPTURES).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let source = from_checked_source(&syntax, &semantics).unwrap();
    let (program, receipt) = optimize(
        source,
        RuleRequest {
            native: true,
            ..CALLS
        },
    )
    .unwrap();
    assert!(receipt.cloned_closure_units > 0, "{receipt:?}");
    program.verify().unwrap();
    let mut compiler = super::super::native_tests::compilation(100_000_000, 256_000_000);
    let source = compiler
        .adopt_checked(program, crate::compilation_policy::WorkDomain::Baseline)
        .unwrap();
    let code = compiler
        .with_native_c(
            source,
            &super::super::native_tests::native_policy(),
            crate::compilation_policy::WorkDomain::Baseline,
            |output| output.take_c(),
        )
        .unwrap();
    super::super::native_tests::compile_and_execute(&code, "1\n11\n2\n", "s3-captured-banks");
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
