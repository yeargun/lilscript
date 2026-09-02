use crate::js_peephole::token::{lex, Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Rewrite {
    pub(crate) start: usize,
    pub(crate) end: usize,
    pub(crate) rhs_start: usize,
    pub(crate) rhs_end: usize,
    pub(crate) identifier_start: usize,
    pub(crate) identifier_end: usize,
    pub(crate) operator: &'static str,
}

pub(crate) fn single_console_log_argument(expression: &str) -> Option<&str> {
    const PREFIX: &str = "console.log(";
    if !expression.starts_with(PREFIX) || !expression.ends_with(')') {
        return None;
    }
    let inner = &expression[PREFIX.len()..expression.len() - 1];
    if inner.is_empty() {
        return None;
    }
    let mut depth = 0i32;
    for ch in inner.chars() {
        match ch {
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => return None,
            _ => {}
        }
        if depth < 0 {
            return None;
        }
    }
    (depth == 0).then_some(inner)
}

pub(crate) fn parenthesized_expression_has_postfix_continuation(
    tokens: &[Token<'_>],
    close: usize,
) -> bool {
    tokens.get(close + 1).is_some_and(|token| {
        matches!(token.text, "(" | "[" | "." | "++" | "--" | "**")
            || token.kind == TokenKind::Template
            || (token.text == "?" && tokens.get(close + 2).is_some_and(|token| token.text == "."))
    })
}

pub(crate) fn paren_depth_at(tokens: &[Token<'_>]) -> Vec<i32> {
    let mut depths = vec![0; tokens.len()];
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate() {
        depths[index] = depth;
        match token.text {
            "(" => depth += 1,
            ")" => depth -= 1,
            _ => {}
        }
    }
    depths
}

pub(crate) fn is_property_identifier(tokens: &[Token<'_>], index: usize) -> bool {
    let previous = index
        .checked_sub(1)
        .map(|prev| tokens[prev].text)
        .unwrap_or(";");
    if previous == "?." {
        return true;
    }
    if previous == "." {
        // Rest/spread is `...ident`. The lexer emits one `.` per character, so
        // the identifier is a value. A member is a single `.` (or `?.`).
        return index
            .checked_sub(2)
            .is_none_or(|before| tokens[before].text != ".");
    }
    if tokens.get(index + 1).map(|token| token.text) == Some("=")
        && matches!(previous, "{" | "}" | ";")
        && enclosing_list_open(tokens, index).is_some_and(|open| class_body_open(tokens, open))
    {
        return true;
    }
    tokens.get(index + 1).map(|token| token.text) == Some(":") && matches!(previous, "{" | ",")
}

fn class_body_open(tokens: &[Token<'_>], open: usize) -> bool {
    tokens[..open]
        .iter()
        .rev()
        .take_while(|token| !matches!(token.text, ";" | "{" | "}"))
        .take(5)
        .any(|token| token.text == "class")
}

pub(crate) fn identifier_is_expression_slot(tokens: &[Token<'_>], index: usize) -> bool {
    if is_property_identifier(tokens, index) {
        return false;
    }
    let previous = index
        .checked_sub(1)
        .map(|prev| tokens[prev].text)
        .unwrap_or(";");
    let next = tokens.get(index + 1).map(|token| token.text).unwrap_or(";");
    if previous == "as" {
        return false;
    }
    let Some(open) = enclosing_list_open(tokens, index) else {
        return true;
    };
    if matches!(
        open.checked_sub(1).map(|prev| tokens[prev].text),
        Some("export") | Some("import")
    ) {
        return false;
    }
    if tokens[open].text != "{" {
        return true;
    }
    if matches!(previous, "{" | ",") && matches!(next, "}" | ",") {
        return false;
    }
    if matches!(previous, "{" | ",")
        && next == "("
        && matching_paren_close(tokens, index + 1)
            .is_some_and(|close| tokens.get(close + 1).map(|token| token.text) == Some("{"))
    {
        return false;
    }
    true
}

fn matching_paren_close(tokens: &[Token<'_>], open: usize) -> Option<usize> {
    if tokens.get(open).map(|token| token.text) != Some("(") {
        return None;
    }
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(open) {
        match token.text {
            "(" => depth += 1,
            ")" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn enclosing_list_open(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    let mut depth = 0i32;
    for i in (0..index).rev() {
        match tokens[i].text {
            "}" | ")" | "]" => depth += 1,
            "{" | "(" | "[" => {
                if depth == 0 {
                    return Some(i);
                }
                depth -= 1;
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn identifier_occurs(
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
    name: &str,
) -> bool {
    if start >= end || end > tokens.len() {
        return false;
    }
    tokens[start..end]
        .iter()
        .enumerate()
        .any(|(offset, token)| {
            token.kind == TokenKind::Identifier
                && token.text == name
                && !is_property_identifier(tokens, start + offset)
        })
}

pub(crate) fn identifier_is_read(
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
    name: &str,
) -> bool {
    if start >= end || end > tokens.len() {
        return false;
    }
    tokens[start..end]
        .iter()
        .enumerate()
        .any(|(offset, token)| {
            if token.kind != TokenKind::Identifier || token.text != name {
                return false;
            }
            let index = start + offset;
            if is_property_identifier(tokens, index) {
                return false;
            }
            let previous = index
                .checked_sub(1)
                .map(|prev| tokens[prev].text)
                .unwrap_or(";");
            if matches!(previous, "var" | "let" | "const") {
                return false;
            }
            if previous == "," && assign_is_in_declaration(tokens, index) {
                return false;
            }
            true
        })
}

pub(crate) fn top_level_stop(tokens: &[Token<'_>], start: usize, stops: &[&str]) -> Option<usize> {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        if depth == 0 && stops.contains(&token.text) {
            return Some(index);
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn replacement_overlaps(
    replacements: &[(usize, usize, String)],
    start: usize,
    end: usize,
) -> bool {
    replacements
        .iter()
        .any(|(existing_start, existing_end, _)| *existing_start < end && start < *existing_end)
}

pub(crate) fn apply_token_rewrites(
    source: &str,
    mut replacements: Vec<(usize, usize, String)>,
) -> (String, usize) {
    if replacements.is_empty() {
        return (source.to_string(), 0);
    }
    replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
    let mut retained = Vec::new();
    let mut last_end = 0;
    for replacement in replacements {
        if replacement.0 >= last_end {
            last_end = replacement.1;
            retained.push(replacement);
        }
    }
    let count = retained.len();
    let mut output = source.to_string();
    for (start, end, replacement) in retained.into_iter().rev() {
        let replacement = separated_at_boundaries(&output, start, end, replacement);
        output.replace_range(start..end, &replacement);
    }
    (output, count)
}

/// The printer's rule, applied at the one place folds join text. Terser never
/// fuses two tokens because spacing is not a transform's job: its `print`
/// (`lib/output.js`, `might_need_space`) emits a space exactly when the last
/// character written and the next token's first character would lex as one
/// token -- two identifier characters, `+ +`, `- -`, `/ /`. Closure's
/// `CodeConsumer` does the same. Every fold here splices strings instead, so a
/// fold that drops the parenthesis after `return(`, or that replaces `!0` with
/// `true` after a keyword, produced `returnIr(...)` -- a call to an undeclared
/// global that shipped in jquerylil and throws on `animate` (039, 040). The
/// guard is symmetric and covers deletions, where the two neighbours meet.
fn separated_at_boundaries(output: &str, start: usize, end: usize, replacement: String) -> String {
    let before = output[..start].bytes().last();
    let after = output[end..].bytes().next();
    let (first, last) = if replacement.is_empty() {
        (after, before)
    } else {
        (replacement.bytes().next(), replacement.bytes().last())
    };
    let leading = would_lex_as_one(before, first);
    let trailing = !replacement.is_empty() && would_lex_as_one(last, after);
    if !leading && !trailing {
        return replacement;
    }
    let mut spaced = String::with_capacity(replacement.len() + 2);
    if leading {
        spaced.push(' ');
    }
    spaced.push_str(&replacement);
    if trailing {
        spaced.push(' ');
    }
    spaced
}

fn would_lex_as_one(left: Option<u8>, right: Option<u8>) -> bool {
    let (Some(left), Some(right)) = (left, right) else {
        return false;
    };
    (is_identifier_char_broad(left) && is_identifier_char_broad(right))
        || (left == b'+' && right == b'+')
        || (left == b'-' && right == b'-')
        || (left == b'/' && right == b'/')
}

/// Identifier characters as a printer sees them: ASCII identifier bytes, digits
/// (`return 1`), and every non-ASCII byte, which can only belong to an
/// identifier or to text a token-aligned splice never cuts through.
fn is_identifier_char_broad(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$' || byte == b'\\' || byte >= 0x80
}

pub(crate) fn rewrite_identifier_span(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
    from: &str,
    to: &str,
) -> String {
    if start >= end {
        return String::new();
    }
    let mut output = String::new();
    let mut cursor = tokens[start].start;
    for (offset, token) in tokens[start..end].iter().enumerate() {
        output.push_str(&source[cursor..token.start]);
        if token.kind == TokenKind::Identifier
            && token.text == from
            && !is_property_identifier(tokens, start + offset)
        {
            output.push_str(to);
        } else {
            output.push_str(token.text);
        }
        cursor = token.end;
    }
    output
}

pub(crate) fn is_statement_boundary(tokens: &[Token<'_>], index: usize) -> bool {
    let prev = index
        .checked_sub(1)
        .map(|prev| tokens[prev].text)
        .unwrap_or(";");
    if matches!(prev, ";" | "{" | "}") {
        return true;
    }
    prev == ":" && colon_starts_a_statement(tokens, index - 1)
}

fn colon_starts_a_statement(tokens: &[Token<'_>], colon: usize) -> bool {
    if colon >= 1 && tokens[colon - 1].kind == TokenKind::Identifier {
        let label_at = colon - 1;
        if label_at == 0 || matches!(tokens[label_at - 1].text, ";" | "{" | "}") {
            return true;
        }
    }
    let mut depth = 0i32;
    let mut index = colon;
    while index > 0 {
        index -= 1;
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            "?" if depth == 0 => return false,
            "case" | "default" if depth == 0 => return true,
            ";" if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

pub(crate) fn parse_bare_assign<'src>(
    source: &'src str,
    tokens: &[Token<'src>],
    at: usize,
) -> Option<(usize, &'src str, &'src str, usize)> {
    if !is_statement_boundary(tokens, at) || tokens.get(at)?.kind != TokenKind::Identifier {
        return None;
    }
    if tokens.get(at + 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let semi = top_level_stop(tokens, at + 2, &[";"])?;
    Some((
        at,
        tokens[at].text,
        &source[tokens[at + 2].start..tokens[semi].start],
        semi + 1,
    ))
}

pub(crate) fn assign_is_in_declaration(tokens: &[Token<'_>], at: usize) -> bool {
    let mut depth = 0i32;
    for index in (0..at).rev() {
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
            }
            ";" if depth == 0 => return false,
            "var" | "let" | "const" if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub(crate) fn next_statement_end(tokens: &[Token<'_>], start: usize) -> usize {
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.text {
            "(" | "[" => depth_paren += 1,
            ")" | "]" => depth_paren -= 1,
            "{" => depth_brace += 1,
            "}" => {
                if depth_brace == 0 && depth_paren == 0 {
                    return index;
                }
                depth_brace -= 1;
            }
            ";" if depth_paren == 0 && depth_brace == 0 => return index,
            _ => {}
        }
    }
    tokens.len()
}

const PRIMARY_EXPRESSION_TIGHTNESS: u8 = 100;

pub(crate) fn binary_operator_tightness(operator: &str) -> Option<u8> {
    Some(match operator {
        "," => 0,
        "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "**=" | "&=" | "|=" | "^=" | "<<=" | ">>="
        | ">>>=" | "&&=" | "||=" | "??=" => 1,
        "?" => 2,
        "??" | "||" => 3,
        "&&" => 4,
        "|" => 5,
        "^" => 6,
        "&" => 7,
        "==" | "===" | "!=" | "!==" => 8,
        "<" | ">" | "<=" | ">=" | "in" | "instanceof" => 9,
        "<<" | ">>" | ">>>" => 10,
        "+" | "-" => 11,
        "*" | "/" | "%" => 12,
        "**" => 13,
        _ => return None,
    })
}

fn postfix_operator_tightness(operator: &str) -> Option<u8> {
    matches!(operator, "." | "?." | "[" | "(" | "++" | "--" | "**").then_some(15)
}

fn prefix_operator_tightness(operator: &str) -> Option<u8> {
    matches!(
        operator,
        "!" | "~"
            | "+"
            | "-"
            | "++"
            | "--"
            | "typeof"
            | "void"
            | "delete"
            | "await"
            | "new"
            | "yield"
    )
    .then_some(14)
}

fn fragment_loosest_tightness(expression: &str) -> u8 {
    let Ok(tokens) = lex(expression) else {
        return 0;
    };
    let mut depth = 0usize;
    let mut loosest = PRIMARY_EXPRESSION_TIGHTNESS;
    for token in &tokens {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            operator if depth == 0 => {
                if let Some(tightness) = binary_operator_tightness(operator) {
                    loosest = loosest.min(tightness);
                }
            }
            _ => {}
        }
    }
    loosest
}

fn previous_token_is_operand(tokens: &[Token<'_>], operator_at: usize) -> bool {
    let Some(previous) = operator_at.checked_sub(1).map(|index| tokens[index].text) else {
        return false;
    };
    matches!(
        previous,
        ")" | "]" | "}" | "this" | "true" | "false" | "null" | "undefined" | "++" | "--"
    ) || tokens.get(operator_at - 1).is_some_and(|token| {
        matches!(
            token.kind,
            TokenKind::Identifier
                | TokenKind::Number
                | TokenKind::String
                | TokenKind::Regex
                | TokenKind::Template
        )
    })
}

pub(crate) fn substituted_expression_needs_grouping(
    tokens: &[Token<'_>],
    use_at: usize,
    expression: &str,
) -> bool {
    // An identifier is a JS Primary. Pasting any looser expression into that
    // slot must follow neighboring operators, not a special case for `|0`.
    let tightness = fragment_loosest_tightness(expression);
    if tightness >= PRIMARY_EXPRESSION_TIGHTNESS {
        return false;
    }
    if let Some(next) = tokens.get(use_at + 1).map(|token| token.text) {
        if postfix_operator_tightness(next).is_some_and(|next| next > tightness)
            || binary_operator_tightness(next).is_some_and(|next| next > tightness)
        {
            return true;
        }
    }
    let Some(previous_at) = use_at.checked_sub(1) else {
        return false;
    };
    let previous = tokens[previous_at].text;
    if prefix_operator_tightness(previous).is_some()
        && !previous_token_is_operand(tokens, previous_at)
    {
        return true;
    }
    binary_operator_tightness(previous).is_some_and(|previous| previous >= tightness)
}

pub(crate) fn wrap_substituted_expression(
    tokens: &[Token<'_>],
    use_at: usize,
    expression: &str,
) -> String {
    if substituted_expression_needs_grouping(tokens, use_at, expression) {
        format!("({expression})")
    } else {
        expression.to_string()
    }
}

pub(crate) fn conditional_test_needs_grouping(tokens: &[Token<'_>]) -> bool {
    let mut depth = 0usize;
    for token in tokens {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," | "?" | "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "&=" | "|=" | "^=" | "<<="
            | ">>=" | ">>>="
                if depth == 0 =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn expression_has_top_level_token(tokens: &[Token<'_>], expected: &str) -> bool {
    let mut depth = 0usize;
    for token in tokens {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            token if token == expected && depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Keep the outermost of any overlapping rewrite so nested candidates cannot
/// splice into a range that already moved.
pub(crate) fn non_overlapping_ranges(
    mut replacements: Vec<(usize, usize, String)>,
) -> Vec<(usize, usize, String)> {
    replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
    let mut retained = Vec::<(usize, usize, String)>::new();
    let mut last_end = 0;
    for replacement in replacements {
        if replacement.0 >= last_end {
            last_end = replacement.1;
            retained.push(replacement);
        }
    }
    retained
}

pub(crate) fn apply_rewrites(source: &str, rewrites: &[Rewrite]) -> String {
    if rewrites.is_empty() {
        return source.to_string();
    }
    let saved = rewrites
        .iter()
        .map(|rewrite| rewrite.end.saturating_sub(rewrite.start))
        .sum::<usize>();
    let mut output = String::with_capacity(source.len().saturating_sub(saved / 4));
    let mut cursor = 0;
    for rewrite in rewrites {
        output.push_str(&source[cursor..rewrite.start]);
        output.push_str(&source[rewrite.identifier_start..rewrite.identifier_end]);
        output.push_str(rewrite.operator);
        output.push('=');
        output.push_str(&source[rewrite.rhs_start..rewrite.rhs_end]);
        cursor = rewrite.end;
    }
    output.push_str(&source[cursor..]);
    output
}

pub(crate) fn non_overlapping_rewrites(rewrites: Vec<Rewrite>) -> Vec<Rewrite> {
    let mut retained = Vec::with_capacity(rewrites.len());
    let mut end = 0;
    for rewrite in rewrites {
        if rewrite.start >= end {
            end = rewrite.end;
            retained.push(rewrite);
        }
    }
    retained
}
