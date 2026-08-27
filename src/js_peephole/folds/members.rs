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
