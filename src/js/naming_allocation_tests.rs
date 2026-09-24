//! Ownership/admission tests for the actual verifier, naming and printer path.
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::output_budget::RetainedCharge;

fn policy() -> ResolvedPolicy {
    crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn ledger(memory: u64, work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}
fn fixture() -> Module {
    let mut module = Module::default();
    let state = module.binding(Binding {
        source_symbol: Some(SymbolId(0)),
        scope: ScopeId::new(0),
        spelling: "retainedState".into(),
        pinned: false,
    });
    let reader = module.binding(Binding {
        source_symbol: Some(SymbolId(1)),
        scope: ScopeId::new(0),
        spelling: "readState".into(),
        pinned: false,
    });
    let body = module.region(ScopeId::new(0));
    let parameter = module.binding(Binding {
        source_symbol: Some(SymbolId(2)),
        scope: module.regions[body.index()].scope,
        spelling: "unusedInput".into(),
        pinned: false,
    });
    let initial = module.expression(Expr::Literal(Literal::Number(7.0)), None);
    let read = module.expression(Expr::Binding(state), None);
    module.regions[body.index()]
        .statements
        .push(Statement::Return(Some(read)));
    let function = FunctionId::new(0);
    module.functions.push(Function {
        parameters: vec![parameter],
        body,
        arrow: false,
        name: FunctionName::Exact("readState".into()),
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
    });
    module.regions[0].statements = vec![
        Statement::Let {
            binding: state,
            value: Some(initial),
        },
        Statement::Function {
            binding: reader,
            function,
        },
    ];
    module.exports = vec![Export {
        binding: reader,
        name: "read".into(),
    }];
    module
}
fn discard(text: String, charge: RetainedCharge<u64>, ledger: &mut BudgetLedger) {
    drop(text);
    charge.discard(&17, ledger).unwrap();
}

#[test]
fn prepared_output_matches_all_inspection_styles_and_transfers_only_artifact_capacity() {
    let module = fixture();
    let policy = policy();
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        for style in [Style::Global, Style::Scoped, Style::Source] {
            let plan = Plan::new(style);
            let expected = module
                .prepare_output_with_policy(&policy)
                .unwrap()
                .render(&plan)
                .unwrap();
            let mut ledger = ledger(1_000_000, 1_000_000);
            let (text, charge) = {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                let output = module
                    .prepare_output_admitted(&policy, &mut budget)
                    .unwrap();
                assert!(output.render(&plan).unwrap_err().contains("artifact owner"));
                output.render_admitted(&plan, usize::MAX, 17).unwrap()
            };
            assert_eq!(text, expected);
            assert_eq!(charge.bytes(), text.capacity() as u64);
            assert_eq!(charge.domain(), domain);
            assert_eq!(ledger.retained_bytes(), charge.bytes());
            assert!(ledger.work_used(domain) > 0);
            discard(text, charge, &mut ledger);
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }
}

#[test]
fn installed_lazy_caches_survive_failed_prints_and_release_with_output() {
    let module = fixture();
    let policy = policy();
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_admitted(&policy, &mut budget)
            .unwrap();
        let initial =
            output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained));
        let candidates = output.source_candidates_admitted().unwrap();
        assert!(!candidates.is_empty());
        let candidate_pointer = candidates.as_ptr();
        let after_candidates =
            output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained));
        assert!(after_candidates > initial);
        assert_eq!(
            output.source_candidates_admitted().unwrap().as_ptr(),
            candidate_pointer
        );
        assert_eq!(
            output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained)),
            after_candidates
        );
        for attempt in 0..2 {
            assert!(matches!(
                output.render_admitted(&Plan::new(Style::Scoped), 0, 17u64),
                Err(OutputError::ByteLimit)
            ));
            assert!(output.basis.scoped.get().is_some());
            assert_eq!(
                output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Scratch)),
                0
            );
            let retained =
                output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained));
            assert!(retained > after_candidates);
            if attempt == 0 {
                let again = output.render_admitted(&Plan::new(Style::Scoped), 0, 17u64);
                assert!(matches!(again, Err(OutputError::ByteLimit)));
                assert_eq!(
                    output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained)),
                    retained
                );
            }
        }
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn partial_lazy_cache_admission_failure_does_not_install_or_leak_it() {
    let module = fixture();
    let policy = policy();
    const MEMORY: u64 = 1_000_000;
    let mut ledger = ledger(MEMORY, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_admitted(&policy, &mut budget)
            .unwrap();
        let before = output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained));
        // Admit the outer free-reference array, then fail its captured-binding
        // payload allocation. This models another live owner's reservation.
        let available = (module.scopes.len() * std::mem::size_of::<Vec<BindingId>>()) as u64;
        let pressure = MEMORY - before - available;
        output
            .with_allocation_budget(|b| b.retain(AllocationClass::Scratch, pressure))
            .unwrap();
        assert!(matches!(
            output.render_admitted(&Plan::new(Style::Scoped), 1_000_000, 17u64),
            Err(OutputError::Admission(AllocationError::Budget(_)))
        ));
        assert!(output.basis.scoped.get().is_none());
        assert_eq!(
            output.with_allocation_budget(|b| b.retained_bytes(AllocationClass::Retained)),
            before
        );
        output
            .with_allocation_budget(|b| b.release(AllocationClass::Scratch, pressure))
            .unwrap();
        assert!(matches!(
            output.render_admitted(&Plan::new(Style::Scoped), 0, 17u64),
            Err(OutputError::ByteLimit)
        ));
        assert!(output.basis.scoped.get().is_some());
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn preparation_exhaustion_and_invalid_target_release_every_partial_buffer() {
    let module = fixture();
    let policy = policy();
    let mut complete = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut complete, WorkDomain::Optional)));
        let output = module
            .prepare_output_admitted(&policy, &mut budget)
            .unwrap();
        drop(output);
    }
    let work = complete.work_used(WorkDomain::Optional);
    let memory = complete.peak_retained_bytes();
    assert!(work > 1 && memory > 1);
    for (memory, work) in [
        (memory - 1, 1_000_000),
        (1_000_000, work - 1),
        (1_000_000, 0),
    ] {
        let mut ledger = ledger(memory, work);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert!(matches!(
                module.prepare_output_admitted(&policy, &mut budget),
                Err(OutputError::Admission(AllocationError::Budget(_)))
            ));
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
    let mut invalid = module;
    let missing = invalid.expression(Expr::Binding(BindingId::new(400)), None);
    invalid.regions[0]
        .statements
        .push(Statement::Evaluate(missing));
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        assert!(matches!(
            invalid.prepare_output_admitted(&policy, &mut budget),
            Err(OutputError::Invalid("unknown binding"))
        ));
    }
    assert!(ledger.peak_retained_bytes() > 0);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn name_index_checks_exact_scope_and_name_even_in_a_collision_cluster() {
    use std::hash::{Hash, Hasher};
    let mut module = Module::default();
    let root = ScopeId::new(0);
    let child_region = module.region(root);
    let child = module.regions[child_region.index()].scope;
    let mut selected = Vec::new();
    for number in 0..100_000 {
        let name = format!("collision{number}");
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        Some(root).hash(&mut hash);
        name.hash(&mut hash);
        if hash.finish() & 31 == 0 {
            selected.push(name);
        }
        if selected.len() == 8 {
            break;
        }
    }
    assert_eq!(selected.len(), 8);
    selected.push(selected[0].clone());
    for (index, name) in selected.iter().enumerate() {
        module.binding(Binding {
            source_symbol: None,
            scope: if index == 8 { child } else { root },
            spelling: name.clone(),
            pinned: true,
        });
    }
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let mut index = NameIndex::new(selected.len(), &mut budget).unwrap();
        assert_eq!(index.slots.len(), 32);
        for (number, name) in selected.iter().enumerate() {
            let scope = module.bindings[number].scope;
            let slot = index
                .find(&module, &selected, Some(scope), name, &mut budget)
                .unwrap()
                .unwrap_err();
            index.slots[slot] = Some(BindingId::new(number));
        }
        for (number, name) in selected.iter().enumerate() {
            assert_eq!(
                index
                    .find(
                        &module,
                        &selected,
                        Some(module.bindings[number].scope),
                        name,
                        &mut budget
                    )
                    .unwrap(),
                Ok(BindingId::new(number))
            );
        }
        assert!(index
            .find(&module, &selected, Some(root), "absent", &mut budget)
            .unwrap()
            .is_err());
    }
    assert_eq!(ledger.retained_bytes(), 0);
    assert!(ledger.work_used(WorkDomain::Optional) > 8);
}

