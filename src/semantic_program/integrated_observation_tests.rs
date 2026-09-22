//! Companion M1 consumer over the unchanged original integration portfolio.
//! This child reuses the parent publication/target/policy owners and fixed caps.
use super::*;
use std::collections::BTreeSet;

const ENTRY: &str = include_str!("fixtures/integrated-observation/entry.lil");
const DOCUMENT_HOST: &str = include_str!("fixtures/integrated-observation/host.js");
const DOCUMENT_EXPECTED: &str = include_str!("fixtures/integrated-observation/expected.json");
const MODES: [LiteralOutput; 2] = [LiteralOutput::Original, LiteralOutput::Observed];
const SOURCE_FILES: [&str; 13] = [
    "integrated-architecture/editable.lil",
    "integrated-architecture/entry.lil",
    "integrated-architecture/factory/barrel.lil",
    "integrated-architecture/factory/boot.lil",
    "integrated-architecture/factory/entry.lil",
    "integrated-architecture/factory/factory.lil",
    "integrated-architecture/factory/public.lil",
    "integrated-architecture/factory/state.lil",
    "integrated-architecture/marked-api.lil",
    "integrated-architecture/marked/rules.lil",
    "integrated-architecture/marked/str-slice.lil",
    "integrated-architecture/products.lil",
    "integrated-observation/entry.lil",
];

