use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{lex, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn flatten_associative_string_concats(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].kind == TokenKind::String
            && tokens.get(cursor + 1).map(|token| token.text) == Some("+")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("(")
            && tokens
                .get(cursor + 3)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 4).map(|token| token.text) == Some("+")
            && tokens.get(cursor + 5).map(|token| token.kind) == Some(TokenKind::String)
            && tokens.get(cursor + 6).map(|token| token.text) == Some(")")
        {
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 6].end,
                format!(
                    "{}+{}+{}",
                    tokens[cursor].text,
                    tokens[cursor + 3].text,
                    tokens[cursor + 5].text
                ),
            ));
            cursor += 7;
            continue;
        }
        cursor += 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

/// Drop `+""` after a value the surrounding `typeof` test already proved is a
/// string, and after `.toString()` in a `typeof === "symbol"` arm. Both
/// producers are specified to yield a string primitive, so a second ToString
/// is a no-op.
pub(crate) fn fold_known_string_coercions(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if let Some((plus_at, after)) = known_string_plus_empty(&tokens, cursor)
            .or_else(|| known_symbol_tostring_plus_empty(&tokens, cursor))
        {
            replacements.push((
                tokens[plus_at].start,
                tokens[plus_at + 1].end,
                String::new(),
            ));
            cursor = after;
            continue;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn string_lit_is(token: &Token<'_>, kind: &str) -> bool {
    token.kind == TokenKind::String
        && match kind {
            "string" => matches!(token.text, "\"string\"" | "'string'"),
            "symbol" => matches!(token.text, "\"symbol\"" | "'symbol'"),
            _ => false,
        }
}

fn typeof_name_is<'a>(tokens: &'a [Token<'a>], at: usize, kind: &str) -> Option<&'a str> {
    if tokens.get(at).map(|token| token.text) == Some("typeof")
        && tokens
            .get(at + 1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && matches!(
            tokens.get(at + 2).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens
            .get(at + 3)
            .is_some_and(|token| string_lit_is(token, kind))
    {
        return Some(tokens[at + 1].text);
    }
    if tokens
        .get(at)
        .is_some_and(|token| string_lit_is(token, kind))
        && matches!(
            tokens.get(at + 1).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens.get(at + 2).map(|token| token.text) == Some("typeof")
        && tokens
            .get(at + 3)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return Some(tokens[at + 3].text);
    }
    None
}

fn is_empty_string_lit(token: Option<&Token<'_>>) -> bool {
    token
        .is_some_and(|token| token.kind == TokenKind::String && matches!(token.text, "\"\"" | "''"))
}

fn known_string_plus_empty(tokens: &[Token<'_>], cursor: usize) -> Option<(usize, usize)> {
    let name = typeof_name_is(tokens, cursor, "string")?;
    if tokens.get(cursor + 4).map(|token| token.text) != Some("?") {
        return None;
    }
    let qmark = cursor + 4;
    if tokens.get(qmark + 1).map(|token| token.text) != Some(name)
        || tokens.get(qmark + 2).map(|token| token.text) != Some("+")
        || !is_empty_string_lit(tokens.get(qmark + 3))
    {
        return None;
    }
    Some((qmark + 2, qmark + 4))
}

fn known_symbol_tostring_plus_empty(tokens: &[Token<'_>], cursor: usize) -> Option<(usize, usize)> {
    let name = typeof_name_is(tokens, cursor, "symbol")?;
    let qmark = cursor + 4;
    if tokens.get(qmark).map(|token| token.text) != Some("?") {
        return None;
    }
    if tokens.get(qmark + 1).map(|token| token.text) != Some(name)
        || tokens.get(qmark + 2).map(|token| token.text) != Some(".")
        || tokens.get(qmark + 3).map(|token| token.text) != Some("toString")
        || tokens.get(qmark + 4).map(|token| token.text) != Some("(")
        || tokens.get(qmark + 5).map(|token| token.text) != Some(")")
        || tokens.get(qmark + 6).map(|token| token.text) != Some("+")
        || !is_empty_string_lit(tokens.get(qmark + 7))
    {
        return None;
    }
    Some((qmark + 6, qmark + 8))
}

#[cfg(test)]
mod tests {
    use super::fold_known_string_coercions;

    #[test]
    fn drops_tostring_after_typeof_string_and_symbol() {
        let source = r#"function R(a){return"string"==typeof a?a+"":"symbol"==typeof a?a.toString()+"":new String(a)+""}"#;
        let (out, count) = fold_known_string_coercions(source).unwrap();
        assert_eq!(count, 2, "{out}");
        assert_eq!(
            out,
            r#"function R(a){return"string"==typeof a?a:"symbol"==typeof a?a.toString():new String(a)+""}"#
        );
    }
}

/// Merge constant operands of a `+` chain into one string literal:
/// `"a"+"b"` becomes `"ab"`, and `" "+80+"h"` becomes `" 80h"`. Once a chain's
/// running value is a string, every following literal operand is concatenated
/// with ToString, so the merge is the value the chain already computes.
///
/// Two contexts refuse the fold. On the left, an operator that binds at least
/// as tightly as the `+` — `-`, `*`, `/`, `%`, `**`, a unary `+`, `typeof` and
/// friends — takes the first literal as *its* operand, so `x-"a"+"b"` is
/// `(x-"a")+"b"` and must stay split. On the right, a tighter operator does the
/// same to the last one: `"a"+"b"*2` multiplies before it concatenates.
pub(crate) fn fold_constant_string_concatenations(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if tokens[cursor].kind != TokenKind::String
            || !left_context_allows_concat_merge(&tokens, cursor)
            || !operand_is_free(&tokens, cursor)
        {
            cursor += 1;
            continue;
        }
        let quote = tokens[cursor].text.as_bytes()[0];
        let mut merged = String::from(inner_string_text(tokens[cursor].text));
        let mut last = cursor;
        let mut index = cursor;
        while tokens.get(index + 1).map(|token| token.text) == Some("+") {
            let Some(operand) = tokens.get(index + 2) else {
                break;
            };
            if !operand_is_free(&tokens, index + 2) {
                break;
            }
            match operand.kind {
                TokenKind::String if operand.text.as_bytes()[0] == quote => {
                    merged.push_str(inner_string_text(operand.text));
                }
                TokenKind::Number if is_plain_decimal_integer(operand.text) => {
                    merged.push_str(operand.text);
                }
                _ => break,
            }
            last = index + 2;
            index += 2;
        }
        if last == cursor {
            cursor += 1;
            continue;
        }
        let quote = quote as char;
        replacements.push((
            tokens[cursor].start,
            tokens[last].end,
            format!("{quote}{merged}{quote}"),
        ));
        cursor = last + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn inner_string_text(text: &str) -> &str {
    &text[1..text.len() - 1]
}

fn is_plain_decimal_integer(text: &str) -> bool {
    (text == "0" || !text.starts_with('0'))
        && text.len() <= 15
        && text.bytes().all(|byte| byte.is_ascii_digit())
}

/// The operand at `index` is not claimed by a tighter operator to its right.
fn operand_is_free(tokens: &[Token], index: usize) -> bool {
    !matches!(
        tokens.get(index + 1).map(|token| token.text),
        Some("*" | "/" | "%" | "**" | "." | "[" | "(" | "?." | "++" | "--")
    )
}

/// Nothing to the left of `index` claims the literal as its own operand.
fn left_context_allows_concat_merge(tokens: &[Token], index: usize) -> bool {
    let Some(previous) = index.checked_sub(1).and_then(|at| tokens.get(at)) else {
        return true;
    };
    match previous.text {
        "-" | "*" | "/" | "%" | "**" | "typeof" | "void" | "delete" | "!" | "~" | "in"
        | "instanceof" => false,
        "+" => index
            .checked_sub(2)
            .and_then(|at| tokens.get(at))
            .is_some_and(|before| {
                matches!(
                    before.kind,
                    TokenKind::Identifier | TokenKind::Number | TokenKind::String
                ) || matches!(before.text, ")" | "]")
            }),
        _ => true,
    }
}
