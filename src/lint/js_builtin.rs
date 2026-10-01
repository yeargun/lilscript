//! `migration/js-builtin` (language.md R12 and §14, batch 1 of two). Each
//! `JS.*` call that has a syntax spelling is reported, with a fix to that
//! spelling.
//!
//! A fix keeps the operation. An operator's syntax resolves to the dynamic
//! operation the call names and lowers exactly as the call did. Member
//! access, calls and literals lower as JavaScript evaluates them: a method is
//! read before its arguments, where `JS.invoke` read it after. A fix is
//! offered only where the syntax cannot mean another operation:
//! - `+` of two typed operands is typed addition;
//! - a call of a typed function is a typed call;
//! - an array literal is a JavaScript array only where the checker expects a
//!   `JsValue`.
//!
//! A call is fixed once none of its operands holds another `JS.*` call, so
//! running `lilscript-lint --fix` to a fixed point rewrites nested calls from
//! the inside out. Parentheses come from the parser's precedences.

use crate::ast::{
    self, precedence, ArrowBody, ClassMember, Expr, ExprKind, Item, RecordElement, Stmt,
    TemplatePart,
};
use crate::check::{builtin_constructor_name, BuiltinCall, CheckedView, Type};
use crate::lexer::{lex, Token, TokenKind};
use crate::module::ModuleId;
use crate::span::Span;

use super::{LintEdit, LintFix, PendingDiagnostic};

pub(super) const RULE: &str = "migration/js-builtin";

pub(super) fn lint(
    module: ModuleId,
    source: &str,
    syntax: &ast::Program<'_, '_>,
    view: &CheckedView<'_, '_, '_>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    // The parser keeps no parentheses, so a span can stop inside them; the
    // tokens restore an operand's balanced text.
    let Ok(tokens) = lex(source) else {
        return;
    };
    // A module that binds the name `undefined` (an import or a declaration)
    // shadows JavaScript's; there `JS.undefined()` keeps its spelling.
    let binds_undefined = syntax
        .imports
        .iter()
        .flat_map(|import| import.specifiers.iter())
        .any(|specifier| specifier.local.name == "undefined")
        || syntax.items.iter().any(|item| match item {
            Item::ExternGlobal(global) => global.name.name == "undefined",
            Item::Extern(function) => function.name.name == "undefined",
            Item::Function(function) => function.name.name == "undefined",
            Item::Stmt(Stmt::VarDecl(declaration, ..)) => declaration.name.name == "undefined",
            _ => false,
        });
    let mut walker = Walker {
        module,
        source,
        tokens: &tokens,
        view,
        pending,
        returns_dynamic: false,
        binds_undefined,
    };
    for item in syntax.items {
        walker.item(item);
    }
}

/// What a position in the syntax allows without parentheses, and what the
/// checker expects there.
#[derive(Clone, Copy, Default)]
struct Position {
    /// The least precedence an expression here binds without parentheses.
    precedence: u8,
    /// A whole statement: where `v.k = x` and `delete v.k` may stand.
    statement: bool,
    /// The checker expects a `JsValue` here, so an array literal is one.
    dynamic: bool,
}

impl Position {
    const ANY: Self = Self {
        precedence: precedence::LOWEST,
        statement: false,
        dynamic: false,
    };

    const fn binding(precedence: u8) -> Self {
        Self {
            precedence,
            statement: false,
            dynamic: false,
        }
    }

    const fn expecting(dynamic: bool) -> Self {
        Self {
            precedence: precedence::LOWEST,
            statement: false,
            dynamic,
        }
    }
}

struct Walker<'a, 'view, 'ast, 'src> {
    module: ModuleId,
    source: &'a str,
    tokens: &'a [Token<'a>],
    view: &'a CheckedView<'view, 'ast, 'src>,
    pending: &'a mut Vec<PendingDiagnostic>,
    /// Whether the enclosing callable returns a `JsValue`.
    returns_dynamic: bool,
    /// Whether the module binds the name `undefined`.
    binds_undefined: bool,
}

