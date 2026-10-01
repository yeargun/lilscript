//! Every expression of a module or of one expression, in source order, for
//! passes that look at expressions one at a time (lint rules, the checker's
//! capture tests). Every form is matched without a wildcard, so a new form
//! must say what it holds.
use crate::ast::{
    self, ArrayElement, ArrowBody, ClassMember, Expr, ExprKind, ForInitializer, Ident, Item,
    RecordElement, Stmt, TemplatePart,
};

pub(crate) fn each_expression<'src>(
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
        Stmt::VarDecl(declaration, ..) => {
            if let Some(value) = &declaration.initializer {
                expression(value, visit);
            }
        }
        Stmt::ArrayDestructure { value, .. }
        | Stmt::RecordDestructure { value, .. }
        | Stmt::Expr(value, ..)
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
        Stmt::Break(_, ..) | Stmt::Continue(_, ..) => {}
    }
}

/// `value` and every expression it holds, nested functions' included.
pub(crate) fn expression<'src>(value: &Expr<'_, 'src>, visit: &mut impl FnMut(&Expr<'_, 'src>)) {
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
        ExprKind::With { value, fields, .. } => {
            expression(value, visit);
            for field in *fields {
                expression(&field.value, visit);
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

/// Every identifier of a module, in source order: declarations, references,
/// member names and module specifiers (the lint's reference counts, the
/// language server's renames).
pub fn each_identifier<'src>(
    program: &ast::Program<'_, 'src>,
    visitor: &mut impl FnMut(&Ident<'src>),
) {
    for import in program.imports {
        for specifier in import.specifiers {
            visitor(&specifier.imported);
            visitor(&specifier.local);
        }
    }
    for import in program.foreign_imports {
        for specifier in import.specifiers {
            visitor(&specifier.imported);
            visitor(&specifier.local);
        }
    }
    for export in program.exports {
        visitor(&export.local);
        visitor(&export.exported);
    }
    for item in program.items {
        item_identifiers(item, visitor);
    }
}

fn item_identifiers<'src>(item: &Item<'_, 'src>, visitor: &mut impl FnMut(&Ident<'src>)) {
    match item {
        Item::Enum(declaration) => {
            visitor(&declaration.name);
            for variant in declaration.variants {
                visitor(&variant.name);
            }
        }
        Item::Function(function) => {
            visitor(&function.name);
            for parameter in function.params {
                visitor(&parameter.name);
                if let Some(default) = &parameter.default {
                    expression_identifiers(default, visitor);
                }
            }
            statements_identifiers(function.body, visitor);
        }
        Item::Extern(function) => {
            visitor(&function.name);
            for parameter in function.params {
                visitor(&parameter.name);
            }
        }
        Item::ExternGlobal(global) => visitor(&global.name),
        Item::Struct(structure) => visitor(&structure.name),
        Item::Class(class) => {
            visitor(&class.name);
            for member in class.members {
                match member {
                    ClassMember::Field(field) => visitor(&field.name),
                    ClassMember::Constructor(constructor) => {
                        for parameter in constructor.params {
                            visitor(&parameter.name);
                        }
                        statements_identifiers(constructor.body, visitor);
                    }
                    ClassMember::Method(method) => {
                        visitor(&method.name);
                        for parameter in method.params {
                            visitor(&parameter.name);
                        }
                        statements_identifiers(method.body, visitor);
                    }
                }
            }
        }
        Item::ExternClass(class) => visitor(&class.name),
        Item::Stmt(statement) => statement_identifiers(statement, visitor),
    }
}

fn statements_identifiers<'src>(
    statements: &[Stmt<'_, 'src>],
    visitor: &mut impl FnMut(&Ident<'src>),
) {
    for statement in statements {
        statement_identifiers(statement, visitor);
    }
}

