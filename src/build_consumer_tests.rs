use super::*;
use crate::module::EntrySource;
use std::{fs, process::Command};

struct Workspace(std::path::PathBuf);
impl Workspace {
    fn new(label: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("lilscript-d2-{label}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        fs::write(path.join("package.json"), "{\"type\":\"module\"}").unwrap();
        Self(path)
    }
    fn source(&self, name: &str, source: &str) -> EntrySource {
        let path = self.0.join(format!("{name}.lil"));
        fs::write(&path, source).unwrap();
        EntrySource {
            name: name.into(),
            path,
        }
    }
    fn execute(&self, compilation: &ServiceCompilation, probe: &str) {
        for file in compilation.javascript(Objective::Raw).unwrap().files() {
            let path = self.0.join(&file.name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, &file.code).unwrap();
        }
        fs::write(self.0.join("probe.mjs"), probe).unwrap();
        let output = Command::new("node")
            .arg(self.0.join("probe.mjs"))
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{:?}",
            String::from_utf8_lossy(&output.stderr),
            compilation.javascript(Objective::Raw).unwrap().files()
        );
        assert_eq!(output.stdout, b"ok\n");
    }
}
impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn config(mode: &str, format: &str, effort: u8) -> ProjectConfig {
    toml::from_str(&format!("objective.codecs='raw'\neffort.level={effort}\n[policy]\nversion=3\n[target.javascript]\nformat='{format}'\n[delivery]\nmode='{mode}'\n")).unwrap()
}

#[test]
fn d2_entries_can_be_consumed_between_other_entry_initializers() {
    for mode in ["split", "preserve-modules"] {
        for effort in [0, 13] {
            let work = Workspace::new(&format!("entry-hazard-{mode}-{effort}"));
            work.source(
                "state",
                "export int flag=0;export void arm(){flag=1;}export int flagValue(){return flag;}",
            );
            work.source("x", "import {flagValue} from \"./state\";int seen=flagValue();export int read(){return seen;}");
            let a = work.source("a", "import {read} from \"./x\";export {read};");
            let b = work.source("b", "import {arm} from \"./state\";export {arm};");
            let built = compile_entries(
                &[a, b],
                &config(mode, "esm", effort),
                ServiceOptions::default(),
            )
            .unwrap();
            work.execute(&built, "const b=await import('./b.js');b.arm();const a=await import('./a.js');if(a.read()!==1)throw Error('interleaved interface');console.log('ok');");
            // A fresh process, with the opposite load/consumer sequence.
            work.execute(&built, "const a=await import('./a.js');const b=await import('./b.js');b.arm();if(a.read()!==0)throw Error('initialization repeated');console.log('ok');");
        }
    }
}

#[test]
fn d2_const_product_graph_views_share_aliases_across_entries() {
    for mode in ["split", "preserve-modules", "single"] {
        for format in ["esm", "cjs"] {
            let work = Workspace::new(&format!("graph-{mode}-{format}"));
            work.source("data", "export struct Point{int x;}export const Point[] values=[Point{7}];export const Record<Point[]> table=record{left:values,right:values};export int read(){return values[0].x+(table[\"left\"]??values)[0].x;}");
            let a = work.source("a", "import {values,read} from \"./data\";export {values,read};");
            let b = work.source("b", "import {values,table} from \"./data\";export {values,table};");
            let built = compile_entries(
                &[a, b],
                &config(mode, format, 13),
                ServiceOptions::default(),
            )
            .unwrap();
            let load = if format == "esm" {
                "const b=await import('./b.js');const a=await import('./a.js');"
            } else {
                "import{createRequire}from'node:module';const require=createRequire(import.meta.url);const b=require('./b.cjs');const a=require('./a.cjs');"
            };
            let identity = if mode == "single" {
                "if(a.values===b.values)throw Error('instances not isolated');"
            } else {
                "if(a.values!==b.values)throw Error('shared instance lost');"
            };
            work.execute(&built, &format!("{load}{identity}if(b.values!==b.table.left||b.table.left!==b.table.right||b.values[0].x!==7||a.read()!==14)throw Error('graph');for(const v of [a.values,b.values,b.values[0],b.table])if(!Object.isFrozen(v))throw Error('freeze');console.log('ok');"));
        }
    }
}

