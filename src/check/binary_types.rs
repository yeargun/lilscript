//! One binary/common-type algorithm for source checking and admitted edits.
//!
//! The caller owns all temporary construction storage. Its query scope must
//! outlive the returned Type and must release charges only after actual values
//! have been dropped. No relation answer or allocation journal lives here.

use super::type_relation::{type_equal_with, RelationAdmission, RelationEvent, Unmetered};
use super::{
    binary_op_name, common_numeric_type, is_js_value, nullish_present_type, BinaryOp, CheckError,
    Span, Type,
};
use std::convert::Infallible;

/// Mechanical construction operations, never a second type evaluator.
///
/// Implementations admit clone visits and the storage they actually construct
/// before cloning/allocating. FunctionType clones retain the shared Arc. Box
/// and Vec payloads require real storage. Growth must retain the old buffer's
/// allowance until the admitted replacement has been allocated and moved.
/// Capacity/allocation/budget errors belong to RelationAdmission::Error.
///
/// An admitted clone's allowance also covers dropping that temporary on any
/// later failure: cleanup cannot request additional work after exhaustion.
pub(crate) trait TypeConstructionAdmission: RelationAdmission {
    fn clone_type<'src>(&mut self, value: &Type<'src>) -> Result<Type<'src>, Self::Error>;
    fn box_type<'src>(&mut self, value: Type<'src>) -> Result<Box<Type<'src>>, Self::Error>;
    fn push_type<'src>(
        &mut self,
        values: &mut Vec<Type<'src>>,
        value: Type<'src>,
    ) -> Result<(), Self::Error>;
}

impl TypeConstructionAdmission for Unmetered {
    fn clone_type<'src>(&mut self, value: &Type<'src>) -> Result<Type<'src>, Infallible> {
        Ok(value.clone())
    }
    fn box_type<'src>(&mut self, value: Type<'src>) -> Result<Box<Type<'src>>, Infallible> {
        Ok(Box::new(value))
    }
    fn push_type<'src>(
        &mut self,
        values: &mut Vec<Type<'src>>,
        value: Type<'src>,
    ) -> Result<(), Infallible> {
        values.push(value);
        Ok(())
    }
}

/// Fixed metadata only. A verifier may reject the operation without rendering
/// the source diagnostic or copying either potentially large operand type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BinaryTypeReason {
    InvalidOperands,
    NullishLeftRequired,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum BinaryTypeError<E> {
    Semantic(BinaryTypeReason),
    Admission(E),
}

impl<E> From<E> for BinaryTypeError<E> {
    fn from(value: E) -> Self {
        Self::Admission(value)
    }
}

#[inline]
fn work<A: RelationAdmission>(admission: &mut A, units: usize) -> Result<(), A::Error> {
    admission.admit(RelationEvent::TypeWork(units))
}

pub(crate) fn checked_binary_type_with<'src, A: TypeConstructionAdmission>(
    op: BinaryOp,
    lhs: &Type<'src>,
    rhs: &Type<'src>,
    admission: &mut A,
) -> Result<Type<'src>, BinaryTypeError<A::Error>> {
    work(admission, 1)?;
    match op {
        BinaryOp::Add if matches!(lhs, Type::String) || matches!(rhs, Type::String) => {
            if is_stringable_with(lhs, admission)? && is_stringable_with(rhs, admission)? {
                return Ok(Type::String);
            }
        }
        BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div
            if lhs.is_numeric() && rhs.is_numeric() =>
        {
            return Ok(common_numeric_type(lhs, rhs));
        }
        BinaryOp::Mod
        | BinaryOp::BitAnd
        | BinaryOp::BitOr
        | BinaryOp::Xor
        | BinaryOp::ShiftLeft
        | BinaryOp::ShiftRight
        | BinaryOp::UnsignedShiftRight
            if matches!(lhs, Type::Int) && matches!(rhs, Type::Int) =>
        {
            return Ok(Type::Int);
        }
        BinaryOp::Eq | BinaryOp::NotEq => {
            if equality_comparable_with(lhs, rhs, admission)? {
                return Ok(Type::Bool);
            }
        }
        BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq
            if (lhs.is_numeric() && rhs.is_numeric())
                || (matches!(lhs, Type::String) && matches!(rhs, Type::String)) =>
        {
            return Ok(Type::Bool);
        }
        BinaryOp::And | BinaryOp::Or if matches!(lhs, Type::Bool) && matches!(rhs, Type::Bool) => {
            return Ok(Type::Bool);
        }
        BinaryOp::Nullish => {
            if matches!(lhs, Type::Null) {
                return Ok(admission.clone_type(rhs)?);
            }
            let present = nullish_present_type(lhs).ok_or(BinaryTypeError::Semantic(
                BinaryTypeReason::NullishLeftRequired,
            ))?;
            return common_type_with(present, rhs, admission)?
                .ok_or(BinaryTypeError::Semantic(BinaryTypeReason::InvalidOperands));
        }
        _ => {}
    }
    Err(BinaryTypeError::Semantic(BinaryTypeReason::InvalidOperands))
}

