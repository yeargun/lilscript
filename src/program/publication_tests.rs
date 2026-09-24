use super::publication::*;
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::primitive::IntBinary;
use std::process::Command;

const SOURCE: &str = "int answer(){return 7;}int other(){return 11;}print(answer());";
const RECORD_SOURCE: &str = "func()->int make(){Record<int> state=record{x:1};return ()=>{state.x=(state.x??0)+1;return state.x??0;};}int other(){return 11;}auto next=make();print(next());print(next());";

fn policy(text: &str) -> crate::compilation_policy::ResolvedPolicy {
    let mut config: crate::config::ProjectConfig = toml::from_str(text).unwrap();
    // These fixtures observe printed traces deliberately. Tests of stripping
    // construct their distinct effect contract explicitly below.
    config.javascript.strip_console = false;
    config
        .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn scalar_request() -> ScalarRequest {
    ScalarRequest {
        max_work: 100_000,
        scratch_bytes: 100_000,
        output_bytes: 100_000,
    }
}
fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 100_000,
        scratch_bytes: 100_000,
        output_bytes: 100_000,
        local_facts: LocalFactsRequest {
            work_quota: 100_000,
            result_bytes: 50_000,
        },
    }
}

const HELPER_SOURCE: &str = "int helper(int value){return value+3;}export int run(int value){return helper(value);}int other(){return 9;}";

