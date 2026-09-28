//! Multi-file delivery (plan M3.3): the delivered files are the scored
//! files, each parses on its own, and loading an entry observes what the
//! single-file program observes (design §3, DL1-DL10).
use super::*;
use crate::diagnostics::render_service_error;
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
        "[javascript]\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n[delivery]\n{delivery}"
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
        "[javascript]\ncost_model='brotli'\ncandidate_proposal_limit=24\nterminal_codec_probe_limit=48\n{extra}"
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
        toml::from_str("[javascript]\n[delivery]\nhost_modules='embed'").unwrap();
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
    // Its local name is the naming allocator's (C18).
    assert!(
        text.contains("import{add as ") && text.contains("}from\"./host.ts\""),
        "{text}"
    );
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
        toml::from_str(&format!("[javascript]\necmascript='{edition}'")).unwrap()
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
/// plain call `f(...)`; a receiver helper with an effect keeps its effect,
/// before the call (the program rules inline it, and its `undefined` makes
/// that call plain too).
#[test]
fn js_call_with_an_undefined_receiver_is_a_plain_call() {
    let source = "extern JsValue target;JsValue undef(){return JS.undefined();}\
                  JsValue noisy(){print(\"receiver\");return JS.undefined();}\
                  print(JS.call(target, undef(), 1));print(JS.call(target, noisy(), 2));";
    let config: ProjectConfig = toml::from_str("").unwrap();
    let text = compile_source(source, &config, ServiceOptions::default())
        .unwrap()
        .javascript(Objective::Brotli)
        .unwrap()
        .javascript()
        .to_string();
    assert!(text.contains("target(1)"), "{text}");
    assert!(text.contains("\"receiver\""), "{text}");
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

// ---------------------------------------------------------------- M3.3a review

fn entries_in(directory: &Path, pairs: &[(&str, &str)]) -> Vec<EntrySource> {
    pairs
        .iter()
        .map(|(name, file)| EntrySource {
            name: name.to_string(),
            path: directory.join(file),
        })
        .collect()
}

fn compile_several(
    directory: &Path,
    pairs: &[(&str, &str)],
    delivery: &str,
) -> Result<ServiceCompilation, ServiceError> {
    compile_entries(&entries_in(directory, pairs), &config(delivery), options())
}

/// Lets every `import()` a load started settle before the script goes on.
const SETTLE: &str = "await new Promise(r=>setTimeout(r,20));";

/// DL3 and DL5 (design §7.6, §7.8): a lazy load evaluates what its target
/// reaches and its importer has not, in the target's own order, even when
/// another entry imports those modules statically in another order.
#[test]
fn a_lazy_load_runs_what_its_importer_has_not_in_the_target_order() {
    let directory = workspace(
        "lazy-order",
        &[
            ("y.lil", "print(\"y\");export int yv(){return 1;}"),
            ("z.lil", "print(\"z\");export int zv(){return 2;}"),
            (
                "a.lil",
                "import {zv} from \"./z\";import {yv} from \"./y\";export int a(){return zv()+yv();}",
            ),
            (
                "d.lil",
                "import {yv} from \"./y\";import {zv} from \"./z\";export int dv(){return yv()*10+zv();}",
            ),
            (
                "b.lil",
                "print(\"b\");import(\"./d\").then((auto m)=>print(m.dv()));export int bb(){return 7;}",
            ),
        ],
    );
    let pairs = [("a", "a.lil"), ("b", "b.lil")];
    for mode in ["split", "preserve-modules"] {
        let compiled = compile_several(&directory, &pairs, &format!("mode='{mode}'")).unwrap();
        each_file_loads(&directory, &compiled);
        // The sources' ES modules: `a` runs z then y; `import("./d")` runs
        // y then z, and only what is not evaluated yet.
        let script = |steps: &str| run_script(&directory, &compiled, &format!("{steps}{SETTLE}"));
        assert_eq!(script("await import('./a.js');"), "z\ny\n", "{mode}");
        assert_eq!(script("await import('./b.js');"), "b\ny\nz\n12\n", "{mode}");
        assert_eq!(
            script(&format!(
                "await import('./b.js');{SETTLE}await import('./a.js');"
            )),
            "b\ny\nz\n12\n",
            "{mode}"
        );
        assert_eq!(
            script("await import('./a.js');await import('./b.js');"),
            "z\ny\nb\n12\n",
            "{mode}"
        );
    }
    // `single` builds `import()` in place: it cannot run y and z lazily.
    let error = compile_several(&directory, &pairs, "mode='single'")
        .map(|_| ())
        .unwrap_err();
    assert!(render_service_error(&error).contains("M3.3d"), "{error}");
    let _ = fs::remove_dir_all(directory);
}

/// `import()` of a module another entry imports statically is a dynamic
/// entry for the entry that does not (design §5.2): it evaluates the module
/// when it loads, once, and an entry's module loaded so is its module file,
/// never its entry file (P7).
#[test]
fn import_of_a_module_another_entry_imports_statically_evaluates_it() {
    let directory = workspace(
        "eager-lazy",
        &[
            ("y.lil", "print(\"y\");export int yv(){return 1;}"),
            (
                "a.lil",
                "import {yv} from \"./y\";print(\"a\");export int a(){return yv();}",
            ),
            (
                "b.lil",
                "import(\"./y\").then((auto m)=>print(m.yv()));export int b(){return 2;}",
            ),
            (
                "c.lil",
                "import(\"./a\").then((auto m)=>print(m.a()));export int c(){return 3;}",
            ),
        ],
    );
    let pairs = [("a", "a.lil"), ("b", "b.lil"), ("c", "c.lil")];
    for mode in ["split", "preserve-modules"] {
        let compiled = compile_several(&directory, &pairs, &format!("mode='{mode}'")).unwrap();
        each_file_loads(&directory, &compiled);
        let script = |steps: &str| run_script(&directory, &compiled, &format!("{steps}{SETTLE}"));
        assert_eq!(script("await import('./b.js');"), "y\n1\n", "{mode}");
        assert_eq!(script("await import('./c.js');"), "y\na\n1\n", "{mode}");
        assert_eq!(
            script("await import('./a.js');await import('./b.js');"),
            "y\na\n1\n",
            "{mode}"
        );
        assert_eq!(
            script(&format!(
                "await import('./b.js');{SETTLE}await import('./a.js');"
            )),
            "y\n1\na\n",
            "{mode}"
        );
        for (name, code) in delivered(&compiled) {
            assert!(
                !code.contains("\"./a.js\""),
                "{mode}: {name} loads the entry file: {code}"
            );
        }
    }
    let error = compile_several(&directory, &pairs, "mode='single'")
        .map(|_| ())
        .unwrap_err();
    assert!(render_service_error(&error).contains("M3.3d"), "{error}");
    let _ = fs::remove_dir_all(directory);
}

/// Two entries, and two modules of one entry, reading one lazily loaded
/// module's member share it (the namespace names it once).
#[test]
fn two_importers_of_one_lazy_module_share_its_namespace() {
    let directory = workspace(
        "lazy-shared",
        &[
            ("feature.lil", "export int answer(int value){return value+2;}"),
            (
                "a.lil",
                "import(\"./feature\").then((auto m)=>print(m.answer(1)));export int a(){return 1;}",
            ),
            (
                "b.lil",
                "import(\"./feature\").then((auto m)=>print(m.answer(2)));export int b(){return 2;}",
            ),
            (
                "x.lil",
                "export void go(){import(\"./feature\").then((auto m)=>print(m.answer(1)));}",
            ),
            (
                "main.lil",
                "import {go} from \"./x\";import(\"./feature\").then((auto m)=>print(m.answer(2)));go();",
            ),
        ],
    );
    let pairs = [("a", "a.lil"), ("b", "b.lil")];
    for mode in ["split", "preserve-modules", "single"] {
        let compiled = compile_several(&directory, &pairs, &format!("mode='{mode}'")).unwrap();
        assert_eq!(
            run_script(
                &directory,
                &compiled,
                &format!("await import('./a.js');{SETTLE}await import('./b.js');{SETTLE}")
            ),
            "3\n4\n",
            "{mode}"
        );
    }
    let single = compile(&directory, "single");
    assert_eq!(run(&directory, &single), "4\n3\n");
    let _ = fs::remove_dir_all(directory);
}

/// Carried host code (design §7.9) runs once for every entry reaching a
/// module that imports it, lowered into the program or not; `single`
/// gives each entry file its own instance.
#[test]
fn carried_host_code_runs_for_every_entry_reaching_it() {
    let directory = workspace(
        "host-two",
        &[
            (
                "host.js",
                "console.log(\"host loaded\");export function add(left, right) { return left + right; }",
            ),
            (
                "a.lil",
                "import extern { add } from \"./host.js\";extern int add(int left, int right);export int a(){return add(20, 22);}",
            ),
            (
                "b.lil",
                "import extern { add } from \"./host.js\";extern int add(int left, int right);export int b(){return add(1, 2);}",
            ),
        ],
    );
    let pairs = [("a", "a.lil"), ("b", "b.lil")];
    for mode in ["split", "preserve-modules"] {
        let compiled = compile_several(
            &directory,
            &pairs,
            &format!("mode='{mode}'\nhost_modules='embed'"),
        )
        .unwrap();
        let script = |steps: &str| run_script(&directory, &compiled, steps);
        assert_eq!(
            script("const m=await import('./a.js');console.log(m.a());"),
            "host loaded\n42\n",
            "{mode}"
        );
        assert_eq!(
            script("const m=await import('./b.js');console.log(m.b());"),
            "host loaded\n3\n",
            "{mode}"
        );
        assert_eq!(
            script("const a=await import('./a.js');const b=await import('./b.js');console.log(a.a(),b.b());"),
            "host loaded\n42 3\n",
            "{mode}"
        );
    }
    let isolated =
        compile_several(&directory, &pairs, "mode='single'\nhost_modules='embed'").unwrap();
    assert_eq!(
        run_script(
            &directory,
            &isolated,
            "const a=await import('./a.js');const b=await import('./b.js');console.log(a.a(),b.b());"
        ),
        "host loaded\nhost loaded\n42 3\n"
    );
    let _ = fs::remove_dir_all(directory);
}

/// preserve-modules with the default templates (design §7.4, P7): an entry
/// module another module imports keeps an internal file named as a chunk,
/// so it never takes its entry file's name; one instance either way.
#[test]
fn preserve_modules_names_an_imported_entry_module_as_a_chunk() {
    let directory = workspace(
        "entry-imports-entry",
        &[
            (
                "a.lil",
                "print(\"a loaded\");export int f(int x){return x+1;}\
                 export class Point{int x;int y;init(int x,int y){this.x=x;this.y=y;}int sum(){return this.x+this.y;}}\
                 export constructor Point;export Point make(int x,int y){return new Point(x,y);}",
            ),
            (
                "b.lil",
                "import {f, make} from \"./a\";export {f as go, make as mk};export int both(){return f(1);}",
            ),
        ],
    );
    let pairs = [("a", "a.lil"), ("b", "b.lil")];
    for mode in ["split", "preserve-modules"] {
        let compiled = compile_several(&directory, &pairs, &format!("mode='{mode}'")).unwrap();
        each_file_loads(&directory, &compiled);
        assert_eq!(
            run_script(
                &directory,
                &compiled,
                "const b=await import('./b.js');const a=await import('./a.js');\
                 console.log(a.f===b.go,a.make===b.mk,b.mk(1,2) instanceof a.Point,b.both());"
            ),
            "a loaded\ntrue true true 2\n",
            "{mode}"
        );
    }
    let preserve = compile_several(&directory, &pairs, "mode='preserve-modules'").unwrap();
    let mut names = delivered(&preserve)
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    names.sort();
    // `a.js` is the entry file, `0.js` its module's internal file.
    assert_eq!(names, ["0.js", "a.js", "b.js"], "{names:?}");
    let _ = fs::remove_dir_all(directory);
}

/// A relative foreign import is spelled from the output directory, which
/// stands for the first entry's source directory: a file delivered in a
/// subdirectory climbs back to it first.
#[test]
fn relative_foreign_imports_resolve_from_nested_files() {
    let directory = workspace(
        "nested-foreign",
        &[("main.lil", "import {x} from \"./sub/x\";print(x());")],
    );
    fs::create_dir_all(directory.join("sub")).unwrap();
    fs::write(
        directory.join("sub/x.lil"),
        "import extern { h } from \"./helper.js\";extern int h();int base=h();export int x(){return base+1;}",
    )
    .unwrap();
    let helper = "export function h() { return 41; }";
    fs::write(directory.join("sub/helper.js"), helper).unwrap();
    let compiled = compile(&directory, "preserve-modules");
    let files = delivered(&compiled);
    let (_, nested) = files
        .iter()
        .find(|(name, _)| name == "sub/x.js")
        .unwrap_or_else(|| panic!("{files:?}"));
    assert!(nested.contains("from\"../sub/helper.js\""), "{nested}");
    let out = directory.join("out");
    let _ = fs::remove_dir_all(&out);
    for (name, code) in &files {
        let path = out.join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, code).unwrap();
    }
    fs::write(out.join("sub/helper.js"), helper).unwrap();
    fs::write(out.join("package.json"), "{\"type\":\"module\"}").unwrap();
    let output = Command::new("node")
        .arg("main.js")
        .current_dir(&out)
        .output()
        .unwrap();
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "42\n",
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = fs::remove_dir_all(directory);
}

