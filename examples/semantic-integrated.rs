//! Public-API driver for the original-module architecture comparison.
//! Compiler phase clocks exclude artifact serialization and external execution.
//! Source edits are observable changes, not claims of optimization equivalence.
use clap::{Parser, ValueEnum};
use lilscript::compilation_policy::{BudgetLedger, WorkDomain, WorkKind};
use lilscript::compiler_service::{
    with_checked_path, CheckedSourceSession, ServiceJavaScript, ServiceOptions, ServiceTarget,
};
use lilscript::semantic_program::publication::*;
use lilscript::semantic_program::{CellBinding, CellId, Constant, OpId, OperationKind, UnitId};
use lilscript::structured_js::selection::{Objective, Objectives};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
enum Mode {
    Direct,
    Search,
    Retain,
    Advance,
    Native,
}

#[derive(Parser)]
struct Args {
    entry: PathBuf,
    #[arg(long)]
    config: PathBuf,
    #[arg(long)]
    output: PathBuf,
    #[arg(long, value_enum, default_value = "direct")]
    mode: Mode,
    #[arg(long)]
    level: Option<u8>,
    #[arg(long, default_value_t = 32)]
    edits: usize,
    /// Retain a real candidate after this numbered edit until final verification.
    #[arg(long)]
    retain_at: Option<usize>,
    #[arg(long, default_value_t = 200_000_000)]
    work: u64,
    #[arg(long, default_value_t = 256_000_000)]
    memory: u64,
}

fn result<T, E: Debug>(value: Result<T, E>) -> Result<T, String> {
    value.map_err(|error| format!("{error:?}"))
}
fn nanos(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_nanos()).expect("phase duration fits u64")
}
fn digest(bytes: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(bytes.as_ref()))
}
fn ledger_metrics(ledger: &BudgetLedger) -> Value {
    json!({
        "baseline_work": ledger.work_used(WorkDomain::Baseline),
        "optional_work": ledger.work_used(WorkDomain::Optional),
        "retained_bytes": ledger.retained_bytes(),
        "peak_retained_bytes": ledger.peak_retained_bytes(),
        "analysis_work": ledger.work_by_kind(WorkKind::Analysis),
        "edit_work": ledger.work_by_kind(WorkKind::Edit),
        "render_work": ledger.work_by_kind(WorkKind::Render),
        "codec_work": ledger.work_by_kind(WorkKind::Codec),
    })
}

struct Artifact {
    label: String,
    javascript: Arc<ServiceJavaScript>,
    expected_edit_value: i32,
}

fn direct_candidate(
    session: &mut CheckedSourceSession<'_>,
    source: SemanticId,
) -> Result<CandidateId, String> {
    let (compilation, _, policy) = session.parts_mut();
    result(compilation.direct_javascript(source, policy, WorkDomain::Baseline))
}

fn render(
    session: &mut CheckedSourceSession<'_>,
    candidate: CandidateId,
    label: &str,
    expected_edit_value: i32,
) -> Result<Artifact, String> {
    Ok(Artifact {
        label: label.to_owned(),
        javascript: Arc::new(result(session.render_javascript(candidate))?),
        expected_edit_value,
    })
}

fn edit_target(program: &SemanticView<'_, '_>) -> Result<(UnitId, OpId), String> {
    let mut cells = (0..program.cell_count())
        .map(|index| program.cell(CellId::from_index(index).unwrap()).unwrap())
        .filter(|cell| cell.name == "editTarget");
    let cell = cells
        .next()
        .ok_or("missing observable editTarget function")?;
    if cells.next().is_some() {
        return Err("ambiguous editTarget".into());
    }
    let CellBinding::Function(unit) = cell.binding else {
        return Err("editTarget must be a named function".into());
    };
    let mut constants = program
        .unit(unit)
        .unwrap()
        .operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| {
            matches!(
                operation.kind,
                OperationKind::Constant(Constant::Integer(11))
            )
        });
    let operation = constants
        .next()
        .ok_or("editTarget must contain its original integer 11")?
        .0;
    if constants.next().is_some() {
        return Err("editTarget integer 11 is ambiguous".into());
    }
    Ok((unit, OpId::from_index(operation).unwrap()))
}

