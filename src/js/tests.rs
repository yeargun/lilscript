use super::*;
use std::process::Command;

#[path = "projection_tests.rs"]
mod projection;

fn expr(module: &mut Module, node: Expr) -> ExprId {
    module.expression(node, None)
}

fn number(module: &mut Module, value: f64) -> ExprId {
    expr(module, Expr::Literal(Literal::Number(value)))
}

fn host(module: &mut Module, name: &str) -> ExprId {
    expr(module, Expr::Host(name.into()))
}

fn binding(module: &mut Module, region: RegionId, source: u32, name: &str) -> BindingId {
    module.binding(Binding {
        source_symbol: Some(SymbolId(source)),
        scope: module.regions[region.index()].scope,
        spelling: name.into(),
        pinned: false,
    })
}

fn call(
    module: &mut Module,
    callee: ExprId,
    arguments: Vec<ExprId>,
    invocation: Invocation,
) -> ExprId {
    expr(
        module,
        Expr::Call {
            callee,
            arguments,
            invocation,
        },
    )
}

fn capture(module: &mut Module, value: ExprId) {
    let output = host(module, "capture");
    let call = call(module, output, vec![value], Invocation::Value);
    module.regions[0].statements.push(Statement::Evaluate(call));
}

fn execute(module: &Module, setup: &str, policy: PrintPolicy) -> String {
    let javascript = module.render(policy).unwrap();
    let script = format!(
        "const outputs=[];function capture(value){{outputs.push(value)}}\n{setup}\n{javascript}\nconsole.log(JSON.stringify(outputs));"
    );
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .expect("Node is required for target semantic tests");
    assert!(
        result.status.success(),
        "{}\n{javascript}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().to_string()
}

#[test]
fn distinct_target_cells_share_provenance_without_aliasing_and_remap_together() {
    let mut module = Module::default();
    binding(&mut module, RegionId::new(0), u32::MAX, "abandoned");
    let orphan = module.region(ScopeId::new(0));
    module.functions.push(Function {
        parameters: vec![],
        body: orphan,
        arrow: false,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
    });
    number(&mut module, 99.0);
    let callable = binding(&mut module, RegionId::new(0), 42, "increment");
    let shared = binding(&mut module, RegionId::new(0), 77, "shared");
    let body = module.region(ScopeId::new(0));
    let parameter = binding(&mut module, body, 77, "parameter");
    let synthetic = module.binding(Binding {
        source_symbol: None,
        scope: ScopeId::new(0),
        spelling: "temporary".into(),
        pinned: false,
    });
    assert_ne!(shared, parameter);
    assert_eq!(
        module.bindings[shared.index()].source_symbol,
        module.bindings[parameter.index()].source_symbol
    );

    let left = expr(&mut module, Expr::Binding(shared));
    let right = expr(&mut module, Expr::Binding(parameter));
    let value = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Add,
            left,
            right,
        },
    );
    let target = expr(&mut module, Expr::Binding(shared));
    let assignment = expr(&mut module, Expr::Assign { target, value });
    let result = expr(&mut module, Expr::Binding(shared));
    module.regions[body.index()].statements = vec![
        Statement::Evaluate(assignment),
        Statement::Return(Some(result)),
    ];
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters: vec![parameter],
        body,
        arrow: false,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
    });
    let initial = number(&mut module, 10.0);
    let callee = expr(&mut module, Expr::Binding(callable));
    let two = number(&mut module, 2.0);
    let first = call(&mut module, callee, vec![two], Invocation::Reference);
    module.regions[0].statements = vec![
        Statement::Let {
            binding: shared,
            value: Some(initial),
        },
        Statement::Function {
            binding: callable,
            function,
        },
        Statement::Let {
            binding: synthetic,
            value: Some(first),
        },
    ];
    let temporary = expr(&mut module, Expr::Binding(synthetic));
    capture(&mut module, temporary);
    let callee = expr(&mut module, Expr::Binding(callable));
    let three = number(&mut module, 3.0);
    let second = call(&mut module, callee, vec![three], Invocation::Reference);
    capture(&mut module, second);
    let current = expr(&mut module, Expr::Binding(shared));
    capture(&mut module, current);

    for mangle_bindings in [false, true] {
        assert_eq!(
            execute(&module, "", PrintPolicy { mangle_bindings }),
            "[12,15,15]"
        );
    }
    let absent = BindingId::new(module.bindings.len());
    let invalid = expr(&mut module, Expr::Binding(absent));
    capture(&mut module, invalid);
    assert_eq!(module.verify().unwrap_err(), "unknown binding");
}

#[test]
fn receiver_lookup_precedes_arguments_and_value_calls_stay_unbound() {
    for invocation in [Invocation::Reference, Invocation::Value] {
        let mut module = Module::default();
        let object = host(&mut module, "object");
        let member = expr(
            &mut module,
            Expr::Member {
                object,
                property: Property::Named("method".into()),
            },
        );
        let argument = host(&mut module, "argument");
        let argument = call(&mut module, argument, vec![], Invocation::Value);
        let invocation_expression = call(&mut module, member, vec![argument], invocation);
        capture(&mut module, invocation_expression);
        let trace = host(&mut module, "trace");
        capture(&mut module, trace);
        let setup = r#"
            const trace=[]; const object={};
            Object.defineProperty(object,'method',{configurable:true,get(){
                trace.push('get');
                function selected(value){ trace.push('call'); return [this===object,value]; }
                selected.call=()=>{throw Error('observable .call fallback')};
                return selected;
            }});
            function argument(){trace.push('argument');Object.defineProperty(object,'method',{value:()=>{throw Error('late lookup')}});return 7}
        "#;
        let expected = if invocation == Invocation::Reference {
            "[[true,7],[\"get\",\"argument\",\"call\"]]"
        } else {
            "[[false,7],[\"get\",\"argument\",\"call\"]]"
        };
        assert_eq!(
            execute(
                &module,
                setup,
                PrintPolicy {
                    mangle_bindings: true
                }
            ),
            expected
        );
    }
}

