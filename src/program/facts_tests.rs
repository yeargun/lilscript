use super::facts::*;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
    WorkKind,
};

fn checked<T>(source: &str, inspect: impl FnOnce(&Program<'_>) -> T) -> T {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    inspect(&program)
}
fn cache() -> FactsCache {
    FactsCache::new(CacheLimits {
        entries: 8,
        bytes: 1_000_000,
        result_bytes: 100_000,
    })
    .unwrap()
}
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 1_000_000,
            optional_work: 1_000_000,
            baseline_retained_bytes: 2_000_000,
            retained_bytes: 4_000_000,
        },
    )
    .unwrap()
}
fn request(quota: u64) -> FactRequest {
    FactRequest {
        attempt: AnalysisAttempt {
            plan: LOCAL_FACTS_PLAN,
            work_quota: quota,
            algorithm_version: LOCAL_FACTS_VERSION,
        },
        result_bytes: 100_000,
    }
}
fn addition(program: &Program<'_>) -> (UnitId, OpId, ValueId) {
    for unit in program.units() {
        for (index, operation) in unit.data().operations.iter().enumerate() {
            if matches!(operation.kind, OperationKind::Binary(BinaryOp::Add)) {
                return (
                    unit.id(),
                    OpId::from_index(index).unwrap(),
                    operation.result.unwrap(),
                );
            }
        }
    }
    panic!("addition fixture")
}

#[test]
fn retained_cache_capacity_stays_charged_between_sessions_in_both_domains() {
    checked(r#"print("left"+"right");"#, |program| {
        let (unit, _, value) = addition(program);
        for (cache_domain, query_domain) in [
            (WorkDomain::Baseline, WorkDomain::Optional),
            (WorkDomain::Optional, WorkDomain::Baseline),
        ] {
            let mut ledger = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 100_000,
                    optional_work: 100_000,
                    baseline_retained_bytes: 0,
                    retained_bytes: 70_000,
                },
            )
            .unwrap();
            let mut cache = RetainedFactsCache::new(
                CacheLimits {
                    entries: 4,
                    bytes: 50_000,
                    result_bytes: 10_000,
                },
                &mut ledger,
                cache_domain,
            )
            .unwrap();
            let baseline = cache.reserved_bytes();
            assert_eq!(baseline, 50_000);
            assert_eq!(ledger.retained_bytes(), baseline);
            let mut req = request(10_000);
            req.result_bytes = 10_000;
            let cold = {
                // Only one cache capacity plus result/history fits. Charging
                // cache capacity again here would reject this valid session.
                let mut session = cache.session(&mut ledger, query_domain, 2).unwrap();
                let result = session.query(program, unit, req).unwrap();
                assert!(!result.cache_hit);
                assert!(result.facts.exact(value).is_some());
                result.receipt
            };
            assert_eq!(ledger.retained_bytes(), baseline);
            assert_eq!(cache.retained_entries(), 1);
            assert!(cache.retained_bytes() <= baseline);
            assert_eq!(ledger.work_used(query_domain), cold.logical_work);
            assert_eq!(ledger.work_used(cache_domain), 0);

            // A candidate allocation between sessions must still see the live
            // cache. Insufficient scratch cannot evict or uncharge that cache.
            assert!(matches!(
                ledger.retain(query_domain, 20_001),
                Err(crate::compilation_policy::BudgetError::MemoryExhausted(_))
            ));
            ledger.retain(query_domain, 15_000).unwrap();
            assert!(matches!(
                cache.session(&mut ledger, query_domain, 2),
                Err(FactsError::Budget(
                    crate::compilation_policy::BudgetError::MemoryExhausted(_)
                ))
            ));
            assert_eq!(ledger.retained_bytes(), baseline + 15_000);
            assert_eq!(cache.retained_entries(), 1);
            ledger.release(query_domain, 15_000).unwrap();
            {
                let mut session = cache.session(&mut ledger, query_domain, 2).unwrap();
                let result = session.query(program, unit, req).unwrap();
                assert!(result.cache_hit);
                assert_eq!(result.receipt, cold);
            }
            assert_eq!(ledger.retained_bytes(), baseline);
            assert_eq!(ledger.work_used(query_domain), cold.logical_work * 2);
            cache.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}

