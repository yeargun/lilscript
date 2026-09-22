use super::*;
use crate::compilation_policy::{BudgetError, BudgetPlan, ResourceLimits, WorkKind};
use crate::lexer::TokenKind;
use std::panic::{catch_unwind, AssertUnwindSafe};

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits {
            wall_time_ms: Some(100_000),
            ..ResourceLimits::default()
        },
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}

#[test]
fn zero_memory_or_work_refuses_before_first_arena_chunk() {
    for (work, memory, expected) in [
        (100, 0, BudgetError::MemoryExhausted(WorkDomain::Baseline)),
        (0, 100_000, BudgetError::WorkExhausted(WorkDomain::Baseline)),
    ] {
        let mut ledger = ledger(work, memory);
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let error = arena.parse("int value=7;").unwrap_err();
        assert_eq!(
            error,
            AdmittedParseError::Resource(AllocationError::Budget(expected))
        );
        assert_eq!(arena.allocated_bytes(), 0);
        assert_eq!(arena.bump().allocation_limit(), Some(0));
        arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), 0));
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.peak_retained_bytes(), 0);
    }
}

#[test]
fn growth_refusal_keeps_old_chunks_values_and_reservations() {
    let mut ledger = ledger(100_000, 1024);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let first = alloc(arena.bump(), Some(&arena), [7u8; 32]).unwrap();
    let actual = arena.allocated_bytes();
    let before_work = arena.with_ledger(|ledger, _| ledger.work_used(WorkDomain::Baseline));
    let error = alloc(arena.bump(), Some(&arena), [9u8; 2048]).unwrap_err();
    assert_eq!(
        error,
        AdmittedParseError::Resource(AllocationError::Budget(BudgetError::MemoryExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(first, &[7u8; 32]);
    assert_eq!(arena.allocated_bytes(), actual);
    assert_eq!(arena.bump().allocation_limit(), Some(actual));
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), actual as u64);
        assert!(ledger.work_used(WorkDomain::Baseline) > before_work);
    });
    assert_eq!(*alloc(arena.bump(), Some(&arena), 11u8).unwrap(), 11);
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn vector_growth_refusal_keeps_initialized_values() {
    let mut ledger = ledger(100_000, 1024);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let mut values = ArenaVec::new_in(arena.bump(), Some(&arena));
    let mut count = 0;
    loop {
        match values.push([count; 64]) {
            Ok(()) => count += 1,
            Err(AdmittedParseError::Resource(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Baseline),
            ))) => break,
            result => panic!("unexpected vector growth result: {result:?}"),
        }
    }
    assert!(count > 0);
    assert_eq!(values.len(), count as usize);
    for (index, value) in values.iter().enumerate() {
        assert_eq!(value, &[index as u8; 64]);
    }
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), arena.allocated_bytes() as u64);
        assert!(ledger.peak_retained_bytes() <= 1024);
    });
    drop(values);
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn unused_growth_allowance_is_sealed_before_releasing_it() {
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    assert_eq!(*alloc(arena.bump(), Some(&arena), 17u64).unwrap(), 17);
    let actual = arena.allocated_bytes();
    assert!(actual > 0);
    assert_eq!(arena.bump().allocation_limit(), Some(actual));
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), actual as u64);
        assert!(ledger.peak_retained_bytes() > ledger.retained_bytes());
    });
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn panic_during_growth_reconciles_then_owner_cleanup_releases_chunks() {
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let _ = attempt(arena.bump(), &arena, Layout::new::<u64>(), || {
            arena
                .bump()
                .try_alloc_with(|| -> u64 { panic!("injected initializer panic") })
                .map_err(|_| AllocationError::AllocationFailed)
        });
    }));
    assert!(failure.is_err());
    let actual = arena.allocated_bytes();
    assert!(actual > 0);
    assert_eq!(arena.bump().allocation_limit(), Some(actual));
    arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), actual as u64));
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);

    let failure = catch_unwind(AssertUnwindSafe(|| {
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let _syntax = arena.parse("int answer(){return 7;}").unwrap();
        panic!("injected client panic with live syntax");
    }));
    assert!(failure.is_err());
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn deadline_blocks_allocation_inside_existing_chunk_but_not_cleanup() {
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    alloc(arena.bump(), Some(&arena), 1u8).unwrap();
    let capacity = arena.bump().chunk_capacity();
    let actual = arena.allocated_bytes();
    arena.with_ledger(|ledger, _| {
        ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
    });
    assert_eq!(
        alloc(arena.bump(), Some(&arena), 2u8).unwrap_err(),
        AdmittedParseError::Resource(AllocationError::Budget(BudgetError::DeadlineExceeded))
    );
    assert_eq!(arena.bump().chunk_capacity(), capacity);
    assert_eq!(arena.allocated_bytes(), actual);
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn syntax_errors_and_template_offsets_match_public_parser() {
    for source in [
        "int value = ;",
        "string value=`outer ${1+}`;",
        "string value=`outer ${#}`;",
        "int value=@;",
        "int value=#;",
    ] {
        let plain = Bump::new();
        let expected = crate::parser::parse_source(&plain, source).unwrap_err();
        let mut ledger = ledger(100_000, 1_000_000);
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        assert_eq!(
            arena.parse(source).unwrap_err(),
            AdmittedParseError::Syntax(expected)
        );
        arena.with_ledger(|ledger, _| {
            assert_eq!(ledger.retained_bytes(), arena.allocated_bytes() as u64);
        });
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn same_grammar_preserves_ast_and_observed_javascript() {
    let sources = [
        "int add(int left,int right){return left+right;}print(add(3,4));",
        "int[] values=[1,2,3];int total=0;for(int value of values){total+=value;}print(total);",
        "int value=7;string text=`outer ${`inner ${value}`}`;print(text);",
    ];
    let expected = ["7\n", "6\n", "outer inner 7\n"];
    for (source, expected) in sources.into_iter().zip(expected) {
        let plain_arena = Bump::new();
        let plain = crate::parser::parse_source(&plain_arena, source).unwrap();
        let mut ledger = ledger(100_000, 1_000_000);
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let admitted = arena.parse(source).unwrap();
        assert_eq!(format!("{plain:?}"), format!("{admitted:?}"));
        let emit = |syntax: &Program<'_, '_>| {
            crate::codegen_js::JsEmitter::new(crate::codegen_js::CodegenOptions::default())
                .emit_program(syntax)
                .unwrap()
        };
        let javascript = emit(&admitted);
        assert_eq!(javascript, emit(&plain));
        let output = std::process::Command::new("node")
            .args(["--eval", &javascript])
            .output()
            .expect("Node is required for parser observation tests");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
        drop(admitted);
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn arena_release_preserves_other_same_ledger_owners_and_source_lifetime() {
    let source = String::from("int answer(){return 7;}print(answer());");
    let mut ledger = ledger(100_000, 1_000_000);
    ledger.retain(WorkDomain::Baseline, 73).unwrap();
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let syntax = arena.parse(&source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = crate::semantic_program::from_checked_source(&syntax, &semantics).unwrap();
    arena.with_ledger(|ledger, domain| ledger.retain(domain, 19).unwrap());
    drop(semantics);
    drop(syntax);
    drop(arena);
    assert!(!program.units().is_empty());
    assert_eq!(ledger.retained_bytes(), 92);
    drop(program);
    ledger.release(WorkDomain::Baseline, 92).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn multiple_programs_preserve_heap_free_identities_after_collection_drop() {
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let programs = arena
        .parse_many([
            "export int first(){return 1;}",
            "export int second(){return 2;}",
        ])
        .unwrap();
    assert_eq!(programs.len(), 2);
    let identities = programs
        .iter()
        .map(|program| program.source_identity().clone())
        .collect::<Vec<_>>();
    assert_ne!(identities[0], identities[1]);
    assert!(programs
        .iter()
        .zip(&identities)
        .all(|(program, identity)| program.source_identity().same(identity)));
    let bytes = arena.allocated_bytes() as u64;
    assert!(bytes > 0);
    drop(programs);
    arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), bytes));
    drop(arena);
    assert!(identities.iter().all(|identity| identity.len() == 1));
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn owning_arena_vector_drops_elements_before_backing_on_success_and_unwind() {
    struct Probe<'a>(&'a Cell<usize>);
    impl Drop for Probe<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let dropped = Cell::new(0);
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    for unwind in [false, true] {
        let result = catch_unwind(AssertUnwindSafe(|| {
            let mut values = ArenaVec::new_in(arena.bump(), Some(&arena));
            values.push(Probe(&dropped)).unwrap();
            values.push(Probe(&dropped)).unwrap();
            if unwind {
                panic!("injected owning-list panic");
            }
            drop(values);
        }));
        assert_eq!(result.is_err(), unwind);
        assert_eq!(dropped.get(), if unwind { 4 } else { 2 });
        let bytes = arena.allocated_bytes() as u64;
        assert!(bytes > 0);
        arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), bytes));
    }
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn multiple_program_syntax_refusal_reports_exact_index_and_releases_arena() {
    for (sources, index) in [
        (["int first=;", "int second=2;"], 0),
        (["int first=1;", "int second=;"], 1),
    ] {
        let mut ledger = ledger(100_000, 1_000_000);
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let error = arena.parse_many(sources).unwrap_err();
        assert_eq!(error.index, index);
        let expected = crate::parser::parse_source(&Bump::new(), sources[index]).unwrap_err();
        assert_eq!(error.error, AdmittedParseError::Syntax(expected));
        assert!(arena.allocated_bytes() > 0);
        arena.with_ledger(|ledger, _| {
            assert_eq!(ledger.retained_bytes(), arena.allocated_bytes() as u64);
        });
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn multiple_program_work_refusal_keeps_partial_arena_owned_until_drop() {
    let first = "export int first(){return 1;}";
    let second = "export int second(){return 2;}";
    let mut calibration = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut calibration, WorkDomain::Baseline);
    let programs = arena.parse_many([first]).unwrap();
    let bytes = arena.allocated_bytes();
    let work = arena.with_ledger(|ledger, domain| ledger.work_used(domain));
    drop(programs);
    drop(arena);
    assert_eq!(calibration.retained_bytes(), 0);

    let mut ledger = ledger(work, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let error = arena.parse_many([first, second]).unwrap_err();
    assert_eq!(error.index, 1);
    assert_eq!(
        error.error,
        AdmittedParseError::Resource(AllocationError::Budget(BudgetError::WorkExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(arena.allocated_bytes(), bytes);
    arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), bytes as u64));
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn multiple_program_zero_memory_refuses_before_arena_growth() {
    let mut ledger = ledger(100_000, 0);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    assert!(arena.parse_many([]).unwrap().is_empty());
    let error = arena.parse_many(["int first=1;"]).unwrap_err();
    assert_eq!(error.index, 0);
    assert_eq!(
        error.error,
        AdmittedParseError::Resource(AllocationError::Budget(BudgetError::MemoryExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(arena.allocated_bytes(), 0);
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn nested_template_fragment_token_capacities_remain_live_together() {
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let mut parent =
        ParserCore::new(arena.bump(), "`outer ${`inner ${value}`}`", Some(&arena)).unwrap();
    let TokenKind::TemplateLiteral(parent_id) = parent.tokens[0].kind else {
        panic!("expected outer template");
    };
    let (parent_raw, parent_spans) = parent.tokens.template(parent_id);
    let parent_expression = parent_spans[0];
    let child_source = &parent_raw[parent_expression.start..parent_expression.end];
    let before_child = arena.with_ledger(|ledger, _| ledger.work_by_kind(WorkKind::Analysis));
    let child = ParserCore::new_fragment(
        arena.bump(),
        child_source,
        parent_expression.start,
        parent.source,
        Some(&arena),
    )
    .unwrap();
    arena.with_ledger(|ledger, _| {
        assert_eq!(
            ledger.work_by_kind(WorkKind::Analysis) - before_child,
            (child_source.len() + child_source.len() - 1) as u64
        );
    });
    let TokenKind::TemplateLiteral(child_id) = child.tokens[0].kind else {
        panic!("expected inner template");
    };
    let (child_raw, child_spans) = child.tokens.template(child_id);
    let child_expression = child_spans[0];
    let leaf_source = &child_raw[child_expression.start..child_expression.end];
    let before_leaf = arena.with_ledger(|ledger, _| ledger.work_by_kind(WorkKind::Analysis));
    let leaf = ParserCore::new_fragment(
        arena.bump(),
        leaf_source,
        parent_expression.start + child_expression.start,
        parent.source,
        Some(&arena),
    )
    .unwrap();
    arena.with_ledger(|ledger, _| {
        assert_eq!(
            ledger.work_by_kind(WorkKind::Analysis) - before_leaf,
            leaf_source.len() as u64
        );
    });
    assert_eq!(leaf.tokens[0].kind, TokenKind::Ident("value"));
    assert_eq!(
        leaf.tokens[0].span.start,
        parent_expression.start + child_expression.start
    );
    let backing = arena.allocated_bytes() as u64;
    let parent_bytes = parent.tokens.bytes;
    let child_bytes = child.tokens.bytes;
    let leaf_bytes = leaf.tokens.bytes;
    assert!(parent_bytes > 0 && child_bytes > 0 && leaf_bytes > 0);
    arena.with_ledger(|ledger, _| {
        assert_eq!(
            ledger.retained_bytes(),
            backing + parent_bytes + child_bytes + leaf_bytes
        );
    });
    drop(leaf);
    arena.with_ledger(|ledger, _| {
        assert_eq!(
            ledger.retained_bytes(),
            backing + parent_bytes + child_bytes
        );
    });
    drop(child);
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), backing + parent_bytes);
    });
    parent.parse_expression().unwrap();
    assert!(parent.is_at_end());
    drop(parent);
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), arena.allocated_bytes() as u64);
    });
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn child_token_refusal_does_not_release_parent_capacity() {
    let bytes = 4 * std::mem::size_of::<Token<'static>>() as u64;
    let mut ledger = ledger(100_000, bytes);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let parent = TokenStorage::new("parent", Some(&arena)).unwrap();
    assert_eq!(parent.bytes, bytes);
    assert_eq!(
        TokenStorage::new("child", Some(&arena)).err().unwrap(),
        AdmittedLexError::Resource(AllocationError::Budget(BudgetError::MemoryExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(parent[0].kind, TokenKind::Ident("parent"));
    assert_eq!(arena.allocated_bytes(), 0);
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), bytes);
        assert_eq!(ledger.peak_retained_bytes(), bytes);
    });
    drop(parent);
    arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), 0));
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn nested_fragment_byte_refusal_preserves_both_parent_streams_and_arena() {
    const WORK: u64 = 100_000;
    let mut ledger = ledger(WORK, 1_000_000);
    ledger.retain(WorkDomain::Baseline, 19).unwrap();
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let parent =
        ParserCore::new(arena.bump(), "`outer ${`inner ${value}`}`", Some(&arena)).unwrap();
    let TokenKind::TemplateLiteral(parent_id) = parent.tokens[0].kind else {
        panic!("expected outer template");
    };
    let (parent_raw, parent_spans) = parent.tokens.template(parent_id);
    let parent_expression = parent_spans[0];
    let child_source = &parent_raw[parent_expression.start..parent_expression.end];
    let child = ParserCore::new_fragment(
        arena.bump(),
        child_source,
        parent_expression.start,
        parent.source,
        Some(&arena),
    )
    .unwrap();
    let TokenKind::TemplateLiteral(child_id) = child.tokens[0].kind else {
        panic!("expected inner template");
    };
    let (child_raw, child_spans) = child.tokens.template(child_id);
    let child_expression = child_spans[0];
    let leaf_source = &child_raw[child_expression.start..child_expression.end];
    assert_eq!(leaf_source, "value");
    let parent_ptr = parent.tokens.as_ptr();
    let child_ptr = child.tokens.as_ptr();
    let backing = arena.allocated_bytes() as u64;
    let live = 19 + backing + parent.tokens.bytes + child.tokens.bytes;
    let peak = arena.with_ledger(|ledger, domain| {
        assert_eq!(ledger.retained_bytes(), live);
        let remainder = WORK - ledger.work_used(domain);
        ledger
            .charge(
                domain,
                WorkKind::Analysis,
                remainder - (leaf_source.len() as u64 - 1),
            )
            .unwrap();
        ledger.peak_retained_bytes()
    });
    let error = ParserCore::new_fragment(
        arena.bump(),
        leaf_source,
        parent_expression.start + child_expression.start,
        parent.source,
        Some(&arena),
    )
    .err()
    .expect("leaf input bytes must be admitted before scanning");
    assert_eq!(
        error,
        AdmittedParseError::Resource(AllocationError::Budget(BudgetError::WorkExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(parent.tokens.as_ptr(), parent_ptr);
    assert_eq!(child.tokens.as_ptr(), child_ptr);
    assert_eq!(
        parent.tokens.template(parent_id),
        (parent_raw, parent_spans)
    );
    assert_eq!(child.tokens.template(child_id), (child_raw, child_spans));
    assert_eq!(arena.allocated_bytes() as u64, backing);
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), live);
        assert_eq!(ledger.peak_retained_bytes(), peak);
    });
    drop(child);
    drop(parent);
    arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), 19 + backing));
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 19);
}

