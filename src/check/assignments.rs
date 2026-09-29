//! Which names a body assigns, for sound narrowing (R1: a narrowed value
//! inhabits its narrowed type). A test narrows a binding until the flow
//! assigns it; code the flow does not see can assign it too:
//! - a function nested in the binding's body (a lambda; for a module, also
//!   its functions, methods and field initializers) runs whenever it is
//!   called, so a binding it assigns is never narrowed;
//! - a lambda runs after its creation, so inside it a captured binding that
//!   its own body assigns anywhere is never narrowed.
//!
//! Kotlin's smart casts follow the same two rules for `var` locals
//! (<https://kotlinlang.org/docs/typecasts.html#smart-casts>), and TypeScript
//! drops a narrowing inside a function expression for a `let` its body
//! assigns anywhere (`isSymbolAssigned`, typescript@v5.3 `checker.ts`).
//!
//! By name, not by symbol: the scan runs before the body's names resolve, and
//! a shadowing name makes the rule stricter, never unsound. Every expression
//! and statement form is matched without a wildcard, so a new form must say
//! what it assigns.
use crate::ast::{
    ArrayElement, ArrowBody, ClassMember, Expr, ExprKind, ForInitializer, Item, Param, Program,
    RecordElement, Stmt, TemplatePart,
};
use ahash::AHashSet;

#[derive(Debug, Default)]
pub(crate) struct Assigned<'src> {
    /// Names the body's own code assigns, outside the functions it nests.
    pub own: AHashSet<&'src str>,
    /// Names a function the body nests assigns.
    pub nested: AHashSet<&'src str>,
}

