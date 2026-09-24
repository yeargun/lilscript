//! Script parameter storage may also be observed through mapped arguments.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Plan, Style};

fn check_arguments(module: bool) {
    let source = r#"
        extern JsValue arguments;
        extern JsValue event(string label);
        extern void observe(JsValue value);
        void run(string value){
            value="secret";
            JS.and(value,event("gate"));
            observe(arguments[0]);
        }
        run("initial");
    "#;
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let mut config = crate::config::ProjectConfig::default();
    config.javascript.strip_console = false;
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: module,
        })
        .unwrap();
    assert_eq!(
        policy.javascript_contract().unwrap().execution,
        if module {
            crate::compilation_contract::JavaScriptExecution::Module
        } else {
            crate::compilation_contract::JavaScriptExecution::Script
        }
    );
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 20_000_000,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 2 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let candidate = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    compiler.with_javascript_output(candidate, &policy, |output| {
        assert_eq!(output.has_literal_alternative().unwrap(), module);
        for mode in [LiteralOutput::Original, LiteralOutput::Observed] {
            for style in [Style::Source, Style::Global, Style::Scoped] {
                let artifact = output.render_bounded_with_literals(&Plan::new(style), mode, usize::MAX).unwrap();
                let javascript = output.take_artifact(artifact).unwrap();
                let encoded = serde_json::to_string(&javascript).unwrap();
                let load = if module {
                    format!("await import('data:text/javascript,'+encodeURIComponent({encoded}));")
                } else {
                    format!("Function({encoded})();")
                };
                let script = format!(
                    "const events=[];globalThis.event=x=>{{events.push(x)}};globalThis.observe=x=>events.push(x);{load}process.stdout.write(JSON.stringify(events));"
                );
                let result = std::process::Command::new("node").args(["--input-type=module", "-e", &script]).output().unwrap();
                assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
                assert_eq!(serde_json::from_slice::<serde_json::Value>(&result.stdout).unwrap(),
                    serde_json::json!(["gate", if module { "initial" } else { "secret" }]), "module={module} {mode:?}/{style:?}: {javascript}");
            }
        }
    }).unwrap();
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn mapped_arguments_observe_parameter_writes_in_every_literal_output_mode() {
    check_arguments(false);
    check_arguments(true);
}