#[test]
fn d2_consumer_annotations_follow_proofs_and_explicit_controls() {
    let source = "int[] make(int n){return [n,n+1];}export int[] data=make(3);export int answer(int n){return n+1;}export int side(){print(99);return 0;}export int effect=side();";
    for (annotations, calls, functions) in [
        ("off", false, false),
        ("calls", true, false),
        ("functions", false, true),
        ("all", true, true),
    ] {
        let mut config = config("single", "esm", 0);
        config.optimization.constant_folding = Some(false);
        config.optimization.inlining = Some(false);
        config.delivery.annotations =
            toml::from_str::<ProjectConfig>(&format!("[delivery]\nannotations='{annotations}'"))
                .unwrap()
                .delivery
                .annotations;
        let built = compile_source(source, &config, ServiceOptions::default()).unwrap();
        let artifact = built.javascript(Objective::Raw).unwrap();
        let code = if artifact.files().is_empty() { artifact.javascript().to_owned() } else { artifact.files().iter().map(|file|file.code.as_str()).collect::<Vec<_>>().join("\n") };
        assert_eq!(
            code.contains("/*#__PURE__*/"),
            calls,
            "{annotations}: {code}"
        );
        assert_eq!(
            code.contains("/*#__NO_SIDE_EFFECTS__*/"),
            functions,
            "{annotations}: {code}"
        );
        if calls {
            assert_eq!(
                code.matches("/*#__PURE__*/").count(),
                1,
                "effectful call annotated: {code}"
            );
        }
        if let Some(layout) = artifact.layout() {
            assert!(layout.files[0].anchored, "{annotations}: {layout:?}\n{code}");
        }
    }
    let mut forbidden = config("single", "cjs", 0);
    forbidden.delivery.annotations = crate::config::ConsumerAnnotations::All;
    assert!(
        compile_source(source, &forbidden, ServiceOptions::default())
            .unwrap_err()
            .to_string()
            .contains("ESM library")
    );
}

#[test]
fn d2_side_effect_metadata_includes_dependencies_and_foreign_modules() {
    let work = Workspace::new("effects");
    work.source("effects", "print(41);export int value=3;");
    let a = work.source("a", "import {value} from \"./effects\";export {value};");
    let b = work.source(
        "b",
        "import extern {value} from \"external\";extern int value;export int read(){return value;}",
    );
    let pure = work.source("pure", "export int read(int n){return n+2;}");
    let built = compile_entries(
        &[a, b, pure],
        &config("preserve-modules", "esm", 0),
        ServiceOptions::default(),
    )
    .unwrap();
    let artifact = built.javascript(Objective::Raw).unwrap();
    let layout = artifact.layout().unwrap();
    for entry in &layout.entries {
        assert_eq!(
            layout.files[entry.file as usize].anchored,
            entry.name != "pure",
            "{entry:?}"
        );
    }
}

