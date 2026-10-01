use super::*;
use crate::compilation_policy::{PolicyConfig, TacticId, TacticPermission};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

fn config(extra: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "objective.codecs='brotli'\n[javascript]\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{extra}"
    )).unwrap()
}

#[test]
fn s3_product_call_copies_preserve_waiting_values_and_mutation_snapshots() {
    let settings = config(
        "[policy.tactics]\ninlining='off'\nconstant-folding='off'\nscalar-replacement='off'",
    );
    for (source, expected) in [
        ("struct P{int x;int y;}int offset(int n){return n+5;}int sum(P p){return p.x+p.y;}P p=P{2,3};print(offset(p.x)+sum(p));", "12\n"),
        ("struct P{int x;int y;}P p=P{1,2};int change(){p.x=9;return 3;}int read(P q,int n){return q.x+q.y+n;}print(read(p,change()));print(p.x);", "6\n9\n"),
        ("struct P{int x;int y;}int update(P p){p.x=9;return p.x+p.y;}P p=P{1,2};print(update(p));print(p.x);", "11\n1\n"),
    ] {
        let result = compile_source(source, &settings, ServiceOptions { objectives: Some(Objectives::All), ..ServiceOptions::default() }).unwrap();
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(), "", ""), expected);
        }
    }
}

pub(super) fn execute_javascript(javascript: &str, setup: &str, body: &str) -> String {
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
            match artifact.sizes().get(codec) {
                Some(size) => assert_eq!(
                    size,
                    crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap()
                ),
                // Only a build without a walk leaves a codec unmeasured
                // (M3.5).
                None => assert!(compiled.report()["search"]["terminal"]["objectives"]
                    .as_array()
                    .is_none_or(Vec::is_empty)),
            }
            assert_eq!(artifact.sha256(), digest(artifact.javascript().as_bytes()));
        }
    }
    assert!(compiled.report().get("backend").is_none());
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
}

#[test]
fn source_service_returns_independently_scored_qualified_winners() {
    let result = compile_source(
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
    let result = compile_source(
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
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/public-structs");
    let result = compile_path(
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
fn independent_winner_handoff_and_all_optional_off_keep_source_literals_and_api() {
    let mut config = config("");
    config.policy = Some(PolicyConfig {
        tactics: TacticId::ALL
            .into_iter()
            .map(|tactic| (tactic, TacticPermission::Off))
            .collect(),
        ..PolicyConfig::default()
    });
    let result = compile_source(
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
    // Even equal bytes have independent policy qualifications. Each codec's
    // disabled search must retain its own direct artifact and public ABI.
    for (index, codec) in [Objective::Raw, Objective::Gzip, Objective::Brotli]
        .into_iter()
        .enumerate()
    {
        assert_eq!(
            result.report()["search"]["objectives"][index]["proposals"],
            0
        );
        let mut one_config = config.clone();
        one_config.objective.codecs = vec![codec];
        let one = compile_source(
            "export string answer(){return \"kept\";}",
            &one_config,
            ServiceOptions::default(),
        )
        .unwrap();
        assert_eq!(
            result.javascript(codec).unwrap().javascript(),
            one.javascript(codec).unwrap().javascript()
        );
    }
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                "",
                "console.log(library.answer());"
            ),
            "kept\n"
        );
    }
}

#[test]
fn path_service_preserves_original_module_initialization_callbacks_and_public_objects() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/modules-javascript");
    let result = compile_path(
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
        include_str!("program/fixtures/modules-javascript/setup.js")
    );
    let body = format!(
        "{}\nconsole.log(JSON.stringify(events));",
        include_str!("program/fixtures/modules-javascript/host.js")
    );
    let output = execute_javascript(
        result.javascript(Objective::Brotli).unwrap().javascript(),
        &setup,
        &body,
    );
    let actual: Value = serde_json::from_str(&output).unwrap();
    let expected: Value = serde_json::from_str(include_str!(
        "program/fixtures/modules-javascript/expected.json"
    ))
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn integrated_original_modules_execute_through_fast_and_searched_public_service() {
    let root =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("src/program/fixtures/integrated-architecture");
    let expected: Value = serde_json::from_str(include_str!(
        "program/fixtures/integrated-architecture/expected.json"
    ))
    .unwrap();
    let setup = format!(
        "const events=[];{}",
        include_str!("program/fixtures/integrated-architecture/setup.js")
    );
    let body = format!(
        "{}\nprocess.stdout.write(JSON.stringify(events));",
        include_str!("program/fixtures/integrated-architecture/host.js")
    );
    for search in [false, true] {
        let mut config = config("");
        if !search {
            config.javascript.candidate_search = crate::config::CandidateSearch::Off;
        }
        let result = compile_path(
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
    let native = compile_path(
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
        include_str!("program/fixtures/integrated-architecture/native.expected.out")
    );
    assert_eq!(native.report()["resources"]["codec_work"], 0);
}

#[test]
fn service_rejects_unknown_runtime_cost_and_unsupported_source_without_fallback() {
    let result = compile_source(
        "export int answer(){return 17;}",
        &config("[policy.constraints]\nmax_startup_work=0"),
        ServiceOptions::default(),
    );
    assert!(result.is_err(), "unknown is not zero");
    let result = compile_source(
        "struct P{int x;}export void add(P[] items){items.push(P{1});}",
        &config(""),
        ServiceOptions::default(),
    )
    .unwrap_err();
    // An exported array of value structs that the function mutates has no
    // D2 adapter, so the target refuses it.
    assert_eq!(result.phase, "javascript");
    assert!(
        result
            .message
            .contains("public value-struct ABI adaptation"),
        "{result}"
    );
    // The target session no longer holds source text, so a formation refusal
    // carries its span in the message but no rendered diagnostic (011 work).
    assert!(result.diagnostic.is_none());
    // Multi-file delivery of one source module is its entry file alone.
    for mode in ["preserve-modules", "split"] {
        let result = compile_source(
            "export int answer(){return 17;}",
            &config(&format!("[delivery]\nmode='{mode}'")),
            ServiceOptions::default(),
        )
        .unwrap();
        // One entry without `import()`: at most its one file (design §7.6).
        assert!(result.javascript(Objective::Brotli).unwrap().files().len() <= 1);
    }
}

#[test]
fn resource_owner_exists_before_discovery_and_parse() {
    let options = ServiceOptions {
        logical_work: 0,
        ..ServiceOptions::default()
    };
    let source = compile_source("not valid syntax", &config(""), options).unwrap_err();
    assert_eq!(source.phase, "frontend resources");
    let path = compile_path(Path::new("/does-not-exist.lil"), &config(""), options).unwrap_err();
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
fn bounded_loop_folding_obeys_configuration_for_javascript_and_native() {
    let source = "int calculate(int seed){int result=seed;for(int i=0;i<4;i+=1){result=result*3+1;}return result;}print(calculate(1));";
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        for permission in [TacticPermission::Off, TacticPermission::On] {
            let mut config = ProjectConfig::default();
            config.objective.codecs = vec![codec];
            config.effort.level = 0;
            config.policy = Some(PolicyConfig {
                tactics: [(TacticId::ConstantFolding, permission)].into(),
                ..PolicyConfig::default()
            });
            let result = compile_source(
                source,
                &config,
                ServiceOptions {
                    target: ServiceTarget::All,
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            let folded = result.report()["phases_ns"]["rules"]["folded_calls"]
                .as_u64()
                .unwrap();
            assert_eq!(
                folded > 0,
                permission == TacticPermission::On,
                "{}",
                result.report()
            );
            assert_eq!(
                execute_javascript(result.javascript(codec).unwrap().javascript(), "", ""),
                "121\n"
            );
            assert_eq!(execute_native(result.native_c().unwrap()), "121\n");
        }
    }
}

#[test]
fn constant_scalar_methods_obey_toml_and_match_native_and_javascript() {
    let source = r#"
        int truncated(float value){return value.toInt();}
        int points(string value){return value.codePointLength();}
        print(truncated(4294967297.75));print(truncated(-4294967297.75));
        print(truncated(0.0/0.0));print(truncated(1.0/0.0));
        print(points("A😀B"));print(points(""));
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for (permission, pristine) in [("off", false), ("on", false), ("off", true), ("on", true)] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n\
                 javascript.assume_pristine_builtins={pristine}\n\
                 [policy.tactics]\nconstant-folding='{permission}'\n\
                 inlining='off'\ncall-specialization='off'"
            ))
            .unwrap();
            let result = compile_source(
                source,
                &config,
                ServiceOptions {
                    target: ServiceTarget::All,
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            let folded = result.report()["phases_ns"]["rules"]["folded_calls"]
                .as_u64()
                .unwrap();
            assert_eq!(folded > 0, permission == "on", "{}", result.report());
            let objective = config.objective.codecs[0];
            assert_eq!(
                execute_javascript(result.javascript(objective).unwrap().javascript(), "", ""),
                "1\n-1\n0\n0\n3\n0\n"
            );
            assert_eq!(
                execute_native(result.native_c().unwrap()),
                "1\n-1\n0\n0\n3\n0\n"
            );
        }
    }
}

#[test]
fn finite_values_and_default_transport_obey_toml_in_both_targets() {
    let tail = "print(99);".repeat(70);
    let source = format!("bool classify(int value){{if(value>0){{return true;}}else{{{tail}return false;}}}}print(classify(1));print(classify(2));void report(int tag,int? value=null){{print(tag);print(value);}}for(int i=0;i<2;i+=1){{report(i);report(i,7);}}int current=1;int snapshot=current;current=2;print(snapshot);print(current);");
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["on", "off"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nconstant-folding='{permission}'\ninlining='off'\ncall-specialization='off'"
            )).unwrap();
            let result = compile_source(
                &source,
                &config,
                ServiceOptions {
                    target: ServiceTarget::All,
                    ..ServiceOptions::default()
                },
            )
            .unwrap();
            let receipt = &result.report()["phases_ns"]["rules"];
            for field in [
                "set_folds",
                "default_arguments_omitted",
                "forwarded_definitions",
            ] {
                assert_eq!(
                    receipt[field].as_u64().unwrap() > 0,
                    permission == "on",
                    "{field}: {receipt}"
                );
            }
            let expected = "true\ntrue\n0\nnull\n0\n7\n1\nnull\n1\n7\n1\n2\n";
            assert_eq!(
                execute_javascript(
                    result
                        .javascript(config.objective.codecs[0])
                        .unwrap()
                        .javascript(),
                    "",
                    ""
                ),
                expected
            );
            assert_eq!(execute_native(result.native_c().unwrap()), expected);
        }
    }
}

#[test]
fn folded_nullable_literals_keep_the_native_tagged_representation() {
    let source = r#"
        T? maybe<T>(bool present,T value){if(present){return value;}return null;}
        int? integer=maybe(true,7);
        float? floating=maybe(true,-0.0);
        bool? boolean=maybe(true,true);
        string? text=maybe(true,"literal");
        string? empty=maybe(true,"");
        int? absent=maybe(false,8);
        print(integer);print(floating);print(boolean);print(text);print(empty);print(absent);
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            target: ServiceTarget::All,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    assert!(
        result.report()["phases_ns"]["rules"]["folded_calls"]
            .as_u64()
            .unwrap()
            > 0
    );
    let expected = "7\n-0\ntrue\nliteral\n\nnull\n";
    assert_eq!(
        execute_javascript(
            result.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            ""
        ),
        expected
    );
    assert_eq!(execute_native(result.native_c().unwrap()), expected);
}

#[test]
fn numeric_primitive_folding_keeps_host_lookups_and_argument_effects() {
    let source = r#"
        extern float next();
        int truncate(float value){return value.toInt();}
        print(truncate(4294967297.5));
        print(truncate(next()));
        print(2.5.floor());
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["off", "on"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\njavascript.assume_pristine_builtins=false\n[policy.tactics]\nconstant-folding='{permission}'\ninlining='off'\ncall-specialization='off'"
            )).unwrap();
            let result = compile_source(source, &config, ServiceOptions::default()).unwrap();
            let javascript = result
                .javascript(config.objective.codecs[0])
                .unwrap()
                .javascript();
            assert_eq!(execute_javascript(javascript,
                "let calls=0;globalThis.next=()=>{calls++;return -4294967297.5};Math.floor=()=>91;Number.prototype.valueOf=()=>{throw Error('primitive conversion must not call valueOf')};",
                "console.log(calls);"), "1\n-1\n91\n1\n");
            assert_eq!(
                result.report()["phases_ns"]["rules"]["folded_calls"]
                    .as_u64()
                    .unwrap()
                    > 0,
                permission == "on",
                "{}",
                result.report()
            );
        }
    }
}

