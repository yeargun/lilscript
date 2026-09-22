use super::naming::{Plan, Style};
use super::selection::{Budget, Objective, Objectives};
use super::*;
use crate::compilation_policy::{CompilationRequest, ResolvedPolicy};
use std::process::Command;

fn policy(tactics: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig =
        toml::from_str(&format!("[policy.tactics]\n{tactics}")).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

fn fixture() -> (Module, BindingId) {
    let mut module = Module::default();
    let state = module.binding(Binding {
        source_symbol: Some(SymbolId(0)),
        scope: ScopeId::new(0),
        spelling: "descriptiveState".into(),
        pinned: false,
    });
    let reader = module.binding(Binding {
        source_symbol: Some(SymbolId(1)),
        scope: ScopeId::new(0),
        spelling: "read".into(),
        pinned: false,
    });
    let body = module.region(ScopeId::new(0));
    let value = module.expression(Expr::Binding(state), None);
    module.regions[body.index()]
        .statements
        .push(Statement::Return(Some(value)));
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters: vec![],
        body,
        arrow: false,
        name: FunctionName::Exact("read".into()),
        strict: false,
        length: None,
        suspension: crate::structured_js::Suspension::None,
    });
    let initial = module.expression(Expr::Literal(Literal::Number(7.0)), None);
    module.regions[0].statements = vec![
        Statement::Let {
            binding: state,
            value: Some(initial),
        },
        Statement::Function {
            binding: reader,
            function,
        },
    ];
    module.exports = vec![
        Export {
            binding: state,
            name: "state".into(),
        },
        Export {
            binding: reader,
            name: "read".into(),
        },
    ];
    (module, state)
}

fn check_runtime(javascript: &str) {
    let script = format!(
        "const m=await import('data:text/javascript,'+encodeURIComponent({}));console.log(m.state,m.read(),m.read.name,m.read.length);",
        serde_json::to_string(javascript).unwrap(),
    );
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for output policy tests");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "7 7 read 0\n");
}

#[test]
fn identifier_mangling_off_restricts_render_and_search_to_source() {
    let (module, state) = fixture();
    for naming_search in ["on", "off"] {
        let resolved = policy(&format!(
            "identifier-mangling='off'\nnaming-search='{naming_search}'"
        ));
        let output = module.prepare_output_with_policy(&resolved).unwrap();
        let source = output.render(&Plan::new(Style::Source)).unwrap();
        assert!(source.contains("descriptiveState"));
        for style in [Style::Global, Style::Scoped] {
            assert!(output
                .render(&Plan::new(style))
                .unwrap_err()
                .contains("identifier mangling is disabled"));
        }
        let mut override_plan = Plan::new(Style::Source);
        override_plan.source_names.push(state);
        assert!(output.render(&override_plan).is_err());
        let selected = output
            .select(
                Budget {
                    plans: 512,
                    candidate_bytes: 100_000,
                },
                Objectives::All,
                crate::compression::measure,
            )
            .unwrap();
        assert_eq!(selected.proposal_steps, 1);
        assert_eq!(selected.attempts.len(), 1);
        assert_eq!(selected.candidates.len(), 1);
        assert_eq!(selected.measurement_calls, 2);
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let winner = selected.winner(codec).unwrap();
            assert_eq!(winner.plan, Plan::new(Style::Source));
            assert_eq!(winner.javascript, source);
        }
        check_runtime(&source);
    }
}

#[test]
fn naming_search_off_keeps_one_mangled_baseline_and_rejects_manual_overrides() {
    let (module, state) = fixture();
    let output = module
        .prepare_output_with_policy(&policy("identifier-mangling='on'\nnaming-search='off'"))
        .unwrap();
    let global = output.render(&Plan::new(Style::Global)).unwrap();
    assert!(!global.contains("descriptiveState"));
    for style in [Style::Source, Style::Scoped] {
        assert!(output
            .render(&Plan::new(style))
            .unwrap_err()
            .contains("naming search is disabled"));
    }
    let mut override_plan = Plan::new(Style::Global);
    override_plan.source_names.push(state);
    assert!(output
        .render(&override_plan)
        .unwrap_err()
        .contains("naming search is disabled"));
    let selected = output
        .select(
            Budget {
                plans: 512,
                candidate_bytes: 100_000,
            },
            Objectives::One(Objective::Raw),
            |_, _| panic!("raw selection does not probe a codec"),
        )
        .unwrap();
    assert_eq!(selected.proposal_steps, 1);
    assert_eq!(selected.attempts.len(), 1);
    assert_eq!(selected.winner(Objective::Raw).unwrap().javascript, global);
    check_runtime(&global);
}

