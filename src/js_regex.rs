//! Fail-closed ECMAScript 2022 regular-expression literal construction.
//!
//! This is deliberately an accepting subset, not a second regular-expression
//! engine. A rejected pattern remains a `RegExp` constructor at the call site,
//! which preserves the runtime position of syntax errors. Accepted patterns
//! use only grammar whose constructor and literal forms have the same meaning.

const MAX_GROUP_DEPTH: usize = 128;

/// Return an ES2022 regular-expression literal for source-encoded LilScript
/// string bodies. Invalid strings, invalid patterns, and valid patterns outside
/// the proven subset return `None`.
///
/// The caller still decides whether the candidate is worthwhile for its
/// selected complete-artifact cost model.
pub(crate) fn es2022_regex_literal(pattern_source: &str, flags_source: &str) -> Option<String> {
    let pattern = decode_source_string_fragment(pattern_source)?;
    let flags = decode_source_string_fragment(flags_source)?;
    literal_from_decoded(&pattern, &flags)
}

fn decode_source_string_fragment(value: &str) -> Option<String> {
    serde_json::from_str(&format!("\"{value}\"")).ok()
}

fn literal_from_decoded(pattern: &str, flags: &str) -> Option<String> {
    let parsed_flags = RegexFlags::parse(flags)?;
    if !PatternValidator::new(pattern, parsed_flags.unicode).validate() {
        return None;
    }

    let body = serialize_literal_body(pattern);
    Some(format!("/{body}/{flags}"))
}

#[derive(Debug, Clone, Copy)]
struct RegexFlags {
    unicode: bool,
}

impl RegexFlags {
    fn parse(flags: &str) -> Option<Self> {
        let mut seen = 0u8;
        let mut unicode = false;
        for flag in flags.chars() {
            let bit = match flag {
                'd' => 1 << 0,
                'g' => 1 << 1,
                'i' => 1 << 2,
                'm' => 1 << 3,
                's' => 1 << 4,
                'u' => {
                    unicode = true;
                    1 << 5
                }
                'y' => 1 << 6,
                // Unicode sets are newer than the ES2022 output contract.
                _ => return None,
            };
            if seen & bit != 0 {
                return None;
            }
            seen |= bit;
        }
        Some(Self { unicode })
    }
}

#[derive(Debug, Clone, Copy)]
enum ClassAtom {
    /// A single code point and therefore a valid range endpoint.
    Single(u32),
    /// A character set, backreference, or non-Unicode astral spelling.
    NonRange,
}

struct PatternValidator<'a> {
    pattern: &'a str,
    position: usize,
    unicode: bool,
    captures: usize,
    decimal_backreferences: Vec<(usize, usize)>,
}

impl<'a> PatternValidator<'a> {
    fn new(pattern: &'a str, unicode: bool) -> Self {
        Self {
            pattern,
            position: 0,
            unicode,
            captures: 0,
            decimal_backreferences: Vec::new(),
        }
    }

    fn validate(mut self) -> bool {
        self.parse_disjunction(0)
            && self.at_end()
            && self
                .decimal_backreferences
                .iter()
                .all(|(start, end)| decimal_at_most(&self.pattern[*start..*end], self.captures))
    }

    fn parse_disjunction(&mut self, depth: usize) -> bool {
        if !self.parse_alternative(depth) {
            return false;
        }
        while self.consume('|') {
            if !self.parse_alternative(depth) {
                return false;
            }
        }
        true
    }

    fn parse_alternative(&mut self, depth: usize) -> bool {
        while !self.at_end() && !matches!(self.peek(), Some('|' | ')')) {
            if !self.parse_term(depth) {
                return false;
            }
        }
        true
    }

    fn parse_term(&mut self, depth: usize) -> bool {
        if self.consume('^') || self.consume('$') {
            return !self.next_starts_quantifier();
        }
        if self.starts_with(r"\b") || self.starts_with(r"\B") {
            self.position += 2;
            return !self.next_starts_quantifier();
        }
        if self.starts_with("(?=") || self.starts_with("(?!") {
            self.position += 3;
            if !self.parse_group_body(depth) {
                return false;
            }
            // Quantified lookahead is an Annex-B-only production. Keeping it
            // as a constructor avoids making the accepted grammar mode-dependent.
            return !self.next_starts_quantifier();
        }
        if !self.parse_atom(depth) {
            return false;
        }
        self.parse_optional_quantifier()
    }