#[test]
fn constant_scalar_code_point_method_keeps_an_observable_host_iterator() {
    let config: ProjectConfig = toml::from_str(
        "objective.codecs='raw'\neffort.level=13\n\
         javascript.assume_pristine_builtins=false\n\
         [policy.tactics]\nconstant-folding='on'",
    )
    .unwrap();
    let result = compile_source(
        "print(\"abc\".codePointLength());",
        &config,
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(result.report()["phases_ns"]["rules"]["folded_calls"], 0);
    let javascript = result.javascript(Objective::Raw).unwrap().javascript();
    assert_eq!(execute_javascript(javascript, "", ""), "3\n");
    assert_eq!(
        execute_javascript(
            javascript,
            "String.prototype[Symbol.iterator]=function*(){yield 'changed';};",
            ""
        ),
        "1\n"
    );
}

#[test]
fn strict_target_inlining_obeys_toml_for_imported_struct_helpers() {
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("helpers.lil"),
        "export struct P{int value;}int read(P p){return p.value+1;}export int sum(P p){return read(p)+2;}").unwrap();
    let entry = scratch.0.join("main.lil");
    std::fs::write(
        &entry,
        "import {P,sum} from \"./helpers\";extern int next();print(sum(P{next()}));",
    )
    .unwrap();
    for codec in ["raw", "gzip", "brotli"] {
        for module in [false, true] {
            let mut lengths = Vec::new();
            for permission in ["off", "on"] {
                let config: ProjectConfig = toml::from_str(&format!(
                    "objective.codecs='{codec}'\neffort.level=13\n\
                     [policy.tactics]\ninlining='{permission}'\nidentifier-mangling='off'"
                ))
                .unwrap();
                let result = compile_path(
                    &entry,
                    &config,
                    ServiceOptions {
                        preserve_root_exports: module,
                        ..ServiceOptions::default()
                    },
                )
                .unwrap();
                let javascript = result
                    .javascript(config.objective.codecs[0])
                    .unwrap()
                    .javascript();
                let setup = "let calls=0;globalThis.next=()=>{if(++calls!==1)throw Error('duplicate');return 7;};";
                let output = if module {
                    execute_javascript(javascript, setup, "")
                } else {
                    let output = Command::new("node")
                        .args(["-e", &format!("{setup}\n{javascript}")])
                        .output()
                        .unwrap();
                    assert!(
                        output.status.success(),
                        "{}\n{javascript}",
                        String::from_utf8_lossy(&output.stderr)
                    );
                    String::from_utf8(output.stdout).unwrap()
                };
                assert_eq!(output, "10\n");
                lengths.push(javascript.len());
            }
            assert!(
                lengths[1] < lengths[0],
                "{codec} module={module}: {lengths:?}"
            );
        }
    }
}

