use super::facts::{
    CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
};
use super::helper_family::*;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    inspect(&from_checked_source(&syntax, &checked).unwrap());
}
pub(super) fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap()
}
fn request() -> FamilyRequest {
    FamilyRequest {
        execution: crate::compilation_contract::JavaScriptExecution::Module,
        attempt: AnalysisAttempt {
            plan: HELPER_FAMILY_PLAN,
            algorithm_version: HELPER_FAMILY_VERSION,
            work_quota: 100_000,
        },
        scratch_bytes: 100_000,
        output_bytes: 100_000,
    }
}
pub(super) fn root(program: &Program<'_>, name: &str) -> CellId {
    program
        .cells
        .iter()
        .enumerate()
        .find_map(|(index, cell)| {
            let id = CellId::from_index(index).unwrap();
            (cell.name == name
                && program
                    .unit(cell.owner)
                    .unwrap()
                    .operations
                    .iter()
                    .any(|op| matches!(op.kind,OperationKind::Initialize(c) if c==id)))
            .then_some(id)
        })
        .unwrap()
}
fn query(
    ready: &mut PreparedHelper,
    program: &Program<'_>,
    cache: &mut RetainedFactsCache,
    ledger: &mut BudgetLedger,
    work: u64,
) {
    let mut session = cache.session(ledger, WorkDomain::Baseline, 1).unwrap();
    let facts = session
        .query(
            program,
            ready.root().body,
            FactRequest {
                attempt: AnalysisAttempt {
                    plan: LOCAL_FACTS_PLAN,
                    algorithm_version: LOCAL_FACTS_VERSION,
                    work_quota: work,
                },
                result_bytes: 50_000,
            },
        )
        .unwrap();
    ready
        .check_body(program, facts.facts, facts.receipt)
        .unwrap();
}
pub(super) fn cache(ledger: &mut BudgetLedger) -> RetainedFactsCache {
    RetainedFactsCache::new(
        CacheLimits {
            entries: 4,
            bytes: 100_000,
            result_bytes: 50_000,
        },
        ledger,
        WorkDomain::Baseline,
    )
    .unwrap()
}
pub(super) fn analyze(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    ledger: &mut BudgetLedger,
    cache: &mut RetainedFactsCache,
) -> (FamilyOutcome, PrerequisiteWork) {
    let preparation =
        prepare(program, uses, cell, request(), ledger, WorkDomain::Optional).unwrap();
    assert_eq!(preparation.receipt.completion, AnalysisCompletion::Complete);
    let outcome = match preparation.outcome {
        PreparationOutcome::Ready(mut ready) => {
            query(&mut ready, program, cache, ledger, 100_000);
            ready.finish(ledger).unwrap()
        }
        PreparationOutcome::Unknown(reason) => FamilyOutcome::Unknown(reason),
        PreparationOutcome::Truncated(reason) => FamilyOutcome::Truncated(reason),
    };
    (outcome, preparation.prerequisites)
}
macro_rules! fixture {
    ($name:literal,$root:literal) => {
        (
            include_str!(concat!("fixtures/helper/", $name, ".lil")),
            $root,
        )
    };
}