/// `[hash:N]` names (design §8) cover every byte a file can load: a change
/// in a module a lazy file imports renames each hashed file that spells the
/// lazy file's name, never keeping a name for other bytes.
#[test]
fn hashed_names_cover_every_file_a_file_can_load() {
    let build = |constant: i32| {
        let directory = workspace(
            &format!("hashed-{constant}"),
            &[
                (
                    "u.lil",
                    &format!("export int k(int n){{if(n<=0){{return {constant};}}return k(n-1);}}"),
                ),
                (
                    "l1.lil",
                    "import {k} from \"./u\";export int one(){return k(3)+1;}",
                ),
                (
                    "l2.lil",
                    "import {k} from \"./u\";export int two(){return k(4)+2;}",
                ),
                (
                    "s.lil",
                    "export void go(){import(\"./l1\").then((auto m)=>print(m.one()));\
                     import(\"./l2\").then((auto m)=>print(m.two()));}",
                ),
                ("a.lil", "import {go} from \"./s\";export void a(){go();}"),
                ("b.lil", "import {go} from \"./s\";export void b(){go();}"),
            ],
        );
        let compiled = compile_several(
            &directory,
            &[("a", "a.lil"), ("b", "b.lil")],
            "mode='split'\nchunk_names='[hash:8].[ext]'",
        )
        .unwrap();
        let files = delivered(&compiled);
        let _ = fs::remove_dir_all(directory);
        files
    };
    let (first, second) = (build(5), build(6));
    assert!(first.len() >= 5, "{first:?}");
    let mut changed = 0;
    for (name, code) in &second {
        if name == "a.js" || name == "b.js" {
            continue;
        }
        match first.iter().find(|(known, _)| known == name) {
            Some((_, old)) => assert_eq!(old, code, "{name} kept its name for other bytes"),
            None => changed += 1,
        }
    }
    assert!(changed >= 3, "{first:?}\n{second:?}");
}