fn document_expected() -> Json {
    let expected: Json = serde_json::from_str(DOCUMENT_EXPECTED).unwrap();
    assert_eq!(expected.as_array().unwrap().len(), 108);
    // Preserve the existing base trace; the sole changed old row is the new
    // public export name. The 48 consumer events precede its final edit read.
    let mut base = super::expected(false);
    let exports = base
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|row| row[0] == "exports")
        .unwrap()[1]
        .as_array_mut()
        .unwrap();
    exports.push(json!("runDocument"));
    exports.sort_by(|a, b| a.as_str().unwrap().cmp(b.as_str().unwrap()));
    let base = base.as_array().unwrap();
    let rows = expected.as_array().unwrap();
    assert_eq!(&rows[..base.len() - 1], &base[..base.len() - 1]);
    assert_eq!(rows.last(), base.last());
    expected
}
fn document_modules<R>(inspect: impl FnOnce(Program<'_>, Json) -> R) -> R {
    super::verify_archives();
    let base = super::directory();
    let fixtures = base.parent().unwrap();
    assert_eq!(
        std::fs::read_to_string(fixtures.join("integrated-observation/entry.lil")).unwrap(),
        ENTRY
    );
    let modules =
        crate::module::discover_modules(&fixtures.join("integrated-observation/entry.lil"))
            .unwrap();
    let inventory: BTreeSet<_> = modules
        .modules
        .iter()
        .map(|module| {
            module
                .path
                .strip_prefix(fixtures)
                .unwrap()
                .to_str()
                .unwrap()
        })
        .collect();
    assert_eq!(inventory, SOURCE_FILES.into_iter().collect::<BTreeSet<_>>());
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &modules).unwrap();
    let checked = crate::semantic::analyze_modules(&syntax, &modules).unwrap();
    let program = from_checked_modules(&syntax, &checked).unwrap();
    program.verify().unwrap();
    assert_eq!(program.modules().len(), 13);
    assert_eq!(program.exports().len(), 11);
    for (index, interface) in program.modules().iter().enumerate() {
        assert!(interface.source.same(syntax[index].source_identity()));
        assert_eq!(
            program.unit(interface.initializer).unwrap().module.index(),
            index
        );
    }
    let files: Vec<_> = modules
        .modules
        .iter()
        .map(|module| {
            assert_eq!(
                std::fs::read_to_string(&module.path).unwrap(),
                module.source
            );
            json!({"file":module.path.strip_prefix(fixtures).unwrap().to_str().unwrap(),
            "source":module.source,"sha256":digest(&module.source)})
        })
        .collect();
    let initialization: Vec<_> = program
        .initialization()
        .iter()
        .map(|unit| {
            modules.modules[program.unit(*unit).unwrap().module.index()]
                .path
                .strip_prefix(fixtures)
                .unwrap()
                .to_str()
                .unwrap()
        })
        .collect();
    assert_eq!(
        initialization.last().copied(),
        Some("integrated-observation/entry.lil")
    );
    let metadata = json!({"files":files,"initialization":initialization,
        "entry":"integrated-observation/entry.lil","source_root":"src/semantic_program/fixtures",
        "config":CONFIG,"config_sha256":digest(CONFIG),"setup":SETUP,"setup_sha256":digest(SETUP),
        "host":DOCUMENT_HOST,"host_sha256":digest(DOCUMENT_HOST),"expected":document_expected(),
        "expected_sha256":digest(DOCUMENT_EXPECTED),"expected_json":DOCUMENT_EXPECTED,
        "scope":"13 original files: unchanged 12-module integration plus authored weak-dispatch consumer; complete Marked rules and attributed str excerpt, not full maintained library coverage"});
    inspect(program, metadata)
}
fn execute_document(javascript: &str) -> Json {
    let script = format!("const events=[];{SETUP}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{DOCUMENT_HOST}\nprocess.stdout.write(JSON.stringify(events));", serde_json::to_string(javascript).unwrap());
    let result = crate::semantic_program::native_tests::execute(Command::new("node").args([
        "--input-type=module",
        "-e",
        &script,
    ]));
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(result.stderr.is_empty());
    let observed: Json = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(observed, document_expected());
    observed
}
#[derive(Debug)]
struct Row {
    descriptor: Vec<u32>,
    style: Style,
    requested: Option<LiteralOutput>,
    output: OutputTactics,
    javascript: String,
    sizes: [usize; 3],
}
fn same_recipe(left: &Row, right: &Row) -> bool {
    left.descriptor == right.descriptor && left.style == right.style && left.output == right.output
}
fn emit(
    kind: &str,
    representation: Option<&str>,
    baseline: bool,
    row: &Row,
    observed: &Json,
    sources: &Json,
) {
    eprintln!(
        "integrated-observation-artifact {}",
        json!({"schema":1,"case":"document-consumer",
        "kind":kind,"representation":representation,"baseline":baseline,"sources":sources,
        "descriptor":row.descriptor,"style":format!("{:?}",row.style),
        "requested_literals":row.requested.map(|mode|format!("{mode:?}")),"output":output_metadata(row.output),
        "javascript":row.javascript,"javascript_sha256":digest(&row.javascript),
        "raw":row.sizes[0],"gzip9":row.sizes[1],"brotli11":row.sizes[2],"observed":observed})
    );
}
fn render_row(
    output: &mut BudgetedJavaScriptOutput<'_, '_>,
    descriptor: &[u32],
    style: Style,
    requested: Option<LiteralOutput>,
) -> Result<(Row, Json), CandidateError> {
    let artifact = match requested {
        Some(mode) => output.render_bounded_with_literals(&Plan::new(style), mode, usize::MAX)?,
        None => output.render(&Plan::new(style))?,
    };
    let (observed, actual) = output.with_artifact(artifact, |view| {
        (execute_document(view.javascript), view.output)
    })?;
    let measured = [
        output.measure(artifact, Objective::Raw)?,
        output.measure(artifact, Objective::Gzip)?,
        output.measure(artifact, Objective::Brotli)?,
    ];
    let javascript = output.take_artifact(artifact)?;
    assert_eq!(javascript.len(), measured[0]);
    Ok((
        Row {
            descriptor: descriptor.to_vec(),
            style,
            requested,
            output: actual,
            javascript,
            sizes: measured,
        },
        observed,
    ))
}

