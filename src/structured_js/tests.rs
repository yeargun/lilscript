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
fn named_binding(module: &Module, name: &str) -> BindingId {
    BindingId::new(
        module
            .bindings
            .iter()
            .position(|binding| binding.spelling == name)
            .unwrap(),
    )
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
        suspension: crate::structured_js::Suspension::None,
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
        suspension: crate::structured_js::Suspension::None,
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
    let mut proof = verify::verify(&module).unwrap();
    let removed = compact::compact(&mut module, &mut proof);
    assert_eq!(
        (
            removed.bindings,
            removed.functions,
            removed.regions,
            removed.scopes
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(module.functions[0].parameters, vec![BindingId::new(2)]);
    assert_eq!(
        module
            .bindings
            .iter()
            .map(|binding| binding.source_symbol)
            .collect::<Vec<_>>(),
        vec![
            Some(SymbolId(42)),
            Some(SymbolId(77)),
            Some(SymbolId(77)),
            None
        ]
    );
    assert_eq!(proof, verify::verify(&module).unwrap());
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
            suspension: crate::structured_js::Suspension::None,
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
        suspension: crate::structured_js::Suspension::None,
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
        suspension: crate::structured_js::Suspension::None,
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

#[test]
fn source_operations_with_one_diagnostic_span_keep_distinct_resolution() {
    use crate::ast::{ArrayElement, ExprKind, Ident, Item, SourceNodes, Stmt};
    use crate::primitive::ResolvedIntrinsic::Property;
    use crate::semantic::BuiltinCall;
    let arena = bumpalo::Bump::new();
    let empty = crate::parse_source(&arena, "").unwrap();
    let nodes = SourceNodes::default();
    let span = crate::span::Span::empty(0);
    let ident = |name| Ident { name, span };
    // Generated syntax can have one diagnostic location while each occurrence
    // still has its own binding, operation and result obligations.
    let array = arena.alloc(nodes.expression(ExprKind::ArrayLiteral {
        elements: arena.alloc_slice_fill_iter([ArrayElement::Value(
            nodes.expression(ExprKind::Int(2, span)),
        )]),
        span,
    }));
    let array_length = nodes.expression(ExprKind::Member {
        object: array,
        property: ident("length"),
        span,
    });
    let string_length = nodes.expression(ExprKind::Member {
        object: arena.alloc(nodes.expression(ExprKind::String("😀x", span))),
        property: ident("length"),
        span,
    });
    let (array_id, string_id) = (array_length.id, string_length.id);
    let multiplied = nodes.expression(ExprKind::Call {
        callee: arena.alloc(nodes.expression(ExprKind::Member {
            object: arena.alloc(nodes.expression(ExprKind::Ident(ident("Math")))),
            property: ident("imul"),
            span,
        })),
        args: arena.alloc_slice_fill_iter([array_length, string_length].into_iter().map(
            |expression| crate::ast::Argument {
                span: expression.span(),
                expression,
                passing: crate::primitive::ParameterPassing::Value,
            },
        )),
        span,
    });
    let multiply_id = multiplied.id;
    let printed = nodes.expression(ExprKind::Call {
        callee: arena.alloc(nodes.expression(ExprKind::Ident(ident("print")))),
        args: arena.alloc_slice_fill_iter([multiplied].into_iter().map(|expression| {
            crate::ast::Argument {
                span: expression.span(),
                expression,
                passing: crate::primitive::ParameterPassing::Value,
            }
        })),
        span,
    });
    let print_id = printed.id;
    let program = empty.with_items(
        &nodes,
        arena.alloc_slice_fill_iter([Item::Stmt(Stmt::Expr(printed))]),
    );
    let semantics = crate::analyze(&program).unwrap();
    assert_eq!(
        semantics.resolved_intrinsic(array_id),
        Some(Property(Intrinsic::ArrayLength))
    );
    assert_eq!(
        semantics.resolved_intrinsic(string_id),
        Some(Property(Intrinsic::StringLength))
    );
    assert_eq!(
        semantics.builtin_call(multiply_id),
        Some(BuiltinCall::MathImul)
    );
    assert_eq!(semantics.builtin_call(print_id), Some(BuiltinCall::Print));
    assert_eq!(
        crate::interpreter::interpret_program(&program, &semantics).unwrap(),
        "3\n"
    );
    crate::lower_to_control_flow(&program, &semantics).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    assert_eq!(execute(tree.target(), "", PrintPolicy::default()), "3\n[]");
    optimize::optimize(
        &mut tree,
        analysis::Mode::Regions,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert_eq!(execute(tree.target(), "", PrintPolicy::default()), "3\n[]");
    // imul's exact low bits differ from binary64 multiplication followed by
    // source integer normalization. Its target call must retain that identity.
    compare_source_output("print(Math.imul(2147483647,2147483647));", "1\n");
}

fn compare_source_with_interpreter(source: &str) {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let expected = crate::interpret_program(&program, &semantics).unwrap();
    compare_source_output(source, &expected);
}

#[test]
fn callable_source_occurrences_do_not_alias_at_the_same_span() {
    use crate::ast::{ExprKind, Item, SourceNodes, Stmt};
    let arena = bumpalo::Bump::new();
    let parsed = crate::parse_source(
        &arena,
        "auto first=()=>1;auto second=()=>2;print(first());print(second());",
    )
    .unwrap();
    let nodes = SourceNodes::continuing(parsed.source_identity());
    let mut items = parsed.items.to_vec();
    for item in &mut items {
        if let Item::Stmt(Stmt::VarDecl(declaration)) = item {
            if let ExprKind::ArrowFunction { span, .. } =
                &mut declaration.initializer.as_mut().unwrap().kind
            {
                *span = crate::span::Span::empty(0);
            }
        }
    }
    let program = parsed.with_items(&nodes, arena.alloc_slice_fill_iter(items));
    let semantics = crate::analyze(&program).unwrap();
    let ir = crate::lower_to_control_flow(&program, &semantics).unwrap();
    let javascript = crate::codegen_ir_js::emit_optimized_ir_js(&ir).unwrap();
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &javascript])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "1\n2\n");
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    assert_eq!(
        execute(tree.target(), "", PrintPolicy::default()),
        "1\n2\n[]"
    );
}

#[test]
fn omitted_defaults_preserve_literals_global_bindings_and_fresh_callables() {
    compare_source_with_interpreter(
        r#"
        int withDefaults(int a,int b=10,int c=100){return a+b+c;}
        auto alias=withDefaults;
        print(withDefaults(3));print(alias(3,1));print(alias(3,1,2));
        int value=4;
        int readDefault(int first,int second=value){return first*10+second;}
        print(readDefault(value++));
        auto shadow=()=>{int value=70;return readDefault(2);};print(shadow());
        int increment(int[] values=[0]){values[0]+=1;return values[0];}
        print(increment());print(increment());
    "#,
    );
    compare_source_with_interpreter(
        r#"
        int offset=2;
        func(int)->int factory(func(int)->int value=(int n)=>{int saved=n+offset;return saved;}){return value;}
        auto first=factory();auto second=factory();
        print(first==second);print(first(2));print(second(4));
        T identity<T>(T value,func(T)->T transform=(T item)=>item){return transform(value);}
        print(identity(3));print(identity("three"));
    "#,
    );
}

#[test]
fn parameter_defaults_snapshot_arguments_inside_the_ordered_call() {
    compare_source_output(
        r#"
        int current=1;
        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++));
        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++,current++));
        print(((int first,int second=first,int third=second)=>first*100+second*10+third)(current++,current++,current++));
        print(current);
    "#,
        "111\n233\n456\n7\n",
    );
    compare_source_output(
        r#"
        int trace=0;
        int index(){trace=trace*10+1;return 0;}
        int first(){trace=trace*10+2;return 7;}
        int second(){trace=trace*10+3;return 8;}
        print((if(index()==0){
            (int a,int b,int c=a)=>{print(trace);return c;}
        }else{
            (int a,int b,int c=a)=>{print(trace);return c;}
        })(first(),second()));
    "#,
        "123\n7\n",
    );
}

#[test]
fn defaults_distinguish_omission_supplied_undefined_and_function_arity() {
    compare_source_output_with_setup(
        r#"
        extern int tag(JsValue value);
        extern int arity(func(JsValue,int)->int callable);
        int choose(JsValue first=7,int second=9){return tag(first)+second;}
        print(choose());print(choose(JS.undefined()));print(choose(JS.undefined(),1));
        print(arity(choose));
    "#,
        "globalThis.tag=value=>value===undefined?100:Number(value);globalThis.arity=callable=>callable.length;",
        "16\n109\n101\n2\n",
    );
}

fn compare_source_output(source: &str, expected: &str) {
    compare_source_output_with_setup(source, "", expected);
}

#[test]
fn source_classes_preserve_probe_inheritance_aliases_and_lexical_receivers() {
    let declarations = include_str!("../../finer/tools/fixtures/probe-classes.lil");
    let source = format!(
        "{declarations}\nprint(classes(-7));print(classes(-1));print(classes(0));\
         print(classes(3));print(classes(29));print(classes(1000000000));\
         print(classes(-2147483647-1));print(classes(2147483647));"
    );
    compare_source_output(&source, "-101\n3\n17\n59\n483\n-1179869167\n17\n3\n");
}

#[test]
fn class_calls_and_updates_capture_the_receiver_before_argument_effects() {
    compare_source_output(
        r#"
        class Cell {
            int value;
            init(int value) { this.value=value; }
            int add(int amount) { this.value+=amount;return this.value; }
            func()->int read() { return ()=>this.value; }
        }
        Cell left=new Cell(1);Cell right=new Cell(20);Cell chosen=left;
        Cell receiver(){print(10);return chosen;}
        int argument(){chosen=right;print(30);return 3;}
        print(receiver().add(argument()));print(left.value);print(right.value);
        func()->int read=left.read();Cell original=left;
        int replace(){left=right;original.value=40;return 5;}
        left.value+=replace();print(original.value);print(left.value);print(read());
        print(original.value++);print(++original.value);
        "#,
        "10\n30\n4\n4\n20\n9\n20\n9\n9\n11\n",
    );
}

#[test]
fn class_initializers_share_super_defaults_and_keep_fresh_fields_after_arguments() {
    compare_source_output_with_setup(
        r#"
        extern int argument();
        class Base {
            int value;
            int[] items;
            Map<string,int> table;
            init(int value=2) { this.value=value; }
            int add(int by=3) { return this.value+by; }
        }
        class Derived extends Base {
            init(int value=4) { super(value);if(value==4){return;}this.value+=1; }
        }
        Derived first=new Derived();Derived second=new Derived(argument());
        first.items.push(9);print(first.items.length);print(second.items.length);
        first.table.set("x",7);print(first.table.get("x"));print(second.table.get("x"));
        print(first.add());print(second.add());print(new Base().add());
        "#,
        "const NativeMap=Map;globalThis.Map=class extends NativeMap{constructor(){super();console.log('map')}};globalThis.argument=()=>{console.log('argument');return 8};",
        "map\nargument\nmap\n1\n0\n7\nnull\n7\n12\nmap\n5\n",
    );
}

#[test]
fn constructor_results_keep_allocation_identity_when_this_is_rebound() {
    compare_source_output(
        r#"
        class Cell {
            int value;
            init(int value) {
                this.value=value;
                if(value==1){this=new Cell(2);return;}
                if(value==4){try{return;}finally{this=new Cell(5);}}
                if(value==6){auto change=()=>{this=new Cell(7);};change();}
            }
            int replace(){this=new Cell(9);return this.value;}
        }
        Cell first=new Cell(1);
        print(first.value);print(new Cell(4).value);print(new Cell(6).value);
        print(first.replace());print(first.value);
        "#,
        "1\n4\n6\n9\n1\n",
    );
}

#[test]
fn class_callable_fields_remain_replaceable_and_generic_methods_erase_types() {
    compare_source_output(
        r#"
        class Holder<T> {
            T value;
            func()->int callback;
            init(T value,func()->int callback){this.value=value;this.callback=callback;}
            T get(){return this.value;}
            U identity<U>(U value){return value;}
        }
        Holder<int> first=new Holder<int>(7,()=>1);
        Holder<string> second=new Holder<string>("ok",()=>3);
        first.callback=()=>2;auto saved=first.callback;
        print(first.callback());print(saved());print(second.callback());
        print(first.get());print(second.get());print(first.identity("generic"));
        "#,
        "2\n2\n3\n7\nok\ngeneric\n",
    );
}

fn compare_source_output_with_setup(source: &str, setup: &str, expected: &str) {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        None,
        Some(analysis::Mode::Tree),
        Some(analysis::Mode::Indexed),
        Some(analysis::Mode::Memoized),
        Some(analysis::Mode::Regions),
        Some(analysis::Mode::Values),
    ] {
        let mut target = lower::lower_slice(&program, &semantics).unwrap();
        let mut unchanged_facts = None;
        if let Some(mode) = mode {
            for _ in 0..3 {
                let outcome = optimize::optimize(
                    &mut target,
                    mode,
                    crate::compilation_contract::JavaScriptWorld::ClosedApplication,
                )
                .unwrap();
                unchanged_facts = outcome.unchanged_facts;
                if !outcome.report.changed() {
                    break;
                }
            }
        }
        let view = match (mode, unchanged_facts) {
            (Some(_), Some(facts)) => {
                Some(extract::JavaScriptView::prepare_reusing(&target, facts).unwrap())
            }
            (Some(mode), None) => Some(extract::JavaScriptView::prepare(&target, mode)),
            (None, _) => None,
        };
        for mangle_bindings in [false, true] {
            let policy = PrintPolicy { mangle_bindings };
            let javascript = match &view {
                Some(view) => view.render(policy),
                None => target.render(policy),
            }
            .unwrap();
            let result = Command::new("node")
                .args([
                    "--input-type=module",
                    "-e",
                    &format!("{setup}\n{javascript}"),
                ])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{javascript}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                expected,
                "{javascript}"
            );
        }
    }
}

#[test]
fn escaping_callable_names_survive_storage_elimination_and_alias_renaming() {
    let source = r#"
        extern string nameOf(func()->int value);
        func()->int pass(func()->int veryLongCallbackParameter){
            auto anotherLongAlias=veryLongCallbackParameter;
            return anotherLongAlias;
        }
        int declared(){return 7;}
        auto original=()=>1;
        auto result=(original=()=>2);
        auto other=()=>3;
        auto third=(other=()=>4);
        print(nameOf(pass(result)));
        print(nameOf(pass(third)));
        print(nameOf(()=>5));
        print(nameOf(pass(declared)));
    "#;
    compare_source_output_with_setup(
        source,
        "globalThis.nameOf=value=>value.name;",
        "original\nother\n\ndeclared\n",
    );
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let names = Names::new(
        tree.target(),
        PrintPolicy {
            mangle_bindings: true,
        },
    )
    .unwrap();
    for (id, binding) in tree
        .target()
        .bindings
        .iter()
        .enumerate()
        .filter(|(_, binding)| {
            matches!(
                binding.spelling.as_str(),
                "veryLongCallbackParameter" | "anotherLongAlias"
            )
        })
    {
        assert_ne!(names.get(BindingId::new(id)), binding.spelling);
    }
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
        suspension: crate::structured_js::Suspension::None,
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
fn scoped_naming_reuses_siblings_and_preserves_nested_captures() {
    let source = r#"
        int a(){return 2;}
        int left(int parameter){
            auto closure=(int increment)=>parameter+increment;
            return closure(a());
        }
        int right(int parameter){return parameter+1;}
        print(left(5));print(right(7));
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let output = tree.output().unwrap();
    let plan = selection::Plan::new(selection::Style::Scoped);
    let names = output.basis.names(&plan).unwrap();
    let parameters: Vec<_> = tree
        .target()
        .bindings
        .iter()
        .enumerate()
        .filter(|(_, binding)| binding.spelling == "parameter")
        .map(|(id, _)| names.get(BindingId::new(id)))
        .collect();
    assert_eq!(parameters.len(), 2);
    assert_eq!(parameters[0], parameters[1]);
    assert_ne!(parameters[0], "a");
    let captured = named_binding(tree.target(), "increment");
    assert_ne!(names.get(captured), parameters[0]);
    let selection = output
        .select(
            selection::Budget {
                plans: 24,
                candidate_bytes: 1 << 20,
            },
            selection::Objectives::All,
            crate::compression::measure,
        )
        .unwrap();
    for artifact in &selection.candidates {
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &artifact.javascript])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "7\n8\n",
            "{}",
            artifact.javascript
        );
    }
}