fn statement_identifiers<'src>(statement: &Stmt<'_, 'src>, visitor: &mut impl FnMut(&Ident<'src>)) {
    match statement {
        Stmt::VarDecl(declaration, ..) => {
            visitor(&declaration.name);
            if let Some(initializer) = &declaration.initializer {
                expression_identifiers(initializer, visitor);
            }
        }
        Stmt::ArrayDestructure {
            bindings, value, ..
        } => {
            for binding in *bindings {
                match binding {
                    crate::ast::ArrayBinding::Hole(_) => {}
                    crate::ast::ArrayBinding::Name(name) | crate::ast::ArrayBinding::Rest(name) => {
                        visitor(&name)
                    }
                }
            }
            expression_identifiers(value, visitor);
        }
        Stmt::RecordDestructure {
            bindings,
            rest,
            value,
            ..
        } => {
            for binding in *bindings {
                visitor(&binding.name);
            }
            if let Some(rest) = rest {
                visitor(&rest);
            }
            expression_identifiers(value, visitor);
        }
        Stmt::Expr(expression, ..) => expression_identifiers(expression, visitor),
        Stmt::Return { value, .. } => {
            if let Some(value) = value {
                expression_identifiers(value, visitor);
            }
        }
        Stmt::Throw { value, .. } => expression_identifiers(value, visitor),
        Stmt::SuperCall { args, .. } => {
            for argument in *args {
                expression_identifiers(&argument.expression, visitor);
            }
        }
        Stmt::Yield { value, .. } => expression_identifiers(value, visitor),
        Stmt::Try {
            body,
            catch,
            finally,
            ..
        } => {
            statements_identifiers(body, visitor);
            if let Some(clause) = catch {
                if let Some(binding) = clause.binding {
                    visitor(&binding.name);
                }
                statements_identifiers(clause.body, visitor);
            }
            if let Some(body) = finally {
                statements_identifiers(body, visitor);
            }
        }
        Stmt::Block { body, .. } => statements_identifiers(body, visitor),
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            ..
        } => {
            expression_identifiers(condition, visitor);
            statement_identifiers(then_branch, visitor);
            if let Some(branch) = else_branch {
                statement_identifiers(branch, visitor);
            }
        }
        Stmt::While {
            condition, body, ..
        } => {
            expression_identifiers(condition, visitor);
            statement_identifiers(body, visitor);
        }
        Stmt::For {
            initializer,
            condition,
            update,
            body,
            ..
        } => {
            if let Some(initializer) = initializer {
                match initializer {
                    crate::ast::ForInitializer::VarDecl(declaration) => {
                        visitor(&declaration.name);
                        if let Some(value) = &declaration.initializer {
                            expression_identifiers(value, visitor);
                        }
                    }
                    crate::ast::ForInitializer::Expr(expression) => {
                        expression_identifiers(expression, visitor)
                    }
                }
            }
            if let Some(condition) = condition {
                expression_identifiers(condition, visitor);
            }
            if let Some(update) = update {
                expression_identifiers(update, visitor);
            }
            statement_identifiers(body, visitor);
        }
        Stmt::ForIn {
            key, object, body, ..
        } => {
            visitor(&key);
            expression_identifiers(object, visitor);
            statement_identifiers(body, visitor);
        }
        Stmt::ForOf {
            element,
            iterable,
            body,
            ..
        } => {
            visitor(&element);
            expression_identifiers(iterable, visitor);
            statement_identifiers(body, visitor);
        }
        Stmt::Break(_, ..) | Stmt::Continue(_, ..) => {}
    }
}