fn edit_once(
    compilation: &mut Compilation<'_>,
    source: SemanticId,
    target: (UnitId, OpId),
    value: i32,
    advance: bool,
) -> Result<(SemanticId, Value), String> {
    // Patch construction and expected-revision lookup belong to the measured edit.
    let start = Instant::now();
    let revision = result(compilation.view(source))?
        .unit_revision(target.0)
        .unwrap();
    let kind = OperationKind::Constant(Constant::Integer(value));
    let operation = [OperationPatch {
        operation: target.1,
        kind: &kind,
        operands: &[],
    }];
    let patches = [UnitPatch {
        unit: target.0,
        expected_revision: revision,
        operations: &operation,
        places: &[],
    }];
    let successor = if advance {
        result(compilation.advance_source(source, &patches, WorkDomain::Baseline))?
    } else {
        result(compilation.edit_source(source, &patches, WorkDomain::Baseline))?
    };
    let edit_ns = nanos(start);
    let view = result(compilation.view(successor))?;
    let receipt = view.receipt();
    if !matches!(view.unit(target.0).unwrap().operations[target.1.index()].kind,
        OperationKind::Constant(Constant::Integer(found)) if found == value)
    {
        return Err("published edit did not install its requested value".into());
    }
    let changes = view
        .changes()
        .iter()
        .map(|change| change.unit.index())
        .collect::<Vec<_>>();
    if changes != [target.0.index()] {
        return Err(format!("unexpected changed units: {changes:?}"));
    }
    Ok((
        successor,
        json!({
            "edit_ns": edit_ns, "value": value,
            "logical_work": receipt.logical_work, "new_retained_payload_bytes": receipt.allocated_bytes,
            "copied_units": receipt.copied_units, "reused_units": receipt.reused_units,
            "copied_payload_bytes": receipt.copied_payload_bytes,
            "verified_units": receipt.verified_units, "verified_operations": receipt.verified_operations,
            "index": format!("{:?}", receipt.index), "changed_units": changes,
        }),
    ))
}

