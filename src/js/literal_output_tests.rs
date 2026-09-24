//! Target shape/output ownership tests, independent of source demand inference.
use super::extract::{Output, OutputError};
use super::naming::{Plan, Style};
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use std::process::Command;

fn policy(compact: bool) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[policy.tactics]\ntarget-compaction='{}'\nidentifier-mangling='on'\nnaming-search='on'",
        if compact { "on" } else { "off" }
    ))
    .unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
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
fn string(module: &mut Module, text: impl Into<StringValue>) -> ExprId {
    module.expression(Expr::Literal(Literal::String(text.into())), None)
}
fn record(module: &mut Module, text: &str) -> ExprId {
    let callee = module.expression(Expr::Host("record".into()), None);
    let argument = string(module, text);
    module.expression(
        Expr::Call {
            callee,
            arguments: vec![argument],
            invocation: Invocation::Reference,
        },
        None,
    )
}
fn fixture() -> (Module, Vec<LiteralAlternative>) {
    let mut module = Module::default();
    let mut rows = Vec::new();
    for (text, observation, op, event) in [
        (
            StringValue::from("truth"),
            WeakLiteralObservation::Truthy,
            Binary::And,
            "truth",
        ),
        (
            StringValue::from(""),
            WeakLiteralObservation::Truthy,
            Binary::And,
            "bad-empty",
        ),
        (
            StringValue::from("nonnull"),
            WeakLiteralObservation::Nullish,
            Binary::Nullish,
            "bad-nullish",
        ),
        (
            StringValue::from(""),
            WeakLiteralObservation::Nullish,
            Binary::Nullish,
            "bad-empty-nullish",
        ),
        (
            StringValue::from_utf16(vec![0xd800]),
            WeakLiteralObservation::Truthy,
            Binary::And,
            "utf16",
        ),
    ] {
        let left = string(&mut module, text);
        rows.push(LiteralAlternative::new(left, observation));
        let right = record(&mut module, event);
        let value = module.expression(Expr::Binary { op, left, right }, None);
        module.regions[0]
            .statements
            .push(Statement::Evaluate(value));
    }
    let exact = record(&mut module, "exact-survives");
    module.regions[0]
        .statements
        .push(Statement::Evaluate(exact));
    (module, rows)
}
fn render(output: &Output<'_>, mode: LiteralOutput, style: Style) -> (String, LiteralOutput) {
    const OWNER: u32 = 73;
    let (text, charge, actual) = output
        .render_with_literals_admitted(&Plan::new(style), mode, usize::MAX, OWNER)
        .unwrap();
    // Explicit test-owned diagnostic copy; compiler text drops before release.
    let observed = text.clone();
    drop(text);
    output.with_allocation_budget(|budget| {
        budget.with_ledger(|ledger| charge.discard(&OWNER, ledger.unwrap().0).unwrap())
    });
    (observed, actual)
}
fn execute(javascript: &str) -> serde_json::Value {
    let script=format!("const events=[];globalThis.record=x=>{{events.push(x);return x;}};await import('data:text/javascript,'+encodeURIComponent({}));process.stdout.write(JSON.stringify(events));",serde_json::to_string(javascript).unwrap());
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for literal output tests");
    assert!(
        result.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}
#[test]
fn one_prepared_output_renders_original_and_observed_without_mutating_literals() {
    let (module, rows) = fixture();
    let before = module.clone();
    let mut ledger = ledger(1_000_000, 10_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget)
            .unwrap();
        assert!(output.has_literal_alternative_admitted().unwrap());
        for style in [Style::Source, Style::Global, Style::Scoped] {
            let (original, actual) = render(&output, LiteralOutput::Original, style);
            assert_eq!(actual, LiteralOutput::Original);
            let (observed, actual) = render(&output, LiteralOutput::Observed, style);
            assert_eq!(actual, LiteralOutput::Observed);
            assert_ne!(original, observed);
            assert!(original.contains("\\ud800"));
            assert!(!observed.contains("\\ud800"));
            assert!(observed.contains("exact-survives"));
            assert_eq!(
                execute(&original),
                serde_json::json!(["truth", "utf16", "exact-survives"])
            );
            assert_eq!(
                execute(&observed),
                serde_json::json!(["truth", "utf16", "exact-survives"])
            );
        }
    }
    assert_eq!(module, before);
    assert_eq!(ledger.retained_bytes(), 0);
    assert_eq!(ledger.work_used(WorkDomain::Baseline), 0);
}
#[test]
fn unreachable_rows_have_no_alternative_and_resolve_to_original() {
    let mut module = Module::default();
    let dead = string(&mut module, "unreachable");
    let rows = [LiteralAlternative::new(
        dead,
        WeakLiteralObservation::Truthy,
    )];
    let mut ledger = ledger(100_000, 100_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget)
            .unwrap();
        assert!(!output.has_literal_alternative_admitted().unwrap());
        let (text, mode) = render(&output, LiteralOutput::Observed, Style::Global);
        assert_eq!(mode, LiteralOutput::Original);
        assert!(text.is_empty());
    }
    assert_eq!(ledger.retained_bytes(), 0);
}
#[test]
fn malformed_sparse_locations_are_rejected_before_prepared_output_escapes() {
    let mut module = Module::default();
    let first = string(&mut module, "first");
    let second = string(&mut module, "second");
    let number = module.expression(Expr::Literal(Literal::Number(0.0)), None);
    let row = |expression| LiteralAlternative::new(expression, WeakLiteralObservation::Truthy);
    for rows in [
        vec![row(first), row(first)],
        vec![row(second), row(first)],
        vec![row(number)],
        vec![row(ExprId::new(1000))],
    ] {
        let mut ledger = ledger(100_000, 100_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let result =
                module.prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget);
            assert!(matches!(
                result,
                Err(OutputError::Invalid("invalid observed literal occurrence"))
            ));
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}
#[test]
fn observed_output_checks_permission_even_if_the_recipe_is_inactive() {
    let (module, rows) = fixture();
    for rows in [rows.as_slice(), &[]] {
        let mut ledger = ledger(1_000_000, 1_000_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let output = module
                .prepare_output_with_literals_admitted(&policy(false), rows, &mut budget)
                .unwrap();
            assert!(!output.has_literal_alternative_admitted().unwrap());
            let result = output.render_with_literals_admitted(
                &Plan::new(Style::Global),
                LiteralOutput::Observed,
                usize::MAX,
                1u32,
            );
            assert!(matches!(
                result,
                Err(OutputError::Invalid(
                    "observed literal output requires target-compaction permission"
                ))
            ));
            let (_, actual) = render(&output, LiteralOutput::Original, Style::Global);
            assert_eq!(actual, LiteralOutput::Original);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}
#[test]
fn selected_literal_shape_preserves_receiver_syntax_and_avoids_directives() {
    // Deliberately exercise target lexical shape independently of source-proof
    // eligibility. This test does not certify a weak receiver observation.
    let mut module = Module::default();
    let first = string(&mut module, "use strict");
    module.regions[0]
        .statements
        .push(Statement::Evaluate(first));
    let receiver = string(&mut module, "");
    let member = module.expression(
        Expr::Member {
            object: receiver,
            property: Property::Named("x".into()),
        },
        None,
    );
    module.regions[0]
        .statements
        .push(Statement::Evaluate(member));
    let rows = [
        LiteralAlternative::new(first, WeakLiteralObservation::Truthy),
        LiteralAlternative::new(receiver, WeakLiteralObservation::Nullish),
    ];
    let mut ledger = ledger(100_000, 100_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget)
            .unwrap();
        let (original, _) = render(&output, LiteralOutput::Original, Style::Global);
        let (observed, _) = render(&output, LiteralOutput::Observed, Style::Global);
        assert!(original.starts_with("(\"use strict\");"), "{original}");
        assert!(observed.contains("(0).x"), "{observed}");
        assert_eq!(execute(&original), serde_json::json!([]));
        assert_eq!(execute(&observed), serde_json::json!([]));
    }
    assert_eq!(ledger.retained_bytes(), 0);
}
#[test]
fn failed_render_and_unwind_release_output_owners_and_leave_reusable_preparation() {
    let (module, rows) = fixture();
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget)
            .unwrap();
        let (original, _) = render(&output, LiteralOutput::Original, Style::Scoped);
        let before = output
            .with_allocation_budget(|budget| budget.retained_bytes(AllocationClass::Retained));
        assert!(matches!(
            output.render_with_literals_admitted(
                &Plan::new(Style::Scoped),
                LiteralOutput::Observed,
                1,
                11u32
            ),
            Err(OutputError::ByteLimit)
        ));
        assert_eq!(
            output
                .with_allocation_budget(|budget| budget.retained_bytes(AllocationClass::Retained)),
            before
        );
        let (again, _) = render(&output, LiteralOutput::Original, Style::Scoped);
        assert_eq!(original, again);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let output = module
            .prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget)
            .unwrap();
        assert!(output.has_literal_alternative_admitted().unwrap());
        panic!("test output-scope unwind");
    }));
    assert!(panic.is_err());
    assert_eq!(ledger.retained_bytes(), 0);
    for (memory, work) in [(0, 100_000), (100_000, 0)] {
        let mut denied = ledger_with_limits(memory, work);
        {
            let mut budget = AllocationBudget::new(Some((&mut denied, WorkDomain::Optional)));
            assert!(matches!(
                module.prepare_output_with_literals_admitted(&policy(true), &rows, &mut budget),
                Err(OutputError::Admission(_))
            ));
        }
        assert_eq!(denied.retained_bytes(), 0);
    }
}
fn ledger_with_limits(memory: u64, work: u64) -> BudgetLedger {
    ledger(memory, work)
}