#[test]
fn scoped_names_constrain_references_across_each_intervening_scope() {
    let sources = [
        (
            r#"
            int outer=7;
            int independent(int reusable){return reusable+1;}
            int enclosing(int captured){
                auto nested=(int argument)=>captured+outer+argument;
                return nested(3);
            }
            print(independent(2));print(enclosing(4));print(outer);
        "#,
            "3\n14\n7\n",
        ),
        (
            r#"
            int outer=7;
            int independent(int reusable){return reusable+1;}
            int enclosing(int captured){
                auto nested=(int argument)=>{outer=argument;return captured;};
                return nested(3);
            }
            print(independent(2));print(enclosing(4));print(outer);
        "#,
            "3\n4\n3\n",
        ),
    ];
    for (source, expected) in sources {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&program).unwrap();
        let tree = lower::lower_slice(&program, &semantics).unwrap();
        let output = tree.output().unwrap();
        let plan = selection::Plan::new(selection::Style::Scoped);
        let names = output.basis.names(&plan).unwrap();
        let name = |source| names.get(named_binding(tree.target(), source));
        assert_eq!(name("outer"), name("reusable"));
        assert_ne!(name("outer"), name("captured"));
        assert_ne!(name("outer"), name("argument"));
        assert_ne!(name("captured"), name("argument"));
        // Both the default and source-retention candidates obey the same
        // lexical constraints, including a write with no outer-value read.
        for source_names in [vec![], vec![named_binding(tree.target(), "captured")]] {
            let code = output
                .render(&selection::Plan {
                    style: selection::Style::Scoped,
                    source_names,
                    raw_spelling: false,
                })
                .unwrap();
            let result = Command::new("node")
                .args(["--input-type=module", "-e", &code])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{code}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(String::from_utf8(result.stdout).unwrap(), expected);
        }
    }
}

#[test]
fn output_search_keeps_budget_prefixes_and_scores_the_complete_union() {
    let source = r#"
        extern int read();
        bool sameParity(int previous,int next){return previous%2==next%2;}
        bool compareWith<T>(T value,func(T,T)->bool compare){return compare(value,value);}
        print(compareWith(read(),sameParity));
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    for _ in 0..3 {
        if !optimize::optimize(
            &mut tree,
            analysis::Mode::Regions,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
        )
        .unwrap()
        .report
        .changed()
        {
            break;
        }
    }
    let before = tree.target().clone();
    let view = extract::JavaScriptView::prepare(&tree, analysis::Mode::Regions);
    let output = view.output().unwrap();
    assert!(output.reused_structure);
    let mut previous: Option<selection::Selection> = None;
    for plans in [1, 3, 8, 24] {
        let mut measurements = 0;
        let result = output
            .select(
                selection::Budget {
                    plans,
                    candidate_bytes: 1 << 20,
                },
                selection::Objectives::All,
                |bytes, objective| {
                    measurements += 1;
                    crate::compression::measure(bytes, objective)
                },
            )
            .unwrap();
        assert_eq!(measurements, 2 * result.candidates.len());
        assert_eq!(measurements, result.measurement_calls);
        assert!(result.attempts.len() <= plans);
        assert!(result.proposal_steps <= plans);
        assert_eq!(
            result.retained_bytes,
            result
                .candidates
                .iter()
                .map(|artifact| artifact.javascript.len())
                .sum::<usize>()
        );
        let metrics =
            |sizes: selection::Sizes| [sizes.raw, sizes.gzip9.unwrap(), sizes.brotli11.unwrap()];
        for (metric, winner) in result.winners.iter().enumerate() {
            let size = metrics(result.candidates[winner.unwrap()].sizes)[metric];
            assert!(result
                .candidates
                .iter()
                .all(|artifact| metrics(artifact.sizes)[metric] >= size));
            if let Some(previous) = &previous {
                assert!(
                    size <= metrics(previous.candidates[previous.winners[metric].unwrap()].sizes)
                        [metric]
                );
            }
        }
        if let Some(previous) = &previous {
            for (earlier, current) in previous.attempts.iter().zip(&result.attempts) {
                assert_eq!(earlier.plan, current.plan);
                assert_eq!(earlier.artifact, current.artifact);
                assert_eq!(earlier.rejection, current.rejection);
            }
        }
        previous = Some(result);
    }
    let result = previous.unwrap();
    for artifact in &result.candidates {
        let code = format!("globalThis.read=()=>7;{}", artifact.javascript);
        let output = Command::new("node")
            .args(["--input-type=module", "-e", &code])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "true\n");
    }
    assert_eq!(
        tree.target(),
        &before,
        "search must not copy choices into semantic storage"
    );
}

#[test]
fn output_search_bounds_text_and_rejects_invalid_measurement_services() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "int value=7;print(value);").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let output = tree.output().unwrap();
    let basic = output
        .render(&selection::Plan::new(selection::Style::Global))
        .unwrap();
    assert!(output
        .select(
            selection::Budget {
                plans: 3,
                candidate_bytes: basic.len() - 1
            },
            selection::Objectives::All,
            crate::compression::measure
        )
        .is_err());
    let result = output
        .select(
            selection::Budget {
                plans: 12,
                candidate_bytes: basic.len(),
            },
            selection::Objectives::All,
            crate::compression::measure,
        )
        .unwrap();
    assert_eq!(result.retained_bytes, basic.len());
    assert_eq!(result.candidates.len(), 1);
    assert!(result
        .attempts
        .iter()
        .any(|attempt| attempt.rejection.is_some()));
    assert_eq!(
        output
            .select(
                selection::Budget {
                    plans: 1,
                    candidate_bytes: 1024
                },
                selection::Objectives::All,
                |_, _| Err("encoder unavailable".into())
            )
            .unwrap_err(),
        "encoder unavailable"
    );
    assert!(output
        .select(
            selection::Budget {
                plans: 1,
                candidate_bytes: 1024
            },
            selection::Objectives::All,
            |_, _| Ok(0)
        )
        .is_err());
}

#[test]
fn invocation_observations_release_private_names_only_in_the_matching_world() {
    use crate::compilation_contract::JavaScriptWorld;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena,
        "int privateArithmeticFunction(int value){return value+2;}print(privateArithmeticFunction(7));").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let original = tree.target().clone();
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let analysis = analysis::Analysis::new_in_execution(
            &tree,
            mode,
            JavaScriptWorld::ClosedApplication,
            crate::compilation_contract::JavaScriptExecution::Module,
        );
        let uses = analysis.binding_uses(named_binding(tree.target(), "privateArithmeticFunction"));
        assert_eq!(uses.reads.len(), 1);
        let analysis::Observation::Call(call) = uses.reads[0].observation else {
            panic!("invocation role lost")
        };
        assert!(
            matches!(tree.target().expressions[call.index()], Expr::Call { callee, .. } if callee == uses.reads[0].expression)
        );
        let closed = extract::JavaScriptView::prepare_reusing_in_execution(
            &tree,
            analysis.detach(),
            crate::compilation_contract::JavaScriptExecution::Module,
        )
        .unwrap();
        assert_eq!(closed.omitted_function_names, 1);
        let code = closed
            .render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap();
        assert!(!code.contains("privateArithmeticFunction"));
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &code])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8(result.stdout).unwrap(), "9\n");
        let library = extract::JavaScriptView::prepare_in_world(
            &tree,
            mode,
            JavaScriptWorld::ReusableLibrary,
        );
        assert_eq!(library.omitted_function_names, 0);
        assert!(library
            .render(PrintPolicy {
                mangle_bindings: true
            })
            .unwrap()
            .contains("privateArithmeticFunction"));
    }
    assert_eq!(
        tree.target(),
        &original,
        "target name freedom must not erase source metadata"
    );
}

#[test]
fn escaped_names_and_observing_calls_keep_function_name_requirements() {
    use crate::compilation_contract::JavaScriptWorld;
    let cases = [
        (
            "extern string nameOf(func(int)->int value);int exposedFunction(int value){return value;}print(nameOf(exposedFunction));",
            "globalThis.nameOf=value=>value.name;",
            "exposedFunction\n",
        ),
        (
            "extern bool observeStack();bool functionWithForeignObservation(){return observeStack();}print(functionWithForeignObservation());",
            "globalThis.observeStack=()=>new Error().stack.includes('functionWithForeignObservation');",
            "true\n",
        ),
        (
            "extern void install();extern bool observation();int functionUsingPrimitive(string value){return value.charCodeAt(0);}install();print(functionUsingPrimitive(\"Q\"));print(observation());",
            "let observed=false;const original=String.prototype.charCodeAt;globalThis.install=()=>{String.prototype.charCodeAt=function(...args){if(String(this)==='Q')observed=new Error().stack.includes('functionUsingPrimitive');return Reflect.apply(original,this,args)}};globalThis.observation=()=>observed;",
            "81\ntrue\n",
        ),
    ];
    for (source, setup, expected) in cases {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&program).unwrap();
        let tree = lower::lower_slice(&program, &semantics).unwrap();
        for mode in [
            analysis::Mode::Indexed,
            analysis::Mode::Regions,
            analysis::Mode::Values,
        ] {
            let view = extract::JavaScriptView::prepare_in_world(
                &tree,
                mode,
                JavaScriptWorld::ClosedApplication,
            );
            assert_eq!(view.omitted_function_names, 0, "{source}");
            let code = format!(
                "{setup}\n{}",
                view.render(PrintPolicy {
                    mangle_bindings: true
                })
                .unwrap()
            );
            let result = Command::new("node")
                .args(["--input-type=module", "-e", &code])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{code}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(String::from_utf8(result.stdout).unwrap(), expected);
        }
    }
}

#[test]
fn construction_is_a_value_observation_even_when_the_body_is_pure() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "int constructibleFunction(int value){return value;}print(constructibleFunction(1));",
    )
    .unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let callable = named_binding(tree.target(), "constructibleFunction");
    let module = tree.target_mut();
    module.regions[0].statements.pop();
    let callee = expr(module, Expr::Binding(callable));
    let argument = number(module, 7.0);
    let instance = expr(
        module,
        Expr::Construct {
            callee,
            arguments: vec![argument],
        },
    );
    let constructor = expr(
        module,
        Expr::Member {
            object: instance,
            property: Property::Named("constructor".into()),
        },
    );
    let name = expr(
        module,
        Expr::Member {
            object: constructor,
            property: Property::Named("name".into()),
        },
    );
    capture(module, name);
    module.verify().unwrap();
    let view = extract::JavaScriptView::prepare_in_world(
        &tree,
        analysis::Mode::Regions,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    );
    assert_eq!(view.omitted_function_names, 0);
    let code = format!(
        "const capture=value=>console.log(value);{}",
        view.render(PrintPolicy {
            mangle_bindings: true
        })
        .unwrap()
    );
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &code])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        String::from_utf8(result.stdout).unwrap(),
        "constructibleFunction\n"
    );
}

#[test]
fn requested_objectives_own_codec_work_and_unmeasured_scores_stay_absent() {
    use selection::{Budget, Objective, Objectives};
    let arena = bumpalo::Bump::new();
    let source = "extern int read();int total(int first,int second){return first+second;}print(total(read(),3));";
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let output = tree.output().unwrap();
    let mut tested = BTreeSet::new();
    for objectives in [
        Objectives::One(Objective::Raw),
        Objectives::One(Objective::Gzip),
        Objectives::One(Objective::Brotli),
        Objectives::All,
    ] {
        let mut previous: Option<selection::Selection> = None;
        for plans in [1, 3, 8, 16] {
            let mut calls: Vec<(Vec<u8>, Objective)> = Vec::new();
            let result = output
                .select(
                    Budget {
                        plans,
                        candidate_bytes: 1 << 20,
                    },
                    objectives,
                    |bytes, objective| {
                        assert_ne!(
                            objective,
                            Objective::Raw,
                            "raw search must never call a codec"
                        );
                        assert!(objectives.iter().any(|requested| requested == objective));
                        assert!(!calls.iter().any(
                            |(artifact, measured)| artifact == bytes && *measured == objective
                        ));
                        calls.push((bytes.to_vec(), objective));
                        crate::compression::measure(bytes, objective)
                    },
                )
                .unwrap();
            let codecs = objectives
                .iter()
                .filter(|objective| *objective != Objective::Raw)
                .count();
            assert_eq!(result.measurement_calls, codecs * result.candidates.len());
            assert_eq!(calls.len(), result.measurement_calls);
            for objective in Objectives::All.iter() {
                let requested = objectives.iter().any(|item| item == objective);
                assert_eq!(result.winner(objective).is_some(), requested);
                for artifact in &result.candidates {
                    assert_eq!(
                        artifact.sizes.get(objective).is_some(),
                        requested || objective == Objective::Raw
                    );
                    if let Some(size) = artifact.sizes.get(objective) {
                        let reference = crate::measure_javascript_transfer_sizes(
                            artifact.javascript.as_bytes(),
                        )
                        .unwrap();
                        let expected = match objective {
                            Objective::Raw => reference.raw,
                            Objective::Gzip => reference.gzip9,
                            Objective::Brotli => reference.brotli11,
                        };
                        assert_eq!(size, expected);
                    }
                }
                if let Some(winner) = result.winner(objective) {
                    let size = winner.sizes.get(objective).unwrap();
                    assert!(result.candidates.iter().all(|artifact| artifact
                        .sizes
                        .get(objective)
                        .unwrap()
                        >= size));
                    if let Some(earlier) = &previous {
                        assert!(
                            size <= earlier
                                .winner(objective)
                                .unwrap()
                                .sizes
                                .get(objective)
                                .unwrap()
                        );
                    }
                }
            }
            if let Some(earlier) = &previous {
                for (old, new) in earlier.attempts.iter().zip(&result.attempts) {
                    assert_eq!(
                        (&old.plan, old.artifact, &old.rejection),
                        (&new.plan, new.artifact, &new.rejection)
                    );
                }
            }
            for artifact in &result.candidates {
                if tested.insert(artifact.javascript.clone()) {
                    let code = format!("globalThis.read=()=>7;{}", artifact.javascript);
                    let result = Command::new("node")
                        .args(["--input-type=module", "-e", &code])
                        .output()
                        .unwrap();
                    assert!(result.status.success());
                    assert_eq!(String::from_utf8(result.stdout).unwrap(), "10\n");
                }
            }
            previous = Some(result);
        }
    }
}

#[test]
fn shared_value_dependencies_preserve_initialization_effects_and_cells() {
    compare_source_with_interpreter(
        r#"
        int effects=0;
        int next(){effects=effects+1;return effects;}
        int f(int x) {
            int a=x+1; int b=a; int unused=next();
            42; if(false){print(999);}
            return b;
        }
        int first=next(); first; print(f(first)); print(effects);
        int cell=1; auto set=()=>{cell=3;};
        int previous=cell; set(); print(previous); print(cell);
    "#,
    );
}

#[test]
fn both_analysis_modes_apply_the_same_legal_transformations() {
    let source =
        "int f(int x){int a=x;int b=a;int unused=7;42;if(false){print(99);}return b;}print(f(3));";
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut outputs = vec![];
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let report = optimize::optimize_in_execution(
            &mut tree,
            mode,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
            crate::compilation_contract::JavaScriptExecution::Module,
        )
        .unwrap()
        .report;
        assert_eq!(report.inlined_bindings, 2);
        assert_eq!(report.removed_bindings, 1);
        assert_eq!(report.discarded_expressions, 1);
        assert_eq!(report.simplified_control, 1);
        outputs.push(
            tree.render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap(),
        );
    }
    assert_eq!(outputs[0], outputs[1]);
    assert_eq!(outputs[0], outputs[2]);
    let run = Command::new("node")
        .args(["--input-type=module", "-e", &outputs[0]])
        .output()
        .unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "3\n");
}