impl<'ast, 'src> Walker<'_, '_, 'ast, 'src> {
    fn item(&mut self, item: &Item<'ast, 'src>) {
        match item {
            Item::Function(function) => self.callable(
                function.params,
                is_js_type(&function.return_type),
                |walker| walker.statements(function.body),
            ),
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Constructor(constructor) => {
                            self.callable(constructor.params, false, |walker| {
                                walker.statements(constructor.body)
                            })
                        }
                        ClassMember::Method(method) => self.callable(
                            method.params,
                            is_js_type(&method.return_type),
                            |walker| walker.statements(method.body),
                        ),
                        ClassMember::Field(_) => {}
                    }
                }
            }
            Item::Stmt(statement) => self.statement(statement),
            _ => {}
        }
    }

    fn callable(
        &mut self,
        params: &[ast::Param<'ast, 'src>],
        returns_dynamic: bool,
        body: impl FnOnce(&mut Self),
    ) {
        for parameter in params {
            if let Some(default) = &parameter.default {
                self.expr(
                    default,
                    Position::expecting(is_js_type(&parameter.parameter.ty)),
                );
            }
        }
        let outer = std::mem::replace(&mut self.returns_dynamic, returns_dynamic);
        body(self);
        self.returns_dynamic = outer;
    }

    fn statements(&mut self, statements: &[Stmt<'ast, 'src>]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &Stmt<'ast, 'src>) {
        match statement {
            Stmt::VarDecl(declaration, ..) => {
                if let Some(initializer) = &declaration.initializer {
                    self.expr(
                        initializer,
                        Position::expecting(is_js_type(&declaration.ty)),
                    );
                }
            }
            Stmt::ArrayDestructure { value, .. } | Stmt::RecordDestructure { value, .. } => {
                self.expr(value, Position::ANY)
            }
            Stmt::Expr(expression, ..) => self.expr(
                expression,
                Position {
                    statement: true,
                    ..Position::ANY
                },
            ),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.expr(value, Position::expecting(self.returns_dynamic));
                }
            }
            Stmt::Throw { value, .. } | Stmt::Yield { value, .. } => {
                self.expr(value, Position::ANY)
            }
            Stmt::SuperCall { args, .. } => {
                for argument in *args {
                    self.expr(&argument.expression, Position::ANY);
                }
            }
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                self.statements(body);
                if let Some(clause) = catch {
                    self.statements(clause.body);
                }
                if let Some(body) = finally {
                    self.statements(body);
                }
            }
            Stmt::Block { body, .. } => self.statements(body),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                self.expr(condition, Position::ANY);
                self.statement(then_branch);
                if let Some(branch) = else_branch {
                    self.statement(branch);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                self.expr(condition, Position::ANY);
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
                    Some(ast::ForInitializer::VarDecl(declaration)) => {
                        if let Some(value) = &declaration.initializer {
                            self.expr(value, Position::expecting(is_js_type(&declaration.ty)));
                        }
                    }
                    Some(ast::ForInitializer::Expr(expression)) => self.expr(
                        expression,
                        Position {
                            statement: true,
                            ..Position::ANY
                        },
                    ),
                    None => {}
                }
                if let Some(condition) = condition {
                    self.expr(condition, Position::ANY);
                }
                if let Some(update) = update {
                    self.expr(
                        update,
                        Position {
                            statement: true,
                            ..Position::ANY
                        },
                    );
                }
                self.statement(body);
            }
            Stmt::ForIn { object, body, .. } => {
                self.expr(object, Position::ANY);
                self.statement(body);
            }
            Stmt::ForOf { iterable, body, .. } => {
                self.expr(iterable, Position::ANY);
                self.statement(body);
            }
            Stmt::Break(_, ..) | Stmt::Continue(_, ..) => {}
        }
    }

    fn expr(&mut self, expression: &Expr<'ast, 'src>, at: Position) {
        if let Some((name, args)) = self.js_call(expression) {
            self.report(expression, name, args, at);
            // A `JS.*` operand that can hold a JavaScript array literal is a
            // `JsValue` parameter.
            for argument in args {
                self.expr(&argument.expression, Position::expecting(true));
            }
            return;
        }
        if let Some(object) = self.truthy_call(expression) {
            self.report_truthy(expression, object);
        }
        let dynamic_operation = self.view.dynamic_operation(expression.id).is_some();
        match &expression.kind {
            ExprKind::Binary { op, lhs, rhs, .. } => {
                self.expr(lhs, Position::binding(op.precedence()));
                self.expr(rhs, Position::binding(op.precedence() + 1));
            }
            ExprKind::DynamicBinary { op, lhs, rhs, .. } => {
                self.expr(
                    lhs,
                    Position {
                        dynamic: true,
                        ..Position::binding(op.precedence())
                    },
                );
                self.expr(
                    rhs,
                    Position {
                        dynamic: true,
                        ..Position::binding(op.precedence() + 1)
                    },
                );
            }
            ExprKind::Unary { expr: operand, .. } | ExprKind::Await { task: operand, .. } => {
                self.expr(operand, Position::binding(precedence::UNARY))
            }
            ExprKind::DynamicUnary { expr: operand, .. } => self.expr(
                operand,
                Position {
                    dynamic: true,
                    ..Position::binding(precedence::UNARY)
                },
            ),
            ExprKind::Update { target, .. } => {
                self.expr(target, Position::binding(precedence::POSTFIX))
            }
            ExprKind::Member { object, .. } | ExprKind::OptionalMember { object, .. } => {
                self.expr(object, Position::binding(precedence::POSTFIX))
            }
            ExprKind::Index { object, index, .. }
            | ExprKind::OptionalIndex { object, index, .. } => {
                self.expr(object, Position::binding(precedence::POSTFIX));
                self.expr(index, Position::ANY);
            }
            ExprKind::Call { callee, args, .. } => {
                self.expr(callee, Position::binding(precedence::POSTFIX));
                let parameters = match self.view.expression_type(callee.id) {
                    Some(Type::Function(signature)) => Some(&signature.params),
                    _ => None,
                };
                for (position, argument) in args.iter().enumerate() {
                    let dynamic = dynamic_operation
                        || parameters
                            .and_then(|parameters| parameters.get(position))
                            .is_some_and(|parameter| parameter.ty == Type::Dynamic);
                    self.expr(&argument.expression, Position::expecting(dynamic));
                }
            }
            ExprKind::New { args, .. } => {
                for argument in *args {
                    self.expr(&argument.expression, Position::expecting(dynamic_operation));
                }
            }
            ExprKind::Construct { callee, args, .. } => {
                self.expr(callee, Position::ANY);
                for argument in *args {
                    self.expr(&argument.expression, Position::expecting(true));
                }
            }
            ExprKind::ArrayLiteral { elements, .. } => {
                let dynamic = dynamic_operation
                    || matches!(
                        self.view.expression_type(expression.id),
                        Some(Type::Array(element)) if **element == Type::Dynamic
                    );
                for element in *elements {
                    self.expr(element.value(), Position::expecting(dynamic));
                }
            }
            ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
                let dynamic = matches!(
                    self.view.expression_type(expression.id),
                    Some(Type::Dynamic)
                ) || matches!(
                    self.view.expression_type(expression.id),
                    Some(Type::Record(value)) if **value == Type::Dynamic
                );
                for entry in *entries {
                    let dynamic = dynamic && matches!(entry, RecordElement::Entry(_));
                    self.expr(entry.value(), Position::expecting(dynamic));
                }
            }
            ExprKind::With { value, fields, .. } => {
                self.expr(value, Position::ANY);
                for field in *fields {
                    self.expr(&field.value, Position::ANY);
                }
            }
            ExprKind::StructLiteral { values, .. } => {
                for value in *values {
                    self.expr(value, Position::ANY);
                }
            }
            ExprKind::ArrowFunction { params, body, .. } => {
                // An arrow's result type is inferred from its body, so its
                // results expect nothing.
                let returns_dynamic = false;
                self.callable(params, returns_dynamic, |walker| match body {
                    // An arrow's expression body is its result, not a
                    // statement: `v.k = x` there would change its type.
                    ArrowBody::Expr(result) => {
                        walker.expr(result, Position::expecting(returns_dynamic))
                    }
                    ArrowBody::Block(statements) => walker.statements(statements),
                });
            }
            ExprKind::Assignment { target, value, .. } => {
                self.expr(target, Position::binding(precedence::POSTFIX));
                let dynamic = matches!(self.view.expression_type(target.id), Some(Type::Dynamic));
                self.expr(value, Position::expecting(dynamic));
            }
            ExprKind::If {
                condition,
                then_value,
                else_value,
                ..
            } => {
                for part in [condition, then_value, else_value] {
                    self.expr(part, Position::ANY);
                }
            }
            ExprKind::Match { value, arms, .. } => {
                self.expr(value, Position::ANY);
                for arm in *arms {
                    self.expr(&arm.value, Position::ANY);
                }
            }
            ExprKind::TypeCheck { value, .. } => {
                self.expr(value, Position::binding(precedence::TYPE_TEST))
            }
            ExprKind::Cast { value, .. } => self.expr(
                value,
                Position {
                    dynamic: true,
                    ..Position::binding(precedence::RELATIONAL)
                },
            ),
            ExprKind::Convert { value, .. } => self.expr(value, Position::expecting(true)),
            ExprKind::Template { parts, .. } => {
                for part in *parts {
                    if let TemplatePart::Expr(value) = part {
                        self.expr(value, Position::ANY);
                    }
                }
            }
            ExprKind::Int(..)
            | ExprKind::Float(..)
            | ExprKind::String(..)
            | ExprKind::Bool(..)
            | ExprKind::Null(_)
            | ExprKind::Ident(_)
            | ExprKind::DynamicImport { .. } => {}
        }
    }

    /// A call of the `JS` namespace's builtin `name`, as the checker resolved
    /// it (a shadowing `JS` binding is not the namespace).
    fn js_call<'e>(
        &self,
        expression: &'e Expr<'ast, 'src>,
    ) -> Option<(&'src str, &'e [ast::Argument<'ast, 'src>])> {
        let ExprKind::Call { callee, args, .. } = &expression.kind else {
            return None;
        };
        let ExprKind::Member {
            object, property, ..
        } = &callee.kind
        else {
            return None;
        };
        let ExprKind::Ident(namespace) = &object.kind else {
            return None;
        };
        (namespace.name == "JS" && self.view.builtin_call(expression.id).is_some())
            .then_some((property.name, *args))
    }

    /// `v.truthy()` on a `JsValue`: v0.1's truthiness, now `bool(v)` (R12).
    fn truthy_call<'e>(&self, expression: &'e Expr<'ast, 'src>) -> Option<&'e Expr<'ast, 'src>> {
        let ExprKind::Call { callee, args, .. } = &expression.kind else {
            return None;
        };
        let ExprKind::Member {
            object, property, ..
        } = &callee.kind
        else {
            return None;
        };
        (property.name == "truthy" && args.is_empty() && self.is_dynamic(object)).then_some(*object)
    }

    fn report_truthy(&mut self, call: &Expr<'ast, 'src>, object: &Expr<'ast, 'src>) {
        if !fix_enabled("truthy") {
            return;
        }
        let span = self.balanced(call.span());
        let replacement = format!("bool({})", self.text(object));
        self.pending.push(PendingDiagnostic {
            module: self.module,
            span,
            rule: RULE,
            message: format!("`.truthy()` is spelled `{replacement}` (R12)"),
            evidence: None,
            help: Some(
                "`lilscript-lint --fix`, run until nothing changes, rewrites every such call"
                    .to_string(),
            ),
            fix: Some(LintFix {
                applicability: "machine-applicable",
                edits: vec![LintEdit { span, replacement }],
            }),
        });
    }

    fn contains_js_call(&self, expression: &Expr<'ast, 'src>) -> bool {
        let mut found = false;
        let mut stack = vec![expression];
        while let Some(expression) = stack.pop() {
            if self.js_call(expression).is_some() {
                found = true;
                break;
            }
            children(expression, &mut stack);
        }
        found
    }

    fn report(
        &mut self,
        call: &Expr<'ast, 'src>,
        name: &str,
        args: &[ast::Argument<'ast, 'src>],
        at: Position,
    ) {
        if !SPELLED.contains(&name) || !fix_enabled(name) {
            return;
        }
        let nested = args
            .iter()
            .any(|argument| self.contains_js_call(&argument.expression));
        let spelling = if nested {
            None
        } else {
            self.spelling(call, name, args, at)
        };
        // The edit covers the call's balanced text: a call whose callee is a
        // parenthesized expression starts inside the parentheses.
        let span = self.balanced(call.span());
        let message = match &spelling {
            Some(text) => format!("`JS.{name}` is spelled `{text}` (R12)"),
            None if nested => {
                format!("`JS.{name}` has a syntax spelling (R12); its operands are rewritten first")
            }
            None => format!("`JS.{name}` has a syntax spelling (R12), not for these operands yet"),
        };
        let fix = spelling.map(|text| {
            let replacement = if precedence_of(name) < at.precedence {
                format!("({text})")
            } else {
                text
            };
            LintFix {
                applicability: "machine-applicable",
                edits: vec![LintEdit { span, replacement }],
            }
        });
        self.pending.push(PendingDiagnostic {
            module: self.module,
            span,
            rule: RULE,
            message,
            evidence: None,
            help: Some(
                "`lilscript-lint --fix`, run until nothing changes, rewrites every such call"
                    .to_string(),
            ),
            fix,
        });
    }

    /// The syntax for `JS.name(args)`, or `None` where the syntax could mean
    /// another operation.
    fn spelling(
        &self,
        call: &Expr<'ast, 'src>,
        name: &str,
        args: &[ast::Argument<'ast, 'src>],
        at: Position,
    ) -> Option<String> {
        let arg = |index: usize| &args[index].expression;
        let dynamic = |index: usize| self.is_dynamic(arg(index));
        let text = |index: usize, least: u8| self.operand(arg(index), least);
        let rest = |from: usize| {
            args[from..]
                .iter()
                .map(|argument| self.text(&argument.expression).to_string())
                .collect::<Vec<_>>()
                .join(", ")
        };
        let binary =
            |op: &str, left: u8, right: u8| format!("{} {op} {}", text(0, left), text(1, right));
        let either_dynamic = || dynamic(0) || dynamic(1);
        let typed_string =
            |index: usize| matches!(self.view.expression_type(arg(index).id), Some(Type::String));
        // A receiver or callee as the dynamic operation needs it: a typed
        // value is viewed as a `JsValue`, which keeps the operation (R12).
        let receiver = |index: usize| {
            if dynamic(index) {
                self.operand(arg(index), precedence::POSTFIX)
            } else {
                format!(
                    "({} as JsValue)",
                    self.operand(arg(index), precedence::RELATIONAL)
                )
            }
        };
        Some(match (name, args.len()) {
            ("get", 2) => self.property(receiver(0), arg(1), &TYPED_MEMBERS),
            ("set", 3) if at.statement => format!(
                "{} = {}",
                self.property(receiver(0), arg(1), &TYPED_MEMBERS),
                text(2, precedence::LOWEST)
            ),
            ("delete", 2) if at.statement => {
                format!("delete {}", self.property(receiver(0), arg(1), &[]))
            }
            ("invoke", 2..) => format!(
                "{}({})",
                self.property(receiver(0), arg(1), &METHOD_WORDS),
                rest(2)
            ),
            ("call", 2..) if dynamic(0) && self.same_binding_receiver(arg(0), arg(1)) => {
                // `JS.call(o.m, o, a)`: the method called on its own receiver.
                format!("{}({})", text(0, precedence::POSTFIX), rest(2))
            }
            ("call", 2..) => {
                let callee = arg(0);
                let unbound = !matches!(
                    callee.kind,
                    ExprKind::Member { .. }
                        | ExprKind::Index { .. }
                        | ExprKind::OptionalMember { .. }
                        | ExprKind::OptionalIndex { .. }
                );
                if unbound && self.is_undefined(arg(1)) {
                    format!("{}({})", receiver(0), rest(2))
                } else {
                    format!("{}.call({})", receiver(0), rest(1))
                }
            }
            ("apply", 3) => format!("{}.apply({})", receiver(0), rest(1)),
            ("construct", 1..) if dynamic(0) => {
                format!("new {}({})", self.constructor(arg(0)), rest(1))
            }
            ("add", 2) if either_dynamic() && !typed_string(0) && !typed_string(1) => {
                binary("+", 9, 10)
            }
            // `+` with a `string` operand is typed concatenation, which prints
            // the same JavaScript `+`. Its type is `string`, so not where the
            // sum is a receiver or a unary operand.
            ("add", 2)
                if (typed_string(0) || typed_string(1))
                    && self.stringable(arg(0))
                    && self.stringable(arg(1))
                    && at.precedence < precedence::UNARY =>
            {
                binary("+", 9, 10)
            }
            ("mod", 2) if either_dynamic() => binary("%", 10, 11),
            ("lessThan", 2) if either_dynamic() => binary("<", 7, 8),
            ("lessThanOrEqual", 2) if either_dynamic() => binary("<=", 7, 8),
            ("greaterThan", 2) if either_dynamic() => binary(">", 7, 8),
            ("greaterThanOrEqual", 2) if either_dynamic() => binary(">=", 7, 8),
            ("or", 2) if either_dynamic() => binary("||", 1, 2),
            ("and", 2) if either_dynamic() => binary("&&", 2, 3),
            ("strictEqual", 2) => binary("===", 6, 7),
            ("strictNotEqual", 2) => binary("!==", 6, 7),
            ("in", 2) => binary("in", 7, 8),
            ("typeOf", 1) => format!("typeof {}", text(0, precedence::UNARY)),
            // Loose equality with null is the nullish test on a `JsValue`
            // (M1.9); strict equality spells the other two.
            ("isNullish", 1) if self.dynamic_or_nullable(arg(0)) => {
                format!("{} == null", text(0, 6))
            }
            ("isUndefined", 1) if !self.binds_undefined => {
                format!("{} === undefined", text(0, 6))
            }
            ("isFalse", 1) => format!("{} === false", text(0, 6)),
            ("string", 1) => format!("string({})", self.text(arg(0))),
            ("number", 1) => format!("float({})", self.text(arg(0))),
            ("undefined", 0) if !self.binds_undefined => "undefined".to_string(),
            ("object", count) if count % 2 == 0 => {
                let mut entries = Vec::with_capacity(count / 2);
                for pair in args.chunks_exact(2) {
                    let key = match &pair[0].expression.kind {
                        ExprKind::String(raw, _) if identifier(raw) => (*raw).to_string(),
                        ExprKind::String(..) => self.text(&pair[0].expression).to_string(),
                        _ => return None,
                    };
                    entries.push(format!("{key}: {}", self.text(&pair[1].expression)));
                }
                if entries.is_empty() {
                    "object {}".to_string()
                } else {
                    format!("object {{ {} }}", entries.join(", "))
                }
            }
            ("array", _) if at.dynamic => format!("[{}]", rest(0)),
            (adapter, 1) if adapter.starts_with("method") || adapter == "staticRest" => {
                match &arg(0).kind {
                    ExprKind::Ident(name) => self.forwarding_lambda(adapter, name)?,
                    _ => self.method_lambda(adapter, arg(0))?,
                }
            }
            ("assume", 1) if self.dynamic_or_nullable(arg(0)) => {
                // The type the call was checked to produce, spelled as this
                // module names it.
                let target = self.source_type(self.view.expression_type(call.id)?)?;
                format!("{} as {target}", text(0, precedence::RELATIONAL))
            }
            ("assume", 1) => {
                // A typed value: itself where it already has the type, else
                // viewed as a `JsValue` first.
                let checked = self.view.expression_type(call.id)?;
                if self.view.expression_type(arg(0).id)? == checked {
                    self.text(arg(0)).to_string()
                } else if *checked == Type::Dynamic {
                    format!("{} as JsValue", text(0, precedence::RELATIONAL))
                } else {
                    let target = self.source_type(checked)?;
                    format!("{} as JsValue as {target}", text(0, precedence::RELATIONAL))
                }
            }
            _ => return None,
        })
    }

    /// `JS.call(o.m, o, …)` where both `o`s are one binding: a method call
    /// on its own receiver, `o.m(…)`, which reads `o.m` once and calls it with
    /// `o` as `this`, as `JS.call` did.
    fn same_binding_receiver(
        &self,
        callee: &Expr<'ast, 'src>,
        receiver: &Expr<'ast, 'src>,
    ) -> bool {
        let (ExprKind::Member { object, .. } | ExprKind::Index { object, .. }) = &callee.kind
        else {
            return false;
        };
        let (ExprKind::Ident(object), ExprKind::Ident(receiver)) = (&object.kind, &receiver.kind)
        else {
            return false;
        };
        object.name == receiver.name
            && self.view.identifier_symbol(object.id).is_some()
            && self.view.identifier_symbol(object.id) == self.view.identifier_symbol(receiver.id)
    }

    /// `JS.method<N>(lambda)`, `JS.methodRest(lambda)` and
    /// `JS.staticRest(lambda)` as the lambda with its roles written (R7):
    /// `this` before the receiver's type, `...` after the rest's.
    fn method_lambda(&self, adapter: &str, lambda: &Expr<'ast, 'src>) -> Option<String> {
        let ExprKind::ArrowFunction { params, .. } = &lambda.kind else {
            return None;
        };
        let (receiver, rest, count) = match adapter {
            "methodRest" => (true, true, 2),
            "staticRest" => (false, true, 1),
            name => (
                true,
                false,
                name.strip_prefix("method")?.parse::<usize>().ok()? + 1,
            ),
        };
        if params.len() != count
            || params.iter().any(|param| {
                param.default.is_some()
                    || param.role != ast::ParamRole::Value
                    || !is_js_type(&param.parameter.ty)
            })
        {
            return None;
        }
        let span = self.balanced(lambda.span());
        let mut text = String::new();
        let mut at = span.start;
        let mut insert = |offset: usize, marker: &str, text: &mut String| {
            text.push_str(&self.source[at..offset]);
            text.push_str(marker);
            at = offset;
        };
        if receiver {
            insert(params[0].parameter.ty.span.start, "this ", &mut text);
        }
        if rest {
            insert(params[count - 1].parameter.ty.span.end, "...", &mut text);
        }
        text.push_str(&self.source[at..span.end]);
        Some(text)
    }

    /// `JS.method<N>(f)`, `JS.methodRest(f)` and `JS.staticRest(f)` over a
    /// binding that is never reassigned: the lambda that forwards to `f`,
    /// which is what the adapter calls (R7). The adapter reads a reassigned
    /// binding once and a lambda on every call, so that one keeps its
    /// spelling.
    fn forwarding_lambda(&self, adapter: &str, name: &ast::Ident<'src>) -> Option<String> {
        let symbol = self.view.identifier_symbol(name.id)?;
        if self.view.symbol_is_reassigned(symbol) {
            return None;
        }
        let (receiver, rest, count) = match adapter {
            "methodRest" => (true, true, 0),
            "staticRest" => (false, true, 0),
            adapter => (
                true,
                false,
                adapter.strip_prefix("method")?.parse::<usize>().ok()?,
            ),
        };
        // Parameter names the forwarded name does not use.
        let fresh = |base: String| {
            let mut spelled = base;
            while spelled == name.name {
                spelled.push('_');
            }
            spelled
        };
        let mut params = Vec::new();
        let mut arguments = Vec::new();
        if receiver {
            let spelled = fresh("self".to_string());
            params.push(format!("this JsValue {spelled}"));
            arguments.push(spelled);
        }
        for index in 0..count {
            let spelled = fresh(format!("a{index}"));
            params.push(format!("JsValue {spelled}"));
            arguments.push(spelled);
        }
        if rest {
            let spelled = fresh("rest".to_string());
            params.push(format!("JsValue... {spelled}"));
            arguments.push(spelled);
        }
        Some(format!(
            "({}) => {}({})",
            params.join(", "),
            name.name,
            arguments.join(", ")
        ))
    }

    /// A checked type in source syntax, where this module's names spell it.
    fn source_type(&self, ty: &Type<'src>) -> Option<String> {
        let grouped = |ty: &Type<'src>| -> Option<String> {
            let text = self.source_type(ty)?;
            Some(if matches!(ty, Type::Union(_)) {
                format!("({text})")
            } else {
                text
            })
        };
        let nominal = |declaration: &crate::check::NominalType<'src>| {
            (self.view.type_binding(declaration.name) == Some(declaration.identity))
                .then(|| declaration.name.to_string())
        };
        Some(match ty {
            Type::Int
            | Type::Float
            | Type::String
            | Type::Bool
            | Type::Dynamic
            | Type::Symbol
            | Type::Regex
            | Type::ArrayBuffer
            | Type::SharedArrayBuffer
            | Type::Int8Array
            | Type::Uint8Array
            | Type::Uint8ClampedArray
            | Type::Int16Array
            | Type::Uint16Array
            | Type::Int32Array
            | Type::Uint32Array
            | Type::Float32Array
            | Type::Float64Array
            | Type::TypeParameter(_) => ty.to_string(),
            Type::Array(element) => format!("{}[]", grouped(element)?),
            Type::Nullable(inner) => format!("{}?", grouped(inner)?),
            Type::Record(value) => format!("Record<{}>", self.source_type(value)?),
            Type::Map(key, value) => {
                format!(
                    "Map<{}, {}>",
                    self.source_type(key)?,
                    self.source_type(value)?
                )
            }
            Type::Set(element) => format!("Set<{}>", self.source_type(element)?),
            Type::Task(value) => format!("Task<{}>", self.source_type(value)?),
            Type::Generator(value) => format!("Generator<{}>", self.source_type(value)?),
            Type::Enum(declaration) | Type::Struct(declaration) | Type::Class(declaration) => {
                nominal(declaration)?
            }
            Type::StructInstance { declaration, args }
            | Type::ClassInstance { declaration, args } => {
                let args = args
                    .iter()
                    .map(|argument| self.source_type(argument))
                    .collect::<Option<Vec<_>>>()?;
                format!("{}<{}>", nominal(declaration)?, args.join(", "))
            }
            Type::Union(members) => members
                .iter()
                .map(|member| self.source_type(member))
                .collect::<Option<Vec<_>>>()?
                .join(" | "),
            _ => return None,
        })
    }

    /// `v.k` for a key that is an identifier-like string literal outside
    /// `reserved`, and `v[k]` otherwise.
    fn property(&self, receiver: String, key: &Expr<'ast, 'src>, reserved: &[&str]) -> String {
        match &key.kind {
            ExprKind::String(raw, _) if identifier(raw) && !reserved.contains(raw) => {
                format!("{receiver}.{raw}")
            }
            _ => format!("{receiver}[{}]", self.text(key)),
        }
    }

    /// `new C(a)` needs `C` to be a member chain of names that is not a
    /// class; anything else is parenthesized.
    fn constructor(&self, callee: &Expr<'ast, 'src>) -> String {
        let text = self.text(callee);
        let mut chain = callee;
        while let ExprKind::Member { object, .. } = &chain.kind {
            chain = object;
        }
        match &chain.kind {
            // A lone name must not also name a class or a builtin type, which
            // `new` would build.
            ExprKind::Ident(name)
                if !std::ptr::eq(chain, callee)
                    || (self.view.type_binding(name.name).is_none()
                        && !builtin_constructor_name(name.name)) =>
            {
                text.to_string()
            }
            _ => format!("({text})"),
        }
    }

    /// An operand typed concatenation takes, as the checker's rule does:
    /// a string, a number, a boolean or a `JsValue`, or a union of them.
    fn stringable(&self, expression: &Expr<'ast, 'src>) -> bool {
        let scalar = |ty: &Type<'src>| {
            matches!(
                ty,
                Type::String | Type::Int | Type::Float | Type::Bool | Type::Dynamic
            )
        };
        match self.view.expression_type(expression.id) {
            Some(Type::Union(members)) => members.iter().all(scalar),
            Some(ty) => scalar(ty),
            None => false,
        }
    }

    fn is_dynamic(&self, expression: &Expr<'ast, 'src>) -> bool {
        matches!(
            self.view.expression_type(expression.id),
            Some(Type::Dynamic)
        )
    }

    fn dynamic_or_nullable(&self, expression: &Expr<'ast, 'src>) -> bool {
        match self.view.expression_type(expression.id) {
            Some(Type::Dynamic) => true,
            Some(Type::Nullable(inner)) => **inner == Type::Dynamic,
            _ => false,
        }
    }

    /// `undefined`, as a previous fix spelled `JS.undefined()`.
    fn is_undefined(&self, expression: &Expr<'ast, 'src>) -> bool {
        matches!(&expression.kind, ExprKind::Ident(name) if name.name == "undefined")
            && self.view.dynamic_operation(expression.id) == Some(BuiltinCall::JsUndefined)
    }

    fn text(&self, expression: &Expr<'ast, 'src>) -> &str {
        let span = self.balanced(expression.span());
        &self.source[span.start..span.end]
    }

    /// `span` widened to the parentheses its text leaves open: the parser
    /// drops a parenthesized operand's parentheses, so `(a - b) / c` spans
    /// from `a`.
    fn balanced(&self, span: Span) -> Span {
        let first = self
            .tokens
            .partition_point(|token| token.span.start < span.start);
        let end = self
            .tokens
            .partition_point(|token| token.span.end <= span.end);
        let (mut depth, mut lowest) = (0i32, 0i32);
        for token in &self.tokens[first..end.max(first)] {
            match token.kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth -= 1;
                    lowest = lowest.min(depth);
                }
                _ => {}
            }
        }
        let (mut open, mut close) = (-lowest, depth - lowest);
        let (mut start, mut stop) = (span.start, span.end);
        let mut before = first;
        while open > 0 && before > 0 && matches!(self.tokens[before - 1].kind, TokenKind::LParen) {
            before -= 1;
            start = self.tokens[before].span.start;
            open -= 1;
        }
        let mut after = end;
        while close > 0
            && after < self.tokens.len()
            && matches!(self.tokens[after].kind, TokenKind::RParen)
        {
            stop = self.tokens[after].span.end;
            after += 1;
            close -= 1;
        }
        Span { start, end: stop }
    }

    /// An operand's text, parenthesized where its position needs `least`.
    fn operand(&self, expression: &Expr<'ast, 'src>, least: u8) -> String {
        let text = self.text(expression);
        if expression.precedence() < least {
            format!("({text})")
        } else {
            text.to_string()
        }
    }
}

