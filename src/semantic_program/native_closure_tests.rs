//! Callback ABI v1 qualification through real public output owners. This is a
//! child of native_tests so compiler discovery, bounded execution and temporary
//! directory cleanup remain owned by the existing native harness.
use super::*;
use crate::compiler_service::{
    with_checked_path, with_checked_source, CheckedSourceSession, ServiceOptions, ServiceTarget,
};
use crate::structured_js::selection::Objective;

const HOST_C: &str = include_str!("fixtures/native-closures/host.c");
const HOST_JS: &str = include_str!("fixtures/native-closures/host.js");
const HOSTS: [(&str, &str); 7] = [
    ("keep", "host_keep"),
    ("invoke", "host_invoke"),
    ("clear", "host_clear"),
    ("clearAll", "host_clear_all"),
    ("take", "host_take"),
    ("expectLive", "host_expect_live"),
    ("assertEmpty", "host_assert_empty"),
];

fn with_hosts<R>(
    source_text: &str,
    optional: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, &NativeHostBindings<'_>) -> R,
) -> R {
    with_host_limits(source_text, optional, MEMORY, inspect)
}

fn with_host_limits<R>(
    source_text: &str,
    optional: u64,
    optional_memory: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, &NativeHostBindings<'_>) -> R,
) -> R {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source_text).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    with_host_program(program, optional, optional_memory, inspect)
}

fn with_host_program<R>(
    program: Program<'_>,
    optional: u64,
    optional_memory: u64,
    inspect: impl FnOnce(&mut Compilation<'_>, SemanticId, &NativeHostBindings<'_>) -> R,
) -> R {
    program.verify().unwrap();
    // Canonical IDs belong to this exact checked program. Detached extern
    // formal symbols do not become host bindings or native storage here.
    let mut bindings = Vec::new();
    for (name, link_name) in HOSTS {
        let mut found = program
            .cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.binding == CellBinding::Foreign && cell.name == name);
        let (index, _) = found.next().unwrap();
        assert!(found.next().is_none(), "ambiguous checked foreign {name}");
        bindings.push(NativeHostBinding {
            cell: CellId::from_index(index).unwrap(),
            link_name,
        });
    }
    bindings.sort_by_key(|binding| binding.cell);
    let hosts = NativeHostBindings {
        callback_abi_version: 1,
        bindings: &bindings,
    };
    let mut compilation = Compilation::new(
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: WORK,
                optional_work: optional,
                baseline_retained_bytes: MEMORY - optional_memory,
                retained_bytes: MEMORY,
            },
        )
        .unwrap(),
        CheckpointLimit { max_live: 3 },
    )
    .unwrap();
    let source = compilation
        .adopt_checked(program, WorkDomain::Baseline)
        .unwrap();
    let result = inspect(&mut compilation, source, &hosts);
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

fn emit_pair(
    compilation: &mut Compilation<'_>,
    source: SemanticId,
    hosts: &NativeHostBindings<'_>,
) -> (String, String) {
    let before = compilation.ledger().retained_bytes();
    let checkpoints = compilation.checkpoint_count();
    let units = revisions(compilation, source);
    let codecs = compilation.ledger().work_by_kind(WorkKind::Codec);
    let artifacts = compilation
        .with_native_c_and_hosts(
            source,
            &native_policy(),
            WorkDomain::Baseline,
            hosts,
            |output| {
                assert!(!output.header().is_empty());
                let c_pointer = output.as_str().as_ptr();
                let header_pointer = output.header().as_ptr();
                let c = output.take_c();
                let header = output.take_header();
                assert_eq!(c_pointer, c.as_ptr());
                assert_eq!(header_pointer, header.as_ptr());
                assert!(output.as_str().is_empty());
                assert!(output.header().is_empty());
                (c, header)
            },
        )
        .unwrap();
    assert_eq!(compilation.ledger().retained_bytes(), before);
    assert_eq!(compilation.ledger().work_by_kind(WorkKind::Codec), codecs);
    assert_eq!(compilation.checkpoint_count(), checkpoints);
    assert_eq!(revisions(compilation, source), units);
    artifacts
}