#[test]
fn nested_template_lexical_refusal_keeps_parent_tokens_and_template_payload_live() {
    const MEMORY: u64 = 1_000_000;
    let mut ledger = ledger(100_000, MEMORY);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let plain = TokenStorage::new("parent", Some(&arena)).unwrap();
    let plain_bytes = plain.bytes;
    drop(plain);
    let parent = TokenStorage::new("`outer ${`inner ${value}`}`", Some(&arena)).unwrap();
    assert!(
        parent.bytes > plain_bytes,
        "template payload is retained too"
    );
    let TokenKind::TemplateLiteral(id) = parent[0].kind else {
        panic!("expected template");
    };
    let (raw, expressions) = parent.template(id);
    assert_eq!(expressions.len(), 1);
    let span = expressions[0];
    let child_source = &raw[span.start..span.end];
    assert_eq!(child_source, "`inner ${value}`");
    let padding = MEMORY - parent.bytes;
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), parent.bytes);
        ledger.retain(WorkDomain::Baseline, padding).unwrap();
    });
    assert_eq!(
        TokenStorage::new(child_source, Some(&arena)).err().unwrap(),
        AdmittedLexError::Resource(AllocationError::Budget(BudgetError::MemoryExhausted(
            WorkDomain::Baseline
        )))
    );
    assert_eq!(parent.template(id), (raw, expressions));
    assert_eq!(arena.allocated_bytes(), 0);
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), MEMORY);
        ledger.release(WorkDomain::Baseline, padding).unwrap();
        assert_eq!(ledger.retained_bytes(), parent.bytes);
    });
    drop(parent);
    arena.with_ledger(|ledger, _| assert_eq!(ledger.retained_bytes(), 0));
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn parser_panic_releases_tokens_before_reusable_arena_capacity() {
    let mut ledger = ledger(100_000, 1_000_000);
    let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
    let failure = catch_unwind(AssertUnwindSafe(|| {
        let parent = ParserCore::new(arena.bump(), "int value=7;", Some(&arena)).unwrap();
        let _child =
            ParserCore::new_fragment(arena.bump(), "value+1", 0, parent.source, Some(&arena))
                .unwrap();
        arena.with_ledger(|ledger, _| {
            assert!(ledger.retained_bytes() > arena.allocated_bytes() as u64);
        });
        panic!("injected panic with live parser token streams");
    }));
    assert!(failure.is_err());
    arena.with_ledger(|ledger, _| {
        assert_eq!(ledger.retained_bytes(), arena.allocated_bytes() as u64);
    });
    let syntax = arena.parse("int replacement=9;").unwrap();
    drop(syntax);
    drop(arena);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn lookahead_token_and_eof_probes_require_exact_work_without_mutating_storage() {
    const WORK: u64 = 100_000;
    for available in 0..=3 {
        let mut ledger = ledger(WORK, 1_000_000);
        ledger.retain(WorkDomain::Baseline, 19).unwrap();
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let parent = TokenStorage::new("`parent ${value}`", Some(&arena)).unwrap();
        let parser = ParserCore::new(arena.bump(), "int value", Some(&arena)).unwrap();
        let pointer = parser.tokens.as_ptr();
        let tokens = parser.tokens.to_vec();
        let parent_pointer = parent.as_ptr();
        let TokenKind::TemplateLiteral(template) = parent[0].kind else {
            panic!("expected retained parent template");
        };
        let parent_payload = parent.template(template);
        let backing = arena.allocated_bytes();
        let (live, peak, before) = arena.with_ledger(|ledger, domain| {
            ledger
                .charge(
                    domain,
                    WorkKind::Analysis,
                    WORK - ledger.work_used(domain) - available,
                )
                .unwrap();
            (
                ledger.retained_bytes(),
                ledger.peak_retained_bytes(),
                ledger.work_by_kind(WorkKind::Analysis),
            )
        });
        for (index, expected) in [Some(TokenKind::Int), Some(TokenKind::Ident("value")), None]
            .into_iter()
            .enumerate()
        {
            let result = parser.lookahead_kind(index).map(|kind| kind.cloned());
            if index as u64 == available {
                assert_eq!(
                    result.unwrap_err(),
                    AdmittedParseError::Resource(AllocationError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    ))
                );
                break;
            }
            assert_eq!(result.unwrap(), expected);
        }
        assert_eq!(parser.cursor, 0);
        assert_eq!(parser.tokens.as_ptr(), pointer);
        assert_eq!(&parser.tokens[..], tokens.as_slice());
        assert_eq!(parent.as_ptr(), parent_pointer);
        assert_eq!(parent.template(template), parent_payload);
        assert_eq!(arena.allocated_bytes(), backing);
        arena.with_ledger(|ledger, _| {
            assert_eq!(ledger.work_by_kind(WorkKind::Analysis) - before, available);
            assert_eq!(ledger.retained_bytes(), live);
            assert_eq!(ledger.peak_retained_bytes(), peak);
        });
        drop(parser);
        arena.with_ledger(|ledger, _| {
            assert_eq!(ledger.retained_bytes(), 19 + backing as u64 + parent.bytes);
        });
        drop(parent);
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 19);
    }
}

