//! Runtime contracts are checked against independently specified host traces.
//! These tests use the compilation owner, including policy-aware Output.
use super::publication::*;
use super::*;
use crate::compilation_policy::{
    BudgetLedger, BudgetPlan, CompilationRequest, ResolvedPolicy, ResourceLimits, WorkDomain,
};
use crate::structured_js::selection::{Plan, Style};
use serde_json::{json, Value as Json};
use std::process::Command;

fn policy(text: &str, exports: bool) -> ResolvedPolicy {
    // Every trace fixture chooses its logging contract explicitly in TOML.
    let config: crate::config::ProjectConfig = toml::from_str(text).unwrap();
    config
        .resolve_policy(CompilationRequest::JavaScript {
            preserve_root_exports: exports,
        })
        .unwrap()
}

fn artifacts(
    source: &str,
    policy: &ResolvedPolicy,
    scalar: bool,
    style: Style,
) -> Result<Vec<(&'static str, String)>, CandidateError> {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let state = scalar.then(|| {
        CellId::from_index(
            program
                .cells
                .iter()
                .position(|cell| cell.name == "state")
                .unwrap(),
        )
        .unwrap()
    });
    let ledger = BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 10_000_000,
            optional_work: 10_000_000,
            baseline_retained_bytes: 0,
            retained_bytes: 10_000_000,
        },
    )
    .unwrap();
    let mut compilation = Compilation::new(ledger, CheckpointLimit { max_live: 4 }).unwrap();
    let result = (|| {
        let source = compilation.adopt_checked(program, WorkDomain::Baseline)?;
        let direct = compilation.direct_javascript(source, policy, WorkDomain::Baseline)?;
        let mut candidates = vec![("direct", direct)];
        if let Some(state) = state {
            let result = compilation.scalar_javascript(
                direct,
                state,
                ScalarRequest {
                    max_work: 100_000,
                    scratch_bytes: 100_000,
                    output_bytes: 100_000,
                },
                policy,
                WorkDomain::Optional,
            )?;
            let ScalarOutcome::Published(scalar) = result.outcome else {
                panic!("expected complete scalar family: {:?}", result.outcome)
            };
            candidates.push(("scalar", scalar));
        }
        let mut rendered = Vec::new();
        for (representation, candidate) in candidates {
            let javascript =
                compilation.with_javascript_output(candidate, policy, |output| {
                    let artifact = output.render(&Plan::new(style))?;
                    output.take_artifact(artifact)
                })??;
            rendered.push((representation, javascript));
        }
        Ok(rendered)
    })();
    assert_eq!(compilation.finish().retained_bytes(), 0);
    result
}

fn execute(javascript: &str, host: &str, observations: &str) -> Json {
    let script = format!(
        "const events=[];\n{host}\nconst library=await import('data:text/javascript,'+encodeURIComponent({}));\n{observations}\nprocess.stdout.write(JSON.stringify(events));",
        serde_json::to_string(javascript).unwrap(),
    );
    let output = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for target contract runtime tests");
    assert!(
        output.status.success(),
        "{}\n{script}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn logging_contract_preserves_arguments_and_throws_and_controls_host_lookups() {
    let source = r#"
        extern int argument(int mark);
        extern void caught();
        extern void debugLog(int value);
        export void run(){
            print(argument(1));
            debugLog(argument(2));
            try{print(argument(3));}catch{caught();}
            debugLog(argument(4));
            try{debugLog(argument(5));}catch{caught();}
        }
    "#;
    let host = r#"
        globalThis.argument=mark=>{events.push('arg:'+mark);if(mark===3||mark===5)throw Error('argument');return mark;};
        globalThis.caught=()=>events.push('caught');
        const consoleObject={get log(){events.push('log-get');return function(value){events.push(this===consoleObject?'print:'+value:'bad-print-receiver');};}};
        Object.defineProperty(globalThis,'console',{configurable:true,get(){events.push('console-get');return consoleObject;}});
        Object.defineProperty(globalThis,'debugLog',{configurable:true,get(){events.push('debug-get');return value=>events.push('debug:'+value);}});
    "#;
    for strip in [false, true] {
        let resolved = policy(&format!("[javascript]\nstrip_console={strip}\n"), true);
        let expected = if strip {
            json!(["start", "arg:1", "arg:2", "arg:3", "caught", "arg:4", "arg:5", "caught", "end"])
        } else {
            json!([
                "start",
                "console-get",
                "log-get",
                "arg:1",
                "print:1",
                "debug-get",
                "arg:2",
                "debug:2",
                "console-get",
                "log-get",
                "arg:3",
                "caught",
                "debug-get",
                "arg:4",
                "debug:4",
                "debug-get",
                "arg:5",
                "caught",
                "end"
            ])
        };
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(
                execute(
                    &javascript,
                    host,
                    "events.push('start');library.run();events.push('end');"
                ),
                expected,
                "strip={strip}\n{javascript}"
            );
        }
    }
}