#[test]
fn retained_cache_admission_rejects_before_metadata_construction() {
    let mut ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100,
            optional_work: 100,
            baseline_retained_bytes: 0,
            retained_bytes: 1_000,
        },
    )
    .unwrap();
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        assert!(matches!(
            RetainedFactsCache::new(
                CacheLimits {
                    entries: 8,
                    bytes: 1_000_000,
                    result_bytes: 100_000,
                },
                &mut ledger,
                domain,
            ),
            Err(FactsError::Budget(
                crate::compilation_policy::BudgetError::MemoryExhausted(_)
            ))
        ));
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.peak_retained_bytes(), 0);
        assert_eq!(ledger.work_used(domain), 0);
    }
    assert!(matches!(
        RetainedFactsCache::new(
            CacheLimits {
                entries: usize::MAX,
                bytes: u64::MAX,
                result_bytes: 100_000,
            },
            &mut ledger,
            WorkDomain::Baseline,
        ),
        Err(FactsError::InvalidLimits)
    ));
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn retained_session_query_limits_preserve_cache_accounting() {
    checked("print(1+2);", |program| {
        let mut ledger = ledger();
        let mut cache = RetainedFactsCache::new(
            CacheLimits {
                entries: 4,
                bytes: 50_000,
                result_bytes: 10_000,
            },
            &mut ledger,
            WorkDomain::Baseline,
        )
        .unwrap();
        let baseline = ledger.retained_bytes();
        let mut req = request(10_000);
        req.result_bytes = 10_000;
        {
            let mut session = cache.session(&mut ledger, WorkDomain::Optional, 0).unwrap();
            assert!(matches!(
                session.query(program, program.initialization[0], req),
                Err(FactsError::QueryLimit)
            ));
        }
        assert_eq!(ledger.retained_bytes(), baseline);
        assert_eq!(cache.retained_entries(), 0);
        assert_eq!(ledger.work_used(WorkDomain::Optional), 0);
        assert!(matches!(
            cache.session(&mut ledger, WorkDomain::Optional, usize::MAX),
            Err(FactsError::InvalidLimits)
        ));
        assert_eq!(ledger.retained_bytes(), baseline);
        for requests in [1, 2, 4, 16] {
            {
                let mut session = cache
                    .session(&mut ledger, WorkDomain::Optional, requests)
                    .unwrap();
                session
                    .query(program, program.initialization[0], req)
                    .unwrap();
            }
            assert_eq!(ledger.retained_bytes(), baseline);
            assert_eq!(cache.retained_entries(), 1);
        }
        cache.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn retained_cache_weak_strong_retry_keeps_cold_and_warm_logical_charges_equal() {
    checked(r#"print("a"+"b");"#, |program| {
        let (unit, _, value) = addition(program);
        let limits = CacheLimits {
            entries: 4,
            bytes: 50_000,
            result_bytes: 10_000,
        };
        let mut ledger = ledger();
        let mut warm = RetainedFactsCache::new(limits, &mut ledger, WorkDomain::Baseline).unwrap();
        let mut cold = RetainedFactsCache::new(limits, &mut ledger, WorkDomain::Optional).unwrap();
        let baseline = ledger.retained_bytes();
        let run = |cache: &mut RetainedFactsCache, ledger: &mut BudgetLedger, quota, domain| {
            let before = ledger.work_used(domain);
            let (known, receipt, hit) = {
                let mut session = cache.session(ledger, domain, 2).unwrap();
                let mut req = request(quota);
                req.result_bytes = 10_000;
                let result = session.query(program, unit, req).unwrap();
                (
                    result.facts.exact(value).is_some(),
                    result.receipt,
                    result.cache_hit,
                )
            };
            assert_eq!(ledger.retained_bytes(), baseline);
            (known, receipt, hit, ledger.work_used(domain) - before)
        };
        let low = run(&mut warm, &mut ledger, 1, WorkDomain::Optional);
        assert!(!low.0 && !low.2);
        assert_eq!(low.1.completion, AnalysisCompletion::Truncated);
        let high = run(&mut warm, &mut ledger, 10_000, WorkDomain::Optional);
        assert!(high.0 && !high.2);
        assert_eq!(high.1.completion, AnalysisCompletion::Complete);
        let low_warm = run(&mut warm, &mut ledger, 1, WorkDomain::Baseline);
        assert!(low_warm.2);
        assert_eq!((low.0, low.1, low.3), (low_warm.0, low_warm.1, low_warm.3));
        let high_cold = run(&mut cold, &mut ledger, 10_000, WorkDomain::Baseline);
        assert!(!high_cold.2);
        let high_warm = run(&mut warm, &mut ledger, 10_000, WorkDomain::Optional);
        assert!(high_warm.2);
        assert_eq!(
            (high.0, high.1, high.3),
            (high_cold.0, high_cold.1, high_cold.3)
        );
        assert_eq!(
            (high.0, high.1, high.3),
            (high_warm.0, high_warm.1, high_warm.3)
        );
        warm.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), cold.reserved_bytes());
        cold.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn exact_utf16_string_knowledge_keeps_the_original_computation() {
    checked(r#"print("\ud800"+"x");"#, |program| {
        let (unit, op, value) = addition(program);
        let revision = program.units[unit.index()].revision();
        let before = program.units[unit.index()].data().operations.len();
        let mut cache = cache();
        let mut ledger = ledger();
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
            let result = session.query(program, unit, request(10_000)).unwrap();
            assert_eq!(result.receipt.completion, AnalysisCompletion::Complete);
            assert_eq!(
                result
                    .facts
                    .string(program, value)
                    .unwrap()
                    .code_units()
                    .collect::<Vec<_>>(),
                [0xd800, 120]
            );
            assert!(matches!(
                result.facts.exact(value),
                Some(ExactValue::String(_))
            ));
            assert_eq!(
                result.facts.can_drop(op, ObservationDemand::Discarded),
                Legality::PermittedUnderContext
            );
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(program.units[unit.index()].revision(), revision);
        assert_eq!(program.units[unit.index()].data().operations.len(), before);
        assert!(matches!(
            program.units[unit.index()].data().operations[op.index()].kind,
            OperationKind::Binary(BinaryOp::Add)
        ));
    });
}

#[test]
fn bounded_string_construction_preserves_utf16_and_records_resource_cost() {
    for (source, expected, upper_bound) in [
        (r#"print("😀"+"é");"#, vec![0xd83d, 0xde00, 0xe9], 6),
        (r#"print("\ud83d"+"\ude00");"#, vec![0xd83d, 0xde00], 4),
        (r#"print("\ud800"+"x");"#, vec![0xd800, 120], 3),
        (r#"print("\0"+"z");"#, vec![0, 122], 2),
        (r#"print(""+"");"#, vec![], 0),
    ] {
        checked(source, |program| {
            let (unit, operation, value) = addition(program);
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let result = session.query(program, unit, request(10_000)).unwrap();
            let StringConstructionKnowledge::Bounded(bound) =
                result.facts.string_construction(program, operation)
            else {
                panic!("exact primitive operands supply a bound: {source}");
            };
            assert_eq!(bound.operation(), operation);
            assert_eq!(bound.dependencies(), result.facts.dependencies());
            assert!(bound.dependencies().valid_for(program));
            assert_eq!(
                bound.operands().as_slice(),
                program
                    .unit(unit)
                    .unwrap()
                    .operands(program.unit(unit).unwrap().operations[operation.index()].operands)
                    .unwrap()
            );
            assert_eq!(bound.utf16_units_upper_bound(), upper_bound);
            assert!(expected.len() as u64 <= upper_bound);
            assert_eq!(
                result
                    .facts
                    .string(program, value)
                    .unwrap()
                    .code_units()
                    .collect::<Vec<_>>(),
                expected
            );
            // Resource cost remains known even though implementation-dependent
            // resource exhaustion is excluded from semantic equivalence.
            assert!(!result.facts.effects(operation).may_throw);
            assert!(result.facts.effects(operation).may_exhaust_resources);
            assert_eq!(
                result
                    .facts
                    .can_drop(operation, ObservationDemand::Discarded),
                Legality::PermittedUnderContext
            );
            assert_eq!(
                result.facts.can_duplicate(operation),
                Legality::PermittedUnderContext
            );
            assert_eq!(
                result.facts.can_speculate(
                    operation,
                    SpeculationContext {
                        operands_available: true,
                    },
                ),
                Legality::PermittedUnderContext
            );
        });
    }
}

#[test]
fn bounded_string_construction_reads_computed_inputs_without_recomputing_them() {
    checked(r#"print(("a"+"b")+"c");"#, |program| {
        let unit = program.initialization[0];
        let data = program.unit(unit).unwrap();
        let (index, op) = data
            .operations
            .iter()
            .enumerate()
            .filter(|(_, op)| matches!(op.kind, OperationKind::Binary(BinaryOp::Add)))
            .last()
            .unwrap();
        let operation = OpId::from_index(index).unwrap();
        let left = data.operands(op.operands).unwrap()[0];
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
        let result = session.query(program, unit, request(10_000)).unwrap();
        assert!(matches!(
            result.facts.exact(left),
            Some(ExactValue::String(ExactString::Computed(_)))
        ));
        let StringConstructionKnowledge::Bounded(bound) =
            result.facts.string_construction(program, operation)
        else {
            panic!("computed operand is already known");
        };
        assert_eq!(bound.utf16_units_upper_bound(), 3);
        assert_eq!(
            result.facts.string_construction(program, operation),
            StringConstructionKnowledge::Bounded(bound)
        );
        assert_eq!(session.work().computations, 1);
    });
}

#[test]
fn bounded_string_knowledge_uses_owned_facts_even_when_result_materialization_truncates() {
    use super::publication::{CheckpointLimit, Compilation, LocalFactsRequest};

    let left = "a".repeat(1000);
    let source = format!("print(\"{left}\"+\"{left}\");");
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, &source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let (unit, operation, value) = addition(&program);
        let revision = program.units[unit.index()].revision();
        let operations_before = format!("{:?}", program.unit(unit).unwrap().operations);
        let mut compiler = Compilation::new(ledger(), CheckpointLimit { max_live: 2 }).unwrap();
        let snapshot = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(
                CacheLimits {
                    entries: 4,
                    bytes: 100_000,
                    result_bytes: 50_000,
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        let memory = compiler.ledger().retained_bytes();
        let mut weak_receipt = None;
        let mut weak_bound = None;
        let mut weak_cache_bytes = 0;
        for (result_bytes, warm) in [(2000, false), (2000, true), (50_000, false)] {
            let before = compiler.ledger().work_used(domain);
            let (bound, receipt, work) = compiler
                .with_local_facts(domain, 1, |group| {
                    let (bound, receipt) = {
                        let result = group
                            .query(
                                snapshot,
                                unit,
                                LocalFactsRequest {
                                    work_quota: 100_000,
                                    result_bytes,
                                },
                            )
                            .unwrap();
                        assert_eq!(result.cache_hit(), warm);
                        assert_eq!(result.string(value).is_some(), result_bytes == 50_000);
                        let StringConstructionKnowledge::Bounded(bound) =
                            result.string_construction(operation)
                        else {
                            panic!("operand knowledge does not require a computed result");
                        };
                        assert_eq!(bound.utf16_units_upper_bound(), 2000);
                        assert_eq!(
                            result.string_construction(operation),
                            StringConstructionKnowledge::Bounded(bound)
                        );
                        assert_eq!(result.string(value).is_some(), result_bytes == 50_000);
                        assert!(!result.facts().effects(operation).may_throw);
                        assert!(result.facts().effects(operation).may_exhaust_resources);
                        (bound, result.receipt())
                    };
                    (bound, receipt, group.work())
                })
                .unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), memory);
            assert_eq!(
                compiler.ledger().work_used(domain) - before,
                receipt.logical_work
            );
            assert_eq!(work.queries, 1);
            assert_eq!(work.computations, usize::from(!warm));
            assert_eq!(work.cache_hits, usize::from(warm));
            let status = compiler.local_facts_status().unwrap();
            if result_bytes == 2000 {
                assert_eq!(receipt.completion, AnalysisCompletion::Truncated);
                if warm {
                    assert_eq!(Some(receipt), weak_receipt);
                    assert_eq!(Some(bound), weak_bound);
                    assert_eq!(status.retained_bytes, weak_cache_bytes);
                    assert_eq!(status.entries, 1);
                } else {
                    weak_receipt = Some(receipt);
                    weak_bound = Some(bound);
                    weak_cache_bytes = status.retained_bytes;
                }
            } else {
                assert_eq!(receipt.completion, AnalysisCompletion::Complete);
                assert_eq!(Some(bound), weak_bound);
                assert_eq!(status.entries, 2);
            }
        }
        let view = compiler.view(snapshot).unwrap();
        assert_eq!(view.unit_revision(unit), Some(revision));
        assert_eq!(
            format!("{:?}", view.unit(unit).unwrap().operations),
            operations_before
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    }
}

#[test]
fn bounded_string_construction_rejects_stale_unknown_and_non_string_evidence() {
    checked(r#"print("a"+"b");"#, |program| {
        let (unit, operation, _) = addition(program);
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 2).unwrap();
        let exhausted = session.query(program, unit, request(0)).unwrap();
        assert_eq!(exhausted.receipt.completion, AnalysisCompletion::Truncated);
        assert!(matches!(
            exhausted.facts.string_construction(program, operation),
            StringConstructionKnowledge::Unknown(StringConstructionUnknown::OperandUnknown {
                reason: UnknownReason::Unvisited,
                ..
            })
        ));
        let result = session.query(program, unit, request(10_000)).unwrap();
        let StringConstructionKnowledge::Bounded(bound) =
            result.facts.string_construction(program, operation)
        else {
            panic!("exact source strings");
        };
        let mut edited = program.clone();
        let mut working = edited.units[unit.index()].clone().into_working();
        let _ = working.get_mut();
        edited.units[unit.index()] = working.freeze();
        let mut tables = program.clone();
        tables.tables_revision = RevisionId::fresh();
        for changed in [&edited, &tables] {
            assert!(!bound.dependencies().valid_for(changed));
            assert_eq!(
                result.facts.string_construction(changed, operation),
                StringConstructionKnowledge::Unknown(StringConstructionUnknown::StaleDependencies)
            );
        }
        assert_eq!(
            result
                .facts
                .string_construction(program, OpId::from_index(99_999).unwrap()),
            StringConstructionKnowledge::Unknown(StringConstructionUnknown::UnknownOperation)
        );
        let constant = program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .position(|op| matches!(op.kind, OperationKind::Constant(_)))
            .unwrap();
        assert_eq!(
            result
                .facts
                .string_construction(program, OpId::from_index(constant).unwrap()),
            StringConstructionKnowledge::Unknown(StringConstructionUnknown::UnsupportedOperation)
        );
    });
    for (source, unknown) in [
        (r#"string word(string input){return input+"tail";}"#, true),
        (r#"extern JsValue input;print(input+"tail");"#, true),
        ("print(1.5+2.5);", false),
    ] {
        checked(source, |program| {
            let (unit, operation, _) = addition(program);
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let result = session.query(program, unit, request(10_000)).unwrap();
            let knowledge = result.facts.string_construction(program, operation);
            if unknown {
                assert!(matches!(
                    knowledge,
                    StringConstructionKnowledge::Unknown(
                        StringConstructionUnknown::OperandUnknown { .. }
                    )
                ));
            } else {
                assert!(matches!(
                    knowledge,
                    StringConstructionKnowledge::Unknown(
                        StringConstructionUnknown::OperandNotString(_)
                    )
                ));
            }
        });
    }
}

#[test]
fn cheap_effect_transfer_preserves_initialization_host_and_construction_obligations() {
    checked(
        r#"extern Record<int> host();int value=2;value+=1;
        print(value*3);print("x"+"y");print("x".length);print(host().item??0);"#,
        |program| {
            let unit_id = program.initialization[0];
            let unit = program.unit(unit_id).unwrap();
            let mut domains = vec![false; unit.values.len()];
            let mut seen = [false; 6];
            for operation in &unit.operations {
                let effects = operation_evaluation_behavior(
                    program, None, unit_id, unit, operation, &domains,
                );
                match operation.kind {
                    OperationKind::Load(place) => match unit.places[place.index()] {
                        Place::Cell(cell) if program.cells[cell.index()].name == "value" => {
                            seen[0] = true;
                            assert_eq!(effects.reads, MemoryAccess::Cell(cell));
                            assert!(effects.may_throw, "primitive type is not TDZ evidence");
                        }
                        // A record the host returned may hold an accessor:
                        // reading it can run a hook, as a conversion can.
                        Place::Member { .. } => {
                            seen[1] = true;
                            assert_eq!(effects, EvaluationBehavior::COERCION);
                        }
                        _ => {}
                    },
                    OperationKind::IntBinary(_) => {
                        seen[2] = true;
                        assert_eq!(effects, EvaluationBehavior::COERCION);
                        assert_eq!(
                            operation_evaluation_behavior(
                                program,
                                None,
                                unit_id,
                                unit,
                                operation,
                                &[]
                            ),
                            EvaluationBehavior::COERCION
                        );
                    }
                    OperationKind::Binary(BinaryOp::Add) => {
                        seen[3] = true;
                        assert_eq!(
                            effects,
                            EvaluationBehavior {
                                may_exhaust_resources: true,
                                ..EvaluationBehavior::TOTAL
                            }
                        );
                    }
                    // The length of a proven primitive string reads nothing.
                    OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                        crate::primitive::Intrinsic::StringLength,
                    )) => {
                        seen[4] = true;
                        assert_eq!(effects, EvaluationBehavior::TOTAL);
                    }
                    OperationKind::Call(call)
                        if matches!(
                            unit.calls[call.index()].target,
                            CallTarget::Builtin(crate::check::BuiltinCall::Print)
                        ) =>
                    {
                        // Printing writes host output.
                        assert!(effects.requires_evaluation());
                        assert_eq!(effects.writes, MemoryAccess::Unknown);
                    }
                    OperationKind::Call(_) => {
                        seen[5] = true;
                        assert_eq!(effects, EvaluationBehavior::UNKNOWN);
                    }
                    _ => {}
                }
                if let Some(result) = operation.result {
                    domains[result.index()] =
                        primitive_result_domain(program, unit, operation, &domains);
                }
            }
            assert_eq!(seen, [true; 6]);
        },
    );
}

#[test]
fn copy_transfer_is_effect_free_but_primitive_knowledge_needs_an_operand_proof() {
    checked("print(-1);", |program| {
        let unit_id = program.initialization[0];
        let mut edited = program.clone();
        let mut working = edited.units[unit_id.index()].clone().into_working();
        let operation = working
            .get_mut()
            .operations
            .iter_mut()
            .find(|op| matches!(op.kind, OperationKind::Unary { .. }))
            .unwrap();
        // CopyValue is a valid core operation for primitives although source
        // conversion currently introduces it only for aggregate transfers.
        operation.kind = OperationKind::CopyValue;
        edited.units[unit_id.index()] = working.freeze();
        edited.verify().unwrap();
        let unit = edited.unit(unit_id).unwrap();
        let mut domains = vec![false; unit.values.len()];
        let mut copy = None;
        for (index, op) in unit.operations.iter().enumerate() {
            if matches!(op.kind, OperationKind::CopyValue) {
                copy = Some(OpId::from_index(index).unwrap());
                assert_eq!(
                    operation_evaluation_behavior(&edited, None, unit_id, unit, op, &domains),
                    EvaluationBehavior::TOTAL
                );
                assert_eq!(
                    operation_evaluation_behavior(&edited, None, unit_id, unit, op, &[]),
                    EvaluationBehavior::TOTAL
                );
                assert!(primitive_result_domain(&edited, unit, op, &domains));
                assert!(!primitive_result_domain(&edited, unit, op, &[]));
            }
            if let Some(result) = op.result {
                domains[result.index()] = primitive_result_domain(&edited, unit, op, &domains);
            }
        }
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
        let result = session.query(&edited, unit_id, request(10_000)).unwrap();
        assert_eq!(
            result.facts.effects(copy.unwrap()),
            EvaluationBehavior::TOTAL
        );
        assert_eq!(
            result
                .facts
                .exact(unit.operations[copy.unwrap().index()].result.unwrap()),
            Some(ExactValue::Integer(1))
        );
    });
    checked(
        "struct Pair{int x;}Pair state=Pair{1};Pair alias=state;",
        |program| {
            let unit = program.unit(program.initialization[0]).unwrap();
            let mut domains = vec![false; unit.values.len()];
            let mut copies = 0;
            for op in &unit.operations {
                if matches!(op.kind, OperationKind::CopyValue) {
                    copies += 1;
                    assert!(!primitive_result_domain(program, unit, op, &domains));
                    assert_eq!(
                        operation_evaluation_behavior(
                            program,
                            None,
                            program.initialization[0],
                            unit,
                            op,
                            &domains
                        ),
                        EvaluationBehavior {
                            may_exhaust_resources: true,
                            ..EvaluationBehavior::TOTAL
                        }
                    );
                }
                if let Some(result) = op.result {
                    domains[result.index()] = primitive_result_domain(program, unit, op, &domains);
                }
            }
            assert!(copies > 0);
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let result = session
                .query(program, program.initialization[0], request(10_000))
                .unwrap();
            for op in &unit.operations {
                if matches!(op.kind, OperationKind::CopyValue) {
                    assert!(result.facts.exact(op.result.unwrap()).is_none());
                }
            }
        },
    );
}

#[test]
fn exact_primitive_copy_reuses_borrowed_string_knowledge_and_keeps_unknown_inputs_unknown() {
    for (source, known) in [
        (r#"print(("\ud800"+"x")+"tail");"#, true),
        (r#"string copy(string input){return input+"tail";}"#, false),
    ] {
        checked(source, |program| {
            let frozen = program
                .units
                .iter()
                .find(|unit| {
                    unit.data()
                        .operations
                        .iter()
                        .any(|op| matches!(op.kind, OperationKind::Binary(BinaryOp::Add)))
                })
                .unwrap();
            let unit_id = frozen.id();
            let index = frozen
                .data()
                .operations
                .iter()
                .rposition(|op| matches!(op.kind, OperationKind::Binary(BinaryOp::Add)))
                .unwrap();
            let original = &frozen.data().operations[index];
            let input = frozen.data().operands(original.operands).unwrap()[0];
            let value = original.result.unwrap();
            let mut edited = program.clone();
            let mut working = edited.units[unit_id.index()].clone().into_working();
            working.get_mut().operations[index].kind = OperationKind::CopyValue;
            working.get_mut().operations[index].operands.len = 1;
            edited.units[unit_id.index()] = working.freeze();
            edited.verify().unwrap();
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let result = session.query(&edited, unit_id, request(10_000)).unwrap();
            assert_eq!(result.receipt.completion, AnalysisCompletion::Complete);
            if known {
                let input = result.facts.string(&edited, input).unwrap();
                let copy = result.facts.string(&edited, value).unwrap();
                assert!(
                    std::ptr::eq(input, copy),
                    "CopyValue retains existing immutable cache storage"
                );
                assert_eq!(copy.code_units().collect::<Vec<_>>(), [0xd800, 120]);
            } else {
                assert!(result.facts.exact(input).is_none());
                assert!(result.facts.exact(value).is_none());
            }
        });
    }
}

#[test]
fn old_fact_algorithm_receipts_cannot_request_new_transfer_answers() {
    checked("print(-1);", |program| {
        let mut cache = cache();
        let mut ledger = ledger();
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let mut old = request(10_000);
            old.attempt.algorithm_version = LOCAL_FACTS_VERSION - 1;
            assert!(matches!(
                session.query(program, program.initialization[0], old),
                Err(FactsError::InvalidAttempt)
            ));
            assert_eq!(session.work().queries, 1);
            assert_eq!(session.work().computations, 0);
            assert_eq!(session.work().executed_fact_steps, 0);
        }
        assert_eq!(cache.retained_entries(), 0);
        assert_eq!(ledger.work_used(WorkDomain::Baseline), 0);
    });
}

#[test]
fn unused_fresh_allocations_can_drop_but_cannot_duplicate_or_erase_operand_effects() {
    checked(
        r#"extern int effect();auto array=[effect()];
        Record<int> recordValue=record{x:effect()};auto plain=object{x:effect()};
        auto callable=()=>effect();"#,
        |program| {
            let unit_id = program.initialization[0];
            let unit = program.unit(unit_id).unwrap();
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let result = session.query(program, unit_id, request(10_000)).unwrap();
            let mut allocations = [false; 4];
            let mut operand_calls = 0;
            for (index, operation) in unit.operations.iter().enumerate() {
                let id = OpId::from_index(index).unwrap();
                let slot = match operation.kind {
                    OperationKind::Allocate {
                        kind: AllocationKind::Array,
                        ..
                    } => Some(0),
                    OperationKind::Allocate {
                        kind: AllocationKind::Record(_),
                        ..
                    } => Some(1),
                    OperationKind::Allocate {
                        kind: AllocationKind::Object(_),
                        ..
                    } => Some(2),
                    OperationKind::Closure(_) => Some(3),
                    _ => None,
                };
                if let Some(slot) = slot {
                    allocations[slot] = true;
                    let behavior = result.facts.effects(id);
                    assert!(behavior.creates_identity && behavior.may_exhaust_resources);
                    assert!(!behavior.may_throw);
                    assert!(!behavior.requires_evaluation());
                    assert_eq!(
                        result.facts.can_drop(id, ObservationDemand::Discarded),
                        Legality::PermittedUnderContext
                    );
                    assert_ne!(
                        result.facts.can_drop(id, ObservationDemand::Exact),
                        Legality::PermittedUnderContext
                    );
                    assert_ne!(
                        result.facts.can_duplicate(id),
                        Legality::PermittedUnderContext
                    );
                    assert_ne!(
                        result.facts.can_speculate(
                            id,
                            SpeculationContext {
                                operands_available: true,
                            },
                        ),
                        Legality::PermittedUnderContext
                    );
                }
                if matches!(operation.kind, OperationKind::Call(_)) {
                    operand_calls += 1;
                    assert!(result.facts.effects(id).requires_evaluation());
                    assert_ne!(
                        result.facts.can_drop(id, ObservationDemand::Discarded),
                        Legality::PermittedUnderContext
                    );
                }
            }
            assert_eq!(allocations, [true; 4]);
            assert_eq!(operand_calls, 3);
        },
    );
}

#[test]
fn resource_exclusion_preserves_coercion_invalid_arguments_and_explicit_errors() {
    checked(
        r#"extern JsValue dynamic;extern int host();
        dynamic+"suffix";"x".repeat(-1);host();
        struct Pair{int x;}Pair pair=Pair{1};throw 7;"#,
        |program| {
            let unit_id = program.initialization[0];
            let unit = program.unit(unit_id).unwrap();
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            let result = session.query(program, unit_id, request(10_000)).unwrap();
            let mut coercion = false;
            let mut repeat = false;
            let mut host = false;
            let mut nominal = false;
            let mut explicit_throw = false;
            for (index, operation) in unit.operations.iter().enumerate() {
                let tracked = match operation.kind {
                    OperationKind::Binary(BinaryOp::Add) => {
                        coercion = true;
                        true
                    }
                    OperationKind::Call(call) => {
                        if matches!(
                            unit.calls[call.index()].target,
                            CallTarget::Intrinsic {
                                operation: ResolvedIntrinsic::Method(
                                    crate::primitive::Intrinsic::StringRepeat
                                ),
                                ..
                            }
                        ) {
                            repeat = true;
                        } else {
                            host = true;
                        }
                        true
                    }
                    OperationKind::Allocate {
                        kind: AllocationKind::Struct(_),
                        ..
                    } => {
                        nominal = true;
                        let id = OpId::from_index(index).unwrap();
                        assert_eq!(
                            result.facts.effects(id),
                            EvaluationBehavior {
                                may_exhaust_resources: true,
                                ..EvaluationBehavior::TOTAL
                            }
                        );
                        assert_eq!(
                            result.facts.can_duplicate(id),
                            Legality::PermittedUnderContext
                        );
                        false
                    }
                    OperationKind::Throw => {
                        explicit_throw = true;
                        true
                    }
                    _ => false,
                };
                if tracked {
                    let id = OpId::from_index(index).unwrap();
                    assert!(result.facts.effects(id).may_throw);
                    assert!(result.facts.effects(id).requires_evaluation());
                    assert_ne!(
                        result.facts.can_drop(id, ObservationDemand::Discarded),
                        Legality::PermittedUnderContext
                    );
                    assert_ne!(
                        result.facts.can_duplicate(id),
                        Legality::PermittedUnderContext
                    );
                }
            }
            assert!(coercion && repeat && host && nominal && explicit_throw);
        },
    );
}

#[test]
fn no_write_throw_and_divergence_are_not_drop_or_speculation_proofs() {
    checked(
        r#"int spin(){while(true){}return 0;}try{throw 3;}catch{}"#,
        |program| {
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 8).unwrap();
            let mut saw_throw = false;
            let mut saw_loop = false;
            for unit in program.units() {
                let result = session.query(program, unit.id(), request(10_000)).unwrap();
                for (index, op) in unit.data().operations.iter().enumerate() {
                    let id = OpId::from_index(index).unwrap();
                    let effects = result.facts.effects(id);
                    if matches!(op.kind, OperationKind::Throw) {
                        saw_throw = true;
                        assert_eq!(effects.writes, MemoryAccess::None);
                        assert!(effects.may_throw);
                        assert_ne!(
                            result.facts.can_drop(id, ObservationDemand::Discarded),
                            Legality::PermittedUnderContext
                        );
                        assert_ne!(
                            result.facts.can_speculate(
                                id,
                                SpeculationContext {
                                    operands_available: true
                                }
                            ),
                            Legality::PermittedUnderContext
                        );
                    }
                    if matches!(op.kind, OperationKind::Loop { .. }) {
                        saw_loop = true;
                        assert!(effects.may_diverge);
                        assert_ne!(
                            result.facts.can_drop(id, ObservationDemand::Discarded),
                            Legality::PermittedUnderContext
                        );
                    }
                }
            }
            assert!(saw_throw && saw_loop);
        },
    );
}

#[test]
fn getters_unknown_calls_and_unproved_lexical_initialization_stay_conservative() {
    checked(
        r#"extern Record<int> object();int local=1;print(local);print(object().value??0);"#,
        |program| {
            let unit = program.initialization[0];
            let data = program.unit(unit).unwrap();
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
            let result = session.query(program, unit, request(10_000)).unwrap();
            let mut getter = false;
            let mut call = false;
            let mut lexical = false;
            for (index, operation) in data.operations.iter().enumerate() {
                let op = OpId::from_index(index).unwrap();
                let effects = result.facts.effects(op);
                match operation.kind {
                    OperationKind::Load(place)
                        if matches!(data.places[place.index()], Place::Member { .. }) =>
                    {
                        getter = true;
                        assert!(effects.may_reenter && effects.may_throw);
                        assert_eq!(effects.writes, MemoryAccess::Unknown);
                    }
                    OperationKind::Load(place) if matches!(data.places[place.index()],Place::Cell(cell) if program.cells[cell.index()].binding!=CellBinding::Foreign) =>
                    {
                        // `print(local)` follows `int local=1` in the same
                        // region, so the temporal dead zone is excluded and the
                        // unused read is droppable. Reads elsewhere stay
                        // conservative (see the companion test below).
                        lexical = true;
                        assert!(!effects.may_throw);
                        assert_eq!(
                            result.facts.can_drop(op, ObservationDemand::Discarded),
                            Legality::PermittedUnderContext
                        );
                    }
                    // Printing writes host output; the unknown host call
                    // may do anything.
                    OperationKind::Call(target)
                        if matches!(
                            data.calls[target.index()].target,
                            CallTarget::Builtin(crate::check::BuiltinCall::Print)
                        ) =>
                    {
                        assert!(effects.requires_evaluation());
                    }
                    OperationKind::Call(_) => {
                        call = true;
                        assert!(effects.may_diverge && effects.may_reenter);
                    }
                    _ => {}
                }
            }
            assert!(getter && call && lexical);
        },
    );
}

#[test]
fn demand_and_operand_availability_are_separate_legality_inputs() {
    checked("print(1+2);", |program| {
        let unit = program.initialization[0];
        let (index, operation) = program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .enumerate()
            .find(|(_, op)| matches!(op.kind, OperationKind::IntBinary(_)))
            .unwrap();
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
        let result = session.query(program, unit, request(10_000)).unwrap();
        let op = OpId::from_index(index).unwrap();
        assert_eq!(
            result.facts.exact(operation.result.unwrap()),
            Some(ExactValue::Integer(3))
        );
        assert_eq!(
            result.facts.can_drop(op, ObservationDemand::Discarded),
            Legality::PermittedUnderContext
        );
        for demand in [
            ObservationDemand::Exact,
            ObservationDemand::Truthy,
            ObservationDemand::Nullish,
        ] {
            assert_ne!(
                result.facts.can_drop(op, demand),
                Legality::PermittedUnderContext
            );
        }
        assert_ne!(
            result.facts.can_speculate(
                op,
                SpeculationContext {
                    operands_available: false
                }
            ),
            Legality::PermittedUnderContext
        );
        assert_eq!(
            result.facts.can_speculate(
                op,
                SpeculationContext {
                    operands_available: true
                }
            ),
            Legality::PermittedUnderContext
        );
    });
}

#[test]
fn weak_strong_retries_and_warm_hits_keep_the_same_attempt_results_and_charges() {
    checked(r#"print("left"+"right");"#, |program| {
        let (unit, _, value) = addition(program);
        let data = program.unit(unit).unwrap();
        let weak = request((data.values.len() + data.operations.len() + 3) as u64);
        let strong = request(10_000);
        let run = |cache: &mut FactsCache, attempt: FactRequest| {
            let mut ledger = ledger();
            let (known, receipt, hit) = {
                let mut session =
                    FactsSession::new(cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
                let result = session.query(program, unit, attempt).unwrap();
                (
                    result.facts.exact(value).is_some(),
                    result.receipt,
                    result.cache_hit,
                )
            };
            (known, receipt, hit, ledger.work_by_kind(WorkKind::Analysis))
        };
        let mut warm = cache();
        let low = run(&mut warm, weak);
        assert!(!low.0);
        assert_eq!(low.1.completion, AnalysisCompletion::Truncated);
        let high = run(&mut warm, strong);
        assert!(high.0);
        assert_eq!(high.1.completion, AnalysisCompletion::Complete);
        let low_warm = run(&mut warm, weak);
        assert!(low_warm.2);
        assert_eq!((low.0, low.1, low.3), (low_warm.0, low_warm.1, low_warm.3));
        let mut cold = cache();
        let high_cold = run(&mut cold, strong);
        assert_eq!(
            (high.0, high.1, high.3),
            (high_cold.0, high_cold.1, high_cold.3)
        );
        let high_warm = run(&mut warm, strong);
        assert!(high_warm.2);
        assert_eq!(high_warm.3, high_cold.3);
    });
}

#[test]
fn one_session_charges_each_dependency_attempt_once_and_releases_its_reservation() {
    checked("print(1+2);", |program| {
        let mut cache = cache();
        let mut ledger = ledger();
        let unit = program.initialization[0];
        let receipt = {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 2).unwrap();
            let receipt = session
                .query(program, unit, request(10_000))
                .unwrap()
                .receipt;
            assert!(
                session
                    .query(program, unit, request(10_000))
                    .unwrap()
                    .cache_hit
            );
            receipt
        };
        assert_eq!(
            ledger.work_by_kind(WorkKind::Analysis),
            receipt.logical_work
        );
        assert_eq!(ledger.retained_bytes(), 0);
        assert!(ledger.peak_retained_bytes() > 0);
    });
}

#[test]
fn repeated_eviction_has_the_same_query_cap_and_logical_bill_for_cold_and_warm_caches() {
    checked(
        "int first(){return 1+2;}int second(){return 4+5;}",
        |program| {
            let functions: Vec<_> = ["first", "second"]
                .into_iter()
                .map(|name| {
                    let unit = program
                        .cells
                        .iter()
                        .find_map(|cell| match cell.binding {
                            CellBinding::Function(unit) if cell.name == name => Some(unit),
                            _ => None,
                        })
                        .unwrap();
                    let value = program
                        .unit(unit)
                        .unwrap()
                        .operations
                        .iter()
                        .find(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
                        .unwrap()
                        .result
                        .unwrap();
                    (unit, value)
                })
                .collect();
            let run = |warm: bool, domain| {
                let mut ledger = ledger();
                let mut cache = RetainedFactsCache::new(
                    CacheLimits {
                        entries: 1,
                        bytes: 50_000,
                        result_bytes: 10_000,
                    },
                    &mut ledger,
                    WorkDomain::Baseline,
                )
                .unwrap();
                let mut req = request(10_000);
                req.result_bytes = 10_000;
                if warm {
                    let mut session = cache.session(&mut ledger, domain, 1).unwrap();
                    session.query(program, functions[0].0, req).unwrap();
                }
                let baseline_bytes = ledger.retained_bytes();
                let baseline_work = ledger.work_used(domain);
                let (receipts, work) = {
                    let mut session = cache.session(&mut ledger, domain, 4).unwrap();
                    let mut receipts = Vec::new();
                    for (index, which) in [0, 1, 0, 1].into_iter().enumerate() {
                        let (unit, value) = functions[which];
                        let result = session.query(program, unit, req).unwrap();
                        assert_eq!(
                            result.facts.exact(value),
                            Some(ExactValue::Integer(if which == 0 { 3 } else { 9 }))
                        );
                        assert_eq!(result.cache_hit, warm && index == 0);
                        receipts.push(result.receipt);
                        assert_eq!(session.work().queries, index + 1);
                    }
                    let work = session.work();
                    assert_eq!(work.queries, 4);
                    assert_eq!(work.cache_hits, usize::from(warm));
                    assert_eq!(work.computations, if warm { 3 } else { 4 });
                    assert_eq!(work.recomputations, 2);
                    let computed_steps = receipts
                        .iter()
                        .enumerate()
                        .filter(|(index, _)| !warm || *index != 0)
                        .map(|(_, receipt)| u128::from(receipt.logical_work))
                        .sum::<u128>();
                    assert_eq!(work.executed_fact_steps, computed_steps);
                    for which in [0, 1, 0] {
                        assert!(matches!(
                            session.query(program, functions[which].0, req),
                            Err(FactsError::QueryLimit)
                        ));
                        assert_eq!(session.work(), work);
                    }
                    (receipts, work)
                };
                assert_eq!(ledger.retained_bytes(), baseline_bytes);
                assert_eq!(cache.retained_entries(), 1);
                let charged = ledger.work_used(domain) - baseline_work;
                assert_eq!(charged, receipts[0].logical_work + receipts[1].logical_work);
                assert_eq!(receipts[0], receipts[2]);
                assert_eq!(receipts[1], receipts[3]);
                cache.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
                (receipts, charged, work)
            };
            for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
                let cold = run(false, domain);
                let warm = run(true, domain);
                assert_eq!((cold.0, cold.1), (warm.0, warm.1));
                assert_eq!(cold.2.queries, warm.2.queries);
                assert!(cold.2.executed_fact_steps > warm.2.executed_fact_steps);
            }
        },
    );
}

#[test]
fn cache_hits_consume_queries_even_when_the_attempt_was_already_billed() {
    checked("print(1+2);", |program| {
        let mut cache = cache();
        let mut ledger = ledger();
        let unit = program.initialization[0];
        let receipt = {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            session
                .query(program, unit, request(10_000))
                .unwrap()
                .receipt
        };
        let before = ledger.work_used(WorkDomain::Baseline);
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 3).unwrap();
            for count in 1..=3 {
                assert!(
                    session
                        .query(program, unit, request(10_000))
                        .unwrap()
                        .cache_hit
                );
                let work = session.work();
                assert_eq!(work.queries, count);
                assert_eq!(work.cache_hits, count);
                assert_eq!(work.computations, 0);
                assert_eq!(work.recomputations, 0);
                assert_eq!(work.executed_fact_steps, 0);
            }
            let work = session.work();
            assert!(matches!(
                session.query(program, unit, request(10_000)),
                Err(FactsError::QueryLimit)
            ));
            assert_eq!(session.work(), work);
        }
        assert_eq!(
            ledger.work_used(WorkDomain::Baseline) - before,
            receipt.logical_work
        );
        assert_eq!(ledger.retained_bytes(), 0);
    });
}

#[test]
fn invalid_attempt_and_unknown_unit_use_query_allowance_before_validation() {
    checked("print(1+2);", |program| {
        let mut cache = cache();
        let mut ledger = ledger();
        let unit = program.initialization[0];
        let missing = UnitId::from_index(program.units.len()).unwrap();
        let mut invalid = request(10_000);
        invalid.attempt.algorithm_version += 1;
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 2).unwrap();
            assert!(matches!(
                session.query(program, missing, request(10_000)),
                Err(FactsError::UnknownUnit)
            ));
            assert_eq!(session.work().queries, 1);
            assert!(matches!(
                session.query(program, unit, invalid),
                Err(FactsError::InvalidAttempt)
            ));
            let work = session.work();
            assert_eq!(work.queries, 2);
            assert_eq!(work.cache_hits, 0);
            assert_eq!(work.computations, 0);
            assert_eq!(work.recomputations, 0);
            assert_eq!(work.executed_fact_steps, 0);
            assert!(matches!(
                session.query(program, unit, request(10_000)),
                Err(FactsError::QueryLimit)
            ));
            assert!(matches!(
                session.query(program, missing, invalid),
                Err(FactsError::QueryLimit)
            ));
            assert_eq!(session.work(), work);
        }
        assert_eq!(cache.retained_entries(), 0);
        assert_eq!(ledger.work_used(WorkDomain::Baseline), 0);
        assert_eq!(ledger.retained_bytes(), 0);
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 0).unwrap();
            assert!(matches!(
                session.query(program, missing, invalid),
                Err(FactsError::QueryLimit)
            ));
            assert_eq!(session.work().queries, 0);
        }
    });
}