struct Workflow {
    artifacts: Vec<Artifact>,
    edits: Vec<Value>,
    mode_details: Value,
    cold_first_complete_ns: Option<u64>,
    native: Option<String>,
    backend_and_workflow_ns: u64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cold = Instant::now();
    let args = Args::parse();
    if args.edits == 0
        || args.edits > 100_000
        || args.retain_at.is_some_and(|n| n == 0 || n >= args.edits)
    {
        return Err("edits must be 1..=100000; retain-at must be before the final edit".into());
    }
    let config_text = std::fs::read_to_string(&args.config)?;
    let parsed = lilscript::config::parse_project_config(&config_text)?;
    for warning in &parsed.warnings {
        eprintln!("warning: {}: {warning}", args.config.display());
    }
    let mut config = parsed.config;
    if let Some(level) = args.level {
        config.javascript.optimization_level = level;
    }
    let options = ServiceOptions {
        target: if args.mode == Mode::Native {
            ServiceTarget::Native
        } else {
            ServiceTarget::JavaScript
        },
        preserve_root_exports: true,
        chunk_extension: Default::default(),
        objectives: Some(Objectives::All),
        logical_work: args.work,
        retained_bytes: args.memory,
        ..ServiceOptions::default()
    };
    let (workflow, finished) = with_checked_path(
        &args.entry,
        &config,
        options,
        |session| -> Result<Workflow, String> {
            let source = session.source();
            let target = if matches!(args.mode, Mode::Retain | Mode::Advance) {
                Some(edit_target(&result(session.compilation().view(source))?)?)
            } else {
                None
            };
            let mut artifacts = Vec::new();
            let mut edits = Vec::new();
            let mode_details;
            let mut cold_first_complete_ns = None;
            let mut native = None;
            let delivery = Instant::now();
            match args.mode {
                Mode::Native => {
                    let qualified = result(session.retain_native(source))?;
                    let (c, header) = result(
                        session
                            .compilation_mut()
                            .take_qualified_native_artifact(qualified),
                    )?;
                    if !header.is_empty() {
                        return Err("unexpected native host header".into());
                    }
                    mode_details = json!({"native_formation_ns": nanos(delivery)});
                    cold_first_complete_ns = Some(nanos(cold));
                    if session.compilation().ledger().work_by_kind(WorkKind::Codec) != 0 {
                        return Err("native request performed codec work".into());
                    }
                    native = Some(c);
                }
                Mode::Search => {
                    let batch = result(session.search_javascript(source, |observation| {
                        if observation.baseline && cold_first_complete_ns.is_none() {
                            cold_first_complete_ns = Some(nanos(cold));
                        }
                    }))?;
                    let search_ns = nanos(delivery);
                    let (delivered, winners, search_report) = batch.into_parts();
                    let delivered = delivered.into_iter().map(Arc::new).collect::<Vec<_>>();
                    for (index, codec) in CODECS.into_iter().enumerate() {
                        let artifact = delivered
                            [winners[index].ok_or("missing requested search winner")?]
                        .clone();
                        artifacts.push(Artifact {
                            label: format!("winner-{codec:?}"),
                            javascript: artifact,
                            expected_edit_value: 11,
                        });
                    }
                    mode_details = json!({"search_ns": search_ns,"search":search_report,
                "ledger":ledger_metrics(session.compilation().ledger())});
                }
                Mode::Direct | Mode::Retain | Mode::Advance => {
                    let candidate = direct_candidate(session, source)?;
                    artifacts.push(render(session, candidate, "baseline", 11)?);
                    cold_first_complete_ns = Some(nanos(cold));
                    let first_delivery_ns = nanos(delivery);
                    if let Some(target) = target {
                        // Both arms retain exactly the same baseline. Their first branch
                        // is the same admitted retained fork; only later ownership differs.
                        let (mut active, first) =
                            edit_once(session.compilation_mut(), source, target, 12, false)?;
                        edits.push(first);
                        let mut retained = None;
                        if args.retain_at == Some(1) {
                            let branch = direct_candidate(session, active)?;
                            artifacts.push(render(session, branch, "retained-branch", 12)?);
                            retained = Some((branch, 12));
                        }
                        for number in 2..=args.edits {
                            let value = if number % 2 == 0 { 11 } else { 12 };
                            let (next, mut receipt) = edit_once(
                                session.compilation_mut(),
                                active,
                                target,
                                value,
                                args.mode == Mode::Advance,
                            )?;
                            let release = Instant::now();
                            if args.mode == Mode::Retain {
                                result(session.compilation_mut().discard(active))?;
                            }
                            receipt["release_ns"] = json!(nanos(release));
                            active = next;
                            edits.push(receipt);
                            if args.retain_at == Some(number) {
                                let branch = direct_candidate(session, active)?;
                                artifacts.push(render(session, branch, "retained-branch", value)?);
                                retained = Some((branch, value));
                            }
                        }
                        let final_candidate = direct_candidate(session, active)?;
                        artifacts.push(render(
                            session,
                            final_candidate,
                            "final",
                            if args.edits % 2 == 0 { 11 } else { 12 },
                        )?);
                        if let Some((branch, value)) = retained {
                            let again = render(session, branch, "retained-branch-recheck", value)?;
                            let original = artifacts
                                .iter()
                                .find(|artifact| artifact.label == "retained-branch")
                                .unwrap();
                            if original.javascript.javascript() != again.javascript.javascript() {
                                return Err(
                                    "retained candidate changed while source advanced".into()
                                );
                            }
                            artifacts.push(again);
                        }
                        let baseline_again = render(session, candidate, "baseline-recheck", 11)?;
                        if baseline_again.javascript.javascript()
                            != artifacts[0].javascript.javascript()
                        {
                            return Err("baseline changed during edits".into());
                        }
                        artifacts.push(baseline_again);
                    }
                    mode_details = json!({"first_delivery_ns":first_delivery_ns});
                }
            }
            let backend_and_workflow_ns = nanos(delivery);
            Ok(Workflow {
                artifacts,
                edits,
                mode_details,
                cold_first_complete_ns,
                native,
                backend_and_workflow_ns,
            })
        },
    )?;
    let workflow = workflow?;
    if finished.ledger.retained_bytes() != 0 {
        return Err("compilation did not release its owned allocations".into());
    }
    let session_report = finished.report;
    let Workflow {
        artifacts,
        edits,
        mode_details,
        cold_first_complete_ns,
        native,
        backend_and_workflow_ns,
    } = workflow;
    // Final driver serialization, native hashing and writes are outside compiler clocks.
    std::fs::create_dir(&args.output)?;
    let sources = &session_report["inputs"]["modules"];
    let source_shape = &session_report["shape"];
    let phases = &session_report["phases_ns"];
    let policy = &session_report[if args.mode == Mode::Native {
        "native_policy"
    } else {
        "javascript_policy"
    }];
    let mut output_records = Vec::new();
    for artifact in artifacts {
        let file = format!("{}.mjs", artifact.label);
        std::fs::write(args.output.join(&file), artifact.javascript.javascript())?;
        output_records.push(json!({"label":artifact.label,"file":file,
            "sha256":artifact.javascript.sha256(),"bytes":artifact.javascript.javascript().len(),"details":artifact.javascript.details(),
            "expected_edit_value":artifact.expected_edit_value}));
    }
    let native_record = if let Some(c) = native {
        std::fs::write(args.output.join("native.c"), &c)?;
        Some(json!({"file":"native.c","sha256":digest(&c),"bytes":c.len()}))
    } else {
        None
    };
    let report = json!({
        "schema":1,"mode":format!("{:?}",args.mode),"entry":args.entry,
        "config":{"path":args.config,"sha256":digest(&config_text),"level_override":args.level},
        "policy":policy,"driver_ceilings":{"logical_work":args.work,"retained_bytes":args.memory,"checkpoints":128},
        "sources":sources,"source_shape":source_shape,
        "phase_ns":{"policy":phases["policy_ns"],"discovery_parse":phases["discovery_parse_ns"],"check":phases["check_ns"],
            "lower":phases["convert_ns"],"frontend_release":phases["frontend_release_ns"],"adopt":phases["adopt_ns"],"backend_and_workflow":backend_and_workflow_ns,"finish":phases["finish_ns"],"source_release":phases["source_release_ns"]},
        "mode_details":mode_details,"edits":edits,"retain_at":args.retain_at,
        "cold_first_complete_ns":cold_first_complete_ns,
        "cold_first_complete_scope":"From process entry before argument/config parsing through first fully scored and qualified JS artifact (all requested codecs), or qualified native C bytes; includes frontend input hashing, release and driver bookkeeping. Search timestamps its committed baseline without copying observation history.",
        "ledger_before_finish":session_report["ledger_before_finish"],"ledger_after_finish":session_report["ledger_after_finish"],
        "service":session_report,
        "artifacts":output_records,"native":native_record,
        "limits":"Discovery and parsing share one clock and one retained syntax owner; they are not independently timed. Frontend and edit phase clocks exclude driver record construction. Direct render clocks include qualification and delivered metadata/hashing. The whole backend workflow includes per-edit metric bookkeeping and qualified delivery metadata/hashing; shared winners move once and labels share caller-owned buffers. Final driver report serialization, artifact writes and external execution are outside both clocks. Whole-process RSS includes frontend and caller-owned diagnostics. Retained versus advance compares real source edits; it does not establish faster cold search. Native C compilation/runtime belongs to the external execution receipt.",
    });
    std::fs::write(
        args.output.join("report.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    Ok(())
}
