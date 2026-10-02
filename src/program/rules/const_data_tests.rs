use super::*;
use crate::build::{compile_source, ServiceOptions};
use crate::config::ProjectConfig;
use std::process::Command;

fn configuration(extra: &str) -> ProjectConfig {
    toml::from_str(&format!(
        "objective.codecs='raw'\neffort.level=0\n[policy]\nversion=3\n{extra}"
    ))
    .unwrap()
}
fn execute(source: &str, config: &ProjectConfig, expected: &str) -> String {
    let result = compile_source(source, config, ServiceOptions::default()).unwrap();
    let code = result
        .javascript(crate::js::selection::Objective::Raw)
        .unwrap()
        .javascript();
    let output = Command::new("node").args(["-e", code]).output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{code}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), expected);
    code.to_string()
}
#[test]
fn q4_required_const_evaluation_survives_folding_veto() {
    let source = "const int square(int n){int total=0;for(int i=0;i<n;i++){total+=n;}return total;}const int answer=square(7);print(answer);";
    for config in [
        configuration(""),
        configuration("[policy.tactics]\nconstant-folding='off'\ndead-code-elimination='off'\n"),
    ] {
        execute(source, &config, "49\n");
    }
}
#[test]
fn q4_const_array_construction_and_nested_alias_reads() {
    execute("int[] build(int n){int[] values=[];for(int i=0;i<n;i++){values.push(i*i);}return values;}const int[] values=build(5);int[] alias=values;print(alias[3]);print(values.length);", &configuration(""), "9\n5\n");
}
#[test]
fn q4_const_record_computed_keys_and_utf16_values() {
    execute(
        r#"Record<string> build(){Record<string> values=record{};for(int i=0;i<3;i++){values[i.toString()]="item"+i.toString();}return values;}const Record<string> values=build();print(values["2"]);"#,
        &configuration(""),
        "item2\n",
    );
}
#[test]
fn q4_const_mutation_rejects_alias_parameter_and_nested_container() {
    for source in [
        "const int[] data=[1,2];int[] alias=data;alias[0]=4;",
        "void change(int[] data){data[0]=4;}const int[] data=[1,2];change(data);",
        "const int[][] data=[[1,2]];int[] alias=data[0];alias.push(3);",
        "const int[] data=[1];int[][] holder=[data];holder[0][0]=7;",
        "extern void change(int[] data);const int[] data=[1];change(data);",
        "extern int[] outside;const int[] data=[1];outside=data;",
        "extern Record<int[]> outside;const int[] data=[1];outside[\"leak\"]=data;",
        "const int[] data=[1];extern void keep(func()->int[] callback);keep(()=>data);",
        "const int[][] data=[[1]];int[][] copy=data.slice();copy[0].push(2);",
        "const Record<int[]> data=record{x:[1]};int[][] copy=Object.values(data);copy[0][0]=2;",
        "const int[] data=[1];int[] copy=data.concat([2]);copy[0]=3;data[0]=4;",
    ] {
        let error =
            compile_source(source, &configuration(""), ServiceOptions::default()).unwrap_err();
        assert!(error.to_string().contains("const"), "{error:?}");
    }
}
#[test]
fn n2_const_readers_preserve_mutable_outer_copies() {
    execute(r#"const int[] data=[1,2];int[] copy=data.slice();copy[0]=9;copy.push(3);print(data.join(","));print(copy.join(","));const Record<string> recordData=record{b:"B",a:"A"};string[] values=Object.values(recordData);values[0]="changed";print(JSON.stringify(recordData));print(Object.keys(recordData).join(","));print(Object.hasOwn(recordData,"a"));print(values.join(","));"#, &configuration(""), "1,2\n9,2,3\n{\"b\":\"B\",\"a\":\"A\"}\nb,a\ntrue\nchanged,A\n");
}
#[test]
fn q4_const_required_failures_and_configured_bounds_are_diagnostic() {
    for (source, extra, fragment) in [
        ("extern int get();const int n=get();", "", "host"),
        ("const float n=1.0.sin();", "", "exact"),
        (
            "int spin(){while(true){}return 1;}const int n=spin();",
            "[language.const_evaluation]\nsteps=40\n",
            "steps",
        ),
        (
            "int f(int n){return f(n+1);}const int n=f(0);",
            "[language.const_evaluation]\ndepth=4\n",
            "depth",
        ),
        (
            "const int[] n=[1,2,3];",
            "[language.const_evaluation]\nbytes=1\n",
            "bytes",
        ),
    ] {
        let error =
            compile_source(source, &configuration(extra), ServiceOptions::default()).unwrap_err();
        assert!(error.to_string().contains(fragment), "{error:?}");
    }
}
#[test]
fn q4_const_bindings_cannot_be_reassigned_or_export_functions() {
    for source in [
        "const int n=1;n=2;",
        "export const int f(int n){return n;}",
        "const int f(int n){return n;}print(f(2));",
    ] {
        assert!(compile_source(source, &configuration(""), ServiceOptions::default()).is_err());
    }
}

#[test]
fn q4_public_const_data_is_deeply_frozen_and_keeps_keys_and_aliases() {
    let source = r#"const int[] child=[1,2];export const Record<int[]> data=record{left:child,right:child};export int first(){return child[0];}"#;
    for level in [0, 13] {
        let mut config = configuration("");
        config.effort.level = level;
        let result = compile_source(source, &config, ServiceOptions::default()).unwrap();
        let code = result
            .javascript(crate::js::selection::Objective::Raw)
            .unwrap()
            .javascript();
        let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));let refused=0;try{{m.data.left[0]=9}}catch(e){{refused++}}try{{m.data.extra=[]}}catch(e){{refused++}}console.log(JSON.stringify([Object.keys(m.data),Object.getPrototypeOf(m.data)===null,Object.isFrozen(m.data),Object.isFrozen(m.data.left),m.data.left===m.data.right,m.first(),refused]));", serde_json::to_string(code).unwrap());
        let output = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{code}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            r#"[["left","right"],true,true,true,true,1,2]"#
        );
    }
}
#[test]
fn q4_const_alias_rebinding_is_distinct_from_mutation_and_nested_values_copy() {
    execute(
        "const int[] values=[1,2];int[] alias=values;alias=[3,4];print(alias[1]);",
        &configuration(""),
        "4\n",
    );
}