#[test]
fn d2_cycles_keep_entry_order_hoisting_and_colliding_public_names() {
    for mode in ["single", "split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
            for effort in [0, 13] {
                let work = Workspace::new(&format!("cycles-{mode}-{format}-{effort}"));
                let a = work.source("a", "import {read as other} from \"./b\";extern void note(string tag);note(other());note(\"a\");export string read(){return \"a\";}");
                let b = work.source("b", "import {read as other} from \"./a\";extern void note(string tag);note(other());note(\"b\");export string read(){return \"b\";}");
                let built = compile_entries(&[a,b], &config(mode, format, effort), ServiceOptions::default()).unwrap_or_else(|error| panic!("{mode}/{format}/{effort}: {error:?}"));
                for first in ["a", "b"] {
                    let second = if first == "a" { "b" } else { "a" };
                    let setup = if format == "esm" { "const load=name=>import('./'+name+'.js');" }
                        else { "const{createRequire}=await import('node:module');const require=createRequire(import.meta.url);const load=name=>Promise.resolve(require('./'+name+'.cjs'));" };
                    let expected = if first == "a" { "a,b,b,a" } else { "b,a,a,b" };
                    let after = if mode == "single" { "a,b,b,a,b,a,a,b" } else { expected };
                    let after = if mode == "single" && first == "b" { "b,a,a,b,a,b,b,a" } else { after };
                    work.execute(&built, &format!("let log=[];globalThis.note=x=>log.push(x);{setup}const x=await load('{first}');if(log.join()!=='{expected}'||x.read.name!=='read'||x.read()!=='{first}')throw Error('first '+log);const y=await load('{second}');if(log.join()!=='{after}'||y.read.name!=='read'||y.read()!=='{second}')throw Error('second '+log);console.log('ok');"));
                }
            }
        }
    }
}


#[test]
fn d2_lazy_effects_live_namespaces_and_failures_run_once() {
    for mode in ["single", "split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
            for effort in [0, 13] {
                let work = Workspace::new(&format!("lazy-effects-{mode}-{format}-{effort}"));
                work.source("state", "extern int seed();export int value=seed();export void bump(){value=value+1;}");
                work.source("feature", "import {value,bump} from \"./state\";extern void note(string s);note(\"feature\");export {value,bump};export int read(){return value;}");
                work.source("broken", "extern int explode();int value=explode();export int read(){return value;}");
                let main = work.source("main", "extern void note(string s);note(\"main\");export async JsValue load(){auto m=await import(\"./feature\");return m;}export async int fail(){auto m=await import(\"./broken\");return m.read();}");
                let built = compile_entries(&[main], &config(mode,format,effort), ServiceOptions::default())
                    .unwrap_or_else(|error| panic!("{mode}/{format}/{effort}: {error}"));
                let load = if format == "esm" { "const m=await import('./main.js');" }
                    else { "import{createRequire}from'node:module';const m=createRequire(import.meta.url)('./main.cjs');" };
                work.execute(&built, &format!("let log=[],seeds=0,failures=0;globalThis.note=s=>log.push(s);globalThis.seed=()=>{{seeds++;return 4}};globalThis.explode=()=>{{failures++;throw Error('boom')}};{load}if(log.join()!= 'main'||seeds)throw Error('eager work');const [a,b]=await Promise.all([m.load(),m.load()]);if(a!==b||a.value!==4||a.read()!==4||seeds!==1||log.join()!=='main,feature')throw Error('load identity/order');a.bump();if(b.value!==5||b.read()!==5)throw Error('live namespace');const errors=await Promise.all([m.fail().catch(e=>e),m.fail().catch(e=>e)]);if(failures!==1||errors.some(e=>!e.message.includes('boom')||e.specifier!=='./broken'))throw Error('cached failure');console.log('ok');"));
            }
        }
    }
}

#[test]
fn d2_lazy_cycles_preserve_hoisting_and_temporal_dead_zones() {
    for mode in ["single", "split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
        for effort in [0,13] {
            let work=Workspace::new(&format!("lazy-cycle-{mode}-{format}-{effort}"));
            work.source("p", "import {read as other} from \"./q\";extern void note(string s);note(other());note(\"p\");export string read(){return \"p\";}");
            work.source("q", "import {read as other} from \"./p\";extern void note(string s);note(other());note(\"q\");export string read(){return \"q\";}");
            work.source("r", "import {run} from \"./s\";extern int initial();int value=initial();export int read(){return value;}export int check(){return run();}");
            work.source("s", "import {read} from \"./r\";int value=read();export int run(){return value;}");
            let main=work.source("main", "export async string load(){auto m=await import(\"./p\");return m.read();}export async int broken(){auto m=await import(\"./r\");return m.check();}");
            let built=compile_entries(&[main],&config(mode,format,effort),ServiceOptions::default()).unwrap_or_else(|error|panic!("{mode}/{format}/{effort}: {error}"));
            let load = if format == "esm" { "const m=await import('./main.js');" }
                else { "import{createRequire}from'node:module';const m=createRequire(import.meta.url)('./main.cjs');" };
            work.execute(&built, &"let log=[],runs=0;globalThis.note=s=>log.push(s);globalThis.initial=()=>{runs++;return 7};const m=await import('./main.js');if(log.length||runs)throw Error('eager');if(await m.load()!=='p'||log.join()!=='p,q,q,p')throw Error('cycle order');let failures=0;for(let i=0;i<2;i++){try{await m.broken()}catch(e){if(!e.message.includes('before initialization'))throw e;failures++}}if(failures!==2||runs)throw Error('TDZ/error caching');console.log('ok');".replace("const m=await import('./main.js');", load));
        }
     }
    }
}