#[test]
fn direct_eval_keeps_lexical_names_and_indirect_eval_stays_global() {
    for invocation in [Invocation::DirectEval, Invocation::Value] {
        let mut module = Module::default();
        let body = module.region(ScopeId::new(0));
        let secret = binding(&mut module, body, 0, "secret");
        let function = binding(&mut module, RegionId::new(0), 1, "readSecret");
        let initial = expr(&mut module, Expr::Literal(Literal::String("local".into())));
        let eval = host(&mut module, "eval");
        let code = expr(&mut module, Expr::Literal(Literal::String("secret".into())));
        let evaluated = call(&mut module, eval, vec![code], invocation);
        module.regions[body.index()].statements = vec![
            Statement::Let {
                binding: secret,
                value: Some(initial),
            },
            Statement::Return(Some(evaluated)),
        ];
        module.functions.push(Function {
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: crate::js::Suspension::None,
            arrow: false,
            parameters: vec![],
            body,
        });
        module.regions[0].statements.push(Statement::Function {
            binding: function,
            function: FunctionId::new(0),
        });
        let reference = expr(&mut module, Expr::Binding(function));
        let value = call(&mut module, reference, vec![], Invocation::Value);
        capture(&mut module, value);
        assert_eq!(
            execute(
                &module,
                "globalThis.secret='global';",
                PrintPolicy {
                    mangle_bindings: true
                }
            ),
            if invocation == Invocation::DirectEval {
                "[\"local\"]"
            } else {
                "[\"global\"]"
            }
        );
    }
}

#[test]
fn constructor_from_a_call_or_a_member_of_a_call_preserves_the_reference() {
    for member in [false, true] {
        let mut module = Module::default();
        let factory = host(
            &mut module,
            if member { "factoryObject" } else { "factory" },
        );
        let mut target = call(&mut module, factory, vec![], Invocation::Value);
        if member {
            target = expr(
                &mut module,
                Expr::Member {
                    object: target,
                    property: Property::Named("Constructor".into()),
                },
            );
        }
        let value = number(&mut module, 7.0);
        let instance = expr(
            &mut module,
            Expr::Construct {
                callee: target,
                arguments: vec![value],
            },
        );
        let field = expr(
            &mut module,
            Expr::Member {
                object: instance,
                property: Property::Named("value".into()),
            },
        );
        capture(&mut module, field);
        assert_eq!(
            execute(
                &module,
                "function Constructor(value){this.value=value};const factory=()=>Constructor;const factoryObject=()=>({Constructor});",
                PrintPolicy::default()
            ),
            "[7]"
        );
    }
}

#[test]
fn expression_grammar_preserves_association_signs_negative_zero_and_object_keys() {
    let mut module = Module::default();
    let one = number(&mut module, 1.0);
    let two = number(&mut module, 2.0);
    let three = number(&mut module, 3.0);
    let inner = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Subtract,
            left: two,
            right: three,
        },
    );
    let value = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Subtract,
            left: one,
            right: inner,
        },
    );
    capture(&mut module, value);
    let negative = expr(
        &mut module,
        Expr::Unary {
            op: Unary::Negate,
            value: one,
        },
    );
    let value = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Subtract,
            left: one,
            right: negative,
        },
    );
    capture(&mut module, value);
    let zero = number(&mut module, -0.0);
    let reciprocal = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Divide,
            left: one,
            right: zero,
        },
    );
    let string = host(&mut module, "String");
    let value = call(&mut module, string, vec![reciprocal], Invocation::Value);
    capture(&mut module, value);
    let key = expr(
        &mut module,
        Expr::Literal(Literal::String("__proto__".into())),
    );
    let object = expr(
        &mut module,
        Expr::Object(vec![(Property::Computed(key), three)]),
    );
    let value = expr(
        &mut module,
        Expr::Member {
            object,
            property: Property::Computed(key),
        },
    );
    capture(&mut module, value);
    assert_eq!(
        execute(&module, "", PrintPolicy::default()),
        "[2,2,\"-Infinity\",3]"
    );
}

#[test]
fn nested_closures_keep_binding_identity_after_renaming() {
    let mut module = Module::default();
    let outer_body = module.region(ScopeId::new(0));
    let inner_body = module.region(module.regions[outer_body.index()].scope);
    let parameter = binding(&mut module, outer_body, 0, "outerValue");
    let outer = binding(&mut module, RegionId::new(0), 1, "makeClosure");
    let value = expr(&mut module, Expr::Binding(parameter));
    module.regions[inner_body.index()]
        .statements
        .push(Statement::Return(Some(value)));
    module.functions.push(Function {
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
        arrow: false,
        parameters: vec![],
        body: inner_body,
    });
    let inner = expr(&mut module, Expr::Function(FunctionId::new(0)));
    module.regions[outer_body.index()]
        .statements
        .push(Statement::Return(Some(inner)));
    module.functions.push(Function {
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
        arrow: false,
        parameters: vec![parameter],
        body: outer_body,
    });
    module.regions[0].statements.push(Statement::Function {
        binding: outer,
        function: FunctionId::new(1),
    });
    let outer = expr(&mut module, Expr::Binding(outer));
    let value = number(&mut module, 42.0);
    let closure = call(&mut module, outer, vec![value], Invocation::Value);
    let value = call(&mut module, closure, vec![], Invocation::Value);
    capture(&mut module, value);
    assert_eq!(
        execute(
            &module,
            "",
            PrintPolicy {
                mangle_bindings: true
            }
        ),
        "[42]"
    );
}

#[test]
fn scope_dependencies_and_loop_transfers_are_verified() {
    let mut dangling = Module::default();
    expr(
        &mut dangling,
        Expr::Unary {
            op: Unary::Not,
            value: ExprId::new(10),
        },
    );
    assert!(dangling.verify().unwrap_err().contains("dependency"));

    let mut module = Module::default();
    let child = module.region(ScopeId::new(0));
    let local = binding(&mut module, child, 0, "local");
    let value = number(&mut module, 1.0);
    module.regions[child.index()]
        .statements
        .push(Statement::Let {
            binding: local,
            value: Some(value),
        });
    module.regions[0].statements.push(Statement::Block(child));
    let leaked = expr(&mut module, Expr::Binding(local));
    capture(&mut module, leaked);
    assert!(module.verify().unwrap_err().contains("lexical scope"));

    let mut transfer = Module::default();
    transfer.regions[0].statements.push(Statement::Continue);
    assert!(transfer.verify().unwrap_err().contains("outside a loop"));
}

#[test]
fn a_shared_value_cannot_accidentally_duplicate_a_call_or_an_expression_graph() {
    let mut module = Module::default();
    let callee = host(&mut module, "effect");
    let value = call(&mut module, callee, vec![], Invocation::Value);
    let twice = expr(&mut module, Expr::Array(vec![value, value]));
    capture(&mut module, twice);
    assert!(module
        .verify()
        .unwrap_err()
        .contains("expression occurrence is shared"));
}

