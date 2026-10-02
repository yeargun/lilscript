//! Concrete C11 recipes for the shared primitive identities. No source/CFG
//! analysis, target graph, allocation or runtime library discovery lives here.
//! The caller admits emitted bytes and invokes ls_runtime_init before source
//! initialization.
//!
//! **Binary64 without per-operation barriers** (plan M11.2). Every double
//! operation is one correctly rounded IEEE operation because the translation
//! unit itself guarantees it under the user's flags, not the driver's:
//! `FLT_EVAL_METHOD == 0` (no excess precision), contraction off in source
//! (`#pragma STDC FP_CONTRACT OFF`; GCC ignores it, so
//! `#pragma GCC optimize("fp-contract=off")`; Clang also gets
//! `#pragma clang fp contract(off)` and `reassociate(off)`), GCC's unsafe-math
//! modes refused by their macros, and a startup check that refuses the one
//! mode that ignores pragmas (Clang's `-ffp-contract=fast`). The emitted file's
//! header states the flags. `ls_f64` is therefore the identity: the volatile
//! store it once was cost 3.8x on float loops.

/// Common scalar ABI and bounded execution qualification.
pub(super) const PROLOGUE: &str = concat!(include_str!("runtime/prologue.h"), include_str!("runtime/string.h"));

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
    CharCodeAtNumber,
    CodeUnitAt,
    StringIndex,
    CharAt,
    StringEqual,
    Strings,
    ClosureRuntime,
    Dynamic,
    Products,
    Arrays,
    Collections,
    Records,
    Json,
    Exceptions,
    Binary,
    Unicode,
    Regex,
}

impl Helper {
    pub(super) const ALL: [Self; 30] = [
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
        Self::ClosureRuntime,
        Self::StringLength,
        Self::CharCodeAt,
        Self::CharCodeAtNumber,
        Self::CodeUnitAt,
        Self::StringIndex,
        Self::CharAt,
        Self::StringEqual,
        Self::Strings,
        Self::Dynamic,
        Self::Products,
        Self::Arrays,
        Self::Collections,
        Self::Records,
        Self::Json,
        Self::Exceptions,
        Self::Binary,
        Self::Unicode,
        Self::Regex,
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
            Self::CharCodeAtNumber => "ls_char_code_at_number",
            Self::CodeUnitAt => "ls_code_unit_at",
            Self::StringIndex => "ls_string_index",
            Self::CharAt => "ls_char_at",
            Self::StringEqual => "ls_string_equal",
            Self::Strings => "ls_string_concat",
            Self::Dynamic => "ls_value_equal",
            Self::Products => "ls_native_temporaries_clear",
            Self::Arrays => "ls_array_new",
            Self::Collections => "ls_map_new",
            Self::Records => "ls_record_get",
            Self::Json => "ls_json_scalar",
            Self::Exceptions => "ls_native_throw",
            Self::Binary => "ls_buffer_new",
            Self::Unicode => "ls_string_case",
            Self::Regex => "ls_regex_new",
            Self::ClosureRuntime => "ls_native_retain",
        }
    }

    pub(super) const fn dependencies(self) -> &'static [Self] {
        match self {
            Self::ToInt32
            | Self::Imul
            | Self::ShiftLeft
            | Self::UnsignedShiftRight
            | Self::StringLength => &[Self::FromU32],
            Self::Multiply => &[Self::ToInt32, Self::FromU32],
            Self::StringIndex => &[Self::CodeUnitAt, Self::ClosureRuntime],
            Self::CharAt => &[Self::ClosureRuntime],
            Self::Strings => &[Self::StringEqual, Self::ClosureRuntime],
            Self::Dynamic => &[Self::Strings, Self::ClosureRuntime],
            Self::Products => &[Self::Dynamic],
            Self::Arrays => &[Self::Products],
            Self::Collections => &[Self::Dynamic],
            Self::Records => &[Self::Collections],
            Self::Json => &[Self::Records],
            Self::Exceptions => &[Self::Records, Self::Products],
            Self::Binary => &[Self::ClosureRuntime, Self::FromU32],
            Self::Unicode => &[Self::Strings],
            Self::Regex => &[Self::Unicode, Self::Exceptions],
            _ => &[],
        }
    }

    pub(super) const fn definition(self) -> &'static str {
        match self {
            Self::ClosureRuntime => super::native_memory::IMPLEMENTATION,
            Self::Strings => super::native_string_runtime::STRINGS,
            Self::Dynamic => super::native::DYNAMIC_RUNTIME,
            Self::Products => include_str!("runtime/temporaries.c"),
            Self::Arrays => include_str!("runtime/arrays.c"),
            Self::Collections => super::native::COLLECTIONS_RUNTIME,
            Self::Records => include_str!("runtime/records.c"),
            Self::Json => include_str!("runtime/json.c"),
            Self::Exceptions => include_str!("runtime/exceptions.c"),
            Self::Binary => super::native::BINARY_RUNTIME,
            Self::Unicode => concat!(include_str!("runtime/unicode-library.c"), include_str!("runtime/unicode.c")),
            Self::Regex => concat!(include_str!("runtime/regex-library.c"), include_str!("runtime/regex.c")),
            Self::FromU32 => {
                include_str!("runtime/from_u32.c")
            }
            // Truncate the binary64 significand, retain exactly the low 32
            // integer bits, then apply sign modulo 2^32. No out-of-range cast,
            // floating remainder or data-dependent loop is necessary.
            Self::ToInt32 => {
                include_str!("runtime/to_int32.c")
            }
            // Each operation is already one rounded binary64 operation (see
            // the module comment); this names the boundary and costs nothing.
            Self::RoundBinary64 => {
                include_str!("runtime/round_binary64.c")
            }
            Self::DoubleBits => {
                include_str!("runtime/double_bits.c")
            }
            // JavaScript's `(a*b)|0`: the exact product while it is at most
            // 2^53 in magnitude, where the double product is exact too, wrapped
            // to 32 bits; beyond it, ToInt32 of the rounded double product.
            Self::Multiply => {
                include_str!("runtime/multiply.c")
            }
            Self::Imul => {
                include_str!("runtime/imul.c")
            }
            Self::Divide => {
                include_str!("runtime/divide.c")
            }
            Self::Remainder => {
                include_str!("runtime/remainder.c")
            }
            Self::ShiftLeft => {
                include_str!("runtime/shift_left.c")
            }
            // Arithmetic, as the prologue's static assertion requires of `>>`
            // on a negative value (implementation-defined in C11 6.5.7p5).
            // Left shifts stay on `uint32_t` (`ls_shl`): a signed left shift
            // of a negative value is undefined (6.5.7p4).
            Self::ShiftRight => {
                include_str!("runtime/shift_right.c")
            }
            Self::UnsignedShiftRight => {
                include_str!("runtime/unsigned_shift_right.c")
            }
            Self::StringLength => {
                include_str!("runtime/string_length.c")
            }
            Self::CharCodeAt => {
                include_str!("runtime/char_code_at.c")
            }
            Self::CharCodeAtNumber => {
                include_str!("runtime/char_code_at_number.c")
            }
            Self::CodeUnitAt => {
                include_str!("runtime/code_unit_at.c")
            }
            Self::StringIndex => {
                include_str!("runtime/string_index.c")
            }
            // Strict equality of strings compares their code units.
            Self::StringEqual => {
                include_str!("runtime/string_equal.c")
            }
            // Immutable strings have value semantics. This view borrows the
            // caller-owned code units, whose lifetime must include the result.
            Self::CharAt => {
                include_str!("runtime/char_at.c")
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