#[test]
fn recursive_type_lookahead_preserves_results_at_every_probe_cutoff() {
    const WORK: u64 = 100_000;
    for (source, expected, probes) in [
        ("", None, 1),
        ("int", Some(1), 5),
        ("int?[]?", Some(5), 13),
        ("Box<int,string>", Some(6), 19),
        ("func(int)->string", Some(6), 21),
        ("func(ref int)->void", Some(7), 27),
        ("(int|float)[]", Some(7), 18),
        ("int[", Some(1), 5),
        ("Box<int", None, 9),
        ("Box<>", None, 3),
        ("func(int", None, 10),
        ("func()->", None, 6),
        ("(int", None, 7),
        ("int|", None, 6),
    ] {
        let plain_arena = Bump::new();
        let plain = ParserCore::new(&plain_arena, source, None).unwrap();
        assert_eq!(plain.scan_type_end(0).unwrap(), expected, "{source}");
        for available in 0..=probes {
            let mut ledger = ledger(WORK, 1_000_000);
            ledger.retain(WorkDomain::Baseline, 19).unwrap();
            let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
            let parser = ParserCore::new(arena.bump(), source, Some(&arena)).unwrap();
            let pointer = parser.tokens.as_ptr();
            let tokens = parser.tokens.to_vec();
            let backing = arena.allocated_bytes();
            let (live, peak, before) = arena.with_ledger(|ledger, domain| {
                ledger
                    .charge(
                        domain,
                        WorkKind::Analysis,
                        WORK - ledger.work_used(domain) - available,
                    )
                    .unwrap();
                (
                    ledger.retained_bytes(),
                    ledger.peak_retained_bytes(),
                    ledger.work_by_kind(WorkKind::Analysis),
                )
            });
            let actual = parser.scan_type_end(0);
            if available == probes {
                assert_eq!(actual.unwrap(), expected, "{source}");
            } else {
                assert_eq!(
                    actual.unwrap_err(),
                    AdmittedParseError::Resource(AllocationError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    )),
                    "{source}: cutoff {available}/{probes}"
                );
            }
            assert_eq!(parser.cursor, 0);
            assert_eq!(parser.tokens.as_ptr(), pointer);
            assert_eq!(&parser.tokens[..], tokens.as_slice());
            assert_eq!(arena.allocated_bytes(), backing);
            arena.with_ledger(|ledger, _| {
                assert_eq!(ledger.work_by_kind(WorkKind::Analysis) - before, available);
                assert_eq!(ledger.retained_bytes(), live);
                assert_eq!(ledger.peak_retained_bytes(), peak);
            });
            drop(parser);
            drop(arena);
            assert_eq!(ledger.retained_bytes(), 19);
        }
    }
}