#[test]
#[ignore = "D2 consumer oracle: set LILSCRIPT_ESBUILD to the pinned esbuild binary"]
fn d2_consumer_bundles_preserve_effects_and_remove_annotated_initializers() {
    let esbuild = std::env::var("LILSCRIPT_ESBUILD").expect("pinned consumer bundler");
    for mode in ["split", "preserve-modules"] {
        for placement in ["auto", "facade"] {
            let mut sizes = Vec::new();
            for annotations in ["off", "all"] {
                let work = Workspace::new(&format!("consumer-{mode}-{placement}-{annotations}"));
                let main = work.source("main", "string[] make(){return [\"UNUSED_SENTINEL_payload\"];}export string[] data=make();export int answer(int x){return x+1;}print(\"kept\");");
                let mut settings = config(mode, "esm", 0);
                settings.optimization.constant_folding = Some(false);
                settings.optimization.inlining = Some(false);
                let controls: ProjectConfig = toml::from_str(&format!("[delivery]\nannotations='{annotations}'\nexport_placement='{placement}'")).unwrap();
                settings.delivery.annotations = controls.delivery.annotations;
                settings.delivery.export_placement = controls.delivery.export_placement;
                let built = compile_entries(&[main], &settings, ServiceOptions::default()).unwrap();
                let artifact = built.javascript(Objective::Raw).unwrap();
                for file in artifact.files() { fs::write(work.0.join(&file.name), &file.code).unwrap(); }
                if placement == "facade" {
                    assert!(artifact.files().iter().find(|file| file.name == "main.js").unwrap().code.starts_with("export{"), "{:?}", artifact.files());
                }
                fs::write(work.0.join("consumer.js"), "import{answer}from'./main.js';console.log(answer(6));").unwrap();
                let bundle = Command::new(&esbuild).current_dir(&work.0)
                    .args(["consumer.js", "--bundle", "--format=esm", "--platform=node", "--minify", "--outfile=consumer-bundle.mjs"])
                    .output().unwrap();
                assert!(bundle.status.success(), "{}", String::from_utf8_lossy(&bundle.stderr));
                let bytes = fs::read(work.0.join("consumer-bundle.mjs")).unwrap();
                let text = std::str::from_utf8(&bytes).unwrap();
                assert_eq!(text.contains("UNUSED_SENTINEL"), annotations == "off", "{mode}/{placement}/{annotations}: {text}");
                let result = Command::new("node").arg(work.0.join("consumer-bundle.mjs")).output().unwrap();
                assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
                assert_eq!(result.stdout, b"kept\n7\n");
                let row = [bytes.len(), crate::compression::canonical_gzip_size(&bytes).unwrap(), crate::compression::canonical_brotli_size(&bytes).unwrap()];
                println!("consumer {mode}/{placement}/{annotations}: raw={} gzip={} brotli={}", row[0], row[1], row[2]);
                sizes.push(row);
            }
            for codec in 0..3 { assert!(sizes[1][codec] < sizes[0][codec]); }
        }
    }
}