impl<'src> Assigned<'src> {
    /// A module: its top-level statements are its own code; its functions,
    /// methods, constructors and field initializers are nested.
    pub(crate) fn module(program: &Program<'_, 'src>) -> Self {
        let mut assigned = Self::default();
        for item in program.items {
            match item {
                Item::Stmt(statement) => assigned.statement(statement, false),
                Item::Function(function) => {
                    assigned.parameters(function.params);
                    assigned.statements(function.body, true);
                }
                Item::Class(class) => {
                    for member in class.members {
                        match member {
                            ClassMember::Field(field) => {
                                if let Some(initializer) = &field.initializer {
                                    assigned.expression(initializer, true);
                                }
                            }
                            ClassMember::Constructor(constructor) => {
                                assigned.parameters(constructor.params);
                                assigned.statements(constructor.body, true);
                            }
                            ClassMember::Method(method) => {
                                assigned.parameters(method.params);
                                assigned.statements(method.body, true);
                            }
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
        assigned
    }

    /// A function's or a lambda's body, with its parameters' defaults.
    pub(crate) fn body(params: &[Param<'_, 'src>], body: &[Stmt<'_, 'src>]) -> Self {
        let mut assigned = Self::default();
        for param in params {
            if let Some(default) = &param.default {
                assigned.expression(default, false);
            }
        }
        assigned.statements(body, false);
        assigned
    }

    /// A lambda whose body is an expression.
    pub(crate) fn expression_body(params: &[Param<'_, 'src>], body: &Expr<'_, 'src>) -> Self {
        let mut assigned = Self::body(params, &[]);
        assigned.expression(body, false);
        assigned
    }

    fn parameters(&mut self, params: &[Param<'_, 'src>]) {
        for param in params {
            if let Some(default) = &param.default {
                self.expression(default, true);
            }
        }
    }

    fn assign(&mut self, target: &Expr<'_, 'src>, nested: bool) {
        if let ExprKind::Ident(ident) = &target.kind {
            if nested {
                self.nested.insert(ident.name);
            } else {
                self.own.insert(ident.name);
            }
        }
    }

    fn statements(&mut self, statements: &[Stmt<'_, 'src>], nested: bool) {
        for statement in statements {
            self.statement(statement, nested);
        }
    }

    fn statement(&mut self, statement: &Stmt<'_, 'src>, nested: bool) {
        match statement {
            Stmt::VarDecl(declaration) => {
                if let Some(value) = &declaration.initializer {
                    self.expression(value, nested);
                }
            }
            Stmt::ArrayDestructure { value, .. } | Stmt::RecordDestructure { value, .. } => {
                self.expression(value, nested)
            }
            Stmt::Expr(expression)
            | Stmt::Throw {
                value: expression, ..
            } => self.expression(expression, nested),
            Stmt::Yield { value, .. } => self.expression(value, nested),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expression(value, nested);
                }
            }
            Stmt::SuperCall { args, .. } => {
                for argument in *args {
                    self.expression(&argument.expression, nested);
                }
            }
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                self.statements(body, nested);
                if let Some(catch) = catch {
                    self.statements(catch.body, nested);
                }
                if let Some(finally) = finally {
                    self.statements(finally, nested);
                }
            }
            Stmt::Block { body, .. } => self.statements(body, nested),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition, nested);
                self.statement(then_branch, nested);
                if let Some(branch) = else_branch {
                    self.statement(branch, nested);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expression(condition, nested);
                self.statement(body, nested);
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
                            self.expression(value, nested);
                        }
                    }
                    Some(ForInitializer::Expr(expression)) => self.expression(expression, nested),
                    None => {}
                }
                if let Some(condition) = condition {
                    self.expression(condition, nested);
                }
                if let Some(update) = update {
                    self.expression(update, nested);
                }
                self.statement(body, nested);
            }
            Stmt::ForIn { object, body, .. } => {
                self.expression(object, nested);
                self.statement(body, nested);
            }
            Stmt::ForOf { iterable, body, .. } => {
                self.expression(iterable, nested);
                self.statement(body, nested);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn expression(&mut self, expression: &Expr<'_, 'src>, nested: bool) {
        match &expression.kind {
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
                            self.expression(value, nested)
                        }
                    }
                }
            }
            ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
                for entry in *entries {
                    match entry {
                        RecordElement::Entry(entry) => self.expression(&entry.value, nested),
                        RecordElement::Spread { value, .. } => self.expression(value, nested),
                    }
                }
            }
            ExprKind::StructLiteral { values, .. } => {
                for value in *values {
                    self.expression(value, nested);
                }
            }
            ExprKind::New { args, .. } => {
                for argument in *args {
                    self.expression(&argument.expression, nested);
                }
            }
            ExprKind::Call { callee, args, .. } | ExprKind::Construct { callee, args, .. } => {
                self.expression(callee, nested);
                for argument in *args {
                    self.expression(&argument.expression, nested);
                }
            }
            ExprKind::ArrowFunction { params, body, .. } => {
                self.parameters(params);
                match body {
                    ArrowBody::Expr(expression) => self.expression(expression, true),
                    ArrowBody::Block(body) => self.statements(body, true),
                }
            }
            ExprKind::Member { object, .. } | ExprKind::OptionalMember { object, .. } => {
                self.expression(object, nested)
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::DynamicUnary { expr, .. }
            | ExprKind::Await { task: expr, .. } => self.expression(expr, nested),
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::DynamicBinary { lhs, rhs, .. } => {
                self.expression(lhs, nested);
                self.expression(rhs, nested);
            }
            ExprKind::TypeCheck { value, .. }
            | ExprKind::Cast { value, .. }
            | ExprKind::Convert { value, .. } => self.expression(value, nested),
            ExprKind::Index { object, index, .. }
            | ExprKind::OptionalIndex { object, index, .. } => {
                self.expression(object, nested);
                self.expression(index, nested);
            }
            ExprKind::If {
                condition,
                then_value,
                else_value,
                ..
            } => {
                self.expression(condition, nested);
                self.expression(then_value, nested);
                self.expression(else_value, nested);
            }
            ExprKind::Match { value, arms, .. } => {
                self.expression(value, nested);
                for arm in *arms {
                    self.expression(&arm.value, nested);
                }
            }
            ExprKind::Assignment { target, value, .. } => {
                self.assign(target, nested);
                self.expression(target, nested);
                self.expression(value, nested);
            }
            ExprKind::Update { target, .. } => {
                self.assign(target, nested);
                self.expression(target, nested);
            }
            ExprKind::Template { parts, .. } => {
                for part in *parts {
                    match part {
                        TemplatePart::String(..) => {}
                        TemplatePart::Expr(expression) => self.expression(expression, nested),
                    }
                }
            }
        }
    }
}
