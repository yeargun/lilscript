//! One semantic body can have distinct callable producers. Incoming argument
//! proofs belong to the selected callable's execution context, not the body ID.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};
use std::process::Command;

#[test]
fn inline_argument_facts_do_not_leak_into_another_callable_with_the_same_body() {
    check_shared_body(
        r#"
        extern int opaque();
        auto safe=(int value)=>{int discarded=value+1;return 7;};
        auto other=(int value)=>{int discarded=value+1;return 7;};
        print(safe(3));print(other(opaque()));
        "#,
        &[&["safe"]],
        "[7,\"coerce\",\"opaque\"]",
    );
}

#[test]
fn inline_return_facts_do_not_qualify_an_unselected_call_of_the_same_body() {
    // Unlike the constant-return fixture, this can only justify the selected
    // call's result domain. The ordinary call returns the host object intact;
    // its caller's discarded arithmetic must still coerce that object.
    check_shared_body(
        r#"
        extern int opaque();
        auto safe=(int value)=>value;
        auto other=(int value)=>value;
        print(safe(3));int discarded=other(opaque())+1;print(99);
        "#,
        &[&["safe"]],
        "[3,\"coerce\",\"opaque\"]",
    );
}

#[test]
fn two_inline_producers_of_one_body_keep_distinct_argument_facts() {
    // Both physical contexts are inline. Selecting the opaque producer is
    // legal in Module execution, but its coercion remains observable. Publish
    // in both orders so proof reuse cannot substitute the first body's facts.
    check_shared_body(
        r#"
        extern int opaque();
        auto safe=(int value)=>{int discarded=value+1;return 7;};
        auto other=(int value)=>{int discarded=value+1;return 7;};
        print(safe(3));print(other(opaque()));
        "#,
        &[&["safe", "other"], &["other", "safe"]],
        "[7,\"coerce\",\"opaque\"]",
    );
}

fn check_shared_body(text: &str, selections: &[&[&str]], expected: &str) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, text).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let root = program.initialization[0];
    let closures: Vec<_> = program
        .unit(root)
        .unwrap()
        .operations
        .iter()
        .enumerate()
        .filter_map(|(index, op)| {
            if let OperationKind::Closure(body) = op.kind {
                Some((OpId::from_index(index).unwrap(), body))
            } else {
                None
            }
        })
        .collect();
    assert_eq!(closures.len(), 2);
    assert_ne!(closures[0].1, closures[1].1);
    let cells: Vec<_> = ["safe", "other"]
        .into_iter()
        .map(|name| {
            let cell = program
                .cells
                .iter()
                .position(|cell| cell.name == name)
                .unwrap();
            (name, CellId::from_index(cell).unwrap())
        })
        .collect();
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let mut compilation = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 20_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 8 },
    )
    .unwrap();
    compilation
        .enable_local_facts(
            CacheLimits {
                entries: 8,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let original = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let revision = compilation
        .view(source)
        .unwrap()
        .unit_revision(root)
        .unwrap();
    let shared_body = OperationKind::Closure(closures[0].1);
    let edited = compilation
        .edit_source(
            source,
            &[UnitPatch {
                unit: root,
                expected_revision: revision,
                operations: &[OperationPatch {
                    operation: closures[1].0,
                    kind: &shared_body,
                    operands: &[],
                }],
                places: &[],
            }],
            WorkDomain::Optional,
        )
        .unwrap();
    assert_eq!(
        compilation.view(edited).unwrap().receipt().verified_units,
        1
    );
    compilation
        .with_semantic(edited, |program, uses, _| {
            assert!(uses.valid_for(program));
            let bodies: Vec<_> = program
                .unit(root)
                .unwrap()
                .operations
                .iter()
                .filter_map(|op| {
                    if let OperationKind::Closure(body) = op.kind {
                        Some(body)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(bodies, vec![closures[0].1, closures[0].1]);
        })
        .unwrap();
    // The shared direct candidate retains meaning without trying to form two
    // ordinary functions of one body, which is outside this first target slice.
    // Every tested descendant elides at least one exact callable producer.
    let direct = compilation
        .direct_javascript(edited, &policy, WorkDomain::Optional)
        .unwrap();
    let observe = |compilation: &mut Compilation<'_>, candidate, label: &str| {
        for compact in [false, true] {
            let mut tactics = OutputTactics::from_policy(&policy);
            tactics.target_compaction = compact;
            if !compact {
                tactics.literals = LiteralOutput::Original;
                tactics.families = crate::js::OutputFamilies::NONE;
            }
            compilation.with_javascript_output_choices_in(candidate, &policy, tactics, WorkDomain::Optional, |output| {
                for style in [Style::Source, Style::Global, Style::Scoped] {
                    let artifact = output.render(&Plan::new(style)).unwrap();
                    let javascript = output.take_artifact(artifact).unwrap();
                    let script = format!(
                        "const events=[];console.log=value=>events.push(value);globalThis.opaque=()=>({{valueOf(){{events.push('coerce');throw Error('opaque')}}}});try{{{javascript}}}catch(error){{events.push(error.message)}}process.stdout.write(JSON.stringify(events));"
                    );
                    let result = Command::new("node").args(["--input-type=module", "-e", &script]).output().unwrap();
                    assert!(result.status.success(), "{label} {compact} {style:?}: {}\n{javascript}", String::from_utf8_lossy(&result.stderr));
                    assert_eq!(String::from_utf8(result.stdout).unwrap(), expected, "{label} {compact} {style:?}: {javascript}");
                }
            }).unwrap();
        }
    };
    observe(&mut compilation, original, "original direct");
    for selection in selections {
        let mut candidate = direct;
        let mut owned = Vec::new();
        for &name in *selection {
            let cell = cells.iter().find(|&&(known, _)| known == name).unwrap().1;
            let selected = compilation
                .inline_helper_javascript(
                    candidate,
                    cell,
                    HelperRequest {
                        max_work: 1_000_000,
                        scratch_bytes: 1_000_000,
                        output_bytes: 1_000_000,
                        local_facts: LocalFactsRequest {
                            work_quota: 100_000,
                            result_bytes: 100_000,
                        },
                    },
                    &policy,
                    WorkDomain::Optional,
                )
                .unwrap();
            let HelperOutcome::Published(inline) = selected.outcome else {
                panic!(
                    "expected complete {name} family for {selection:?}: {:?}",
                    selected.outcome
                )
            };
            candidate = inline;
            owned.push(inline.semantic_id());
        }
        observe(
            &mut compilation,
            candidate,
            &format!("shared body {selection:?}"),
        );
        for id in owned.into_iter().rev() {
            compilation.discard(id).unwrap();
        }
    }
    for id in [direct.semantic_id(), edited, original.semantic_id(), source] {
        compilation.discard(id).unwrap();
    }
    assert_eq!(compilation.finish().retained_bytes(), 0);
}
