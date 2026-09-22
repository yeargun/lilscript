// Test-only frozen prior helpers; never linked in production.
// Extracted from src/semantic.rs SHA256 039727514ec0e0119ff516f209ac6c25b8cb1d9d234f6aecbd26be24d5824bc1.
use super::*;

pub(super) fn old_checked_binary_type<'src>(
    op: BinaryOp,
    lhs: &Type<'src>,
    rhs: &Type<'src>,
    span: Span,
) -> Result<Type<'src>, SemanticError> {
    match op {
        BinaryOp::Add if lhs == &Type::String || rhs == &Type::String => {
            if old_is_stringable(lhs) && old_is_stringable(rhs) {
                Ok(Type::String)
            } else {
                Err(old_invalid_binary(op, lhs, rhs, span))
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            if lhs.is_numeric() && rhs.is_numeric() =>
        {
            Ok(common_numeric_type(lhs, rhs))
        }
        BinaryOp::Mod
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::Xor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::UnsignedShiftRight
            if lhs == &Type::Int && rhs == &Type::Int =>
        {
            Ok(Type::Int)
        }
        BinaryOp::Eq | BinaryOp::NotEq if old_equality_comparable(lhs, rhs) => Ok(Type::Bool),
        BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq
            if (lhs.is_numeric() && rhs.is_numeric())
                || (lhs == &Type::String && rhs == &Type::String) =>
        {
            Ok(Type::Bool)
        }
        BinaryOp::And | BinaryOp::Or if lhs == &Type::Bool && rhs == &Type::Bool => Ok(Type::Bool),
        BinaryOp::Nullish => {
            if lhs == &Type::Null {
                return Ok(rhs.clone());
            }
            let present = nullish_present_type(lhs).ok_or_else(|| {
                SemanticError::new(
                    span,
                    format!("operator `??` requires a nullable left operand, found `{lhs}`"),
                )
            })?;
            old_common_type(present, rhs).ok_or_else(|| old_invalid_binary(op, lhs, rhs, span))
        }
        _ => Err(old_invalid_binary(op, lhs, rhs, span)),
    }
}

pub(super) fn old_common_type<'src>(lhs: &Type<'src>, rhs: &Type<'src>) -> Option<Type<'src>> {
    if lhs == rhs {
        return Some(lhs.clone());
    }
    if lhs.is_numeric() && rhs.is_numeric() {
        return Some(common_numeric_type(lhs, rhs));
    }
    match (lhs, rhs) {
        (Type::Nullable(inner), Type::Null) | (Type::Null, Type::Nullable(inner)) => {
            Some(Type::Nullable(inner.clone()))
        }
        (Type::Null, other) | (other, Type::Null) if !matches!(other, Type::Null | Type::Void) => {
            Some(Type::Nullable(Box::new(other.clone())))
        }
        (Type::Nullable(lhs), Type::Nullable(rhs)) => {
            old_common_type(lhs, rhs).map(|inner| Type::Nullable(Box::new(inner)))
        }
        (Type::Nullable(nullable), other) | (other, Type::Nullable(nullable)) => {
            old_common_type(nullable, other).map(|inner| Type::Nullable(Box::new(inner)))
        }
        (Type::Array(lhs), Type::Array(rhs)) => {
            old_common_type(lhs, rhs).map(|element| Type::Array(Box::new(element)))
        }
        (Type::Record(lhs), Type::Record(rhs)) if lhs == rhs => Some(Type::Record(lhs.clone())),
        (Type::Task(lhs), Type::Task(rhs)) => {
            old_common_type(lhs, rhs).map(|value| Type::Task(Box::new(value)))
        }
        (Type::Generator(lhs), Type::Generator(rhs)) => {
            old_common_type(lhs, rhs).map(|value| Type::Generator(Box::new(value)))
        }
        _ if !matches!(lhs, Type::Void | Type::GenericFunction(_))
            && !matches!(rhs, Type::Void | Type::GenericFunction(_)) =>
        {
            Some(old_normalize_union(vec![lhs.clone(), rhs.clone()]))
        }
        _ => None,
    }
}

pub(super) fn old_normalize_union<'src>(members: Vec<Type<'src>>) -> Type<'src> {
    let mut flattened = Vec::new();
    for member in members {
        old_append_union_member(&mut flattened, member);
    }
    if flattened
        .iter()
        .any(|member| matches!(member, Type::Nullable(_)))
    {
        flattened.retain(|member| member != &Type::Null);
    }
    if flattened.len() == 2 {
        let null = flattened.iter().position(|member| member == &Type::Null);
        if let Some(null) = null {
            let inner = flattened.remove(1 - null);
            return Type::Nullable(Box::new(inner));
        }
    }
    if flattened.len() == 1 {
        flattened.pop().expect("one union member remains")
    } else {
        Type::Union(flattened)
    }
}

pub(super) fn old_append_union_member<'src>(flattened: &mut Vec<Type<'src>>, member: Type<'src>) {
    if let Type::Union(nested) = member {
        for member in nested {
            old_append_union_member(flattened, member);
        }
    } else if !flattened.contains(&member) {
        flattened.push(member);
    }
}

pub(super) fn old_equality_comparable(lhs: &Type<'_>, rhs: &Type<'_>) -> bool {
    match (lhs, rhs) {
        (Type::Null, Type::Null)
        | (Type::Null, Type::Nullable(_))
        | (Type::Nullable(_), Type::Null) => true,
        (Type::Nullable(lhs), Type::Nullable(rhs)) => old_equality_comparable(lhs, rhs),
        (Type::Nullable(lhs), rhs) => old_equality_comparable(lhs, rhs),
        (lhs, Type::Nullable(rhs)) => old_equality_comparable(lhs, rhs),
        (Type::Union(lhs), Type::Union(rhs)) => lhs
            .iter()
            .any(|lhs| rhs.iter().any(|rhs| old_equality_comparable(lhs, rhs))),
        (Type::Union(lhs), rhs) => lhs.iter().any(|lhs| old_equality_comparable(lhs, rhs)),
        (lhs, Type::Union(rhs)) => rhs.iter().any(|rhs| old_equality_comparable(lhs, rhs)),
        (lhs, rhs) if is_js_value(lhs) || is_js_value(rhs) => {
            let other = if is_js_value(lhs) { rhs } else { lhs };
            matches!(
                other,
                Type::TypeParameter("$js")
                    | Type::Null
                    | Type::Bool
                    | Type::String
                    | Type::Int
                    | Type::Float
            ) || is_js_value(other)
        }
        _ => {
            (lhs == rhs || (lhs.is_numeric() && rhs.is_numeric()))
                && old_equality_type_supported(lhs)
                && old_equality_type_supported(rhs)
        }
    }
}

pub(super) fn old_equality_type_supported(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(members) => members.iter().all(old_equality_type_supported),
        Type::Null
        | Type::Struct(_)
        | Type::StructInstance { .. }
        | Type::GenericFunction(_)
        | Type::Void => false,
        Type::Function(_) => true,
        _ => true,
    }
}

pub(super) fn old_is_stringable(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(members) => members.iter().all(old_is_stringable),
        _ => matches!(
            ty,
            Type::String | Type::Int | Type::Float | Type::Bool | Type::TypeParameter("$js")
        ),
    }
}

pub(super) fn old_invalid_binary(
    op: BinaryOp,
    lhs: &Type<'_>,
    rhs: &Type<'_>,
    span: Span,
) -> SemanticError {
    SemanticError::new(
        span,
        format!(
            "operator `{}` cannot be applied to `{lhs}` and `{rhs}`",
            binary_op_name(op)
        ),
    )
}
