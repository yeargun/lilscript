//! Execute the complete two-file artifact and check its independently encoded
//! per-file scores. The same source also executes through the joint route.
use super::*;
use crate::semantic_program::demand::{DemandMode, DemandPlan};
use crate::semantic_program::fixed_resource::ResourceChoice;
use crate::semantic_program::javascript_resource::ResourceView;
use crate::structured_js::selection::Objective;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::process::Command;

const HOST: &str = include_str!("fixtures/fixed-javascript-resources/host.js");
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];

fn digest(text: &str) -> String {
    format!("{:x}", Sha256::digest(text.as_bytes()))
}

fn output_metadata(output: OutputTactics) -> serde_json::Value {
    json!({"dead_code_elimination":output.dead_code_elimination,
        "target_compaction":output.target_compaction,"literals":format!("{:?}",output.literals)})
}

fn choices(compact: bool) -> OutputTactics {
    OutputTactics {
        dead_code_elimination: true,
        target_compaction: compact,
        literals: if compact {
            LiteralOutput::Observed
        } else {
            LiteralOutput::Original
        },
    }
}

fn render_choices(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
    compact: bool,
) -> ArtifactId {
    compiler
        .with_javascript_output_choices_in(
            candidate,
            policy,
            choices(compact),
            WorkDomain::Baseline,
            |output| {
                let id = output.render(&Plan::new(style))?;
                output.retain_artifact(id)
            },
        )
        .unwrap()
        .unwrap()
}

fn assert_physical_cut(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    body: UnitId,
    policy: &ResolvedPolicy,
) {
    let index = compiler.candidate_slot(candidate).unwrap();
    let checkpoint = compiler.slots[index].checkpoint.as_ref().unwrap();
    let map = checkpoint.implementations.as_ref().unwrap();
    let Some(ResourceChoice::Consumer { consumed, .. }) = map.resource() else {
        panic!("consumer resource")
    };
    let initializer = consumed.borrow().initializer();
    let demand = DemandPlan::build_resource(
        &checkpoint.semantic.program,
        Some(&checkpoint.semantic.uses),
        Some(map),
        policy.javascript_contract().unwrap(),
        DemandMode::Prune,
        ResourceView::Consumer(consumed.borrow()),
        Some((&mut compiler.ledger, WorkDomain::Baseline)),
    )
    .unwrap();
    assert!(
        demand
            .contexts()
            .iter()
            .all(|context| context.unit != body && context.unit != initializer),
        "the consumer must not analyze/materialize producer body or initializer contexts"
    );
    demand.discard(Some(&mut compiler.ledger)).unwrap();
}

fn qualify(
    compiler: &mut Compilation<'_>,
    artifact: ArtifactId,
    directory: &Path,
    label: &str,
    package: bool,
) -> usize {
    let mut measured = [0; 3];
    for (i, codec) in CODECS.into_iter().enumerate() {
        measured[i] = compiler
            .measure_artifact(artifact, codec, WorkDomain::Baseline)
            .unwrap();
        let before = compiler.ledger.work_used(WorkDomain::Baseline);
        assert_eq!(
            compiler
                .measure_artifact(artifact, codec, WorkDomain::Baseline)
                .unwrap(),
            measured[i]
        );
        assert_eq!(
            compiler.ledger.work_used(WorkDomain::Baseline) - before,
            1,
            "completed artifact score cache"
        );
    }
    let (entry, dependency, actual_output) = compiler
        .with_artifact(artifact, |view| {
            assert_eq!(view.sizes.raw, measured[0]);
            assert_eq!(view.sizes.gzip9, Some(measured[1]));
            assert_eq!(view.sizes.brotli11, Some(measured[2]));
            (
                view.javascript.to_owned(),
                view.dependency.map(|p| {
                    (
                        p.specifier.to_owned(),
                        p.javascript.to_owned(),
                        p.sizes,
                        p.output,
                    )
                }),
                view.output,
            )
        })
        .unwrap();
    assert_eq!(dependency.is_some(), package);
    for (i, codec) in CODECS.into_iter().enumerate() {
        let primary = crate::compression::measure(entry.as_bytes(), codec).unwrap();
        let fixed = dependency.as_ref().map_or(0, |(_, text, sizes, _)| {
            let size = crate::compression::measure(text.as_bytes(), codec).unwrap();
            assert_eq!(sizes.get(codec), Some(size));
            size
        });
        assert_eq!(
            measured[i],
            primary + fixed,
            "actual per-file {codec:?}, {label}"
        );
    }
    let path = directory.join(label);
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("entry.mjs"), &entry).unwrap();
    std::fs::write(path.join("host.mjs"), HOST).unwrap();
    let producer = dependency.as_ref().map(|(specifier, text, _, output)| {
        assert_eq!(specifier, "./producer.mjs");
        std::fs::write(path.join("producer.mjs"), text).unwrap();
        json!({"file":path.join("producer.mjs"),"sha256":digest(text),"bytes":text.len(),"output":output_metadata(*output)})
    });
    let run = Command::new("timeout")
        .args(["15s", "node"])
        .arg(path.join("host.mjs"))
        .arg(path.join("entry.mjs"))
        .output()
        .unwrap();
    assert!(
        run.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&run.stderr)
    );
    assert_eq!(run.stdout, b"fixed-resource-value-order-ok\n", "{label}");
    println!(
        "fixed-resource-artifact {}",
        json!({"label":label,"entry":path.join("entry.mjs"),"entry_sha256":digest(&entry),"output":output_metadata(actual_output),"producer":producer,"sizes":{"raw":measured[0],"gzip9":measured[1],"brotli11":measured[2]},"stdout":"fixed-resource-value-order-ok\n"})
    );
    measured[0]
}