/// The `JS.*` builtins with a syntax spelling in this batch.
const SPELLED: [&str; 41] = [
    "get",
    "set",
    "delete",
    "invoke",
    "call",
    "apply",
    "construct",
    "add",
    "mod",
    "lessThan",
    "lessThanOrEqual",
    "greaterThan",
    "greaterThanOrEqual",
    "or",
    "and",
    "strictEqual",
    "strictNotEqual",
    "in",
    "typeOf",
    "string",
    "number",
    "undefined",
    "object",
    "array",
    "assume",
    "method0",
    "method1",
    "method2",
    "method3",
    "method4",
    "method5",
    "method6",
    "method7",
    "method8",
    "method9",
    "method10",
    "methodRest",
    "staticRest",
    "isNullish",
    "isUndefined",
    "isFalse",
];

/// `LILSCRIPT_JS_FIX_ONLY=get,set,…`, a diagnostic knob: only those `JS.*`
/// names are reported and fixed, so a port's rewrite can be measured one
/// family of spellings at a time.
fn fix_enabled(name: &str) -> bool {
    std::env::var("LILSCRIPT_JS_FIX_ONLY")
        .map(|only| only.split(',').any(|enabled| enabled.trim() == name))
        .unwrap_or(true)
}

/// The members a `JsValue` has with a declared type (v0.1); reading one is
/// not a dynamic property, so the fix writes it with brackets.
const TYPED_MEMBERS: [&str; 6] = [
    "length",
    "message",
    "specifier",
    "truthy",
    "isArray",
    "isObject",
];