fn command_record(command: &mut Command, phase: &str) -> serde_json::Value {
    let invocation = format!("{command:?}");
    let started = Instant::now();
    let result = execute(command);
    let elapsed = started.elapsed().as_secs_f64();
    assert!(
        result.status.success(),
        "{phase}: {invocation}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::json!({
        "phase":phase,"command":invocation,"status":result.status.code(),
        "stdout":String::from_utf8_lossy(&result.stdout),
        "stderr":String::from_utf8_lossy(&result.stderr),"elapsed_seconds":elapsed,
    })
}

fn execute_host_artifacts(
    c: &str,
    header: &str,
    expected: &str,
    name: &str,
) -> Vec<serde_json::Value> {
    let directory = ScratchDirectory::new(name);
    let program = directory.0.join("program.c");
    let interface = directory.0.join("program.h");
    let host = directory.0.join("host.c");
    std::fs::write(&program, c).unwrap();
    std::fs::write(&interface, header).unwrap();
    std::fs::write(&host, HOST_C).unwrap();
    let mut executions = Vec::new();
    for compiler in compilers() {
        for (profile, optimization, instrumentation) in [
            ("O0", "-O0", &[][..]),
            ("O2", "-O2", &[][..]),
            (
                "ubsan",
                "-O2",
                &["-fsanitize=undefined", "-fno-sanitize-recover=all"][..],
            ),
            (
                "asan-lsan",
                "-O1",
                &["-fsanitize=address,leak", "-fno-omit-frame-pointer"][..],
            ),
        ] {
            let stem = format!("{}-{profile}", compiler.family);
            let program_object = directory.0.join(format!("{stem}-program.o"));
            let host_object = directory.0.join(format!("{stem}-host.o"));
            let executable = directory.0.join(&stem);
            let base = [
                "-std=c11",
                "-fno-fast-math",
                "-ffp-contract=off",
                "-DLS_NATIVE_QUALIFICATION",
                optimization,
            ];
            let mut commands = Vec::new();
            // Separate compilation is an interface test: host.c knows only the
            // generated program.h, not the implementation translation unit.
            for (phase, input, object) in [
                ("compile-program", &program, &program_object),
                ("compile-host", &host, &host_object),
            ] {
                let mut command = Command::new(&compiler.executable);
                command
                    .args(base)
                    .args(instrumentation)
                    .arg("-c")
                    .arg(input)
                    .arg("-o")
                    .arg(object);
                commands.push(command_record(&mut command, phase));
            }
            let mut link = Command::new(&compiler.executable);
            link.args(base)
                .args(instrumentation)
                .arg(&program_object)
                .arg(&host_object)
                .arg("-lm")
                .arg("-o")
                .arg(&executable);
            commands.push(command_record(&mut link, "link"));
            let bytes = std::fs::read(&executable).unwrap();
            let binary_hash = digest(&bytes);
            let binary_bytes = bytes.len();
            drop(bytes);
            let mut run = Command::new(&executable);
            run.env("UBSAN_OPTIONS", "halt_on_error=1")
                .env("ASAN_OPTIONS", "detect_leaks=1:halt_on_error=1")
                .env("LSAN_OPTIONS", "exitcode=23");
            let invocation = format!("{run:?}");
            let started = Instant::now();
            let result = execute(&mut run);
            let elapsed = started.elapsed().as_secs_f64();
            assert!(
                result.status.success(),
                "{name}/{stem}: {invocation}\n{}\n{c}\n{header}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert!(
                result.stderr.is_empty(),
                "{name}/{stem}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8_lossy(&result.stdout),
                expected,
                "{name}/{stem}"
            );
            executions.push(serde_json::json!({
                "compiler":compiler.family,"executable":compiler.executable,"version":compiler.version,
                "profile":profile,"commands":commands,"run_command":invocation,
                "run_status":result.status.code(),"stdout":String::from_utf8_lossy(&result.stdout),
                "stderr":String::from_utf8_lossy(&result.stderr),"run_elapsed_seconds":elapsed,
                "native_executable_sha256":binary_hash,"native_executable_bytes":binary_bytes,
                "memory_check":"actual runtime live objects at authored scope boundaries and process exit; ASan+LSan profile enabled",
                "timing_scope":"subprocess wall time including existing spawn/wait polling; RSS unmeasured",
            }));
        }
    }
    assert_eq!(executions.len(), 8);
    executions
}