#[test]
fn d2_lazy_already_eager_and_function_only_namespaces_are_live_and_cached() {
    for mode in ["single", "split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
            let work=Workspace::new(&format!("eager-namespace-{mode}-{format}"));
            work.source("state", "export int value=3;export void bump(){value=value+1;}");
            work.source("pure", "export int answer(int value){return value+1;}");
            let main=work.source("main", "import {bump} from \"./state\";export {bump};export async JsValue namespace(){auto m=await import(\"./state\");return m;}export async JsValue loadPure(){auto m=await import(\"./pure\");return m;}");
            let built=compile_entries(&[main],&config(mode,format,13),ServiceOptions::default()).unwrap_or_else(|error|panic!("{mode}/{format}: {error}"));
            if mode == "single" { assert!(built.javascript(Objective::Raw).unwrap().files().iter().all(|file|!file.code.contains("function*"))); }
            let load = if format == "esm" { "const m=await import('./main.js');" }
                else { "import{createRequire}from'node:module';const m=createRequire(import.meta.url)('./main.cjs');" };
            work.execute(&built, &format!("{load}const a=await m.namespace(),b=await m.namespace();if(a!==b||a.value!==3)throw Error('eager namespace');m.bump();if(a.value!==4)throw Error('snapshot');const c=await m.loadPure(),d=await m.loadPure();if(c!==d||c.answer(5)!==6||c.answer.name!=='answer')throw Error('function namespace');console.log('ok');"));
        }
    }
}

