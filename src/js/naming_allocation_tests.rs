//! Ownership/admission tests for the actual verifier, naming and printer path.
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::output_budget::RetainedCharge;

fn policy() -> ResolvedPolicy {
    toml::from_str::<crate::config::ProjectConfig>("[policy.tactics]\nnaming-compaction='on'")
        .unwrap()
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
        class: None,
        defined: false,
    });
    let reader = module.binding(Binding {
        source_symbol: Some(SymbolId(1)),
        scope: ScopeId::new(0),
        spelling: "readState".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let body = module.region(ScopeId::new(0));
    let parameter = module.binding(Binding {
        source_symbol: Some(SymbolId(2)),
        scope: module.regions[body.index()].scope,
        spelling: "unusedInput".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let initial = module.expression(Expr::Literal(Literal::Number(7.0)), None);
    let read = module.expression(Expr::Binding(state), None);
    module.regions[body.index()]
        .statements
        .push(Statement::Return(Some(read)));
    let function = FunctionId::new(0);
    module.functions.push(Function {
        rest: false,
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
fn local_read_order_shortens_hot_locals_without_capturing_or_renaming_other_scopes() {
    let mut module = fixture();
    let body = module.functions[0].body;
    let scope = module.regions[body.index()].scope;
    for index in 1..60 {
        let parameter = module.binding(Binding {
            source_symbol: Some(SymbolId((index + 2) as u32)),
            scope,
            spelling: format!("argument{index}"),
            pinned: false,
            class: None,
            defined: false,
        });
        module.functions[0].parameters.push(parameter);
    }
    let hot = *module.functions[0].parameters.last().unwrap();
    let mut sum = module.expression(Expr::Binding(BindingId::new(0)), None);
    for _ in 0..8 {
        let read = module.expression(Expr::Binding(hot), None);
        sum = module.expression(
            Expr::Binary {
                op: Binary::Add,
                left: sum,
                right: read,
            },
            None,
        );
    }
    module.regions[body.index()].statements = vec![Statement::Return(Some(sum))];
    // An unrelated sibling keeps its binding spellings even though the first
    // function's allocation order changes; the first still captures root state.
    let sibling_body = module.region(ScopeId::new(0));
    let sibling_local = module.binding(Binding {
        source_symbol: Some(SymbolId(100)),
        scope: module.regions[sibling_body.index()].scope,
        spelling: "siblingInput".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let sibling = module.binding(Binding {
        source_symbol: Some(SymbolId(101)),
        scope: ScopeId::new(0),
        spelling: "sibling".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let read = module.expression(Expr::Binding(sibling_local), None);
    module.regions[sibling_body.index()].statements = vec![Statement::Return(Some(read))];
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        rest: false,
        parameters: vec![sibling_local],
        body: sibling_body,
        arrow: false,
        name: FunctionName::Exact("sibling".into()),
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    module.regions[0].statements.push(Statement::Function {
        binding: sibling,
        function,
    });
    module.exports.push(Export {
        binding: sibling,
        name: "sibling".into(),
    });
    let original = Plan::new(Style::Scoped);
    let mut local = original.clone();
    local.local_read_order = true;
    let mut budget = AllocationBudget::new(None);
    let structure = verify::verify_in(&module, &mut budget).unwrap();
    let basis = naming::Basis::new_in(&module, &structure, &mut budget).unwrap();
    let before = basis.names_in(&original, &mut budget).unwrap();
    let after = basis.names_in(&local, &mut budget).unwrap();
    assert!(after.get(hot).len() < before.get(hot).len());
    for unchanged in [BindingId::new(0), BindingId::new(1), sibling, sibling_local] {
        assert_eq!(before.get(unchanged), after.get(unchanged));
    }
    let compact_seed = Plan {
        compact_order: true,
        ..original.clone()
    };
    let compact_frequency = Plan {
        compact_order: true,
        ..local.clone()
    };
    let first = basis.names_in(&compact_seed, &mut budget).unwrap();
    let second = basis.names_in(&compact_frequency, &mut budget).unwrap();
    assert!(second.get(hot).len() < first.get(hot).len());
    for unchanged in [BindingId::new(0), BindingId::new(1), sibling, sibling_local] {
        assert_eq!(first.get(unchanged), second.get(unchanged));
    }
    let resolved = policy();
    let output = module.prepare_output_with_policy(&resolved).unwrap();
    let old_code = output.render(&original).unwrap();
    let code = output.render(&local).unwrap();
    assert!(code.len() < old_code.len());
    let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));console.log(m.read(...Array.from({{length:60}},(_,i)=>i)),m.read.name,m.read.length,m.sibling(23));", serde_json::to_string(&code).unwrap());
    let actual = std::process::Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(
        String::from_utf8(actual.stdout).unwrap(),
        "479 readState 60 23\n"
    );
    for setting in ["identifier-mangling", "naming-search"] {
        let config: crate::config::ProjectConfig =
            toml::from_str(&format!("[policy.tactics]\n{setting}='off'")).unwrap();
        let disabled = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        assert!(local.check_policy(&disabled).is_err());
        assert!(module
            .prepare_output_with_policy(&disabled)
            .unwrap()
            .render(&local)
            .is_err());
    }
}

#[test]
fn observed_alphabet_preserves_captures_public_names_and_each_permission_boundary() {
    let alphabet =
        Alphabet::observed([b"ZZZZzz_$$".as_slice()], &mut AllocationBudget::new(None)).unwrap();
    assert!(alphabet.as_str().starts_with("Zz$_"));
    let mut actual = alphabet.0;
    let mut expected = Alphabet::default().0;
    actual.sort_unstable();
    expected.sort_unstable();
    assert_eq!(actual, expected);
    assert_eq!(
        Alphabet::observed([b"".as_slice()], &mut AllocationBudget::new(None)).unwrap(),
        Alphabet::default()
    );
    let module = fixture();
    let enabled = policy();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        let mut plan = Plan::new(style);
        plan.alphabet = alphabet;
        let code = module
            .prepare_output_with_policy(&enabled)
            .unwrap()
            .render(&plan)
            .unwrap();
        let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));console.log(m.read(99),m.read.name,m.read.length);", serde_json::to_string(&code).unwrap());
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{code}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "7 readState 1\n");
    }
    for (setting, style) in [
        ("identifier-mangling", Style::Source),
        ("naming-search", Style::Scoped),
        ("naming-alphabet", Style::Scoped),
    ] {
        let config: crate::config::ProjectConfig =
            toml::from_str(&format!("[policy.tactics]\n{setting}='off'")).unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let mut plan = Plan::new(style);
        assert!(plan.check_policy(&policy).is_ok());
        plan.alphabet = alphabet;
        assert!(plan.check_policy(&policy).is_err());
        assert!(module
            .prepare_output_with_policy(&policy)
            .unwrap()
            .render(&plan)
            .is_err());
    }
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
            class: None,
            defined: false,
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
            class: None,
            defined: false,
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

#[test]
fn g1_compact_names_skip_dead_slots_and_keep_public_reflection_and_captures() {
    let mut module = fixture();
    let scope = module.regions[module.functions[0].body.index()].scope;
    // Allocate holes before the late live declaration; dead pins must not
    // reserve its one-character spelling either.
    for n in 0..80 {
        module.binding(Binding {
            source_symbol: None,
            scope,
            spelling: if n == 0 {
                "b".into()
            } else {
                format!("dead{n}")
            },
            pinned: n == 0,
            class: None,
            defined: false,
        });
    }
    let live = module.binding(Binding {
        source_symbol: None,
        scope,
        spelling: "lateLive".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    let initial = module.expression(Expr::Literal(Literal::Number(5.0)), None);
    let read = module.expression(Expr::Binding(live), None);
    let state = module.expression(Expr::Binding(BindingId::new(0)), None);
    let sum = module.expression(
        Expr::Binary {
            op: Binary::Add,
            left: state,
            right: read,
        },
        None,
    );
    let body = module.functions[0].body;
    module.regions[body.index()].statements = vec![
        Statement::Let {
            binding: live,
            value: Some(initial),
        },
        Statement::Return(Some(sum)),
    ];
    let dead_body = module.region(ScopeId::new(0));
    module.functions.push(Function {
        rest: false,
        parameters: vec![],
        body: dead_body,
        arrow: false,
        name: FunctionName::Exact("a".into()),
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    let mut budget = AllocationBudget::new(None);
    let verified = verify::verify_in(&module, &mut budget).unwrap();
    let basis = Basis::new_in(&module, &verified, &mut budget).unwrap();
    let legacy = Plan {
        self_named: true,
        ..Plan::new(Style::Scoped)
    };
    let compact = Plan {
        compact_order: true,
        ..legacy.clone()
    };
    let old = basis.names_in(&legacy, &mut budget).unwrap();
    let new = basis.names_in(&compact, &mut budget).unwrap();
    assert!(old.get(live).len() > new.get(live).len());
    assert_eq!(new.get(BindingId::new(0)), "a");
    assert_eq!(new.get(module.functions[0].parameters[0]), "b");
    assert_ne!(new.get(BindingId::new(0)), new.get(live));
    let code = module
        .prepare_output_with_policy(&policy())
        .unwrap()
        .render(&compact)
        .unwrap();
    let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));console.log(m.read(0),m.read.name,m.read.length);", serde_json::to_string(&code).unwrap());
    let actual = std::process::Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        actual.status.success(),
        "{}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(actual.stdout, b"12 readState 1\n");
    for setting in ["identifier-mangling", "naming-search", "naming-compaction"] {
        let config: crate::config::ProjectConfig =
            toml::from_str(&format!("[policy.tactics]\n{setting}='off'")).unwrap();
        let off = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        assert!(module
            .prepare_output_with_policy(&off)
            .unwrap()
            .render(&compact)
            .is_err());
    }
    let config: crate::config::ProjectConfig =
        toml::from_str("[policy.tactics]\nnaming-alphabet='off'\nnaming-compaction='on'").unwrap();
    let off = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    assert_eq!(
        module
            .prepare_output_with_policy(&off)
            .unwrap()
            .render(&compact)
            .unwrap(),
        code
    );
}

#[test]
fn g1_compact_order_tracks_printed_owners_not_binding_arena_order() {
    let mut module = fixture();
    // Both root bindings are unobserved, so printed order has a visible effect.
    module.functions[0].name = FunctionName::Unobserved;
    module.regions[0].statements.swap(0, 1);
    let mut budget = AllocationBudget::new(None);
    let structure = verify::verify_in(&module, &mut budget).unwrap();
    let basis = Basis::new_in(&module, &structure, &mut budget).unwrap();
    let plan = Plan {
        compact_order: true,
        ..Plan::new(Style::Scoped)
    };
    let names = basis.names_in(&plan, &mut budget).unwrap();
    assert_eq!(names.get(BindingId::new(1)), "a");
    assert_eq!(names.get(BindingId::new(0)), "b");
    assert_eq!(names.get(BindingId::new(2)), "a"); // reuses the noncaptured root name
    assert_eq!(
        basis.compact_in(&mut budget).unwrap().printed,
        [BindingId::new(1), BindingId::new(2), BindingId::new(0)]
    );
}

#[test]
fn g1_full_continuation_alphabet_is_unique_at_every_length_boundary() {
    let mut seen = std::collections::HashSet::new();
    for index in 0..(54 + 54 * 64 + 54 * 64 * 64) {
        let mut bytes = [0; usize::BITS as usize];
        let len = encode_name(index, Alphabet::default(), true, &mut bytes);
        let name = std::str::from_utf8(&bytes[..len]).unwrap().to_owned();
        assert!(!name.as_bytes()[0].is_ascii_digit());
        assert_eq!(
            len,
            if index < 54 {
                1
            } else if index < 54 + 54 * 64 {
                2
            } else {
                3
            }
        );
        assert!(seen.insert(name));
    }
    assert!(seen.contains("a0") && seen.contains("_9"));
    let mut bytes = [0; usize::BITS as usize];
    let len = encode_name(usize::MAX, Alphabet::default(), true, &mut bytes);
    assert!(len <= bytes.len());
}

#[test]
fn g1_compact_names_preserve_direct_eval_and_import_bindings() {
    for eval in [false, true] {
        let mut module = fixture();
        let body = module.functions[0].body;
        let returned = if eval {
            let callee = module.expression(Expr::Host("eval".into()), None);
            let code = module.expression(
                Expr::Literal(Literal::String("retainedState+unusedInput".into())),
                None,
            );
            module.expression(
                Expr::Call {
                    callee,
                    arguments: vec![code],
                    invocation: Invocation::DirectEval,
                },
                None,
            )
        } else {
            let imported = module.binding(Binding {
                source_symbol: None,
                scope: ScopeId::new(0),
                spelling: "importedValue".into(),
                pinned: false,
                class: None,
                defined: false,
            });
            module.imports.push(Import {
                source: "data:text/javascript,export const publicAmount=12".into(),
                imported: "publicAmount".into(),
                binding: imported,
            });
            module.expression(Expr::Binding(imported), None)
        };
        module.regions[body.index()].statements = vec![Statement::Return(Some(returned))];
        let plan = Plan {
            compact_order: true,
            ..Plan::new(Style::Scoped)
        };
        let code = module
            .prepare_output_with_policy(&policy())
            .unwrap()
            .render(&plan)
            .unwrap();
        let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));console.log(m.read(5));", serde_json::to_string(&code).unwrap());
        let actual = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            actual.status.success(),
            "{code}: {}",
            String::from_utf8_lossy(&actual.stderr)
        );
        assert_eq!(actual.stdout, b"12\n");
    }
}

#[test]
fn g1_compact_permission_starts_at_fourteen_and_accepts_an_explicit_thirteen_override() {
    for (level, mode, expected) in [
        (13, "auto", false),
        (14, "auto", true),
        (13, "on", true),
        (15, "off", false),
    ] {
        let config: crate::config::ProjectConfig = toml::from_str(&format!(
            "effort.level={level}\n[policy.tactics]\nnaming-compaction='{mode}'"
        ))
        .unwrap();
        let resolved = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        assert_eq!(
            crate::representation::ChoiceFamily::NameAllocation
                .spec()
                .enabled(&resolved),
            expected
        );
        let plan = Plan {
            compact_order: true,
            ..Plan::new(Style::Scoped)
        };
        assert_eq!(plan.check_policy(&resolved).is_ok(), expected);
    }
}

#[test]
fn g1_compact_names_visit_class_constructors_and_prototype_methods() {
    let mut module = fixture();
    let mut function = |parameter_name: &str, constructor: bool| {
        let body = module.region(ScopeId::new(0));
        let parameter = module.binding(Binding {
            source_symbol: None,
            scope: module.regions[body.index()].scope,
            spelling: parameter_name.into(),
            pinned: false,
            class: None,
            defined: false,
        });
        let argument = module.expression(Expr::Binding(parameter), None);
        let state = module.expression(Expr::Binding(BindingId::new(0)), None);
        let sum = module.expression(
            Expr::Binary {
                op: Binary::Add,
                left: state,
                right: argument,
            },
            None,
        );
        if constructor {
            let this = module.expression(Expr::This, None);
            let field = module.expression(
                Expr::Member {
                    object: this,
                    property: Property::Named("value".into()),
                },
                None,
            );
            let set = module.expression(
                Expr::Assign {
                    target: field,
                    value: sum,
                },
                None,
            );
            module.regions[body.index()]
                .statements
                .push(Statement::Evaluate(set));
        } else {
            module.regions[body.index()]
                .statements
                .push(Statement::Return(Some(sum)));
        }
        let id = FunctionId::new(module.functions.len());
        module.functions.push(Function {
            rest: false,
            parameters: vec![parameter],
            body,
            arrow: false,
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: Suspension::None,
        });
        id
    };
    let constructor = function("constructorInput", true);
    let method = function("methodInput", false);
    let class = module.expression(
        Expr::Class {
            name: "Box".into(),
            base: None,
            constructor: Some(constructor),
            methods: vec![("read".into(), method)],
        },
        None,
    );
    let binding = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "classValue".into(),
        pinned: false,
        class: None,
        defined: false,
    });
    module.reserved.push("Box".into());
    module.regions[0].statements.push(Statement::Let {
        binding,
        value: Some(class),
    });
    module.exports.push(Export {
        binding,
        name: "Box".into(),
    });
    let compact = Plan {
        compact_order: true,
        ..Plan::new(Style::Scoped)
    };
    let code = module
        .prepare_output_with_policy(&policy())
        .unwrap()
        .render(&compact)
        .unwrap();
    let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));const box=new m.Box(5);console.log(box.value,box.read(3),m.Box.name,m.Box.length,box.read.length);", serde_json::to_string(&code).unwrap());
    let actual = std::process::Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        actual.status.success(),
        "{code}: {}",
        String::from_utf8_lossy(&actual.stderr)
    );
    assert_eq!(actual.stdout, b"12 10 Box 1 1\n");
}
