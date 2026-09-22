use super::implementations::*;
use super::record_family::{self, FamilyOutcome, FamilyRequest, RecordFamily};
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, RuntimeRisk, TacticId,
    TacticUse, WorkDomain, WorkKind,
};

const MEMORY: u64 = 10_000_000;

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&source).unwrap();
    let program = from_checked_source(&source, &semantics).unwrap();
    inspect(&program);
}

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap()
}

fn family(
    program: &Program<'_>,
    uses: &UseIndex,
    name: &str,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> RecordFamily {
    let state = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == name)
            .unwrap(),
    )
    .unwrap();
    let result = record_family::analyze(
        program,
        uses,
        state,
        FamilyRequest {
            attempt: AnalysisAttempt {
                plan: record_family::RECORD_FAMILY_PLAN,
                algorithm_version: record_family::RECORD_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
        },
        ledger,
        domain,
    )
    .unwrap();
    match result.outcome {
        FamilyOutcome::Complete(family) => family,
        outcome => panic!("expected complete record family: {outcome:?}"),
    }
}

#[test]
fn direct_choice_has_no_optional_storage_or_tactic_provenance() {
    let mut ledger = ledger();
    let before = ledger.clone();
    let direct = ImplementationMap::direct();
    let shared = direct.share(&mut ledger, WorkDomain::Optional).unwrap();
    assert_eq!(direct.records().len(), 0);
    assert!(direct.tactics().is_empty());
    assert_eq!(direct.retained_bytes(), 0);
    shared.discard(&mut ledger).unwrap();
    direct.discard(&mut ledger).unwrap();
    assert_eq!(ledger, before);
}

#[test]
fn sorted_siblings_share_evidence_and_release_only_at_final_original_domain_owner() {
    checked(
        "Record<int> first=record{x:1};Record<int> second=record{x:2};print(first.x);print(second.x);",
        |program| {
            for oldest_first in [false, true] {
                let mut ledger = ledger();
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let index_bytes = ledger.retained_bytes();
                let direct = ImplementationMap::direct();
                let second = family(program, &uses, "second", &mut ledger, WorkDomain::Baseline);
                let one = direct
                    .with_scalar(second, &mut ledger, WorkDomain::Optional)
                    .unwrap();
                let first = family(program, &uses, "first", &mut ledger, WorkDomain::Optional);
                let two = one
                    .with_scalar(first, &mut ledger, WorkDomain::Baseline)
                    .unwrap();
                assert!(two.records().next().unwrap().root().state < one.records().next().unwrap().root().state);
                assert!(std::ptr::eq(
                    one.records().next().unwrap(),
                    two.records().nth(1).unwrap()
                ));
                assert_eq!(one.records().len(), 1);
                assert_eq!(two.records().len(), 2);
                assert_eq!(
                    two.tactics(),
                    &[TacticUse { tactic: TacticId::ScalarReplacement, risk: RuntimeRisk::Neutral }]
                );
                assert!(two.valid_for(program, &uses));
                let before = ledger.retained_bytes();
                let shared = two.share(&mut ledger, WorkDomain::Optional).unwrap();
                assert!(ledger.retained_bytes() - before < two.retained_bytes());
                assert!(std::ptr::eq(
                    two.records().next().unwrap(),
                    shared.records().next().unwrap()
                ));
                shared.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), before);
                if oldest_first {
                    one.discard(&mut ledger).unwrap();
                    assert_eq!(ledger.retained_bytes(), index_bytes + two.retained_bytes());
                    two.discard(&mut ledger).unwrap();
                } else {
                    two.discard(&mut ledger).unwrap();
                    assert_eq!(ledger.retained_bytes(), index_bytes + one.retained_bytes());
                    one.discard(&mut ledger).unwrap();
                }
                direct.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), index_bytes);
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        },
    );
}

