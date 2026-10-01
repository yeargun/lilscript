use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "lilscript-build-cache-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).unwrap();
        Self(directory)
    }
    fn config(&self, extra: &str) -> ProjectConfig {
        let mut config: ProjectConfig = toml::from_str(&format!(
            "[optimization]\npreset='maximum'\n[effort]\nlevel=0\n{extra}"
        ))
        .unwrap();
        config.cache.directory = Some(self.0.join("cache"));
        config
    }
    fn file(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn same_output(a: &ServiceCompilation, b: &ServiceCompilation) {
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        match (a.javascript(codec), b.javascript(codec)) {
            (Some(a), Some(b)) => {
                assert_eq!(a.javascript(), b.javascript());
                assert_eq!(a.files(), b.files());
                assert_eq!(a.layout(), b.layout());
                assert_eq!(a.sha256(), b.sha256());
                assert_eq!(a.sizes(), b.sizes());
            }
            (None, None) => (),
            _ => panic!("objective differs"),
        }
    }
    assert_eq!(a.native_c(), b.native_c());
    assert_eq!(a.native_header(), b.native_header());
    let mut left_resources = a.report["resources"].clone();
    let mut right_resources = b.report["resources"].clone();
    // Warm codec receipts can avoid their physical scratch peak. Logical
    // admission/search and final handoff must remain exactly equal.
    left_resources
        .as_object_mut()
        .unwrap()
        .remove("peak_retained_bytes");
    right_resources
        .as_object_mut()
        .unwrap()
        .remove("peak_retained_bytes");
    assert_eq!(left_resources, right_resources);
    for key in [
        "search",
        "javascript_policy",
        "javascript_policies",
        "native_policy",
        "inputs",
    ] {
        assert_eq!(a.report[key], b.report[key], "{key}");
    }
}

#[test]
fn q2_build_cache_source_preserves_all_objectives_logical_limits_and_hard_vetoes() {
    let scratch = Scratch::new();
    let mut config = scratch.config("");
    config.effort.level = 13;
    let source = "export int calculate(int input){if(input<0){return input-1;}return input+1;}";
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let cold = compile_source(source, &config, options).unwrap();
    assert_eq!(cold.report["build_cache"]["state"], "stored");
    let warm = compile_source(source, &config, options).unwrap();
    assert_eq!(
        warm.report["build_cache"]["hit"], true,
        "cold: {:?}; warm: {:?}",
        cold.report["build_cache"], warm.report["build_cache"]
    );
    assert_eq!(warm.report["resources"], cold.report["resources"]);
    assert!(warm.report["phases_ns"]["check_ns"].is_null());
    assert_eq!(warm.report["codec_cache"]["encodes"], 0);
    let mut disabled = config.clone();
    disabled.cache.build_reuse = false;
    let off = compile_source(source, &disabled, options).unwrap();
    assert!(off.report["build_cache"].is_null());
    same_output(&cold, &warm);
    same_output(&cold, &off);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        assert_eq!(crate::build::tests::execute_javascript(warm.javascript(codec).unwrap().javascript(), "", "console.log(JSON.stringify([library.calculate(-2),library.calculate(0),library.calculate(9)]));"), "[-3,1,10]\n");
    }
    let mut veto = config.clone();
    veto.mangle.identifiers = Some(false);
    let vetoed = compile_source(source, &veto, options).unwrap();
    assert_eq!(vetoed.report["build_cache"]["hit"], false);
    let mut veto_off = veto.clone();
    veto_off.cache.build_reuse = false;
    same_output(
        &vetoed,
        &compile_source(source, &veto_off, options).unwrap(),
    );
    assert_ne!(
        cold.report["build_cache"]["key"],
        vetoed.report["build_cache"]["key"]
    );
    let tiny = ServiceOptions {
        logical_work: 8,
        ..options
    };
    let cold_error = compile_source(source, &disabled, tiny).unwrap_err();
    let warm_error = compile_source(source, &config, tiny).unwrap_err();
    assert_eq!(
        (cold_error.phase, cold_error.message),
        (warm_error.phase, warm_error.message)
    );
}

