use crate::js_peephole::rewrite::{
    apply_token_rewrites, assign_is_in_declaration, identifier_is_expression_slot,
    identifier_is_read, identifier_occurs, is_property_identifier, parse_bare_assign,
    replacement_overlaps, substituted_expression_needs_grouping, top_level_stop,
    wrap_substituted_expression,
};
use crate::js_peephole::scope::{
    collect_same_scope_name_uses, collect_unbound_name_uses, enclosing_block_end,
    enclosing_block_start, enclosing_function_span, function_binds_name,
    identifier_assigned_before, identifier_is_arrow_parameter,
    name_is_bound_in_nested_function_between, name_is_declared_in_visible_scope,
    name_use_is_mutated, nested_function_end, parse_function_expression, use_is_in_nested_function,
    FunctionExpression,
};
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn fold_identifier_copies(
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
        // The value of `name` after `name=name.member` is the member value,
        // not another read through the old base object. Substituting a later
        // use would produce `name.member` and repeated cleanup rounds would
        // compound it into `name.member.member...`.
        if tokens[cursor + 2].text == name {
            cursor += 1;
            continue;
        }
        // Reads are only replaceable inside the enclosing block: the copy may
        // execute conditionally, so a use beyond the block cannot take the
        // source expression. But `var` hoists past the block, so any use out
        // there still needs the binding — it forbids the fold entirely.
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        let function_end = enclosing_function_span(&tokens, &matching_close, cursor)
            .map(|(_, end)| end)
            .unwrap_or(tokens.len());
        if identifier_occurs(&tokens, scope_end, function_end, name) {
            cursor += 1;
            continue;
        }
        // An assignment whose name is not bound in this function writes an
        // outer slot. Replacing inner reads and deleting that store would
        // leave the captured binding unchanged.
        if let Some((fn_body, fn_end)) = enclosing_function_span(&tokens, &matching_close, cursor) {
            if !function_binds_name(
                &tokens,
                &matching_close,
                &matching_open,
                fn_body,
                fn_end,
                name,
            ) {
                cursor += 1;
                continue;
            }
        }
        let mut reads = Vec::new();
        let mut scan = rhs_end + 1;
        if tokens.get(scan).map(|token| token.text) == Some(";")
            || tokens.get(scan).map(|token| token.text) == Some(",")
        {
            scan += 1;
        }
        // A later write to the copy (or a shadowing arrow parameter) does not
        // merely end the read scan: reads collected before it may re-execute
        // after the write inside a loop, and removing the copy assignment
        // would orphan the surviving uses. The whole fold must be abandoned.
        let mut name_rebound = false;
        let mut stopped_at = None;
        while scan < scope_end {
            if let Some(close) = nested_function_end(&tokens, &matching_close, scan) {
                if nested_function_assigns_captured_name(
                    &tokens,
                    &matching_close,
                    &matching_open,
                    scan,
                    close,
                    name,
                ) {
                    name_rebound = true;
                    break;
                }
                if nested_function_assigns_captured_name(
                    &tokens,
                    &matching_close,
                    &matching_open,
                    scan,
                    close,
                    tokens[cursor + 2].text,
                ) {
                    stopped_at = Some(scan);
                    break;
                }
                scan = close + 1;
                continue;
            }
            if tokens[scan].kind == TokenKind::Identifier
                && tokens[scan].text == name
                && !is_property_identifier(&tokens, scan)
            {
                if identifier_is_arrow_parameter(&tokens, scan) {
                    name_rebound = true;
                    break;
                }
                // Compound assignments are writes to the copy, not reads:
                // rewriting `b+=x` to the source would mutate the source.
                if tokens.get(scan + 1).is_some_and(|token| {
                    token.text.ends_with('=')
                        && !matches!(token.text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
                }) {
                    name_rebound = true;
                    break;
                }
                if matches!(
                    tokens.get(scan + 1).map(|token| token.text),
                    Some("++") | Some("--")
                ) || matches!(
                    tokens.get(scan.wrapping_sub(1)).map(|token| token.text),
                    Some("++") | Some("--")
                ) {
                    name_rebound = true;
                    break;
                }
                if tokens.get(scan.wrapping_sub(1)).map(|token| token.text) == Some("[") {
                    stopped_at = Some(scan);
                    break;
                }
                reads.push(scan);
            }
            // Any mutation of the copied source invalidates the remaining
            // reads: rebinding the base identifier, writing through a
            // computed index, or storing to the borrowed member itself.
            if tokens[scan].kind == TokenKind::Identifier
                && tokens[scan].text == tokens[cursor + 2].text
                && !is_property_identifier(&tokens, scan)
            {
                let next = tokens.get(scan + 1).map(|token| token.text);
                let assigning = |text: &str| {
                    text.ends_with('=')
                        && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
                };
                if next.is_some_and(assigning)
                    || matches!(next, Some("++") | Some("--"))
                    || matches!(
                        tokens.get(scan.wrapping_sub(1)).map(|token| token.text),
                        Some("++") | Some("--")
                    )
                {
                    stopped_at = Some(scan);
                    break;
                }
                if rhs_end == cursor + 4 {
                    if next == Some("[") {
                        stopped_at = Some(scan);
                        break;
                    }
                    if next == Some(".")
                        && tokens.get(scan + 2).map(|token| token.text)
                            == Some(tokens[cursor + 4].text)
                        && tokens
                            .get(scan + 3)
                            .map(|token| token.text)
                            .is_some_and(assigning)
                    {
                        stopped_at = Some(scan);
                        break;
                    }
                }
            }
            scan += 1;
        }
        // A source mutation ends the replaceable region, but any use of the
        // copy at or past that point still needs the copy's value: deleting
        // the assignment would leave those reads bound to a stale earlier
        // definition (or nothing at all), so the fold must back off entirely.
        if let Some(stop) = stopped_at {
            let mut probe = stop;
            while probe < scope_end {
                if tokens[probe].kind == TokenKind::Identifier
                    && tokens[probe].text == name
                    && !is_property_identifier(&tokens, probe)
                {
                    name_rebound = true;
                    break;
                }
                probe += 1;
            }
        }
        if name_rebound {
            cursor += 1;
            continue;
        }
        if rhs_end != cursor + 2
            && reads
                .iter()
                .any(|&read| !identifier_is_expression_slot(&tokens, read))
        {
            cursor += 1;
            continue;
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
        if rhs_end == cursor + 4
            && reads.iter().any(|&read| {
                source_object_invoked_between(
                    &tokens,
                    &matching_close,
                    rhs_end + 1,
                    read,
                    tokens[cursor + 2].text,
                )
            })
        {
            cursor += 1;
            continue;
        }
        // A captured cell can change through a sibling closure invoked by a
        // call that does not spell that assignment in this window. Folding
        // `oldValue=current` across `track(fn)` would re-read the cell.
        if rhs_end == cursor + 2
            && source_may_change_across_calls(
                &tokens,
                &matching_close,
                &matching_open,
                cursor,
                tokens[cursor + 2].text,
            )
            && reads
                .iter()
                .any(|&read| call_occurs_between(&tokens, &matching_close, rhs_end + 1, read))
        {
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

fn source_may_change_across_calls(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    cursor: usize,
    source: &str,
) -> bool {
    if let Some((fn_body, fn_end)) = enclosing_function_span(tokens, matching_close, cursor) {
        if function_binds_name(
            tokens,
            matching_close,
            matching_open,
            fn_body,
            fn_end,
            source,
        ) {
            return name_assigned_in_nested_function(
                tokens,
                matching_close,
                matching_open,
                fn_body,
                fn_end,
                source,
            );
        }
    }
    name_assigned_in_nested_function(
        tokens,
        matching_close,
        matching_open,
        0,
        tokens.len(),
        source,
    )
}

fn name_assigned_in_nested_function(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    start: usize,
    end: usize,
    name: &str,
) -> bool {
    let mut index = start;
    while index < end {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if nested_function_assigns_captured_name(
                tokens,
                matching_close,
                matching_open,
                index,
                close,
                name,
            ) {
                return true;
            }
            index = close + 1;
            continue;
        }
        index += 1;
    }
    false
}

fn call_occurs_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> bool {
    let mut index = from;
    while index < to {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if close >= to {
                return false;
            }
            index = close + 1;
            continue;
        }
        if tokens[index].text == "new" {
            return true;
        }
        if tokens[index].text == "(" {
            if let Some(prev) = index.checked_sub(1).and_then(|prev| tokens.get(prev)) {
                if prev.kind == TokenKind::Identifier || matches!(prev.text, ")" | "]") {
                    return true;
                }
            }
        }
        index += 1;
    }
    false
}

fn nested_function_assigns_captured_name(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    _scan: usize,
    close: usize,
    name: &str,
) -> bool {
    let Some(body) = matching_open.get(close).copied().flatten() else {
        return false;
    };
    let shadowed = function_binds_name(tokens, matching_close, matching_open, body, close, name);
    let assigning = |text: &str| {
        text.ends_with('=') && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
    };
    let mut index = body + 1;
    while index < close {
        if let Some(inner_close) = nested_function_end(tokens, matching_close, index) {
            if nested_function_assigns_captured_name(
                tokens,
                matching_close,
                matching_open,
                index,
                inner_close,
                name,
            ) {
                return true;
            }
            index = inner_close + 1;
            continue;
        }
        if shadowed {
            index += 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && !is_property_identifier(tokens, index)
        {
            if tokens
                .get(index + 1)
                .map(|token| token.text)
                .is_some_and(assigning)
                || matches!(
                    tokens.get(index + 1).map(|token| token.text),
                    Some("++") | Some("--")
                )
                || matches!(
                    tokens.get(index.wrapping_sub(1)).map(|token| token.text),
                    Some("++") | Some("--")
                )
            {
                return true;
            }
        }
        index += 1;
    }
    false
}

pub(crate) fn assignment_span_to_remove(
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
    // A comma sequence opening a block (`{X,rest}`) terminates the removed
    // assignment with `,` instead of `;`; leaving it would orphan the comma.
    let end = if matches!(
        tokens.get(after).map(|token| token.text),
        Some(";") | Some(",")
    ) {
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

fn span_has_eager_member_read(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    end: usize,
) -> bool {
    let mut index = from;
    while index < end {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if close >= end {
                return false;
            }
            index = close + 1;
            continue;
        }
        if matches!(tokens[index].text, "." | "[") {
            return true;
        }
        index += 1;
    }
    false
}

fn span_contains_call(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    end: usize,
) -> bool {
    let mut index = from;
    while index < end {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if close >= end {
                return false;
            }
            index = close + 1;
            continue;
        }
        if tokens[index].text == "("
            && index > from
            && (tokens[index - 1].kind == TokenKind::Identifier
                || matches!(tokens[index - 1].text, ")" | "]" | "}" | "."))
        {
            return true;
        }
        index += 1;
    }
    false
}

fn source_object_invoked_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
    base: &str,
) -> bool {
    let mut index = from;
    while index < to {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if close >= to {
                return false;
            }
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == base
            && !is_property_identifier(tokens, index)
        {
            let next = tokens.get(index + 1).map(|token| token.text);
            if next == Some("(") {
                return true;
            }
            if next == Some(".") {
                let mut cursor = index + 2;
                while cursor + 1 < to
                    && tokens[cursor].kind == TokenKind::Identifier
                    && tokens.get(cursor + 1).map(|token| token.text) == Some(".")
                {
                    cursor += 2;
                }
                if cursor < to
                    && tokens[cursor].kind == TokenKind::Identifier
                    && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
                {
                    return true;
                }
            }
            let prev = tokens.get(index.wrapping_sub(1)).map(|token| token.text);
            if matches!(prev, Some("(") | Some(",")) && matches!(next, Some(")") | Some(",")) {
                return true;
            }
        }
        index += 1;
    }
    false
}

fn assigning_operator(text: &str) -> bool {
    text.ends_with('=') && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
}

/// `obj.prop` names whatever `obj` currently refers to. Rematerializing a
/// copied member after `obj` is rebound (or that property is written) would
/// read a different JavaScript value — including every `extern class` / DOM
/// field spelled the same way.
fn source_receiver_overwritten_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
    base: &str,
    prop: Option<&str>,
) -> bool {
    let mut index = from;
    while index < to {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if close >= to {
                return false;
            }
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == base
            && !is_property_identifier(tokens, index)
        {
            let next = tokens.get(index + 1).map(|token| token.text);
            let prev = index
                .checked_sub(1)
                .map(|prev| tokens[prev].text)
                .unwrap_or(";");
            if prev == "delete"
                || next == Some("[")
                || next.is_some_and(assigning_operator)
                || matches!(next, Some("++") | Some("--"))
                || matches!(
                    tokens.get(index.wrapping_sub(1)).map(|token| token.text),
                    Some("++") | Some("--")
                )
            {
                return true;
            }
            if let Some(prop) = prop {
                if next == Some(".") && tokens.get(index + 2).map(|token| token.text) == Some(prop) {
                    let after = tokens.get(index + 3).map(|token| token.text);
                    if after.is_some_and(|text| {
                        matches!(text, "++" | "--") || assigning_operator(text)
                    }) {
                        return true;
                    }
                }
            }
        }
        index += 1;
    }
    false
}

fn source_object_passed_as_argument_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
    base: &str,
) -> bool {
    let mut index = from;
    while index < to {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if close >= to {
                return false;
            }
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == base
            && !is_property_identifier(tokens, index)
        {
            let next = tokens.get(index + 1).map(|token| token.text);
            let prev = tokens.get(index.wrapping_sub(1)).map(|token| token.text);
            if matches!(prev, Some("(") | Some(",")) && matches!(next, Some(")") | Some(",")) {
                return true;
            }
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
    let prior =
        collect_unbound_name_uses(tokens, matching_close, name, scope_start, name_at, name_at)
            .into_iter()
            .filter(|&use_at| identifier_is_read(tokens, use_at, use_at + 1, name))
            .collect::<Vec<_>>();
    if prior
        .iter()
        .any(|&use_at| !use_is_in_nested_function(tokens, matching_close, scope_start, use_at))
    {
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

/// `var V=EXPR;callee(V…)` where the binding's only use opens the argument
/// list of the immediately following call collapses to `callee(EXPR…)`: the
/// callee chain is a plain member read, so evaluation order of `EXPR` and any
/// later arguments is unchanged, and the binding disappears entirely.
pub(crate) fn fold_adjacent_binding_into_leading_call_arg(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if !matches!(tokens[cursor].text, "var" | "let")
            || !matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}"
            )
        {
            cursor += 1;
            continue;
        }
        let name_at = cursor + 1;
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[name_at].text;
        let rhs_start = name_at + 2;
        let Some(stop) = top_level_stop(&tokens, rhs_start, &[";", ","]) else {
            cursor += 1;
            continue;
        };
        if tokens[stop].text != ";" || stop == rhs_start {
            cursor += 1;
            continue;
        }
        let mut chain = stop + 1;
        if tokens
            .get(chain)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens[chain].text == name
        {
            cursor += 1;
            continue;
        }
        while tokens.get(chain + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(chain + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            chain += 2;
        }
        let use_at = chain + 2;
        // A use that is not a whole argument (something follows it before the
        // `,` or `)`) splices the inlined expression into a larger expression,
        // which is only precedence-safe for a primary chain: `arrow([...])`
        // is a syntax error and `b||c+1` rebinds. Such uses require the
        // expression to be a member/call chain rooted in a single atom.
        if tokens.get(chain + 1).map(|token| token.text) != Some("(")
            || tokens
                .get(use_at)
                .is_none_or(|token| token.kind != TokenKind::Identifier || token.text != name)
            || name_use_is_mutated(&tokens, use_at)
        {
            cursor += 1;
            continue;
        }
        if !matches!(
            tokens.get(use_at + 1).map(|token| token.text),
            Some(",") | Some(")")
        ) && !expression_is_primary_chain(&tokens, &matching_close, rhs_start, stop)
        {
            cursor += 1;
            continue;
        }
        let scope_start = enclosing_block_start(&matching_close, name_at)
            .map(|open| open + 1)
            .unwrap_or(0);
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        let uses = collect_binding_uses(
            &tokens,
            &matching_close,
            name,
            name_at,
            stop + 1,
            scope_start,
            scope_end,
        );
        if uses != Some(vec![use_at]) {
            cursor += 1;
            continue;
        }
        let expression = &source[tokens[rhs_start].start..tokens[stop - 1].end];
        if !replacement_overlaps(&replacements, tokens[cursor].start, tokens[use_at].end) {
            replacements.push((tokens[cursor].start, tokens[stop].end, String::new()));
            replacements.push((
                tokens[use_at].start,
                tokens[use_at].end,
                wrap_substituted_expression(&tokens, use_at, expression),
            ));
        }
        cursor = use_at + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count / 2))
}

/// A member/call chain rooted in one atom (`x`, `x.y[0](a).z`, `"s".big()`)
/// keeps its meaning when pasted into any expression slot: it parses as a
/// single CallExpression/MemberExpression with maximal binding power. Any
/// top-level operator, arrow, or keyword breaks that guarantee.
fn expression_is_primary_chain(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> bool {
    let Some(first) = tokens.get(from) else {
        return false;
    };
    if !matches!(
        first.kind,
        TokenKind::Identifier | TokenKind::Number | TokenKind::String
    ) {
        return false;
    }
    let mut index = from + 1;
    while index < to {
        match tokens[index].text {
            "." => {
                // Contextual keywords (`get`, `set`, ...) lex as Keyword but
                // are ordinary property names after a dot.
                if tokens.get(index + 1).is_none_or(|token| {
                    !matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword)
                }) {
                    return false;
                }
                index += 2;
            }
            "(" | "[" => {
                let Some(close) = matching_close.get(index).copied().flatten() else {
                    return false;
                };
                if close >= to {
                    return false;
                }
                index = close + 1;
            }
            _ => return false,
        }
    }
    true
}

fn can_rematerialize_literal(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    use_at: usize,
    rhs_kind: TokenKind,
    effectful: bool,
    guarded: bool,
    rhs_from: usize,
    rhs_end: usize,
    after_init: usize,
) -> bool {
    let nested = use_is_in_nested_function(tokens, matching_close, from, use_at);
    if effectful && (guarded || nested) {
        return false;
    }
    if nested && rhs_kind != TokenKind::Regex {
        return false;
    }
    if tokens.get(use_at.wrapping_sub(1)).map(|token| token.text) == Some("=")
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
    if effectful
        && rhs_identifier_assigned_between(
            tokens,
            matching_close,
            rhs_from,
            rhs_end,
            after_init,
            use_at,
        )
    {
        return false;
    }
    true
}

fn rhs_identifier_assigned_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    rhs_from: usize,
    rhs_end: usize,
    from: usize,
    use_at: usize,
) -> bool {
    let assigning = |text: &str| {
        text.ends_with('=') && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
    };
    let mut names = Vec::new();
    let mut index = rhs_from;
    while index <= rhs_end {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier && !is_property_identifier(tokens, index) {
            let name = tokens[index].text;
            if !names.iter().any(|existing| *existing == name) {
                names.push(name);
            }
        }
        index += 1;
    }
    if names.is_empty() {
        return false;
    }
    let mut index = from;
    while index < use_at {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && names.iter().any(|name| *name == tokens[index].text)
            && !is_property_identifier(tokens, index)
            && (tokens
                .get(index + 1)
                .is_some_and(|token| assigning(token.text))
                || matches!(
                    tokens.get(index + 1).map(|token| token.text),
                    Some("++") | Some("--")
                )
                || matches!(
                    tokens.get(index.wrapping_sub(1)).map(|token| token.text),
                    Some("++") | Some("--")
                ))
        {
            return true;
        }
        index += 1;
    }
    false
}

