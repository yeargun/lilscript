//! Evaluation of the checked enum operation, independent of target spelling.
use super::facts::{StoredExact as Exact, StoredString, Work};
use super::*;
use crate::primitive::EnumOperation;

pub(super) fn literal(value: Constant) -> Option<Exact> {
    Some(match value {
        Constant::Integer(value) => Exact::Integer(value),
        Constant::String(value) => Exact::String(StoredString::Source(value)),
        _ => return None,
    })
}

pub(super) fn evaluate(
    program: &Program<'_>,
    declaration: NominalId,
    operation: EnumOperation,
    left: &Exact,
    right: Option<&Exact>,
    work: &mut Work,
) -> Option<Exact> {
    if !work.charge(program.enums.len() as u64 + 1) {
        return None;
    }
    let definition = program.enum_definition(declaration)?;
    if operation == EnumOperation::Abi {
        return Some(left.clone());
    }
    if operation == EnumOperation::Has {
        let (Exact::Integer(value), Some(Exact::Integer(flags))) = (left, right) else {
            return None;
        };
        return Some(Exact::Boolean(value & flags == *flags));
    }
    if definition.abi == crate::ast::EnumAbi::Flags {
        let Exact::Integer(value) = left else {
            return None;
        };
        return Some(if (*value as u32) & !definition.flag_mask == 0 {
            left.clone()
        } else {
            Exact::Null
        });
    }
    for (ordinal, variant) in definition.variants.iter().enumerate() {
        if !work.charge(1) {
            return None;
        }
        let equal = match (variant.value, left) {
            (Constant::Integer(value), Exact::Integer(actual)) => value == *actual,
            (Constant::String(value), Exact::String(actual)) => {
                let value = &program.strings[value.index()];
                let actual = match actual {
                    StoredString::Source(id) => &program.strings[id.index()],
                    StoredString::Computed(value) => value,
                };
                if !work.charge((value.storage_bytes() + actual.storage_bytes()) as u64) {
                    return None;
                }
                value == actual
            }
            _ => false,
        };
        if equal {
            return if operation == EnumOperation::Ordinal {
                Some(Exact::Integer(ordinal as i32))
            } else {
                literal(variant.value)
            };
        }
    }
    (operation == EnumOperation::From).then_some(Exact::Null)
}