    fn parse_atom(&mut self, depth: usize) -> bool {
        match self.peek() {
            Some('.') => {
                self.advance();
                true
            }
            Some('[') => self.parse_character_class(),
            Some('\\') => self.parse_escape(false).is_some(),
            Some('(') => {
                self.advance();
                if self.consume('?') {
                    if !self.consume(':') {
                        return false;
                    }
                } else {
                    self.captures += 1;
                }
                self.parse_group_body(depth)
            }
            Some(character)
                if !is_pattern_syntax(character) && !unsupported_raw_control(character) =>
            {
                self.advance();
                true
            }
            _ => false,
        }
    }

    fn parse_group_body(&mut self, depth: usize) -> bool {
        if depth >= MAX_GROUP_DEPTH {
            return false;
        }
        self.parse_disjunction(depth + 1) && self.consume(')')
    }

    fn parse_optional_quantifier(&mut self) -> bool {
        match self.peek() {
            Some('*' | '+' | '?') => {
                self.advance();
            }
            Some('{') => {
                if !self.parse_braced_quantifier() {
                    return false;
                }
            }
            _ => return true,
        }
        self.consume('?');
        !self.next_starts_quantifier()
    }

    fn parse_braced_quantifier(&mut self) -> bool {
        if !self.consume('{') {
            return false;
        }
        let minimum_start = self.position;
        self.consume_decimal_digits();
        let minimum_end = self.position;
        if minimum_start == minimum_end {
            return false;
        }
        if self.consume('}') {
            return true;
        }
        if !self.consume(',') {
            return false;
        }
        let maximum_start = self.position;
        self.consume_decimal_digits();
        let maximum_end = self.position;
        if !self.consume('}') {
            return false;
        }
        maximum_start == maximum_end
            || decimal_not_greater(
                &self.pattern[minimum_start..minimum_end],
                &self.pattern[maximum_start..maximum_end],
            )
    }

    fn parse_character_class(&mut self) -> bool {
        if !self.consume('[') {
            return false;
        }
        self.consume('^');
        if self.consume(']') {
            return true;
        }

        let mut at_start = true;
        while !self.at_end() {
            if self.consume(']') {
                return true;
            }
            if self.peek() == Some('-') {
                if !at_start && !self.followed_by_closing_bracket() {
                    return false;
                }
                self.advance();
                at_start = false;
                continue;
            }

            let Some(left) = self.parse_class_atom() else {
                return false;
            };
            at_start = false;
            if self.peek() != Some('-') || self.followed_by_closing_bracket() {
                continue;
            }
            self.advance();
            let Some(right) = self.parse_class_atom() else {
                return false;
            };
            match (left, right) {
                (ClassAtom::Single(left), ClassAtom::Single(right)) if left <= right => {}
                _ => return false,
            }
        }
        false
    }

    fn parse_class_atom(&mut self) -> Option<ClassAtom> {
        if self.peek() == Some('\\') {
            return self.parse_escape(true);
        }
        let character = self.peek()?;
        if matches!(character, ']' | '-' | '[') || unsupported_raw_control(character) {
            return None;
        }
        self.advance();
        if self.unicode || character as u32 <= 0xffff {
            Some(ClassAtom::Single(character as u32))
        } else {
            // Without `u`, an astral spelling is two UTF-16 code units. It is
            // safe as an ordinary class member but not proven as a range end.
            Some(ClassAtom::NonRange)
        }
    }

