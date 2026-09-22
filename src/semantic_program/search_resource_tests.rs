//! Complete fixed inputs join the same admitted search and artifact owners.
//! Setup is explicit pre-existing-input work, never a cold Whole baseline claim.
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, CompilationRequest, ResourceLimits, RuntimeRisk, TacticUse,
};
use crate::semantic_program::implementation_identity::{ProductTransport, ResourceDescription};
use std::path::{Path, PathBuf};
use std::process::Command;

const WORK: u64 = 200_000_000;
const MEMORY: u64 = 128_000_000;
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const HOST: &str = include_str!("fixtures/fixed-javascript-resources/host.js");

fn policy(extra: &str) -> ResolvedPolicy {
    policy_with(48, extra)
}
fn policy_with(proposals: usize, extra: &str) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit=96\ncandidate_limit=12\ncandidate_beam_width=8\n[policy.tactics]\nidentifier-mangling='on'\nnaming-search='on'\ntarget-compaction='on'\nscalar-replacement='on'\ncall-specialization='on'\ninlining='on'\nconstant-folding='on'\nstring-pooling='on'\nstartup-reconstruction='on'\n{extra}\n"
    )).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn request() -> SearchRequest {
    let local_facts = LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    };
    SearchRequest {
        objectives: Objectives::All,
        scalar: ScalarRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
        },
        helper: HelperRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        string: StringRequest {
            max_work: 1_000_000,
            scratch_bytes: 1_000_000,
            output_bytes: 1_000_000,
            local_facts,
        },
        facts_cache: CacheLimits {
            entries: 32,
            bytes: 2_000_000,
            result_bytes: 100_000,
        },
    }
}
fn with_source(inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, CellId, UnitId)) {
    with_source_at("fixed-javascript-resources/entry.lil", inspect);
}
fn with_source_at(
    entry: &str,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, CellId, UnitId),
) {
    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures")
        .join(entry);
    let modules = crate::module::discover_modules(&entry).unwrap();
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::semantic::analyze_modules(&syntax, &modules).unwrap();
    let program = crate::semantic_program::from_checked_modules(&syntax, &checked).unwrap();
    let (cell, body) = program
        .cells()
        .iter()
        .enumerate()
        .find_map(|(index, cell)| {
            if cell.name == "score" {
                if let CellBinding::Function(body) = cell.binding {
                    return Some((CellId::from_index(index).unwrap(), body));
                }
            }
            None
        })
        .unwrap();
    let ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: WORK,
            retained_bytes: MEMORY,
            terminal_work: 0,
        },
    )
    .unwrap();
    let mut compiler = Compilation::new(ledger, CheckpointLimit { max_live: 128 }).unwrap();
    let source = compiler
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    inspect(&mut compiler, source, cell, body);
    assert_eq!(compiler.finish().retained_bytes(), 0);
}
fn render(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> ArtifactId {
    compiler
        .with_javascript_output_in(candidate, policy, WorkDomain::Baseline, |output| {
            let artifact = output.render(&Plan::new(style))?;
            output.retain_artifact(artifact)
        })
        .unwrap()
        .unwrap()
}
struct Inputs {
    direct: CandidateId,
    fields: CandidateId,
    producers: [CandidateId; 2],
    packages: [CandidateId; 2],
}
fn inputs(
    compiler: &mut Compilation<'_>,
    source: SemanticId,
    cell: CellId,
    body: UnitId,
    policy: &ResolvedPolicy,
) -> Inputs {
    let direct = compiler
        .direct_javascript(source, policy, WorkDomain::Baseline)
        .unwrap();
    let FunctionOutcome::Published(fields) = compiler
        .scalar_function_javascript(direct, body, request().scalar, policy, WorkDomain::Baseline)
        .unwrap()
        .outcome
    else {
        panic!("field transport")
    };
    let mut producers = Vec::new();
    let mut packages = Vec::new();
    for base in [direct, fields] {
        let ProducerOutcome::Published(producer) = compiler
            .producer_javascript(
                base,
                cell,
                "./producer.mjs",
                "scoreABI",
                request().scalar,
                policy,
                WorkDomain::Baseline,
            )
            .unwrap()
            .outcome
        else {
            panic!("physical export")
        };
        let artifact = render(compiler, producer, policy, Style::Global);
        packages.push(
            compiler
                .freeze_producer_javascript(direct, artifact, policy, WorkDomain::Baseline)
                .unwrap(),
        );
        producers.push(producer);
    }
    Inputs {
        direct,
        fields,
        producers: producers.try_into().unwrap(),
        packages: packages.try_into().unwrap(),
    }
}
#[derive(Clone)]
struct Row {
    candidate: CandidateId,
    kind: u8,
    style: Style,
    output: OutputTactics,
    text: String,
    producer: Option<String>,
    sizes: [usize; 3],
}
fn independent_sizes(text: &str, producer: Option<&str>) -> [usize; 3] {
    CODECS.map(|codec| {
        crate::compression::measure(text.as_bytes(), codec).unwrap()
            + producer.map_or(0, |text| {
                crate::compression::measure(text.as_bytes(), codec).unwrap()
            })
    })
}
fn directory() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("target")
        .join(format!(
            "fixed-resource-search-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
    std::fs::create_dir_all(&path).unwrap();
    path
}
fn execute(root: &Path, label: &str, row: &Row) {
    execute_with_host(
        root,
        label,
        row,
        HOST,
        "fixed-resource-value-order-ok\n",
        "fixed-resource-search-artifact",
    );
}
fn execute_with_host(
    root: &Path,
    label: &str,
    row: &Row,
    host: &str,
    expected: &str,
    marker: &str,
) {
    let path = root.join(label);
    std::fs::create_dir(&path).unwrap();
    std::fs::write(path.join("entry.mjs"), &row.text).unwrap();
    std::fs::write(path.join("host.mjs"), host).unwrap();
    if let Some(text) = &row.producer {
        std::fs::write(path.join("producer.mjs"), text).unwrap();
    }
    let output = Command::new("timeout")
        .args(["15s", "node"])
        .arg(path.join("host.mjs"))
        .arg(path.join("entry.mjs"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{label}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, expected.as_bytes());
    println!(
        "{marker} {}",
        serde_json::json!({
            "label":label, "kind":row.kind, "naming":format!("{:?}",row.style),
            "output":{"dead_code_elimination":row.output.dead_code_elimination,"target_compaction":row.output.target_compaction,"literals":format!("{:?}",row.output.literals)},
            "entry":path.join("entry.mjs"),"producer":row.producer.as_ref().map(|_|path.join("producer.mjs")),
            "sizes":{"raw":row.sizes[0],"gzip9":row.sizes[1],"brotli11":row.sizes[2]},
            "stdout":expected
        })
    );
}

#[test]
fn preexisting_consumers_share_complete_codec_selection_and_execute_every_observation() {
    with_source(|compiler, source, cell, body| {
        let policy = policy("");
        let input = inputs(compiler, source, cell, body, &policy);
        let root = directory();
        let mut manual = Vec::new();
        for (kind, candidate) in [
            (0, input.direct),
            (1, input.packages[0]),
            (2, input.packages[1]),
        ] {
            for style in STYLES {
                let artifact = render(compiler, candidate, &policy, style);
                let row = compiler
                    .with_artifact(artifact, |view| Row {
                        candidate,
                        kind,
                        style,
                        output: view.output,
                        text: view.javascript.to_owned(),
                        producer: view.dependency.map(|d| d.javascript.to_owned()),
                        sizes: independent_sizes(
                            view.javascript,
                            view.dependency.map(|d| d.javascript),
                        ),
                    })
                    .unwrap();
                execute(&root, &format!("manual-{}", manual.len()), &row);
                manual.push(row);
                compiler.discard_artifact(artifact).unwrap();
            }
        }
        let mut observed = Vec::new();
        let mut search = compiler
            .search_javascript_with_candidates_observed(
                source,
                &input.packages,
                &policy,
                request(),
                |view| {
                    let kind = match view.recipe_descriptor.resource() {
                        ResourceDescription::Whole => {
                            assert!(view.recipe_descriptor.whole_words().is_some());
                            assert!(view.dependency.is_none());
                            0
                        }
                        ResourceDescription::Consumer { consumed, producer } => {
                            assert!(view.recipe_descriptor.whole_words().is_none());
                            let dependency = view.dependency.unwrap();
                            assert_eq!(dependency.javascript, producer.javascript());
                            assert_eq!(dependency.output, producer.provenance().output());
                            match consumed.parameters()[0].transport() {
                                ProductTransport::Packed => 1,
                                ProductTransport::Fields => 2,
                            }
                        }
                        ResourceDescription::Producer { .. } => {
                            panic!("fragment entered complete search")
                        }
                    };
                    let sizes =
                        independent_sizes(view.javascript, view.dependency.map(|d| d.javascript));
                    for (i, codec) in CODECS.into_iter().enumerate() {
                        assert_eq!(view.sizes.get(codec), Some(sizes[i]));
                    }
                    observed.push(Row {
                        candidate: view.candidate,
                        kind,
                        style: view.naming.style,
                        output: view.output,
                        text: view.javascript.to_owned(),
                        producer: view.dependency.map(|d| d.javascript.to_owned()),
                        sizes,
                    });
                },
            )
            .unwrap();
        for expected in &manual {
            assert!(
                observed.iter().any(|row| row.kind == expected.kind
                    && row.style == expected.style
                    && row.output == expected.output
                    && row.text == expected.text
                    && row.producer == expected.producer),
                "missing provided complete candidate kind {} {:?}",
                expected.kind,
                expected.style
            );
        }
        for (index, row) in observed.iter().enumerate() {
            execute(&root, &format!("search-{index}"), row);
        }
        for (i, codec) in CODECS.into_iter().enumerate() {
            search
                .with_winner(codec, |view, plan| {
                    assert_eq!(
                        view.sizes.get(codec),
                        observed.iter().map(|row| row.sizes[i]).min()
                    );
                    assert!(observed.iter().any(|row| row.candidate == view.candidate
                        && row.style == plan.style
                        && row.output == view.output
                        && row.text == view.javascript
                        && row.producer.as_deref() == view.dependency.map(|d| d.javascript)));
                })
                .unwrap();
        }
        // The complete handle remains in Compilation after Search releases its
        // frontier; neither source candidate nor fixed input is consumed.
        let winner = search.take_winner_artifact(Objective::Brotli).unwrap();
        drop(search);
        compiler
            .with_artifact(winner, |view| {
                assert_eq!(
                    view.sizes.brotli11,
                    observed.iter().map(|row| row.sizes[2]).min()
                )
            })
            .unwrap();
        compiler.discard_artifact(winner).unwrap();
        for candidate in input.packages.into_iter().chain(input.producers) {
            compiler.with_implementations(candidate, |_| ()).unwrap();
        }
    });
}

fn stage(
    compiler: &mut Compilation<'_>,
    portfolio: &mut Portfolio,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> usize {
    compiler
        .with_javascript_output_in(candidate, policy, WorkDomain::Baseline, |output| {
            let plan = Plan::new(style);
            let artifact = output.render(&plan)?;
            portfolio.stage(output, 0, artifact, 0)
        })
        .unwrap()
        .unwrap()
}
#[test]
fn shared_producer_text_counts_once_and_package_handoff_is_atomic() {
    with_source(|compiler, source, cell, body| {
        let policy = policy("");
        let input = inputs(compiler, source, cell, body, &policy);
        let again = render(compiler, input.producers[0], &policy, Style::Global);
        let distinct = compiler
            .freeze_producer_javascript(input.direct, again, &policy, WorkDomain::Baseline)
            .unwrap();
        let mut portfolio = Portfolio::new(RevisionId::fresh());
        let a = stage(
            compiler,
            &mut portfolio,
            input.packages[0],
            &policy,
            Style::Global,
        );
        let b = stage(
            compiler,
            &mut portfolio,
            input.packages[0],
            &policy,
            Style::Scoped,
        );
        let c = stage(compiler, &mut portfolio, distinct, &policy, Style::Global);
        let primary = portfolio
            .entries
            .iter()
            .map(|(_, entry)| entry.capacity)
            .sum::<usize>();
        let artifact_a = portfolio.entries.get(a).unwrap().artifact;
        let artifact_b = portfolio.entries.get(b).unwrap().artifact;
        let artifact_c = portfolio.entries.get(c).unwrap().artifact;
        let dep_a = compiler.artifacts.dependency(artifact_a).unwrap().unwrap();
        let dep_b = compiler.artifacts.dependency(artifact_b).unwrap().unwrap();
        let dep_c = compiler.artifacts.dependency(artifact_c).unwrap().unwrap();
        assert!(dep_a.same_owner(dep_b));
        assert!(!dep_a.same_owner(dep_c));
        assert_eq!(dep_a.view().javascript, dep_c.view().javascript);
        let unique = dep_a.view().retained_capacity + dep_c.view().retained_capacity;
        let retained = compiler.ledger.retained_bytes();
        {
            let mut budget =
                AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
            assert_eq!(
                portfolio
                    .retained_package_bytes(&compiler.artifacts, &mut budget, primary, |_| true)
                    .unwrap(),
                primary + unique
            );
            portfolio.selected = [Some(a); 3];
            assert!(portfolio
                .take_winner(&mut compiler.artifacts, &mut budget, Objective::Raw)
                .is_none());
            assert_eq!(portfolio.selected, [Some(a); 3]);
            let taken = portfolio
                .take_winner_artifact(&compiler.artifacts, &mut budget, Objective::Raw)
                .unwrap();
            assert_eq!(taken, artifact_a);
            assert_eq!(portfolio.selected, [None; 3]);
            compiler
                .artifacts
                .with_artifact(taken, |view| assert!(view.dependency.is_some()))
                .unwrap();
            compiler.artifacts.discard(taken, &mut budget).unwrap();
            portfolio.discard_all(&mut compiler.artifacts, &mut budget);
        }
        assert!(compiler.ledger.retained_bytes() < retained);
        compiler
            .with_implementations(input.packages[0], |_| ())
            .unwrap();
    });
}

#[test]
fn invalid_producer_seed_preserves_baseline_and_zero_optional_does_not_consume_inputs() {
    for zero in [false, true] {
        with_source(|compiler, source, cell, body| {
            let policy = policy_with(if zero { 0 } else { 48 }, "");
            let input = inputs(compiler, source, cell, body, &policy);
            let mut observed = 0;
            let search = compiler
                .search_javascript_with_candidates_observed(
                    source,
                    &[input.producers[0]],
                    &policy,
                    request(),
                    |_| observed += 1,
                )
                .unwrap();
            if zero {
                assert!(search.stopped().is_none());
                assert_eq!(observed, 1);
            } else {
                assert!(matches!(
                    search.stopped(),
                    Some(SearchError::Candidate(CandidateError::Artifact(
                        "supplied search candidate must be a complete Consumer package"
                    )))
                ));
            }
            for codec in CODECS {
                assert!(search
                    .with_winner(codec, |view, _| assert!(view.dependency.is_none()))
                    .is_some());
            }
            drop(search);
            compiler
                .with_implementations(input.producers[0], |_| ())
                .unwrap();
            compiler
                .with_implementations(input.packages[0], |_| ())
                .unwrap();
        });
    }
}

#[test]
fn final_package_cost_uses_merged_risk_and_never_fabricates_unknown_runtime_evidence() {
    let policy = policy("");
    let mut ledger = BudgetLedger::new_baseline_first(
        ResourceLimits::default(),
        BaselineFirstPlan {
            logical_work: WORK,
            retained_bytes: MEMORY,
            terminal_work: 0,
        },
    )
    .unwrap();
    let owner = RevisionId::fresh();
    let output = OutputTactics {
        dead_code_elimination: false,
        target_compaction: false,
        literals: LiteralOutput::Original,
        raw_structure: false,
    };
    let (consumer, producer) = {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        (
            ArtifactProvenance::build(
                &[],
                output,
                &Plan::new(Style::Source),
                &policy,
                owner,
                &mut budget,
            )
            .unwrap(),
            ArtifactProvenance::build(
                &[TacticUse {
                    tactic: TacticId::StartupReconstruction,
                    risk: RuntimeRisk::Startup,
                }],
                output,
                &Plan::new(Style::Global),
                &policy,
                owner,
                &mut budget,
            )
            .unwrap(),
        )
    };
    let baseline = CandidateCostEvidence {
        startup_work: Some(0),
        ..CandidateCostEvidence::size_only(100)
    };
    let cost = CandidateCostEvidence {
        startup_work: Some(2),
        ..CandidateCostEvidence::size_only(80)
    };
    {
        let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
        assert!(matches!(
            consumer.admit(&policy, cost, baseline, &mut budget),
            Err(ProvenanceError::Admission(
                AdmissionError::UndeclaredRuntimeRisk(RuntimeRisk::Startup)
            ))
        ));
        consumer
            .admit_with_dependency(&producer, &policy, cost, baseline, &mut budget)
            .unwrap();
        let bounded = policy_with(48, "[policy.constraints]\nmax_startup_work=0\n");
        assert!(matches!(
            consumer.admit_with_dependency(
                &producer,
                &bounded,
                CandidateCostEvidence::size_only(80),
                CandidateCostEvidence::size_only(100),
                &mut budget
            ),
            Err(ProvenanceError::Admission(
                AdmissionError::MissingCostEvidence("startup work")
            ))
        ));
        assert!(matches!(
            consumer.admit_with_dependency(&producer, &bounded, cost, baseline, &mut budget),
            Err(ProvenanceError::Admission(AdmissionError::Constraint(
                "startup work"
            )))
        ));
    }
    producer.discard(owner, &mut ledger).unwrap();
    consumer.discard(owner, &mut ledger).unwrap();
    assert_eq!(ledger.retained_bytes(), 0);
}

#[test]
fn complete_package_qualification_owns_both_streams_and_retains_dependency_provenance() {
    with_source(|compiler, source, cell, body| {
        let policy = policy("");
        let input = inputs(compiler, source, cell, body, &policy);
        let producer = render(compiler, input.producers[0], &policy, Style::Global);
        assert!(matches!(
            compiler.qualify_artifact(
                producer,
                &policy,
                Objective::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::Artifact(_))
        ));
        let package = render(compiler, input.packages[0], &policy, Style::Scoped);
        compiler
            .measure_artifact(package, Objective::Brotli, WorkDomain::Baseline)
            .unwrap();
        let receipt = compiler
            .qualify_artifact(
                package,
                &policy,
                Objective::Brotli,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        compiler
            .with_qualified_artifact(&receipt, |view, provenance| {
                let dependency = view
                    .dependency
                    .expect("qualified package keeps its producer");
                assert_eq!(provenance.naming().style, Style::Scoped);
                assert_eq!(
                    receipt.cost().transfer_bytes as usize,
                    independent_sizes(view.javascript, Some(dependency.javascript))[2]
                );
            })
            .unwrap();
        assert!(compiler.take_qualified_artifact(receipt).is_err());
        compiler
            .with_qualified_artifact(&receipt, |view, _| assert!(view.dependency.is_some()))
            .unwrap();
        let bounded = policy_with(48, "[policy.constraints]\nmax_startup_work=0\n");
        assert!(matches!(
            compiler.qualify_artifact(
                package,
                &bounded,
                Objective::Brotli,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline
            ),
            Err(CandidateError::Admission(
                AdmissionError::MissingCostEvidence("startup work")
            ))
        ));
    });
}

#[test]
fn complete_package_keeps_consumer_and_producer_recipes_after_all_candidates_are_disposed() {
    with_source(|compiler, source, cell, body| {
        let policy = policy("");
        let input = inputs(compiler, source, cell, body, &policy);
        let (producer_words, pointer) = compiler
            .with_implementation_description(
                input.producers[1],
                WorkDomain::Baseline,
                |description| {
                    (
                        description.recipe_words().to_vec(),
                        description.recipe_words().as_ptr(),
                    )
                },
            )
            .unwrap();
        assert!(
            producer_words.len() > 6,
            "fixture has a selected function layout"
        );
        let consumer_words = compiler
            .with_implementation_description(
                input.packages[1],
                WorkDomain::Baseline,
                |description| description.recipe_words().to_vec(),
            )
            .unwrap();
        let package = render(compiler, input.packages[1], &policy, Style::Scoped);
        let receipt = compiler
            .qualify_artifact(
                package,
                &policy,
                Objective::Raw,
                ArtifactRuntimeEvidence::default(),
                None,
                WorkDomain::Baseline,
            )
            .unwrap();
        for candidate in [
            input.direct,
            input.fields,
            input.producers[0],
            input.producers[1],
            input.packages[0],
            input.packages[1],
        ] {
            compiler.discard(candidate.semantic_id()).unwrap();
        }
        compiler.discard(source).unwrap();
        compiler
            .with_qualified_artifact(&receipt, |view, provenance| {
                assert_eq!(view.implementation.recipe_words(), consumer_words);
                let ResourceDescription::Consumer { producer, .. } = view.implementation.resource()
                else {
                    panic!("complete consumer identity survives disposal");
                };
                assert_eq!(producer.recipe_words(), producer_words);
                assert_eq!(producer.recipe_words().as_ptr(), pointer);
                assert_eq!(producer.javascript(), view.dependency.unwrap().javascript);
                assert_eq!(producer.provenance().naming().style, Style::Global);
                assert_eq!(provenance.naming().style, Style::Scoped);
                assert_eq!(
                    receipt.cost().transfer_bytes as usize,
                    view.javascript.len() + producer.javascript().len()
                );
            })
            .unwrap();
        compiler.discard_artifact(package).unwrap();
    });
}

#[test]
fn required_consumer_root_preserves_delivery_in_zero_and_optional_search() {
    for zero in [true, false] {
        for transport in 0..2 {
            with_source(|compiler, source, cell, body| {
                let policy = policy_with(if zero { 0 } else { 48 }, "");
                let input = inputs(compiler, source, cell, body, &policy);
                // The required map already selects a parameter product and
                // shared function layout. Discovering the independent caller
                // cells must query the new opportunity's family, not .next().
                let parameter = compiler
                    .view(source)
                    .unwrap()
                    .unit(body)
                    .unwrap()
                    .parameters[0];
                let selected = compiler
                    .combine_javascript(
                        input.packages[transport],
                        input.fields,
                        &policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap();
                let ProductOutcome::Published(required) = compiler
                    .scalar_product_javascript(
                        selected,
                        parameter,
                        request().scalar,
                        &policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap()
                    .outcome
                else {
                    panic!("parameter product proof");
                };
                let root = directory();
                compiler
                    .with_implementation_description(
                        input.packages[transport],
                        WorkDomain::Baseline,
                        |_| (),
                    )
                    .unwrap();
                let before = compiler.ledger.retained_bytes();
                let producer = compiler
                    .with_implementation_description(
                        input.packages[transport],
                        WorkDomain::Baseline,
                        |description| {
                            let ResourceDescription::Consumer { producer, .. } =
                                description.resource()
                            else {
                                panic!("required Consumer");
                            };
                            producer.javascript().to_owned()
                        },
                    )
                    .unwrap();
                assert_eq!(compiler.ledger.retained_bytes(), before);
                let mut observed = Vec::new();
                // The opposite fixed producer is a valid optional package, but
                // it cannot replace this root's required immutable dependency.
                let mut search = compiler
                    .search_javascript_with_candidates_observed(
                        required.semantic_id(),
                        &[input.packages[1 - transport]],
                        &policy,
                        request(),
                        |view| {
                            assert!(matches!(
                                view.recipe_descriptor.resource(),
                                ResourceDescription::Consumer { .. }
                            ));
                            assert_eq!(view.dependency.unwrap().javascript, producer);
                            let sizes = independent_sizes(view.javascript, Some(&producer));
                            for (i, codec) in CODECS.into_iter().enumerate() {
                                assert_eq!(view.sizes.get(codec), Some(sizes[i]));
                            }
                            observed.push(Row {
                                candidate: view.candidate,
                                kind: (transport + 1) as u8,
                                style: view.naming.style,
                                output: view.output,
                                text: view.javascript.to_owned(),
                                producer: Some(producer.clone()),
                                sizes,
                            });
                        },
                    )
                    .unwrap();
                if zero {
                    assert_eq!(observed.len(), 1);
                    assert!(search.stopped().is_none());
                } else {
                    assert!(search.counters().conflicting_choices > 0);
                    assert!(search.counters().redundant_choices > 0);
                    // This fixture also copies into module-owned state. It
                    // does not promise another flattenable copy component;
                    // the distinct-family fixture below establishes that case.
                }
                for (index, row) in observed.iter().enumerate() {
                    execute(&root, &format!("required-{transport}-{zero}-{index}"), row);
                }
                for (i, codec) in CODECS.into_iter().enumerate() {
                    search
                        .with_winner(codec, |view, _| {
                            assert_eq!(view.dependency.unwrap().javascript, producer);
                            assert_eq!(
                                view.sizes.get(codec),
                                observed.iter().map(|row| row.sizes[i]).min()
                            );
                        })
                        .unwrap();
                }
                assert!(search.take_winner(Objective::Raw).is_none());
                assert!(search.with_winner(Objective::Raw, |_, _| ()).is_some());
                let winner = search.take_winner_artifact(Objective::Brotli).unwrap();
                drop(search);
                compiler
                    .with_artifact(winner, |view| {
                        assert_eq!(view.dependency.unwrap().javascript, producer);
                    })
                    .unwrap();
                compiler.discard_artifact(winner).unwrap();
                compiler
                    .with_implementations(input.packages[transport], |_| ())
                    .unwrap();
            });
        }
    }
}

#[test]
fn sparse_pool_scan_is_admitted_and_whole_pool_keeps_constant_work() {
    with_source(|compiler, source, cell, body| {
        let policy = policy("");
        let input = inputs(compiler, source, cell, body, &policy);
        let mut portfolio = Portfolio::new(RevisionId::fresh());
        let mut positions = Vec::new();
        for _ in 0..32 {
            positions.push(stage(
                compiler,
                &mut portfolio,
                input.packages[0],
                &policy,
                Style::Global,
            ));
        }
        {
            let mut budget =
                AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
            for &position in &positions[..31] {
                portfolio.discard_entry(position, &mut compiler.artifacts, &mut budget);
            }
        }
        assert_eq!(portfolio.resource_entries, 1);
        assert_eq!(portfolio.entries.len(), 1);
        assert!(portfolio.entries.capacity() >= 32);
        let primary = portfolio.entries.retained_text_bytes();
        // This query borrows actual immutable artifacts. Its separate tiny
        // work ledger owns no retained values and cannot cover the sparse scan.
        let mut denied = BudgetLedger::new_baseline_first(
            ResourceLimits::default(),
            BaselineFirstPlan {
                logical_work: 1,
                retained_bytes: 0,
                terminal_work: 0,
            },
        )
        .unwrap();
        {
            let mut budget = AllocationBudget::new(Some((&mut denied, WorkDomain::Baseline)));
            assert!(matches!(
                portfolio
                    .retained_package_bytes(&compiler.artifacts, &mut budget, primary, |_| true),
                Err(SearchError::Candidate(CandidateError::Budget(
                    BudgetError::WorkExhausted(WorkDomain::Baseline)
                )))
            ));
        }
        assert_eq!(denied.retained_bytes(), 0);
        assert_eq!(portfolio.entries.len(), 1);
        {
            let mut budget =
                AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
            portfolio.discard_entry(positions[31], &mut compiler.artifacts, &mut budget);
        }
        stage(
            compiler,
            &mut portfolio,
            input.direct,
            &policy,
            Style::Global,
        );
        assert_eq!(portfolio.resource_entries, 0);
        assert!(portfolio.entries.capacity() >= 32);
        let work = compiler.ledger.work_used(WorkDomain::Baseline);
        {
            let mut budget =
                AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
            assert_eq!(
                portfolio
                    .retained_package_bytes(
                        &compiler.artifacts,
                        &mut budget,
                        portfolio.entries.retained_text_bytes(),
                        |_| true
                    )
                    .unwrap(),
                portfolio.entries.retained_text_bytes()
            );
        }
        assert_eq!(compiler.ledger.work_used(WorkDomain::Baseline), work);
        let mut budget = AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
        portfolio.discard_all(&mut compiler.artifacts, &mut budget);
    });
}

#[test]
fn stale_supplied_input_is_not_a_choice_conflict_and_fragment_root_is_refused() {
    with_source(|compiler, source, cell, body| {
        let policy = policy("");
        let input = inputs(compiler, source, cell, body, &policy);
        let before = compiler.ledger.retained_bytes();
        assert!(matches!(
            compiler.search_javascript(input.producers[0].semantic_id(), &policy, request()),
            Err(SearchError::Candidate(CandidateError::Artifact(
                "producer fragment is not a complete search baseline"
            )))
        ));
        assert_eq!(compiler.ledger.retained_bytes(), before);
        let (unit, operation, expected_revision) = {
            let view = compiler.view(source).unwrap();
            (0..view.unit_count())
                .find_map(|index| {
                    let unit = UnitId::from_index(index).unwrap();
                    let operation =
                        view.unit(unit)
                            .unwrap()
                            .operations
                            .iter()
                            .position(|operation| {
                                matches!(
                                    operation.kind,
                                    OperationKind::Constant(Constant::Integer(0))
                                )
                            })?;
                    Some((
                        unit,
                        OpId::from_index(operation).unwrap(),
                        view.unit_revision(unit).unwrap(),
                    ))
                })
                .expect("authored zero in state initializer")
        };
        let kind = OperationKind::Constant(Constant::Integer(1));
        let edited = compiler
            .edit_source(
                source,
                &[UnitPatch {
                    unit,
                    expected_revision,
                    operations: &[OperationPatch {
                        operation,
                        kind: &kind,
                        operands: &[],
                    }],
                    places: &[],
                }],
                WorkDomain::Baseline,
            )
            .unwrap();
        let mut observed = 0;
        let search = compiler
            .search_javascript_with_candidates_observed(
                edited,
                &[input.packages[0]],
                &policy,
                request(),
                |_| observed += 1,
            )
            .unwrap();
        assert!(observed >= 1);
        assert!(matches!(
            search.stopped(),
            Some(SearchError::Candidate(CandidateError::StaleEvidence))
        ));
        assert_eq!(search.counters().conflicting_choices, 0);
        for codec in CODECS {
            assert!(search
                .with_winner(codec, |view, _| assert!(view.dependency.is_none()))
                .is_some());
        }
        drop(search);
        compiler
            .with_implementations(input.packages[0], |_| ())
            .unwrap();
        compiler
            .with_implementations(input.producers[0], |_| ())
            .unwrap();
        assert_ne!(
            compiler.view(source).unwrap().unit_revision(unit),
            compiler.view(edited).unwrap().unit_revision(unit)
        );
    });
}

#[test]
fn required_consumer_search_selects_exact_distinct_product_and_function_families() {
    const FAMILY_HOST: &str = include_str!("fixtures/search-resource-families/host.js");
    for transport in 0..2 {
        with_source_at(
            "search-resource-families/entry.lil",
            |compiler, source, cell, body| {
                let policy = policy("");
                let input = inputs(compiler, source, cell, body, &policy);
                let (local, saved, fold) = {
                    let slot = compiler.lookup(source).unwrap();
                    let program = &compiler.slots[slot]
                        .checkpoint
                        .as_ref()
                        .unwrap()
                        .semantic
                        .program;
                    let local = program
                        .cells()
                        .iter()
                        .position(|cell| cell.name == "local")
                        .unwrap();
                    let saved = program
                        .cells()
                        .iter()
                        .position(|cell| cell.name == "saved")
                        .unwrap();
                    let CellBinding::Function(fold) = program
                        .cells()
                        .iter()
                        .find(|cell| cell.name == "fold")
                        .unwrap()
                        .binding
                    else {
                        panic!("authored private fold function");
                    };
                    (
                        CellId::from_index(local).unwrap(),
                        CellId::from_index(saved).unwrap(),
                        fold,
                    )
                };
                let point = compiler
                    .view(source)
                    .unwrap()
                    .unit(body)
                    .unwrap()
                    .parameters[0];
                let selected = compiler
                    .combine_javascript(
                        input.packages[transport],
                        input.fields,
                        &policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap();
                let ProductOutcome::Published(required) = compiler
                    .scalar_product_javascript(
                        selected,
                        point,
                        request().scalar,
                        &policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap()
                    .outcome
                else {
                    panic!("required producer parameter product");
                };
                let ProductOutcome::Published(caller_fields) = compiler
                    .scalar_product_javascript(
                        required,
                        local,
                        request().scalar,
                        &policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap()
                    .outcome
                else {
                    panic!("independent local/saved product component");
                };
                let FunctionOutcome::Published(manual) = compiler
                    .scalar_function_javascript(
                        caller_fields,
                        fold,
                        request().scalar,
                        &policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap()
                    .outcome
                else {
                    panic!("independent fold transport");
                };
                // These are actual complete families in a nonempty published map.
                // The product lookup must find by covered cell, not canonical root;
                // the function lookup must select the requested second body.
                {
                    let slot = compiler.candidate_slot(manual).unwrap();
                    let map = compiler.slots[slot]
                        .checkpoint
                        .as_ref()
                        .unwrap()
                        .implementations
                        .as_ref()
                        .unwrap();
                    let mut budget =
                        AllocationBudget::new(Some((&mut compiler.ledger, WorkDomain::Baseline)));
                    assert_eq!(map.products().len(), 2);
                    assert_eq!(map.functions().len(), 2);
                    assert_eq!(
                        map.function_for_body(body, &mut budget)
                            .unwrap()
                            .unwrap()
                            .body(),
                        body
                    );
                    assert_eq!(
                        map.function_for_body(fold, &mut budget)
                            .unwrap()
                            .unwrap()
                            .body(),
                        fold
                    );
                    let producer_family =
                        map.product_for_cell(point, &mut budget).unwrap().unwrap();
                    let caller_family = map.product_for_cell(local, &mut budget).unwrap().unwrap();
                    assert_ne!(producer_family.root(), caller_family.root());
                    assert_eq!(
                        caller_family
                            .cells()
                            .iter()
                            .map(|cell| cell.cell)
                            .collect::<Vec<_>>(),
                        vec![local.min(saved), local.max(saved)]
                    );
                    assert_eq!(
                        map.product_for_cell(saved, &mut budget)
                            .unwrap()
                            .unwrap()
                            .root(),
                        caller_family.root()
                    );
                }
                let expected_words = compiler
                    .with_implementation_description(manual, WorkDomain::Baseline, |description| {
                        description.recipe_words().to_vec()
                    })
                    .unwrap();
                let artifact = render(compiler, manual, &policy, Style::Global);
                let manual_row = compiler
                    .with_artifact(artifact, |view| Row {
                        candidate: manual,
                        kind: (transport + 1) as u8,
                        style: Style::Global,
                        output: view.output,
                        text: view.javascript.to_owned(),
                        producer: view
                            .dependency
                            .map(|dependency| dependency.javascript.to_owned()),
                        sizes: independent_sizes(
                            view.javascript,
                            view.dependency.map(|dependency| dependency.javascript),
                        ),
                    })
                    .unwrap();
                compiler.discard_artifact(artifact).unwrap();
                let root = directory();
                execute_with_host(
                    &root,
                    &format!("family-{transport}-manual"),
                    &manual_row,
                    FAMILY_HOST,
                    "fixed-resource-family-order-ok\n",
                    "fixed-resource-search-family-artifact",
                );
                let mut reached = false;
                let mut observed = Vec::new();
                let search = compiler
                    .search_javascript_observed(
                        required.semantic_id(),
                        &policy,
                        request(),
                        |view| {
                            assert!(matches!(
                                view.recipe_descriptor.resource(),
                                ResourceDescription::Consumer { .. }
                            ));
                            assert_eq!(
                                view.dependency.map(|dependency| dependency.javascript),
                                manual_row.producer.as_deref()
                            );
                            let sizes = independent_sizes(
                                view.javascript,
                                view.dependency.map(|dependency| dependency.javascript),
                            );
                            for (i, codec) in CODECS.into_iter().enumerate() {
                                assert_eq!(view.sizes.get(codec), Some(sizes[i]));
                            }
                            if view.recipe_descriptor.recipe_words() == expected_words
                                && view.naming.style == Style::Global
                            {
                                assert_eq!(view.output, manual_row.output);
                                assert_eq!(view.javascript, manual_row.text);
                                reached = true;
                            }
                            observed.push(Row {
                                candidate: view.candidate,
                                kind: (transport + 1) as u8,
                                style: view.naming.style,
                                output: view.output,
                                text: view.javascript.to_owned(),
                                producer: view
                                    .dependency
                                    .map(|dependency| dependency.javascript.to_owned()),
                                sizes,
                            });
                        },
                    )
                    .unwrap();
                assert!(
                    reached,
                    "actual independent recipe missing: counters={:?}, stopped={:?}",
                    search.counters(),
                    search.stopped()
                );
                for (index, row) in observed.iter().enumerate() {
                    execute_with_host(
                        &root,
                        &format!("family-{transport}-search-{index}"),
                        row,
                        FAMILY_HOST,
                        "fixed-resource-family-order-ok\n",
                        "fixed-resource-search-family-artifact",
                    );
                }
                for (i, codec) in CODECS.into_iter().enumerate() {
                    search
                        .with_winner(codec, |view, _| {
                            assert_eq!(
                                view.sizes.get(codec),
                                observed.iter().map(|row| row.sizes[i]).min()
                            )
                        })
                        .unwrap();
                }
                drop(search);
                compiler.with_implementations(required, |_| ()).unwrap();
            },
        );
    }
}
