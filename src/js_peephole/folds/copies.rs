use crate::js_peephole::rewrite::{
    apply_token_rewrites, assign_is_in_declaration, identifier_is_read, identifier_occurs,
    is_property_identifier, parse_bare_assign, replacement_overlaps, top_level_stop,
};
use crate::js_peephole::scope::{
    collect_same_scope_name_uses, collect_unbound_name_uses, enclosing_block_end,
    enclosing_block_start, enclosing_function_span, function_scope_declares,
    identifier_assigned_before, name_is_declared_in_visible_scope, name_use_is_mutated,
    nested_function_end, parse_function_expression, use_is_in_nested_function,
};
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn fold_identifier_copies(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | "let" | "var" | "const" | ","
            )
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let (rhs, rhs_end) = if tokens.get(cursor + 3).map(|token| token.text) == Some(".")
            && tokens
                .get(cursor + 4)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && matches!(
                tokens.get(cursor + 5).map(|token| token.text),
                Some(";") | Some(",") | Some("}") | None
            ) {
            (
                source[tokens[cursor + 2].start..tokens[cursor + 4].end].to_string(),
                cursor + 4,
            )
        } else if matches!(
            tokens.get(cursor + 3).map(|token| token.text),
            Some(";") | Some(",") | Some("}") | None
        ) {
            (tokens[cursor + 2].text.to_string(), cursor + 2)
        } else {
            cursor += 1;
            continue;
        };
        if rhs == name {
            cursor += 1;
            continue;
        }
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        let mut reads = Vec::new();
        let mut scan = rhs_end + 1;
        if tokens.get(scan).map(|token| token.text) == Some(";")
            || tokens.get(scan).map(|token| token.text) == Some(",")
        {
            scan += 1;
        }
        while scan < scope_end {
            if let Some(close) = nested_function_end(&tokens, &matching_close, scan) {
                scan = close + 1;
                continue;
            }
            if tokens[scan].kind == TokenKind::Identifier
                && tokens[scan].text == name
                && !is_property_identifier(&tokens, scan)
            {
                if tokens.get(scan + 1).map(|token| token.text) == Some("=")
                    && tokens.get(scan + 2).map(|token| token.text) != Some("=")
                {
                    break;
                }
                if matches!(
                    tokens.get(scan + 1).map(|token| token.text),
                    Some("++") | Some("--")
                ) || matches!(
                    tokens.get(scan.wrapping_sub(1)).map(|token| token.text),
                    Some("++") | Some("--") | Some("[")
                ) {
                    break;
                }
                reads.push(scan);
            }
            if rhs_end == cursor + 2
                && tokens[scan].kind == TokenKind::Identifier
                && tokens[scan].text == tokens[cursor + 2].text
                && tokens.get(scan + 1).map(|token| token.text) == Some("=")
                && tokens.get(scan + 2).map(|token| token.text) != Some("=")
            {
                break;
            }
            scan += 1;
        }
        if reads.is_empty() {
            let scope_start = enclosing_block_start(&matching_close, cursor)
                .map(|open| open + 1)
                .unwrap_or(0);
            if rhs_end == cursor + 2
                && !name_is_declared_in_visible_scope(
                    &tokens,
                    &matching_close,
                    cursor,
                    tokens[cursor + 2].text,
                )
            {
                let Some(nested) = collect_binding_uses(
                    &tokens,
                    &matching_close,
                    name,
                    cursor,
                    rhs_end + 1,
                    scope_start,
                    scope_end,
                ) else {
                    cursor += 1;
                    continue;
                };
                let rhs_uses = collect_unbound_name_uses(
                    &tokens,
                    &matching_close,
                    tokens[cursor + 2].text,
                    scope_start,
                    scope_end,
                    cursor,
                );
                if nested.len() == 1
                    && !name_use_is_mutated(&tokens, nested[0])
                    && rhs_uses
                        .iter()
                        .all(|&use_at| !name_use_is_mutated(&tokens, use_at))
                {
                    reads = nested;
                } else {
                    cursor += 1;
                    continue;
                }
            } else {
                cursor += 1;
                continue;
            }
        }
        if rhs_end != cursor + 2 && reads.len() != 1 {
            cursor += 1;
            continue;
        }
        let prev = cursor
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";");
        if matches!(prev, "let" | "var" | "const" | ",") {
            for read in reads {
                replacements.push((tokens[read].start, tokens[read].end, rhs.clone()));
            }
        } else {
            let (from, to) = assignment_span_to_remove(&tokens, cursor, rhs_end);
            replacements.push((from, to, String::new()));
            for read in reads {
                replacements.push((tokens[read].start, tokens[read].end, rhs.clone()));
            }
        }
        cursor = rhs_end + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count / 2 + count % 2))
}