#[test]
fn scalar_objects_public_construction_preserves_callback_initialization_errors() {
    let source = "class State{int early;int late;init(int a,int b){this.early=a;this.late=b;}}\n\
        extern int initialize(func()->int read);\n\
        int make(){State state=new State(1,initialize(()=>state.early));return state.early;}\n\
        try{print(make());}catch(auto error){print(99);}";
    let after = "try{console.log(saved())}catch(error){console.log(error.name)}";
    for codec in ["raw", "gzip", "brotli"] {
        for permission in ["off", "on"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nscalar-replacement='{permission}'"
            )).unwrap();
            let result = compile_source(source, &config, ServiceOptions::default()).unwrap();
            let javascript = result
                .javascript(config.objective.codecs[0])
                .unwrap()
                .javascript();
            assert_eq!(execute_javascript(javascript,
                "globalThis.saved=null;globalThis.initialize=read=>{saved=read;try{console.log(read())}catch(error){console.log('during:'+error.name)}return 2;};",
                after), "during:ReferenceError\n1\n1\n", "{codec}/{permission}\n{javascript}");
            assert_eq!(execute_javascript(javascript,
                "globalThis.saved=null;globalThis.initialize=read=>{saved=read;throw Error('initializer');};",
                after), "99\nReferenceError\n", "{codec}/{permission}\n{javascript}");
        }
    }
}

#[test]
fn service_reports_unsupported_permissions_separately_from_policy_identity() {
    let result = compile_source(
        "print(7);",
        &config("[policy.tactics]\nrecurring-reconstruction='on'"),
        ServiceOptions::default(),
    )
    .unwrap();
    assert_eq!(
        execute_javascript(
            result.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            ""
        ),
        "7\n"
    );
    let report = result.report();
    assert!(report["policy_diagnostics"]["javascript"]
        .as_array()
        .unwrap()
        .iter()
        .any(|value| value.as_str().unwrap().contains("recurring-reconstruction")));
    assert!(report["javascript_policy"].get("diagnostics").is_none());
    let tactic =
        &report["javascript_policy"]["tactics"][TacticId::RecurringReconstruction as usize];
    assert_eq!(tactic["state"]["permission"], "on");
    assert_eq!(tactic["available"], false);
    assert_eq!(tactic["state"]["enabled"], false);
}

#[test]
fn application_iife_owns_private_roots_and_independent_loads_under_each_objective() {
    let source = "extern int next();extern void retain(func()->int callback);int shared=next();retain(()=>shared);";
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let mut config = config("[policy.tactics]\nidentifier-mangling='off'");
        config.objective.codecs = vec![codec];
        let result = compile_source(
            source,
            &config,
            ServiceOptions {
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        check_scores(&result);
        let artifact = result.javascript(codec).unwrap();
        assert_eq!(
            result.report()["javascript_policy"]["contract"]["delivery"]["format"],
            "iife"
        );
        assert_eq!(artifact.details()["execution"], "Script");
        let script = format!(
            r#"
            const vm=require('node:vm');const callbacks=[];let value=0;
            const context=vm.createContext({{next:()=>++value,retain:f=>callbacks.push(f)}});
            vm.runInContext('let shared=91;var a=18',context);
            const before=Object.keys(context).sort();const code={};
            vm.runInContext(code,context);vm.runInContext(code,context);
            const actual=callbacks.map(f=>f());
            if(JSON.stringify(actual)!=='[1,2]' ||
               JSON.stringify(before)!==JSON.stringify(Object.keys(context).sort()) ||
               vm.runInContext('shared',context)!==91)throw Error('private scope leaked');
            console.log(JSON.stringify(actual));
        "#,
            serde_json::to_string(artifact.javascript()).unwrap()
        );
        let oracle = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            oracle.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&oracle.stderr),
            artifact.javascript()
        );
        assert_eq!(oracle.stdout, b"[1,2]\n");
    }
}

