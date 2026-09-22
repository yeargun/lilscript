//! Explicit entry point for the architecture experiment, never a CLI fallback.
//! Stage clocks exclude diagnostic serialization and archival artifact writes.
use lilscript::compilation_contract::JavaScriptWorld;
use lilscript::structured_js::{
    analysis::Mode,
    extract::JavaScriptView,
    lower::lower_slice,
    optimize,
    selection::{Budget, Objective, Objectives, Plan, Style},
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("usage: structured-slice <source.lil> [--analysis none|tree|indexed|memoized|regions|values] [--fact-reuse unchanged|none] [--owned-data arrays|scalars] [--rounds N] [--metrics new.json]")?;
    let mut mode = None;
    let mut mode_name = String::from("none");
    let mut rounds = 3usize;
    let mut reuse_facts = true;
    let mut owned_data = optimize::OwnedData::Scalars;
    let mut metrics = None;
    let mut style = Style::Global;
    let mut search_plans = None;
    let mut search_bytes = 8 * 1024 * 1024;
    let mut objective = Objective::Brotli;
    let mut all_objectives = false;
    let mut search_artifacts = None;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--analysis" => {
                mode_name = args.next().ok_or("missing analysis mode")?;
                mode = match mode_name.as_str() {
                    "none" => None,
                    "tree" => Some(Mode::Tree),
                    "indexed" => Some(Mode::Indexed),
                    "memoized" => Some(Mode::Memoized),
                    "regions" => Some(Mode::Regions),
                    "values" => Some(Mode::Values),
                    _ => return Err("unknown analysis mode".into()),
                };
            }
            "--rounds" => rounds = args.next().ok_or("missing round count")?.parse()?,
            "--fact-reuse" => {
                reuse_facts = match args.next().as_deref() {
                    Some("unchanged") => true,
                    Some("none") => false,
                    _ => return Err("fact reuse must be unchanged or none".into()),
                }
            }
            "--metrics" => metrics = Some(args.next().ok_or("missing metrics path")?),
            "--name-style" => {
                style = match args.next().as_deref() {
                    Some("global") => Style::Global,
                    Some("scoped") => Style::Scoped,
                    Some("source") => Style::Source,
                    _ => return Err("unknown naming style".into()),
                }
            }
            "--search-plans" => {
                search_plans = Some(args.next().ok_or("missing search budget")?.parse()?)
            }
            "--search-bytes" => {
                search_bytes = args.next().ok_or("missing search byte budget")?.parse()?
            }
            "--search-all-objectives" => all_objectives = true,
            "--objective" => {
                objective = match args.next().as_deref() {
                    Some("raw") => Objective::Raw,
                    Some("gzip") => Objective::Gzip,
                    Some("brotli") => Objective::Brotli,
                    _ => return Err("unknown objective".into()),
                }
            }
            "--search-artifacts" => {
                search_artifacts = Some(std::path::PathBuf::from(
                    args.next().ok_or("missing artifact directory")?,
                ))
            }
            "--owned-data" => {
                owned_data = match args.next().as_deref() {
                    Some("arrays") => optimize::OwnedData::Arrays,
                    Some("scalars") => optimize::OwnedData::Scalars,
                    _ => return Err("owned data must be arrays or scalars".into()),
                };
            }
            _ => return Err(format!("unknown argument: {argument}").into()),
        }
    }
    if !(1..=16).contains(&rounds) {
        return Err("rounds must be in 1..=16".into());
    }
    if (search_artifacts.is_some() || all_objectives) && search_plans.is_none() {
        return Err("search artifacts and objectives require a search budget".into());
    }
    let start = Instant::now();
    let source = std::fs::read_to_string(path)?;
    let read_ms = start.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    let arena = bumpalo::Bump::new();
    let program = lilscript::parse_source(&arena, &source)?;
    let parse_ms = stage.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    let semantics = lilscript::analyze(&program)?;
    let analyze_ms = stage.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    let mut module = lower_slice(&program, &semantics).map_err(|error| {
        format!(
            "unsupported experimental source at {}..{}: {}",
            error.span.start, error.span.end, error.feature
        )
    })?;
    let lower_ms = stage.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    let mut reports = vec![];
    let mut unchanged_facts = None;
    if let Some(mode) = mode {
        for _ in 0..rounds {
            let result = optimize::optimize_with(
                &mut module,
                mode,
                JavaScriptWorld::ClosedApplication,
                owned_data,
            )?;
            let report = result.report;
            unchanged_facts = result.unchanged_facts;
            reports.push(report);
            if !report.changed() {
                break;
            }
        }
    }
    let optimize_ms = stage.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    if !reuse_facts {
        unchanged_facts = None;
    }
    let view = match (mode, unchanged_facts) {
        (Some(_), Some(facts)) => Some(JavaScriptView::prepare_reusing(&module, facts)?),
        (Some(mode), None) => Some(JavaScriptView::prepare_in_world(
            &module,
            mode,
            JavaScriptWorld::ClosedApplication,
        )),
        (None, _) => None,
    };
    let legalize_ms = stage.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    let output = match &view {
        Some(view) => view.output(),
        None => module.output(),
    }?;
    let output_prepare_ms = stage.elapsed().as_secs_f64() * 1000.0;
    let stage = Instant::now();
    let mut codec_ms = 0.0;
    let mut selected = None;
    let javascript = if let Some(plans) = search_plans {
        let objectives = if all_objectives {
            Objectives::All
        } else {
            Objectives::One(objective)
        };
        let selection = output.select(
            Budget {
                plans,
                candidate_bytes: search_bytes,
            },
            objectives,
            |bytes, codec| {
                let started = Instant::now();
                let sizes = lilscript::compression::measure(bytes, codec);
                codec_ms += started.elapsed().as_secs_f64() * 1000.0;
                sizes
            },
        )?;
        let winner = selection.winners[match objective {
            Objective::Raw => 0,
            Objective::Gzip => 1,
            Objective::Brotli => 2,
        }]
        .expect("selected objective was requested");
        selected = Some((selection, winner));
        None
    } else {
        Some(output.render(&Plan::new(style))?)
    };
    let render_ms = stage.elapsed().as_secs_f64() * 1000.0 - codec_ms;
    let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;
    let mut search_metrics = None;
    if let Some((selection, _)) = &selected {
        let plans = search_plans.expect("a selection has an explicit search budget");
        let objectives = selection.objectives;
        if let Some(directory) = &search_artifacts {
            std::fs::create_dir(directory)?;
            for (index, candidate) in selection.candidates.iter().enumerate() {
                std::fs::write(
                    directory.join(format!("candidate-{index}.mjs")),
                    &candidate.javascript,
                )?;
            }
        }
        let plan = |plan: &Plan| {
            serde_json::json!({
                "style":format!("{:?}",plan.style).to_lowercase(),
                "sourceNames":plan.source_names.iter().map(|symbol| symbol.index()).collect::<Vec<_>>(),
            })
        };
        search_metrics = Some(serde_json::json!({
            "budget":{"plans":plans,"candidateBytes":search_bytes},
            "objective":format!("{:?}",objective).to_lowercase(),
            "objectives":objectives.iter().map(|objective| format!("{:?}",objective).to_lowercase()).collect::<Vec<_>>(),
            "winners":selection.winners,
            "renderedBytes":selection.rendered_bytes,"retainedBytes":selection.retained_bytes,
            "duplicateArtifacts":selection.duplicate_artifacts,"measurementCalls":selection.measurement_calls,
            "proposalSteps":selection.proposal_steps,
            "attempts":selection.attempts.iter().map(|attempt| serde_json::json!({
                "plan":plan(&attempt.plan),"artifact":attempt.artifact,"rejection":attempt.rejection,
            })).collect::<Vec<_>>(),
            "candidates":selection.candidates.iter().enumerate().map(|(index,candidate)| serde_json::json!({
                "plan":plan(&candidate.plan),"sizes":candidate.sizes,
                "sha256":candidate.sha256.iter().map(|byte| format!("{byte:02x}")).collect::<String>(),
                "path":search_artifacts.as_ref().map(|directory| directory.join(format!("candidate-{index}.mjs"))),
            })).collect::<Vec<_>>(),
        }));
    }
    let javascript = javascript.unwrap_or_else(|| {
        let (selection, winner) = selected.as_mut().unwrap();
        std::mem::take(&mut selection.candidates[*winner].javascript)
    });
    let target_choices = view.as_ref().map(|view| {
        serde_json::json!({
            "omittedNormalizations": view.omitted_normalizations,
            "omittedFunctionNames":view.omitted_function_names,
            "nameWork":{"scannedStatements":view.name_work.scanned_statements,"candidates":view.name_work.candidates,"statements":view.name_work.statements,
                "expressions":view.name_work.expressions},
            "reusedFacts": view.reused_facts,
            "queries": view.work.queries, "expressionVisits": view.work.expression_visits,
            "storedSummaries": view.work.stored_summaries,
            "temporarySummaries": view.work.temporary_summaries,
            "flow":flow_metrics(view.work.flow),
        })
    });

    if let Some(path) = metrics {
        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)?;
        serde_json::to_writer_pretty(
            &mut file,
            &serde_json::json!({
                "schemaVersion":4,"analysis":mode_name,"roundLimit":rounds,
                "timingScope":"compiler stages exclude diagnostic serialization and artifact archive I/O",
                "factReuse":if reuse_facts { "unchanged" } else { "none" },
                "ownedData":if owned_data == optimize::OwnedData::Arrays { "arrays" } else { "scalars" },
                "sourceBytes":source.len(),"javascriptBytes":javascript.len(),
                "expressionStorageSlots":module.target().expressions.len(),
                "milliseconds":{"read":read_ms,"parse":parse_ms,"semantic":analyze_ms,
                    "lower":lower_ms,"optimize":optimize_ms,"legalize":legalize_ms,
                    "outputPreparation":output_prepare_ms,"render":render_ms,"codec":codec_ms,"total":elapsed_ms},
                "rounds":reports.iter().map(report_metrics).collect::<Vec<_>>(),"targetChoices":target_choices,"search":search_metrics,
                "outputPreparation":{"namingBases":1,"reusedStructure":output.reused_structure},
            }),
        )?;
        writeln!(file)?;
    }
    print!("{javascript}");
    Ok(())
}