#[test]
fn document_consumer_runs_with_live_observed_literals_and_zero_optional_work() {
    document_modules(|program, sources| {
        let policy = super::policy(true);
        let mut compiler = super::compilation(0, false);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let candidate = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let descriptor = compiler
            .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let style = Plan::seeds_for_policy(&policy).unwrap()[0];
        compiler
            .with_javascript_output(candidate, &policy, |output| {
                assert!(output.has_literal_alternative()?);
                let (row, observed) = render_row(output, &descriptor, style, None)?;
                assert_eq!(row.output.literals, LiteralOutput::Observed);
                assert!(row.output.dead_code_elimination && row.output.target_compaction);
                emit(
                    "zero-optional",
                    Some("direct"),
                    true,
                    &row,
                    &observed,
                    &sources,
                );
                Ok::<_, CandidateError>(())
            })
            .unwrap()
            .unwrap();
        assert_eq!(compiler.ledger().work_used(WorkDomain::Optional), 0);
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

#[test]
fn document_consumer_literal_modes_compose_with_the_existing_portfolio_and_fixed_search() {
    let (manual, source_metadata, mapping) = document_modules(|program, sources| {
        let policy = super::policy(true);
        assert_eq!(policy.objective().unwrap().optional_alternatives, 96);
        let mapping = super::canonical_mapping(&program);
        let mut compiler = super::compilation(WORK, false);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let targets = super::targets(&compiler.view(source).unwrap());
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        compiler
            .enable_local_facts(super::cache(), WorkDomain::Optional)
            .unwrap();
        let mut manual = Vec::new();
        for (representation, candidate) in
            super::portfolio(&mut compiler, direct, &targets, &policy)
        {
            let descriptor = compiler
                .with_recipe_descriptor(candidate, WorkDomain::Baseline, |words| words.to_vec())
                .unwrap();
            let retained = compiler.ledger().retained_bytes();
            compiler
                .with_javascript_output(candidate, &policy, |output| {
                    assert!(
                        output.has_literal_alternative()?,
                        "consumer must remain a weak string client under every supplied map"
                    );
                    for style in STYLES {
                        for requested in MODES {
                            let (row, observed) =
                                render_row(output, &descriptor, style, Some(requested))?;
                            assert_eq!(row.output.literals, requested);
                            assert!(
                                row.output.dead_code_elimination && row.output.target_compaction
                            );
                            emit(
                                "manual",
                                Some(representation),
                                false,
                                &row,
                                &observed,
                                &sources,
                            );
                            manual.push((representation, row));
                        }
                        let pair = &manual[manual.len() - 2..];
                        assert_ne!(
                            pair[0].1.javascript, pair[1].1.javascript,
                            "active original/observed output must differ"
                        );
                    }
                    Ok::<_, CandidateError>(())
                })
                .unwrap()
                .unwrap();
            assert_eq!(compiler.ledger().retained_bytes(), retained);
        }
        assert_eq!(manual.len(), 72);
        assert_eq!(
            manual
                .iter()
                .map(|(_, row)| &row.descriptor)
                .collect::<BTreeSet<_>>()
                .len(),
            12
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
        (manual, sources, mapping)
    });
    let manual_minima: [usize; 3] = std::array::from_fn(|index| {
        manual
            .iter()
            .map(|(_, row)| row.sizes[index])
            .min()
            .unwrap()
    });
    document_modules(|program, sources| {
        assert_eq!(
            sources, source_metadata,
            "independent source owners must read the same 13 original files"
        );
        assert_eq!(
            super::canonical_mapping(&program),
            mapping,
            "descriptor namespaces must have equal indexed semantic owners"
        );
        let policy = super::policy(true);
        let objective = policy.objective().unwrap();
        assert_eq!(objective.optional_alternatives, 96);
        let mut compiler = super::compilation(WORK, true);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let mut measured = Vec::new();
        let mut baseline_count = 0;
        eprintln!(
            "integrated-observation-source {}",
            json!({"schema":1,"case":"document-consumer","sources":sources,
            "canonical_mapping_sha256":digest(&mapping),"policy":policy.receipt(),"manual_maps":12,"manual_artifacts":72,"manual_minima":manual_minima})
        );
        let search = compiler
            .search_javascript_observed(
                source,
                &policy,
                SearchRequest {
                    objectives: Objectives::All,
                    scalar: super::request(),
                    helper: super::helper_request(),
                    string: super::string_request(),
                    facts_cache: super::cache(),
                },
                |view| {
                    let observed = execute_document(view.javascript);
                    let row = Row {
                        descriptor: view
                            .recipe_descriptor
                            .whole_words()
                            .expect("Whole cohort")
                            .to_vec(),
                        style: view.naming.style,
                        requested: None,
                        output: view.output,
                        javascript: view.javascript.to_owned(),
                        sizes: super::sizes(view.sizes),
                    };
                    if view.baseline {
                        baseline_count += 1;
                        assert_eq!(view.output.literals, LiteralOutput::Observed);
                    }
                    if let Some((_, supplied)) = manual
                        .iter()
                        .find(|(_, supplied)| same_recipe(&row, supplied))
                    {
                        assert_eq!(
                            row.javascript, supplied.javascript,
                            "same source/map/name/actual output must yield the same bytes"
                        );
                        assert_eq!(row.sizes, supplied.sizes);
                    }
                    emit("search", None, view.baseline, &row, &observed, &sources);
                    measured.push((view.candidate, row));
                },
            )
            .unwrap();
        assert_eq!(baseline_count, 1);
        let minima: [usize; 3] = std::array::from_fn(|index| {
            measured
                .iter()
                .map(|(_, row)| row.sizes[index])
                .min()
                .unwrap()
        });
        let mut winners = Vec::new();
        for (index, codec) in CODECS.into_iter().enumerate() {
            winners.push(search.with_winner(codec, |view, plan| {
                let sizes = super::sizes(view.sizes);
                let (_, row) = measured.iter().find(|(candidate, row)| *candidate == view.candidate
                    && row.style == plan.style && row.output == view.output
                    && row.javascript == view.javascript && row.sizes == sizes).expect("winner belongs to the exact executed identity/output union");
                assert_eq!(sizes[index], minima[index]);
                let observed = execute_document(view.javascript);
                json!({"objective":format!("{codec:?}"),"descriptor":row.descriptor,"style":format!("{:?}",row.style),
                    "output":output_metadata(view.output),"javascript_sha256":digest(view.javascript),"sizes":sizes,"observed":observed})
            }).unwrap());
        }
        let comparison: Vec<_> = manual.iter().map(|(name, row)| json!({"representation":name,
            "descriptor":row.descriptor,"style":format!("{:?}",row.style),"requested_literals":row.requested.map(|mode|format!("{mode:?}")),
            "output":output_metadata(row.output),"javascript_sha256":digest(&row.javascript),"sizes":row.sizes,
            "reached":measured.iter().any(|(_, seen)| same_recipe(row, seen))})).collect();
        let gaps: [i64; 3] =
            std::array::from_fn(|index| minima[index] as i64 - manual_minima[index] as i64);
        let counters = search.counters();
        assert!(counters.proposals <= objective.optional_alternatives);
        assert!(counters.codec_probes <= objective.optional_codec_probes);
        eprintln!(
            "integrated-observation-summary {}",
            json!({"schema":1,"case":"document-consumer",
            "schedule_version":crate::compilation_policy::SEARCH_SCHEDULE_VERSION,"canonical_mapping_sha256":digest(&mapping),
            "manual_maps":12,"manual_artifacts":72,"measured":measured.len(),"manual_minima":manual_minima,"search_minima":minima,
            "search_minus_manual":gaps,"manual_comparison":comparison,"winners":winners,
            "caps":{"alternatives":objective.optional_alternatives,"codec_probes":objective.optional_codec_probes,"beam":objective.beam_width},
            "counters":{"structures":counters.structures,"proof_queries":counters.proof_queries,"renders":counters.renders,
                "proposals":counters.proposals,"structural_attempts":counters.structural_attempts,"codec_probes":counters.codec_probes,
                "unknown_proofs":counters.unknown_proofs,"skipped_unknown":counters.skipped_unknown,"skipped_truncated":counters.skipped_truncated,
                "duplicate_states":counters.duplicate_states,"inactive_function_layouts":counters.inactive_function_layouts,"beam_evictions":counters.beam_evictions},
            "stopped":search.stopped().map(|error|format!("{error:?}")),"discovery_refusal":search.discovery_refusal().map(|error|format!("{error:?}")),
            "work_baseline":search.ledger().work_used(WorkDomain::Baseline),"work_optional":search.ledger().work_used(WorkDomain::Optional),
            "peak_retained_bytes":search.ledger().peak_retained_bytes(),
            "scope":"same 13 original files and fixed 96-alternative base policy; twelve supplied maps times three names times two actual literal modes; finite search can miss supplied maps or lose any codec; no complete optimization claim"})
        );
        drop(search);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
