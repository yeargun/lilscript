use super::*;

#[derive(Clone, Copy, Debug)]
enum Kind {
    Expression,
    Statements,
    Block,
}

fn fixture(kind: Kind, nested: bool, explicit: bool) -> Module {
    let mut module = Module::default();
    let root = module.root;
    let scope = module.regions[root.index()].scope;
    let site = if nested { module.region(scope) } else { root };
    let declaration = if explicit { root } else { site };
    let f = binding(&mut module, declaration, 0, "inner");
    let body = module.region(module.regions[declaration.index()].scope);
    let output = host(&mut module, "rhs");
    let value = call(&mut module, output, vec![], Invocation::Value);
    let mut parameters = Vec::new();
    let locked = if !matches!(kind, Kind::Statements) {
        let parameter = binding(&mut module, body, 1, "input");
        parameters.push(parameter);
        expr(&mut module, Expr::Binding(parameter))
    } else {
        host(&mut module, "locked")
    };
    let target = expr(
        &mut module,
        Expr::Member {
            object: locked,
            property: Property::Named("field".into()),
        },
    );
    let assigned = expr(&mut module, Expr::Assign { target, value });
    module.regions[body.index()].statements = vec![match kind {
        Kind::Statements => Statement::Evaluate(assigned),
        _ => Statement::Return(Some(assigned)),
    }];
    let function = FunctionId::new(module.functions.len());
    module.functions.push(Function {
        parameters,
        body,
        arrow: true,
        name: FunctionName::Unobserved,
        strict: explicit,
        length: None,
        suspension: Suspension::None,
    });
    let created = expr(&mut module, Expr::Function(function));
    module.regions[declaration.index()]
        .statements
        .push(Statement::Let {
            binding: f,
            value: Some(created),
        });
    let callee = expr(&mut module, Expr::Binding(f));
    let arguments = if matches!(kind, Kind::Statements) {
        vec![]
    } else {
        let argument = host(&mut module, "argument");
        vec![call(&mut module, argument, vec![], Invocation::Value)]
    };
    let called = call(&mut module, callee, arguments, Invocation::Value);
    module.regions[site.index()]
        .statements
        .push(Statement::Evaluate(called));
    if nested {
        let outer = binding(&mut module, root, 2, "outer");
        let function = FunctionId::new(module.functions.len());
        module.functions.push(Function {
            parameters: vec![],
            body: site,
            arrow: true,
            name: FunctionName::Unobserved,
            strict: true,
            length: None,
            suspension: Suspension::None,
        });
        let created = expr(&mut module, Expr::Function(function));
        module.regions[root.index()]
            .statements
            .push(Statement::Let {
                binding: outer,
                value: Some(created),
            });
        let outer = expr(&mut module, Expr::Binding(outer));
        let called = call(&mut module, outer, vec![], Invocation::Value);
        module.regions[root.index()]
            .statements
            .push(Statement::Evaluate(called));
    }
    module.root_rows =
        vec![RootRow::new(0, Anchor::Anchored); module.regions[root.index()].statements.len()];
    module.verify().unwrap();
    module
}

fn observe(module: &Module, strict: bool) -> String {
    let code = format!(
        "{}{}",
        if strict { "'use strict';" } else { "" },
        module.render(PrintPolicy::default()).unwrap()
    );
    let output = Command::new("node").args(["-e", &format!(
        "const vm=require('node:vm'),events=[],locked=Object.freeze({{}});try{{vm.runInNewContext({},{{locked,argument(){{events.push('argument');return locked}},rhs(){{events.push('rhs');return 7}}}})}}catch(error){{events.push(error.name)}}console.log(JSON.stringify(events));",
        serde_json::to_string(&code).unwrap(),
    )]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

#[test]
fn strict_target_inliners_preserve_known_execution_modes_and_effect_order() {
    for kind in [Kind::Expression, Kind::Statements, Kind::Block] {
        for (global, nested, explicit, expected) in [
            (false, false, true, 0),
            (true, false, true, 1),
            (false, true, true, 1),
            (false, true, false, 1),
        ] {
            let mut module = fixture(kind, nested, explicit);
            let oracle = if matches!(kind, Kind::Statements) {
                r#"["rhs","TypeError"]"#
            } else {
                r#"["argument","rhs","TypeError"]"#
            };
            assert_eq!(observe(&module, global), oracle);
            let mut budget = AllocationBudget::new(None);
            let changed = match kind {
                Kind::Expression => {
                    module
                        .inline_expression_functions(64, true, global, &mut budget)
                        .unwrap()
                        .0
                }
                Kind::Statements => {
                    module
                        .inline_statement_functions(true, global, &mut budget)
                        .unwrap()
                        .0
                }
                Kind::Block => module
                    .inline_single_calls(true, global, &mut budget)
                    .unwrap(),
            };
            assert_eq!(
                changed, expected,
                "{kind:?} global={global} nested={nested} explicit={explicit}"
            );
            module.verify().unwrap();
            assert_eq!(
                observe(&module, global),
                oracle,
                "{kind:?} global={global} nested={nested}"
            );
        }
    }
}
