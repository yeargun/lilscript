use super::*;

fn array(module: &mut Module, value: f64) -> ExprId {
    let value = number(module, value);
    expr(module, Expr::Array(vec![value]))
}

fn field(module: &mut Module, binding: BindingId) -> ExprId {
    let object = expr(module, Expr::Binding(binding));
    let key = number(module, 0.0);
    expr(module, Expr::Member { object, property: Property::Computed(key) })
}

// A repeated parameter read after a property access needs stable arguments;
// merely preserving the order of their first reads is insufficient.
fn fixture(kind: usize) -> Module {
    let mut m = Module::default();
    let root = m.root;
    let scope = m.regions[root.index()].scope;
    let function_binding = binding(&mut m, root, 0, "combine");
    let body = m.region(scope);
    let a = binding(&mut m, body, 1, "a");
    let b = binding(&mut m, body, 2, "b");
    let first = field(&mut m, a);
    let second = field(&mut m, b);
    let sum = expr(&mut m, Expr::Binary { op: Binary::Add, left: first, right: second });
    let again = field(&mut m, a);
    let value = expr(&mut m, Expr::Binary { op: Binary::Add, left: sum, right: again });
    m.regions[body.index()].statements.push(Statement::Return(Some(value)));
    let function = FunctionId::new(m.functions.len());
    m.functions.push(Function {
        parameters: vec![a, b], rest: false, body, arrow: true,
        name: FunctionName::Unobserved, strict: false, length: None,
        suspension: Suspension::None,
    });
    let created = expr(&mut m, Expr::Function(function));
    m.regions[root.index()].statements.push(Statement::Let { binding: function_binding, value: Some(created) });

    let outer = m.region(scope);
    let inner = if kind == 0 { m.region(m.regions[outer.index()].scope) } else { outer };
    let x = binding(&mut m, outer, 3, "x");
    let y = binding(&mut m, outer, 4, "y");
    let value = if kind == 1 {
        let item = binding(&mut m, outer, 5, "item");
        let one = number(&mut m, 1.0);
        let two = number(&mut m, 2.0);
        let iterable = expr(&mut m, Expr::Array(vec![one, two]));
        m.regions[root.index()].statements.push(Statement::ForOf { binding: item, iterable, body: outer });
        let item = expr(&mut m, Expr::Binding(item));
        expr(&mut m, Expr::Array(vec![item]))
    } else {
        m.regions[root.index()].statements.push(Statement::Block(outer));
        array(&mut m, 1.0)
    };
    if kind != 2 {
        m.regions[outer.index()].statements.push(Statement::Let { binding: x, value: Some(value) });
    }
    let value = array(&mut m, 7.0);
    m.regions[outer.index()].statements.push(Statement::Let { binding: y, value: Some(value) });
    if kind == 0 { m.regions[outer.index()].statements.push(Statement::Block(inner)); }

    let first = expr(&mut m, Expr::Binding(x));
    let second = match kind {
        3 => {
            let target = expr(&mut m, Expr::Binding(x));
            let value = array(&mut m, 9.0);
            expr(&mut m, Expr::Assign { target, value })
        }
        4 => {
            let mutate = binding(&mut m, outer, 6, "mutate");
            let body = m.region(m.regions[outer.index()].scope);
            let target = expr(&mut m, Expr::Binding(x));
            let value = array(&mut m, 9.0);
            let assigned = expr(&mut m, Expr::Assign { target, value });
            let returned = expr(&mut m, Expr::Binding(x));
            m.regions[body.index()].statements = vec![Statement::Evaluate(assigned), Statement::Return(Some(returned))];
            let function = FunctionId::new(m.functions.len());
            m.functions.push(Function {
                parameters: vec![], rest: false, body, arrow: true,
                name: FunctionName::Unobserved, strict: false, length: None,
                suspension: Suspension::None,
            });
            let created = expr(&mut m, Expr::Function(function));
            m.regions[outer.index()].statements.push(Statement::Let { binding: mutate, value: Some(created) });
            let callee = expr(&mut m, Expr::Binding(mutate));
            call(&mut m, callee, vec![], Invocation::Value)
        }
        _ => expr(&mut m, Expr::Binding(y)),
    };
    let callee = expr(&mut m, Expr::Binding(function_binding));
    let value = call(&mut m, callee, vec![first, second], Invocation::Value);
    if kind == 2 {
        m.regions[inner.index()].statements.push(Statement::Let { binding: x, value: Some(value) });
    } else {
        let output = host(&mut m, "capture");
        let observed = call(&mut m, output, vec![value], Invocation::Value);
        m.regions[inner.index()].statements.push(Statement::Evaluate(observed));
    }
    m.root_rows = vec![RootRow::new(0, Anchor::Anchored); m.regions[root.index()].statements.len()];
    m.verify().unwrap();
    m
}

fn observe(module: &Module) -> String {
    let code = module.render(PrintPolicy::default()).unwrap();
    let result = Command::new("node").args(["-e", &format!(
        "const seen=[];function capture(value){{seen.push(value)}}try{{{code}}}catch(error){{seen.push(error.name)}}console.log(JSON.stringify(seen));"
    )]).output().unwrap();
    assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
    String::from_utf8(result.stdout).unwrap().trim().to_owned()
}

#[test]
fn expression_inlining_uses_local_initialization_without_moving_tdz_or_mutation() {
    for (kind, expected, changed) in [(0, "[9]", 1), (1, "[9,11]", 1), (2, "[\"ReferenceError\"]", 0), (3, "[11]", 0), (4, "[11]", 0)] {
        let mut module = fixture(kind);
        assert_eq!(observe(&module), expected, "original {kind}");
        let mut budget = AllocationBudget::new(None);
        assert_eq!(module.inline_expression_functions(64, true, false, &mut budget).unwrap(), changed, "kind {kind}");
        assert_eq!(budget.retained_bytes(AllocationClass::Scratch), 0);
        module.verify().unwrap();
        assert_eq!(observe(&module), expected, "inlined {kind}");
    }
}
