//! Concrete C11 recipes for the shared primitive identities. No source/CFG
//! analysis, target graph, allocation or runtime library discovery lives here.
//! The caller admits emitted bytes and invokes ls_runtime_init before source
//! initialization. Its qualified C driver also fixes -fno-fast-math and
//! -ffp-contract=off; arbitrary command-line floating-point modes are not an ABI.

/// Common scalar ABI and bounded execution qualification. FP_CONTRACT is an
/// additional compiler-supported guard: separate volatile ls_f64 boundaries
/// and the qualified driver flags carry the actual operation-rounding contract.
pub(super) const PROLOGUE: &str = r#"#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <inttypes.h>
#include <limits.h>
#include <float.h>
#include <fenv.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#if !defined(__STDC_VERSION__) || __STDC_VERSION__ < 201112L
#error "LilScript native requires C11"
#endif
#if defined(__FAST_MATH__) || (defined(__FINITE_MATH_ONLY__) && __FINITE_MATH_ONLY__ != 0)
#error "LilScript native forbids fast-math and finite-only arithmetic"
#endif
#if FLT_EVAL_METHOD != 0
#error "LilScript native requires evaluation in the declared floating type"
#endif
#pragma STDC FP_CONTRACT OFF
_Static_assert(CHAR_BIT == 8, "LilScript native requires 8-bit bytes");
_Static_assert(sizeof(uint16_t) == 2 && sizeof(uint32_t) == 4 && sizeof(uint64_t) == 8,
               "LilScript native requires exact integer widths");
_Static_assert(SIZE_MAX >= UINT32_MAX, "LilScript native requires at least 32-bit object sizes");
_Static_assert(sizeof(double) == 8 && FLT_RADIX == 2 && DBL_MANT_DIG == 53 &&
               DBL_MIN_EXP == -1021 && DBL_MAX_EXP == 1024 && DBL_HAS_SUBNORM == 1,
               "LilScript native requires binary64 with subnormals");
typedef struct { const uint16_t *data; size_t length; } ls_string;
static int ls_runtime_init(void) {
    double one = 1.0, negative_zero = -0.0;
    uint64_t one_bits, zero_bits;
    memcpy(&one_bits, &one, sizeof one_bits);
    memcpy(&zero_bits, &negative_zero, sizeof zero_bits);
    if (fegetround() != FE_TONEAREST || one_bits != UINT64_C(0x3ff0000000000000) ||
        zero_bits != UINT64_C(0x8000000000000000)) {
        fputs("unsupported LilScript native floating-point environment\n", stderr);
        return 0;
    }
    /* FE_TONEAREST does not exclude flush-to-zero or denormals-are-zero. */
    uint64_t tiny_bits = UINT64_C(1), doubled_bits;
    double tiny;
    memcpy(&tiny, &tiny_bits, sizeof tiny);
    volatile double input = tiny, factor = 2.0;
    volatile double doubled = input * factor;
    double observed = doubled;
    memcpy(&doubled_bits, &observed, sizeof doubled_bits);
    if (doubled_bits != UINT64_C(2)) {
        fputs("LilScript native requires gradual binary64 underflow\n", stderr);
        return 0;
    }
    return 1;
}
"#;

/// Declaration order is topological: definitions mention only earlier helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(super) enum Helper {
    FromU32,
    ToInt32,
    RoundBinary64,
    DoubleBits,
    Multiply,
    Imul,
    Divide,
    Remainder,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    StringLength,
    CharCodeAt,
    CharAt,
    StringEqual,
    Strings,
    ClosureRuntime,
    Dynamic,
    Collections,
    Binary,
}