/// Method names whose call means something else: the typed members, and
/// `call` and `apply`, which name `JS.call` and `JS.apply`.
const METHOD_WORDS: [&str; 5] = ["truthy", "isArray", "isObject", "call", "apply"];

/// How tightly the spelling of `JS.name(…)` binds.
fn precedence_of(name: &str) -> u8 {
    match name {
        "set" => precedence::LOWEST,
        "isNullish" | "isUndefined" | "isFalse" => 6,
        name if name.starts_with("method") || name == "staticRest" => precedence::LOWEST,
        "or" => 1,
        "and" => 2,
        "strictEqual" | "strictNotEqual" => 6,
        "lessThan" | "lessThanOrEqual" | "greaterThan" | "greaterThanOrEqual" | "in" | "assume" => {
            precedence::RELATIONAL
        }
        "add" => 9,
        "mod" => 10,
        "typeOf" | "delete" => precedence::UNARY,
        "get" | "invoke" | "call" | "apply" => precedence::POSTFIX,
        _ => precedence::PRIMARY,
    }
}

/// A string literal's content that is an identifier in LilScript's lexer
/// and needs no decoding.
fn identifier(raw: &str) -> bool {
    let mut chars = raw.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_' || first == '$')
        && chars.all(|next| next.is_ascii_alphanumeric() || next == '_' || next == '$')
}

