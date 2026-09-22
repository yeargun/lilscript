//! Allocation-free facts about the result of an actual JavaScript Number
//! operation. This module knows neither source types nor callable identities.
//!
//! A raw result and its subsequent `|0` result are different facts. In
//! particular, a declared `int` or a call named `Math.imul` is not an input
//! proof. Callers must establish the Number domain from the actual producer.
//! These facts describe normal results only: they do not authorize removing,
//! duplicating or moving evaluation, or dropping a coercion with host effects.
//! No transfer chooses a literal representation or evaluates string payloads.
//!
//! Rules follow ECMAScript Number arithmetic and ToInt32:
//! https://tc39.es/ecma262/multipage/ecmascript-data-types-and-values.html#sec-numeric-types
//! https://tc39.es/ecma262/multipage/abstract-operations.html#sec-toint32

use crate::structured_js::{Binary, Unary};

const SAFE_INTEGER: i64 = 9_007_199_254_740_991;
const NUMBER: u8 = 1;
const POSITIVE_ZERO: u8 = 2;
const NEGATIVE_ZERO: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IntegerBounds {
    minimum: i64,
    maximum: i64,
}

/// A proved Number domain, optionally restricted to finite integral values in
/// the safe-integer interval. Zero signs are independent of those bounds.
/// Private fields keep invalid intervals and impossible zero flags out.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NumberFacts {
    integers: Option<IntegerBounds>,
    flags: u8,
}

impl Default for NumberFacts {
    fn default() -> Self {
        Self::UNKNOWN
    }
}

impl NumberFacts {
    /// The input may be an object, BigInt, string, or any other value.
    pub(crate) const UNKNOWN: Self = Self {
        integers: None,
        flags: 0,
    };
    /// Number is established, but fractions, infinities, NaN and both zeros
    /// remain possible. No source-type or host-call inference happens here.
    pub(crate) const NUMBER: Self = Self {
        integers: None,
        flags: NUMBER | POSITIVE_ZERO | NEGATIVE_ZERO,
    };
    /// The result of an actual signed-i32 normalization, excluding -0.
    pub(crate) const I32: Self = Self {
        integers: Some(IntegerBounds {
            minimum: i32::MIN as i64,
            maximum: i32::MAX as i64,
        }),
        flags: NUMBER | POSITIVE_ZERO,
    };

    pub(crate) fn literal(value: f64) -> Self {
        let mut result = Self {
            integers: None,
            flags: NUMBER,
        };
        if value == 0.0 {
            result.flags |= if value.is_sign_negative() {
                NEGATIVE_ZERO
            } else {
                POSITIVE_ZERO
            };
        }
        if value.is_finite() && value.fract() == 0.0 && value.abs() <= SAFE_INTEGER as f64 {
            result.integers = Some(IntegerBounds {
                minimum: value as i64,
                maximum: value as i64,
            });
        }
        result
    }

    /// The caller supplies an independently established integral range. A
    /// range containing zero admits +0, and admits -0 only when requested.
    pub(crate) fn integer_range(
        minimum: i64,
        maximum: i64,
        may_negative_zero: bool,
    ) -> Option<Self> {
        if minimum > maximum || minimum < -SAFE_INTEGER || maximum > SAFE_INTEGER {
            return None;
        }
        let zero = minimum <= 0 && maximum >= 0;
        if may_negative_zero && !zero {
            return None;
        }
        Some(Self {
            integers: Some(IntegerBounds { minimum, maximum }),
            flags: NUMBER
                | if zero { POSITIVE_ZERO } else { 0 }
                | if may_negative_zero { NEGATIVE_ZERO } else { 0 },
        })
    }

    pub(crate) fn is_number(self) -> bool {
        self.flags & NUMBER != 0
    }

    pub(crate) fn integer_bounds(self) -> Option<(i64, i64)> {
        self.integers.map(|range| (range.minimum, range.maximum))
    }

    pub(crate) fn may_negative_zero(self) -> bool {
        !self.is_number() || self.flags & NEGATIVE_ZERO != 0
    }

    /// Same Number value before and after ToInt32, including the zero sign.
    /// This also establishes that normalization itself cannot coerce a host
    /// value. Evaluation of the operand remains an independent obligation.
    pub(crate) fn normalization_redundant(self) -> bool {
        !self.may_negative_zero()
            && self.integers.is_some_and(|range| {
                range.minimum >= i32::MIN as i64 && range.maximum <= i32::MAX as i64
            })
    }

