use crate::js_peephole::rewrite::{
    apply_token_rewrites, is_property_identifier, parenthesized_expression_has_postfix_continuation,
};
use crate::js_peephole::token::{
    ascii_identifier_name_string, is_identifier_start, lex, lex_certainly, matching_closers, Token,
    TokenKind,
};
use crate::js_peephole::JavaScriptParseError;

/// Canonicalize two grammar-local spellings emitted by the IR backend:
/// `typeof(identifier)` and `object["IdentifierName"]`. The compact lexer does
/// not distinguish division from regular-expression literals, so callers must
/// prove that regex literals are absent before entering this pass. Quoted
/// strings and templates are single tokens and exact byte-adjacency checks keep
/// comments and escaped property keys outside every replacement.
pub(crate) fn canonicalize_leaf_syntax(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let Some(tokens) = lex_certainly(source)? else {
        return Ok((source.to_string(), 0));
    };
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();

    for index in 0..tokens.len() {
        if tokens[index].text == "typeof"
            && tokens.get(index + 1).map(|token| token.text) == Some("(")
            && matching_close[index + 1] == Some(index + 3)
            && tokens
                .get(index + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens[index].end == tokens[index + 1].start
            && tokens[index + 1].end == tokens[index + 2].start
            && tokens[index + 2].end == tokens[index + 3].start
            && !parenthesized_expression_has_postfix_continuation(&tokens, index + 3)
        {
            let mut replacement = format!("typeof {}", tokens[index + 2].text);
            if tokens.get(index + 4).is_some_and(|next| {
                next.start == tokens[index + 3].end
                    && matches!(
                        next.kind,
                        TokenKind::Identifier | TokenKind::Keyword | TokenKind::Number
                    )
            }) {
                replacement.push(' ');
            }
            replacements.push((tokens[index].start, tokens[index + 3].end, replacement));
        }

        if tokens[index].text != "["
            || tokens.get(index + 1).map(|token| token.kind) != Some(TokenKind::String)
            || tokens.get(index + 2).map(|token| token.text) != Some("]")
            || tokens[index].end != tokens[index + 1].start
            || tokens[index + 1].end != tokens[index + 2].start
            || tokens.get(index.wrapping_sub(1)).map(|token| token.end) != Some(tokens[index].start)
            || !tokens
                .get(index.wrapping_sub(1))
                .is_some_and(member_object_can_end_with)
        {
            continue;
        }
        let Some(property) = ascii_identifier_name_string(tokens[index + 1].text) else {
            continue;
        };
        let mut replacement = format!(".{property}");
        if tokens.get(index + 3).is_some_and(|next| {
            next.start == tokens[index + 2].end
                && matches!(
                    next.kind,
                    TokenKind::Identifier | TokenKind::Keyword | TokenKind::Number
                )
        }) {
            replacement.push(' ');
        }
        replacements.push((tokens[index].start, tokens[index + 2].end, replacement));
    }

    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
    }
    replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
    replacements.dedup_by(|left, right| left.0 == right.0 && left.1 == right.1);
    let count = replacements.len();
    let mut output = source.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok((output, count))
}

/// `cond?,x:y` is never valid JavaScript (`?.` is optional chaining, `??` is
/// nullish). A comma immediately after a ternary `?` is an empty then-arm
/// operand left behind when a sequenced value was deleted. Dropping that comma
/// restores `cond?x:y`.
pub(crate) fn fold_empty_ternary_then_comma(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut index = 0usize;
    while index + 1 < tokens.len() {
        if tokens[index].text == "?"
            && !matches!(
                tokens.get(index + 1).map(|token| token.text),
                Some(".") | Some("[")
            )
            && tokens.get(index + 1).map(|token| token.text) == Some(",")
        {
            replacements.push((
                tokens[index + 1].start,
                tokens[index + 1].end,
                String::new(),
            ));
        }
        index += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn member_object_can_end_with(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Identifier
        || (token.kind == TokenKind::Keyword && matches!(token.text, "this" | "super"))
}

/// Drops the space in `return "x"`, `case -1:`, `typeof {}` and friends.
///
/// A keyword only needs a separator when the next token could otherwise merge
/// into it, which is exactly when that token starts with an identifier
/// character or a digit (`return x`, `return 5`). String, template and regex
/// literals, and punctuation that opens a group or applies a prefix operator,
/// can all sit flush against the keyword.
///
/// `.` is deliberately excluded: `return .5` re-lexes as a member access on the
/// keyword when the space is removed.
pub(crate) fn elide_separating_keyword_spaces(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let Some(tokens) = lex_certainly(source)? else {
        return Ok((source.to_string(), 0));
    };
    let mut cuts = Vec::new();
    for pair in tokens.windows(2) {
        let [left, right] = pair else { continue };
        if left.kind != TokenKind::Keyword || right.start != left.end + 1 {
            continue;
        }
        if source.as_bytes().get(left.end) != Some(&b' ') {
            continue;
        }
        let flush =
            match right.kind {
                TokenKind::String | TokenKind::Template | TokenKind::Regex => true,
                TokenKind::Punct => right.text.as_bytes().first().is_some_and(|byte| {
                    matches!(byte, b'[' | b'(' | b'{' | b'!' | b'~' | b'+' | b'-')
                }),
                _ => false,
            };
        if flush {
            cuts.push(left.end);
        }
    }
    if cuts.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for cut in &cuts {
        output.push_str(&source[cursor..*cut]);
        cursor = cut + 1;
    }
    output.push_str(&source[cursor..]);
    Ok((output, cuts.len()))
}

const FUSED_OPERAND_KEYWORDS: &[&str] = &["return", "throw"];

/// `returna&&x` lexes as one identifier, not `return` plus `a`. A rewrite that
/// dropped the separator after a statement keyword leaves a ReferenceError.
/// Split only those fused names, and only in expression position: property
/// keys and `function returned()` bindings stay intact.
pub(crate) fn split_fused_keyword_identifiers(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Identifier || is_property_identifier(&tokens, index) {
            continue;
        }
        if index.checked_sub(1).is_some_and(|previous| {
            matches!(
                tokens[previous].text,
                "function" | "class" | "const" | "let" | "var" | "import" | "export"
            )
        }) {
            continue;
        }
        let Some(keyword) = FUSED_OPERAND_KEYWORDS.iter().copied().find(|keyword| {
            token.text.len() > keyword.len()
                && token.text.starts_with(keyword)
                && token
                    .text
                    .as_bytes()
                    .get(keyword.len())
                    .is_some_and(|byte| is_identifier_start(*byte) || byte.is_ascii_digit())
        }) else {
            continue;
        };
        let rest = &token.text[keyword.len()..];
        if rest.len() != 1 {
            continue;
        }
        replacements.push((token.start, token.end, format!("{keyword} {rest}")));
    }
    Ok(apply_token_rewrites(source, replacements))
}