#[test]
fn renaming_cannot_capture_host_names_or_another_lexical_binding() {
    let mut module = Module::default();
    let local = binding(&mut module, RegionId::new(0), 0, "capture");
    let value = number(&mut module, 1.0);
    module.regions[0].statements.push(Statement::Let {
        binding: local,
        value: Some(value),
    });
    capture(&mut module, value);
    assert_eq!(execute(&module, "", PrintPolicy::default()), "[1]");
    assert_eq!(
        execute(
            &module,
            "",
            PrintPolicy {
                mangle_bindings: true
            }
        ),
        "[1]"
    );
    module.bindings[local.index()].pinned = true;
    assert!(module
        .render(PrintPolicy::default())
        .unwrap_err()
        .contains("external identifier"));
}

#[test]
fn explicit_print_policy_is_reentrant_and_preserves_provenance() {
    let mut module = Module::default();
    let symbol = binding(&mut module, RegionId::new(0), 0, "sourceName");
    let nodes = crate::ast::SourceNodes::default();
    let source = nodes.expression(crate::ast::ExprKind::Int(7, crate::span::Span::new(0, 1)));
    let value = module.expression(Expr::Literal(Literal::Number(7.0)), Some(source.id));
    module.regions[0].statements.push(Statement::Let {
        binding: symbol,
        value: Some(value),
    });
    let plain = module.render(PrintPolicy::default()).unwrap();
    let small = module
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    assert_ne!(plain, small);
    assert_eq!(plain, module.render(PrintPolicy::default()).unwrap());
    assert_eq!(module.origins[value.index()], Some(source.id));
}

fn reflected_function(module: &mut Module, arrow: bool, name: &str) -> ExprId {
    let body = module.region(ScopeId::new(0));
    let value = expr(module, Expr::This);
    module.regions[body.index()]
        .statements
        .push(Statement::Return(Some(value)));
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters: vec![],
        body,
        arrow,
        name: FunctionName::Exact(name.into()),
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
    });
    expr(module, Expr::Function(function))
}

#[test]
fn callable_name_materialization_preserves_anonymity_descriptors_and_construction() {
    for arrow in [false, true] {
        for name in ["", "retained", "__proto__", "two words", "quoted\"line\n"] {
            let mut module = Module::default();
            let destination = binding(&mut module, RegionId::new(0), 0, "destination");
            module.bindings[destination.index()].pinned = true;
            let value = reflected_function(&mut module, arrow, name);
            module.regions[0].statements.push(Statement::Let {
                binding: destination,
                value: Some(value),
            });
            let describe = host(&mut module, "describe");
            let value = expr(&mut module, Expr::Binding(destination));
            let described = call(&mut module, describe, vec![value], Invocation::Value);
            capture(&mut module, described);
            let expected =
                serde_json::json!([[name, 0, true, !arrow, false, false, true]]).to_string();
            let setup = r#"
                function describe(fn){
                    let constructed;
                    try{constructed=new fn instanceof fn}catch{constructed=false}
                    const descriptor=Object.getOwnPropertyDescriptor(fn,'name');
                    return [fn.name,fn.length,fn()===undefined,constructed,
                        descriptor.writable,descriptor.enumerable,descriptor.configurable];
                }
            "#;
            assert_eq!(
                execute(
                    &module,
                    setup,
                    PrintPolicy {
                        mangle_bindings: true
                    }
                ),
                expected
            );
        }
    }
    let mut module = Module::default();
    let value = reflected_function(&mut module, false, "kept");
    let invoked = call(&mut module, value, vec![], Invocation::Value);
    capture(&mut module, invoked);
    let value = reflected_function(&mut module, false, "constructorName");
    let instance = expr(
        &mut module,
        Expr::Construct {
            callee: value,
            arguments: vec![],
        },
    );
    let constructor = expr(
        &mut module,
        Expr::Member {
            object: instance,
            property: Property::Named("constructor".into()),
        },
    );
    let name = expr(
        &mut module,
        Expr::Member {
            object: constructor,
            property: Property::Named("name".into()),
        },
    );
    capture(&mut module, name);
    assert_eq!(
        execute(&module, "", PrintPolicy::default()),
        "[null,\"constructorName\"]"
    );
}

#[test]
fn anonymous_callable_values_do_not_acquire_object_property_names() {
    for computed in [false, true] {
        let mut module = Module::default();
        let value = reflected_function(&mut module, true, "");
        let property = if computed {
            Property::Computed(host(&mut module, "key"))
        } else {
            Property::Named("slot".into())
        };
        let object = expr(&mut module, Expr::Object(vec![(property, value)]));
        let value = expr(
            &mut module,
            Expr::Member {
                object,
                property: Property::Named("slot".into()),
            },
        );
        let name = expr(
            &mut module,
            Expr::Member {
                object: value,
                property: Property::Named("name".into()),
            },
        );
        capture(&mut module, name);
        let trace = host(&mut module, "trace");
        capture(&mut module, trace);
        assert_eq!(
            execute(
                &module,
                "const trace=[];const key={[Symbol.toPrimitive](){trace.push('key');return 'slot'}};",
                PrintPolicy::default()
            ),
            if computed {
                "[\"\",[\"key\"]]"
            } else {
                "[\"\",[]]"
            }
        );
    }
}

#[test]
fn exception_region_verification_enforces_ownership_scope_and_control_boundaries() {
    let mut missing = Module::default();
    let body = missing.region(ScopeId::new(0));
    missing.regions[0].statements.push(Statement::Try {
        body,
        catch: None,
        finally: None,
    });
    assert!(missing
        .verify()
        .unwrap_err()
        .contains("requires a catch or finally"));

    let mut shared = Module::default();
    let body = shared.region(ScopeId::new(0));
    shared.regions[0].statements.push(Statement::Try {
        body,
        catch: None,
        finally: Some(body),
    });
    assert!(shared.verify().unwrap_err().contains("shared"));

    let mut wrong_scope = Module::default();
    let body = wrong_scope.region(ScopeId::new(0));
    let handler = wrong_scope.region(ScopeId::new(0));
    let root_binding = binding(&mut wrong_scope, RegionId::new(0), 0, "caught");
    wrong_scope.regions[0].statements.push(Statement::Try {
        body,
        catch: Some(Catch {
            binding: Some(root_binding),
            body: handler,
        }),
        finally: None,
    });
    assert!(wrong_scope
        .verify()
        .unwrap_err()
        .contains("binding declaration"));

    for transfer in [
        Statement::Break,
        Statement::Continue,
        Statement::Return(None),
    ] {
        let mut invalid = Module::default();
        let body = invalid.region(ScopeId::new(0));
        let finalizer = invalid.region(ScopeId::new(0));
        invalid.regions[finalizer.index()].statements.push(transfer);
        invalid.regions[0].statements.push(Statement::Try {
            body,
            catch: None,
            finally: Some(finalizer),
        });
        assert!(invalid.verify().unwrap_err().contains("outside"));
    }
}

