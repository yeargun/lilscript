use super::*;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-choices-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn config(&self, level: u8) -> ProjectConfig {
        let mut config: ProjectConfig = toml::from_str(&format!(
            "[optimization]\npreset='maximum'\n[effort]\nlevel={level}\n"
        ))
        .unwrap();
        config.decisions.write = Some(self.0.join("choices.lock"));
        config
    }
    fn document(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.0.join("choices.lock")).unwrap()).unwrap()
    }
    fn save(&self, document: &Value) {
        std::fs::write(
            self.0.join("choices.lock"),
            serde_json::to_vec(document).unwrap(),
        )
        .unwrap();
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
const SOURCE: &str = "export int calculate(int input){if(input<0){return input-1;}return input+1;}";
const CODECS: [Objective; 3] = [Objective::Raw, Objective::Gzip, Objective::Brotli];
fn all() -> ServiceOptions {
    ServiceOptions {
        objectives: Some(Objectives::All),
        ..ServiceOptions::default()
    }
}
fn reading(config: &ProjectConfig) -> ProjectConfig {
    let mut config = config.clone();
    config.decisions.read = config.decisions.write.take();
    config
}
fn check_runtime(output: &ServiceCompilation) {
    for codec in CODECS {
        let artifact = output.javascript(codec).unwrap();
        assert_eq!(crate::build::tests::execute_javascript(artifact.javascript(), "", "console.log(JSON.stringify([library.calculate(-3),library.calculate(0),library.calculate(9)]));"), "[-4,1,10]\n");
        assert_eq!(
            artifact.sizes().get(codec).unwrap(),
            crate::compression::measure(artifact.javascript().as_bytes(), codec).unwrap()
        );
    }
}

#[test]
fn q2_decision_lock_rejudges_all_objectives_and_rewrites_atomically() {
    let scratch = Scratch::new();
    let mut config = scratch.config(13);
    config.cache.directory = Some(scratch.0.join("cache"));
    let cold = compile_source(SOURCE, &config, all()).unwrap();
    assert_eq!(cold.report["decisions"]["write"], "written");
    assert!(
        cold.report["build_cache"].is_null(),
        "explicit lock IO cannot be a complete-build hit"
    );
    let document: Document = serde_json::from_value(scratch.document()).unwrap();
    assert!(document.assignments.iter().all(Option::is_some));
    for assignment in document.assignments.iter().flatten() {
        let round: SavedDecision =
            serde_json::from_value(serde_json::to_value(assignment).unwrap()).unwrap();
        assert_eq!(assignment, &round);
    }
    config.decisions.read = config.decisions.write.clone();
    let replay = compile_source(SOURCE, &config, all()).unwrap();
    assert_eq!(replay.report["decisions"]["read"], "matched");
    assert_eq!(replay.report["decisions"]["write"], "written");
    for (slot, codec) in CODECS.into_iter().enumerate() {
        let report = &replay.report["search"]["terminal"]["objectives"][slot];
        assert_eq!(report["starts"][0]["name"], "decision-lock");
        assert!(report["decision_error"].is_null(), "{report}");
        assert!(
            matches!(
                report["starts"][0]["outcome"].as_str(),
                Some("kept" | "identical" | "rejected")
            ),
            "{report}"
        );
        assert!(
            replay.javascript(codec).unwrap().sizes().get(codec)
                <= cold.javascript(codec).unwrap().sizes().get(codec)
        );
    }
    check_runtime(&replay);
    assert!(std::fs::read_dir(&scratch.0).unwrap().all(|file| !file
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));
}

#[test]
fn q2_decision_lock_complete_identity_rejects_source_contract_effort_and_limit_changes() {
    let scratch = Scratch::new();
    let config = scratch.config(0);
    compile_source(SOURCE, &config, all()).unwrap();
    let mut config = reading(&config);
    let original = Request::new(&config, all()).unwrap();
    let mut keys = Vec::new();
    for change in 0..5 {
        let mut changed = config.clone();
        let mut options = all();
        let mut input = source_inputs(SOURCE);
        match change {
            0 => input = source_inputs(&SOURCE.replace("input+1", "input+2")),
            1 => changed.mangle.identifiers = Some(false),
            2 => changed.effort.level = 1,
            3 => options.logical_work -= 1,
            _ => options.preserve_root_exports = false,
        }
        let mut request = Request::new(&changed, options).unwrap();
        request.identify(&input);
        assert_eq!(request.report["reason"], "fingerprint-mismatch");
        keys.push(request.fingerprint);
    }
    config.cache.codec_reuse = false;
    let mut physical = Request::new(&config, all()).unwrap();
    physical.identify(&source_inputs(SOURCE));
    assert_eq!(physical.report["read"], "matched");
    assert_eq!(
        original.salt, physical.salt,
        "physical caches do not grant choices"
    );
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), 5);
}