#[test]
fn arrow_binding_and_reference_decisions_admit_their_final_probe() {
    const WORK: u64 = 100_000;
    for (source, mode, expected, probes) in [
        ("(int value)=>value", 0, true, 5),
        ("()=>1", 0, true, 3),
        ("((1))", 0, false, 6),
        ("(1", 0, false, 2),
        ("int value", 1, true, 6),
        ("int", 1, false, 6),
        ("Box<int,string>?[] value", 1, true, 25),
        ("ref int value", 2, true, 7),
        ("ref int", 2, false, 7),
        ("ref", 2, false, 2),
        ("ref<int> value", 2, false, 2),
        ("int value", 2, false, 1),
        ("ref func(int)->string callback", 2, true, 23),
        ("ref int)", 3, true, 7),
        ("ref int,", 3, true, 7),
        ("ref int", 3, false, 7),
        ("ref func(ref int)->void)", 3, true, 29),
    ] {
        let inspect = |parser: &ParserCore<'_, '_>| match mode {
            0 => parser.is_arrow_function_start(),
            1 => parser.looks_like_typed_binding(),
            2 => parser.reference_parameter_at(0, true),
            3 => parser.reference_parameter_at(0, false),
            _ => unreachable!(),
        };
        let plain_arena = Bump::new();
        let plain = ParserCore::new(&plain_arena, source, None).unwrap();
        assert_eq!(inspect(&plain).unwrap(), expected, "{source}");
        for available in [probes - 1, probes] {
            let mut ledger = ledger(WORK, 1_000_000);
            let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
            let parser = ParserCore::new(arena.bump(), source, Some(&arena)).unwrap();
            let (live, peak, before) = arena.with_ledger(|ledger, domain| {
                ledger
                    .charge(
                        domain,
                        WorkKind::Analysis,
                        WORK - ledger.work_used(domain) - available,
                    )
                    .unwrap();
                (
                    ledger.retained_bytes(),
                    ledger.peak_retained_bytes(),
                    ledger.work_by_kind(WorkKind::Analysis),
                )
            });
            let actual = inspect(&parser);
            if available == probes {
                assert_eq!(actual.unwrap(), expected, "{source}");
            } else {
                assert_eq!(
                    actual.unwrap_err(),
                    AdmittedParseError::Resource(AllocationError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    )),
                    "{source}: cutoff {available}/{probes}"
                );
            }
            assert_eq!(parser.cursor, 0);
            arena.with_ledger(|ledger, _| {
                assert_eq!(ledger.work_by_kind(WorkKind::Analysis) - before, available);
                assert_eq!(ledger.retained_bytes(), live);
                assert_eq!(ledger.peak_retained_bytes(), peak);
            });
            drop(parser);
            drop(arena);
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }
}

