//! Six original source files enter discovery/checking/lowering separately.
//! Fixed runtime observations qualify direct output and existing representation
//! clients; this fixture does not stand in for a maintained-library fleet.
use super::facts::CacheLimits;
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BaselineFirstPlan, BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy,
    ResourceLimits, WorkDomain,
};
use crate::js::selection::{Objective, Objectives, Plan, Sizes, Style};
use serde_json::{json, Value as Json};
use sha2::{Digest, Sha256};
use std::path::Path;
use std::process::Command;

const FILES: [(&str, &str); 6] = [
    (
        "entry.lil",
        include_str!("fixtures/modules-javascript/entry.lil"),
    ),
    (
        "barrel.lil",
        include_str!("fixtures/modules-javascript/barrel.lil"),
    ),
    (
        "factory.lil",
        include_str!("fixtures/modules-javascript/factory.lil"),
    ),
    (
        "state.lil",
        include_str!("fixtures/modules-javascript/state.lil"),
    ),
    (
        "boot.lil",
        include_str!("fixtures/modules-javascript/boot.lil"),
    ),
    (
        "public.lil",
        include_str!("fixtures/modules-javascript/public.lil"),
    ),
];
const SETUP: &str = include_str!("fixtures/modules-javascript/setup.js");
const HOST: &str = include_str!("fixtures/modules-javascript/host.js");
const EXPECTED: &str = include_str!("fixtures/modules-javascript/expected.json");
const STYLES: [Style; 3] = [Style::Global, Style::Scoped, Style::Source];
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
const WORK: u64 = 200_000_000;
const MEMORY: u64 = 128_000_000;

fn digest(value: impl AsRef<[u8]>) -> String {
    format!("{:x}", Sha256::digest(value.as_ref()))
}

