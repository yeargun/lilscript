use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};
use crate::config::CompressionCostModel;

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
fn fixture() -> (Module, Names) {
    let mut module = Module::default();
    let mut values = Vec::new();
    for value in [f64::MAX, f64::MIN_POSITIVE, -0.0, 1.0 / 3.0] {
        values.push(module.expression(Expr::Literal(Literal::Number(value)), None));
    }
    values.push(module.expression(
        Expr::Literal(Literal::String(StringValue::from_utf16(vec![
            0xd800,
            0,
            b'"' as u16,
            b'\\' as u16,
            0xd83e,
            0xdd80,
        ]))),
        None,
    ));
    let template = module.expression(
        Expr::Template(vec![
            TemplatePart::String("$".into()),
            TemplatePart::String("{x}`\n".into()),
        ]),
        None,
    );
    values.push(template);
    let array = module.expression(Expr::Array(values), None);
    module.regions[0]
        .statements
        .push(Statement::Evaluate(array));
    let names = Names::new(
        &module,
        PrintPolicy {
            mangle_bindings: false,
        },
    )
    .unwrap();
    (module, names)
}

#[test]
fn admitted_printer_preserves_utf16_number_template_spelling_and_retains_complete_text() {
    let (module, names) = fixture();
    let expected = render(&module, &names);
    assert!(expected.contains("\\ud800"));
    assert!(expected.contains("$\\{x}\\`\\n"));
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        let mut ledger = ledger(100_000_000, 100_000_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
            let output =
                render_admitted(&module, &names, expected.len(), &mut budget).unwrap();
            assert_eq!(output, expected);
            let retained = output.capacity() as u64;
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), retained);
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            // Each codec coexists with the same raw artifact, then returns to
            // exactly that live text capacity; no compressed buffer escapes.
            for codec in [
                CompressionCostModel::Raw,
                CompressionCostModel::Gzip,
                CompressionCostModel::Brotli,
            ] {
                let score =
                    crate::compression::measure_admitted(output.as_bytes(), codec, &mut budget)
                        .unwrap();
                assert_eq!(
                    score,
                    crate::compression::measure(output.as_bytes(), codec).unwrap()
                );
                assert_eq!(budget.retained_bytes(AllocationClass::Retained), retained);
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            }
            drop(output);
            budget.release(AllocationClass::Retained, retained).unwrap();
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert!(ledger.peak_retained_bytes() > expected.len() as u64);
        let other = if domain == WorkDomain::Baseline {
            WorkDomain::Optional
        } else {
            WorkDomain::Baseline
        };
        assert_eq!(ledger.work_used(other), 0);
    }
}

#[test]
fn byte_work_and_relocation_failure_drop_partial_text_before_releasing_its_budget() {
    let (module, names) = fixture();
    let expected = render(&module, &names);
    for domain in [WorkDomain::Baseline, WorkDomain::Optional] {
        for (memory, work, limit, reason) in [
            (100_000, 100_000, expected.len() - 1, PrintError::ByteLimit),
            (
                0,
                100_000,
                usize::MAX,
                PrintError::Admission(AllocationError::Budget(BudgetError::MemoryExhausted(
                    domain,
                ))),
            ),
            (
                100_000,
                0,
                usize::MAX,
                PrintError::Admission(AllocationError::Budget(BudgetError::WorkExhausted(domain))),
            ),
            // Initial punctuation fits; relocation for f64::MAX needs another
            // admitted buffer while the old allocation is still live.
            (
                16,
                100_000,
                usize::MAX,
                PrintError::Admission(AllocationError::Budget(BudgetError::MemoryExhausted(
                    domain,
                ))),
            ),
        ] {
            let mut ledger = ledger(memory, work);
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, domain)));
                assert_eq!(
                    render_admitted(&module, &names, limit, &mut budget),
                    Err(reason)
                );
                assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
                assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            }
            assert_eq!(ledger.retained_bytes(), 0);
            assert!(ledger.peak_retained_bytes() <= memory);
            assert!(ledger.work_used(domain) <= work);
        }
    }
}

#[test]
fn a_failed_optional_render_preserves_the_already_rendered_incumbent() {
    let (module, names) = fixture();
    let mut ledger = ledger(100_000, 100_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let incumbent = render_admitted(&module, &names, usize::MAX, &mut budget).unwrap();
        let before = budget.retained_bytes(AllocationClass::Retained);
        let same_bytes = incumbent.as_bytes().to_vec(); // Test-owned observation.
        assert_eq!(
            render_admitted(&module, &names, 1, &mut budget),
            Err(PrintError::ByteLimit)
        );
        assert_eq!(incumbent.as_bytes(), same_bytes);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), before);
        drop(incumbent);
        budget.release(AllocationClass::Retained, before).unwrap();
    }
    assert_eq!(ledger.retained_bytes(), 0);
}