fn qualify(name: &str, source_text: &str, expected: &str) {
    let row = with_hosts(source_text, WORK, |compilation, source, hosts| {
        fixture_artifacts(name, source_text, expected, compilation, source, hosts)
    });
    eprintln!("native-closure-artifact {}", row);
}

fn fixture_artifacts(
    name: &str,
    source_text: &str,
    expected: &str,
    compilation: &mut Compilation<'_>,
    source: SemanticId,
    hosts: &NativeHostBindings<'_>,
) -> serde_json::Value {
    let before = compilation.ledger().retained_bytes();
    let render_before = compilation.ledger().work_by_kind(WorkKind::Render);
    let (c, header) = emit_pair(compilation, source, hosts);
    let native_work = compilation.ledger().work_by_kind(WorkKind::Render) - render_before;
    assert!(header.contains("#define LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION 1"));
    assert!(header.contains("host_keep_arg1_retain"));
    assert!(header.contains("host_keep_arg1_release"));
    assert!(header.contains("host_keep_arg1_call"));
    assert!(header.contains("host_take_result"));
    let executions = execute_host_artifacts(&c, &header, expected, name);
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    let policy = config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap();
    let candidate = compilation
        .direct_javascript(source, &policy, WorkDomain::Baseline)
        .unwrap();
    let javascript = compilation
            .with_javascript_output(candidate, &policy, |output| {
                let mut rows = Vec::new();
                for style in [Style::Global, Style::Scoped, Style::Source] {
                    let artifact = output.render(&Plan::new(style))?;
                    let actual = output.with_artifact(artifact, |view| {
                        let script = format!("{HOST_JS}\n{}", view.javascript);
                        let ran = execute(Command::new("node").args([
                            "--input-type=module",
                            "-e",
                            &script,
                        ]));
                        assert!(
                            ran.status.success(),
                            "{name}/{style:?}: {}\n{}",
                            String::from_utf8_lossy(&ran.stderr),
                            view.javascript
                        );
                        assert!(ran.stderr.is_empty());
                        assert_eq!(
                            String::from_utf8_lossy(&ran.stdout),
                            expected,
                            "{name}/{style:?}"
                        );
                        (view.output, String::from_utf8(ran.stdout).unwrap())
                    })?;
                    let raw = output.measure(artifact, Objective::Raw)?;
                    let gzip9 = output.measure(artifact, Objective::Gzip)?;
                    let brotli11 = output.measure(artifact, Objective::Brotli)?;
                    let text = output.take_artifact(artifact)?;
                    rows.push(serde_json::json!({
                        "style":format!("{style:?}"),"output":{"dead_code_elimination":actual.0.dead_code_elimination,"target_compaction":actual.0.target_compaction,"literals":format!("{:?}",actual.0.literals)},
                        "javascript":text,"javascript_sha256":digest(&text),
                        "raw":raw,"gzip9":gzip9,"brotli11":brotli11,"stdout":actual.1,
                    }));
                }
                Ok::<_, CandidateError>(rows)
            })
            .unwrap()
            .unwrap();
    compilation.discard(candidate.semantic_id()).unwrap();
    assert_eq!(compilation.ledger().retained_bytes(), before);
    assert_eq!(compilation.checkpoint_count(), 1);
    let bindings = hosts.bindings.iter().map(|binding| serde_json::json!({"cell":binding.cell.index(),"link_name":binding.link_name})).collect::<Vec<_>>();
    serde_json::json!({
        "case":name,"source":source_text,"source_sha256":digest(source_text),
        "expected":expected,"expected_sha256":digest(expected),
        "c":c,"c_sha256":digest(&c),"header":header,"header_sha256":digest(&header),
        "host_c":HOST_C,"host_c_sha256":digest(HOST_C),"host_js":HOST_JS,"host_js_sha256":digest(HOST_JS),
        "callback_abi_version":1,"host_bindings":bindings,"native_render_work":native_work,
        "executions":executions,"javascript":javascript,
        "compiler_build":if cfg!(debug_assertions) {"debug-assertions"} else {"release-no-debug-assertions"},
        "qualification":"original checked source and fixed trace; same generated C/header artifacts compiled as separate TUs; real reference-count owner and Node callback oracle",
    })
}