    fn parse_escape(&mut self, in_class: bool) -> Option<ClassAtom> {
        if !self.consume('\\') {
            return None;
        }
        let escaped = self.advance()?;
        match escaped {
            'b' if in_class => Some(ClassAtom::Single(0x08)),
            'b' | 'B' => None,
            'd' | 'D' | 's' | 'S' | 'w' | 'W' => Some(ClassAtom::NonRange),
            'f' => Some(ClassAtom::Single(0x0c)),
            'n' => Some(ClassAtom::Single(0x0a)),
            'r' => Some(ClassAtom::Single(0x0d)),
            't' => Some(ClassAtom::Single(0x09)),
            'v' => Some(ClassAtom::Single(0x0b)),
            'c' => {
                let letter = self.advance()?;
                if !letter.is_ascii_alphabetic() {
                    return None;
                }
                Some(ClassAtom::Single(
                    (letter.to_ascii_uppercase() as u32) & 0x1f,
                ))
            }
            '0' => {
                if self.peek().is_some_and(|next| next.is_ascii_digit()) {
                    return None;
                }
                Some(ClassAtom::Single(0))
            }
            'x' => self.parse_fixed_hex(2).map(ClassAtom::Single),
            'u' if self.unicode && self.consume('{') => {
                let start = self.position;
                while self.peek().is_some_and(|next| next.is_ascii_hexdigit()) {
                    self.advance();
                }
                let end = self.position;
                if start == end || end - start > 6 || !self.consume('}') {
                    return None;
                }
                let value = u32::from_str_radix(&self.pattern[start..end], 16).ok()?;
                (value <= 0x10ffff).then_some(ClassAtom::Single(value))
            }
            'u' => self.parse_fixed_hex(4).map(ClassAtom::Single),
            '1'..='9' if !in_class => {
                let start = self.position - escaped.len_utf8();
                self.consume_decimal_digits();
                self.decimal_backreferences.push((start, self.position));
                Some(ClassAtom::NonRange)
            }
            character if valid_identity_escape(character, in_class) => {
                Some(ClassAtom::Single(character as u32))
            }
            _ => None,
        }
    }

    fn parse_fixed_hex(&mut self, digits: usize) -> Option<u32> {
        let start = self.position;
        for _ in 0..digits {
            if !self.peek().is_some_and(|next| next.is_ascii_hexdigit()) {
                return None;
            }
            self.advance();
        }
        u32::from_str_radix(&self.pattern[start..self.position], 16).ok()
    }

    fn consume_decimal_digits(&mut self) {
        while self.peek().is_some_and(|next| next.is_ascii_digit()) {
            self.advance();
        }
    }

    fn next_starts_quantifier(&self) -> bool {
        matches!(self.peek(), Some('*' | '+' | '?' | '{'))
    }

    fn followed_by_closing_bracket(&self) -> bool {
        self.remaining()
            .strip_prefix('-')
            .is_some_and(|rest| rest.starts_with(']'))
    }

    fn starts_with(&self, value: &str) -> bool {
        self.remaining().starts_with(value)
    }

    fn consume(&mut self, expected: char) -> bool {
        if self.peek() != Some(expected) {
            return false;
        }
        self.advance();
        true
    }

    fn peek(&self) -> Option<char> {
        self.remaining().chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.position += character.len_utf8();
        Some(character)
    }

    fn remaining(&self) -> &'a str {
        &self.pattern[self.position..]
    }

    fn at_end(&self) -> bool {
        self.position == self.pattern.len()
    }
}

fn is_pattern_syntax(character: char) -> bool {
    matches!(
        character,
        '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|'
    )
}

fn valid_identity_escape(character: char, in_class: bool) -> bool {
    matches!(
        character,
        '^' | '$' | '\\' | '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '/'
    ) || (in_class && character == '-')
}

fn unsupported_raw_control(character: char) -> bool {
    character.is_control() && !matches!(character, '\n' | '\r')
}

fn decimal_at_most(value: &str, maximum: usize) -> bool {
    decimal_not_greater(value, &maximum.to_string())
}

fn decimal_not_greater(left: &str, right: &str) -> bool {
    let left = left.trim_start_matches('0');
    let right = right.trim_start_matches('0');
    let left = if left.is_empty() { "0" } else { left };
    let right = if right.is_empty() { "0" } else { right };
    left.len() < right.len() || (left.len() == right.len() && left <= right)
}

