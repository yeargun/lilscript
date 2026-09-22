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
