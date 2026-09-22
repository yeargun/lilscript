//! JavaScript spelling of decoded language strings. No source escape decoder
//! belongs in this layer. Both JavaScript backends share this printer.

use crate::literal::StringValue;
use std::fmt::{self, Write};

pub(crate) fn literal(value: &StringValue, quote: char) -> String {
    let capacity = value
        .as_unicode()
        .map_or_else(
            || value.as_unpaired_utf16().unwrap().len().saturating_mul(3),
            str::len,
        )
        .saturating_add(2);
    let mut out = String::with_capacity(capacity);
    out.push(quote);
    contents(&mut out, value, quote, false).unwrap();
    out.push(quote);
    out
}

/// `after_dollar` carries the preceding cooked chunk boundary for templates.
/// The writer can enforce an output bound without allocating a temporary text.
pub(crate) fn contents(
    out: &mut impl Write,
    value: &StringValue,
    quote: char,
    mut after_dollar: bool,
) -> fmt::Result {
    debug_assert!(matches!(quote, '\'' | '"' | '`'));
    if let Some(value) = value.as_unicode() {
        for ch in value.chars() {
            character(out, ch, quote, after_dollar)?;
            after_dollar = ch == '$';
        }
    } else {
        // Keep lone surrogates exact; valid pairs can use their UTF-8 spelling.
        // Each decoded segment is independent after a surrogate escape.
        for ch in char::decode_utf16(value.as_unpaired_utf16().unwrap().iter().copied()) {
            match ch {
                Ok(ch) => {
                    character(out, ch, quote, after_dollar)?;
                    after_dollar = ch == '$';
                }
                Err(error) => {
                    write!(out, "\\u{:04x}", error.unpaired_surrogate())?;
                    after_dollar = false;
                }
            }
        }
    }
    Ok(())
}

fn character(out: &mut impl Write, ch: char, quote: char, after_dollar: bool) -> fmt::Result {
    match ch {
        ch if ch == quote => {
            out.write_char('\\')?;
            out.write_char(ch)
        }
        '{' if quote == '`' && after_dollar => out.write_str("\\{"),
        '\\' => out.write_str("\\\\"),
        '\n' => out.write_str("\\n"),
        '\r' => out.write_str("\\r"),
        '\t' => out.write_str("\\t"),
        '\u{8}' => out.write_str("\\b"),
        '\u{c}' => out.write_str("\\f"),
        ch if ch < ' ' => write!(out, "\\u{:04x}", ch as u32),
        ch => out.write_char(ch),
    }
}