#[test]
fn large_scope_uses_single_name_payloads_and_fails_capacity_before_allocation() {
    let mut module = Module::default();
    for number in 0..4096 {
        let binding = module.binding(Binding {
            source_symbol: Some(SymbolId(number)),
            scope: ScopeId::new(0),
            spelling: format!("source{number}"),
            pinned: false,
        });
        module.regions[0].statements.push(Statement::Let {
            binding,
            value: None,
        });
    }
    let mut ledger = ledger(16_000_000, 16_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        assert!(matches!(
            NameIndex::new(usize::MAX, &mut budget),
            Err(OutputError::Admission(AllocationError::Capacity))
        ));
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        let structure = verify::verify_in(&module, &mut budget).unwrap();
        let basis = Basis::new_in(&module, &structure, &mut budget).unwrap();
        for style in [Style::Global, Style::Scoped, Style::Source] {
            let mut phase = budget.scope();
            let names = basis.names_in(&Plan::new(style), &mut phase).unwrap();
            let unique: std::collections::BTreeSet<_> =
                names.bindings.iter().map(String::as_str).collect();
            assert_eq!(unique.len(), 4096);
            for (id, name) in names.bindings.iter().enumerate() {
                assert_eq!(
                    names
                        .resolve_in(&module, ScopeId::new(0), name, &mut phase)
                        .unwrap(),
                    Some(BindingId::new(id))
                );
                if style == Style::Source {
                    assert_eq!(name, &module.bindings[id].spelling);
                }
            }
            drop(unique);
            drop(names);
            phase.finish_retained().unwrap();
        }
        drop(basis);
        drop(structure);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}