#[test]
fn logging_stripping_preserves_shadowed_calls_and_separately_observed_foreign_values() {
    let resolved = policy("[javascript]\nstrip_console=true\n", true);
    let shadowed = "extern void observe(int value);export void run(){func(int)->void debugLog=(int value)=>{observe(value+10);};debugLog(2);}";
    for (_, javascript) in artifacts(shadowed, &resolved, false, Style::Global).unwrap() {
        assert_eq!(
            execute(
                &javascript,
                "globalThis.observe=value=>events.push(value);",
                "library.run();"
            ),
            json!([12])
        );
    }
    let observed = "extern void debugLog(int value);extern void retain(func(int)->void callback);export void run(){retain(debugLog);debugLog(3);}";
    let host = r#"
        Object.defineProperty(globalThis,'debugLog',{configurable:true,get(){events.push('debug-get');return value=>events.push(['debug',value]);}});
        globalThis.retain=callback=>{events.push('retain');callback(8);};
    "#;
    for (_, javascript) in artifacts(observed, &resolved, false, Style::Global).unwrap() {
        assert_eq!(
            execute(&javascript, host, "library.run();"),
            json!(["debug-get", "retain", ["debug", 8]]),
            "{javascript}"
        );
    }
}

#[test]
fn stripping_rejects_non_void_debug_calls_without_fabricating_a_result() {
    let source = "extern int debugLog(int value);export int run(){return debugLog(2);}";
    let observed = policy("[javascript]\nstrip_console=false\n", true);
    for (_, javascript) in artifacts(source, &observed, false, Style::Global).unwrap() {
        assert_eq!(
            execute(
                &javascript,
                "globalThis.debugLog=value=>{events.push(['debug',value]);return value+1;};",
                "events.push(['result',library.run()]);"
            ),
            json!([["debug", 2], ["result", 3]])
        );
    }
    let stripped = policy("[javascript]\nstrip_console=true\n", true);
    assert!(matches!(
        artifacts(source, &stripped, false, Style::Global),
        Err(CandidateError::Unsupported(_))
    ));
}

#[test]
fn record_nullish_and_catch_contracts_preserve_getters_and_lazy_fallback_in_both_representations() {
    let source = r#"
        extern Record<int> hostRecord();
        extern int fallback();
        export func()->int make(){
            Record<int> state=record{x:1};
            return ()=>{
                int value=hostRecord().x??fallback();
                state.x=(state.x??0)+value;
                return state.x??0;
            };
        }
        export int recover(){try{throw 9;}catch{return 4;}}
    "#;
    let host = r#"
        const values=[undefined,0,null,5,'throw',2];
        globalThis.hostRecord=()=>{events.push('host');return {get x(){events.push('get');const value=values.shift();if(value==='throw')throw Error('getter');return value;}};};
        globalThis.fallback=()=>{events.push('fallback');return 7;};
    "#;
    let observations = r#"
        const next=library.make();
        for(let i=0;i<6;i++){try{events.push(['result',next()]);}catch(error){events.push(['caught',error.message]);}}
        events.push(['recover',library.recover()]);
    "#;
    let expected = json!([
        "host",
        "get",
        "fallback",
        ["result", 8],
        "host",
        "get",
        ["result", 8],
        "host",
        "get",
        "fallback",
        ["result", 15],
        "host",
        "get",
        ["result", 20],
        "host",
        "get",
        ["caught", "getter"],
        "host",
        "get",
        ["result", 22],
        ["recover", 4]
    ]);
    for edition in ["es2015", "es2022"] {
        let resolved = policy(
            &format!("[javascript]\nstrip_console=false\necmascript='{edition}'\n"),
            true,
        );
        for (representation, javascript) in
            artifacts(source, &resolved, true, Style::Global).unwrap()
        {
            assert_eq!(
                execute(&javascript, host, observations),
                expected,
                "{edition} {representation}\n{javascript}"
            );
            if edition == "es2015" {
                assert!(!javascript.contains("??"), "{javascript}");
                assert!(!javascript.contains("catch{"), "{javascript}");
                assert!(javascript.contains("catch("), "{javascript}");
            }
        }
    }
}

