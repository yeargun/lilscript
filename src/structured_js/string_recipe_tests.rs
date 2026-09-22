//! Target facts follow the actual mutable method recipe, not its source name.
use super::*;
use crate::compilation_contract::{JavaScriptExecution, JavaScriptWorld};
use std::process::Command;

const MODES: [analysis::Mode; 5] = [
    analysis::Mode::Tree,
    analysis::Mode::Indexed,
    analysis::Mode::Memoized,
    analysis::Mode::Regions,
    analysis::Mode::Values,
];

#[test]
fn sloppy_hidden_caller_escape_keeps_opaque_argument_effects_after_optimization() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "extern void probe();int helper(int value){probe();int discarded=value+1;return 7;}print(helper(1));",
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    for mode in MODES {
        for execution in [JavaScriptExecution::Script, JavaScriptExecution::Module] {
            for optimized in [false, true] {
                let mut tree = lower::lower_slice(&syntax, &semantics).unwrap();
                if optimized {
                    if execution == JavaScriptExecution::Script {
                        // Compatibility entrypoints must retain the weaker
                        // contract even for a closed source-use graph.
                        optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication)
                            .unwrap();
                    } else {
                        optimize::optimize_in_execution(
                            &mut tree,
                            mode,
                            JavaScriptWorld::ClosedApplication,
                            execution,
                        )
                        .unwrap();
                    }
                }
                let view = if execution == JavaScriptExecution::Script {
                    extract::JavaScriptView::prepare_in_world(
                        &tree,
                        mode,
                        JavaScriptWorld::ClosedApplication,
                    )
                } else {
                    extract::JavaScriptView::prepare_in_execution(
                        &tree,
                        mode,
                        JavaScriptWorld::ClosedApplication,
                        execution,
                    )
                };
                let javascript = view
                    .render(PrintPolicy {
                        mangle_bindings: true,
                    })
                    .unwrap();
                // Function constructs a sloppy host callback even when the
                // tested artifact is a module. Strict helper frames are hidden;
                // sloppy helper frames can escape and be invoked again later.
                let script = format!(
                    "const trace=[];console.log=value=>trace.push(value);globalThis.saved=null;globalThis.probe=Function(\"if(!globalThis.saved)globalThis.saved=globalThis.probe.caller;\");\n{javascript}\ntrace.push(saved===null?'sealed':'retained');if(saved){{try{{trace.push(saved({{valueOf(){{trace.push('coerce');throw Error('opaque')}}}}));}}catch(error){{trace.push(error.message);}}}}process.stdout.write(JSON.stringify(trace));"
                );
                let (mode_argument, expected) = match execution {
                    JavaScriptExecution::Script => (
                        "--input-type=commonjs",
                        "[7,\"retained\",\"coerce\",\"opaque\"]",
                    ),
                    JavaScriptExecution::Module => ("--input-type=module", "[7,\"sealed\"]"),
                };
                let output = Command::new("node")
                    .args([mode_argument, "-e", &script])
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{mode:?}/{execution:?}/{optimized}: {}\n{javascript}",
                    String::from_utf8_lossy(&output.stderr)
                );
                assert_eq!(
                    String::from_utf8(output.stdout).unwrap(),
                    expected,
                    "{mode:?}/{execution:?}/{optimized}: {javascript}"
                );
            }
        }
    }
}

#[test]
fn incoming_domain_snapshots_require_their_original_execution_contract() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        "int helper(int value){return value+1;}print(helper(3));",
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let tree = lower::lower_slice(&syntax, &semantics).unwrap();
    let parameter = BindingId::new(
        tree.target()
            .bindings
            .iter()
            .position(|binding| binding.spelling == "value")
            .unwrap(),
    );
    for mode in MODES {
        let script =
            analysis::Analysis::new_in_world(&tree, mode, JavaScriptWorld::ClosedApplication);
        assert_eq!(script.execution(), JavaScriptExecution::Script);
        assert_eq!(
            script.binding_uses(parameter).entry_value,
            analysis::ValueKind::Unknown
        );
        assert_eq!(script.work().parameters.argument_queries, 0);
        assert_eq!(script.work().parameters.temporary_bytes, 0);
        assert!(analysis::Analysis::resume_in_execution(
            &tree,
            script.detach(),
            JavaScriptExecution::Module
        )
        .is_err());
        let module = || {
            analysis::Analysis::new_in_execution(
                &tree,
                mode,
                JavaScriptWorld::ClosedApplication,
                JavaScriptExecution::Module,
            )
        };
        let facts = module();
        assert_eq!(
            facts.binding_uses(parameter).entry_value,
            analysis::ValueKind::Number
        );
        assert!(facts.binding_uses(parameter).entry_i32);
        let resumed = analysis::Analysis::resume_in_execution(
            &tree,
            facts.detach(),
            JavaScriptExecution::Module,
        )
        .unwrap();
        assert_eq!(resumed.execution(), JavaScriptExecution::Module);
        assert_eq!(
            resumed.binding_uses(parameter).entry_value,
            analysis::ValueKind::Number
        );
        assert!(analysis::Analysis::resume(&tree, resumed.detach()).is_err());
        assert!(extract::JavaScriptView::prepare_reusing(&tree, module().detach()).is_err());
        assert!(extract::JavaScriptView::prepare_reusing_in_execution(
            &tree,
            module().detach(),
            JavaScriptExecution::Module
        )
        .is_ok());
    }
}

