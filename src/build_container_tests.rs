use super::*;
fn settings(format: &str, extra: &str) -> ProjectConfig {
    toml::from_str(&format!("objective.codecs='raw'\neffort.level=0\n[policy]\nversion=3\n[target.javascript]\nformat='{format}'\n[delivery]\n{extra}")).unwrap()
}
fn node(script: &str) -> String {
    let result = Command::new("node").args(["-e", script]).output().unwrap();
    assert!(
        result.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap()
}
fn emit(result: &ServiceCompilation, directory: &Path) -> PathBuf {
    let artifact = result.javascript(Objective::Raw).unwrap();
    for file in artifact.files() {
        let path = directory.join(&file.name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, &file.code).unwrap();
    }
    let layout = artifact.layout().unwrap();
    directory.join(&artifact.files()[layout.entries[0].file as usize].name)
}
#[test]
fn d1_cjs_iife_umd_live_exports_and_strict_callbacks() {
    let source="export int count=1;export void bump(){count++;}export int read(){return count;}extern JsValue this;export bool strict(){return this===JS.undefined();}";
    for format in ["cjs", "iife", "umd"] {
        let config = settings(format, "global='Library'\n");
        let built = compile_source(source, &config, ServiceOptions::default()).unwrap();
        let scratch = Scratch::new();
        let entry = emit(&built, &scratch.0);
        let code = std::fs::read_to_string(entry).unwrap();
        let code = serde_json::to_string(&code).unwrap();
        let base = "const vm=require('node:vm');";
        let oracle="const initial=m.count;const f=m.strict;m.bump();console.log(JSON.stringify([initial,m.count,m.read(),f(),Object.keys(m).sort()]));";
        let expected = "[1,2,2,true,[\"bump\",\"count\",\"read\",\"strict\"]]\n";
        if format != "iife" {
            assert_eq!(node(&format!("{base}let context={{exports:{{}},require,module:{{exports:{{}}}}}};context.module.exports=context.exports;vm.runInNewContext({code},context);let m=context.module.exports;{oracle}")),expected);
        }
        if format != "cjs" {
            assert_eq!(node(&format!("{base}let context={{}};vm.runInNewContext({code},context);let m=context.Library;{oracle}")),expected);
        }
        if format == "umd" {
            assert_eq!(node(&format!("{base}let m;const define=(deps,f)=>m=f({{}});define.amd={{}};vm.runInNewContext({code},{{define}});{oracle}")),expected);
        }
    }
}
#[test]
fn d1_external_imports_stay_live_and_unbound_in_every_container() {
    let scratch = Scratch::new();
    let entry = scratch.0.join("entry.lil");
    std::fs::write(&entry,"import extern {value,next} from \"provider\";extern int value;extern int next();export int read(){return value;}export int run(){return next();}").unwrap();
    for format in ["cjs", "iife", "umd"] {
        let config = settings(
            format,
            "global='Library'\n[delivery.globals]\nprovider='Provider'\n",
        );
        let built = compile_path(&entry, &config, ServiceOptions::default()).unwrap();
        let code = std::fs::read_to_string(emit(&built, &scratch.0)).unwrap();
        let code = serde_json::to_string(&code).unwrap();
        let provider="const provider={value:7,next:function(){'use strict';if(this!==undefined)throw Error('bound import');return ++provider.value}};";
        let context = if format == "cjs" {
            "let context={exports:{},require:s=>provider};context.module={exports:context.exports};"
        } else {
            "let context={Provider:provider};"
        };
        let result = if format == "cjs" {
            "context.exports"
        } else {
            "context.Library"
        };
        assert_eq!(node(&format!("const vm=require('node:vm');{provider}{context}vm.runInNewContext({code},context);const m={result};console.log(m.read(),m.run(),m.read());")),"7 8 8\n");
    }
}
#[test]
fn d1_cjs_split_preserve_and_lazy_files_keep_shared_state() {
    for mode in ["split", "preserve-modules"] {
        let scratch = Scratch::new();
        std::fs::write(
            scratch.0.join("shared.lil"),
            "export int count=1;export void bump(){count++;}print(\"shared\");",
        )
        .unwrap();
        std::fs::write(
            scratch.0.join("lazy.lil"),
            "import {count} from \"./shared\";export int read(){print(\"lazy\");return count;}",
        )
        .unwrap();
        let a = scratch.0.join("a.lil");
        let b = scratch.0.join("b.lil");
        std::fs::write(&a,"import {count,bump} from \"./shared\";export {bump};export int read(){return count;}export void later(){import(\"./lazy\").then((auto m)=>print(m.read()));}").unwrap();
        std::fs::write(
            &b,
            "import {count} from \"./shared\";export int read(){return count;}",
        )
        .unwrap();
        let config = settings("cjs", &format!("mode='{mode}'"));
        let result = compile_entries(
            &[
                EntrySource {
                    name: "a".into(),
                    path: a,
                },
                EntrySource {
                    name: "b".into(),
                    path: b,
                },
            ],
            &config,
            ServiceOptions::default(),
        )
        .unwrap();
        emit(&result, &scratch.0);
        assert_eq!(node(&format!("const a=require({}),b=require({});console.log(a.read(),b.read());a.bump();console.log(a.read(),b.read());a.later();",serde_json::to_string(&scratch.0.join("a.cjs")).unwrap(),serde_json::to_string(&scratch.0.join("b.cjs")).unwrap())),"shared\n1 1\n2 2\nlazy\n2\n");
    }
}
#[test]
fn d1_module_paths_are_injective_and_preserved_files_load() {
    let scratch = Scratch::new();
    for (name, n) in [("a b", 2), ("a_b", 3), ("a%20b", 5), ("雪", 7)] {
        std::fs::write(
            scratch.0.join(format!("{name}.lil")),
            format!("print({n});export int value(){{return {n};}}"),
        )
        .unwrap();
    }
    let entry = scratch.0.join("entry.lil");
    std::fs::write(&entry,"import {value as a} from \"./a b\";import {value as b} from \"./a_b\";import {value as c} from \"./a%20b\";import {value as d} from \"./雪\";export int read(){return a()+b()+c()+d();}").unwrap();
    for format in ["esm", "cjs"] {
        let mut config = settings(format, "mode='preserve-modules'");
        config
            .policy
            .as_mut()
            .unwrap()
            .tactics
            .insert(TacticId::Inlining, TacticPermission::Off);
        let result = compile_path(&entry, &config, ServiceOptions::default()).unwrap();
        let output = scratch.0.join(format);
        std::fs::create_dir_all(&output).unwrap();
        std::fs::write(output.join("package.json"), "{\"type\":\"module\"}").unwrap();
        let entry = emit(&result, &output);
        let read = if format == "esm" {
            format!(
                "import({}).then(m=>console.log(m.read()))",
                serde_json::to_string(&entry).unwrap()
            )
        } else {
            format!(
                "console.log(require({}).read())",
                serde_json::to_string(&entry).unwrap()
            )
        };
        assert_eq!(node(&read), "2\n3\n5\n7\n17\n");
        let names = result
            .javascript(Objective::Raw)
            .unwrap()
            .files()
            .iter()
            .map(|file| file.name.as_str())
            .collect::<Vec<_>>();
        assert!(names.iter().any(|name| name.contains("a%20b")), "{names:?}");
        assert!(
            names.iter().any(|name| name.contains("a%2520b")),
            "{names:?}"
        );
    }
}

#[test]
fn d1_default_interop_and_direct_default_are_explicit() {
    let scratch = Scratch::new();
    let entry = scratch.0.join("entry.lil");
    std::fs::write(&entry,"import extern {default as add} from \"provider\";extern int add(int n);export int run(int n){return add(n);}").unwrap();
    for interop in ["node", "es-module"] {
        let config = settings("cjs", &format!("default_interop='{interop}'"));
        let built = compile_path(&entry, &config, ServiceOptions::default()).unwrap();
        let code = std::fs::read_to_string(emit(&built, &scratch.0)).unwrap();
        let provider = if interop == "node" {
            "n=>n+4"
        } else {
            "({default:n=>n+4})"
        };
        assert_eq!(node(&format!("const vm=require('node:vm');let ctx={{exports:{{}},require:s=>{provider}}};vm.runInNewContext({},ctx);console.log(ctx.exports.run(3));",serde_json::to_string(&code).unwrap())),"7\n");
    }
    let source = "int value(int n){return n+3;}export {value as default};";
    let config = settings("cjs", "exports='default'");
    let built = compile_source(source, &config, ServiceOptions::default()).unwrap();
    let entry = emit(&built, &scratch.0);
    assert_eq!(
        node(&format!(
            "console.log(require({})(4));",
            serde_json::to_string(&entry).unwrap()
        )),
        "7\n"
    );
    let error =
        compile_source("export int value=3;", &config, ServiceOptions::default()).unwrap_err();
    assert!(error.to_string().contains("exactly one default"), "{error}");
}
#[test]
fn d1_container_controls_have_distinct_identity_and_hygienic_bindings() {
    let source = "int $e=3;int $0=4;int $$r=5;export int read(){return $e+$0+$$r;}";
    let mut config = settings("iife", "global='Library'\nglobal_binding='property'");
    config
        .policy
        .as_mut()
        .unwrap()
        .tactics
        .insert(TacticId::IdentifierMangling, TacticPermission::Off);
    let built = compile_source(source, &config, ServiceOptions::default()).unwrap();
    let scratch = Scratch::new();
    let code = std::fs::read_to_string(emit(&built, &scratch.0)).unwrap();
    assert_eq!(node(&format!("const vm=require('node:vm');let c={{}};vm.runInNewContext({},c);console.log(c.Library.read());",serde_json::to_string(&code).unwrap())),"12\n");
    let before = config
        .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .fingerprint();
    config.delivery.global = Some("Different".into());
    let after = config
        .resolve_policy(crate::compilation_policy::CompilationRequest::JavaScript {
            preserve_root_exports: true,
        })
        .unwrap()
        .fingerprint();
    assert_ne!(before, after);
}
#[test]
fn d1_cjs_single_and_split_cycles_preserve_hoisted_calls() {
    let scratch = Scratch::new();
    let a = scratch.0.join("a.lil");
    let b = scratch.0.join("b.lil");
    std::fs::write(&a,"import {other} from \"./b\";export int answer(){return 41;}print(other());export int read(){return other();}").unwrap();
    std::fs::write(
        &b,
        "import {answer} from \"./a\";export int other(){return answer()+1;}",
    )
    .unwrap();
    for mode in ["single", "split"] {
        let config = settings("cjs", &format!("mode='{mode}'"));
        let built = compile_path(&a, &config, ServiceOptions::default()).unwrap();
        let entry = emit(&built, &scratch.0);
        assert_eq!(
            node(&format!(
                "console.log(require({}).read())",
                serde_json::to_string(&entry).unwrap()
            )),
            "42\n42\n"
        );
    }
}

#[test]
fn d1_library_frames_preserve_module_this_and_lexical_host_arguments() {
    let source="extern JsValue this;extern JsValue arguments;export bool empty=this===JS.undefined();export JsValue arity=arguments.length;";
    for format in ["cjs", "iife", "umd"] {
        let config = settings(format, "global='Library'");
        let built = compile_source(source, &config, ServiceOptions::default()).unwrap();
        let scratch = Scratch::new();
        let code = std::fs::read_to_string(emit(&built, &scratch.0)).unwrap();
        let surface = if format == "cjs" {
            "c.exports"
        } else {
            "c.Library"
        };
        assert_eq!(node(&format!("const vm=require('node:vm');let c={{exports:{{}},require,arguments:[1,2,3,4,5]}};vm.runInNewContext({},c);let m={surface};console.log(m.empty,m.arity);",serde_json::to_string(&code).unwrap())),"true 5\n");
    }
}

#[test]
fn d1_global_multi_entry_templates_publish_distinct_namespaces() {
    let scratch = Scratch::new();
    let a = scratch.0.join("a.lil");
    let b = scratch.0.join("b.lil");
    std::fs::write(&a, "export int read(){return 3;}").unwrap();
    std::fs::write(&b, "export int read(){return 4;}").unwrap();
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
    let config = settings("iife", "global='Library_[name]'");
    let built = compile_entries(&entries, &config, ServiceOptions::default()).unwrap();
    let codes = built
        .javascript(Objective::Raw)
        .unwrap()
        .files()
        .iter()
        .map(|file| file.code.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(node(&format!("const vm=require('node:vm');const c={{}};vm.runInNewContext({},c);console.log(c.Library_a.read(),c.Library_b.read());",serde_json::to_string(&codes).unwrap())),"3 4\n");
    assert!(compile_entries(
        &entries,
        &settings("iife", "global='Library'"),
        ServiceOptions::default()
    )
    .unwrap_err()
    .to_string()
    .contains("several global entries"));
    let reserved = [EntrySource { name: "exports".into(), path: entries[0].path.clone() }];
    assert!(compile_entries(&reserved, &settings("umd", "global='[name]'"), ServiceOptions::default())
        .unwrap_err().to_string().contains("safe publication identifier"));
}
