//! Exact edit ownership, live copy readers, and refusal cleanup. The end-to-end
//! cases use conversion's owner, so publication checks all surviving arenas.
use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
};

fn ledger(work: u64, bytes: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: work,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: bytes,
        },
    )
    .unwrap()
}
const ALL: RuleRequest = RuleRequest {
    reuse_normalization: true,
    unroll: true,
    pool: true,
    choices: crate::representation::RegionalChoices::ALL,
    fold: true,
    dead_code: true,
    inline: true,
    scalar: true,
    native: false,
    pristine_builtins: false,
    seal: Seal::Module,
};

#[test]
fn q2_source_edits_publish_exact_storage_across_grafts_signatures_and_aggregates() {
    for source in [
        "int f(int x,int spare){return x+1;}print(f(2,4));print(f(5,4));",
        "func()->int make(int n){int[] state=[n,0];return ()=>{state[0]+=1;return state[0];};}auto a=make(1);auto b=make(10);print(a());print(b());print(a());",
        "int mark(int n){print(n);return n;}int[] a=[mark(1),mark(2)];int[] b=a;b[0]=mark(3);print(a[0]);print(b[1]);",
        "int choose(bool yes){if(yes){return 3;}return 7;}print(choose(true));print(choose(false));",
        "int add(int n,int m=3){return n+m;}print(add(2));print(add(4,5));",
        "string join(string left){return left+\"b\";}print(join(\"a\"));print(join(\"c\"));",
        "JsValue f=(JsValue n)=>JS.add(n,1);JsValue ns=JS.object(\"f\",f,\"n\",7);print(JS.invoke(ns,\"f\",3));print(ns[\"n\"]);",
        "JsValue a=JS.object(\"x\",1,\"y\",2);a.x=3;print(a.x);print(a.y);",
    ] {
        let arena=bumpalo::Bump::new();
        let syntax=crate::parse_source(&arena,source).unwrap();
        let checked=crate::analyze(&syntax).unwrap();
        let mut ledger=ledger(100_000_000,64<<20);
        {
            let mut budget=AllocationBudget::new(Some((&mut ledger,WorkDomain::Baseline)));
            let (prepared,receipt)=super::super::from_source::from_checked_source_with_rules(
                &syntax,&checked,Some(ALL),false,&Default::default(),&Default::default(),None,&mut budget,
            ).unwrap_or_else(|error|panic!("{source}\n{error:?}"));
            assert!(receipt.rounds>0);
            let program=prepared.program();
            program.verify().unwrap();
            let (reference,_)=optimize(from_checked_source(&syntax,&checked).unwrap(),ALL).unwrap();
            let render=|program:&Program<'_>| super::super::javascript::lower(program).unwrap().render(crate::js::PrintPolicy::default()).unwrap();
            assert_eq!(render(program),render(&reference),"{source}");
            assert_eq!(budget.retained_bytes(Retained),0,"publication transfers all live storage");
            assert_eq!(budget.retained_bytes(Scratch),0);
            budget.with_ledger(|ledger|prepared.discard(ledger.unwrap().0));
        }
        assert_eq!(ledger.retained_bytes(),0);
    }
}

#[test]
fn q2_source_shared_body_and_tables_keep_old_reservations_until_readers_drop() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "int value=3;print(value);print(\"same\");").unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let mut ledger = ledger(100_000_000, 64 << 20);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        // Adopt an inspection fixture only in this kernel test. Production
        // always arrives with the existing conversion reservations.
        let initial =
            super::super::publication::program_retained_bytes(&program, &mut budget).unwrap();
        budget.retain(Retained, initial).unwrap();
        let mut editor = edit::Editor::new_in(program, &mut budget).unwrap();
        let root = editor.program().initialization()[0];
        let reader = editor.handle(root);
        let strings = std::sync::Arc::clone(&editor.program().strings);
        let types = std::sync::Arc::clone(&editor.program().types);
        let cells = std::sync::Arc::clone(&editor.program().cells);
        let revision = reader.revision();
        let data = editor.unit_mut_in(root, &mut budget).unwrap();
        for op in &mut data.operations {
            if matches!(op.kind, OperationKind::Constant(Constant::Integer(3))) {
                op.kind = OperationKind::Constant(Constant::Integer(4));
            }
        }
        editor
            .intern_string_in(&"added".into(), &mut budget)
            .unwrap();
        editor
            .intern_type_in(
                &crate::check::Type::Array(Box::new(crate::check::Type::Int)),
                &mut budget,
            )
            .unwrap();
        editor.unit_and_cells_in(root, &mut budget).unwrap().1[0].reassigned = true;
        editor.commit_in(&mut budget).unwrap();
        assert_ne!(editor.handle(root).revision(), revision);
        assert!(reader
            .data()
            .operations
            .iter()
            .any(|op| matches!(op.kind, OperationKind::Constant(Constant::Integer(3)))));
        let live = super::super::publication::program_retained_bytes(editor.program(), &mut budget)
            .unwrap();
        assert!(
            budget.retained_bytes(Retained) > live,
            "snapshots remain charged"
        );
        drop((reader, strings, types, cells));
        editor.commit_in(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(Retained), live);
        let program = editor.finish_in(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(Scratch), 0);
        program.verify().unwrap();
        drop(program);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn q2_source_edit_refusals_do_not_publish_partial_programs_or_leak_reservations() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "func()->int make(int n){int[] a=[n,2];return ()=>a[0]+a[1];}auto f=make(3);print(f());",
    )
    .unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let mut passed = 0;
    let mut refused = 0;
    for (work, bytes) in [
        (100, 64 << 20),
        (500, 64 << 20),
        (2_000, 64 << 20),
        (10_000, 64 << 20),
        (100_000_000, 4096),
        (100_000_000, 8192),
        (100_000_000, 16384),
        (100_000_000, 32768),
        (100_000_000, 64 << 20),
    ] {
        let mut ledger = ledger(work, bytes);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            match super::super::from_source::from_checked_source_with_rules(
                &syntax,
                &checked,
                Some(ALL),
                false,
                &Default::default(),
                &Default::default(),
                None,
                &mut budget,
            ) {
                Ok((prepared, _)) => {
                    passed += 1;
                    budget.with_ledger(|ledger| prepared.discard(ledger.unwrap().0));
                }
                Err(super::super::from_source::ConversionError::Resources(
                    crate::output_budget::AllocationError::Budget(_),
                )) => refused += 1,
                Err(error) => panic!("unexpected refusal at {work}/{bytes}: {error:?}"),
            }
            assert_eq!(budget.retained_bytes(Retained), 0);
            assert_eq!(budget.retained_bytes(Scratch), 0);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
    assert!(passed > 0 && refused > 0);
}

