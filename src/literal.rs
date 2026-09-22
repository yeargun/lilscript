//! Decoded language string values. JavaScript strings can contain unpaired
//! UTF-16 surrogates; a Rust UTF-8 String alone cannot represent those values.
//! Ordinary strings retain compact UTF-8 storage. Encoded source spelling is
//! consumed here once and is not an optimization-time substitute for the value.

use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct StringValue(Repr);

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Repr {
    Unicode(String),
    CodeUnits(Vec<u16>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringEscapeError {
    Incomplete,
    InvalidHex,
    InvalidCodePoint,
    LegacyNumericEscape,
    UnescapedLineBreak,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum StringDecodeError {
    Escape(StringEscapeError),
    Resources(AllocationError),
}

impl From<StringEscapeError> for StringDecodeError {
    fn from(error: StringEscapeError) -> Self {
        Self::Escape(error)
    }
}

impl From<AllocationError> for StringDecodeError {
    fn from(error: AllocationError) -> Self {
        Self::Resources(error)
    }
}

impl StringDecodeError {
    fn inspection(self) -> StringEscapeError {
        match self {
            Self::Escape(error) => error,
            Self::Resources(error) => panic!("string decoding allocation failed: {error}"),
        }
    }
}

impl From<String> for StringValue {
    fn from(value: String) -> Self {
        Self(Repr::Unicode(value))
    }
}
impl From<&str> for StringValue {
    fn from(value: &str) -> Self {
        value.to_owned().into()
    }
}

impl Default for StringValue {
    fn default() -> Self {
        String::new().into()
    }
}

impl PartialOrd for StringValue {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for StringValue {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.code_units().cmp(other.code_units())
    }
}

impl StringValue {
    /// Copy an already decoded value after its consumer admits payload memory.
    /// Preserve canonical storage and lone surrogates without a second UTF-16
    /// decoding/canonicalization allocation. The caller owns work accounting.
    pub(crate) fn try_clone(&self) -> Result<Self, std::collections::TryReserveError> {
        Ok(match &self.0 {
            Repr::Unicode(value) => {
                let mut copy = String::new();
                copy.try_reserve_exact(value.len())?;
                copy.push_str(value);
                Self(Repr::Unicode(copy))
            }
            Repr::CodeUnits(value) => {
                let mut copy = Vec::new();
                copy.try_reserve_exact(value.len())?;
                copy.extend_from_slice(value);
                Self(Repr::CodeUnits(copy))
            }
        })
    }

    /// Preserve the representation of an existing `as_unpaired_utf16` payload
    /// after an admitted exact copy. Callers must copy that payload unchanged;
    /// arbitrary/new UTF-16 input must still use the canonicalizing constructor.
    /// No decoding, second buffer, or allocation occurs here.
    pub(crate) fn from_preserved_unpaired_utf16(units: Vec<u16>) -> Self {
        Self(Repr::CodeUnits(units))
    }

    /// Logical payload bytes, available without scanning or converting text.
    pub fn storage_bytes(&self) -> usize {
        match &self.0 {
            Repr::Unicode(value) => value.len(),
            Repr::CodeUnits(value) => value.len() * std::mem::size_of::<u16>(),
        }
    }

    /// Retained backing capacity for compiler-owned memory reservations. This
    /// excludes the inline value and allocator bookkeeping; unlike payload
    /// length, it includes spare capacity kept after string construction.
    pub(crate) fn capacity_bytes(&self) -> usize {
        match &self.0 {
            Repr::Unicode(value) => value.capacity(),
            Repr::CodeUnits(value) => value.capacity() * std::mem::size_of::<u16>(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match &self.0 {
            Repr::Unicode(value) => value.is_empty(),
            Repr::CodeUnits(value) => value.is_empty(),
        }
    }

    pub fn push(&mut self, other: &Self) {
        if other.is_empty() {
            return;
        }
        if let (Repr::Unicode(left), Repr::Unicode(right)) = (&mut self.0, &other.0) {
            left.push_str(right);
        } else {
            // A high/low surrogate pair can become valid at the join. Restore
            // canonical storage so equality and hashing do not depend on the
            // sequence of folds that produced the value.
            *self = Self::from_utf16(self.code_units().chain(other.code_units()).collect());
        }
    }

    pub fn concat(&self, other: &Self) -> Self {
        let mut result = self.clone();
        result.push(other);
        result
    }

    pub fn contains_unicode(&self, needle: &str) -> bool {
        match &self.0 {
            Repr::Unicode(value) => value.contains(needle),
            Repr::CodeUnits(value) => {
                let needle: Vec<_> = needle.encode_utf16().collect();
                needle.is_empty() || value.windows(needle.len()).any(|part| part == needle)
            }
        }
    }

    pub fn index_of(&self, needle: &Self, position: i32, last: bool) -> i32 {
        let receiver = self.code_units().collect::<Vec<_>>();
        let needle = needle.code_units().collect::<Vec<_>>();
        let position = if position < 0 {
            0
        } else {
            usize::try_from(position)
                .unwrap_or(usize::MAX)
                .min(receiver.len())
        };
        if needle.is_empty() {
            return position as i32;
        }
        if needle.len() > receiver.len() {
            return -1;
        }
        if last {
            (0..=position.min(receiver.len() - needle.len()))
                .rev()
                .find(|index| receiver[*index..*index + needle.len()] == needle)
                .map_or(-1, |index| index as i32)
        } else if position + needle.len() > receiver.len() {
            -1
        } else {
            (position..=receiver.len() - needle.len())
                .find(|index| receiver[*index..*index + needle.len()] == needle)
                .map_or(-1, |index| index as i32)
        }
    }

    pub fn repeat(&self, count: usize) -> Self {
        match &self.0 {
            Repr::Unicode(value) => value.repeat(count).into(),
            Repr::CodeUnits(value) => Self::from_utf16(value.repeat(count)),
        }
    }

    /// Iterate the language's UTF-16 code units without materializing a second
    /// string representation. Literal semantic operations share this view.
    pub fn code_units(&self) -> impl Iterator<Item = u16> + '_ {
        self.as_unicode()
            .into_iter()
            .flat_map(str::encode_utf16)
            .chain(self.as_unpaired_utf16().into_iter().flatten().copied())
    }
    pub fn from_utf16(units: Vec<u16>) -> Self {
        match String::from_utf16(&units) {
            Ok(value) => value.into(),
            Err(_) => Self(Repr::CodeUnits(units)),
        }
    }

    pub fn into_unicode(self) -> Option<String> {
        match self.0 {
            Repr::Unicode(value) => Some(value),
            Repr::CodeUnits(_) => None,
        }
    }

    pub fn as_unicode(&self) -> Option<&str> {
        match &self.0 {
            Repr::Unicode(value) => Some(value),
            Repr::CodeUnits(_) => None,
        }
    }

    pub fn as_unpaired_utf16(&self) -> Option<&[u16]> {
        match &self.0 {
            Repr::Unicode(_) => None,
            Repr::CodeUnits(units) => Some(units),
        }
    }

    /// Decode a lexer-validated, quote-stripped source literal. Legacy numeric
    /// escapes are rejected rather than giving strict and non-strict artifacts
    /// different meanings. The initial migration does not enable that syntax.
    pub fn decode_source(source: &str) -> Result<Self, StringEscapeError> {
        Self::decode(source, false, &mut AllocationBudget::new(None))
            .map_err(StringDecodeError::inspection)
    }

    /// Cook one untagged template chunk. Escapes share the ordinary string
    /// decoder; physical CR and CRLF become LF, while escaped CR stays CR.
    pub fn decode_template(source: &str) -> Result<Self, StringEscapeError> {
        Self::decode(source, true, &mut AllocationBudget::new(None))
            .map_err(StringDecodeError::inspection)
    }

    /// Successful payload capacity belongs to the caller's Retained class.
    /// Decoder scratch and conversion overlap are admitted in a child scope.
    pub(crate) fn decode_source_admitted(
        source: &str,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, StringDecodeError> {
        Self::decode(source, false, budget)
    }

    pub(crate) fn decode_template_admitted(
        source: &str,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, StringDecodeError> {
        Self::decode(source, true, budget)
    }

    fn decode(
        source: &str,
        template: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, StringDecodeError> {
        let mut phase = budget.scope();
        let value = Self::decode_in(source, template, &mut phase)?;
        // The shared ledger already bounds the sum of parent and child storage.
        phase
            .finish_retained()
            .expect("decoded payload transfers within its allocation owner");
        Ok(value)
    }

    fn decode_in(
        source: &str,
        template: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, StringDecodeError> {
        analysis_work(budget, source.len())?;
        if !source.contains('\\') {
            analysis_work(budget, source.len())?;
            if !template && source.contains(['\n', '\r']) {
                return Err(StringEscapeError::UnescapedLineBreak.into());
            }
            if !template || !source.contains('\r') {
                return budget
                    .string(AllocationClass::Retained, source)
                    .map(Into::into)
                    .map_err(Into::into);
            }
        }
        analysis_work(budget, source.len())?;
        let mut chars = source.chars().peekable();
        // Decoding never emits more UTF-16 units than source UTF-8 bytes.
        let mut units = budget.vector(AllocationClass::Scratch, source.len())?;
        while let Some(ch) = chars.next() {
            if ch != '\\' {
                if !template && matches!(ch, '\n' | '\r') {
                    return Err(StringEscapeError::UnescapedLineBreak.into());
                }
                if ch == '\r' {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                    units.push(10);
                } else {
                    push_char(&mut units, ch);
                }
                continue;
            }
            let escaped = chars.next().ok_or(StringEscapeError::Incomplete)?;
            match escaped {
                'n' => units.push(10),
                'r' => units.push(13),
                't' => units.push(9),
                'b' => units.push(8),
                'f' => units.push(12),
                'v' => units.push(11),
                '0' if !chars.peek().is_some_and(|ch| ch.is_ascii_digit()) => units.push(0),
                '0'..='9' => return Err(StringEscapeError::LegacyNumericEscape.into()),
                'x' => units.push(hex(&mut chars, 2)? as u16),
                'u' => {
                    if chars.peek() == Some(&'{') {
                        chars.next();
                        let mut count = 0;
                        let mut value = 0u32;
                        loop {
                            let ch = chars.next().ok_or(StringEscapeError::Incomplete)?;
                            if ch == '}' {
                                break;
                            }
                            count += 1;
                            if count > 6 {
                                return Err(StringEscapeError::InvalidCodePoint.into());
                            }
                            value = value * 16
                                + ch.to_digit(16).ok_or(StringEscapeError::InvalidHex)?;
                        }
                        if count == 0 || value > 0x10ffff {
                            return Err(StringEscapeError::InvalidCodePoint.into());
                        }
                        if value <= 0xffff {
                            units.push(value as u16);
                        } else {
                            push_char(
                                &mut units,
                                char::from_u32(value).ok_or(StringEscapeError::InvalidCodePoint)?,
                            );
                        }
                    } else {
                        units.push(hex(&mut chars, 4)? as u16);
                    }
                }
                '\n' | '\u{2028}' | '\u{2029}' => {}
                '\r' => {
                    if chars.peek() == Some(&'\n') {
                        chars.next();
                    }
                }
                // Escaped quotes, slashes and ECMAScript identity escapes.
                other => push_char(&mut units, other),
            }
        }
        canonicalize_decoded(units, budget)
    }
}

fn analysis_work(budget: &mut AllocationBudget<'_>, bytes: usize) -> Result<(), AllocationError> {
    budget.work(
        WorkKind::Analysis,
        u64::try_from(bytes).map_err(|_| AllocationError::Capacity)?,
    )
}

fn canonicalize_decoded(
    units: Vec<u16>,
    budget: &mut AllocationBudget<'_>,
) -> Result<StringValue, StringDecodeError> {
    let units_bytes = units
        .capacity()
        .checked_mul(std::mem::size_of::<u16>())
        .and_then(|bytes| u64::try_from(bytes).ok())
        .ok_or(AllocationError::Capacity)?;
    analysis_work(budget, units.len())?;
    let mut bytes = 0usize;
    for decoded in char::decode_utf16(units.iter().copied()) {
        let Ok(ch) = decoded else {
            budget.promote(units_bytes)?;
            return Ok(StringValue::from_preserved_unpaired_utf16(units));
        };
        bytes = bytes
            .checked_add(ch.len_utf8())
            .ok_or(AllocationError::Capacity)?;
    }
    analysis_work(budget, units.len())?;
    budget.work(
        WorkKind::Render,
        u64::try_from(bytes).map_err(|_| AllocationError::Capacity)?,
    )?;
    let mut utf8 = budget.vector(AllocationClass::Retained, bytes)?;
    for decoded in char::decode_utf16(units.iter().copied()) {
        let ch = decoded.expect("validated UTF-16 input is unchanged");
        utf8.extend_from_slice(ch.encode_utf8(&mut [0; 4]).as_bytes());
    }
    drop(units);
    budget.release(AllocationClass::Scratch, units_bytes)?;
    // SAFETY: every byte came from char::encode_utf8, and capacity was exact.
    Ok(unsafe { String::from_utf8_unchecked(utf8) }.into())
}

fn push_char(units: &mut Vec<u16>, ch: char) {
    units.extend_from_slice(ch.encode_utf16(&mut [0; 2]));
}

fn hex(chars: &mut impl Iterator<Item = char>, digits: usize) -> Result<u32, StringEscapeError> {
    let mut value = 0;
    for _ in 0..digits {
        value = value * 16
            + chars
                .next()
                .ok_or(StringEscapeError::Incomplete)?
                .to_digit(16)
                .ok_or(StringEscapeError::InvalidHex)?;
    }
    Ok(value)
}

#[cfg(test)]
#[path = "literal_admission_tests.rs"]
mod admission_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equivalent_spellings_have_one_value_and_surrogates_remain_code_units() {
        assert_eq!(
            StringValue::decode_source(r"A\n\u{1f600}").unwrap(),
            StringValue::decode_source(r"\x41\u000a\ud83d\ude00").unwrap()
        );
        assert_eq!(
            StringValue::decode_source(r"\ud800X\udfff")
                .unwrap()
                .as_unpaired_utf16(),
            Some(&[0xd800, 88, 0xdfff][..])
        );
        assert_eq!(
            StringValue::decode_source(r"\0\v\z").unwrap().as_unicode(),
            Some("\0\u{b}z")
        );
        assert_eq!(
            StringValue::decode_source("a\\\nb").unwrap().as_unicode(),
            Some("ab")
        );
    }

    #[test]
    fn invalid_escapes_do_not_become_literal_backslash_text() {
        for value in [
            r"\x",
            r"\u00x0",
            r"\u{}",
            r"\u{110000}",
            r"\08",
            r"\1",
            "\\",
        ] {
            assert!(StringValue::decode_source(value).is_err(), "{value}");
        }
    }

    #[test]
    fn template_cooking_preserves_code_units_and_normalizes_physical_lines() {
        let source = "a\r\nb\rc\n\\r\\n\\`\\${x}\\\r\nd\\ud800";
        let expected: Vec<u16> = "a\nb\nc\n\r\n`${x}d"
            .encode_utf16()
            .chain([0xd800])
            .collect();
        assert_eq!(
            StringValue::decode_template(source)
                .unwrap()
                .code_units()
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            StringValue::decode_source("a\nb"),
            Err(StringEscapeError::UnescapedLineBreak)
        );
        for invalid in [r"\x", r"\u{}", r"\u{110000}", r"\08", r"\1", "\\"] {
            assert!(StringValue::decode_template(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn value_concatenation_recanonicalizes_surrogate_boundaries() {
        let joined = StringValue::decode_source(r"\ud83d")
            .unwrap()
            .concat(&StringValue::decode_source(r"\ude00").unwrap());
        let literal = StringValue::from("😀");
        assert_eq!(joined, literal);
        let mut values = std::collections::HashSet::new();
        values.insert(joined);
        values.insert(literal);
        assert_eq!(values.len(), 1);
        let lone = StringValue::decode_source(r"\ud800field_\udfff").unwrap();
        assert!(lone.contains_unicode("field_"));
        assert!(!lone.contains_unicode("fields_"));
    }
}