#[test]
fn primitive_construction_verification_rejects_invalid_operators_and_arity() {
    for (operation, count) in [
        (Intrinsic::ArrayBufferNew, 0),
        (Intrinsic::ArrayBufferNew, 2),
        (Intrinsic::MapNew, 1),
        (Intrinsic::SetNew, 1),
        (Intrinsic::ArrayLength, 1),
    ] {
        let mut module = Module::default();
        let size = number(&mut module, 4.0);
        let value = expr(
            &mut module,
            Expr::ConstructIntrinsic {
                operation,
                arguments: vec![size; count],
            },
        );
        module.regions[0]
            .statements
            .push(Statement::Evaluate(value));
        assert!(module
            .verify()
            .unwrap_err()
            .contains("primitive construction"));
    }
}

#[test]
fn primitive_identity_is_resolved_before_backend_selection() {
    use crate::primitive::ResolvedIntrinsic::{Method, Property};
    let source = r#"
        class Custom {int charCodeAt(int index){return index+100;} int map(int index){return index+200;}}
        int typed(string text,int index){return text.charCodeAt(index);}
        int custom(Custom object,int index){return object.charCodeAt(index);}
        int count(int[] values){return values.length;}
        int mapped(int[] values){return values.map((int value)=>value+1).length;}
        int customMap(Custom object,int index){return object.map(index);}
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let returned = |index: usize| {
        let crate::ast::Item::Function(function) = &program.items[index] else {
            panic!("expected function")
        };
        let crate::ast::Stmt::Return {
            value: Some(value), ..
        } = &function.body[0]
        else {
            panic!("expected return")
        };
        value
    };
    let method = |index: usize| {
        let crate::ast::ExprKind::Call { callee, .. } = &returned(index).kind else {
            panic!("expected call")
        };
        semantics.resolved_intrinsic(callee.id)
    };
    let crate::ast::ExprKind::Member { object: mapped, .. } = &returned(4).kind else {
        panic!("expected result length")
    };
    let crate::ast::ExprKind::Call { callee: map, .. } = &mapped.kind else {
        panic!("expected map call")
    };
    assert_eq!(method(1), Some(Method(Intrinsic::StringCharCodeAt)));
    assert_eq!(method(2), None);
    assert_eq!(
        semantics.resolved_intrinsic(returned(3).id),
        Some(Property(Intrinsic::ArrayLength))
    );
    assert_eq!(
        semantics.resolved_intrinsic(map.id),
        Some(Method(Intrinsic::ArrayMap))
    );
    assert_eq!(method(5), None);

    let mut module = Module::default();
    let receiver = expr(&mut module, Expr::Literal(Literal::String("abc".into())));
    let invalid = expr(
        &mut module,
        Expr::Intrinsic {
            operation: Intrinsic::StringCharCodeAt,
            receiver,
            arguments: vec![],
        },
    );
    module.regions[0]
        .statements
        .push(Statement::Evaluate(invalid));
    assert_eq!(
        module.verify().unwrap_err(),
        "unsupported intrinsic or invalid operand count"
    );
}

#[test]
fn normalized_strings_preserve_utf16_values_and_javascript_escape_semantics() {
    let mut module = Module::default();
    for literal in [r"\x41\uD800", r"\ud83d\ude00", r#"a\n\t\\\"b"#, r"\0\v\z"] {
        let value = expr(
            &mut module,
            Expr::Literal(Literal::String(
                StringValue::decode_source(literal).unwrap(),
            )),
        );
        capture(&mut module, value);
    }
    assert_eq!(
        execute(&module, "", PrintPolicy::default()),
        r#"["A\ud800","😀","a\n\t\\\"b","\u0000\u000bz"]"#
    );
}