#[test]
fn duplicate_and_budget_rejections_discard_incoming_evidence_and_preserve_base() {
    checked(
        "Record<int> first=record{x:1};Record<int> second=record{x:2};print(first.x);print(second.x);",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let direct = ImplementationMap::direct();
            let first = family(program, &uses, "first", &mut ledger, WorkDomain::Baseline);
            let map = direct.with_scalar(first, &mut ledger, WorkDomain::Optional).unwrap();
            let before = ledger.retained_bytes();
            let duplicate = family(program, &uses, "first", &mut ledger, WorkDomain::Optional);
            assert!(matches!(
                map.with_scalar(duplicate, &mut ledger, WorkDomain::Optional),
                Err(ImplementationError::DuplicateRoot)
            ));
            assert_eq!(ledger.retained_bytes(), before);

            let second = family(program, &uses, "second", &mut ledger, WorkDomain::Baseline);
            let padding = MEMORY - ledger.retained_bytes();
            ledger.retain(WorkDomain::Optional, padding).unwrap();
            assert!(matches!(
                map.share(&mut ledger, WorkDomain::Optional),
                Err(ImplementationError::Budget(BudgetError::MemoryExhausted(
                    WorkDomain::Optional
                )))
            ));
            assert_eq!(ledger.retained_bytes(), MEMORY);
            assert!(matches!(
                map.with_scalar(second, &mut ledger, WorkDomain::Optional),
                Err(ImplementationError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional)))
            ));
            assert_eq!(ledger.retained_bytes(), before + padding);
            ledger.release(WorkDomain::Optional, padding).unwrap();
            assert_eq!(ledger.retained_bytes(), before);

            let remaining = 10_000_000 - ledger.work_used(WorkDomain::Optional);
            let second = family(program, &uses, "second", &mut ledger, WorkDomain::Baseline);
            ledger.charge(WorkDomain::Optional, WorkKind::Edit, remaining).unwrap();
            assert!(matches!(
                map.with_scalar(second, &mut ledger, WorkDomain::Optional),
                Err(ImplementationError::Budget(BudgetError::WorkExhausted(WorkDomain::Optional)))
            ));
            assert_eq!(ledger.retained_bytes(), before);
            assert!(map.valid_for(program, &uses));
            map.discard(&mut ledger).unwrap();
            direct.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn consumer_revision_invalidates_map_while_unrelated_unit_changes_preserve_it() {
    checked(
        "int unrelated(){return 4;}func()->int make(){Record<int> state=record{x:1,y:2};return ()=>state.x??0;}print(make()());",
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let direct = ImplementationMap::direct();
            let state = family(program, &uses, "state", &mut ledger, WorkDomain::Optional);
            let map = direct.with_scalar(state, &mut ledger, WorkDomain::Optional).unwrap();
            let unrelated = program.cells.iter().find_map(|cell| match cell.binding {
                CellBinding::Function(unit) if cell.name == "unrelated" => Some(unit),
                _ => None,
            }).unwrap();
            let mutate = |id: UnitId, replace_key: bool| {
                let mut changed = program.clone();
                let mut working = changed.units[id.index()].clone().into_working();
                let unit = working.get_mut();
                if replace_key {
                    let key = StringId::from_index(program.strings.iter().position(|value| value.code_units().eq("y".encode_utf16())).unwrap()).unwrap();
                    let place = unit.places.iter_mut().find(|place| matches!(place, Place::Member { .. })).unwrap();
                    let Place::Member { key: old, .. } = place else { unreachable!() };
                    *old = key;
                } else {
                    let operation = unit.operations.iter_mut().find(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(4)))).unwrap();
                    operation.kind = OperationKind::Constant(Constant::Integer(5));
                }
                changed.units[id.index()] = working.freeze();
                changed.verify().unwrap();
                changed
            };
            let unrelated_change = mutate(unrelated, false);
            let updated = uses.updated(&unrelated_change, &[unrelated], &mut ledger, WorkDomain::Optional).unwrap();
            ledger.charge(WorkDomain::Optional, WorkKind::Edit, map.validation_work(&unrelated_change).unwrap()).unwrap();
            assert!(map.valid_for(&unrelated_change, &updated));
            assert!(map.valid_for_published(&unrelated_change, &updated));
            assert!(map.published_validation_work().unwrap() < map.validation_work(&unrelated_change).unwrap());
            updated.discard(&mut ledger).unwrap();
            let consumer = map.records().next().unwrap().captures()[0];
            let changed = mutate(consumer, true);
            let updated = uses.updated(&changed, &[consumer], &mut ledger, WorkDomain::Optional).unwrap();
            assert!(!map.valid_for(&changed, &updated));
            assert!(!map.valid_for_published(&changed, &updated));
            assert!(!map.valid_for(&changed, &uses));
            assert!(map.valid_for(program, &uses));
            updated.discard(&mut ledger).unwrap();
            map.discard(&mut ledger).unwrap();
            direct.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