#[test]
fn q4_define_defaults_overrides_types_and_exact_policy_identity() {
    let source = r#"define bool FEATURE=false;define int SIZE=3;define float SCALE=1.5;define string LABEL="default";const int[] data=[SIZE,SIZE+1];print(FEATURE);print(data[1]);print(SCALE);print(LABEL);"#;
    execute(source, &configuration(""), "false\n4\n1.5\ndefault\n");
    execute(
        source,
        &configuration(
            "[defines]\nFEATURE=true\nSIZE=7\nSCALE=-0.0\nLABEL='literal source; print(99)'\n",
        ),
        "true\n8\n-0\nliteral source; print(99)\n",
    );
    for extra in [
        "[defines]\nUNKNOWN=1",
        "[defines]\nSIZE=2147483648",
        "[defines]\nFEATURE=1",
        "[defines]\nSCALE=9007199254740993",
    ] {
        assert!(
            compile_source(source, &configuration(extra), ServiceOptions::default())
                .unwrap_err()
                .to_string()
                .contains("define")
        );
    }
    assert!(toml::from_str::<ProjectConfig>("[defines]\nSIZE=[1,2]").is_err());
    let request = crate::compilation_policy::CompilationRequest::JavaScript {
        preserve_root_exports: true,
    };
    let negative = configuration("[defines]\nSCALE=-0.0")
        .resolve_policy(request)
        .unwrap();
    let positive = configuration("[defines]\nSCALE=0.0")
        .resolve_policy(request)
        .unwrap();
    assert_ne!(negative.fingerprint(), positive.fingerprint());
}

#[test]
fn q4_const_nested_value_copy_static_alias_paths_and_utf16() {
    execute("struct Inner{int x;}struct Outer{Inner inner;int[] shared;}int construct(){Outer a=Outer{Inner{1},[2]};Outer b=a;b.inner.x=7;b.shared[0]=9;return a.inner.x*100+a.shared[0]*10+b.inner.x;}const int result=construct();print(result);", &configuration(""), "197\n");
    execute("const int[][] first=[[1,2]];const int[][] second=first;const int[] third=second[0];print(third==first[0]);print(third[1]);", &configuration(""), "true\n2\n");
    execute(
        r#"const string text="\ud800\u0000😀";const int size=text.length;print(size);"#,
        &configuration(""),
        "4\n",
    );
}