#[test]
fn changed_unit_and_owned_table_revisions_invalidate_cached_answers() {
    checked("print(1+2);", |program| {
        let unit = program.initialization[0];
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 8).unwrap();
        let deps = session
            .query(program, unit, request(10_000))
            .unwrap()
            .facts
            .dependencies();
        let mut edited = program.clone();
        let mut working = edited.units[unit.index()].clone().into_working();
        let _ = working.get_mut();
        edited.units[unit.index()] = working.freeze();
        assert!(!deps.valid_for(&edited));
        assert!(
            !session
                .query(&edited, unit, request(10_000))
                .unwrap()
                .cache_hit
        );
        let mut tables = program.clone();
        tables.tables_revision = RevisionId::fresh();
        assert!(!deps.valid_for(&tables));
        assert!(
            !session
                .query(&tables, unit, request(10_000))
                .unwrap()
                .cache_hit
        );
        assert!(deps.valid_for(program));
    });
}

#[test]
fn cache_and_query_count_remain_bounded_across_arbitrary_quotas() {
    checked("print(1+2);", |program| {
        let mut cache = FactsCache::new(CacheLimits {
            entries: 4,
            bytes: 50_000,
            result_bytes: 10_000,
        })
        .unwrap();
        let mut ledger = ledger();
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 6).unwrap();
            for quota in 0..6 {
                let mut req = request(quota);
                req.result_bytes = 10_000;
                session
                    .query(program, program.initialization[0], req)
                    .unwrap();
            }
            let mut req = request(7);
            req.result_bytes = 10_000;
            assert!(matches!(
                session.query(program, program.initialization[0], req),
                Err(FactsError::QueryLimit)
            ));
        }
        assert_eq!(cache.retained_entries(), 2);
        assert!(cache.retained_bytes() <= 50_000);
    });
}