#[test]
fn lookahead_deadlines_preserve_parent_tokens_and_arena_until_actual_drop() {
    for (mode, source) in [
        (0, "(int value)=>value"),
        (1, "Box<int> value"),
        (2, "func(ref int)->void"),
        (3, "ref int value"),
    ] {
        let mut ledger = ledger(100_000, 1_000_000);
        ledger.retain(WorkDomain::Baseline, 19).unwrap();
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let parent = TokenStorage::new("`parent ${value}`", Some(&arena)).unwrap();
        let parser = ParserCore::new(arena.bump(), source, Some(&arena)).unwrap();
        let TokenKind::TemplateLiteral(template) = parent[0].kind else {
            panic!("expected retained parent template");
        };
        let payload = parent.template(template);
        let parent_pointer = parent.as_ptr();
        let pointer = parser.tokens.as_ptr();
        let tokens = parser.tokens.to_vec();
        let backing = arena.allocated_bytes();
        let (live, peak, before) = arena.with_ledger(|ledger, domain| {
            ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
            (
                ledger.retained_bytes(),
                ledger.peak_retained_bytes(),
                ledger.work_used(domain),
            )
        });
        let result = match mode {
            0 => parser.is_arrow_function_start(),
            1 => parser.looks_like_typed_binding(),
            2 => parser.scan_type_end(0).map(|end| end.is_some()),
            3 => parser.reference_parameter_at(0, true),
            _ => unreachable!(),
        };
        assert_eq!(
            result.unwrap_err(),
            AdmittedParseError::Resource(AllocationError::Budget(BudgetError::DeadlineExceeded))
        );
        assert_eq!(parser.cursor, 0);
        assert_eq!(parser.tokens.as_ptr(), pointer);
        assert_eq!(&parser.tokens[..], tokens.as_slice());
        assert_eq!(parent.as_ptr(), parent_pointer);
        assert_eq!(parent.template(template), payload);
        assert_eq!(arena.allocated_bytes(), backing);
        arena.with_ledger(|ledger, domain| {
            assert_eq!(ledger.work_used(domain), before);
            assert_eq!(ledger.retained_bytes(), live);
            assert_eq!(ledger.peak_retained_bytes(), peak);
        });
        drop(parser);
        arena.with_ledger(|ledger, _| {
            assert_eq!(ledger.retained_bytes(), 19 + backing as u64 + parent.bytes);
        });
        drop(parent);
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 19);
    }
}