pub(crate) fn common_type_with<'src, A: TypeConstructionAdmission>(
    lhs: &Type<'src>,
    rhs: &Type<'src>,
    admission: &mut A,
) -> Result<Option<Type<'src>>, A::Error> {
    work(admission, 1)?;
    if type_equal_with(lhs, rhs, admission)? {
        return admission.clone_type(lhs).map(Some);
    }
    if lhs.is_numeric() && rhs.is_numeric() {
        return Ok(Some(common_numeric_type(lhs, rhs)));
    }
    let (left, right, wrapper): (_, _, fn(Box<Type<'src>>) -> Type<'src>) = match (lhs, rhs) {
        (Type::Nullable(inner), Type::Null) | (Type::Null, Type::Nullable(inner)) => {
            let inner = admission.clone_type(inner)?;
            return Ok(Some(Type::Nullable(admission.box_type(inner)?)));
        }
        (Type::Null, other) | (other, Type::Null) if !matches!(other, Type::Null | Type::Void) => {
            let inner = admission.clone_type(other)?;
            return Ok(Some(Type::Nullable(admission.box_type(inner)?)));
        }
        (Type::Nullable(left), Type::Nullable(right)) => {
            (left.as_ref(), right.as_ref(), Type::Nullable)
        }
        (Type::Nullable(nullable), other) | (other, Type::Nullable(nullable)) => {
            (nullable.as_ref(), other, Type::Nullable)
        }
        (Type::Array(left), Type::Array(right)) => (left.as_ref(), right.as_ref(), Type::Array),
        (Type::Record(left), Type::Record(right)) => {
            if type_equal_with(left, right, admission)? {
                let inner = admission.clone_type(left)?;
                return Ok(Some(Type::Record(admission.box_type(inner)?)));
            }
            return union_pair(lhs, rhs, admission).map(Some);
        }
        (Type::Task(left), Type::Task(right)) => (left.as_ref(), right.as_ref(), Type::Task),
        (Type::Generator(left), Type::Generator(right)) => {
            (left.as_ref(), right.as_ref(), Type::Generator)
        }
        _ if !matches!(lhs, Type::Void | Type::GenericFunction(_))
            && !matches!(rhs, Type::Void | Type::GenericFunction(_)) =>
        {
            return union_pair(lhs, rhs, admission).map(Some);
        }
        _ => return Ok(None),
    };
    match common_type_with(left, right, admission)? {
        Some(inner) => Ok(Some(wrapper(admission.box_type(inner)?))),
        None => Ok(None),
    }
}

fn union_pair<'src, A: TypeConstructionAdmission>(
    lhs: &Type<'src>,
    rhs: &Type<'src>,
    admission: &mut A,
) -> Result<Type<'src>, A::Error> {
    let mut members = Vec::new();
    let lhs = admission.clone_type(lhs)?;
    admission.push_type(&mut members, lhs)?;
    let rhs = admission.clone_type(rhs)?;
    admission.push_type(&mut members, rhs)?;
    normalize_union_with(members, admission)
}

pub(crate) fn normalize_union_with<'src, A: TypeConstructionAdmission>(
    members: Vec<Type<'src>>,
    admission: &mut A,
) -> Result<Type<'src>, A::Error> {
    work(admission, 1)?;
    let mut flattened = Vec::new();
    for member in members {
        append_union_member_with(&mut flattened, member, admission)?;
    }
    let mut nullable = false;
    for member in &flattened {
        work(admission, 1)?;
        if matches!(member, Type::Nullable(_)) {
            nullable = true;
            break;
        }
    }
    if nullable {
        // Pay the full in-place scan/compaction before invoking retain; it
        // performs no allocation and examines only Type discriminants.
        work(admission, flattened.len())?;
        flattened.retain(|member| !matches!(member, Type::Null));
    }
    if flattened.len() == 2 {
        let mut null = None;
        for (position, member) in flattened.iter().enumerate() {
            work(admission, 1)?;
            if matches!(member, Type::Null) {
                null = Some(position);
                break;
            }
        }
        if let Some(null) = null {
            let position = 1 - null;
            work(admission, flattened.len() - position - 1)?;
            let inner = flattened.remove(position);
            return Ok(Type::Nullable(admission.box_type(inner)?));
        }
    }
    if flattened.len() == 1 {
        Ok(flattened.pop().expect("one union member remains"))
    } else {
        Ok(Type::Union(flattened))
    }
}

fn append_union_member_with<'src, A: TypeConstructionAdmission>(
    flattened: &mut Vec<Type<'src>>,
    member: Type<'src>,
    admission: &mut A,
) -> Result<(), A::Error> {
    work(admission, 1)?;
    if let Type::Union(nested) = member {
        for member in nested {
            append_union_member_with(flattened, member, admission)?;
        }
    } else {
        for previous in flattened.iter() {
            if type_equal_with(previous, &member, admission)? {
                return Ok(());
            }
        }
        admission.push_type(flattened, member)?;
    }
    Ok(())
}