#[test]
fn payload_memory_truncation_is_independent_of_cache_warmth() {
    let left = "a".repeat(1000);
    let source = format!("print(\"{left}\"+\"{left}\");");
    checked(&source, |program| {
        let (unit, _, value) = addition(program);
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 8).unwrap();
        let mut small = request(100_000);
        small.result_bytes = 2000;
        let result = session.query(program, unit, small).unwrap();
        assert_eq!(result.receipt.completion, AnalysisCompletion::Truncated);
        assert!(result.facts.exact(value).is_none());
        let low_receipt = result.receipt;
        let large = session.query(program, unit, request(100_000)).unwrap();
        assert!(large.facts.exact(value).is_some());
        let small_again = session.query(program, unit, small).unwrap();
        assert!(small_again.cache_hit);
        assert_eq!(small_again.receipt, low_receipt);
        assert!(small_again.facts.exact(value).is_none());
    });
}

#[test]
fn exhausted_work_is_rejected_before_cold_analysis_or_cache_population() {
    checked("print(1+2);", |program| {
        let mut cache = cache();
        let mut ledger = BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 0,
                optional_work: 0,
                baseline_retained_bytes: 2_000_000,
                retained_bytes: 4_000_000,
            },
        )
        .unwrap();
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 1).unwrap();
            assert!(matches!(
                session.query(program, program.initialization[0], request(10_000)),
                Err(FactsError::Budget(_))
            ));
            let work = session.work();
            assert_eq!(work.queries, 1);
            assert_eq!(work.computations, 0);
            assert_eq!(work.cache_hits, 0);
            assert_eq!(work.executed_fact_steps, 0);
            assert!(matches!(
                session.query(program, program.initialization[0], request(10_000)),
                Err(FactsError::QueryLimit)
            ));
            assert_eq!(session.work(), work);
        }
        assert_eq!(cache.retained_entries(), 0);
        assert_eq!(ledger.work_by_kind(WorkKind::Analysis), 0);
    });
}