#[test]
fn d2_source_maps_retain_inlined_origins_and_final_delivery_names() {
    for mode in ["single", "split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
            for maps in ["inline", "external"] {
                let work = Workspace::new(&format!("maps-{mode}-{format}-{maps}"));
                let helper = "extern int tripwire(int n);\r\nexport int bridge(int n){/* 😀 */return tripwire(n);}";
                work.source("helper", helper);
                let main = work.source("main", "import {bridge} from \"./helper\";export int run(int n){return bridge(n)+1;}");
                let mut settings = config(mode, format, 13);
                let controls: ProjectConfig = toml::from_str(&format!("[delivery]\nsource_maps='{maps}'\nsource_root='https://example.test/src/'")).unwrap();
                settings.delivery.source_maps = controls.delivery.source_maps;
                settings.delivery.source_root = controls.delivery.source_root;
                settings.delivery.chunk_names = Some("chunks/[name]-[hash:8].[ext]".into());
                let built = compile_entries(&[main], &settings, ServiceOptions::default()).unwrap_or_else(|error| panic!("{mode}/{format}/{maps}: {error}"));
                let artifact = built.javascript(Objective::Raw).unwrap();
                let layout = artifact.layout().unwrap();
                let map_count = layout.files.iter().filter(|file| matches!(file.role, crate::js::delivery::FileRole::SourceMap(_))).count();
                assert_eq!(map_count, if maps == "external" { artifact.files().len() / 2 } else { 0 });
                for (index, file) in layout.files.iter().enumerate() {
                    if let crate::js::delivery::FileRole::SourceMap(owner) = file.role {
                        assert!(!file.anchored && file.imports.is_empty());
                        assert_eq!(artifact.files()[index].name, format!("{}.map", artifact.files()[owner as usize].name));
                    }
                }
                let expected_column = helper.split("\r\n").nth(1).unwrap().split("tripwire").next().unwrap().encode_utf16().count();
                let files = serde_json::to_string(&artifact.files().iter().filter(|file| !file.name.ends_with(".map")).map(|file| file.name.as_str()).collect::<Vec<_>>()).unwrap();
                let probe = format!(r#"
import {{readFileSync}} from 'node:fs';
import {{SourceMap,createRequire}} from 'node:module';
let found=0;
for(const name of {files}){{
  const code=readFileSync(new URL(name,import.meta.url),'utf8');
  const url=code.match(/sourceMappingURL=([^\n]+)/)[1];
  const json=JSON.parse(url.startsWith('data:')?Buffer.from(url.split(',')[1],'base64').toString():readFileSync(new URL(url,new URL(name,import.meta.url)),'utf8'));
  if(json.file!==name.split('/').at(-1)||json.version!==3||json.sourceRoot!=='https://example.test/src/')throw Error('final map metadata');
  const map=new SourceMap(json);
  for(const match of code.matchAll(/tripwire\(/g)){{
    const prefix=code.slice(0,match.index), lines=prefix.split('\n');
    const at=map.findEntry(lines.length-1,lines.at(-1).length);
    if(!at.originalSource.endsWith('helper.lil')||at.originalLine!==1||at.originalColumn!=={expected_column})throw Error('origin '+JSON.stringify(at));
    if(!json.sourcesContent.some(text=>text.includes('😀')))throw Error('missing original text');
    found++;
  }}
}}
if(!found)throw Error('fixture lost its mapped call');
globalThis.tripwire=n=>n+2;
const m='{format}'==='esm'?await import('./main.js'):createRequire(import.meta.url)('./main.cjs');
if(m.run(4)!==7)throw Error('execution');
console.log('ok');
"#);
                work.execute(&built, &probe);
            }
        }
    }
}

#[test]
fn d2_source_map_controls_and_budget_release() {
    let mut settings = config("single", "esm", 0);
    settings.delivery.source_maps = crate::config::SourceMaps::External;
    settings.delivery.sources_content = false;
    let built = compile_source("export int answer(int n){return n+1;}", &settings, ServiceOptions::default()).unwrap();
    assert_eq!(built.report()["resources"]["retained_bytes_after_handoff"], 0);
    let artifact = built.javascript(Objective::Raw).unwrap();
    let map = artifact.files().iter().find(|file| file.name.ends_with(".map")).unwrap();
    let json: serde_json::Value = serde_json::from_str(&map.code).unwrap();
    assert!(json.get("sourcesContent").is_none() && json.get("sourceRoot").is_none());
    assert_eq!(json["sources"], serde_json::json!(["source.lil"]));
    assert!(!json["mappings"].as_str().unwrap().is_empty());
    // The factory verifies that all owned retained storage has been released.
    assert_eq!(artifact.files().iter().map(|file| file.code.len()).sum::<usize>(), artifact.files().iter().map(|file| file.sizes.raw).sum::<usize>());
}

#[test]
fn d2_application_frame_maps_and_single_file_service_view() {
    for maps in [crate::config::SourceMaps::Inline, crate::config::SourceMaps::External] {
        let mut settings = config("single", "iife", 13);
        settings.delivery.source_maps = maps;
        let built = compile_source("extern int tripwire(int n);\nprint(tripwire(5));", &settings,
            ServiceOptions { preserve_root_exports: false, ..ServiceOptions::default() }).unwrap();
        let artifact = built.javascript(Objective::Raw).unwrap();
        let code = &artifact.files()[0].code;
        assert!(code.starts_with("(()=>{"), "{code}");
        if maps == crate::config::SourceMaps::Inline { assert_eq!(artifact.javascript(), code); }
        else { assert!(artifact.javascript().is_empty()); }
        let code = serde_json::to_string(code).unwrap();
        let external = artifact.files().get(1).map(|file| file.code.as_str()).unwrap_or("null");
        let probe = format!("import{{SourceMap}}from'node:module';import vm from'node:vm';const code={code};const url=code.match(/sourceMappingURL=([^\\n]+)/)[1];const json=url.startsWith('data:')?JSON.parse(Buffer.from(url.split(',')[1],'base64').toString()):{external};const map=new SourceMap(json);const offset=code.indexOf('tripwire('),lines=code.slice(0,offset).split('\\n');const at=map.findEntry(lines.length-1,lines.at(-1).length);if(at.originalSource!=='source.lil'||at.originalLine!==1||at.originalColumn!==6)throw Error(JSON.stringify(at));let value;vm.runInNewContext(code,{{tripwire:n=>n+2,console:{{log:n=>value=n}}}});if(value!==7)throw Error('execution');console.log('ok');");
        let output = Command::new("node").args(["--input-type=module", "-e", &probe]).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        assert_eq!(output.stdout, b"ok\n");
    }
}

#[test]
fn d2_embedded_hosts_keep_separate_entry_ownership_and_lazy_evaluation() {
    for mode in ["single", "split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
            for effort in [0, 13] {
                let work = Workspace::new(&format!("owned-hosts-{mode}-{format}-{effort}"));
                fs::write(work.0.join("a-host.js"), "note('host-a');export function readA(){return 11}").unwrap();
                fs::write(work.0.join("b-host.js"), "note('host-b');export function readB(){return 22}").unwrap();
                fs::write(work.0.join("leaf-host.js"), "note('host-leaf');export function leaf(){return 30}").unwrap();
                fs::write(work.0.join("feature-host.js"), "import{leaf}from'./leaf-host.js';note('host-feature');export function read(){return leaf()+3}").unwrap();
                work.source("side", "extern void note(string s);note(\"side\");export int readSide(){return 1;}");
                work.source("feature", "import extern {read} from \"./feature-host.js\";extern int read();extern void note(string s);note(\"feature\");export int answer(){return read();}");
                let a = work.source("a", "import extern {readA} from \"./a-host.js\";extern int readA();import {readSide} from \"./side\";import extern {readB} from \"./b-host.js\";extern int readB();extern void note(string s);note(\"a\");export int answer(){return readA()+readB()+readSide();}export async int load(){auto m=await import(\"./feature\");return m.answer();}");
                let b = work.source("b", "import extern {readB} from \"./b-host.js\";extern int readB();extern void note(string s);note(\"b\");export int answer(){return readB();}");
                let mut settings = config(mode, format, effort);
                settings.delivery.host_modules = crate::config::HostModules::Embed;
                let built = compile_entries(&[a,b], &settings, ServiceOptions::default()).unwrap_or_else(|error| panic!("{mode}/{format}/{effort}: {error}"));
                let loader = if format == "esm" { "const load=name=>import('./'+name+'.js');" }
                    else { "import{createRequire}from'node:module';const require=createRequire(import.meta.url);const load=name=>Promise.resolve(require('./'+name+'.cjs'));" };
                let expected = if mode == "single" { "host-b,b,host-a,side,host-b,a" } else { "host-b,b,host-a,side,a" };
                work.execute(&built, &format!("let notes=[];globalThis.note=s=>notes.push(s);{loader}const b=await load('b');if(notes.join()!=='host-b,b'||b.answer()!==22)throw Error('unrelated hosts '+notes);const a=await load('a');if(notes.join()!=='{expected}'||a.answer()!==34)throw Error('mixed import order '+notes);if(await a.load()!==33||await a.load()!==33||notes.join()!=='{expected},host-leaf,host-feature,feature')throw Error('lazy host once '+notes);console.log('ok');"));
            }
        }
    }
}