impl Helper {
    pub(super) const ALL: [Self; 20] = [
        Self::FromU32,
        Self::ToInt32,
        Self::RoundBinary64,
        Self::DoubleBits,
        Self::Multiply,
        Self::Imul,
        Self::Divide,
        Self::Remainder,
        Self::ShiftLeft,
        Self::ShiftRight,
        Self::UnsignedShiftRight,
        Self::StringLength,
        Self::CharCodeAt,
        Self::CharAt,
        Self::StringEqual,
        Self::Strings,
        Self::ClosureRuntime,
        Self::Dynamic,
        Self::Collections,
        Self::Binary,
    ];

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::FromU32 => "ls_from_u32",
            Self::ToInt32 => "ls_to_i32",
            Self::RoundBinary64 => "ls_f64",
            Self::DoubleBits => "ls_f64_bits",
            Self::Multiply => "ls_mul",
            Self::Imul => "ls_imul",
            Self::Divide => "ls_div",
            Self::Remainder => "ls_rem",
            Self::ShiftLeft => "ls_shl",
            Self::ShiftRight => "ls_shr",
            Self::UnsignedShiftRight => "ls_ushr",
            Self::StringLength => "ls_string_length",
            Self::CharCodeAt => "ls_char_code_at",
            Self::CharAt => "ls_char_at",
            Self::StringEqual => "ls_string_equal",
            Self::Strings => "ls_string_concat",
            Self::Dynamic => "ls_value_equal",
            Self::Collections => "ls_map_new",
            Self::Binary => "ls_buffer_new",
            Self::ClosureRuntime => "ls_native_retain",
        }
    }

    pub(super) const fn dependencies(self) -> &'static [Self] {
        match self {
            Self::ToInt32
            | Self::Imul
            | Self::ShiftLeft
            | Self::ShiftRight
            | Self::UnsignedShiftRight
            | Self::StringLength => &[Self::FromU32],
            Self::Multiply => &[Self::ToInt32, Self::RoundBinary64],
            Self::Strings => &[Self::StringEqual],
            Self::Dynamic => &[Self::Strings, Self::ClosureRuntime],
            Self::Collections => &[Self::Dynamic],
            Self::Binary => &[Self::ClosureRuntime, Self::FromU32],
            _ => &[],
        }
    }

    pub(super) const fn definition(self) -> &'static str {
        match self {
            Self::ClosureRuntime => super::native_memory::IMPLEMENTATION,
            Self::Strings => super::native_string_runtime::STRINGS,
            Self::Dynamic => super::native::DYNAMIC_RUNTIME,
            Self::Collections => super::native::COLLECTIONS_RUNTIME,
            Self::Binary => super::native::BINARY_RUNTIME,
            Self::FromU32 => {
                r#"static inline int32_t ls_from_u32(uint32_t value) {
    return value <= INT32_MAX ? (int32_t)value : (int32_t)((int64_t)value - INT64_C(4294967296));
}
"#
            }
            // Truncate the binary64 significand, retain exactly the low 32
            // integer bits, then apply sign modulo 2^32. No out-of-range cast,
            // floating remainder or data-dependent loop is necessary.
            Self::ToInt32 => {
                r#"static inline int32_t ls_to_i32(double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof bits);
    unsigned biased = (unsigned)((bits >> 52) & UINT64_C(2047));
    if (biased < 1023 || biased >= 1107) return 0;
    unsigned exponent = biased - 1023;
    uint64_t significand = (bits & UINT64_C(0x000fffffffffffff)) | UINT64_C(0x0010000000000000);
    uint32_t low = exponent < 52 ? (uint32_t)(significand >> (52 - exponent))
                                : (uint32_t)((uint32_t)significand << (exponent - 52));
    if (bits >> 63) low = (uint32_t)(UINT32_C(0) - low);
    return ls_from_u32(low);
}
"#
            }
            Self::RoundBinary64 => {
                r#"static inline double ls_f64(double value) {
    volatile double rounded = value;
    return rounded;
}
"#
            }
            Self::DoubleBits => {
                r#"static inline double ls_f64_bits(uint64_t bits) {
    double value;
    memcpy(&value, &bits, sizeof value);
    return value;
}
"#
            }
            Self::Multiply => {
                r#"static inline int32_t ls_mul(int32_t left, int32_t right) {
    return ls_to_i32(ls_f64((double)left * (double)right));
}
"#
            }
            Self::Imul => {
                r#"static inline int32_t ls_imul(int32_t left, int32_t right) {
    uint64_t product = (uint64_t)(uint32_t)left * (uint64_t)(uint32_t)right;
    return ls_from_u32((uint32_t)product);
}
"#
            }
            Self::Divide => {
                r#"static inline int32_t ls_div(int32_t left, int32_t right) {
    if (right == 0) return 0;
    if (left == INT32_MIN && right == -1) return INT32_MIN;
    return left / right;
}
"#
            }
            Self::Remainder => {
                r#"static inline int32_t ls_rem(int32_t left, int32_t right) {
    if (right == 0 || (left == INT32_MIN && right == -1)) return 0;
    return left % right;
}
"#
            }
            Self::ShiftLeft => {
                r#"static inline int32_t ls_shl(int32_t value, int32_t count) {
    uint32_t shift = (uint32_t)count & UINT32_C(31);
    return ls_from_u32((uint32_t)((uint32_t)value << shift));
}
"#
            }
            Self::ShiftRight => {
                r#"static inline int32_t ls_shr(int32_t value, int32_t count) {
    uint32_t shift = (uint32_t)count & UINT32_C(31);
    if (shift == 0) return value;
    uint32_t bits = (uint32_t)value >> shift;
    if (value < 0) bits |= (uint32_t)(UINT32_MAX << (32 - shift));
    return ls_from_u32(bits);
}
"#
            }
            Self::UnsignedShiftRight => {
                r#"static inline int32_t ls_ushr(int32_t value, int32_t count) {
    return ls_from_u32((uint32_t)value >> ((uint32_t)count & UINT32_C(31)));
}
"#
            }
            Self::StringLength => {
                r#"static inline int32_t ls_string_length(ls_string value) {
    return ls_from_u32((uint32_t)value.length);
}
"#
            }
            Self::CharCodeAt => {
                r#"static inline int32_t ls_char_code_at(ls_string value, int32_t index) {
    return index < 0 || (size_t)index >= value.length ? 0 : (int32_t)value.data[(size_t)index];
}
"#
            }
            // Strict equality of strings compares their code units.
            Self::StringEqual => {
                r#"static inline bool ls_string_equal(ls_string left, ls_string right) {
    return left.length == right.length &&
           (left.length == 0 || memcmp(left.data, right.data, left.length * sizeof *left.data) == 0);
}
"#
            }
            // Immutable strings have value semantics. This view borrows the
            // caller-owned code units, whose lifetime must include the result.
            Self::CharAt => {
                r#"static inline ls_string ls_char_at(ls_string value, int32_t index) {
    if (index < 0 || (size_t)index >= value.length) return (ls_string){NULL, 0};
    return (ls_string){value.data + (size_t)index, 1};
}
"#
            }
        }
    }
}

/// Only target recipe support, never another semantic dependency graph.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Helpers(u32);

impl Helpers {
    pub(super) fn require(&mut self, helper: Helper) {
        let bit = 1u32 << helper as u8;
        if self.0 & bit != 0 {
            return;
        }
        for &dependency in helper.dependencies() {
            self.require(dependency);
        }
        self.0 |= bit;
    }

    pub(super) fn contains(self, helper: Helper) -> bool {
        self.0 & (1u32 << helper as u8) != 0
    }

    pub(super) fn iter(self) -> impl Iterator<Item = Helper> {
        Helper::ALL
            .into_iter()
            .filter(move |helper| self.contains(*helper))
    }
}