#[test]
fn private_wrapper_preserves_classic_this_and_accepts_explicit_bare() {
    let source = "extern JsValue this;extern void invoke(func()->void callback);extern bool isGlobal(JsValue value);void probe(){print(isGlobal(this));}invoke(probe);";
    for format in [
        crate::config::JavaScriptFormat::Iife,
        crate::config::JavaScriptFormat::Bare,
    ] {
        let mut config = config("");
        config.target.javascript.format = format;
        let result = compile_source(
            source,
            &config,
            ServiceOptions {
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        let artifact = result.javascript(Objective::Brotli).unwrap();
        assert_eq!(
            artifact.javascript().starts_with("(()=>{"),
            format == crate::config::JavaScriptFormat::Iife
        );
        let script = format!(
            "const vm=require('node:vm');vm.runInThisContext({}+{});",
            serde_json::to_string(
                "globalThis.invoke=f=>f();globalThis.isGlobal=value=>value===globalThis;"
            )
            .unwrap(),
            serde_json::to_string(artifact.javascript()).unwrap()
        );
        let output = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stdout, b"true\n");
    }
}

#[test]
fn script_containers_refuse_module_exports_and_imports() {
    for format in ["iife", "bare"] {
        let settings = config(&format!("[target.javascript]\nformat='{format}'"));
        let error = compile_source(
            "export int answer(){return 7;}",
            &settings,
            ServiceOptions::default(),
        )
        .unwrap_err();
        assert!(error.message.contains("library global exports"), "{error}");
    }
    let error = compile_source(
        "print(7);",
        &config("[target.javascript]\nformat='esm'"),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert!(
        error.message.contains("requires module execution"),
        "{error}"
    );
    let scratch = Scratch::new();
    let entry = scratch.0.join("entry.lil");
    std::fs::write(
        &entry,
        r#"import extern { next } from "host";extern int next();print(next());"#,
    )
    .unwrap();
    let error = compile_path(
        &entry,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap_err();
    assert!(error.message.contains("static imports require"), "{error}");
}

#[test]
fn native_and_all_share_checked_meaning_without_native_codec_work() {
    let source = "int twice(int value){return value*2;}print(twice(21));";
    let javascript = compile_source(
        source,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for target in [ServiceTarget::Native, ServiceTarget::All] {
        let result = compile_source(
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
        let error = compile_source(
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
fn searched_string_records_respect_scalar_permissions_under_every_objective() {
    let source = r#"
        export func(string)->string make(string seed) {
            Record<string> state=record{value:seed};
            return (string suffix)=>{
                state.value=(state.value??"")+suffix;
                return state.value??"";
            };
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        let mut sizes = Vec::new();
        for permission in ["off", "on"] {
            let config: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=13\n[policy.tactics]\nscalar-replacement='{permission}'"
            )).unwrap();
            let compiled = compile_source(source, &config, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let objective = match codec {
                "raw" => Objective::Raw,
                "gzip" => Objective::Gzip,
                _ => Objective::Brotli,
            };
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(
                execute_javascript(artifact.javascript(), "", "const a=library.make('a'),b=library.make('b');console.log(a('!'),b('?'),a('.'),a!==b,library.make.length);"),
                "a! b? a!. true 1\n"
            );
            if permission == "off" {
                assert!(
                    artifact.javascript().contains("__proto__:"),
                    "{}",
                    artifact.javascript()
                );
                assert!(!artifact.details()["output"]["rules"]["scalar_replacement"]
                    .as_bool()
                    .unwrap());
            } else {
                assert!(
                    !artifact.javascript().contains("__proto__:"),
                    "{}",
                    artifact.javascript()
                );
            }
            sizes.push(artifact.sizes().get(objective).unwrap());
        }
        assert!(
            sizes[1] <= sizes[0],
            "{codec}: scalar alternatives regressed {sizes:?}"
        );
    }
}

#[test]
fn searched_nullish_assignment_preserves_stores_and_value_copies_at_each_syntax_floor() {
    let source = r#"
        struct Point { int x; }
        export int fill((int?)[] values,int index){return values[index]??=7;}
        export int copies(){
            Point initial=Point{3};Point? slot=null;
            Point first=slot??=initial;first.x=4;initial.x=5;
            Point second=slot??=initial;
            return first.x*100+second.x*10+initial.x;
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for floor in ["es2015", "es2022"] {
            for permission in ["off", "on"] {
                let configured: ProjectConfig = toml::from_str(&format!(
                    "objective.codecs='{codec}'\n[javascript]\necmascript='{floor}'\n[policy.tactics]\ntarget-compaction='{permission}'"
                )).unwrap();
                let compiled =
                    compile_source(source, &configured, ServiceOptions::default()).unwrap();
                check_scores(&compiled);
                let objective = match codec {
                    "raw" => Objective::Raw,
                    "gzip" => Objective::Gzip,
                    _ => Objective::Brotli,
                };
                let javascript = compiled.javascript(objective).unwrap().javascript();
                if floor == "es2015" {
                    assert!(!javascript.contains("??"), "{javascript}");
                }
                assert_eq!(
                    execute_javascript(
                        javascript,
                        "",
                        r#"
                    const events=[], values=new Proxy([null,0],{
                        get:(a,k)=>{events.push('get:'+k);return a[k]},
                        set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}
                    });
                    console.log(library.fill(values,0),library.fill(values,1),library.fill(values,0));
                    console.log(events.join(','));console.log(library.copies());
                "#
                    ),
                    "7 0 7\nget:0,set:0:7,get:1,get:0\n435\n",
                    "{codec}/{floor}/{permission}"
                );
            }
        }
    }
}

#[test]
fn searched_typed_record_spread_keeps_value_copies_and_all_objective_contracts() {
    let source = r#"
        struct Point{int x;int y;}
        export string run(){
            Point seed=Point{1,2};
            Record<Point> original=record{point:seed};
            Record<Point> copy=record{...original};
            seed.x=8;Point first=original.point??Point{0,0};first.x=7;
            original.point=first;first.y=9;
            Point saved=copy.point??Point{0,0};Point changed=original.point??Point{0,0};
            return `${saved.x},${saved.y},${changed.x},${changed.y}`;
        }
        export int order(){
            Record<int> source=record{value:1};
            func()->int change=()=>{source.value=9;return 2;};
            Record<int> copy=record{...source,other:change(),...record{third:3}};
            return (copy.value??0)*100+(copy.other??0)*10+(copy.third??0);
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        for floor in ["es2015", "es2022"] {
            for compact in ["on", "off"] {
                let configured: ProjectConfig = toml::from_str(&format!(
                    "objective.codecs='{codec}'\neffort.level=13\n\
                     [javascript]\necmascript='{floor}'\n\
                     [policy.tactics]\ntarget-compaction='{compact}'"
                ))
                .unwrap();
                let compiled =
                    compile_source(source, &configured, ServiceOptions::default()).unwrap();
                check_scores(&compiled);
                let objective = match codec {
                    "raw" => Objective::Raw,
                    "gzip" => Objective::Gzip,
                    _ => Objective::Brotli,
                };
                let javascript = compiled.javascript(objective).unwrap().javascript();
                assert_eq!(
                    execute_javascript(
                        javascript,
                        "",
                        "console.log(library.run(),library.order());"
                    ),
                    "1,2,7,2 123\n"
                );
            }
        }
    }
}

#[test]
fn stable_rule_scheduling_preserves_searched_artifacts_and_behavior() {
    let source = r#"
        int add(int value){return value+1;}
        export int run(int input){
            Record<int> pair=record{left:input,right:add(input)};
            int total=0;
            for(int i=0;i<4;i+=1){
                if(i==1){continue;}
                total+=(pair.left??0)+(pair.right??0);
            }
            if(input<0){return total+1;}return total;
        }
        export func(int)->int make(int initial){
            Record<int> state=record{value:initial};
            return (int step)=>{state.value=(state.value??0)+step;return state.value??0;};
        }
    "#;
    for codec in ["raw", "gzip", "brotli"] {
        let configured: ProjectConfig =
            toml::from_str(&format!("objective.codecs='{codec}'\neffort.level=13")).unwrap();
        let objective = match codec {
            "raw" => Objective::Raw,
            "gzip" => Objective::Gzip,
            _ => Objective::Brotli,
        };
        let runs = [false, true].map(|dense| {
            let _audit = dense.then(crate::schedule::DenseAudit::new);
            let compiled = compile_source(source, &configured, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(execute_javascript(artifact.javascript(), "",
                "const a=library.make(2),b=library.make(7);console.log(library.run(3),library.run(-1),a(3),b(4),a(1));"),
                "21 -2 5 11 6\n");
            (artifact.javascript().to_owned(), artifact.sizes, compiled.report()["phases_ns"]["rules"].clone())
        });
        assert_eq!(runs[0], runs[1], "{codec}");
    }
}

#[test]
fn check_service_diagnoses_detached_primitive_methods_before_target_selection() {
    for source in [
        "auto method=\"text\".charCodeAt;",
        "Map<string,int> values=new Map<string,int>();auto method=values.get;",
        "auto values=new Set<int>();auto method=values.has;",
        "string? value=null;auto method=value?.charCodeAt;",
    ] {
        let error = check_source(source, &ProjectConfig::default()).unwrap_err();
        assert!(
            error.message.contains("called through its receiver"),
            "{error}"
        );
        for target in [ServiceTarget::JavaScript, ServiceTarget::Native] {
            let error = compile_source(
                source,
                &ProjectConfig::default(),
                ServiceOptions {
                    target,
                    ..ServiceOptions::default()
                },
            )
            .unwrap_err();
            assert!(
                error.message.contains("called through its receiver"),
                "{error}"
            );
        }
    }
    check_source(
        "extern JsValue object;auto method=object.method;",
        &ProjectConfig::default(),
    )
    .unwrap();
    check_source(
        "struct Holder{func(int)->int callback;}Holder h=Holder{(int v)=>v+1};\
        auto callback=h.callback;print(callback(2));",
        &ProjectConfig::default(),
    )
    .unwrap();
}

#[test]
fn searched_local_read_order_is_independently_judged_and_reports_its_veto() {
    let parameters = (0..60)
        .map(|index| format!("int input{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let expression = ["input59"; 8].join("+");
    let source = format!("export int hot({parameters}){{return {expression};}}");
    for codec in ["raw", "gzip", "brotli"] {
        for allowed in [true, false] {
            let configured: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=14\n[policy.tactics]\n\
                 naming-search='{}'\nnaming-alphabet='off'\ntarget-compaction='off'\n\
                 inlining='off'\ncall-specialization='off'\nconstant-folding='off'",
                if allowed { "on" } else { "off" }
            ))
            .unwrap();
            let compiled = compile_source(&source, &configured, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let objective = match codec {
                "raw" => Objective::Raw,
                "gzip" => Objective::Gzip,
                _ => Objective::Brotli,
            };
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(execute_javascript(artifact.javascript(), "",
                "console.log(library.hot(...Array.from({length:60},(_,i)=>i)),library.hot.name,library.hot.length);"),
                "472 hot 60\n");
            let report = compiled.report();
            let objectives = report["search"]["terminal"]["objectives"]
                .as_array()
                .unwrap();
            let trials = objectives[0]["joint_trials"].as_array().unwrap();
            let local = trials
                .iter()
                .filter(|trial| trial["name"] == "naming:local-read-order")
                .collect::<Vec<_>>();
            assert!(!local.is_empty(), "{report}");
            if allowed {
                assert!(local.iter().any(|trial| matches!(
                    trial["outcome"].as_str(),
                    Some("kept" | "rejected" | "identical")
                )));
                if codec == "raw" {
                    assert_eq!(artifact.details["output"]["local_read_order"], true);
                }
            } else {
                assert!(local.iter().all(|trial| trial["outcome"] == "vetoed"));
                assert_eq!(artifact.details["output"]["local_read_order"], false);
            }
        }
    }
}

#[test]
fn searched_private_properties_are_independent_and_preserve_each_objective_winner() {
    let source = include_str!("program/fixtures/private-fields.lil");
    for codec in ["raw", "gzip", "brotli"] {
        let objective = match codec {
            "raw" => Objective::Raw,
            "gzip" => Objective::Gzip,
            _ => Objective::Brotli,
        };
        let mut sizes = Vec::new();
        for permission in ["off", "on"] {
            let configured: ProjectConfig = toml::from_str(&format!(
                "objective.codecs='{codec}'\neffort.level=14\n[policy.tactics]\nproperty-mangling='{permission}'\nscalar-replacement='off'\ninlining='off'"
            )).unwrap();
            let compiled = compile_source(source, &configured, ServiceOptions::default()).unwrap();
            check_scores(&compiled);
            let artifact = compiled.javascript(objective).unwrap();
            assert_eq!(
                execute_javascript(
                    artifact.javascript(),
                    "",
                    "const a=library.make(2),b=library.make(10);console.log(a(3),b(1),a(-2));"
                ),
                "15 21 13\n"
            );
            sizes.push(artifact.sizes.get(objective).unwrap());
            let report = compiled.report();
            let trials = report["search"]["terminal"]["objectives"][0]["joint_trials"]
                .as_array()
                .unwrap();
            let properties = trials
                .iter()
                .filter(|t| t["name"] == "properties:private-fields")
                .collect::<Vec<_>>();
            // One protected final trial, plus any deferred combinations.
            assert!(!properties.is_empty(), "{report}");
            if permission == "off" {
                assert!(properties.iter().all(|trial| trial["outcome"] == "vetoed"));
                assert_eq!(artifact.details["output"]["property_mangling"], false);
            } else if codec == "raw" {
                assert_eq!(properties[0]["outcome"], "kept");
                assert_eq!(artifact.details["output"]["property_mangling"], true);
            }
        }
        assert!(sizes[1] <= sizes[0], "{codec}: {sizes:?}");
    }
    for (identifiers, compact, expected) in [("off", "on", true), ("on", "off", false)] {
        let configured: ProjectConfig = toml::from_str(&format!(
            "objective.codecs='raw'\neffort.level=14\n[policy.tactics]\nproperty-mangling='on'\nidentifier-mangling='{identifiers}'\ntarget-compaction='{compact}'\nscalar-replacement='off'\ninlining='off'"
        )).unwrap();
        let compiled = compile_source(source, &configured, ServiceOptions::default()).unwrap();
        check_scores(&compiled);
        let artifact = compiled.javascript(Objective::Raw).unwrap();
        assert_eq!(artifact.details["output"]["property_mangling"], expected);
        assert_eq!(
            execute_javascript(
                artifact.javascript(),
                "",
                "console.log(library.make(2)(3));"
            ),
            "15\n"
        );
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
    let default = compile_source(source, &config, options).unwrap();
    let (batch, finished) = with_checked_source(source, &config, options, |session| {
        assert!(session.phases()["frontend_release_ns"].is_u64());
        assert_eq!(session.compilation().checkpoint_count(), 1);
        let mut baselines = 0;
        let batch = session
            .search_javascript(session.source(), |observation| {
                baselines += usize::from(observation.baseline);
            })
            .unwrap();
        assert_eq!(baselines, 3);
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
    use crate::program::{CellBinding, CellId, Constant, OpId, OperationKind};

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
        .join("src/program/fixtures/search-structural-valley/entry.lil")
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
    let result = compile_source(
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
    assert!(report.get("backend").is_none());
    assert_eq!(report["ledger_after_finish"]["retained_bytes"], 0);
    assert!(report["phases_ns"]["source_release_ns"].is_null());
}

#[test]
fn delivered_rewrite_metadata_survives_finalization_and_marks_inherited_history() {
    use crate::program::{CellBinding, CellId, Constant, OpId, OperationKind};

    // The program rules fold `3+4` before publication; this test needs the
    // publication layer's own rewrite of it.
    SKIP_PROGRAM_RULES.with(|skip| skip.set(true));
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
    SKIP_PROGRAM_RULES.with(|skip| skip.set(false));
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
    let error = compile_path(&path, &config(""), ServiceOptions::default()).unwrap_err();
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
    let error = compile_path(
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
    let error = compile_source(
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
        let error = compile_source(
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
    let (modules, syntax) = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap();
    let source_bytes = sources.allocated_bytes() as u64;
    drop(syntax);
    drop(arena);
    let work = frontend.ledger.work_used(WorkDomain::Baseline);
    drop(modules);
    sources.discard(&mut frontend.ledger).unwrap();
    let releases = ReleaseObserver::new();
    let error = compile_path(
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
    let (modules, syntax) = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap();
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
        let error = compile_path(
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
    let error = compile_path(
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
fn path_factory_parses_each_canonical_module_once_and_releases_syntax_before_backend() {
    use crate::parser::admitted_arena_activity_for_test;
    use crate::program::CellId;

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
    let (modules, syntax) = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap();
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
    let error = discover_parsed_modules_admitted(
        &[crate::module::EntrySource::of(&path)],
        None,
        &config,
        &sources,
        &arena,
    )
    .unwrap_err();
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
    let error = compile_path(&path, &config, options).unwrap_err();
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
    let result = compile_source(
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
    // the root module's directory as the old route's linker spelled them.
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
    let result = compile_path(
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
    // An import's identity is `(source, imported)`: both modules' `twice`
    // is one binding, the default is the default binding, and the one
    // specifier has one declaration (C18).
    assert_eq!(javascript.matches("import").count(), 1, "{javascript}");
    assert_eq!(javascript.matches("twice").count(), 1, "{javascript}");
    assert!(
        javascript.starts_with("import ") && !javascript.contains("default as"),
        "{javascript}"
    );
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
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "hi422:42\n");
}

/// Three wrong programs the harvest of the old route's tests found in the
/// searched output (plan M1.9): an async body inlined as its return value,
/// field defaults created before a constructor's arguments, and `==` on a
/// `JsValue` compiled as `===` against the language's dynamic equality.
#[test]
fn searched_output_keeps_async_calls_construction_order_and_dynamic_equality() {
    let source = r#"
        extern int read();
        extern JsValue dynamic();
        extern void note(string label);
        async int immediate() { return 1; }
        class Cache {
            Map<string, int> guard;
            int value;
            init(int value) { this.value = value; }
        }
        export void run() {
            immediate().then((int v) => print(v));
            Cache cache = new Cache(read());
            print(cache.value);
            JsValue v = dynamic();
            print(v == 0);
            print(v != 0);
        }
    "#;
    let setup = "const events=[];globalThis.Map=class{constructor(){events.push('map')}};\
        globalThis.read=()=>{events.push('read');return 7};\
        globalThis.dynamic=()=>({valueOf(){events.push('coerce');return 0}});";
    for codec in [Objective::Raw, Objective::Brotli] {
        let result = compile_source(
            source,
            &config(""),
            ServiceOptions {
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        assert_eq!(
            execute_javascript(
                result.javascript(codec).unwrap().javascript(),
                setup,
                "library.run();await 0;console.log(events.join(','));"
            ),
            "7\ntrue\nfalse\n1\nread,map,coerce,coerce\n",
            "{}",
            result.javascript(codec).unwrap().javascript()
        );
    }
}

/// A method is its own function (plan M8.2 A1, law P1): no private callback
/// shares an adapter factory's body, and where the contract does not keep
/// names, a method may carry the name JavaScript infers (R6). This pins the
/// gained name: the object key names the method, where the adapter's result
/// was anonymous.
#[test]
fn adapter_callbacks_are_their_own_methods_and_may_gain_an_inferred_name() {
    let source = "JsValue holder = JS.object(\"greet\", JS.method1((JsValue self, JsValue who) => JS.add(self[\"p\"], who)));\n\
                  JsValue other = JS.object(\"echo\", JS.method1((JsValue self, JsValue who) => JS.add(who, self[\"p\"])));\n\
                  export JsValue make() { return JS.object(\"a\", holder, \"b\", other); }";
    for keep in [false, true] {
        let result = compile_source(
            source,
            &config(&format!("keep_function_names={keep}")),
            ServiceOptions {
                objectives: Some(Objectives::All),
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            let javascript = result.javascript(codec).unwrap().javascript();
            // No shared factory: each callback reads its receiver as `this`.
            assert!(!javascript.contains("(this,"), "{javascript}");
            assert_eq!(javascript.matches("this.p").count(), 2, "{javascript}");
            let names = if keep {
                "[\"\",\"\"]"
            } else {
                "[\"greet\",\"echo\"]"
            };
            assert_eq!(
                execute_javascript(
                    javascript,
                    "",
                    "const m=library.make();const o={p:'!',greet:m.a.greet,echo:m.b.echo};console.log(JSON.stringify([m.a.greet.name,m.b.echo.name]),o.greet('x'),o.echo('y'),m.a.greet.length);"
                ),
                format!("{names} !x y! 1\n"),
                "keep_function_names={keep}\n{javascript}"
            );
        }
    }
}

/// A class body is strict. Under `--target js` a static body moves into its
/// prototype method only when strictness cannot change what it does (plan
/// M8.2 A1, architecture §10.2): a failed host write that a sloppy frame
/// ignores stays in a sloppy function; a body without one becomes the
/// method.
#[test]
fn class_bodies_in_a_classic_script_keep_sloppy_host_writes_out_of_the_class() {
    let source = "extern JsValue Object;\n\
                  export class Writer {\n\
                    string label() { return \"w\"; }\n\
                    bool poke(JsValue target) { target[\"x\"] = 1; return true; }\n\
                  }\n\
                  export constructor Writer;\n\
                  JsValue writer = new Writer();\n\
                  print(JS.invoke(writer, \"label\"));\n\
                  print(JS.invoke(writer, \"poke\", JS.invoke(Object, \"freeze\", JS.object())));";
    // A path build, as the CLI's `--target js` makes it.
    let scratch = Scratch::new();
    std::fs::write(scratch.0.join("writer.lil"), source).unwrap();
    let result = compile_path(
        &scratch.0.join("writer.lil"),
        &config(""),
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let javascript = result.javascript(Objective::Brotli).unwrap().javascript();
    // `label` is its body; `poke` forwards to a sloppy static body.
    assert!(javascript.contains("label(){return\"w\"}"), "{javascript}");
    assert!(
        javascript.contains("poke(") && javascript.contains("(this,"),
        "{javascript}"
    );
    let output = Command::new("node")
        .args(["-e", javascript])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "w\ntrue\n");
}

#[test]
fn application_class_forwarders_keep_sloppy_assignment_and_delete_semantics() {
    // Y5 hides caller reflection, but a class is still strict. Moving one
    // of these static bodies into its forwarding method would throw on
    // the frozen property instead of leaving it unchanged.
    for (method, expected) in [
        (
            "void poke(JsValue target) { target[\"x\"] = 1; }",
            "undefined\n",
        ),
        (
            "void poke(JsValue target) { JS.delete(target, \"x\"); }",
            "undefined\n",
        ),
    ] {
        let source = format!(
            "extern JsValue Object; export class Writer {{ {method} }}\n\
             export constructor Writer; JsValue writer = new Writer();\n\
             print(JS.invoke(writer, \"poke\", JS.invoke(Object, \"freeze\", JS.object(\"x\", 0))));"
        );
        let scratch = Scratch::new();
        std::fs::write(scratch.0.join("writer.lil"), source).unwrap();
        let compiled = compile_path(
            &scratch.0.join("writer.lil"),
            &config(""),
            ServiceOptions {
                preserve_root_exports: false,
                ..ServiceOptions::default()
            },
        )
        .unwrap();
        let javascript = compiled.javascript(Objective::Brotli).unwrap().javascript();
        let output = Command::new("node")
            .args(["-e", javascript])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{javascript}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap(),
            expected,
            "{javascript}"
        );
    }
}

#[test]
fn the_objective_codec_settings_judge_report_and_fingerprint_the_build() {
    // Law B2 (M3.5): the judge is the configured codec, and its settings are
    // objective configuration.
    use crate::compression::{BrotliMode, BrotliSettings, CodecSettings, GzipSettings};
    let source = "export int answer(int value){int total=0;for(int i=0;i<value;i++){total+=i*3+1;}return total;}";
    let config = |settings: &str| {
        crate::config::parse_project_config(&format!("[objective]\ncodecs='brotli'\n{settings}"))
            .unwrap()
            .config
    };
    let canonical = config("");
    let nine = config("[objective.brotli]\nquality=9\nwindow=18\n");
    let request = CompilationRequest::JavaScript {
        preserve_root_exports: true,
    };
    let (canonical_policy, nine_policy) = (
        canonical.resolve_policy(request).unwrap(),
        nine.resolve_policy(request).unwrap(),
    );
    assert_ne!(canonical_policy.fingerprint(), nine_policy.fingerprint());
    assert_eq!(
        nine_policy.receipt()["objective"]["codec_settings"]["brotli"]["quality"],
        9
    );
    let compiled = compile_source(source, &nine, ServiceOptions::default()).unwrap();
    let artifact = compiled.javascript(Objective::Brotli).unwrap();
    let settings = CodecSettings {
        brotli: BrotliSettings {
            quality: 9,
            window: 18,
            mode: BrotliMode::Generic,
        },
        gzip: GzipSettings::CANONICAL,
    };
    let judged = crate::compression::measure_admitted_with(
        artifact.javascript().as_bytes(),
        Objective::Brotli,
        &settings,
        &mut crate::output_budget::AllocationBudget::new(None),
    )
    .unwrap();
    assert_eq!(artifact.sizes().get(Objective::Brotli), Some(judged));
}

/// Owner answer Y5 (2026-09-29): an application script's top-level bindings
/// are the program's own unless declared global, so its roots seal as a
/// module's do. A top-level function called once with constants folds to
/// its value and goes (the catalog's `number/clamp` family, which the
/// competitor bars print as the constant).
#[test]
fn an_application_scripts_roots_are_the_programs_own() {
    let source = "int clamp(int value, int lo, int hi) { if (value < lo) { return lo; } if (value > hi) { return hi; } return value; } print(clamp(-2, 0, 10));";
    let compiled = compile_source(
        source,
        &config(""),
        ServiceOptions {
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let javascript = compiled
        .javascript(Objective::Brotli)
        .expect("the Brotli artifact")
        .javascript();
    assert_eq!(
        javascript.trim(),
        "(()=>{console.log(0);})();",
        "{javascript}"
    );
}

#[test]
fn s4_native_capabilities_are_reported_before_lowering_with_source_spans() {
    for (source, part) in [
        ("int prefix=1;JsValue value=3;print(value);", "JsValue"),
        (
            "int prefix=1;Record<int> value=record{x:3};print(value.x??0);",
            "native records",
        ),
        (
            "int prefix=1;try{print(prefix);}finally{print(2);}",
            "native exceptions",
        ),
        (
            "int prefix=1;export int answer(){return prefix;}",
            "native exported ABI",
        ),
    ] {
        for target in [ServiceTarget::Native, ServiceTarget::All] {
            let error = compile_source(
                source,
                &ProjectConfig::default(),
                ServiceOptions {
                    target,
                    ..ServiceOptions::default()
                },
            )
            .unwrap_err();
            assert_eq!(error.phase, "check", "{error:?}");
            assert!(error.message.contains(part), "{error:?}");
            let diagnostic = error.diagnostic.expect("source-owned capability error");
            assert!(
                diagnostic.span.start > 0 && diagnostic.span.end > diagnostic.span.start,
                "{diagnostic:?}"
            );
        }
    }
}

#[test]
fn s4_variadic_exports_use_real_rest_arrays_and_reflected_length() {
    let source="export int sum(int first,int... rest){int result=first;for(int i=0;i<rest.length;i++){result+=rest[i];}return result;}export auto read=(int... rest)=>rest.length;";
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "console.log(library.sum.length,library.sum(2),library.sum(2,3,4),library.read.length,library.read(),library.read(1,2,3));"),"1 2 9 0 0 3\n");
    }
}

#[test]
fn s4_forwarded_host_alias_preserves_omission_and_explicit_argument_order() {
    let source = "extern int choose(int value=7);extern int next();export int run(){auto alias=choose;int a=alias();int b=alias(next());return a+b;}";
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),
            "let events=[];globalThis.choose=function(x){events.push(['choose',arguments.length,x]);return arguments.length?x:10;};globalThis.next=()=>{events.push(['next']);return 3;};",
            "console.log(library.run());console.log(JSON.stringify(events));"),
            "13\n[[\"choose\",0,null],[\"next\"],[\"choose\",1,3]]\n");
    }
}

#[test]
fn s4_assumed_generic_and_nullable_structs_snapshot_public_fields_once() {
    let source = r#"
        struct Point{int x;}
        struct Box<T>{T value;}
        export int read(JsValue input){
            Box<Point>? snapshot=JS.assume(input);
            if(snapshot==null){return -1;}
            Box<Point> copy=snapshot;copy.value.x+=1;
            return snapshot.value.x*10+copy.value.x;
        }
        export int sum(JsValue input){
            Point?[] items=JS.assume(input);int n=0;
            for(int i=0;i<items.length;i++){Point? p=items[i];if(p!=null){n+=p.x;}}
            return n;
        }
    "#;
    let result=compile_source(source,&config(""),ServiceOptions {
        objectives:Some(Objectives::All),..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "let reads=0;const p={get x(){reads++;return 3;}};console.log(JSON.stringify([library.read(null),library.read(undefined),library.read({value:p}),reads,library.sum([null,p,undefined,{x:4}]),reads]));"),
            "[-1,-1,34,1,7,2]\n");
    }
}

#[test]
fn s4_checked_binary_reads_and_development_traps_reach_public_routes() {
    let source = r#"
        export number get(int i){Float64Array a=new Float64Array(1);a[0]=2.5;return a.get(i)??-3.0;}
        export int read(int i){Uint8Array a=new Uint8Array(1);a[0]=255;return a[i];}
        export int unit(int i){return "𐐀".codeUnitAt(i);}
    "#;
    let result=compile_source(source,&config("checks='development'"),ServiceOptions {
        objectives:Some(Objectives::All),..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const outcomes=[library.get(-1),library.get(0),library.get(1),library.read(0),library.unit(0),library.unit(1)];for(const fn of [library.read,library.unit])for(const i of [-1,9]){try{fn(i);outcomes.push('missed');}catch(e){outcomes.push(e instanceof RangeError);}}console.log(JSON.stringify(outcomes));"),
            "[-3,2.5,-3,255,55297,56320,true,true,true,true]\n");
    }
}

#[test]
fn s4_host_callbacks_keep_struct_defaults_rest_and_primitive_receivers() {
    let source = r#"
        struct Point{int x;}
        extern void observe(JsValue callback,JsValue method,JsValue nullable);
        JsValue callback=(Point p=Point{4},Point... rest)=>{
            for(int i=0;i<rest.length;i++){p.x+=rest[i].x;}return p;
        };
        JsValue method=(this int self,Point p=Point{2},Point... rest)=>{
            p.x+=self;for(int i=0;i<rest.length;i++){p.x+=rest[i].x;}return p;
        };
        JsValue nullable=(Point? ... rest)=>{
            int n=0;for(int i=0;i<rest.length;i++){Point? p=rest[i];if(p!=null){n+=p.x;}}
            return Point{n};
        };
        observe(callback,method,nullable);
    "#;
    for script in [false, true] {
        let result = compile_source(source,&config(""),ServiceOptions {
            preserve_root_exports: !script,
            objectives: Some(Objectives::All),..ServiceOptions::default()
        }).unwrap();
        for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            let javascript = result.javascript(codec).unwrap().javascript();
            let code = format!(
                "globalThis.observe=(f,g,h)=>{{const q={{x:3}};console.log(JSON.stringify([f.length,f(),f(undefined,q),g.length,g.call(5),g.call(5,q,q),h(null,q),q]));}};{javascript}"
            );
            let mut command = Command::new("node");
            if !script {command.arg("--input-type=module");}
            let output = command.args(["-e", &code]).output().unwrap();
            assert!(output.status.success(), "{}\n{javascript}", String::from_utf8_lossy(&output.stderr));
            assert_eq!(String::from_utf8(output.stdout).unwrap(),
                "[0,{\"x\":4},{\"x\":7},0,{\"x\":7},{\"x\":11},{\"x\":3},{\"x\":3}]\n");
        }
    }
}

#[test]
fn s4_dual_class_imports_and_reexports_preserve_runtime_identity() {
    let scratch = Scratch::new();
    for (file, source) in [
        ("entry.lil", r#"import {Crate} from "./barrel";
            export {Crate};export constructor Crate;
            export int read(){Crate box=new Crate(9);return box.value;}
            export async JsValue load(){auto ns=await import("./box");return ns.Box;}"#),
        ("barrel.lil", r#"import {Box as Crate} from "./box";export {Crate};export constructor Crate;"#),
        ("box.lil", "export class Box{int value;init(int value){this.value=value;}}export constructor Box;"),
    ] {
        std::fs::write(scratch.0.join(file), source).unwrap();
    }
    let result = compile_path(&scratch.0.join("entry.lil"), &config(""), ServiceOptions {
        objectives: Some(Objectives::All), ..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(), "",
            "const box=new library.Crate(4);console.log(JSON.stringify([box.value,box.constructor===library.Crate,box instanceof library.Crate,library.read(),await library.load()===library.Crate]));"),
            "[4,true,true,9,true]\n");
    }
}

#[test]
fn s4_generic_struct_schemas_keep_copies_and_public_shapes() {
    let source = r#"
        struct Box<T>{T value;}
        struct Point{int x;int y;}
        export Box<Point> change(Box<Point> original){
            Box<Point> copy=original;copy.value.x=8;return copy;
        }
        export Box<int> scalar(int value){return Box{value};}
        export Box<Point>? maybe(bool yes){
            if(yes){return Box{Point{1,2}};}return null;
        }
        export int shifts(int n){Box<Box<Box<int>>> box=Box{Box{Box{4}}};return box.value.value.value+(n>>2)+(n>>>3);}
        export int nested(){
            Box<Box<Point>> first=Box{Box{Point{3,4}}};
            Box<Box<Point>> copy=first;copy.value.value.y=9;
            return first.value.value.y*10+copy.value.value.y;
        }
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const p={value:{x:1,y:2}},q=library.change(p);console.log(JSON.stringify([p,q,p!==q,p.value!==q.value,library.scalar(7),library.maybe(false),library.maybe(true),library.nested(),library.shifts(-8)]));"),
            "[{\"value\":{\"x\":1,\"y\":2}},{\"value\":{\"x\":8,\"y\":2}},true,true,{\"value\":7},null,{\"value\":{\"x\":1,\"y\":2}},49,536870913]\n");
    }
}

#[test]
fn s4_struct_boundary_defaults_and_rest_keep_public_arity_and_value_copies() {
    let source = r#"
        struct Point{int x;}
        export int count(Point point=Point{7},Point... rest){
            int n=point.x;for(int i=0;i<rest.length;i++){n+=rest[i].x;rest[i].x=99;}return n;
        }
        export Point? same(Point? point){return point;}
        Point id<T>(Point value,T unused){return value;}
        export int privateCopy(){
            Point first=Point{4};Point copy=id(first,true);copy.x=8;return first.x*10+copy.x;
        }
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const p={x:2},q={x:3};console.log(JSON.stringify([library.count.length,library.count(),library.count(undefined,q),library.count(p,q),p,q,library.same(null),library.same(p),library.privateCopy()]));"),
            "[0,7,10,5,{\"x\":2},{\"x\":3},null,{\"x\":2},48]\n");
    }
}

#[test]
fn s4_struct_constructor_and_prototype_boundaries_keep_class_identity() {
    let source = r#"
        struct Point{int x;}
        class Box{
            int total;
            init(Point first=Point{3},Point... rest){
                this.total=first.x;for(int i=0;i<rest.length;i++){this.total+=rest[i].x;}
            }
            Point add(Point value=Point{4},Point... rest){
                value.x+=this.total;for(int i=0;i<rest.length;i++){value.x+=rest[i].x;}return value;
            }
        }
        class Derived extends Box{init(Point first=Point{2}){super(first);}}
        export constructor Box;export constructor Derived;
        export Box make(){return new Box(Point{5},Point{6});}
        export Derived derived(){return new Derived(Point{7});}
    "#;
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "let reads=0;const p={get x(){reads++;return 8;}},q={x:2},box=new library.Box(p,q),base=new library.Box(),derived=new library.Derived();console.log(JSON.stringify([library.Box.name,library.Box.length,box.constructor===library.Box,box instanceof library.Box,box.total,base.total,box.add.length,box.add(),box.add(q,q),q,reads,derived.total,derived instanceof library.Box,derived.constructor===library.Derived,library.make().total,library.derived().total]));"),
            "[\"Box\",0,true,true,10,3,0,{\"x\":14},{\"x\":14},{\"x\":2},1,2,true,true,11,7]\n");
    }
}

#[test]
fn s4_receiver_and_class_rest_exports_preserve_public_calling_conventions() {
    let source="export constructor Box;class Box{int value;init(int x=7,int... values){this.value=x;for(int i=0;i<values.length;i++){this.value+=values[i];}}int sum(int first=2,int... values){int n=this.value+first;for(int i=0;i<values.length;i++){n+=values[i];}return n;}}export auto add=(this int self,int first=2,int... values)=>{int n=self+first;for(int i=0;i<values.length;i++){n+=values[i];}return n;};";
    let result = compile_source(
        source,
        &config(""),
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const a=new library.Box(),b=new library.Box(1,2,3);console.log(library.Box.length,a.value,b.value,a.sum.length,a.sum(),a.sum(1,2,3),library.add.length,library.add.call(7),library.add.call(7,1,2,3));"),"0 7 6 0 9 13 0 9 13\n");
    }
}

#[test]
fn s4_explicit_field_contract_is_target_and_effort_independent() {
    let source="class C{int x;init(bool early){if(early){return;}this.x=3;}}print(new C(true).x);";
    for target in [ServiceTarget::JavaScript,ServiceTarget::Native,ServiceTarget::All] {
        for effort in [0,13] {
            let mut settings=ProjectConfig::default();
            settings.language.field_initialization=crate::config::FieldInitialization::Explicit;
            settings.effort.level=effort;
            let error=compile_source(source,&settings,ServiceOptions{target,preserve_root_exports:false,..ServiceOptions::default()}).unwrap_err();
            assert_eq!(error.phase,"check","{error:?}");
            assert!(error.message.contains("field `x`") && error.message.contains("R3"),"{error:?}");
            let diagnostic=error.diagnostic.unwrap();
            assert!(source[diagnostic.span.start..diagnostic.span.end].contains('x'));
        }
    }
    let valid="class C{int x;init(bool early){try{if(early){return;}}finally{this.x=3;}}}print(new C(true).x);";
    let mut settings=ProjectConfig::default();
    settings.language.field_initialization=crate::config::FieldInitialization::Explicit;
    let result=compile_source(valid,&settings,ServiceOptions{preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_javascript(result.javascript(Objective::Brotli).unwrap().javascript(),"",""),"3\n");
    let legacy=compile_source(source,&ProjectConfig::default(),ServiceOptions{preserve_root_exports:false,..ServiceOptions::default()}).unwrap();
    assert_eq!(execute_javascript(legacy.javascript(Objective::Brotli).unwrap().javascript(),"",""),"0\n");
}

#[test]
fn s4_explicit_field_contract_reaches_imported_modules_and_check_api() {
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("entry.lil"),"import {C} from \"./child\";print(new C().x);").unwrap();
    std::fs::write(scratch.0.join("child.lil"),"export class C{int x;}").unwrap();
    let settings:ProjectConfig=toml::from_str("[language]\nfield_initialization='explicit'").unwrap();
    let error=compile_path(&scratch.0.join("entry.lil"),&settings,ServiceOptions::default()).unwrap_err();
    assert_eq!(error.phase,"check");
    assert!(error.diagnostic.unwrap().path.ends_with("child.lil"));
    let error=check_path(&scratch.0.join("entry.lil"),None,&settings).unwrap_err();
    assert_eq!(error.phase,"check");
    assert!(error.message.contains("field `x`"));
}

#[test]
fn s4_field_contract_is_strictly_parsed_and_fingerprinted() {
    use crate::compilation_policy::CompilationRequest;
    let legacy=ProjectConfig::default();
    let explicit:ProjectConfig=toml::from_str("[language]\nfield_initialization='explicit'").unwrap();
    for request in [CompilationRequest::Native,CompilationRequest::JavaScript{preserve_root_exports:true}] {
        let before=legacy.resolve_policy(request.clone()).unwrap();
        let after=explicit.resolve_policy(request).unwrap();
        assert_ne!(before.fingerprint(),after.fingerprint());
        assert_eq!(after.receipt()["source_contract"]["field_initialization"],"explicit");
    }
    for bad in ["[language]\nfield_initialization='auto'","[language]\nfield_initializations='explicit'"]{
        assert!(toml::from_str::<ProjectConfig>(bad).is_err());
    }
}


#[test]
fn s4_development_bounds_survive_unused_results_and_inlined_calls() {
    let source=r#"
        void local(int i){int[] a=[1];a[i];}
        void binary(int i){Uint8Array a=new Uint8Array(1);a[i];}
        void unit(int i){"x".codeUnitAt(i);}
        export void run(int kind,int index){if(kind==0){local(index);}else if(kind==1){binary(index);}else{unit(index);}}
    "#;
    let result=compile_source(source,&config("checks='development'"),ServiceOptions {
        objectives:Some(Objectives::All),..ServiceOptions::default()
    }).unwrap();
    for codec in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
        assert_eq!(execute_javascript(result.javascript(codec).unwrap().javascript(),"",
            "const r=[];for(let kind=0;kind<3;kind++)for(const i of [0,-1,1]){try{library.run(kind,i);r.push('ok');}catch(e){r.push(e instanceof RangeError);}}console.log(JSON.stringify(r));"),
            "[\"ok\",true,true,\"ok\",true,true,\"ok\",true,true]\n");
    }
}

#[test]
fn s4_private_constructor_exports_dissolve_but_observed_values_keep_their_abi() {
    let scratch=Scratch::new();
    std::fs::write(scratch.0.join("box.lil"),
        "export class Box{int longPrivateField;init(int n){this.longPrivateField=n;}int read(int add=2){return this.longPrivateField+add;}}export constructor Box;"
    ).unwrap();
    std::fs::write(scratch.0.join("barrel.lil"),
        r#"import {Box as Crate} from "./box";export {Crate};export constructor Crate;"#
    ).unwrap();
    for observation in ["", "export JsValue ctor(){return Crate;}", "export constructor Crate;", "export async JsValue ctor(){auto ns=await import(\"./box\");return ns.Box;}"] {
        std::fs::write(scratch.0.join("entry.lil"),format!(
            "import {{Crate}} from \"./barrel\";export int run(int n){{Crate b=new Crate(n);return b.read();}}{observation}"
        )).unwrap();
        let compiled=compile_path(&scratch.0.join("entry.lil"),&config(""),ServiceOptions {
            objectives:Some(Objectives::All),..ServiceOptions::default()
        }).unwrap();
        for objective in [Objective::Raw,Objective::Gzip,Objective::Brotli] {
            let javascript=compiled.javascript(objective).unwrap().javascript();
            if observation.is_empty() {
                assert!(!javascript.contains("longPrivateField"),"{javascript}");
                assert!(!javascript.contains("class "),"{javascript}");
                assert_eq!(execute_javascript(javascript,"","console.log(library.run(5));"),"7\n");
            } else {
                assert_eq!(execute_javascript(javascript,"",
                    "const C=library.Crate??await library.ctor();const b=new C(6);console.log(JSON.stringify([library.run(5),b.longPrivateField,b.read(),b.read(4),b.constructor===C,b instanceof C,Object.keys(b),C.length,C.prototype.read.length]));"),
                    "[7,6,8,10,true,true,[\"longPrivateField\"],1,0]\n");
            }
        }
    }
}
