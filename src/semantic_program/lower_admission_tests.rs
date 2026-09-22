use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use crate::semantic_program::publication::{
    CheckpointLimit, Compilation, PreparedProgram, PublicationError,
};

const WORK: u64 = 100_000_000;
const MEMORY: u64 = 100_000_000;
const SENTINEL: u64 = 37;
const SOURCE: &str = "func()->int make(int seed){int value=seed;return ()=>{value+=1;return value;};}auto next=make(3);print(next());print(next());";

fn ledger(work: u64, memory: u64) -> BudgetLedger {
    let mut ledger = BudgetLedger::new(
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
    .unwrap();
    ledger.retain(WorkDomain::Baseline, SENTINEL).unwrap();
    ledger
}

fn render(program: &Program<'_>) -> String {
    program
        .to_javascript()
        .unwrap()
        .render(crate::structured_js::PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap()
}

fn addresses(program: &Program<'_>) -> (usize, usize, usize, usize) {
    (
        program.units.as_ptr() as usize,
        Arc::as_ptr(&program.cells) as usize,
        Arc::as_ptr(&program.types) as usize,
        program.units[0].data() as *const UnitData as usize,
    )
}

fn prepare<'ast, 'src>(
    syntax: &ast::Program<'ast, 'src>,
    semantics: &SemanticModel<'ast, 'src>,
    ledger: &mut BudgetLedger,
) -> Result<PreparedProgram<'src>, ConversionError> {
    from_checked_source_admitted(
        syntax,
        semantics,
        &mut AllocationBudget::new(Some((ledger, WorkDomain::Baseline))),
    )
}

#[test]
fn admitted_lower_matches_structure_and_runs_conversion_and_verification_once() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let ordinary = from_checked_source(&syntax, &semantics).unwrap();
    let mut integrated_ledger = ledger(WORK, MEMORY);
    let integrated = prepare(&syntax, &semantics, &mut integrated_ledger).unwrap();
    let mut explicit_ledger = ledger(WORK, MEMORY);
    let explicit = {
        let mut budget = AllocationBudget::new(Some((&mut explicit_ledger, WorkDomain::Baseline)));
        let mut scope = budget.scope();
        let program = convert_source(&syntax, &semantics, &mut scope).unwrap();
        verify_conversion(&program, syntax.span, &mut scope).unwrap();
        PreparedProgram::new(program, &mut scope).unwrap()
    };
    assert_eq!(integrated.program().types, ordinary.types);
    assert_eq!(integrated.program().units.len(), ordinary.units.len());
    for (actual, expected) in integrated.program().units.iter().zip(&ordinary.units) {
        assert_eq!(
            format!("{:?}", actual.data()),
            format!("{:?}", expected.data())
        );
    }
    let javascript = render(integrated.program());
    assert_eq!(javascript, render(&ordinary));
    assert_eq!(javascript, render(explicit.program()));
    assert_eq!(
        integrated_ledger.work_used(WorkDomain::Baseline),
        explicit_ledger.work_used(WorkDomain::Baseline)
    );
    for kind in [WorkKind::Analysis, WorkKind::Render, WorkKind::Edit] {
        assert_eq!(
            integrated_ledger.work_by_kind(kind),
            explicit_ledger.work_by_kind(kind)
        );
    }
    assert_eq!(
        integrated_ledger.retained_bytes(),
        explicit_ledger.retained_bytes()
    );
    assert_eq!(
        integrated_ledger.peak_retained_bytes(),
        explicit_ledger.peak_retained_bytes()
    );
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .expect("Node is required for admitted lowering observations");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "4\n5\n");
    integrated.discard(&mut integrated_ledger);
    explicit.discard(&mut explicit_ledger);
    assert_eq!(integrated_ledger.retained_bytes(), SENTINEL);
    assert_eq!(explicit_ledger.retained_bytes(), SENTINEL);
}

