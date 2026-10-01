use super::*;

fn checked<'a>(arena: &'a bumpalo::Bump, source: &'a str) -> Program<'a> {
    let syntax = crate::parse_source(arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    from_checked_source(&syntax, &semantics).unwrap()
}

fn same(cold: &ProgramEffects, reused: &ProgramEffects) {
    assert_eq!(cold.structural.units, reused.structural.units);
    assert_eq!(cold.structural.roots, reused.structural.roots);
    assert_eq!(cold.structural.statements, reused.structural.statements);
    assert_eq!(cold.complete.units, reused.complete.units);
    assert_eq!(cold.complete.roots, reused.complete.roots);
    assert_eq!(cold.complete.access, reused.complete.access);
}

#[test]
fn q2_effect_components_match_dense_summaries_for_recursion_callbacks_and_initialization() {
    for source in [
        "bool even(int n){if(n==0){return true;}return odd(n-1);}bool odd(int n){if(n==0){return false;}return even(n-1);}print(even(10));",
        "int changed=0;int map(int n){changed+=n;return n+1;}int[] values=[1,2,3];print(values.map(map));print(changed);",
        "int outer(int n){int count=n;auto read=()=>count;auto update=(int v)=>{count+=v;return read();};return update(3);}print(outer(2));",
        "int early=read();int K=4;int read(){return K;}print(early);",
        "int K=7;int read(){return K;}print(read());",
        "extern int host();pure int compute(int n){return n*n;}int choose(bool first){if(first){return host();}return compute(2);}print(choose(false));",
    ] {
        let arena = bumpalo::Bump::new();
        let program = checked(&arena, source);
        for seal in [Seal::Module, Seal::StructuralOnly] {
            let cold = ProgramEffects::build(&program, seal);
            let reused = ProgramEffects::build_reusing(&program, seal, None, true);
            same(&cold, &reused);
            let warm = ProgramEffects::build_reusing(&program, seal, Some(&reused), true);
            same(&cold, &warm);
            assert_eq!(warm.stats.visited, 0, "{source}");
            assert!(warm.stats.components_reused > 0);
            assert!(reused.stats.visited <= cold.stats.visited);
            let disabled = ProgramEffects::build_reusing(&program, seal, Some(&warm), false);
            same(&cold, &disabled);
            assert_eq!(disabled.stats.visited, cold.stats.visited);
            assert_eq!(disabled.stats.reused, 0);
        }
    }
}

#[test]
fn q2_effect_components_keep_recursive_limits_and_do_not_restart_stable_members() {
    // Each recursive ring propagates an observable call; tails and unrelated
    // groups must neither borrow incomplete summaries nor lose divergence.
    for count in [2, 8, 31, 33, 40] {
        let mut source = String::new();
        for i in 0..count {
            let next = (i + 1) % count;
            source.push_str(&format!(
                "int f{i}(int n){{if(n==0){{{}return n;}}return f{next}(n-1);}}",
                if i == count - 1 { "print(n);" } else { "" },
            ));
        }
        source.push_str("int spare(int n){return n+7;}print(f0(3));print(spare(2));");
        let arena = bumpalo::Bump::new();
        let program = checked(&arena, &source);
        let cold = ProgramEffects::build(&program, Seal::Module);
        let reused = ProgramEffects::build_reusing(&program, Seal::Module, None, true);
        same(&cold, &reused);
        let warm = ProgramEffects::build_reusing(&program, Seal::Module, Some(&reused), true);
        same(&cold, &warm);
        assert_eq!(warm.stats.visited, 0);
    }
}

#[test]
fn q2_effect_components_invalidate_tables_and_contract_even_with_unchanged_bodies() {
    let arena = bumpalo::Bump::new();
    let program = checked(
        &arena,
        "string read(string s,int at){return s[at];}print(read(\"a\",0));",
    );
    let previous = ProgramEffects::build_reusing(&program, Seal::Module, None, true);
    let mut changed = program.clone();
    changed.trap_index_reads = true;
    let warm = ProgramEffects::build_reusing(&changed, Seal::Module, Some(&previous), true);
    same(&ProgramEffects::build(&changed, Seal::Module), &warm);
    changed.tables_revision = RevisionId::fresh();
    let restamped = ProgramEffects::build_reusing(&changed, Seal::Module, Some(&warm), true);
    same(&ProgramEffects::build(&changed, Seal::Module), &restamped);
    assert!(restamped.stats.visited > 0);
    assert_ne!(previous.complete.units, warm.complete.units);
}

#[test]
fn q2_admitted_effect_stages_match_cold_limits_and_release_both_generations() {
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
    let arena = bumpalo::Bump::new();
    let program = checked(&arena,
        "int K=4;int left(int n){if(n==0){return K;}return right(n-1);}int right(int n){if(n==0){print(n);return K;}return left(n-1);}print(left(2));");
    for seal in [Seal::Module, Seal::StructuralOnly] {
        let reference = ProgramEffects::build(&program, seal);
        let run = |reuse, work, memory| {
            let mut ledger = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
                baseline_work: 0, optional_work: work, baseline_retained_bytes: 0, retained_bytes: memory,
            }).unwrap();
            let result = {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                (|| {
                    let first = ProgramEffects::build_reusing_in(&program, seal, None, reuse, &mut budget)?;
                    same(&reference, &first);
                    let second = ProgramEffects::build_reusing_in(&program, seal, Some(&first), reuse, &mut budget)?;
                    same(&reference, &second);
                    Ok::<_, AllocationError>(())
                })()
            };
            assert_eq!(ledger.retained_bytes(), 0);
            (result, ledger.work_used(WorkDomain::Optional), ledger.peak_retained_bytes(),
                [WorkKind::Analysis, WorkKind::Edit, WorkKind::Render, WorkKind::Codec].map(|kind| ledger.work_by_kind(kind)))
        };
        let complete = run(true, 1_000_000, 1_000_000);
        assert!(complete.0.is_ok(), "{complete:?}");
        assert_eq!(complete, run(false, 1_000_000, 1_000_000));
        for work in [0, 1, 32, 128, 512, complete.1 / 2, complete.1 - 1, complete.1] {
            for memory in [0, 64, 1024, complete.2 / 2, complete.2 - 1, complete.2] {
                assert_eq!(run(true, work, memory), run(false, work, memory),
                    "seal={seal:?} work={work} memory={memory}");
            }
        }
    }
}