#[derive(Clone)]
struct Targets {
    record: CellId,
    step: CellId,
    label: CellId,
    strings: Vec<ValueRef>,
}
fn named_cell(program: &Program<'_>, name: &str) -> CellId {
    let mut found = program
        .cells
        .iter()
        .enumerate()
        .filter(|(_, cell)| cell.name == name);
    let id = CellId::from_index(found.next().expect(name).0).unwrap();
    assert!(found.next().is_none(), "ambiguous fixture cell {name}");
    id
}
fn with_modules<R>(inspect: impl FnOnce(Program<'_>, Targets, Json) -> R) -> R {
    let directory =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/modules-javascript");
    let discovered = crate::module::discover_modules(&directory.join("entry.lil")).unwrap();
    assert_eq!(discovered.modules.len(), FILES.len());
    for module in &discovered.modules {
        let name = module.path.file_name().unwrap().to_str().unwrap();
        let expected = FILES.iter().find(|(file, _)| *file == name).unwrap().1;
        assert_eq!(
            module.source, expected,
            "discovery must retain original file bytes"
        );
    }
    let arena = bumpalo::Bump::new();
    let syntax = crate::module::parse_modules(&arena, &discovered).unwrap();
    let semantics = crate::check::analyze_modules(&syntax, &discovered).unwrap();
    let program = super::from_source::from_checked_modules(&syntax, &semantics).unwrap();
    program.verify().unwrap();
    assert_eq!(program.modules.len(), 6);
    assert_eq!(
        program
            .units
            .iter()
            .filter(|unit| unit.data().kind == UnitKind::ModuleInitialization)
            .count(),
        6,
        "there is no fabricated combined initialization body"
    );
    for (index, module) in program.modules.iter().enumerate() {
        assert!(module.source.same(syntax[index].source_identity()));
        assert_eq!(
            program.unit(module.initializer).unwrap().module.index(),
            index
        );
    }
    for unit in &program.units {
        let module = unit.data().module.index();
        for operation in &unit.data().operations {
            if let Some(origin) = operation.origin {
                assert!(origin.index() < syntax[module].source_identity().len());
            }
        }
    }
    let order = program
        .initialization
        .iter()
        .map(|unit| {
            let module = program.unit(*unit).unwrap().module.index();
            discovered.modules[module]
                .path
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        order,
        [
            "boot.lil",
            "state.lil",
            "factory.lil",
            "public.lil",
            "barrel.lil",
            "entry.lil"
        ]
    );
    let exported = |name: &str| {
        program
            .exports()
            .iter()
            .find(|export| export.name == name)
            .unwrap()
            .target
    };
    assert_eq!(
        exported("total"),
        exported("liveTotal"),
        "live aliases share canonical storage"
    );
    assert_eq!(program.exports().len(), 6);
    assert!(program.exports.len() > program.exports().len());
    let strings = program
        .units
        .iter()
        .flat_map(|unit| {
            unit.data().operations.iter().filter_map(|operation| {
                (matches!(operation.kind, OperationKind::Binary(BinaryOp::Add))
                    && operation.result.is_some_and(|value| {
                        matches!(
                            program.types[unit.data().values[value.index()].ty.index()],
                            Type::String
                        )
                    }))
                .then(|| ValueRef {
                    unit: unit.id(),
                    value: operation.result.unwrap(),
                })
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(strings.len(), 1);
    let targets = Targets {
        record: named_cell(&program, "state"),
        step: named_cell(&program, "step"),
        label: named_cell(&program, "makeLabel"),
        strings,
    };
    let metadata = json!({
        "source_files": FILES.map(|(name, source)| json!({"name": name, "sha256": digest(source), "bytes": source.len()})),
        "initialization": order,
        "setup_sha256": digest(SETUP), "host_sha256": digest(HOST), "expected_sha256": digest(EXPECTED),
        "source_scope": "six separate original files through discovery/parse/analyze_modules/from_checked_modules; no source concatenation",
    });
    inspect(program, targets, metadata)
}
fn policy(compact: bool) -> ResolvedPolicy {
    policy_limits(compact, 48, 48)
}
fn policy_limits(compact: bool, proposals: usize, probes: usize) -> ResolvedPolicy {
    let config: crate::config::ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit={probes}\n[policy.tactics]\ntarget-compaction='{}'\nscalar-replacement='on'\ninlining='on'\nconstant-folding='on'\nstring-pooling='on'\nidentifier-mangling='on'\nnaming-search='on'\n",
        if compact { "on" } else { "off" }
    )).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
}
fn compiler<'src>(search: bool) -> Compilation<'src> {
    compiler_work(search, WORK)
}
fn compiler_work<'src>(search: bool, work: u64) -> Compilation<'src> {
    let limits = ResourceLimits::default();
    let ledger = if search {
        BudgetLedger::new_baseline_first(
            limits,
            BaselineFirstPlan {
                logical_work: work,
                retained_bytes: MEMORY,
                terminal_work: 0,
            },
        )
    } else {
        BudgetLedger::new(
            limits,
            BudgetPlan {
                baseline_work: work,
                optional_work: work,
                baseline_retained_bytes: 0,
                retained_bytes: MEMORY,
            },
        )
    }
    .unwrap();
    Compilation::new(ledger, CheckpointLimit { max_live: 32 }).unwrap()
}
fn cache() -> CacheLimits {
    CacheLimits {
        entries: 32,
        bytes: 2_000_000,
        result_bytes: 100_000,
    }
}
fn scalar_request() -> ScalarRequest {
    ScalarRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
    }
}
fn local_request() -> LocalFactsRequest {
    LocalFactsRequest {
        work_quota: 100_000,
        result_bytes: 100_000,
    }
}
fn helper_request() -> HelperRequest {
    HelperRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
        local_facts: local_request(),
    }
}
fn string_request() -> StringRequest {
    StringRequest {
        max_work: 1_000_000,
        scratch_bytes: 1_000_000,
        output_bytes: 1_000_000,
        local_facts: local_request(),
    }
}
fn execute(javascript: &str) {
    let script = format!("const events=[];{SETUP}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{HOST}\nprocess.stdout.write(JSON.stringify(events));", serde_json::to_string(javascript).unwrap());
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for module runtime observations");
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let actual: Json = serde_json::from_slice(&output.stdout).unwrap();
    let expected: Json = serde_json::from_str(EXPECTED).unwrap();
    assert_eq!(actual, expected, "{javascript}");
}
fn scores(sizes: Sizes) -> [usize; 3] {
    [sizes.raw, sizes.gzip9.unwrap(), sizes.brotli11.unwrap()]
}
fn search_metrics(
    search: &JavaScriptSearch<'_, '_>,
    policy: &ResolvedPolicy,
    case: &str,
    work_limit: u64,
    measured_count: usize,
) -> Json {
    let counters = search.counters();
    let objective = policy.objective().unwrap();
    json!({
        "case": case, "measured_count": measured_count,
        "schedule_version": crate::compilation_policy::SEARCH_SCHEDULE_VERSION,
        "stopped": search.stopped().map(|reason| format!("{reason:?}")),
        "discovery_refusal": search.discovery_refusal().map(|reason| format!("{reason:?}")),
        "limits": {"logical_work": work_limit, "retained_bytes": MEMORY,
            "proposals": objective.optional_alternatives,
            "codec_probes": objective.optional_codec_probes,
            "beam": objective.beam_width, "retained_candidates": objective.retained_candidates,
            "retained_candidate_bytes": objective.retained_candidate_bytes,
            "schedule": objective.search},
        "counters": {"proposals": counters.proposals,
            "structural_attempts": counters.structural_attempts,
            "skipped_unknown": counters.skipped_unknown, "skipped_truncated": counters.skipped_truncated,
            "proof_queries": counters.proof_queries,
            "unknown_proofs": counters.unknown_proofs, "truncated_proofs": counters.truncated_proofs,
            "inventory_truncated": counters.inventory_truncated,
            "conflicting_choices": counters.conflicting_choices, "duplicate_states": counters.duplicate_states,
            "structures": counters.structures, "renders": counters.renders,
            "codec_probes": counters.codec_probes, "admitted_artifacts": counters.admitted_artifacts,
            "beam_evictions": counters.beam_evictions, "queued_artifacts": counters.queued_artifacts,
            "pending_peak": counters.pending_peak, "pressure_scores": counters.pressure_scores,
            "diversity_scores": counters.diversity_scores, "scoring_events": counters.scoring_events},
        "work": {"baseline": search.ledger().work_used(WorkDomain::Baseline),
            "optional": search.ledger().work_used(WorkDomain::Optional)},
        "retained_bytes": search.ledger().retained_bytes(),
        "peak_retained_bytes": search.ledger().peak_retained_bytes(),
        "winners": CODECS.map(|codec| search.with_winner(codec, |view, plan| json!({
            "objective": format!("{codec:?}"), "style": format!("{:?}", plan.style),
            "javascript_sha256": digest(view.javascript), "sizes": scores(view.sizes)
        })).unwrap())
    })
}
fn artifact(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    compact: bool,
    representation: &str,
    style: Style,
    metadata: &Json,
) {
    let (javascript, measured) = measure_candidate(compiler, candidate, policy, style);
    eprintln!(
        "module-javascript-artifact {}",
        json!({
            "case": "six-source-module-contract", "compact": compact, "representation": representation,
            "style": format!("{style:?}"), "sources": metadata,
            "javascript": javascript, "javascript_sha256": digest(&javascript),
            "raw": measured[0], "gzip9": measured[1], "brotli11": measured[2],
            "observation": "fixed module/TDZ/live-binding/factory/host/finally expectations passed before scoring"
        })
    );
}
fn measure_candidate(
    compiler: &mut Compilation<'_>,
    candidate: CandidateId,
    policy: &ResolvedPolicy,
    style: Style,
) -> (String, [usize; 3]) {
    compiler
        .with_implementation_description(candidate, WorkDomain::Baseline, |_| ())
        .unwrap();
    let retained = compiler.ledger().retained_bytes();
    let (javascript, measured) = compiler
        .with_javascript_output(candidate, policy, |output| {
            let artifact = output.render(&Plan::new(style))?;
            output.with_artifact(artifact, |view| execute(view.javascript))?;
            for codec in CODECS {
                output.measure(artifact, codec)?;
            }
            let measured = output.with_artifact(artifact, |view| scores(view.sizes))?;
            Ok::<_, CandidateError>((output.take_artifact(artifact)?, measured))
        })
        .unwrap()
        .unwrap();
    assert_eq!(compiler.ledger().retained_bytes(), retained);
    assert_eq!(javascript.len(), measured[0]);
    (javascript, measured)
}

#[test]
fn six_original_modules_preserve_cycles_live_exports_factories_and_tdz_in_every_naming_style() {
    for compact in [false, true] {
        with_modules(|program, _, metadata| {
            let mut compiler = compiler(false);
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let policy = policy(compact);
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            for style in STYLES {
                artifact(
                    &mut compiler,
                    direct,
                    &policy,
                    compact,
                    "direct",
                    style,
                    &metadata,
                );
            }
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn record_helper_and_string_choices_use_the_same_original_module_environments() {
    for compact in [false, true] {
        with_modules(|program, targets, metadata| {
            let mut compiler = compiler(false);
            compiler
                .enable_local_facts(cache(), WorkDomain::Baseline)
                .unwrap();
            let source = compiler
                .adopt_checked(program, WorkDomain::Baseline)
                .unwrap();
            let policy = policy(compact);
            let direct = compiler
                .direct_javascript(source, &policy, WorkDomain::Baseline)
                .unwrap();
            let scalar = match compiler
                .scalar_javascript(
                    direct,
                    targets.record,
                    scalar_request(),
                    &policy,
                    WorkDomain::Optional,
                )
                .unwrap()
                .outcome
            {
                ScalarOutcome::Published(candidate) => candidate,
                other => panic!("existing private-record family must qualify: {other:?}"),
            };
            let mut choices = vec![("scalar", scalar)];
            for (label, cell) in [
                ("scalar-step", targets.step),
                ("scalar-step-label", targets.label),
            ] {
                let base = choices.last().unwrap().1;
                let inline = match compiler
                    .inline_helper_javascript(
                        base,
                        cell,
                        helper_request(),
                        &policy,
                        WorkDomain::Optional,
                    )
                    .unwrap()
                    .outcome
                {
                    HelperOutcome::Published(candidate) => candidate,
                    other => panic!("existing helper {label} must qualify: {other:?}"),
                };
                choices.push((label, inline));
            }
            let base = choices.last().unwrap().1;
            for (label, choice) in [
                ("literal", StringChoice::LiteralAtDefinition),
                (
                    "shared-literal",
                    StringChoice::SharedLiteral {
                        activation: targets.strings[0].unit,
                    },
                ),
            ] {
                let selected = match compiler
                    .represent_string_javascript(
                        base,
                        &targets.strings,
                        choice,
                        string_request(),
                        &policy,
                        WorkDomain::Optional,
                    )
                    .unwrap()
                    .outcome
                {
                    StringOutcome::Published(candidate) => candidate,
                    other => panic!("existing computed-string choice must qualify: {other:?}"),
                };
                choices.push((label, selected));
            }
            for (label, candidate) in choices {
                for style in STYLES {
                    artifact(
                        &mut compiler,
                        candidate,
                        &policy,
                        compact,
                        label,
                        style,
                        &metadata,
                    );
                }
            }
            assert_eq!(compiler.finish().retained_bytes(), 0);
        });
    }
}

#[test]
fn bounded_search_retains_exact_per_codec_winners_for_the_original_module_graph() {
    with_modules(|program, _, metadata| {
        let mut compiler = compiler(true);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true);
        let request = SearchRequest {
            objectives: Objectives::All,
            scalar: scalar_request(),
            helper: helper_request(),
            string: string_request(),
            facts_cache: cache(),
        };
        let mut measured = Vec::new();
        let search = compiler.search_javascript_observed(source, &policy, request, |observation| {
            execute(observation.javascript);
            let sizes = scores(observation.sizes);
            measured.push((digest(observation.javascript), sizes));
            eprintln!("module-search-artifact {}", json!({
                "sources": metadata, "recipe_fingerprint": observation.recipe_fingerprint,
                "recipe_descriptor": observation.recipe_descriptor.whole_words().expect("Whole cohort"),
                "style": format!("{:?}", observation.naming.style), "baseline": observation.baseline,
                "javascript": observation.javascript, "javascript_sha256": digest(observation.javascript),
                "raw": sizes[0], "gzip9": sizes[1], "brotli11": sizes[2],
                "observation": "fixed complete module trace passed for this committed exact artifact"
            }));
        }).unwrap();
        assert!(!measured.is_empty());
        for (index, objective) in CODECS.into_iter().enumerate() {
            let (hash, sizes) = search
                .with_winner(objective, |view, _| {
                    execute(view.javascript);
                    (digest(view.javascript), scores(view.sizes))
                })
                .unwrap();
            assert!(measured
                .iter()
                .any(|(observed, scores)| *observed == hash && *scores == sizes));
            assert_eq!(
                sizes[index],
                measured
                    .iter()
                    .map(|(_, sizes)| sizes[index])
                    .min()
                    .unwrap()
            );
            assert!(sizes[index] <= measured[0].1[index]);
        }
        assert!(search.counters().scoring_events > 0);
        eprintln!(
            "module-search-metrics {}",
            search_metrics(
                &search,
                &policy,
                "original-48-proposal-search",
                WORK,
                measured.len()
            )
        );
        drop(search);
        assert_eq!(compiler.finish().retained_bytes(), 0);
    });
}

/// These axes name publisher inputs, not patterns recognized in emitted JS.
/// Canonical words below come from the same identity owner as search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
struct OracleAxes {
    scalar: bool,
    inline_step: bool,
    inline_label: bool,
    string: OracleString,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
enum OracleString {
    Computed,
    Literal,
    Shared,
}
struct OracleArtifact {
    axes: OracleAxes,
    descriptor: Vec<u32>,
    plan: Plan,
    javascript: String,
    sizes: [usize; 3],
}

fn publish_inline(
    compiler: &mut Compilation<'_>,
    base: CandidateId,
    helper: CellId,
    policy: &ResolvedPolicy,
) -> CandidateId {
    match compiler
        .inline_helper_javascript(base, helper, helper_request(), policy, WorkDomain::Optional)
        .unwrap()
        .outcome
    {
        HelperOutcome::Published(candidate) => candidate,
        other => panic!("independent oracle helper must qualify: {other:?}"),
    }
}

fn module_oracle() -> Vec<OracleArtifact> {
    with_modules(|program, targets, _| {
        let mut compiler = compiler(false);
        compiler
            .enable_local_facts(cache(), WorkDomain::Baseline)
            .unwrap();
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = policy(true);
        let direct = compiler
            .direct_javascript(source, &policy, WorkDomain::Baseline)
            .unwrap();
        let scalar = match compiler
            .scalar_javascript(
                direct,
                targets.record,
                scalar_request(),
                &policy,
                WorkDomain::Optional,
            )
            .unwrap()
            .outcome
        {
            ScalarOutcome::Published(candidate) => candidate,
            other => panic!("independent oracle record must qualify: {other:?}"),
        };
        let mut oracle = Vec::new();
        for (scalar_selected, record) in [(false, direct), (true, scalar)] {
            let step_inline = publish_inline(&mut compiler, record, targets.step, &policy);
            for (step_selected, step) in [(false, record), (true, step_inline)] {
                let label_inline = publish_inline(&mut compiler, step, targets.label, &policy);
                for (label_selected, label) in [(false, step), (true, label_inline)] {
                    for string in [
                        OracleString::Computed,
                        OracleString::Literal,
                        OracleString::Shared,
                    ] {
                        let candidate = if string == OracleString::Computed {
                            label
                        } else {
                            let choice = match string {
                                OracleString::Literal => StringChoice::LiteralAtDefinition,
                                OracleString::Shared => StringChoice::SharedLiteral {
                                    activation: targets.strings[0].unit,
                                },
                                OracleString::Computed => unreachable!(),
                            };
                            match compiler
                                .represent_string_javascript(
                                    label,
                                    &targets.strings,
                                    choice,
                                    string_request(),
                                    &policy,
                                    WorkDomain::Optional,
                                )
                                .unwrap()
                                .outcome
                            {
                                StringOutcome::Published(candidate) => candidate,
                                other => {
                                    panic!("independent oracle string must qualify: {other:?}")
                                }
                            }
                        };
                        let axes = OracleAxes {
                            scalar: scalar_selected,
                            inline_step: step_selected,
                            inline_label: label_selected,
                            string,
                        };
                        let descriptor = compiler
                            .with_recipe_descriptor(candidate, WorkDomain::Optional, |words| {
                                words.to_vec()
                            })
                            .unwrap();
                        for style in STYLES {
                            // Runtime observations precede codec measurements inside the shared
                            // explicit-candidate helper, including for losing oracle states.
                            let (javascript, sizes) =
                                measure_candidate(&mut compiler, candidate, &policy, style);
                            eprintln!(
                                "module-search-oracle-artifact {}",
                                json!({
                                    "case": "24-compatible-module-recipes", "axes": axes,
                                    "recipe_descriptor": descriptor, "style": format!("{style:?}"),
                                    "javascript": javascript, "javascript_sha256": digest(&javascript),
                                    "raw": sizes[0], "gzip9": sizes[1], "brotli11": sizes[2],
                                    "observation": "fixed 33-event complete module trace passed before codec measurement"
                                })
                            );
                            oracle.push(OracleArtifact {
                                axes,
                                descriptor: descriptor.clone(),
                                plan: Plan::new(style),
                                javascript,
                                sizes,
                            });
                        }
                    }
                }
            }
        }
        assert_eq!(oracle.len(), 24 * STYLES.len());
        let unique = oracle
            .iter()
            .map(|entry| entry.descriptor.clone())
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            unique.len(),
            24,
            "each independently selected family combination has an exact identity"
        );
        assert_eq!(compiler.finish().retained_bytes(), 0);
        oracle
    })
}

struct DiscoveryArtifact {
    descriptor: Vec<u32>,
    plan: Plan,
    javascript: String,
    sizes: [usize; 3],
}
struct DiscoveryRun {
    observed: Vec<DiscoveryArtifact>,
    best: [usize; 3],
    baseline: [usize; 3],
    baseline_work: u64,
    optional_work: u64,
}
fn discovery_run(
    proposals: usize,
    work: u64,
    case: &str,
    oracle: &[OracleArtifact],
) -> DiscoveryRun {
    discovery_run_with_beam(proposals, work, case, oracle, None)
}
fn discovery_run_with_beam(
    proposals: usize,
    work: u64,
    case: &str,
    oracle: &[OracleArtifact],
    beam: Option<usize>,
) -> DiscoveryRun {
    with_modules(|program, _, _| {
        let mut compiler = compiler_work(true, work);
        let source = compiler
            .adopt_checked(program, WorkDomain::Baseline)
            .unwrap();
        let policy = match beam {
            None => policy_limits(true, proposals, 384),
            Some(width) => {
                let config: crate::config::ProjectConfig = toml::from_str(&format!(
                    "[javascript]\noptimization_level=15\nstrip_console=false\ncandidate_proposal_limit={proposals}\nterminal_codec_probe_limit=384\ncandidate_limit=64\ncandidate_beam_width={width}\n[policy.tactics]\ntarget-compaction='on'\nscalar-replacement='on'\ninlining='on'\nconstant-folding='on'\nstring-pooling='on'\nidentifier-mangling='on'\nnaming-search='on'\n"
                )).unwrap();
                config
                    .resolve_policy(CompilationRequest::JavaScript {
                        preserve_root_exports: true,
                    })
                    .unwrap()
            }
        };
        let request = SearchRequest {
            objectives: Objectives::All,
            scalar: scalar_request(),
            helper: helper_request(),
            string: string_request(),
            facts_cache: cache(),
        };
        let mut observed = Vec::new();
        let search = compiler.search_javascript_observed(source, &policy, request, |observation| {
        // The scorer has completed before this callback; do not publish a codec
        // claim in the test receipt until the independent full trace succeeds.
        execute(observation.javascript);
        let entry = DiscoveryArtifact { descriptor: observation.recipe_descriptor.whole_words().expect("Whole cohort").to_vec(),
            plan: observation.naming.clone(), javascript: observation.javascript.to_owned(),
            sizes: scores(observation.sizes) };
        if let Some(expected) = oracle.iter().find(|expected|
            expected.descriptor == entry.descriptor && expected.plan == entry.plan) {
            assert_eq!(entry.javascript, expected.javascript,
                "same pinned original-source contracts and deterministic semantic identities, recipes and naming must form the same complete artifact");
            assert_eq!(entry.sizes, expected.sizes);
        }
        eprintln!("module-discovery-artifact {}", json!({
            "case": case, "proposal_limit": proposals, "logical_work_limit": work,
            "recipe_descriptor": entry.descriptor, "style": format!("{:?}", entry.plan.style),
            "javascript": entry.javascript, "javascript_sha256": digest(&entry.javascript),
            "raw": entry.sizes[0], "gzip9": entry.sizes[1], "brotli11": entry.sizes[2],
            "baseline": observation.baseline,
            "observation": "fixed 33-event complete module trace passed before recording codec claims"
        }));
        observed.push(entry);
    }).unwrap();
        assert!(!observed.is_empty());
        let baseline = observed[0].sizes;
        let best = std::array::from_fn(|index| {
            search
                .with_winner(CODECS[index], |view, plan| {
                    assert!(
                        observed
                            .iter()
                            .any(|entry| entry.javascript == view.javascript
                                && entry.plan == *plan
                                && entry.sizes == scores(view.sizes)),
                        "winner must be a fully observed eligible artifact"
                    );
                    let measured = scores(view.sizes)[index];
                    assert_eq!(
                        measured,
                        observed
                            .iter()
                            .map(|entry| entry.sizes[index])
                            .min()
                            .unwrap()
                    );
                    assert!(measured <= baseline[index]);
                    measured
                })
                .unwrap()
        });
        let objective = policy.objective().unwrap();
        assert_eq!(objective.optional_alternatives, proposals);
        assert!(search.counters().proposals <= proposals);
        assert!(search.counters().codec_probes <= objective.optional_codec_probes);
        assert!(search.ledger().peak_retained_bytes() <= MEMORY);
        let baseline_work = search.baseline_seal().baseline_work;
        let optional_work = search.ledger().work_used(WorkDomain::Optional);
        assert!(baseline_work.checked_add(optional_work).unwrap() <= work);
        let covered = oracle
            .iter()
            .filter(|expected| {
                observed.iter().any(|entry| {
                    expected.descriptor == entry.descriptor && expected.plan == entry.plan
                })
            })
            .count();
        let mut metrics = search_metrics(&search, &policy, case, work, observed.len());
        metrics["oracle_artifact_coverage"] = json!(covered);
        metrics["oracle_artifacts"] = json!(oracle.len());
        metrics["oracle_minima"] = json!(std::array::from_fn::<_, 3, _>(|index| oracle
            .iter()
            .map(|entry| entry.sizes[index])
            .min()
            .unwrap()));
        eprintln!("module-search-metrics {metrics}");
        drop(search);
        // Reusable facts and arena slot metadata legitimately outlive Search;
        // Compilation owns their final release alongside every remaining source.
        assert_eq!(compiler.finish().retained_bytes(), 0);
        DiscoveryRun {
            observed,
            best,
            baseline,
            baseline_work,
            optional_work,
        }
    })
}

#[test]
fn independently_composed_module_recipes_bound_nested_search_quality() {
    let oracle = module_oracle();
    let literal_axes = OracleAxes {
        scalar: true,
        inline_step: true,
        inline_label: true,
        string: OracleString::Literal,
    };
    let known_literal = oracle
        .iter()
        .find(|entry| entry.axes == literal_axes && entry.plan.style == Style::Global)
        .unwrap();
    let computed = oracle
        .iter()
        .find(|entry| {
            entry.axes
                == OracleAxes {
                    string: OracleString::Computed,
                    ..literal_axes
                }
                && entry.plan == known_literal.plan
        })
        .unwrap();
    assert!(known_literal.sizes[0] < computed.sizes[0],
            "this real compatible literal remains a raw-size improvement over the both-inline computation");
    let mut largest = None;
    for proposals in [48, 96, 192, 384] {
        let run = discovery_run(proposals, WORK, "nested-proposal-limits", &oracle);
        if let Some(previous) = &largest {
            let previous: &DiscoveryRun = previous;
            assert_eq!(run.baseline, previous.baseline);
        }
        // No arbitrary cross-proposal prefix or monotonic-quality theorem:
        // terminal staged draining can change scoring/discovery order.
        largest = Some(run);
    }
    let largest = largest.unwrap();
    assert!(
        largest
            .observed
            .iter()
            .any(|entry| entry.descriptor == known_literal.descriptor
                && entry.plan == known_literal.plan),
        "a known eligible combination must be discovered at this explicit largest finite allowance"
    );
    // Beam search prunes by score, so whether the default width evicts a
    // subtree depends on the exact output bytes. The completeness claim is
    // made where it holds by construction: a beam that can keep this whole
    // 24-structure space matches the independent oracle on every objective.
    let complete = discovery_run_with_beam(384, WORK, "nested-complete-beam", &oracle, Some(24));
    for index in 0..3 {
        let minimum = oracle.iter().map(|entry| entry.sizes[index]).min().unwrap();
        assert!(
            complete.best[index] <= minimum,
            "a space-covering beam must match this small independent oracle on each objective: objective {index}, search {}, oracle {minimum}",
            complete.best[index]
        );
        assert!(largest.best[index] <= largest.baseline[index]);
    }
    // Work-only ceilings come from actual admitted baseline/optional work,
    // keeping the source, schedule, proposal and codec caps unchanged.
    let mut limits = [0, largest.optional_work / 2, largest.optional_work]
        .map(|optional| largest.baseline_work.checked_add(optional).unwrap());
    limits.sort_unstable();
    for work in limits {
        let run = discovery_run(384, work, "nested-work-limits", &oracle);
        assert_eq!(run.baseline, largest.baseline);
        if work == largest.baseline_work {
            assert_eq!(
                run.best, run.baseline,
                "zero optional work preserves the completely scored direct artifact"
            );
        }
    }
}