#[test]
fn lowering_memory_and_work_refusals_restore_the_original_parent_charge() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut calibration = ledger(WORK, MEMORY);
    let prepared = prepare(&syntax, &semantics, &mut calibration).unwrap();
    let complete_work = calibration.work_used(WorkDomain::Baseline);
    let complete_peak = calibration.peak_retained_bytes();
    prepared.discard(&mut calibration);
    assert_eq!(calibration.retained_bytes(), SENTINEL);
    assert!(complete_work > 2 && complete_peak > SENTINEL + 2);
    for (work, memory, expected) in [
        (0, MEMORY, BudgetError::WorkExhausted(WorkDomain::Baseline)),
        (
            complete_work / 2,
            MEMORY,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
        ),
        (
            complete_work - 1,
            MEMORY,
            BudgetError::WorkExhausted(WorkDomain::Baseline),
        ),
        (
            WORK,
            SENTINEL,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
        ),
        (
            WORK,
            SENTINEL + (complete_peak - SENTINEL) / 2,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
        ),
        (
            WORK,
            complete_peak - 1,
            BudgetError::MemoryExhausted(WorkDomain::Baseline),
        ),
    ] {
        let mut ledger = ledger(work, memory);
        let error = prepare(&syntax, &semantics, &mut ledger)
            .err()
            .expect("insufficient admission must refuse");
        assert!(
            matches!(error, ConversionError::Resources(AllocationError::Budget(actual)) if actual == expected),
            "{error:?}"
        );
        assert_eq!(
            ledger.retained_bytes(),
            SENTINEL,
            "work={work}, memory={memory}"
        );
        assert!(ledger.peak_retained_bytes() <= memory);
    }
}

#[test]
fn expired_lowering_does_not_allocate_or_release_another_owner() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
    let error = prepare(&syntax, &semantics, &mut ledger).err().unwrap();
    assert!(
        matches!(
            error,
            ConversionError::Resources(AllocationError::Budget(BudgetError::DeadlineExceeded))
        ),
        "{error:?}"
    );
    assert_eq!(ledger.retained_bytes(), SENTINEL);
    assert_eq!(ledger.peak_retained_bytes(), SENTINEL);
}