#[test]
fn q2_build_cache_transitive_graph_host_linkage_and_current_resolution_are_inputs() {
    let scratch = Scratch::new();
    let config = scratch.config("[delivery]\nhost_modules='embed'\n");
    scratch.file("leaf.lil", "export int value(){return 2;}");
    scratch.file(
        "middle.lil",
        "import {value} from \"./leaf.lil\";export int twice(){return value()*2;}",
    );
    scratch.file("host.mjs", "function f(){return 1}export{f as next}");
    let entry = scratch.file("main.lil", "import {twice} from \"./middle.lil\";import extern {next} from \"./host.mjs\";extern int next();export int result(){return twice()+next();}");
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let cold = compile_path(&entry, &config, options).unwrap();
    let warm = compile_path(&entry, &config, options).unwrap();
    assert_eq!(
        warm.report["build_cache"]["hit"], true,
        "cold: {:?}; warm: {:?}",
        cold.report["build_cache"], warm.report["build_cache"]
    );
    same_output(&cold, &warm);
    for (file, source, expected) in [
        ("leaf.lil", "export int value(){return 3;}", "7\n"),
        ("host.mjs", "function f(){return 2}export{f as next}", "8\n"),
        (
            "middle.lil",
            "import {value} from \"./other.lil\";export int twice(){return value()*2;}",
            "12\n",
        ),
    ] {
        scratch.file("other.lil", "export int value(){return 5;}");
        scratch.file(file, source);
        let changed = compile_path(&entry, &config, options).unwrap();
        assert_eq!(changed.report["build_cache"]["hit"], false, "{file}");
        let hit = compile_path(&entry, &config, options).unwrap();
        assert_eq!(hit.report["build_cache"]["hit"], true);
        same_output(&changed, &hit);
        for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
            assert_eq!(
                crate::build::tests::execute_javascript(
                    hit.javascript(codec).unwrap().javascript(),
                    "",
                    "console.log(library.result());"
                ),
                expected
            );
        }
    }
    // The body is byte-identical, but its exported name changes. A body-only
    // host digest would incorrectly hit and conceal the missing-import error.
    scratch.file("host.mjs", "function f(){return 2}export{f as old}");
    let mut off = config.clone();
    off.cache.build_reuse = false;
    let cold = compile_path(&entry, &off, options);
    let on = compile_path(&entry, &config, options);
    match (cold, on) {
        (Ok(cold), Ok(on)) => {
            assert_eq!(on.report["build_cache"]["hit"], false);
            same_output(&cold, &on);
        }
        (Err(cold), Err(on)) => assert_eq!((cold.phase, cold.message), (on.phase, on.message)),
        _ => panic!("host linkage reused stale output"),
    }
    scratch.file("other.lil", "export int value(){return \"not an int\";}");
    assert!(compile_path(&entry, &config, options).is_err());
}

#[test]
#[cfg(unix)]
fn q2_build_cache_rediscovers_retargeted_symlinks_without_editing_old_files() {
    let scratch = Scratch::new();
    let config = scratch.config("");
    let first = scratch.file("first.lil", "export int n(){return 1;}");
    let second = scratch.file("second.lil", "export int n(){return 2;}");
    let link = scratch.0.join("linked.lil");
    std::os::unix::fs::symlink(&first, &link).unwrap();
    let entry = scratch.file("main.lil", "import {n} from \"./linked.lil\";export {n};");
    let options = ServiceOptions::default();
    let one = compile_path(&entry, &config, options).unwrap();
    assert_eq!(
        compile_path(&entry, &config, options).unwrap().report["build_cache"]["hit"],
        true
    );
    std::fs::remove_file(&link).unwrap();
    std::os::unix::fs::symlink(&second, &link).unwrap();
    let two = compile_path(&entry, &config, options).unwrap();
    assert_eq!(two.report["build_cache"]["hit"], false);
    assert_ne!(
        one.report["build_cache"]["key"],
        two.report["build_cache"]["key"]
    );
    assert_eq!(
        std::fs::read_to_string(&first).unwrap(),
        "export int n(){return 1;}"
    );
    assert_eq!(
        crate::build::tests::execute_javascript(
            two.javascript(Objective::Brotli).unwrap().javascript(),
            "",
            "console.log(library.n());"
        ),
        "2\n"
    );
}