fn leaf_helper(
    program: &Program<'_>,
    uses: &UseIndex,
    name: &str,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> super::helper_family::HelperFamily {
    use super::facts::{
        CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
    };
    use super::helper_family::{self, FamilyOutcome, PreparationOutcome};

    let cell = CellId::from_index(
        program
            .cells
            .iter()
            .position(|cell| cell.name == name)
            .unwrap(),
    )
    .unwrap();
    let preparation = helper_family::prepare(
        program,
        uses,
        cell,
        helper_family::FamilyRequest {
            execution: crate::compilation_contract::JavaScriptExecution::Module,
            attempt: AnalysisAttempt {
                plan: helper_family::HELPER_FAMILY_PLAN,
                algorithm_version: helper_family::HELPER_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
        },
        ledger,
        domain,
    )
    .unwrap();
    let mut prepared = match preparation.outcome {
        PreparationOutcome::Ready(prepared) => prepared,
        other => panic!("expected prepared helper: {other:?}"),
    };
    // Standalone map tests own this explicit common cache; production helper
    // selection uses the cache already owned by Compilation.
    let mut cache = RetainedFactsCache::new(
        CacheLimits {
            entries: 1,
            bytes: 100_000,
            result_bytes: 50_000,
        },
        ledger,
        domain,
    )
    .unwrap();
    {
        let mut session = cache.session(ledger, domain, 1).unwrap();
        let facts = session
            .query(
                program,
                prepared.root().body,
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
        prepared
            .check_body(program, facts.facts, facts.receipt)
            .unwrap();
    }
    cache.discard(ledger).unwrap();
    match prepared.finish(ledger).unwrap() {
        FamilyOutcome::Complete(family) => family,
        other => panic!("expected complete helper: {other:?}"),
    }
}

const COMPOSED_SOURCE: &str = "int first(int value){return value+1;}int second(int value){return value*2;}Record<int> state=record{x:1};print(first(1));print(second(2));print(state.x);";

#[test]
fn helper_and_record_maps_compose_in_both_orders_and_share_original_evidence() {
    checked(COMPOSED_SOURCE, |program| {
        for scalar_first in [false, true] {
            for oldest_first in [false, true] {
                let mut ledger = ledger();
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let index_bytes = ledger.retained_bytes();
                let direct = ImplementationMap::direct();
                let scalar = family(program, &uses, "state", &mut ledger, WorkDomain::Baseline);
                let helper =
                    leaf_helper(program, &uses, "second", &mut ledger, WorkDomain::Optional);
                let (one, two) = if scalar_first {
                    let one = direct
                        .with_scalar(scalar, &mut ledger, WorkDomain::Optional)
                        .unwrap();
                    let two = one
                        .with_inline_helper(helper, &mut ledger, WorkDomain::Baseline)
                        .unwrap();
                    assert!(std::ptr::eq(
                        one.records().next().unwrap(),
                        two.records().next().unwrap()
                    ));
                    (one, two)
                } else {
                    let one = direct
                        .with_inline_helper(helper, &mut ledger, WorkDomain::Baseline)
                        .unwrap();
                    let two = one
                        .with_scalar(scalar, &mut ledger, WorkDomain::Optional)
                        .unwrap();
                    assert!(std::ptr::eq(
                        one.helpers().next().unwrap(),
                        two.helpers().next().unwrap()
                    ));
                    (one, two)
                };
                assert_eq!(two.records().len(), 1);
                assert_eq!(two.helpers().len(), 1);
                assert_eq!(
                    two.tactics(),
                    &[
                        TacticUse {
                            tactic: TacticId::ScalarReplacement,
                            risk: RuntimeRisk::Neutral
                        },
                        TacticUse {
                            tactic: TacticId::Inlining,
                            risk: RuntimeRisk::Neutral
                        },
                    ]
                );
                assert!(two.valid_for(program, &uses));
                let helper =
                    leaf_helper(program, &uses, "first", &mut ledger, WorkDomain::Baseline);
                let three = two
                    .with_inline_helper(helper, &mut ledger, WorkDomain::Optional)
                    .unwrap();
                assert!(
                    three.helpers().next().unwrap().root().cell
                        < three.helpers().nth(1).unwrap().root().cell
                );
                assert!(std::ptr::eq(
                    two.helpers().next().unwrap(),
                    three.helpers().nth(1).unwrap()
                ));
                assert!(std::ptr::eq(
                    two.records().next().unwrap(),
                    three.records().next().unwrap()
                ));
                let before_share = ledger.retained_bytes();
                let shared = three.share(&mut ledger, WorkDomain::Baseline).unwrap();
                assert!(ledger.retained_bytes() - before_share < three.retained_bytes());
                assert!(std::ptr::eq(
                    three.helpers().next().unwrap(),
                    shared.helpers().next().unwrap()
                ));
                shared.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), before_share);
                three.discard(&mut ledger).unwrap();
                if oldest_first {
                    one.discard(&mut ledger).unwrap();
                    assert_eq!(ledger.retained_bytes(), index_bytes + two.retained_bytes());
                    two.discard(&mut ledger).unwrap();
                } else {
                    two.discard(&mut ledger).unwrap();
                    assert_eq!(ledger.retained_bytes(), index_bytes + one.retained_bytes());
                    one.discard(&mut ledger).unwrap();
                }
                direct.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), index_bytes);
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        }
    });
}

