use super::*;
use crate::compilation_policy::{BudgetPlan, CompilationRequest, ResourceLimits};
use crate::js::{AltId, ChoiceFamily, ChoiceKey, ChoiceMap, SiteId};

fn ledger(bytes: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 0,
            baseline_retained_bytes: 0,
            retained_bytes: bytes,
        },
    )
    .unwrap()
}

#[test]
fn q2_print_variants_match_full_formation_and_fit_the_admitted_stage() {
    let sources = [
        include_str!("../../tests/cases/objective_judged_spellings.lil"),
        r#"export string quoted(bool yes) { if(yes){return "a \"quote\"";} return "other"; }
        export float change(float value, float step) { value += step; return value; }
        extern void observe(int value);
        export void loop(int end) { int i=0; while(i<end){observe(i);i+=1;} }
        export void conditional(bool yes){if(yes){observe(7);}}"#,
    ];
    let mut compared = 0;
    for source in sources {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let policy = crate::config::ProjectConfig::default()
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let language = policy.javascript_contract().unwrap();
        let demand =
            DemandPlan::build(&program, None, None, language, DemandMode::Preserve, None).unwrap();
        for objective in [
            js::selection::Objective::Raw,
            js::selection::Objective::Gzip,
        ] {
            let families = js::OutputFamilies::seed(objective);
            let head = form_head(
                &program,
                None,
                language,
                &demand,
                true,
                js::TargetRules::SEMANTIC,
                &[],
                families.head(),
                None,
                true,
                &mut AllocationBudget::new(None),
            )
            .unwrap();
            let build = |choices: &ChoiceMap| {
                let mut owner = ledger(100_000_000);
                let result = {
                    let mut budget =
                        AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
                    budget
                        .record(|budget| {
                            form_tail(head.clone_in(budget)?, families, choices, budget)
                        })
                        .unwrap()
                };
                assert_eq!(owner.retained_bytes(), 0);
                (result.0, result.1.unwrap())
            };
            let (base, receipt) = build(&ChoiceMap::SEEDS);
            let sites: Vec<_> = base
                .choice_sites
                .iter()
                .filter(|site| site.key.family.print_only())
                .collect();
            let assignments: Vec<_> = sites
                .iter()
                .map(|site| ChoiceMap::SEEDS.with(site.key, AltId(1 - site.applied.0)))
                .chain(std::iter::once(
                    ChoiceMap::SEEDS.with_all(
                        sites
                            .iter()
                            .map(|site| (site.key, AltId(1 - site.applied.0))),
                    ),
                ))
                .collect();
            for choices in assignments {
                let (cold, cold_bill) = build(&choices);
                assert_eq!(
                    receipt.work().collect::<Vec<_>>(),
                    cold_bill.work().collect::<Vec<_>>()
                );
                assert_eq!(receipt.live_bytes(), cold_bill.live_bytes());
                assert_eq!(receipt.peak_bytes(), cold_bill.peak_bytes());
                let stage = receipt
                    .with_extra_peak(base.print_site_overlap().unwrap())
                    .unwrap();
                let mut owner = ledger(stage.peak_bytes());
                let mut warm = {
                    let mut budget =
                        AllocationBudget::new(Some((&mut owner, WorkDomain::Baseline)));
                    let mut warm = base.clone_without_print_forms(&mut budget).unwrap();
                    warm.reprint_choices(
                        families,
                        js::TargetRules::SEMANTIC,
                        &choices,
                        language.ecmascript.year(),
                        &mut budget,
                    )
                    .unwrap();
                    assert!(budget.retained_bytes(AllocationClass::Retained) <= stage.live_bytes());
                    warm
                };
                assert_eq!(owner.retained_bytes(), 0);
                assert!(owner.peak_retained_bytes() <= stage.peak_bytes());
                // The journal describes a formation's edits, not its structure.
                warm.journal = cold.journal.clone();
                assert_eq!(warm, cold, "{choices:?}");
                compared += sites.len();
            }
        }
        demand.discard(None).unwrap();
    }
    assert!(
        compared > 5,
        "fixtures must exercise actual discovered print choices"
    );
}

#[test]
fn q2_print_reuse_keys_keep_every_structural_assignment_and_joint_permission() {
    let seeds = ChoiceMap::SEEDS;
    for family in ChoiceFamily::ALL {
        let choices = seeds.with(
            ChoiceKey {
                family,
                site: SiteId::Formed(0),
            },
            AltId(1),
        );
        assert_eq!(
            seeds.same_structure(&choices),
            family.print_only(),
            "{family:?}"
        );
    }
    assert!(!seeds.same_structure(&seeds.clone().from_joint()));
}