#[test]
fn nested_types_and_transitive_captures_keep_the_same_graph_and_observation() {
    let nested = format!("int{}", "[]".repeat(24));
    let source = format!("int inspect({nested} values){{return 7;}}func()->func()->int make(int seed){{int value=seed;return ()=>()=>{{value+=1;return value;}};}}auto middle=make(8);auto next=middle();print(next());print(next());");
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, &source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let ordinary = from_checked_source(&syntax, &semantics).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let prepared = prepare(&syntax, &semantics, &mut ledger).unwrap();
    assert!(
        prepared
            .program()
            .units
            .iter()
            .filter(|unit| !unit.data().captures.is_empty())
            .count()
            >= 2
    );
    assert_eq!(prepared.program().types, ordinary.types);
    for (actual, expected) in prepared.program().units.iter().zip(&ordinary.units) {
        assert_eq!(actual.data().captures, expected.data().captures);
    }
    let javascript = render(prepared.program());
    assert_eq!(javascript, render(&ordinary));
    let output = std::process::Command::new("node")
        .args(["--input-type=module", "--eval", &javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "9\n10\n");
    prepared.discard(&mut ledger);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn prepared_adoption_preserves_payload_addresses_without_charging_it_twice() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    let prepared = prepare(&syntax, &semantics, &mut ledger).unwrap();
    let original = addresses(prepared.program());
    let retained_payload = ledger.retained_bytes() - SENTINEL;
    assert!(retained_payload > 0);
    drop(semantics);
    drop(syntax);
    drop(arena);
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let before = compiler.ledger().retained_bytes();
    let work_before = compiler.ledger().work_used(WorkDomain::Baseline);
    let source = compiler.adopt_prepared(prepared).unwrap();
    let receipt = compiler.view(source).unwrap().receipt();
    assert!(!receipt.adopted_after_frontend);
    assert_eq!(
        compiler.ledger().retained_bytes() - before,
        receipt.allocated_bytes
    );
    assert_eq!(
        compiler.ledger().work_used(WorkDomain::Baseline) - work_before,
        receipt.logical_work
    );
    compiler
        .with_semantic(source, |program, uses, _| {
            assert_eq!(addresses(program), original);
            assert!(uses.valid_for(program));
        })
        .unwrap();
    compiler.discard(source).unwrap();
    assert_eq!(compiler.finish().retained_bytes(), SENTINEL);
}

#[test]
fn full_store_and_reverse_index_refusal_discard_only_the_new_prepared_graph() {
    for full_store in [true, false] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, SOURCE).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let mut ledger = ledger(WORK, MEMORY);
        let first = prepare(&syntax, &semantics, &mut ledger).unwrap();
        let mut compiler = Compilation::new(
            ledger,
            CheckpointLimit {
                max_live: if full_store { 1 } else { 2 },
            },
        )
        .unwrap();
        let source = compiler.adopt_prepared(first).unwrap();
        let parent_bytes = compiler.ledger().retained_bytes();
        let parent_addresses = compiler
            .with_semantic(source, |program, _, _| addresses(program))
            .unwrap();
        let second = compiler
            .with_semantic(source, |_, _, ledger| {
                prepare(&syntax, &semantics, ledger).unwrap()
            })
            .unwrap();
        assert!(compiler.ledger().retained_bytes() > parent_bytes);
        if !full_store {
            let entry_work = second.program().units.len() as u64 + 9;
            compiler
                .with_semantic(source, |_, _, ledger| {
                    let remaining = WORK - ledger.work_used(WorkDomain::Baseline) - entry_work;
                    ledger
                        .charge(WorkDomain::Baseline, WorkKind::Analysis, remaining)
                        .unwrap();
                })
                .unwrap();
        }
        let error = compiler.adopt_prepared(second).unwrap_err();
        if full_store {
            assert_eq!(error, PublicationError::StoreFull);
        } else {
            assert!(
                matches!(
                    error,
                    PublicationError::Uses(crate::semantic_program::uses::UseError::Budget(
                        BudgetError::WorkExhausted(WorkDomain::Baseline)
                    ))
                ),
                "expected index refusal, got {error:?}"
            );
        }
        assert_eq!(compiler.checkpoint_count(), 1);
        assert_eq!(compiler.ledger().retained_bytes(), parent_bytes);
        compiler
            .with_semantic(source, |program, uses, _| {
                assert_eq!(addresses(program), parent_addresses);
                assert!(uses.valid_for(program));
            })
            .unwrap();
        compiler.discard(source).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), SENTINEL);
    }
}

#[test]
fn prepared_partition_rejects_an_unadmitted_program_instead_of_topping_up() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, SOURCE).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let ordinary = from_checked_source(&syntax, &semantics).unwrap();
    let mut ledger = ledger(WORK, MEMORY);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let error = PreparedProgram::new(ordinary, &mut budget)
            .err()
            .expect("ordinary graph has no transfer authority");
        assert_eq!(error, PublicationError::InvalidPatch);
    }
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}

#[test]
fn type_clone_reconciles_its_own_spare_capacity_before_exact_preparation() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut members = Vec::with_capacity(32);
    members.extend([Type::Int, Type::Float]);
    let source_capacity = members.capacity();
    let ty = Type::Union(members);
    let mut ledger = ledger(WORK, MEMORY);
    let prepared = {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let mut scope = budget.scope();
        let mut lower = Lower::new(
            semantics.view(),
            std::slice::from_ref(&syntax),
            ModuleId::from_index(0).unwrap(),
            &mut scope,
        )
        .unwrap();
        let id = lower.ty(&ty).unwrap();
        let program = lower.finish().unwrap();
        let Type::Union(cloned) = &program.types[id.index()] else {
            panic!("expected unchanged union payload");
        };
        assert_eq!(cloned, &[Type::Int, Type::Float]);
        assert!(cloned.capacity() < source_capacity);
        verify_conversion(&program, syntax.span, &mut scope).unwrap();
        PreparedProgram::new(program, &mut scope).unwrap()
    };
    prepared.discard(&mut ledger);
    assert_eq!(ledger.retained_bytes(), SENTINEL);
}