    /// Facts AFTER a real ToInt32 operation completes normally. Calling this
    /// does not prove that performing it is harmless or optional.
    pub(crate) fn to_int32(self) -> Self {
        if let Some(range) = self.integers {
            if range.minimum >= i32::MIN as i64 && range.maximum <= i32::MAX as i64 {
                return Self::integer_range(range.minimum, range.maximum, false).unwrap();
            }
            if range.minimum == range.maximum {
                return Self::literal((range.minimum as i32) as f64);
            }
        }
        Self::I32
    }

    pub(crate) fn join(self, other: Self) -> Self {
        if !self.is_number() || !other.is_number() {
            return Self::UNKNOWN;
        }
        Self {
            integers: self
                .integers
                .zip(other.integers)
                .map(|(a, b)| IntegerBounds {
                    minimum: a.minimum.min(b.minimum),
                    maximum: a.maximum.max(b.maximum),
                }),
            flags: self.flags | other.flags,
        }
    }

    pub(crate) fn unary(self, op: Unary) -> Self {
        if !self.is_number() {
            // Unary + uses ToNumber: BigInt and invalid host coercions throw,
            // but normal completion cannot return BigInt or a string.
            return if op == Unary::Plus {
                Self::NUMBER
            } else {
                Self::UNKNOWN
            };
        }
        match op {
            Unary::Delete => Self::UNKNOWN,
            Unary::Plus => self,
            Unary::Negate => Self {
                integers: self.integers.map(|range| IntegerBounds {
                    minimum: -range.maximum,
                    maximum: -range.minimum,
                }),
                flags: NUMBER
                    | if self.flags & POSITIVE_ZERO != 0 {
                        NEGATIVE_ZERO
                    } else {
                        0
                    }
                    | if self.flags & NEGATIVE_ZERO != 0 {
                        POSITIVE_ZERO
                    } else {
                        0
                    },
            },
            Unary::BitNot => {
                let (minimum, maximum) = self.to_int32().integer_bounds().unwrap();
                Self::integer_range(!(maximum as i32) as i64, !(minimum as i32) as i64, false)
                    .unwrap()
            }
            Unary::Not | Unary::TypeOf | Unary::Void => Self::UNKNOWN,
        }
    }

    /// Raw Number arithmetic only. No wrapping or truncation is inferred for
    /// `+`, `-`, `*`, `/` or `%`; use `to_int32` separately after emission.
    /// All work is bounded by a fixed number of scalar operations.
    pub(crate) fn binary(self, op: Binary, right: Self) -> Self {
        match op {
            Binary::Nullish => {
                return if self.is_number() {
                    self
                } else {
                    Self::UNKNOWN
                };
            }
            Binary::And | Binary::Or => return self.join(right),
            Binary::BitAnd
            | Binary::BitOr
            | Binary::BitXor
            | Binary::ShiftLeft
            | Binary::ShiftRight
            | Binary::UnsignedShiftRight => {
                // Mixed Number/BigInt throws. Consequently one proved Number
                // forces the other operand's Number branch on normal
                // completion. >>> rejects BigInt even with two unknowns.
                // The original coercions, lookup and throws remain required.
                return if self.is_number() || right.is_number() || op == Binary::UnsignedShiftRight
                {
                    self.bitwise(op, right)
                } else {
                    Self::UNKNOWN
                };
            }
            Binary::Less
            | Binary::LessEqual
            | Binary::Greater
            | Binary::GreaterEqual
            | Binary::StrictEqual
            | Binary::StrictNotEqual
            | Binary::Equal
            | Binary::NotEqual
            | Binary::In => return Self::UNKNOWN,
            _ => {}
        }
        if !self.is_number() || !right.is_number() {
            return if op != Binary::Add && (self.is_number() || right.is_number()) {
                Self::NUMBER
            } else {
                Self::UNKNOWN
            };
        }
        let Some((a, b)) = self.integers.zip(right.integers) else {
            return Self::NUMBER;
        };
        let negative_zero = self.product_negative_zero(right);
        match op {
            Binary::Add => Self::bounded(
                a.minimum as i128 + b.minimum as i128,
                a.maximum as i128 + b.maximum as i128,
                self.flags & NEGATIVE_ZERO != 0 && right.flags & NEGATIVE_ZERO != 0,
            ),
            Binary::Subtract => Self::bounded(
                a.minimum as i128 - b.maximum as i128,
                a.maximum as i128 - b.minimum as i128,
                self.flags & NEGATIVE_ZERO != 0 && right.flags & POSITIVE_ZERO != 0,
            ),
            Binary::Multiply => {
                let products = [
                    a.minimum as i128 * b.minimum as i128,
                    a.minimum as i128 * b.maximum as i128,
                    a.maximum as i128 * b.minimum as i128,
                    a.maximum as i128 * b.maximum as i128,
                ];
                Self::bounded(
                    *products.iter().min().unwrap(),
                    *products.iter().max().unwrap(),
                    negative_zero,
                )
            }
            Binary::Divide if b.minimum > 0 || b.maximum < 0 => {
                if b.minimum == 1 && b.maximum == 1 {
                    self
                } else if b.minimum == -1 && b.maximum == -1 {
                    self.unary(Unary::Negate)
                } else if a.minimum == a.maximum && b.minimum == b.maximum {
                    let result = Self::literal(a.minimum as f64 / b.minimum as f64);
                    if result.integer_bounds() == Some((0, 0)) {
                        Self::bounded(0, 0, negative_zero)
                    } else {
                        result
                    }
                } else if a.minimum == 0 && a.maximum == 0 {
                    Self::bounded(0, 0, negative_zero)
                } else {
                    Self::NUMBER
                }
            }
            Binary::Remainder if b.minimum > 0 || b.maximum < 0 => {
                let bound = b.minimum.abs().max(b.maximum.abs()) - 1;
                Self::bounded(
                    a.minimum.min(0).max(-bound) as i128,
                    a.maximum.max(0).min(bound) as i128,
                    a.minimum < 0 || self.flags & NEGATIVE_ZERO != 0,
                )
            }
            _ => Self::NUMBER,
        }
    }