#[test]
fn d2_foreign_requests_preserve_unused_loads_order_and_lazy_boundaries() {
    for mode in ["split", "preserve-modules"] {
        for format in ["esm", "cjs"] {
            let work = Workspace::new(&format!("foreign-order-{mode}-{format}"));
            let ext = if format == "esm" { "js" } else { "cjs" };
            for (name, value) in [("first", 1), ("last", 2), ("lazy", 3), ("unused", 4)] {
                let exports = if format == "esm" { format!("export const value={value};") } else { format!("exports.value={value};") };
                fs::write(work.0.join(format!("{name}.{ext}")), format!("note('{name}');{exports}")).unwrap();
            }
            work.source("empty", &format!("import extern {{value}} from \"./unused.{ext}\";extern int value;"));
            work.source("side", "extern void note(string s);note(\"side\");export int read(){return 5;}");
            work.source("feature", &format!("import extern {{value}} from \"./lazy.{ext}\";extern int value;export int read(){{return value;}}"));
            let main = work.source("main", &format!("import extern {{value as a}} from \"./first.{ext}\";extern int a;import {{read}} from \"./side\";import extern {{value as b}} from \"./last.{ext}\";extern int b;import \"./empty\";export int answer(){{return read()+b;}}export async int load(){{auto m=await import(\"./feature\");return m.read();}}"));
            let built = compile_entries(&[main.clone()], &config(mode, format, 13), ServiceOptions::default()).unwrap_or_else(|error| panic!("{mode}/{format}: {error}"));
            let load = if format == "esm" { "const m=await import('./main.js');" } else { "import{createRequire}from'node:module';const m=createRequire(import.meta.url)('./main.cjs');" };
            work.execute(&built, &format!("let log=[];globalThis.note=x=>log.push(x);{load}if(log.join()!=='first,side,last,unused'||m.answer()!==7)throw Error('request order '+log);if(await m.load()!==3||await m.load()!==3||log.join()!=='first,side,last,unused,lazy')throw Error('lazy request '+log);console.log('ok');"));
            let error = compile_entries(&[main], &config("single", format, 0), ServiceOptions::default()).unwrap_err().to_string();
            assert!(error.contains("single-file static imports"), "{error}");
            let lazy = work.source("only-lazy", "export async int load(){auto m=await import(\"./feature\");return m.read();}");
            let error = compile_entries(&[lazy], &config("single", format, 0), ServiceOptions::default()).unwrap_err().to_string();
            assert!(error.contains("lazy foreign module"), "{error}");
        }
    }
}