fn serialize_literal_body(pattern: &str) -> String {
    if pattern.is_empty() {
        return "(?:)".to_string();
    }

    let mut body = String::with_capacity(pattern.len());
    let mut escaped = false;
    for character in pattern.chars() {
        if escaped {
            body.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' => {
                body.push('\\');
                escaped = true;
            }
            '/' => body.push_str(r"\/"),
            '\n' => body.push_str(r"\n"),
            '\r' => body.push_str(r"\r"),
            '\u{2028}' => body.push_str(r"\u2028"),
            '\u{2029}' => body.push_str(r"\u2029"),
            _ => body.push(character),
        }
    }
    body
}

#[cfg(test)]
mod tests {
    use super::{es2022_regex_literal, literal_from_decoded};

    fn literal(pattern: &str, flags: &str) -> Option<String> {
        literal_from_decoded(pattern, flags)
    }

    #[test]
    fn accepts_structured_es2022_patterns() {
        let arithmetic = r"^(?:([+-])=|)([+-]?(?:\d*\.|)\d+(?:[eE][+-]?\d+|))([a-z%]*)$";
        assert_eq!(literal(arithmetic, "i"), Some(format!("/{arithmetic}/i")));

        let markup = r"^<([a-z][^\/\0>]*)>(?:<\/\1>|)$";
        assert_eq!(literal(markup, "i"), Some(format!("/{markup}/i")));

        let headers = r"^(.*?):[ \t]*([^\r\n]*)$";
        assert_eq!(literal(headers, "mg"), Some(format!("/{headers}/mg")));

        assert_eq!(literal(r"\1(a)", "u"), Some(r"/\1(a)/u".to_string()));
    }

    #[test]
    fn serializes_literal_lexical_boundaries_without_changing_pattern_source() {
        assert_eq!(literal("", ""), Some("/(?:)/".to_string()));
        assert_eq!(literal("^//", ""), Some(r"/^\/\//".to_string()));
        assert_eq!(literal(r"^\/$", ""), Some(r"/^\/$/".to_string()));
        assert_eq!(
            literal("\n\r\u{2028}\u{2029}", ""),
            Some(r"/\n\r\u2028\u2029/".to_string())
        );
        assert_eq!(literal("\t", ""), None);
    }

    #[test]
    fn accepts_classes_ranges_escapes_and_quantifiers() {
        for pattern in [
            r"[\0-\x1f\x7f]|[^\/\w-]",
            r"(?:\cA|\u0041|\x41|\n){2,4}?",
            r"a{2,}|b{999999999999999999999999}",
            r"[]|[^]",
            r"[-a]|[a-]|[\-]",
        ] {
            assert_eq!(
                literal(pattern, ""),
                Some(format!("/{pattern}/")),
                "{pattern}"
            );
        }
        assert_eq!(
            literal(r"\u{1f600}+", "u"),
            Some(r"/\u{1f600}+/u".to_string())
        );
    }

    #[test]
    fn rejects_invalid_or_unproven_patterns_and_flags() {
        for pattern in [
            "[",
            "(",
            ")",
            r"\",
            "*a",
            "a{2,1}",
            "[z-a]",
            r"[\d-a]",
            r"\2(a)",
            r"\01",
            r"(?<name>a)\k<name>",
            r"(?<=a)b",
            r"\p{L}",
            "a{}",
        ] {
            assert_eq!(literal(pattern, "u"), None, "{pattern}");
        }

        assert!(literal("sale", "dgimsuy").is_some());
        for flags in ["gg", "v", "uv", "z", "I"] {
            assert_eq!(literal("sale", flags), None, "{flags}");
        }
    }

    #[test]
    fn rejects_source_fragments_that_cannot_be_decoded_exactly() {
        assert_eq!(es2022_regex_literal(r"\v", ""), None);
        assert_eq!(
            es2022_regex_literal(r"\\d{2,4}", "gi"),
            Some(r"/\d{2,4}/gi".to_string())
        );
    }
}