#[test]
fn complete_helper_families_accept_the_pinned_primitive_and_record_slice() {
    for (source, name) in [
        fixture!("ordered-repeated-unused", "mix"),
        fixture!("argument-throw-finally", "mix"),
        fixture!("lazy-call", "even"),
        fixture!("nested-calls", "twice"),
        fixture!("parameter-mutation", "twiceNext"),
        fixture!("retained-callers", "add"),
        fixture!("forward-function-prefix", "add"),
        fixture!("record-reentry-argument", "step"),
        fixture!("record-unused-throw", "step"),
        fixture!("record-absence-keys", "step"),
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let before = ledger.retained_bytes();
            let revisions: Vec<_> = program.units.iter().map(|unit| unit.revision()).collect();
            let (outcome, prerequisites) =
                analyze(program, &uses, root(program, name), &mut ledger, &mut cache);
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{name}: {source}\n{outcome:?}")
            };
            assert!(family.dependencies().valid_for(program, &uses));
            assert!(!family.calls().is_empty());
            let body = program.unit(family.root().body).unwrap();
            assert!(matches!(
                body.operations[family
                    .tail_return()
                    .expect("value-returning fixture")
                    .index()]
                .kind,
                OperationKind::Return
            ));
            let behavior = family.body_effects();
            // Eligibility preserves the unchanged leaf schedule. Opaque call
            // inputs may execute conversion hooks; private storage alone is
            // not a purity certificate.
            assert!(
                !behavior.may_suspend && !behavior.creates_identity && !behavior.transfers_control
            );
            for (index, operation) in body.operations.iter().enumerate() {
                let facts = family
                    .operation_facts(OpId::from_index(index).unwrap())
                    .unwrap();
                if matches!(operation.kind, OperationKind::Return) {
                    assert!(facts.behavior.transfers_control);
                }
            }
            for call in family.calls() {
                assert!(family.handle_loads().contains(&record_family::OpRef {
                    unit: call.caller,
                    operation: call.load
                }));
                assert_eq!(
                    family.environments()[call.environment as usize].caller,
                    call.caller
                );
            }
            if name == "step" {
                assert_eq!(prerequisites.attempts, 1);
                assert!(prerequisites.logical_work > 0);
                assert!(!family.captures().is_empty());
            } else {
                assert_eq!(prerequisites, PrerequisiteWork::default());
            }
            assert_eq!(ledger.retained_bytes(), before + family.retained_bytes());
            assert_eq!(
                revisions,
                program
                    .units
                    .iter()
                    .map(|unit| unit.revision())
                    .collect::<Vec<_>>()
            );
            family.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), before);
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn private_storage_does_not_certify_opaque_string_contents_or_pure_coercion() {
    for source in [
        "extern string opaque();string helper(string value){return value+\"!\";}helper(opaque());",
        "extern string opaque();func()->string make(){string value=opaque();auto helper=()=>value+\"!\";return ()=>helper();}auto run=make();run();",
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, _) = analyze(program, &uses, root(program, "helper"), &mut ledger, &mut cache);
            let FamilyOutcome::Complete(family) = outcome else { panic!("{outcome:?}") };
            let body = program.unit(family.root().body).unwrap();
            let mut reads = 0;
            let mut additions = 0;
            for (index, operation) in body.operations.iter().enumerate() {
                let observed = family.operation_facts(OpId::from_index(index).unwrap()).unwrap();
                match operation.kind {
                    OperationKind::Load(_) => {
                        reads += 1;
                        assert!(!observed.primitive_result);
                        assert!(!observed.behavior.may_throw && !observed.behavior.may_reenter);
                    }
                    OperationKind::Binary(BinaryOp::Add) => {
                        additions += 1;
                        assert!(observed.behavior.may_throw && observed.behavior.may_reenter && observed.behavior.may_diverge);
                        assert!(!observed.behavior.may_suspend && !observed.behavior.creates_identity);
                    }
                    _ => {},
                }
            }
            assert!(reads > 0 && additions > 0);
            assert!(family.body_effects().may_throw && family.body_effects().may_reenter && family.body_effects().may_diverge);
            assert!(!family.body_effects().transfers_control);
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}

