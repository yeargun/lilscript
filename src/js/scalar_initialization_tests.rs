use super::*;

#[derive(Clone, Copy, Debug)]
enum Initializer {
    Inert,
    Effect,
    SelfRead,
    Parameter,
    Local,
    Uninitialized,
}

fn fixture(captured: bool, initializer: Initializer) -> Module {
    let mut module = Module::default();
    let root = module.root;
    let body = module.region(module.regions[root.index()].scope);
    let scope = module.regions[body.index()].scope;
    let state = binding(&mut module, body, 0, "state");
    let other = binding(&mut module, body, 1, "other");
    if matches!(initializer, Initializer::Local) {
        let value = number(&mut module, 2.0);
        module.regions[body.index()]
            .statements
            .push(Statement::Let {
                binding: other,
                value: Some(value),
            });
    }
    if captured {
        let getter = module.region(scope);
        let read = expr(&mut module, Expr::Binding(state));
        let field = expr(
            &mut module,
            Expr::Member {
                object: read,
                property: Property::Named("early".into()),
            },
        );
        module.regions[getter.index()].statements = vec![Statement::Return(Some(field))];
        let function = FunctionId::new(module.functions.len());
        module.functions.push(Function {
            rest: false,
            parameters: vec![],
            body: getter,
            arrow: true,
            name: FunctionName::Unobserved,
            strict: false,
            length: None,
            suspension: Suspension::None,
        });
        let created = expr(&mut module, Expr::Function(function));
        let remember = host(&mut module, "remember");
        let remember = call(&mut module, remember, vec![created], Invocation::Value);
        module.regions[body.index()]
            .statements
            .push(Statement::Evaluate(remember));
    }
    let early = number(&mut module, 1.0);
    let late = match initializer {
        Initializer::Inert => number(&mut module, 2.0),
        Initializer::Effect => {
            let effect = host(&mut module, "initialize");
            call(&mut module, effect, vec![], Invocation::Value)
        }
        Initializer::SelfRead => {
            let read = expr(&mut module, Expr::Binding(state));
            expr(
                &mut module,
                Expr::Member {
                    object: read,
                    property: Property::Named("early".into()),
                },
            )
        }
        Initializer::Parameter | Initializer::Uninitialized => {
            expr(&mut module, Expr::Binding(other))
        }
        Initializer::Local => {
            let read = expr(&mut module, Expr::Binding(other));
            let array = expr(&mut module, Expr::Array(vec![read]));
            expr(
                &mut module,
                Expr::Object(vec![(Property::Named("nested".into()), array)]),
            )
        }
    };
    let object = expr(
        &mut module,
        Expr::Object(vec![
            (Property::Named("early".into()), early),
            (Property::Named("late".into()), late),
        ]),
    );
    module.regions[body.index()]
        .statements
        .push(Statement::Let {
            binding: state,
            value: Some(object),
        });
    if matches!(initializer, Initializer::Uninitialized) {
        let value = number(&mut module, 2.0);
        module.regions[body.index()]
            .statements
            .push(Statement::Let {
                binding: other,
                value: Some(value),
            });
    }
    let read = expr(&mut module, Expr::Binding(state));
    let field = expr(
        &mut module,
        Expr::Member {
            object: read,
            property: Property::Named("early".into()),
        },
    );
    module.regions[body.index()]
        .statements
        .push(Statement::Return(Some(field)));
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        rest: false,
        parameters: if matches!(initializer, Initializer::Parameter) {
            vec![other]
        } else {
            vec![]
        },
        body,
        arrow: true,
        name: FunctionName::Unobserved,
        strict: false,
        length: None,
        suspension: Suspension::None,
    });
    let function = expr(&mut module, Expr::Function(function));
    let arguments = if matches!(initializer, Initializer::Parameter) {
        vec![number(&mut module, 2.0)]
    } else {
        vec![]
    };
    let result = call(&mut module, function, arguments, Invocation::Value);
    capture(&mut module, result);
    module.root_rows = vec![RootRow::new(0, Anchor::Anchored); 1];
    module.verify().unwrap();
    module
}

fn observe(module: &Module, during: &str) -> String {
    let code = module.render(PrintPolicy::default()).unwrap();
    let output = Command::new("node").args(["-e", &format!(
        "const vm=require('node:vm'),events=[];let reader;const read=()=>{{try{{events.push(reader())}}catch(error){{events.push(error.name)}}}};try{{vm.runInNewContext({},{{capture:value=>events.push(value),remember:f=>reader=f,initialize:()=>{{{};return 2}}}})}}catch(error){{events.push(error.name)}}if(reader)read();console.log(JSON.stringify(events));",
        serde_json::to_string(&code).unwrap(), during,
    )]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn scalar_objects_preserve_reentry_and_failed_initialization() {
    for (during, oracle) in [
        ("read()", r#"["ReferenceError",1,1]"#),
        (
            "throw Error('initializer')",
            r#"["Error","ReferenceError"]"#,
        ),
    ] {
        let mut module = fixture(true, Initializer::Effect);
        assert_eq!(observe(&module, during), oracle);
        let changed = module
            .scalarize_member_objects(&mut AllocationBudget::new(None))
            .unwrap();
        module.verify().unwrap();
        assert_eq!(
            observe(&module, during),
            oracle,
            "changed={changed}, during={during}"
        );
        assert_eq!(changed, 0);
    }
}

#[test]
fn scalar_objects_preserve_an_initializer_self_read() {
    let mut module = fixture(false, Initializer::SelfRead);
    assert_eq!(observe(&module, ""), r#"["ReferenceError"]"#);
    let changed = module
        .scalarize_member_objects(&mut AllocationBudget::new(None))
        .unwrap();
    module.verify().unwrap();
    assert_eq!(observe(&module, ""), r#"["ReferenceError"]"#);
    assert_eq!(changed, 0);
}

#[test]
fn scalar_objects_preserve_a_captured_initializer_tdz_from_another_binding() {
    let mut module = fixture(true, Initializer::Uninitialized);
    let oracle = r#"["ReferenceError","ReferenceError"]"#;
    assert_eq!(observe(&module, ""), oracle);
    assert_eq!(
        module
            .scalarize_member_objects(&mut AllocationBudget::new(None))
            .unwrap(),
        0
    );
    module.verify().unwrap();
    assert_eq!(observe(&module, ""), oracle);
}

#[test]
fn scalar_objects_keep_safe_captures_and_uncaptured_effects_eligible() {
    for (captured, initializer, during, oracle) in [
        (true, Initializer::Inert, "", "[1,1]"),
        (true, Initializer::Parameter, "", "[1,1]"),
        (true, Initializer::Local, "", "[1,1]"),
        (
            false,
            Initializer::Effect,
            "events.push('effect')",
            "[\"effect\",1]",
        ),
    ] {
        let mut module = fixture(captured, initializer);
        assert_eq!(observe(&module, during), oracle);
        assert_eq!(
            module
                .scalarize_member_objects(&mut AllocationBudget::new(None))
                .unwrap(),
            1
        );
        module.verify().unwrap();
        assert_eq!(observe(&module, during), oracle);
    }
}
