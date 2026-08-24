use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{lex, TokenKind};
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
