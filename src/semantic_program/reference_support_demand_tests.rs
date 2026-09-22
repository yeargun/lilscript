//! The support decision belongs to completed physical demand, before target
//! binding creation. The original location cursor resolves all inline actuals.
use super::demand::{DemandMode, DemandPlan};
use super::helper_family::FamilyOutcome;
use super::implementations::ImplementationMap;
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{CompilationRequest, WorkDomain, WorkKind};

fn inspect(
    source: &str,
    inline: &[&str],
    modes: &[DemandMode],
    mut check: impl FnMut(&DemandPlan<'_, '_>, DemandMode),
) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let mut ledger = super::helper_family_tests::ledger();
    let uses = UseIndex::build(&program, &mut ledger, WorkDomain::Baseline).unwrap();
    let mut map = ImplementationMap::direct();
    if !inline.is_empty() {
        let mut cache = super::helper_family_tests::cache(&mut ledger);
        for name in inline {
            let (outcome, _) = super::helper_family_tests::analyze(
                &program,
                &uses,
                super::helper_family_tests::root(&program, name),
                &mut ledger,
                &mut cache,
            );
            let FamilyOutcome::Complete(family) = outcome else {
                panic!("{outcome:?}");
            };
            let next = map
                .with_inline_helper(family, &mut ledger, WorkDomain::Optional)
                .unwrap();
            map.discard(&mut ledger).unwrap();
            map = next;
        }
        cache.discard(&mut ledger).unwrap();
    }
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    for &mode in modes {
        let retained = ledger.retained_bytes();
        let plan = DemandPlan::build(
            &program,
            Some(&uses),
            Some(&map),
            policy.javascript_contract().unwrap(),
            mode,
            Some((&mut ledger, WorkDomain::Baseline)),
        )
        .unwrap();
        let before = ledger.work_used(WorkDomain::Baseline);
        plan.writes_incoming_references_visited(|amount| {
            ledger.charge(WorkDomain::Baseline, WorkKind::Analysis, amount as u64)
        })
        .unwrap();
        assert_eq!(ledger.work_used(WorkDomain::Baseline) - before, 1);
        // A denied getter must not masquerade as a negative support fact.
        let denied = plan.writes_incoming_references_visited(|_| Err("query admission"));
        assert_eq!(denied, Err("query admission"));
        check(&plan, mode);
        plan.discard(Some(&mut ledger)).unwrap();
        assert_eq!(ledger.retained_bytes(), retained);
    }
    map.discard(&mut ledger).unwrap();
    uses.discard(&mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

fn writes(plan: &DemandPlan<'_, '_>) -> bool {
    plan.writes_incoming_references_visited(|_| Ok::<_, ()>(()))
        .unwrap()
}

#[test]
fn incoming_writes_follow_live_contexts_and_preserved_bodies_not_source_exposure() {
    let cases = [
        (
            "ordinary dead local store",
            "int value=1;value=4;print(7);",
            false,
            false,
        ),
        (
            "read-only incoming location",
            "int read(ref int value){return value;}int state=1;print(read(ref state));",
            false,
            false,
        ),
        (
            "unformed versus preserved writer body",
            "void overwrite(ref int value){value=4;}print(7);",
            false,
            true,
        ),
        (
            "whole reference write after an earlier read",
            "int overwrite(ref int value){int old=value;value=7;return old;}int state=1;print(overwrite(ref state));print(state);",
            true,
            true,
        ),
        (
            "write inside a retained dynamic branch",
            "extern bool choose();void overwrite(ref int value){if(choose()){value=4;}}int state=1;overwrite(ref state);print(state);",
            true,
            true,
        ),
        (
            "projected incoming write",
            "struct Pair{int x;int y;}void overwrite(ref Pair value){value.x=7;}Pair state=Pair{1,2};overwrite(ref state);print(state.x);",
            true,
            true,
        ),
    ];
    for (name, source, prune, preserve) in cases {
        inspect(
            source,
            &[],
            &[DemandMode::Prune, DemandMode::Preserve],
            |plan, mode| {
                assert_eq!(
                    writes(plan),
                    if mode == DemandMode::Prune {
                        prune
                    } else {
                        preserve
                    },
                    "{name}: {mode:?}"
                );
            },
        );
    }
}

#[test]
fn inline_write_resolution_distinguishes_owned_actuals_from_named_incoming_roots() {
    let cases = [
        (
            "inline whole owned actual",
            "void overwrite(ref int value){value=7;}int state=1;overwrite(ref state);print(state);",
            false,
        ),
        (
            "inline projected owned actual",
            "struct Pair{int x;int y;}void overwrite(ref int value){value=7;}Pair state=Pair{1,2};overwrite(ref state.x);print(state.x);",
            false,
        ),
        (
            "inline into ordinary by-value parameter",
            "void overwrite(ref int value){value=7;}int relay(int value){overwrite(ref value);return value;}print(relay(1));",
            false,
        ),
        (
            "inline into a named reference parameter",
            "void overwrite(ref int value){value=7;}void relay(ref int value){overwrite(ref value);}int state=1;relay(ref state);print(state);",
            true,
        ),
        (
            "inline projected prefix through named incoming root",
            "struct Pair{int x;int y;}struct Outer{Pair pair;}void overwrite(ref int value){value=7;}void relay(ref Outer value){overwrite(ref value.pair.x);}Outer state=Outer{Pair{1,2}};relay(ref state);print(state.pair.x);",
            true,
        ),
    ];
    for (name, source, expected) in cases {
        inspect(source, &["overwrite"], &[DemandMode::Prune], |plan, _| {
            assert!(
                plan.contexts()
                    .iter()
                    .any(|context| context.kind.is_inline()),
                "{name}"
            );
            assert_eq!(writes(plan), expected, "{name}");
        });
    }
}