fn helper_cell(program: &Program<'_>) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "helper")
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn helper_publication_reuses_owned_facts_and_the_same_semantic_snapshot() {
    for oldest_first in [false, true] {
        checked(HELPER_SOURCE, |program| {
            let helper = helper_cell(&program);
            let body = function(&program, "helper");
            let mut compiler = compilation();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy(""), WorkDomain::Baseline)
                .unwrap();
            enable_helper_facts(&mut compiler, WorkDomain::Baseline);
            let receipt = compiler
                .with_local_facts(WorkDomain::Optional, 1, |group| {
                    let facts = group
                        .query(source, body, helper_request().local_facts)
                        .unwrap();
                    assert!(!facts.cache_hit());
                    facts.receipt()
                })
                .unwrap();
            let addresses = snapshot_addresses(&mut compiler, source);
            let publication = compiler
                .inline_helper_javascript(
                    direct,
                    helper,
                    helper_request(),
                    &policy(""),
                    WorkDomain::Optional,
                )
                .unwrap();
            assert_eq!(publication.local_facts_receipt, Some(receipt));
            assert_eq!(publication.prerequisite_attempts, 0);
            assert_eq!(publication.prerequisite_work, 0);
            let inline = published_helper(publication);
            assert_eq!(
                snapshot_addresses(&mut compiler, inline.semantic_id()),
                addresses
            );
            assert_eq!(
                compiler.view(source).unwrap().unit_revision(body),
                compiler
                    .view(inline.semantic_id())
                    .unwrap()
                    .unit_revision(body)
            );
            compiler
                .with_local_facts(WorkDomain::Baseline, 2, |group| {
                    for snapshot in [direct.semantic_id(), inline.semantic_id()] {
                        let facts = group
                            .query(snapshot, body, helper_request().local_facts)
                            .unwrap();
                        assert!(facts.cache_hit());
                        assert_eq!(facts.receipt(), receipt);
                    }
                    assert_eq!(group.work().computations, 0);
                })
                .unwrap();
            if oldest_first {
                compiler.discard(source).unwrap();
                compiler.discard(direct.semantic_id()).unwrap();
                assert_eq!(
                    snapshot_addresses(&mut compiler, inline.semantic_id()),
                    addresses
                );
                compiler.discard(inline.semantic_id()).unwrap();
            } else {
                compiler.discard(inline.semantic_id()).unwrap();
                assert_eq!(snapshot_addresses(&mut compiler, source), addresses);
                compiler.discard(direct.semantic_id()).unwrap();
                compiler.discard(source).unwrap();
            }
            assert_eq!(compiler.checkpoint_count(), 0);
            compiler.discard_local_facts().unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn helper_recipe_checks_permissions_on_insertion_rebase_and_output_and_rejects_stale_body() {
    checked(HELPER_SOURCE, |program| {
        let helper = helper_cell(&program);
        let body = function(&program, "helper");
        let body_constant = integer_operation(&program, body);
        let other = function(&program, "other");
        let other_constant = integer_operation(&program, other);
        let mut compiler = Compilation::new(
            ledger(100_000_000, 100_000_000),
            CheckpointLimit { max_live: 8 },
        )
        .unwrap();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy(""), WorkDomain::Baseline)
            .unwrap();
        enable_helper_facts(&mut compiler, WorkDomain::Optional);
        let off = policy("[policy.tactics]\ninlining='off'\n");
        let work = compiler.ledger().work_used(WorkDomain::Optional);
        assert!(matches!(
            compiler.inline_helper_javascript(
                direct,
                helper,
                helper_request(),
                &off,
                WorkDomain::Optional
            ),
            Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::Inlining
            ))
        ));
        assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), work);
        let inline = published_helper(
            compiler
                .inline_helper_javascript(
                    direct,
                    helper,
                    helper_request(),
                    &policy(""),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        assert!(matches!(
            compiler.with_javascript_output(inline, &off, |_| ()),
            Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::Inlining
            ))
        ));
        let unrelated = replace_integer(&mut compiler, source, other, other_constant, 10).unwrap();
        assert!(matches!(
            compiler.rebase_javascript(inline, unrelated, &off, WorkDomain::Optional),
            Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::Inlining
            ))
        ));
        let rebased = compiler
            .rebase_javascript(inline, unrelated, &policy(""), WorkDomain::Optional)
            .unwrap();
        assert_eq!(
            snapshot_addresses(&mut compiler, rebased.semantic_id()),
            snapshot_addresses(&mut compiler, unrelated)
        );
        let changed = replace_integer(&mut compiler, source, body, body_constant, 4).unwrap();
        let count = compiler.checkpoint_count();
        let bytes = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.rebase_javascript(inline, changed, &policy(""), WorkDomain::Optional),
            Err(CandidateError::StaleEvidence)
        ));
        assert_eq!(compiler.checkpoint_count(), count);
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn rejected_helper_preparation_or_fact_query_reclaims_pending_evidence() {
    checked(HELPER_SOURCE, |program| {
        let helper = helper_cell(&program);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy(""), WorkDomain::Baseline)
            .unwrap();
        let bytes = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.inline_helper_javascript(
                direct,
                helper,
                helper_request(),
                &policy(""),
                WorkDomain::Optional
            ),
            Err(CandidateError::LocalFacts(
                CompilationFactsError::NotEnabled
            ))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        enable_helper_facts(&mut compiler, WorkDomain::Baseline);
        let bytes = compiler.ledger().retained_bytes();
        let invalid_facts = HelperRequest {
            local_facts: LocalFactsRequest {
                result_bytes: 0,
                ..helper_request().local_facts
            },
            ..helper_request()
        };
        assert!(matches!(
            compiler.inline_helper_javascript(
                direct,
                helper,
                invalid_facts,
                &policy(""),
                WorkDomain::Optional
            ),
            Err(CandidateError::LocalFacts(CompilationFactsError::Facts(
                super::facts::FactsError::InvalidAttempt
            )))
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        assert_eq!(compiler.checkpoint_count(), 2);
        assert_eq!(compiler.local_facts_status().unwrap().entries, 0);
        for request in [
            HelperRequest {
                max_work: 0,
                ..helper_request()
            },
            HelperRequest {
                scratch_bytes: 0,
                ..helper_request()
            },
            HelperRequest {
                output_bytes: 0,
                ..helper_request()
            },
        ] {
            let result = compiler
                .inline_helper_javascript(
                    direct,
                    helper,
                    request,
                    &policy(""),
                    WorkDomain::Optional,
                )
                .unwrap();
            assert!(matches!(result.outcome, HelperOutcome::Truncated(_)));
            assert_eq!(compiler.ledger().retained_bytes(), bytes);
            assert_eq!(compiler.checkpoint_count(), 2);
        }
        let inline = published_helper(
            compiler
                .inline_helper_javascript(
                    direct,
                    helper,
                    helper_request(),
                    &policy(""),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        let bytes = compiler.ledger().retained_bytes();
        assert!(matches!(
            compiler.inline_helper_javascript(
                inline,
                helper,
                helper_request(),
                &policy(""),
                WorkDomain::Optional
            ),
            Err(CandidateError::DuplicateRoot)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), bytes);
        assert_eq!(compiler.checkpoint_count(), 3);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
fn enable_helper_facts(compiler: &mut Compilation<'_>, domain: WorkDomain) {
    compiler
        .enable_local_facts(
            super::facts::CacheLimits {
                entries: 4,
                bytes: 300_000,
                result_bytes: 50_000,
            },
            domain,
        )
        .unwrap();
}
fn published_helper(result: HelperPublication) -> CandidateId {
    match result.outcome {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("expected helper publication, got {other:?}"),
    }
}
fn state(program: &Program<'_>) -> CellId {
    CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == "state")
            .unwrap(),
    )
    .unwrap()
}
fn published(result: ScalarPublication) -> CandidateId {
    match result.outcome {
        ScalarOutcome::Published(candidate) => candidate,
        other => panic!("expected publication, got {other:?}"),
    }
}
fn snapshot_addresses(compiler: &mut Compilation<'_>, id: SemanticId) -> (usize, usize) {
    compiler
        .with_semantic(id, |program, uses, _| {
            (
                program as *const Program<'_> as usize,
                uses as *const super::uses::UseIndex as usize,
            )
        })
        .unwrap()
}

fn checked(source: &str, inspect: impl FnOnce(Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    inspect(program);
}
fn ledger(optional: u64, memory: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: optional,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}
fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        ledger(100_000_000, 100_000_000),
        CheckpointLimit { max_live: 4 },
    )
    .unwrap()
}
fn function(program: &Program<'_>, name: &str) -> UnitId {
    let cell = program.cells.iter().find(|cell| cell.name == name).unwrap();
    let CellBinding::Function(unit) = cell.binding else {
        panic!("function")
    };
    unit
}
fn integer_operation(program: &Program<'_>, unit: UnitId) -> OpId {
    OpId::from_index(
        program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(_))))
            .unwrap(),
    )
    .unwrap()
}
fn replace_integer(
    compilation: &mut Compilation<'_>,
    base: SemanticId,
    unit: UnitId,
    operation: OpId,
    value: i32,
) -> Result<SemanticId, PublicationError> {
    let expected_revision = compilation.view(base)?.unit_revision(unit).unwrap();
    let kind = OperationKind::Constant(Constant::Integer(value));
    compilation.edit_source(
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
// This test harness renders a transient observation of a borrowed checkpoint;
// target artifact construction is not a retained publication-store API.
fn execute(compilation: &mut Compilation<'_>, id: SemanticId) -> String {
    let source = compilation
        .with_semantic(id, |program, uses, _| {
            assert!(uses.valid_for(program));
            program
                .to_javascript()
                .unwrap()
                .render(crate::js::PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap()
        })
        .unwrap();
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &source])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}\n{source}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}