#[test]
fn q2_legality_probes_distinguish_decline_from_refusal_and_drop_temporaries() {
    use super::super::analysis_storage::{optional, Map};
    let mut ledger = ledger(1_000_000, 1024);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let declined = optional(&mut budget, |attempt| {
            let _temporary = attempt.vector::<u64>(Retained, 32)?;
            None::<Vec<u64>>
        })
        .unwrap();
        assert!(declined.is_none());
        assert_eq!(budget.retained_bytes(Retained), 0);
        let refused = optional(&mut budget, |attempt| {
            let first = attempt.vector::<u64>(Retained, 32)?;
            let _second = attempt.vector::<u64>(Retained, 256)?;
            Some(first)
        });
        assert!(matches!(
            refused,
            Err(crate::output_budget::AllocationError::Budget(_))
        ));
        assert_eq!(budget.retained_bytes(Retained), 0);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = optional(&mut budget, |attempt| {
                let _temporary = attempt.vector::<u64>(Retained, 32)?;
                if !std::hint::black_box(false) {
                    panic!("probe unwind");
                }
                None::<Vec<u64>>
            });
        }));
        assert!(unwound.is_err());
        assert_eq!(budget.retained_bytes(Retained), 0);
        let mut map = Map::new(Scratch);
        for (key, value) in [(9u32, 90u32), (1, 10), (4, 40), (9, 99)] {
            map.insert(key, value, &mut budget).unwrap();
        }
        assert_eq!(
            map.iter()
                .map(|(key, value)| (*key, *value))
                .collect::<Vec<_>>(),
            [(1, 10), (4, 40), (9, 99)]
        );
        assert_eq!(map.get(&9), Some(&99));
        assert_eq!(map.get(&2), None);
        map.release(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(Scratch), 0);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn q2_exact_value_analysis_releases_answers_and_propagates_nested_refusals() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena,
        "string decorate(string value){return value+\"!\";}int choose(bool yes){if(yes){return 2;}return 3;}print(decorate(\"a\"));print(decorate(\"b\"));print(choose(true));print(choose(false));"
    ).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let effects = program.effects(Seal::Module);
    let mut passed = 0;
    let mut refused = 0;
    for (work, bytes) in [
        (1, 64 << 20),
        (100_000_000, 64),
        (300, 64 << 20),
        (100_000_000, 64 << 20),
    ] {
        let mut ledger = ledger(work, bytes);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let result = budget.with_temporary(
                |budget| {
                    values::ProgramValues::compute_in(
                        &program,
                        &effects,
                        Seal::Module,
                        false,
                        budget,
                    )
                },
                |values, _| {
                    assert!(values.evaluated > 0);
                    Ok::<_, crate::output_budget::AllocationError>(())
                },
            );
            if result.is_ok() {
                passed += 1;
            } else {
                refused += 1;
            }
            assert_eq!(budget.retained_bytes(Retained), 0);
            assert_eq!(budget.retained_bytes(Scratch), 0);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
    assert!(passed > 0 && refused > 0);
}

#[test]
fn q2_stable_local_plans_replay_identical_logical_work_and_peak_storage() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena,
        "export int first(int n){if(n>0){return n+1;}else{return n-1;}}export int second(int n){return n+2;}print(first(2));print(second(3));"
    ).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let runs = [false, true].map(|reuse_normalization| {
        let mut ledger = ledger(100_000_000, 64 << 20);
        let (rendered, reused) = {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let (prepared, receipt) = super::super::from_source::from_checked_source_with_rules(
                &syntax,
                &checked,
                Some(RuleRequest {
                    reuse_normalization,
                    ..ALL
                }),
                false,
                &Default::default(),
                &Default::default(),
                None,
                &mut budget,
            )
            .unwrap();
            let rendered = super::super::javascript::lower(prepared.program())
                .unwrap()
                .render(crate::js::PrintPolicy::default())
                .unwrap();
            budget.with_ledger(|ledger| prepared.discard(ledger.unwrap().0));
            (rendered, receipt.local_units_reused)
        };
        assert_eq!(ledger.retained_bytes(), 0);
        (
            rendered,
            ledger.work_used(WorkDomain::Baseline),
            ledger.peak_retained_bytes(),
            reused,
        )
    });
    assert_eq!(
        (&runs[0].0, runs[0].1, runs[0].2),
        (&runs[1].0, runs[1].1, runs[1].2)
    );
    assert_eq!(runs[0].3, 0);
    assert!(runs[1].3 > 0);
}
