//! String choices use the existing compilation/query/checkpoint owners.
use super::facts::{CacheLimits, FactsError};
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits,
    TacticId, WorkDomain,
};
use crate::js::selection::{Plan, Style};

const SOURCE: &str = "export string word(){string first=\"path/\"+\"ready\";string second=\"path/\"+\"ready\";return first+second;}int other(){return 7;}";
const WORK: u64 = 10_000_000;
const MEMORY: u64 = 10_000_000;

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(from_checked_source(&syntax, &semantics).unwrap());
}

fn policy(extra: &str) -> ResolvedPolicy {
    let text = format!("[javascript]\nstrip_console=false\n{extra}");
    let config: crate::config::ProjectConfig = toml::from_str(&text).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn enabled() -> ResolvedPolicy {
    policy("[policy.tactics]\nconstant-folding='on'\nstring-pooling='on'\n")
}

fn request() -> StringRequest {
    StringRequest {
        max_work: 100_000,
        scratch_bytes: 100_000,
        output_bytes: 100_000,
        local_facts: LocalFactsRequest {
            work_quota: 100_000,
            result_bytes: 100_000,
        },
    }
}

fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 10 },
    )
    .unwrap()
}

fn enable_facts(compiler: &mut Compilation<'_>) {
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 8,
                bytes: 1_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Baseline,
        )
        .unwrap();
}

fn function(program: &Program<'_>, name: &str) -> UnitId {
    match program
        .cells
        .iter()
        .find(|cell| cell.name == name)
        .unwrap()
        .binding
    {
        CellBinding::Function(unit) => unit,
        _ => panic!("expected function"),
    }
}

fn definitions(program: &Program<'_>, unit: UnitId, count: usize) -> Vec<ValueRef> {
    let definitions = program
        .unit(unit)
        .unwrap()
        .operations
        .iter()
        .filter(|operation| matches!(operation.kind, OperationKind::Binary(BinaryOp::Add)))
        .take(count)
        .map(|operation| ValueRef {
            unit,
            value: operation.result.unwrap(),
        })
        .collect::<Vec<_>>();
    assert_eq!(definitions.len(), count);
    definitions
}

fn published(publication: StringPublication) -> CandidateId {
    match publication.outcome {
        StringOutcome::Published(candidate) => candidate,
        other => panic!("expected complete string choice: {other:?}"),
    }
}

fn render(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
) -> String {
    compiler
        .with_javascript_output_in(candidate, policy, WorkDomain::Baseline, |output| {
            let artifact = output.render(&Plan::new(Style::Global))?;
            output.take_artifact(artifact)
        })
        .unwrap()
        .unwrap()
}

fn addresses(compiler: &mut Compilation<'_>, source: SemanticId) -> (usize, usize) {
    compiler
        .with_semantic(source, |program, uses, _| {
            (
                program as *const Program<'_> as usize,
                uses as *const super::uses::UseIndex as usize,
            )
        })
        .unwrap()
}