#[test]
fn physical_siblings_share_one_snapshot_with_constant_fork_work_and_storage() {
    let mut receipts = Vec::new();
    for additional_units in [0, 160] {
        let mut source = SOURCE.to_owned();
        for index in 0..additional_units {
            source.push_str(&format!("int extra{index}(){{return {index};}}"));
        }
        for source_first in [false, true] {
            checked(&source, |program| {
                let mut compiler = compilation();
                let empty = compiler.ledger().retained_bytes();
                let source = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let addresses = snapshot_addresses(&mut compiler, source);
                let direct = compiler
                    .direct_javascript(source, &policy(""), WorkDomain::Optional)
                    .unwrap();
                let sibling = compiler
                    .direct_javascript(direct.semantic_id(), &policy(""), WorkDomain::Optional)
                    .unwrap();
                for candidate in [direct, sibling] {
                    assert_eq!(
                        snapshot_addresses(&mut compiler, candidate.semantic_id()),
                        addresses
                    );
                    let view = compiler.view(candidate.semantic_id()).unwrap();
                    assert!(view.changes().is_empty());
                    assert!(view.cell_changes().is_empty());
                    assert_eq!(view.receipt().index.rebuilt_units, 0);
                    receipts.push((view.receipt().logical_work, view.receipt().allocated_bytes));
                }
                if source_first {
                    compiler.discard(source).unwrap();
                    compiler.discard(direct.semantic_id()).unwrap();
                    assert_eq!(execute(&mut compiler, sibling.semantic_id()), "7\n");
                    compiler.discard(sibling.semantic_id()).unwrap();
                } else {
                    compiler.discard(sibling.semantic_id()).unwrap();
                    compiler.discard(direct.semantic_id()).unwrap();
                    compiler.discard(source).unwrap();
                }
                assert_eq!(compiler.ledger().retained_bytes(), empty);
                assert_eq!(compiler.finish().retained_bytes(), 0);
            });
        }
    }
    assert!(receipts.iter().all(|receipt| *receipt == receipts[0]));
}

