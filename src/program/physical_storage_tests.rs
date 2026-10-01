//! Focused S2 qualification through published, policy-controlled target output.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};
use std::process::Command;

fn compiler<'a>() -> Compilation<'a> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 100_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap()
}
fn policy(native: bool, scalar: bool) -> crate::compilation_policy::ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[policy.tactics]\nscalar-replacement='{}'\ntarget-compaction='on'\n",
        if scalar { "on" } else { "off" }
    ))
    .unwrap();
    config
        .resolve_policy(if native {
            CompilationRequest::Native
        } else {
            CompilationRequest::JavaScript {
                preserve_root_exports: false,
            }
        })
        .unwrap()
}
fn js(source: &str, expected: &str, scalar: bool) -> String {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let mut compiler = compiler();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy(false, scalar);
    let candidate = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let text = compiler
        .with_javascript_output(candidate, &policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .unwrap()
        .unwrap();
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &text])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{text}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        expected,
        "{text}"
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
    text
}
#[test]
fn s2_owned_nested_products_mutate_across_branches_and_loops() {
    let source = "struct P{int x;int y;}struct B{P p;int z;}B value=B{P{1,2},3};for(int i=0;i<3;i+=1){if(i==1){value.p.x+=5;}else{value.p.y+=2;}}print(value.p.x);print(value.p.y);print(value.z);";
    let off = js(source, "6\n6\n3\n", false);
    let on = js(source, "6\n6\n3\n", true);
    assert!(on.len() < off.len(), "on: {on}\noff: {off}");
}
#[test]
fn s2_product_snapshots_and_nested_aliases_keep_independent_values() {
    for source in [
        "struct P{int x;int y;}P a=P{1,2};P b=a;a.x=7;print(a.x);print(b.x);",
        "struct P{int x;int y;}struct B{P p;}P original=P{1,2};B a=B{original};a.p.x=7;print(a.p.x);print(original.x);",
    ] {
        for scalar in [false, true] {
            js(source, "7\n1\n", scalar);
        }
    }
}
#[test]
fn s2_native_final_use_transfer_preserves_loops_and_live_aliases() {
    let source = "int[] make(int n){return [n,n+1];}int sum=0;for(int i=0;i<4;i+=1){int[] a=make(i);int[] b=a;a[0]+=10;sum+=b[0]+b[1];}print(sum);";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    for scalar in [false, true] {
        let program = from_checked_source(&syntax, &checked).unwrap();
        let mut compiler = compiler();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let (code, transfers) = compiler
            .with_native_c(
                source,
                &policy(true, scalar),
                WorkDomain::Baseline,
                |output| (output.take_c(), output.ownership_transfers()),
            )
            .unwrap();
        if scalar {
            assert!(transfers > 0);
        } else {
            assert_eq!(transfers, 0);
        }
        super::native_tests::compile_and_execute(
            &code,
            "56\n",
            if scalar { "s2-move" } else { "s2-copy" },
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    }
}

#[test]
fn s2_native_shared_banks_preserve_captures_and_class_aliases() {
    let source = "func()->int make(int n){int[] state=[n,0];return ()=>{state[0]+=1;return state[0];};}auto a=make(2);auto b=make(8);print(a());print(b());print(a());class Pair{int x=1;int y=2;}Pair p=new Pair();Pair alias=p;alias.x=7;print(p.x);print(p.y);";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let (program, receipt) = super::rules::optimize(
        program,
        super::rules::RuleRequest {
            unroll: true, pool: true,
            fold: false,
            dead_code: true,
            inline: false,
            scalar: true,
            native: true,
            pristine_builtins: false,
            seal: super::call_graph::Seal::Module,
        },
    )
    .unwrap();
    assert!(receipt.scalarized_allocations >= 2, "{receipt:?}");
    let mut compiler = compiler();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let code = compiler
        .with_native_c(
            source,
            &policy(true, true),
            WorkDomain::Baseline,
            |output| output.take_c(),
        )
        .unwrap();
    super::native_tests::compile_and_execute(&code, "3\n9\n4\n7\n2\n", "s2-shared-banks");
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