#[test]
fn nested_grouping_charges_repeated_arrow_scans_and_refuses_before_inner_syntax() {
    const WORK: u64 = 100_000;
    for depth in [1usize, 2, 4, 8, 16] {
        let source = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
        let probes = (depth * (depth + 3)) as u64;
        for available in [probes - 1, probes] {
            let mut ledger = ledger(WORK, 1_000_000);
            ledger.retain(WorkDomain::Baseline, 19).unwrap();
            let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
            let mut parser = ParserCore::new(arena.bump(), &source, Some(&arena)).unwrap();
            let pointer = parser.tokens.as_ptr();
            let backing = arena.allocated_bytes();
            let (live, peak, before) = arena.with_ledger(|ledger, domain| {
                ledger
                    .charge(
                        domain,
                        WorkKind::Analysis,
                        WORK - ledger.work_used(domain) - available,
                    )
                    .unwrap();
                (
                    ledger.retained_bytes(),
                    ledger.peak_retained_bytes(),
                    ledger.work_by_kind(WorkKind::Analysis),
                )
            });
            let result = parser.parse_expression();
            if available == probes {
                let expression = result.unwrap();
                assert!(matches!(expression.kind, crate::ast::ExprKind::Int(1, _)));
                assert!(parser.is_at_end());
            } else {
                assert_eq!(
                    result.unwrap_err(),
                    AdmittedParseError::Resource(AllocationError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    ))
                );
                assert_eq!(parser.cursor, depth - 1);
            }
            assert_eq!(parser.tokens.as_ptr(), pointer);
            assert_eq!(arena.allocated_bytes(), backing);
            arena.with_ledger(|ledger, _| {
                assert_eq!(ledger.work_by_kind(WorkKind::Analysis) - before, available);
                assert_eq!(ledger.retained_bytes(), live);
                assert_eq!(ledger.peak_retained_bytes(), peak);
            });
            drop(parser);
            drop(arena);
            assert_eq!(ledger.retained_bytes(), 19);
        }
    }
}

