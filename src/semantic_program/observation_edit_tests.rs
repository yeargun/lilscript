//! A real place edit upgrades a formerly weak literal client to Exact. Output
//! choices belong to each immutable candidate; old bytes remain independently owned.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Objective, Plan, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::process::Command;

const BEFORE: &str = r#"extern JsValue effect(); extern void observe(string value);
export void run(){string first="weak-value";string second="exact-value";
JS.and(first,effect());observe(second);}"#;
const AFTER: &str = r#"extern JsValue effect(); extern void observe(string value);
export void run(){string first="weak-value";string second="exact-value";
JS.and(first,effect());observe(first);}"#;
const SETUP: &str = "globalThis.effect=()=>{events.push('effect');return 0;};globalThis.observe=value=>events.push(value);";
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const MODES: [LiteralOutput; 2] = [LiteralOutput::Original, LiteralOutput::Observed];
const WORK: u64 = 100_000_000;
const MEMORY: u64 = 128_000_000;

fn digest(value: &str) -> String {
    format!("{:x}", Sha256::digest(value.as_bytes()))
}
fn policy() -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(
        "[javascript]\noptimization_level=15\nstrip_console=false\n[policy.tactics]\ndead-code-elimination='on'\ntarget-compaction='on'\nidentifier-mangling='on'\nnaming-search='on'\ninlining='off'\nscalar-replacement='off'\ncall-specialization='off'\nconstant-folding='off'\nstring-pooling='off'",
    ).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn compilation<'src>() -> Compilation<'src> {
    Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: WORK,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 8 },
    )
    .unwrap()
}
fn checked<R>(source: &str, inspect: impl FnOnce(Program<'_>) -> R) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    inspect(program)
}
fn execute(javascript: &str, exact_value: &str) -> Json {
    let script = format!("const events=[];{SETUP}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));library.run();process.stdout.write(JSON.stringify(events));", serde_json::to_string(javascript).unwrap());
    let result = super::native_tests::execute(Command::new("node").args([
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
    assert_eq!(observed, json!(["effect", exact_value]));
    observed
}
fn output_metadata(output: OutputTactics) -> Json {
    json!({"dead_code_elimination":output.dead_code_elimination,
        "target_compaction":output.target_compaction,"literals":format!("{:?}",output.literals)})
}
#[derive(Debug, PartialEq, Eq)]
struct Row {
    style: Style,
    requested: LiteralOutput,
    output: OutputTactics,
    javascript: String,
    sizes: [usize; 3],
}
fn matrix(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    phase: &str,
    has_alternative: bool,
    exact_value: &str,
    retain_observed: bool,
) -> (Vec<Row>, Option<ArtifactId>) {
    compiler.with_javascript_output(candidate, policy, |output| {
        assert_eq!(output.has_literal_alternative()?, has_alternative);
        let mut rows = Vec::new();
        let mut retained = None;
        for style in STYLES {
            for requested in MODES {
                let artifact = output.render_bounded_with_literals(&Plan::new(style), requested, usize::MAX)?;
                let (observed, actual) = output.with_artifact(artifact, |view| {
                    (execute(view.javascript, exact_value), view.output)
                })?;
                assert_eq!(actual.literals, if has_alternative { requested } else { LiteralOutput::Original });
                assert!(actual.dead_code_elimination && actual.target_compaction);
                let sizes = [output.measure(artifact, Objective::Raw)?, output.measure(artifact, Objective::Gzip)?, output.measure(artifact, Objective::Brotli)?];
                // Diagnostic copies are test-owned; compiler ownership remains
                // with staging or the one explicitly retained artifact below.
                let javascript = output.with_artifact(artifact, |view| view.javascript.to_owned())?;
                assert_eq!(sizes[0], javascript.len());
                eprintln!("observation-edit-artifact {}", json!({"schema":1,"phase":phase,
                    "before":BEFORE,"after":AFTER,"before_sha256":digest(BEFORE),"after_sha256":digest(AFTER),
                    "setup":SETUP,"observations":"library.run();","expected":["effect",exact_value],"observed":observed,
                    "has_literal_alternative":has_alternative,"style":format!("{style:?}"),"requested_literals":format!("{requested:?}"),
                    "output":output_metadata(actual),"javascript":javascript,"javascript_sha256":digest(&javascript),
                    "raw":sizes[0],"gzip9":sizes[1],"brotli11":sizes[2]}));
                rows.push(Row { style, requested, output: actual, javascript, sizes });
                if retain_observed && style == Style::Scoped && requested == LiteralOutput::Observed {
                    retained = Some(output.retain_artifact(artifact)?);
                } else {
                    output.discard_artifact(artifact)?;
                }
            }
        }
        assert_eq!(output.has_literal_alternative()?, has_alternative);
        Ok::<_, CandidateError>((rows, retained))
    }).unwrap().unwrap()
}

fn targets(program: &Program<'_>) -> (UnitId, CellId, CellId, PlaceId) {
    let run = program
        .cells()
        .iter()
        .find(|cell| cell.name == "run")
        .unwrap();
    let CellBinding::Function(unit) = run.binding else {
        panic!("run is a named function")
    };
    let owned = |name| {
        let mut found = program
            .cells()
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.owner == unit && cell.name == name);
        let (index, _) = found.next().unwrap();
        assert!(found.next().is_none());
        CellId::from_index(index).unwrap()
    };
    let first = owned("first");
    let second = owned("second");
    let observe = CellId::from_index(
        program
            .cells()
            .iter()
            .position(|cell| cell.name == "observe")
            .unwrap(),
    )
    .unwrap();
    let data = program.unit(unit).unwrap();
    let mut calls = data.calls.iter().filter(|call| match call.target {
        CallTarget::Value { callee, .. } => {
            let definition = &data.operations[data.values[callee.index()].definition.index()];
            matches!(definition.kind, OperationKind::Load(place) if matches!(data.places[place.index()], Place::Cell(cell) if cell == observe))
        }
        CallTarget::Reference { place } => matches!(data.places[place.index()], Place::Cell(cell) if cell == observe),
        _ => false,
    });
    let call = calls.next().expect("original observe call");
    assert!(calls.next().is_none());
    let [CallArgument::Value(value)] = data.arguments(call.arguments).unwrap() else {
        panic!("observe has one value argument")
    };
    let definition = &data.operations[data.values[value.index()].definition.index()];
    let OperationKind::Load(place) = definition.kind else {
        panic!("observe reads its lexical string argument")
    };
    assert!(matches!(data.places[place.index()], Place::Cell(cell) if cell == second));
    (unit, first, second, place)
}

#[test]
fn weak_to_exact_place_edit_rebuilds_output_and_keeps_old_candidate_and_artifact() {
    checked(BEFORE, |program| {
        let (unit, first, second, place) = targets(&program);
        let policy = policy();
        let mut compiler = compilation();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let old = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let view = compiler.view(source).unwrap();
        let revision = view.unit_revision(unit).unwrap();
        let tables_revision = view.tables_revision();
        let first_revision = view.cell_users(first).unwrap().revision();
        let second_revision = view.cell_users(second).unwrap().revision();
        let (before, artifact) = matrix(
            &mut compiler,
            old,
            &policy,
            "before",
            true,
            "exact-value",
            true,
        );
        let artifact = artifact.unwrap();
        for pair in before.chunks_exact(2) {
            assert_ne!(pair[0].javascript, pair[1].javascript);
        }
        let replacement = Place::Cell(first);
        let patches = [UnitPatch {
            unit,
            expected_revision: revision,
            operations: &[],
            places: &[PlacePatch {
                place,
                replacement: &replacement,
            }],
        }];
        let edited = compiler
            .edit_source(source, &patches, WorkDomain::Optional)
            .unwrap();
        let view = compiler.view(edited).unwrap();
        let next_revision = view.unit_revision(unit).unwrap();
        assert_ne!(next_revision, revision);
        assert_eq!(view.unit_uses(unit).unwrap().revision(), next_revision);
        assert_eq!(view.tables_revision(), tables_revision);
        assert_eq!(
            view.changes(),
            &[UnitChange {
                unit,
                previous: revision,
                current: next_revision
            }]
        );
        assert_ne!(view.cell_users(first).unwrap().revision(), first_revision);
        assert_ne!(view.cell_users(second).unwrap().revision(), second_revision);
        assert_eq!(
            compiler.view(source).unwrap().unit_revision(unit),
            Some(revision)
        );
        assert_eq!(
            compiler
                .view(old.semantic_id())
                .unwrap()
                .unit_revision(unit),
            Some(revision)
        );
        let changed = compiler
            .direct_javascript(edited, &policy, WorkDomain::Optional)
            .unwrap();
        let (after, _) = matrix(
            &mut compiler,
            changed,
            &policy,
            "edited",
            false,
            "weak-value",
            false,
        );
        for pair in after.chunks_exact(2) {
            assert_eq!(pair[0].javascript, pair[1].javascript);
        }
        let clean = checked(AFTER, |program| {
            let mut clean = compilation();
            let source = clean.adopt_checked(program, WorkDomain::Baseline).unwrap();
            let candidate = clean
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let (rows, _) = matrix(
                &mut clean,
                candidate,
                &policy,
                "clean",
                false,
                "weak-value",
                false,
            );
            assert_eq!(clean.finish().retained_bytes(), 0);
            rows
        });
        assert_eq!(
            after, clean,
            "edited output must match a clean source rebuild for every name/mode"
        );
        let (again, _) = matrix(
            &mut compiler,
            old,
            &policy,
            "old-rerender",
            true,
            "exact-value",
            false,
        );
        assert_eq!(before, again);
        let old_observed = before
            .iter()
            .find(|row| row.style == Style::Scoped && row.requested == LiteralOutput::Observed)
            .unwrap();
        compiler
            .with_artifact(artifact, |view| {
                assert_eq!(view.candidate, old);
                assert_eq!(view.output, old_observed.output);
                assert_eq!(view.javascript, old_observed.javascript);
                assert_eq!(
                    [
                        view.sizes.raw,
                        view.sizes.gzip9.unwrap(),
                        view.sizes.brotli11.unwrap()
                    ],
                    old_observed.sizes
                );
                execute(view.javascript, "exact-value");
            })
            .unwrap();
        let retained = compiler.ledger().retained_bytes();
        let checkpoints = compiler.checkpoint_count();
        assert_eq!(
            compiler.edit_source(edited, &patches, WorkDomain::Optional),
            Err(PublicationError::StaleRevision(unit))
        );
        assert_eq!(compiler.ledger().retained_bytes(), retained);
        assert_eq!(compiler.checkpoint_count(), checkpoints);
        assert_eq!(
            compiler.view(edited).unwrap().unit_revision(unit),
            Some(next_revision)
        );
        compiler.discard_artifact(artifact).unwrap();
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}
