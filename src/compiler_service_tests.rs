use super::*;
use crate::compilation_policy::{PolicyConfig, TacticId, TacticPermission};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

fn config(extra: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{extra}"
    )).unwrap()
}

fn execute_javascript(javascript: &str, setup: &str, body: &str) -> String {
    let script = format!(
        "{setup}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));{body}",
        serde_json::to_string(javascript).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn check_scores(compiled: &ServiceCompilation) {
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        if let Some(artifact) = compiled.javascript(codec) {
            assert_eq!(
                artifact.sizes().get(codec).unwrap(),
                crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap()
            );
            assert_eq!(artifact.sha256(), digest(artifact.javascript().as_bytes()));
        }
    }
    assert_eq!(compiled.report()["backend"], "semantic");
    assert_eq!(
        compiled.report()["resources"]["retained_bytes_after_handoff"],
        0
    );
    assert!(
        compiled.report()["resources"]["frontend_logical_work"]
            .as_u64()
            .unwrap()
            > 0
    );
    assert_eq!(
        compiled.report()["resources"]["frontend_allocation_accounting"],
        "partial"
    );
    let resources = &compiled.report()["resources"];
    let phases = &resources["frontend_phase_accounting"];
    assert_eq!(
        phases["semantic_verification"],
        "pre-admitted work and owned scratch"
    );
    assert_eq!(
        phases["checker"],
        "pre-admitted fixed source-node tables, module facts/interfaces, initialization order and canonical declaration vectors; model stays live through conversion"
    );
    assert_eq!(
        phases["checker_module_graph"],
        "ownership/dependency/order validation work and shared iterative schedule work/storage admitted; exact dependency/import/export capacities"
    );
    assert_eq!(
        phases["checker_analyzer"],
        "scope/narrowing, type-parameter, return, constructor and generator context vector backing admitted; dropped and released per Analyzer without releasing shared declarations"
    );
    assert_eq!(
        phases["checker_binary"],
        "existing iterative expression visits and continuation probes charged; continuation vector backing/growth admitted and released after each expression, including nested overlap and ordinary failures"
    );
    assert_eq!(
        phases["checker_narrowing"],
        "leaf queries and iterative traversal probes charged; pending/answer vector backing and growth overlap admitted and released per query; nested maps/types remain uninstrumented"
    );
    assert_eq!(
        phases["checker_narrowing_reuse"],
        "binary continuations retain syntax-only guard inputs/projections; empty branches are skipped, retained guards re-resolve in the active scope; no result cache or source reassociation"
    );
    assert!(phases["checker_remaining"]
        .as_str()
        .unwrap()
        .contains("uninstrumented"));
    assert_eq!(
        phases["conversion_construction"],
        "pre-admitted graph and temporary storage; retained charges transfer into publication"
    );
    assert_eq!(
        phases["lexer_tokens"],
        "pre-admitted Vec capacities, including overlapping nested fragments"
    );
    assert_eq!(
        phases["lexer_templates"],
        "pre-admitted scanner frames, interpolation vectors/boxed copies and template tables; scanner byte work charged"
    );
    assert_eq!(
        phases["lexer_work"],
        "full input-byte tariff admitted before each lexical stream, including nested fragments; cooperative checks after Logos calls"
    );
    assert_eq!(
        phases["parser_lookahead_work"],
        "each token probe, including EOF, in arrow and type/reference lookahead admitted before inspection"
    );
    assert_eq!(
        phases["source_identity"],
        "inline opaque stamp and node count; included in typed owner capacities, no shared heap allocation"
    );
    if resources["source_buffer_capacity"].is_u64() {
        assert_eq!(
            phases["discovery_parse_arena"],
            "one pre-admitted arena and owned program list, reused for checking"
        );
        assert_eq!(
            phases["main_parse_arena"],
            "not repeated; discovery programs reused"
        );
        assert!(compiled.report()["phases_ns"]["discovery_parse_ns"].is_u64());
        assert!(compiled.report()["phases_ns"]["parse_ns"].is_null());
    } else {
        assert_eq!(phases["discovery_parse_arena"], "not used");
        assert_eq!(phases["main_parse_arena"], "pre-admitted arena backing");
    }
    assert_eq!(
        phases["legacy_ast_specialization"],
        "nonzero for_of_specialize_family unsupported"
    );
}

#[test]
fn source_service_returns_independently_scored_qualified_winners() {
    let result = compile_source_semantic(
        "export int byte(int value){return(value&255)+1;}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "const events=[library.byte.name,library.byte.length,library.byte(-1)];events.push(library.byte({valueOf(){events.push('coerce');return 511;}}));console.log(JSON.stringify(events));"
            ),
            "[\"byte\",1,256,\"coerce\",256]\n"
        );
    }
}

#[test]
fn source_service_publishes_value_struct_functions_through_the_d2_adapter() {
    // Every searched winner, not only the direct artifact, carries the same
    // public object ABI: a fresh object out, one read per field in.
    let result = compile_source_semantic(
        "struct Point{int x;int y;}\
         export Point make(int x,int y){return Point{x,y};}\
         export int sum(Point p){return p.x+p.y;}\
         export Point shift(Point p,int by){return Point{p.x+by,p.y+by};}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "const reads=[];const p=library.shift({get x(){reads.push('x');return 1},get y(){reads.push('y');return 2}},3);\
                 console.log(JSON.stringify([p,Object.keys(p),library.sum(library.make(4,5)),library.make(1,2)!==library.make(1,2),reads,\
                 [library.make.name,library.sum.name,library.shift.name],[library.make.length,library.sum.length,library.shift.length]]));"
            ),
            "[{\"x\":4,\"y\":5},[\"x\",\"y\"],9,true,[\"x\",\"y\"],[\"make\",\"sum\",\"shift\"],[2,1,2]]\n"
        );
    }
}

#[test]
fn path_service_publishes_re_exported_value_struct_functions_with_source_names() {
    // A struct function declared in one module and re-exported under an
    // alias by another keeps its source name, as JavaScript does for
    // `export {translate as move}`.
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/public-structs");
    let result = compile_path_semantic(
        &root.join("entry.lil"),
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "const o=library.origin();const p=library.move(o,3,-4);\
                 console.log(JSON.stringify([o,p,library.manhattan(o,p),[library.move.name,library.origin.name,library.manhattan.name],[library.move.length,library.manhattan.length],'translate' in library]));"
            ),
            "[{\"x\":0,\"y\":0},{\"x\":3,\"y\":-4},7,[\"translate\",\"origin\",\"manhattan\"],[3,2],false]\n"
        );
    }
}

