//! R2's observation rule, shared by the selected source contract and migration
//! lint. Resolve operations by checked identity, never by a user's method name.
use super::*;
use crate::compilation_policy::WorkKind;
use crate::primitive::{Intrinsic, ResolvedIntrinsic};

#[derive(Clone, Copy)]
pub(crate) enum Fix<'ast, 'src> {
    None,
    NullText(&'ast Expr<'ast, 'src>),
    LooseEquality {
        lhs: &'ast Expr<'ast, 'src>,
        rhs: &'ast Expr<'ast, 'src>,
        equal: bool,
    },
}
pub(crate) struct Observation<'ast, 'src> {
    pub span: Span,
    pub message: &'static str,
    pub help: &'static str,
    pub fix: Fix<'ast, 'src>,
}

pub(crate) fn same_storage_pin(a: &Type<'_>, b: &Type<'_>) -> bool {
    let pin = |ty: &Type<'_>| match ty {
        Type::Nullable(inner) if inner.boundary == AbsencePin::Undefined => AbsencePin::Undefined,
        _ => AbsencePin::Null,
    };
    pin(a) == pin(b)
}

pub(crate) fn may_be_absent(ty: &Type<'_>) -> bool {
    match ty {
        Type::Null | Type::Nullable(_) | Type::Dynamic | Type::Unknown | Type::TypeParameter(_) => {
            true
        }
        Type::Union(members) => members.iter().any(may_be_absent),
        _ => false,
    }
}

pub(crate) fn optional(ty: &Type<'_>) -> bool {
    matches!(ty, Type::Nullable(_) | Type::Null)
}

/// Payload traversal is also used for declared but never evaluated containers.
pub(crate) fn invalid_key(
    ty: &Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    let mut invalid = false;
    let result = type_payload::measure_payload(type_payload::Payload::Type(ty), budget, |node| {
        if let type_payload::Payload::Type(Type::Set(key) | Type::Map(key, _)) = node {
            invalid |= optional(key);
        }
        Ok::<_, std::convert::Infallible>(())
    });
    match result {
        Ok(_) => Ok(invalid),
        Err(type_payload::PayloadError::Allocation(error)) => Err(error),
        Err(type_payload::PayloadError::Visitor(never)) => match never {},
    }
}

pub(super) fn json_observes(
    ty: &Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    let mut observes = false;
    let result = type_payload::measure_payload(type_payload::Payload::Type(ty), budget, |node| {
        if let type_payload::Payload::Type(ty) = node {
            observes |= optional(ty);
        }
        Ok::<_, std::convert::Infallible>(())
    });
    match result {
        Ok(_) => Ok(observes),
        Err(type_payload::PayloadError::Allocation(error)) => Err(error),
        Err(type_payload::PayloadError::Visitor(never)) => match never {},
    }
}