    fn bounded(minimum: i128, maximum: i128, negative_zero: bool) -> Self {
        if minimum < -(SAFE_INTEGER as i128) || maximum > SAFE_INTEGER as i128 {
            return Self::NUMBER;
        }
        Self::integer_range(minimum as i64, maximum as i64, negative_zero).unwrap_or(Self::NUMBER)
    }

    fn product_negative_zero(self, right: Self) -> bool {
        let negative = |value: Self| {
            value.flags & NEGATIVE_ZERO != 0
                || value.integers.is_some_and(|range| range.minimum < 0)
        };
        let positive = |value: Self| {
            value.flags & POSITIVE_ZERO != 0
                || value.integers.is_some_and(|range| range.maximum > 0)
        };
        (self.flags & POSITIVE_ZERO != 0 && negative(right))
            || (self.flags & NEGATIVE_ZERO != 0 && positive(right))
            || (right.flags & POSITIVE_ZERO != 0 && negative(self))
            || (right.flags & NEGATIVE_ZERO != 0 && positive(self))
    }

    fn bitwise(self, op: Binary, right: Self) -> Self {
        let (a_min, a_max) = self.to_int32().integer_bounds().unwrap();
        let (b_min, b_max) = right.to_int32().integer_bounds().unwrap();
        let shift = (b_min == b_max).then_some(b_min as u32 & 31);
        let range = |min, max| Self::integer_range(min, max, false).unwrap();
        if a_min == a_max && b_min == b_max {
            let a = a_min as i32;
            let b = b_min as i32;
            return Self::literal(match op {
                Binary::BitAnd => (a & b) as f64,
                Binary::BitOr => (a | b) as f64,
                Binary::BitXor => (a ^ b) as f64,
                Binary::ShiftLeft => a.wrapping_shl(b as u32 & 31) as f64,
                Binary::ShiftRight => (a >> (b as u32 & 31)) as f64,
                Binary::UnsignedShiftRight => ((a as u32) >> (b as u32 & 31)) as f64,
                _ => unreachable!("bitwise transfer receives only bitwise operations"),
            });
        }
        match op {
            Binary::BitAnd if a_min >= 0 || b_min >= 0 => {
                let a_bound = if a_min >= 0 { a_max } else { i32::MAX as i64 };
                let b_bound = if b_min >= 0 { b_max } else { i32::MAX as i64 };
                range(0, a_bound.min(b_bound))
            }
            Binary::BitOr | Binary::BitXor if a_min >= 0 && b_min >= 0 => range(0, i32::MAX as i64),
            Binary::ShiftRight if shift.is_some() => {
                let shift = shift.unwrap();
                range(
                    (a_min as i32 >> shift) as i64,
                    (a_max as i32 >> shift) as i64,
                )
            }
            Binary::ShiftLeft if shift.is_some() => {
                let factor = 1i64 << shift.unwrap();
                let (minimum, maximum) = (a_min * factor, a_max * factor);
                if minimum >= i32::MIN as i64 && maximum <= i32::MAX as i64 {
                    range(minimum, maximum)
                } else {
                    Self::I32
                }
            }
            Binary::UnsignedShiftRight => {
                let (minimum, maximum) = if a_min >= 0 || a_max < 0 {
                    (a_min as u32 as i64, a_max as u32 as i64)
                } else {
                    (0, u32::MAX as i64)
                };
                shift.map_or_else(
                    || range(0, maximum),
                    |shift| range(minimum >> shift, maximum >> shift),
                )
            }
            _ => Self::I32,
        }
    }
}
