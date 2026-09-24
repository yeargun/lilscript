//! An original generic body can be inlined when its T formal is unused.
//! Body-wide nominal qualification must not be repeated at every occurrence.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits,
    WorkDomain, WorkKind,
};
use crate::js::selection::{Plan, Style};
use serde_json::{json, Value as Json};
use std::process::Command;

const WORK: u64 = 100_000_000;
fn owner<'src>() -> Compilation<'src> {
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: 128_000_000,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 8 }).unwrap();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 8,
                bytes: 2_000_000,
                result_bytes: 1_000_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    compiler
}
fn policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ninlining='on'\ntarget-compaction='off'\n",
    ).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 5_000_000,
        scratch_bytes: 8_000_000,
        output_bytes: 8_000_000,
        local_facts: LocalFactsRequest {
            work_quota: 1_000_000,
            result_bytes: 1_000_000,
        },
    }
}
fn checked(source: &str, inspect: impl FnOnce(Program<'_>, CellId)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let helper = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "first")
            .unwrap(),
    )
    .unwrap();
    inspect(program, helper);
}
fn inline(
    compiler: &mut Compilation<'_>,
    direct: CandidateId,
    cell: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .inline_helper_javascript(direct, cell, helper_request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("unused-T generic helper must be eligible: {other:?}"),
    }
}
fn execute(javascript: &str, expected: Json) {
    let script = format!("const library=await import('data:text/javascript,'+encodeURIComponent({}));process.stdout.write(JSON.stringify([library.run(2),library.run(5)]));",
        serde_json::to_string(javascript).unwrap());
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node required for generic inline occurrence test");
    assert!(
        output.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Json>(&output.stdout).unwrap(),
        expected
    );
}
fn render(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
) -> Result<String, CandidateError> {
    compiler
        .with_javascript_output(candidate, policy, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .and_then(|result| result)
}
fn measured(calls: usize) -> u64 {
    let mut source=String::from("struct P{int value;}int first<T>(int x,T ignored){return x;}export int run(int seed){P p=P{seed};int total=0;");
    for index in 0..calls {
        source.push_str(&format!("total+=first(seed+{index},p);"));
    }
    source.push_str("return total;}");
    let mut prepared = 0;
    checked(&source, |program, cell| {
        let mut compiler = owner();
        let policy = policy();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let selected = inline(&mut compiler, direct, cell, &policy);
        compiler
            .with_implementations(selected, |map| {
                assert_eq!(map.helpers().len(), 1);
                assert_eq!(map.helpers().next().unwrap().calls().len(), calls);
            })
            .unwrap();
        compiler
            .with_implementation_description(selected, WorkDomain::Baseline, |_| ())
            .unwrap();
        if calls == 16 {
            compiler
                .with_implementation_description(direct, WorkDomain::Baseline, |_| ())
                .unwrap();
        }
        let retained = compiler.ledger().retained_bytes();
        let before = compiler.ledger().work_used(WorkDomain::Baseline);
        // Formation and target preparation only: source checking, helper
        // discovery, rendering, codecs and Node execution are outside this row.
        compiler
            .with_javascript_output(selected, &policy, |_| ())
            .unwrap();
        prepared = compiler.ledger().work_used(WorkDomain::Baseline) - before;
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        let triangular = calls * (calls - 1) / 2;
        let expected = json!([2 * calls + triangular, 5 * calls + triangular]);
        execute(
            &render(&mut compiler, selected, &policy).unwrap(),
            expected.clone(),
        );
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        if calls == 16 {
            execute(&render(&mut compiler, direct, &policy).unwrap(), expected);
            // Exhaust the real Optional domain after publication. A failed
            // preparation cannot corrupt the candidate or its Baseline retry.
            compiler
                .with_semantic(source, |_, _, ledger| {
                    let remaining = WORK - ledger.work_used(WorkDomain::Optional);
                    ledger
                        .charge(WorkDomain::Optional, WorkKind::Analysis, remaining)
                        .unwrap();
                })
                .unwrap();
            let mut entered = false;
            let result =
                compiler.with_javascript_output_in(selected, &policy, WorkDomain::Optional, |_| {
                    entered = true;
                });
            assert!(matches!(
                result,
                Err(CandidateError::Budget(BudgetError::WorkExhausted(
                    WorkDomain::Optional
                )))
            ));
            assert!(!entered);
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            compiler
                .with_javascript_output(selected, &policy, |_| ())
                .unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), retained);
        }
        println!(
            "{}",
            json!({"kind":"generic-inline-preparation","calls":calls,"prepared_work":prepared,"helper_calls":calls,"executed":true})
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    prepared
}

#[test]
fn generic_inline_occurrences_do_not_repeat_body_wide_qualification() {
    let work = [measured(16), measured(64), measured(256)];
    let first = work[1]
        .checked_sub(work[0])
        .expect("larger original call population adds work");
    let second = work[2]
        .checked_sub(work[1])
        .expect("larger original call population adds work");
    // Fourfold population steps give ~4–6× increments for N and N log N,
    // versus 16× for repeated all-call proofs. The generous 8× ceiling tests
    // the growth class, not an implementation-mirroring exact tariff.
    assert!(
        first > 0 && second < first.checked_mul(8).unwrap(),
        "unexpected repeated-body growth: {work:?}"
    );
}

#[test]
fn unused_generic_constant_return_has_explicit_nominal_scope_refusal() {
    for nominal in [false, true] {
        let argument = if nominal {
            "P p=P{seed};return first(p);"
        } else {
            "return first(seed);"
        };
        let source=format!("struct P{{int value;}}int first<T>(T ignored){{return 3;}}export int run(int seed){{{argument}}}");
        checked(&source, |program, cell| {
            let mut compiler = owner();
            let policy = policy();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let selected = inline(&mut compiler, direct, cell, &policy);
            if !nominal {
                for candidate in [direct, selected] {
                    compiler
                        .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
                        .unwrap();
                }
            }
            let retained = compiler.ledger().retained_bytes();
            for candidate in [direct, selected] {
                let result = render(&mut compiler, candidate, &policy);
                if nominal {
                    match result {
                        Err(CandidateError::Unsupported(error)) => assert!(
                            error.feature.contains("closed value forwarding"),
                            "{error:?}"
                        ),
                        other => {
                            panic!("constant origin is outside this nominal proof slice: {other:?}")
                        }
                    }
                } else {
                    execute(&result.unwrap(), json!([3, 3]));
                }
                assert_eq!(compiler.ledger().retained_bytes(), retained);
            }
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