fn assignment_span_to_remove(
    tokens: &[Token<'_>],
    name_at: usize,
    rhs_end: usize,
) -> (usize, usize) {
    let after = rhs_end + 1;
    let prev = name_at.checked_sub(1).map(|index| tokens[index].text);
    if matches!(prev, Some("let") | Some("var") | Some("const")) {
        if tokens.get(after).map(|token| token.text) == Some(",") {
            return (tokens[name_at].start, tokens[after].end);
        }
        if tokens.get(after).map(|token| token.text) == Some(";") {
            return (tokens[name_at - 1].start, tokens[after].end);
        }
    }
    if prev == Some(",") {
        return (tokens[name_at - 1].start, tokens[rhs_end].end);
    }
    let end = if tokens.get(after).map(|token| token.text) == Some(";") {
        tokens[after].end
    } else {
        tokens[rhs_end].end
    };
    (tokens[name_at].start, end)
}

fn cheap_literal_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    rhs: usize,
) -> Option<usize> {
    if tokens.get(rhs).map(|token| token.kind) == Some(TokenKind::Regex) {
        return Some(rhs);
    }
    if tokens.get(rhs).map(|token| token.text) == Some("this")
        && matches!(
            tokens.get(rhs + 1).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens
            .get(rhs + 2)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(rhs + 3).map(|token| token.text) == Some("?")
        && ((tokens.get(rhs + 4).map(|token| token.text) == Some("void")
            && tokens.get(rhs + 5).map(|token| token.text) == Some("0")
            && tokens.get(rhs + 6).map(|token| token.text) == Some(":")
            && tokens.get(rhs + 7).map(|token| token.text) == Some("this"))
            || (tokens.get(rhs + 4).map(|token| token.text) == Some("undefined")
                && tokens.get(rhs + 5).map(|token| token.text) == Some(":")
                && tokens.get(rhs + 6).map(|token| token.text) == Some("this")))
    {
        return Some(if tokens[rhs + 4].text == "void" {
            rhs + 7
        } else {
            rhs + 6
        });
    }
    if tokens.get(rhs).map(|token| token.text) == Some("(")
        && tokens.get(rhs + 1).map(|token| token.text) == Some(")")
        && tokens.get(rhs + 2).map(|token| token.text) == Some("=>")
        && tokens.get(rhs + 3).map(|token| token.text) == Some("{")
    {
        let close = matching_close.get(rhs + 3).copied().flatten()?;
        return (close == rhs + 4).then_some(close);
    }
    if tokens.get(rhs).map(|token| token.text) == Some("function")
        && tokens.get(rhs + 1).map(|token| token.text) == Some("(")
        && tokens.get(rhs + 2).map(|token| token.text) == Some(")")
        && tokens.get(rhs + 3).map(|token| token.text) == Some("{")
    {
        let close = matching_close.get(rhs + 3).copied().flatten()?;
        return (close == rhs + 4).then_some(close);
    }
    if tokens
        .get(rhs)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(rhs + 1).map(|token| token.text) == Some("(")
        && tokens
            .get(rhs + 2)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(rhs + 3).map(|token| token.text) == Some(")")
        && tokens.get(rhs + 4).map(|token| token.text) == Some("?")
        && tokens
            .get(rhs + 5)
            .is_some_and(|token| cheap_ternary_arm(token))
        && tokens.get(rhs + 6).map(|token| token.text) == Some(":")
        && tokens
            .get(rhs + 7)
            .is_some_and(|token| cheap_ternary_arm(token))
    {
        return Some(rhs + 7);
    }
    if tokens
        .get(rhs)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(rhs + 1).map(|token| token.text) == Some("(")
    {
        let close = matching_close.get(rhs + 1).copied().flatten()?;
        if matches!(
            tokens.get(close + 1).map(|token| token.text),
            Some(",") | Some(";") | Some("}") | None
        ) {
            return Some(close);
        }
    }
    if tokens
        .get(rhs)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(rhs + 1).map(|token| token.text) == Some(".")
        && tokens
            .get(rhs + 2)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(rhs + 3).map(|token| token.text) == Some("(")
    {
        let close = matching_close.get(rhs + 3).copied().flatten()?;
        if tokens.get(close + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(close + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && matches!(
                tokens.get(close + 3).map(|token| token.text),
                Some(",") | Some(";") | Some("}") | None
            )
        {
            return Some(close + 2);
        }
        if matches!(
            tokens.get(close + 1).map(|token| token.text),
            Some(",") | Some(";") | Some("}") | None
        ) {
            return Some(close);
        }
    }
    None
}

fn cheap_ternary_arm(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Identifier
        || matches!(token.text, "null" | "true" | "false" | "undefined")
}

fn span_is_effectful(tokens: &[Token<'_>], from: usize, end: usize) -> bool {
    let mut index = from;
    while index < end {
        if matches!(tokens[index].text, "new" | "." | "[" | "++" | "--") {
            return true;
        }
        if tokens[index].text == "("
            && index > from
            && (tokens[index - 1].kind == TokenKind::Identifier
                || matches!(tokens[index - 1].text, ")" | "]" | "}"))
        {
            return true;
        }
        index += 1;
    }
    false
}

fn collect_binding_uses(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name: &str,
    name_at: usize,
    after_init: usize,
    scope_start: usize,
    scope_end: usize,
) -> Option<Vec<usize>> {
    let prior = collect_unbound_name_uses(
        tokens,
        matching_close,
        name,
        scope_start,
        name_at,
        name_at,
    )
    .into_iter()
    .filter(|&use_at| identifier_is_read(tokens, use_at, use_at + 1, name))
    .collect::<Vec<_>>();
    if prior.iter().any(|&use_at| {
        !use_is_in_nested_function(tokens, matching_close, scope_start, use_at)
    }) {
        return None;
    }
    let mut uses = prior;
    uses.extend(collect_unbound_name_uses(
        tokens,
        matching_close,
        name,
        after_init,
        scope_end,
        name_at,
    ));
    Some(uses)
}

fn can_rematerialize_literal(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    use_at: usize,
    rhs_kind: TokenKind,
    effectful: bool,
    guarded: bool,
) -> bool {
    let nested = use_is_in_nested_function(tokens, matching_close, from, use_at);
    if effectful && (guarded || nested) {
        return false;
    }
    if nested && rhs_kind != TokenKind::Regex {
        return false;
    }
    if tokens
        .get(use_at.wrapping_sub(1))
        .map(|token| token.text)
        == Some("=")
        && tokens
            .get(use_at.wrapping_sub(2))
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && matches!(
            tokens.get(use_at + 1).map(|token| token.text),
            Some(",") | Some(";") | Some(")") | None
        )
    {
        return false;
    }
    true
}

fn use_is_guarded(
    tokens: &[Token<'_>],
    from: usize,
    use_at: usize,
) -> bool {
    let mut depth = 0i32;
    let mut index = from;
    while index < use_at {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "if" | "for" | "while" | "do" | "?" | "&&" | "||" | "??"
                if depth == 0 =>
            {
                return true;
            }
            _ => {}
        }
        index += 1;
    }
    false
}

fn rematerialized_literal_needs_grouping(
    tokens: &[Token<'_>],
    use_at: usize,
    kind: TokenKind,
    literal: &str,
) -> bool {
    if kind == TokenKind::Regex {
        return false;
    }
    let previous = use_at
        .checked_sub(1)
        .map(|index| tokens[index].text)
        .unwrap_or(";");
    if literal.contains('?') {
        return !matches!(
            previous,
            "(" | "[" | "{" | "," | ";" | ":" | "=" | "return" | "throw"
        );
    }
    !matches!(
        previous,
        "(" | "["
            | "{"
            | ","
            | ";"
            | ":"
            | "="
            | "return"
            | "throw"
            | "void"
            | "typeof"
            | "delete"
            | "await"
            | "yield"
            | "!"
            | "~"
    )
}

fn assignment_crosses_loop_boundary(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scope_start: usize,
    assign_at: usize,
    use_at: usize,
) -> bool {
    let mut cursor = scope_start;
    while cursor < assign_at {
        let keyword = tokens[cursor].text;
        if !matches!(keyword, "for" | "while" | "do") {
            cursor += 1;
            continue;
        }
        let body_start = if keyword == "do" {
            cursor + 1
        } else {
            let header_open = cursor + 1;
            if tokens.get(header_open).map(|token| token.text) != Some("(") {
                cursor += 1;
                continue;
            }
            let Some(header_close) = matching_close.get(header_open).copied().flatten() else {
                return true;
            };
            header_close + 1
        };
        let statement_end = if tokens.get(body_start).map(|token| token.text) == Some("{") {
            matching_close.get(body_start).copied().flatten()
        } else {
            top_level_stop(tokens, body_start, &[";"])
        };
        let Some(mut statement_end) = statement_end else {
            return true;
        };
        if keyword == "do" {
            if tokens.get(statement_end + 1).map(|token| token.text) != Some("while")
                || tokens.get(statement_end + 2).map(|token| token.text) != Some("(")
            {
                return true;
            }
            let Some(while_close) = matching_close.get(statement_end + 2).copied().flatten() else {
                return true;
            };
            statement_end = while_close;
        }
        if assign_at > cursor && assign_at <= statement_end && use_at > statement_end {
            return true;
        }
        cursor += 1;
    }
    false
}

pub(crate) fn fold_single_use_literal_bindings(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_single_use_literals(source, None)
}

pub(crate) fn fold_single_use_regex_bindings(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_single_use_literals(source, Some(TokenKind::Regex))
}

fn fold_single_use_literals(
    source: &str,
    only: Option<TokenKind>,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if !matches!(tokens[cursor].text, "let" | "var" | "const") {
            cursor += 1;
            continue;
        }
        let mut name_at = cursor + 1;
        loop {
            if tokens
                .get(name_at)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
                || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
            {
                break;
            }
            let Some(literal_end) = cheap_literal_end(&tokens, &matching_close, name_at + 2) else {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            };
            if !matches!(
                tokens.get(literal_end + 1).map(|token| token.text),
                Some(",") | Some(";")
            ) {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            if only.is_some_and(|kind| tokens[name_at + 2].kind != kind) {
                let stop = literal_end + 1;
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let name = tokens[name_at].text;
            let stop = literal_end + 1;
            let scope_start = enclosing_block_start(&matching_close, name_at)
                .map(|open| open + 1)
                .unwrap_or(0);
            let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
            let Some(uses) = collect_binding_uses(
                &tokens,
                &matching_close,
                name,
                name_at,
                stop + 1,
                scope_start,
                scope_end,
            ) else {
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            };
            if uses.len() == 1
                && !name_use_is_mutated(&tokens, uses[0])
                && !assignment_crosses_loop_boundary(
                    &tokens,
                    &matching_close,
                    scope_start,
                    name_at,
                    uses[0],
                )
                && can_rematerialize_literal(
                    &tokens,
                    &matching_close,
                    scope_start,
                    uses[0],
                    tokens[name_at + 2].kind,
                    span_is_effectful(&tokens, name_at + 2, literal_end + 1),
                    use_is_guarded(&tokens, stop + 1, uses[0]),
                )
            {
                let use_at = uses[0];
                let (from, to) = assignment_span_to_remove(&tokens, name_at, literal_end);
                if !replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
                    && !replacement_overlaps(&replacements, from, to)
                {
                    let literal = &source[tokens[name_at + 2].start..tokens[literal_end].end];
                    let rendered = if rematerialized_literal_needs_grouping(
                        &tokens,
                        use_at,
                        tokens[name_at + 2].kind,
                        literal,
                    ) {
                        format!("({literal})")
                    } else {
                        literal.to_string()
                    };
                    replacements.push((tokens[use_at].start, tokens[use_at].end, rendered));
                    replacements.push((from, to, String::new()));
                }
            }
            if tokens[stop].text == ";" {
                break;
            }
            name_at = stop + 1;
        }
        cursor += 1;
    }
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || assign_is_in_declaration(&tokens, cursor)
            || !matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | ","
            )
        {
            cursor += 1;
            continue;
        }
        let Some(literal_end) = cheap_literal_end(&tokens, &matching_close, cursor + 2) else {
            cursor += 1;
            continue;
        };
        if !matches!(
            tokens.get(literal_end + 1).map(|token| token.text),
            Some(",") | Some(";") | Some("}") | None
        ) {
            cursor += 1;
            continue;
        }
        if only.is_some_and(|kind| tokens[cursor + 2].kind != kind) {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let scope_start = enclosing_block_start(&matching_close, cursor)
            .map(|open| open + 1)
            .unwrap_or(0);
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        if identifier_is_read(&tokens, scope_start, cursor, name) {
            cursor += 1;
            continue;
        }
        let Some(uses) = collect_binding_uses(
            &tokens,
            &matching_close,
            name,
            cursor,
            literal_end + 1,
            scope_start,
            scope_end,
        ) else {
            cursor += 1;
            continue;
        };
        if uses.len() == 1
            && !name_use_is_mutated(&tokens, uses[0])
            && !assignment_crosses_loop_boundary(
                &tokens,
                &matching_close,
                scope_start,
                cursor,
                uses[0],
            )
            && can_rematerialize_literal(
                &tokens,
                &matching_close,
                scope_start,
                uses[0],
                tokens[cursor + 2].kind,
                span_is_effectful(&tokens, cursor + 2, literal_end + 1),
                use_is_guarded(&tokens, literal_end + 1, uses[0]),
            )
        {
            let use_at = uses[0];
            let (from, to) = assignment_span_to_remove(&tokens, cursor, literal_end);
            if !replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
                && !replacement_overlaps(&replacements, from, to)
            {
                let literal = &source[tokens[cursor + 2].start..tokens[literal_end].end];
                let rendered = if rematerialized_literal_needs_grouping(
                    &tokens,
                    use_at,
                    tokens[cursor + 2].kind,
                    literal,
                ) {
                    format!("({literal})")
                } else {
                    literal.to_string()
                };
                replacements.push((tokens[use_at].start, tokens[use_at].end, rendered));
                replacements.push((from, to, String::new()));
            }
        }
        cursor = literal_end + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrototypeToStringKind {
    Object,
    Function,
}

fn prototype_tostring_call(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<(PrototypeToStringKind, usize)> {
    let kind = match tokens.get(at).map(|token| token.text) {
        Some("Object") => PrototypeToStringKind::Object,
        Some("Function") => PrototypeToStringKind::Function,
        _ => return None,
    };
    if tokens.get(at + 1).map(|token| token.text) != Some(".")
        || tokens.get(at + 2).map(|token| token.text) != Some("prototype")
        || tokens.get(at + 3).map(|token| token.text) != Some(".")
        || tokens.get(at + 4).map(|token| token.text) != Some("toString")
        || tokens.get(at + 5).map(|token| token.text) != Some(".")
        || tokens.get(at + 6).map(|token| token.text) != Some("call")
        || tokens.get(at + 7).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let close = matching_close.get(at + 7).copied().flatten()?;
    Some((kind, close))
}

fn collect_prototype_tostring_aliases<'src>(
    tokens: &[Token<'src>],
    matching_close: &[Option<usize>],
) -> (Option<&'src str>, Option<&'src str>) {
    let mut empty_objects = std::collections::HashSet::<&str>::new();
    let mut functions = std::collections::HashSet::<&str>::new();
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Identifier
            || tokens.get(index + 1).map(|token| token.text) != Some("=")
        {
            continue;
        }
        if tokens.get(index + 2).map(|token| token.text) == Some("{")
            && matching_close.get(index + 2).copied().flatten() == Some(index + 3)
        {
            empty_objects.insert(tokens[index].text);
        }
        if tokens
            .get(index + 2)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 3).map(|token| token.text) == Some(".")
            && tokens.get(index + 4).map(|token| token.text) == Some("hasOwnProperty")
            && matches!(
                tokens.get(index + 5).map(|token| token.text),
                Some(",") | Some(";") | None
            )
        {
            functions.insert(tokens[index].text);
        }
    }
    let mut object_alias = None;
    let mut function_alias = None;
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Identifier
            || tokens.get(index + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(index + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(index + 3).map(|token| token.text) != Some(".")
            || tokens.get(index + 4).map(|token| token.text) != Some("toString")
            || !matches!(
                tokens.get(index + 5).map(|token| token.text),
                Some(",") | Some(";") | None
            )
        {
            continue;
        }
        let ident = tokens[index + 2].text;
        let alias = tokens[index].text;
        if empty_objects.contains(ident) {
            object_alias = Some(alias);
        }
        if functions.contains(ident) {
            function_alias = Some(alias);
        }
    }
    (object_alias, function_alias)
}

pub(crate) fn fold_prototype_tostring_aliases(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let (object_alias, function_alias) =
        collect_prototype_tostring_aliases(&tokens, &matching_close);
    if object_alias.is_none() && function_alias.is_none() {
        return Ok((source.to_string(), 0));
    }
    let mut replacements = Vec::<(usize, usize, String)>::new();
    if let Some(alias) = function_alias {
        let mut cursor = 0usize;
        while cursor + 3 < tokens.len() {
            if tokens[cursor].kind == TokenKind::Identifier
                && tokens[cursor].text == alias
                && tokens.get(cursor + 1).map(|token| token.text) == Some(".")
                && tokens.get(cursor + 2).map(|token| token.text) == Some("call")
                && tokens.get(cursor + 3).map(|token| token.text) == Some("(")
                && is_simple_declarator_rhs(&tokens, cursor)
                && !identifier_assigned_before(&tokens, alias, cursor)
            {
                replacements.push((
                    tokens[cursor].start,
                    tokens[cursor].end,
                    "Function.prototype.toString".to_string(),
                ));
                cursor += 4;
                continue;
            }
            cursor += 1;
        }
    }
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        let Some((kind, close)) = prototype_tostring_call(&tokens, &matching_close, cursor) else {
            cursor += 1;
            continue;
        };
        let alias = match kind {
            PrototypeToStringKind::Object => object_alias,
            PrototypeToStringKind::Function => function_alias,
        };
        let Some(alias) = alias else {
            cursor += 1;
            continue;
        };
        let args = &source[tokens[cursor + 8].start..tokens[close].start];
        let rendered = format!("{alias}.call({args})");
        if kind == PrototypeToStringKind::Function
            && tokens.get(cursor.wrapping_sub(1)).map(|token| token.text) == Some("=")
            && cursor >= 2
            && tokens[cursor - 2].kind == TokenKind::Identifier
            && matches!(
                cursor
                    .checked_sub(3)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                "let" | "var" | "const" | ","
            )
        {
            let name_at = cursor - 2;
            let name = tokens[name_at].text;
            let uses = (0..tokens.len())
                .filter(|&index| {
                    index != name_at
                        && tokens[index].kind == TokenKind::Identifier
                        && tokens[index].text == name
                })
                .collect::<Vec<_>>();
            if uses.len() == 1 && !name_use_is_mutated(&tokens, uses[0]) {
                replacements.push((tokens[uses[0]].start, tokens[uses[0]].end, rendered));
                let (from, to) = assignment_span_to_remove(&tokens, name_at, close);
                replacements.push((from, to, String::new()));
                cursor = close + 1;
                continue;
            }
        }
        if is_simple_declarator_rhs(&tokens, cursor)
            && !identifier_assigned_before(&tokens, alias, cursor)
        {
            cursor += 1;
            continue;
        }
        replacements.push((tokens[cursor].start, tokens[close].end, rendered));
        cursor = close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn is_simple_declarator_rhs(tokens: &[Token<'_>], call_at: usize) -> bool {
    call_at >= 2
        && tokens.get(call_at.wrapping_sub(1)).map(|token| token.text) == Some("=")
        && tokens[call_at - 2].kind == TokenKind::Identifier
        && matches!(
            call_at
                .checked_sub(3)
                .map(|index| tokens[index].text)
                .unwrap_or(";"),
            "let" | "var" | "const" | ","
        )
}

pub(crate) fn fold_temp_index_keys(source: &str) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        let name_at = if matches!(tokens[cursor].text, "var" | "let") {
            cursor + 1
        } else {
            cursor
        };
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(semi) = top_level_stop(&tokens, name_at + 2, &[";"]) else {
            cursor += 1;
            continue;
        };
        let name = tokens[name_at].text;
        if tokens
            .get(semi + 1)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(semi + 2).map(|token| token.text) != Some("[")
            || tokens.get(semi + 3).map(|token| token.text) != Some(name)
            || tokens.get(semi + 4).map(|token| token.text) != Some("]")
            || tokens.get(semi + 5).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(assign_end) = top_level_stop(&tokens, semi + 6, &[";", "}"]) else {
            cursor += 1;
            continue;
        };
        let scope_end =
            enclosing_block_end(&matching_closers(&tokens), name_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, name_at + 2, semi, name)
            || identifier_occurs(&tokens, semi + 6, assign_end, name)
            || identifier_occurs(&tokens, assign_end + 1, scope_end, name)
        {
            cursor += 1;
            continue;
        }
        let key = &source[tokens[name_at + 2].start..tokens[semi].start];
        let object = tokens[semi + 1].text;
        let value = &source[tokens[semi + 6].start..tokens[assign_end].start];
        let from = if name_at
            .checked_sub(1)
            .is_some_and(|index| tokens[index].text == ",")
        {
            tokens[name_at - 1].start
        } else {
            tokens[cursor].start
        };
        let replace_end = if tokens[assign_end].text == "}" {
            tokens[assign_end].start
        } else {
            tokens[assign_end].end
        };
        replacements.push((from, replace_end, format!("{object}[{key}]={value};")));
        cursor = assign_end + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_chained_identifier_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        let Some((first_at, first_name, rhs, after_first)) =
            parse_bare_assign(source, &tokens, cursor)
        else {
            cursor += 1;
            continue;
        };
        if tokens.get(after_first).map(|token| token.kind) != Some(TokenKind::Identifier)
            || tokens.get(after_first + 1).map(|token| token.text) != Some("=")
            || tokens.get(after_first + 2).map(|token| token.text) != Some(first_name)
            || !matches!(
                tokens.get(after_first + 3).map(|token| token.text),
                None | Some(";") | Some(",") | Some(")") | Some("}")
            )
        {
            cursor += 1;
            continue;
        }
        let second_name = tokens[after_first].text;
        replacements.push((
            tokens[first_at].start,
            tokens[after_first + 2].end,
            format!("{second_name}={first_name}={rhs}"),
        ));
        cursor = after_first + 3;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn property_read_chain_end(tokens: &[Token<'_>], object_at: usize) -> Option<usize> {
    if tokens.get(object_at).map(|token| token.kind) != Some(TokenKind::Identifier) {
        return None;
    }
    let mut end = object_at;
    loop {
        if tokens.get(end + 1).map(|token| token.text) != Some("[") {
            break;
        }
        let key = tokens.get(end + 2)?;
        if !matches!(key.kind, TokenKind::Identifier | TokenKind::Number) {
            return None;
        }
        if tokens.get(end + 3).map(|token| token.text) != Some("]") {
            return None;
        }
        end += 3;
    }
    (end > object_at).then_some(end)
}

pub(crate) fn fold_single_use_index_temps(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | "let" | "var" | "const" | ","
            )
        {
            cursor += 1;
            continue;
        }
        let Some(rhs_end) = property_read_chain_end(&tokens, cursor + 2) else {
            cursor += 1;
            continue;
        };
        if !matches!(
            tokens.get(rhs_end + 1).map(|token| token.text),
            Some(";") | Some(",") | Some("}") | None
        ) {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let rhs = source[tokens[cursor + 2].start..tokens[rhs_end].end].to_string();
        let chain_idents = tokens[cursor + 2..=rhs_end]
            .iter()
            .filter(|token| token.kind == TokenKind::Identifier)
            .map(|token| token.text)
            .collect::<Vec<_>>();
        if chain_idents.iter().any(|ident| *ident == name) {
            cursor += 1;
            continue;
        }
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        let mut reads = Vec::new();
        let mut scan = rhs_end + 1;
        if matches!(
            tokens.get(scan).map(|token| token.text),
            Some(";") | Some(",")
        ) {
            scan += 1;
        }
        let mut safe = true;
        let mut chain_mutated = false;
        while scan < scope_end {
            if let Some(close) = nested_function_end(&tokens, &matching_close, scan) {
                scan = close + 1;
                continue;
            }
            if tokens[scan].kind == TokenKind::Identifier
                && tokens[scan].text == name
                && !is_property_identifier(&tokens, scan)
            {
                if tokens.get(scan + 1).map(|token| token.text) == Some("=")
                    && tokens.get(scan + 2).map(|token| token.text) != Some("=")
                    || matches!(
                        tokens.get(scan + 1).map(|token| token.text),
                        Some("++") | Some("--")
                    )
                    || matches!(
                        tokens.get(scan.wrapping_sub(1)).map(|token| token.text),
                        Some("++") | Some("--")
                    )
                    || (tokens.get(scan.wrapping_sub(1)).map(|token| token.text)
                        == Some("[")
                        && tokens.get(scan + 1).map(|token| token.text) != Some("]"))
                {
                    safe = false;
                    break;
                }
                if chain_mutated {
                    safe = false;
                    break;
                }
                reads.push(scan);
            }
            if tokens[scan].kind == TokenKind::Identifier
                && chain_idents.contains(&tokens[scan].text)
                && (tokens.get(scan + 1).map(|token| token.text) == Some("=")
                    && tokens.get(scan + 2).map(|token| token.text) != Some("=")
                    || matches!(
                        tokens.get(scan + 1).map(|token| token.text),
                        Some("++") | Some("--")
                    )
                    || matches!(
                        tokens.get(scan.wrapping_sub(1)).map(|token| token.text),
                        Some("++") | Some("--")
                    ))
            {
                chain_mutated = true;
            }
            scan += 1;
        }
        let thenable = reads.len() == 2
            && tokens.get(reads[0] + 1).map(|token| token.text) == Some("&&")
            && reads[1] == reads[0] + 2
            && tokens.get(reads[1] + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(reads[1] + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier);
        if !safe || (reads.len() != 1 && !thenable) {
            cursor += 1;
            continue;
        }
        let (from, to) = assignment_span_to_remove(&tokens, cursor, rhs_end);
        replacements.push((from, to, String::new()));
        for &read in &reads {
            replacements.push((tokens[read].start, tokens[read].end, rhs.clone()));
        }
        cursor = rhs_end + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count / 2 + count % 2))
}

pub(crate) fn fold_single_use_call_argument_members(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 5 < tokens.len() {
        if !matches!(tokens[cursor].text, "var" | "let")
            || tokens
                .get(cursor + 1)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 2).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 3)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 4).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 5)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                tokens.get(cursor + 6).map(|token| token.text),
                Some(",") | Some(";")
            )
        {
            cursor += 1;
            continue;
        }
        let name_at = cursor + 1;
        let name = tokens[name_at].text;
        let literal_end = cursor + 5;
        let stop = cursor + 6;
        let scope_start = enclosing_block_start(&matching_close, name_at)
            .map(|open| open + 1)
            .unwrap_or(0);
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, scope_start, name_at, name) {
            cursor += 1;
            continue;
        }
        let (uses, nested_use) = collect_same_scope_name_uses(
            &tokens,
            &matching_close,
            name,
            stop + 1,
            scope_end,
            name_at,
        );
        if nested_use || uses.len() != 1 || name_use_is_mutated(&tokens, uses[0]) {
            cursor += 1;
            continue;
        }
        let use_at = uses[0];
        let prev = use_at
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";");
        if !matches!(prev, "," | "(") {
            cursor += 1;
            continue;
        }
        let (from, to) = assignment_span_to_remove(&tokens, name_at, literal_end);
        if replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
            || replacement_overlaps(&replacements, from, to)
        {
            cursor += 1;
            continue;
        }
        let literal = &source[tokens[cursor + 3].start..tokens[literal_end].end];
        replacements.push((
            tokens[use_at].start,
            tokens[use_at].end,
            literal.to_string(),
        ));
        replacements.push((from, to, String::new()));
        cursor = stop + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_copied_receiver_method_reassign(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 9 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 3).map(|token| token.text) != Some(",")
            || tokens.get(cursor + 4).map(|token| token.text) != Some(tokens[cursor].text)
            || tokens.get(cursor + 5).map(|token| token.text) != Some("=")
            || tokens.get(cursor + 6).map(|token| token.text) != Some(tokens[cursor].text)
            || tokens.get(cursor + 7).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 8)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 9).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let Some(close) = matching_close.get(cursor + 9).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let name = tokens[cursor].text;
        if identifier_occurs(&tokens, cursor + 10, close, name) {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[cursor + 7].end,
            format!("{}={}.", name, tokens[cursor + 2].text),
        ));
        cursor = close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_outer_copy_property_assign(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut matching_open = vec![None; tokens.len()];
    for (open, close) in matching_close.iter().enumerate() {
        if let Some(close) = *close {
            matching_open[close] = Some(open);
        }
    }
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens[cursor].text == tokens[cursor + 2].text
            || !matches!(
                tokens.get(cursor + 3).map(|token| token.text),
                Some(",") | Some(";")
            )
            || tokens.get(cursor + 4).map(|token| token.text) != Some(tokens[cursor].text)
            || tokens.get(cursor + 5).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 6)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 7).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let Some((body, end)) = enclosing_function_span(&tokens, &matching_close, cursor) else {
            cursor += 1;
            continue;
        };
        if function_scope_declares(&tokens, &matching_open, body, end, name) {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[cursor + 5].start,
            tokens[cursor + 2].text.to_string(),
        ));
        cursor += 8;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn name_use_is_invoke(tokens: &[Token<'_>], use_at: usize) -> bool {
    matches!(
        tokens.get(use_at + 1).map(|token| token.text),
        Some("(") | Some(".") | Some("[")
    ) || tokens
        .get(use_at.wrapping_sub(1))
        .map(|token| token.text)
        == Some("new")
}

pub(crate) fn fold_single_use_function_values(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if !matches!(tokens[cursor].text, "let" | "var" | "const") {
            cursor += 1;
            continue;
        }
        let mut name_at = cursor + 1;
        loop {
            if tokens
                .get(name_at)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
                || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
            {
                break;
            }
            let Some(function) =
                parse_function_expression(&tokens, &matching_close, name_at + 2)
            else {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            };
            if !matches!(
                tokens.get(function.end + 1).map(|token| token.text),
                Some(",") | Some(";")
            ) {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let name = tokens[name_at].text;
            let stop = function.end + 1;
            if identifier_occurs(&tokens, name_at + 2, function.end + 1, name) {
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let scope_start = enclosing_block_start(&matching_close, name_at)
                .map(|open| open + 1)
                .unwrap_or(0);
            let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
            let Some(uses) = collect_binding_uses(
                &tokens,
                &matching_close,
                name,
                name_at,
                stop + 1,
                scope_start,
                scope_end,
            ) else {
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            };
            if uses.len() == 1 && !name_use_is_mutated(&tokens, uses[0]) {
                let use_at = uses[0];
                let called = tokens.get(use_at + 1).map(|token| token.text) == Some("(");
                let member_or_new = matches!(
                    tokens.get(use_at + 1).map(|token| token.text),
                    Some(".") | Some("[")
                ) || tokens
                    .get(use_at.wrapping_sub(1))
                    .map(|token| token.text)
                    == Some("new");
                let expression_arrow = function.is_arrow && function.block_open.is_none();
                // A single called use keeps identical semantics as an IIFE
                // whether or not the body returns: the call site consumes the
                // same value either way, and recursion is excluded above.
                let procedure_iife = called && function.block_open.is_some();
                let nested =
                    use_is_in_nested_function(&tokens, &matching_close, scope_start, use_at);
                let allow = if member_or_new {
                    false
                } else if called {
                    expression_arrow || procedure_iife
                } else {
                    !nested
                };
                if allow {
                    let (from, to) = assignment_span_to_remove(&tokens, name_at, function.end);
                    if !replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
                        && !replacement_overlaps(&replacements, from, to)
                    {
                        let literal = &source[tokens[name_at + 2].start..tokens[function.end].end];
                        let rendered = if expression_arrow
                            || procedure_iife
                            || rematerialized_literal_needs_grouping(
                                &tokens,
                                use_at,
                                tokens[name_at + 2].kind,
                                literal,
                            ) {
                            format!("({literal})")
                        } else {
                            literal.to_string()
                        };
                        replacements.push((tokens[use_at].start, tokens[use_at].end, rendered));
                        replacements.push((from, to, String::new()));
                    }
                }
            }
            if tokens[stop].text == ";" {
                break;
            }
            name_at = stop + 1;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn operand_assigned_in_range(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
    name: &str,
) -> bool {
    let mut index = start;
    while index < end {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && matches!(
                tokens.get(index + 1).map(|token| token.text),
                Some("=") | Some("++") | Some("--") | Some("+=") | Some("-=")
            )
        {
            return true;
        }
        if matches!(tokens[index].text, "++" | "--")
            && tokens.get(index + 1).map(|token| token.text) == Some(name)
        {
            return true;
        }
        index += 1;
    }
    false
}

pub(crate) fn fold_typeof_identifier_caches(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens.get(cursor + 2).map(|token| token.text) != Some("typeof")
            || tokens
                .get(cursor + 3)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                tokens.get(cursor + 4).map(|token| token.text),
                Some(",") | Some(";") | Some("}") | None
            )
        {
            cursor += 1;
            continue;
        }
        let name_at = cursor;
        let name = tokens[name_at].text;
        let operand = tokens[cursor + 3].text;
        if name == operand {
            cursor += 1;
            continue;
        }
        let scope_start = enclosing_block_start(&matching_close, name_at)
            .map(|open| open + 1)
            .unwrap_or(0);
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, scope_start, name_at, name) {
            cursor += 1;
            continue;
        }
        let (uses, nested_use) = collect_same_scope_name_uses(
            &tokens,
            &matching_close,
            name,
            cursor + 5,
            scope_end,
            name_at,
        );
        if nested_use || uses.is_empty() || uses.iter().any(|&use_at| name_use_is_mutated(&tokens, use_at))
        {
            cursor += 1;
            continue;
        }
        let last_use = *uses.last().unwrap();
        if operand_assigned_in_range(&tokens, &matching_close, name_at, last_use, operand) {
            cursor += 1;
            continue;
        }
        let (from, to) = assignment_span_to_remove(&tokens, name_at, cursor + 3);
        if replacement_overlaps(&replacements, from, to)
            || uses.iter().any(|&use_at| {
                replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
            })
        {
            cursor += 1;
            continue;
        }
        let rendered = format!("typeof {operand}");
        for use_at in uses {
            replacements.push((tokens[use_at].start, tokens[use_at].end, rendered.clone()));
        }
        replacements.push((from, to, String::new()));
        cursor += 5;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_window_property_caches(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens.get(cursor + 2).map(|token| token.text) != Some("window")
            || tokens.get(cursor + 3).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 4)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                tokens.get(cursor + 5).map(|token| token.text),
                Some(",") | Some(";") | Some("}") | None
            )
        {
            cursor += 1;
            continue;
        }
        let name_at = cursor;
        let name = tokens[name_at].text;
        let property = tokens[cursor + 4].text;
        if name == "window" || name == property {
            cursor += 1;
            continue;
        }
        let scope_start = enclosing_block_start(&matching_close, name_at)
            .map(|open| open + 1)
            .unwrap_or(0);
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, scope_start, name_at, name) {
            cursor += 1;
            continue;
        }
        let (uses, nested_use) = collect_same_scope_name_uses(
            &tokens,
            &matching_close,
            name,
            cursor + 6,
            scope_end,
            name_at,
        );
        if nested_use
            || uses.is_empty()
            || uses.len() > 3
            || uses
                .iter()
                .any(|&use_at| name_use_is_mutated(&tokens, use_at))
        {
            cursor += 1;
            continue;
        }
        let (from, to) = assignment_span_to_remove(&tokens, name_at, cursor + 4);
        if replacement_overlaps(&replacements, from, to)
            || uses.iter().any(|&use_at| {
                replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
            })
        {
            cursor += 1;
            continue;
        }
        let rendered = format!("window.{property}");
        for use_at in uses {
            replacements.push((tokens[use_at].start, tokens[use_at].end, rendered.clone()));
        }
        replacements.push((from, to, String::new()));
        cursor += 6;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_copied_object_index_writes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 3).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let alias = &source[tokens[cursor].start..tokens[cursor + 2].end];
        let mut scan = cursor + 4;
        while tokens.get(scan).map(|token| token.kind) == Some(TokenKind::Identifier)
            && tokens.get(scan + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(scan + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(scan + 3).map(|token| token.text) == Some("=")
        {
            scan += 4;
        }
        if tokens
            .get(scan)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                tokens.get(scan + 1).map(|token| token.text),
                Some(";") | Some(",") | Some("}")
            )
        {
            cursor += 1;
            continue;
        }
        let name = tokens[scan].text;
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        let mut index = scan + 2;
        while index + 1 < scope_end {
            if let Some(close) = nested_function_end(&tokens, &matching_close, index) {
                index = close + 1;
                continue;
            }
            if tokens[index].kind == TokenKind::Identifier
                && tokens[index].text == name
                && tokens.get(index + 1).map(|token| token.text) == Some("=")
            {
                break;
            }
            if tokens[index].kind == TokenKind::Identifier
                && tokens[index].text == name
                && tokens.get(index + 1).map(|token| token.text) == Some("[")
            {
                replacements.push((tokens[index].start, tokens[index].end, alias.to_string()));
                index += 2;
                continue;
            }
            index += 1;
        }
        cursor = scan + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_single_use_object_values(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if !matches!(tokens[cursor].text, "let" | "var" | "const") {
            cursor += 1;
            continue;
        }
        let mut name_at = cursor + 1;
        loop {
            if tokens
                .get(name_at)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
                || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
            {
                break;
            }
            if tokens.get(name_at + 2).map(|token| token.text) != Some("{") {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let Some(object_end) = matching_close.get(name_at + 2).copied().flatten() else {
                break;
            };
            if !matches!(
                tokens.get(object_end + 1).map(|token| token.text),
                Some(",") | Some(";")
            ) {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let name = tokens[name_at].text;
            let stop = object_end + 1;
            if identifier_occurs(&tokens, name_at + 2, object_end + 1, name) {
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let scope_start = enclosing_block_start(&matching_close, name_at)
                .map(|open| open + 1)
                .unwrap_or(0);
            let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
            let Some(uses) = collect_binding_uses(
                &tokens,
                &matching_close,
                name,
                name_at,
                object_end + 2,
                scope_start,
                scope_end,
            ) else {
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            };
            if uses.len() == 1
                && !name_use_is_mutated(&tokens, uses[0])
                && !name_use_is_invoke(&tokens, uses[0])
            {
                let use_at = uses[0];
                let (from, to) = assignment_span_to_remove(&tokens, name_at, object_end);
                if !replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
                    && !replacement_overlaps(&replacements, from, to)
                {
                    let literal = &source[tokens[name_at + 2].start..tokens[object_end].end];
                    replacements.push((tokens[use_at].start, tokens[use_at].end, literal.to_string()));
                    replacements.push((from, to, String::new()));
                }
            }
            if tokens[stop].text == ";" {
                break;
            }
            name_at = stop + 1;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}