#[test]
fn structural_proofs_belong_to_their_immutable_program_state() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "print(1+2);").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let (proof, reused) = tree.structure().unwrap();
    assert!(!reused);
    assert!(tree.structure().unwrap().1);
    let mut branch = tree.clone();
    assert!(std::ptr::eq(proof, branch.structure().unwrap().0));

    // A stale success must not conceal an invalid edit in a cloned branch.
    branch.target_mut().origins.pop();
    assert_eq!(
        branch.structure().unwrap_err(),
        "missing expression provenance slots"
    );
    assert!(tree.structure().is_ok());
    // A cached failure must also be invalidated when that state is repaired.
    branch.target_mut().origins.push(None);
    assert!(!branch.structure().unwrap().1);
    assert!(branch.structure().unwrap().1);

    // Owned edits cannot compact handles while leaving old analysis valid.
    let facts = analysis::Analysis::new(&branch, analysis::Mode::Indexed).detach();
    branch
        .edit(|module| {
            module.expression(Expr::Literal(Literal::Number(99.0)), None);
        })
        .unwrap();
    assert!(analysis::Analysis::resume(&branch, facts).is_err());
    assert!(branch.structure().unwrap().1);
    assert_eq!(
        branch.structure().unwrap().0,
        &verify::verify(branch.target()).unwrap()
    );
}

#[test]
fn unchanged_optimization_reuses_structure_and_compaction_preserves_it() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "int f(int x){int a=x+1;int b=a+2;if(false){print(99);}return b;}print(f(3));",
    )
    .unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let mut ended = false;
    for round in 0..8 {
        let report = optimize::optimize(
            &mut tree,
            analysis::Mode::Regions,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
        )
        .unwrap()
        .report;
        assert_eq!(report.reused_structure, round > 0);
        assert_eq!(
            report.verified_states,
            usize::from(round == 0) + usize::from(report.changed())
        );
        assert_eq!(
            tree.structure().unwrap().0,
            &verify::verify(tree.target()).unwrap()
        );
        if !report.changed() {
            assert_eq!(report.verified_states, 0);
            ended = true;
            break;
        }
    }
    assert!(ended);
    let javascript = tree
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &javascript])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{javascript}\n{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "6\n");
}

#[test]
fn detached_facts_reject_edits_and_independently_edited_branches() {
    use analysis::{Analysis, Mode};
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "print(1+2);").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let one = ExprId::new(
        tree.target()
            .expressions
            .iter()
            .position(|node| matches!(node, Expr::Literal(Literal::Number(1.0))))
            .unwrap(),
    );

    let facts = Analysis::new(&tree, Mode::Indexed).detach();
    let mut branch = tree.clone();
    let facts = Analysis::resume(&branch, facts).unwrap().detach();
    branch.target_mut().expressions[one.index()] = Expr::Literal(Literal::Number(10.0));
    assert!(
        Analysis::resume(&branch, facts).is_err(),
        "a changed program cannot consume its old proofs"
    );

    let mut other_branch = tree.clone();
    other_branch.target_mut().expressions[one.index()] = Expr::Literal(Literal::Number(30.0));
    let branch_facts = Analysis::new(&branch, Mode::Indexed).detach();
    assert!(
        Analysis::resume(&other_branch, branch_facts).is_err(),
        "independent branches with the same edit count need distinct identities"
    );

    let unrelated = lower::lower_slice(&program, &semantics).unwrap();
    let facts = Analysis::new(&tree, Mode::Indexed).detach();
    assert!(
        Analysis::resume(&unrelated, facts).is_err(),
        "equal slot counts are not proof identity"
    );
}

#[test]
fn unchanged_pass_hands_current_facts_to_target_selection() {
    let arena = bumpalo::Bump::new();
    let program =
        crate::parse_source(&arena, "int f(int x){return x>>>1;}print(f(2147483647));").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let result = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert!(!result.report.changed());
    assert!(result.report.work.expression_visits > 0);
    let view =
        extract::JavaScriptView::prepare_reusing(&tree, result.unchanged_facts.unwrap()).unwrap();
    assert!(view.reused_facts);
    assert!(view.work.queries > 0);
    assert_eq!(view.work.expression_visits, 0);
    assert_eq!(view.work.stored_summaries, 0);
    let fresh = extract::JavaScriptView::prepare_in_world(
        &tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    );
    for mangle_bindings in [false, true] {
        let policy = PrintPolicy { mangle_bindings };
        assert_eq!(view.render(policy).unwrap(), fresh.render(policy).unwrap());
    }
}

#[test]
fn materialization_keeps_long_statement_chains_within_the_target_depth_budget() {
    for nesting in [0, 12] {
        let mut source = String::from("int f(int input){");
        for _ in 0..nesting {
            source.push_str("if(input>0){");
        }
        source.push_str("int v0=input;");
        for index in 1..=1200 {
            source.push_str(&format!("int v{index}=v{}+1;", index - 1));
        }
        source.push_str("return v1200;");
        for _ in 0..nesting {
            source.push('}');
        }
        if nesting > 0 {
            source.push_str("return 0;");
        }
        source.push_str("}print(f(7));print(f(2147483647));print(f(-2147483647-1));");
        compare_source_output(
            &source,
            if nesting == 0 {
                "1207\n-2147482449\n-2147482448\n"
            } else {
                "1207\n-2147482449\n0\n"
            },
        );
    }
}

#[test]
fn each_query_visits_shared_value_dependencies_at_most_once() {
    use analysis::{Analysis, Mode};
    let mut source = String::from("int f(int input){int v0=input;");
    for index in 1..=72 {
        source.push_str(&format!("int v{index}=v{0}+v{0};", index - 1));
    }
    source.push_str("return v72;}print(f(7));");
    compare_source_output(&source, "0\n");
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, &source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let body = &tree.target().regions[tree.target().functions[0].body.index()];
    let Statement::Return(Some(root)) = body.statements.last().unwrap() else {
        panic!("missing return")
    };
    let mut results = vec![];
    for mode in [Mode::Tree, Mode::Indexed, Mode::Memoized] {
        let mut analysis = Analysis::new(&tree, mode);
        let before = analysis.work();
        results.push(analysis.facts(*root));
        let first = analysis.work();
        assert!(
            first.expression_visits - before.expression_visits <= tree.target().expressions.len(),
            "one query must not expand a shared dependency exponentially"
        );
        assert_eq!(analysis.facts(*root), *results.last().unwrap());
        if mode == Mode::Tree {
            assert!(analysis.work().temporary_summaries > first.temporary_summaries);
            assert_eq!(analysis.work().stored_summaries, 0);
        } else {
            assert_eq!(analysis.work().expression_visits, first.expression_visits);
        }
    }
    assert_eq!(results[0], results[1]);
    assert_eq!(results[0], results[2]);
}

#[test]
fn range_proofs_preserve_overflow_negative_zero_and_explicit_bitwise_operations() {
    compare_source_with_interpreter(
        r#"
        int unsigned(int x,int shift){int y=x>>>shift;return y;}
        int positive(int x){int y=x>>>1;return y;}
        print(unsigned(-1,0)); print(unsigned(-1,32)); print(positive(-1));
        int zero=-0;print(zero); print(2147483647+1);
        int explicit(int x){return x|0;}print(explicit(7));
    "#,
    );
    let arena = bumpalo::Bump::new();
    let program =
        crate::parse_source(&arena, "int f(int x){return (x>>>1)|0;}print(f(-1));").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    let operations = tree
        .target()
        .expressions
        .iter()
        .filter(|node| matches!(node, Expr::IntBinary { .. } | Expr::IntNegate(_)))
        .count();
    let view = extract::JavaScriptView::prepare(&tree, analysis::Mode::Indexed);
    assert!(view.omitted_normalizations > 0);
    assert!(
        view.render(PrintPolicy::default()).unwrap().contains("|0"),
        "source-written normalization stays explicit"
    );
    assert!(operations > 0);
    assert_eq!(
        operations,
        tree.target()
            .expressions
            .iter()
            .filter(|node| matches!(node, Expr::IntBinary { .. } | Expr::IntNegate(_)))
            .count(),
        "target spelling cannot erase semantic operators"
    );
}

#[test]
fn binding_updates_preserve_signed_arithmetic_and_rhs_observation_order() {
    compare_source_with_interpreter(
        r#"
        int value=2147483647;print(value++);print(value);print(++value);
        int minimum=-2147483647-1;print(minimum--);print(minimum);
        int cell=3;print(cell+=(cell=9));print(cell);
        cell-=2;cell*=3;cell/=4;cell%=5;
        cell|=8;cell^=3;cell&=15;cell<<=2;cell>>=1;cell>>>=0;
        print(cell);cell/=0;print(cell);
        int negative=-1;negative>>>=32;print(negative);
        float fraction=1.5;print(fraction++);fraction*=2.0;print(--fraction);
        string text="first";text+="-last";print(text);
    "#,
    );
}

#[test]
fn postfix_snapshots_survive_captured_mutation_and_short_circuiting() {
    compare_source_with_interpreter(
        r#"
        int cell=4;int temporary=99;int a=42;
        auto advance=()=>{cell+=2;return cell++;};
        print(advance());print(cell);
        print(cell++ + cell++);print(cell);
        bool skipped=false&&(cell++>0);print(cell);
        bool taken=true&&(cell++>0);print(cell);
        print(temporary);print(a);
        int? missing=null;int? present=0;
        print(missing??=(cell++));print(present??=(cell++));print(cell);
    "#,
    );
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "int value=1;value++;--value;print(value);").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    assert!(
        tree.target()
            .bindings
            .iter()
            .all(|binding| binding.source_symbol.is_some()),
        "discarded update results must not create snapshot cells"
    );
}

#[test]
fn nullish_binding_assignment_preserves_lazy_callable_named_evaluation() {
    compare_source_output_with_setup(
        r#"
        extern string nameOf(func()->int value);
        (func()->int)? callback=null;
        func()->int first=callback??=()=>7;
        func()->int second=callback??=()=>8;
        print(nameOf(first));print(nameOf(second));print(first());print(second());
    "#,
        "globalThis.nameOf=value=>value.name;",
        "callback\ncallback\n7\n7\n",
    );
}

#[test]
fn source_records_and_arrays_preserve_aliases_keys_and_absent_values() {
    compare_source_with_interpreter(
        r#"
        Record<int> values=record{x:1,y:2,__proto__:4,"quoted-key":6};
        Record<int> alias=values;
        alias["x"]=7;
        print(values.x==null);print(values.x??0);
        print(values["__proto__"]??0);print(values.toString==null);
        print(values["quoted-key"]??0);
        values.missing=9;print(alias["missing"]??0);
        int[] array=[3,5,7];int[] shared=array;
        shared[1]=11;print(array[1]);print(array[0]+array[2]);
    "#,
    );
    // The reference interpreter rejects these out-of-range element reads.
    // The JavaScript contract uses signed normalization and the empty string.
    compare_source_output(
        r#"
        int[] numbers=[7];string[] strings=["ok"];
        print(numbers[4]);print(strings[4]);print("abc"[8]);
        print("\ud800X"[0].charCodeAt(0));
    "#,
        "0\n\n\n55296\n",
    );
}

#[test]
fn reference_construction_preserves_order_without_loading_a_write_target() {
    compare_source_output_with_setup(
        r#"
        extern Record<int> object();extern string key();extern int replacement();
        extern string trace();
        print(object()[key()]??41);
        print(object()[key()]=replacement());
        print(trace());
    "#,
        r#"
        const events=[];
        const value=new Proxy(Object.create(null),{
            get(target,key){events.push('get');return undefined},
            set(target,key,value){events.push('set:'+value);return true}
        });
        globalThis.object=()=>{events.push('object');return value};
        globalThis.key=()=>{events.push('key');return 'slot'};
        globalThis.replacement=()=>{events.push('value');return 7};
        globalThis.trace=()=>events.join(',');
    "#,
        "41\n7\nobject,key,get,object,key,value,set:7\n",
    );
}

#[test]
fn nullish_evaluation_preserves_falsy_values_and_conditional_writes() {
    compare_source_with_interpreter(
        r#"
        int effects=0;int once(){effects=effects+1;return 9;}
        int? missing=null;int? present=0;
        print(missing??once());print(present??once());print(effects);
        bool? absent=null;bool? no=false;
        print((absent??false)||true);print((no??true)&&false);
        print(absent??(false||true));print(no??(true&&false));
        int cell=3;present??(cell=7);print(cell);
        missing??(cell=11);print(cell);
    "#,
    );
}

#[test]
fn record_function_values_keep_named_creation_separate_from_assignment() {
    compare_source_output_with_setup(
        r#"
        extern string nameOf(func()->int value);
        func()->int fallback=()=>0;
        Record<func()->int> callbacks=record{handler:()=>7};
        print(nameOf(callbacks.handler??fallback));
        callbacks.handler=()=>8;
        print(nameOf(callbacks.handler??fallback));
    "#,
        "globalThis.nameOf=value=>value.name;",
        "handler\n\n",
    );
}

#[test]
fn source_functions_branches_and_loops_match_the_independent_interpreter() {
    compare_source_with_interpreter(
        r#"
        float sum(float limit) {
            float i=0.0; float total=0.0;
            while(i<limit) {
                i=i+1.0;
                if(i==2.0) { continue; }
                if(i>5.0) { break; }
                total=total+i;
            }
            return total;
        }
        print(sum(7.0));
    "#,
    );
}

#[test]
fn source_short_circuit_and_mutable_closures_match_the_interpreter() {
    compare_source_with_interpreter(
        r#"
        float count=0.0;
        bool change(){count=count+1.0;return true;}
        print(false && change()); print(true || change()); print(count);
        auto add=(float amount)=>{count=count+amount;return count;};
        print(add(3.0)); print(add(4.0));
    "#,
    );
}

#[test]
fn erased_type_parameters_preserve_callback_bindings_and_invocations() {
    compare_source_with_interpreter(
        r#"
        bool sameParity(int previous,int next){return previous%2==next%2;}
        bool compareWith(int value,func(int,int)->bool compare){return compare(value,value);}
        print(compareWith(7,sameParity));
    "#,
    );
    // The reference interpreter currently rejects generic function signatures
    // before evaluating their body. These results are independently declared;
    // non-generic callback execution remains covered by interpreter cases.
    compare_source_output(
        r#"
        bool sameParity(int previous,int next){return previous%2==next%2;}
        bool compareWith<T>(T value,func(T,T)->bool compare){return compare(value,value);}
        T identity<T>(T value){return value;}
        print(compareWith(7,sameParity));
        print(identity(42));print(identity("typed"));print(identity(false));
    "#,
        "true\n42\ntyped\nfalse\n",
    );
}

#[test]
fn source_integer_operations_follow_the_language_contract() {
    compare_source_with_interpreter(
        r#"
        int add(int x){return x+1;}
        int minimum=-2147483647-1;
        print(add(2147483647)); print(minimum-1);
        print(-1 >>> 0); print(-1 >>> 32); print(-1 >>> 1);
        print(minimum); print(-minimum);
        print(1073741825*1073741825); print(9/0); print(9%0);
        print(-9/2); print(-9%2); print(minimum/-1);
        print(1<<33); print(-8>>34); print(3&1); print(1^3); print(1|2);
        print(-0);
    "#,
    );
}