#[test]
fn q2_build_cache_multi_file_and_native_handoffs_remain_complete() {
    let scratch = Scratch::new();
    let config = scratch.config("[delivery]\nmode='preserve-modules'\n");
    scratch.file("lib.lil", "export int n(int x){return x+2;}");
    let first = scratch.file("first.lil", "import {n} from \"./lib.lil\";export {n};");
    let second = scratch.file(
        "second.lil",
        "import {n} from \"./lib.lil\";export int twice(int x){return n(x)*2;}",
    );
    let entries = [EntrySource::of(&first), EntrySource::of(&second)];
    let options = ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    };
    let cold = compile_entries(&entries, &config, options).unwrap();
    let warm =
        compile_entries(&[entries[1].clone(), entries[0].clone()], &config, options).unwrap();
    assert_eq!(
        warm.report["build_cache"]["hit"], true,
        "cold: {:?}; warm: {:?}",
        cold.report["build_cache"], warm.report["build_cache"]
    );
    same_output(&cold, &warm);
    assert!(warm.javascript(Objective::Raw).unwrap().files().len() >= 2);
    for codec in [Objective::Raw, Objective::Gzip, Objective::Brotli] {
        let artifact = warm.javascript(codec).unwrap();
        let directory = scratch.0.join(format!("out-{codec:?}"));
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
        for file in artifact.files() {
            let path = directory.join(&file.name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &file.code).unwrap();
        }
        let layout = artifact.layout().unwrap();
        let file = &artifact.files()[layout
            .entries
            .iter()
            .find(|entry| entry.name == "second")
            .unwrap()
            .file as usize]
            .name;
        let code = format!(
            "const m=await import({});console.log(m.twice(3));",
            serde_json::to_string(&format!("file://{}", directory.join(file).display())).unwrap()
        );
        let result = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &code])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"10\n");
    }
    let config = scratch.config("");
    let options = ServiceOptions {
        target: ServiceTarget::Native,
        ..ServiceOptions::default()
    };
    let source = "int plus(int n){return n+1;}print(plus(2));";
    let cold = compile_source(source, &config, options).unwrap();
    let warm = compile_source(source, &config, options).unwrap();
    assert_eq!(
        warm.report["build_cache"]["hit"], true,
        "cold: {:?}; warm: {:?}",
        cold.report["build_cache"], warm.report["build_cache"]
    );
    same_output(&cold, &warm);
    assert!(!warm.native_c().unwrap().is_empty());
    // Native public exports remain an owned language/ABI limitation. A prior
    // private-program hit cannot turn that diagnostic into cached success.
    assert!(compile_source("export int plus(int n){return n+1;}", &config, options).is_err());
}

#[test]
fn q2_build_cache_keys_cover_contracts_requests_and_disable_physical_audits() {
    let scratch = Scratch::new();
    let base = scratch.config("");
    let source = "print(1);";
    let options = ServiceOptions::default();
    let original = Request::source(source, &base, options)
        .unwrap()
        .unwrap()
        .key;
    for extra in [
        "[language]\nchar_code_at='number'\n",
        "[mangle]\nidentifiers=false\n",
        "[objective]\ncodecs='raw'\n",
        "[host.javascript]\nread='env.read'\n",
        "[policy.resources]\nlogical_work=1000000\n",
    ] {
        let config = scratch.config(extra);
        assert_ne!(
            Request::source(source, &config, options)
                .unwrap()
                .unwrap()
                .key,
            original,
            "{extra}"
        );
    }
    for options in [
        ServiceOptions {
            preserve_root_exports: false,
            ..options
        },
        ServiceOptions {
            logical_work: options.logical_work - 1,
            ..options
        },
        ServiceOptions {
            retained_bytes: options.retained_bytes - 1,
            ..options
        },
        ServiceOptions {
            chunk_extension: ChunkExtension::Mjs,
            ..options
        },
        ServiceOptions {
            target: ServiceTarget::Native,
            ..options
        },
        ServiceOptions {
            objectives: Some(Objectives::All),
            ..options
        },
    ] {
        assert_ne!(
            Request::source(source, &base, options)
                .unwrap()
                .unwrap()
                .key,
            original
        );
    }
    for kind in 0..4 {
        let mut config = base.clone();
        match kind {
            0 => config.cache.build_reuse = false,
            1 => config.cache.normalization_reuse = false,
            2 => config.cache.codec_reuse = false,
            _ => config.cache.directory = None,
        };
        assert!(Request::source(source, &config, options).unwrap().is_none());
    }
    let deadline = scratch.config("[policy.resources]\nwall_time_ms=100000\n");
    assert!(Request::source(source, &deadline, options)
        .unwrap()
        .is_none());
    let mut context = Request::source(source, &base, options).unwrap().unwrap();
    context.salt[0] ^= 1;
    context.identify(source_inputs(source));
    assert_ne!(context.key, original);
}