#[test]
fn helper_raw_domains_retain_transitive_writer_dependencies_and_revalidate_edits() {
    checked(
        "extern string opaque();string text=\"start\";void writer(){string value=opaque();text=\"safe\";}string helper(string input){return input+\"!\";}print(helper(text));",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let helper = root(program, "helper");
            let writer = function(program, "writer");
            let text = root(program, "text");
            let (outcome, _) = analyze(program, &uses, helper, &mut ledger, &mut cache);
            let FamilyOutcome::Complete(family) = outcome else { panic!("{outcome:?}") };
            assert!(!family.body_effects().may_throw && !family.body_effects().may_reenter);
            assert!(family.return_primitive());
            assert!(family.dependencies().units().iter().any(|(unit, _)| *unit == writer));
            assert!(family.dependencies().cells().iter().any(|(cell, _)| *cell == text));
            let changed = edited(program, writer, |data| {
                let opaque_result = data.operations.iter().find_map(|operation| matches!(operation.kind, OperationKind::Call(_)).then_some(operation.result).flatten()).unwrap();
                let store = data.operations.iter().find(|operation| matches!(operation.kind, OperationKind::Store(place) if matches!(data.places[place.index()], Place::Cell(cell) if cell == text))).unwrap();
                data.operands[store.operands.start as usize] = opaque_result;
            });
            let changed_uses = UseIndex::build(&changed, &mut ledger, WorkDomain::Baseline).unwrap();
            assert!(!family.dependencies().valid_for(&changed, &changed_uses));
            let (outcome, _) = analyze(&changed, &changed_uses, helper, &mut ledger, &mut cache);
            let FamilyOutcome::Complete(revised) = outcome else { panic!("{outcome:?}") };
            assert!(revised.body_effects().may_throw && revised.body_effects().may_reenter);
            family.discard(&mut ledger).unwrap();
            revised.discard(&mut ledger).unwrap();
            changed_uses.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn helper_record_prerequisite_supplies_runtime_payload_domains_without_second_analysis() {
    checked(
        "int run(){Record<int> state=record{count:1};auto helper=(int delta)=>{state.count=(state.count??0)+delta;return state.count??0;};return helper(2);}print(run());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, prerequisites) = analyze(program, &uses, root(program, "helper"), &mut ledger, &mut cache);
            let FamilyOutcome::Complete(family) = outcome else { panic!("{outcome:?}") };
            assert_eq!(prerequisites.attempts, 1);
            assert!(prerequisites.logical_work > 0);
            assert!(!family.body_effects().may_throw && !family.body_effects().may_reenter);
            assert!(family.return_primitive());
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn helper_product_presence_shares_copy_evidence_but_keeps_discarded_coercion() {
    checked(
        "struct P{int x;int y;}extern int opaque();P state=P{opaque(),2};P saved=state;auto helper=()=>{state.x;return state.x+saved.x;};print(helper());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, prerequisites) = analyze(program, &uses, root(program, "helper"), &mut ledger, &mut cache);
            let FamilyOutcome::Complete(family) = outcome else { panic!("{outcome:?}") };
            assert_eq!(prerequisites.attempts, 1, "capturing both independent copies shares one complete component proof");
            assert!(prerequisites.logical_work > 0);
            assert!(family.body_effects().may_throw && family.body_effects().may_reenter);
            assert!(family.return_primitive());
            let body = program.unit(family.root().body).unwrap();
            let mut reads = 0;
            for (index, operation) in body.operations.iter().enumerate() {
                if matches!(operation.kind, OperationKind::Load(place) if matches!(body.places[place.index()], Place::Field { .. })) {
                    reads += 1;
                    let facts = family.operation_facts(OpId::from_index(index).unwrap()).unwrap();
                    assert!(facts.primitive_result);
                    assert_eq!(facts.behavior, facts::EvaluationBehavior::COERCION);
                }
            }
            assert_eq!(reads, 3, "even the unused first read executes its conversion");
            for name in ["state", "saved"] {
                assert!(family.dependencies().cells().iter().any(|(cell,_)| *cell == root(program,name)));
            }
            assert!(family.dependencies().creators().iter().any(|(unit,_)| *unit==family.root().body));
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(),0);
        },
    );
}

#[test]
fn complete_use_and_body_proofs_reject_the_pinned_observable_cases() {
    for (source, name) in [
        fixture!("escaped-name", "twice"),
        fixture!("mutable-callee", "helper"),
        fixture!("reference-preparation", "value"),
        fixture!("body-host-call", "helper"),
        fixture!("body-throw", "helper"),
        fixture!("early-capture", "helper"),
        fixture!("recursive", "count"),
        fixture!("body-escaping-closure", "helper"),
    ] {
        checked(source, |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let before = ledger.retained_bytes();
            let (outcome, _) =
                analyze(program, &uses, root(program, name), &mut ledger, &mut cache);
            assert!(
                matches!(outcome, FamilyOutcome::Unknown(_)),
                "{source}\n{outcome:?}"
            );
            assert_eq!(ledger.retained_bytes(), before);
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
        });
    }
}

#[test]
fn captured_mutable_scalars_and_branch_results_have_initialized_storage_evidence() {
    checked(
        "func()->int make(){int count=1;auto step=(int delta)=>{count+=delta;int result=if(true){count}else{delta};return result;};return ()=>step(2);}auto run=make();print(run());print(run());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, prerequisites) = analyze(
                program,
                &uses,
                root(program, "step"),
                &mut ledger,
                &mut cache,
            );
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{outcome:?}")
            };
            assert_eq!(prerequisites.attempts, 0);
            assert_ne!(family.body_effects().writes, facts::MemoryAccess::None);
            assert_eq!(family.captures().len(), 1);
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
        },
    );
}

