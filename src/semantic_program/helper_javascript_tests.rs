//! Execute complete helper choices through the common compilation owner.
//! The pinned fixtures specify observations and eligibility independently of
//! the recognizer. An unsupported positive is a test failure, not a new golden.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use std::process::Command;

struct Case {
    name: &'static str,
    source: &'static str,
    host: &'static str,
    expected: &'static str,
    helper: &'static str,
}

macro_rules! case {
    ($name:literal, $helper:literal) => {
        Case {
            name: $name,
            source: include_str!(concat!("fixtures/helper/", $name, ".lil")),
            host: include_str!(concat!("fixtures/helper/", $name, ".setup.js")),
            expected: include_str!(concat!("fixtures/helper/", $name, ".expected.out")),
            helper: $helper,
        }
    };
}

fn policy() -> ResolvedPolicy {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 8 },
    )
    .unwrap()
}

fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
        local_facts: LocalFactsRequest {
            work_quota: 100_000,
            result_bytes: 100_000,
        },
    }
}

fn scalar_request() -> ScalarRequest {
    ScalarRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
    }
}

fn named_cell(program: &Program<'_>, name: &str) -> CellId {
    let mut found = program
        .cells
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            (cell.name == name).then(|| CellId::from_index(index).unwrap())
        });
    let cell = found.next().unwrap_or_else(|| panic!("missing {name}"));
    assert!(found.next().is_none(), "ambiguous fixture cell {name}");
    cell
}

fn execute(case: &Case, representation: &str, style: Style, javascript: &str) {
    let script = format!("{}\n{javascript}", case.host);
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for helper representation integration tests");
    assert!(
        result.status.success(),
        "{} {representation} {style:?}: {}\n{script}",
        case.name,
        String::from_utf8_lossy(&result.stderr),
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        case.expected,
        "{} {representation} {style:?}\n{script}",
        case.name,
    );
}

fn inline(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    helper: CellId,
    policy: &ResolvedPolicy,
    case: &Case,
) -> CandidateId {
    let publication = compiler
        .inline_helper_javascript(base, helper, helper_request(), policy, WorkDomain::Optional)
        .unwrap();
    match publication.outcome {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("{}: expected admitted helper, got {other:?}", case.name),
    }
}

fn scalar(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    state: CellId,
    policy: &ResolvedPolicy,
    case: &Case,
) -> CandidateId {
    let publication = compiler
        .scalar_javascript(base, state, scalar_request(), policy, WorkDomain::Optional)
        .unwrap();
    match publication.outcome {
        ScalarOutcome::Published(candidate) => candidate,
        other => panic!("{}: expected admitted record, got {other:?}", case.name),
    }
}