#[test]
fn string_choices_reuse_the_owned_query_and_semantic_snapshot() {
    checked(SOURCE, |program| {
        let body = function(&program, "word");
        let definitions = definitions(&program, body, 2);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &enabled(), WorkDomain::Baseline)
            .unwrap();
        enable_facts(&mut compiler);
        let facts_receipt = compiler
            .with_local_facts(WorkDomain::Optional, 1, |group| {
                let facts = group.query(source, body, request().local_facts).unwrap();
                assert!(!facts.cache_hit());
                assert!(facts.string(definitions[0].value).is_some());
                facts.receipt()
            })
            .unwrap();
        let semantic = addresses(&mut compiler, source);
        let mut choices = Vec::new();
        for choice in [
            StringChoice::LiteralAtDefinition,
            StringChoice::SharedLiteral { activation: body },
        ] {
            let publication = compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    choice,
                    request(),
                    &enabled(),
                    WorkDomain::Optional,
                )
                .unwrap();
            assert_eq!(publication.local_facts_receipt, Some(facts_receipt));
            let candidate = published(publication);
            assert_eq!(addresses(&mut compiler, candidate.semantic_id()), semantic);
            assert_eq!(
                compiler
                    .view(candidate.semantic_id())
                    .unwrap()
                    .unit_revision(body),
                compiler.view(source).unwrap().unit_revision(body)
            );
            choices.push(candidate);
        }
        compiler
            .with_local_facts(WorkDomain::Baseline, choices.len(), |group| {
                for candidate in choices {
                    let facts = group
                        .query(candidate.semantic_id(), body, request().local_facts)
                        .unwrap();
                    assert!(facts.cache_hit());
                    assert_eq!(facts.receipt(), facts_receipt);
                }
                assert_eq!(group.work().computations, 0);
            })
            .unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn sharing_keeps_string_payload_until_its_last_original_domain_owner() {
    for first_domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        for oldest_first in [false, true] {
            checked(SOURCE, |program| {
                let body = function(&program, "word");
                let definitions = definitions(&program, body, 2);
                let mut compiler = compilation();
                let source = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let direct = compiler
                    .direct_javascript(source, &enabled(), WorkDomain::Baseline)
                    .unwrap();
                enable_facts(&mut compiler);
                let before = compiler.ledger().retained_bytes();
                let selected = published(
                    compiler
                        .represent_string_javascript(
                            direct,
                            &definitions,
                            StringChoice::SharedLiteral { activation: body },
                            request(),
                            &enabled(),
                            first_domain,
                        )
                        .unwrap(),
                );
                let first_bytes = compiler.ledger().retained_bytes();
                let sibling_domain = if first_domain == WorkDomain::Baseline {
                    WorkDomain::Optional
                } else {
                    WorkDomain::Baseline
                };
                let sibling = compiler
                    .rebase_javascript(selected, source, &enabled(), sibling_domain)
                    .unwrap();
                let sibling_bytes = compiler
                    .view(sibling.semantic_id())
                    .unwrap()
                    .receipt()
                    .allocated_bytes;
                assert!(
                    sibling_bytes < first_bytes - before,
                    "sibling must share the already owned family payload"
                );
                assert_eq!(
                    addresses(&mut compiler, selected.semantic_id()),
                    addresses(&mut compiler, sibling.semantic_id())
                );
                let expected = render(&mut compiler, selected, &enabled());
                let (first, last) = if oldest_first {
                    (selected, sibling)
                } else {
                    (sibling, selected)
                };
                compiler.discard(first.semantic_id()).unwrap();
                assert_eq!(render(&mut compiler, last, &enabled()), expected);
                compiler.discard(last.semantic_id()).unwrap();
                assert_eq!(compiler.ledger().retained_bytes(), before);
                assert_eq!(compiler.finish().retained_bytes(), 0);
            });
        }
    }
}

