use std::collections::{HashMap, HashSet};

use crate::js_peephole::rewrite::{
    apply_token_rewrites, identifier_occurs, is_property_identifier, is_statement_boundary,
};
use crate::js_peephole::scope::{
    collect_same_scope_name_uses, enclosing_function_span, parse_function_expression,
};
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

const BITWISE_OPS: &[&str] = &["&", "|", "^", "<<", ">>", ">>>"];

pub(crate) fn fold_int32_coercions(source: &str) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let helpers = int32_coerce_helpers(&tokens);
    let property_helpers = int32_property_helpers(&tokens);
    let has_helpers = has_predicate_helpers(&tokens, &matching_close);
    let bitwise_callees = bitwise_first_param_callees(&tokens, &matching_close);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if let Some(next) =
            fold_known_integer_length(source, &tokens, &matching_close, cursor, &mut replacements)
        {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_indexof_int32(&tokens, &matching_close, cursor, &mut replacements)
        {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_has_predicate_calls(
            source,
            &tokens,
            &matching_close,
            &has_helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_property_int32_calls(
            source,
            &tokens,
            &matching_close,
            &property_helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_index_int32_postfix(
            source,
            &tokens,
            &matching_close,
            &helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_member_int32_update(
            source,
            &tokens,
            &matching_close,
            &helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_temp_int32_decrement(
            source,
            &tokens,
            &matching_close,
            &helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_coerce_before_bitwise(
            source,
            &tokens,
            &matching_close,
            &helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_bitwise_only_coerce_temp(
            source,
            &tokens,
            &matching_close,
            &helpers,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_unary_plus_on_numeric_literal(&tokens, cursor, &mut replacements) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_unary_plus_before_bitwise(&tokens, cursor, &mut replacements) {
            cursor = next;
            continue;
        }
        if let Some(next) =
            fold_grouped_plus_int32(source, &tokens, &matching_close, cursor, &mut replacements)
        {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_int32_arg_to_bitwise_callee(
            &tokens,
            &matching_close,
            &bitwise_callees,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) = fold_int32_temp_to_bitwise_callee(
            &tokens,
            &matching_close,
            &bitwise_callees,
            cursor,
            &mut replacements,
        ) {
            cursor = next;
            continue;
        }
        if let Some(next) =
            fold_bitflag_field_update(source, &tokens, &matching_close, cursor, &mut replacements)
        {
            cursor = next;
            continue;
        }
        if let Some(next) =
            fold_xor_minus_one(source, &tokens, &matching_close, cursor, &mut replacements)
        {
            cursor = next;
            continue;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn int32_coerce_helpers<'a>(tokens: &'a [Token<'a>]) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    let mut assigned = HashSet::new();
    let mut rejected = HashSet::new();
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Identifier || is_property_identifier(tokens, index) {
            continue;
        }
        if tokens.get(index + 1).map(|token| token.text) != Some("=") {
            continue;
        }
        let name = tokens[index].text;
        if !assigned.insert(name) {
            rejected.insert(name);
            continue;
        }
        if is_int32_coerce_arrow(tokens, index + 2) {
            names.insert(name);
        }
    }
    names.retain(|name| !rejected.contains(name));
    names
}

fn int32_property_helpers<'a>(tokens: &'a [Token<'a>]) -> HashMap<&'a str, &'a str> {
    let mut names = HashMap::new();
    let mut assigned = HashSet::new();
    let mut rejected = HashSet::new();
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Identifier || is_property_identifier(tokens, index) {
            continue;
        }
        if tokens.get(index + 1).map(|token| token.text) != Some("=") {
            continue;
        }
        let name = tokens[index].text;
        if !assigned.insert(name) {
            rejected.insert(name);
            continue;
        }
        if let Some(property) = is_property_int32_arrow(tokens, index + 2) {
            names.insert(name, property);
        }
    }
    names.retain(|name, _| !rejected.contains(name));
    names
}

fn is_property_int32_arrow<'a>(tokens: &'a [Token<'a>], at: usize) -> Option<&'a str> {
    let param = tokens.get(at)?;
    if param.kind != TokenKind::Identifier {
        return None;
    }
    if tokens.get(at + 1).map(|token| token.text) != Some("=>") {
        return None;
    }
    let mut body = at + 2;
    if tokens.get(body).map(|token| token.text) == Some("+") {
        body += 1;
    }
    if tokens.get(body).map(|token| token.text) != Some(param.text)
        || tokens.get(body + 1).map(|token| token.text) != Some(".")
        || tokens
            .get(body + 2)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(body + 3).map(|token| token.text) != Some("|")
        || tokens.get(body + 4).map(|token| token.text) != Some("0")
    {
        return None;
    }
    if is_expression_continuation(tokens.get(body + 5).map(|token| token.text)) {
        return None;
    }
    Some(tokens[body + 2].text)
}

fn has_predicate_helpers<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    let mut assigned = HashSet::new();
    let mut rejected = HashSet::new();
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Identifier || is_property_identifier(tokens, index) {
            continue;
        }
        if tokens.get(index + 1).map(|token| token.text) != Some("=") {
            continue;
        }
        let name = tokens[index].text;
        if !assigned.insert(name) {
            rejected.insert(name);
            continue;
        }
        if is_has_predicate_arrow(tokens, matching_close, index + 2) {
            names.insert(name);
        }
    }
    names.retain(|name| !rejected.contains(name));
    names
}

fn paren_opens_call_or_new(tokens: &[Token<'_>], open: usize) -> bool {
    let Some(prev) = open.checked_sub(1).map(|index| &tokens[index]) else {
        return false;
    };
    match prev.kind {
        TokenKind::Identifier | TokenKind::String | TokenKind::Template => true,
        TokenKind::Keyword if matches!(prev.text, "this" | "super" | "import") => true,
        TokenKind::Punct if matches!(prev.text, ")" | "]" | "?." | "?.[") => true,
        _ => false,
    }
}

fn fold_indexof_int32(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens.get(cursor).map(|token| token.text) != Some(".")
        || tokens.get(cursor + 1).map(|token| token.text) != Some("indexOf")
        || tokens.get(cursor + 2).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let close = matching_close.get(cursor + 2).copied().flatten()?;
    if tokens.get(close + 1).map(|token| token.text) != Some("|")
        || tokens.get(close + 2).map(|token| token.text) != Some("0")
    {
        return None;
    }
    replacements.push((
        tokens[close + 1].start,
        tokens[close + 2].end,
        String::new(),
    ));
    Some(close + 3)
}

fn grouping_has_top_level_comma(
    tokens: &[Token<'_>],
    open: usize,
    close: usize,
    matching_close: &[Option<usize>],
) -> bool {
    let mut index = open + 1;
    while index < close {
        match tokens[index].text {
            "(" | "[" | "{" => {
                index = matching_close
                    .get(index)
                    .copied()
                    .flatten()
                    .map(|close| close + 1)
                    .unwrap_or(index + 1);
            }
            "," => return true,
            _ => index += 1,
        }
    }
    false
}

fn fold_known_integer_length(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens.get(cursor).map(|token| token.text) == Some("(")
        && !paren_opens_call_or_new(tokens, cursor)
    {
        let close = matching_close.get(cursor).copied().flatten()?;
        if close >= 4
            && tokens.get(close - 1).map(|token| token.text) == Some("0")
            && tokens.get(close - 2).map(|token| token.text) == Some("|")
            && tokens
                .get(close - 3)
                .is_some_and(|token| matches!(token.text, "length" | "size"))
            && tokens.get(close - 4).map(|token| token.text) == Some(".")
            && !grouping_has_top_level_comma(tokens, cursor, close, matching_close)
        {
            let recv_from = cursor + 1;
            let recv_to = close - 4;
            if recv_from < recv_to {
                let recv = source[tokens[recv_from].start..tokens[recv_to].start].to_string();
                // `(+array.length|0)` may be the right operand of another
                // addition. Keeping its redundant unary plus while removing
                // the grouping would turn `left+(+array.length|0)` into the
                // invalid/token-changing `left++array.length`. Length and
                // size are already numeric, so discard only that leading
                // coercion together with the proven-redundant `|0`.
                let recv = recv.strip_prefix('+').unwrap_or(&recv);
                let property = tokens[close - 3].text;
                replacements.push((
                    tokens[cursor].start,
                    tokens[close].end,
                    format!("{recv}.{property}"),
                ));
                return Some(close + 1);
            }
        }
    }
    if tokens.get(cursor).map(|token| token.text) != Some(".")
        || !tokens
            .get(cursor + 1)
            .is_some_and(|token| matches!(token.text, "length" | "size"))
        || tokens.get(cursor + 2).map(|token| token.text) != Some("|")
        || tokens.get(cursor + 3).map(|token| token.text) != Some("0")
    {
        return None;
    }
    replacements.push((
        tokens[cursor + 2].start,
        tokens[cursor + 3].end,
        String::new(),
    ));
    Some(cursor + 4)
}

fn is_has_predicate_arrow(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> bool {
    if tokens.get(at).map(|token| token.text) != Some("(") {
        return false;
    }
    let Some(close) = matching_close.get(at).copied().flatten() else {
        return false;
    };
    if close != at + 4
        || tokens[at + 1].kind != TokenKind::Identifier
        || tokens[at + 2].text != ","
        || tokens[at + 3].kind != TokenKind::Identifier
    {
        return false;
    }
    let left = tokens[at + 1].text;
    let right = tokens[at + 3].text;
    if tokens.get(close + 1).map(|token| token.text) != Some("=>") {
        return false;
    }
    let mut body = close + 2;
    if tokens.get(body).map(|token| token.text) == Some("!")
        && tokens.get(body + 1).map(|token| token.text) == Some("!")
    {
        body += 2;
    }
    tokens.get(body).map(|token| token.text) == Some(left)
        && tokens.get(body + 1).map(|token| token.text) == Some(".")
        && tokens.get(body + 2).map(|token| token.text) == Some("has")
        && tokens.get(body + 3).map(|token| token.text) == Some("(")
        && tokens.get(body + 4).map(|token| token.text) == Some(right)
        && tokens.get(body + 5).map(|token| token.text) == Some(")")
        && !is_expression_continuation(tokens.get(body + 6).map(|token| token.text))
}

fn fold_has_predicate_calls(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if is_property_identifier(tokens, cursor)
        || !tokens.get(cursor).is_some_and(|token| {
            token.kind == TokenKind::Identifier && helpers.contains(token.text)
        })
        || tokens.get(cursor + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let close = matching_close.get(cursor + 1).copied().flatten()?;
    let comma = crate::js_peephole::rewrite::top_level_stop(tokens, cursor + 2, &[","])?;
    if comma >= close {
        return None;
    }
    let receiver = bitwise_operand_text(source, tokens, matching_close, cursor + 2, comma);
    let key = &source[tokens[comma + 1].start..tokens[close].start];
    if key.trim().is_empty() {
        return None;
    }
    replacements.push((
        tokens[cursor].start,
        tokens[close].end,
        format!("{receiver}.has({key})"),
    ));
    Some(close + 1)
}

fn fold_property_int32_calls(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashMap<&str, &str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if is_property_identifier(tokens, cursor) {
        return None;
    }
    let property = *helpers.get(tokens.get(cursor).map(|token| token.text).unwrap_or(""))?;
    if tokens.get(cursor + 1).map(|token| token.text) != Some("(") {
        return None;
    }
    let close = matching_close.get(cursor + 1).copied().flatten()?;
    if close <= cursor + 2 {
        return None;
    }
    if matches!(
        tokens.get(close + 1).map(|token| token.text),
        Some("(") | Some("[") | Some(".") | Some("?.") | Some("++") | Some("--")
    ) {
        return None;
    }
    let operand = bitwise_operand_text(source, tokens, matching_close, cursor + 2, close);
    replacements.push((
        tokens[cursor].start,
        tokens[close].end,
        format!("{operand}.{property}"),
    ));
    Some(close + 1)
}

fn is_int32_coerce_arrow(tokens: &[Token<'_>], at: usize) -> bool {
    let Some(param) = tokens.get(at) else {
        return false;
    };
    if param.kind != TokenKind::Identifier {
        return false;
    }
    if tokens.get(at + 1).map(|token| token.text) != Some("=>") {
        return false;
    }
    let body = at + 2;
    let (zero_at, ok) = if tokens.get(body).map(|token| token.text) == Some("+")
        && tokens.get(body + 1).map(|token| token.text) == Some(param.text)
        && tokens.get(body + 2).map(|token| token.text) == Some("|")
        && tokens.get(body + 3).map(|token| token.text) == Some("0")
    {
        (body + 3, true)
    } else if tokens.get(body).map(|token| token.text) == Some(param.text)
        && tokens.get(body + 1).map(|token| token.text) == Some("|")
        && tokens.get(body + 2).map(|token| token.text) == Some("0")
    {
        (body + 2, true)
    } else {
        (0, false)
    };
    ok && !is_expression_continuation(tokens.get(zero_at + 1).map(|token| token.text))
}

fn is_expression_continuation(text: Option<&str>) -> bool {
    matches!(
        text,
        Some(
            "+" | "-"
                | "*"
                | "/"
                | "%"
                | "**"
                | "("
                | "["
                | "."
                | "?"
                | "&&"
                | "||"
                | "??"
                | "&"
                | "|"
                | "^"
                | "="
                | "++"
                | "--"
        )
    )
}

fn is_lvalue_head(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Identifier || token.text == "this"
}

fn lvalue_range(tokens: &[Token<'_>], start: usize) -> Option<(usize, usize)> {
    if start >= tokens.len()
        || is_property_identifier(tokens, start)
        || !is_lvalue_head(&tokens[start])
    {
        return None;
    }
    let mut end = start + 1;
    while tokens.get(end).map(|token| token.text) == Some(".")
        && tokens
            .get(end + 1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        end += 2;
    }
    Some((start, end))
}

fn lvalue_eq(tokens: &[Token<'_>], left: (usize, usize), right: (usize, usize)) -> bool {
    if left.1 - left.0 != right.1 - right.0 {
        return false;
    }
    tokens[left.0..left.1]
        .iter()
        .zip(&tokens[right.0..right.1])
        .all(|(a, b)| a.text == b.text)
}

fn lvalue_text(source: &str, tokens: &[Token<'_>], range: (usize, usize)) -> String {
    source[tokens[range.0].start..tokens[range.1 - 1].end].to_string()
}

fn coerce_call(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    at: usize,
) -> Option<(usize, usize, usize)> {
    if is_property_identifier(tokens, at) {
        return None;
    }
    if !tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier && helpers.contains(token.text))
    {
        return None;
    }
    if tokens.get(at + 1).map(|token| token.text) != Some("(") {
        return None;
    }
    let close = matching_close.get(at + 1).copied().flatten()?;
    if close <= at + 2 {
        return None;
    }
    Some((at, at + 2, close + 1))
}

fn grouped_int32_lvalue(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<((usize, usize), usize)> {
    if tokens.get(at).map(|token| token.text) != Some("(") {
        return None;
    }
    let close = matching_close.get(at).copied().flatten()?;
    let inner = at + 1;
    let lvalue_at = if tokens.get(inner).map(|token| token.text) == Some("+") {
        inner + 1
    } else {
        inner
    };
    let lvalue = lvalue_range(tokens, lvalue_at)?;
    if tokens.get(lvalue.1).map(|token| token.text) != Some("|")
        || tokens.get(lvalue.1 + 1).map(|token| token.text) != Some("0")
        || lvalue.1 + 2 != close
    {
        return None;
    }
    Some((lvalue, close + 1))
}

fn int32_lvalue_read(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    at: usize,
) -> Option<((usize, usize), usize)> {
    if let Some((_, arg_start, call_end)) = coerce_call(tokens, matching_close, helpers, at) {
        let lvalue = lvalue_range(tokens, arg_start)?;
        if lvalue.1 + 1 != call_end {
            return None;
        }
        return Some((lvalue, call_end));
    }
    grouped_int32_lvalue(tokens, matching_close, at)
}

fn consume_unit_int32_tail(tokens: &[Token<'_>], at: usize) -> Option<(char, usize)> {
    let op = tokens.get(at).map(|token| token.text)?;
    if op != "+" && op != "-" {
        return None;
    }
    if tokens.get(at + 1).map(|token| token.text) != Some("1") {
        return None;
    }
    let mut end = at + 2;
    if tokens.get(end).map(|token| token.text) == Some("|")
        && tokens.get(end + 1).map(|token| token.text) == Some("0")
    {
        end += 2;
    }
    Some((if op == "+" { '+' } else { '-' }, end))
}

fn fold_member_int32_update(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    let lhs = lvalue_range(tokens, cursor)?;
    if tokens.get(lhs.1).map(|token| token.text) != Some("=") {
        return None;
    }
    let (rhs, read_end) = int32_lvalue_read(tokens, matching_close, helpers, lhs.1 + 1)?;
    if !lvalue_eq(tokens, lhs, rhs) {
        return None;
    }
    let (sign, end) = consume_unit_int32_tail(tokens, read_end)?;
    let op = if sign == '+' { "++" } else { "--" };
    replacements.push((
        tokens[lhs.0].start,
        tokens[end - 1].end,
        format!("{}{op}", lvalue_text(source, tokens, lhs)),
    ));
    Some(end)
}

fn fold_temp_int32_decrement(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens[cursor].kind != TokenKind::Identifier || is_property_identifier(tokens, cursor) {
        return None;
    }
    if tokens.get(cursor + 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let temp = tokens[cursor].text;
    let (member, read_end) = int32_lvalue_read(tokens, matching_close, helpers, cursor + 2)?;
    let (sign, value_end) = consume_unit_int32_tail(tokens, read_end)?;
    if sign != '-' {
        return None;
    }
    if !matches!(
        tokens.get(value_end).map(|token| token.text),
        Some(";") | Some(",")
    ) {
        return None;
    }
    let assign_at = value_end + 1;
    let assigned = lvalue_range(tokens, assign_at)?;
    if !lvalue_eq(tokens, member, assigned) {
        return None;
    }
    if tokens.get(assigned.1).map(|token| token.text) != Some("=")
        || tokens.get(assigned.1 + 1).map(|token| token.text) != Some(temp)
    {
        return None;
    }
    replacements.push((
        tokens[cursor].start,
        tokens[assigned.1 + 1].end,
        format!("{temp}=--{}", lvalue_text(source, tokens, member)),
    ));
    Some(assigned.1 + 2)
}

fn fold_index_int32_postfix(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens[cursor].kind != TokenKind::Identifier || is_property_identifier(tokens, cursor) {
        return None;
    }
    if tokens.get(cursor + 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let temp = tokens[cursor].text;
    let (member, read_end) = int32_lvalue_read(tokens, matching_close, helpers, cursor + 2)?;
    if !matches!(
        tokens.get(read_end).map(|token| token.text),
        Some(",") | Some(";")
    ) {
        return None;
    }
    let array_at = read_end + 1;
    let array = lvalue_range(tokens, array_at)?;
    if tokens.get(array.1).map(|token| token.text) != Some("[")
        || tokens.get(array.1 + 1).map(|token| token.text) != Some(temp)
        || tokens.get(array.1 + 2).map(|token| token.text) != Some("]")
        || tokens.get(array.1 + 3).map(|token| token.text) != Some("=")
    {
        return None;
    }
    let rhs_at = array.1 + 4;
    let rhs_stop = crate::js_peephole::rewrite::top_level_stop(tokens, rhs_at, &[",", ";", ")"])?;
    if identifier_occurs(tokens, rhs_at, rhs_stop, temp) {
        return None;
    }
    if !matches!(tokens[rhs_stop].text, "," | ";") {
        return None;
    }
    let store = lvalue_range(tokens, rhs_stop + 1)?;
    if !lvalue_eq(tokens, member, store) {
        return None;
    }
    if tokens.get(store.1).map(|token| token.text) != Some("=")
        || tokens.get(store.1 + 1).map(|token| token.text) != Some(temp)
        || tokens.get(store.1 + 2).map(|token| token.text) != Some("+")
        || tokens.get(store.1 + 3).map(|token| token.text) != Some("1")
    {
        return None;
    }
    let mut end = store.1 + 4;
    if tokens.get(end).map(|token| token.text) == Some("|")
        && tokens.get(end + 1).map(|token| token.text) == Some("0")
    {
        end += 2;
    }
    let scope_end = enclosing_function_span(tokens, matching_close, cursor)
        .map(|(_, close)| close)
        .unwrap_or(tokens.len());
    if identifier_occurs(tokens, end, scope_end, temp) {
        return None;
    }
    let rhs = &source[tokens[rhs_at].start..tokens[rhs_stop].start];
    replacements.push((
        tokens[cursor].start,
        tokens[end - 1].end,
        format!(
            "{}[{}++]={rhs}",
            lvalue_text(source, tokens, array),
            lvalue_text(source, tokens, member)
        ),
    ));
    Some(end)
}

fn operand_needs_grouping(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
) -> bool {
    if start >= end {
        return true;
    }
    if tokens[start].text == "(" && matching_close.get(start).copied().flatten() == Some(end - 1) {
        return false;
    }
    let mut depth = 0i32;
    for (offset, token) in tokens[start..end].iter().enumerate() {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ if depth == 0
                && matches!(
                    token.text,
                    "+" | "-"
                        | ","
                        | "?"
                        | ":"
                        | "="
                        | "&&"
                        | "||"
                        | "??"
                        | "|"
                        | "^"
                        | "&"
                        | "=="
                        | "!="
                        | "==="
                        | "!=="
                        | "<"
                        | ">"
                        | "<="
                        | ">="
                        | "in"
                        | "instanceof"
                ) =>
            {
                if offset == 0 && matches!(token.text, "+" | "-" | "!" | "~") {
                    continue;
                }
                return true;
            }
            _ => {}
        }
    }
    false
}

fn bitwise_operand_text(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
) -> String {
    let inner = &source[tokens[start].start..tokens[end - 1].end];
    if operand_needs_grouping(tokens, matching_close, start, end) {
        format!("({inner})")
    } else {
        inner.to_string()
    }
}

fn fold_coerce_before_bitwise(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    let (call_start, arg_start, call_end) = coerce_call(tokens, matching_close, helpers, cursor)?;
    let after = tokens.get(call_end).map(|token| token.text)?;
    if !BITWISE_OPS.contains(&after) {
        return None;
    }
    let operand = bitwise_operand_text(source, tokens, matching_close, arg_start, call_end - 1);
    replacements.push((tokens[call_start].start, tokens[call_end - 1].end, operand));
    Some(call_end)
}

fn remaining_ident_uses_are_bitwise(
    tokens: &[Token<'_>],
    from: usize,
    to: usize,
    name: &str,
) -> bool {
    let mut saw = false;
    let mut index = from;
    while index < to {
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && !is_property_identifier(tokens, index)
        {
            let previous = index
                .checked_sub(1)
                .map(|prev| tokens[prev].text)
                .unwrap_or(";");
            if matches!(previous, "var" | "let" | "const") {
                index += 1;
                continue;
            }
            let next = tokens.get(index + 1).map(|token| token.text);
            if next == Some("=") {
                index += 1;
                continue;
            }
            if !next.is_some_and(|op| BITWISE_OPS.contains(&op)) {
                return false;
            }
            saw = true;
        }
        index += 1;
    }
    saw
}

fn fold_bitwise_only_coerce_temp(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    helpers: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens[cursor].kind != TokenKind::Identifier || is_property_identifier(tokens, cursor) {
        return None;
    }
    if tokens.get(cursor + 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let temp = tokens[cursor].text;
    let (call_start, arg_start, call_end) =
        coerce_call(tokens, matching_close, helpers, cursor + 2)?;
    if call_start != cursor + 2 {
        return None;
    }
    let scope_end = enclosing_function_span(tokens, matching_close, cursor)
        .map(|(_, close)| close)
        .unwrap_or(tokens.len());
    if !remaining_ident_uses_are_bitwise(tokens, call_end, scope_end, temp) {
        return None;
    }
    let operand = bitwise_operand_text(source, tokens, matching_close, arg_start, call_end - 1);
    replacements.push((tokens[call_start].start, tokens[call_end - 1].end, operand));
    Some(call_end)
}

fn simple_member_end(tokens: &[Token<'_>], at: usize) -> Option<usize> {
    if tokens.get(at).map(|token| token.text) == Some("this")
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens
            .get(at + 2)
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
    {
        return Some(at + 2);
    }
    if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens
            .get(at + 2)
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
    {
        return Some(at + 2);
    }
    if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return Some(at);
    }
    None
}

fn plus_is_unary(tokens: &[Token<'_>], cursor: usize) -> bool {
    let Some(prev) = cursor.checked_sub(1).map(|index| &tokens[index]) else {
        return true;
    };
    match prev.kind {
        TokenKind::Identifier | TokenKind::Number | TokenKind::String | TokenKind::Template => {
            false
        }
        TokenKind::Keyword if matches!(prev.text, "this" | "super" | "true" | "false" | "null") => {
            false
        }
        TokenKind::Punct if matches!(prev.text, ")" | "]" | "}" | "++" | "--") => false,
        _ => true,
    }
}

/// `+7227` → `7227`, `a-+1` → `a-1`, `b/+18` → `b/18`: a number coercion of a
/// numeric literal is the literal (Terser `evaluate`, Oxc
/// `constant_evaluation/mod.rs:489`). The emitter writes the coercion for a
/// `JsValue` operand and constant propagation later makes the operand a
/// literal, so the `+` survives to the artifact: 101 sites on katexlil (047).
/// A `+` that follows `+`, `++`, or a binary `+` operand is not unary and is
/// left alone; the splice guard keeps `a+ +1` from fusing.
fn fold_unary_plus_on_numeric_literal(
    tokens: &[Token<'_>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens.get(cursor).map(|token| token.text) != Some("+") || !plus_is_unary(tokens, cursor) {
        return None;
    }
    let literal = tokens.get(cursor + 1)?;
    if literal.kind != TokenKind::Number
        || literal.text.starts_with('.')
        || !literal.text.bytes().next().is_some_and(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    // `+ +1` and `- +1` stay legal after the splice; `++1` would not.
    if cursor
        .checked_sub(1)
        .is_some_and(|index| matches!(tokens[index].text, "+" | "++"))
    {
        return None;
    }
    replacements.push((tokens[cursor].start, tokens[cursor].end, String::new()));
    Some(cursor + 2)
}

fn fold_unary_plus_before_bitwise(
    tokens: &[Token<'_>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens.get(cursor).map(|token| token.text) != Some("+") || !plus_is_unary(tokens, cursor) {
        return None;
    }
    let operand_end = simple_member_end(tokens, cursor + 1)?;
    if !tokens
        .get(operand_end + 1)
        .is_some_and(|token| BITWISE_OPS.contains(&token.text))
    {
        return None;
    }
    replacements.push((tokens[cursor].start, tokens[cursor].end, String::new()));
    Some(cursor + 1)
}

fn fold_grouped_plus_int32(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens.get(cursor).map(|token| token.text) != Some("(")
        || paren_opens_call_or_new(tokens, cursor)
    {
        return None;
    }
    let close = matching_close.get(cursor).copied().flatten()?;
    if tokens.get(cursor + 1).map(|token| token.text) != Some("+") {
        return None;
    }
    let operand_end = simple_member_end(tokens, cursor + 2)?;
    if tokens.get(operand_end + 1).map(|token| token.text) != Some("|")
        || tokens.get(operand_end + 2).map(|token| token.text) != Some("0")
        || operand_end + 3 != close
    {
        return None;
    }
    let next = tokens.get(close + 1).map(|token| token.text);
    let prev = cursor
        .checked_sub(1)
        .map(|index| tokens[index].text)
        .unwrap_or(";");
    if !next.is_some_and(|op| BITWISE_OPS.contains(&op))
        && !matches!(prev, "&" | "|" | "^" | "<<" | ">>" | ">>>")
    {
        return None;
    }
    let inner = source[tokens[cursor + 2].start..tokens[operand_end].end].to_string();
    replacements.push((tokens[cursor].start, tokens[close].end, inner));
    Some(close + 1)
}

fn fold_bitflag_field_update(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    let name_at = if matches!(
        tokens.get(cursor).map(|token| token.text),
        Some("var" | "let")
    ) && tokens
        .get(cursor + 1)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        cursor + 1
    } else if tokens
        .get(cursor)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        cursor
    } else {
        return None;
    };
    if tokens.get(name_at + 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let mut value_at = name_at + 2;
    if tokens.get(value_at).map(|token| token.text) == Some("+") {
        value_at += 1;
    }
    let field_end = simple_member_end(tokens, value_at)?;
    if field_end < value_at + 2
        || tokens.get(field_end + 1).map(|token| token.text) != Some("|")
        || tokens.get(field_end + 2).map(|token| token.text) != Some("0")
    {
        return None;
    }
    let temp = tokens[name_at].text;
    let field = &source[tokens[value_at].start..tokens[field_end].end];
    let after_init = field_end + 3;
    if !matches!(
        tokens.get(after_init).map(|token| token.text),
        Some(";") | Some(",")
    ) {
        return None;
    }
    let cond_at = after_init + 1;
    let qmark = crate::js_peephole::rewrite::top_level_stop(tokens, cond_at, &["?"])?;
    if tokens.get(qmark).map(|token| token.text) != Some("?") {
        return None;
    }
    if tokens.get(qmark + 1).map(|token| token.text) != Some(tokens[value_at].text) {
        return None;
    }
    let then_field_end = simple_member_end(tokens, qmark + 1)?;
    if &source[tokens[qmark + 1].start..tokens[then_field_end].end] != field
        || tokens.get(then_field_end + 1).map(|token| token.text) != Some("=")
        || tokens.get(then_field_end + 2).map(|token| token.text) != Some(temp)
        || tokens.get(then_field_end + 3).map(|token| token.text) != Some("|")
    {
        return None;
    }
    let colon = crate::js_peephole::rewrite::top_level_stop(tokens, then_field_end + 4, &[":"])?;
    if tokens.get(colon).map(|token| token.text) != Some(":") {
        return None;
    }
    let mask = source[tokens[then_field_end + 4].start..tokens[colon].start].to_string();
    if mask.is_empty() {
        return None;
    }
    if tokens.get(colon + 1).map(|token| token.text) != Some(tokens[value_at].text) {
        return None;
    }
    let else_field_end = simple_member_end(tokens, colon + 1)?;
    if &source[tokens[colon + 1].start..tokens[else_field_end].end] != field
        || tokens.get(else_field_end + 1).map(|token| token.text) != Some("=")
        || tokens.get(else_field_end + 2).map(|token| token.text) != Some(temp)
        || tokens.get(else_field_end + 3).map(|token| token.text) != Some("&")
        || tokens.get(else_field_end + 4).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let and_close = matching_close.get(else_field_end + 4).copied().flatten()?;
    let and_inner = &source[tokens[else_field_end + 5].start..tokens[and_close].start];
    if and_inner != format!("{mask}^-1") && and_inner != format!("({mask}^-1)") {
        return None;
    }
    let cond = source[tokens[cond_at].start..tokens[qmark].start].to_string();
    let start = if matches!(tokens[cursor].text, "var" | "let") {
        tokens[cursor].start
    } else {
        tokens[name_at].start
    };
    replacements.push((
        start,
        tokens[and_close].end,
        format!(
            "{cond}?{field}|={mask}:{field}&={}",
            bitwise_not_operand(&mask)
        ),
    ));
    Some(and_close + 1)
}

fn bitwise_not_operand(mask: &str) -> String {
    if is_simple_bitwise_not_operand(mask) {
        format!("~{mask}")
    } else {
        format!("~({mask})")
    }
}

fn is_simple_bitwise_not_operand(mask: &str) -> bool {
    let mut chars = mask.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if !(first.is_ascii_alphabetic() || first == '_' || first == '$' || first.is_ascii_digit()) {
        return false;
    }
    chars.all(|character| {
        character.is_ascii_alphanumeric()
            || character == '_'
            || character == '$'
            || character == '.'
    }) && !mask.starts_with('.')
        && !mask.ends_with('.')
        && !mask.contains("..")
}

fn simple_primary_end(tokens: &[Token<'_>], start: usize) -> Option<usize> {
    let head = tokens.get(start)?;
    if head.kind != TokenKind::Identifier && head.kind != TokenKind::Number && head.text != "this" {
        return None;
    }
    let mut index = start + 1;
    while tokens.get(index).map(|token| token.text) == Some(".")
        && tokens
            .get(index + 1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        index += 2;
    }
    Some(index)
}

fn fold_xor_minus_one(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if tokens[cursor].text == "(" {
        let close = matching_close.get(cursor).copied().flatten()?;
        if close >= cursor + 5
            && tokens[close - 3].text == "^"
            && tokens[close - 2].text == "-"
            && tokens[close - 1].text == "1"
            && simple_primary_end(tokens, cursor + 1) == Some(close - 3)
        {
            let operand = &source[tokens[cursor + 1].start..tokens[close - 3].start];
            replacements.push((
                tokens[cursor].start,
                tokens[close].end,
                format!("~{operand}"),
            ));
            return Some(close + 1);
        }
    }
    if tokens[cursor].text == "&=" {
        let operand_from = cursor + 1;
        let operand_to = simple_primary_end(tokens, operand_from)?;
        if tokens.get(operand_to).map(|token| token.text) == Some("^")
            && tokens.get(operand_to + 1).map(|token| token.text) == Some("-")
            && tokens.get(operand_to + 2).map(|token| token.text) == Some("1")
        {
            let operand = &source[tokens[operand_from].start..tokens[operand_to].start];
            replacements.push((
                tokens[operand_from].start,
                tokens[operand_to + 2].end,
                format!("~{operand}"),
            ));
            return Some(operand_to + 3);
        }
    }
    None
}

fn bitwise_first_param_callees<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
) -> HashSet<&'a str> {
    let mut names = HashSet::new();
    let mut assigned = HashSet::new();
    let mut rejected = HashSet::new();
    for index in 0..tokens.len() {
        if tokens[index].text == "function"
            && is_statement_boundary(tokens, index)
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 2).map(|token| token.text) == Some("(")
        {
            let name = tokens[index + 1].text;
            if !assigned.insert(name) {
                rejected.insert(name);
                continue;
            }
            if function_first_param_is_bitwise_only(tokens, matching_close, index) {
                names.insert(name);
            }
            continue;
        }
        if tokens[index].kind != TokenKind::Identifier || is_property_identifier(tokens, index) {
            continue;
        }
        if tokens.get(index + 1).map(|token| token.text) != Some("=") {
            continue;
        }
        let name = tokens[index].text;
        if parse_function_expression(tokens, matching_close, index + 2).is_none() {
            continue;
        }
        if !assigned.insert(name) {
            rejected.insert(name);
            continue;
        }
        if function_expr_first_param_is_bitwise_only(tokens, matching_close, index + 2) {
            names.insert(name);
        }
    }
    names.retain(|name| !rejected.contains(name));
    names
}

fn function_first_param_is_bitwise_only(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    function_at: usize,
) -> bool {
    function_expr_first_param_is_bitwise_only(tokens, matching_close, function_at)
}

fn function_expr_first_param_is_bitwise_only(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    expr_at: usize,
) -> bool {
    let Some(expr) = parse_function_expression(tokens, matching_close, expr_at) else {
        return false;
    };
    if expr.params_from >= expr.params_to
        || tokens[expr.params_from].kind != TokenKind::Identifier
        || tokens[expr.params_from].text == "..."
    {
        return false;
    }
    if expr.params_from + 1 < expr.params_to && tokens[expr.params_from + 1].text != "," {
        return false;
    }
    let param = tokens[expr.params_from].text;
    let (body_from, body_end) = if let Some(block_open) = expr.block_open {
        (block_open + 1, expr.end)
    } else {
        let mut arrow = expr.params_to;
        while arrow < expr.end && tokens.get(arrow).map(|token| token.text) != Some("=>") {
            arrow += 1;
        }
        if tokens.get(arrow).map(|token| token.text) != Some("=>") {
            return false;
        }
        (arrow + 1, expr.end + 1)
    };
    let (uses, nested_use) = collect_same_scope_name_uses(
        tokens,
        matching_close,
        param,
        body_from,
        body_end,
        usize::MAX,
    );
    if nested_use || uses.is_empty() {
        return false;
    }
    uses.iter()
        .all(|&use_at| ident_use_is_bitwise(tokens, use_at))
}

fn ident_use_is_bitwise(tokens: &[Token<'_>], use_at: usize) -> bool {
    let next = tokens.get(use_at + 1).map(|token| token.text);
    if next.is_some_and(|op| BITWISE_OPS.contains(&op)) {
        return true;
    }
    let previous = use_at
        .checked_sub(1)
        .map(|index| tokens[index].text)
        .unwrap_or(";");
    BITWISE_OPS.contains(&previous)
}

fn fold_int32_arg_to_bitwise_callee(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    callees: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    if is_property_identifier(tokens, cursor)
        || !tokens.get(cursor).is_some_and(|token| {
            token.kind == TokenKind::Identifier && callees.contains(token.text)
        })
        || tokens.get(cursor + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let open = cursor + 1;
    let close = matching_close.get(open).copied().flatten()?;
    if close <= open + 1 {
        return None;
    }
    let (coerce_start, coerce_end) =
        trailing_int32_coerce_span(tokens, matching_close, open + 1, close)?;
    replacements.push((
        tokens[coerce_start].start,
        tokens[coerce_end].end,
        String::new(),
    ));
    Some(close + 1)
}

fn trailing_int32_coerce_span(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    arg_start: usize,
    call_close: usize,
) -> Option<(usize, usize)> {
    let mut depth = 0i32;
    let mut arg_end = call_close;
    let mut index = arg_start;
    while index < call_close {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => {
                arg_end = index;
                break;
            }
            _ => {}
        }
        index += 1;
    }
    if arg_end <= arg_start + 1 {
        return None;
    }
    if tokens.get(arg_end - 2).map(|token| token.text) == Some("|")
        && tokens.get(arg_end - 1).map(|token| token.text) == Some("0")
    {
        return Some((arg_end - 2, arg_end - 1));
    }
    if tokens.get(arg_start).map(|token| token.text) == Some("+")
        && plus_is_unary(tokens, arg_start)
        && tokens.get(arg_start + 1).map(|token| token.text) != Some("+")
    {
        return Some((arg_start, arg_start));
    }
    if tokens.get(arg_start).map(|token| token.text) == Some("(") {
        let inner_close = matching_close.get(arg_start).copied().flatten()?;
        if inner_close + 1 == arg_end
            && tokens.get(inner_close - 2).map(|token| token.text) == Some("|")
            && tokens.get(inner_close - 1).map(|token| token.text) == Some("0")
        {
            return Some((inner_close - 2, inner_close - 1));
        }
        if inner_close + 1 == arg_end
            && tokens.get(arg_start + 1).map(|token| token.text) == Some("+")
            && plus_is_unary(tokens, arg_start + 1)
            && tokens.get(arg_start + 2).map(|token| token.text) != Some("+")
        {
            return Some((arg_start + 1, arg_start + 1));
        }
    }
    None
}

fn fold_int32_temp_to_bitwise_callee(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    callees: &HashSet<&str>,
    cursor: usize,
    replacements: &mut Vec<(usize, usize, String)>,
) -> Option<usize> {
    let name_at = if matches!(
        tokens.get(cursor).map(|token| token.text),
        Some("var") | Some("let")
    ) && tokens
        .get(cursor + 1)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        cursor + 1
    } else if tokens
        .get(cursor)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && !is_property_identifier(tokens, cursor)
    {
        cursor
    } else {
        return None;
    };
    if tokens.get(name_at + 1).map(|token| token.text) != Some("=") {
        return None;
    }
    let temp = tokens[name_at].text;
    let rhs_from = name_at + 2;
    let rhs_stop = crate::js_peephole::rewrite::top_level_stop(tokens, rhs_from, &[",", ";", "}"])?;
    if rhs_stop < rhs_from + 2
        || tokens.get(rhs_stop - 2).map(|token| token.text) != Some("|")
        || tokens.get(rhs_stop - 1).map(|token| token.text) != Some("0")
    {
        return None;
    }
    let scope_end = enclosing_function_span(tokens, matching_close, cursor)
        .map(|(_, close)| close)
        .unwrap_or(tokens.len());
    let (uses, nested_use) =
        collect_same_scope_name_uses(tokens, matching_close, temp, rhs_stop, scope_end, name_at);
    if nested_use || uses.is_empty() {
        return None;
    }
    if !uses
        .iter()
        .all(|&use_at| ident_is_first_arg_to_callee(tokens, matching_close, callees, use_at))
    {
        return None;
    }
    replacements.push((
        tokens[rhs_stop - 2].start,
        tokens[rhs_stop - 1].end,
        String::new(),
    ));
    Some(rhs_stop)
}

fn ident_is_first_arg_to_callee(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    callees: &HashSet<&str>,
    use_at: usize,
) -> bool {
    if use_at == 0 || tokens.get(use_at - 1).map(|token| token.text) != Some("(") {
        return false;
    }
    let open = use_at - 1;
    if open == 0
        || !tokens.get(open - 1).is_some_and(|token| {
            token.kind == TokenKind::Identifier && callees.contains(token.text)
        })
        || is_property_identifier(tokens, open - 1)
    {
        return false;
    }
    let Some(close) = matching_close.get(open).copied().flatten() else {
        return false;
    };
    matches!(
        tokens.get(use_at + 1).map(|token| token.text),
        Some(",") | Some(")")
    ) && (tokens.get(use_at + 1).map(|token| token.text) != Some(")") || use_at + 1 == close)
}

#[cfg(test)]
mod tests {
    use super::fold_int32_coercions;

    #[test]
    fn drops_unary_plus_on_numeric_literals_only() {
        let source = "h=+7227;h=h/+2540;var i=c[a-+1];b=+0;x=a+ +1;y=+b.size-+1;z=1++1;w=-+1;q=[+0,+.5,+1e3]";
        let (out, count) = fold_int32_coercions(source).unwrap();
        assert_eq!(
            out,
            "h=7227;h=h/2540;var i=c[a-1];b=0;x=a+ +1;y=+b.size-1;z=1++1;w=-1;q=[0,+.5,1e3]",
            "{out}"
        );
        assert!(count >= 8, "{count}");
    }

    #[test]
    fn keeps_call_parens_when_argument_ends_with_length_or_zero() {
        let source = "ne.call(arguments,2,arguments.length|0);new Array(a-r.length|0);e.splice(0,e.length|0);x=(arguments.length|0)>2";
        let (out, count) = fold_int32_coercions(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("ne.call("), "{out}");
        assert!(out.contains("new Array("), "{out}");
        assert!(out.contains("e.splice("), "{out}");
        assert!(
            out.contains("arguments.length>2") || out.contains("(arguments.length)>2"),
            "{out}"
        );
        assert!(!out.contains("ne.callarguments"), "{out}");
        assert!(!out.contains("new Arraya"), "{out}");
        assert!(!out.contains("e.splice0"), "{out}");
    }

    #[test]
    fn drops_unary_plus_before_bitwise_and_contracts_bitflag_writes() {
        let source = "function g(){return 0!=(+this.y&4)}function s(e){var n=+this.y|0;e?this.y=n|4:this.y=n&(4^-1)}";
        let (out, count) = fold_int32_coercions(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(
            out.contains("this.y&4") || out.contains("this.y&4"),
            "{out}"
        );
        assert!(!out.contains("+this.y&"), "{out}");
        assert!(
            out.contains("this.y|=4") && out.contains("this.y&=~4"),
            "{out}"
        );
    }

    #[test]
    fn keeps_binary_plus_before_int32_or() {
        let source = "r=n+i|0;i=n+a.length+r|0;n=o+n|0;x=+this.y|0";
        let (out, _) = fold_int32_coercions(source).unwrap();
        assert!(out.contains("n+i|0") || out.contains("n+i"), "{out}");
        assert!(
            out.contains("a.length+r") || out.contains("n+a.length"),
            "{out}"
        );
        assert!(out.contains("o+n|0") || out.contains("o+n"), "{out}");
        assert!(!out.contains("ni|0"), "{out}");
        assert!(!out.contains("lengthr"), "{out}");
        assert!(!out.contains("on|0"), "{out}");
        assert!(!out.contains("+this.y|0"), "{out}");
    }

    #[test]
    fn drops_int32_coerce_on_bitwise_callee_args() {
        let source = "function Tb(a,b,c){return c?a|b:a&(b^-1)}function s(a){this.v=Tb(this.v|0,1,!!a);let c=this.v|0;this.v=Tb(c,16,!0)}";
        let (out, count) = fold_int32_coercions(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("Tb(this.v,1,!!a)"), "{out}");
        assert!(
            out.contains("let c=this.v;")
                || out.contains("let c=this.v,")
                || out.contains("c=this.v;"),
            "{out}"
        );
        assert!(!out.contains("Tb(this.v|0"), "{out}");
        assert!(!out.contains("c=this.v|0"), "{out}");
    }

    #[test]
    fn drops_unary_plus_on_bitwise_callee_args() {
        let source =
            "function Ja(a,b,c){return c?a|b:a&(b^-1)}function s(e){this.j=Ja(+this.j,2,!!e)}";
        let (out, count) = fold_int32_coercions(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("Ja(this.j,2,!!e)"), "{out}");
        assert!(!out.contains("Ja(+this.j"), "{out}");
    }

    #[test]
    fn drops_int32_coerce_on_bitwise_arrow_callee_args() {
        let source = "Tb=(a,b,c)=>c?a|b:a&(b^-1);S=function(a){this.v=Tb(this.v|0,c,!!a);let d=this.v|0;this.v=Tb(d,b,1==(a|0))}";
        let (out, count) = fold_int32_coercions(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("Tb(this.v,c,!!a)"), "{out}");
        assert!(
            out.contains("let d=this.v;")
                || out.contains("d=this.v;")
                || out.contains("let d=this.v,"),
            "{out}"
        );
        assert!(!out.contains("Tb(this.v|0"), "{out}");
        assert!(!out.contains("d=this.v|0"), "{out}");
        assert!(out.contains("1==(a|0)"), "{out}");
    }

    #[test]
    fn rewrites_xor_minus_one_to_bitwise_not() {
        let grouped = fold_int32_coercions("function Tb(a,b,c){return c?a|b:a&(b^-1)}").unwrap();
        assert!(grouped.0.contains("a&~b"), "{}", grouped.0);
        assert!(!grouped.0.contains("^-1"), "{}", grouped.0);

        let assign = fold_int32_coercions("function s(e){e?this.y|=4:this.y&=4^-1}").unwrap();
        assert!(assign.0.contains("this.y&=~4"), "{}", assign.0);
        assert!(!assign.0.contains("4^-1"), "{}", assign.0);

        let additive = fold_int32_coercions("function s(a,b){return a+b^-1}").unwrap();
        assert!(
            additive.0.contains("a+b^-1") || additive.0.contains("a+(b^-1)"),
            "{}",
            additive.0
        );
        assert!(!additive.0.contains("a+~b"), "{}", additive.0);
    }
}
