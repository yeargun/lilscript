//! Every expression of a module, in source order, for rules that look at
//! expressions one at a time. Every form is matched without a wildcard, so a
//! new form must say what it holds.
use crate::ast::{
    self, ArrayElement, ArrowBody, ClassMember, Expr, ExprKind, ForInitializer, Item,
    RecordElement, Stmt, TemplatePart,
};

pub(super) fn each_expression<'src>(
    syntax: &ast::Program<'_, 'src>,
    visit: &mut impl FnMut(&Expr<'_, 'src>),
) {
    for item in syntax.items {
        match item {
            Item::Stmt(statement) => statement_expressions(statement, visit),
            Item::Function(function) => body(function.params, function.body, visit),
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Field(field) => {
                            if let Some(initializer) = &field.initializer {
                                expression(initializer, visit);
                            }
                        }
                        ClassMember::Constructor(constructor) => {
                            body(constructor.params, constructor.body, visit)
                        }
                        ClassMember::Method(method) => body(method.params, method.body, visit),
                    }
                }
            }
            Item::Extern(_)
            | Item::ExternGlobal(_)
            | Item::ExternClass(_)
            | Item::Struct(_)
            | Item::Enum(_) => {}
        }
    }
}

fn body<'src>(
    params: &[ast::Param<'_, 'src>],
    statements: &[Stmt<'_, 'src>],
    visit: &mut impl FnMut(&Expr<'_, 'src>),
) {
    for param in params {
        if let Some(default) = &param.default {
            expression(default, visit);
        }
    }
    for statement in statements {
        statement_expressions(statement, visit);
    }
}

fn statement_expressions<'src>(
    statement: &Stmt<'_, 'src>,
    visit: &mut impl FnMut(&Expr<'_, 'src>),
) {
    match statement {
        Stmt::VarDecl(declaration) => {
            if let Some(value) = &declaration.initializer {
                expression(value, visit);
            }
        }
        Stmt::ArrayDestructure { value, .. }
        | Stmt::RecordDestructure { value, .. }
        | Stmt::Expr(value)
        | Stmt::Throw { value, .. }
        | Stmt::Yield { value, .. } => expression(value, visit),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                expression(value, visit);
            }
        }
        Stmt::SuperCall { args, .. } => {
            for argument in *args {
                expression(&argument.expression, visit);
            }
        }
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            for statement in *body {
                statement_expressions(statement, visit);
            }
            if let Some(catch) = catch {
                for statement in catch.body {
                    statement_expressions(statement, visit);
                }
            }
            if let Some(finally) = finally {
                for statement in *finally {
                    statement_expressions(statement, visit);
                }
            }
        }
        Stmt::Block { body, .. } => {
            for statement in *body {
                statement_expressions(statement, visit);
            }
        }
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            expression(condition, visit);
            statement_expressions(then_branch, visit);
            if let Some(branch) = else_branch {
                statement_expressions(branch, visit);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            expression(condition, visit);
            statement_expressions(body, visit);
        }
        Stmt::For {
            initializer,
            condition,
            update,
            body,
            ..
        } => {
            match initializer {
                Some(ForInitializer::VarDecl(declaration)) => {
                    if let Some(value) = &declaration.initializer {
                        expression(value, visit);
                    }
                }
                Some(ForInitializer::Expr(value)) => expression(value, visit),
                None => {}
            }
            if let Some(condition) = condition {
                expression(condition, visit);
            }
            if let Some(update) = update {
                expression(update, visit);
            }
            statement_expressions(body, visit);
        }
        Stmt::ForIn { object, body, .. } => {
            expression(object, visit);
            statement_expressions(body, visit);
        }
        Stmt::ForOf { iterable, body, .. } => {
            expression(iterable, visit);
            statement_expressions(body, visit);
        }
        Stmt::Break(_) | Stmt::Continue(_) => {}
    }
}

fn expression<'src>(value: &Expr<'_, 'src>, visit: &mut impl FnMut(&Expr<'_, 'src>)) {
    visit(value);
    match &value.kind {
        ExprKind::Int(..)
        | ExprKind::Float(..)
        | ExprKind::String(..)
        | ExprKind::Bool(..)
        | ExprKind::Null(_)
        | ExprKind::Ident(_)
        | ExprKind::DynamicImport { .. } => {}
        ExprKind::ArrayLiteral { elements, .. } => {
            for element in *elements {
                match element {
                    ArrayElement::Value(value) | ArrayElement::Spread { value, .. } => {
                        expression(value, visit)
                    }
                }
            }
        }
        ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
            for entry in *entries {
                match entry {
                    RecordElement::Entry(entry) => expression(&entry.value, visit),
                    RecordElement::Spread { value, .. } => expression(value, visit),
                }
            }
        }
        ExprKind::StructLiteral { values, .. } => {
            for value in *values {
                expression(value, visit);
            }
        }
        ExprKind::New { args, .. } => {
            for argument in *args {
                expression(&argument.expression, visit);
            }
        }
        ExprKind::Call { callee, args, .. } | ExprKind::Construct { callee, args, .. } => {
            expression(callee, visit);
            for argument in *args {
                expression(&argument.expression, visit);
            }
        }
        ExprKind::ArrowFunction { params, body, .. } => {
            for param in *params {
                if let Some(default) = &param.default {
                    expression(default, visit);
                }
            }
            match body {
                ArrowBody::Expr(value) => expression(value, visit),
                ArrowBody::Block(statements) => {
                    for statement in *statements {
                        statement_expressions(statement, visit);
                    }
                }
            }
        }
        ExprKind::Member { object, .. } | ExprKind::OptionalMember { object, .. } => {
            expression(object, visit)
        }
        ExprKind::Unary { expr, .. }
        | ExprKind::DynamicUnary { expr, .. }
        | ExprKind::Await { task: expr, .. } => expression(expr, visit),
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::DynamicBinary { lhs, rhs, .. } => {
            expression(lhs, visit);
            expression(rhs, visit);
        }
        ExprKind::TypeCheck { value, .. }
        | ExprKind::Cast { value, .. }
        | ExprKind::Convert { value, .. } => expression(value, visit),
        ExprKind::Index { object, index, .. } | ExprKind::OptionalIndex { object, index, .. } => {
            expression(object, visit);
            expression(index, visit);
        }
        ExprKind::If {
            condition,
            then_value,
            else_value,
            ..
        } => {
            expression(condition, visit);
            expression(then_value, visit);
            expression(else_value, visit);
        }
        ExprKind::Match { value, arms, .. } => {
            expression(value, visit);
            for arm in *arms {
                expression(&arm.value, visit);
            }
        }
        ExprKind::Assignment { target, value, .. } => {
            expression(target, visit);
            expression(value, visit);
        }
        ExprKind::Update { target, .. } => expression(target, visit),
        ExprKind::Template { parts, .. } => {
            for part in *parts {
                if let TemplatePart::Expr(value) = part {
                    expression(value, visit);
                }
            }
        }
    }
}
