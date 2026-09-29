//! The edit journal (M5.2): helpers record what they edit, a direct write is
//! caught by the check, a write that changes nothing records nothing, and
//! renumbering carries the entries.
use super::*;

/// `print(1); print(2);` at the root: the tree and its two argument nodes.
fn two_prints() -> (Module, [ExprId; 2]) {
    let mut module = Module::default();
    let mut arguments = [ExprId::new(0); 2];
    for (slot, value) in arguments.iter_mut().zip([1.0, 2.0]) {
        let argument = module.expression(Expr::Literal(Literal::Number(value)), None);
        let callee = module.expression(Expr::Host("print".into()), None);
        let call = module.expression(
            Expr::Call {
                callee,
                arguments: vec![argument],
                invocation: Invocation::Value,
            },
            None,
        );
        module.regions[0].statements.push(Statement::Evaluate(call));
        *slot = argument;
    }
    (module, arguments)
}

#[test]
fn a_helper_edit_is_recorded_and_checks() {
    let (mut module, [first, _]) = two_prints();
    let before = module.clone();
    module.open_journal();
    assert!(module.set_expression(first, Expr::Literal(Literal::Number(3.0))));
    module.remove_statement(0, 1);
    let journal = module.take_journal();
    assert_eq!(journal.edits(), 2);
    module.check_journal(&before, &journal).unwrap();
}

#[test]
fn a_direct_write_is_caught() {
    let (mut module, [first, _]) = two_prints();
    let before = module.clone();
    module.open_journal();
    module.expressions[first.index()] = Expr::Literal(Literal::Number(3.0));
    let journal = module.take_journal();
    let error = module.check_journal(&before, &journal).unwrap_err();
    assert!(error.contains("region 0"), "{error}");
}

#[test]
fn a_write_that_changes_nothing_records_nothing() {
    let (mut module, [first, _]) = two_prints();
    let before = module.clone();
    module.open_journal();
    assert!(!module.set_expression(first, Expr::Literal(Literal::Number(1.0))));
    let statement = module.regions[0].statements[0].clone();
    assert!(!module.set_statement(0, 0, statement));
    let journal = module.take_journal();
    assert_eq!(journal.edits(), 0);
    module.check_journal(&before, &journal).unwrap();
    // A helper that borrows for an edit and makes none is a phantom edit.
    module.open_journal();
    let _ = module.statements_mut(0);
    let journal = module.take_journal();
    let error = module.check_journal(&before, &journal).unwrap_err();
    assert!(error.contains("nothing changed"), "{error}");
}

#[test]
fn nothing_is_recorded_outside_a_rule_and_renumbering_carries_entries() {
    let (mut module, [first, second]) = two_prints();
    assert!(module.set_expression(first, Expr::Literal(Literal::Number(4.0))));
    assert_eq!(module.take_journal().edits(), 0);
    let before = module.clone();
    module.open_journal();
    // An edit, then a node created after its parent: renumbering moves both.
    module.set_expression(second, Expr::Literal(Literal::Number(5.0)));
    let late = module.expression(Expr::Literal(Literal::Number(6.0)), None);
    let Statement::Evaluate(call) = module.regions[0].statements[0] else {
        unreachable!()
    };
    if let Expr::Call { arguments, .. } = module.expression_mut(call) {
        arguments[0] = late;
    }
    module
        .renumber(&mut crate::output_budget::AllocationBudget::new(None))
        .unwrap();
    let journal = module.take_journal();
    module.check_journal(&before, &journal).unwrap();
}
