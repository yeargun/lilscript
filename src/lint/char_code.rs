//! `migration/char-code` (language.md R11, M10.9's first batch):
//! `s.charCodeAt(i)` returns JavaScript's number once R11 lands, NaN past the
//! end, where today it is an `int`. The fix keeps today's meaning:
//! - inside `for (int i = k; i < s.length; …)` with `k` a non-negative
//!   literal, where the loop assigns neither `i` (but in its update) nor
//!   `s`, the index is in range: `s.codeUnitAt(i)`, a code unit with no
//!   normalization;
//! - elsewhere, `(s.charCodeAt(i) | 0)`, today's `int` exactly.
use crate::ast::{
    self, ArrayElement, ArrowBody, BinaryOp, ClassMember, Expr, ExprKind, ForInitializer, Item,
    RecordElement, Stmt, TemplatePart,
};
use crate::check::assignments::Assigned;
use crate::check::{CheckedView, Type};
use crate::module::ModuleId;
use crate::span::Span;

use super::{LintEdit, LintFix, PendingDiagnostic};

pub(super) const RULE: &str = "migration/char-code";

/// A loop that bounds `index` by `text.length` over its whole body.
struct Bound<'src> {
    index: &'src str,
    text: &'src str,
}

struct Walk<'a, 'view, 'src> {
    module: ModuleId,
    view: &'a CheckedView<'view, 'view, 'src>,
    bounds: Vec<Bound<'src>>,
    pending: &'a mut Vec<PendingDiagnostic>,
}

