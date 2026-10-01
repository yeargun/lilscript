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
                &syntax,&checked,Some(ALL),false,&Default::default(),None,&mut budget,
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