#[test]
fn constant_language_operations_match_independent_evaluation_at_i32_boundaries() {
    let values = [
        i32::MIN,
        -1073741825,
        -9,
        -1,
        0,
        1,
        2,
        31,
        32,
        1073741825,
        i32::MAX,
    ];
    let literal = |value: i32| {
        if value == i32::MIN {
            "(-2147483647-1)".to_string()
        } else {
            format!("({value})")
        }
    };
    let mut source = String::new();
    for left in values {
        for right in values {
            for operator in ["+", "-", "*", "/", "%", ">>>"] {
                source.push_str(&format!(
                    "print({}{operator}{});",
                    literal(left),
                    literal(right)
                ));
            }
        }
    }
    // The interpreter and unoptimized JavaScript do not call the compiler's
    // constant evaluator. Include binary64 multiplication, signed overflow,
    // zero division, negative remainders and masked shift counts in one matrix.
    compare_source_with_interpreter(&source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, &source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let result = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert!(result.report.folded_constants >= values.len() * values.len() * 6);
    let live = verify::verify(tree.target()).unwrap().live_expressions;
    assert!(!tree
        .target()
        .expressions
        .iter()
        .zip(live)
        .any(|(node, live)| live && matches!(node, Expr::IntBinary { .. } | Expr::IntNegate(_))));
}

#[test]
fn a_constant_result_does_not_erase_effects_or_prove_raw_normalization_redundant() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let module = tree.target_mut();
        let observed = host(module, "observe");
        let observed = call(module, observed, vec![], Invocation::Value);
        let zero = number(module, 0.0);
        let sequence = expr(module, Expr::Sequence(vec![observed, zero]));
        let negated = expr(module, Expr::IntNegate(sequence));
        capture(module, negated);
        let mut analysis = analysis::Analysis::new(&tree, mode);
        let facts = analysis.facts(negated);
        assert_eq!(
            facts
                .integer
                .and_then(analysis::IntegerRange::singleton_i32),
            Some(0)
        );
        assert!(
            !facts.normalization_redundant,
            "raw negation would produce negative zero"
        );
        assert!(!facts.effects.discardable());
        drop(analysis);
        let result = optimize::optimize(
            &mut tree,
            mode,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
        )
        .unwrap();
        assert!(!result.report.changed());
        assert_eq!(
            execute(
                tree.target(),
                "function observe(){capture('effect')}",
                PrintPolicy::default()
            ),
            "[\"effect\",0]"
        );
    }
}

#[test]
fn owned_compaction_removes_dead_closures_and_names_without_losing_source_identity() {
    let source = r#"
        int outer(int x) {
            if(false){int abandoned=99;auto gone=()=>abandoned;print(gone());}
            int removed=123;
            auto reader=()=>x+1;
            return reader();
        }
        print(outer(7));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let original: BTreeMap<_, _> = tree
        .target()
        .origins
        .iter()
        .enumerate()
        .filter_map(|(index, origin)| {
            Some((
                origin.as_ref()?.index(),
                tree.source_facts(ExprId::new(index))?.span,
            ))
        })
        .collect();
    let outcome = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert!(outcome.report.compacted.expressions > 0);
    assert!(outcome.report.compacted.regions > 0);
    assert!(outcome.report.compacted.functions > 0);
    assert!(outcome.report.compacted.scopes > 0);
    assert!(outcome.report.compacted.bindings > 0);
    let live = verify::verify(tree.target()).unwrap();
    assert!(live.live_expressions.iter().all(|live| *live));
    assert!(live.region_depths.iter().all(Option::is_some));
    assert!(live.live_functions.iter().all(|live| *live));
    assert_eq!(live.live_bindings.len(), tree.target().bindings.len());
    assert!(live.live_bindings.iter().all(|live| *live));
    assert!(tree
        .target()
        .bindings
        .iter()
        .all(|binding| binding.spelling != "abandoned" && binding.spelling != "gone"));
    for (index, origin) in tree.target().origins.iter().enumerate() {
        if let Some(origin) = origin {
            assert_eq!(
                tree.source_facts(ExprId::new(index)).unwrap().span,
                original[&origin.index()]
            );
        }
    }
    tree.render(PrintPolicy {
        mangle_bindings: true,
    })
    .unwrap();
}

#[test]
fn signed_offset_reassociation_preserves_rounding_effects_and_materialization() {
    let values = [i32::MIN, -9, -1, 0, 1, 31, 32, 1073741825, i32::MAX];
    let literal = |value: i32| {
        if value == i32::MIN {
            "(-2147483647-1)".to_string()
        } else {
            format!("({value})")
        }
    };
    let mut source = String::new();
    for (index, (a, b)) in values.into_iter().zip(values.into_iter().rev()).enumerate() {
        source.push_str(&format!(
            "int f{index}(int x){{int a=x+{};int b=a-{};return 31+b;}}",
            literal(a),
            literal(b)
        ));
        for input in values {
            source.push_str(&format!("print(f{index}({}));", literal(input)));
        }
    }
    source.push_str(
        r#"
        int calls=0;
        int next(){calls=calls+1;return 2147483647;}
        print((next()+2147483647)+2147483647);print(calls);
        int cell=7; int old=cell+1; cell=99; print(old+2);print(old);
        int multiply(int x){return (x*1073741825)*1073741825;}
        print(multiply(1073741825));
        float precision(float x){return (x+10000000000000000.0)-10000000000000000.0;}
        print(precision(1.0));
    "#,
    );
    compare_source_with_interpreter(&source);

    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(
        &arena,
        "int f(int x){int a=x+1;int b=a+2;return b+3;}print(f(7));",
    )
    .unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let outcome = optimize::optimize_in_execution(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
        crate::compilation_contract::JavaScriptExecution::Module,
    )
    .unwrap();
    assert!(outcome.report.simplified_integer_arithmetic >= 2);
    assert!(outcome.report.inserted_expressions >= 2);
    assert_eq!(
        tree.target()
            .expressions
            .iter()
            .filter(|node| matches!(node, Expr::IntBinary { .. }))
            .count(),
        1,
        "a single owned transaction combines arithmetic across its proved substitutions"
    );
    let javascript = tree.render(PrintPolicy::default()).unwrap();
    let run = Command::new("node")
        .args(["--input-type=module", "-e", &javascript])
        .output()
        .unwrap();
    assert!(run.status.success());
    assert_eq!(String::from_utf8(run.stdout).unwrap(), "13\n");
}

#[test]
fn private_array_observations_keep_initialization_effects_at_their_original_site() {
    let source = r#"
        int calls=0;
        int next(){calls=calls+1;print(calls);return calls;}
        int[] values=[next(),next(),7];
        print(100);print(values.length);print(values.length);print(calls);
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let expected = crate::interpret_program(&program, &semantics).unwrap();
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
    ] {
        for data in [optimize::OwnedData::Arrays, optimize::OwnedData::Scalars] {
            let mut tree = lower::lower_slice(&program, &semantics).unwrap();
            let mut removed = 0;
            for _ in 0..3 {
                let outcome = optimize::optimize_with(
                    &mut tree,
                    mode,
                    crate::compilation_contract::JavaScriptWorld::ClosedApplication,
                    data,
                )
                .unwrap();
                removed += outcome.report.removed_allocations;
                if !outcome.report.changed() {
                    break;
                }
            }
            assert_eq!(removed, usize::from(data == optimize::OwnedData::Scalars));
            assert_eq!(
                tree.target()
                    .expressions
                    .iter()
                    .filter(|node| matches!(node, Expr::Array(_)))
                    .count(),
                usize::from(data == optimize::OwnedData::Arrays)
            );
            let javascript = extract::JavaScriptView::prepare(&tree, mode)
                .render(PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap();
            let result = Command::new("node")
                .args(["--input-type=module", "-e", &javascript])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{}\n{javascript}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                expected,
                "{javascript}"
            );
        }
    }
    compare_source_with_interpreter(
        r#"
        int[] original=[1,2];int[] alias=original;original=[3];
        print(alias.length);print(original.length);
        int[] captured=[1,2];auto length=()=>captured.length;print(length());
        int count(int[] values){return values.length;}
        int[] passed=[3,4,5];print(count(passed));
    "#,
    );
}

#[test]
fn array_length_places_and_receiver_calls_cannot_be_scalar_observations() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for receiver_call in [false, true] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let module = tree.target_mut();
        let symbol = binding(module, RegionId::new(0), 900, "values");
        let one = number(module, 1.0);
        let array = expr(module, Expr::Array(vec![one]));
        module.regions[0].statements.push(Statement::Let {
            binding: symbol,
            value: Some(array),
        });
        let object = expr(module, Expr::Binding(symbol));
        let length = expr(
            module,
            Expr::Member {
                object,
                property: Property::Named("length".into()),
            },
        );
        let operation = if receiver_call {
            call(module, length, vec![], Invocation::Reference)
        } else {
            let zero = number(module, 0.0);
            expr(
                module,
                Expr::Assign {
                    target: length,
                    value: zero,
                },
            )
        };
        module.regions[0]
            .statements
            .push(Statement::Evaluate(operation));
        let object = expr(module, Expr::Binding(symbol));
        let length = expr(
            module,
            Expr::Member {
                object,
                property: Property::Named("length".into()),
            },
        );
        capture(module, length);
        let outcome = optimize::optimize(
            &mut tree,
            analysis::Mode::Indexed,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
        )
        .unwrap();
        assert_eq!(outcome.report.removed_allocations, 0);
        let javascript = tree
            .render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap();
        let script = format!(
            "const outputs=[];function capture(v){{outputs.push(v)}}try{{{javascript}}}catch(e){{outputs.push(e instanceof TypeError)}}console.log(JSON.stringify(outputs))"
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            if receiver_call { "[true]\n" } else { "[0]\n" }
        );
    }
}

#[test]
fn lexical_ownership_distinguishes_nested_blocks_from_captured_writes() {
    let source = r#"
        int f(int input){
            int local=input;{local=local+1;}
            int shared=1;auto write=()=>{shared=9;};write();
            return local+shared;
        }
        print(f(7));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let analysis = analysis::Analysis::new(&tree, analysis::Mode::Indexed);
    let uses = |name| analysis.binding_uses(named_binding(tree.target(), name));
    assert!(uses("local").function.is_some());
    assert!(!uses("local").captured);
    assert!(
        uses("shared").captured,
        "a captured write counts even without a captured read"
    );
    assert_eq!(uses("shared").writes.len(), 1);

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
fn discardable_cell_reads_remain_nonrepeatable_and_tdz_reads_still_throw() {
    compare_source_with_interpreter("int f(int x){x=2;x;return x;}print(f(7));");
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "int f(int x){x=2;x;return x;}print(f(7));").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let region = &tree.target().regions[tree.target().functions[0].body.index()];
    let Statement::Evaluate(read) = region.statements[1] else {
        panic!("missing cell read")
    };
    let mut analysis = analysis::Analysis::new(&tree, analysis::Mode::Indexed);
    let facts = analysis.facts(read);
    assert!(facts.effects.discardable());
    assert!(!facts.effects.stable_scalar());
    drop(analysis);
    let outcome = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert_eq!(outcome.report.discarded_expressions, 1);

    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let module = tree.target_mut();
    let symbol = binding(module, RegionId::new(0), 999, "later");
    let read = expr(module, Expr::Binding(symbol));
    module.regions[0].statements.push(Statement::Evaluate(read));
    let one = number(module, 1.0);
    module.regions[0].statements.push(Statement::Let {
        binding: symbol,
        value: Some(one),
    });
    let target = expr(module, Expr::Binding(symbol));
    let assignment = expr(module, Expr::Assign { target, value: one });
    module.regions[0]
        .statements
        .push(Statement::Evaluate(assignment));
    let outcome = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert_eq!(outcome.report.discarded_expressions, 0);
    let javascript = tree
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let script = format!("try{{{javascript}}}catch(e){{console.log(e instanceof ReferenceError)}}");
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(result.status.success());
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "true\n");
}

#[test]
fn direct_eval_invalidates_target_normalization_proofs_as_well_as_names() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let module = tree.target_mut();
    let symbol = binding(module, RegionId::new(0), 1000, "value");
    let one = number(module, 1.0);
    module.regions[0].statements.push(Statement::Let {
        binding: symbol,
        value: Some(one),
    });
    let eval = host(module, "eval");
    let source = expr(
        module,
        Expr::Literal(Literal::String(
            StringValue::decode_source("value=1.5").unwrap(),
        )),
    );
    let eval = call(module, eval, vec![source], Invocation::DirectEval);
    module.regions[0].statements.push(Statement::Evaluate(eval));
    let read = expr(module, Expr::Binding(symbol));
    let negate = expr(module, Expr::IntNegate(read));
    capture(module, negate);
    let outcome = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert!(!outcome.report.changed());
    let view =
        extract::JavaScriptView::prepare_reusing(&tree, outcome.unchanged_facts.unwrap()).unwrap();
    assert_eq!(view.omitted_normalizations, 0);
    let javascript = view
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    let script = format!("function capture(value){{console.log(value)}}{javascript}");
    let result = Command::new("node")
        .args(["--input-type=module", "-e", &script])
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(String::from_utf8(result.stdout).unwrap(), "-1\n");
}

#[test]
fn source_eval_waits_for_a_declared_binding_contract() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "extern int eval(string source);").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let error = lower::lower_slice(&program, &semantics).unwrap_err();
    assert_eq!(error.feature, "source-level eval binding contract");
}

fn compare_region_value_facts(tree: &lower::AnnotatedTree<'_, '_>) {
    use crate::compilation_contract::JavaScriptWorld;
    use analysis::{Analysis, Mode};
    let mut direct =
        Analysis::new_in_world(tree, Mode::Regions, JavaScriptWorld::ClosedApplication);
    let mut indexed =
        Analysis::new_in_world(tree, Mode::Values, JavaScriptWorld::ClosedApplication);
    for (id, expression) in tree.target().expressions.iter().enumerate() {
        assert_eq!(
            direct.facts(ExprId::new(id)),
            indexed.facts(ExprId::new(id)),
            "{id}: {expression:?}"
        );
    }
}

#[test]
fn region_versions_preserve_assignment_values_and_merge_all_branch_inputs() {
    use crate::compilation_contract::JavaScriptWorld;
    use analysis::{Analysis, IntegerRange, Mode};
    let source = r#"
        int values(int choice) {
            int x=2; x=5; int a=x+3;
            if(choice>0){x=9;}else{x=9;}
            int b=x+2;
            if(choice>0){x=3;}else{x=8;}
            int c=x+1;
            x=20;
            return a+b+c+x;
        }
        print(values(0));print(values(1));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    compare_region_value_facts(&tree);
    let symbol = named_binding(tree.target(), "x");
    for mode in [Mode::Regions, Mode::Values] {
        let mut analysis = Analysis::new_in_world(&tree, mode, JavaScriptWorld::ClosedApplication);
        let reads: Vec<_> = analysis
            .binding_uses(symbol)
            .reads
            .iter()
            .map(|read| read.expression)
            .collect();
        let ranges: Vec<_> = reads
            .into_iter()
            .map(|read| analysis.facts(read).integer)
            .collect();
        assert_eq!(
            ranges,
            vec![
                Some(IntegerRange {
                    minimum: 5,
                    maximum: 5
                }),
                Some(IntegerRange {
                    minimum: 9,
                    maximum: 9
                }),
                Some(IntegerRange {
                    minimum: 3,
                    maximum: 8
                }),
                Some(IntegerRange {
                    minimum: 20,
                    maximum: 20
                }),
            ]
        );
        if mode == Mode::Values {
            assert!(analysis.work().flow.producer_edges > 0);
            assert!(analysis.work().flow.joins >= 2);
        } else {
            assert_eq!(analysis.work().flow.dependency_bytes, 0);
        }
    }
}

#[test]
fn region_values_do_not_promote_one_loop_iteration_or_one_short_circuit_path() {
    let source = r#"
        int looped(int n) {
            int x=2; int i=0;
            while(i<n) {
                x=x+1;i=i+1;
                if(i==2){continue;}
                if(i==4){break;}
                print(x+1);
            }
            print(x+1);
            bool update=n>0&&(x=8)>0;
            print(x+1);
            bool keep=n>0||(x=12)>0;
            return x+1;
        }
        print(looped(0));print(looped(1));print(looped(8));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    compare_region_value_facts(&tree);
}

#[test]
fn region_values_keep_captured_writes_and_reentrant_calls_observable() {
    let source = r#"
        int outer(int n) {
            int cell=1;
            auto change=(int value)=>{cell=value;return cell;};
            int before=cell+1;
            int result=change(n)+cell;
            return before+result+cell;
        }
        print(outer(4));print(outer(7));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    compare_region_value_facts(&tree);
}