pub(super) fn edited<'src>(
    program: &Program<'src>,
    unit: UnitId,
    edit: impl FnOnce(&mut UnitData),
) -> Program<'src> {
    let mut changed = program.clone();
    let mut working = changed.units[unit.index()].clone().into_working();
    edit(working.get_mut());
    changed.units[unit.index()] = working.freeze();
    changed.verify().unwrap();
    changed
}
fn function(program: &Program<'_>, name: &str) -> UnitId {
    let CellBinding::Function(unit) = program.cells[root(program, name).index()].binding else {
        panic!("function")
    };
    unit
}

#[test]
fn changed_body_new_callable_capture_and_record_consumer_invalidate_precisely() {
    checked(
        "extern void observe(func(int)->int value);int helper(int value){return value+1;}int other(int value){return value;}int unrelated(){observe(other);return 4;}print(helper(2));",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, _) = analyze(
                program,
                &uses,
                root(program, "helper"),
                &mut ledger,
                &mut cache,
            );
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{outcome:?}")
            };
            let remote = function(program, "unrelated");
            let changed = edited(program, remote, |unit| {
                unit.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(4))))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Integer(5));
            });
            let updated = uses
                .updated(&changed, &[remote], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(family.dependencies().valid_for(&changed, &updated));
            updated.discard(&mut ledger).unwrap();
            let body = family.root().body;
            let changed = edited(program, body, |unit| {
                unit.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(1))))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Integer(2));
            });
            let updated = uses
                .updated(&changed, &[body], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert!(!family.dependencies().valid_for(&changed, &updated));
            updated.discard(&mut ledger).unwrap();
            // A previously uninvolved consumer now passes the helper to host
            // code while its original producer and body remain unchanged.
            let other = root(program, "other");
            let changed = edited(program, remote, |unit| {
                for place in &mut unit.places {
                    if *place == Place::Cell(other) {
                        *place = Place::Cell(family.root().cell);
                    }
                }
                for capture in &mut unit.captures {
                    if *capture == other {
                        *capture = family.root().cell;
                    }
                }
                unit.captures.sort();
            });
            let updated = uses
                .updated(&changed, &[remote], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert_ne!(
                uses.cell(family.root().cell).unwrap().revision(),
                updated.cell(family.root().cell).unwrap().revision()
            );
            assert!(!family.dependencies().valid_for(&changed, &updated));
            let escaped = prepare(
                &changed,
                &updated,
                family.root().cell,
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(matches!(
                escaped.outcome,
                PreparationOutcome::Unknown(UnknownReason::CallableObservation)
            ));
            updated.discard(&mut ledger).unwrap();
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
        },
    );
    checked(
        include_str!("fixtures/helper/record-reentry-argument.lil"),
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let (outcome, _) = analyze(
                program,
                &uses,
                root(program, "step"),
                &mut ledger,
                &mut cache,
            );
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{outcome:?}")
            };
            let state = root(program, "state");
            let consumer = uses
                .cell(state)
                .unwrap()
                .sites()
                .iter()
                .find_map(|site| match *site {
                    uses::CellUseSite::Unit {
                        unit,
                        usage: uses::CellUse::Capture,
                    } if unit != family.root().body => Some(unit),
                    _ => None,
                })
                .unwrap();
            let changed = edited(program, consumer, |unit| {
                unit.operations
                    .iter_mut()
                    .find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(0))))
                    .unwrap()
                    .kind = OperationKind::Constant(Constant::Integer(1));
            });
            let updated = uses
                .updated(&changed, &[consumer], &mut ledger, WorkDomain::Optional)
                .unwrap();
            assert_eq!(
                uses.cell(state).unwrap().revision(),
                updated.cell(state).unwrap().revision()
            );
            assert!(!family.dependencies().valid_for(&changed, &updated));
            updated.discard(&mut ledger).unwrap();
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
        },
    );
}

