//! `migration/absence` (language.md R2, M10.9's first batch): absent is
//! `null` or `undefined`, which one unspecified, once R2 lands. An operation
//! that could observe which is refused on a `T?` operand without narrowing.
//! Until then the rule warns, and where today's meaning has one spelling it
//! writes it:
//! - `print(x)` and `string(x)` print "null" today: `x ?? "null"`;
//! - `x === y` and `x !== y` on typed operands: `==` and `!=`, which treat both
//!   spellings alike;
//! - `typeof x`, a search for an absent value (`includes`, `indexOf`,
//!   `lastIndexOf`) and `sort` of a `T?[]` are reported without a fix.
use crate::ast::{self, DynamicBinaryOp, DynamicUnaryOp, Expr, ExprKind};
use crate::check::binary_types::equality_comparable_with;
use crate::check::type_relation::Unmetered;
use crate::check::{CheckedView, Type};
use crate::module::ModuleId;
use crate::span::Span;

use super::{LintEdit, LintFix, PendingDiagnostic};

pub(super) const RULE: &str = "migration/absence";

pub(super) fn lint(
    module: ModuleId,
    source: &str,
    syntax: &ast::Program<'_, '_>,
    view: &CheckedView<'_, '_, '_>,
    pending: &mut Vec<PendingDiagnostic>,
) {
    let absent = |expression: &Expr<'_, '_>| {
        matches!(
            view.expression_type(expression.id),
            Some(Type::Nullable(_) | Type::Null)
        )
    };
    let mut report = |span: Span, message: &str, help: &str, edits: Vec<LintEdit>| {
        pending.push(PendingDiagnostic {
            module,
            span,
            rule: RULE,
            message: format!("{message} (R2)"),
            evidence: None,
            help: Some(help.to_string()),
            fix: (!edits.is_empty()).then_some(LintFix {
                applicability: "machine-applicable",
                edits,
            }),
        });
    };
    crate::ast_walk::each_expression(syntax, &mut |expression| match &expression.kind {
        ExprKind::Call { callee, args, .. } => {
            if let (ExprKind::Ident(name), [argument]) = (&callee.kind, args) {
                if name.name == "print" && absent(&argument.expression) {
                    report(
                        argument.expression.span(),
                        "`print` of an optional value shows which spelling absence has",
                        "`x ?? \"null\"` prints what it prints today",
                        with_null_text(&argument.expression),
                    );
                }
            }
            if let ExprKind::Member {
                object, property, ..
            } = &callee.kind
            {
                let searched = matches!(property.name, "includes" | "indexOf" | "lastIndexOf")
                    && args
                        .first()
                        .is_some_and(|argument| absent(&argument.expression));
                let sorted = property.name == "sort"
                    && matches!(
                        view.expression_type(object.id),
                        Some(Type::Array(element)) if matches!(**element, Type::Nullable(_) | Type::Null)
                    );
                if searched {
                    report(
                        property.span,
                        "a search for an optional value finds one spelling of absence",
                        "narrow the value first, or search with `findIndex` and `== null`",
                        Vec::new(),
                    );
                }
                if sorted {
                    report(
                        property.span,
                        "`sort` orders the spellings of absence differently",
                        "sort the present values, or give absence a value first",
                        Vec::new(),
                    );
                }
            }
        }
        ExprKind::Convert { value, .. } if absent(value) => report(
            value.span(),
            "a string conversion of an optional value shows which spelling absence has",
            "`x ?? \"null\"` converts as it does today",
            with_null_text(value),
        ),
        ExprKind::DynamicUnary {
            op: DynamicUnaryOp::TypeOf,
            expr,
            ..
        } if absent(expr) => report(
            expr.span(),
            "`typeof` of an optional value tells the spellings of absence apart",
            "narrow with `== null` first",
            Vec::new(),
        ),
        ExprKind::DynamicBinary {
            op: op @ (DynamicBinaryOp::StrictEq | DynamicBinaryOp::StrictNotEq),
            lhs,
            rhs,
            ..
        } if absent(lhs) || absent(rhs) => {
            let typed = |side: &Expr<'_, '_>| {
                view.expression_type(side.id)
                    .filter(|ty| !matches!(ty, Type::Dynamic | Type::Unknown))
            };
            let loose = match (typed(lhs), typed(rhs)) {
                (Some(left), Some(right)) => {
                    equality_comparable_with(left, right, &mut Unmetered).unwrap_or(false)
                }
                _ => false,
            };
            let spelled = if *op == DynamicBinaryOp::StrictEq {
                "==="
            } else {
                "!=="
            };
            let edits = if loose {
                source
                    .get(lhs.span().end..rhs.span().start)
                    .and_then(|gap| gap.find(spelled))
                    .map(|offset| {
                        let start = lhs.span().end + offset;
                        vec![LintEdit {
                            span: Span {
                                start,
                                end: start + 3,
                            },
                            replacement: if *op == DynamicBinaryOp::StrictEq {
                                "=="
                            } else {
                                "!="
                            }
                            .to_string(),
                        }]
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            report(
                expression.span(),
                "strict equality on an optional value tells the spellings of absence apart",
                "`==` and `!=` treat both alike",
                edits,
            );
        }
        _ => {}
    });
}

/// `x ?? "null"` in place of `x`, parenthesized unless `x` binds tighter.
fn with_null_text(value: &Expr<'_, '_>) -> Vec<LintEdit> {
    let span = value.span();
    let tight = matches!(
        value.kind,
        ExprKind::Ident(_)
            | ExprKind::Member { .. }
            | ExprKind::OptionalMember { .. }
            | ExprKind::Call { .. }
            | ExprKind::Index { .. }
            | ExprKind::OptionalIndex { .. }
            | ExprKind::Null(_)
    );
    let mut edits = Vec::with_capacity(2);
    if !tight {
        edits.push(LintEdit {
            span: Span {
                start: span.start,
                end: span.start,
            },
            replacement: "(".to_string(),
        });
    }
    edits.push(LintEdit {
        span: Span {
            start: span.end,
            end: span.end,
        },
        replacement: if tight {
            " ?? \"null\""
        } else {
            ") ?? \"null\""
        }
        .to_string(),
    });
    edits
}