#[test]
fn equivalent_escaped_record_keys_have_one_semantic_identity() {
    let arena = bumpalo::Bump::new();
    let program =
        crate::parse_source(&arena, r#"Record<int> values=record{a:1,"\x61":2};"#).unwrap();
    let error = crate::check::analyze(&program).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
}

#[test]
fn exports_reject_missing_bindings_duplicate_names_and_nested_cells() {
    let mut module = Module::default();
    module.exports.push(Export {
        binding: BindingId::new(0),
        name: "missing".into(),
    });
    assert!(module.verify().is_err());
    let scope = module.regions[module.root.index()].scope;
    let value = module.binding(Binding {
        source_symbol: None,
        scope,
        spelling: "value".into(),
        pinned: false,
    });
    let initial = module.expression(Expr::Literal(Literal::Number(1.0)), None);
    module.regions[module.root.index()]
        .statements
        .push(Statement::Let {
            binding: value,
            value: Some(initial),
        });
    module.exports[0] = Export {
        binding: value,
        name: "public".into(),
    };
    module.verify().unwrap();
    module.exports.push(Export {
        binding: value,
        name: "public".into(),
    });
    assert!(module.verify().unwrap_err().contains("duplicate"));
    module.exports.pop();
    let nested = module.region(scope);
    let local = module.binding(Binding {
        source_symbol: None,
        scope: module.regions[nested.index()].scope,
        spelling: "local".into(),
        pinned: false,
    });
    module.regions[nested.index()]
        .statements
        .push(Statement::Let {
            binding: local,
            value: None,
        });
    module.regions[module.root.index()]
        .statements
        .push(Statement::Block(nested));
    module.exports[0].binding = local;
    assert!(module.verify().is_err());
}

#[test]
fn a_sequenced_computed_object_key_is_parenthesized() {
    // A computed key is an AssignmentExpression: `{[(a,b)]:v}`, never
    // `{[a,b]:v}`. Effects scheduled before a key keep their order.
    let mut module = Module::default();
    let note = host(&mut module, "capture");
    let first = number(&mut module, 1.0);
    let effect = call(&mut module, note, vec![first], Invocation::Value);
    let name = expr(&mut module, Expr::Literal(Literal::String("a".into())));
    let key = expr(&mut module, Expr::Sequence(vec![effect, name]));
    let two = number(&mut module, 2.0);
    let object = expr(
        &mut module,
        Expr::Object(vec![(Property::Computed(key), two)]),
    );
    let read = expr(&mut module, Expr::Literal(Literal::String("a".into())));
    let value = expr(
        &mut module,
        Expr::Member {
            object,
            property: Property::Computed(read),
        },
    );
    capture(&mut module, value);
    assert_eq!(execute(&module, "", PrintPolicy::default()), "[1,2]");
}

/// With `loop_head_declarations`, a block's counter moves into the for head,
/// unless a closure in the loop captures it: the head's binding is fresh
/// each iteration, the block's is shared.

#[test]
fn a_loop_counter_moves_into_the_for_head_only_when_no_closure_captures_it() {
    let build = |captured: bool| {
        let mut module = Module::default();
        module.loop_head_declarations = true;
        let block = module.region(ScopeId::new(0));
        let body = module.region(module.regions[block.index()].scope);
        let counter = binding(&mut module, block, 0, "i");
        let zero = number(&mut module, 0.0);
        module.regions[block.index()]
            .statements
            .push(Statement::Let {
                binding: counter,
                value: Some(zero),
            });
        let read = expr(&mut module, Expr::Binding(counter));
        let three = number(&mut module, 3.0);
        let condition = expr(
            &mut module,
            Expr::Binary {
                op: Binary::Less,
                left: read,
                right: three,
            },
        );
        let target = expr(&mut module, Expr::Binding(counter));
        let current = expr(&mut module, Expr::Binding(counter));
        let one = number(&mut module, 1.0);
        let next = expr(
            &mut module,
            Expr::Binary {
                op: Binary::Add,
                left: current,
                right: one,
            },
        );
        let update = expr(
            &mut module,
            Expr::Assign {
                target,
                value: next,
            },
        );
        let value = if captured {
            let closure = module.region(module.regions[body.index()].scope);
            let read = expr(&mut module, Expr::Binding(counter));
            module.regions[closure.index()]
                .statements
                .push(Statement::Return(Some(read)));
            module.functions.push(Function {
                name: FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::js::Suspension::None,
                arrow: true,
                parameters: vec![],
                body: closure,
            });
            expr(&mut module, Expr::Function(FunctionId::new(0)))
        } else {
            expr(&mut module, Expr::Binding(counter))
        };
        let output = host(&mut module, "capture");
        let recorded = call(&mut module, output, vec![value], Invocation::Value);
        module.regions[body.index()]
            .statements
            .push(Statement::Evaluate(recorded));
        module.regions[block.index()]
            .statements
            .push(Statement::Loop {
                condition: Some(condition),
                update: Some(update),
                body,
            });
        module.regions[0].statements.push(Statement::Block(block));
        module
    };
    let policy = PrintPolicy {
        mangle_bindings: false,
    };
    let plain = build(false);
    let text = plain.render(policy).unwrap();
    assert!(text.starts_with("for(let i=0;i<3;i=i+1)"), "{text}");
    assert_eq!(execute(&plain, "", policy), "[0,1,2]");
    let captured = build(true);
    let text = captured.render(policy).unwrap();
    assert!(text.starts_with("{let i=0;for(;"), "{text}");
    assert_eq!(
        execute(&captured, "globalThis.x=0;", policy).len(),
        "[null,null,null]".len()
    );
}

/// With `logical_statements`, a one-statement `if` prints as `&&` or `||`
/// only where neither side needs grouping.

#[test]
fn single_statement_ifs_print_as_logical_expressions_where_shorter() {
    let build = |negate: bool, assign: bool| {
        let mut module = Module::default();
        module.logical_statements = true;
        let yes = module.region(module.regions[0].scope);
        let flag = host(&mut module, "flag");
        let condition = if negate {
            expr(
                &mut module,
                Expr::Unary {
                    op: Unary::Not,
                    value: flag,
                },
            )
        } else {
            flag
        };
        let statement = if assign {
            let target = host(&mut module, "slot");
            let value = number(&mut module, 1.0);
            expr(&mut module, Expr::Assign { target, value })
        } else {
            let output = host(&mut module, "capture");
            let value = number(&mut module, 7.0);
            call(&mut module, output, vec![value], Invocation::Value)
        };
        module.regions[yes.index()]
            .statements
            .push(Statement::Evaluate(statement));
        module.regions[0].statements.push(Statement::If {
            condition,
            yes,
            no: None,
        });
        module
    };
    let policy = PrintPolicy {
        mangle_bindings: false,
    };
    assert_eq!(
        build(false, false).render(policy).unwrap(),
        "flag&&capture(7);"
    );
    assert_eq!(
        build(true, false).render(policy).unwrap(),
        "flag||capture(7);"
    );
    assert_eq!(
        build(false, true).render(policy).unwrap(),
        "if(flag)slot=1;"
    );
    assert_eq!(
        execute(&build(true, false), "globalThis.flag=false;", policy),
        "[7]"
    );
    assert_eq!(
        execute(&build(false, false), "globalThis.flag=false;", policy),
        "[]"
    );
}

/// `let f=()=>x;let x=f();` reads `x` before its declaration completes and
/// throws. Moved under `let x;`, the body would read `undefined` instead, so
/// the call stays.

#[test]
fn a_call_initializing_a_binding_its_function_reads_stays_a_call() {
    let mut module = Module::default();
    let root = RegionId::new(0);
    let root_scope = module.regions[root.index()].scope;
    let f = binding(&mut module, root, 1, "f");
    let x = binding(&mut module, root, 2, "x");
    let body = module.region(root_scope);
    let read = expr(&mut module, Expr::Binding(x));
    module.regions[body.index()].statements = vec![Statement::Return(Some(read))];
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters: vec![],
        body,
        arrow: true,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    let created = expr(&mut module, Expr::Function(function));
    let callee = expr(&mut module, Expr::Binding(f));
    let called = call(&mut module, callee, vec![], Invocation::Value);
    let result = expr(&mut module, Expr::Binding(x));
    let output = host(&mut module, "capture");
    let captured = call(&mut module, output, vec![result], Invocation::Value);
    module.regions[root.index()].statements = vec![
        Statement::Let {
            binding: f,
            value: Some(created),
        },
        Statement::Let {
            binding: x,
            value: Some(called),
        },
        Statement::Evaluate(captured),
    ];
    module.root_modules = vec![0; 3];
    module.verify().unwrap();
    let mut budget = AllocationBudget::new(None);
    assert_eq!(module.inline_single_calls(true, &mut budget).unwrap(), 0);
    module.verify().unwrap();
}

/// A function called once, as a statement, becomes a block there; its tail
/// returns store the call's result. One that returns early stays.

#[test]
fn a_function_called_once_becomes_a_block_at_its_call() {
    let mut module = Module::default();
    let root = RegionId::new(0);
    let root_scope = module.regions[root.index()].scope;
    let f = binding(&mut module, root, 1, "f");
    let result = binding(&mut module, root, 2, "result");
    let body = module.region(root_scope);
    let body_scope = module.regions[body.index()].scope;
    let parameter = binding(&mut module, body, 3, "p");
    // (p)=>{capture(p);if(p)return 1;else return 2}
    let yes = module.region(body_scope);
    let one = number(&mut module, 1.0);
    module.regions[yes.index()].statements = vec![Statement::Return(Some(one))];
    let no = module.region(body_scope);
    let two = number(&mut module, 2.0);
    module.regions[no.index()].statements = vec![Statement::Return(Some(two))];
    let test = expr(&mut module, Expr::Binding(parameter));
    let seen = expr(&mut module, Expr::Binding(parameter));
    let output = host(&mut module, "capture");
    let captured = call(&mut module, output, vec![seen], Invocation::Value);
    module.regions[body.index()].statements = vec![
        Statement::Evaluate(captured),
        Statement::If {
            condition: test,
            yes,
            no: Some(no),
        },
    ];
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters: vec![parameter],
        body,
        arrow: true,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    let created = expr(&mut module, Expr::Function(function));
    let callee = expr(&mut module, Expr::Binding(f));
    let argument = host(&mut module, "flag");
    let called = call(&mut module, callee, vec![argument], Invocation::Value);
    let read = expr(&mut module, Expr::Binding(result));
    let output = host(&mut module, "capture");
    let reported = call(&mut module, output, vec![read], Invocation::Value);
    module.regions[root.index()].statements = vec![
        Statement::Let {
            binding: f,
            value: Some(created),
        },
        Statement::Let {
            binding: result,
            value: Some(called),
        },
        Statement::Evaluate(reported),
    ];
    module.root_modules = vec![0; 3];
    module.verify().unwrap();
    let before = [
        execute(
            &module,
            "const flag=0;",
            PrintPolicy {
                mangle_bindings: false,
            },
        ),
        execute(
            &module,
            "const flag=1;",
            PrintPolicy {
                mangle_bindings: false,
            },
        ),
    ];
    // The early form, `if(p)return 1;…`, would need a loop to leave.
    let mut early = module.clone();
    early.regions[body.index()].statements.swap(0, 1);
    let mut budget = AllocationBudget::new(None);
    assert_eq!(early.inline_single_calls(true, &mut budget).unwrap(), 0);
    assert_eq!(module.inline_single_calls(true, &mut budget).unwrap(), 1);
    module.verify().unwrap();
    let javascript = module
        .render(PrintPolicy {
            mangle_bindings: false,
        })
        .unwrap();
    assert!(
        !javascript.contains("for(;;)") && !javascript.contains("=>"),
        "{javascript}"
    );
    assert_eq!(before[0], "[0,2]");
    assert_eq!(before[1], "[1,1]");
    assert_eq!(
        execute(
            &module,
            "const flag=0;",
            PrintPolicy {
                mangle_bindings: false
            }
        ),
        before[0]
    );
    assert_eq!(
        execute(
            &module,
            "const flag=1;",
            PrintPolicy {
                mangle_bindings: false
            }
        ),
        before[1]
    );
}

/// `let j={k:x};capture(0);capture(j)` creates the object at its one use:
/// reading `x` there runs nothing and yields the same value. An assignment
/// to `x` in between keeps it; so does a read of `x` in its TDZ, which throws
/// before `capture(0)` runs; and so does a parameter that `arguments` aliases.

#[test]
fn a_value_of_settled_reads_is_created_at_its_one_use() {
    // `let x=1;[before]let j={k:x};[between]capture(0);capture(j);capture(x)`.
    let build = |assigned: bool| {
        let mut module = Module::default();
        let root = RegionId::new(0);
        let x = binding(&mut module, root, 1, "x");
        let j = binding(&mut module, root, 2, "j");
        let one = number(&mut module, 1.0);
        let read = expr(&mut module, Expr::Binding(x));
        let object = expr(
            &mut module,
            Expr::Object(vec![(Property::Named("k".into()), read)]),
        );
        let zero = number(&mut module, 0.0);
        let output = host(&mut module, "capture");
        let first = call(&mut module, output, vec![zero], Invocation::Value);
        let used = expr(&mut module, Expr::Binding(j));
        let output = host(&mut module, "capture");
        let second = call(&mut module, output, vec![used], Invocation::Value);
        let later = expr(&mut module, Expr::Binding(x));
        let output = host(&mut module, "capture");
        let third = call(&mut module, output, vec![later], Invocation::Value);
        let mut statements = vec![
            Statement::Let {
                binding: x,
                value: Some(one),
            },
            Statement::Let {
                binding: j,
                value: Some(object),
            },
        ];
        if assigned {
            let target = expr(&mut module, Expr::Binding(x));
            let two = number(&mut module, 2.0);
            let assign = expr(&mut module, Expr::Assign { target, value: two });
            statements.push(Statement::Evaluate(assign));
        }
        statements.extend([
            Statement::Evaluate(first),
            Statement::Evaluate(second),
            Statement::Evaluate(third),
        ]);
        module.root_modules = vec![0; statements.len()];
        module.regions[root.index()].statements = statements;
        module.verify().unwrap();
        module
    };
    let policy = PrintPolicy {
        mangle_bindings: false,
    };
    let mut budget = AllocationBudget::new(None);
    let mut moved = build(false);
    assert_eq!(moved.forward_single_uses(&mut budget).unwrap().0, 1);
    moved.verify().unwrap();
    assert_eq!(
        moved.render(policy).unwrap(),
        "let x=1;capture(0);capture({k:x});capture(x);"
    );
    assert_eq!(execute(&moved, "", policy), "[0,{\"k\":1},1]");
    let mut kept = build(true);
    assert_eq!(kept.forward_single_uses(&mut budget).unwrap().0, 0);
    assert_eq!(execute(&kept, "", policy), "[0,{\"k\":1},2]");

    // `{let j={k:x};capture(0);capture(j)}let x=1;`: the read throws first.
    let mut module = Module::default();
    let root = RegionId::new(0);
    let root_scope = module.regions[root.index()].scope;
    let x = binding(&mut module, root, 1, "x");
    let block = module.region(root_scope);
    let j = binding(&mut module, block, 2, "j");
    let read = expr(&mut module, Expr::Binding(x));
    let object = expr(
        &mut module,
        Expr::Object(vec![(Property::Named("k".into()), read)]),
    );
    let zero = number(&mut module, 0.0);
    let output = host(&mut module, "capture");
    let first = call(&mut module, output, vec![zero], Invocation::Value);
    let used = expr(&mut module, Expr::Binding(j));
    let output = host(&mut module, "capture");
    let second = call(&mut module, output, vec![used], Invocation::Value);
    module.regions[block.index()].statements = vec![
        Statement::Let {
            binding: j,
            value: Some(object),
        },
        Statement::Evaluate(first),
        Statement::Evaluate(second),
    ];
    let one = number(&mut module, 1.0);
    module.regions[root.index()].statements = vec![
        Statement::Block(block),
        Statement::Let {
            binding: x,
            value: Some(one),
        },
    ];
    module.root_modules = vec![0; 2];
    module.verify().unwrap();
    assert_eq!(module.forward_single_uses(&mut budget).unwrap().0, 0);

    // `function(p){let j={k:p};arguments[0]=2;capture(j)}`: in a sloppy frame
    // the store assigns `p`.
    let mut module = Module::default();
    let root = RegionId::new(0);
    let root_scope = module.regions[root.index()].scope;
    let f = binding(&mut module, root, 1, "f");
    let body = module.region(root_scope);
    let p = binding(&mut module, body, 2, "p");
    let j = binding(&mut module, body, 3, "j");
    let read = expr(&mut module, Expr::Binding(p));
    let object = expr(
        &mut module,
        Expr::Object(vec![(Property::Named("k".into()), read)]),
    );
    let arguments = host(&mut module, "arguments");
    let key = number(&mut module, 0.0);
    let target = expr(
        &mut module,
        Expr::Member {
            object: arguments,
            property: Property::Computed(key),
        },
    );
    let two = number(&mut module, 2.0);
    let store = expr(&mut module, Expr::Assign { target, value: two });
    let used = expr(&mut module, Expr::Binding(j));
    let output = host(&mut module, "capture");
    let captured = call(&mut module, output, vec![used], Invocation::Value);
    module.regions[body.index()].statements = vec![
        Statement::Let {
            binding: j,
            value: Some(object),
        },
        Statement::Evaluate(store),
        Statement::Evaluate(captured),
    ];
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters: vec![p],
        body,
        arrow: false,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    module.regions[root.index()].statements = vec![Statement::Function {
        binding: f,
        function,
    }];
    module.exports.push(Export {
        binding: f,
        name: "f".into(),
    });
    module.root_modules = vec![0];
    module.verify().unwrap();
    assert_eq!(module.forward_single_uses(&mut budget).unwrap().0, 0);
}

#[test]
fn sibling_branches_cannot_share_a_scope() {
    let mut invalid = Module::default();
    let yes = invalid.region(ScopeId::new(0));
    let no = invalid.region(ScopeId::new(0));
    invalid.regions[no.index()].scope = invalid.regions[yes.index()].scope;
    let condition = number(&mut invalid, 1.0);
    invalid.regions[0].statements.push(Statement::If {
        condition,
        yes,
        no: Some(no),
    });
    assert!(invalid.verify().unwrap_err().contains("scope is shared"));
}

#[test]
fn a_loop_update_cannot_be_reused_as_a_body_statement() {
    let mut module = Module::default();
    let body = module.region(ScopeId::new(0));
    let counter = binding(&mut module, RegionId::new(0), 1, "counter");
    let zero = number(&mut module, 0.0);
    module.regions[0].statements.push(Statement::Let {
        binding: counter,
        value: Some(zero),
    });
    let read = expr(&mut module, Expr::Binding(counter));
    let two = number(&mut module, 2.0);
    let condition = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Less,
            left: read,
            right: two,
        },
    );
    let target = expr(&mut module, Expr::Binding(counter));
    let current = expr(&mut module, Expr::Binding(counter));
    let one = number(&mut module, 1.0);
    let next = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Add,
            left: current,
            right: one,
        },
    );
    let update = expr(
        &mut module,
        Expr::Assign {
            target,
            value: next,
        },
    );
    module.regions[0].statements.push(Statement::Loop {
        condition: Some(condition),
        update: Some(update),
        body,
    });
    module.verify().unwrap();
    // Reusing a loop update as a body occurrence would execute it twice.
    module.regions[body.index()]
        .statements
        .push(Statement::Evaluate(update));
    assert!(module
        .verify()
        .unwrap_err()
        .contains("occurrence is shared"));
}