/// A dynamic entry entering a static cycle at another module than a static
/// entry would evaluate it in another order (§5.5): refused, naming both.
#[test]
fn a_lazy_load_entering_a_cycle_elsewhere_is_refused() {
    let directory = workspace(
        "lazy-cycle-entry",
        &[
            (
                "p.lil",
                "import {qv} from \"./q\";print(\"p\");export int pv(){return 1;}",
            ),
            (
                "q.lil",
                "import {pv} from \"./p\";export int qv(){return 2;}",
            ),
            (
                "a.lil",
                "import {pv} from \"./p\";export int a(){return pv();}",
            ),
            (
                "b.lil",
                "import(\"./q\").then((auto m)=>print(m.qv()));export int b(){return 0;}",
            ),
        ],
    );
    let error = compile_several(
        &directory,
        &[("a", "a.lil"), ("b", "b.lil")],
        "mode='split'",
    )
    .map(|_| ())
    .unwrap_err();
    let message = render_service_error(&error);
    assert!(
        message.contains("import(") && message.contains("cycle"),
        "{message}"
    );
    let _ = fs::remove_dir_all(directory);
}

/// Design §14.1's order and closure cases (DL2, DL3, P4, P5), per load
/// sequence, in `split` and `preserve-modules`; `single` gives each entry
/// its own instance, so each file runs everything it reaches.
#[test]
fn entries_run_their_modules_in_their_own_order_and_nothing_else() {
    let directory = workspace(
        "orders",
        &[
            ("m1.lil", "print(\"m1\");export int v1(){return 1;}"),
            ("m2.lil", "print(\"m2\");export int v2(){return 2;}"),
            ("m3.lil", "print(\"m3\");export int v3(){return 3;}"),
            // anchored_interleave: e imports m1, m2, m3; f imports m1, m3.
            (
                "e.lil",
                "import \"./m1\";import \"./m2\";import \"./m3\";export int e(){return 1;}",
            ),
            ("f.lil", "import \"./m1\";import \"./m3\";export int f(){return 2;}"),
            // entries_disagree: g imports m2 then m3, h imports m3 then m2.
            ("g.lil", "import {v2} from \"./m2\";import {v3} from \"./m3\";export int g(){return v2()+v3();}"),
            ("h.lil", "import {v3} from \"./m3\";import {v2} from \"./m2\";export int h(){return v2()*v3();}"),
            // throwing_initializer: only t reaches core, which throws.
            (
                "core.lil",
                "extern int boom();print(\"core start\");int value=boom();export int cv(){return value;}",
            ),
            ("t.lil", "import {cv} from \"./core\";export int t(){return cv()+1;}"),
            ("u.lil", "print(\"u ok\");export int u(){return 7;}"),
        ],
    );
    let cases: [(&[(&str, &str)], &[(&str, &str)]); 3] = [
        (
            &[("e", "e.lil"), ("f", "f.lil")],
            &[
                ("await import('./e.js');", "m1\nm2\nm3\n"),
                ("await import('./f.js');", "m1\nm3\n"),
                ("await import('./f.js');await import('./e.js');", "m1\nm3\nm2\n"),
                ("await import('./e.js');await import('./f.js');", "m1\nm2\nm3\n"),
            ],
        ),
        (
            &[("g", "g.lil"), ("h", "h.lil")],
            &[
                ("await import('./g.js');", "m2\nm3\n"),
                ("await import('./h.js');", "m3\nm2\n"),
                ("await import('./h.js');await import('./g.js');", "m3\nm2\n"),
            ],
        ),
        (
            &[("t", "t.lil"), ("u", "u.lil")],
            &[
                (
                    "globalThis.boom=()=>{throw 'boom'};try{await import('./t.js')}catch(e){console.log('threw='+e)}",
                    "core start\nthrew=boom\n",
                ),
                ("globalThis.boom=()=>{throw 'boom'};await import('./u.js');", "u ok\n"),
                (
                    "globalThis.boom=()=>{throw 'boom'};await import('./u.js');try{await import('./t.js')}catch(e){console.log('threw='+e)}",
                    "u ok\ncore start\nthrew=boom\n",
                ),
            ],
        ),
    ];
    for (pairs, scenarios) in cases {
        for mode in ["split", "preserve-modules"] {
            let compiled = compile_several(&directory, pairs, &format!("mode='{mode}'")).unwrap();
            for (steps, expected) in scenarios {
                assert_eq!(
                    run_script(&directory, &compiled, steps),
                    *expected,
                    "{mode} {pairs:?}: {steps}"
                );
            }
        }
    }
    let isolated = compile_several(
        &directory,
        &[("e", "e.lil"), ("f", "f.lil")],
        "mode='single'",
    )
    .unwrap();
    assert_eq!(
        run_script(
            &directory,
            &isolated,
            "await import('./f.js');await import('./e.js');"
        ),
        "m1\nm3\nm1\nm2\nm3\n"
    );
    let _ = fs::remove_dir_all(directory);
}