pub(crate) fn observation<'ast, 'src>(
    view: CheckedView<'_, 'ast, 'src>,
    expression: &'ast Expr<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<Observation<'ast, 'src>>, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    let absent = |value: &Expr<'_, '_>| view.expression_type(value.id).is_some_and(optional);
    let text = |value: &'ast Expr<'ast, 'src>| Observation {
        span: value.span(),
        message: "string conversion of an optional value observes its absent spelling (R2)",
        help: "narrow first; `x ?? \"null\"` preserves the legacy text",
        fix: Fix::NullText(value),
    };
    let result = match &expression.kind {
        ExprKind::Call { callee, args, .. } => {
            let builtin = view.builtin_call(expression.id).or_else(|| view.dynamic_operation(expression.id));
            let intrinsic = view.resolved_intrinsic(expression.id).or_else(|| view.resolved_intrinsic(callee.id));
            if matches!(builtin, Some(BuiltinCall::Print | BuiltinCall::JsString)) {
                args.iter().find(|arg| absent(&arg.expression)).map(|arg| text(&arg.expression))
            } else if matches!(builtin, Some(BuiltinCall::JsonStringify)) {
                if let Some(arg) = args.first() {
                    if let Some(ty) = view.expression_type(arg.expression.id) {
                        if json_observes(ty, budget)? {
                            return Ok(Some(Observation { span: arg.span,
                                message: "JSON serialization observes optional values outside optional object fields (R2)",
                                help: "narrow optional values before serializing, or serialize a declared shape with optional keys",
                                fix: Fix::None }));
                        }
                    }
                }
                None
            } else if matches!(intrinsic, Some(ResolvedIntrinsic::Method(Intrinsic::ArrayIncludes | Intrinsic::ArrayIndexOf)))
                && args.first().is_some_and(|arg| absent(&arg.expression)) {
                let span = match &callee.kind {
                    ExprKind::Member { property, .. } => property.span,
                    _ => args[0].span,
                };
                Some(Observation { span,
                    message: "an array search for an optional value distinguishes absent spellings (R2)",
                    help: "narrow first, or use `findIndex` with `== null`", fix: Fix::None })
            } else { None }
        }
        ExprKind::Convert { target, value, .. } if matches!(target.kind, TypeKind::String) && absent(value) => Some(text(value)),
        ExprKind::Convert { target, value, .. } if matches!(target.kind, TypeKind::Float) && absent(value) => Some(Observation {
            span: value.span(), message: "number conversion of an optional value distinguishes absent spellings (R2)",
            help: "narrow or provide an explicit numeric default before conversion", fix: Fix::None,
        }),
        ExprKind::Template { parts, .. } => parts.iter().find_map(|part| match part {
            TemplatePart::Expr(value) if absent(value) => Some(text(value)),
            _ => None,
        }),
        ExprKind::Binary { op: BinaryOp::Add, lhs, rhs, .. }
            if view.expression_type(expression.id) == Some(&Type::String) => {
                if absent(lhs) { Some(text(lhs)) } else if absent(rhs) { Some(text(rhs)) } else { None }
            }
        ExprKind::DynamicUnary { op: DynamicUnaryOp::TypeOf, expr, .. } if absent(expr) => Some(Observation {
            span: expr.span(), message: "`typeof` on an optional value distinguishes absent spellings (R2)",
            help: "narrow with `== null` first", fix: Fix::None,
        }),
        ExprKind::DynamicBinary { op: op @ (DynamicBinaryOp::StrictEq | DynamicBinaryOp::StrictNotEq), lhs, rhs, .. }
            // JsValue === null remains a JavaScript spelling observation. Only a
            // typed optional operand has the source language's erased spelling.
            if [*lhs, *rhs].iter().any(|side| matches!(view.expression_type(side.id), Some(Type::Nullable(_))))
                || absent(lhs) && absent(rhs) => Some(Observation {
                    span: expression.span(), message: "strict equality on an optional value distinguishes absent spellings (R2)",
                    help: "`==` and `!=` treat both absent spellings alike",
                    fix: Fix::LooseEquality { lhs, rhs, equal: *op == DynamicBinaryOp::StrictEq },
                }),
        _ => None,
    };
    Ok(result)
}

pub(super) fn check(
    view: CheckedView<'_, '_, '_>,
    module: Option<usize>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AdmittedCheckError> {
    for symbol in view.symbols() {
        if view.declarations.symbol_modules[symbol.id.0 as usize] == module
            && invalid_key(&symbol.ty, budget)?
        {
            return Err(AdmittedCheckError::new(
                symbol.span,
                "optional Set elements and Map keys distinguish absent spellings (R2); use a present key type",
            ));
        }
    }
    for definition in view.structs() {
        if definition.module != module {
            continue;
        }
        for field in definition.fields.values() {
            if invalid_key(&field.ty, budget)? {
                return Err(AdmittedCheckError::new(
                    field.span,
                    "optional Set elements and Map keys distinguish absent spellings (R2); use a present key type",
                ));
            }
        }
    }
    for definition in view.classes() {
        if definition.module != module {
            continue;
        }
        for field in definition.fields.values() {
            if invalid_key(&field.ty, budget)? {
                return Err(AdmittedCheckError::new(
                    field.span,
                    "optional Set elements and Map keys distinguish absent spellings (R2); use a present key type",
                ));
            }
        }
    }
    let mut seen = budget.filled(AllocationClass::Scratch, view.type_count(), false)?;
    for info in &view.facts.source_info {
        if let Some(expression) = info.expression {
            if let Some(id) = view
                .expression_type_id(expression.id)
                .filter(|id| !seen[id.index()])
            {
                seen[id.index()] = true;
                if invalid_key(view.checked_type(id), budget)? {
                    return Err(AdmittedCheckError::new(
                        expression.span(),
                        "optional Set elements and Map keys distinguish absent spellings (R2); use a present key type",
                    ));
                }
            }
            if let Some(problem) = observation(view, expression, budget)? {
                return Err(AdmittedCheckError::new(
                    problem.span,
                    format!("{}; {}", problem.message, problem.help),
                ));
            }
        }
    }
    Ok(())
}