#[test]
fn an_integer_negation_keeps_the_effects_of_its_operand() {
    let mut module = Module::default();
    let observed = host(&mut module, "observe");
    let observed = call(&mut module, observed, vec![], Invocation::Value);
    let zero = number(&mut module, 0.0);
    let sequence = expr(&mut module, Expr::Sequence(vec![observed, zero]));
    let negated = expr(&mut module, Expr::IntNegate(sequence));
    capture(&mut module, negated);
    assert_eq!(
        execute(
            &module,
            "function observe(){capture('effect')}",
            PrintPolicy::default()
        ),
        "[\"effect\",0]"
    );
}

#[test]
fn direct_eval_writes_keep_integer_normalization() {
    let mut module = Module::default();
    let symbol = binding(&mut module, RegionId::new(0), 1000, "value");
    let one = number(&mut module, 1.0);
    module.regions[0].statements.push(Statement::Let {
        binding: symbol,
        value: Some(one),
    });
    let eval = host(&mut module, "eval");
    let source = expr(
        &mut module,
        Expr::Literal(Literal::String(
            StringValue::decode_source("value=1.5").unwrap(),
        )),
    );
    let eval = call(&mut module, eval, vec![source], Invocation::DirectEval);
    module.regions[0].statements.push(Statement::Evaluate(eval));
    let read = expr(&mut module, Expr::Binding(symbol));
    let negate = expr(&mut module, Expr::IntNegate(read));
    capture(&mut module, negate);
    for mangle_bindings in [false, true] {
        assert_eq!(
            execute(&module, "", PrintPolicy { mangle_bindings }),
            "[-1]"
        );
    }
}