#[test]
fn scalar_siblings_share_semantics_and_release_evidence_after_final_owner() {
    for scalar_first in [false, true] {
        checked(RECORD_SOURCE, |program| {
            let state = state(&program);
            let mut compiler = Compilation::new(
                ledger(10_000_000, 10_000_000),
                CheckpointLimit { max_live: 5 },
            )
            .unwrap();
            let empty = compiler.ledger().retained_bytes();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy(""), WorkDomain::Baseline)
                .unwrap();
            let base_memory = compiler.ledger().retained_bytes();
            let scalar = published(
                compiler
                    .scalar_javascript(
                        direct,
                        state,
                        scalar_request(),
                        &policy(""),
                        WorkDomain::Optional,
                    )
                    .unwrap(),
            );
            let scalar_memory = compiler.ledger().retained_bytes();
            assert_eq!(
                scalar_memory - base_memory,
                compiler
                    .view(scalar.semantic_id())
                    .unwrap()
                    .receipt()
                    .allocated_bytes
            );
            let rebased = compiler
                .rebase_javascript(scalar, source, &policy(""), WorkDomain::Baseline)
                .unwrap();
            assert!(
                compiler
                    .view(rebased.semantic_id())
                    .unwrap()
                    .receipt()
                    .allocated_bytes
                    < scalar_memory - base_memory
            );
            let addresses = snapshot_addresses(&mut compiler, source);
            assert_eq!(
                snapshot_addresses(&mut compiler, scalar.semantic_id()),
                addresses
            );
            assert_eq!(
                snapshot_addresses(&mut compiler, rebased.semantic_id()),
                addresses
            );
            let before = compiler.ledger().retained_bytes();
            assert!(matches!(
                compiler.scalar_javascript(
                    scalar,
                    state,
                    scalar_request(),
                    &policy(""),
                    WorkDomain::Optional
                ),
                Err(CandidateError::DuplicateRoot)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), before);
            compiler.discard(source).unwrap();
            compiler.discard(direct.semantic_id()).unwrap();
            let (first, last) = if scalar_first {
                (scalar, rebased)
            } else {
                (rebased, scalar)
            };
            compiler.discard(first.semantic_id()).unwrap();
            let rendered = compiler
                .with_javascript_output(last, &policy(""), |output| {
                    let artifact = output.render(&crate::js::selection::Plan::new(
                        crate::js::selection::Style::Global,
                    ))?;
                    output.take_artifact(artifact)
                })
                .unwrap()
                .unwrap();
            assert!(!rendered.contains("Object.create"));
            compiler.discard(last.semantic_id()).unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), empty);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn source_edits_clear_choices_and_explicit_rebase_rejects_stale_participants() {
    checked(RECORD_SOURCE, |program| {
        let state = state(&program);
        let make = function(&program, "make");
        let other = function(&program, "other");
        let make_literal = integer_operation(&program, make);
        let other_literal = integer_operation(&program, other);
        let mut compiler = Compilation::new(
            ledger(10_000_000, 10_000_000),
            CheckpointLimit { max_live: 7 },
        )
        .unwrap();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy(""), WorkDomain::Baseline)
            .unwrap();
        let scalar = published(
            compiler
                .scalar_javascript(
                    direct,
                    state,
                    scalar_request(),
                    &policy(""),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        let unrelated = replace_integer(
            &mut compiler,
            scalar.semantic_id(),
            other,
            other_literal,
            23,
        )
        .unwrap();
        let inherited = compiler
            .rebase_javascript(scalar, unrelated, &policy(""), WorkDomain::Optional)
            .unwrap();
        assert_eq!(
            snapshot_addresses(&mut compiler, inherited.semantic_id()),
            snapshot_addresses(&mut compiler, unrelated)
        );
        assert_ne!(
            snapshot_addresses(&mut compiler, source),
            snapshot_addresses(&mut compiler, unrelated)
        );
        let changed =
            replace_integer(&mut compiler, scalar.semantic_id(), make, make_literal, 8).unwrap();
        let memory = compiler.ledger().retained_bytes();
        let count = compiler.checkpoint_count();
        assert!(matches!(
            compiler.rebase_javascript(scalar, changed, &policy(""), WorkDomain::Optional),
            Err(CandidateError::StaleEvidence)
        ));
        assert_eq!(compiler.checkpoint_count(), count);
        assert_eq!(compiler.ledger().retained_bytes(), memory);
        assert_eq!(execute(&mut compiler, changed), "9\n10\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn candidate_policy_unknown_and_truncated_results_publish_nothing() {
    checked(RECORD_SOURCE, |program| {
        let state = state(&program);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy(""), WorkDomain::Baseline)
            .unwrap();
        let off = policy("[policy.tactics]\nscalar-replacement='off'\n");
        let memory = compiler.ledger().retained_bytes();
        let work = compiler.ledger().work_used(WorkDomain::Optional);
        assert!(matches!(
            compiler.scalar_javascript(direct, state, scalar_request(), &off, WorkDomain::Optional),
            Err(CandidateError::ForbiddenTactic(
                crate::compilation_policy::TacticId::ScalarReplacement
            ))
        ));
        assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), work);
        for request in [
            ScalarRequest {
                max_work: 0,
                ..scalar_request()
            },
            ScalarRequest {
                scratch_bytes: 0,
                ..scalar_request()
            },
            ScalarRequest {
                output_bytes: 0,
                ..scalar_request()
            },
        ] {
            assert!(matches!(
                compiler
                    .scalar_javascript(direct, state, request, &policy(""), WorkDomain::Optional)
                    .unwrap()
                    .outcome,
                ScalarOutcome::Truncated(_)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), memory);
            assert_eq!(compiler.checkpoint_count(), 2);
        }
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    checked("func()->int make(string key){Record<int> state=record{x:1};return ()=>state[key]??0;}print(make(\"x\")());", |program| {
        let state = state(&program);
        let mut compiler = compilation();
        let source = compiler.adopt_checked(program, WorkDomain::Baseline).unwrap();
        let direct = compiler.direct_javascript(source, &policy(""), WorkDomain::Baseline).unwrap();
        let memory = compiler.ledger().retained_bytes();
        assert_eq!(compiler.scalar_javascript(direct, state, scalar_request(), &policy(""), WorkDomain::Optional).unwrap().outcome, ScalarOutcome::Unknown(ScalarUnknownReason::DynamicKey));
        assert_eq!(compiler.ledger().retained_bytes(), memory);
        assert_eq!(compiler.checkpoint_count(), 2);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn rejected_scalar_publication_releases_completed_analysis_and_map_allocations() {
    checked(RECORD_SOURCE, |program| {
        let state = state(&program);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy(""), WorkDomain::Baseline)
            .unwrap();
        let scalar = published(
            compiler
                .scalar_javascript(
                    direct,
                    state,
                    scalar_request(),
                    &policy(""),
                    WorkDomain::Optional,
                )
                .unwrap(),
        );
        let work = compiler
            .view(scalar.semantic_id())
            .unwrap()
            .receipt()
            .logical_work;
        let peak = compiler.ledger().peak_retained_bytes();
        assert_eq!(compiler.finish().retained_bytes(), 0);
        for (quota, memory) in [(work - 1, 100_000_000), (work, peak - 1)] {
            checked(RECORD_SOURCE, |program| {
                let mut compiler =
                    Compilation::new(ledger(quota, memory), CheckpointLimit { max_live: 4 })
                        .unwrap();
                let source = compiler
                    .adopt_checked(program, WorkDomain::Baseline)
                    .unwrap();
                let direct = compiler
                    .direct_javascript(source, &policy(""), WorkDomain::Baseline)
                    .unwrap();
                let before = compiler.ledger().retained_bytes();
                assert!(compiler
                    .scalar_javascript(
                        direct,
                        state,
                        scalar_request(),
                        &policy(""),
                        WorkDomain::Optional
                    )
                    .is_err());
                assert_eq!(compiler.ledger().retained_bytes(), before);
                assert_eq!(compiler.checkpoint_count(), 2);
                assert_eq!(execute(&mut compiler, source), "2\n3\n");
                assert_eq!(compiler.finish().retained_bytes(), 0);
            });
        }
    });
}

#[test]
fn immutable_target_binding_shares_contract_storage_across_codec_and_effort_policies() {
    checked(SOURCE, |program| {
        let mut compiler = compilation();
        let empty = compiler.ledger().retained_bytes();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let names = "[mangle]\npreserve_properties=['public_name','kept']\n";
        let first_policy = policy(&format!(
            "{names}[javascript]\ncost_model='raw'\noptimization_level=0\n"
        ));
        let first = compiler
            .direct_javascript(source, &first_policy, WorkDomain::Baseline)
            .unwrap();
        let first_receipt = compiler.view(first.semantic_id()).unwrap().receipt();
        let payload =
            (2 * std::mem::size_of::<String>() + "public_name".len() + "kept".len()) as u64;
        compiler
            .with_implementation_description(first, WorkDomain::Baseline, |_| ())
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        for (codec, effort) in [("gzip", 8), ("brotli", 16)] {
            let next_policy = policy(&format!(
                "{names}[javascript]\ncost_model='{codec}'\noptimization_level={effort}\n"
            ));
            assert_eq!(first_policy.contract(), next_policy.contract());
            assert_ne!(first_policy.fingerprint(), next_policy.fingerprint());
            let next = compiler
                .direct_javascript(source, &next_policy, WorkDomain::Optional)
                .unwrap();
            let receipt = compiler.view(next.semantic_id()).unwrap().receipt();
            assert_eq!(
                first_receipt.allocated_bytes,
                receipt.allocated_bytes + payload
            );
            assert_eq!(
                snapshot_addresses(&mut compiler, first.semantic_id()),
                snapshot_addresses(&mut compiler, next.semantic_id())
            );
            compiler
                .with_javascript_output(first, &next_policy, |_| ())
                .unwrap();
            compiler.discard(next.semantic_id()).unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), before);
        }
        compiler.discard(first.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        // The one immutable compilation binding remains for later target
        // requests; discarding its final candidate does not silently retarget.
        assert_eq!(compiler.ledger().retained_bytes(), empty + payload);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn contract_changes_reject_candidate_and_output_reuse_without_changing_sources() {
    use crate::compilation_policy::CompilationRequest;
    checked(SOURCE, |program| {
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.strip_console = false;
        let resolve = |config: &crate::config::ProjectConfig, exports| {
            config
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: exports,
                })
                .unwrap()
        };
        let original = resolve(&config, true);
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &original, WorkDomain::Baseline)
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        let mut variants = Vec::new();
        let mut changed = config.clone();
        changed.javascript.strip_console = true;
        variants.push(resolve(&changed, true));
        let mut changed = config.clone();
        changed.javascript.ecmascript = crate::js_syntax_target::EcmaScriptEdition::Es2015;
        variants.push(resolve(&changed, true));
        // The public-shape variant: keeping every function's source name.
        let mut changed = config.clone();
        changed.javascript.keep_function_names = true;
        variants.push(resolve(&changed, true));
        let mut changed = config.clone();
        changed.mangle.preserve_properties = Some(vec!["stable".into()]);
        variants.push(resolve(&changed, true));
        variants.push(resolve(&config, false));
        // Delivery is part of the contract: a candidate formed for one file
        // is not another mode's candidate.
        for mode in [
            crate::config::BundleMode::Split,
            crate::config::BundleMode::PreserveModules,
        ] {
            let mut changed = config.clone();
            changed.bundle.mode = mode;
            variants.push(resolve(&changed, true));
        }
        let mut changed = config.clone();
        changed.bundle.mode = crate::config::BundleMode::Split;
        changed.bundle.min_chunk_bytes = 1;
        let split = resolve(&changed, true);
        changed.bundle.min_chunk_bytes = 2;
        assert_ne!(split.fingerprint(), resolve(&changed, true).fingerprint());
        for variant in variants {
            assert!(matches!(
                compiler.direct_javascript(source, &variant, WorkDomain::Optional),
                Err(CandidateError::ContractMismatch)
            ));
            assert!(matches!(
                compiler.with_javascript_output(direct, &variant, |_| panic!(
                    "mismatched output callback"
                )),
                Err(CandidateError::ContractMismatch)
            ));
            assert_eq!(compiler.ledger().retained_bytes(), before);
            assert_eq!(compiler.checkpoint_count(), 2);
        }
        assert_eq!(execute(&mut compiler, source), "7\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn failed_first_target_releases_provisional_contract_and_allows_a_different_retry() {
    // The retained binding is larger than frontend/index scratch for this
    // fixture, making peak-1 fail after binding copy at candidate admission.
    let name = "contract_name_".repeat(2048);
    let initial = policy(&format!(
        "[mangle]\npreserve_properties=['{name}','second_name']\n"
    ));
    let different = policy("");
    let mut success = None;
    checked(SOURCE, |program| {
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &initial, WorkDomain::Optional)
            .unwrap();
        success = Some((
            compiler
                .view(direct.semantic_id())
                .unwrap()
                .receipt()
                .logical_work,
            compiler.ledger().peak_retained_bytes(),
        ));
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    let (work, peak) = success.unwrap();
    for (quota, memory) in [(work - 1, 100_000_000), (work, peak - 1)] {
        checked(SOURCE, |program| {
            let mut compiler =
                Compilation::new(ledger(quota, memory), CheckpointLimit { max_live: 4 }).unwrap();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let before = compiler.ledger().retained_bytes();
            assert!(compiler
                .direct_javascript(source, &initial, WorkDomain::Optional)
                .is_err());
            assert_eq!(compiler.ledger().retained_bytes(), before);
            assert_eq!(compiler.checkpoint_count(), 1);
            // Both failures occur after the new dynamic contract was admitted
            // and copied. A different contract must still be accepted now.
            let direct = compiler
                .direct_javascript(source, &different, WorkDomain::Baseline)
                .unwrap();
            compiler
                .with_javascript_output(direct, &different, |_| ())
                .unwrap();
            assert_eq!(execute(&mut compiler, source), "7\n");
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn rejected_source_adoption_and_unsupported_package_do_not_bind_the_store() {
    checked(SOURCE, |program| {
        let mut compiler = compilation();
        let empty = compiler.ledger().retained_bytes();
        let clone = program.clone();
        assert!(matches!(
            compiler.adopt_checked(clone, WorkDomain::Baseline),
            Err(PublicationError::SharedInput)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let before = compiler.ledger().retained_bytes();
        // A native contract is no JavaScript package; refusing it binds nothing.
        let mut config: crate::config::ProjectConfig =
            toml::from_str("[mangle]\npreserve_properties=['not_retained']\n").unwrap();
        config.javascript.strip_console = false;
        let unsupported = config
            .resolve_policy(crate::compilation_policy::CompilationRequest::Native)
            .unwrap();
        assert!(matches!(
            compiler.direct_javascript(source, &unsupported, WorkDomain::Optional),
            Err(CandidateError::NotJavaScript)
        ));
        assert_eq!(compiler.ledger().retained_bytes(), before);
        let direct = compiler
            .direct_javascript(source, &policy(""), WorkDomain::Baseline)
            .unwrap();
        compiler.discard(direct.semantic_id()).unwrap();
        compiler.discard(source).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn checked_source_branch_publishes_semantics_and_uses_together() {
    checked(SOURCE, |program| {
        let answer = function(&program, "answer");
        let other = function(&program, "other");
        let operation = integer_operation(&program, answer);
        let root = program.initialization[0];
        let mut compiler = compilation();
        let empty = compiler.ledger().retained_bytes();
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let adopted = compiler.view(base).unwrap().receipt();
        assert!(adopted.adopted_after_frontend);
        assert_eq!(
            compiler.ledger().retained_bytes(),
            empty + adopted.allocated_bytes
        );
        let retained = compiler.ledger().retained_bytes();
        let next = replace_integer(&mut compiler, base, answer, operation, 42).unwrap();
        let receipt = compiler.view(next).unwrap().receipt();
        assert_eq!(
            compiler.ledger().retained_bytes(),
            retained + receipt.allocated_bytes
        );
        assert_eq!(receipt.verified_units, 1);
        assert_eq!(
            receipt.verified_operations,
            compiler
                .view(next)
                .unwrap()
                .unit(answer)
                .unwrap()
                .operations
                .len()
        );
        assert_eq!(receipt.index.rebuilt_units, 1);
        assert_eq!(compiler.view(next).unwrap().changes().len(), 1);
        assert!(compiler.view(next).unwrap().cell_changes().is_empty());
        assert!(std::ptr::eq(
            compiler.view(base).unwrap().unit(root).unwrap(),
            compiler.view(next).unwrap().unit(root).unwrap()
        ));
        assert!(std::ptr::eq(
            compiler.view(base).unwrap().unit_uses(other).unwrap(),
            compiler.view(next).unwrap().unit_uses(other).unwrap()
        ));
        assert_eq!(
            compiler.view(base).unwrap().tables_revision(),
            compiler.view(next).unwrap().tables_revision()
        );
        assert_eq!(execute(&mut compiler, base), "7\n");
        assert_eq!(execute(&mut compiler, next), "42\n");
        compiler.discard(next).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        compiler.discard(base).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn last_sibling_releases_original_domain_units_and_tables() {
    checked(SOURCE, |program| {
        let answer = function(&program, "answer");
        let operation = integer_operation(&program, answer);
        let mut compiler = compilation();
        let empty = compiler.ledger().retained_bytes();
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let next = replace_integer(&mut compiler, base, answer, operation, 19).unwrap();
        compiler.discard(base).unwrap();
        assert!(matches!(
            compiler.view(base),
            Err(PublicationError::UnknownCheckpoint)
        ));
        assert_eq!(execute(&mut compiler, next), "19\n");
        assert!(compiler.ledger().retained_bytes() > empty);
        compiler.discard(next).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn borrowed_place_patch_derives_complete_cell_use_changes() {
    checked(
        "int left=1;int right=2;int answer(){return left+right;}print(answer());",
        |program| {
            let unit = function(&program, "answer");
            let left = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "left")
                    .unwrap(),
            )
            .unwrap();
            let right = CellId::from_index(
                program
                    .cells
                    .iter()
                    .position(|cell| cell.name == "right")
                    .unwrap(),
            )
            .unwrap();
            let place = PlaceId::from_index(
                program
                    .unit(unit)
                    .unwrap()
                    .places
                    .iter()
                    .position(|place| *place == Place::Cell(left))
                    .unwrap(),
            )
            .unwrap();
            let mut compiler = compilation();
            let base = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let replacement = Place::Cell(right);
            let expected_revision = compiler.view(base).unwrap().unit_revision(unit).unwrap();
            let next = compiler
                .edit_source(
                    base,
                    &[UnitPatch {
                        unit,
                        expected_revision,
                        operations: &[],
                        places: &[PlacePatch {
                            place,
                            replacement: &replacement,
                        }],
                    }],
                    WorkDomain::Optional,
                )
                .unwrap();
            let changes = compiler.view(next).unwrap().cell_changes().to_vec();
            assert_eq!(changes.len(), 2);
            for cell in [left, right] {
                let change = changes.iter().find(|change| change.cell == cell).unwrap();
                assert_eq!(
                    change.previous,
                    compiler
                        .view(base)
                        .unwrap()
                        .cell_users(cell)
                        .unwrap()
                        .revision()
                );
                assert_eq!(
                    change.current,
                    compiler
                        .view(next)
                        .unwrap()
                        .cell_users(cell)
                        .unwrap()
                        .revision()
                );
                assert_ne!(change.previous, change.current);
            }
            assert_eq!(execute(&mut compiler, base), "3\n");
            assert_eq!(execute(&mut compiler, next), "4\n");
            assert_eq!(compiler.finish().retained_bytes(), 0);
        },
    );
}

#[test]
fn invalid_types_forward_reads_and_stale_dependencies_leave_base_usable() {
    checked(SOURCE, |program| {
        let answer = function(&program, "answer");
        let operation = integer_operation(&program, answer);
        let value = program.unit(answer).unwrap().operations[operation.index()]
            .result
            .unwrap();
        let mut compiler = compilation();
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let expected_revision = compiler.view(base).unwrap().unit_revision(answer).unwrap();
        for (kind, operands) in [
            (OperationKind::Constant(Constant::Boolean(true)), vec![]),
            (OperationKind::IntBinary(IntBinary::Add), vec![value, value]),
        ] {
            let error = compiler
                .edit_source(
                    base,
                    &[UnitPatch {
                        unit: answer,
                        expected_revision,
                        operations: &[OperationPatch {
                            operation,
                            kind: &kind,
                            operands: &operands,
                        }],
                        places: &[],
                    }],
                    WorkDomain::Optional,
                )
                .unwrap_err();
            assert_eq!(error, PublicationError::InvalidReplacement);
            assert_eq!(compiler.ledger().retained_bytes(), retained);
            assert_eq!(compiler.checkpoint_count(), 1);
            assert_eq!(execute(&mut compiler, base), "7\n");
        }
        let kind = OperationKind::Constant(Constant::Integer(9));
        assert_eq!(
            compiler
                .edit_source(
                    base,
                    &[UnitPatch {
                        unit: answer,
                        expected_revision: RevisionId::fresh(),
                        operations: &[OperationPatch {
                            operation,
                            kind: &kind,
                            operands: &[]
                        }],
                        places: &[],
                    }],
                    WorkDomain::Optional
                )
                .unwrap_err(),
            PublicationError::StaleRevision(answer)
        );
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn bounded_slot_reuse_rejects_stale_and_foreign_store_ids() {
    checked(SOURCE, |program| {
        let answer = function(&program, "answer");
        let operation = integer_operation(&program, answer);
        let mut compiler = Compilation::new(
            ledger(100_000_000, 100_000_000),
            CheckpointLimit { max_live: 2 },
        )
        .unwrap();
        let mut current = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let foreign = compilation();
        assert!(matches!(
            foreign.view(current),
            Err(PublicationError::UnknownCheckpoint)
        ));
        assert_eq!(foreign.finish().retained_bytes(), 0);
        for value in 0..32 {
            let next = replace_integer(&mut compiler, current, answer, operation, value).unwrap();
            let before = compiler.ledger().retained_bytes();
            assert_eq!(
                replace_integer(&mut compiler, current, answer, operation, value).unwrap_err(),
                PublicationError::StoreFull
            );
            assert_eq!(compiler.ledger().retained_bytes(), before);
            compiler.discard(current).unwrap();
            assert!(matches!(
                compiler.view(current),
                Err(PublicationError::UnknownCheckpoint)
            ));
            assert_eq!(
                compiler.discard(current).unwrap_err(),
                PublicationError::UnknownCheckpoint
            );
            current = next;
        }
        assert_eq!(compiler.checkpoint_count(), 1);
        assert_eq!(execute(&mut compiler, current), "31\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn repeated_operation_patches_rebuild_operands_without_retaining_dead_ranges() {
    checked("int answer(){return 1+2;}print(answer());", |program| {
        let unit = function(&program, "answer");
        let operation = OpId::from_index(
            program
                .unit(unit)
                .unwrap()
                .operations
                .iter()
                .position(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
                .unwrap(),
        )
        .unwrap();
        let operands = program
            .unit(unit)
            .unwrap()
            .operands(program.unit(unit).unwrap().operations[operation.index()].operands)
            .unwrap()
            .to_vec();
        let initial_length = program.unit(unit).unwrap().operands.len();
        let mut compiler = compilation();
        let mut current = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let kind = OperationKind::IntBinary(IntBinary::Subtract);
        for _ in 0..16 {
            let expected_revision = compiler.view(current).unwrap().unit_revision(unit).unwrap();
            let next = compiler
                .edit_source(
                    current,
                    &[UnitPatch {
                        unit,
                        expected_revision,
                        operations: &[OperationPatch {
                            operation,
                            kind: &kind,
                            operands: &operands,
                        }],
                        places: &[],
                    }],
                    WorkDomain::Optional,
                )
                .unwrap();
            assert_eq!(
                compiler
                    .view(next)
                    .unwrap()
                    .unit(unit)
                    .unwrap()
                    .operands
                    .len(),
                initial_length
            );
            compiler.discard(current).unwrap();
            current = next;
        }
        assert_eq!(execute(&mut compiler, current), "-1\n");
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn adoption_rejects_external_arc_owners_and_accounts_nested_payload_capacity() {
    checked("extern func(int[])->string external;struct Pair{int x;int y;}enum Label{A,B}Record<int> state=record{x:1};print(\"payload\");", |program| {
        let retained = program.clone();
        let mut compiler = compilation();
        let empty = compiler.ledger().retained_bytes();
        assert_eq!(compiler.adopt_checked(program, WorkDomain::Baseline).unwrap_err(), PublicationError::SharedInput);
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        let id = compiler.adopt_checked(retained, WorkDomain::Baseline).unwrap();
        assert!(compiler.view(id).unwrap().receipt().allocated_bytes > 0);
        compiler.discard(id).unwrap();
        assert_eq!(compiler.ledger().retained_bytes(), empty);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn work_and_memory_failures_restore_baseline_even_after_copy_or_index_work() {
    let mut measured = None;
    checked(SOURCE, |program| {
        let unit = function(&program, "answer");
        let operation = integer_operation(&program, unit);
        let mut compiler = compilation();
        let base = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let baseline_peak = compiler.ledger().peak_retained_bytes();
        let next = replace_integer(&mut compiler, base, unit, operation, 42).unwrap();
        measured = Some((
            baseline_peak,
            compiler.ledger().peak_retained_bytes(),
            compiler.view(next).unwrap().receipt().logical_work,
        ));
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    let (baseline_peak, edit_peak, edit_work) = measured.unwrap();
    assert!(edit_peak > baseline_peak);
    for optional_work in [0, 1, edit_work / 4, edit_work / 2, edit_work - 1] {
        checked(SOURCE, |program| {
            let unit = function(&program, "answer");
            let operation = integer_operation(&program, unit);
            let mut compiler = Compilation::new(
                ledger(optional_work, 100_000_000),
                CheckpointLimit { max_live: 4 },
            )
            .unwrap();
            let base = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            assert!(matches!(
                replace_integer(&mut compiler, base, unit, operation, 42),
                Err(PublicationError::Budget(_)) | Err(PublicationError::Uses(_))
            ));
            assert_eq!(
                compiler.ledger().retained_bytes(),
                retained,
                "work {optional_work}"
            );
            assert_eq!(compiler.checkpoint_count(), 1);
            assert_eq!(execute(&mut compiler, base), "7\n");
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
    for memory in [
        baseline_peak,
        (baseline_peak + edit_peak) / 2,
        edit_peak - 1,
    ] {
        checked(SOURCE, |program| {
            let unit = function(&program, "answer");
            let operation = integer_operation(&program, unit);
            let mut compiler =
                Compilation::new(ledger(100_000_000, memory), CheckpointLimit { max_live: 4 })
                    .unwrap();
            let base = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            assert!(matches!(
                replace_integer(&mut compiler, base, unit, operation, 42),
                Err(PublicationError::Budget(_)) | Err(PublicationError::Uses(_))
            ));
            assert_eq!(
                compiler.ledger().retained_bytes(),
                retained,
                "memory {memory}"
            );
            assert!(compiler.ledger().peak_retained_bytes() <= memory);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn adoption_budget_failures_release_partial_units_tables_and_index() {
    let mut measured = None;
    checked(SOURCE, |program| {
        let mut compiler = compilation();
        let empty = compiler.ledger().retained_bytes();
        let id = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        measured = Some((
            empty,
            compiler.ledger().peak_retained_bytes(),
            compiler.view(id).unwrap().receipt().logical_work,
        ));
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
    let (empty, peak, adoption_work) = measured.unwrap();
    for quota in [0, 1, adoption_work / 2, adoption_work - 1] {
        checked(SOURCE, |program| {
            let budget = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 4 + quota,
                    optional_work: 0,
                    baseline_retained_bytes: 0,
                    retained_bytes: 100_000_000,
                },
            )
            .unwrap();
            let mut compiler = Compilation::new(budget, CheckpointLimit { max_live: 4 }).unwrap();
            assert!(matches!(
                compiler.adopt_checked(program, WorkDomain::Baseline),
                Err(PublicationError::Budget(_)) | Err(PublicationError::Uses(_))
            ));
            assert_eq!(
                compiler.ledger().retained_bytes(),
                empty,
                "adoption work {quota}"
            );
            assert_eq!(compiler.checkpoint_count(), 0);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
    for memory in [empty, (empty + peak) / 2, peak - 1] {
        checked(SOURCE, |program| {
            let mut compiler =
                Compilation::new(ledger(100_000_000, memory), CheckpointLimit { max_live: 4 })
                    .unwrap();
            assert!(matches!(
                compiler.adopt_checked(program, WorkDomain::Baseline),
                Err(PublicationError::Budget(_)) | Err(PublicationError::Uses(_))
            ));
            assert_eq!(
                compiler.ledger().retained_bytes(),
                empty,
                "adoption memory {memory}"
            );
            assert_eq!(compiler.checkpoint_count(), 0);
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