#[test]
fn q2_decision_lock_untrusted_assignments_cannot_override_toml_or_panic() {
    let scratch = Scratch::new();
    let mut config = scratch.config(1);
    config.mangle.identifiers = Some(false);
    compile_source(SOURCE, &config, all()).unwrap();
    let clean = scratch.document();
    let config = reading(&config);
    for change in 0..4 {
        let mut document = clean.clone();
        for assignment in document["assignments"].as_array_mut().unwrap() {
            match change {
                0 => assignment["naming"]["style"] = json!("Global"),
                1 => assignment["recipe"] = json!([3, 1, u32::MAX, 0, 0, 0, 0, 0, 0, 0]),
                2 => assignment["recipe"] = json!([3, 0, 0, 0, 0, 0, 77]),
                _ => assignment["naming"]["alphabet"] = json!("a".repeat(54)),
            }
        }
        scratch.save(&document);
        let result = compile_source(SOURCE, &config, all()).unwrap();
        if change == 3 {
            assert_eq!(result.report["decisions"]["reason"], "invalid-document");
        } else {
            assert_eq!(result.report["decisions"]["read"], "matched");
            for row in result.report["search"]["terminal"]["objectives"]
                .as_array()
                .unwrap()
            {
                assert_eq!(row["starts"][0]["outcome"], "refused", "{row}");
                assert!(row["decision_error"].is_string());
            }
        }
        check_runtime(&result);
    }
}

#[test]
fn q2_decision_lock_zero_effort_io_bounds_and_failed_write_keep_the_previous_file() {
    let scratch = Scratch::new();
    let config = scratch.config(0);
    let cold = compile_source(SOURCE, &config, all()).unwrap();
    let original = std::fs::read(scratch.0.join("choices.lock")).unwrap();
    let read = reading(&config);
    let replay = compile_source(SOURCE, &read, all()).unwrap();
    assert_eq!(replay.report["decisions"]["read"], "matched");
    assert_eq!(
        replay.report["search"], cold.report["search"],
        "effort zero grants no optional replay"
    );
    for codec in CODECS {
        assert_eq!(
            replay.javascript(codec).unwrap().javascript(),
            cold.javascript(codec).unwrap().javascript()
        );
    }
    let mut request = Request::new(&config, all()).unwrap();
    request.identify(&source_inputs(SOURCE));
    request.maximum = 10;
    let mut output = cold;
    assert!(request.finish(&mut output).is_err());
    assert_eq!(
        std::fs::read(scratch.0.join("choices.lock")).unwrap(),
        original
    );
    for text in [vec![b'x'; MAXIMUM + 1], b"{truncated".to_vec()] {
        std::fs::write(scratch.0.join("choices.lock"), text).unwrap();
        let mut request = Request::new(&read, all()).unwrap();
        request.identify(&source_inputs(SOURCE));
        assert_eq!(request.report["read"], "miss");
    }
    std::fs::remove_file(scratch.0.join("choices.lock")).unwrap();
    let result = compile_source(SOURCE, &read, all()).unwrap();
    assert_eq!(result.report["decisions"]["reason"], "unreadable");
    let mut bad = config.clone();
    bad.decisions.write = Some(scratch.0.join("missing/choices.lock"));
    assert!(compile_source(SOURCE, &bad, all())
        .unwrap_err()
        .to_string()
        .contains("decisions write"));
}