fn flow_metrics(work: lilscript::structured_js::analysis::FlowWork) -> serde_json::Value {
    serde_json::json!({
        "expressions":work.expressions,"cellUpdates":work.cell_updates,
        "mergedCells":work.merged_cells,"invalidatedCells":work.invalidated_cells,
        "producerEdges":work.producer_edges,"joins":work.joins,
        "dependencyBytes":work.dependency_bytes,"peakLiveValues":work.peak_live_values,
    })
}

fn report_metrics(report: &optimize::Report) -> serde_json::Value {
    serde_json::json!({
                "removedBindings":report.removed_bindings,"removedStores":report.removed_stores,"inlinedBindings":report.inlined_bindings,
                "discardedExpressions":report.discarded_expressions,"simplifiedControl":report.simplified_control,
                "foldedConstants":report.folded_constants,
                "simplifiedIntegerArithmetic":report.simplified_integer_arithmetic,
                "removedAllocations":report.removed_allocations,
                "retainedForDepth":report.retained_for_depth,
                "editedRegions":report.edited_regions,"remappedExpressions":report.remapped_expressions,
                "insertedExpressions":report.inserted_expressions,
                "compacted":{"expressions":report.compacted.expressions,"regions":report.compacted.regions,
                    "functions":report.compacted.functions,"scopes":report.compacted.scopes,"bindings":report.compacted.bindings},
                "queries":report.work.queries,"expressionVisits":report.work.expression_visits,
                "storedSummaries":report.work.stored_summaries,
                "temporarySummaries":report.work.temporary_summaries,
                "verifiedStates":report.verified_states,"reusedStructure":report.reused_structure,
                "flow":flow_metrics(report.work.flow),

    })
}
