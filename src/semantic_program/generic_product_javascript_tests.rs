//! One original imported parametric body, concrete downstream storage/helper
//! choices, and raw snapshot/coercion observations. No source reconstruction.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Objective, Plan, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::process::Command;

const ENTRY: &str = include_str!("fixtures/generic-provenance/entry.lil");
const SNAPSHOT: &str = include_str!("fixtures/generic-provenance/snapshot.lil");
const POINT: &str = include_str!("fixtures/generic-provenance/point.lil");
const OBSERVATIONS: &str = include_str!("fixtures/generic-provenance/observations.js");
const EXPECTED: &str = include_str!("fixtures/generic-provenance/expected.json");
fn policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str("[javascript]\nstrip_console=false\n[policy.tactics]\nscalar-replacement='on'\ninlining='on'\ntarget-compaction='on'\n").unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn owner<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000_000,
                optional_work: 100_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 256_000_000,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 32 },
    )
    .unwrap()
}
fn product_request() -> ProductRequest {
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
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn execute(javascript: &str) -> Json {
    let script = format!("const events=[];const library=await import('data:text/javascript,'+encodeURIComponent({}));\n{}\nprocess.stdout.write(JSON.stringify(events));", serde_json::to_string(javascript).unwrap(), OBSERVATIONS);
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node required for original generic product observations");
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let actual: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        actual,
        serde_json::from_str::<Json>(EXPECTED).unwrap(),
        "{script}"
    );
    actual
}
fn sources(inspect: impl FnOnce(Program<'_>)) {
    let sources = [ENTRY, SNAPSHOT, POINT];
    let graph = crate::module::ModuleSet {
        modules: sources
            .iter()
            .enumerate()
            .map(|(id, source)| crate::module::ModuleSource {
                path: format!("/generic-runtime-{id}.lil").into(),
                source: (*source).into(),
                dependencies: if id == 0 { vec![2, 1] } else { vec![] },
                foreign_dependencies: vec![],
                dynamic_dependencies: vec![],
                offset: 0,
            })
            .collect(),
        dependency_order: vec![2, 1, 0],
        root: 0,
        eager: vec![true; 3],
    };
    let arena = bumpalo::Bump::new();
    let syntax: Vec<_> = sources
        .iter()
        .map(|source| crate::parse_source(&arena, source).unwrap())
        .collect();
    let semantics = crate::semantic::analyze_modules(&syntax, &graph).unwrap();
    let program = from_checked_modules(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program);
}

#[test]
fn generic_snapshots_feed_selected_product_and_inline_clients_with_one_raw_body() {
    sources(|program| {
        let (helper, body) = program
            .cells
            .iter()
            .enumerate()
            .find_map(|(index, cell)| {
                if cell.name == "score" {
                    if let CellBinding::Function(body) = cell.binding {
                        Some((CellId::from_index(index).unwrap(), body))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let parameter = program.unit(body).unwrap().parameters[0];
        let snapshot = program
            .cells
            .iter()
            .find_map(|cell| {
                if cell.name == "snapshot" {
                    if let CellBinding::Function(body) = cell.binding {
                        Some(body)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .unwrap();
        let declared = program.unit(snapshot).unwrap().callable_type.unwrap();
        assert!(matches!(
            program.types[declared.index()],
            Type::GenericFunction(_)
        ));
        let mut compiler = owner();
        let policy = policy();
        compiler
            .enable_local_facts(
                CacheLimits {
                    entries: 16,
                    bytes: 1_000_000,
                    result_bytes: 100_000,
                },
                WorkDomain::Baseline,
            )
            .unwrap();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let mut measured = Vec::new();
        for fields in [false, true] {
            for inline in [false, true] {
                let mut candidate = direct;
                if fields {
                    candidate = match compiler
                        .scalar_product_javascript(
                            candidate,
                            parameter,
                            product_request(),
                            &policy,
                            WorkDomain::Optional,
                        )
                        .unwrap()
                        .outcome
                    {
                        ProductOutcome::Published(candidate) => candidate,
                        other => panic!("product client: {other:?}"),
                    };
                }
                if inline {
                    candidate = match compiler
                        .inline_helper_javascript(
                            candidate,
                            helper,
                            helper_request(),
                            &policy,
                            WorkDomain::Optional,
                        )
                        .unwrap()
                        .outcome
                    {
                        HelperOutcome::Published(candidate) => candidate,
                        other => panic!("helper client: {other:?}"),
                    };
                }
                let descriptor = compiler
                    .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
                    .unwrap();
                compiler
                    .with_semantic(source, |program, uses, _| {
                        assert!(uses.valid_for(program));
                        assert_eq!(
                            program.unit(snapshot).unwrap().callable_type,
                            Some(declared)
                        );
                        assert_eq!(
                            program
                                .units
                                .iter()
                                .filter(|unit| unit.data().callable_type == Some(declared))
                                .count(),
                            1
                        );
                    })
                    .unwrap();
                let retained = compiler.ledger().retained_bytes();
                for style in [Style::Global, Style::Scoped, Style::Source] {
                    let row=compiler.with_javascript_output(candidate,&policy,|output| {
                    let artifact=output.render(&Plan::new(style))?;
                    let (observed, actual_output)=output.with_artifact(artifact,|view|(execute(view.javascript),view.output))?;
                    let sizes=[output.measure(artifact,Objective::Raw)?,output.measure(artifact,Objective::Gzip)?,output.measure(artifact,Objective::Brotli)?];
                    let javascript=output.take_artifact(artifact)?;
                    Ok::<_,CandidateError>(json!({"kind":"generic-product-artifact","fields":fields,"inline":inline,"style":format!("{style:?}"),"recipe_descriptor":descriptor,"output":{"dead_code_elimination":actual_output.dead_code_elimination,"target_compaction":actual_output.target_compaction,"literals":format!("{:?}",actual_output.literals)},"sizes":sizes,"javascript_sha256":hash(&javascript),"javascript":javascript,"observed":observed,"expected":serde_json::from_str::<Json>(EXPECTED).unwrap(),"sources":[{"file":"entry.lil","source":ENTRY,"sha256":hash(ENTRY)},{"file":"snapshot.lil","source":SNAPSHOT,"sha256":hash(SNAPSHOT)},{"file":"point.lil","source":POINT,"sha256":hash(POINT)}],"observations":OBSERVATIONS}))
                }).unwrap().unwrap();
                    assert_eq!(compiler.ledger().retained_bytes(), retained);
                    println!("{}", row);
                    measured.push(row);
                }
            }
        }
        assert_eq!(measured.len(), 12);
        let winners: Vec<_> = (0..3)
            .map(|objective| {
                measured
                    .iter()
                    .min_by_key(|row| row["sizes"][objective].as_u64().unwrap())
                    .unwrap()
            })
            .collect();
        // These are finite measured-union winners, not a claim that this
        // manual portfolio exhausts the compiler's search or improves bytes.
        println!(
            "{}",
            json!({"kind":"generic-product-summary","artifacts":measured.len(),"winners":winners.iter().enumerate().map(|(objective,row)|json!({"objective":objective,"sizes":row["sizes"],"fields":row["fields"],"inline":row["inline"],"style":row["style"],"output":row["output"],"javascript_sha256":row["javascript_sha256"]})).collect::<Vec<_>>()})
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
