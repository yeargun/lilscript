use super::*;
use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
use crate::output_budget::AllocationClass::{Retained, Scratch};

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

/// Isolate each nested owner: outer rows fit 8 KiB, their one large payload
/// does not. These storage fixtures need no executable program around them.
fn payloads() -> Vec<(&'static str, Module)> {
    let mut cases = Vec::new();
    let text = "x".repeat(16_384);
    let id = ExprId::new(0);
    for (label, expression) in [
        (
            "unicode literal",
            Expr::Literal(Literal::String(text.clone().into())),
        ),
        (
            "unpaired utf16",
            Expr::Literal(Literal::String(StringValue::from_utf16(vec![0xd800; 8192]))),
        ),
        ("host name", Expr::Host(Host::new(text.clone()))),
        (
            "member property",
            Expr::Member {
                object: id,
                property: Property::Named(text.clone()),
            },
        ),
        (
            "arguments",
            Expr::Call {
                callee: id,
                arguments: vec![id; 4096],
                invocation: Invocation::Value,
            },
        ),
        (
            "template",
            Expr::Template(vec![TemplatePart::String(text.clone().into())]),
        ),
        (
            "object key",
            Expr::Object(vec![(Property::Named(text.clone()), id)]),
        ),
        (
            "class name",
            Expr::Class {
                name: text.clone(),
                base: None,
                constructor: None,
                methods: vec![],
                members: vec![],
            },
        ),
        (
            "class method",
            Expr::Class {
                name: "C".into(),
                base: None,
                constructor: None,
                methods: vec![(text.clone(), FunctionId::new(0))],
                members: vec![],
            },
        ),
        ("regexp", Expr::Regex(text.clone())),
        (
            "module specifier",
            Expr::LoadModule {
                module: 0,
                specifier: text.clone(),
                members: vec![],
                promise: id,
                string: id,
            },
        ),
        (
            "module member",
            Expr::LoadModule {
                module: 0,
                specifier: "./lib".into(),
                members: vec![(text.clone(), id)],
                promise: id,
                string: id,
            },
        ),
    ] {
        let mut module = Module::default();
        module.expression(expression, None);
        cases.push((label, module));
    }
    for label in [
        "region statements",
        "function parameters",
        "function name",
        "binding spelling",
        "import",
        "export",
        "entry",
        "reserved",
        "carried",
        "choice",
        "print defaults",
        "delivery name",
    ] {
        let mut module = Module::default();
        match label {
            "region statements" => {
                module.regions[0].statements = vec![Statement::Evaluate(id); 1024]
            }
            "function parameters" | "function name" => module.functions.push(Function {
                parameters: if label == "function parameters" {
                    vec![BindingId::new(0); 4096]
                } else {
                    vec![]
                },
                name: if label == "function name" {
                    FunctionName::Exact(text.clone().into())
                } else {
                    FunctionName::Unobserved
                },
                rest: false,
                body: RegionId::new(0),
                arrow: true,
                strict: false,
                length: None,
                suspension: Suspension::None,
            }),
            "binding spelling" => {
                module.binding(Binding {
                    source_symbol: None,
                    scope: ScopeId::new(0),
                    spelling: text.clone(),
                    pinned: false,
                    class: None,
                    defined: false,
                });
            }
            "import" => module.imports.push(Import {
                source: text.clone().into(),
                imported: text.clone(),
                binding: BindingId::new(0),
            }),
            "export" => module.exports.push(Export {
                name: text.clone(),
                binding: BindingId::new(0),
            }),
            "entry" => module.entries.push(EntryPublic {
                name: text.clone(),
                exports: vec![0],
            }),
            "reserved" => module.reserved.push(text.clone()),
            "carried" => module.carried.push(text.clone()),
            "choice" => module.choice_sites.push(ChoiceSite {
                estimate_codec: crate::config::CompressionCostModel::Raw,
                key: ChoiceKey {
                    family: ChoiceFamily::DataEncoding,
                    site: SiteId::Formed(0),
                },
                name: text.clone(),
                alternatives: vec![],
                seed: AltId(0),
                applied: AltId(0),
                pinned: false,
            }),
            "print defaults" => {
                module.print_forms = Some(spellings::PrintForms {
                    defaults: vec![spellings::Defaults {
                        values: vec![Some(id); 4096],
                        absorbed: 0,
                    }],
                    ..Default::default()
                })
            }
            "delivery name" => {
                module.delivery = Some(delivery::DeliveryPlan {
                    mode: crate::config::DeliveryMode::Single,
                    files: vec![],
                    entries: vec![],
                    setters: vec![],
                    statics: 1,
                    entry_names: vec![text.clone()],
                    naming: vec![],
                    preload: crate::config::PreloadPolicy::None,
                    format: crate::config::JavaScriptFormat::Esm,
                    container: crate::compilation_policy::ContainerContract {
                        global:Some(text.clone()),globals:vec![(text.clone(),text.clone())],..Default::default()
                    },
                    request_bytes: 0,
                    depth_bytes: 0,
                })
            }
            _ => unreachable!(),
        }
        cases.push((label, module));
    }
    cases
}