#[test]
fn string_permissions_apply_at_insert_rebase_and_output_without_binding_codec() {
    checked(SOURCE, |program| {
        let body = function(&program, "word");
        let definitions = definitions(&program, body, 2);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &enabled(), WorkDomain::Baseline)
            .unwrap();
        enable_facts(&mut compiler);
        let no_fold = policy("[policy.tactics]\nconstant-folding='off'\nstring-pooling='on'\n");
        let no_pool = policy("[policy.tactics]\nconstant-folding='on'\nstring-pooling='off'\n");
        for (choice, forbidden, tactic) in [
            (
                StringChoice::LiteralAtDefinition,
                &no_fold,
                TacticId::ConstantFolding,
            ),
            (
                StringChoice::SharedLiteral { activation: body },
                &no_pool,
                TacticId::StringPooling,
            ),
        ] {
            let bytes = compiler.ledger().retained_bytes();
            let work = compiler.ledger().work_used(WorkDomain::Optional);
            assert!(
                matches!(compiler.represent_string_javascript(direct, &definitions, choice, request(), forbidden, WorkDomain::Optional), Err(CandidateError::ForbiddenTactic(found)) if found == tactic)
            );
            assert_eq!(compiler.ledger().retained_bytes(), bytes);
            assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), work);
            let selected = published(
                compiler
                    .represent_string_javascript(
                        direct,
                        &definitions,
                        choice,
                        request(),
                        &enabled(),
                        WorkDomain::Optional,
                    )
                    .unwrap(),
            );
            assert!(
                matches!(compiler.rebase_javascript(selected, source, forbidden, WorkDomain::Optional), Err(CandidateError::ForbiddenTactic(found)) if found == tactic)
            );
            let mut callback = false;
            assert!(
                matches!(compiler.with_javascript_output(selected, forbidden, |_| callback = true), Err(CandidateError::ForbiddenTactic(found)) if found == tactic)
            );
            assert!(!callback);
        }
        let literal = published(
            compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    StringChoice::LiteralAtDefinition,
                    request(),
                    &no_pool,
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        for codec in ["raw", "gzip", "brotli"] {
            let compatible = policy(&format!("cost_model='{codec}'\n[policy.tactics]\nconstant-folding='on'\nstring-pooling='on'\n"));
            assert!(!render(&mut compiler, literal, &compatible).is_empty());
        }
        let wrong_target = policy(
            "ecmascript='es2018'\n[policy.tactics]\nconstant-folding='on'\nstring-pooling='on'\n",
        );
        assert!(matches!(
            compiler.represent_string_javascript(
                direct,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request(),
                &wrong_target,
                WorkDomain::Optional
            ),
            Err(CandidateError::ContractMismatch)
        ));
        assert!(matches!(
            compiler.with_javascript_output(literal, &wrong_target, |_| ()),
            Err(CandidateError::ContractMismatch)
        ));
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn rejected_string_attempts_release_reservations_and_preserve_the_base() {
    checked(SOURCE, |program| {
        let body = function(&program, "word");
        let definitions = definitions(&program, body, 2);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &enabled(), WorkDomain::Baseline)
            .unwrap();
        let bytes = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.represent_string_javascript(
                direct,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request(),
                &enabled(),
                WorkDomain::Optional
            ),
            Err(CandidateError::LocalFacts(
                CompilationFactsError::NotEnabled
            ))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        enable_facts(&mut compiler);
        let bytes = compiler.ledger().retained_bytes();
        let invalid = StringRequest {
            local_facts: LocalFactsRequest {
                result_bytes: 0,
                ..request().local_facts
            },
            ..request()
        };
        assert!(matches!(
            compiler.represent_string_javascript(
                direct,
                &definitions,
                StringChoice::LiteralAtDefinition,
                invalid,
                &enabled(),
                WorkDomain::Optional
            ),
            Err(CandidateError::LocalFacts(CompilationFactsError::Facts(
                FactsError::InvalidAttempt
            ))) | Err(CandidateError::InvalidRequest)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        for bounded in [
            StringRequest {
                max_work: 0,
                ..request()
            },
            StringRequest {
                scratch_bytes: 0,
                ..request()
            },
            StringRequest {
                output_bytes: 0,
                ..request()
            },
        ] {
            let result = compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    StringChoice::LiteralAtDefinition,
                    bounded,
                    &enabled(),
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(
                matches!(result.outcome, StringOutcome::Truncated(_)),
                "{:?}",
                result.outcome
            );
            assert_eq!(compiler.ledger().retained_bytes(), bytes);
            assert_eq!(compiler.checkpoint_count(), 2);
        }
        let selected = published(
            compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    StringChoice::LiteralAtDefinition,
                    request(),
                    &enabled(),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        let bytes = compiler.ledger().retained_bytes();
        let checkpoints = compiler.checkpoint_count();
        // An overlap is rejected even when a differently grouped recipe selects
        // just the second definition and changes its physical representation.
        assert!(matches!(
            compiler.represent_string_javascript(
                selected,
                &definitions[1..],
                StringChoice::SharedLiteral { activation: body },
                request(),
                &enabled(),
                WorkDomain::Optional
            ),
            Err(CandidateError::DuplicateRoot)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        assert_eq!(compiler.checkpoint_count(), checkpoints);
        compiler
            .with_semantic(source, |_, _, ledger| {
                ledger
                    .charge(
                        WorkDomain::Optional,
                        crate::compilation_policy::WorkKind::Edit,
                        WORK - ledger.work_used(WorkDomain::Optional),
                    )
                    .unwrap();
            })
            .unwrap();
        assert!(matches!(
            compiler.represent_string_javascript(
                direct,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request(),
                &enabled(),
                WorkDomain::Optional
            ),
            Err(CandidateError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        assert!(!render(&mut compiler, direct, &enabled()).is_empty());
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

fn replace_operation(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    unit: UnitId,
    operation: OpId,
    kind: OperationKind,
) -> SemanticId {
    let expected_revision = compiler.view(source).unwrap().unit_revision(unit).unwrap();
    compiler
        .edit_source(
            source,
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
        .unwrap()
}

#[test]
fn rebase_reuses_unaffected_string_evidence_and_rejects_edited_definitions() {
    checked(SOURCE, |program| {
        let body = function(&program, "word");
        let definitions = definitions(&program, body, 2);
        let changed_string =
            program.unit(body).unwrap().values[definitions[0].value.index()].definition;
        let literal = program
            .strings
            .iter()
            .position(|value| value == &StringValue::from("path/"))
            .unwrap();
        let other = function(&program, "other");
        let integer = OpId::from_index(
            program
                .unit(other)
                .unwrap()
                .operations
                .iter()
                .position(|operation| {
                    matches!(
                        operation.kind,
                        OperationKind::Constant(Constant::Integer(_))
                    )
                })
                .unwrap(),
        )
        .unwrap();
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &enabled(), WorkDomain::Baseline)
            .unwrap();
        enable_facts(&mut compiler);
        let selected = published(
            compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    StringChoice::SharedLiteral { activation: body },
                    request(),
                    &enabled(),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        let expected = render(&mut compiler, selected, &enabled());
        let unrelated = replace_operation(
            &mut compiler,
            source,
            other,
            integer,
            OperationKind::Constant(Constant::Integer(11)),
        );
        let rebased = compiler
            .rebase_javascript(selected, unrelated, &enabled(), WorkDomain::Optional)
            .unwrap();
        assert_eq!(
            compiler.view(unrelated).unwrap().unit_revision(body),
            compiler.view(source).unwrap().unit_revision(body)
        );
        assert_eq!(render(&mut compiler, rebased, &enabled()), expected);
        let changed = replace_operation(
            &mut compiler,
            source,
            body,
            changed_string,
            OperationKind::Constant(Constant::String(StringId::from_index(literal).unwrap())),
        );
        let bytes = compiler.ledger().retained_bytes();
        let count = compiler.checkpoint_count();
        assert!(matches!(
            compiler.rebase_javascript(selected, changed, &enabled(), WorkDomain::Optional),
            Err(CandidateError::StaleEvidence)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        assert_eq!(compiler.checkpoint_count(), count);
        assert_eq!(render(&mut compiler, selected, &enabled()), expected);
        let computation = compiler
            .direct_javascript(changed, &enabled(), WorkDomain::Optional)
            .unwrap();
        assert_ne!(render(&mut compiler, computation, &enabled()), expected);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn facts_session_and_sibling_memory_failures_release_only_their_own_reservations() {
    checked(SOURCE, |program| {
        let body = function(&program, "word");
        let definitions = definitions(&program, body, 2);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &enabled(), WorkDomain::Baseline)
            .unwrap();
        // Session scratch follows the cache's explicit result capacity, while
        // this particular string/facts attempt has smaller independent caps.
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
        let bytes = compiler.ledger().retained_bytes();
        let padding = MEMORY - bytes - 250_000;
        compiler
            .with_semantic(source, |_, _, ledger| {
                ledger.retain(WorkDomain::Optional, padding).unwrap()
            })
            .unwrap();
        let admitted = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.represent_string_javascript(
                direct,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request(),
                &enabled(),
                WorkDomain::Optional
            ),
            Err(CandidateError::LocalFacts(CompilationFactsError::Facts(
                FactsError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional))
            )))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), admitted);
        assert_eq!(compiler.checkpoint_count(), 2);
        assert_eq!(compiler.local_facts_status().unwrap().entries, 0);
        compiler
            .with_semantic(source, |_, _, ledger| {
                ledger.release(WorkDomain::Optional, padding).unwrap()
            })
            .unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        let selected = published(
            compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    StringChoice::LiteralAtDefinition,
                    request(),
                    &enabled(),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        let expected = render(&mut compiler, selected, &enabled());
        let padding = MEMORY - compiler.ledger().retained_bytes();
        compiler
            .with_semantic(source, |_, _, ledger| {
                ledger.retain(WorkDomain::Optional, padding).unwrap()
            })
            .unwrap();
        let count = compiler.checkpoint_count();
        assert!(matches!(
            compiler.rebase_javascript(selected, source, &enabled(), WorkDomain::Optional),
            Err(CandidateError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), MEMORY);
        assert_eq!(compiler.checkpoint_count(), count);
        compiler
            .with_semantic(source, |_, _, ledger| {
                ledger.release(WorkDomain::Optional, padding).unwrap()
            })
            .unwrap();
        assert_eq!(render(&mut compiler, selected, &enabled()), expected);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn unknown_string_values_and_unproved_activation_sharing_publish_nothing() {
    for source_text in [
        "export string word(string part){return part+\"ready\";}",
        "export string word(){return \"path/\"+\"ready\";}",
    ] {
        checked(source_text, |program| {
            let body = function(&program, "word");
            let definitions = definitions(&program, body, 1);
            let choice = if source_text.contains("string part") {
                StringChoice::LiteralAtDefinition
            } else {
                StringChoice::SharedLiteral {
                    activation: program.initialization[0],
                }
            };
            let mut compiler = compilation();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &enabled(), WorkDomain::Baseline)
                .unwrap();
            enable_facts(&mut compiler);
            let bytes = compiler.ledger().retained_bytes();
            let result = compiler
                .represent_string_javascript(
                    direct,
                    &definitions,
                    choice,
                    request(),
                    &enabled(),
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(
                matches!(result.outcome, StringOutcome::Unknown(_)),
                "{:?}",
                result.outcome
            );
            assert_eq!(compiler.ledger().retained_bytes(), bytes);
            assert_eq!(compiler.checkpoint_count(), 2);
            assert!(!render(&mut compiler, direct, &enabled()).is_empty());
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
