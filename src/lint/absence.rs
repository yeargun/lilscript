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
use crate::ast::{self, Expr, ExprKind};
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
    use crate::check::absence::{Fix, observation};
    let mut budget = crate::output_budget::AllocationBudget::new(None);
    crate::ast_walk::each_expression(syntax, &mut |expression| {
        let Some(problem) =
            observation(*view, expression, &mut budget).expect("unlimited inspection admission")
        else {
            return;
        };
        let edits = match problem.fix {
            Fix::None => Vec::new(),
            Fix::NullText(value) => with_null_text(value),
            Fix::LooseEquality { lhs, rhs, equal } => {
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
                if loose {
                    source
                        .get(lhs.span().end..rhs.span().start)
                        .and_then(|gap| gap.find(if equal { "===" } else { "!==" }))
                        .map(|offset| {
                            let start = lhs.span().end + offset;
                            vec![LintEdit {
                                span: Span {
                                    start,
                                    end: start + 3,
                                },
                                replacement: if equal { "==" } else { "!=" }.into(),
                            }]
                        })
                        .unwrap_or_default()
                } else {
                    Vec::new()
                }
            }
        };
        pending.push(PendingDiagnostic {
            module,
            span: problem.span,
            rule: RULE,
            message: problem.message.to_string(),
            evidence: None,
            help: Some(problem.help.to_string()),
            fix: (!edits.is_empty()).then_some(LintFix {
                applicability: "machine-applicable",
                edits,
            }),
        });
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
