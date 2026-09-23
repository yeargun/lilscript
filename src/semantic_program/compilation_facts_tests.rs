//! Exercise facts through the actual compilation owner and retained snapshots.
//! These tests deliberately do not construct a second cache or ledger for a
//! candidate, and do not turn exact knowledge into a physical representation.
use super::facts::{CacheLimits, ExactValue, FactsError};
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    AnalysisCompletion, AnalysisWorkReceipt, BudgetError, BudgetLedger, BudgetPlan,
    CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain, WorkKind,
};
use std::panic::{catch_unwind, AssertUnwindSafe};

const SOURCE: &str = "string word(){return \"left\"+\"right\";}int changed(){return 7;}";
const RECORD_SOURCE: &str = "string word(){return \"left\"+\"right\";}int changed(){return 7;}func()->int make(){Record<int> state=record{x:1};return ()=>state.x??0;}auto read=make();print(read());";
const MEMORY: u64 = 200_000;

fn checked<'src, R>(source: &'src str, inspect: impl FnOnce(Program<'src>) -> R) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(from_checked_source(&syntax, &semantics).unwrap())
}

fn plan(memory: u64) -> BudgetPlan {
    BudgetPlan {
        baseline_work: 10_000_000,
        optional_work: 10_000_000,
        baseline_retained_bytes: 0,
        retained_bytes: memory,
    }
}

fn compilation<'src>(memory: u64, max_live: usize) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(ResourceLimits::default(), plan(memory)).unwrap(),
        CheckpointLimit { max_live },
    )
    .unwrap()
}

fn limits() -> CacheLimits {
    CacheLimits {
        entries: 8,
        bytes: 50_000,
        result_bytes: 10_000,
    }
}

fn request() -> LocalFactsRequest {
    LocalFactsRequest {
        work_quota: 10_000,
        result_bytes: 10_000,
    }
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

fn function(program: &Program<'_>, name: &str) -> UnitId {
    let cell = program.cells.iter().find(|cell| cell.name == name).unwrap();
    let CellBinding::Function(unit) = cell.binding else {
        panic!("expected a declared function")
    };
    unit
}

fn word(program: &Program<'_>) -> (UnitId, OpId, ValueId) {
    let unit = function(program, "word");
    let (index, operation) = program
        .unit(unit)
        .unwrap()
        .operations
        .iter()
        .enumerate()
        .find(|(_, op)| matches!(op.kind, OperationKind::Binary(BinaryOp::Add)))
        .unwrap();
    (
        unit,
        OpId::from_index(index).unwrap(),
        operation.result.unwrap(),
    )
}

fn integer(program: &Program<'_>) -> (UnitId, OpId, ValueId) {
    let unit = function(program, "changed");
    let (index, operation) = program
        .unit(unit)
        .unwrap()
        .operations
        .iter()
        .enumerate()
        .find(|(_, op)| matches!(op.kind, OperationKind::Constant(Constant::Integer(_))))
        .unwrap();
    (
        unit,
        OpId::from_index(index).unwrap(),
        operation.result.unwrap(),
    )
}

fn replace_integer(
    compiler: &mut Compilation<'_>,
    base: SemanticId,
    unit: UnitId,
    operation: OpId,
) -> Result<SemanticId, PublicationError> {
    let expected_revision = compiler.view(base)?.unit_revision(unit).unwrap();
    let kind = OperationKind::Constant(Constant::Integer(19));
    compiler.edit_source(
        base,
        &[UnitPatch {
            unit,
            expected_revision,
            operations: &[OperationPatch {
                operation,
                kind: &kind,
                operands: &[],
            }],
            places: &[],
        }],
        WorkDomain::Optional,
    )
}

#[derive(Debug)]
struct Observation {
    string: Option<String>,
    receipt: AnalysisWorkReceipt,
    hit: bool,
    charged: u64,
    computations: usize,
    executed_steps: u128,
}

fn observe(
    compiler: &mut Compilation<'_>,
    snapshot: SemanticId,
    unit: UnitId,
    value: ValueId,
    request: LocalFactsRequest,
    domain: WorkDomain,
) -> Observation {
    let memory = compiler.ledger().retained_bytes();
    let before = compiler.ledger().work_used(domain);
    let (string, receipt, hit, work) = compiler
        .with_local_facts(domain, 1, |group| {
            let (string, receipt, hit) = {
                let result = group.query(snapshot, unit, request).unwrap();
                (
                    result
                        .string(value)
                        .map(|string| string.as_unicode().unwrap().to_owned()),
                    result.receipt(),
                    result.cache_hit(),
                )
            };
            (string, receipt, hit, group.work())
        })
        .unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), memory);
    assert_eq!(work.queries, 1);
    assert_eq!(work.cache_hits, usize::from(hit));
    Observation {
        string,
        receipt,
        hit,
        charged: compiler.ledger().work_used(domain) - before,
        computations: work.computations,
        executed_steps: work.executed_fact_steps,
    }
}