fn expression_identifiers<'src>(
    expression: &Expr<'_, 'src>,
    visitor: &mut impl FnMut(&Ident<'src>),
) {
    match expression {
        Expr {
            kind: ExprKind::Ident(identifier),
            ..
        } => visitor(&identifier),
        Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } => {
            for element in *elements {
                expression_identifiers(element.value(), visitor);
            }
        }
        Expr {
            kind: ExprKind::RecordLiteral { entries, .. },
            ..
        }
        | Expr {
            kind: ExprKind::ObjectLiteral { entries, .. },
            ..
        } => {
            for entry in *entries {
                if let crate::ast::RecordElement::Entry(entry) = entry {
                    visitor(&entry.key);
                }
                expression_identifiers(entry.value(), visitor);
            }
        }
        Expr {
            kind: ExprKind::With { value, fields, .. },
            ..
        } => {
            expression_identifiers(value, visitor);
            for field in *fields {
                visitor(&field.key);
                expression_identifiers(&field.value, visitor);
            }
        }
        Expr {
            kind: ExprKind::StructLiteral { name, values, .. },
            ..
        } => {
            visitor(&name);
            for value in *values {
                expression_identifiers(value, visitor);
            }
        }
        Expr {
            kind: ExprKind::New { class, args, .. },
            ..
        } => {
            visitor(&class);
            for argument in *args {
                expression_identifiers(&argument.expression, visitor);
            }
        }
        Expr {
            kind: ExprKind::Member { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalMember { object, .. },
            ..
        } => expression_identifiers(object, visitor),
        Expr {
            kind: ExprKind::Call { callee, args, .. },
            ..
        } => {
            expression_identifiers(callee, visitor);
            for argument in *args {
                expression_identifiers(&argument.expression, visitor);
            }
        }
        Expr {
            kind: ExprKind::ArrowFunction { params, body, .. },
            ..
        } => {
            for parameter in *params {
                visitor(&parameter.name);
            }
            match body {
                ArrowBody::Expr(expression) => expression_identifiers(expression, visitor),
                ArrowBody::Block(body) => statements_identifiers(body, visitor),
            }
        }
        Expr {
            kind: ExprKind::Unary { expr, .. },
            ..
        } => expression_identifiers(expr, visitor),
        Expr {
            kind: ExprKind::Await { task, .. },
            ..
        } => expression_identifiers(task, visitor),
        Expr {
            kind: ExprKind::Binary { lhs, rhs, .. },
            ..
        } => {
            expression_identifiers(lhs, visitor);
            expression_identifiers(rhs, visitor);
        }
        Expr {
            kind: ExprKind::TypeCheck { value, .. },
            ..
        } => expression_identifiers(value, visitor),
        Expr {
            kind: ExprKind::Index { object, index, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalIndex { object, index, .. },
            ..
        } => {
            expression_identifiers(object, visitor);
            expression_identifiers(index, visitor);
        }
        Expr {
            kind: ExprKind::Assignment { target, value, .. },
            ..
        } => {
            expression_identifiers(target, visitor);
            expression_identifiers(value, visitor);
        }
        Expr {
            kind: ExprKind::Update { target, .. },
            ..
        } => expression_identifiers(target, visitor),
        Expr {
            kind: ExprKind::Cast { value, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Convert { value, .. },
            ..
        }
        | Expr {
            kind: ExprKind::DynamicUnary { expr: value, .. },
            ..
        } => expression_identifiers(value, visitor),
        Expr {
            kind: ExprKind::DynamicBinary { lhs, rhs, .. },
            ..
        } => {
            expression_identifiers(lhs, visitor);
            expression_identifiers(rhs, visitor);
        }
        Expr {
            kind: ExprKind::Construct { callee, args, .. },
            ..
        } => {
            expression_identifiers(callee, visitor);
            for argument in *args {
                expression_identifiers(&argument.expression, visitor);
            }
        }
        Expr {
            kind: ExprKind::Template { parts, .. },
            ..
        } => {
            for part in *parts {
                if let crate::ast::TemplatePart::Expr(expression) = part {
                    expression_identifiers(expression, visitor);
                }
            }
        }
        Expr {
            kind: ExprKind::Match { value, arms, .. },
            ..
        } => {
            expression_identifiers(value, visitor);
            for arm in *arms {
                if let crate::ast::MatchPattern::EnumVariant {
                    enum_name, variant, ..
                } = arm.pattern
                {
                    visitor(&enum_name);
                    visitor(&variant);
                }
                expression_identifiers(&arm.value, visitor);
            }
        }
        Expr {
            kind:
                ExprKind::If {
                    condition,
                    then_value,
                    else_value,
                    ..
                },
            ..
        } => {
            expression_identifiers(condition, visitor);
            expression_identifiers(then_value, visitor);
            expression_identifiers(else_value, visitor);
        }
        Expr {
            kind: ExprKind::Int(..),
            ..
        }
        | Expr {
            kind: ExprKind::Float(..),
            ..
        }
        | Expr {
            kind: ExprKind::String(..),
            ..
        }
        | Expr {
            kind: ExprKind::Bool(..),
            ..
        }
        | Expr {
            kind: ExprKind::Null(..),
            ..
        }
        | Expr {
            kind: ExprKind::DynamicImport { .. },
            ..
        } => {}
    }
}

/// Statement occurrences, including statements in nested arrow bodies. The
/// expression walker already owns finding those bodies; the statement walk
/// follows only control-flow children, so every occurrence is visited once.
pub(crate) fn each_statement<'src>(
    syntax: &ast::Program<'_, 'src>,
    visit: &mut impl FnMut(&Stmt<'_, 'src>),
) {
    fn statement<'src>(node: &Stmt<'_, 'src>, visit: &mut impl FnMut(&Stmt<'_, 'src>)) {
        visit(node);
        match node {
            Stmt::Block { body, .. } => {
                for child in *body {
                    statement(child, visit);
                }
            }
            Stmt::If {
                then_branch,
                else_branch,
                ..
            } => {
                statement(then_branch, visit);
                if let Some(child) = else_branch {
                    statement(child, visit);
                }
            }
            Stmt::While { body, .. }
            | Stmt::For { body, .. }
            | Stmt::ForIn { body, .. }
            | Stmt::ForOf { body, .. } => statement(body, visit),
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                for child in *body {
                    statement(child, visit);
                }
                if let Some(catch) = catch {
                    for child in catch.body {
                        statement(child, visit);
                    }
                }
                if let Some(finally) = finally {
                    for child in *finally {
                        statement(child, visit);
                    }
                }
            }
            Stmt::VarDecl(..)
            | Stmt::Expr(..)
            | Stmt::Return { .. }
            | Stmt::Throw { .. }
            | Stmt::Yield { .. }
            | Stmt::SuperCall { .. }
            | Stmt::ArrayDestructure { .. }
            | Stmt::RecordDestructure { .. }
            | Stmt::Break(..)
            | Stmt::Continue(..) => {}
        }
    }
    for item in syntax.items {
        match item {
            Item::Stmt(node) => statement(node, visit),
            Item::Function(function) => {
                for node in function.body {
                    statement(node, visit);
                }
            }
            Item::Class(class) => {
                for member in class.members {
                    let body = match member {
                        ClassMember::Constructor(c) => c.body,
                        ClassMember::Method(m) => m.body,
                        ClassMember::Field(_) => continue,
                    };
                    for node in body {
                        statement(node, visit);
                    }
                }
            }
            _ => {}
        }
    }
    each_expression(syntax, &mut |expression| {
        if let ast::ExprKind::ArrowFunction {
            body: ArrowBody::Block(body),
            ..
        } = &expression.kind
        {
            for node in *body {
                statement(node, visit);
            }
        }
    });
}
