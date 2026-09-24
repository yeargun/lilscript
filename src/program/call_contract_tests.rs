//! Retained call knowledge is checked before target formation. Runtime
//! expectations include replaceable host methods, rather than pristine-only
//! equivalence between omitted arguments and their nominal default values.
use super::publication::*;
use super::uses::{UseIndex, ValueUse};
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Plan, Sizes, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::process::Command;

const PRIMITIVE_SOURCE: &str = include_str!("fixtures/call-contract/primitive-calls.lil");
const MARKED_SOURCE: &str = include_str!("../../finer/tools/fixtures/marked-brackets.lil");

fn checked<R>(source: &str, inspect: impl FnOnce(&Program<'_>) -> R) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(&program)
}

fn ledger() -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 100_000_000,
            optional_work: 100_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 256_000_000,
        },
    )
    .unwrap()
}

struct Emission {
    compact: bool,
    style: Style,
    javascript: String,
    sizes: Sizes,
}

fn artifacts(source: &str) -> Vec<Emission> {
    let mut emitted = Vec::new();
    for compact in [false, true] {
        let config: crate::config::ProjectConfig = toml::from_str(&format!(
            "[javascript]\nstrip_console=false\n[policy.tactics]\ntarget-compaction='{}'\n",
            if compact { "on" } else { "off" },
        ))
        .unwrap();
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let mut compilation = Compilation::new(ledger(), CheckpointLimit { max_live: 2 }).unwrap();
        let source = compilation
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let candidate = compilation
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        compilation
            .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
            .unwrap();
        for style in [Style::Global, Style::Scoped, Style::Source] {
            let retained = compilation.ledger().retained_bytes();
            let (javascript, sizes) = compilation
                .with_javascript_output(candidate, &policy, |output| {
                    let artifact = output.render(&Plan::new(style))?;
                    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
                        output.measure(artifact, codec)?;
                    }
                    let sizes = output.with_artifact(artifact, |view| view.sizes)?;
                    Ok::<_, CandidateError>((output.take_artifact(artifact)?, sizes))
                })
                .unwrap()
                .unwrap();
            assert_eq!(compilation.ledger().retained_bytes(), retained);
            assert_eq!(sizes.raw, javascript.len());
            assert!(sizes.gzip9.is_some() && sizes.brotli11.is_some());
            emitted.push(Emission {
                compact,
                style,
                javascript,
                sizes,
            });
        }
        assert_eq!(compilation.finish().retained_bytes(), 0);
    }
    emitted
}