#[test]
fn admitted_lookahead_preserves_public_parser_grammar_and_syntax_diagnostics() {
    let sources = [
        "func(int)->int twice=(int x)=>((x*2));",
        "(func(int)->int)[] transforms=[];func(int)->int first=transforms[0];",
        "void f(ref p,ref ref,ref int n,ref ref alias,ref<Point> generic,ref[] array,ref? maybe){} func(ref,ref int,ref ref)->void callback;",
        "Box<int,string>?[] value=[];",
    ];
    for source in sources {
        let plain = Bump::new();
        let expected = crate::parser::parse_source(&plain, source).unwrap();
        let mut ledger = ledger(100_000, 1_000_000);
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        let actual = arena.parse(source).unwrap();
        assert_eq!(&*actual, &*expected, "{source}");
        drop(actual);
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 0);
    }
    for source in [
        "func(int)-> callback;",
        "Box<int,> value=[];",
        "int value=((1);",
        "void f(ref int){}",
    ] {
        let plain = Bump::new();
        let expected = crate::parser::parse_source(&plain, source).unwrap_err();
        let mut ledger = ledger(100_000, 1_000_000);
        let arena = AdmittedArena::new(&mut ledger, WorkDomain::Baseline);
        assert_eq!(
            arena.parse(source).unwrap_err(),
            AdmittedParseError::Syntax(expected),
            "{source}"
        );
        drop(arena);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}