#[test]
fn early_callable_creation_is_rejected_while_the_closed_named_prefix_is_accepted() {
    checked(
        "func()->int make(){auto helper=(int value)=>value+1;return ()=>helper(2);}print(make()());",
        |program| {
            let target = root(program, "helper");
            let owner = program.cells[target.index()].owner;
            let changed = edited(program, owner, |unit| {
                let creator = unit
                    .operations
                    .iter()
                    .enumerate()
                    .filter_map(|(index, op)| {
                        matches!(op.kind, OperationKind::Closure(_))
                            .then_some(OpId::from_index(index).unwrap())
                    })
                    .last()
                    .unwrap();
                let entry = &mut unit.regions[unit.entry.index()].operations;
                let index = entry.iter().position(|id| *id == creator).unwrap();
                entry.remove(index);
                entry.insert(0, creator);
            });
            let mut ledger = ledger();
            let uses = UseIndex::build(&changed, &mut ledger, WorkDomain::Baseline).unwrap();
            let result = prepare(
                &changed,
                &uses,
                target,
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap();
            assert!(matches!(
                result.outcome,
                PreparationOutcome::Unknown(UnknownReason::EarlyCaptureOrRead)
            ));
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn helper_attempt_caps_and_one_shot_fact_qualification_release_every_reservation() {
    checked(
        include_str!("fixtures/helper/record-reentry-argument.lil"),
        |program| {
            for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
                let mut ledger = ledger();
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let mut cache = cache(&mut ledger);
                let baseline = ledger.retained_bytes();
                let target = root(program, "step");
                for (work, scratch, output, limit) in [
                    (0, 100_000, 100_000, ResourceLimit::Work),
                    (100_000, 0, 100_000, ResourceLimit::Scratch),
                    (100_000, 100_000, 0, ResourceLimit::Output),
                ] {
                    let mut req = request();
                    req.attempt.work_quota = work;
                    req.scratch_bytes = scratch;
                    req.output_bytes = output;
                    let result = prepare(program, &uses, target, req, &mut ledger, domain).unwrap();
                    assert!(
                        matches!(result.outcome,PreparationOutcome::Truncated(reason) if reason==limit)
                    );
                    assert_eq!(ledger.retained_bytes(), baseline);
                }
                let before_work = ledger.work_used(domain);
                let result =
                    prepare(program, &uses, target, request(), &mut ledger, domain).unwrap();
                assert_eq!(
                    ledger.work_used(domain) - before_work,
                    result.receipt.logical_work + result.prerequisites.logical_work
                );
                let PreparationOutcome::Ready(mut ready) = result.outcome else {
                    panic!("expected prepared")
                };
                assert!(ledger.retained_bytes() > baseline);
                query(&mut ready, program, &mut cache, &mut ledger, 0);
                assert!(matches!(
                    ready.finish(&mut ledger).unwrap(),
                    FamilyOutcome::Truncated(ResourceLimit::LocalFacts)
                ));
                assert_eq!(ledger.retained_bytes(), baseline);
                let PreparationOutcome::Ready(mut ready) =
                    prepare(program, &uses, target, request(), &mut ledger, domain)
                        .unwrap()
                        .outcome
                else {
                    panic!("expected prepared")
                };
                query(&mut ready, program, &mut cache, &mut ledger, 100_000);
                {
                    let mut session = cache.session(&mut ledger, WorkDomain::Baseline, 1).unwrap();
                    let result = session
                        .query(
                            program,
                            ready.root().body,
                            FactRequest {
                                attempt: AnalysisAttempt {
                                    plan: LOCAL_FACTS_PLAN,
                                    algorithm_version: LOCAL_FACTS_VERSION,
                                    work_quota: 100_000,
                                },
                                result_bytes: 50_000,
                            },
                        )
                        .unwrap();
                    assert_eq!(
                        ready.check_body(program, result.facts, result.receipt),
                        Err(FamilyError::AlreadyChecked)
                    );
                }
                ready.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), baseline);
                let PreparationOutcome::Ready(ready) =
                    prepare(program, &uses, target, request(), &mut ledger, domain)
                        .unwrap()
                        .outcome
                else {
                    panic!("expected prepared")
                };
                assert!(matches!(
                    ready.finish(&mut ledger).unwrap(),
                    FamilyOutcome::Unknown(UnknownReason::UnqualifiedFacts)
                ));
                assert_eq!(ledger.retained_bytes(), baseline);
                cache.discard(&mut ledger).unwrap();
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        },
    );
}

#[test]
fn helpers_created_in_a_loop_keep_their_original_state_activation() {
    checked(
        "auto a=()=>0;auto b=()=>0;int i=0;while(i<2){int state=i;auto helper=(int delta)=>{state+=delta;return state;};if(i==0){a=()=>helper(1);}else{b=()=>helper(1);}i+=1;}print(a());print(b());print(a());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let target = root(program, "helper");
            let (outcome, _) = analyze(program, &uses, target, &mut ledger, &mut cache);
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{outcome:?}")
            };
            assert_ne!(
                program.cells[target.index()].region,
                program
                    .unit(program.cells[target.index()].owner)
                    .unwrap()
                    .entry
            );
            assert_eq!(family.calls().len(), 2);
            assert_eq!(family.captures(), &[root(program, "state")]);
            assert_eq!(family.environments().len(), 3);
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn helper_product_parameters_and_local_copies_share_presence_without_erasing_coercion() {
    checked(
        "struct Point{int x;int y;}extern int raw();int step(Point p){Point saved=p;p.x=9;saved.y;return p.x+saved.x;}Point state=Point{raw(),2};print(step(state));print(step(Point{3,4}));",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let before = ledger.retained_bytes();
            let (outcome, prerequisites) = analyze(
                program,
                &uses,
                root(program, "step"),
                &mut ledger,
                &mut cache,
            );
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{outcome:?}")
            };
            assert_eq!(family.calls().len(), 2);
            assert_eq!(prerequisites.attempts, 1, "one parameter/copy component");
            assert!(family.dependencies().valid_for(program, &uses));
            let body = program.unit(family.root().body).unwrap();
            let mut loads = 0;
            for (index, operation) in body.operations.iter().enumerate() {
                if matches!(operation.kind, OperationKind::Load(place)
                    if matches!(body.places[place.index()], Place::Field { .. }))
                {
                    loads += 1;
                    let facts = family
                        .operation_facts(OpId::from_index(index).unwrap())
                        .unwrap();
                    assert!(facts.primitive_result);
                    assert_eq!(facts.behavior, facts::EvaluationBehavior::COERCION);
                }
            }
            assert_eq!(loads, 3, "includes the discarded saved.y read");
            assert!(family.return_primitive());
            family.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), before);
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn helper_product_parameter_presence_rejects_one_opaque_actual_among_known_calls() {
    checked(
        "struct Point{int x;int y;}extern Point incoming();int step(Point p){return p.x;}print(step(Point{1,2}));print(step(incoming()));",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let before = ledger.retained_bytes();
            let (outcome, _) = analyze(
                program,
                &uses,
                root(program, "step"),
                &mut ledger,
                &mut cache,
            );
            assert!(matches!(outcome, FamilyOutcome::Unknown(UnknownReason::RequiredProductEvidence)), "{outcome:?}");
            assert_eq!(ledger.retained_bytes(), before);
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}
