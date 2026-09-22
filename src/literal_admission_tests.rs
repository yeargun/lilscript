use super::*;
use crate::compilation_policy::{
    BudgetError, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain,
};

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

fn decode(
    source: &str,
    template: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<StringValue, StringDecodeError> {
    if template {
        StringValue::decode_template_admitted(source, budget)
    } else {
        StringValue::decode_source_admitted(source, budget)
    }
}

#[test]
fn admitted_source_and_template_decoding_share_values_and_canonical_storage() {
    for (source, template) in [
        ("plain \u{e9}\u{1f600}", false),
        (r"A\n\u{1f600}", false),
        (r"\x41\u000a\ud83d\ude00", false),
        (r"\ud800X\udfff", false),
        (r"\u{d800}", false),
        (r"\0\v\z", false),
        ("a\\\r\nb", false),
        ("a\r\nb\rc\n\\r\\n\\`\\${x}\\\r\nd\\ud800", true),
        ("plain\r\ntext\r", true),
        ("", false),
        ("", true),
    ] {
        let expected = if template {
            StringValue::decode_template(source)
        } else {
            StringValue::decode_source(source)
        }
        .unwrap();
        let mut ledger = ledger(100_000, 100_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let actual = decode(source, template, &mut budget).unwrap();
        assert_eq!(actual, expected, "source={source:?}, template={template}");
        assert_eq!(actual.as_unicode(), expected.as_unicode());
        assert_eq!(actual.as_unpaired_utf16(), expected.as_unpaired_utf16());
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert_eq!(
            budget.retained_bytes(AllocationClass::Retained),
            actual.capacity_bytes() as u64
        );
        drop(actual);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn admitted_decoding_matches_independent_node_utf16_observations() {
    let cases = [
        (r"A\n\u{1f600}\ud800X\udfff", false),
        (r"\ud83d\ude00", false),
        (r"\0\v\z", false),
        ("a\\\r\nb", false),
        ("a\r\nb\rc\n\\r\\n\\`\\${x}\\\r\nd\\ud800", true),
    ];
    let script = format!(
        r#"const cases={};process.stdout.write(JSON.stringify(cases.map(([source,template])=>{{const quote=template?'`':'"';const value=Function('return '+quote+source+quote)();return Array.from({{length:value.length}},(_,index)=>value.charCodeAt(index));}})));"#,
        serde_json::to_string(&cases).unwrap()
    );
    let observed = std::process::Command::new("node")
        .args(["--eval", &script])
        .output()
        .expect("Node is required for literal UTF-16 observation tests");
    assert!(
        observed.status.success(),
        "{}",
        String::from_utf8_lossy(&observed.stderr)
    );
    let observed: Vec<Vec<u16>> = serde_json::from_slice(&observed.stdout).unwrap();
    assert_eq!(observed.len(), cases.len());
    for ((source, template), expected) in cases.into_iter().zip(observed) {
        let mut ledger = ledger(100_000, 100_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let value = decode(source, template, &mut budget).unwrap();
        assert_eq!(value.code_units().collect::<Vec<_>>(), expected);
        drop(value);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn unicode_conversion_admits_both_buffers_at_the_exact_transient_peak() {
    let source = r"\u20ac";
    let scratch = (source.len() * std::mem::size_of::<u16>()) as u64;
    let output = "\u{20ac}".len() as u64;
    let mut ledger = ledger(100_000, scratch + output);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let value = StringValue::decode_source_admitted(source, &mut budget).unwrap();
    assert_eq!(value.as_unicode(), Some("\u{20ac}"));
    assert_eq!(value.capacity_bytes() as u64, output);
    budget.with_ledger(|owner| {
        let (ledger, _) = owner.unwrap();
        assert_eq!(ledger.retained_bytes(), output);
        assert_eq!(ledger.peak_retained_bytes(), scratch + output);
        assert_eq!(
            ledger.work_by_kind(WorkKind::Analysis),
            2 * source.len() as u64 + 2
        );
        assert_eq!(ledger.work_by_kind(WorkKind::Render), output + 2);
    });
    drop(value);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn valid_surrogate_pair_is_unicode_but_unpaired_units_need_no_second_buffer() {
    for (source, expected_unicode, units) in [
        (r"\ud83d\ude00", Some("\u{1f600}"), None),
        (r"\ud800", None, Some(&[0xd800][..])),
    ] {
        let scratch = (source.len() * std::mem::size_of::<u16>()) as u64;
        let extra = expected_unicode.map_or(0, |value| value.len()) as u64;
        let mut ledger = ledger(100_000, scratch + extra);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let value = StringValue::decode_source_admitted(source, &mut budget).unwrap();
        assert_eq!(value.as_unicode(), expected_unicode);
        assert_eq!(value.as_unpaired_utf16(), units);
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        budget.with_ledger(|owner| {
            let (ledger, _) = owner.unwrap();
            assert_eq!(ledger.peak_retained_bytes(), scratch + extra);
            assert_eq!(
                ledger.retained_bytes(),
                if extra == 0 { scratch } else { extra }
            );
        });
        drop(value);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn plain_unicode_copy_uses_only_its_exact_utf8_capacity() {
    let source = "text \u{1f600}";
    let bytes = source.len() as u64;
    let mut ledger = ledger(100_000, bytes);
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    let value = StringValue::decode_source_admitted(source, &mut budget).unwrap();
    assert_eq!(value.as_unicode(), Some(source));
    assert_eq!(value.capacity_bytes() as u64, bytes);
    budget.with_ledger(|owner| {
        let (ledger, _) = owner.unwrap();
        assert_eq!(ledger.peak_retained_bytes(), bytes);
        assert_eq!(ledger.work_by_kind(WorkKind::Analysis), bytes * 2);
        assert_eq!(ledger.work_by_kind(WorkKind::Render), bytes + 1);
    });
    drop(value);
    drop(budget);
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn syntax_refusal_releases_partial_decoder_storage_and_preserves_parent() {
    for template in [false, true] {
        for source in [
            r"text\x",
            r"\u00x0",
            r"\u{}",
            r"\u{110000}",
            r"\08",
            r"\1",
            "\\",
        ] {
            let expected = if template {
                StringValue::decode_template(source)
            } else {
                StringValue::decode_source(source)
            }
            .unwrap_err();
            let mut ledger = ledger(100_000, 100_000);
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            let parent = budget.string(AllocationClass::Retained, "kept").unwrap();
            let error = decode(source, template, &mut budget).unwrap_err();
            assert_eq!(error, StringDecodeError::Escape(expected));
            assert_eq!(parent, "kept");
            assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), 4);
            budget.with_ledger(|owner| assert_eq!(owner.unwrap().0.retained_bytes(), 4));
            drop(parent);
            drop(budget);
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }
}

#[test]
fn memory_refusal_covers_initial_storage_and_utf8_conversion_overlap() {
    let source = r"\u20ac";
    let scratch = (source.len() * std::mem::size_of::<u16>()) as u64;
    for (memory, expected_peak) in [(4, 4), (4 + scratch + 2, 4 + scratch)] {
        let mut ledger = ledger(100_000, memory);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let parent = budget.string(AllocationClass::Retained, "kept").unwrap();
        let error = StringValue::decode_source_admitted(source, &mut budget).unwrap_err();
        assert_eq!(
            error,
            StringDecodeError::Resources(AllocationError::Budget(BudgetError::MemoryExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 4);
        budget.with_ledger(|owner| {
            let (ledger, _) = owner.unwrap();
            assert_eq!(ledger.retained_bytes(), 4);
            assert_eq!(ledger.peak_retained_bytes(), expected_peak);
        });
        drop(parent);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
    }
}

#[test]
fn work_refusal_before_scanning_and_during_conversion_leaves_no_storage() {
    let source = r"\u20ac";
    let mut calibration = ledger(100_000, 100_000);
    let mut budget = AllocationBudget::new(Some((&mut calibration, WorkDomain::Baseline)));
    let value = StringValue::decode_source_admitted(source, &mut budget).unwrap();
    drop(value);
    drop(budget);
    let work = calibration.work_used(WorkDomain::Baseline);
    for limit in [0, work - 1] {
        let mut ledger = ledger(limit, 100_000);
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        assert_eq!(
            StringValue::decode_source_admitted(source, &mut budget).unwrap_err(),
            StringDecodeError::Resources(AllocationError::Budget(BudgetError::WorkExhausted(
                WorkDomain::Baseline
            )))
        );
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
        drop(budget);
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(
            ledger.peak_retained_bytes(),
            if limit == 0 { 0 } else { 12 }
        );
    }
}

#[test]
fn deadline_refusal_remains_typed_without_allocating() {
    let mut ledger = ledger(100_000, 100_000);
    ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(100_000));
    let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
    assert_eq!(
        StringValue::decode_template_admitted("\r\n", &mut budget).unwrap_err(),
        StringDecodeError::Resources(AllocationError::Budget(BudgetError::DeadlineExceeded))
    );
    drop(budget);
    assert_eq!(ledger.peak_retained_bytes(), 0);
    assert_eq!(ledger.retained_bytes(), 0);
}
