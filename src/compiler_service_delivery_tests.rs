//! Multi-file delivery on the semantic route (008-D1): the delivered files
//! are the scored files, each parses and loads on its own, and loading the
//! entry observes exactly what the single-file program observes.
use super::*;
use std::fs;
use std::process::Command;

fn workspace(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let directory = std::env::temp_dir().join(format!(
        "lilscript-delivery-{name}-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    for (file, source) in files {
        fs::write(directory.join(file), source).unwrap();
    }
    directory
}

fn compile(directory: &Path, mode: &str) -> ServiceCompilation {
    let config: ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n[bundle]\nmode='{mode}'"
    ))
    .unwrap();
    compile_path_semantic(
        &directory.join("main.lil"),
        &config,
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: true,
            ..ServiceOptions::default()
        },
    )
    .unwrap()
}

/// Write the delivered files into `out/` and run the entry as a module.
fn run(directory: &Path, compiled: &ServiceCompilation) -> String {
    let artifact = compiled.javascript(Objective::Brotli).unwrap();
    let out = directory.join("out");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("entry.js"), artifact.javascript()).unwrap();
    for chunk in artifact.chunks() {
        fs::write(out.join(&chunk.name), &chunk.code).unwrap();
    }
    for chunk in artifact.chunks() {
        // Every chunk parses and loads without the entry.
        let loaded = Command::new("node")
            .args(["--input-type=module", "-e"])
            .arg(format!(
                "globalThis.read=()=>7;await import({});",
                serde_json::to_string(out.join(&chunk.name).to_str().unwrap()).unwrap()
            ))
            .output()
            .unwrap();
        assert!(
            loaded.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&loaded.stderr),
            chunk.code
        );
    }
    let output = Command::new("node")
        .args(["--input-type=module", "-e"])
        .arg(format!(
            "let n=0;globalThis.read=()=>{{n+=1;return 7*n}};await import({});",
            serde_json::to_string(out.join("entry.js").to_str().unwrap()).unwrap()
        ))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stderr),
        artifact.javascript()
    );
    String::from_utf8(output.stdout).unwrap()
}

fn delivered(compiled: &ServiceCompilation) -> Vec<(String, String)> {
    let artifact = compiled.javascript(Objective::Brotli).unwrap();
    std::iter::once(("entry.js".to_string(), artifact.javascript().to_string()))
        .chain(
            artifact
                .chunks()
                .iter()
                .map(|chunk| (chunk.name.clone(), chunk.code.clone())),
        )
        .collect()
}