pub(super) fn lint<'src>(
    module: ModuleId,
    syntax: &ast::Program<'_, 'src>,
    view: &CheckedView<'_, '_, 'src>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let mut walk = Walk {
        module,
        view,
        bounds: Vec::new(),
        pending,
    };
    for item in syntax.items {
        match item {
            Item::Stmt(statement) => walk.statement(statement),
            Item::Function(function) => walk.body(function.params, function.body),
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Field(field) => {
                            if let Some(initializer) = &field.initializer {
                                walk.expression(initializer);
                            }
                        }
                        ClassMember::Constructor(constructor) => {
                            walk.body(constructor.params, constructor.body)
                        }
                        ClassMember::Method(method) => walk.body(method.params, method.body),
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

impl<'src> Walk<'_, '_, 'src> {
    fn body(&mut self, params: &[ast::Param<'_, 'src>], body: &[Stmt<'_, 'src>]) {
        // A function body starts outside every loop.
        let bounds = std::mem::take(&mut self.bounds);
        for param in params {
            if let Some(default) = &param.default {
                self.expression(default);
            }
        }
        for statement in body {
            self.statement(statement);
        }
        self.bounds = bounds;
    }

    fn statement(&mut self, statement: &Stmt<'_, 'src>) {
        match statement {
            Stmt::VarDecl(declaration) => {
                if let Some(value) = &declaration.initializer {
                    self.expression(value);
                }
            }
            Stmt::ArrayDestructure { value, .. }
            | Stmt::RecordDestructure { value, .. }
            | Stmt::Expr(value)
            | Stmt::Throw { value, .. }
            | Stmt::Yield { value, .. } => self.expression(value),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expression(value);
                }
            }
            Stmt::SuperCall { args, .. } => {
                for argument in *args {
                    self.expression(&argument.expression);
                }
            }
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                for statement in *body {
                    self.statement(statement);
                }
                if let Some(catch) = catch {
                    for statement in catch.body {
                        self.statement(statement);
                    }
                }
                if let Some(finally) = finally {
                    for statement in *finally {
                        self.statement(statement);
                    }
                }
            }
            Stmt::Block { body, .. } => {
                for statement in *body {
                    self.statement(statement);
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expression(condition);
                self.statement(then_branch);
                if let Some(branch) = else_branch {
                    self.statement(branch);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expression(condition);
                self.statement(body);
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
                            self.expression(value);
                        }
                    }
                    Some(ForInitializer::Expr(expression)) => self.expression(expression),
                    None => {}
                }
                if let Some(condition) = condition {
                    self.expression(condition);
                }
                if let Some(update) = update {
                    self.expression(update);
                }
                let bound = loop_bound(initializer.as_ref(), condition.as_ref(), body);
                let bounded = bound.is_some();
                self.bounds.extend(bound);
                self.statement(body);
                if bounded {
                    self.bounds.pop();
                }
            }
            Stmt::ForIn { object, body, .. } => {
                self.expression(object);
                self.statement(body);
            }
            Stmt::ForOf { iterable, body, .. } => {
                self.expression(iterable);
                self.statement(body);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn expression(&mut self, expression: &Expr<'_, 'src>) {
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
                            self.expression(value)
                        }
                    }
                }
            }
            ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
                for entry in *entries {
                    match entry {
                        RecordElement::Entry(entry) => self.expression(&entry.value),
                        RecordElement::Spread { value, .. } => self.expression(value),
                    }
                }
            }
            ExprKind::StructLiteral { values, .. } => {
                for value in *values {
                    self.expression(value);
                }
            }
            ExprKind::New { args, .. } => {
                for argument in *args {
                    self.expression(&argument.expression);
                }
            }
            ExprKind::Call { callee, args, .. } => {
                self.char_code(expression, callee, args);
                self.expression(callee);
                for argument in *args {
                    self.expression(&argument.expression);
                }
            }
            ExprKind::Construct { callee, args, .. } => {
                self.expression(callee);
                for argument in *args {
                    self.expression(&argument.expression);
                }
            }
            ExprKind::ArrowFunction { params, body, .. } => {
                // A lambda may run after the loop: it starts outside it.
                let bounds = std::mem::take(&mut self.bounds);
                for param in *params {
                    if let Some(default) = &param.default {
                        self.expression(default);
                    }
                }
                match body {
                    ArrowBody::Expr(expression) => self.expression(expression),
                    ArrowBody::Block(body) => {
                        for statement in *body {
                            self.statement(statement);
                        }
                    }
                }
                self.bounds = bounds;
            }
            ExprKind::Member { object, .. } | ExprKind::OptionalMember { object, .. } => {
                self.expression(object)
            }
            ExprKind::Unary { expr, .. }
            | ExprKind::DynamicUnary { expr, .. }
            | ExprKind::Await { task: expr, .. } => self.expression(expr),
            // `s.charCodeAt(i) | 0` is already today's `int`: the fix wrote it,
            // or the author did. Its call is not reported again.
            ExprKind::Binary {
                op: BinaryOp::BitOr,
                lhs,
                rhs,
                ..
            } if matches!(rhs.kind, ExprKind::Int(0, _))
                && matches!(&lhs.kind, ExprKind::Call { callee, .. }
                    if matches!(&callee.kind, ExprKind::Member { property, .. } if property.name == "charCodeAt")) =>
            {
                if let ExprKind::Call { callee, args, .. } = &lhs.kind {
                    self.expression(callee);
                    for argument in *args {
                        self.expression(&argument.expression);
                    }
                }
            }
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::DynamicBinary { lhs, rhs, .. } => {
                self.expression(lhs);
                self.expression(rhs);
            }
            ExprKind::TypeCheck { value, .. }
            | ExprKind::Cast { value, .. }
            | ExprKind::Convert { value, .. } => self.expression(value),
            ExprKind::Index { object, index, .. } | ExprKind::OptionalIndex { object, index, .. } => {
                self.expression(object);
                self.expression(index);
            }
            ExprKind::If {
                condition,
                then_value,
                else_value,
                ..
            } => {
                self.expression(condition);
                self.expression(then_value);
                self.expression(else_value);
            }
            ExprKind::Match { value, arms, .. } => {
                self.expression(value);
                for arm in *arms {
                    self.expression(&arm.value);
                }
            }
            ExprKind::Assignment { target, value, .. } => {
                self.expression(target);
                self.expression(value);
            }
            ExprKind::Update { target, .. } => self.expression(target),
            ExprKind::Template { parts, .. } => {
                for part in *parts {
                    if let TemplatePart::Expr(expression) = part {
                        self.expression(expression);
                    }
                }
            }
        }
    }

    /// A typed `s.charCodeAt(i)`: its report and fix.
    fn char_code(
        &mut self,
        call: &Expr<'_, 'src>,
        callee: &Expr<'_, 'src>,
        args: &[ast::Argument<'_, 'src>],
    ) {
        let ExprKind::Member {
            object, property, ..
        } = &callee.kind
        else {
            return;
        };
        if property.name != "charCodeAt"
            || args.len() != 1
            || !matches!(self.view.expression_type(object.id), Some(Type::String))
        {
            return;
        }
        let bounded = match (&object.kind, &args[0].expression.kind) {
            (ExprKind::Ident(text), ExprKind::Ident(index)) => self
                .bounds
                .iter()
                .any(|bound| bound.text == text.name && bound.index == index.name),
            _ => false,
        };
        let span = call.span();
        let edits = if bounded {
            vec![LintEdit {
                span: property.span,
                replacement: "codeUnitAt".to_string(),
            }]
        } else {
            vec![
                LintEdit {
                    span: Span {
                        start: span.start,
                        end: span.start,
                    },
                    replacement: "(".to_string(),
                },
                LintEdit {
                    span: Span {
                        start: span.end,
                        end: span.end,
                    },
                    replacement: " | 0)".to_string(),
                },
            ]
        };
        self.pending.push(PendingDiagnostic {
            module: self.module,
            span: property.span,
            rule: RULE,
            message: "`charCodeAt` returns a number, NaN past the end, once R11 lands".to_string(),
            evidence: None,
            help: Some(
                if bounded {
                    "the loop bounds the index by the string's length: `codeUnitAt` reads the code unit"
                } else {
                    "`| 0` keeps today's `int`, 0 past the end"
                }
                .to_string(),
            ),
            fix: Some(LintFix {
                applicability: "machine-applicable",
                edits,
            }),
        });
    }
}

/// `for (int i = k; i < s.length; …)`, `k` a non-negative literal, whose body
/// assigns neither `i` nor `s` (the update may step `i`).
fn loop_bound<'src>(
    initializer: Option<&ForInitializer<'_, 'src>>,
    condition: Option<&Expr<'_, 'src>>,
    body: &Stmt<'_, 'src>,
) -> Option<Bound<'src>> {
    let Some(ForInitializer::VarDecl(declaration)) = initializer else {
        return None;
    };
    let index = declaration.name.name;
    if !matches!(declaration.initializer.as_ref().map(|value| &value.kind), Some(ExprKind::Int(start, _)) if *start >= 0)
    {
        return None;
    }
    let ExprKind::Binary {
        op: BinaryOp::Less,
        lhs,
        rhs,
        ..
    } = &condition?.kind
    else {
        return None;
    };
    let ExprKind::Ident(compared) = &lhs.kind else {
        return None;
    };
    let ExprKind::Member {
        object, property, ..
    } = &rhs.kind
    else {
        return None;
    };
    let ExprKind::Ident(text) = &object.kind else {
        return None;
    };
    if compared.name != index || property.name != "length" {
        return None;
    }
    let assigned = Assigned::body(&[], std::slice::from_ref(body));
    let written = |name: &str| assigned.own.contains(name) || assigned.nested.contains(name);
    (!written(index) && !written(text.name)).then_some(Bound {
        index,
        text: text.name,
    })
}