#[test]
fn actual_resources_preserve_raw_snapshots_reentry_and_errors_under_both_transports() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!(
            "fixed-resource-runtime-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir(&root).unwrap();
    for entry_name in ["entry.lil", "entry-captured.lil"] {
        with_entry(entry_name, |program, cell, body| {
            assert_eq!(program.modules.len(), 2);
            let mut compiler = compiler(24, 30_000_000);
            let policy = policy();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let FunctionOutcome::Published(fields) = compiler
                .scalar_function_javascript(direct, body, request(), &policy, WorkDomain::Baseline)
                .unwrap()
                .outcome
            else {
                panic!("complete internal product call proof")
            };
            for (layout, base) in [("packed", direct), ("fields", fields)] {
                for style in STYLES {
                    let artifact = render_choices(&mut compiler, base, &policy, style, true);
                    qualify(
                        &mut compiler,
                        artifact,
                        &root,
                        &format!("{entry_name}-joint-{layout}-{style:?}"),
                        false,
                    );
                    compiler.discard_artifact(artifact).unwrap();
                }
            }
            for (producer_layout, producer_base) in [("packed", direct), ("fields", fields)] {
                let selected = producer(&mut compiler, producer_base, cell, &policy);
                for (consumer_layout, consumer_base) in [("packed", direct), ("fields", fields)] {
                    let artifact = render(&mut compiler, selected, &policy, Style::Global);
                    let package = compiler
                        .freeze_producer_javascript(
                            consumer_base,
                            artifact,
                            &policy,
                            WorkDomain::Baseline,
                        )
                        .unwrap();
                    assert_physical_cut(&mut compiler, package, body, &policy);
                    for compact in [false, true] {
                        for style in STYLES {
                            let artifact =
                                render_choices(&mut compiler, package, &policy, style, compact);
                            let length=qualify(&mut compiler,artifact,&root,&format!("{entry_name}-{producer_layout}-{consumer_layout}-{compact}-{style:?}"),true);
                            let retained = compiler.ledger.retained_bytes();
                            compiler
                                .with_javascript_output_choices_in(
                                    package,
                                    &policy,
                                    choices(compact),
                                    WorkDomain::Baseline,
                                    |output| {
                                        assert!(
                                            output
                                                .render_bounded(&Plan::new(style), length - 1)
                                                .is_err(),
                                            "package byte limit includes both files"
                                        );
                                    },
                                )
                                .unwrap();
                            assert_eq!(compiler.ledger.retained_bytes(), retained);
                            compiler.discard_artifact(artifact).unwrap();
                        }
                    }
                    compiler.discard(package.semantic_id()).unwrap();
                }
                compiler.discard(selected.semantic_id()).unwrap();
            }
            for candidate in [fields, direct] {
                compiler.discard(candidate.semantic_id()).unwrap();
            }
            compiler.discard(source).unwrap();
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}