#[test]
fn preserve_modules_moves_self_contained_functions_into_module_chunks() {
    let directory = workspace(
        "state",
        &[
            (
                "lib.lil",
                "extern int read();export int counter=read();\
                 export void bump(){counter=counter+read();}\
                 export int twice(int x){for(int i=0;i<2;i++){x=x+read();}return x;}",
            ),
            (
                "main.lil",
                "import {counter,bump,twice} from \"./lib\";bump();counter=counter+twice(3);print(counter);",
            ),
        ],
    );
    let single = compile(&directory, "single");
    let bundle = compile(&directory, "preserve-modules");
    let expected = run(&directory, &single);
    assert_eq!(run(&directory, &bundle), expected);
    let artifact = bundle.javascript(Objective::Brotli).unwrap();
    // Only the pure function moves; the state and its mutator stay.
    assert_eq!(artifact.chunks().len(), 1, "{:?}", delivered(&bundle));
    let chunk = &artifact.chunks()[0];
    assert!(chunk.name.starts_with("chunk-") && chunk.name.ends_with("-lib.js"));
    assert!(chunk.code.contains("export{"), "{}", chunk.code);
    assert!(!chunk.code.contains("import"), "{}", chunk.code);
    assert!(artifact.javascript().contains(&format!("from\"./{}\"", chunk.name)));
    assert_eq!(artifact.entry_dependencies(), [chunk.name.clone()]);
    // The scored bytes are exactly the delivered files.
    let total: usize = delivered(&bundle).iter().map(|(_, code)| code.len()).sum();
    assert_eq!(artifact.sizes().raw, total);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn mutually_recursive_module_functions_import_each_other_across_chunks() {
    let directory = workspace(
        "cycle",
        &[
            (
                "even.lil",
                "import {isOdd} from \"./odd\";export bool isEven(int n){if(n==0){return true;}return isOdd(n-1);}",
            ),
            (
                "odd.lil",
                "import {isEven} from \"./even\";export bool isOdd(int n){if(n==0){return false;}return isEven(n-1);}",
            ),
            (
                "main.lil",
                "extern int read();import {isEven} from \"./even\";import {isOdd} from \"./odd\";\
                 int value=read();print(isEven(value));print(isOdd(value+1));",
            ),
        ],
    );
    let single = compile(&directory, "single");
    let bundle = compile(&directory, "preserve-modules");
    assert_eq!(run(&directory, &bundle), run(&directory, &single));
    let artifact = bundle.javascript(Objective::Brotli).unwrap();
    assert_eq!(artifact.chunks().len(), 2, "{:?}", delivered(&bundle));
    for chunk in artifact.chunks() {
        assert_eq!(chunk.dependencies.len(), 1, "{}", chunk.code);
        assert!(!chunk.dependencies.contains(&"entry.js".to_string()));
    }
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn functions_that_read_module_state_stay_in_the_entry() {
    let directory = workspace(
        "reader",
        &[
            (
                "lib.lil",
                "extern int read();int base=read();export int offset(int x){return x+base;}",
            ),
            ("main.lil", "import {offset} from \"./lib\";print(offset(4));"),
        ],
    );
    let single = compile(&directory, "single");
    let bundle = compile(&directory, "preserve-modules");
    assert_eq!(run(&directory, &bundle), run(&directory, &single));
    assert!(bundle
        .javascript(Objective::Brotli)
        .unwrap()
        .chunks()
        .is_empty());
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn bundle_delivery_names_and_bytes_are_deterministic() {
    let directory = workspace(
        "determinism",
        &[
            (
                "math.lil",
                "export int square(int x){return x*x;}export int cube(int x){return x*square(x);}",
            ),
            (
                "main.lil",
                "extern int read();import {cube} from \"./math\";print(cube(read()));",
            ),
        ],
    );
    let first = delivered(&compile(&directory, "preserve-modules"));
    let second = delivered(&compile(&directory, "preserve-modules"));
    assert_eq!(first, second);
    assert_eq!(first.len(), 2);
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn functions_that_call_into_the_entry_module_stay_in_the_entry() {
    // `twice` reads `base`, which the entry module declares; moving it would
    // make a chunk import the entry.
    let directory = workspace(
        "entry-cycle",
        &[
            (
                "lib.lil",
                "import {base} from \"./main\";export int twice(int x){return base(x)*2;}",
            ),
            (
                "main.lil",
                "import {twice} from \"./lib\";export int base(int x){return x+1;}print(twice(3));",
            ),
        ],
    );
    let single = compile(&directory, "single");
    let bundle = compile(&directory, "preserve-modules");
    assert_eq!(run(&directory, &bundle), run(&directory, &single));
    assert!(bundle
        .javascript(Objective::Brotli)
        .unwrap()
        .chunks()
        .is_empty());
    let _ = fs::remove_dir_all(directory);
}

fn split(directory: &Path, min_chunk_bytes: usize, cost: &str) -> ServiceCompilation {
    let config: ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n\
         [bundle]\nmode='split'\nmin_chunk_bytes={min_chunk_bytes}\nmax_chunks=1\nshared_min_imports=2\n\
         [bundle.cost]\nraw_weight=1\ngzip_weight=0\nbrotli_weight=0\ndependency_depth_penalty_bytes=0\n{cost}"
    ))
    .unwrap();
    compile_path_semantic(
        &directory.join("main.lil"),
        &config,
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: true,
            ..ServiceOptions::default()
        },
    )
    .unwrap()
}

/// The default route's split rule: only a module that several modules
/// import, whose chunk has the minimum size and lowers the deploy cost.
#[test]
fn split_keeps_a_shared_module_chunk_only_when_it_lowers_the_deploy_cost() {
    let directory = workspace(
        "split",
        &[
            // Large enough that moving it out saves more than its import line.
            (
                "shared.lil",
                "export int shared(int value){int total=0;for(int i=0;i<value;i++){\
                 if(i%3==0){total=total+i*value;}else{total=total-i;}}return total+value;}",
            ),
            (
                "left.lil",
                "import {shared} from \"./shared\";export int left(){return shared(2);}",
            ),
            (
                "right.lil",
                "import {shared} from \"./shared\";export int right(){return shared(3);}",
            ),
            (
                "main.lil",
                "import {left} from \"./left\";import {right} from \"./right\";print(left()+right());",
            ),
        ],
    );
    let single = compile(&directory, "single");
    let expected = run(&directory, &single);
    // Cache reuse outweighs the request: the shared module is split out.
    let reused = split(
        &directory,
        1,
        "request_overhead_bytes=0\ncache_reuse_discount_percent=100",
    );
    assert_eq!(run(&directory, &reused), expected);
    let artifact = reused.javascript(Objective::Brotli).unwrap();
    assert_eq!(artifact.chunks().len(), 1, "{:?}", delivered(&reused));
    let chunk = &artifact.chunks()[0];
    assert!(chunk.name.ends_with("-shared.js"), "{}", chunk.name);
    assert_eq!(chunk.importers, 2);
    let total: usize = delivered(&reused).iter().map(|(_, code)| code.len()).sum();
    assert_eq!(artifact.sizes().raw, total);
    // A request that costs more than the reuse saves keeps one file.
    let costly = split(
        &directory,
        1,
        "request_overhead_bytes=1000000\ncache_reuse_discount_percent=0",
    );
    assert!(costly
        .javascript(Objective::Brotli)
        .unwrap()
        .chunks()
        .is_empty());
    assert_eq!(run(&directory, &costly), expected);
    // Below the minimum chunk size, nothing is a candidate.
    let small = split(
        &directory,
        100_000,
        "request_overhead_bytes=0\ncache_reuse_discount_percent=100",
    );
    assert!(small
        .javascript(Objective::Brotli)
        .unwrap()
        .chunks()
        .is_empty());
    let _ = fs::remove_dir_all(directory);
}

fn lazy_workspace(name: &str, main: &str) -> std::path::PathBuf {
    workspace(
        name,
        &[
            (
                "feature.lil",
                "export int answer(int value){return value+2;}export int unused(int value){return value*99;}",
            ),
            ("main.lil", main),
        ],
    )
}

fn with_bundle(directory: &Path, bundle: &str) -> ServiceCompilation {
    let config: ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n[bundle]\n{bundle}"
    ))
    .unwrap();
    compile_path_semantic(
        &directory.join("main.lil"),
        &config,
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: true,
            ..ServiceOptions::default()
        },
    )
    .unwrap()
}