fn check_case(case: Case, admitted: bool, compose_record: bool) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let helper = named_cell(&program, case.helper);
    let state = compose_record.then(|| named_cell(&program, "state"));
    let revisions: Vec<_> = program.units.iter().map(FrozenUnit::revision).collect();
    let mut compiler = compilation();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 32,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy();
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let mut candidates = vec![("direct/shared", direct)];
    if admitted {
        let inlined = inline(&mut compiler, direct, helper, &policy, &case);
        candidates.push(("direct/inline", inlined));
        if let Some(state) = state {
            let scalar_shared = scalar(&mut compiler, direct, state, &policy, &case);
            let scalar_then_inline = inline(&mut compiler, scalar_shared, helper, &policy, &case);
            let inline_then_scalar = scalar(&mut compiler, inlined, state, &policy, &case);
            candidates.extend([
                ("scalar/shared", scalar_shared),
                ("scalar/inline (record selected first)", scalar_then_inline),
                ("scalar/inline (helper selected first)", inline_then_scalar),
            ]);
        }
    } else {
        assert!(!compose_record);
        let publication = compiler
            .inline_helper_javascript(
                direct,
                helper,
                helper_request(),
                &policy,
                WorkDomain::Optional,
            )
            .unwrap();
        assert!(
            matches!(publication.outcome, HelperOutcome::Unknown(_)),
            "{}: negative family must be rejected, got {:?}",
            case.name,
            publication.outcome,
        );
    }
    for (representation, candidate) in candidates {
        let view = compiler.view(candidate.semantic_id()).unwrap();
        for (index, &revision) in revisions.iter().enumerate() {
            assert_eq!(
                view.unit_revision(UnitId::from_index(index).unwrap()),
                Some(revision),
                "{} {representation} must retain semantic units",
                case.name,
            );
        }
        compiler
            .with_javascript_output(candidate, &policy, |output| {
                for style in [Style::Source, Style::Scoped, Style::Global] {
                    let artifact = output.render(&Plan::new(style)).unwrap();
                    let javascript = output.take_artifact(artifact).unwrap();
                    execute(&case, representation, style, &javascript);
                }
            })
            .unwrap();
        compiler.discard(candidate.semantic_id()).unwrap();
    }
    compiler.discard(source).unwrap();
    // Facts, proofs, physical maps and shared source storage belong to this
    // compilation and must all be released even when families were rejected.
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn shared_and_inline_helpers_preserve_argument_schedule_and_fresh_parameters() {
    for case in [
        case!("ordered-repeated-unused", "mix"),
        case!("argument-throw-finally", "mix"),
        case!("lazy-call", "even"),
        case!("nested-calls", "twice"),
        case!("parameter-mutation", "twiceNext"),
    ] {
        check_case(case, true, false);
    }
}

#[test]
fn inline_helpers_preserve_opaque_string_parameter_and_capture_coercions() {
    for case in [
        Case {
            name: "opaque private string parameter",
            source: "extern string opaque();string helper(string value){return value+\"!\";}print(helper(opaque()));helper(opaque());try{helper(opaque());}catch{print(9);}finally{print(10);}",
            host: "let calls=0;globalThis.opaque=()=>({[Symbol.toPrimitive](hint){console.log('coerce:'+hint+':'+(++calls));if(calls==3)throw Error('conversion');return 'ok';}});",
            expected: "coerce:default:1\nok!\ncoerce:default:2\ncoerce:default:3\n9\n10\n",
            helper: "helper",
        },
        Case {
            name: "opaque captured string with discarded result",
            source: "extern string opaque();func()->void make(){string value=opaque();auto helper=()=>{value+\"!\";return;};return ()=>helper();}auto run=make();run();try{run();}catch{print(9);}finally{print(10);}",
            host: "let calls=0;globalThis.opaque=()=>({[Symbol.toPrimitive](hint){console.log('coerce:'+hint+':'+(++calls));if(calls==2)throw Error('conversion');return 'ok';}});",
            expected: "coerce:default:1\ncoerce:default:2\n9\n10\n",
            helper: "helper",
        },
        Case {
            name: "conversion reentry changes a later captured-string read",
            source: "extern string opaque();extern void keep(func()->void replace);func()->string make(){string text=opaque();keep(()=>{text=\"changed\";});auto helper=()=>{string first=text+\"!\";return first+text;};return ()=>helper();}auto run=make();print(run());print(run());",
            host: "let replace;globalThis.keep=value=>{replace=value};globalThis.opaque=()=>({[Symbol.toPrimitive](hint){console.log('coerce:'+hint);replace();return 'old';}});",
            expected: "coerce:default\nold!changed\nchanged!changed\n",
            helper: "helper",
        },
    ] {
        check_case(case, true, false);
    }
}

#[test]
fn repeated_inline_loop_call_resets_its_locals_without_capturing_caller_names() {
    check_case(
        Case {
            name: "same helper call in loop with source name collisions",
            source: "int next(int value){value+=1;int doubled=value+value;return doubled;}int run(){int value=0;int doubled=100;int total=0;while(value<3){total+=next(value);value+=1;}print(value);print(doubled);return total;}print(run());print(run());",
            host: "",
            expected: "3\n100\n12\n3\n100\n12\n",
            helper: "next",
        },
        true,
        false,
    );
}