#[test]
fn region_value_barriers_follow_getters_and_property_key_coercion_order() {
    use crate::compilation_contract::JavaScriptWorld;
    use analysis::Mode;
    for scenario in 0..3 {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, "").unwrap();
        let semantics = crate::analyze(&program).unwrap();
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let module = tree.target_mut();
        let cell = binding(module, RegionId::new(0), 1000, "cell");
        let object = binding(module, RegionId::new(0), 1001, "holder");
        let key = binding(module, RegionId::new(0), 1002, "key");
        let one = number(module, 1.0);
        module.regions[0].statements.push(Statement::Let {
            binding: cell,
            value: Some(one),
        });
        let writer_body = module.region(ScopeId::new(0));
        let target = expr(module, Expr::Binding(cell));
        let nine = number(module, 9.0);
        let write = expr(
            module,
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
            suspension: crate::structured_js::Suspension::None,
            arrow: true,
            parameters: vec![],
            body: writer_body,
        });
        let writer = expr(module, Expr::Function(FunctionId::new(0)));
        let install = host(module, "install");
        let install = call(module, install, vec![writer], Invocation::Value);
        module.regions[0]
            .statements
            .push(Statement::Evaluate(install));
        let host_object = host(module, "foreignObject");
        let host_key = host(module, "foreignKey");
        module.regions[0].statements.push(Statement::Let {
            binding: object,
            value: Some(host_object),
        });
        module.regions[0].statements.push(Statement::Let {
            binding: key,
            value: Some(host_key),
        });
        let target = expr(module, Expr::Binding(cell));
        let reset = expr(module, Expr::Assign { target, value: one });
        module.regions[0]
            .statements
            .push(Statement::Evaluate(reset));
        let holder = expr(module, Expr::Binding(object));
        let key_read = expr(module, Expr::Binding(key));
        let read = expr(module, Expr::Binding(cell));
        let negative = expr(module, Expr::IntNegate(read));
        match scenario {
            0 => {
                let method = expr(
                    module,
                    Expr::Member {
                        object: holder,
                        property: Property::Named("method".into()),
                    },
                );
                let result = call(module, method, vec![negative], Invocation::Reference);
                capture(module, result);
            }
            1 => {
                let object = expr(
                    module,
                    Expr::Object(vec![(Property::Computed(key_read), negative)]),
                );
                capture(module, object);
            }
            2 => {
                let target = expr(
                    module,
                    Expr::Member {
                        object: holder,
                        property: Property::Computed(key_read),
                    },
                );
                let write = expr(
                    module,
                    Expr::Assign {
                        target,
                        value: negative,
                    },
                );
                module.regions[0]
                    .statements
                    .push(Statement::Evaluate(write));
                let holder = expr(module, Expr::Binding(object));
                let read = expr(
                    module,
                    Expr::Member {
                        object: holder,
                        property: Property::Named("x".into()),
                    },
                );
                capture(module, read);
            }
            _ => unreachable!(),
        }
        compare_region_value_facts(&tree);
        let setup = "let writer;function install(value){writer=value}const foreignObject={get method(){writer();return value=>value}};const foreignKey={toString(){writer();return 'x'}};";
        let expected = match scenario {
            0 => "[-9]",
            1 => "[{\"x\":-9}]",
            // Simple assignment coerces its key during PutValue, after RHS.
            2 => "[-1]",
            _ => unreachable!(),
        };
        assert_eq!(
            execute(tree.target(), setup, PrintPolicy::default()),
            expected
        );
        for mode in [Mode::Regions, Mode::Values] {
            let mut tree = tree.clone();
            for _ in 0..3 {
                let result =
                    optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication)
                        .unwrap();
                if !result.report.changed() {
                    break;
                }
            }
            let view = extract::JavaScriptView::prepare_in_world(
                &tree,
                mode,
                JavaScriptWorld::ClosedApplication,
            );
            let javascript = view
                .render(PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap();
            let script = format!(
                "const outputs=[];function capture(value){{outputs.push(value)}}{setup}{javascript};console.log(JSON.stringify(outputs))"
            );
            let result = Command::new("node")
                .args(["--input-type=module", "-e", &script])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{javascript}\n{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap().trim(),
                expected,
                "{mode:?} {scenario}"
            );
        }
    }
}

#[test]
fn unobserved_cells_disappear_without_losing_initializer_effects_or_assignment_values() {
    let source = r#"
        int tick(int value){print(value);return value;}
        int work(){int discarded=tick(1);discarded=tick(2);int result=(discarded=tick(3))+5;return result;}
        int neverCalled(){print(99);return 1;}
        print(work());
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        analysis::Mode::Indexed,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let mut stores = 0;
        for _ in 0..3 {
            let result = optimize::optimize(
                &mut tree,
                mode,
                crate::compilation_contract::JavaScriptWorld::ClosedApplication,
            )
            .unwrap();
            stores += result.report.removed_stores;
            if !result.report.changed() {
                break;
            }
        }
        assert_eq!(stores, 2);
        assert!(tree
            .target()
            .bindings
            .iter()
            .all(|binding| !matches!(binding.spelling.as_str(), "discarded" | "neverCalled")));
        assert_eq!(tree.target().functions.len(), 2);
    }
}

#[test]
fn removing_unobserved_stores_requires_initialization_and_the_declared_public_boundary() {
    use crate::compilation_contract::JavaScriptWorld;
    use analysis::Mode;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "int exposed=1;exposed=2;").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [Mode::Indexed, Mode::Regions, Mode::Values] {
        let mut library = lower::lower_slice(&program, &semantics).unwrap();
        let result =
            optimize::optimize(&mut library, mode, JavaScriptWorld::ReusableLibrary).unwrap();
        assert!(!result.report.changed());
        assert_eq!(library.target().bindings.len(), 1);
        let mut closed = lower::lower_slice(&program, &semantics).unwrap();
        let result =
            optimize::optimize(&mut closed, mode, JavaScriptWorld::ClosedApplication).unwrap();
        assert_eq!(result.report.removed_stores, 1);
        assert!(closed.target().bindings.is_empty());

        let empty = crate::parse_source(&arena, "").unwrap();
        let semantics = crate::analyze(&empty).unwrap();
        let mut tree = lower::lower_slice(&empty, &semantics).unwrap();
        let module = tree.target_mut();
        let symbol = binding(module, RegionId::new(0), 1000, "uninitialized");
        let target = expr(module, Expr::Binding(symbol));
        let rhs = host(module, "take");
        let rhs = call(module, rhs, vec![], Invocation::Value);
        let assign = expr(module, Expr::Assign { target, value: rhs });
        module.regions[0]
            .statements
            .push(Statement::Evaluate(assign));
        let initial = number(module, 1.0);
        module.regions[0].statements.push(Statement::Let {
            binding: symbol,
            value: Some(initial),
        });
        let result =
            optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication).unwrap();
        assert_eq!(result.report.removed_stores, 0);
        let javascript = tree
            .render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap();
        let script = format!(
            "const trace=[];function take(){{trace.push('rhs');return 7}}try{{{javascript}}}catch(error){{trace.push(error.name)}}console.log(JSON.stringify(trace))"
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            "[\"rhs\",\"ReferenceError\"]"
        );
    }
}

#[test]
fn region_planning_consumes_child_results_without_assuming_arena_execution_order() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    let module = tree.target_mut();
    let child = module.region(ScopeId::new(0));
    let value = number(module, 42.0);
    module.regions[child.index()]
        .statements
        .push(Statement::Evaluate(value));
    module.regions[0]
        .statements
        .push(Statement::Block(RegionId::new(0)));
    module.regions.swap(0, 1);
    module.root = RegionId::new(1);
    // This valid program has its root stored after its child. The verifier's
    // ownership traversal supplies the order; numeric arena order does not.
    let result = optimize::optimize(
        &mut tree,
        analysis::Mode::Indexed,
        crate::compilation_contract::JavaScriptWorld::ClosedApplication,
    )
    .unwrap();
    assert_eq!(result.report.discarded_expressions, 1);
    assert_eq!(result.report.simplified_control, 1);
    assert_eq!(tree.target().regions.len(), 1);
    assert!(tree.target().regions[tree.target().root.index()]
        .statements
        .is_empty());
}

#[test]
fn source_templates_keep_probe_bindings_and_string_operations_visible() {
    compare_source_output(
        r#"
        int strings(int seed) {
          string a = "probe";
          string b = a + "-lil";
          string t = `seed=${seed} len=${b.length}`;
          int code = b.charCodeAt(1);
          int found = t.indexOf("len");
          string sliced = b.slice(0, 5);
          return b.length + t.length + code + found + sliced.length;
        }
        int templateBinding(int seed) {
          int live = seed * 2;
          string rendered = `value=${live}`;
          return rendered.length;
        }
        print(strings(7));print(strings(-3));
        print(templateBinding(7));print(templateBinding(-3));
        int live=9;
        print(`${live}${true}${2.5}`);
        print(`prefix ${`inner ${live}`} suffix`);
        print(`${"}" + live /* } ` ignored inside a comment */}`);
        print(`${(() => {return "quoted }";})()}`);
        print(``);
        "#,
        "147\n149\n8\n8\n9true2.5\nprefix inner 9 suffix\n}9\nquoted }\n\n",
    );
}

#[test]
fn source_templates_cook_escapes_without_losing_surrogates_or_creating_substitutions() {
    let source = r#"
        extern void units(string value);
        int amount=7;
        units(`\ud800${amount}\udfff`);
        units(`\` \${number} \\ \u{1f600}`);
        units(`line one
line two${amount}`);
        units(`\u0024{notAnIdentifier}${amount}`);
        units(`a\
b${amount}`);
        "#
    .replace("line one\nline two", "line one\r\nline two");
    compare_source_output_with_setup(
        &source,
        "globalThis.units=value=>console.log(Array.from({length:value.length},(_,i)=>value.charCodeAt(i)).join(','));",
        "55296,55,57343\n96,32,36,123,110,117,109,98,101,114,125,32,92,32,55357,56832\n108,105,110,101,32,111,110,101,10,108,105,110,101,32,116,119,111,55\n36,123,110,111,116,65,110,73,100,101,110,116,105,102,105,101,114,125,55\n97,98,55\n",
    );
}

#[test]
fn template_conversions_preserve_string_hint_order_reentry_and_abrupt_completion() {
    compare_source_output_with_setup(
        r#"
        extern JsValue token(func()->void write);
        extern JsValue symbol();
        extern int later();
        extern string trace();
        int value=0;
        void write(){value=2147483647;}
        JsValue delayed=token(write);
        value=0;
        print(`${delayed}${value+1}`);
        value=0;
        string unused=`${delayed}`;
        print(value+1);
        try {print(`${symbol()}${later()}`);}catch{print("caught");}
        print(trace());
        "#,
        "const events=[];globalThis.token=write=>({[Symbol.toPrimitive](hint){events.push(hint);write();if(hint!=='string')throw new Error('wrong hint');return 'converted:'}});globalThis.symbol=()=>Symbol('failure');globalThis.later=()=>{events.push('later');return 9};globalThis.trace=()=>events.join(',');",
        "converted:-2147483648\n-2147483648\ncaught\nstring,string\n",
    );
}

#[test]
fn unused_template_results_preserve_target_tdz_reads() {
    let arena = bumpalo::Bump::new();
    let program =
        crate::parse_source(&arena, "string value=\"ok\";string unused=`${value}`;").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        // Source checking rejects a direct predeclaration read. The target
        // supports it, and transformations must retain its ReferenceError.
        tree.target_mut().regions[0].statements.swap(0, 1);
        optimize::optimize(
            &mut tree,
            mode,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
        )
        .unwrap();
        let code = tree
            .render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap();
        let script = format!("try{{{code}}}catch(e){{console.log(e instanceof ReferenceError)}}");
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(result.status.success());
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "true\n",
            "{mode:?}: {code}"
        );
    }
}

#[test]
fn template_conversions_keep_observable_function_names() {
    compare_source_output_with_setup(
        r#"
        extern JsValue inspected();
        string observedTemplate(){return `${inspected()}`;}
        print(observedTemplate());
        "#,
        "globalThis.inspected=()=>({toString(){return new Error().stack.includes('observedTemplate')?'named':'lost'}});",
        "named\n",
    );
}

#[test]
fn source_exception_regions_preserve_probe_recovery_and_finalizer_completions() {
    compare_source_output(
        r#"
        int exceptions(int seed) {
          int total = 0;
          try {
            if (seed >= 0) {
              throw "seeded";
            }
            total += 1;
          } catch (auto caught) {
            total += 10;
          } finally {
            total += 100;
          }
          return total;
        }
        print(exceptions(7));print(exceptions(-3));
        int changed=0;
        int saved(){try{return changed;}finally{changed=9;}}
        print(saved());print(changed);
        int replaced(){try{return 1;}finally{return 2;}}
        print(replaced());
        try {
            try {throw "original";} finally {print("inner");throw "replacement";}
        } catch (auto error) {print(error);} finally {print("outer");}
        try {print("normal");} finally {print("done");}
        try {throw null;} catch {print("caught");}
    "#,
        "110\n101\n0\n9\n2\ninner\nreplacement\nouter\nnormal\ndone\ncaught\n",
    );
}

#[test]
fn finalizers_preserve_and_override_loop_transfers_without_copying_bodies() {
    compare_source_output(
        r#"
        int total=0;
        for(int i=0;i<5;i++){
            try {
                if(i==0){continue;}
                if(i==2){break;}
                total+=10;
            } finally {total+=i+1;}
        }
        print(total);
        int visits=0;
        for(int i=0;i<3;i++){
            try {break;} finally {visits+=1;if(i<2){continue;}}
        }
        print(visits);
        int stopped=0;
        for(int i=0;i<3;i++){
            try {continue;} finally {stopped+=1;break;}
        }
        print(stopped);
        int caught=0;
        for(int i=0;i<3;i++){
            try {throw i;}catch{continue;}finally{caught+=1;}
        }
        print(caught);
    "#,
        "16\n3\n1\n3\n",
    );
}

#[test]
fn exception_predecessors_do_not_inherit_values_from_unreached_body_tails() {
    compare_source_output_with_setup(
        r#"
        int choose(bool fail){
            int value=2147483647;
            try {if(fail){throw "early";}value=0;}
            catch {print(value+1);}
            finally {print(value+1);}
            return value+1;
        }
        print(choose(true));print(choose(false));
        extern void fail();
        int value=2147483647;
        try {fail();value=0;} catch(auto error){print(error);print(value+1);}
        finally {print(value+1);}
        print(value+1);
        int stopped(){
            int result=2147483647;
            try {return result+1;result=0;}
            finally {print(result+1);}
        }
        print(stopped());
    "#,
        "globalThis.fail=()=>{throw 'foreign'};",
        "-2147483648\n-2147483648\n-2147483648\n1\n1\nforeign\n-2147483648\n-2147483648\n-2147483648\n-2147483648\n-2147483648\n",
    );
}