const LAZY_MAIN: &str = "import(\"./feature\").then((auto feature)=>print(feature.answer(40)))\
    .catch((auto error)=>print(error.message));";

/// `import()` of a module nothing imports statically loads its own chunk,
/// which exports exactly the namespace members some code reads.
#[test]
fn dynamic_import_loads_a_lazy_chunk_serving_only_the_members_read() {
    let directory = lazy_workspace("lazy", LAZY_MAIN);
    let single = compile(&directory, "single");
    assert_eq!(run(&directory, &single), "42\n");
    let single_text = single.javascript(Objective::Brotli).unwrap().javascript().to_string();
    assert!(!single_text.contains("99"), "{single_text}");
    for (bundle, preloaded) in [
        ("mode='preserve-modules'", false),
        ("mode='split'\nmin_chunk_bytes=1\nmax_chunks=1\npreload='entry'", true),
    ] {
        let compiled = with_bundle(&directory, bundle);
        assert_eq!(run(&directory, &compiled), "42\n");
        let artifact = compiled.javascript(Objective::Brotli).unwrap();
        assert_eq!(artifact.chunks().len(), 1, "{:?}", delivered(&compiled));
        let chunk = &artifact.chunks()[0];
        assert!(chunk.lazy && chunk.name.ends_with("-feature.js"), "{}", chunk.name);
        assert!(chunk.code.contains("as answer}"), "{}", chunk.code);
        assert!(!chunk.code.contains("99"), "{}", chunk.code);
        let links = artifact.entry_links();
        assert!(links.dependencies.is_empty());
        assert_eq!(links.dynamic_dependencies, [chunk.name.clone()]);
        assert_eq!(links.preload.len(), usize::from(preloaded));
        assert_eq!(artifact.javascript().contains("modulepreload"), preloaded);
        assert!(artifact
            .javascript()
            .contains(&format!("import(\"./{}\")", chunk.name)));
        // A failed load rejects with the source specifier and a message.
        let out = directory.join("out");
        fs::remove_file(out.join(&chunk.name)).unwrap();
        let output = Command::new("node")
            .args(["--input-type=module", "-e"])
            .arg(format!(
                "await import({});await new Promise(r=>setTimeout(r,10));",
                serde_json::to_string(out.join("entry.js").to_str().unwrap()).unwrap()
            ))
            .output()
            .unwrap();
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(printed.contains("ERR_MODULE_NOT_FOUND"), "{printed}");
    }
    let _ = fs::remove_dir_all(directory);
}