#[test]
fn inline_helpers_preserve_retained_callers_and_completed_function_prefix() {
    for case in [
        case!("retained-callers", "add"),
        case!("forward-function-prefix", "add"),
    ] {
        check_case(case, true, false);
    }
}

#[test]
fn record_and_helper_choices_compose_in_both_orders_with_reentry_and_cleanup() {
    for case in [
        case!("record-reentry-argument", "step"),
        case!("record-unused-throw", "step"),
        case!("record-absence-keys", "step"),
    ] {
        check_case(case, true, true);
    }
}

#[test]
fn inline_record_helpers_resolve_the_original_factory_activation() {
    // This supplements the pinned fixtures without changing their source or
    // expected output. Reentry mutates the second factory while a helper from
    // the first factory retains its own captured record and call-local values.
    check_case(
        Case {
            name: "two factory record activations",
            source: "extern void keep(func()->int read,func(int)->void write);extern int argument();func()->int make(int seed){Record<int> state=record{count:seed};auto step=(int delta)=>{state.count=(state.count??0)+delta;return state.count??0;};keep(()=>state.count??0,(int next)=>{state.count=next;});return ()=>step(argument());}auto first=make(2);auto second=make(20);print(first());print(second());print(first());",
            host: "const readers=[],writers=[];function keep(r,w){readers.push(r);writers.push(w)}function argument(){console.log('seen:'+readers[0]()+','+readers[1]());writers[1](readers[1]()+10);return 1}",
            expected: "seen:2,20\n3\nseen:3,30\n41\nseen:3,41\n4\n",
            helper: "step",
        },
        true,
        true,
    );
}

#[test]
fn rejected_helpers_keep_direct_behavior_and_observable_identity() {
    for case in [
        case!("escaped-name", "twice"),
        case!("mutable-callee", "helper"),
        case!("body-host-call", "helper"),
        case!("body-throw", "helper"),
        // This is a prerequisite negative: the captured record's initializer
        // calls a callback that reads it before initialization has completed.
        case!("early-capture", "helper"),
        case!("recursive", "count"),
        case!("body-escaping-closure", "helper"),
    ] {
        check_case(case, false, false);
    }
}

#[test]
fn a_prepared_reference_call_keeps_getter_receiver_and_argument_order() {
    // The pinned reference fixture has no private callable cell. Rejecting
    // its JsValue root proves ineligibility, not a helper-family reference-use
    // classifier; executing the direct candidate independently checks that
    // the prepared getter resolves before argument reentry changes the method.
    check_case(case!("reference-preparation", "value"), false, false);
}

