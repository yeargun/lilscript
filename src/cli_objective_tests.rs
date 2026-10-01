use super::*;
use lilscript::js::selection::Objective;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let path = std::env::temp_dir().join(format!(
            "lilscript-objective-cli-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, text: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, text).unwrap();
        path
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn configuration(codecs: &str, delivery: &str) -> ProjectConfig {
    lilscript::config::parse_project_config(&format!(
        "objective.codecs={codecs}\neffort.level=3\n[delivery]\nentry_names='[name].mjs'\n{delivery}"
    )).unwrap().config
}

fn arguments(scratch: &Scratch, target: &str) -> Args {
    Args::try_parse_from([
        "lilscript",
        "--target",
        target,
        "--out-dir",
        scratch.0.join("out").to_str().unwrap(),
    ])
    .unwrap()
}

fn execute(directory: &Path, source: &str) -> String {
    let output = Command::new("node")
        .current_dir(directory)
        .args(["--input-type=module", "-e", source])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

fn verify_manifest(directory: &Path, result: &ServiceCompilation, codecs: &[Objective]) -> Value {
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("lilscript.manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["version"], 4);
    assert_eq!(
        manifest["codecs"],
        json!(codecs.iter().map(|codec| codec.name()).collect::<Vec<_>>())
    );
    let outputs = manifest["outputs"].as_array().unwrap();
    assert_eq!(outputs.len(), codecs.len());
    for (output, codec) in outputs.iter().zip(codecs) {
        let artifact = result.javascript(*codec).unwrap();
        assert_eq!(output["codec"], codec.name());
        assert_eq!(
            output["policy_fingerprint"],
            artifact.details()["policy_fingerprint"]
        );
        let files = output["files"].as_array().unwrap();
        let names: Vec<_> = files
            .iter()
            .map(|file| file["file"].as_str().unwrap())
            .collect();
        let mut total = 0;
        for file in files {
            let name = file["file"].as_str().unwrap();
            assert!(name.starts_with(&format!("{}/", codec.name())));
            let bytes = fs::read(directory.join(name)).unwrap();
            assert_eq!(file["sha256"], format!("{:x}", Sha256::digest(&bytes)));
            assert_eq!(file["bytes"], bytes.len());
            let expected = if artifact.files().is_empty() {
                artifact.javascript()
            } else {
                let relative = name.strip_prefix(&format!("{}/", codec.name())).unwrap();
                &artifact
                    .files()
                    .iter()
                    .find(|file| file.name == relative)
                    .unwrap()
                    .code
            };
            assert_eq!(
                bytes,
                expected.as_bytes(),
                "writer must not transform judged bytes"
            );
            let size = lilscript::compression::measure(&bytes, *codec).unwrap();
            assert_eq!(file["codec_bytes"], size);
            total += size;
            for field in ["imports", "dynamic_imports"] {
                for linked in file[field].as_array().unwrap() {
                    assert!(names.contains(&linked.as_str().unwrap()));
                }
            }
        }
        assert_eq!(output["codec_total"], total);
        for entry in output["entries"].as_array().unwrap() {
            assert!(names.contains(&entry["file"].as_str().unwrap()));
            for file in entry["closure"].as_array().unwrap() {
                assert!(names.contains(&file.as_str().unwrap()));
            }
        }
        for file in output["side_effects"].as_array().unwrap() {
            assert!(names.contains(&file.as_str().unwrap()));
        }
    }
    manifest
}

#[test]
fn objective_cli_writes_exact_separate_winners_and_cleans_only_owned_stale_files() {
    let scratch = Scratch::new();
    let entry = scratch.write(
        "entry.lil",
        "export int answer(int value){return(value&255)+1;}",
    );
    let entries = [EntrySource {
        name: "main".into(),
        path: entry,
    }];
    let args = arguments(&scratch, "js-module");
    let options = service_options_with_environment(&args, None).unwrap();
    let config = configuration("['brotli','raw','gzip']", "");
    let result = lilscript::compile_entries(&entries, &config, options).unwrap();
    assert!(explain_human(result.report()).contains("raw, gzip, brotli bytes, effort 3"));
    write_objective_deliveries(
        &args,
        &entries,
        &config,
        &result,
        options.requested_objectives(&config).unwrap(),
    )
    .unwrap();
    let directory = scratch.0.join("out");
    verify_manifest(
        &directory,
        &result,
        &[Objective::Raw, Objective::Gzip, Objective::Brotli],
    );
    for codec in ["raw", "gzip", "brotli"] {
        assert_eq!(
            execute(
                &directory,
                &format!("const m=await import('./{codec}/main.mjs');console.log(m.answer(511));")
            ),
            "256\n"
        );
    }
    scratch.write("out/unrelated.txt", "keep");
    scratch.write("out/gzip/note.txt", "keep");
    // Switching back to one objective retains the ordinary root layout/v3
    // manifest, and removes only files listed in the previous combined one.
    let single = configuration("'raw'", "");
    build(&args, &entries, &single, options).unwrap();
    assert!(directory.join("main.mjs").is_file());
    for codec in ["raw", "gzip", "brotli"] {
        assert!(!directory.join(codec).join("main.mjs").exists());
    }
    assert_eq!(
        fs::read_to_string(directory.join("unrelated.txt")).unwrap(),
        "keep"
    );
    assert_eq!(
        fs::read_to_string(directory.join("gzip/note.txt")).unwrap(),
        "keep"
    );
    let manifest: Value =
        serde_json::from_slice(&fs::read(directory.join("lilscript.manifest.json")).unwrap())
            .unwrap();
    assert_eq!(manifest["version"], 3);
    assert_eq!(
        execute(
            &directory,
            "const m=await import('./main.mjs');console.log(m.answer(511));"
        ),
        "256\n"
    );
}

#[test]
fn objective_cli_preserves_multi_entry_lazy_and_host_imports() {
    for mode in ["split", "preserve-modules"] {
        let scratch = Scratch::new();
        scratch.write(
            "host.js",
            "console.log('host');export function add(a,b){return a+b;}",
        );
        scratch.write("shared.lil", r#"import extern {add} from "./host.js";extern int add(int a,int b);print("shared");export int value(){return add(20,22);}"#);
        scratch.write(
            "lazy.lil",
            r#"import {value} from "./shared";export int answer(){print("lazy");return value()+1;}"#,
        );
        let a = scratch.write(
            "a.lil",
            r#"import {value} from "./shared";export int run(){return value();}"#,
        );
        let b = scratch.write(
            "b.lil",
            r#"export void later(){import("./lazy").then((auto m)=>print(m.answer()));}"#,
        );
        let entries = [
            EntrySource {
                name: "a".into(),
                path: a,
            },
            EntrySource {
                name: "b".into(),
                path: b,
            },
        ];
        let args = arguments(&scratch, "js-module");
        let options = service_options_with_environment(&args, None).unwrap();
        let config = configuration("['gzip','brotli']", &format!("mode='{mode}'\nhost_modules='embed'\nchunk_names='chunks/[name]-[hash:8].mjs'\nmodule_names='[path].mjs'"));
        let result = lilscript::compile_entries(&entries, &config, options).unwrap();
        write_objective_deliveries(
            &args,
            &entries,
            &config,
            &result,
            options.requested_objectives(&config).unwrap(),
        )
        .unwrap();
        let directory = scratch.0.join("out");
        let manifest = verify_manifest(&directory, &result, &[Objective::Gzip, Objective::Brotli]);
        for output in manifest["outputs"].as_array().unwrap() {
            let entry = |name: &str| {
                output["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|entry| entry["name"] == name)
                    .unwrap()["file"]
                    .as_str()
                    .unwrap()
            };
            let a = serde_json::to_string(&format!("./{}", entry("a"))).unwrap();
            let b = serde_json::to_string(&format!("./{}", entry("b"))).unwrap();
            assert_eq!(execute(&directory, &format!("const a=await import({a});const b=await import({b});console.log(a.run());b.later();await new Promise(r=>setTimeout(r,40));")), "host\nshared\n42\nlazy\n43\n", "{mode}");
        }
    }
}

#[test]
fn objective_cli_policy_fingerprints_cover_the_set_and_ambiguous_output_fails_early() {
    let config = configuration("['gzip','raw']", "");
    let args = Args::try_parse_from([
        "lilscript",
        "missing.lil",
        "--target",
        "js-module",
        "--print-policy",
    ])
    .unwrap();
    let options = service_options_with_environment(&args, None).unwrap();
    let loaded = LoadedConfig {
        config: config.clone(),
        path: None,
        warnings: vec![],
    };
    let receipt = policy_report(&args, &loaded, options).unwrap();
    assert_eq!(receipt["objectives"], json!(["raw", "gzip"]));
    assert_ne!(receipt["fingerprint"], receipt["primary_fingerprint"]);
    let mut reordered = loaded;
    reordered.config.objective.codecs.reverse();
    assert_eq!(
        receipt["fingerprint"],
        policy_report(&args, &reordered, options).unwrap()["fingerprint"]
    );
    reordered.config.objective.gzip.level = 2;
    assert_ne!(
        receipt["fingerprint"],
        policy_report(&args, &reordered, options).unwrap()["fingerprint"]
    );
    let entries = [EntrySource {
        name: "main".into(),
        path: PathBuf::from("missing.lil"),
    }];
    assert!(build(&args, &entries, &config, options)
        .unwrap_err()
        .contains("--out-dir"));
    let scratch = Scratch::new();
    let mut args = arguments(&scratch, "js-module");
    args.output = Some(scratch.0.join("wrong.js"));
    assert!(build(&args, &entries, &config, options)
        .unwrap_err()
        .contains("no -o"));
    assert!(!scratch.0.join("out").exists());
}

#[test]
fn objective_cli_target_all_keeps_one_native_build_beside_javascript_variants() {
    let scratch = Scratch::new();
    let path = scratch.write("entry.lil", "print(5);");
    let entries = [EntrySource {
        name: "main".into(),
        path,
    }];
    let args = arguments(&scratch, "all");
    let config = configuration("['raw','brotli']", "");
    build(
        &args,
        &entries,
        &config,
        service_options_with_environment(&args, None).unwrap(),
    )
    .unwrap();
    let directory = scratch.0.join("out");
    for codec in ["raw", "brotli"] {
        assert_eq!(
            execute(&directory, &format!("await import('./{codec}/main.mjs');")),
            "5\n"
        );
    }
    assert!(directory.join("native/entry.c").is_file());
    let native = Command::new(directory.join("native/entry"))
        .output()
        .unwrap();
    assert!(
        native.status.success(),
        "{}",
        String::from_utf8_lossy(&native.stderr)
    );
    assert_eq!(native.stdout, b"5\n");
}

#[test]
fn d1_v2_stale_cleanup_preserves_unlisted_files_and_external_paths() {
    let scratch=Scratch::new();
    let old=scratch.write("out/old.cjs","old");let kept=scratch.write("out/kept.cjs","current");
    let note=scratch.write("out/note.txt","keep");let outside=scratch.write("outside.cjs","keep");
    #[cfg(unix)] std::os::unix::fs::symlink(&outside,scratch.0.join("out/link.cjs")).unwrap();
    let manifest=scratch.write("out/lilscript.manifest.json",r#"{"version":2,"chunks":[{"file":"old.cjs"},{"file":"kept.cjs"},{"file":"../outside.cjs"},{"file":"link.cjs"}]}"#);
    remove_stale_files(&scratch.0.join("out"),&manifest,&[(kept.clone(),"current")]).unwrap();
    assert!(!old.exists());assert!(kept.exists());assert!(note.exists());assert!(outside.exists());
    #[cfg(unix)] assert!(scratch.0.join("out/link.cjs").symlink_metadata().unwrap().file_type().is_symlink());
}
#[test]
fn d1_manifest_labels_are_relative_to_the_primary_entry_and_cost_names_its_leaf() {
    assert_eq!(relative_source_name(Path::new("/project/src"),Path::new("/project/src/a.lil")),"a.lil");
    assert_eq!(relative_source_name(Path::new("/project/src"),Path::new("/shared/lib.lil")),"../../shared/lib.lil");
    let error=lilscript::config::parse_project_config("[bundle.cost]\nunknown_cost=1").unwrap_err();
    assert!(error.contains("bundle.cost.unknown_cost"),"{error}");
}
#[test]
fn d1_cli_cjs_delivers_exact_files_and_uses_cjs_extension() {
    let scratch=Scratch::new();let entry=scratch.write("entry.lil","export int n=2;export void bump(){n++;}");
    let entries=[EntrySource{name:"main".into(),path:entry}];let args=arguments(&scratch,"js-module");
    let options=service_options_with_environment(&args,None).unwrap();
    let config=lilscript::config::parse_project_config("objective.codecs=['raw','gzip','brotli']\neffort.level=3\n[target.javascript]\nformat='cjs'").unwrap().config;
    let result=lilscript::compile_entries(&entries,&config,options).unwrap();
    write_objective_deliveries(&args,&entries,&config,&result,options.requested_objectives(&config).unwrap()).unwrap();
    let directory=scratch.0.join("out");verify_manifest(&directory,&result,&[Objective::Raw,Objective::Gzip,Objective::Brotli]);
    for codec in ["raw","gzip","brotli"] {
        assert!(directory.join(codec).join("main.cjs").exists());
        let output=std::process::Command::new("node").args(["-e",&format!("const m=require('./{codec}/main.cjs');m.bump();console.log(m.n)")]).current_dir(&directory).output().unwrap();
        assert!(output.status.success(),"{}",String::from_utf8_lossy(&output.stderr));assert_eq!(output.stdout,b"3\n");
    }
}