#[test]
fn nullable_capture_refinement_does_not_make_later_length_evaluation_total() {
    checked(
        r#"
        func()->int make(){string? value="ok";
            if(value!=null){auto read=()=>{value.length;return value.length;};value=null;return read;}
            return ()=>0;
        }print(make()());
    "#,
        |program| {
            assert!(program.cells.iter().any(|cell| cell.name == "value"
                && matches!(program.types[cell.ty.index()], Type::Nullable(_))));
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 8).unwrap();
            let mut lengths = 0;
            for unit in program.units() {
                let result = session.query(program, unit.id(), request(10_000)).unwrap();
                for (index, op) in unit.data().operations.iter().enumerate() {
                    if matches!(
                        op.kind,
                        OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                            crate::primitive::Intrinsic::StringLength
                        ))
                    ) {
                        lengths += 1;
                        let id = OpId::from_index(index).unwrap();
                        assert!(result.facts.effects(id).may_throw);
                        assert_ne!(
                            result.facts.can_drop(id, ObservationDemand::Discarded),
                            Legality::PermittedUnderContext
                        );
                        assert_ne!(
                            result.facts.can_speculate(
                                id,
                                SpeculationContext {
                                    operands_available: true
                                }
                            ),
                            Legality::PermittedUnderContext
                        );
                    }
                }
            }
            assert_eq!(lengths, 2);
        },
    );
}