#[test]
fn root_export_contract_controls_only_the_public_module_boundary() {
    let source = "extern void observe(int value);export int answer(){return 7;}observe(answer());";
    let host = "globalThis.observe=value=>events.push(['initialization',value]);";
    for exports in [false, true] {
        let resolved = policy("[javascript]\nstrip_console=false\n", exports);
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            let expected = if exports {
                json!([
                    ["initialization", 7],
                    ["exports", ["answer"]],
                    ["answer", 7]
                ])
            } else {
                json!([["initialization", 7], ["exports", []]])
            };
            assert_eq!(execute(&javascript, host, "events.push(['exports',Object.keys(library)]);if(library.answer)events.push(['answer',library.answer()]);"), expected);
        }
    }
}

#[test]
fn explicit_callable_spelling_preserves_names_and_arity_with_selected_constructibility() {
    let source =
        "export int add(int left,int right){return left+right;}export auto callback=()=>9;";
    let observations = r#"
        function constructible(value){try{Reflect.construct(function(){},[],value);return true;}catch(error){if(!(error instanceof TypeError))throw error;return false;}}
        events.push([library.add.name,library.add.length,library.add(2,3),Object.hasOwn(library.add,'prototype'),constructible(library.add)]);
        events.push([library.callback.name,library.callback.length,library.callback(),Object.hasOwn(library.callback,'prototype'),constructible(library.callback)]);
    "#;
    for spelling in ["arrow", "function"] {
        let resolved = policy(
            &format!("[javascript]\nstrip_console=false\nfunction_spelling='{spelling}'\n"),
            true,
        );
        // D2: the private spelling knob no longer reaches the public edge. A
        // declared function stays an ordinary constructible `function`; an
        // exported arrow stays an arrow — exactly as the source declares them.
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(
                execute(&javascript, "", observations),
                json!([
                    ["add", 2, 5, true, true],
                    ["callback", 0, 9, false, false]
                ]),
                "{spelling}\n{javascript}"
            );
        }
    }
}

#[test]
fn declared_receivers_and_lexical_descendants_preserve_their_enclosing_receiver() {
    let direct = "extern JsValue this;export JsValue receiver(){return this;}";
    let descendant = "extern JsValue this;export func()->JsValue capture(){return ()=>this;}";
    for (source, observations, expected) in [
        (
            direct,
            "const owner={};events.push(library.receiver.call(owner)===owner,Object.hasOwn(library.receiver,'prototype'));",
            json!([true,true]),
        ),
        (
            descendant,
            "const outer={},inner={};events.push(library.capture.call(outer).call(inner)===outer,Object.hasOwn(library.capture,'prototype'));",
            json!([true,true]),
        ),
    ] {
        for spelling in ["", "function_spelling='arrow'", "function_spelling='function'"] {
            let resolved = policy(&format!("[javascript]\nstrip_console=false\n{spelling}\n"), true);
            for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
                assert_eq!(execute(&javascript, "", observations), expected, "{spelling}\n{javascript}");
            }
        }
    }
}

#[test]
fn closure_spelling_preserves_lexical_module_this() {
    let source = "extern JsValue this;export auto callback=()=>this;";
    for spelling in ["arrow", "function"] {
        let resolved = policy(
            &format!("[javascript]\nstrip_console=false\nfunction_spelling='{spelling}'\n"),
            true,
        );
        let observations = "events.push(library.callback.call({})===undefined);";
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(
                execute(&javascript, "", observations),
                json!([true]),
                "{spelling}\n{javascript}"
            );
        }
    }
}

#[test]
fn own_arguments_and_lexical_descendant_arguments_survive_callable_spelling_policies() {
    let source = r#"
        extern JsValue arguments;
        export JsValue first(int value){return arguments[0];}
        export func()->JsValue make(int value){return ()=>arguments[0];}
    "#;
    let observations = r#"
        const captured=library.make(7);
        events.push(['first',library.first(5),library.first.name,library.first.length,Object.hasOwn(library.first,'prototype')]);
        events.push(['captured',captured(),captured(99),library.make.name,library.make.length,Object.hasOwn(library.make,'prototype')]);
    "#;
    for spelling in [
        "",
        "function_spelling='arrow'",
        "function_spelling='function'",
    ] {
        let resolved = policy(
            &format!("[javascript]\nstrip_console=false\n{spelling}\n"),
            true,
        );
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(
                execute(&javascript, "", observations),
                json!([
                    ["first", 5, "first", 1, true],
                    ["captured", 7, 7, "make", 1, true]
                ]),
                "{spelling}\n{javascript}"
            );
        }
    }
}

