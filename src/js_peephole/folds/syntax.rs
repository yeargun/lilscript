use crate::js_peephole::rewrite::{
    apply_token_rewrites, is_property_identifier, parenthesized_expression_has_postfix_continuation,
};
use crate::js_peephole::scope::{enclosing_block_start, GeneratedBindingIndex};
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

/// Sequence commas cannot elide an operand (`a,,b` is a SyntaxError). Array
/// holes (`[a,,b]`) are the only place that empty slot is legal, so those
/// stay. A comma that opens a statement is the same hole after a terminator.
pub(crate) fn fold_empty_comma_operators(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut bracket_depth = 0i32;
    let mut paren_depth = 0i32;
    let mut brace_depth = 0i32;
    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "(" => paren_depth += 1,
            ")" => paren_depth -= 1,
            "[" => bracket_depth += 1,
            "]" => bracket_depth -= 1,
            "{" => brace_depth += 1,
            "}" => brace_depth -= 1,
            "," if bracket_depth <= 0 => {
                let previous = index.checked_sub(1).map(|at| tokens[at].text);
                let empty_operand = matches!(previous, Some(","));
                let statement_leading =
                    paren_depth == 0 && brace_depth == 0 && matches!(previous, None | Some(";"));
                if empty_operand || statement_leading {
                    replacements.push((token.start, token.end, String::new()));
                }
            }
            _ => {}
        }
    }
    Ok(apply_token_rewrites(source, replacements))
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

/// A binding compared with itself observes only the equality algorithm, never
/// coercion between different operands, so strict and loose equality agree.
/// Keep this as an objective-scored proposal because changing a repeated
/// operator can help raw bytes while perturbing a compressed dictionary.
pub(crate) fn fold_same_binding_strict_equality(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let candidates = (0..tokens.len().saturating_sub(2))
        .filter(|index| {
            tokens[*index].kind == TokenKind::Identifier
                && matches!(tokens[*index + 1].text, "===" | "!==")
                && tokens[*index + 2].kind == TokenKind::Identifier
                && tokens[*index + 2].text == tokens[*index].text
        })
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let bindings = GeneratedBindingIndex::new(&tokens, &matching_close);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for index in candidates {
        if bindings.name_is_visible(index, tokens[index].text) {
            replacements.push((
                tokens[index + 1].start,
                tokens[index + 1].end,
                if tokens[index + 1].text == "===" {
                    "=="
                } else {
                    "!="
                }
                .to_string(),
            ));
        }
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
    let mut matching_close = None;
    let mut bindings = None;
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
        let previous = index.checked_sub(1).and_then(|at| tokens.get(at));
        let comma_statement_boundary = if previous.is_some_and(|token| token.text == ",") {
            let matching_close = matching_close.get_or_insert_with(|| matching_closers(&tokens));
            comma_is_directly_inside_enclosing_block(&tokens, matching_close, index - 1)
        } else {
            false
        };
        if !comma_statement_boundary
            && previous.is_none_or(|previous| !matches!(previous.text, "{" | "}" | ";"))
        {
            continue;
        }
        let matching_close = matching_close.get_or_insert_with(|| matching_closers(&tokens));
        if bindings.is_none() {
            bindings = Some(GeneratedBindingIndex::new(&tokens, matching_close));
        }
        let bindings = bindings.as_ref().expect("binding index was initialized");
        let rest_starts_with_digit = rest.as_bytes().first().is_some_and(u8::is_ascii_digit);
        if bindings.name_is_visible(index, token.text)
            || (!rest_starts_with_digit && !bindings.name_is_visible(index, rest))
        {
            continue;
        }
        // A fused keyword is lexed as an expression identifier. A later
        // sequencing pass can consequently turn the preceding statement
        // terminator into a comma. Restore that boundary together with
        // the keyword separator, otherwise `E,return od()` is still
        // invalid JavaScript.
        if comma_statement_boundary {
            let previous = previous.expect("comma boundary was checked");
            replacements.push((previous.start, previous.end, ";".to_string()));
        }
        replacements.push((token.start, token.end, format!("{keyword} {rest}")));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn comma_is_directly_inside_enclosing_block(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    comma: usize,
) -> bool {
    let Some(block_open) = enclosing_block_start(matching_close, comma) else {
        return false;
    };
    let (mut parens, mut brackets, mut braces) = (0i32, 0i32, 0i32);
    for token in &tokens[block_open + 1..comma] {
        match token.text {
            "(" => parens += 1,
            ")" => parens -= 1,
            "[" => brackets += 1,
            "]" => brackets -= 1,
            "{" => braces += 1,
            "}" => braces -= 1,
            _ => {}
        }
    }
    parens == 0 && brackets == 0 && braces == 0
}

/// A regex literal alone in statement position evaluates to a fresh `RegExp`
/// and discards it: no user code runs, so the statement has no effect. The
/// shape is left behind when a single-use regex binding is rematerialized at
/// its use site but its declaration is consumed by a different fold first, and
/// it costs the whole pattern's bytes twice.
pub(crate) fn drop_pure_regex_expression_statements(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Regex {
            continue;
        }
        let starts_statement = match index.checked_sub(1) {
            None => true,
            Some(previous) => matches!(tokens[previous].text, ";" | "}"),
        };
        if !starts_statement {
            continue;
        }
        // Only a bare literal, never the head of a member or call chain.
        let Some(next) = tokens.get(index + 1) else {
            continue;
        };
        if next.text != ";" {
            continue;
        }
        replacements.push((token.start, next.end, String::new()));
    }
    Ok(apply_token_rewrites(source, replacements))
}

#[cfg(test)]
mod tests {
    use super::fold_empty_comma_operators;

    #[test]
    fn collapses_empty_comma_operators_outside_arrays() {
        let (out, count) = fold_empty_comma_operators("a=b,,c=d;").unwrap();
        assert!(count >= 1, "{out}");
        assert_eq!(out, "a=b,c=d;");
        let (holes, hole_count) = fold_empty_comma_operators("[a,,b]").unwrap();
        assert_eq!(hole_count, 0, "{holes}");
        assert_eq!(holes, "[a,,b]");
        let (stmt, stmt_count) = fold_empty_comma_operators("a=b;,c=d").unwrap();
        assert!(stmt_count >= 1, "{stmt}");
        assert_eq!(stmt, "a=b;c=d");
    }
}