#[test]
fn template_parts_print_null_undefined_and_negative_zero() {
    let mut module = Module::default();
    let null = expr(&mut module, Expr::Literal(Literal::Null));
    let undefined = expr(&mut module, Expr::Literal(Literal::Undefined));
    let negative_zero = number(&mut module, -0.0);
    let text = expr(
        &mut module,
        Expr::Template(vec![
            TemplatePart::Expression(null),
            TemplatePart::Expression(undefined),
            TemplatePart::Expression(negative_zero),
        ]),
    );
    capture(&mut module, text);
    for mangle_bindings in [false, true] {
        assert_eq!(
            execute(&module, "", PrintPolicy { mangle_bindings }),
            r#"["nullundefined0"]"#
        );
    }
}

/// A host getter or a key's `toString` may write a cell the expression reads.
/// The printed program keeps the evaluation order: receiver lookup before
/// arguments, key coercion before the value in an object literal, and, for a
/// simple assignment, key coercion after the right-hand side (PutValue).
#[test]
fn host_getters_and_key_coercion_run_in_evaluation_order() {
    for scenario in 0..3 {
        let mut module = Module::default();
        let cell = binding(&mut module, RegionId::new(0), 1000, "cell");
        let object = binding(&mut module, RegionId::new(0), 1001, "holder");
        let key = binding(&mut module, RegionId::new(0), 1002, "key");
        let one = number(&mut module, 1.0);
        module.regions[0].statements.push(Statement::Let {
            binding: cell,
            value: Some(one),
        });
        let writer_body = module.region(ScopeId::new(0));
        let target = expr(&mut module, Expr::Binding(cell));
        let nine = number(&mut module, 9.0);
        let write = expr(
            &mut module,
            Expr::Assign {
                target,
                value: nine,
            },
        );
        module.regions[writer_body.index()]
            .statements
            .push(Statement::Evaluate(write));
        module.functions.push(Function {
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: crate::js::Suspension::None,
            arrow: true,
            parameters: vec![],
            body: writer_body,
        });
        let writer = expr(&mut module, Expr::Function(FunctionId::new(0)));
        let install = host(&mut module, "install");
        let install = call(&mut module, install, vec![writer], Invocation::Value);
        module.regions[0]
            .statements
            .push(Statement::Evaluate(install));
        let host_object = host(&mut module, "foreignObject");
        let host_key = host(&mut module, "foreignKey");
        module.regions[0].statements.push(Statement::Let {
            binding: object,
            value: Some(host_object),
        });
        module.regions[0].statements.push(Statement::Let {
            binding: key,
            value: Some(host_key),
        });
        let target = expr(&mut module, Expr::Binding(cell));
        let reset = expr(&mut module, Expr::Assign { target, value: one });
        module.regions[0]
            .statements
            .push(Statement::Evaluate(reset));
        let holder = expr(&mut module, Expr::Binding(object));
        let key_read = expr(&mut module, Expr::Binding(key));
        let read = expr(&mut module, Expr::Binding(cell));
        let negative = expr(&mut module, Expr::IntNegate(read));
        match scenario {
            0 => {
                let method = expr(
                    &mut module,
                    Expr::Member {
                        object: holder,
                        property: Property::Named("method".into()),
                    },
                );
                let result = call(&mut module, method, vec![negative], Invocation::Reference);
                capture(&mut module, result);
            }
            1 => {
                let object = expr(
                    &mut module,
                    Expr::Object(vec![(Property::Computed(key_read), negative)]),
                );
                capture(&mut module, object);
            }
            2 => {
                let target = expr(
                    &mut module,
                    Expr::Member {
                        object: holder,
                        property: Property::Computed(key_read),
                    },
                );
                let write = expr(
                    &mut module,
                    Expr::Assign {
                        target,
                        value: negative,
                    },
                );
                module.regions[0]
                    .statements
                    .push(Statement::Evaluate(write));
                let holder = expr(&mut module, Expr::Binding(object));
                let read = expr(
                    &mut module,
                    Expr::Member {
                        object: holder,
                        property: Property::Named("x".into()),
                    },
                );
                capture(&mut module, read);
            }
            _ => unreachable!(),
        }
        let setup = "let writer;function install(value){writer=value}const foreignObject={get method(){writer();return value=>value}};const foreignKey={toString(){writer();return 'x'}};";
        let expected = match scenario {
            0 => "[-9]",
            1 => "[{\"x\":-9}]",
            2 => "[-1]",
            _ => unreachable!(),
        };
        for mangle_bindings in [false, true] {
            assert_eq!(
                execute(&module, setup, PrintPolicy { mangle_bindings }),
                expected,
                "{scenario}"
            );
        }
    }
}