#[test]
fn sibling_closures_share_the_owner_arguments_object_with_capture_before_branch_execution() {
    let source = r#"
        extern JsValue arguments;
        extern void retain(func()->JsValue reader,func()->void writer);
        export func()->JsValue make(int value){
            if(value<0){return ()=>arguments;}
            auto reader=()=>arguments;
            auto writer=()=>{arguments[0]=value+1;};
            retain(reader,writer);
            return ()=>arguments;
        }
    "#;
    let host = "const held=[];globalThis.retain=(reader,writer)=>held.push({reader,writer});";
    let observations = r#"
        const early=library.make(-2),first=library.make(7),second=library.make(20);
        const firstObject=first(),secondObject=second();
        events.push(['initial',early()[0],firstObject[0],secondObject[0],firstObject===held[0].reader(),firstObject!==secondObject]);
        held[0].writer.call({},88);
        events.push(['first',first()[0],held[0].reader()[0],second()[0],first()===firstObject]);
        held[1].writer();
        events.push(['second',first()[0],second()[0],second()===secondObject]);
    "#;
    for spelling in [
        "",
        "function_spelling='arrow'",
        "function_spelling='function'",
    ] {
        let resolved = policy(
            &format!("[javascript]\nstrip_console=false\n{spelling}\n"),
            true,
        );
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(
                execute(&javascript, host, observations),
                json!([
                    ["initial", -2, 7, 20, true, true],
                    ["first", 8, 8, 20, true],
                    ["second", 8, 21, true]
                ]),
                "{spelling}\n{javascript}"
            );
        }
    }
}

#[test]
fn local_arguments_shadow_does_not_create_an_ambient_function_obligation() {
    let source = r#"
        extern JsValue arguments;
        export int local(){int arguments=9;return arguments;}
        export func()->int makeLocal(int value){int arguments=value+1;return ()=>arguments;}
    "#;
    let resolved = policy(
        "[javascript]\nstrip_console=false\nfunction_spelling='arrow'\n",
        true,
    );
    let observations = r#"
        events.push([library.local(),Object.hasOwn(library.local,'prototype')]);
        events.push([library.makeLocal(7)(),Object.hasOwn(library.makeLocal,'prototype')]);
    "#;
    // The shadowing is the point: a local named `arguments` creates no ambient
    // obligation. Both exports are source declarations, so under D2 they stay
    // ordinary functions with their own `prototype` whatever the private
    // spelling knob says.
    for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
        assert_eq!(
            execute(&javascript, "", observations),
            json!([[9, true], [8, true]]),
            "{javascript}"
        );
    }
}

#[test]
fn module_arguments_lookup_stays_lazy_and_observes_each_global_read() {
    let source = "extern JsValue arguments;export auto callback=()=>arguments;";
    for spelling in ["", "function_spelling='arrow'"] {
        let resolved = policy(
            &format!("[javascript]\nstrip_console=false\n{spelling}\n"),
            true,
        );
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(execute(
                &javascript,
                "delete globalThis.arguments;",
                "events.push(['created',typeof library.callback]);try{library.callback();events.push('unexpected return');}catch(error){events.push(['call',error.name]);}",
            ), json!([["created","function"],["call","ReferenceError"]]), "{spelling}\n{javascript}");
            let host = "let reads=0;Object.defineProperty(globalThis,'arguments',{configurable:true,get(){events.push(['arguments-get',++reads]);return reads;}});";
            assert_eq!(execute(
                &javascript,
                host,
                "events.push(['created',reads]);events.push(['value',library.callback()]);events.push(['value',library.callback()]);",
            ), json!([["created",0],["arguments-get",1],["value",1],["arguments-get",2],["value",2]]), "{spelling}\n{javascript}");
        }
    }
}