macro_rules! closure_fixture {
    ($test:ident, $name:literal) => {
        #[test]
        fn $test() {
            qualify(
                $name,
                include_str!(concat!("fixtures/native-closures/", $name, ".lil")),
                include_str!(concat!("fixtures/native-closures/", $name, ".expected.txt")),
            );
        }
    };
}
closure_fixture!(
    plain_struct_captures_keep_independent_values_and_shared_sibling_cells,
    "factories"
);
closure_fixture!(
    overlapping_reference_paths_and_reentrant_host_clear_keep_live_owners,
    "reentry"
);
closure_fixture!(
    callable_identity_and_prepared_callee_survive_later_argument_replacement,
    "identity"
);
closure_fixture!(
    iteration_boxes_continue_break_and_temporary_owners_do_not_accumulate,
    "iterations"
);
closure_fixture!(
    managed_select_and_nested_returns_transfer_before_cleanup,
    "returns"
);

fn public_qualified_pair(
    session: &mut CheckedSourceSession<'_>,
) -> (String, String, serde_json::Value) {
    let mut bindings = {
        let view = session.compilation().view(session.source()).unwrap();
        HOSTS
            .into_iter()
            .map(|(name, link_name)| {
                let mut found = (0..view.cell_count()).filter_map(|index| {
                    let id = CellId::from_index(index).unwrap();
                    let cell = view.cell(id).unwrap();
                    (cell.binding == CellBinding::Foreign && cell.name == name).then_some(id)
                });
                let cell = found.next().unwrap();
                assert!(found.next().is_none(), "ambiguous checked foreign {name}");
                NativeHostBinding { cell, link_name }
            })
            .collect::<Vec<_>>()
    };
    bindings.sort_by_key(|binding| binding.cell);
    let binding_description = bindings
        .iter()
        .map(|binding| {
            serde_json::json!({
                "cell":binding.cell.index(),"link_name":binding.link_name,
            })
        })
        .collect::<Vec<_>>();
    let (compilation, source, policy) = session.parts_mut();
    assert!(matches!(
        policy.contract(),
        crate::compilation_policy::CompilationContract::Native { .. }
    ));
    let before = compilation.ledger().retained_bytes();
    let codec_before = compilation.ledger().work_by_kind(WorkKind::Codec);
    let receipt = compilation
        .retain_native_c_and_hosts(
            source,
            policy,
            ArtifactRuntimeEvidence::default(),
            WorkDomain::Baseline,
            &NativeHostBindings {
                callback_abi_version: 1,
                bindings: &bindings,
            },
        )
        .unwrap();
    drop(bindings);
    assert_eq!(receipt.policy_fingerprint(), policy.fingerprint());
    assert_eq!(receipt.abi_version(), crate::package::LILSCRIPT_ABI_VERSION);
    assert_eq!(receipt.callback_abi_version(), 1);
    assert_eq!(
        receipt.cost(),
        crate::compilation_policy::CandidateCostEvidence::size_only(
            (receipt.c_bytes() + receipt.header_bytes()) as u64,
        )
    );
    let (c_pointer, header_pointer, capacity) = compilation
        .with_qualified_native_artifact(&receipt, |view| {
            assert_eq!(view.c.len(), receipt.c_bytes());
            assert_eq!(view.header.len(), receipt.header_bytes());
            assert!(view
                .header
                .contains("#define LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION 1"));
            assert!(view.header.contains("host_keep_arg1_retain"));
            assert!(view.header.contains("host_keep_arg1_release"));
            assert!(view.header.contains("host_keep_arg1_call"));
            assert!(view.header.contains("host_take_result"));
            (
                view.c.as_ptr(),
                view.header.as_ptr(),
                view.retained_capacity as u64,
            )
        })
        .unwrap();
    let held = compilation.ledger().retained_bytes();
    assert!(held >= before + capacity);
    let (c, header) = compilation.take_qualified_native_artifact(receipt).unwrap();
    assert_eq!(c.as_ptr(), c_pointer, "qualified C handoff must not copy");
    assert_eq!(
        header.as_ptr(),
        header_pointer,
        "qualified header handoff must not copy"
    );
    assert_eq!(compilation.ledger().retained_bytes(), held - capacity);
    assert_eq!(
        compilation.ledger().work_by_kind(WorkKind::Codec),
        codec_before
    );
    assert!(compilation
        .with_qualified_native_artifact(&receipt, |_| ())
        .is_err());
    let evidence = serde_json::json!({
        "host_bindings":binding_description,
        "callback_abi_version":receipt.callback_abi_version(),
        "abi_version":receipt.abi_version(),
        "policy_fingerprint":receipt.policy_fingerprint(),
        "c_source_bytes":receipt.c_bytes(),"header_source_bytes":receipt.header_bytes(),
        "metric":"exact emitted C plus header source bytes; native executable separately measured by harness",
        "runtime_evidence":"unknown; runtime trace and sanitizer executions follow terminal handoff",
        "retained_file_capacity":capacity,
    });
    (c, header, evidence)
}

