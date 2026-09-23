//! Target-formation ownership tests. Capacity observations are test-only;
//! production admits each allocation before creating it.
use super::demand::DemandMode;
use super::implementations::ImplementationMap;
use super::javascript::{self, FormationError};
use super::uses::UseIndex;
use super::*;
use crate::compilation_contract::JavaScriptCompilationContract;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::structured_js as js;

const MEMORY: u64 = 20_000_000;
const WORK: u64 = 20_000_000;
fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: WORK,
            optional_work: WORK,
            baseline_retained_bytes: 0,
            retained_bytes: MEMORY,
        },
    )
    .unwrap()
}
fn contract() -> JavaScriptCompilationContract {
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    *config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .javascript_contract()
        .unwrap()
}
fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    inspect(&from_checked_source(&syntax, &semantics).unwrap());
}
fn bytes<T>(values: &Vec<T>) -> u64 {
    (values.capacity() * std::mem::size_of::<T>()) as u64
}
fn property(value: &js::Property) -> u64 {
    match value {
        js::Property::Named(name) => name.capacity() as u64,
        js::Property::Computed(_) => 0,
    }
}
fn module_storage(module: &js::Module) -> u64 {
    let mut total = bytes(&module.expressions)
        + bytes(&module.origins)
        + bytes(&module.functions)
        + bytes(&module.bindings)
        + bytes(&module.exports)
        + bytes(&module.scopes)
        + bytes(&module.regions)
        + bytes(&module.root_modules)
        + bytes(&module.defined_parameters)
        + bytes(&module.binding_classes);
    for expression in &module.expressions {
        total += match expression {
            js::Expr::Literal(js::Literal::String(value)) => value.capacity_bytes() as u64,
            js::Expr::Host(name) => name.capacity() as u64,
            js::Expr::Call { arguments, .. }
            | js::Expr::Construct { arguments, .. }
            | js::Expr::Intrinsic { arguments, .. }
            | js::Expr::ConstructIntrinsic { arguments, .. }
            | js::Expr::Sequence(arguments)
            | js::Expr::Array(arguments) => bytes(arguments),
            js::Expr::Member { property: key, .. } => property(key),
            js::Expr::Object(entries) => {
                bytes(entries) + entries.iter().map(|(key, _)| property(key)).sum::<u64>()
            }
            js::Expr::Template(parts) => {
                bytes(parts)
                    + parts
                        .iter()
                        .map(|part| match part {
                            js::TemplatePart::String(value) => value.capacity_bytes() as u64,
                            js::TemplatePart::Expression(_) => 0,
                        })
                        .sum::<u64>()
            }
            _ => 0,
        };
    }
    total += module
        .regions
        .iter()
        .map(|region| bytes(&region.statements))
        .sum::<u64>();
    total += module
        .bindings
        .iter()
        .map(|binding| binding.spelling.capacity() as u64)
        .sum::<u64>();
    total += module
        .exports
        .iter()
        .map(|export| export.name.capacity() as u64)
        .sum::<u64>();
    total += module
        .functions
        .iter()
        .map(|function| {
            bytes(&function.parameters)
                + match &function.name {
                    js::FunctionName::Exact(name) => name.capacity_bytes() as u64,
                    js::FunctionName::Unobserved => 0,
                }
        })
        .sum::<u64>();
    total
}
fn render(module: &js::Module) -> String {
    module
        .render(js::PrintPolicy {
            mangle_bindings: false,
        })
        .unwrap()
}

#[test]
fn formation_charges_exact_live_module_capacity_and_releases_all_planning_scratch() {
    let source = r#"
        func()->int factory(int start){int count=start;return ()=>{count+=1;return count;};}
        auto a=factory(2);auto b=factory(9);
        Record<int> state=record{x:1,__proto__:4};int[] values=[3,5,7];
        int total=0;for(int i=0;i<2;i+=1){try{total+=values[i];}finally{total+=1;}}
        print(a());print(b());print(a());print(total);print(state.x??0);print("\ud800X".charCodeAt(0));
    "#;
    checked(source, |program| {
        let inspection = render(&javascript::lower(program).unwrap());
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let retained_before = ledger.retained_bytes();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                let (module, literals) = javascript::lower_admitted(
                    program,
                    &uses,
                    &ImplementationMap::direct(),
                    &contract(),
                    DemandMode::Prune,
                    false,
                    &mut budget,
                )
                .unwrap();
                assert!(literals.is_empty());
                let stored = module_storage(&module);
                assert!(stored > 0);
                assert_eq!(budget.retained_bytes(AllocationClass::Retained), stored);
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
                let actual = render(&module);
                assert_eq!(actual, inspection);
                let output = std::process::Command::new("node")
                    .args(["--input-type=module", "-e", &actual])
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    "3\n10\n4\n10\n1\n55296\n"
                );
                drop(module);
                budget.release(AllocationClass::Retained, stored).unwrap();
            }
            assert_eq!(ledger.retained_bytes(), retained_before);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}