#[test]
fn helper_insert_rejections_release_incoming_family_without_dropping_sibling_recipes() {
    checked(COMPOSED_SOURCE, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let direct = ImplementationMap::direct();
        let helper = leaf_helper(program, &uses, "first", &mut ledger, WorkDomain::Baseline);
        let map = direct
            .with_inline_helper(helper, &mut ledger, WorkDomain::Optional)
            .unwrap();
        let before = ledger.retained_bytes();
        let duplicate = leaf_helper(program, &uses, "first", &mut ledger, WorkDomain::Optional);
        assert!(matches!(
            map.with_inline_helper(duplicate, &mut ledger, WorkDomain::Baseline),
            Err(ImplementationError::DuplicateRoot)
        ));
        assert_eq!(ledger.retained_bytes(), before);
        let second = leaf_helper(program, &uses, "second", &mut ledger, WorkDomain::Baseline);
        let padding = MEMORY - ledger.retained_bytes();
        ledger.retain(WorkDomain::Optional, padding).unwrap();
        assert!(matches!(
            map.with_inline_helper(second, &mut ledger, WorkDomain::Optional),
            Err(ImplementationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Optional
            )))
        ));
        assert_eq!(ledger.retained_bytes(), before + padding);
        ledger.release(WorkDomain::Optional, padding).unwrap();
        assert!(map.valid_for(program, &uses));
        map.discard(&mut ledger).unwrap();
        direct.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

