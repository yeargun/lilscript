//! Private product calls independently vary caller/callee storage, physical
//! transport and eligible shared/inlined bodies. Fixed observations precede
//! complete-artifact measurements and exhaustive-space comparison.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Objective, Objectives, Plan, Style};
use serde_json::{Value as Json, json};
use sha2::{Digest, Sha256};
use std::process::Command;

struct Case {
    name: &'static str,
    source: &'static str,
    setup: &'static str,
    observations: &'static str,
    expected: &'static str,
}
macro_rules! case {
    ($name:literal) => {
        Case {
            name: $name,
            source: include_str!(concat!("fixtures/product-calls/", $name, ".lil")),
            setup: include_str!(concat!("fixtures/product-calls/", $name, ".setup.js")),
            observations: include_str!(concat!(
                "fixtures/product-calls/",
                $name,
                ".observations.js"
            )),
            expected: include_str!(concat!("fixtures/product-calls/", $name, ".expected.json")),
        }
    };
}
fn policy(compact: bool) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ncall-specialization='on'\nscalar-replacement='on'\ninlining='on'\ntarget-compaction='{}'\n",
        if compact { "on" } else { "off" },
    ))
    .unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn compilation<'src>(optional: u64) -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: optional,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 256 },
    )
    .unwrap()
}
fn named_cell(program: &Program<'_>, name: &str) -> CellId {
    let mut matches = program
        .cells()
        .iter()
        .enumerate()
        .filter_map(|(index, cell)| {
            (cell.name == name).then(|| CellId::from_index(index).unwrap())
        });
    let cell = matches
        .next()
        .unwrap_or_else(|| panic!("missing fixture cell {name}"));
    assert!(matches.next().is_none(), "ambiguous fixture cell {name}");
    cell
}
fn request() -> ProductRequest {
    ProductRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
    }
}
fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
        local_facts: LocalFactsRequest {
            work_quota: 100_000,
            result_bytes: 100_000,
        },
    }
}
fn flat(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    cell: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .scalar_product_javascript(base, cell, request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        ProductOutcome::Published(candidate) => candidate,
        other => panic!("expected complete product component, got {other:?}"),
    }
}
fn inline(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    cell: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .inline_helper_javascript(base, cell, helper_request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("expected compatible product helper, got {other:?}"),
    }
}
fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}
fn execute(case: &Case, javascript: &str) -> Json {
    let script = format!(
        "const events=[];console.log=value=>events.push(value);\n{}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{}\nprocess.stdout.write(JSON.stringify(events));",
        case.setup,
        serde_json::to_string(javascript).unwrap(),
        case.observations
    );
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for product execution");
    assert!(
        result.status.success(),
        "{}: {}\n{script}",
        case.name,
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&result.stdout).unwrap();
    let expected: Json = serde_json::from_str(case.expected).unwrap();
    assert_eq!(observed, expected, "{}\n{script}", case.name);
    observed
}
fn artifact(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    case: &Case,
    axes: Axes,
    compact: bool,
    style: Style,
) -> Json {
    let descriptor = compiler
        .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let row=compiler.with_javascript_output(candidate,policy,|output| {
        let artifact=output.render(&Plan::new(style))?;
        let observed=output.with_artifact(artifact,|view|execute(case,view.javascript))?;
        let raw=output.measure(artifact,Objective::Raw)?;
        let gzip9=output.measure(artifact,Objective::Gzip)?;
        let brotli11=output.measure(artifact,Objective::Brotli)?;
        let text=output.take_artifact(artifact)?;
        assert_eq!(raw,text.len());
        Ok::<_,CandidateError>(json!({"case":case.name,"axes":axes.json(),"compact":compact,"style":format!("{style:?}"),"recipe_descriptor":descriptor,
            "source":case.source,"source_sha256":digest(case.source),"setup":case.setup,"observations":case.observations,
            "expected":serde_json::from_str::<Json>(case.expected).unwrap(),"observed":observed,
            "javascript":text,"javascript_sha256":digest(&text),"raw":raw,"gzip9":gzip9,"brotli11":brotli11}))
    }).unwrap().unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    row
}
#[derive(Clone, Copy, Debug)]
struct Axes {
    caller_flat: bool,
    callee_flat: bool,
    fields: bool,
    inline: bool,
}
impl Axes {
    fn json(self) -> Json {
        json!({"caller_flat":self.caller_flat,"callee_flat":self.callee_flat,"fields":self.fields,"inline":self.inline})
    }
}
const PACKED: Axes = Axes {
    caller_flat: false,
    callee_flat: false,
    fields: false,
    inline: false,
};
fn fields(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    body: UnitId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .scalar_function_javascript(base, body, request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        FunctionOutcome::Published(candidate) => candidate,
        other => panic!("expected closed private function transport, got {other:?}"),
    }
}
fn selected(
    compiler: &mut Compilation<'_>,
    direct: CandidateId,
    axes: Axes,
    state: Option<CellId>,
    parameters: &[CellId],
    helper: CellId,
    body: UnitId,
    policy: &ResolvedPolicy,
    reverse: bool,
) -> CandidateId {
    let mut candidate = direct;
    let order = if reverse { [3, 2, 1, 0] } else { [0, 1, 2, 3] };
    for action in order {
        match action {
            0 if axes.caller_flat => candidate = flat(compiler, candidate, state.unwrap(), policy),
            1 if axes.callee_flat => {
                for &cell in parameters {
                    candidate = flat(compiler, candidate, cell, policy);
                }
            }
            2 if axes.fields => candidate = fields(compiler, candidate, body, policy),
            3 if axes.inline => candidate = inline(compiler, candidate, helper, policy),
            _ => {}
        }
    }
    candidate
}
fn matrix(case: Case, callee_names: &[&str], oracle: bool, inline_adversary: bool) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let state = named_cell(&program, "state");
    let helper = named_cell(&program, "step");
    let parameters: Vec<_> = callee_names
        .iter()
        .map(|name| named_cell(&program, name))
        .collect();
    let body = program.cells()[parameters[0].index()].owner;
    assert!(
        parameters
            .iter()
            .all(|cell| program.cells()[cell.index()].owner == body)
    );
    let mut compiler = compilation(100_000_000);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy(true);
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    compiler
        .enable_local_facts(
            CacheLimits {
                entries: 64,
                bytes: 4_000_000,
                result_bytes: 100_000,
            },
            WorkDomain::Optional,
        )
        .unwrap();
    let axes: Vec<_> = if oracle {
        (0..16)
            .map(|bits| Axes {
                caller_flat: bits & 1 != 0,
                callee_flat: bits & 2 != 0,
                fields: bits & 4 != 0,
                inline: bits & 8 != 0,
            })
            .collect()
    } else {
        let mut axes = vec![
            PACKED,
            Axes {
                fields: true,
                ..PACKED
            },
            Axes {
                caller_flat: true,
                callee_flat: true,
                ..PACKED
            },
            Axes {
                caller_flat: true,
                callee_flat: true,
                fields: true,
                ..PACKED
            },
        ];
        if inline_adversary {
            axes.push(Axes {
                caller_flat: true,
                callee_flat: true,
                fields: true,
                inline: true,
            });
        }
        axes
    };
    let mut rows = Vec::new();
    for axes in axes {
        let candidate = selected(
            &mut compiler,
            direct,
            axes,
            Some(state),
            &parameters,
            helper,
            body,
            &policy,
            false,
        );
        let reverse = if oracle {
            Some(selected(
                &mut compiler,
                direct,
                axes,
                Some(state),
                &parameters,
                helper,
                body,
                &policy,
                true,
            ))
        } else {
            None
        };
        for compact in [false, true] {
            if oracle && !compact {
                continue;
            }
            let rendering = policy_for_compaction(compact);
            let names: &[Style] = if oracle {
                &[Style::Global, Style::Scoped, Style::Source]
            } else {
                &[Style::Scoped]
            };
            for &style in names {
                let row = artifact(
                    &mut compiler,
                    candidate,
                    &rendering,
                    &case,
                    axes,
                    compact,
                    style,
                );
                if let Some(reverse) = reverse {
                    let reverse = artifact(
                        &mut compiler,
                        reverse,
                        &rendering,
                        &case,
                        axes,
                        compact,
                        style,
                    );
                    assert_eq!(
                        row["javascript"], reverse["javascript"],
                        "publication order changed physical recipes: {axes:?}"
                    );
                }
                eprintln!("product-call-artifact {row}");
                rows.push(row);
            }
        }
    }
    if oracle {
        assert_eq!(rows.len(), 48);
        let minima:Vec<_>=["raw","gzip9","brotli11"].into_iter().map(|codec|{
            let minimum=rows.iter().map(|row|row[codec].as_u64().unwrap()).min().unwrap();
            let winners:Vec<_>=rows.iter().filter(|row|row[codec].as_u64()==Some(minimum))
                .map(|row|json!({"axes":row["axes"],"style":row["style"],"javascript_sha256":row["javascript_sha256"]})).collect();
            json!({"codec":codec,"minimum":minimum,"winners":winners})
        }).collect();
        eprintln!(
            "product-call-oracle {}",
            json!({"case":case.name,"structures":16,"naming_plans":3,"artifacts":rows.len(),"minima":minima,
            "scope":"independent caller/callee storage, physical transport and eligible helper choices for the same by-value product; complete artifacts, no assumed winner"})
        );
        automatic_search(&case, &rows);
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
fn policy_for_compaction(compact: bool) -> ResolvedPolicy {
    policy(compact)
}
fn automatic_search(case: &Case, oracle: &[Json]) {
    // Automatic search owns the one-way baseline-first lifecycle. Check the
    // identical source in a fresh owner, independently of the manual oracle's
    // retained candidates, fixed ledger and warmed prerequisite cache.
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: 400_000_000,
            retained_bytes: 256_000_000,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 256 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let config:crate::config::ProjectConfig=toml::from_str("[javascript]\nstrip_console=false\ncandidate_proposal_limit=768\nterminal_codec_probe_limit=1536\ncandidate_beam_width=32\n[policy.tactics]\ncall-specialization='on'\nscalar-replacement='on'\ninlining='on'\ntarget-compaction='on'\nidentifier-mangling='on'\nnaming-search='on'\n").unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let request = SearchRequest {
        objectives: Objectives::All,
        scalar: request(),
        helper: helper_request(),
        string: StringRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts: LocalFactsRequest {
                work_quota: 100_000,
                result_bytes: 100_000,
            },
        },
        facts_cache: CacheLimits {
            entries: 32,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    };
    let mut observed = Vec::new();
    let search=compiler.search_javascript_observed(source,&policy,request,|entry| {
        let actual=execute(case,entry.javascript);
        let row=json!({"case":case.name,"style":format!("{:?}",entry.naming.style),"javascript":entry.javascript,"javascript_sha256":digest(entry.javascript),
            "raw":entry.sizes.raw,"gzip9":entry.sizes.gzip9,"brotli11":entry.sizes.brotli11,"baseline":entry.baseline,
            "recipe_descriptor":entry.recipe_descriptor.whole_words().expect("Whole cohort"),"recipe_fingerprint":entry.recipe_fingerprint,"observed":actual});
        eprintln!("product-call-search-artifact {row}");
        observed.push(row);
    }).unwrap();
    for entry in oracle {
        assert!(
            observed
                .iter()
                .any(|row| row["javascript"] == entry["javascript"]
                    && row["style"] == entry["style"]),
            "missing independent oracle artifact {}/{}; counters={:?}; stop={:?}",
            entry["axes"],
            entry["style"],
            search.counters(),
            search.stopped()
        );
        // A selected shared signature is inactive when this fixture's one
        // creator is fully inlined. Keep all sixteen explicitly published
        // maps in the oracle, but require search to visit the twelve active
        // combinations rather than render four redundant maps again.
        if entry["axes"]["inline"] == true && entry["axes"]["fields"] == true {
            let equivalent = oracle
                .iter()
                .find(|other| {
                    other["axes"]["inline"] == true
                        && other["axes"]["fields"] == false
                        && other["axes"]["caller_flat"] == entry["axes"]["caller_flat"]
                        && other["axes"]["callee_flat"] == entry["axes"]["callee_flat"]
                        && other["style"] == entry["style"]
                })
                .expect("independently published active counterpart");
            assert_ne!(entry["recipe_descriptor"], equivalent["recipe_descriptor"]);
            assert_eq!(entry["javascript"], equivalent["javascript"]);
            for key in ["raw", "gzip9", "brotli11"] {
                assert_eq!(entry[key], equivalent[key]);
            }
        } else {
            assert!(
                observed.iter().any(|row| {
                    row["recipe_descriptor"] == entry["recipe_descriptor"]
                        && row["style"] == entry["style"]
                        && row["javascript"] == entry["javascript"]
                }),
                "missing active oracle choice {}/{}",
                entry["axes"],
                entry["style"]
            );
        }
    }
    let mut winners = Vec::new();
    for (codec, key) in [
        (Objective::Raw, "raw"),
        (Objective::Gzip, "gzip9"),
        (Objective::Brotli, "brotli11"),
    ] {
        let winner=search.with_winner(codec,|view,plan| {
            execute(case,view.javascript);
            json!({"codec":key,"style":format!("{:?}",plan.style),"javascript_sha256":digest(view.javascript),"size":view.sizes.get(codec).unwrap()})
        }).expect("each requested codec retains its own eligible winner");
        let best = observed
            .iter()
            .map(|row| row[key].as_u64().unwrap())
            .min()
            .unwrap();
        assert_eq!(
            winner["size"].as_u64(),
            Some(best),
            "winner must minimize actual measured union for {key}"
        );
        assert!(
            best <= oracle
                .iter()
                .map(|row| row[key].as_u64().unwrap())
                .min()
                .unwrap()
        );
        winners.push(winner);
    }
    let counters = search.counters();
    assert!(counters.inactive_function_layouts > 0);
    eprintln!(
        "product-call-search-summary {}",
        json!({"case":case.name,"oracle_artifacts":oracle.len(),"observed_artifacts":observed.len(),"structures":counters.structures,
        "proof_queries":counters.proof_queries,"equivalent_product_hints":counters.equivalent_product_hints,"inactive_function_layouts":counters.inactive_function_layouts,"renders":counters.renders,"codec_probes":counters.codec_probes,
        "winners":winners,"stop":search.stopped().map(|stop|format!("{stop:?}"))})
    );
    drop(search);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
#[test]
fn private_product_oracle_covers_all_storage_transport_and_inline_combinations() {
    matrix(case!("arithmetic"), &["local"], true, true);
}
#[test]
fn product_arguments_freeze_before_later_argument_reentry_and_throw() {
    matrix(case!("argument-reentry"), &["local"], false, true);
    matrix(case!("mixed-slot-reentry"), &["local"], false, true);
    matrix(
        Case {
            name: "throwing-argument",
            setup: "let replace;globalThis.keep=value=>{replace=value;};globalThis.late=()=>{events.push('late');replace();throw 7;};",
            expected: "[\"late\",99,20,30]",
            ..case!("argument-reentry")
        },
        &["local"],
        false,
        true,
    );
}
#[test]
fn raw_product_transport_never_performs_the_callees_discarded_integer_coercion() {
    matrix(case!("opaque-field"), &["local"], false, true);
    matrix(
        Case {
            name: "throwing-field",
            setup: "let replace;globalThis.keep=value=>{replace=value;};globalThis.late=()=>{events.push('late');return 3;};globalThis.opaque=()=>({[Symbol.toPrimitive](hint){events.push('coerce:'+hint);replace();throw 7;}});",
            expected: "[\"late\",\"coerce:number\",99,9]",
            ..case!("opaque-field")
        },
        &["local"],
        false,
        true,
    );
    matrix(
        Case {
            name: "bigint-field",
            setup: "globalThis.keep=()=>{};globalThis.late=()=>{events.push('late');return 3;};globalThis.opaque=()=>1n;",
            expected: "[\"late\",99,1]",
            ..case!("opaque-field")
        },
        &["local"],
        false,
        true,
    );
    matrix(
        Case {
            name: "symbol-field",
            setup: "globalThis.keep=()=>{};globalThis.late=()=>{events.push('late');return 3;};globalThis.opaque=()=>Symbol('payload');",
            expected: "[\"late\",99,1]",
            ..case!("opaque-field")
        },
        &["local"],
        false,
        true,
    );
}
#[test]
fn two_identical_actuals_have_independent_nested_values_and_shared_reference_leaves() {
    matrix(
        case!("nested-two-arguments"),
        &["local", "other"],
        false,
        false,
    );
}
#[test]
fn scalar_local_returns_pack_the_original_immutable_snapshot() {
    matrix(case!("packed-return"), &["local"], false, true);
}
#[test]
fn expanded_products_and_primitive_references_share_source_argument_positions() {
    matrix(case!("mixed-reference"), &["local", "other"], false, false);
}
#[test]
fn shared_parameter_banks_belong_to_each_captured_and_recursive_activation() {
    matrix(case!("captured-activations"), &["local"], false, false);
    matrix(case!("recursive-forwarding"), &["local"], false, false);
}
#[test]
fn zero_width_transport_preserves_argument_prefix_and_its_abrupt_completion() {
    for case in [
        case!("empty-prefix"),
        Case {
            name: "empty-throwing-prefix",
            setup: "globalThis.mark=()=>{events.push(1);throw 7;};",
            expected: "[1,99]",
            ..case!("empty-prefix")
        },
    ] {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, case.source).unwrap();
        let checked = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &checked).unwrap();
        let helper = named_cell(&program, "step");
        let CellBinding::Function(body) = program.cells()[helper.index()].binding else {
            panic!()
        };
        let mut compiler = compilation(100_000_000);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true);
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let expanded = fields(&mut compiler, direct, body, &policy);
        for (candidate, axes) in [
            (direct, PACKED),
            (
                expanded,
                Axes {
                    fields: true,
                    ..PACKED
                },
            ),
        ] {
            for compact in [false, true] {
                let row = artifact(
                    &mut compiler,
                    candidate,
                    &policy_for_compaction(compact),
                    &case,
                    axes,
                    compact,
                    Style::Scoped,
                );
                eprintln!("product-call-artifact {row}");
            }
        }
        assert_eq!(compiler.finish().retained_bytes(), 0);
    }
}
#[test]
fn no_optional_work_preserves_the_complete_executable_packed_call_baseline() {
    let case = case!("arithmetic");
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let helper = named_cell(&program, "step");
    let CellBinding::Function(body) = program.cells()[helper.index()].binding else {
        panic!()
    };
    let mut compiler = compilation(0);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy(true);
    let direct = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let result =
        compiler.scalar_function_javascript(direct, body, request(), &policy, WorkDomain::Optional);
    assert!(!matches!(
        result,
        Ok(FunctionPublication {
            outcome: FunctionOutcome::Published(_),
            ..
        })
    ));
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    artifact(
        &mut compiler,
        direct,
        &policy,
        &case,
        PACKED,
        true,
        Style::Scoped,
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
#[test]
fn original_product_call_sources_execute_through_both_native_compilers() {
    for (case, expected) in [
        (case!("arithmetic"), "6\n7\n1\n"),
        (case!("packed-return"), "1\n9\n1\n2\n"),
        (case!("mixed-reference"), "3\n17\n1\n2\n"),
        (case!("recursive-forwarding"), "6\n1\n"),
    ] {
        super::native_struct_tests::qualify(
            &format!("product-call-{}", case.name),
            case.source,
            expected,
        );
    }
}

#[test]
fn immutable_auto_closure_products_use_the_common_callable_owner() {
    matrix(case!("auto-closure"), &["local"], false, true);
}