#[test]
fn q4_static_schema_parameter_specialization_preserves_runtime_inputs() {
    let source = "const int[] schema=[3,7];int validate(int[] shape,int n){if(shape[0]==3){return n+shape[1];}return n-1;}extern int input();print(validate(schema,input()));print(validate(schema,input()));";
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let checked = crate::analyze(&syntax).unwrap();
    let program = super::super::super::from_checked_source(&syntax, &checked).unwrap();
    let request = RuleRequest {
        reuse_normalization: true,
        unroll: true,
        pool: true,
        choices: crate::representation::RegionalChoices::ALL,
        fold: true,
        dead_code: true,
        inline: false,
        scalar: true,
        native: false,
        pristine_builtins: true,
        seal: Seal::Module,
    };
    let (program, receipt) = super::super::optimize(program, request).unwrap();
    program.verify().unwrap();
    assert!(receipt.constant_parameters > 0, "{receipt:?}");
    let code = super::super::super::javascript::lower(&program)
        .unwrap()
        .render(crate::js::PrintPolicy::default())
        .unwrap();
    let script = format!("let n=0;globalThis.input=()=>++n;{code}");
    let output = Command::new("node").args(["-e", &script]).output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{code}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8(output.stdout).unwrap(), "8\n9\n");
}

#[test]
fn q4_private_const_returns_preserve_aliases_but_public_returns_are_refused() {
    let source = "const int[] child=[1,2];int[] get(){return child;}const int[] second=get();int[] alias=get();print(second==child);print(alias==child);print(alias[1]);";
    execute(source, &configuration(""), "true\ntrue\n2\n");
    for source in [
        "const int[] child=[1,2];export int[] get(){return child;}",
        "const int[] child=[1,2];int[] get(){return child;}get()[0]=9;",
    ] {
        assert!(
            compile_source(source, &configuration(""), ServiceOptions::default())
                .unwrap_err()
                .to_string()
                .contains("const")
        );
    }
}

#[test]
fn q4_const_cross_module_evaluation_owns_transitive_static_captures() {
    super::super::super::module_helper_tests::modules(
        &[
            r#"import {get} from "./data";const int[] first=get();print(first[0]);print(first==get());"#,
            "export const int[] data=[4];export int[] get(){return data;}",
        ],
        &[&[1], &[]],
        &[1, 0],
        |program| {
            let code = program
                .to_javascript()
                .unwrap()
                .render(crate::js::PrintPolicy::default())
                .unwrap();
            let output = Command::new("node")
                .args(["--input-type=module", "-e", &code])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{code}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(String::from_utf8(output.stdout).unwrap(), "4\ntrue\n");
        },
    );
}

#[test]
fn q4_many_const_initializers_reuse_one_sparse_frame() {
    let mut source = String::new();
    for index in 0..256 {
        source.push_str(&format!("const int value{index}={index};"));
    }
    source.push_str("print(value255);");
    execute(
        &source,
        &configuration("[language.const_evaluation]\nbytes=262144"),
        "255\n",
    );
}

#[test]
fn d2_public_const_products_preserve_private_layout_and_reference_aliases() {
    let source = "struct Point{int x;}export const Point point=Point{3};export const Point[] child=[point];export const Record<Point[]> data=record{left:child,right:child};export int read(){return point.x+child[0].x+(data[\"right\"]??child)[0].x;}";
    for effort in [0, 13] {
        let mut config = configuration(""); config.effort.level = effort;
        let built = compile_source(source, &config, ServiceOptions::default()).unwrap();
        let code = built.javascript(crate::js::selection::Objective::Raw).unwrap().javascript();
        let script = format!("const m=await import('data:text/javascript,'+encodeURIComponent({}));if(m.point.x!==3||m.child[0].x!==3||m.read()!==9||m.child!==m.data.left||m.data.left!==m.data.right)throw Error('layout/alias');for(const v of [m.point,m.child,m.child[0],m.data])if(!Object.isFrozen(v))throw Error('mutable');if(Object.getPrototypeOf(m.data)!==null)throw Error('record prototype');console.log('ok');",serde_json::to_string(code).unwrap());
        let result = Command::new("node").args(["--input-type=module", "-e", &script]).output().unwrap();
        assert!(result.status.success(), "{}\n{code}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(result.stdout,b"ok\n");
    }
}
