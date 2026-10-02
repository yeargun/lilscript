//! Several packaging contracts share one checked program, one ledger and a
//! mandatory incumbent for every requested format/subset/objective.
use super::*;

#[derive(Debug)]
pub struct OutputPolicies {
    pub name: String,
    pub policies: Vec<ResolvedPolicy>,
}
impl OutputPolicies {
    pub fn receipt(&self) -> Value {
        json!({"name":self.name,"policies":self.policies.iter().map(ResolvedPolicy::receipt).collect::<Vec<_>>()})
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServiceOutput {
    name: String,
    pub(super) winners: [Option<usize>; 3],
}
impl ServiceOutput {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn objectives(&self) -> impl Iterator<Item = Objective> + '_ {
        [Objective::Raw, Objective::Gzip, Objective::Brotli]
            .into_iter()
            .filter(|&codec| self.winners[codec_index(codec)].is_some())
    }
}
impl ServiceCompilation {
    /// Empty for the original single-output API. Multi-format results include
    /// `primary`, followed by the configured additional outputs in name order.
    pub fn outputs(&self) -> &[ServiceOutput] {
        &self.outputs
    }
    pub fn javascript_output(&self, name: &str, codec: Objective) -> Option<&ServiceJavaScript> {
        if name == "primary" {
            return self.javascript(codec);
        }
        self.outputs
            .iter()
            .find(|output| output.name == name)
            .and_then(|output| output.winners[codec_index(codec)])
            .map(|index| &self.javascript[index])
    }
}

impl ServiceOptions {
    pub fn resolve_additional_outputs(
        &self,
        config: &ProjectConfig,
    ) -> Result<Vec<OutputPolicies>, String> {
        config.validate()?;
        if self.target == ServiceTarget::Native && !config.delivery.also.is_empty() {
            return Err("`delivery.also` requires a JavaScript target".into());
        }
        let mut outputs = Vec::new();
        for output in &config.delivery.also {
            let configured = output.configuration(config);
            let policies = self.resolve_javascript_policies(&configured)?;
            outputs.push(OutputPolicies {
                name: output.name.clone(),
                policies,
            });
        }
        outputs.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(outputs)
    }
}

impl CheckedSourceSession<'_> {
    pub(super) fn compile_javascript_outputs(
        &mut self,
        output: &mut ServiceCompilation,
        observe: impl FnMut(SearchObservation<'_>),
    ) -> Result<Value, ServiceError> {
        let primary = self
            .independent_javascript
            .as_deref()
            .unwrap_or_else(|| std::slice::from_ref(self.javascript.as_ref().unwrap()));
        let groups = std::iter::once(("primary", primary))
            .chain(
                self.additional_outputs
                    .iter()
                    .map(|group| (group.name.as_str(), group.policies.as_slice())),
            )
            .collect::<Vec<_>>();
        let none = [None, None, None];
        let mut requests = Vec::new();
        for (index, (_, policies)) in groups.iter().enumerate() {
            for policy in *policies {
                requests.push((
                    policy,
                    search_request(
                        self.options,
                        policy,
                        Objectives::One(policy.objective().unwrap().codec),
                    ),
                    if index == 0 {
                        &self.decisions.assignments
                    } else {
                        &none
                    },
                ));
            }
        }
        let results = self
            .compilation
            .search_javascript_outputs(self.source, &requests, observe)
            .map_err(|error| ServiceError::output("javascript outputs", error))?;
        let mut results = results.into_iter().map(Option::unwrap);
        let mut reports = Vec::new();
        for (name, policies) in groups {
            let mut group = ServiceOutput {
                name: name.into(),
                winners: [None; 3],
            };
            for policy in policies {
                let result = results.next().unwrap();
                let codec = policy.objective().unwrap().codec;
                let mut report = search_report(
                    search_request_report(search_request(
                        self.options,
                        policy,
                        Objectives::One(codec),
                    )),
                    result.counters,
                    result.stopped,
                    serde_json::to_value(result.terminal).unwrap(),
                );
                report["output"] = json!(name);
                report["policy"] = policy.receipt();
                report["optional_work_allowance"] = json!(result.optional_work_allowance);
                report["optional_work_used"] = json!(result.optional_work_used);
                reports.push(report);
                match deliver_javascript(
                    &mut self.compilation,
                    result.winner,
                    name == "primary" && self.decisions.is_writing(),
                ) {
                    Ok(artifact) => {
                        group.winners[codec_index(codec)] = Some(output.javascript.len());
                        output.javascript.push(artifact);
                    }
                    Err(error) => {
                        for pending in results {
                            self.compilation
                                .discard_artifact(pending.winner.artifact())
                                .expect("output owns pending handoff");
                        }
                        return Err(error);
                    }
                }
            }
            if name == "primary" {
                output.winners = group.winners;
            }
            output.outputs.push(group);
        }
        let mut combined = json!({"independent":true,
            "resource_schedule":"all output baselines first; equal shares of remaining optional work; shared hard memory and deadline"});
        for field in [
            "proposals",
            "structures",
            "renders",
            "codec_probes",
            "proof_queries",
            "beam_evictions",
            "admitted_artifacts",
        ] {
            combined[field] = json!(reports
                .iter()
                .map(|report| report[field].as_u64().unwrap_or(0))
                .sum::<u64>());
        }
        let stops = reports
            .iter()
            .filter_map(|report| {
                report["stop"]
                    .as_str()
                    .map(|stop| format!("{}: {stop}", report["output"].as_str().unwrap()))
            })
            .collect::<Vec<_>>();
        combined["stop"] = if stops.is_empty() {
            Value::Null
        } else {
            json!(stops.join("; "))
        };
        combined["terminal"] = json!({"objectives":reports.iter().flat_map(|report| {
            report["terminal"]["objectives"].as_array().into_iter().flatten().map(|stage| {
                let mut stage=stage.clone(); stage["output"]=report["output"].clone(); stage
            })
        }).collect::<Vec<_>>()});
        combined["outputs"] = json!(reports);
        Ok(combined)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{fs, process::Command};

    const CONFIG: &str = r#"
        [policy]
        version=3
        [effort]
        level=13
        [objective]
        codecs=['raw','gzip','brotli']
        [javascript]
        candidate_proposal_limit=2
        terminal_codec_probe_limit=4
        [mangle]
        identifiers=false
        [delivery]
        mode='split'
        [[delivery.also]]
        name='cjs'
        format='cjs'
        entries=['mini']
        [[delivery.also]]
        name='browser'
        format='iife'
        mode='single'
        entries=['mini']
        global='Mini'
        global_binding='property'
    "#;

    #[test]
    fn d3_outputs_share_input_with_independent_formats_codecs_and_subsets() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-d3-outputs-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        fs::create_dir_all(&directory).unwrap();
        for (file, source) in [
            (
                "shared.lil",
                r#"export int counter=2;export int increment(){counter=counter+1;return counter;}export void reset(){counter=99;}"#,
            ),
            (
                "full.lil",
                r#"import {increment,reset} from "./shared";print("full-only");reset();export int answer(){return increment();}"#,
            ),
            (
                "mini.lil",
                r#"import {increment} from "./shared";print("mini");export int answer(){return increment();}export void lazy(){import("./lazy").then((auto m)=>print(m.value));}"#,
            ),
            ("lazy.lil", r#"print("lazy");export int value=9;"#),
        ] {
            fs::write(directory.join(file), source).unwrap();
        }
        let entries = ["full", "mini"].map(|name| crate::module::EntrySource {
            name: name.into(),
            path: directory.join(format!("{name}.lil")),
        });
        let config = crate::config::parse_project_config(CONFIG).unwrap().config;
        let options = ServiceOptions {
            logical_work: 1_000_000_000,
            retained_bytes: 512_000_000,
            ..ServiceOptions::default()
        };
        let built = compile_entries(&entries, &config, options)
            .unwrap_or_else(|error| panic!("{}", crate::render_service_error(&error)));
        assert_eq!(built.outputs().len(), 3);
        assert_eq!(
            built.report()["inputs"]["modules"]
                .as_array()
                .unwrap()
                .len(),
            4
        );
        assert_eq!(
            built.report()["search"]["outputs"]
                .as_array()
                .unwrap()
                .len(),
            9
        );
        for group in built.outputs() {
            for codec in group.objectives() {
                let artifact = built.javascript_output(group.name(), codec).unwrap();
                let out = directory.join(group.name()).join(codec.name());
                fs::create_dir_all(&out).unwrap();
                fs::write(out.join("package.json"), "{\"type\":\"module\"}").unwrap();
                for file in artifact.files() {
                    let path = out.join(&file.name);
                    fs::create_dir_all(path.parent().unwrap()).unwrap();
                    fs::write(path, &file.code).unwrap();
                }
                let layout = artifact.layout().unwrap();
                let entry = layout
                    .entries
                    .iter()
                    .find(|entry| entry.name == "mini")
                    .unwrap();
                let filename = &artifact.files()[entry.file as usize].name;
                let load = match group.name() {
                    "primary" => format!("const api=await import({});",json!(format!("./{filename}"))),
                    "cjs" => format!("import {{createRequire}} from 'node:module';const api=createRequire(import.meta.url)({});",json!(format!("./{filename}"))),
                    _ => format!("await import({});const api=globalThis.Mini;",json!(format!("./{filename}"))),
                };
                fs::write(out.join("probe.mjs"),format!("{load}console.log(api.answer(),api.answer());api.lazy();await new Promise(r=>setTimeout(r,20));")).unwrap();
                let result = Command::new("node")
                    .arg(out.join("probe.mjs"))
                    .output()
                    .unwrap();
                assert!(
                    result.status.success(),
                    "{} {}: {}",
                    group.name(),
                    codec.name(),
                    String::from_utf8_lossy(&result.stderr)
                );
                assert_eq!(
                    String::from_utf8(result.stdout).unwrap(),
                    "mini\n3 4\nlazy\n9\n",
                    "{} {}",
                    group.name(),
                    codec.name()
                );
                if group.name() != "primary" {
                    assert_eq!(
                        layout.entries.iter().filter(|entry| !entry.dynamic).count(),
                        1
                    );
                }
                assert!(
                    artifact
                        .files()
                        .iter()
                        .any(|file| file.code.contains("counter")),
                    "hard naming veto"
                );
            }
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn d3_outputs_allow_format_specific_default_surfaces() {
        let config = crate::config::parse_project_config(
            r#"
            effort.level=0
            objective.codecs='raw'
            [[delivery.also]]
            name='cjs'
            format='cjs'
            exports='default'
            es_module_marker='never'
            default_interop='es-module'
            [[delivery.also]]
            name='browser'
            format='iife'
            exports='default'
            global='Answer'
            global_binding='property'
        "#,
        )
        .unwrap()
        .config;
        let built = compile_source(
            "int answer(){return 42;}export {answer as default};",
            &config,
            ServiceOptions::default(),
        )
        .unwrap();
        let cjs = built
            .javascript_output("cjs", Objective::Raw)
            .unwrap()
            .javascript();
        let browser = built
            .javascript_output("browser", Objective::Raw)
            .unwrap()
            .javascript();
        let probe=format!("const vm=require('node:vm');const c={{module:{{exports:{{}}}},exports:{{}},require}};vm.runInNewContext({},c);if(typeof c.module.exports!=='function'||c.module.exports()!==42)throw Error('CJS default');const b={{}};vm.runInNewContext({},b);if(typeof b.Answer!=='function'||b.Answer()!==42)throw Error('browser default');",serde_json::to_string(cjs).unwrap(),serde_json::to_string(browser).unwrap());
        let output = std::process::Command::new("node")
            .args(["-e", &probe])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn d3_outputs_validate_config_and_keep_complete_build_cache_receipts() {
        for config in [
            "[[delivery.also]]\nname='primary'",
            "[[delivery.also]]\nname='../escape'",
            "[[delivery.also]]\nname='cjs'\nunknown=true",
            "[[delivery.also]]\nname='cjs'\nentries=['a','a']",
            "[[delivery.also]]\nname='cjs'\ncodecs=['raw','raw']",
        ] {
            assert!(
                crate::config::parse_project_config(config).is_err(),
                "{config}"
            );
        }
        let mut config = crate::config::parse_project_config("[policy]\nversion=3\n[effort]\nlevel=0\n[objective]\ncodecs='raw'\n[[delivery.also]]\nname='cjs'\nformat='cjs'\ncodecs='gzip'").unwrap().config;
        let directory =
            std::env::temp_dir().join(format!("lilscript-d3-output-cache-{}", std::process::id()));
        let _ = fs::remove_dir_all(&directory);
        config.cache.directory = Some(directory.clone());
        let first = compile_source(
            "export int answer(){return 42;}",
            &config,
            ServiceOptions::default(),
        )
        .unwrap();
        let second = compile_source(
            "export int answer(){return 42;}",
            &config,
            ServiceOptions::default(),
        )
        .unwrap();
        assert_eq!(second.report()["build_cache"]["hit"], true);
        for (name, codec) in [("primary", Objective::Raw), ("cjs", Objective::Gzip)] {
            assert_eq!(
                first.javascript_output(name, codec).unwrap().sha256(),
                second.javascript_output(name, codec).unwrap().sha256()
            );
        }
        assert!(compile_source(
            "",
            &config,
            ServiceOptions {
                target: ServiceTarget::Native,
                ..ServiceOptions::default()
            }
        )
        .is_err());
        config.delivery.also[0].entries = vec!["missing".into()];
        let error = compile_source(
            "export int answer(){return 42;}",
            &config,
            ServiceOptions::default(),
        )
        .unwrap_err();
        assert!(
            error.message.contains("unknown selected delivery entry"),
            "{error:?}"
        );
        fs::remove_dir_all(directory).unwrap();
    }
}