#[test]
fn d2_carried_text_selects_only_each_entrys_host_closure() {
    let work = Workspace::new("carried-subset");
    let mut entries = Vec::new();
    for name in ["a", "b"] {
        // Classes keep this on the carried-text fallback, outside typed lowering.
        fs::write(work.0.join(format!("{name}-host.js")), format!("note('{name}');class X{{value(){{return 7}}}}export function read(){{return new X().value()}}" )).unwrap();
        entries.push(work.source(name, &format!("import extern {{read}} from \"./{name}-host.js\";extern int read();export int answer(){{return read();}}")));
    }
    let mut settings = config("single", "esm", 13);
    settings.delivery.host_modules = crate::config::HostModules::Embed;
    let built = compile_entries(&entries, &settings, ServiceOptions::default()).unwrap();
    work.execute(&built, "let log=[];globalThis.note=x=>log.push(x);const b=await import('./b.js');if(log.join()!=='b'||b.answer()!==7)throw Error('entry b '+log);const a=await import('./a.js');if(log.join()!=='b,a'||a.answer()!==7)throw Error('entry a '+log);console.log('ok');");
}

#[test]
fn d2_source_maps_keep_copied_class_field_initializer_origins() {
    for effort in [0, 13] {
        let work = Workspace::new(&format!("field-map-{effort}"));
        let source = "extern int fieldwire(int n);\nexport class Box{\nint value=fieldwire(3);\n}";
        work.source("box", source);
        let main = work.source("main", "import {Box} from \"./box\";export int run(){Box b=new Box();return b.value;}");
        let mut settings = config("single", "esm", effort);
        settings.delivery.source_maps = crate::config::SourceMaps::External;
        let built = compile_entries(&[main], &settings, ServiceOptions::default()).unwrap();
        work.execute(&built, "import{readFileSync}from'node:fs';import{SourceMap}from'node:module';const code=readFileSync(new URL('./main.js',import.meta.url),'utf8');const map=new SourceMap(JSON.parse(readFileSync(new URL('./main.js.map',import.meta.url),'utf8')));const offset=code.indexOf('fieldwire(');if(offset<0)throw Error('fixture');const lines=code.slice(0,offset).split('\\n');const origin=map.findEntry(lines.length-1,lines.at(-1).length);if(origin.originalSource!=='box.lil'||origin.originalLine!==2||origin.originalColumn!==10)throw Error(JSON.stringify(origin));globalThis.fieldwire=n=>n+5;const m=await import('./main.js');if(m.run()!==8)throw Error('execution');console.log('ok');");
    }
}