#[test]
fn exact_string_length_can_be_total_without_trusting_a_refined_load_type() {
    checked(r#"print("\ud800x".length);"#, |program| {
        let unit = program.initialization[0];
        let mut cache = cache();
        let mut ledger = ledger();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
        let result = session.query(program, unit, request(10_000)).unwrap();
        for (index, op) in program.unit(unit).unwrap().operations.iter().enumerate() {
            if matches!(
                op.kind,
                OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                    crate::primitive::Intrinsic::StringLength
                ))
            ) {
                assert_eq!(
                    result.facts.exact(op.result.unwrap()),
                    Some(ExactValue::Integer(2))
                );
                assert_eq!(
                    result.facts.can_drop(
                        OpId::from_index(index).unwrap(),
                        ObservationDemand::Discarded
                    ),
                    Legality::PermittedUnderContext
                );
            }
        }
    });
}

#[test]
fn refined_nullable_numeric_load_does_not_supply_an_unconditional_operand_domain() {
    checked(
        r#"int? value=1;if(value!=null){auto read=()=>value+2;value=null;print(read());}"#,
        |program| {
            let mut cache = cache();
            let mut ledger = ledger();
            let mut found = false;
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 8).unwrap();
            for unit in program.units() {
                let result = session.query(program, unit.id(), request(10_000)).unwrap();
                for (index, op) in unit.data().operations.iter().enumerate() {
                    if matches!(op.kind, OperationKind::IntBinary(_)) {
                        found = true;
                        assert_ne!(
                            result.facts.can_drop(
                                OpId::from_index(index).unwrap(),
                                ObservationDemand::Discarded
                            ),
                            Legality::PermittedUnderContext
                        );
                    }
                }
            }
            assert!(found);
        },
    );
}