#[test]
fn q2_build_cache_corrupt_colliding_concurrent_and_oversized_records_are_misses() {
    let scratch = Scratch::new();
    let config = scratch.config("");
    let options = ServiceOptions::default();
    let source = "export int n(){return 3;}";
    let output = compile_source(source, &config, options).unwrap();
    let first = Request::source(source, &config, options).unwrap().unwrap();
    let mut second = Request::source(source, &config, options).unwrap().unwrap();
    second.key[1] ^= 1;
    first.write_record(&output).unwrap();
    assert!(first.read_payload().is_some());
    assert!(second.read_payload().is_none());
    second.write_record(&output).unwrap();
    assert!(first.read_payload().is_none());
    assert!(second.read_payload().is_some());
    std::thread::scope(|scope| {
        for request in [&first, &second] {
            let output = &output;
            scope.spawn(move || {
                for _ in 0..12 {
                    request.write_record(output).unwrap();
                    if let Some(payload) = request.read_payload() {
                        assert!(serde_json::from_slice::<CachedCompilation>(&payload).is_ok());
                    }
                }
            });
        }
    });
    assert!(std::fs::metadata(&first.file).unwrap().len() <= FILE_BYTES);
    first.write_record(&output).unwrap();
    let mut file = OpenOptions::new().write(true).open(&first.file).unwrap();
    file.seek(SeekFrom::Start(first.offset() + HEADER as u64))
        .unwrap();
    file.write_all(b"!").unwrap();
    assert!(first.read_payload().is_none());
    let repaired = compile_source(source, &config, options).unwrap();
    assert_eq!(repaired.report["build_cache"]["hit"], false);
    same_output(&output, &repaired);
    file.set_len(first.offset() + HEADER as u64 + 1).unwrap();
    assert!(first.read_payload().is_none());
    first.write_record(&output).unwrap();
    file.seek(SeekFrom::Start(first.offset() + 40)).unwrap();
    file.write_all(&u64::MAX.to_le_bytes()).unwrap();
    assert!(first.read_payload().is_none());
    let mut limited = Request::source(source, &config, options).unwrap().unwrap();
    limited.maximum = 8;
    assert!(limited.write_record(&output).is_err());
    let mut writer = LimitedBytes {
        bytes: Vec::new(),
        maximum: 15,
    };
    assert_eq!(writer.write(&[1; 9]).unwrap(), 9);
    assert!(writer.bytes.capacity() <= 15);
    assert!(writer.write(&[2; 7]).is_err());
    assert_eq!(writer.bytes.len(), 9);
    let mut changed = compile_source("export int n(){return 4;}", &config, options).unwrap();
    limited.write(&mut changed);
    assert_eq!(changed.report["build_cache"]["state"], "input-changed");
    // Neither a malformed DTO nor an incompatible request can publish an
    // unchecked winner index or invoke the service's indexed accessors.
    let mut malformed = serde_json::to_value(&output).unwrap();
    malformed["winners"] = json!([null, null, 99999]);
    assert!(serde_json::from_value::<CachedCompilation>(malformed)
        .unwrap()
        .restore([false, false, true], false)
        .is_none());
}

#[test]
fn q2_build_cache_io_failure_preserves_build_and_diagnostics() {
    let scratch = Scratch::new();
    let mut config = scratch.config("");
    let options = ServiceOptions::default();
    let obstacle = scratch.file("not-a-directory", "occupied");
    config.cache.directory = Some(obstacle);
    let source = "export int n(){return 1;}";
    let cached = compile_source(source, &config, options).unwrap();
    assert_eq!(cached.report["build_cache"]["state"], "not-stored");
    let mut off = config.clone();
    off.cache.build_reuse = false;
    same_output(&cached, &compile_source(source, &off, options).unwrap());
    let invalid = "export int n(){return \"text\";}";
    let a = compile_source(invalid, &config, options).unwrap_err();
    let b = compile_source(invalid, &off, options).unwrap_err();
    assert_eq!((a.phase, a.message), (b.phase, b.message));
}