fn execute_all(source: &str, setup: &str, expected: &str) {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    for mode in MODES {
        let mut tree = lower::lower_slice(&syntax, &semantics).unwrap();
        optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication).unwrap();
        let javascript = extract::JavaScriptView::prepare_in_world(
            &tree,
            mode,
            JavaScriptWorld::ClosedApplication,
        )
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
        let script =
            format!("const trace=[];{setup}\n{javascript}\nconsole.log(JSON.stringify(trace));");
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .expect("Node is required for mutable target method observations");
        assert!(
            result.status.success(),
            "{mode:?}: {}\n{javascript}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            expected,
            "{mode:?}: {javascript}"
        );
    }
}

#[test]
fn method_results_require_runtime_domain_and_freshness_evidence_in_every_strategy() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(
        &arena,
        r#"
            string sliced="text".slice(0);sliced+sliced;
            string[] pieces="text".split(",");pieces.length;
            "text".charAt(0);"text".charCodeAt(0);"text".indexOf("t");
        "#,
    )
    .unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let tree = lower::lower_slice(&syntax, &semantics).unwrap();
    for mode in MODES {
        let mut facts = analysis::Analysis::new(&tree, mode);
        let mut methods = 0;
        for (index, expression) in tree.target().expressions.iter().enumerate() {
            let id = ExprId::new(index);
            let result = facts.facts(id);
            match expression {
                Expr::Intrinsic { operation, .. }
                    if matches!(intrinsic_form(*operation), IntrinsicForm::Method(_)) =>
                {
                    methods += 1;
                    assert!(!result.own_effects.discardable(), "{mode:?}/{operation:?}");
                    assert!(!result.effects.stable_scalar());
                    assert!(!result.normalization_redundant);
                    if integer_intrinsic(*operation) {
                        assert_eq!(result.value, analysis::ValueKind::Number);
                        assert_eq!(
                            result.integer,
                            Some(analysis::IntegerRange {
                                minimum: i32::MIN as i64,
                                maximum: i32::MAX as i64,
                            })
                        );
                    } else {
                        assert_eq!(result.value, analysis::ValueKind::Unknown);
                        assert_eq!(result.integer, None);
                        assert!(result.constant.is_none());
                    }
                    if *operation == Intrinsic::StringSlice {
                        assert_eq!(tree.source_type(id), Some(&crate::semantic::Type::String));
                    }
                }
                Expr::Binding(binding)
                    if tree.target().bindings[binding.index()].spelling == "sliced" =>
                {
                    // A later typed cell read cannot resurrect the rejected
                    // domain through its source annotation.
                    assert_eq!(result.value, analysis::ValueKind::Unknown);
                }
                Expr::Binary {
                    op: Binary::Add, ..
                } => {
                    assert_eq!(result.value, analysis::ValueKind::Unknown);
                    assert!(!result.own_effects.discardable());
                }
                Expr::Intrinsic {
                    operation: Intrinsic::ArrayLength,
                    ..
                } => {
                    assert!(!result.own_effects.discardable());
                    assert!(!result.normalization_redundant);
                }
                _ => {}
            }
        }
        assert_eq!(methods, 5);
    }
}