#[test]
fn real_direct_and_scalar_siblings_share_facts_and_later_groups_replay_work() {
    checked(RECORD_SOURCE, |program| {
        let (unit, _, value) = word(&program);
        let state = CellId::from_index(
            program
                .cells
                .iter()
                .position(|cell| cell.name == "state")
                .unwrap(),
        )
        .unwrap();
        let mut compiler = compilation(1_000_000, 4);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let resolved = policy();
        let direct = compiler
            .direct_javascript(source, &resolved, WorkDomain::Baseline)
            .unwrap();
        let scalar = match compiler
            .scalar_javascript(
                direct,
                state,
                ScalarRequest {
                    max_work: 100_000,
                    scratch_bytes: 100_000,
                    output_bytes: 100_000,
                },
                &resolved,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            ScalarOutcome::Published(candidate) => candidate,
            other => panic!("expected a real scalar sibling: {other:?}"),
        };
        let revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
        for sibling in [direct.semantic_id(), scalar.semantic_id()] {
            assert_eq!(
                compiler.view(sibling).unwrap().unit_revision(unit),
                Some(revision)
            );
        }
        let render = |compiler: &mut Compilation<'_>| {
            compiler
                .with_javascript_output(scalar, &resolved, |output| {
                    let artifact = output.render(&crate::structured_js::selection::Plan::new(
                        crate::structured_js::selection::Style::Global,
                    ))?;
                    output.take_artifact(artifact)
                })
                .unwrap()
                .unwrap()
        };
        let emitted_before = render(&mut compiler);
        compiler
            .enable_local_facts(limits(), WorkDomain::Baseline)
            .unwrap();
        let before = compiler.ledger().work_used(WorkDomain::Optional);
        let memory = compiler.ledger().retained_bytes();
        let (receipts, pointers, work) = compiler
            .with_local_facts(WorkDomain::Optional, 3, |group| {
                let mut receipts = Vec::new();
                let mut pointers = Vec::new();
                for (index, snapshot) in [source, direct.semantic_id(), scalar.semantic_id()]
                    .into_iter()
                    .enumerate()
                {
                    let result = group.query(snapshot, unit, request()).unwrap();
                    assert_eq!(result.cache_hit(), index != 0);
                    assert_eq!(
                        result.string(value).unwrap().as_unicode(),
                        Some("leftright")
                    );
                    assert_eq!(result.facts().dependencies().unit_revision, revision);
                    pointers.push(result.facts() as *const super::facts::UnitFacts as usize);
                    receipts.push(result.receipt());
                }
                (receipts, pointers, group.work())
            })
            .unwrap();
        assert!(pointers.iter().all(|pointer| *pointer == pointers[0]));
        assert!(receipts.iter().all(|receipt| *receipt == receipts[0]));
        assert_eq!(work.queries, 3);
        assert_eq!(work.cache_hits, 2);
        assert_eq!(work.computations, 1);
        assert_eq!(work.recomputations, 0);
        assert_eq!(
            work.executed_fact_steps,
            u128::from(receipts[0].logical_work)
        );
        assert_eq!(
            compiler.ledger().work_used(WorkDomain::Optional) - before,
            receipts[0].logical_work
        );
        assert_eq!(compiler.ledger().retained_bytes(), memory);
        let again = observe(
            &mut compiler,
            scalar.semantic_id(),
            unit,
            value,
            request(),
            WorkDomain::Optional,
        );
        assert!(again.hit);
        assert_eq!(again.receipt, receipts[0]);
        assert_eq!(again.charged, receipts[0].logical_work);
        assert_eq!(again.computations, 0);
        assert_eq!(again.executed_steps, 0);
        assert_eq!(compiler.checkpoint_count(), 3);
        // Learning the exact computed string did not choose a literal recipe
        // or mutate this already published implementation.
        assert_eq!(render(&mut compiler), emitted_before);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn source_edits_reuse_unaffected_facts_without_destroying_old_snapshot_answers() {
    checked(SOURCE, |program| {
        let (word_unit, _, word_value) = word(&program);
        let (changed_unit, changed_operation, changed_value) = integer(&program);
        let mut compiler = compilation(MEMORY, 3);
        let original = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(limits(), WorkDomain::Baseline)
            .unwrap();
        compiler
            .with_local_facts(WorkDomain::Optional, 2, |group| {
                assert!(!group
                    .query(original, word_unit, request())
                    .unwrap()
                    .cache_hit());
                let result = group.query(original, changed_unit, request()).unwrap();
                assert_eq!(
                    result.facts().exact(changed_value),
                    Some(ExactValue::Integer(7))
                );
                assert!(!result.cache_hit());
            })
            .unwrap();
        let changed =
            replace_integer(&mut compiler, original, changed_unit, changed_operation).unwrap();
        assert_eq!(
            compiler.view(changed).unwrap().unit_revision(word_unit),
            compiler.view(original).unwrap().unit_revision(word_unit)
        );
        assert_ne!(
            compiler.view(changed).unwrap().unit_revision(changed_unit),
            compiler.view(original).unwrap().unit_revision(changed_unit)
        );
        let work = compiler
            .with_local_facts(WorkDomain::Optional, 3, |group| {
                {
                    let result = group.query(changed, word_unit, request()).unwrap();
                    assert!(result.cache_hit());
                    assert_eq!(
                        result.string(word_value).unwrap().as_unicode(),
                        Some("leftright")
                    );
                }
                {
                    let result = group.query(changed, changed_unit, request()).unwrap();
                    assert!(!result.cache_hit());
                    assert_eq!(
                        result.facts().exact(changed_value),
                        Some(ExactValue::Integer(19))
                    );
                }
                {
                    let result = group.query(original, changed_unit, request()).unwrap();
                    assert!(result.cache_hit());
                    assert_eq!(
                        result.facts().exact(changed_value),
                        Some(ExactValue::Integer(7))
                    );
                }
                group.work()
            })
            .unwrap();
        assert_eq!((work.cache_hits, work.computations), (2, 1));
        compiler.discard(original).unwrap();
        assert!(
            observe(
                &mut compiler,
                changed,
                word_unit,
                word_value,
                request(),
                WorkDomain::Optional
            )
            .hit
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn weak_and_strong_requests_have_the_same_answers_and_charges_when_cold_or_warm() {
    let strong = request();
    for weak in [
        LocalFactsRequest {
            work_quota: 1,
            ..request()
        },
        LocalFactsRequest {
            result_bytes: std::mem::size_of::<super::facts::UnitFacts>() as u64,
            ..request()
        },
    ] {
        let run = |requests: &[LocalFactsRequest]| {
            checked(SOURCE, |program| {
                let (unit, _, value) = word(&program);
                let mut compiler = compilation(MEMORY, 2);
                let snapshot = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                compiler
                    .enable_local_facts(limits(), WorkDomain::Baseline)
                    .unwrap();
                let rows = requests
                    .iter()
                    .map(|request| {
                        observe(
                            &mut compiler,
                            snapshot,
                            unit,
                            value,
                            *request,
                            WorkDomain::Optional,
                        )
                    })
                    .collect::<Vec<_>>();
                assert_eq!(compiler.finish().retained_bytes(), 0);
                rows
            })
        };
        let warm = run(&[weak, strong, weak, strong]);
        let cold_weak = run(&[weak]).pop().unwrap();
        let cold_strong = run(&[strong]).pop().unwrap();
        for (index, expected) in [
            (0, &cold_weak),
            (1, &cold_strong),
            (2, &cold_weak),
            (3, &cold_strong),
        ] {
            assert_eq!(warm[index].string, expected.string);
            assert_eq!(warm[index].receipt, expected.receipt);
            assert_eq!(warm[index].charged, expected.charged);
            assert_eq!(warm[index].charged, warm[index].receipt.logical_work);
            assert_eq!(warm[index].hit, index >= 2);
        }
        assert_eq!(cold_weak.receipt.completion, AnalysisCompletion::Truncated);
        assert_eq!(cold_weak.string, None);
        assert_eq!(cold_strong.receipt.completion, AnalysisCompletion::Complete);
        assert_eq!(cold_strong.string.as_deref(), Some("leftright"));
        assert!(cold_strong.executed_steps > 0);
        assert_eq!(warm[2].executed_steps, 0);
        assert_eq!(warm[3].executed_steps, 0);
    }
}

#[test]
fn retained_capacity_and_query_scratch_stay_with_their_original_domains() {
    for (capacity_domain, query_domain) in [
        (WorkDomain::Baseline, WorkDomain::Optional),
        (WorkDomain::Optional, WorkDomain::Baseline),
    ] {
        checked(SOURCE, |program| {
            let (unit, _, value) = word(&program);
            let mut compiler = compilation(MEMORY, 3);
            let snapshot = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let source_memory = compiler.ledger().retained_bytes();
            let capacity_work = compiler.ledger().work_used(capacity_domain);
            compiler
                .enable_local_facts(limits(), capacity_domain)
                .unwrap();
            assert_eq!(
                compiler.ledger().retained_bytes(),
                source_memory + limits().bytes
            );
            let cold = observe(
                &mut compiler,
                snapshot,
                unit,
                value,
                request(),
                query_domain,
            );
            assert!(!cold.hit);
            assert_eq!(compiler.ledger().work_used(capacity_domain), capacity_work);
            let occupied = compiler.ledger().retained_bytes();
            let cache = compiler.local_facts_status().unwrap();
            assert_eq!(cache.entries, 1);
            assert_eq!(cache.reserved_bytes, limits().bytes);
            assert!(cache.retained_bytes <= cache.reserved_bytes);
            let competing = MEMORY - occupied - 100;
            compiler
                .with_semantic(snapshot, |_, _, ledger| {
                    ledger.retain(query_domain, competing)
                })
                .unwrap()
                .unwrap();
            assert!(matches!(
                compiler.with_local_facts(query_domain, 1, |_| panic!("scratch admission must fail before callback")),
                Err(CompilationFactsError::Facts(FactsError::Budget(BudgetError::MemoryExhausted(domain)))) if domain == query_domain
            ));
            assert_eq!(compiler.ledger().retained_bytes(), occupied + competing);
            assert_eq!(compiler.local_facts_status(), Some(cache));
            compiler
                .with_semantic(snapshot, |_, _, ledger| {
                    ledger.release(query_domain, competing)
                })
                .unwrap()
                .unwrap();
            let warm = observe(
                &mut compiler,
                snapshot,
                unit,
                value,
                request(),
                query_domain,
            );
            assert!(warm.hit);
            assert_eq!(warm.receipt, cold.receipt);
            assert_eq!(warm.charged, cold.charged);
            compiler.discard_local_facts().unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), source_memory);
            assert_eq!(compiler.local_facts_status(), None);
            compiler.enable_local_facts(limits(), query_domain).unwrap();
            assert!(
                !observe(
                    &mut compiler,
                    snapshot,
                    unit,
                    value,
                    request(),
                    capacity_domain
                )
                .hit
            );
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn persistent_cache_capacity_competes_with_real_publication_until_explicitly_discarded() {
    checked(SOURCE, |program| {
        let (unit, operation, _) = integer(&program);
        let mut compiler = compilation(MEMORY, 3);
        let snapshot = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(limits(), WorkDomain::Optional)
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        let competing = MEMORY - before - 100;
        compiler
            .with_semantic(snapshot, |_, _, ledger| {
                ledger.retain(WorkDomain::Optional, competing)
            })
            .unwrap()
            .unwrap();
        let held = compiler.ledger().retained_bytes();
        assert!(replace_integer(&mut compiler, snapshot, unit, operation).is_err());
        assert_eq!(compiler.checkpoint_count(), 1);
        assert_eq!(compiler.ledger().retained_bytes(), held);
        compiler.discard_local_facts().unwrap();
        let edited = replace_integer(&mut compiler, snapshot, unit, operation).unwrap();
        assert_ne!(
            compiler.view(edited).unwrap().unit_revision(unit),
            compiler.view(snapshot).unwrap().unit_revision(unit)
        );
        compiler
            .with_semantic(snapshot, |_, _, ledger| {
                ledger.release(WorkDomain::Optional, competing)
            })
            .unwrap()
            .unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn callback_errors_and_unwinding_release_only_scratch_and_keep_cached_results() {
    checked(SOURCE, |program| {
        let (unit, _, value) = word(&program);
        let mut compiler = compilation(MEMORY, 2);
        let snapshot = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(limits(), WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let result: Result<(), &str> = compiler
            .with_local_facts(WorkDomain::Optional, 1, |group| {
                assert!(!group.query(snapshot, unit, request()).unwrap().cache_hit());
                Err("consumer declined this fact")
            })
            .unwrap();
        assert_eq!(result, Err("consumer declined this fact"));
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        let panic = catch_unwind(AssertUnwindSafe(|| {
            compiler
                .with_local_facts(WorkDomain::Optional, 1, |group| {
                    assert!(group.query(snapshot, unit, request()).unwrap().cache_hit());
                    panic!("consumer unwinds while owner retains cache");
                })
                .unwrap();
        }));
        assert!(panic.is_err());
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        assert!(
            observe(
                &mut compiler,
                snapshot,
                unit,
                value,
                request(),
                WorkDomain::Optional
            )
            .hit
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn owner_rejects_disabled_queries_failed_enable_and_duplicate_enable_atomically() {
    checked(SOURCE, |program| {
        let mut compiler = compilation(MEMORY, 2);
        compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let memory = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.with_local_facts(WorkDomain::Optional, 1, |_| ()),
            Err(CompilationFactsError::NotEnabled)
        ));
        assert!(matches!(
            compiler.enable_local_facts(
                CacheLimits {
                    bytes: MEMORY,
                    ..limits()
                },
                WorkDomain::Optional
            ),
            Err(CompilationFactsError::Facts(FactsError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            )))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), memory);
        compiler
            .enable_local_facts(limits(), WorkDomain::Baseline)
            .unwrap();
        assert!(matches!(
            compiler.enable_local_facts(limits(), WorkDomain::Optional),
            Err(CompilationFactsError::AlreadyEnabled)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), memory + limits().bytes);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn stale_and_foreign_checkpoint_handles_never_query_another_snapshot() {
    checked(SOURCE, |program| {
        let (unit, _, value) = word(&program);
        let mut compiler = compilation(MEMORY, 2);
        let stale = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler.discard(stale).unwrap();
        checked(SOURCE, |program| {
            let live = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            assert_ne!(stale, live);
            checked(SOURCE, |program| {
                let mut other = compilation(MEMORY, 2);
                let foreign = other.adopt_checked(program, WorkDomain::Baseline).unwrap();
                compiler
                    .enable_local_facts(limits(), WorkDomain::Baseline)
                    .unwrap();
                let work = compiler
                    .with_local_facts(WorkDomain::Optional, 4, |group| {
                        for invalid in [stale, foreign] {
                            assert!(matches!(
                                group.query(invalid, unit, request()),
                                Err(CompilationFactsError::Publication(
                                    PublicationError::UnknownCheckpoint
                                ))
                            ));
                        }
                        assert!(matches!(
                            group.query(live, UnitId::from_index(99).unwrap(), request()),
                            Err(CompilationFactsError::Facts(FactsError::UnknownUnit))
                        ));
                        {
                            let result = group.query(live, unit, request()).unwrap();
                            assert!(!result.cache_hit());
                            assert_eq!(
                                result.string(value).unwrap().as_unicode(),
                                Some("leftright")
                            );
                        }
                        group.work()
                    })
                    .unwrap();
                assert_eq!(work.computations, 1);
                assert_eq!(work.cache_hits, 0);
                assert_eq!(other.finish().retained_bytes(), 0);
            });
        });
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn native_source_queries_do_not_create_target_state_or_target_work() {
    checked(SOURCE, |program| {
        let (unit, _, value) = word(&program);
        let native = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::Native)
            .unwrap();
        assert!(native.javascript_contract().is_none());
        let mut compiler = Compilation::new(
            native.ledger(plan(MEMORY)).unwrap(),
            CheckpointLimit { max_live: 2 },
        )
        .unwrap();
        let snapshot = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let source_memory = compiler.ledger().retained_bytes();
        assert_eq!(compiler.local_facts_status(), None);
        compiler
            .enable_local_facts(limits(), WorkDomain::Baseline)
            .unwrap();
        let result = observe(
            &mut compiler,
            snapshot,
            unit,
            value,
            request(),
            WorkDomain::Baseline,
        );
        assert_eq!(result.string.as_deref(), Some("leftright"));
        assert_eq!(compiler.checkpoint_count(), 1);
        assert_eq!(compiler.ledger().work_by_kind(WorkKind::Render), 0);
        assert_eq!(compiler.ledger().work_by_kind(WorkKind::Codec), 0);
        compiler.discard_local_facts().unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), source_memory);
        // Queries did not install a default JS delivery contract: a later
        // explicit first target may choose a distinct ABI/syntax configuration.
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.ecmascript = crate::js_syntax_target::EcmaScriptEdition::Es2015;
        let javascript = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        compiler
            .direct_javascript(snapshot, &javascript, WorkDomain::Baseline)
            .unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