#[test]
fn catch_bindings_have_fresh_scope_cells_and_survive_escaping_closures() {
    compare_source_output(
        r#"
        (func()->JsValue)[] readers=[];
        (func()->void)[] writers=[];
        for(int i=0;i<3;i++){
            try {throw i;}catch(auto caught){
                readers.push(()=>caught);
                writers.push(()=>{caught=99;});
            }
        }
        print(readers[0]());print(readers[1]());print(readers[2]());
        writers[1]();print(readers[0]());print(readers[1]());print(readers[2]());
        try {throw "outer";}catch(auto caught){
            auto read=()=>caught;
            try {throw "inner";}catch(auto caught){print(caught);}
            print(read());
        }
    "#,
        "0\n1\n2\n0\n99\n2\ninner\nouter\n",
    );
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
fn source_collections_preserve_probe_behavior_aliases_and_nullable_values() {
    // Map/Set are outside the independent interpreter's implemented subset.
    // These explicit observations cover the complete Probe collections body.
    compare_source_output_with_setup(
        r#"
        int collections(int seed) {
          Map<string, int> map = new Map<string, int>();
          map.set("a", seed);
          map.set("b", seed * 2);
          Set<int> set = new Set<int>();
          set.add(seed);
          set.add(seed);
          set.add(seed + 1);
          int total = map.size + set.size;
          if (map.has("a")) { total += map.get("a") ?? 0; }
          return total;
        }
        print(collections(7));print(collections(0));print(collections(-3));
        Map<string,int?> values=new Map();auto alias=values;
        print(values.set("zero",0).set("null",null)==values);
        print(alias.get("zero")??99);print(values.get("null")==null);
        print(values.get("missing")==null);print(values.has("null"));
        print(alias.delete("zero"));print(values.delete("zero"));print(values.size);
        values.clear();print(alias.size);
        Map<string,bool> flags=new Map();flags.set("off",false);print(flags.get("off")??true);
        Map<string,string> words=new Map();words.set("empty","");print(words.get("empty")??"missing");
        Set<float> seen=new Set();print(seen.add(0.0).add(-0.0)==seen);print(seen.size);
        extern float nan();seen.add(nan());seen.add(nan());print(seen.size);print(seen.has(nan()));
        print(seen.delete(0.0));print(seen.size);seen.clear();print(seen.size);
        Map<string,func()->int> callbacks=new Map();callbacks.set("run",()=>7);
        auto found=callbacks.get("run")??(()=>0);print(found());
        extern string nameOf(func()->int value);print(nameOf(found));
    "#,
        "globalThis.nan=()=>NaN;globalThis.nameOf=value=>value.name;",
        "11\n4\n1\ntrue\n0\ntrue\ntrue\ntrue\ntrue\nfalse\n1\n0\nfalse\n\ntrue\n1\n2\ntrue\ntrue\n1\n0\n7\n\n",
    );
}

#[test]
fn collection_receiver_lookup_arguments_and_language_results_stay_ordered() {
    compare_source_output_with_setup(
        r#"
        extern Map<string,int> receiver();extern string key();extern void report();
        extern void install(func()->void mutate);
        int edge=0;install(()=>{edge=2147483647;});edge=0;
        receiver().set(key(),edge+1);report();
        print(receiver().get(key())==null);report();
        print(receiver().size);report();
        receiver().get("unused");report();
    "#,
        r#"
        let events=[],mutate;
        const collection=new Proxy(Object.create(null),{get(target,name){
            events.push('get:'+name);
            if(name==='size')return 4294967295;
            if(name==='set'){
                mutate();
                return function(key,value){events.push('set:'+key+':'+value+':'+(this===collection));return this};
            }
            if(name==='get')return function(key){events.push('get:'+key+':'+(this===collection));return undefined};
            throw Error('unexpected member');
        }});
        globalThis.receiver=()=>{events.push('receiver');return collection};
        globalThis.key=()=>{events.push('key');return 'a'};
        globalThis.install=action=>{mutate=action};
        globalThis.report=()=>{console.log(events.join(','));events=[]};
    "#,
        "receiver,get:set,key,set:a:-2147483648:true\ntrue\nreceiver,get:get,key,get:a:true\n-1\nreceiver,get:size\nreceiver,get:get,get:unused:true\n",
    );
}

#[test]
fn unused_collection_construction_retains_lookup_allocation_and_reentry() {
    compare_source_output_with_setup(
        r#"
        extern void install(func()->void mutate);
        int changes=0;install(()=>{changes+=1;});
        new Map<string,int>();new Set<int>();print(changes);
    "#,
        r#"
        globalThis.install=mutate=>{
            for(const name of ['Map','Set']){
                const Native=globalThis[name];
                Object.defineProperty(globalThis,name,{configurable:true,get(){
                    console.log('lookup:'+name);mutate();
                    return new Proxy(Native,{construct(target,args){
                        console.log('construct:'+name+':'+args.length);mutate();return new Native();
                    }});
                }});
            }
        };
    "#,
        "lookup:Map\nconstruct:Map:0\nlookup:Set\nconstruct:Set:0\n4\n",
    );
}

#[test]
fn checked_collection_result_identity_attaches_to_the_language_value() {
    use crate::primitive::ResolvedIntrinsic::{Constructor, Method};
    let source =
        "Map<string,int> values=new Map();auto found=values.get(\"missing\");print(found==null);";
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let (result, raw) = tree
        .target()
        .expressions
        .iter()
        .enumerate()
        .find_map(|(id, node)| {
            let Expr::Binary {
                op: Binary::Nullish,
                left,
                ..
            } = node
            else {
                return None;
            };
            Some((ExprId::new(id), *left))
        })
        .unwrap();
    assert_eq!(
        tree.source_facts(result).unwrap().resolution,
        lower::Resolution::Primitive(Method(Intrinsic::MapGet))
    );
    assert!(matches!(
        tree.source_facts(result).unwrap().ty,
        Some(crate::semantic::Type::Nullable(_))
    ));
    assert!(tree.source_facts(raw).is_none());
    assert!(tree.target().expressions.iter().enumerate().any(|(id, node)| {
        matches!(node, Expr::ConstructIntrinsic { operation: Intrinsic::MapNew, arguments } if arguments.is_empty())
            && tree.source_facts(ExprId::new(id)).unwrap().resolution
                == lower::Resolution::Primitive(Constructor(Intrinsic::MapNew))
    }));
    for mode in [
        analysis::Mode::Indexed,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut analysis = analysis::Analysis::new(&tree, mode);
        assert!(!analysis.facts(raw).effects.discardable());
        assert!(!analysis.facts(result).effects.discardable());
    }
}

#[test]
fn binary_memory_construction_preserves_probe_views_and_shared_storage() {
    compare_source_with_interpreter(
        r#"
        int memory(int seed) {
          ArrayBuffer storage = new ArrayBuffer(8);
          Uint8Array bytes = new Uint8Array(storage);
          bytes[0] = seed;
          bytes[1] = seed >>> 8;
          bytes[2] = 255;
          Uint8Array view = bytes.subarray(1, 4);
          int previous = view[0]++;
          return bytes[0] + bytes[1] + bytes[2] + previous + view.byteOffset + view.length;
        }
        print(memory(7));print(memory(0));print(memory(-3));print(memory(511));
        SharedArrayBuffer shared=new SharedArrayBuffer(8);
        Int32Array signed=new Int32Array(shared);
        Uint32Array unsigned=new Uint32Array(shared);
        signed[0]=-1;print(unsigned[0]);print(signed.byteLength);
        auto tail=unsigned.subarray(1);tail[0]=42;print(signed[1]);
        print(tail.byteOffset);print(tail.length);print(tail.buffer==shared);
        auto copied=unsigned.slice(0,1);copied[0]=9;print(signed[0]);print(copied[0]);
    "#,
    );
}

#[test]
fn binary_memory_uses_the_checked_kinds_and_keeps_store_rounding() {
    compare_source_with_interpreter(
        r#"
        Int8Array a=new Int8Array(1);a[0]=255;print(a[0]);
        Uint8Array b=new Uint8Array(1);b[0]=-1;print(b[0]);
        Uint8ClampedArray c=new Uint8ClampedArray(1);c[0]=511;print(c[0]);
        Int16Array d=new Int16Array(1);d[0]=65535;print(d[0]);
        Uint16Array e=new Uint16Array(1);e[0]=-1;print(e[0]);
        Int32Array f=new Int32Array(1);f[0]=-1;print(f[0]);
        Uint32Array g=new Uint32Array(1);g[0]=-1;print(g[0]);
        Float32Array h=new Float32Array(1);h[0]=16777217.0;print(h[0]);
        Float64Array i=new Float64Array(1);i[0]=16777217.0;print(i[0]);
        print(a.length);print(b.byteOffset);print(c.byteLength);print(d.byteLength);
        print(e.byteLength);print(f.byteLength);print(g.byteLength);print(h.byteLength);print(i.byteLength);
        a.fill(7);print(a[0]);auto copy=a.slice(0);a.set(copy);print(a[0]);
    "#,
    );
}

#[test]
fn implicit_constructor_lookup_precedes_arguments_and_preserves_throws() {
    compare_source_output_with_setup(
        r#"
        extern void install(func()->void mutate);
        extern void check(func()->void action);
        int edge=0;
        install(()=>{edge=2147483647;});
        edge=0;
        auto value=new ArrayBuffer(edge+1);
        print(value.byteLength);
        check(()=>{new Uint8Array(-1);});
    "#,
        r#"
        const NativeBuffer=globalThis.ArrayBuffer;
        globalThis.install=mutate=>Object.defineProperty(globalThis,'ArrayBuffer',{
            configurable:true,get(){
                console.log('lookup');mutate();
                return new Proxy(NativeBuffer,{construct(target,args){
                    console.log(args[0]);return new NativeBuffer(0);
                }});
            }
        });
        globalThis.check=action=>{try{action();console.log('missed')}catch(error){console.log(error.name)}};
    "#,
        "lookup\n-2147483648\n0\nRangeError\n",
    );
}

#[test]
fn constructor_and_property_resolution_survive_optional_access_fusion() {
    use crate::primitive::ResolvedIntrinsic::{Constructor, Property};
    let source = r#"
        ArrayBuffer storage=new ArrayBuffer(8);
        auto view=new Uint8Array(storage);
        int inspect(ArrayBuffer? value){return value?.byteLength??0;}
        print(view.length);print(view.byteLength);print(view.byteOffset);auto underlying=view.buffer;
        print(inspect(storage));print(inspect(null));
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let initializer = |index: usize| {
        let crate::ast::Item::Stmt(crate::ast::Stmt::VarDecl(declaration)) = &program.items[index]
        else {
            panic!("expected variable")
        };
        declaration.initializer.as_ref().unwrap()
    };
    let printed = |index: usize| {
        let crate::ast::Item::Stmt(crate::ast::Stmt::Expr(value)) = &program.items[index] else {
            panic!("expected print")
        };
        let crate::ast::ExprKind::Call { args, .. } = &value.kind else {
            panic!("expected call")
        };
        &args[0]
    };
    let crate::ast::Item::Function(inspect) = &program.items[2] else {
        panic!("expected inspect")
    };
    let crate::ast::Stmt::Return {
        value: Some(value), ..
    } = &inspect.body[0]
    else {
        panic!("expected return")
    };
    let crate::ast::ExprKind::Binary { lhs: optional, .. } = &value.kind else {
        panic!("expected fallback")
    };
    let operation =
        |expression: &crate::ast::Expr<'_, '_>| semantics.resolved_intrinsic(expression.id);
    assert_eq!(
        operation(initializer(0)),
        Some(Constructor(Intrinsic::ArrayBufferNew))
    );
    assert_eq!(
        operation(initializer(1)),
        Some(Constructor(Intrinsic::Uint8ArrayNew))
    );
    assert_eq!(
        operation(&printed(3).expression),
        Some(Property(Intrinsic::Uint8ArrayLength))
    );
    assert_eq!(
        operation(&printed(4).expression),
        Some(Property(Intrinsic::Uint8ArrayByteLength))
    );
    assert_eq!(
        operation(&printed(5).expression),
        Some(Property(Intrinsic::Uint8ArrayByteOffset))
    );
    assert_eq!(
        operation(initializer(6)),
        Some(Property(Intrinsic::Uint8ArrayBuffer))
    );
    assert_eq!(
        operation(optional),
        Some(Property(Intrinsic::BufferByteLength))
    );
    // The fused IR operation has the whole ?? location, while its checked
    // property identity came from the optional member. Both must be retained.
    crate::lower_to_control_flow(&program, &semantics).unwrap();

    let source = "ArrayBuffer value=new ArrayBuffer(4);print(value.byteLength);";
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let construction = tree
        .target()
        .expressions
        .iter()
        .enumerate()
        .find_map(|(id, node)| {
            matches!(node, Expr::ConstructIntrinsic { .. }).then_some(ExprId::new(id))
        })
        .unwrap();
    assert!(matches!(
        tree.source_facts(construction).unwrap().resolution,
        lower::Resolution::Primitive(Constructor(Intrinsic::ArrayBufferNew))
    ));
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
fn indexed_compounds_capture_receiver_key_and_old_value_before_rhs() {
    compare_source_with_interpreter(
        r#"
        int[] values=[10,20];int[] original=values;int index=0;
        int rhs(){values=[90,91];index=1;return 2;}
        print(values[index]+=rhs());print(original[0]);print(values[0]);print(index);
        int[] second=values;
        int key(){values=[30,31];return 0;}
        print(values[key()]++);print(second[0]);print(values[0]);
        int[] edge=[2147483647];print(edge[0]++);print(edge[0]);
        print(--edge[0]);print(edge[0]);
        edge[0]>>>=1;print(edge[0]);
        edge[0]/=0;print(edge[0]);
        float[] fractions=[1.25];print(fractions[0]++);print(++fractions[0]);
        string[] words=["a"];print(words[0]+="b");print(words[0]);
    "#,
    );
}

#[test]
fn indexed_updates_preserve_proxy_get_set_order_and_lazy_nullish_writes() {
    compare_source_output_with_setup(
        r#"
        extern int[] receiver();extern int key();extern int rhs();extern void report();
        print(receiver()[key()]+=rhs());report();
        print(receiver()[key()]++);report();
        print(++receiver()[key()]);report();
        extern (int?)[] nullable();
        print(nullable()[key()]??=rhs());report();
        print(nullable()[key()]??=rhs());report();
    "#,
        r#"
        let events=[];
        const data=new Proxy([2147483647],{
            get:(a,k)=>{events.push('get:'+k);return a[k]},
            set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}
        });
        const optional=new Proxy([null],{
            get:(a,k)=>{events.push('get:'+k);return a[k]},
            set:(a,k,v)=>{events.push('set:'+k+':'+v);a[k]=v;return true}
        });
        globalThis.receiver=()=>{events.push('receiver');return data};
        globalThis.nullable=()=>{events.push('nullable');return optional};
        globalThis.key=()=>{events.push('key');return 0};
        globalThis.rhs=()=>{events.push('rhs');return 1};
        globalThis.report=()=>{console.log(events.join(','));events=[]};
    "#,
        "-2147483648\nreceiver,key,get:0,rhs,set:0:-2147483648\n-2147483648\nreceiver,key,get:0,set:0:-2147483647\n-2147483646\nreceiver,key,get:0,set:0:-2147483646\n1\nnullable,key,get:0,rhs,set:0:1\n1\nnullable,key,get:0\n",
    );
}

#[test]
fn typed_array_stores_do_not_redefine_prefix_postfix_or_compound_results() {
    compare_source_output_with_setup(
        r#"
        extern Uint8Array bytes();
        auto values=bytes();
        print(++values[0]);print(values[0]);
        values[0]=255;print(values[0]++);print(values[0]);
        print(values[0]+=257);print(values[0]);
        extern Uint32Array words();
        auto ints=words();print(ints[0]++);print(ints[0]);
        extern Float32Array floats();
        auto fractions=floats();print(++fractions[0]);print(fractions[0]);
    "#,
        "globalThis.bytes=()=>new Uint8Array([255]);globalThis.words=()=>new Uint32Array([4294967295]);globalThis.floats=()=>new Float32Array([16777216]);",
        "256\n0\n255\n0\n257\n1\n-1\n0\n16777217\n16777216\n",
    );
}

#[test]
fn indexed_nullish_callable_creation_keeps_its_anonymous_name() {
    compare_source_output_with_setup(
        r#"
        extern string nameOf(func()->int value);
        ((func()->int)?)[] callbacks=[null];
        auto first=callbacks[0]??=()=>7;
        auto second=callbacks[0]??=()=>8;
        print(nameOf(first));print(nameOf(second));print(first());print(second());
    "#,
        "globalThis.nameOf=f=>f.name;",
        "\n\n7\n7\n",
    );
}

#[test]
fn for_control_keeps_continue_update_break_and_initializer_effects() {
    compare_source_with_interpreter(
        r#"
        int steps=0;
        int advance(int value){steps++;print(value);return value+1;}
        int walk(){
            int total=0;
            for(int i=0;i<5;i=advance(i)){
                if(i==1)continue;
                if(i==3)break;
                total+=i;
            }
            return total;
        }
        print(walk());print(steps);
        int i=7;
        for(i=2;i<4;i++)print(i);
        print(i);
        for(print(90);false;print(91))print(92);
        int stop(){for(;;){return 12;}return 90;}
        print(stop());
        int until=0;for(;until<2;)until++;print(until);
        int nested=0;
        for(int a=0;a<3;a++){
            for(int b=0;b<3;b++){
                if(b==1)continue;
                if(a==2)break;
                nested++;
            }
        }
        print(nested);
    "#,
    );
}

#[test]
fn for_initializer_cells_are_shared_and_body_cells_are_fresh() {
    let source = r#"
        void captures(){
            (func()->int)[] shared=[];
            (func()->int)[] fresh=[];
            (func()->int)[] writers=[];
            for(int index=0;index<3;index++){
                int local=index;
                shared.push(()=>index);
                fresh.push(()=>local);
                writers.push(()=>++local);
            }
            print(shared[0]());print(shared[2]());
            print(fresh[0]());print(fresh[1]());print(fresh[2]());
            print(writers[0]());print(fresh[0]());print(fresh[1]());
            int index=90;print(index);
        }
        captures();
    "#;
    // These are source cell-lifetime checks, not a choice of JS spelling.
    compare_source_with_interpreter(source);
    compare_source_output(source, "3\n3\n0\n1\n2\n1\n1\n1\n90\n");
}

#[test]
fn for_update_facts_do_not_assume_code_after_continue_executed() {
    compare_source_with_interpreter(
        r#"
        int edge=2147483647;
        int count=0;
        for(;count<1;print(edge+1)){
            count++;
            continue;
            edge=0;
        }
        int snapshots=0;
        int checks=0;
        bool test(int previous){checks++;return previous<2;}
        for(;test(snapshots++);print(snapshots++))print(snapshots);
        print(snapshots);print(checks);
        int absent=0;
        for(;false;print(absent++))print(999);
        print(absent);
        int skip(){for(int i=0;i<3;print(999))return i;return 90;}
        print(skip());
    "#,
    );
}

#[test]
fn for_updates_participate_in_scope_verification_and_late_naming() {
    let source = r#"
        extern int take(int value);
        int outer=take(3);
        int run(int input){
            int current=input;
            for(int counter=0;counter<2;current+=outer){
                counter++;
            }
            return current;
        }
        print(run(4));
    "#;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let mut tree = lower::lower_slice(&program, &semantics).unwrap();
    tree.target().verify().unwrap();
    for style in [
        naming::Style::Global,
        naming::Style::Scoped,
        naming::Style::Source,
    ] {
        let javascript = tree
            .output()
            .unwrap()
            .render(&naming::Plan::new(style))
            .unwrap();
        let output = Command::new("node")
            .args([
                "--input-type=module",
                "-e",
                &format!("globalThis.take=x=>x;{javascript}"),
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "10\n");
    }
    let module = tree.target_mut();
    let (body, update) = module
        .regions
        .iter()
        .flat_map(|region| &region.statements)
        .find_map(|statement| match statement {
            Statement::Loop {
                body,
                update: Some(update),
                ..
            } => Some((*body, *update)),
            _ => None,
        })
        .unwrap();
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
fn empty_control_retains_condition_effects_and_repeated_loop_execution() {
    let source = r#"
        int identity(int n){{if(n>0){int gone=n+1;}else{int gone=n+2;}}return n;}
        bool choose(){print(5);return true;}
        if(choose()){int gone=1;}else{int gone=2;}
        int count=0;
        bool next(){count=count+1;print(count);return count<3;}
        while(next()){int gone=1;}
        print(count);print(identity(7));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        analysis::Mode::Indexed,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        optimize::optimize_in_execution(
            &mut tree,
            mode,
            crate::compilation_contract::JavaScriptWorld::ClosedApplication,
            crate::compilation_contract::JavaScriptExecution::Module,
        )
        .unwrap();
        assert!(tree.target().regions.iter().all(|region| {
            region
                .statements
                .iter()
                .all(|statement| !matches!(statement, Statement::If { .. } | Statement::Block(_)))
        }));
        assert!(tree.target().regions.iter().any(|region| {
            region
                .statements
                .iter()
                .any(|statement| matches!(statement, Statement::Loop { .. }))
        }));
        let javascript = tree.render(PrintPolicy::default()).unwrap();
        let run = Command::new("node")
            .args(["--input-type=module", "-e", &javascript])
            .output()
            .unwrap();
        assert!(run.status.success());
        assert_eq!(String::from_utf8(run.stdout).unwrap(), "5\n1\n2\n3\n3\n7\n");
    }
}

#[test]
fn primitive_integer_results_use_emitted_normalization_without_assuming_host_range() {
    // This operation is taken from Probe's real codeAt coverage. The
    // independent interpreter uses UTF-16 and zero for an absent code unit.
    let source = r#"
        int codeAt(string text,int index){return text.charCodeAt(index);}
        int last(int[] values){return values.pop();}
        print(codeAt("ab",0));print(codeAt("",0));print(codeAt("ab",-1));
        print(codeAt("𝄞",0));print(codeAt("𝄞",1));print(codeAt("𝄞",2));
    "#;
    compare_source_with_interpreter(source);
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    for mode in [
        analysis::Mode::Indexed,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut analysis = analysis::Analysis::new(&tree, mode);
        let primitive = ExprId::new(
            tree.target()
                .expressions
                .iter()
                .position(|expression| {
                    matches!(
                        expression,
                        Expr::Intrinsic {
                            operation: Intrinsic::StringCharCodeAt,
                            ..
                        }
                    )
                })
                .unwrap(),
        );
        let facts = analysis.facts(primitive);
        assert_eq!(
            facts.integer,
            Some(analysis::IntegerRange {
                minimum: i32::MIN as i64,
                maximum: i32::MAX as i64
            })
        );
        assert!(
            !facts.normalization_redundant,
            "a replaceable code-unit method does not prove the raw result's range"
        );
        assert_eq!(
            tree.source_facts(primitive).unwrap().resolution,
            lower::Resolution::Primitive(crate::primitive::ResolvedIntrinsic::Method(
                Intrinsic::StringCharCodeAt
            ))
        );
        // Other not-yet-semantic receiver calls retain the earlier explicit
        // primitive-result boundary instead of borrowing the declared range.
        let (normalized, raw) = tree
            .target()
            .expressions
            .iter()
            .enumerate()
            .find_map(|(id, expression)| {
                if let Expr::ToInt32(raw) = expression {
                    return Some((ExprId::new(id), *raw));
                }
                None
            })
            .unwrap();
        assert_eq!(analysis.facts(raw).integer, None);
        assert!(!analysis.facts(normalized).normalization_redundant);
        assert!(analysis.facts(normalized).integer.unwrap().fits_i32());
        assert_eq!(
            tree.source_facts(normalized).unwrap().ty,
            Some(&crate::semantic::Type::Int)
        );
        assert!(tree.source_facts(raw).is_none());
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
    // The existing IR consumer must use the checked contextual operation too.
    // A similarly spelled user method keeps its nominal calling convention.
    crate::lower::lower_to_control_flow(&program, &semantics).unwrap();

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
fn typed_primitive_values_keep_utf16_effects_and_mutable_collection_observations() {
    // String.length is outside the current interpreter's implemented member
    // set. These declared cases complement the independent code-unit checks.
    compare_source_output(
        r#"
        int calls=0;
        string next(){calls=calls+1;return "𝄞";}
        next().length;print(calls);
        next().charCodeAt(0);print(calls);
        print("𝄞".length);print("𝄞".charCodeAt(0));print("𝄞".charCodeAt(1));
        print("𝄞".charCodeAt(2));print("\ud800".charCodeAt(0));
        int index=0;print("".charCodeAt(index=3));print(index);
        "ab".charAt(0);"ab".indexOf("a");
        int[] values=[1];int first=values.length;values.push(2);
        print(first);print(values.length);
        print("abc".indexOf("b"));print("abcabc".indexOf("b",3));
    "#,
        "1\n2\n2\n55348\n56606\n0\n55296\n0\n3\n1\n2\n1\n4\n",
    );
}

#[test]
fn primitive_method_values_require_a_supported_receiver_contract() {
    for source in [
        "auto method=\"text\".charCodeAt;print(method(0));",
        "auto values=new Map<string,int>();auto method=values.get;print(method(\"x\"));",
        "auto values=new Set<int>();auto method=values.has;print(method(1));",
    ] {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&program).unwrap();
        let error = lower::lower_slice(&program, &semantics).unwrap_err();
        assert_eq!(error.feature, "primitive methods require a receiver call");
    }
}

#[test]
fn marked_bracket_scanner_preserves_its_real_control_and_utf16_operations() {
    let source = include_str!("../../finer/tools/fixtures/marked-brackets.lil");
    // The archived port body is unchanged except for removing its export
    // keyword. These declared boundary cases execute against the emitted JS;
    // the current reference interpreter does not implement string.length.
    // Explicit calls also keep the Rust test independent of an extern setup.
    let body = source
        .split("extern string sample(int index);")
        .next()
        .unwrap();
    let source = format!(
        r#"{body}
        print(findClosingBracket("))","(",")"));
        print(findClosingBracket("ab(cd)e)tail","(",")"));
        print(findClosingBracket("()","(",")"));
        print(findClosingBracket("((x)","(",")"));
        print(findClosingBracket("a\\)b)","(",")"));
        print(findClosingBracket("𝄞)text","(",")"));
    "#
    );
    compare_source_output(&source, "0\n7\n-1\n-2\n4\n2\n");
}

#[test]
fn source_types_bindings_and_builtin_identity_survive_target_naming() {
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "float value=7.0;print(value+1.0);").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    let binary = tree
        .target()
        .expressions
        .iter()
        .position(|node| matches!(node, Expr::Binary { .. }))
        .unwrap();
    assert_eq!(
        tree.source_facts(ExprId::new(binary)).unwrap().ty,
        Some(&crate::Type::Float)
    );
    let print = tree
        .target()
        .expressions
        .iter()
        .enumerate()
        .find_map(|(index, _)| {
            let facts = tree.source_facts(ExprId::new(index))?;
            (facts.resolution == lower::Resolution::Builtin(crate::semantic::BuiltinCall::Print))
                .then_some(index)
        })
        .unwrap();
    let before = tree.source_facts(ExprId::new(print)).unwrap().span;
    tree.render(PrintPolicy {
        mangle_bindings: true,
    })
    .unwrap();
    assert_eq!(tree.source_facts(ExprId::new(print)).unwrap().span, before);
    assert!(tree.target().expressions.iter().enumerate().any(|(index, node)|
        matches!(node, Expr::Binding(symbol) if tree.source_facts(ExprId::new(index)).unwrap().resolution == lower::Resolution::Binding(tree.target().bindings[symbol.index()].source_symbol.unwrap()))));
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
    compare_source_with_interpreter(r#"print("a\n\u0041\t");print("\ud83d\ude00");"#);
}

#[test]
fn escaped_record_keys_and_inferred_names_preserve_code_units() {
    compare_source_output_with_setup(
        r#"
        extern string nameOf(func()->int value);
        func()->int fallback=()=>0;
        Record<func()->int> callbacks=record{"\x68andler":()=>7,"\ud800":()=>8};
        print(nameOf(callbacks["handler"]??fallback));
        print(nameOf(callbacks["\ud800"]??fallback).charCodeAt(0));
        print((callbacks["handler"]??fallback)());
        print((callbacks["\ud800"]??fallback)());
        "#,
        "globalThis.nameOf=value=>value.name;",
        "handler\n55296\n7\n8\n",
    );
}

#[test]
fn equivalent_escaped_record_keys_have_one_semantic_identity() {
    let arena = bumpalo::Bump::new();
    let program =
        crate::parse_source(&arena, r#"Record<int> values=record{a:1,"\x61":2};"#).unwrap();
    let error = crate::semantic::analyze(&program).unwrap_err();
    assert!(error.to_string().contains("duplicate"), "{error}");
}

#[test]
fn exact_string_values_cross_cells_joins_and_utf16_operations() {
    compare_source_output(
        r#"
        string values(bool choice) {
            string high="\ud83d";
            string pair=high+"\ude00";
            string same="";
            if(choice){same="\x61";}else{same="a";}
            return `${pair.length},${pair.charCodeAt(1)},${pair.indexOf("\ude00")},${pair.charAt(-1).length},${pair.charAt(1).charCodeAt(0)},${same}`;
        }
        print(values(true));print(values(false));
        string changed="ab";
        print((changed="different").length);print(changed);
        print(`\u0024${"{"}identifier}${"\\"}${"`"}`);
        print(`a${true}${2147483647}`);
        float negative=-0.0;print(`zero=${negative}`);
        "#,
        "2,56832,1,0,56832,a\n2,56832,1,0,56832,a\n9\ndifferent\n${identifier}\\`\natrue2147483647\nzero=0\n",
    );
}

#[test]
fn exact_string_knowledge_is_bounded_and_literal_payloads_stay_owned_by_the_program() {
    use crate::compilation_contract::JavaScriptWorld;
    assert!(std::mem::size_of::<analysis::Facts>() <= 32);
    assert_eq!(std::mem::size_of::<Option<constants::Known>>(), 4);
    let mut source = String::from("string part0=\"x\";");
    for i in 1..=14 {
        source.push_str(&format!("string part{i}=part{}+part{};", i - 1, i - 1));
    }
    source.push_str("print(part14.length);");
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, &source).unwrap();
    let semantics = crate::analyze(&program).unwrap();
    let tree = lower::lower_slice(&program, &semantics).unwrap();
    for mode in [analysis::Mode::Regions, analysis::Mode::Values] {
        let mut facts =
            analysis::Analysis::new_in_world(&tree, mode, JavaScriptWorld::ClosedApplication);
        for (index, expression) in tree.target().expressions.iter().enumerate() {
            if let Expr::Literal(Literal::String(_)) = expression {
                assert_eq!(
                    facts.facts(ExprId::new(index)).constant,
                    Some(constants::Known(ExprId::new(index)))
                );
            }
        }
        assert!(facts.work().constants.budget_stops > 0);
        assert!(facts.work().constants.owned_string_bytes <= 1024 * 1024);
        assert!(facts.work().constants.charged_work <= 8 * 1024 * 1024);
        assert!(facts.work().constants.evaluated_values < 14);
    }
    compare_source_output(&source, "16384\n");
}

#[test]
fn target_template_constants_preserve_null_undefined_and_negative_zero_distinctions() {
    use crate::compilation_contract::JavaScriptWorld;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let module = tree.target_mut();
        let null = expr(module, Expr::Literal(Literal::Null));
        let undefined = expr(module, Expr::Literal(Literal::Undefined));
        let negative_zero = number(module, -0.0);
        let text = expr(
            module,
            Expr::Template(vec![
                TemplatePart::Expression(null),
                TemplatePart::Expression(undefined),
                TemplatePart::Expression(negative_zero),
            ]),
        );
        capture(module, text);
        assert_eq!(
            execute(tree.target(), "", PrintPolicy::default()),
            r#"["nullundefined0"]"#
        );
        optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication).unwrap();
        assert_eq!(
            execute(
                tree.target(),
                "",
                PrintPolicy {
                    mangle_bindings: true
                }
            ),
            r#"["nullundefined0"]"#
        );
    }
}

#[test]
fn planned_observations_close_storage_without_another_semantic_round() {
    use crate::compilation_contract::JavaScriptWorld;
    let cases: &[(&str, &str, &str, &[&str])] = &[
        (
            r#"
            extern int read();
            int strings(int seed) {
                string a="probe";string b=a+"-lil";
                string t=`seed=${seed} len=${b.length}`;
                int code=b.charCodeAt(1);int found=t.indexOf("len");
                string sliced=b.slice(0,5);
                return b.length+t.length+code+found+sliced.length;
            }
            print(strings(read()));print(strings(-3));print(strings(0));
            "#,
            "globalThis.read=()=>17;",
            "149\n149\n147\n",
            // Method results are mutable host operations. `b` has multiple
            // surviving method receivers; `code` must retain its evaluation
            // before indexOf/slice rather than moving to the return expression.
            &["a"],
        ),
        (
            r#"
            int strings(int seed) {
                string a="probe";string b=a+"-lil";
                string t=`seed=${seed} len=${b.length}`;
                int code=b.charCodeAt(1);int found=t.indexOf("len");
                string sliced=b.slice(0,5);
                return b.length+t.length+code+found+sliced.length;
            }
            print(strings(17));
            "#,
            r#"
            const events=[];const output=console.log.bind(console);
            console.log=value=>output(JSON.stringify([...events,value]));
            String.prototype.charCodeAt=function(){events.push('code');return {valueOf(){events.push('code-coerce');return 1}}};
            String.prototype.indexOf=function(){events.push('find');return 2};
            String.prototype.slice=function(){events.push('slice');return {get length(){events.push('length');return 3}}};
            "#,
            "[\"code\",\"code-coerce\",\"find\",\"slice\",\"length\",28]\n",
            &["a"],
        ),
        (
            r#"
            extern void units(string value);
            int amount=7;
            units(`\ud800${amount}\udfff`);
            units(`\u0024{notAnIdentifier}${amount}`);
            units(`a${amount}`);
            "#,
            "globalThis.units=value=>console.log(Array.from({length:value.length},(_,i)=>value.charCodeAt(i)).join(','));",
            "55296,55,57343\n36,123,110,111,116,65,110,73,100,101,110,116,105,102,105,101,114,125,55\n97,55\n",
            &["amount"],
        ),
        (
            r#"
            extern int take();
            int source=take();
            int unused(){return source;}
            if(false){print(unused());}
            int move(int seed){int first=seed+1;int second=first+2;return second+3;}
            int outer(){int captured=take();func()->int forgotten=()=>captured;return 2;}
            int nested(){int n=7;string text=`${n}`;int size=text.length;return size+size;}
            print(move(8));print(outer());print(nested());
            "#,
            "globalThis.take=()=>{console.log('take');return 5};",
            "take\n14\ntake\n2\n2\n",
            &[
                "source",
                "unused",
                "first",
                "second",
                "captured",
                "forgotten",
                "n",
                "text",
                "size",
            ],
        ),
        (
            r#"
            extern int take();
            int abandoned=0;int observed=0;
            observed=(abandoned=take());
            int[] lengths=[abandoned];
            print(lengths.length);print(observed);
            "#,
            "globalThis.take=()=>{console.log('take');return 5};",
            "take\n1\n5\n",
            &["abandoned", "lengths"],
        ),
    ];
    for &(source, setup, expected, removed) in cases {
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&program).unwrap();
        for mode in [analysis::Mode::Regions, analysis::Mode::Values] {
            let mut tree = lower::lower_slice(&program, &semantics).unwrap();
            optimize::optimize_in_execution(
                &mut tree,
                mode,
                JavaScriptWorld::ClosedApplication,
                crate::compilation_contract::JavaScriptExecution::Module,
            )
            .unwrap();
            for binding in &tree.target().bindings {
                assert!(
                    !removed.contains(&binding.spelling.as_str()),
                    "{mode:?}: {} survived one planned transaction\n{source}",
                    binding.spelling
                );
            }
            let javascript = tree
                .render(PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap();
            let result = Command::new("node")
                .args([
                    "--input-type=module",
                    "-e",
                    &format!("{setup}\n{javascript}"),
                ])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{javascript}\n{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap(),
                expected,
                "{mode:?}\n{javascript}"
            );
        }
    }
}

#[test]
fn planned_dead_reads_do_not_authorize_uninitialized_stores() {
    use crate::compilation_contract::JavaScriptWorld;
    let arena = bumpalo::Bump::new();
    let program = crate::parse_source(&arena, "").unwrap();
    let semantics = crate::analyze(&program).unwrap();
    for mode in [
        analysis::Mode::Indexed,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut tree = lower::lower_slice(&program, &semantics).unwrap();
        let module = tree.target_mut();
        let cell = binding(module, RegionId::new(0), 1000, "uninitialized");
        let array = binding(module, RegionId::new(0), 1001, "lengthOnly");
        let target = expr(module, Expr::Binding(cell));
        let take = host(module, "take");
        let value = call(module, take, vec![], Invocation::Value);
        let assign = expr(module, Expr::Assign { target, value });
        module.regions[0]
            .statements
            .push(Statement::Evaluate(assign));
        let initial = number(module, 1.0);
        module.regions[0].statements.push(Statement::Let {
            binding: cell,
            value: Some(initial),
        });
        let read = expr(module, Expr::Binding(cell));
        let initial = expr(module, Expr::Array(vec![read]));
        module.regions[0].statements.push(Statement::Let {
            binding: array,
            value: Some(initial),
        });
        let object = expr(module, Expr::Binding(array));
        let length = expr(
            module,
            Expr::Member {
                object,
                property: Property::Named("length".into()),
            },
        );
        capture(module, length);
        let outcome =
            optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication).unwrap();
        assert_eq!(outcome.report.removed_allocations, 1);
        assert_eq!(outcome.report.removed_stores, 0);
        assert!(tree
            .target()
            .bindings
            .iter()
            .any(|binding| binding.spelling == "uninitialized"));
        let javascript = tree
            .render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap();
        let script = format!(
            "const trace=[];function take(){{trace.push('rhs');return 7}}function capture(value){{trace.push(value)}}try{{{javascript}}}catch(error){{trace.push(error.name)}}console.log(JSON.stringify(trace))"
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap().trim(),
            "[\"rhs\",\"ReferenceError\"]"
        );
    }
}

#[test]
fn later_storage_decisions_follow_transferred_statement_owners_and_depth() {
    use crate::compilation_contract::JavaScriptWorld;
    for (initial_depth, returned_depth, retained, expected) in
        [(1, 3, false, "12\n"), (200, 350, true, "558\n")]
    {
        let source = format!(
            "int work(int seed){{int a=seed{};int b=a+1;func()->int abandoned=()=>a;return b{};}}print(work(7));",
            "+1".repeat(initial_depth),
            "+1".repeat(returned_depth)
        );
        let arena = bumpalo::Bump::new();
        let program = crate::parse_source(&arena, &source).unwrap();
        let semantics = crate::analyze(&program).unwrap();
        for mode in [
            analysis::Mode::Indexed,
            analysis::Mode::Regions,
            analysis::Mode::Values,
        ] {
            let mut tree = lower::lower_slice(&program, &semantics).unwrap();
            let outcome = optimize::optimize_in_execution(
                &mut tree,
                mode,
                JavaScriptWorld::ClosedApplication,
                crate::compilation_contract::JavaScriptExecution::Module,
            )
            .unwrap();
            let exists = |name: &str| {
                tree.target()
                    .bindings
                    .iter()
                    .any(|binding| binding.spelling == name)
            };
            assert!(!exists("b") && !exists("abandoned"));
            assert_eq!(
                exists("a"),
                retained,
                "{mode:?} at depth {initial_depth}/{returned_depth}"
            );
            assert_eq!(outcome.report.retained_for_depth > 0, retained);
            let code = tree
                .render(PrintPolicy {
                    mangle_bindings: true,
                })
                .unwrap();
            let run = Command::new("node")
                .args(["--input-type=module", "-e", &code])
                .output()
                .unwrap();
            assert!(
                run.status.success(),
                "{code}\n{}",
                String::from_utf8_lossy(&run.stderr)
            );
            assert_eq!(
                String::from_utf8(run.stdout).unwrap(),
                expected,
                "{mode:?}\n{code}"
            );
        }
    }
}

#[test]
fn explicit_exports_survive_cleanup_compaction_and_name_extraction() {
    use crate::compilation_contract::JavaScriptWorld;
    let arena = bumpalo::Bump::new();
    let source = crate::parse_source(
        &arena,
        "int unused=0;int value=1;int advance(){value+=1;return value;}print(advance());",
    )
    .unwrap();
    let semantics = crate::analyze(&source).unwrap();
    for mode in [analysis::Mode::Tree, analysis::Mode::Values] {
        let mut tree = lower::lower_slice(&source, &semantics).unwrap();
        tree.edit(|module| {
            module.exports.push(Export {
                binding: named_binding(module, "value"),
                name: "count".into(),
            });
            module.exports.push(Export {
                binding: named_binding(module, "advance"),
                name: "run".into(),
            });
        })
        .unwrap();
        optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication).unwrap();
        assert!(!tree
            .target()
            .bindings
            .iter()
            .any(|binding| binding.spelling == "unused"));
        assert_eq!(tree.target().exports.len(), 2);
        let view = extract::JavaScriptView::prepare_in_world(
            &tree,
            mode,
            JavaScriptWorld::ClosedApplication,
        );
        assert_eq!(view.omitted_function_names, 0);
        let javascript = view
            .render(PrintPolicy {
                mangle_bindings: true,
            })
            .unwrap();
        let script = format!(
            "const lib=await import('data:text/javascript,'+encodeURIComponent({}));console.log(lib.count,lib.run(),lib.count,lib.run.name);",
            serde_json::to_string(&javascript).unwrap()
        );
        let result = Command::new("node")
            .args(["--input-type=module", "-e", &script])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}\n{javascript}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(
            String::from_utf8(result.stdout).unwrap(),
            "2\n2 3 3 advance\n"
        );
    }
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
fn private_parameter_domains_require_complete_actual_argument_evidence() {
    use crate::compilation_contract::JavaScriptWorld;
    let cases = [
        (
            "int arithmetic(int value){return value+1;}print(arithmetic(3));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Number,
            true,
        ),
        (
            "int arithmetic(int value){return value+1;}int actual=3;print(arithmetic(actual));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Number,
            true,
        ),
        (
            "int arithmetic(int value){return value+1;}print(arithmetic(3));",
            JavaScriptWorld::ReusableLibrary,
            analysis::ValueKind::Unknown,
            false,
        ),
        (
            "extern int opaque();int arithmetic(int value){return value+1;}print(arithmetic(3));print(arithmetic(opaque()));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Unknown,
            false,
        ),
        (
            "extern void retain(func(int)->int callback);int arithmetic(int value){return value+1;}retain(arithmetic);print(arithmetic(3));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Unknown,
            false,
        ),
        (
            "extern int opaque();int arithmetic(int value){auto mutate=()=>{value=opaque();};mutate();return value+1;}print(arithmetic(3));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Unknown,
            false,
        ),
        (
            "string arithmetic(string value){return value+\"!\";}print(arithmetic(\"x\".slice(0)));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Unknown,
            false,
        ),
        (
            "float arithmetic(float value){return value+1.0;}print(arithmetic(-0.0));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Number,
            false,
        ),
        (
            "float arithmetic(float value){return value+1.0;}print(arithmetic(1.0));print(arithmetic(4294967296.0));",
            JavaScriptWorld::ClosedApplication,
            analysis::ValueKind::Number,
            false,
        ),
    ];
    for (source, world, expected, signed_i32) in cases {
        let arena = bumpalo::Bump::new();
        let source_program = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&source_program).unwrap();
        let tree = lower::lower_slice(&source_program, &semantics).unwrap();
        let parameter = named_binding(tree.target(), "value");
        for mode in [
            analysis::Mode::Tree,
            analysis::Mode::Indexed,
            analysis::Mode::Memoized,
            analysis::Mode::Regions,
            analysis::Mode::Values,
        ] {
            let mut facts = analysis::Analysis::new_in_execution(
                &tree,
                mode,
                world,
                crate::compilation_contract::JavaScriptExecution::Module,
            );
            assert_eq!(
                facts.binding_uses(parameter).entry_value,
                expected,
                "{mode:?}: {source}"
            );
            assert_eq!(
                facts.binding_uses(parameter).entry_i32,
                signed_i32,
                "{mode:?}: {source}"
            );
            let work = facts.work().parameters;
            assert!(work.temporary_bytes > 0);
            assert_eq!(work.expressions, tree.target().expressions.len());
            if expected == analysis::ValueKind::Number {
                assert!(work.argument_queries > 0 && work.argument_slots >= work.argument_queries);
                assert!(work.memoized_bytes > 0);
            }
            if world == JavaScriptWorld::ReusableLibrary {
                assert_eq!(work.argument_queries, 0);
                assert_eq!(
                    work.memoized_bytes, 0,
                    "no incoming proof allocated query memo storage"
                );
            }
            if expected == analysis::ValueKind::Number {
                for (index, expression) in tree.target().expressions.iter().enumerate() {
                    if matches!(expression, Expr::Binding(binding) if *binding == parameter) {
                        let value = facts.facts(ExprId::new(index));
                        assert_eq!(value.value, expected);
                        assert_eq!(
                            value.integer,
                            signed_i32.then_some(analysis::IntegerRange {
                                minimum: i32::MIN as i64,
                                maximum: i32::MAX as i64
                            }),
                            "actual signed arguments establish a coarse range, not literal specialization"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn opaque_private_arguments_keep_coercion_and_throws_after_optimization() {
    use crate::compilation_contract::JavaScriptWorld;
    let source = "extern int opaque();int arithmetic(int value){int discarded=value+1;return 7;}print(arithmetic(opaque()));";
    let arena = bumpalo::Bump::new();
    let source_program = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&source_program).unwrap();
    for mode in [
        analysis::Mode::Tree,
        analysis::Mode::Indexed,
        analysis::Mode::Memoized,
        analysis::Mode::Regions,
        analysis::Mode::Values,
    ] {
        let mut tree = lower::lower_slice(&source_program, &semantics).unwrap();
        optimize::optimize(&mut tree, mode, JavaScriptWorld::ClosedApplication).unwrap();
        let javascript = tree.render(PrintPolicy::default()).unwrap();
        for (host, expected) in [
            (
                "({valueOf(){events.push('coerce');throw Error('opaque')}})",
                "[\"coerce\",\"Error\"]",
            ),
            ("Symbol('opaque')", "[\"TypeError\"]"),
            ("1n", "[\"TypeError\"]"),
        ] {
            let script = format!(
                "const events=[];globalThis.opaque=()=>({host});try{{{javascript}}}catch(error){{events.push(error.name)}}console.log(JSON.stringify(events));"
            );
            let result = Command::new("node")
                .args(["--input-type=module", "-e", &script])
                .output()
                .unwrap();
            assert!(
                result.status.success(),
                "{mode:?}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(
                String::from_utf8(result.stdout).unwrap().trim(),
                expected,
                "{mode:?}: {javascript}"
            );
        }
    }
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
    let object = expr(&mut module, Expr::Object(vec![(Property::Computed(key), two)]));
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
        module.regions[block.index()].statements.push(Statement::Let {
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
        let update = expr(&mut module, Expr::Assign { target, value: next });
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
                suspension: crate::structured_js::Suspension::None,
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
        module.regions[block.index()].statements.push(Statement::Loop {
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
    assert_eq!(build(false, false).render(policy).unwrap(), "flag&&capture(7);");
    assert_eq!(build(true, false).render(policy).unwrap(), "flag||capture(7);");
    assert_eq!(build(false, true).render(policy).unwrap(), "if(flag)slot=1;");
    assert_eq!(execute(&build(true, false), "globalThis.flag=false;", policy), "[7]");
    assert_eq!(execute(&build(false, false), "globalThis.flag=false;", policy), "[]");
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
        Statement::Let { binding: f, value: Some(created) },
        Statement::Let { binding: x, value: Some(called) },
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
        Statement::If { condition: test, yes, no: Some(no) },
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
        Statement::Let { binding: f, value: Some(created) },
        Statement::Let { binding: result, value: Some(called) },
        Statement::Evaluate(reported),
    ];
    module.root_modules = vec![0; 3];
    module.verify().unwrap();
    let before = [
        execute(&module, "const flag=0;", PrintPolicy { mangle_bindings: false }),
        execute(&module, "const flag=1;", PrintPolicy { mangle_bindings: false }),
    ];
    // The early form, `if(p)return 1;…`, would need a loop to leave.
    let mut early = module.clone();
    early.regions[body.index()].statements.swap(0, 1);
    let mut budget = AllocationBudget::new(None);
    assert_eq!(early.inline_single_calls(true, &mut budget).unwrap(), 0);
    assert_eq!(module.inline_single_calls(true, &mut budget).unwrap(), 1);
    module.verify().unwrap();
    let javascript = module.render(PrintPolicy { mangle_bindings: false }).unwrap();
    assert!(!javascript.contains("for(;;)") && !javascript.contains("=>"), "{javascript}");
    assert_eq!(before[0], "[0,2]");
    assert_eq!(before[1], "[1,1]");
    assert_eq!(execute(&module, "const flag=0;", PrintPolicy { mangle_bindings: false }), before[0]);
    assert_eq!(execute(&module, "const flag=1;", PrintPolicy { mangle_bindings: false }), before[1]);
}