#[test]
fn late_literal_formation_denial_preserves_incumbent_and_cleans_demand_and_partial_module() {
    let payload = "abcdefgh".repeat(4096);
    let source =
        format!("string produce(){{return \"{payload}\";}}print(produce());print(\"{payload}\");");
    checked(&source, |program| {
        let mut probe = ledger();
        let probe_uses = UseIndex::build(program, &mut probe, WorkDomain::Baseline).unwrap();
        let base = probe.retained_bytes();
        let required;
        {
            let mut budget = AllocationBudget::new(Some((&mut probe, WorkDomain::Optional)));
            let (module, literals) = javascript::lower_admitted(
                program,
                &probe_uses,
                &ImplementationMap::direct(),
                &contract(),
                DemandMode::Prune,
                false,
                &mut budget,
            )
            .unwrap();
            assert!(literals.is_empty());
            assert_eq!(
                budget.retained_bytes(AllocationClass::Retained),
                module_storage(&module)
            );
            drop(module);
        }
        required = probe.peak_retained_bytes() - base;
        probe_uses.discard(&mut probe).unwrap();
        assert_eq!(probe.retained_bytes(), 0);
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            // Existing winner storage is owned by the test just as the public
            // Compilation artifact store owns an already completed artifact.
            let incumbent = b"previous complete artifact".to_vec();
            ledger
                .retain(WorkDomain::Baseline, incumbent.capacity() as u64)
                .unwrap();
            let existing = ledger.retained_bytes();
            let padding = MEMORY - existing - (required - 1);
            ledger.retain(WorkDomain::Baseline, padding).unwrap();
            let before = ledger.retained_bytes();
            let error = {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                let error = javascript::lower_admitted(
                    program,
                    &uses,
                    &ImplementationMap::direct(),
                    &contract(),
                    DemandMode::Prune,
                    false,
                    &mut budget,
                )
                .unwrap_err();
                assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
                error
            };
            assert!(
                matches!(error, FormationError::Allocation(AllocationError::Budget(BudgetError::MemoryExhausted(found))) if found == domain),
                "{error:?}"
            );
            assert_eq!(ledger.retained_bytes(), before);
            assert_eq!(incumbent, b"previous complete artifact");
            ledger.release(WorkDomain::Baseline, padding).unwrap();
            let stored = incumbent.capacity() as u64;
            drop(incumbent);
            ledger.release(WorkDomain::Baseline, stored).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}

#[test]
fn observed_literal_payloads_share_formation_admission_and_unwind_after_payload_drop() {
    let source = r#"extern JsValue event();string flag="truth-only";JS.and(flag,event());"#;
    checked(source, |program| {
        for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
            let mut ledger = ledger();
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let before = ledger.retained_bytes();
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                let (module, literals) = javascript::lower_admitted(
                    program,
                    &uses,
                    &ImplementationMap::direct(),
                    &contract(),
                    DemandMode::Prune,
                    true,
                    &mut budget,
                )
                .unwrap();
                assert!(!literals.is_empty(), "real Formation weak-literal client");
                assert_eq!(
                    budget.retained_bytes(AllocationClass::Retained),
                    module_storage(&module) + bytes(&literals)
                );
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
                panic!("test unwind while target and literal alternatives coexist");
            }));
            assert!(panic.is_err());
            assert_eq!(ledger.retained_bytes(), before);
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        }
    });
}

#[test]
fn completed_formation_unwinds_after_actual_module_drop_in_the_original_domain() {
    checked("int twice(int n){return n*2;}print(twice(4));", |program| {
        let mut ledger = ledger();
        let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
        let before = ledger.retained_bytes();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let (module, literals) = javascript::lower_admitted(
                program,
                &uses,
                &ImplementationMap::direct(),
                &contract(),
                DemandMode::Prune,
                false,
                &mut budget,
            )
            .unwrap();
            assert!(literals.is_empty());
            assert_eq!(
                budget.retained_bytes(AllocationClass::Retained),
                module_storage(&module)
            );
            panic!("test callback after complete formation");
        }));
        assert!(result.is_err());
        assert_eq!(ledger.retained_bytes(), before);
        uses.discard(&mut ledger).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    });
}