fn execute(emission: &Emission, before_import: &str, after_import: &str) -> Json {
    // Prototype replacement happens after import unless a fixture's top-level
    // source requires its host setup. Node's loader is not part of the trace.
    let script = format!(
        "const events=[];{before_import}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{after_import}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(&emission.javascript).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for call contract runtime tests");
    assert!(
        output.status.success(),
        "compact={} style={:?}: {}\n{}",
        emission.compact,
        emission.style,
        String::from_utf8_lossy(&output.stderr),
        emission.javascript,
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn method_site(program: &Program<'_>) -> (UnitId, CallId) {
    program
        .units()
        .iter()
        .find_map(|unit| {
            unit.data()
                .calls
                .iter()
                .enumerate()
                .find_map(|(index, site)| {
                    matches!(
                        site.target,
                        CallTarget::Intrinsic {
                            operation: ResolvedIntrinsic::Method(
                                crate::primitive::Intrinsic::StringIndexOf
                            ),
                            ..
                        }
                    )
                    .then_some((unit.id(), CallId::from_index(index).unwrap()))
                })
        })
        .unwrap()
}

#[test]
fn omitted_method_operands_retain_checked_signature_and_complete_uses() {
    checked(PRIMITIVE_SOURCE, |program| {
        let mut budget = ledger();
        let uses = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let mut arities = Vec::new();
        for unit in program.units() {
            for (index, site) in unit.data().calls.iter().enumerate() {
                let CallTarget::Intrinsic {
                    operation,
                    receiver: Some(receiver),
                } = site.target
                else {
                    continue;
                };
                let call = CallId::from_index(index).unwrap();
                let local = uses.unit(unit.id()).unwrap();
                let operation_id = local.call_operation(call).unwrap();
                let invocation = &unit.data().operations[operation_id.index()];
                assert_eq!(invocation.operands.len, 0);
                let arguments = unit.data().arguments(site.arguments).unwrap();
                assert_eq!(site.contract.defaults, DefaultConvention::PreserveOmission);
                assert_eq!(site.contract.supplied as usize, arguments.len());
                let ty = &program.types[site.contract.signature.unwrap().index()];
                assert!(crate::primitive::intrinsic_call_contract(operation)
                    .unwrap()
                    .matches(ty));
                assert!(local.value_uses(receiver).unwrap().iter().any(|usage| {
                    matches!(usage, ValueUse::CallReceiver { call: found, .. } if *found == call)
                }));
                for (position, argument) in arguments.iter().enumerate() {
                    let CallArgument::Value(value) = *argument else {
                        panic!("value intrinsic argument")
                    };
                    assert!(local
                        .value_uses(value)
                        .unwrap()
                        .contains(&ValueUse::CallArgument {
                            operation: operation_id,
                            call,
                            position: position as u32,
                        }));
                }
                arities.push(site.contract.supplied);
            }
        }
        assert_eq!(arities, [1, 2]);
        uses.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn replaceable_method_preserves_arity_lookup_exceptions_and_reentry() {
    let expected: Json = serde_json::from_str(include_str!(
        "fixtures/call-contract/primitive-calls.expected.json"
    ))
    .unwrap();
    for emission in artifacts(PRIMITIVE_SOURCE) {
        assert_eq!(
            execute(
                &emission,
                "",
                include_str!("fixtures/call-contract/primitive-calls.host.js")
            ),
            expected,
            "compact={} style={:?}",
            emission.compact,
            emission.style,
        );
    }
}

#[test]
fn fully_supplied_default_signatures_preserve_explicit_values_and_host_call_arity() {
    let source = r#"
        extern int choose(int value=7);
        extern JsValue receive(JsValue value=7);
        extern JsValue explicitValue();
        export int direct(){return choose(3);}
        export int throughValue(){auto callback=choose;return callback(4);}
        export JsValue explicit(){return receive(explicitValue());}
    "#;
    let host = r#"
        globalThis.choose=function(value){events.push(["choose",arguments.length,value]);return value;};
        globalThis.receive=function(value){events.push(["receive",arguments.length,value===undefined]);return value;};
        globalThis.explicitValue=()=>{events.push(["explicit-value"]);return undefined;};
        events.push(["direct",library.direct()]);
        events.push(["through-value",library.throughValue()]);
        events.push(["explicit",library.explicit()===undefined]);
        events.push(["public",library.direct.length,library.throughValue.length,library.explicit.length]);
    "#;
    let expected = json!([
        ["choose", 1, 3],
        ["direct", 3],
        ["choose", 1, 4],
        ["through-value", 4],
        ["explicit-value"],
        ["receive", 1, true],
        ["explicit", true],
        ["public", 0, 0, 0]
    ]);
    for emission in artifacts(source) {
        assert_eq!(execute(&emission, "", host), expected);
    }
}

#[test]
fn archived_marked_bracket_source_runs_without_adaptation_through_common_output_owner() {
    let origin: Json = serde_json::from_str(include_str!(
        "../../finer/tools/fixtures/marked-brackets.origin.json"
    ))
    .unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(MARKED_SOURCE.as_bytes())),
        origin["fixtureSha256"].as_str().unwrap(),
    );
    let setup = format!(
        "globalThis.sample=index=>{}[index];console.log=value=>events.push(String(value));",
        origin["samples"],
    );
    let expected = json!(["0", "7", "-1", "-2", "4", "2"]);
    for emission in artifacts(MARKED_SOURCE) {
        assert_eq!(execute(&emission, &setup, ""), expected);
        eprintln!(
            "call-contract-artifact {}",
            json!({
                "case":"archived-marked-brackets",
                "fixture_sha256":origin["fixtureSha256"],
                "compact":emission.compact,
                "style":format!("{:?}",emission.style),
                "javascript":emission.javascript,
                "raw":emission.sizes.raw,
                "gzip9":emission.sizes.gzip9,
                "brotli11":emission.sizes.brotli11,
            })
        );
    }
}

#[test]
fn verifier_rejects_missing_forged_or_inconsistent_call_contracts() {
    let source = r#"extern int different(string value,int position=7);
        export int find(string value){return value.indexOf("x");}"#;
    checked(source, |program| {
        let (unit, call) = method_site(program);
        let different = program
            .cells
            .iter()
            .find(|cell| cell.name == "different")
            .unwrap()
            .ty;
        let int = TypeId::from_index(
            program
                .types
                .iter()
                .position(|ty| ty == &Type::Int)
                .unwrap(),
        )
        .unwrap();
        for change in 0..5 {
            let mut broken = program.clone();
            let mut working = broken.units[unit.index()].clone().into_working();
            let site = &mut working.get_mut().calls[call.index()];
            match change {
                0 => site.contract.signature = None,
                1 => site.contract.signature = Some(int),
                2 => site.contract.signature = Some(different),
                3 => site.contract.supplied = 2,
                4 => site.contract.defaults = DefaultConvention::MaterializeAtCaller,
                _ => unreachable!(),
            }
            broken.units[unit.index()] = working.freeze();
            assert!(
                broken.verify().is_err(),
                "accepted malformed contract {change}"
            );
        }
    });
    // Relabeling the supplied `3` as the omitted default `7`: a LilScript
    // callee checks the claimed default evaluation, and a host call, which
    // preserves omissions, has no synthesized argument to claim.
    for (source, message) in [
        (
            "int choose(int value=7){return value;}print(choose(3));",
            "caller default evaluations disagree",
        ),
        (
            "extern int choose(int value=7);print(choose(3));",
            "preserved omission has synthesized arguments",
        ),
    ] {
        checked(source, |program| {
            let mut broken = program.clone();
            let entry = program.initialization[0];
            let mut working = broken.units[entry.index()].clone().into_working();
            let site = working
                .get_mut()
                .calls
                .iter_mut()
                .find(|site| {
                    matches!(site.target, CallTarget::Value { .. }) && site.contract.supplied == 1
                })
                .unwrap();
            site.contract.supplied = 0;
            broken.units[entry.index()] = working.freeze();
            let error = broken.verify().unwrap_err();
            assert!(error.contains(message), "{source}: {error}");
        });
    }
}

#[test]
fn caller_default_evaluations_are_verified_and_unsupported_kinds_stay_explicit() {
    // Scalar, array and parameter defaults are evaluated by the caller after
    // every supplied argument, and the callee still guards `undefined`.
    for source in [
        "int choose(int value=7){return value;}print(choose(3));print(choose());",
        "extern int choose(int value=7);print(choose());",
        "extern int choose(int[] values=[]);print(choose());",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        from_checked_source(&syntax, &semantics)
            .unwrap_or_else(|error| panic!("{source}: {error:?}"))
            .verify()
            .unwrap();
    }
    // Struct and class defaults are evaluated by the caller too. An arrow
    // default is applied only by the guarded callee, so a caller cannot omit
    // it while supplying a later evaluated default.
    for source in [
        "struct P{int x;}int read(P value=P{2}){return value.x;}print(read());",
        "class C{int x;init(int x=3){this.x=x;}}int read(C value=new C()){return value.x;}print(read());",
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        from_checked_source(&syntax, &semantics)
            .unwrap_or_else(|error| panic!("{source}: {error:?}"))
            .verify()
            .unwrap();
    }
    let source = "int apply(func(int)->int f=(int x)=>x,int bias=1){return f(bias);}print(apply());";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    assert_eq!(
        from_checked_source(&syntax, &semantics)
            .unwrap_err()
            .feature,
        "arrow default before a caller-evaluated default"
    );
}

#[test]
fn dynamic_calls_retain_a_real_checked_type_and_builtins_cannot_hide_missing_contracts() {
    checked("extern JsValue callable;callable(7);print(1);", |program| {
        let data = program.units[0].data();
        let dynamic = data
            .calls
            .iter()
            .find(|site| matches!(site.target, CallTarget::Value { .. }))
            .unwrap();
        assert_eq!(
            program.types[dynamic.contract.signature.unwrap().index()],
            Type::TypeParameter("$js")
        );
        let mut broken = program.clone();
        let mut working = broken.units[0].clone().into_working();
        let site = working
            .get_mut()
            .calls
            .iter_mut()
            .find(|site| matches!(site.target, CallTarget::Builtin(_)))
            .unwrap();
        assert_eq!(site.contract.signature, None);
        // `JS.or`/`JS.and` convert to short-circuit operations, never calls,
        // so a call naming one has no contract to verify against.
        site.target = CallTarget::Builtin(BuiltinCall::JsOr);
        broken.units[0] = working.freeze();
        assert!(broken
            .verify()
            .unwrap_err()
            .contains("builtin has no supported semantic call contract"));
    });
}

#[test]
fn known_reference_places_cannot_claim_another_callable_default_contract() {
    for source in [
        "extern (func(int)->int)[] callbacks();extern int optional(int value=7);export int run(){return callbacks()[0](3);}",
        "struct Holder{func(int)->int callback;}extern Holder object();extern int optional(int value=7);export int run(){return object().callback(3);}",
    ] {
        checked(source, |program| {
            let optional = program.cells.iter().find(|cell| cell.name == "optional").unwrap().ty;
            let mut broken = program.clone();
            // An element is a reference place; a function held in a value
            // struct is loaded and called as a value (it has no receiver).
            let (unit, call) = broken.units.iter().find_map(|unit| {
                let data = unit.data();
                data.calls.iter().enumerate().find_map(|(index, site)| {
                    let known = match site.target {
                        CallTarget::Reference { .. } => true,
                        CallTarget::Value { callee, .. } => matches!(
                            data.operations[data.values[callee.index()].definition.index()].kind,
                            OperationKind::Load(place) if matches!(data.places[place.index()], Place::Field { .. })
                        ),
                        _ => false,
                    };
                    known.then_some((unit.id(), index))
                })
            }).unwrap();
            let mut working = broken.units[unit.index()].clone().into_working();
            working.get_mut().calls[call].contract.signature = Some(optional);
            broken.units[unit.index()] = working.freeze();
            let error = broken.verify().unwrap_err();
            assert!(error.contains("prepared place") || error.contains("prepared value"), "{error}");
        });
    }
}

#[test]
fn call_argument_edits_invalidate_the_unit_index_and_incremental_uses_match_rebuild() {
    checked(PRIMITIVE_SOURCE, |program| {
        let mut budget = ledger();
        let initial = UseIndex::build(program, &mut budget, WorkDomain::Baseline).unwrap();
        let mut revised = program.clone();
        let (unit, call) = revised
            .units
            .iter()
            .find_map(|unit| {
                unit.data()
                    .calls
                    .iter()
                    .enumerate()
                    .find_map(|(index, site)| {
                        (site.contract.defaults == DefaultConvention::PreserveOmission
                            && site.contract.supplied == 2)
                            .then_some((unit.id(), CallId::from_index(index).unwrap()))
                    })
            })
            .unwrap();
        let invocation = initial.unit(unit).unwrap().call_operation(call).unwrap();
        let mut working = revised.units[unit.index()].clone().into_working();
        let data = working.get_mut();
        let removed_index = data.calls[call.index()].arguments.start as usize + 1;
        let CallArgument::Value(removed) = data.call_arguments.remove(removed_index) else {
            panic!("value argument")
        };
        data.calls[call.index()].contract.supplied = 1;
        data.calls[call.index()].arguments.len = 1;
        for site in &mut data.calls {
            if site.arguments.start as usize > removed_index {
                site.arguments.start -= 1;
            }
        }
        revised.units[unit.index()] = working.freeze();
        revised.verify().unwrap();
        assert!(!initial.valid_for(&revised));
        let incremental = initial
            .updated(&revised, &[unit], &mut budget, WorkDomain::Optional)
            .unwrap();
        let rebuilt = UseIndex::build(&revised, &mut budget, WorkDomain::Optional).unwrap();
        assert!(incremental.valid_for(&revised));
        assert_eq!(incremental.receipt().rebuilt_units, 1);
        assert!(initial
            .unit(unit)
            .unwrap()
            .value_uses(removed)
            .unwrap()
            .contains(&ValueUse::CallArgument {
                operation: invocation,
                call,
                position: 1,
            }));
        assert!(!incremental
            .unit(unit)
            .unwrap()
            .value_uses(removed)
            .unwrap()
            .contains(&ValueUse::CallArgument {
                operation: invocation,
                call,
                position: 1,
            }));
        for frozen in revised.units() {
            let left = incremental.unit(frozen.id()).unwrap();
            let right = rebuilt.unit(frozen.id()).unwrap();
            assert_eq!(left.cell_uses(), right.cell_uses());
            assert_eq!(left.closures(), right.closures());
            for index in 0..frozen.data().values.len() {
                let value = ValueId::from_index(index).unwrap();
                assert_eq!(left.value_uses(value), right.value_uses(value));
            }
            for index in 0..frozen.data().calls.len() {
                let call = CallId::from_index(index).unwrap();
                assert_eq!(left.call_operation(call), right.call_operation(call));
            }
        }
        for index in 0..revised.cells.len() {
            let cell = CellId::from_index(index).unwrap();
            assert_eq!(
                incremental.cell(cell).unwrap().sites(),
                rebuilt.cell(cell).unwrap().sites()
            );
        }
        incremental.discard(&mut budget).unwrap();
        initial.discard(&mut budget).unwrap();
        rebuilt.discard(&mut budget).unwrap();
        assert_eq!(budget.retained_bytes(), 0);
    });
}

#[test]
fn checked_intrinsics_form_their_legacy_spelling_and_the_rest_stay_explicit() {
    checked(
        "export bool includes(string value){return value.includes(\"x\");}",
        |program| {
            let javascript = program
        .to_javascript()
        .unwrap()
        .render(crate::js::PrintPolicy::default())
        .unwrap();
            assert!(javascript.contains(".includes("), "{javascript}");
        },
    );
    // Code points count through the string iterator, as in the legacy
    // emitter, normalized like every integer intrinsic.
    checked(
        "export int points(string value){return value.codePointLength();}",
        |program| {
            let javascript = program
                .to_javascript()
                .unwrap()
                .render(crate::js::PrintPolicy::default())
                .unwrap();
            assert!(javascript.contains("[...") && javascript.contains("].length|0"), "{javascript}");
        },
    );
    checked(
        "export bool includes(string value){return value.includes(\"x\");}",
        |program| {
            assert!(program
                .units
                .iter()
                .flat_map(|unit| &unit.data().calls)
                .any(|site| {
                    matches!(
                        site.target,
                        CallTarget::Intrinsic {
                            operation: ResolvedIntrinsic::Method(
                                crate::primitive::Intrinsic::StringIncludes
                            ),
                            ..
                        }
                    ) && site.contract.signature.is_some()
                }));
        },
    );
}