pub(crate) fn equality_comparable_with<A: RelationAdmission>(
    lhs: &Type<'_>,
    rhs: &Type<'_>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    work(admission, 1)?;
    match (lhs, rhs) {
        (Type::Null, Type::Null)
        | (Type::Null, Type::Nullable(_))
        | (Type::Nullable(_), Type::Null) => Ok(true),
        (Type::Nullable(lhs), Type::Nullable(rhs)) => equality_comparable_with(lhs, rhs, admission),
        (Type::Nullable(lhs), rhs) => equality_comparable_with(lhs, rhs, admission),
        (lhs, Type::Nullable(rhs)) => equality_comparable_with(lhs, rhs, admission),
        (Type::Union(lhs), Type::Union(rhs)) => {
            for lhs in lhs {
                // The outer iteration still runs when the right union is
                // empty; charging only recursive pairs would miss that work.
                work(admission, 1)?;
                for rhs in rhs {
                    if equality_comparable_with(lhs, rhs, admission)? {
                        return Ok(true);
                    }
                }
            }
            Ok(false)
        }
        (Type::Union(lhs), rhs) => {
            for lhs in lhs {
                if equality_comparable_with(lhs, rhs, admission)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (lhs, Type::Union(rhs)) => {
            for rhs in rhs {
                if equality_comparable_with(lhs, rhs, admission)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (lhs, rhs) if is_js_value(lhs) || is_js_value(rhs) => {
            let other = if is_js_value(lhs) { rhs } else { lhs };
            Ok(matches!(
                other,
                Type::Null | Type::Bool | Type::String | Type::Int | Type::Float
            ) || is_js_value(other))
        }
        _ => Ok(
            (type_equal_with(lhs, rhs, admission)? || (lhs.is_numeric() && rhs.is_numeric()))
                && equality_type_supported_with(lhs, admission)?
                && equality_type_supported_with(rhs, admission)?,
        ),
    }
}

fn equality_type_supported_with<A: RelationAdmission>(
    ty: &Type<'_>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    work(admission, 1)?;
    match ty {
        Type::Union(members) => {
            for member in members {
                if !equality_type_supported_with(member, admission)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        Type::Null
        | Type::Struct(_)
        | Type::StructInstance { .. }
        | Type::GenericFunction(_)
        | Type::Void => Ok(false),
        _ => Ok(true),
    }
}

pub(crate) fn is_stringable_with<A: RelationAdmission>(
    ty: &Type<'_>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    work(admission, 1)?;
    match ty {
        Type::Union(members) => {
            for member in members {
                if !is_stringable_with(member, admission)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(
            matches!(ty, Type::String | Type::Int | Type::Float | Type::Bool) || is_js_value(ty),
        ),
    }
}

pub(super) fn checked_binary_type_plain<'src>(
    op: BinaryOp,
    lhs: &Type<'src>,
    rhs: &Type<'src>,
    span: Span,
) -> Result<Type<'src>, CheckError> {
    match checked_binary_type_with(op, lhs, rhs, &mut Unmetered) {
        Ok(ty) => Ok(ty),
        Err(BinaryTypeError::Semantic(reason)) => Err(source_error(reason, op, lhs, rhs, span)),
        Err(BinaryTypeError::Admission(never)) => match never {},
    }
}

/// This explicit source/inspection boundary is the only diagnostic allocator.
fn source_error(
    reason: BinaryTypeReason,
    op: BinaryOp,
    lhs: &Type<'_>,
    rhs: &Type<'_>,
    span: Span,
) -> CheckError {
    CheckError::new(
        span,
        match reason {
            BinaryTypeReason::InvalidOperands => format!(
                "operator `{}` cannot be applied to `{lhs}` and `{rhs}`",
                binary_op_name(op)
            ),
            BinaryTypeReason::NullishLeftRequired => {
                format!("operator `??` requires a nullable left operand, found `{lhs}`")
            }
        },
    )
}

pub(super) fn common_type_plain<'src>(lhs: &Type<'src>, rhs: &Type<'src>) -> Option<Type<'src>> {
    infallible(common_type_with(lhs, rhs, &mut Unmetered))
}
pub(super) fn normalize_union_plain<'src>(members: Vec<Type<'src>>) -> Type<'src> {
    infallible(normalize_union_with(members, &mut Unmetered))
}
pub(super) fn equality_comparable_plain(lhs: &Type<'_>, rhs: &Type<'_>) -> bool {
    infallible(equality_comparable_with(lhs, rhs, &mut Unmetered))
}
pub(super) fn is_stringable_plain(ty: &Type<'_>) -> bool {
    infallible(is_stringable_with(ty, &mut Unmetered))
}
fn infallible<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

#[cfg(test)]
#[path = "binary_types_tests.rs"]
mod tests;
