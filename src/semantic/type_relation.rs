//! One assignability algorithm for plain checking and admitted verification.
//!
//! Invariant is exactly assignability in both directions. Carrying that mode
//! through invariant children avoids evaluating the same symmetric requirement
//! twice at every Array or callable-parameter layer. No relation answer survives
//! the query, and existing Type/DefaultValue equality remains authoritative.

use super::{DefaultValue, FunctionSignature, Type};
use std::convert::Infallible;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelationMode {
    Assignable,
    Invariant,
}

/// Admission occurs before the named work. Equality events must admit the
/// potentially deep existing equality operation, including names and defaults;
/// a single unit charge is not generally its cost. SignatureValidation admits
/// the existing parameter-record scan. ParameterPair admits one paired record.
#[derive(Clone, Copy, Debug)]
pub(crate) enum RelationEvent<'a, 'src> {
    /// Shared non-relation type algorithms admit their ordinary traversal
    /// steps here; construction/storage still belongs to their caller.
    TypeWork(usize),
    Visit {
        mode: RelationMode,
        expected: &'a Type<'src>,
        actual: &'a Type<'src>,
    },
    TypeEquality {
        left: &'a Type<'src>,
        right: &'a Type<'src>,
    },
    DefaultEquality {
        left: &'a DefaultValue<'src>,
        right: &'a DefaultValue<'src>,
    },
    SignatureValidation(&'a FunctionSignature<'src>),
    ParameterPair,
}

pub(crate) trait RelationAdmission {
    type Error;

    fn admit(&mut self, event: RelationEvent<'_, '_>) -> Result<(), Self::Error>;
}

/// Plain source/inspection wrappers share the same infallible adapter.
pub(crate) struct Unmetered;

impl RelationAdmission for Unmetered {
    type Error = Infallible;

    #[inline]
    fn admit(&mut self, _: RelationEvent<'_, '_>) -> Result<(), Infallible> {
        Ok(())
    }
}

/// Reuse the existing structural equality after admitting its complete work.
pub(crate) fn type_equal_with<A: RelationAdmission>(
    left: &Type<'_>,
    right: &Type<'_>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    admission.admit(RelationEvent::TypeEquality { left, right })?;
    Ok(left == right)
}

/// A refused check is an error, never an incompatible type result. The caller
/// owns temporary admission storage and its policy; this module owns neither.
pub(crate) fn is_type_assignable_with<A: RelationAdmission>(
    expected: &Type<'_>,
    actual: &Type<'_>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    relate(expected, actual, RelationMode::Assignable, admission)
}

pub(super) fn is_type_assignable_plain(expected: &Type<'_>, actual: &Type<'_>) -> bool {
    match is_type_assignable_with(expected, actual, &mut Unmetered) {
        Ok(result) => result,
        Err(never) => match never {},
    }
}

fn relate<A: RelationAdmission>(
    expected: &Type<'_>,
    actual: &Type<'_>,
    mode: RelationMode,
    admission: &mut A,
) -> Result<bool, A::Error> {
    admission.admit(RelationEvent::Visit {
        mode,
        expected,
        actual,
    })?;
    if type_equal_with(expected, actual, admission)? {
        // Preserve this shortcut even for equal invalid function signatures.
        // Admission/full source validation owns their structural legality.
        return Ok(true);
    }
    unequal(expected, actual, mode, admission)
}

/// Use only with an equality failure established by the current comparison,
/// or propagated through the matching one-field Type constructors below.
fn relate_known_unequal<A: RelationAdmission>(
    expected: &Type<'_>,
    actual: &Type<'_>,
    mode: RelationMode,
    admission: &mut A,
) -> Result<bool, A::Error> {
    admission.admit(RelationEvent::Visit {
        mode,
        expected,
        actual,
    })?;
    unequal(expected, actual, mode, admission)
}

fn unequal<A: RelationAdmission>(
    expected: &Type<'_>,
    actual: &Type<'_>,
    mode: RelationMode,
    admission: &mut A,
) -> Result<bool, A::Error> {
    match (expected, actual) {
        (Type::Array(expected), Type::Array(actual)) => {
            // Failed derived equality of matching Array wrappers proves their
            // children unequal. One invariant descent pays every relation
            // visit without repeating either equality or both directions.
            return relate_known_unequal(expected, actual, RelationMode::Invariant, admission);
        }
        (Type::Task(expected), Type::Task(actual))
        | (Type::Generator(expected), Type::Generator(actual))
        | (Type::Nullable(expected), Type::Nullable(actual)) => {
            return relate_known_unequal(expected, actual, mode, admission);
        }
        (Type::Record(_), Type::Record(_))
        | (Type::Map(_, _), Type::Map(_, _))
        | (Type::Set(_), Type::Set(_)) => {
            // These relations require exactly the same child equalities as
            // their derived wrapper equality, which already returned false.
            return Ok(false);
        }
        (Type::Function(expected), Type::Function(actual)) => {
            return functions(expected, actual, mode, admission);
        }
        _ => {}
    }

    if mode == RelationMode::Invariant {
        // Equality is symmetric and is already false. Preserve both original
        // coverage directions for unions and asymmetric nullable/JsValue rules.
        if !relate_known_unequal(expected, actual, RelationMode::Assignable, admission)? {
            return Ok(false);
        }
        return relate_known_unequal(actual, expected, RelationMode::Assignable, admission);
    }

    match (expected, actual) {
        (Type::TypeParameter("$js"), _) => Ok(!actual.is_void()),
        (Type::Float, Type::Int) => Ok(true),
        (Type::Nullable(_), Type::Null) => Ok(true),
        (Type::Nullable(expected), actual) => {
            relate(expected, actual, RelationMode::Assignable, admission)
        }
        (Type::Union(expected), Type::Union(actual)) => {
            for actual in actual {
                let mut covered = false;
                for expected in expected {
                    if relate(expected, actual, RelationMode::Assignable, admission)? {
                        covered = true;
                        break;
                    }
                }
                if !covered {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        (Type::Union(expected), actual) => {
            for expected in expected {
                if relate(expected, actual, RelationMode::Assignable, admission)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        (expected, Type::Union(actual)) => {
            for actual in actual {
                if !relate(expected, actual, RelationMode::Assignable, admission)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn functions<A: RelationAdmission>(
    expected: &FunctionSignature<'_>,
    actual: &FunctionSignature<'_>,
    mode: RelationMode,
    admission: &mut A,
) -> Result<bool, A::Error> {
    if expected.params.len() != actual.params.len() {
        return Ok(false);
    }
    admission.admit(RelationEvent::SignatureValidation(expected))?;
    if expected.validate_parameters().is_err() {
        return Ok(false);
    }
    admission.admit(RelationEvent::SignatureValidation(actual))?;
    if actual.validate_parameters().is_err() {
        return Ok(false);
    }
    for (expected, actual) in expected.params.iter().zip(&actual.params) {
        admission.admit(RelationEvent::ParameterPair)?;
        if expected.passing != actual.passing
            || !relate(&expected.ty, &actual.ty, RelationMode::Invariant, admission)?
        {
            return Ok(false);
        }
        match (&expected.default, &actual.default) {
            (Some(left), Some(right)) => {
                admission.admit(RelationEvent::DefaultEquality { left, right })?;
                if left != right {
                    return Ok(false);
                }
            }
            (Some(_), None) => return Ok(false),
            (None, Some(_)) if mode == RelationMode::Invariant => return Ok(false),
            _ => {}
        }
    }
    relate(&expected.return_type, &actual.return_type, mode, admission)
}

#[cfg(test)]
#[path = "type_relation_tests.rs"]
mod tests;