#[test]
fn a_public_closure_over_module_arguments_stays_an_arrow_whatever_the_private_spelling() {
    // This used to be refused: the spelling knob forced `function` onto an
    // exported closure, which would rebind `arguments`. Under D2 the public
    // edge keeps the source's arrow, so the closure compiles and still reads
    // the global `arguments` lazily on each call.
    let source = "extern JsValue arguments;export auto callback=()=>arguments;";
    let resolved = policy(
        "[javascript]\nstrip_console=false\nfunction_spelling='function'\n",
        true,
    );
    let outputs = artifacts(source, &resolved, false, Style::Global).unwrap();
    assert!(!outputs.is_empty());
    for (_, javascript) in outputs {
        let host = "let reads=0;Object.defineProperty(globalThis,'arguments',{configurable:true,get(){events.push(['arguments-get',++reads]);return reads;}});";
        assert_eq!(execute(
            &javascript,
            host,
            "events.push(['created',reads]);events.push(['value',library.callback()]);",
        ), json!([["created",0],["arguments-get",1],["value",1]]), "{javascript}");
    }
}

#[test]
fn disabled_identifier_mangling_preserves_legal_source_cells_and_rejects_mangled_plans() {
    let source = "int descriptiveCounter=7;export int readCounter(int suppliedIncrement){int updatedCounter=descriptiveCounter+suppliedIncrement;return updatedCounter;}";
    for search in ["on", "off"] {
        let resolved = policy(&format!("[javascript]\nstrip_console=false\n[policy.tactics]\nidentifier-mangling='off'\nnaming-search='{search}'\n"), true);
        for (_, javascript) in artifacts(source, &resolved, false, Style::Source).unwrap() {
            for name in ["descriptiveCounter", "suppliedIncrement", "updatedCounter"] {
                assert!(javascript.contains(name), "missing {name}: {javascript}");
            }
            assert_eq!(execute(&javascript, "", "events.push([library.readCounter.name,library.readCounter.length,library.readCounter(5)]);"), json!([["readCounter",1,12]]));
        }
        for style in [Style::Global, Style::Scoped] {
            assert!(matches!(
                artifacts(source, &resolved, false, style),
                Err(CandidateError::Output(_))
            ));
        }
    }
}

#[test]
fn mutable_builtin_integer_results_normalize_after_lookup_arguments_and_receiver_call() {
    let source =
        "extern int operand(int n);export int multiply(){return Math.imul(operand(1),operand(2));}";
    let resolved = policy("[javascript]\nstrip_console=false\n", true);
    let host = r#"
        let result=NaN;const failure={};
        globalThis.operand=n=>{events.push('arg:'+n);return n;};
        Object.defineProperty(Math,'imul',{configurable:true,get(){events.push('get');return function(a,b){events.push(['call',this===Math,a,b]);return result;};}});
    "#;
    let observations = r#"
        events.push(['result',library.multiply()]);
        result=4294967297;
        events.push(['result',library.multiply()]);
        result={valueOf(){events.push('coerce');throw failure;}};
        try{library.multiply();events.push('unexpected return');}catch(error){events.push(['thrown',error===failure]);}
    "#;
    for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
        assert_eq!(
            execute(&javascript, host, observations),
            json!([
                "get",
                "arg:1",
                "arg:2",
                ["call", true, 1, 2],
                ["result", 0],
                "get",
                "arg:1",
                "arg:2",
                ["call", true, 1, 2],
                ["result", 1],
                "get",
                "arg:1",
                "arg:2",
                ["call", true, 1, 2],
                "coerce",
                ["thrown", true]
            ]),
            "{javascript}"
        );
    }
}

#[test]
fn print_return_expression_is_void_and_keeps_host_throw_completion_when_enabled() {
    let source = "extern int argument();export void report(){return print(argument());}";
    let host = r#"
        let fail=false;const failure={};
        globalThis.argument=()=>{events.push('arg');return 7;};
        const logger={get log(){events.push('get');return function(value){events.push(['call',this===logger,value]);if(fail)throw failure;return 123;};}};
        Object.defineProperty(globalThis,'console',{configurable:true,value:logger});
    "#;
    let observations = r#"
        events.push(['result',library.report()===undefined]);
        fail=true;
        try{events.push(['second',library.report()===undefined]);}catch(error){events.push(['thrown',error===failure]);}
    "#;
    for strip in [false, true] {
        let resolved = policy(&format!("[javascript]\nstrip_console={strip}\n"), true);
        let expected = if strip {
            json!(["arg", ["result", true], "arg", ["second", true]])
        } else {
            json!([
                "get",
                "arg",
                ["call", true, 7],
                ["result", true],
                "get",
                "arg",
                ["call", true, 7],
                ["thrown", true]
            ])
        };
        for (_, javascript) in artifacts(source, &resolved, false, Style::Global).unwrap() {
            assert_eq!(
                execute(&javascript, host, observations),
                expected,
                "strip={strip}\n{javascript}"
            );
        }
    }
}