/// A lazy module that calls into the entry module cannot move without
/// importing the entry, so its namespace is built in place.
#[test]
fn a_lazy_module_that_calls_the_entry_loads_in_place() {
    let directory = workspace(
        "lazy-cycle",
        &[
            (
                "feature.lil",
                "import {base} from \"./main\";export int run(){return base()+2;}",
            ),
            (
                "main.lil",
                "export int base(){return 40;}import(\"./feature\").then((auto feature)=>print(feature.run()));",
            ),
        ],
    );
    let single = compile(&directory, "single");
    assert_eq!(run(&directory, &single), "42\n");
    let bundle = with_bundle(&directory, "mode='split'\nmin_chunk_bytes=1\nmax_chunks=8");
    assert_eq!(run(&directory, &bundle), "42\n");
    let artifact = bundle.javascript(Objective::Brotli).unwrap();
    assert!(artifact.chunks().is_empty(), "{:?}", delivered(&bundle));
    assert!(artifact.javascript().contains(".resolve().then("));
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn split_refuses_more_lazy_chunks_than_max_chunks() {
    let directory = workspace(
        "lazy-limit",
        &[
            ("first.lil", "export int answer(){return 1;}"),
            ("second.lil", "export int answer(){return 2;}"),
            (
                "main.lil",
                "import(\"./first\").then((auto first)=>print(first.answer()));\
                 import(\"./second\").then((auto second)=>print(second.answer()));",
            ),
        ],
    );
    let config: ProjectConfig = toml::from_str(
        "[javascript]\nstrip_console=false\n[bundle]\nmode='split'\nmin_chunk_bytes=1\nmax_chunks=1",
    )
    .unwrap();
    let error = compile_path_semantic(
        &directory.join("main.lil"),
        &config,
        ServiceOptions::default(),
    )
    .unwrap_err();
    assert!(error.message.contains("bundle.max_chunks"), "{error}");
    let two = with_bundle(&directory, "mode='split'\nmin_chunk_bytes=1\nmax_chunks=2");
    assert_eq!(two.javascript(Objective::Brotli).unwrap().chunks().len(), 2);
    assert_eq!(run(&directory, &two), "1\n2\n");
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn lazy_modules_must_be_initialization_free() {
    let directory = workspace(
        "lazy-init",
        &[
            ("feature.lil", "int seed=read();extern int read();export int answer(){return seed;}"),
            (
                "main.lil",
                "import(\"./feature\").then((auto feature)=>print(feature.answer()));",
            ),
        ],
    );
    let config: ProjectConfig = toml::from_str("[javascript]\nstrip_console=false").unwrap();
    let error = compile_path_semantic(
        &directory.join("main.lil"),
        &config,
        ServiceOptions::default(),
    )
    .unwrap_err();
    assert!(error.to_string().contains("initialization-free"), "{error}");
    let _ = fs::remove_dir_all(directory);
}

/// `then`, `catch` and `finally` chain host tasks with checked callbacks.
#[test]
fn tasks_chain_then_catch_and_finally() {
    let directory = workspace(
        "tasks",
        &[(
            "main.lil",
            "Task<int> ok=Task.resolve(5);\
             ok.then((int value)=>print(value+1)).finally(()=>print(\"done\"));\
             Task<int> failed=Task.reject(JS.object(\"reason\",\"no\"));\
             failed.catch((auto error)=>print(\"caught\"));",
        )],
    );
    let single = compile(&directory, "single");
    assert_eq!(run(&directory, &single), "6\ncaught\ndone\n");
    let _ = fs::remove_dir_all(directory);
}
