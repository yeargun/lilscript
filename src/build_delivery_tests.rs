//! Multi-file delivery (plan M3.3): the delivered files are the scored
//! files, each parses on its own, and loading an entry observes what the
//! single-file program observes (design §3, DL1-DL10).
use super::*;
use crate::module::EntrySource;
use std::fs;
use std::process::Command;

fn workspace(name: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let directory =
        std::env::temp_dir().join(format!("lilscript-delivery-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&directory);
    fs::create_dir_all(&directory).unwrap();
    for (file, source) in files {
        fs::write(directory.join(file), source).unwrap();
    }
    directory
}

fn options() -> ServiceOptions {
    ServiceOptions {
        target: ServiceTarget::JavaScript,
        preserve_root_exports: true,
        ..ServiceOptions::default()
    }
}

fn config(delivery: &str) -> ProjectConfig {
    crate::config::parse_project_config(&format!(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n[delivery]\n{delivery}"
    ))
    .unwrap()
    .config
}

fn compile(directory: &Path, mode: &str) -> ServiceCompilation {
    compile_path(
        &directory.join("main.lil"),
        &config(&format!("mode='{mode}'")),
        options(),
    )
    .unwrap()
}

/// Every delivered file, by name; one file is `main.js`.
fn delivered(compiled: &ServiceCompilation) -> Vec<(String, String)> {
    let artifact = compiled.javascript(Objective::Brotli).unwrap();
    if artifact.layout().is_none() {
        return vec![("main.js".to_string(), artifact.javascript().to_string())];
    }
    artifact
        .files()
        .iter()
        .map(|file| (file.name.clone(), file.code.clone()))
        .collect()
}

/// Write the delivered files into `out/` and run `script` there as a
/// module; `entry` names the file the default script loads.
fn run_script(directory: &Path, compiled: &ServiceCompilation, script: &str) -> String {
    let out = directory.join("out");
    let _ = fs::remove_dir_all(&out);
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("package.json"), "{\"type\":\"module\"}").unwrap();
    for (name, code) in delivered(compiled) {
        let path = out.join(&name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, code).unwrap();
    }
    fs::write(out.join("probe.mjs"), script).unwrap();
    let output = Command::new("node")
        .arg(out.join("probe.mjs"))
        .current_dir(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{:?}",
        String::from_utf8_lossy(&output.stderr),
        delivered(compiled)
    );
    String::from_utf8(output.stdout).unwrap()
}

/// Run the `main` entry with a `read` host that counts its calls.
fn run(directory: &Path, compiled: &ServiceCompilation) -> String {
    run_script(
        directory,
        compiled,
        "let n=0;globalThis.read=()=>{n+=1;return 7*n};await import('./main.js');",
    )
}

/// Every file parses and loads (with its imports) on its own.
fn each_file_loads(directory: &Path, compiled: &ServiceCompilation) {
    for (name, _) in delivered(compiled) {
        run_script(
            directory,
            compiled,
            &format!("globalThis.read=()=>7;await import('./{name}');"),
        );
    }
}

/// preserve-modules (design §7.4, DL8): every module is a file, and the
/// entry's write to the library's `counter` goes through a setter the
/// library's file exports (ES imports are read-only).
#[test]
fn preserve_modules_keeps_each_module_in_its_file_and_writes_through_a_setter() {
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
    each_file_loads(&directory, &bundle);
    let files = delivered(&bundle);
    assert_eq!(
        files
            .iter()
            .map(|(name, _)| name.as_str())
            .collect::<Vec<_>>(),
        ["main.js", "lib.js"],
        "{files:?}"
    );
    let (_, main) = &files[0];
    assert!(main.contains("from\"./lib.js\""), "{main}");
    let artifact = bundle.javascript(Objective::Brotli).unwrap();
    let layout = artifact.layout().unwrap();
    assert_eq!(layout.files.len(), 2);
    // The scored bytes are exactly the delivered files (one entry: its row
    // holds every file once).
    let total: usize = files.iter().map(|(_, code)| code.len()).sum();
    assert_eq!(artifact.sizes().raw, total);
    let _ = fs::remove_dir_all(directory);
}

/// A static cycle among delivered files is refused in M3.3a: ES modules
/// reproduce it only with a hoisting constraint the tree does not carry yet.
#[test]
fn preserve_modules_refuses_a_static_cycle() {
    let directory = workspace(
        "cycle",
        &[
            (
                "lib.lil",
                "import {base} from \"./main\";export int twice(int x){int t=0;for(int i=0;i<x;i++){t=t+base(i);}return t;}",
            ),
            (
                "main.lil",
                "extern int read();import {twice} from \"./lib\";export int base(int x){return x+read();}print(twice(3));print(twice(4));",
            ),
        ],
    );
    let single = compile(&directory, "single");
    assert_eq!(run(&directory, &single), "45\n160\n");
    let error = compile_path(
        &directory.join("main.lil"),
        &config("mode='preserve-modules'"),
        options(),
    )
    .map(|_| ())
    .unwrap_err();
    assert!(error.to_string().contains("M3.3d"), "{error}");
    // `split` has one file per label, and one entry means one label: the
    // cycle stays inside one file.
    let bundle = compile(&directory, "split");
    assert_eq!(run(&directory, &bundle), "45\n160\n");
    let _ = fs::remove_dir_all(directory);
}

/// A function reading its module's state stays in that module's file.
#[test]
fn functions_that_read_module_state_stay_with_it() {
    let directory = workspace(
        "reader",
        &[
            (
                "lib.lil",
                "extern int read();int base=read();export int offset(int x){return x+base;}",
            ),
            (
                "main.lil",
                "import {offset} from \"./lib\";print(offset(4));",
            ),
        ],
    );
    let single = compile(&directory, "single");
    for mode in ["preserve-modules", "split"] {
        let bundle = compile(&directory, mode);
        assert_eq!(run(&directory, &bundle), run(&directory, &single), "{mode}");
        each_file_loads(&directory, &bundle);
    }
    let _ = fs::remove_dir_all(directory);
}

/// Names and bytes are a function of sources, configuration and binary
/// (DL10): the same build twice, and the entries table in another order.
#[test]
fn delivery_names_and_bytes_are_deterministic() {
    let directory = workspace(
        "determinism",
        &[
            (
                "math.lil",
                "export int square(int x){return x*x;}export int cube(int x){return x*square(x);}",
            ),
            (
                "left.lil",
                "extern int read();import {cube} from \"./math\";export int left(){return cube(read());}",
            ),
            (
                "right.lil",
                "import {square} from \"./math\";export int right(int x){return square(x)+1;}",
            ),
            ("main.lil", "print(1);"),
        ],
    );
    let build = |entries: &[(&str, &str)]| {
        let entries = entries
            .iter()
            .map(|(name, file)| EntrySource {
                name: name.to_string(),
                path: directory.join(file),
            })
            .collect::<Vec<_>>();
        let compiled = compile_entries(&entries, &config("mode='split'"), options()).unwrap();
        delivered(&compiled)
    };
    let first = build(&[("left", "left.lil"), ("right", "right.lil")]);
    let second = build(&[("left", "left.lil"), ("right", "right.lil")]);
    let reordered = build(&[("right", "right.lil"), ("left", "left.lil")]);
    assert_eq!(first, second);
    assert_eq!(first, reordered);
    assert!(first.iter().any(|(name, _)| name == "left.js"));
    assert!(first.iter().any(|(name, _)| name == "right.js"));
    let _ = fs::remove_dir_all(directory);
}

/// Two entries over one module (DL1, DL3, DL6): code both reach is one
/// file both facades import, so its state is one instance whichever loads
/// first; an entry never runs a module it does not reach.
#[test]
fn two_entries_share_one_instance_of_what_both_reach() {
    let directory = workspace(
        "shared",
        &[
            (
                "core.lil",
                "int count=0;print(\"core loaded\");export void bump(){count=count+1;}export int read(){return count;}",
            ),
            (
                "a.lil",
                "import {bump,read} from \"./core\";export int twice(){bump();bump();return read();}export {bump,read};",
            ),
            (
                "b.lil",
                "import {read} from \"./core\";print(\"b loaded\");export int onlyB(){return 7;}export {read};",
            ),
        ],
    );
    let entries = [("a", "a.lil"), ("b", "b.lil")]
        .iter()
        .map(|(name, file)| EntrySource {
            name: name.to_string(),
            path: directory.join(file),
        })
        .collect::<Vec<_>>();
    let compiled = compile_entries(&entries, &config("mode='split'"), options()).unwrap();
    each_file_loads(&directory, &compiled);
    assert_eq!(
        run_script(
            &directory,
            &compiled,
            "const b=await import('./b.js');const a=await import('./a.js');\
             console.log(b.read(),a.twice(),b.read(),a.read===b.read,b.onlyB());"
        ),
        "core loaded\nb loaded\n0 2 2 true 7\n"
    );
    assert_eq!(
        run_script(&directory, &compiled, "await import('./a.js');"),
        "core loaded\n"
    );
    let artifact = compiled.javascript(Objective::Brotli).unwrap();
    let layout = artifact.layout().unwrap();
    // One row per entry; each file is in the rows of the entries loading it.
    assert_eq!(layout.entries.len(), 2);
    let sizes = artifact
        .files()
        .iter()
        .map(|file| file.code.len())
        .collect::<Vec<_>>();
    let rows = layout.rows(&sizes);
    assert_eq!(artifact.sizes().raw as u64, rows.iter().sum::<u64>());
    // `single` with several entries: isolated instances (DL1's exception).
    let isolated = compile_entries(&entries, &config("mode='single'"), options()).unwrap();
    assert_eq!(delivered(&isolated).len(), 2);
    assert_eq!(
        run_script(
            &directory,
            &isolated,
            "const a=await import('./a.js');const b=await import('./b.js');console.log(a.twice(),b.read());"
        ),
        "core loaded\ncore loaded\nb loaded\n2 0\n"
    );
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

const LAZY_MAIN: &str = "import(\"./feature\").then((auto feature)=>print(feature.answer(40)))\
    .catch((auto error)=>print(error.message));";

/// `import()` of a module nothing imports statically loads its own file,
/// which exports exactly the namespace members some code reads.
#[test]
fn dynamic_import_loads_a_lazy_file_serving_only_the_members_read() {
    let directory = lazy_workspace("lazy", LAZY_MAIN);
    let single = compile(&directory, "single");
    assert_eq!(run(&directory, &single), "42\n");
    let single_text = single
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(!single_text.contains("99"), "{single_text}");
    for (delivery, preloaded) in [
        ("mode='preserve-modules'", false),
        ("mode='split'\npreload='entry'", true),
    ] {
        let compiled =
            crate::build::compile_path(&directory.join("main.lil"), &config(delivery), options())
                .unwrap();
        assert_eq!(run(&directory, &compiled), "42\n");
        let files = delivered(&compiled);
        assert_eq!(files.len(), 2, "{files:?}");
        let (lazy_name, lazy) = &files[1];
        assert!(lazy_name.ends_with(".js"), "{lazy_name}");
        // The importer receives the namespace, so the member keeps its name.
        assert!(
            lazy.contains("export{answer}") || lazy.contains(" as answer}"),
            "{lazy}"
        );
        assert!(!lazy.contains("99"), "{lazy}");
        let (_, main) = &files[0];
        assert_eq!(main.contains("modulepreload"), preloaded, "{main}");
        assert!(
            main.contains(&format!("import(\"./{lazy_name}\")")),
            "{main}"
        );
        // A failed load rejects with the source specifier and a message.
        let out = directory.join("out");
        fs::remove_file(out.join(lazy_name)).unwrap();
        let output = Command::new("node")
            .arg("-e")
            .arg("import('./main.js').then(()=>new Promise(r=>setTimeout(r,10)))")
            .current_dir(&out)
            .output()
            .unwrap();
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(printed.contains("ERR_MODULE_NOT_FOUND"), "{printed}");
    }
    let _ = fs::remove_dir_all(directory);
}

/// A lazy module that calls into the entry module (design §7.8, the old
/// `tests/bundles/lazy-cycle`): what both need is never in the facade,
/// so the lazy file imports a shared file, not the entry.
#[test]
fn a_lazy_module_that_calls_the_entry_module_never_imports_the_facade() {
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
    let bundle = compile(&directory, "split");
    assert_eq!(run(&directory, &bundle), "42\n");
    for (name, code) in delivered(&bundle) {
        if name != "main.js" {
            assert!(!code.contains("main.js"), "{name}: {code}");
        }
    }
    let _ = fs::remove_dir_all(directory);
}

/// Two lazily loaded modules: a file each.
#[test]
fn every_lazily_loaded_module_gets_its_file() {
    let directory = workspace(
        "lazy-two",
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
    let two = compile(&directory, "split");
    assert_eq!(delivered(&two).len(), 3, "{:?}", delivered(&two));
    assert_eq!(run(&directory, &two), "1\n2\n");
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn lazy_modules_must_be_initialization_free() {
    let directory = workspace(
        "lazy-init",
        &[
            (
                "feature.lil",
                "int seed=read();extern int read();export int answer(){return seed;}",
            ),
            (
                "main.lil",
                "import(\"./feature\").then((auto feature)=>print(feature.answer()));",
            ),
        ],
    );
    let error = compile_path(
        &directory.join("main.lil"),
        &config(""),
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

fn host_workspace(name: &str, host: &str) -> std::path::PathBuf {
    workspace(
        name,
        &[
            (
                "math.ts",
                "export function sum(left: number, right: number): number {\n  return left + right\n}\n",
            ),
            ("host.ts", host),
            (
                "main.lil",
                "import extern { add } from \"./host.ts\";extern int add(int left, int right);print(add(20, 22));",
            ),
        ],
    )
}

const HOST: &str = "import { sum } from \"./math.ts\"\n// Adds.\nexport function add(left: number, right?: number): number {\n  return sum(left, right as number)\n}\n";

fn with_config(directory: &Path, extra: &str) -> Result<ServiceCompilation, ServiceError> {
    let config: ProjectConfig = toml::from_str(&format!(
        "[javascript]\nstrip_console=false\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{extra}"
    ))
    .unwrap();
    compile_path(
        &directory.join("main.lil"),
        &config,
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: true,
            ..ServiceOptions::default()
        },
    )
}

/// A relative TypeScript host module and the host modules it imports travel
/// with the output, type-stripped and compacted, evaluated once each.
#[test]
fn relative_host_modules_travel_with_the_output() {
    let directory = host_workspace("host", HOST);
    for mode in ["single", "preserve-modules", "split"] {
        let compiled = with_config(
            &directory,
            &format!("[delivery]\nmode='{mode}'\nhost_modules='embed'"),
        )
        .unwrap();
        let files = delivered(&compiled);
        for (_, text) in &files {
            assert!(!text.contains("number") && !text.contains("Adds"), "{text}");
            assert!(!text.contains('\n'), "{text}");
        }
        if files.len() == 1 {
            assert!(!files[0].1.contains("import"), "{}", files[0].1);
        }
        assert_eq!(run(&directory, &compiled), "42\n");
    }
    // A script output runs host code strict, as the module it was written as.
    let config: ProjectConfig =
        toml::from_str("[javascript]\nstrip_console=false\n[delivery]\nhost_modules='embed'")
            .unwrap();
    let script = compile_path(
        &directory.join("main.lil"),
        &config,
        ServiceOptions {
            target: ServiceTarget::JavaScript,
            preserve_root_exports: false,
            ..ServiceOptions::default()
        },
    )
    .unwrap();
    let text = script
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(text.contains("\"use strict\""), "{text}");
    let output = Command::new("node").args(["-e", &text]).output().unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "42\n", "{text}");
    let _ = fs::remove_dir_all(directory);
}

#[test]
fn host_modules_that_cannot_travel_stay_imports_unless_embedding_is_required() {
    // A runtime TypeScript construct has no erasable form.
    let directory = host_workspace(
        "host-enum",
        "export enum Mode { A }\nexport function add(left: number, right: number): number { return left + right }\n",
    );
    let auto = with_config(&directory, "[delivery]\nhost_modules='auto'").unwrap();
    let text = auto
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(text.contains("from\"./host.ts\""), "{text}");
    let error = with_config(&directory, "[delivery]\nhost_modules='embed'").unwrap_err();
    assert!(error.message.contains("enum"), "{error}");
    let _ = fs::remove_dir_all(&directory);
    // Syntax newer than the target edition stays with the importer's toolchain.
    let directory = host_workspace(
        "host-edition",
        "export function add(left: number, right: number): number { return (left ?? 0) + right }\n",
    );
    let old = with_config(
        &directory,
        "ecmascript='es2019'\n[delivery]\nhost_modules='auto'",
    )
    .unwrap();
    let text = old
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(text.contains("from\"./host.ts\""), "{text}");
    let current = with_config(&directory, "[delivery]\nhost_modules='auto'").unwrap();
    assert!(!current
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .contains("import"));
    // `external`, the default, imports every host module from its own specifier.
    let external = with_config(&directory, "").unwrap();
    let text = external
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(text.contains("import{add}from\"./host.ts\""), "{text}");
    let _ = fs::remove_dir_all(directory);
}

/// Embedded host modules of a strict output become program code: what the
/// program does not reach is gone, one-line wrappers inline, and the rest
/// runs as the module did.
#[test]
fn embedded_host_modules_become_program_code_in_a_strict_output() {
    let directory = workspace(
        "host-lowered",
        &[
            (
                "host.ts",
                "const offset: number = 2\n\
                 export function callMethod1(obj: any, name: string, arg: any): any { return obj[name](arg) }\n\
                 export function kind(value: any): string { return value instanceof Map ? \"map\" : typeof value }\n\
                 export function getProp(obj: any, key: string): any { return obj?.[key] }\n\
                 export function sumTo(n: number): number { let total = 0; for (let i = 0; i < n; i++) { total += i } return total + offset }\n\
                 export function safe(): string { try { return (null as any).x } catch (e) { return \"caught\" } }\n\
                 export function neverUsedByTheProgram(): string { return \"UNUSED_MARKER\" }\n",
            ),
            (
                "main.lil",
                "import extern { callMethod1, kind, getProp, sumTo, safe } from \"./host.ts\";\
                 extern JsValue callMethod1(JsValue obj, string name, JsValue arg);\
                 extern string kind(JsValue value);\
                 extern JsValue getProp(JsValue obj, string key);\
                 extern int sumTo(int n);\
                 extern string safe();\
                 print(callMethod1(JS.array(1, 2, 3), \"indexOf\", 2));\
                 print(kind(JS.object()));\
                 print(getProp(JS.object(), \"x\"));\
                 print(sumTo(4));\
                 print(safe());",
            ),
        ],
    );
    let compiled = with_config(&directory, "[delivery]\nhost_modules='embed'").unwrap();
    let text = compiled
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    // No module is carried as text, and nothing the program leaves unused.
    assert!(
        !text.contains("(()=>") && !text.contains("UNUSED_MARKER"),
        "{text}"
    );
    assert!(text.contains(".indexOf(2)"), "{text}");
    assert_eq!(
        run(&directory, &compiled),
        "1\nobject\nundefined\n8\ncaught\n"
    );
    let _ = fs::remove_dir_all(directory);
}

/// Across modules, values nothing reads are gone and every effect stays,
/// including a module imported only for its effects.
#[test]
fn liveness_across_modules_drops_unread_values_and_keeps_effects() {
    let directory = workspace(
        "liveness",
        &[
            (
                "lib.lil",
                "extern int read();int plain=5;int effect=read();int[] table=[1,2,3];\
                 export int used(int x){return x+1;}export int unused(int x){return x*99;}",
            ),
            (
                "side.lil",
                "extern int read();int touched=read();export int nothing(){return 0;}",
            ),
            (
                "main.lil",
                "import \"./side\";import {used} from \"./lib\";print(used(41));",
            ),
        ],
    );
    let compiled = compile(&directory, "single");
    let text = compiled
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(
        !text.contains("99") && !text.contains("[1,2,3]") && !text.contains('5'),
        "{text}"
    );
    assert_eq!(text.matches("read()").count(), 2, "{text}");
    // Each module's effects run once, in initialization order.
    assert_eq!(run(&directory, &compiled), "42\n");
    let _ = fs::remove_dir_all(directory);
}

/// Two local compactions: an exception nothing reads needs no binding, and
/// `new X` constructs as `new X()` does where no call or member follows.
#[test]
fn unread_exceptions_and_argumentless_constructions_print_short() {
    let source = "extern int read();int total=0;try{if(read()>0){throw \"x\";}}catch(auto e){total=total+1;}\
                  Map<string,int> map=new Map<string,int>();map.set(\"a\",total);print(map.size);";
    let config = |edition: &str| -> ProjectConfig {
        toml::from_str(&format!(
            "[javascript]\nstrip_console=false\necmascript='{edition}'"
        ))
        .unwrap()
    };
    let text = |edition: &str| {
        compile_source(source, &config(edition), ServiceOptions::default())
            .unwrap()
            .javascript(Objective::Brotli)
            .unwrap()
            .javascript()
            .to_string()
    };
    let current = text("es2022");
    assert!(current.contains("catch{"), "{current}");
    assert!(
        current.contains("new Map;") || current.contains("new Map,"),
        "{current}"
    );
    // Before ES2019 a catch clause needs its binding.
    let old = text("es2017");
    assert!(old.contains("catch("), "{old}");
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!("globalThis.read=()=>1;{current}"),
        ])
        .output()
        .unwrap();
    assert_eq!(String::from_utf8_lossy(&output.stdout), "1\n", "{current}");
}

/// `JS.call(f, undef(), ...)` whose `undef` only returns undefined is the
/// plain call `f(...)`; a receiver helper with an effect keeps its call.
#[test]
fn js_call_with_an_undefined_receiver_is_a_plain_call() {
    let source = "extern JsValue target;JsValue undef(){return JS.undefined();}\
                  JsValue noisy(){print(\"receiver\");return JS.undefined();}\
                  print(JS.call(target, undef(), 1));print(JS.call(target, noisy(), 2));";
    let config: ProjectConfig = toml::from_str("[javascript]\nstrip_console=false").unwrap();
    let text = compile_source(source, &config, ServiceOptions::default())
        .unwrap()
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(text.contains("target(1)"), "{text}");
    assert_eq!(text.matches(".call(").count(), 1, "{text}");
    let output = Command::new("node")
        .args([
            "--input-type=module",
            "-e",
            &format!("globalThis.target=function(n){{return [this===undefined,n].join()}};{text}"),
        ])
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "true,1\nreceiver\ntrue,2\n",
        "{text}"
    );
}