#[test]
fn q2_decision_lock_current_module_graph_and_multifile_delivery() {
    let scratch = Scratch::new();
    let mut config = scratch.config(1);
    config.delivery.mode = crate::config::DeliveryMode::PreserveModules;
    std::fs::write(
        scratch.0.join("value.lil"),
        "export int stored=3;export void set(int value){stored=value;}",
    )
    .unwrap();
    std::fs::write(
        scratch.0.join("main.lil"),
        "import {stored,set} from \"./value.lil\";export int next(){set(stored+1);return stored;}",
    )
    .unwrap();
    let entry = scratch.0.join("main.lil");
    let cold = compile_path(&entry, &config, all()).unwrap();
    let config = reading(&config);
    let replay = compile_path(&entry, &config, all()).unwrap();
    assert_eq!(replay.report["decisions"]["read"], "matched");
    for codec in CODECS {
        let artifact = replay.javascript(codec).unwrap();
        assert!(artifact.files().len() > 1);
        assert_eq!(artifact.files(), cold.javascript(codec).unwrap().files());
        let directory = scratch.0.join(codec.name());
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("package.json"), "{\"type\":\"module\"}").unwrap();
        for file in artifact.files() {
            let path = directory.join(&file.name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, &file.code).unwrap();
        }
        let layout = artifact.layout().unwrap();
        let file = &artifact.files()[layout.entries[0].file as usize].name;
        let command = format!("import{{next}}from './{file}';console.log(next(),next());");
        let result = std::process::Command::new("node")
            .args(["--input-type=module", "-e", &command])
            .current_dir(&directory)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8(result.stdout).unwrap(), "4 5\n");
    }
    std::fs::write(
        scratch.0.join("value.lil"),
        "export int stored=9;export void set(int value){stored=value;}",
    )
    .unwrap();
    let changed = compile_path(&entry, &config, all()).unwrap();
    assert_eq!(
        changed.report["decisions"]["reason"],
        "fingerprint-mismatch"
    );
}

#[test]
fn q2_decision_lock_config_is_strict_relative_and_never_native() {
    assert!(crate::config::parse_project_config("[decisions]\nread=false").is_err());
    assert!(crate::config::parse_project_config("[decisions]\nunknown='file'").is_err());
    let mut config: ProjectConfig =
        toml::from_str("[decisions]\nread='saved.lock'\nwrite='new.lock'").unwrap();
    config.config_dir = Some(PathBuf::from("/project"));
    let request = Request::new(&config, all()).unwrap();
    assert_eq!(
        request.read.as_deref(),
        Some(Path::new("/project/saved.lock"))
    );
    assert_eq!(
        request.write.as_deref(),
        Some(Path::new("/project/new.lock"))
    );
    assert!(Request::new(
        &config,
        ServiceOptions {
            target: ServiceTarget::Native,
            ..all()
        }
    )
    .is_err());
    config.decisions.read = Some(PathBuf::new());
    assert!(Request::new(&config, all()).is_err());
}

#[test]
fn q2_decision_lock_optional_work_refusal_keeps_the_admitted_baseline() {
    let scratch = Scratch::new();
    let config = scratch.config(13);
    let options = ServiceOptions {
        objectives: Some(Objectives::One(Objective::Raw)),
        ..ServiceOptions::default()
    };
    let full = compile_source(SOURCE, &config, options).unwrap();
    let baseline_work = full.report["resources"]["baseline_work"].as_u64().unwrap();
    let limited = ServiceOptions {
        logical_work: baseline_work + 1,
        ..options
    };
    let cold = compile_source(SOURCE, &config, limited).unwrap();
    let replay = compile_source(SOURCE, &reading(&config), limited).unwrap();
    assert_eq!(replay.report["decisions"]["read"], "matched");
    let terminal = &replay.report["search"]["terminal"]["objectives"][0];
    assert_eq!(terminal["starts"][0]["outcome"], "stopped", "{terminal}");
    assert!(
        replay.report["resources"]["optional_work"]
            .as_u64()
            .unwrap()
            <= 1
    );
    assert_eq!(
        replay.javascript(Objective::Raw).unwrap().javascript(),
        cold.javascript(Objective::Raw).unwrap().javascript()
    );
}