fn use_is_guarded(tokens: &[Token<'_>], from: usize, use_at: usize) -> bool {
    let mut depth = 0i32;
    let mut index = from;
    while index < use_at {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "if" | "for" | "while" | "do" | "?" | "&&" | "||" | "??" if depth == 0 => {
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
    substituted_expression_needs_grouping(tokens, use_at, literal)
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
                && identifier_is_expression_slot(&tokens, uses[0])
                && !name_use_is_mutated(&tokens, uses[0])
                && !identifier_occurs(&tokens, name_at + 2, literal_end + 1, name)
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
                    name_at + 2,
                    literal_end,
                    stop + 1,
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
            && identifier_is_expression_slot(&tokens, uses[0])
            && !name_use_is_mutated(&tokens, uses[0])
            && !identifier_occurs(&tokens, cursor + 2, literal_end + 1, name)
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
                cursor + 2,
                literal_end,
                literal_end + 1,
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
    let matching_close = matching_closers(&tokens);
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
        // A top-level comma would make the span a following declarator (or a
        // comma expression), so the key must run unbroken to the semicolon.
        let Some(semi) = top_level_stop(&tokens, name_at + 2, &[";", ","]) else {
            cursor += 1;
            continue;
        };
        if tokens[semi].text != ";" {
            cursor += 1;
            continue;
        }
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
        // A `var` declarator (and a bare assignment to a pooled name) is
        // function-scoped: the emitter reuses hoisted names far outside the
        // enclosing block, so the no-other-use scan must cover the whole
        // enclosing function span on both sides of the fold site.
        let (scope_start, scope_end) = enclosing_function_span(&tokens, &matching_close, name_at)
            .map(|(body, end)| (body + 1, end))
            .unwrap_or((0, tokens.len()));
        if identifier_occurs(&tokens, scope_start, cursor, name)
            || identifier_occurs(&tokens, name_at + 2, semi, name)
            || identifier_occurs(&tokens, semi + 6, assign_end, name)
            || identifier_occurs(&tokens, assign_end + 1, scope_end, name)
        {
            cursor += 1;
            continue;
        }
        let key = &source[tokens[name_at + 2].start..tokens[semi].start];
        let object = tokens[semi + 1].text;
        let value = &source[tokens[semi + 6].start..tokens[assign_end].start];
        // Dropping a trailing declarator consumes the declaration's own
        // semicolon, so the replacement must re-terminate the declaration.
        let (from, prefix) = if name_at
            .checked_sub(1)
            .is_some_and(|index| tokens[index].text == ",")
        {
            (tokens[name_at - 1].start, ";")
        } else {
            (tokens[cursor].start, "")
        };
        let replace_end = if tokens[assign_end].text == "}" {
            tokens[assign_end].start
        } else {
            tokens[assign_end].end
        };
        replacements.push((
            from,
            replace_end,
            format!("{prefix}{object}[{key}]={value};"),
        ));
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
        // Reads are only replaceable inside the enclosing block (the temp may
        // be assigned conditionally), but a use beyond the block still sees
        // the assigned value — and when the binding is a `var` declarator it
        // hoists function-wide, so even uses before it need the declaration.
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        let (function_start, function_end) =
            enclosing_function_span(&tokens, &matching_close, cursor)
                .map(|(body, end)| (body + 1, end))
                .unwrap_or((0, tokens.len()));
        let declared = cursor
            .checked_sub(1)
            .is_some_and(|index| matches!(tokens[index].text, "var" | "let" | "const"));
        if (declared && identifier_occurs(&tokens, function_start, cursor, name))
            || identifier_occurs(&tokens, scope_end, function_end, name)
        {
            cursor += 1;
            continue;
        }
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
                // An arrow parameter of the same name is a fresh binding, and
                // later same-name tokens may read it rather than the temp.
                if identifier_is_arrow_parameter(&tokens, scan) {
                    safe = false;
                    break;
                }
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
                    || (tokens.get(scan.wrapping_sub(1)).map(|token| token.text) == Some("[")
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
        let chain_base = tokens[cursor + 2].text;
        if reads.iter().any(|&read| {
            source_object_invoked_between(&tokens, &matching_close, rhs_end + 1, read, chain_base)
        }) {
            cursor += 1;
            continue;
        }
        let (from, to) = assignment_span_to_remove(&tokens, cursor, rhs_end);
        // A chain such as `k=e[0],h=k[3];h.add()` exposes two otherwise-valid
        // candidates in the same token stream. Applying both groups at once
        // makes `h`'s deletion overlap the substitution inside its initializer;
        // the generic rewrite filter would retain the deletion but discard the
        // substitution, leaving `k[3]` after `k` itself was removed. Keep every
        // delete+substitute group atomic. The optimizer repeats this fold, so
        // the remaining link is rematerialized safely in the following round.
        if replacement_overlaps(&replacements, from, to)
            || reads.iter().any(|read| {
                replacement_overlaps(&replacements, tokens[*read].start, tokens[*read].end)
            })
        {
            cursor += 1;
            continue;
        }
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
        if source_receiver_overwritten_between(
            &tokens,
            &matching_close,
            stop + 1,
            use_at,
            tokens[cursor + 3].text,
            Some(tokens[cursor + 5].text),
        ) || source_object_passed_as_argument_between(
            &tokens,
            &matching_close,
            stop + 1,
            use_at,
            tokens[cursor + 3].text,
        ) {
            cursor += 1;
            continue;
        }
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
        // A declaration head means this is the binding itself, not a
        // reassignment of an outer name: rewriting would orphan the keyword
        // (`let e=a;e.x=` must not become `let a.x=`).
        if cursor
            .checked_sub(1)
            .is_some_and(|index| matches!(tokens[index].text, "var" | "let" | "const"))
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let Some((body, end)) = enclosing_function_span(&tokens, &matching_close, cursor) else {
            cursor += 1;
            continue;
        };
        if function_binds_name(&tokens, &matching_close, &matching_open, body, end, name) {
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
    ) || tokens.get(use_at.wrapping_sub(1)).map(|token| token.text) == Some("new")
}

fn function_literal_move_changes_capture(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    literal_start: usize,
    function: FunctionExpression,
    scope_start: usize,
    use_at: usize,
) -> bool {
    let own_name = function
        .named
        .then(|| tokens.get(literal_start + 1))
        .flatten()
        .filter(|token| token.kind == TokenKind::Identifier)
        .map(|token| token.text);
    for at in literal_start..=function.end {
        if tokens[at].kind != TokenKind::Identifier || is_property_identifier(tokens, at) {
            continue;
        }
        let name = tokens[at].text;
        if own_name == Some(name) {
            continue;
        }
        let bound_by_literal = if let Some(body) = function.block_open {
            function_binds_name(
                tokens,
                matching_close,
                matching_open,
                body,
                function.end,
                name,
            )
        } else {
            tokens[function.params_from..function.params_to]
                .iter()
                .any(|parameter| parameter.kind == TokenKind::Identifier && parameter.text == name)
        };
        if bound_by_literal {
            continue;
        }
        if name_is_bound_in_nested_function_between(
            tokens,
            matching_close,
            scope_start,
            use_at,
            name,
        ) {
            return true;
        }
    }
    false
}

pub(crate) fn fold_single_use_function_values(
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
            let Some(function) = parse_function_expression(&tokens, &matching_close, name_at + 2)
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
            if uses.len() == 1
                && identifier_is_expression_slot(&tokens, uses[0])
                && !name_use_is_mutated(&tokens, uses[0])
            {
                let use_at = uses[0];
                let called = tokens.get(use_at + 1).map(|token| token.text) == Some("(");
                let member_or_new = matches!(
                    tokens.get(use_at + 1).map(|token| token.text),
                    Some(".") | Some("[")
                ) || tokens.get(use_at.wrapping_sub(1)).map(|token| token.text)
                    == Some("new");
                let expression_arrow = function.is_arrow && function.block_open.is_none();
                // A single called use keeps identical semantics as an IIFE
                // whether or not the body returns: the call site consumes the
                // same value either way, and recursion is excluded above.
                let procedure_iife = called && function.block_open.is_some();
                let nested =
                    use_is_in_nested_function(&tokens, &matching_close, scope_start, use_at);
                let capture_changes = nested
                    && function_literal_move_changes_capture(
                        &tokens,
                        &matching_close,
                        &matching_open,
                        name_at + 2,
                        function,
                        scope_start,
                        use_at,
                    );
                let allow = if member_or_new {
                    false
                } else if called {
                    (expression_arrow || procedure_iife) && !capture_changes
                } else {
                    !nested
                };
                if allow {
                    let (from, to) = assignment_span_to_remove(&tokens, name_at, function.end);
                    if !replacement_overlaps(
                        &replacements,
                        tokens[use_at].start,
                        tokens[use_at].end,
                    ) && !replacement_overlaps(&replacements, from, to)
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
        if nested_use
            || uses.is_empty()
            || uses
                .iter()
                .any(|&use_at| name_use_is_mutated(&tokens, use_at))
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
        let base = tokens[cursor].text;
        let property = tokens[cursor + 2].text;
        let assigning = |text: &str| {
            text.ends_with('=') && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
        };
        let scope_end = enclosing_block_end(&matching_close, cursor).unwrap_or(tokens.len());
        let mut index = scan + 2;
        while index + 1 < scope_end {
            if let Some(close) = nested_function_end(&tokens, &matching_close, index) {
                index = close + 1;
                continue;
            }
            if tokens[index].kind == TokenKind::Identifier
                && tokens[index].text == name
                && tokens
                    .get(index + 1)
                    .map(|token| token.text)
                    .is_some_and(assigning)
            {
                break;
            }
            // Rebinding the alias base (or rewriting the aliased property)
            // detaches `base.property` from the copied object: later reads
            // must keep using the original name.
            if tokens[index].kind == TokenKind::Identifier
                && tokens[index].text == base
                && !is_property_identifier(&tokens, index)
            {
                let next = tokens.get(index + 1).map(|token| token.text);
                if next.is_some_and(assigning)
                    || matches!(next, Some("++") | Some("--") | Some("["))
                    || (next == Some(".")
                        && tokens.get(index + 2).map(|token| token.text) == Some(property)
                        && tokens
                            .get(index + 3)
                            .map(|token| token.text)
                            .is_some_and(assigning))
                {
                    break;
                }
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
            // The use scan is bounded by the enclosing block, but a `var`
            // binding hoists function-wide: occurrences outside the block
            // still need the declaration, so they forbid deleting it.
            let (function_start, function_end) =
                enclosing_function_span(&tokens, &matching_close, name_at)
                    .map(|(body, end)| (body + 1, end))
                    .unwrap_or((0, tokens.len()));
            if uses.len() == 1
                && identifier_is_expression_slot(&tokens, uses[0])
                && !name_use_is_mutated(&tokens, uses[0])
                && !name_use_is_invoke(&tokens, uses[0])
                && !identifier_occurs(&tokens, function_start, scope_start, name)
                && !identifier_occurs(&tokens, scope_end, function_end, name)
                && !(span_has_eager_member_read(&tokens, &matching_close, name_at + 2, object_end)
                    && span_contains_call(&tokens, &matching_close, object_end + 1, uses[0]))
            {
                let use_at = uses[0];
                let (from, to) = assignment_span_to_remove(&tokens, name_at, object_end);
                if !replacement_overlaps(&replacements, tokens[use_at].start, tokens[use_at].end)
                    && !replacement_overlaps(&replacements, from, to)
                {
                    let literal = &source[tokens[name_at + 2].start..tokens[object_end].end];
                    replacements.push((
                        tokens[use_at].start,
                        tokens[use_at].end,
                        literal.to_string(),
                    ));
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

fn member_index_end(tokens: &[Token<'_>], open: usize) -> Option<usize> {
    if tokens.get(open).map(|token| token.text) != Some("[") {
        return None;
    }
    let mut depth = 1i32;
    let mut index = open + 1;
    while index < tokens.len() {
        match tokens[index].text {
            "[" => depth += 1,
            "]" => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

fn copied_method_parts(tokens: &[Token<'_>], at: usize) -> Option<(usize, usize)> {
    if !tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == "this")
    {
        return None;
    }
    let mut recv_end = at;
    let mut index = at + 1;
    loop {
        if tokens.get(index).map(|token| token.text) == Some(".")
            && tokens
                .get(index + 1)
                .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
        {
            let method_at = index + 1;
            let after = method_at + 1;
            if matches!(tokens.get(after).map(|token| token.text), Some("," | ";")) {
                return Some((recv_end, method_at));
            }
            recv_end = method_at;
            index = after;
            continue;
        }
        if tokens.get(index).map(|token| token.text) == Some("[") {
            let close = member_index_end(tokens, index)?;
            recv_end = close;
            index = close + 1;
            continue;
        }
        return None;
    }
}

pub(crate) fn fold_copied_method_call(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        let name_at = if matches!(tokens[cursor].text, "var" | "let" | "const") {
            cursor + 1
        } else {
            cursor
        };
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || is_property_identifier(&tokens, name_at)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some((recv_end, method_at)) = copied_method_parts(&tokens, name_at + 2) else {
            cursor += 1;
            continue;
        };
        let name = tokens[name_at].text;
        let recv = &source[tokens[name_at + 2].start..tokens[recv_end].end];
        let method = tokens[method_at].text;
        let mut call_at = method_at + 2;
        let saw_return = tokens.get(call_at).map(|token| token.text) == Some("return");
        if saw_return {
            call_at += 1;
        }
        let assign_result = tokens.get(call_at).map(|token| token.text) == Some(name)
            && tokens.get(call_at + 1).map(|token| token.text) == Some("=");
        if assign_result {
            call_at += 2;
        }
        if tokens.get(call_at).map(|token| token.text) != Some(name)
            || tokens.get(call_at + 1).map(|token| token.text) != Some(".")
            || tokens.get(call_at + 2).map(|token| token.text) != Some("call")
            || tokens.get(call_at + 3).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let Some(close) = matching_close.get(call_at + 3).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let recv_end_in_call = call_at + 4 + (recv_end - name_at - 2);
        if recv_end_in_call >= close
            || source[tokens[call_at + 4].start..tokens[recv_end_in_call].end] != *recv
        {
            cursor += 1;
            continue;
        }
        let after_recv = recv_end_in_call + 1;
        if tokens.get(after_recv).map(|token| token.text) != Some(",") && after_recv != close {
            cursor += 1;
            continue;
        }
        if identifier_occurs(&tokens, method_at + 2, call_at, name)
            || identifier_occurs(&tokens, call_at + 4, close, name)
        {
            cursor += 1;
            continue;
        }
        let args = if after_recv == close {
            String::new()
        } else {
            source[tokens[after_recv + 1].start..tokens[close].start].to_string()
        };
        let call = format!("{recv}.{method}({args})");
        let replacement = if assign_result {
            format!("{name}={call}")
        } else if saw_return {
            format!("return {call}")
        } else {
            call
        };
        replacements.push((tokens[cursor].start, tokens[close].end, replacement));
        cursor = close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}