#[test]
fn length_properties_remain_precise_for_actual_string_and_array_recipes() {
    let arena = bumpalo::Bump::new();
    let syntax = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let mut tree = lower::lower_slice(&syntax, &semantics).unwrap();
    let module = tree.target_mut();
    let text = module.expression(
        Expr::Literal(Literal::String(crate::literal::StringValue::from("𝄞"))),
        None,
    );
    let element = module.expression(Expr::Literal(Literal::Number(1.0)), None);
    let array = module.expression(Expr::Array(vec![element]), None);
    let string_length = module.expression(
        Expr::Intrinsic {
            operation: Intrinsic::StringLength,
            receiver: text,
            arguments: Vec::new(),
        },
        None,
    );
    let array_length = module.expression(
        Expr::Intrinsic {
            operation: Intrinsic::ArrayLength,
            receiver: array,
            arguments: Vec::new(),
        },
        None,
    );
    module.regions[module.root.index()].statements.extend([
        Statement::Evaluate(string_length),
        Statement::Evaluate(array_length),
    ]);
    module.verify().unwrap();
    for mode in MODES {
        let mut facts = analysis::Analysis::new(&tree, mode);
        for (id, length) in [(string_length, 2), (array_length, 1)] {
            let result = facts.facts(id);
            assert!(result.own_effects.discardable());
            assert!(result.normalization_redundant);
            assert_eq!(result.integer.unwrap().singleton_i32(), Some(length));
        }
    }
}

#[test]
fn mutable_method_lookup_and_invocation_keep_distinct_cell_snapshots() {
    execute_all(
        r#"
            extern void install(func()->void lookup,func()->void invoke);
            extern void observe(int value);
            int state=0;
            void lookup(){state=2;}
            void invoke(){state=3;}
            int second(){state=5;return state;}
            install(lookup,invoke);state=1;
            "ab".slice(state,second());
            observe(state);
        "#,
        r#"
            const original=Object.getOwnPropertyDescriptor(String.prototype,'slice');
            globalThis.install=(lookup,invoke)=>Object.defineProperty(String.prototype,'slice',{
                configurable:true,get(){
                    if(String(this)!=='ab')return original.value;
                    trace.push('lookup');lookup();
                    return function(start,end){trace.push(['args',start,end]);invoke();return 'result';};
                }
            });
            globalThis.observe=value=>{Object.defineProperty(String.prototype,'slice',original);trace.push(['after',value]);};
        "#,
        r#"["lookup",["args",2,5],["after",3]]"#,
    );
}

#[test]
fn replaced_slice_and_split_preserve_raw_values_aliases_and_length_getters() {
    execute_all(
        r#"
            extern void inspect(string[] first,string[] second,string value);
            string[] first="ab".split(",");
            string[] second="ab".split(",");
            inspect(first,second,"ab".slice(0));
        "#,
        r#"
            const originalSplit=String.prototype.split,originalSlice=String.prototype.slice;
            const shared=['initial'],returned={valueOf(){throw Error('unexpected coercion');}};
            String.prototype.split=function(){trace.push('split');return shared;};
            String.prototype.slice=function(){trace.push('slice');return returned;};
            globalThis.inspect=(a,b,value)=>{
                String.prototype.split=originalSplit;String.prototype.slice=originalSlice;
                trace.push([a===b,value===returned]);a[0]='changed';trace.push(b[0]);
            };
        "#,
        r#"["split","split","slice",[true,true],"changed"]"#,
    );
    execute_all(
        r#"extern void restore();"ab".split(",").length;restore();"#,
        r#"
            const original=String.prototype.split;
            String.prototype.split=function(){trace.push('split');return {
                get length(){trace.push('length');return {valueOf(){trace.push('coerce');return 5;}};}
            };};
            globalThis.restore=()=>{String.prototype.split=original;};
        "#,
        r#"["split","length","coerce"]"#,
    );
}

#[test]
fn mutable_integer_methods_keep_normalization_and_observable_result_coercion() {
    execute_all(
        r#"extern void observe(int first,int second);observe("ab".charCodeAt(0),"ab".indexOf("a"));"#,
        r#"
            const code=String.prototype.charCodeAt,find=String.prototype.indexOf;
            String.prototype.charCodeAt=function(){trace.push('code');return {
                valueOf(){trace.push('coerce');return 4294967297;}
            };};
            String.prototype.indexOf=function(){trace.push('find');return -2147483649;};
            globalThis.observe=(a,b)=>{String.prototype.charCodeAt=code;String.prototype.indexOf=find;trace.push([a,b]);};
        "#,
        r#"["code","coerce","find",[1,2147483647]]"#,
    );
}