/// A declaration of exactly `JsValue`: where the checker expects one, an
/// array literal is a JavaScript array.
fn is_js_type(ty: &ast::TypeRef<'_, '_>) -> bool {
    matches!(ty.kind, ast::TypeKind::Named { name: "JsValue", args } if args.is_empty())
}

fn children<'e, 'ast, 'src>(
    expression: &'e Expr<'ast, 'src>,
    stack: &mut Vec<&'e Expr<'ast, 'src>>,
) {
    match &expression.kind {
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::DynamicBinary { lhs, rhs, .. } => {
            stack.extend([*lhs, *rhs])
        }
        ExprKind::Index { object, index, .. } | ExprKind::OptionalIndex { object, index, .. } => {
            stack.extend([*object, *index])
        }
        ExprKind::Assignment { target, value, .. } => stack.extend([*target, *value]),
        ExprKind::Unary { expr, .. }
        | ExprKind::DynamicUnary { expr, .. }
        | ExprKind::Await { task: expr, .. }
        | ExprKind::Update { target: expr, .. }
        | ExprKind::Member { object: expr, .. }
        | ExprKind::OptionalMember { object: expr, .. }
        | ExprKind::TypeCheck { value: expr, .. }
        | ExprKind::Cast { value: expr, .. }
        | ExprKind::Convert { value: expr, .. } => stack.push(expr),
        ExprKind::Call { callee, args, .. } | ExprKind::Construct { callee, args, .. } => {
            stack.push(callee);
            stack.extend(args.iter().map(|argument| &argument.expression));
        }
        ExprKind::New { args, .. } => {
            stack.extend(args.iter().map(|argument| &argument.expression))
        }
        ExprKind::ArrayLiteral { elements, .. } => {
            stack.extend(elements.iter().map(|element| element.value()))
        }
        ExprKind::RecordLiteral { entries, .. } | ExprKind::ObjectLiteral { entries, .. } => {
            stack.extend(entries.iter().map(|entry| entry.value()))
        }
        ExprKind::With { value, fields, .. } => {
            stack.push(value);
            stack.extend(fields.iter().map(|field| &field.value));
        }
        ExprKind::StructLiteral { values, .. } => stack.extend(values.iter()),
        ExprKind::ArrowFunction { body, .. } => match body {
            ArrowBody::Expr(result) => stack.push(result),
            // A block body's calls are its own statements', fixed there.
            ArrowBody::Block(_) => {}
        },
        ExprKind::If {
            condition,
            then_value,
            else_value,
            ..
        } => stack.extend([*condition, *then_value, *else_value]),
        ExprKind::Match { value, arms, .. } => {
            stack.push(value);
            stack.extend(arms.iter().map(|arm| &arm.value));
        }
        ExprKind::Template { parts, .. } => {
            stack.extend(parts.iter().filter_map(|part| match part {
                TemplatePart::Expr(value) => Some(value),
                TemplatePart::String(..) => None,
            }))
        }
        ExprKind::Int(..)
        | ExprKind::Float(..)
        | ExprKind::String(..)
        | ExprKind::Bool(..)
        | ExprKind::Null(_)
        | ExprKind::Ident(_)
        | ExprKind::DynamicImport { .. } => {}
    }
}
