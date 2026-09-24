//! Packed and flat candidates execute independently specified observations.
//! The helper oracle combines layouts with accesses to the very same product
//! cells; it is not a union of unrelated transformations.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Objectives, Plan, Style};
use serde_json::{json, Value as Json};
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
            source: include_str!(concat!("fixtures/product-storage/", $name, ".lil")),
            setup: include_str!(concat!("fixtures/product-storage/", $name, ".setup.js")),
            observations: include_str!(concat!(
                "fixtures/product-storage/",
                $name,
                ".observations.js"
            )),
            expected: include_str!(concat!(
                "fixtures/product-storage/",
                $name,
                ".expected.json"
            )),
        }
    };
}
fn policy(compact: bool) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\n[policy.tactics]\ntarget-compaction='{}'\n",
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
        CheckpointLimit { max_live: 64 },
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
    representation: &str,
    compact: bool,
    style: Style,
) -> Json {
    compiler
        .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
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
        Ok::<_,CandidateError>(json!({"case":case.name,"representation":representation,"compact":compact,"style":format!("{style:?}"),
            "source":case.source,"source_sha256":digest(case.source),"setup":case.setup,"observations":case.observations,
            "expected":serde_json::from_str::<Json>(case.expected).unwrap(),"observed":observed,
            "javascript":text,"javascript_sha256":digest(&text),"raw":raw,"gzip9":gzip9,"brotli11":brotli11}))
    }).unwrap().unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    row
}
fn matrix(case: Case, helper: Option<&str>, oracle: bool) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    program.verify().unwrap();
    let state = named_cell(&program, "state");
    let helper = helper.map(|name| named_cell(&program, name));
    let revisions: Vec<_> = program.units().iter().map(|unit| unit.revision()).collect();
    let mut compiler = compilation(100_000_000);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let selected = policy(true);
    let direct = compiler
        .direct_javascript(source, &selected, WorkDomain::Baseline)
        .unwrap();
    let scalar = flat(&mut compiler, direct, state, &selected);
    let mut candidates = vec![("packed/shared", direct), ("flat/shared", scalar)];
    let reverse = if let Some(helper) = helper {
        compiler
            .enable_local_facts(
                CacheLimits {
                    entries: 32,
                    bytes: 2_000_000,
                    result_bytes: 100_000,
                },
                WorkDomain::Optional,
            )
            .unwrap();
        let inlined = inline(&mut compiler, direct, helper, &selected);
        let flat_inline = inline(&mut compiler, scalar, helper, &selected);
        let inline_flat = flat(&mut compiler, inlined, state, &selected);
        candidates.extend([("packed/inline", inlined), ("flat/inline", flat_inline)]);
        Some(inline_flat)
    } else {
        None
    };
    let mut rows = Vec::new();
    for &(representation, candidate) in &candidates {
        let view = compiler.view(candidate.semantic_id()).unwrap();
        for (index, &revision) in revisions.iter().enumerate() {
            assert_eq!(
                view.unit_revision(UnitId::from_index(index).unwrap()),
                Some(revision)
            );
        }
        for compact in [false, true] {
            // The small oracle fixes compaction, leaving 4 structures ×3
            // naming plans. Other conformance cases exercise both compactions.
            if oracle && !compact {
                continue;
            }
            let policy = policy(compact);
            for style in [Style::Global, Style::Scoped, Style::Source] {
                let row = artifact(
                    &mut compiler,
                    candidate,
                    &policy,
                    &case,
                    representation,
                    compact,
                    style,
                );
                if representation == "flat/inline" {
                    let reverse = artifact(
                        &mut compiler,
                        reverse.unwrap(),
                        &policy,
                        &case,
                        "flat/inline/reverse",
                        compact,
                        style,
                    );
                    assert_eq!(
                        row["javascript"], reverse["javascript"],
                        "equivalent selected maps must form identical output irrespective of publication order"
                    );
                }
                eprintln!("product-storage-artifact {row}");
                rows.push(row);
            }
        }
    }
    if oracle {
        assert_eq!(rows.len(), 12);
        let minima:Vec<_>=["raw","gzip9","brotli11"].into_iter().map(|codec|{
            let minimum=rows.iter().map(|row|row[codec].as_u64().unwrap()).min().unwrap();
            let winners:Vec<_>=rows.iter().filter(|row|row[codec].as_u64()==Some(minimum)).map(|row|json!({"representation":row["representation"],"style":row["style"],"javascript_sha256":row["javascript_sha256"]})).collect();
            json!({"codec":codec,"minimum":minimum,"winners":winners})
        }).collect();
        eprintln!(
            "product-storage-oracle {}",
            json!({"case":case.name,"structures":4,"naming_plans":3,"artifacts":rows.len(),"minima":minima,"scope":"independently enumerated compatible shared/inline and packed/flat choices for the same captured component; complete artifact measurements, no assumed winner"})
        );
    }
    if oracle {
        if let Some(reverse) = reverse {
            compiler.discard(reverse.semantic_id()).unwrap();
        }
        for (_, candidate) in candidates {
            compiler.discard(candidate.semantic_id()).unwrap();
        }
        automatic_search(&case, &rows);
    }
    assert_eq!(compiler.finish().retained_bytes(), 0);
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
            logical_work: 200_000_000,
            retained_bytes: 256_000_000,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 64 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let config:crate::config::ProjectConfig=toml::from_str("[javascript]\nstrip_console=false\ncandidate_proposal_limit=192\nterminal_codec_probe_limit=384\ncandidate_beam_width=8\n[policy.tactics]\nscalar-replacement='on'\ninlining='on'\ntarget-compaction='on'\nidentifier-mangling='on'\nnaming-search='on'\n").unwrap();
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
        eprintln!("product-search-artifact {row}");
        observed.push(row);
    }).unwrap();
    for entry in oracle {
        assert!(
            observed
                .iter()
                .any(|row| row["javascript"] == entry["javascript"]
                    && row["style"] == entry["style"]),
            "missing independent oracle artifact {}/{}; counters={:?}; stop={:?}",
            entry["representation"],
            entry["style"],
            search.counters(),
            search.stopped()
        );
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
    assert!(
        counters.structures >= 4,
        "both interacting axes must be discovered"
    );
    assert!(
        counters.equivalent_product_hints >= 1,
        "the copied cell is the same complete family"
    );
    eprintln!(
        "product-search-summary {}",
        json!({"case":case.name,"oracle_artifacts":oracle.len(),"observed_artifacts":observed.len(),"structures":counters.structures,
        "proof_queries":counters.proof_queries,"equivalent_product_hints":counters.equivalent_product_hints,"renders":counters.renders,"codec_probes":counters.codec_probes,
        "winners":winners,"stop":search.stopped().map(|stop|format!("{stop:?}"))})
    );
    drop(search);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
#[test]
fn flat_products_preserve_nested_copies_shared_array_leaves_and_factory_activations() {
    matrix(case!("copies-and-activations"), None, false);
}
#[test]
fn flat_product_stores_reload_current_nested_parent_after_rhs_reentry() {
    matrix(case!("reentry"), None, false);
    let original = case!("reentry");
    matrix(
        Case {
            name: "throwing-reentry",
            setup: "let replace;globalThis.keep=value=>{replace=value;};globalThis.rhs=()=>{events.push('rhs');replace();throw 7;};",
            expected: "[\"rhs\",99,10,20,30,1,2,3]",
            ..original
        },
        None,
        false,
    );
}
#[test]
fn flat_product_initializers_keep_unused_effects_and_throw_order() {
    matrix(case!("initializer-effects"), None, false);
}
#[test]
fn shared_and_inline_product_helpers_preserve_discarded_coercion_and_raw_snapshots() {
    matrix(case!("coercing-helper"), Some("step"), false);
    let original = case!("coercing-helper");
    matrix(
        Case {
            name: "bigint-discarded-product-read",
            setup: "globalThis.keep=value=>{};globalThis.opaque=()=>1n;",
            expected: "[99,99]",
            ..original
        },
        Some("step"),
        false,
    );
}
#[test]
fn product_storage_and_captured_helper_oracle_measures_the_complete_compatible_space() {
    matrix(case!("helper-oracle"), Some("step"), true);
}
#[test]
fn zero_optional_work_keeps_the_executable_packed_baseline() {
    let case = case!("copies-and-activations");
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let state = named_cell(&program, "state");
    let mut compiler = compilation(0);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let policy = policy(true);
    let baseline = compiler
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let before = compiler.ledger().retained_bytes();
    let attempt = compiler.scalar_product_javascript(
        baseline,
        state,
        request(),
        &policy,
        WorkDomain::Optional,
    );
    assert!(
        !matches!(
            attempt,
            Ok(ProductPublication {
                outcome: ProductOutcome::Published(_),
                ..
            })
        ),
        "zero optional work cannot publish a new physical choice"
    );
    assert_eq!(compiler.ledger().retained_bytes(), before);
    assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), 0);
    assert_eq!(compiler.checkpoint_count(), 2);
    let row = artifact(
        &mut compiler,
        baseline,
        &policy,
        &case,
        "zero-optional-packed",
        true,
        Style::Global,
    );
    assert_eq!(
        row["observed"],
        serde_json::from_str::<Json>(case.expected).unwrap()
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn direct_literal_projection_is_a_load_rule_and_keeps_assignment_evaluation() {
    let case = Case {
        name: "direct-literal-load-only",
        source: "extern int effect();print([3,4][1]);print(([1,2][0]=effect()));",
        setup: "globalThis.effect=()=>{events.push('effect');return 9;};",
        observations: "",
        expected: "[4,\"effect\",9]",
    };
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, case.source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &checked).unwrap();
    let mut compiler = compilation(0);
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let selected = policy(true);
    let direct = compiler
        .direct_javascript(source, &selected, WorkDomain::Baseline)
        .unwrap();
    for compact in [false, true] {
        for style in [Style::Global, Style::Scoped, Style::Source] {
            artifact(
                &mut compiler,
                direct,
                &policy(compact),
                &case,
                "direct-load-rule",
                compact,
                style,
            );
        }
    }
    assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), 0);
    assert_eq!(
        compiler.checkpoint_count(),
        2,
        "cheap load rule does not publish a representation candidate"
    );
    assert_eq!(compiler.finish().retained_bytes(), 0);
}

#[test]
fn frozen_product_copies_keep_each_loop_and_branch_activation_separate() {
    matrix(case!("loop-branches"), None, false);
}