#[test]
fn q2_target_copies_admit_each_nested_owner_and_rollback_refusals() {
    for (label, source) in payloads() {
        let original = source.clone();
        for (work, bytes, succeeds) in [
            (0, 1_000_000, false),
            (1_000_000, 8192, false),
            (1_000_000, 1_000_000, true),
        ] {
            let mut ledger = ledger(work, bytes);
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                budget.retain(Retained, 64).unwrap();
                let result = source.clone_in(&mut budget);
                assert_eq!(result.is_ok(), succeeds, "{label}: {result:?}");
                assert_eq!(source, original, "{label}");
                match result {
                    Ok(copy) => {
                        assert_eq!(copy, source, "{label}");
                        assert!(budget.retained_bytes(Retained) > 16_384, "{label}");
                        drop(copy);
                    }
                    Err(_) => assert_eq!(budget.retained_bytes(Retained), 64, "{label}"),
                }
                assert_eq!(budget.retained_bytes(Scratch), 0, "{label}");
            }
            assert_eq!(ledger.retained_bytes(), 0, "{label}");
        }
    }
}

#[test]
fn q2_copied_unicode_and_proof_rows_keep_runtime_and_release_replaced_payloads() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, r#"string value="\ud800é";print(value.length);print(value.charCodeAt(0));print(value.charCodeAt(1));"#).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = crate::program::from_checked_source(&syntax, &semantics).unwrap();
    let source = program.to_javascript().unwrap();
    let mut ledger = ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        let copy = source.clone_in(&mut budget).unwrap();
        copy.verify().unwrap();
        let code = copy.render(PrintPolicy::default()).unwrap();
        let output = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &code])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "2\n55296\n233\n");
        drop(copy);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    let mut module = Module::default();
    let id = module.expression(
        Expr::Literal(Literal::String("proof".repeat(100).into())),
        None,
    );
    let plain = module.expression(Expr::Literal(Literal::Number(1.0)), None);
    let behavior = Behaviour {
        reads: false,
        writes: false,
        throws: false,
        diverges: false,
        reenters: false,
        suspends: false,
    };
    let mut ledger = self::ledger(1_000_000, 1_000_000);
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        module
            .record_behaviour_in(id, behavior, &mut budget)
            .unwrap();
        let retained = budget.retained_bytes(Retained);
        for _ in 0..100 {
            module
                .record_behaviour_in(id, behavior, &mut budget)
                .unwrap();
            assert_eq!(budget.retained_bytes(Retained), retained);
        }
        module.copy_behaviour_in(plain, id, &mut budget).unwrap();
        assert_eq!(budget.retained_bytes(Retained), retained - 500);
        module
            .record_behaviour_in(id, behavior, &mut budget)
            .unwrap();
        assert_eq!(budget.retained_bytes(Retained), retained);
        module.regions[0]
            .statements
            .push(Statement::Evaluate(plain));
        module.renumber(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(Retained), retained - 500);
        drop(module);
    }
    assert_eq!(ledger.retained_bytes(), 0);
}