fn qualify_public_factory(name: &str, source_text: &str, expected: &str, path_factory: bool) {
    let config: crate::config::ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n").unwrap();
    let options = ServiceOptions {
        target: ServiceTarget::Native,
        logical_work: WORK,
        retained_bytes: MEMORY,
        ..ServiceOptions::default()
    };
    let ((c, header, evidence), native_finished) = if path_factory {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("src/semantic_program/fixtures/native-closures")
            .join(format!("{name}.lil"));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), source_text);
        with_checked_path(&path, &config, options, |session| {
            public_qualified_pair(session)
        })
        .unwrap()
    } else {
        with_checked_source(source_text, &config, options, |session| {
            public_qualified_pair(session)
        })
        .unwrap()
    };
    assert_eq!(native_finished.ledger.retained_bytes(), 0);
    assert_eq!(native_finished.ledger.work_by_kind(WorkKind::Codec), 0);
    assert_eq!(native_finished.report["request"]["target"], "Native");
    assert!(native_finished.report["javascript_policy"].is_null());
    assert!(!native_finished.report["native_policy"].is_null());
    if path_factory {
        assert!(
            native_finished.report["resources"]["source_buffer_capacity"]
                .as_u64()
                .unwrap()
                > 0
        );
    } else {
        assert!(native_finished.report["resources"]["source_buffer_capacity"].is_null());
    }
    // Execute only after the public factory has finalized all source and
    // compilation owners. The host sees the header, never program.c internals.
    let executions = execute_host_artifacts(&c, &header, expected, &format!("public-{name}"));
    let (javascript, javascript_finished) = with_checked_source(
        source_text,
        &config,
        ServiceOptions {
            objectives: Some(crate::structured_js::selection::Objectives::One(
                Objective::Raw,
            )),
            ..ServiceOptions::default()
        },
        |session| {
            let candidate = {
                let (compilation, source, policy) = session.parts_mut();
                compilation
                    .direct_javascript(source, policy, WorkDomain::Baseline)
                    .unwrap()
            };
            session.render_javascript(candidate).unwrap()
        },
    )
    .unwrap();
    assert_eq!(javascript_finished.ledger.retained_bytes(), 0);
    let script = format!("{HOST_JS}\n{}", javascript.javascript());
    let ran = execute(Command::new("node").args(["--input-type=module", "-e", &script]));
    assert!(
        ran.status.success(),
        "{name}: {}",
        String::from_utf8_lossy(&ran.stderr)
    );
    assert!(ran.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&ran.stdout), expected);
    eprintln!(
        "native-public-factory-artifact {}",
        serde_json::json!({
            "case":name,"factory":if path_factory {"with_checked_path"} else {"with_checked_source"},
            "source_sha256":digest(source_text),"expected_sha256":digest(expected),
            "c_source":c,"header_source":header,
            "c_sha256":digest(&c),"header_sha256":digest(&header),
            "host_c_sha256":digest(HOST_C),"host_js_sha256":digest(HOST_JS),
            "qualification":evidence,"native_report":native_finished.report,
            "evidence_storage":"test-only returned strings and serialized receipts; excluded from compiler ledger",
            "javascript_sha256":digest(javascript.javascript()),"javascript_report":javascript_finished.report,
            "stdout":String::from_utf8_lossy(&ran.stdout),"executions":executions,
        })
    );
}