#[test]
fn global_cache_bound_and_unrelated_unit_reuse_hold_across_many_units() {
    let source = (0..8)
        .map(|index| format!("int f{index}(){{return {index};}}"))
        .collect::<String>();
    checked(&source, |program| {
        let mut cache = FactsCache::new(CacheLimits {
            entries: 3,
            bytes: 50_000,
            result_bytes: 10_000,
        })
        .unwrap();
        let mut ledger = ledger();
        let mut req = request(10_000);
        req.result_bytes = 10_000;
        {
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 16).unwrap();
            for unit in program.units() {
                session.query(program, unit.id(), req).unwrap();
            }
        }
        assert_eq!(cache.retained_entries(), 3);
        assert!(cache.retained_bytes() <= 50_000);
        let untouched = program.units.last().unwrap().id();
        let mut edited = program.clone();
        let mut change = edited.units[0].clone().into_working();
        let _ = change.get_mut();
        edited.units[0] = change.freeze();
        let mut session =
            FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
        let result = session.query(&edited, untouched, req).unwrap();
        assert!(result.cache_hit);
        assert!(result.facts.dependencies().valid_for(&edited));
        assert!(result.facts.dependencies().valid_for(program));
    });
}

#[test]
fn a_local_read_from_another_unit_keeps_its_temporal_dead_zone_risk() {
    // The function body is a different unit with no `Initialize` of `value`;
    // nothing there proves the read follows initialization, so it may throw
    // and an unused copy of it cannot be dropped.
    checked(
        "int value=1;int read(){return value;}print(read());",
        |program| {
            let read = program
                .cells
                .iter()
                .find_map(|cell| match cell.binding {
                    CellBinding::Function(unit) if cell.name == "read" => Some(unit),
                    _ => None,
                })
                .unwrap();
            let data = program.unit(read).unwrap();
            let mut cache = cache();
            let mut ledger = ledger();
            let mut session =
                FactsSession::new(&mut cache, &mut ledger, WorkDomain::Baseline, 4).unwrap();
            let result = session.query(program, read, request(10_000)).unwrap();
            let mut loads = 0;
            for (index, operation) in data.operations.iter().enumerate() {
                if let OperationKind::Load(place) = operation.kind {
                    if matches!(data.places[place.index()], Place::Cell(_)) {
                        loads += 1;
                        let op = OpId::from_index(index).unwrap();
                        assert!(result.facts.effects(op).may_throw);
                        assert_ne!(
                            result.facts.can_drop(op, ObservationDemand::Discarded),
                            Legality::PermittedUnderContext
                        );
                    }
                }
            }
            assert!(loads > 0, "expected a cell read in `read`");
        },
    );
}