#[test]
fn shared_winner_handoff_and_all_optional_off_keep_source_literals_and_api() {
    let mut config = config("");
    config.policy = Some(PolicyConfig {
        tactics: TacticId::ALL
            .into_iter()
            .map(|tactic| (tactic, TacticPermission::Off))
            .collect(),
        ..PolicyConfig::default()
    });
    let result = compile_source_semantic(
        "export string answer(){return \"kept\";}",
        &config,
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    check_scores(&result);
    assert_eq!(result.report()["search"]["proposals"], 0);
    let first = result.javascript(Objective::Raw).unwrap();
    for codec in [Objective::Gzip, Objective::Brotli] {
        assert!(std::ptr::eq(first, result.javascript(codec).unwrap()));
    }
    assert_eq!(
        execute_javascript(first.javascript(), "", "console.log(library.answer());"),
        "kept\n"
    );
}

#[test]
fn path_service_preserves_original_module_initialization_callbacks_and_public_objects() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/modules-javascript");
    let result = compile_path_semantic(
        &root.join("entry.lil"),
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap();
    check_scores(&result);
    assert_eq!(result.report()["shape"]["modules"], 6);
    assert_eq!(
        result.report()["inputs"]["modules"]
            .as_array()
            .unwrap()
            .len(),
        6
    );
    let setup = format!(
        "const events=[];{}",
        include_str!("semantic_program/fixtures/modules-javascript/setup.js")
    );
    let body = format!(
        "{}\nconsole.log(JSON.stringify(events));",
        include_str!("semantic_program/fixtures/modules-javascript/host.js")
    );
    let output = execute_javascript(
        result.javascript(Objective::Brotli).unwrap().javascript(),
        &setup,
        &body,
    );
    let actual: Value = serde_json::from_str(&output).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "semantic_program/fixtures/modules-javascript/expected.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn integrated_original_modules_execute_through_fast_and_searched_public_service() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/integrated-architecture");
    let expected: Value = serde_json::from_str(include_str!(
        "semantic_program/fixtures/integrated-architecture/expected.json"
    ))
    .unwrap();
    let setup = format!(
        "const events=[];{}",
        include_str!("semantic_program/fixtures/integrated-architecture/setup.js")
    );
    let body = format!(
        "{}\nprocess.stdout.write(JSON.stringify(events));",
        include_str!("semantic_program/fixtures/integrated-architecture/host.js")
    );
    for search in [false, true] {
        let mut config = config("");
        if !search {
            config.javascript.candidate_search = crate::config::CandidateSearch::Off;
        }
        let result = compile_path_semantic(
            &root.join("entry.lil"),
            &config,
            ServiceOptions {
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        check_scores(&result);
        assert!(result.report()["shape"]["modules"].as_u64().unwrap() > 6);
        if !search {
            assert_eq!(result.report()["search"]["proposals"], 0);
        }
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let actual: Value = serde_json::from_str(&execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                &setup,
                &body,
            ))
            .unwrap();
            assert_eq!(actual, expected, "search={search}, codec={codec:?}");
        }
    }
    let native = compile_path_semantic(
        &root.join("native-entry.lil"),
        &config(""),
        ServiceOptions {
            target: ServiceTarget::Native,
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert_eq!(
        execute_native(native.native_c().unwrap()),
        include_str!("semantic_program/fixtures/integrated-architecture/native.expected.out")
    );
    assert_eq!(native.report()["resources"]["codec_work"], 0);
}

#[test]
fn service_rejects_unknown_runtime_cost_and_unsupported_source_without_fallback() {
    let result = compile_source_semantic(
        "export int answer(){return 17;}",
        &config("[policy.constraints]\nmax_startup_work=0"),
        ServiceOptions::default(),
    );
    assert!(result.is_err(), "unknown is not zero");
    let result = compile_source_semantic(
        "struct P{int x;}export void add(P[] items){items.push(P{1});}",
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap_err();
    // An exported array of value structs that the function mutates has no
    // D2 adapter, so the target refuses it.
    assert_eq!(result.phase, "javascript");
    assert!(
        result.message.contains("public value-struct ABI adaptation"),
        "{result}"
    );
    // The target session no longer holds source text, so a formation refusal
    // carries its span in the message but no rendered diagnostic (011 work).
    assert!(result.diagnostic.is_none());
    // Multi-file delivery of one source module is its entry file alone.
    for mode in ["preserve-modules", "split"] {
        let result = compile_source_semantic(
            "export int answer(){return 17;}",
            &config(&format!("[bundle]\nmode='{mode}'")),
            ServiceOptions::default(),
        )
        .unwrap();
        assert!(result
            .javascript(Objective::Brotli)
            .unwrap()
            .chunks()
            .is_empty());
    }
}

#[test]
fn resource_owner_exists_before_discovery_and_parse() {
    let options = ServiceOptions {
        logical_work: 0,
        ..ServiceOptions::default()
    };
    let source = compile_source_semantic("not valid syntax", &config(""), options).unwrap_err();
    assert_eq!(source.phase, "frontend resources");
    let path =
        compile_path_semantic(Path::new("/does-not-exist.lil"), &config(""), options).unwrap_err();
    assert_eq!(path.phase, "frontend resources");
}

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-service-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if !std::thread::panicking() {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
}

fn execute_native(c: &str) -> String {
    let scratch = Scratch::new();
    let executable = scratch.0.join("program");
    let cc = std::env::var_os("LILSCRIPT_NATIVE_CC").unwrap_or_else(|| "cc".into());
    let mut child = Command::new(cc)
        .args([
            "-x",
            "c",
            "-std=c11",
            "-O1",
            "-fno-fast-math",
            "-ffp-contract=off",
            "-o",
        ])
        .arg(&executable)
        .args(["-", "-lm"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(c.as_bytes()).unwrap();
    let status = child.wait_with_output().unwrap();
    assert!(
        status.status.success(),
        "{}",
        String::from_utf8_lossy(&status.stderr)
    );
    let output = Command::new(&executable).output().unwrap();
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn native_and_all_share_checked_meaning_without_native_codec_work() {
    let source = "int twice(int value){return value*2;}print(twice(21));";
    let javascript = compile_source_semantic(
        source,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for target in [ServiceTarget::Native, ServiceTarget::All] {
        let result = compile_source_semantic(
            source,
            &config(""),
            ServiceOptions {
                target,
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(execute_native(result.native_c().unwrap()), "42\n");
        if target == ServiceTarget::Native {
            assert_eq!(result.report()["resources"]["codec_work"], 0);
            assert!(result.javascript(Objective::Brotli).is_none());
        } else {
            check_scores(&result);
            let standalone_peak = javascript.report()["resources"]["peak_retained_bytes"]
                .as_u64()
                .unwrap();
            let combined_peak = result.report()["resources"]["peak_retained_bytes"]
                .as_u64()
                .unwrap();
            assert!(
                combined_peak >= standalone_peak + result.native_c().unwrap().len() as u64,
                "native output stays admitted throughout JavaScript search"
            );
            let output = Command::new("node")
                .args([
                    "-e",
                    result.javascript(Objective::Brotli).unwrap().javascript(),
                ])
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(String::from_utf8(output.stdout).unwrap(), "42\n");
        }
    }
}

#[test]
fn native_and_all_reject_unknown_required_runtime_evidence() {
    for target in [ServiceTarget::Native, ServiceTarget::All] {
        let error = compile_source_semantic(
            "print(42);",
            &config("[policy.constraints]\nmax_startup_work=0"),
            ServiceOptions {
                target,
                ..ServiceOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, "native");
    }
}

#[test]
fn scoped_search_and_default_service_share_winners_handoff_and_budget() {
    let source =
        "int byte(int value){return value&255;}export int answer(int value){return byte(value)+1;}";
    let config = config("");
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let default = compile_source_semantic(source, &config, options).unwrap();
    let (batch, finished) = with_checked_source(source, &config, options, |session| {
        assert!(session.phases()["frontend_release_ns"].is_u64());
        assert_eq!(session.compilation().checkpoint_count(), 1);
        let mut baselines = 0;
        let batch = session
            .search_javascript(session.source(), |observation| {
                baselines += usize::from(observation.baseline);
            })
            .unwrap();
        assert_eq!(baselines, 1);
        batch
    })
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let scoped = batch.javascript(codec).unwrap();
        assert_eq!(
            scoped.javascript(),
            default.javascript(codec).unwrap().javascript()
        );
        assert_eq!(scoped.sizes(), default.javascript(codec).unwrap().sizes());
        let canonical_details = |artifact: &ServiceJavaScript| {
            let mut details = artifact.details().clone();
            let semantic = details["semantic"].as_object_mut().unwrap();
            assert!(semantic.remove("snapshot_identity").unwrap().is_string());
            assert!(semantic.remove("meaning_identity").unwrap().is_string());
            details
        };
        assert_eq!(
            canonical_details(scoped),
            canonical_details(default.javascript(codec).unwrap())
        );
    }
    assert_eq!(batch.report(), &default.report()["search"]);
    assert_eq!(finished.report["resources"], default.report()["resources"]);
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

#[test]
fn scoped_source_edit_keeps_one_owner_and_delivers_qualified_parent_and_child() {
    use crate::semantic_program::{CellBinding, CellId, Constant, OpId, OperationKind};

    let ((baseline, changed), finished) = with_checked_source(
        "export int answer(){return 17;}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
        |session| {
            let source = session.source();
            let (unit, operation, revision) = {
                let view = session.compilation().view(source).unwrap();
                let unit = (0..view.cell_count())
                    .find_map(|index| {
                        let cell = view.cell(CellId::from_index(index).unwrap()).unwrap();
                        match cell.binding {
                            CellBinding::Function(unit) if cell.name == "answer" => Some(unit),
                            _ => None,
                        }
                    })
                    .unwrap();
                let operation = view
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .position(|op| {
                        matches!(op.kind, OperationKind::Constant(Constant::Integer(17)))
                    })
                    .unwrap();
                (
                    unit,
                    OpId::from_index(operation).unwrap(),
                    view.unit_revision(unit).unwrap(),
                )
            };
            let candidate = {
                let (owner, _, policy) = session.parts_mut();
                owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let baseline = session.render_javascript(candidate).unwrap();
            let kind = OperationKind::Constant(Constant::Integer(19));
            let operations = [OperationPatch {
                operation,
                kind: &kind,
                operands: &[],
            }];
            let patches = [UnitPatch {
                unit,
                expected_revision: revision,
                operations: &operations,
                places: &[],
            }];
            let changed = session
                .compilation_mut()
                .edit_source(source, &patches, WorkDomain::Baseline)
                .unwrap();
            let changed = {
                let (owner, _, policy) = session.parts_mut();
                owner
                    .direct_javascript(changed, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let changed = session.render_javascript(changed).unwrap();
            let recheck = session.render_javascript(candidate).unwrap();
            assert_eq!(baseline.javascript(), recheck.javascript());
            assert_ne!(baseline.javascript(), changed.javascript());
            assert_eq!(
                session
                    .compilation()
                    .view(source)
                    .unwrap()
                    .unit_revision(unit),
                Some(revision)
            );
            (baseline, changed)
        },
    )
    .unwrap();
    for (artifact, expected) in [(baseline, "17\n"), (changed, "19\n")] {
        assert_eq!(
            execute_javascript(artifact.javascript(), "", "console.log(library.answer());"),
            expected
        );
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(
                artifact.sizes().get(codec).unwrap(),
                crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap()
            );
        }
        assert!(artifact.details()["policy_fingerprint"].is_array());
        assert!(artifact.details()["recipe_words"].is_array());
        assert_eq!(artifact.details()["resource"], "whole");
    }
    assert_eq!(finished.ledger.retained_bytes(), 0);
    assert!(
        finished.report["ledger_after_finish"]["edit_work"]
            .as_u64()
            .unwrap()
            > 0
    );
}

#[test]
fn scoped_direct_delivery_cannot_bypass_runtime_admission() {
    let ((), finished) = with_checked_source(
        "export int answer(){return 17;}",
        &config("[policy.constraints]\nmax_startup_work=0"),
        ServiceOptions::default(),
        |session| {
            let candidate = {
                let (owner, source, policy) = session.parts_mut();
                owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let error = session.render_javascript(candidate).unwrap_err();
            assert_eq!(error.phase, "javascript admission");
            assert!(error.message.contains("MissingCostEvidence"));
        },
    )
    .unwrap();
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

#[test]
fn scoped_package_delivery_refusal_releases_handoffs_without_discarding_inputs() {
    use crate::semantic_program::CellId;
    use crate::structured_js::selection::Style;

    let entry = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/fixed-javascript-resources/entry.lil");
    let ((), finished) = with_checked_path(
        &entry,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
        |session| {
            let source = session.source();
            let cell = {
                let view = session.compilation().view(source).unwrap();
                (0..view.cell_count())
                    .map(|index| CellId::from_index(index).unwrap())
                    .find(|&cell| view.cell(cell).unwrap().name == "score")
                    .unwrap()
            };
            let (whole, consumer) = {
                let (owner, _, policy) = session.parts_mut();
                let whole = owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap();
                let ProducerOutcome::Published(producer) = owner
                    .producer_javascript(
                        whole,
                        cell,
                        "./producer.mjs",
                        "scoreABI",
                        FunctionRequest {
                            max_work: 500_000,
                            scratch_bytes: 800_000,
                            output_bytes: 800_000,
                        },
                        policy,
                        WorkDomain::Baseline,
                    )
                    .unwrap()
                    .outcome
                else {
                    panic!("complete physical export");
                };
                let artifact = owner
                    .with_javascript_output(producer, policy, |output| {
                        let artifact = output.render(&Plan::new(Style::Global)).unwrap();
                        output.retain_artifact(artifact).unwrap()
                    })
                    .unwrap();
                let consumer = owner
                    .freeze_producer_javascript(whole, artifact, policy, WorkDomain::Baseline)
                    .unwrap();
                // Cache construction and arena slot capacity are intentional retained
                // storage. Warm both before isolating refusal-path artifact ownership.
                let retained = owner
                    .with_javascript_output(consumer, policy, |output| {
                        (0..64)
                            .map(|_| {
                                let artifact = output.render(&Plan::new(Style::Global)).unwrap();
                                output.retain_artifact(artifact).unwrap()
                            })
                            .collect::<Vec<_>>()
                    })
                    .unwrap();
                for artifact in retained {
                    owner.discard_artifact(artifact).unwrap();
                }
                (whole, consumer)
            };
            let before = session.compilation().ledger().retained_bytes();
            for _ in 0..2 {
                let error = session.render_javascript(consumer).unwrap_err();
                assert_eq!(error.phase, "handoff");
                assert!(error.message.contains("resource packages"));
                assert_eq!(session.compilation().ledger().retained_bytes(), before);
                assert!(session.compilation().view(consumer.semantic_id()).is_ok());
            }
            assert!(!session
                .render_javascript(whole)
                .unwrap()
                .javascript()
                .is_empty());
            {
                let (owner, _, policy) = session.parts_mut();
                let request = search_request(
                    ServiceOptions {
                        objectives: Some(Objectives::All),
                        ..ServiceOptions::default()
                    },
                    policy,
                    Objectives::All,
                );
                owner
                    .enable_local_facts(request.facts_cache, WorkDomain::Baseline)
                    .unwrap();
            }
            let before = session.compilation().ledger().retained_bytes();
            let error = session
                .search_javascript(consumer.semantic_id(), |_| {})
                .unwrap_err();
            assert_eq!(error.phase, "handoff");
            assert!(error.message.contains("resource packages"));
            assert_eq!(session.compilation().ledger().retained_bytes(), before);
            assert!(session.compilation().view(consumer.semantic_id()).is_ok());
        },
    )
    .unwrap();
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

struct ReleaseObserver;

impl ReleaseObserver {
    fn new() -> Self {
        RELEASE_EVENTS.with(|events| {
            assert!(events.borrow_mut().replace(Vec::new()).is_none());
        });
        Self
    }

    fn events(&self) -> Vec<&'static str> {
        RELEASE_EVENTS.with(|events| {
            events
                .borrow()
                .as_ref()
                .unwrap()
                .iter()
                .map(|event| event.0)
                .collect()
        })
    }

    fn retained(&self, event: &str) -> u64 {
        RELEASE_EVENTS.with(|events| {
            events
                .borrow()
                .as_ref()
                .unwrap()
                .iter()
                .find(|entry| entry.0 == event)
                .unwrap()
                .1
                .unwrap()
        })
    }
}

impl Drop for ReleaseObserver {
    fn drop(&mut self) {
        RELEASE_EVENTS.with(|events| {
            events.borrow_mut().take();
        });
    }
}

fn finalization_fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("src/semantic_program/fixtures/search-structural-valley/entry.lil")
}

#[test]
fn path_factory_finalizes_before_source_release_on_success_and_callback_error() {
    for fail in [false, true] {
        let releases = ReleaseObserver::new();
        let (outcome, finished) = with_checked_path(
            &finalization_fixture(),
            &config(""),
            ServiceOptions::default(),
            |session| {
                assert!(releases.events().is_empty());
                let (owner, source, policy) = session.parts_mut();
                owner
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap();
                assert!(owner.ledger().retained_bytes() > 0);
                record_release("client");
                if fail {
                    Err("client declined")
                } else {
                    Ok(17)
                }
            },
        )
        .unwrap();
        assert_eq!(outcome, if fail { Err("client declined") } else { Ok(17) });
        assert_eq!(
            releases.events(),
            [
                "client",
                "compilation",
                "source",
                "source-charge",
                "finished"
            ]
        );
        let capacity = finished.report["resources"]["source_buffer_capacity"]
            .as_u64()
            .unwrap();
        assert!(capacity > 0);
        assert_eq!(releases.retained("compilation"), capacity);
        assert_eq!(releases.retained("source"), capacity);
        assert_eq!(releases.retained("source-charge"), 0);
        assert_eq!(releases.retained("finished"), 0);
        assert!(finished.report["phases_ns"]["source_release_ns"].is_u64());
        assert_eq!(finished.ledger.retained_bytes(), 0);
        assert_eq!(finished.report["ledger_after_finish"]["retained_bytes"], 0);
    }
}

#[test]
fn path_factory_finalizes_and_releases_source_before_resuming_same_panic() {
    let releases = ReleaseObserver::new();
    let payload = std::sync::Arc::new(String::from("original client panic"));
    let outcome = std::panic::catch_unwind({
        let payload = payload.clone();
        move || {
            let _ = with_checked_path(
                &finalization_fixture(),
                &config(""),
                ServiceOptions::default(),
                |session| -> () {
                    let (owner, source, policy) = session.parts_mut();
                    owner
                        .direct_javascript(source, policy, WorkDomain::Baseline)
                        .unwrap();
                    record_release("client");
                    std::panic::panic_any(payload);
                },
            );
        }
    });
    let resumed = outcome
        .unwrap_err()
        .downcast::<std::sync::Arc<String>>()
        .unwrap();
    assert!(std::sync::Arc::ptr_eq(&payload, &resumed));
    assert_eq!(
        releases.events(),
        [
            "client",
            "compilation",
            "source",
            "source-charge",
            "finished"
        ]
    );
    assert!(releases.retained("compilation") > 0);
    assert_eq!(
        releases.retained("compilation"),
        releases.retained("source")
    );
    assert_eq!(releases.retained("source-charge"), 0);
    assert_eq!(releases.retained("finished"), 0);
}

#[test]
fn source_factory_finalizes_without_claiming_caller_source_release() {
    let source = String::from("export int answer(){return 17;}");
    let releases = ReleaseObserver::new();
    let (length, finished) =
        with_checked_source(&source, &config(""), ServiceOptions::default(), |session| {
            assert!(session.compilation().view(session.source()).is_ok());
            source.len()
        })
        .unwrap();
    assert_eq!(length, source.len());
    assert_eq!(source, "export int answer(){return 17;}");
    assert_eq!(releases.events(), ["compilation", "finished"]);
    assert!(finished.report["phases_ns"]["source_release_ns"].is_null());
    assert!(finished.report["resources"]["source_buffer_capacity"].is_null());
    assert_eq!(
        finished.report["resources"]["source_buffer_accounting"],
        "caller-owned text; excluded"
    );
    assert_eq!(finished.ledger.retained_bytes(), 0);
}

#[test]
fn default_service_preserves_target_metadata_after_factory_finalization() {
    let result = compile_source_semantic(
        "print(42);",
        &config(""),
        ServiceOptions {
            target: ServiceTarget::All,
            preserve_root_exports: false,
            objectives: Some(Objectives::One(Objective::Raw)),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let report = result.report();
    let javascript = result.javascript(Objective::Raw).unwrap();
    let native = result.native_c().unwrap();
    assert_eq!(report["artifacts"][0]["sha256"], javascript.sha256());
    assert_eq!(report["artifacts"][0]["details"], *javascript.details());
    assert_eq!(report["winners"], json!([0, null, null]));
    assert_eq!(report["native_sha256"], digest(native.as_bytes()));
    assert_eq!(report["native_delivery"]["c_bytes"], native.len());
    assert_eq!(
        report["native_delivery"]["semantic"],
        javascript.details()["semantic"]
    );
    assert!(report["first_artifact_ns"].is_u64());
    assert!(report["search"].is_object());
    assert_eq!(report["backend"], "semantic");
    assert_eq!(report["ledger_after_finish"]["retained_bytes"], 0);
    assert!(report["phases_ns"]["source_release_ns"].is_null());
}

#[test]
fn delivered_rewrite_metadata_survives_finalization_and_marks_inherited_history() {
    use crate::semantic_program::{CellBinding, CellId, Constant, OpId, OperationKind};

    let ((folded_output, changed_output, before, meaning), finished) = with_checked_source(
        "export int answer(){return 3+4;}",
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::One(Objective::Raw)),
            ..ServiceOptions::default()
        },
        |session| {
            let (unit, operation, before, meaning) = {
                let view = session.compilation().view(session.source()).unwrap();
                let unit = (0..view.cell_count())
                    .find_map(|index| {
                        let cell = view.cell(CellId::from_index(index).unwrap()).unwrap();
                        match cell.binding {
                            CellBinding::Function(unit) if cell.name == "answer" => Some(unit),
                            _ => None,
                        }
                    })
                    .unwrap();
                let operation = view
                    .unit(unit)
                    .unwrap()
                    .operations
                    .iter()
                    .position(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
                    .unwrap();
                (
                    unit,
                    OpId::from_index(operation).unwrap(),
                    format!("{:?}", view.snapshot_identity()),
                    format!("{:?}", view.meaning_identity()),
                )
            };
            let (folded, candidate) = {
                let (owner, source, policy) = session.parts_mut();
                let folded = owner
                    .fold_literal_int_binary(source, unit, operation, policy, WorkDomain::Baseline)
                    .unwrap()
                    .unwrap();
                let candidate = owner
                    .direct_javascript(folded, policy, WorkDomain::Baseline)
                    .unwrap();
                (folded, candidate)
            };
            let folded_output = session.render_javascript(candidate).unwrap();
            let revision = session
                .compilation()
                .view(folded)
                .unwrap()
                .unit_revision(unit)
                .unwrap();
            let replacement = OperationKind::Constant(Constant::Integer(9));
            let operations = [OperationPatch {
                operation,
                kind: &replacement,
                operands: &[],
            }];
            let patches = [UnitPatch {
                unit,
                expected_revision: revision,
                operations: &operations,
                places: &[],
            }];
            let changed = session
                .compilation_mut()
                .edit_source(folded, &patches, WorkDomain::Baseline)
                .unwrap();
            let candidate = {
                let (owner, _, policy) = session.parts_mut();
                owner
                    .direct_javascript(changed, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            let changed_output = session.render_javascript(candidate).unwrap();
            (folded_output, changed_output, before, meaning)
        },
    )
    .unwrap();
    assert_eq!(finished.ledger.retained_bytes(), 0);
    let folded = &folded_output.details()["semantic"];
    let changed = &changed_output.details()["semantic"];
    assert_eq!(folded["meaning_identity"], meaning);
    assert_ne!(folded["snapshot_identity"], before);
    assert_eq!(folded["rewrite_order"], "newest-first");
    assert_eq!(folded["rewrites"].as_array().unwrap().len(), 1);
    let step = &folded["rewrites"][0];
    assert_eq!(step["meaning_identity"], meaning);
    assert_eq!(step["before_snapshot"], before);
    assert_eq!(step["after_snapshot"], folded["snapshot_identity"]);
    assert_eq!(step["inherited"], false);
    let fold = &step["fold"];
    assert_eq!(fold["rule"], "literal-int-binary");
    assert_eq!(fold["version"], LITERAL_INT_FOLD_VERSION);
    assert_eq!(fold["operator"], "Add");
    assert_eq!(fold["left"], 3);
    assert_eq!(fold["right"], 4);
    assert_eq!(fold["result"], 7);
    for field in ["unit", "operation", "left_producer", "right_producer"] {
        assert!(fold[field].is_u64());
    }
    assert_ne!(changed["meaning_identity"], meaning);
    assert_ne!(changed["snapshot_identity"], folded["snapshot_identity"]);
    assert_eq!(changed["rewrites"][0]["inherited"], true);
    assert_eq!(changed["rewrites"][0]["fold"], *fold);
    assert_eq!(changed["rewrites"][0]["meaning_identity"], meaning);
    for (artifact, expected) in [(folded_output, "7\n"), (changed_output, "9\n")] {
        assert_eq!(
            execute_javascript(artifact.javascript(), "", "console.log(library.answer());"),
            expected
        );
    }
}

#[test]
fn path_checker_refusal_releases_source_buffers_but_preserves_owned_diagnostic() {
    let scratch = Scratch::new();
    let path = scratch.0.join("bad.lil");
    let source = "export int answer(){return unknown_name;}";
    std::fs::write(&path, source).unwrap();
    let mut frontend = Frontend::new(&config(""), ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    sources.store(source, &mut frontend.ledger).unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    sources.discard(&mut frontend.ledger).unwrap();
    let releases = ReleaseObserver::new();
    let error = compile_path_semantic(&path, &config(""), ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase, "check");
    assert_eq!(error.diagnostic.unwrap().source, source);
    assert_eq!(releases.events(), ["source", "source-charge"]);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn path_source_memory_refusal_is_typed_without_diagnostic_source_copy() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(&path, "export int answer(){return 17;}").unwrap();
    let error = compile_path_semantic(
        &path,
        &config(""),
        ServiceOptions {
            retained_bytes: 1,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "discovery resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
}

#[test]
fn source_parser_memory_refusal_remains_typed_at_the_public_boundary() {
    let error = compile_source_semantic(
        "export int answer(){return 17;}",
        &config(""),
        ServiceOptions {
            retained_bytes: 1,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "parse resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
    let cause = std::error::Error::source(&error).unwrap();
    assert_eq!(
        cause.downcast_ref::<ServiceResourceError>(),
        error.resource.as_ref()
    );
}

#[test]
fn source_checker_and_conversion_work_refusals_remain_typed_at_the_public_boundary() {
    let source = "export int answer(){return 17;}";
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let syntax = arena.parse(source).unwrap();
    arena
        .with_ledger(|ledger, domain| {
            ledger.charge(domain, WorkKind::Analysis, source.len() as u64)
        })
        .unwrap();
    let checker_work = arena.with_ledger(|ledger, domain| ledger.work_used(domain));
    let conversion_work = arena.with_ledger(|ledger, domain| {
        with_analyzed_source(
            &syntax,
            &mut AllocationBudget::new(Some((ledger, domain))),
            |_, budget| {
                budget
                    .work(WorkKind::Analysis, source.len() as u64)
                    .unwrap();
                budget.with_ledger(|owner| {
                    let (ledger, domain) = owner.unwrap();
                    ledger.work_used(domain)
                })
            },
        )
        .unwrap()
    });
    drop(syntax);
    drop(arena);
    for (work, phase) in [
        (checker_work, "check resources"),
        (conversion_work, "conversion resources"),
    ] {
        let error = compile_source_semantic(
            source,
            &config,
            ServiceOptions {
                logical_work: work,
                ..ServiceOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, phase);
        assert_eq!(
            error.resource,
            Some(ServiceResourceError::Budget(
                crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
            ))
        );
        assert!(error.diagnostic.is_none());
    }
}

#[test]
fn deadline_at_adoption_discards_prepared_graph_before_releasing_its_charge() {
    let source = "export int answer(){return 17;}";
    let config = config("[policy.resources]\nwall_time_ms=60000");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    frontend
        .ledger
        .set_deadline_elapsed_for_test(std::time::Duration::ZERO);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let syntax = arena.parse(source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let prepared = arena
        .with_ledger(|ledger, domain| {
            from_checked_source_admitted(
                &syntax,
                &semantics,
                &mut AllocationBudget::new(Some((ledger, domain))),
            )
        })
        .unwrap();
    drop(semantics);
    drop(syntax);
    drop(arena);
    assert!(frontend.ledger.retained_bytes() > 0);
    frontend
        .ledger
        .set_deadline_elapsed_for_test(std::time::Duration::from_millis(60000));

    let releases = ReleaseObserver::new();
    let error = match frontend.adopt(prepared, json!({"root":0})) {
        Ok(_) => panic!("expired handoff must not adopt a graph"),
        Err((error, ledger)) => {
            assert_eq!(ledger.retained_bytes(), 0);
            error
        }
    };
    assert_eq!(error.phase, "frontend resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::DeadlineExceeded
        ))
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(releases.events(), ["prepared-discard"]);
    assert_eq!(releases.retained("prepared-discard"), 0);
}

#[test]
fn store_construction_refusal_returns_the_owner_for_prepared_graph_cleanup() {
    for memory_refusal in [false, true] {
        let source = "export int answer(){return 17;}";
        let options = ServiceOptions {
            logical_work: 1_000_000,
            retained_bytes: 1_000_000,
            ..ServiceOptions::default()
        };
        let mut frontend = Frontend::new(&config(""), options).unwrap();
        let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
        let syntax = arena.parse(source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let prepared = arena
            .with_ledger(|ledger, domain| {
                from_checked_source_admitted(
                    &syntax,
                    &semantics,
                    &mut AllocationBudget::new(Some((ledger, domain))),
                )
            })
            .unwrap();
        drop(semantics);
        drop(syntax);
        drop(arena);
        let prepared_bytes = frontend.ledger.retained_bytes();
        assert!(prepared_bytes > 0);
        let remaining_charge = if memory_refusal {
            let remaining = options.retained_bytes - prepared_bytes;
            frontend
                .ledger
                .retain(WorkDomain::Baseline, remaining)
                .unwrap();
            remaining
        } else {
            let remaining = options.logical_work - frontend.ledger.work_used(WorkDomain::Baseline);
            frontend
                .ledger
                .charge(WorkDomain::Baseline, WorkKind::Analysis, remaining)
                .unwrap();
            0
        };

        let releases = ReleaseObserver::new();
        let error = match frontend.adopt(prepared, json!({"root":0})) {
            Ok(_) => panic!("exhausted store construction must refuse"),
            Err((error, mut ledger)) => {
                assert_eq!(ledger.retained_bytes(), remaining_charge);
                ledger
                    .release(WorkDomain::Baseline, remaining_charge)
                    .unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
                error
            }
        };
        let expected = if memory_refusal {
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        } else {
            crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
        };
        assert_eq!(error.phase, "adoption");
        assert_eq!(
            error.message,
            format!("{:?}", PublicationError::Budget(expected))
        );
        assert_eq!(releases.events(), ["prepared-discard"]);
        assert_eq!(releases.retained("prepared-discard"), remaining_charge);
    }
}

#[test]
fn path_post_discovery_work_refusal_releases_sources_before_returning() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    let source = "export int answer(){return 17;}";
    std::fs::write(&path, source).unwrap();
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let (modules, syntax) =
        discover_parsed_modules_admitted(&path, &config, &sources, &arena).unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    drop(syntax);
    drop(arena);
    let work = frontend.ledger.work_used(WorkDomain::Baseline);
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    let releases = ReleaseObserver::new();
    let error = compile_path_semantic(
        &path,
        &config,
        ServiceOptions {
            logical_work: work,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "frontend resources");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(releases.events(), ["source", "source-charge"]);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn path_checker_and_conversion_refusals_release_sources_without_a_diagnostic_copy() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(&path, "export int answer(){return 17;}").unwrap();
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let (modules, syntax) =
        discover_parsed_modules_admitted(&path, &config, &sources, &arena).unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    let bytes = modules
        .modules
        .iter()
        .map(|module| module.source.len() as u64)
        .sum();
    arena
        .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
        .unwrap();
    arena
        .with_ledger(|ledger, domain| ledger.charge(domain, WorkKind::Analysis, bytes))
        .unwrap();
    let checker_work = arena.with_ledger(|ledger, domain| ledger.work_used(domain));
    let conversion_work = arena.with_ledger(|ledger, domain| {
        with_analyzed_modules(
            &syntax,
            &modules,
            &mut AllocationBudget::new(Some((ledger, domain))),
            |_, budget| {
                budget.work(WorkKind::Analysis, bytes).unwrap();
                budget.with_ledger(|owner| {
                    let (ledger, domain) = owner.unwrap();
                    ledger.work_used(domain)
                })
            },
        )
        .unwrap()
    });
    drop(syntax);
    drop(arena);
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    assert_eq!(frontend.ledger.retained_bytes(), 0);

    for (work, phase) in [
        (checker_work, "check resources"),
        (conversion_work, "conversion resources"),
    ] {
        let releases = ReleaseObserver::new();
        let error = compile_path_semantic(
            &path,
            &config,
            ServiceOptions {
                logical_work: work,
                ..ServiceOptions::default()
            },
        )
        .unwrap_err();
        assert_eq!(error.phase, phase);
        assert_eq!(
            error.resource,
            Some(ServiceResourceError::Budget(
                crate::compilation_policy::BudgetError::WorkExhausted(WorkDomain::Baseline)
            ))
        );
        assert!(error.diagnostic.is_none());
        assert_eq!(releases.events(), ["source", "source-charge"]);
        assert_eq!(releases.retained("source"), source_bytes);
        assert_eq!(releases.retained("source-charge"), 0);
    }
}

#[test]
fn path_store_refusal_releases_prepared_graph_then_factory_source_buffers() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(&path, "export int answer(){return 17;}").unwrap();
    let config = config("");
    let (_, completed) =
        with_checked_path(&path, &config, ServiceOptions::default(), |_| ()).unwrap();
    let resources = &completed.report["resources"];
    let work = resources["frontend_logical_work"].as_u64().unwrap();
    let source_bytes = resources["source_buffer_capacity"].as_u64().unwrap();
    assert_eq!(completed.ledger.retained_bytes(), 0);

    let releases = ReleaseObserver::new();
    let error = compile_path_semantic(
        &path,
        &config,
        ServiceOptions {
            logical_work: work,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert_eq!(error.phase, "adoption");
    assert_eq!(
        error.message,
        format!(
            "{:?}",
            PublicationError::Budget(crate::compilation_policy::BudgetError::WorkExhausted(
                WorkDomain::Baseline
            ))
        )
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(
        releases.events(),
        ["prepared-discard", "source", "source-charge"]
    );
    assert_eq!(releases.retained("prepared-discard"), source_bytes);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn semantic_service_rejects_legacy_ast_specialization_before_io_or_work() {
    let config = config("[optimization]\nfor_of_specialize_family=4");
    let source = "export int answer(){return sum$pick([1,2],0);}";
    let scratch = Scratch::new();
    let missing = scratch.0.join("missing.lil");
    for target in [
        ServiceTarget::JavaScript,
        ServiceTarget::Native,
        ServiceTarget::All,
    ] {
        let options = ServiceOptions {
            target,
            logical_work: 0,
            retained_bytes: 0,
            ..ServiceOptions::default()
        };
        for error in [
            compile_source_semantic(source, &config, options).unwrap_err(),
            compile_path_semantic(&missing, &config, options).unwrap_err(),
        ] {
            assert_eq!(error.phase, "configuration");
            assert!(error.message.contains("for_of_specialize_family"));
            assert!(error.message.contains("set it to 0"));
            assert!(error.resource.is_none());
            assert!(error.diagnostic.is_none());
        }
    }
}

#[test]
fn explicit_zero_specialization_matches_omitted_source_and_module_compilation() {
    let source = "export int answer(){return 17;}";
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(
        &path,
        "import {answer} from \"./answer.lil\";export {answer};",
    )
    .unwrap();
    std::fs::write(scratch.0.join("answer.lil"), source).unwrap();
    let options = ServiceOptions {
        objectives: Some(Objectives::One(Objective::Raw)),
        ..ServiceOptions::default()
    };
    let mut outputs = Vec::new();
    for extra in ["", "[optimization]\nfor_of_specialize_family=0"] {
        let mut config = config(extra);
        config.javascript.candidate_search = crate::config::CandidateSearch::Off;
        let source = compile_source_semantic(source, &config, options).unwrap();
        let modules = compile_path_semantic(&path, &config, options).unwrap();
        check_scores(&source);
        check_scores(&modules);
        assert_eq!(modules.report()["shape"]["modules"], 2);
        outputs.push((
            source
                .javascript(Objective::Raw)
                .unwrap()
                .javascript()
                .to_owned(),
            modules
                .javascript(Objective::Raw)
                .unwrap()
                .javascript()
                .to_owned(),
        ));
    }
    assert_eq!(outputs[0], outputs[1]);
    for javascript in [&outputs[1].0, &outputs[1].1] {
        assert_eq!(
            execute_javascript(javascript, "", "console.log(library.answer());"),
            "17\n"
        );
    }
}

#[test]
fn path_factory_parses_each_canonical_module_once_and_releases_syntax_before_backend() {
    use crate::parser::admitted_arena_activity_for_test;
    use crate::semantic_program::CellId;

    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    std::fs::write(
        &path,
        "import {twice} from \"./math.lil\";export int answer(){return twice(21);}",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("math.lil"),
        "export int twice(int value){return value*2;}",
    )
    .unwrap();
    let (live_before, parses_before) = admitted_arena_activity_for_test();
    let (result, finished) =
        with_checked_path(&path, &config(""), ServiceOptions::default(), |session| {
            let (live, parses) = admitted_arena_activity_for_test();
            assert_eq!(
                live, live_before,
                "actual syntax arena must already be dropped"
            );
            assert_eq!(parses - parses_before, 2);
            assert_eq!(session.shape["modules"], 2);
            let source = session.source();
            {
                let view = session.compilation().view(source).unwrap();
                for expected in ["answer", "twice"] {
                    assert!((0..view.cell_count()).any(|index| {
                        view.cell(CellId::from_index(index).unwrap()).unwrap().name == expected
                    }));
                }
            }
            session.compile_targets()
        })
        .unwrap();
    let output = finish_output(result, finished).unwrap();
    check_scores(&output);
    assert_eq!(
        admitted_arena_activity_for_test(),
        (live_before, parses_before + 2)
    );
    assert_eq!(
        execute_javascript(
            output.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            "console.log(library.answer());",
        ),
        "42\n"
    );
}

#[test]
fn shared_module_discovery_refusal_releases_partial_sources_on_the_same_ledger() {
    let scratch = Scratch::new();
    let path = scratch.0.join("entry.lil");
    let mut entry = String::new();
    for module in 0..8 {
        entry.push_str(&format!(
            "import {{value{module}}} from \"./part{module}.lil\";"
        ));
        let mut source = format!("export int value{module}(){{");
        for local in 0..64 {
            source.push_str(&format!("int local{local}={local};"));
        }
        source.push_str("return local63;}");
        std::fs::write(scratch.0.join(format!("part{module}.lil")), source).unwrap();
    }
    entry.push_str("export int answer(){return value0();}");
    std::fs::write(&path, entry).unwrap();
    let config = config("");
    let mut frontend = Frontend::new(&config, ServiceOptions::default()).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let (modules, syntax) =
        discover_parsed_modules_admitted(&path, &config, &sources, &arena).unwrap();
    drop(syntax);
    drop(arena);
    let discovery_peak = frontend.ledger.peak_retained_bytes();
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    assert_eq!(frontend.ledger.retained_bytes(), 0);

    let options = ServiceOptions {
        retained_bytes: discovery_peak - 1,
        ..ServiceOptions::default()
    };
    let mut frontend = Frontend::new(&config, options).unwrap();
    let sources = StableSourceArena::new(WorkDomain::Baseline);
    let arena = AdmittedArena::new(&mut frontend.ledger, WorkDomain::Baseline);
    let error = discover_parsed_modules_admitted(&path, &config, &sources, &arena).unwrap_err();
    assert!(matches!(error, ModuleDiscoveryError::Resources(_)));
    drop(arena);
    let source_bytes = sources.allocated_bytes() as u64;
    assert!(
        source_bytes > 0,
        "refusal must retain some discovered source storage"
    );
    assert_eq!(frontend.ledger.retained_bytes(), source_bytes);
    sources.discard(&mut frontend.ledger).unwrap();
    assert_eq!(frontend.ledger.retained_bytes(), 0);

    let releases = ReleaseObserver::new();
    let error = compile_path_semantic(&path, &config, options).unwrap_err();
    assert_eq!(error.phase, "discovery resources", "{error:?}");
    assert_eq!(
        error.resource,
        Some(ServiceResourceError::Budget(
            crate::compilation_policy::BudgetError::MemoryExhausted(WorkDomain::Baseline)
        ))
    );
    assert!(error.diagnostic.is_none());
    assert_eq!(releases.events(), ["source", "source-charge"]);
    assert_eq!(releases.retained("source"), source_bytes);
    assert_eq!(releases.retained("source-charge"), 0);
}

#[test]
fn exported_defaults_keep_their_javascript_length_and_values() {
    // JavaScript's `length` stops at the first default; an omitted or
    // `undefined` argument takes the default, as with default syntax.
    let result = compile_source_semantic(
        "export int read(int value, int scale = 2, JsValue extra = JS.undefined()){return value * scale;}",
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(
        execute_javascript(
            result.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            "console.log(JSON.stringify([library.read.length,library.read(3),library.read(3,4),library.read(3,undefined)]));"
        ),
        "[1,6,12,6]\n"
    );
}

#[test]
fn foreign_imports_become_es_imports_of_their_extern_values() {
    // `import extern` binds a module's extern value declarations to a
    // JavaScript module's exports; the output imports them, spelled from
    // the root module's directory as the legacy linker spells them.
    let scratch = Scratch::new();
    std::fs::create_dir(scratch.0.join("lib")).unwrap();
    std::fs::write(
        scratch.0.join("host.mjs"),
        "export const greeting='hi';export function twice(x){return x*2}export default {answer:42};",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("lib/greet.lil"),
        "import extern { greeting, twice, default as config } from \"../host.mjs\";\n\
         extern string greeting;\nextern JsValue config;\nextern int twice(int value);\n\
         export string greet() { return greeting + JS.string(JS.get(config, \"answer\")) + twice(1); }\n",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("main.lil"),
        "import { greet } from \"./lib/greet.lil\";\n\
         import extern { twice } from \"./host.mjs\";\nextern int twice(int value);\n\
         export string run() { return greet() + \":\" + twice(21); }\n",
    )
    .unwrap();
    let result = compile_path_semantic(
        &scratch.0.join("main.lil"),
        &config(""),
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let javascript = result.javascript(Objective::Brotli).unwrap().javascript();
    assert!(javascript.contains("from\"./host.mjs\""), "{javascript}");
    // Both modules import `twice`: its pinned local name is declared once.
    assert_eq!(javascript.matches("import{twice}").count(), 1, "{javascript}");
    std::fs::write(scratch.0.join("out.mjs"), javascript).unwrap();
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!(
                "const m=await import({});console.log(m.run());",
                serde_json::to_string(&format!("file://{}", scratch.0.join("out.mjs").display()))
                    .unwrap()
            ),
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "hi422:42\n");
}