#[test]
fn public_source_factory_qualified_callbacks_compile_as_separate_translation_units() {
    qualify_public_factory(
        "factories",
        include_str!("fixtures/native-closures/factories.lil"),
        include_str!("fixtures/native-closures/factories.expected.txt"),
        false,
    );
}

#[test]
fn public_path_factory_qualified_reentrant_callbacks_compile_as_separate_translation_units() {
    qualify_public_factory(
        "reentry",
        include_str!("fixtures/native-closures/reentry.lil"),
        include_str!("fixtures/native-closures/reentry.expected.txt"),
        true,
    );
}

#[test]
fn callback_output_header_handoff_refusal_and_unwind_release_the_same_owner() {
    let source_text = include_str!("fixtures/native-closures/factories.lil");
    with_hosts(source_text, 0, |compilation, source, hosts| {
        let before = compilation.ledger().retained_bytes();
        let mut called = false;
        let refused = compilation.with_native_c_and_hosts(
            source,
            &native_policy(),
            WorkDomain::Optional,
            hosts,
            |_| {
                called = true;
            },
        );
        assert!(matches!(
            refused,
            Err(NativeError::Allocation(AllocationError::Budget(
                BudgetError::WorkExhausted(WorkDomain::Optional)
            )))
        ));
        assert!(!called);
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let (c, header) = emit_pair(compilation, source, hosts);
        let header_only = compilation
            .with_native_c_and_hosts(
                source,
                &native_policy(),
                WorkDomain::Baseline,
                hosts,
                |output| {
                    let pointer = output.header().as_ptr();
                    let handed_off = output.take_header();
                    assert_eq!(pointer, handed_off.as_ptr());
                    assert_eq!(output.as_str(), c);
                    handed_off
                },
            )
            .unwrap();
        assert_eq!(header_only, header);
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let c_only = compilation
            .with_native_c_and_hosts(
                source,
                &native_policy(),
                WorkDomain::Baseline,
                hosts,
                |output| {
                    let pointer = output.as_str().as_ptr();
                    let handed_off = output.take_c();
                    assert_eq!(pointer, handed_off.as_ptr());
                    assert_eq!(output.header(), header);
                    handed_off
                },
            )
            .unwrap();
        assert_eq!(c_only, c);
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            compilation
                .with_native_c_and_hosts(
                    source,
                    &native_policy(),
                    WorkDomain::Baseline,
                    hosts,
                    |output| {
                        assert_eq!(output.as_str(), c);
                        assert_eq!(output.header(), header);
                        panic!("consumer stopped with both native artifacts alive");
                    },
                )
                .unwrap();
        }));
        assert!(unwound.is_err());
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let after = emit_pair(compilation, source, hosts);
        assert_eq!(after, (c, header));
    });
    with_host_limits(source_text, WORK, 256, |compilation, source, hosts| {
        let before = compilation.ledger().retained_bytes();
        let mut called = false;
        let refused = compilation.with_native_c_and_hosts(
            source,
            &native_policy(),
            WorkDomain::Optional,
            hosts,
            |_| {
                called = true;
            },
        );
        assert!(matches!(
            refused,
            Err(NativeError::Allocation(AllocationError::Budget(
                BudgetError::MemoryExhausted(WorkDomain::Optional)
            )))
        ));
        assert!(!called);
        assert_eq!(compilation.ledger().retained_bytes(), before);
        let (c, header) = emit_pair(compilation, source, hosts);
        assert!(!c.is_empty() && !header.is_empty());
    });
    checked(SIMPLE, WORK, |compilation, source| {
        let c = emit_native(compilation, source);
        compilation
            .with_native_c_and_hosts(
                source,
                &native_policy(),
                WorkDomain::Baseline,
                &NativeHostBindings::EMPTY,
                |output| {
                    assert_eq!(
                        output.as_str(),
                        c,
                        "empty-host extension must preserve the old complete C route"
                    );
                    assert!(output.header().is_empty());
                    assert!(output.take_header().is_empty());
                },
            )
            .unwrap();
    });
}

#[path = "native_closure_module_tests.rs"]
mod module_tests;
