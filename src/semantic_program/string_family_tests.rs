use super::facts::{self, CacheLimits, FactRequest, RetainedFactsCache};
use super::string_family::*;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, AnalysisWorkReceipt, BudgetLedger, BudgetPlan,
    ResourceLimits, WorkDomain,
};

fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&source).unwrap();
    inspect(&from_checked_source(&source, &semantics).unwrap());
}
fn ledger() -> BudgetLedger {
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
        attempt: AnalysisAttempt {
            plan: STRING_FAMILY_PLAN,
            algorithm_version: STRING_FAMILY_VERSION,
            work_quota: 100_000,
        },
        scratch_bytes: 100_000,
        output_bytes: 100_000,
        local_facts: FactRequest {
            attempt: AnalysisAttempt {
                plan: facts::LOCAL_FACTS_PLAN,
                algorithm_version: facts::LOCAL_FACTS_VERSION,
                work_quota: 100_000,
            },
            result_bytes: 100_000,
        },
    }
}
fn cache(ledger: &mut BudgetLedger) -> RetainedFactsCache {
    RetainedFactsCache::new(
        CacheLimits {
            entries: 4,
            bytes: 1_000_000,
            result_bytes: 100_000,
        },
        ledger,
        WorkDomain::Baseline,
    )
    .unwrap()
}
fn values(program: &Program<'_>, kind: impl Fn(&OperationKind) -> bool) -> Vec<ValueRef> {
    program
        .units
        .iter()
        .flat_map(|unit| {
            unit.data()
                .operations
                .iter()
                .filter_map(|op| {
                    kind(&op.kind).then(|| ValueRef {
                        unit: unit.id(),
                        value: op.result.unwrap(),
                    })
                })
                .collect::<Vec<_>>()
        })
        .collect()
}
fn adds(program: &Program<'_>) -> Vec<ValueRef> {
    values(program, |kind| {
        matches!(kind, OperationKind::Binary(BinaryOp::Add))
    })
}
fn analyze(
    program: &Program<'_>,
    definitions: &[ValueRef],
    choice: StringChoice,
    request: FamilyRequest,
    cache: &mut RetainedFactsCache,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> (FamilyAnalysis, Option<(AnalysisWorkReceipt, bool)>) {
    let prepared = prepare(program, definitions, choice, request, ledger, domain).unwrap();
    match prepared.outcome {
        PreparationOutcome::Ready(mut prepared) => {
            let observation = {
                let mut session = cache.session(ledger, domain, 1).unwrap();
                let result = session
                    .query(program, prepared.unit(), request.local_facts)
                    .unwrap();
                prepared
                    .check_facts(program, result.facts, result.receipt)
                    .unwrap();
                (result.receipt, result.cache_hit)
            };
            (prepared.finish(ledger).unwrap(), Some(observation))
        }
        PreparationOutcome::Unknown(reason) => (
            FamilyAnalysis {
                outcome: FamilyOutcome::Unknown(reason),
                receipt: prepared.receipt,
            },
            None,
        ),
        PreparationOutcome::Truncated(limit) => (
            FamilyAnalysis {
                outcome: FamilyOutcome::Truncated(limit),
                receipt: prepared.receipt,
            },
            None,
        ),
    }
}

#[test]
fn computed_payload_is_owned_losslessly_after_cache_discard_in_both_domains() {
    for (source, units) in [
        (r#"print("\ud800"+"x\0`\\");"#, vec![0xd800, 120, 0, 96, 92]),
        (r#"print("\ud83d"+"\ude00");"#, vec![0xd83d, 0xde00]),
    ] {
        checked(source, |program| {
            for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
                let mut ledger = ledger();
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let mut cache = cache(&mut ledger);
                let definitions = adds(program);
                let before = format!(
                    "{:?}",
                    program.unit(definitions[0].unit).unwrap().operations
                );
                let mut receipts = Vec::new();
                let mut families = Vec::new();
                for expected_hit in [false, true] {
                    let work = ledger.work_used(domain);
                    let (analysis, prerequisite) = analyze(
                        program,
                        &definitions,
                        StringChoice::LiteralAtDefinition,
                        request(),
                        &mut cache,
                        &mut ledger,
                        domain,
                    );
                    let (facts_receipt, hit) = prerequisite.unwrap();
                    assert_eq!(hit, expected_hit);
                    assert_eq!(
                        ledger.work_used(domain) - work,
                        analysis.receipt.logical_work + facts_receipt.logical_work
                    );
                    receipts.push((analysis.receipt, facts_receipt));
                    let FamilyOutcome::Complete(family) = analysis.outcome else {
                        panic!("computed exact string")
                    };
                    assert_eq!(
                        family.payload(program).code_units().collect::<Vec<_>>(),
                        units
                    );
                    assert!(family.dependencies().valid_for(program, &uses));
                    families.push(family);
                }
                assert_eq!(receipts[0], receipts[1]);
                assert!(!std::ptr::eq(
                    families[0].payload(program),
                    families[1].payload(program)
                ));
                cache.discard(&mut ledger).unwrap();
                for family in families {
                    assert_eq!(
                        family.payload(program).code_units().collect::<Vec<_>>(),
                        units
                    );
                    family.discard(&mut ledger).unwrap();
                }
                assert_eq!(
                    format!(
                        "{:?}",
                        program.unit(definitions[0].unit).unwrap().operations
                    ),
                    before
                );
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
        });
    }
}

#[test]
fn equal_source_payload_is_reused_and_shared_placement_stays_in_its_activation() {
    checked(r#"if(true){print("a"+"b");}print("ab");"#, |program| {
        let mut ledger = ledger();
        let mut cache = cache(&mut ledger);
        let mut definitions = adds(program);
        let literal = values(
            program,
            |kind| matches!(kind, OperationKind::Constant(Constant::String(id)) if program.strings[id.index()].as_unicode()==Some("ab")),
        )[0];
        definitions.push(literal);
        definitions.reverse();
        let unit = definitions[0].unit;
        let (analysis, _) = analyze(
            program,
            &definitions,
            StringChoice::SharedLiteral { activation: unit },
            request(),
            &mut cache,
            &mut ledger,
            WorkDomain::Optional,
        );
        let FamilyOutcome::Complete(family) = analysis.outcome else {
            panic!("equal source data")
        };
        assert!(family
            .definitions()
            .windows(2)
            .all(|pair| pair[0] < pair[1]));
        for (definition, operation) in family.definitions().iter().zip(family.operations()) {
            assert_eq!(
                program.unit(unit).unwrap().values[definition.value.index()].definition,
                *operation
            );
        }
        let OperationKind::Constant(Constant::String(id)) = program.unit(unit).unwrap().operations
            [program.unit(unit).unwrap().values[literal.value.index()]
                .definition
                .index()]
        .kind
        else {
            unreachable!()
        };
        assert!(std::ptr::eq(
            family.payload(program),
            &program.strings[id.index()]
        ));
        assert_eq!(
            family.choice(),
            StringChoice::SharedLiteral { activation: unit }
        );
        family.discard(&mut ledger).unwrap();
        cache.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn stale_facts_and_source_edits_cannot_qualify_retained_string_choices() {
    checked(
        r#"string stable(){return "x";}print("a"+"b");"#,
        |program| {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut cache = cache(&mut ledger);
            let definitions = adds(program);
            let unit = definitions[0].unit;
            let (analysis, _) = analyze(
                program,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request(),
                &mut cache,
                &mut ledger,
                WorkDomain::Optional,
            );
            let FamilyOutcome::Complete(family) = analysis.outcome else {
                panic!("exact string")
            };
            let changed = |id: UnitId| {
                let mut edited = program.clone();
                let mut working = edited.units[id.index()].clone().into_working();
                let _ = working.get_mut();
                edited.units[id.index()] = working.freeze();
                edited
            };
            let edited = changed(unit);
            let other = program
                .units
                .iter()
                .find(|candidate| candidate.id() != unit)
                .unwrap()
                .id();
            assert!(!family.dependencies().valid_for(&edited, &uses));
            assert!(family.dependencies().valid_for(&changed(other), &uses));
            let mut tables = program.clone();
            tables.tables_revision = RevisionId::fresh();
            assert!(!family.dependencies().valid_for(&tables, &uses));
            let PreparationOutcome::Ready(mut prepared) = prepare(
                program,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request(),
                &mut ledger,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
            else {
                panic!("prepared")
            };
            {
                let mut session = cache.session(&mut ledger, WorkDomain::Optional, 1).unwrap();
                let result = session.query(&edited, unit, request().local_facts).unwrap();
                prepared
                    .check_facts(&edited, result.facts, result.receipt)
                    .unwrap();
                assert_eq!(
                    prepared.check_facts(&edited, result.facts, result.receipt),
                    Err(FamilyError::AlreadyChecked)
                );
            }
            assert!(matches!(
                prepared.finish(&mut ledger).unwrap().outcome,
                FamilyOutcome::Unknown(UnknownReason::UnqualifiedFacts)
            ));
            family.discard(&mut ledger).unwrap();
            cache.discard(&mut ledger).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
}

#[test]
fn unsupported_producers_cross_activations_and_unequal_groups_stay_computed() {
    checked(
        r#"string other(){return "c"+"d";}string value="a";print(value+"b");print("x"+"y");print(if(true){"yes"}else{"no"});"#,
        |program| {
            let mut ledger = ledger();
            let mut cache = cache(&mut ledger);
            let all = adds(program);
            let root = program.initialization[0];
            let local: Vec<_> = all
                .iter()
                .copied()
                .filter(|value| value.unit == root)
                .collect();
            let remote = all
                .iter()
                .copied()
                .find(|value| value.unit != root)
                .unwrap();
            for (definitions, choice, expected) in [
                (
                    vec![local[0]],
                    StringChoice::LiteralAtDefinition,
                    UnknownReason::UnknownValue,
                ),
                (
                    vec![local[1], remote],
                    StringChoice::LiteralAtDefinition,
                    UnknownReason::CrossUnitDefinitions,
                ),
                (
                    vec![local[1]],
                    StringChoice::SharedLiteral {
                        activation: remote.unit,
                    },
                    UnknownReason::UnsupportedActivation,
                ),
                (
                    vec![local[1], local[1]],
                    StringChoice::LiteralAtDefinition,
                    UnknownReason::DuplicateDefinitions,
                ),
                (
                    values(program, |kind| matches!(kind, OperationKind::Select { .. })),
                    StringChoice::LiteralAtDefinition,
                    UnknownReason::UnsupportedProducer,
                ),
            ] {
                let memory = ledger.retained_bytes();
                let (result, _) = analyze(
                    program,
                    &definitions,
                    choice,
                    request(),
                    &mut cache,
                    &mut ledger,
                    WorkDomain::Optional,
                );
                assert!(
                    matches!(result.outcome,FamilyOutcome::Unknown(reason) if reason==expected),
                    "{:?}",
                    result.outcome
                );
                assert_eq!(ledger.retained_bytes(), memory);
            }
            cache.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        },
    );
    checked(r#"print("a"+"b");print("x"+"y");"#, |program| {
        let mut ledger = ledger();
        let mut cache = cache(&mut ledger);
        let (result, _) = analyze(
            program,
            &adds(program),
            StringChoice::LiteralAtDefinition,
            request(),
            &mut cache,
            &mut ledger,
            WorkDomain::Optional,
        );
        assert!(matches!(
            result.outcome,
            FamilyOutcome::Unknown(UnknownReason::UnequalValues)
        ));
        cache.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn preparation_and_payload_limits_release_all_storage_and_bill_performed_work() {
    let source = format!("print(\"{}\"+\"{}\");", "a".repeat(1000), "b".repeat(1000));
    checked(&source, |program| {
        let mut ledger = ledger();
        let mut cache = cache(&mut ledger);
        let definitions = adds(program);
        for (request, expected) in [
            (
                {
                    let mut r = request();
                    r.attempt.work_quota = 0;
                    r
                },
                ResourceLimit::Work,
            ),
            (
                {
                    let mut r = request();
                    r.scratch_bytes = 0;
                    r
                },
                ResourceLimit::Scratch,
            ),
            (
                {
                    let mut r = request();
                    r.output_bytes = 0;
                    r
                },
                ResourceLimit::Output,
            ),
            (
                {
                    let mut r = request();
                    r.attempt.work_quota = 100;
                    r
                },
                ResourceLimit::Work,
            ),
            (
                {
                    let mut r = request();
                    r.output_bytes = (std::mem::size_of::<StringFamily>()
                        + std::mem::size_of::<ValueRef>()
                        + std::mem::size_of::<OpId>()
                        + 1999) as u64;
                    r
                },
                ResourceLimit::Output,
            ),
            (
                {
                    let mut r = request();
                    r.local_facts.result_bytes = 2000;
                    r
                },
                ResourceLimit::LocalFacts,
            ),
        ] {
            let before = ledger.work_used(WorkDomain::Optional);
            let memory = ledger.retained_bytes();
            let (result, prerequisite) = analyze(
                program,
                &definitions,
                StringChoice::LiteralAtDefinition,
                request,
                &mut cache,
                &mut ledger,
                WorkDomain::Optional,
            );
            assert!(
                matches!(result.outcome,FamilyOutcome::Truncated(limit) if limit==expected),
                "{:?}",
                result.outcome
            );
            assert_eq!(result.receipt.completion, AnalysisCompletion::Truncated);
            assert_eq!(
                ledger.work_used(WorkDomain::Optional) - before,
                result.receipt.logical_work + prerequisite.map_or(0, |p| p.0.logical_work)
            );
            assert_eq!(ledger.retained_bytes(), memory);
        }
        let memory = ledger.retained_bytes();
        let before = ledger.work_used(WorkDomain::Optional);
        let prepared = prepare(
            program,
            &definitions,
            StringChoice::LiteralAtDefinition,
            request(),
            &mut ledger,
            WorkDomain::Optional,
        )
        .unwrap();
        let preview = prepared.receipt;
        let PreparationOutcome::Ready(prepared) = prepared.outcome else {
            panic!("prepared")
        };
        assert!(ledger.retained_bytes() > memory);
        prepared.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), memory);
        assert_eq!(
            ledger.work_used(WorkDomain::Optional) - before,
            preview.logical_work
        );
        cache.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