#[test]
fn enabled_search_retains_alternatives_and_prepared_permissions_do_not_change() {
    let (module, state) = fixture();
    let constrained = module
        .prepare_output_with_policy(&policy("identifier-mangling='off'\nnaming-search='off'"))
        .unwrap();
    let output = module
        .prepare_output_with_policy(&policy("identifier-mangling='on'\nnaming-search='on'"))
        .unwrap();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        output.render(&Plan::new(style)).unwrap();
    }
    let mut override_plan = Plan::new(Style::Global);
    override_plan.source_names.push(state);
    assert!(output
        .render(&override_plan)
        .unwrap()
        .contains("descriptiveState"));
    let selected = output
        .select(
            Budget {
                plans: 16,
                candidate_bytes: 100_000,
            },
            Objectives::One(Objective::Raw),
            |_, _| unreachable!(),
        )
        .unwrap();
    for style in [Style::Global, Style::Scoped, Style::Source] {
        assert!(selected
            .attempts
            .iter()
            .any(|attempt| attempt.plan == Plan::new(style)));
    }
    assert!(selected
        .attempts
        .iter()
        .any(|attempt| !attempt.plan.source_names.is_empty()));
    assert!(constrained.render(&Plan::new(Style::Global)).is_err());
    assert!(constrained.render(&override_plan).is_err());
}

#[test]
fn policy_free_research_output_stays_explicit_and_native_policy_is_rejected() {
    let (module, _) = fixture();
    let research = module.prepare_output().unwrap();
    for style in [Style::Source, Style::Global, Style::Scoped] {
        research.render(&Plan::new(style)).unwrap();
    }
    let native = crate::config::ProjectConfig::default()
        .resolve_policy(CompilationRequest::Native)
        .unwrap();
    assert!(
        matches!(module.prepare_output_with_policy(&native), Err(reason) if reason.contains("JavaScript compilation policy"))
    );
}

fn edition(year: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig =
        toml::from_str(&format!("[javascript]\necmascript='{year}'")).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}

#[test]
fn target_edition_backstop_rejects_live_nullish_but_allows_unused_arena_nodes() {
    let mut module = Module::default();
    let left = module.expression(Expr::Literal(Literal::Null), None);
    let right = module.expression(Expr::Literal(Literal::Number(3.0)), None);
    let value = module.expression(
        Expr::Binary {
            op: Binary::Nullish,
            left,
            right,
        },
        None,
    );
    module
        .prepare_output_with_policy(&edition("es2015"))
        .unwrap();
    module.regions[0]
        .statements
        .push(Statement::Evaluate(value));
    assert!(
        matches!(module.prepare_output_with_policy(&edition("es2019")), Err(reason) if reason.contains("es2020"))
    );
    module
        .prepare_output_with_policy(&edition("es2020"))
        .unwrap();
    // Inspection remains available; it does not certify an edition contract.
    module.prepare_output().unwrap();
}

#[test]
fn target_edition_backstop_checks_reachable_optional_catch_binding() {
    let mut module = Module::default();
    let block = module.region(ScopeId::new(0));
    let scope = module.regions[block.index()].scope;
    let body = module.region(scope);
    let catch = module.region(scope);
    module.regions[block.index()]
        .statements
        .push(Statement::Try {
            body,
            catch: Some(Catch {
                binding: None,
                body: catch,
            }),
            finally: None,
        });
    module
        .prepare_output_with_policy(&edition("es2015"))
        .unwrap();
    module.regions[0].statements.push(Statement::Block(block));
    assert!(
        matches!(module.prepare_output_with_policy(&edition("es2018")), Err(reason) if reason.contains("es2019"))
    );
    module
        .prepare_output_with_policy(&edition("es2019"))
        .unwrap();
}