/// Structural equality for the exit rules: the same operations on the same
/// bindings and literals, numbers by bits; a created function is its own.
#[test]
fn same_expression_compares_structure_numbers_by_bits_and_never_functions() {
    let mut module = Module::default();
    let region = module.root;
    let x = binding(&mut module, region, 0, "x");
    let mut sum = |module: &mut Module, right: f64| {
        let left = expr(module, Expr::Binding(x));
        let right = number(module, right);
        expr(
            module,
            Expr::Binary {
                op: Binary::Add,
                left,
                right,
            },
        )
    };
    let (a, b, c) = (
        sum(&mut module, 1.0),
        sum(&mut module, 1.0),
        sum(&mut module, 2.0),
    );
    let (zero, negative_zero) = (number(&mut module, 0.0), number(&mut module, -0.0));
    let (nan, other_nan) = (number(&mut module, f64::NAN), number(&mut module, f64::NAN));
    let body = module.region(module.regions[0].scope);
    module.functions.push(Function {
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: crate::js::Suspension::None,
        arrow: true,
        parameters: vec![],
        body,
    });
    let first = expr(&mut module, Expr::Function(FunctionId::new(0)));
    let second = expr(&mut module, Expr::Function(FunctionId::new(0)));
    let mut budget = AllocationBudget::new(None);
    let mut same = |left, right| module.same_expression(left, right, &mut budget).unwrap();
    assert!(same(a, b));
    assert!(!same(a, c));
    assert!(!same(zero, negative_zero));
    assert!(same(nan, other_nan));
    assert!(!same(first, second));
}

/// Exit to `break` in a `for…of`: leaving it closes the iterator, after a
/// return's value is evaluated but before the value read after a `break`,
/// so only a return of nothing or of a literal becomes a `break`.
#[test]
fn a_for_of_exit_becomes_a_break_only_when_it_evaluates_nothing() {
    for literal in [true, false] {
        let mut module = Module::default();
        let body = module.region(module.regions[0].scope);
        let loop_body = module.region(module.regions[body.index()].scope);
        let yes = module.region(module.regions[loop_body.index()].scope);
        let item = binding(&mut module, loop_body, 0, "item");
        let value = |module: &mut Module| {
            if literal {
                number(module, 7.0)
            } else {
                host(module, "value")
            }
        };
        let inner = value(&mut module);
        module.regions[yes.index()]
            .statements
            .push(Statement::Return(Some(inner)));
        let condition = expr(&mut module, Expr::Binding(item));
        module.regions[loop_body.index()]
            .statements
            .push(Statement::If {
                condition,
                yes,
                no: None,
            });
        let iterable = host(&mut module, "values");
        module.regions[body.index()]
            .statements
            .push(Statement::ForOf {
                binding: item,
                iterable,
                body: loop_body,
            });
        let after = value(&mut module);
        module.regions[body.index()]
            .statements
            .push(Statement::Return(Some(after)));
        module.functions.push(Function {
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: crate::js::Suspension::None,
            arrow: true,
            parameters: vec![],
            body,
        });
        let function = expr(&mut module, Expr::Function(FunctionId::new(0)));
        module.regions[0]
            .statements
            .push(Statement::Evaluate(function));
        module
            .compress_statements(StatementSpellings::NONE, &mut AllocationBudget::new(None))
            .unwrap();
        let exit = &module.regions[yes.index()].statements[0];
        assert_eq!(matches!(exit, Statement::Break), literal, "{exit:?}");
    }
}