fn string_family_for(
    program: &Program<'_>,
    values: &[super::string_family::ValueRef],
    choice: super::string_family::StringChoice,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> super::string_family::StringFamily {
    use super::facts::{
        CacheLimits, FactRequest, RetainedFactsCache, LOCAL_FACTS_PLAN, LOCAL_FACTS_VERSION,
    };
    use super::string_family;
    let request = FactRequest {
        attempt: AnalysisAttempt {
            plan: LOCAL_FACTS_PLAN,
            algorithm_version: LOCAL_FACTS_VERSION,
            work_quota: 100_000,
        },
        result_bytes: 100_000,
    };
    let preparation = string_family::prepare(
        program,
        values,
        choice,
        string_family::FamilyRequest {
            attempt: AnalysisAttempt {
                plan: string_family::STRING_FAMILY_PLAN,
                algorithm_version: string_family::STRING_FAMILY_VERSION,
                work_quota: 100_000,
            },
            scratch_bytes: 100_000,
            output_bytes: 100_000,
            local_facts: request,
        },
        ledger,
        domain,
    )
    .unwrap();
    let mut prepared = match preparation.outcome {
        string_family::PreparationOutcome::Ready(prepared) => prepared,
        other => panic!("string preparation: {other:?}"),
    };
    let mut cache = RetainedFactsCache::new(
        CacheLimits {
            entries: 1,
            // One full qualified result plus the cache's own metadata.
            bytes: 200_000,
            result_bytes: 100_000,
        },
        ledger,
        domain,
    )
    .unwrap();
    {
        let mut session = cache.session(ledger, domain, 1).unwrap();
        let view = session.query(program, prepared.unit(), request).unwrap();
        prepared
            .check_facts(program, view.facts, view.receipt)
            .unwrap();
    }
    cache.discard(ledger).unwrap();
    match prepared.finish(ledger).unwrap().outcome {
        string_family::FamilyOutcome::Complete(family) => family,
        other => panic!("string proof: {other:?}"),
    }
}

fn string_values(program: &Program<'_>) -> Vec<super::string_family::ValueRef> {
    program
        .units
        .iter()
        .enumerate()
        .flat_map(|(unit, body)| {
            body.data().operations.iter().filter_map(move |op| {
                matches!(op.kind, OperationKind::Binary(BinaryOp::Add)).then(|| {
                    super::string_family::ValueRef {
                        unit: UnitId::from_index(unit).unwrap(),
                        value: op.result.unwrap(),
                    }
                })
            })
        })
        .collect()
}

#[test]
fn map_union_shares_once_proved_families_and_commutes_across_all_recipe_kinds() {
    use super::implementation_identity::ImplementationIdentity;
    use super::string_family::StringChoice;
    use crate::output_budget::AllocationBudget;
    let source = format!("{COMPOSED_SOURCE}print(\"left\"+\"right\");print(\"left\"+\"right\");");
    checked(&source, |program| {
        for oldest_first in [false, true] {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let index_bytes = ledger.retained_bytes();
            let direct = ImplementationMap::direct();
            let record = direct
                .with_scalar(
                    family(program, &uses, "state", &mut ledger, WorkDomain::Baseline),
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            let helper = direct
                .with_inline_helper(
                    leaf_helper(program, &uses, "first", &mut ledger, WorkDomain::Optional),
                    &mut ledger,
                    WorkDomain::Baseline,
                )
                .unwrap();
            let definitions = string_values(program);
            let strings = direct
                .with_string(
                    string_family_for(
                        program,
                        &definitions,
                        StringChoice::SharedLiteral {
                            activation: definitions[0].unit,
                        },
                        &mut ledger,
                        WorkDomain::Baseline,
                    ),
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            let proof_work = ledger.work_by_kind(WorkKind::Analysis);
            let rh = record
                .union(&helper, &mut ledger, WorkDomain::Optional)
                .unwrap();
            let rhs = rh
                .union(&strings, &mut ledger, WorkDomain::Optional)
                .unwrap();
            let sh = strings
                .union(&helper, &mut ledger, WorkDomain::Baseline)
                .unwrap();
            let shr = sh
                .union(&record, &mut ledger, WorkDomain::Baseline)
                .unwrap();
            assert_eq!(
                ledger.work_by_kind(WorkKind::Analysis),
                proof_work,
                "union never reenters family/facts analysis"
            );
            assert!(std::ptr::eq(
                record.records().next().unwrap(),
                rhs.records().next().unwrap()
            ));
            assert!(std::ptr::eq(
                helper.helpers().next().unwrap(),
                rhs.helpers().next().unwrap()
            ));
            assert!(std::ptr::eq(
                strings.strings().next().unwrap(),
                rhs.strings().next().unwrap()
            ));
            assert!(std::ptr::eq(
                rhs.records().next().unwrap(),
                shr.records().next().unwrap()
            ));
            assert!(std::ptr::eq(
                rhs.helpers().next().unwrap(),
                shr.helpers().next().unwrap()
            ));
            assert!(std::ptr::eq(
                rhs.strings().next().unwrap(),
                shr.strings().next().unwrap()
            ));
            assert!(rhs.valid_for(program, &uses) && shr.valid_for(program, &uses));
            let owner = RevisionId::fresh();
            let (left, right) = {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let left = ImplementationIdentity::build(Some(&rhs), owner, &mut budget).unwrap();
                let right = ImplementationIdentity::build(Some(&shr), owner, &mut budget).unwrap();
                assert!(left.equivalent(&right, &mut budget).unwrap());
                (left, right)
            };
            left.discard(owner, &mut ledger).unwrap();
            right.discard(owner, &mut ledger).unwrap();
            rh.discard(&mut ledger).unwrap();
            sh.discard(&mut ledger).unwrap();
            if oldest_first {
                record.discard(&mut ledger).unwrap();
                helper.discard(&mut ledger).unwrap();
                strings.discard(&mut ledger).unwrap();
                rhs.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), index_bytes + shr.retained_bytes());
                shr.discard(&mut ledger).unwrap();
            } else {
                shr.discard(&mut ledger).unwrap();
                rhs.discard(&mut ledger).unwrap();
                record.discard(&mut ledger).unwrap();
                helper.discard(&mut ledger).unwrap();
                strings.discard(&mut ledger).unwrap();
            }
            direct.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}

#[test]
fn map_union_deduplicates_independent_equivalent_proofs_without_arc_identity() {
    use super::string_family::StringChoice;
    let source = format!("{COMPOSED_SOURCE}print(\"left\"+\"right\");");
    checked(&source, |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let definitions = string_values(program);
        let make = |ledger: &mut BudgetLedger| {
            let a = ImplementationMap::direct()
                .with_scalar(
                    family(program, &uses, "state", ledger, WorkDomain::Optional),
                    ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            let b = a
                .with_inline_helper(
                    leaf_helper(program, &uses, "first", ledger, WorkDomain::Optional),
                    ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            let c = b
                .with_string(
                    string_family_for(
                        program,
                        &definitions,
                        StringChoice::LiteralAtDefinition,
                        ledger,
                        WorkDomain::Optional,
                    ),
                    ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            a.discard(ledger).unwrap();
            b.discard(ledger).unwrap();
            c
        };
        let left = make(&mut ledger);
        let right = make(&mut ledger);
        assert!(!std::ptr::eq(
            left.records().next().unwrap(),
            right.records().next().unwrap()
        ));
        assert!(!std::ptr::eq(
            left.helpers().next().unwrap(),
            right.helpers().next().unwrap()
        ));
        assert!(!std::ptr::eq(
            left.strings().next().unwrap(),
            right.strings().next().unwrap()
        ));
        let before = ledger.retained_bytes();
        let union = left
            .union(&right, &mut ledger, WorkDomain::Baseline)
            .unwrap();
        assert_eq!(
            (
                union.records().len(),
                union.helpers().len(),
                union.strings().len()
            ),
            (1, 1, 1)
        );
        assert!(
            ledger.retained_bytes() - before < union.retained_bytes(),
            "only a reference shell is newly charged"
        );
        assert!(std::ptr::eq(
            left.strings().next().unwrap(),
            union.strings().next().unwrap()
        ));
        right.discard(&mut ledger).unwrap();
        left.discard(&mut ledger).unwrap();
        assert!(union.valid_for(program, &uses));
        union.discard(&mut ledger).unwrap();
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn map_union_rejects_overlapping_string_partitions_and_choices_atomically() {
    use super::string_family::StringChoice;
    checked(
        "print(\"a\"+\"b\");print(\"a\"+\"b\");print(\"a\"+\"b\");print(\"a\"+\"b\");",
        |program| {
            let mut ledger = ledger();
            let definitions = string_values(program);
            assert_eq!(definitions.len(), 4);
            let shared = StringChoice::SharedLiteral {
                activation: definitions[0].unit,
            };
            let make = |values: &[_], choice, ledger: &mut BudgetLedger| {
                ImplementationMap::direct()
                    .with_string(
                        string_family_for(program, values, choice, ledger, WorkDomain::Optional),
                        ledger,
                        WorkDomain::Optional,
                    )
                    .unwrap()
            };
            let left = make(&[definitions[0], definitions[2]], shared, &mut ledger);
            for (indices, choice) in [
                ([1, 2], shared),
                ([0, 1], shared),
                ([0, 2], StringChoice::LiteralAtDefinition),
            ] {
                let right = make(
                    &[definitions[indices[0]], definitions[indices[1]]],
                    choice,
                    &mut ledger,
                );
                let retained = ledger.retained_bytes();
                assert!(matches!(
                    left.union(&right, &mut ledger, WorkDomain::Optional),
                    Err(ImplementationError::ConflictingChoice)
                ));
                assert_eq!(ledger.retained_bytes(), retained);
                assert_eq!(
                    left.strings().next().unwrap().definitions(),
                    &[definitions[0], definitions[2]]
                );
                right.discard(&mut ledger).unwrap();
            }
            let disjoint = make(&[definitions[1], definitions[3]], shared, &mut ledger);
            let union = left
                .union(&disjoint, &mut ledger, WorkDomain::Baseline)
                .unwrap();
            assert_eq!(union.strings().len(), 2);
            assert_eq!(
                union
                    .strings()
                    .flat_map(|family| family.definitions())
                    .count(),
                4
            );
            assert!(union
                .tactics()
                .iter()
                .any(|usage| usage.tactic == TacticId::StringPooling));
            disjoint.discard(&mut ledger).unwrap();
            left.discard(&mut ledger).unwrap();
            union.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn map_union_work_and_partial_allocation_failures_keep_both_inputs_reusable() {
    checked(COMPOSED_SOURCE, |program| {
        for memory_failure in [false, true] {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let left = ImplementationMap::direct()
                .with_scalar(
                    family(program, &uses, "state", &mut ledger, WorkDomain::Baseline),
                    &mut ledger,
                    WorkDomain::Optional,
                )
                .unwrap();
            let right = ImplementationMap::direct()
                .with_inline_helper(
                    leaf_helper(program, &uses, "first", &mut ledger, WorkDomain::Optional),
                    &mut ledger,
                    WorkDomain::Baseline,
                )
                .unwrap();
            let before = ledger.retained_bytes();
            let work_before = ledger.work_used(WorkDomain::Optional);
            let successful = left
                .union(&right, &mut ledger, WorkDomain::Optional)
                .unwrap();
            let union_work = ledger.work_used(WorkDomain::Optional) - work_before;
            let union_bytes = ledger.retained_bytes() - before;
            successful.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), before);
            let padding = if memory_failure {
                let padding = MEMORY - before - union_bytes + 1;
                ledger.retain(WorkDomain::Optional, padding).unwrap();
                padding
            } else {
                let remaining = 10_000_000 - ledger.work_used(WorkDomain::Optional);
                ledger
                    .charge(
                        WorkDomain::Optional,
                        WorkKind::Edit,
                        remaining - union_work + 1,
                    )
                    .unwrap();
                0
            };
            let error = left
                .union(&right, &mut ledger, WorkDomain::Optional)
                .unwrap_err();
            assert_eq!(
                error,
                if memory_failure {
                    ImplementationError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional))
                } else {
                    ImplementationError::Budget(BudgetError::WorkExhausted(WorkDomain::Optional))
                }
            );
            assert_eq!(ledger.retained_bytes(), before + padding);
            ledger.release(WorkDomain::Optional, padding).unwrap();
            assert!(left.valid_for(program, &uses) && right.valid_for(program, &uses));
            let recovered = left
                .union(&right, &mut ledger, WorkDomain::Baseline)
                .unwrap();
            assert!(std::ptr::eq(
                left.records().next().unwrap(),
                recovered.records().next().unwrap()
            ));
            assert!(std::ptr::eq(
                right.helpers().next().unwrap(),
                recovered.helpers().next().unwrap()
            ));
            left.discard(&mut ledger).unwrap();
            right.discard(&mut ledger).unwrap();
            recovered.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}
