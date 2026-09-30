//! The printed tree against its own printed text: admitted; against a text
//! printed from another structure, or damaged the way a printer bug would
//! damage it, refused.
use super::*;
use crate::admission_parse::admit;

fn expr(module: &mut Module, node: Expr) -> ExprId {
    module.expression(node, None)
}

fn host(module: &mut Module, name: &str) -> ExprId {
    expr(module, Expr::Host(name.into()))
}

fn number(module: &mut Module, value: f64) -> ExprId {
    expr(module, Expr::Literal(Literal::Number(value)))
}

fn call(module: &mut Module, name: &str, arguments: Vec<ExprId>) -> ExprId {
    let callee = host(module, name);
    expr(
        module,
        Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        },
    )
}

fn evaluate(module: &mut Module, region: RegionId, name: &str) {
    let value = call(module, name, vec![]);
    module.regions[region.index()]
        .statements
        .push(Statement::Evaluate(value));
}

/// `f(); if (c) { g(); h(); } k((1 + 2) * 3, !0, void 0);` with the `if` body
/// holding `split` statements, the rest of them after the `if`.
fn program(split: usize) -> Module {
    let mut module = Module::default();
    let root = module.root;
    evaluate(&mut module, root, "f");
    let scope = module.regions[root.index()].scope;
    let yes = module.region(scope);
    let condition = host(&mut module, "c");
    module.regions[root.index()].statements.push(Statement::If {
        condition,
        yes,
        no: None,
    });
    let mut after = Vec::new();
    for (index, name) in ["g", "h"].into_iter().enumerate() {
        let value = call(&mut module, name, vec![]);
        if index < split {
            module.regions[yes.index()]
                .statements
                .push(Statement::Evaluate(value));
        } else {
            after.push(Statement::Evaluate(value));
        }
    }
    module.regions[root.index()].statements.extend(after);
    let one = number(&mut module, 1.0);
    let two = number(&mut module, 2.0);
    let three = number(&mut module, 3.0);
    let sum = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Add,
            left: one,
            right: two,
        },
    );
    let product = expr(
        &mut module,
        Expr::Binary {
            op: Binary::Multiply,
            left: sum,
            right: three,
        },
    );
    let truth = expr(&mut module, Expr::Literal(Literal::Bool(true)));
    let nothing = expr(&mut module, Expr::Literal(Literal::Undefined));
    let value = call(&mut module, "k", vec![product, truth, nothing]);
    module.regions[root.index()]
        .statements
        .push(Statement::Evaluate(value));
    module
}

fn printed(module: &Module) -> String {
    module.render(PrintPolicy::default()).unwrap()
}

#[test]
fn a_printed_tree_is_admitted() {
    let module = program(2);
    let text = printed(&module);
    assert!(text.contains("(1+2)*3"), "{text}");
    admit(
        &digest(&module, None, crate::config::JavaScriptFormat::Bare),
        &text,
        false,
    )
    .unwrap();
    admit(
        &digest(&module, None, crate::config::JavaScriptFormat::Bare),
        &text,
        true,
    )
    .unwrap();
}

#[test]
fn private_frame_is_part_of_independently_admitted_structure() {
    let module = program(2);
    let bare = printed(&module);
    let wrapped = format!("(()=>{{{bare}}})();");
    let expected = digest(&module, None, crate::config::JavaScriptFormat::Iife);
    admit(&expected, &wrapped, false).unwrap();
    assert!(admit(&expected, &bare, false).is_err());
    // Replacing the arrow changes lexical this/arguments semantics.
    assert!(admit(&expected, &wrapped.replacen("()=>", "function()", 1), false).is_err());
    assert!(admit(
        &expected,
        &wrapped.replacen("{", "{'use strict';", 1),
        false
    )
    .is_err());
}

#[test]
fn a_misprinted_tree_is_refused() {
    let module = program(2);
    let expected = digest(&module, None, crate::config::JavaScriptFormat::Bare);
    // Another structure printed: the second statement left the `if`.
    let moved = printed(&program(1));
    let refusal = admit(&expected, &moved, false).unwrap_err();
    assert!(refusal.0.contains("top-level statements"), "{}", refusal.0);
    // The braces a two-statement body needs, dropped: the same failure.
    let text = printed(&module);
    let braceless = text.replacen("{", "", 1).replacen("}", "", 1);
    assert_ne!(braceless, text);
    assert!(admit(&expected, &braceless, false).is_err());
    // Grouping parentheses dropped: `1+2*3` is another expression.
    let ungrouped = text.replace("(1+2)*3", "1+2*3");
    let refusal = admit(&expected, &ungrouped, false).unwrap_err();
    assert!(refusal.0.contains("different structure"), "{}", refusal.0);
    // A separator dropped: the text does not parse.
    let refusal = admit(&expected, &text.replacen(";", " ", 1), false).unwrap_err();
    assert!(refusal.0.contains("does not parse"), "{}", refusal.0);
}

#[test]
fn every_module_the_printer_spells_differently_is_one_structure() {
    // The raw plan's spellings (compound assignment, conditional statements)
    // and the default ones read back as the tree.
    let module = program(2);
    let text = module
        .render(PrintPolicy {
            mangle_bindings: true,
        })
        .unwrap();
    admit(
        &digest(&module, None, crate::config::JavaScriptFormat::Bare),
        &text,
        false,
    )
    .unwrap();
}