#[test]
fn selected_helper_removes_callable_support_and_uses_distinct_expansion_bindings() {
    use super::helper_family::{self, FamilyOutcome, FamilyRequest, PreparationOutcome};
    use super::implementations::ImplementationMap;
    use crate::compilation_policy::AnalysisAttempt;
    use crate::structured_js as js;

    let source = "int next(int value){value+=1;int doubled=value+value;return doubled;}print(next(1)+next(2));";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let helper = named_cell(&program, "next");
    let parameter_symbol = program.cells[named_cell(&program, "value").index()].source_symbol;
    let local_symbol = program.cells[named_cell(&program, "doubled").index()].source_symbol;
    let helper_symbol = program.cells[helper.index()].source_symbol;
    // Structural inspection uses raw immutable input and the same internal
    // analysis/Formation contracts. The runtime matrix above tests publication;
    // this route needs no Program clone or public escape from Output ownership.
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 100_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap();
    let uses = super::uses::UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
    let mut cache = super::facts::RetainedFactsCache::new(
        CacheLimits {
            entries: 4,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
        &mut ledger,
        WorkDomain::Baseline,
    )
    .unwrap();
    let preparation = helper_family::prepare(
        &program,
        &uses,
        helper,
        FamilyRequest {
            execution: crate::compilation_contract::JavaScriptExecution::Module,
            attempt: AnalysisAttempt {
                plan: helper_family::HELPER_FAMILY_PLAN,
                algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                work_quota: 1_000_000,
            },
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
        },
        &mut ledger,
        WorkDomain::Optional,
    )
    .unwrap();
    let mut prepared = match preparation.outcome {
        PreparationOutcome::Ready(prepared) => prepared,
        other => panic!("expected a complete helper preparation: {other:?}"),
    };
    {
        let mut session = cache.session(&mut ledger, WorkDomain::Optional, 1).unwrap();
        let facts = session
            .query(
                &program,
                prepared.root().body,
                super::facts::FactRequest {
                    attempt: AnalysisAttempt {
                        plan: super::facts::LOCAL_FACTS_PLAN,
                        algorithm_version: super::facts::LOCAL_FACTS_VERSION,
                        work_quota: 100_000,
                    },
                    result_bytes: 100_000,
                },
            )
            .unwrap();
        prepared
            .check_body(&program, facts.facts, facts.receipt)
            .unwrap();
    }
    let family = match prepared.finish(&mut ledger).unwrap() {
        FamilyOutcome::Complete(family) => family,
        other => panic!("expected proved helper: {other:?}"),
    };
    assert_eq!(family.calls().len(), 2);
    let direct = ImplementationMap::direct();
    let inlined = direct
        .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
        .unwrap();
    let policy = policy();
    let mut lower = |map| {
        super::javascript::lower_with_implementations(
            &program,
            &uses,
            map,
            policy.javascript_contract().unwrap(),
            super::demand::DemandMode::Prune,
            &mut ledger,
            WorkDomain::Optional,
        )
        .unwrap()
    };
    let shared_target = lower(&direct);
    let inline_target = lower(&inlined);
    shared_target.verify().unwrap();
    inline_target.verify().unwrap();
    let calls = |target: &js::Module| {
        target
            .expressions
            .iter()
            .filter(|expression| matches!(expression, js::Expr::Call { .. }))
            .count()
    };
    // The only source callable is the helper. A no-op recipe or an
    // IIFE replacement would leave a function and extra invocations.
    assert_eq!(shared_target.functions.len(), 1);
    assert_eq!(calls(&shared_target), 3);
    assert!(inline_target.functions.is_empty());
    assert_eq!(calls(&inline_target), 1); // The original print remains.
    assert!(!inline_target
        .expressions
        .iter()
        .any(|expression| matches!(expression, js::Expr::Function(_))));
    assert!(!inline_target
        .bindings
        .iter()
        .any(|binding| binding.source_symbol == Some(helper_symbol)));
    for symbol in [parameter_symbol, local_symbol] {
        let bindings: Vec<_> = inline_target
            .bindings
            .iter()
            .enumerate()
            .filter_map(|(index, binding)| {
                (binding.source_symbol == Some(symbol)).then_some(js::BindingId::new(index))
            })
            .collect();
        assert_eq!(bindings.len(), 2, "one cell per static expansion");
        assert_ne!(bindings[0], bindings[1]);
        for binding in bindings {
            assert_eq!(
                inline_target.bindings[binding.index()].scope,
                inline_target.regions[inline_target.root.index()].scope,
            );
            assert!(inline_target.expressions.iter().any(|expression| {
                matches!(expression, js::Expr::Assign { target, .. }
                    if matches!(inline_target.expressions[target.index()],
                        js::Expr::Binding(found) if found == binding))
            }));
        }
    }
    inlined.discard(&mut ledger).unwrap();
    direct.discard(&mut ledger).unwrap();
    cache.discard(&mut ledger).unwrap();
    uses.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}