/// Design §14.1 `interface_hazard` (§5.4 R2): entry `b` can arm `r`'s flag
/// before `a` loads `x`, whose initializer reads it; no fact may fold the
/// read to the value `r` starts with. The guard of R2, which M3.3a leaves
/// open because no fact consumer reads across entries yet.
#[test]
fn an_interface_called_before_another_entry_loads_is_observed() {
    let directory = workspace(
        "interface-hazard",
        &[
            (
                "r.lil",
                "int flag=0;export void arm(){flag=1;}export int flagValue(){return flag;}",
            ),
            (
                "x.lil",
                "import {flagValue} from \"./r\";int seen=flagValue();export int read(){return seen;}",
            ),
            ("a.lil", "import {read} from \"./x\";export {read};"),
            ("b.lil", "import {arm} from \"./r\";export {arm};"),
        ],
    );
    let pairs = [("a", "a.lil"), ("b", "b.lil")];
    for mode in ["split", "preserve-modules"] {
        let compiled = compile_several(&directory, &pairs, &format!("mode='{mode}'")).unwrap();
        assert_eq!(
            run_script(
                &directory,
                &compiled,
                "const a=await import('./a.js');console.log(a.read());"
            ),
            "0\n",
            "{mode}"
        );
        assert_eq!(
            run_script(
                &directory,
                &compiled,
                "const b=await import('./b.js');b.arm();const a=await import('./a.js');console.log(a.read());"
            ),
            "1\n",
            "{mode}"
        );
    }
    let _ = fs::remove_dir_all(directory);
}
