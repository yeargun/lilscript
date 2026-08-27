use crate::js_peephole::rewrite::{
    apply_token_rewrites, assign_is_in_declaration, identifier_is_expression_slot,
    identifier_is_read, identifier_occurs, is_property_identifier, parse_bare_assign,
    replacement_overlaps, substituted_expression_needs_grouping, top_level_stop,
};
use crate::js_peephole::scope::{
    collect_same_scope_name_uses, collect_unbound_name_uses, enclosing_block_end,
    enclosing_block_start, enclosing_function_span, function_binds_name,
    identifier_is_arrow_parameter, name_is_bound_in_nested_function_between,
    name_is_declared_in_visible_scope, name_use_is_mutated, nested_function_end,
    parse_function_expression, use_is_in_nested_function, FunctionExpression,
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
        if !matches!(
            tokens.get(cursor + 3).map(|token| token.text),
            Some(";") | Some(",") | Some("}") | None
        ) {
            cursor += 1;
            continue;
        }
        let rhs = tokens[cursor + 2].text.to_string();
        let rhs_end = cursor + 2;
        // A self-copy is not useful and deleting it could expose an earlier
        // value to reads that followed the assignment.
        if tokens[cursor + 2].text == name {
            cursor += 1;
            continue;
        }
        // Rewrites in one pass are derived from the same token snapshot. If
        // the source binding was itself established by an earlier identifier
        // copy, that earlier copy can be removed while rewriting this copy's
        // uses with the now-stale source name. Defer the dependent copy until
        // the next fixed-point pass has reparsed the earlier rewrite. For
        // example, folding both `slot=first;slot=second` and `out=slot` at
        // once must not leave `out`'s reads pointing at `first`.
        let binding_scope_start = enclosing_function_span(&tokens, &matching_close, cursor)
            .map(|(body, _)| body + 1)
            .unwrap_or(0);
        if has_prior_identifier_copy_assignment(
            &tokens,
            &matching_close,
            binding_scope_start,
            cursor,
            &rhs,
        ) {
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
        if reads.is_empty() {
            let scope_start = enclosing_block_start(&matching_close, cursor)
                .map(|open| open + 1)
                .unwrap_or(0);
            if !name_is_declared_in_visible_scope(
                &tokens,
                &matching_close,
                cursor,
                tokens[cursor + 2].text,
            ) {
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

fn has_prior_identifier_copy_assignment(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
    name: &str,
) -> bool {
    let mut scan = start;
    while scan + 3 < end {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            scan = close + 1;
            continue;
        }
        let previous = scan
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .map(|token| token.text)
            .unwrap_or(";");
        if tokens[scan].kind == TokenKind::Identifier
            && tokens[scan].text == name
            && tokens.get(scan + 1).map(|token| token.text) == Some("=")
            && tokens
                .get(scan + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && matches!(
                tokens.get(scan + 3).map(|token| token.text),
                Some(";") | Some(",") | Some("}")
            )
            && matches!(previous, ";" | "{" | "}" | "let" | "var" | "const" | ",")
        {
            return true;
        }
        scan += 1;
    }
    false
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
            && (tokens
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
                ))
        {
            return true;
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

fn cheap_literal_end(tokens: &[Token<'_>], rhs: usize) -> Option<usize> {
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
    None
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

#[allow(clippy::too_many_arguments)]
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
            if !names.contains(&name) {
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
            let Some(literal_end) = cheap_literal_end(&tokens, name_at + 2) else {
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
                    false,
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
        let Some(literal_end) = cheap_literal_end(&tokens, cursor + 2) else {
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
                false,
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

/// Move the leading assignment of a parenthesized sequence into its first use.
///
/// `(x=R,"number"==typeof x?A:B)` becomes
/// `"number"==typeof(x=R)?A:B`. Only literal/operator tokens may precede the
/// first use, so delaying the assignment across that prefix cannot reorder an
/// observable evaluation. This is the small, proven `collapse_vars` subset
/// generated control-flow tends to expose after return-branch folding.
pub(crate) fn fold_sequence_assignments_into_first_use(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();

    for open in 0..tokens.len() {
        if tokens[open].text != "("
            || tokens
                .get(open + 1)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(open + 2).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let Some(close) = matching_close[open] else {
            continue;
        };
        let Some(comma) = top_level_stop(&tokens, open + 3, &[","]) else {
            continue;
        };
        if comma >= close || comma == open + 3 {
            continue;
        }
        // A second root comma would remain a sequence after the leading
        // assignment moved, so the outer parentheses could not be elided.
        if top_level_stop(&tokens, comma + 1, &[","]).is_some_and(|next| next < close) {
            continue;
        }
        let name = tokens[open + 1].text;
        let mut use_at = None;
        for index in comma + 1..close {
            let token = &tokens[index];
            if token.kind == TokenKind::Identifier {
                if token.text == name
                    && !is_property_identifier(&tokens, index)
                    && !name_use_is_mutated(&tokens, index)
                {
                    use_at = Some(index);
                } else if !matches!(token.text, "typeof" | "void" | "true" | "false" | "null") {
                    break;
                }
            } else if !sequence_assignment_inert_prefix_token(token) {
                break;
            }
            if use_at.is_some() {
                break;
            }
        }
        let Some(use_at) = use_at else {
            continue;
        };

        let previous = open
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .map(|token| token.text);
        let next = tokens.get(close + 1).map(|token| token.text);
        if next.is_some_and(|token| matches!(token, "." | "[" | "(" | "?." | "**")) {
            continue;
        }
        let tail_has_conditional = has_top_level_token(&tokens, comma + 1, close, "?");
        let can_drop_outer = if tail_has_conditional {
            previous.is_some_and(|token| matches!(token, ":" | "?" | "return" | "=>"))
        } else if previous == Some("&&") {
            !["||", "??", "="]
                .into_iter()
                .any(|operator| has_top_level_token(&tokens, comma + 1, close, operator))
        } else {
            previous.is_some_and(|token| matches!(token, ":" | "?" | "return" | "=>"))
        };
        if !can_drop_outer {
            continue;
        }

        let assignment = &source[tokens[open + 1].start..tokens[comma].start];
        let prefix = source[tokens[comma].end..tokens[use_at].start].trim_end();
        let suffix = &source[tokens[use_at].end..tokens[close].start];
        replacements.push((
            tokens[open].start,
            tokens[close].end,
            format!("{prefix}({assignment}){suffix}"),
        ));
    }

    Ok(apply_token_rewrites(source, replacements))
}

/// Delay a standalone assignment across an inert prefix into its first read.
///
/// `x=R;return typeof x=="number"` becomes
/// `return typeof(x=R)=="number"`. Calls, property reads, short-circuit
/// operators, and other observable prefixes stop the scan, so the assignment
/// never crosses a value-producing or conditional evaluation.
/// Can the token at `index` be evaluated ahead of a moved assignment?
///
/// Placing `x=EXPR` at the first read of `x` in the following statement is safe
/// exactly when nothing evaluated before that read can observe `x`, change it,
/// or run code of its own. Reading a binding, a literal, and the pure operators
/// all qualify; a call, a property access, an assignment, or an update does not.
///
/// This replaces a hand-kept list of "inert" tokens that omitted identifiers,
/// `&&`, `?`, and much else, so the fold stopped at the first ordinary
/// expression and almost never fired.
fn prefix_cannot_observe(tokens: &[Token<'_>], index: usize) -> bool {
    let token = &tokens[index];
    match token.kind {
        // A template can run `toString` on an interpolation.
        TokenKind::Template => false,
        TokenKind::Number | TokenKind::String | TokenKind::Regex => true,
        // Reading a binding is pure. A property name is reached through `.`,
        // which is rejected below, so it never gets here.
        TokenKind::Identifier => true,
        TokenKind::Keyword => matches!(
            token.text,
            "return" | "typeof" | "void" | "if" | "while" | "true" | "false" | "null"
        ),
        TokenKind::Punct => match token.text {
            // Member access can run a getter.
            "." | "?." | "[" | "]" => false,
            // A `(` that follows a value is a call; otherwise it groups.
            "(" => index.checked_sub(1).is_none_or(|previous| {
                !matches!(tokens[previous].kind, TokenKind::Identifier)
                    && !matches!(tokens[previous].text, ")" | "]")
            }),
            ")" => true,
            "!" | "~" | "+" | "-" | "*" | "/" | "%" | "**" | "<<" | ">>" | ">>>" | "<" | "<="
            | ">" | ">=" | "==" | "!=" | "===" | "!==" | "&" | "^" | "|" | "&&" | "||" | "??"
            | "?" | ":" => true,
            _ => false,
        },
    }
}

pub(crate) fn fold_statement_assignments_into_first_use(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    #[derive(Clone)]
    struct Candidate {
        start: usize,
        finish: usize,
        remove_start: usize,
        remove_end: usize,
        use_start: usize,
        use_end: usize,
        assignment: String,
    }

    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut matching_open = vec![None; tokens.len()];
    for (open, close) in matching_close.iter().enumerate() {
        if let Some(close) = *close {
            matching_open[close] = Some(open);
        }
    }
    let mut paren_depth = vec![0i32; tokens.len()];
    let mut bracket_depth = vec![0i32; tokens.len()];
    let mut brace_depth = vec![0i32; tokens.len()];
    let (mut parens, mut brackets, mut braces) = (0i32, 0i32, 0i32);
    for (index, token) in tokens.iter().enumerate() {
        paren_depth[index] = parens;
        bracket_depth[index] = brackets;
        brace_depth[index] = braces;
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

    let mut candidates = Vec::new();
    for cursor in 0..tokens.len().saturating_sub(3) {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || paren_depth[cursor] != 0
            || bracket_depth[cursor] != 0
            || is_property_identifier(&tokens, cursor)
        {
            continue;
        }
        let previous = cursor
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .map(|token| token.text);
        let statement_start = matches!(previous, None | Some("{") | Some(";"))
            || cursor.checked_sub(1).is_some_and(|close| {
                tokens[close].text == ")"
                    && matching_open[close].is_some_and(|open| {
                        open.checked_sub(1)
                            .is_some_and(|before| matches!(tokens[before].text, "for" | "while"))
                    })
            });
        if !statement_start {
            continue;
        }
        // A comma-separated declarator is not a sequence expression. Walk to
        // the current root statement boundary and reject the complete
        // declaration family together.
        let mut statement = cursor;
        while statement > 0 {
            let previous = statement - 1;
            if paren_depth[previous] == paren_depth[cursor]
                && bracket_depth[previous] == bracket_depth[cursor]
                && brace_depth[previous] == brace_depth[cursor]
                && matches!(tokens[previous].text, ";" | "{" | "}")
            {
                break;
            }
            statement -= 1;
        }
        if tokens[statement..cursor]
            .iter()
            .any(|token| matches!(token.text, "var" | "let" | "const"))
        {
            continue;
        }

        let mut delimiter = None;
        for index in cursor + 2..tokens.len() {
            if paren_depth[index] == paren_depth[cursor]
                && bracket_depth[index] == bracket_depth[cursor]
                && brace_depth[index] == brace_depth[cursor]
                && matches!(tokens[index].text, "," | ";")
            {
                delimiter = Some(index);
                break;
            }
            if brace_depth[index] < brace_depth[cursor] {
                break;
            }
        }
        let Some(delimiter) = delimiter else {
            continue;
        };
        if delimiter == cursor + 2 {
            continue;
        }
        let next_start = delimiter + 1;
        if next_start >= tokens.len() || tokens[next_start].text == "}" {
            continue;
        }
        let mut next_end = tokens.len();
        for index in next_start..tokens.len() {
            if paren_depth[index] == paren_depth[cursor]
                && bracket_depth[index] == bracket_depth[cursor]
                && brace_depth[index] == brace_depth[cursor]
                && matches!(tokens[index].text, "," | ";")
            {
                next_end = index;
                break;
            }
            if brace_depth[index] < brace_depth[cursor] {
                next_end = index;
                break;
            }
        }
        let name = tokens[cursor].text;
        let mut use_at = None;
        for index in next_start..next_end {
            let token = &tokens[index];
            if token.kind == TokenKind::Identifier
                && token.text == name
                && !is_property_identifier(&tokens, index)
            {
                if name_use_is_mutated(&tokens, index) {
                    // A simple assignment target itself has no observable
                    // evaluation. Search its right side for the first read.
                    if tokens.get(index + 1).map(|token| token.text) == Some("=") {
                        continue;
                    }
                    break;
                }
                use_at = Some(index);
                break;
            }
            // Everything before the read has to be evaluated where it already
            // is, so it may not observe or disturb the value being moved.
            if !prefix_cannot_observe(&tokens, index) {
                break;
            }
        }
        let Some(use_at) = use_at else {
            continue;
        };
        candidates.push(Candidate {
            start: tokens[cursor].start,
            finish: tokens[use_at].end,
            remove_start: tokens[cursor].start,
            remove_end: tokens[delimiter].end,
            use_start: tokens[use_at].start,
            use_end: tokens[use_at].end,
            assignment: source[tokens[cursor].start..tokens[delimiter].start].to_string(),
        });
    }

    candidates.sort_by_key(|candidate| (candidate.start, candidate.finish));
    let mut retained = Vec::new();
    let mut last_end = 0usize;
    for candidate in candidates {
        if candidate.start >= last_end {
            last_end = candidate.finish;
            retained.push(candidate);
        }
    }
    let mut replacements = Vec::with_capacity(retained.len() * 2);
    for candidate in retained {
        replacements.push((candidate.remove_start, candidate.remove_end, String::new()));
        replacements.push((
            candidate.use_start,
            candidate.use_end,
            format!("({})", candidate.assignment),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn sequence_assignment_inert_prefix_token(token: &Token<'_>) -> bool {
    matches!(token.kind, TokenKind::Number | TokenKind::String)
        || matches!(
            token.text,
            "(" | ")"
                | "typeof"
                | "void"
                | "true"
                | "false"
                | "null"
                | "undefined"
                | "!"
                | "~"
                | "+"
                | "-"
                | "=="
                | "!="
                | "==="
                | "!=="
                | "<"
                | "<="
                | ">"
                | ">="
        )
}

fn has_top_level_token(tokens: &[Token<'_>], start: usize, end: usize, needle: &str) -> bool {
    let mut depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            text if depth == 0 && text == needle => return true,
            _ => {}
        }
    }
    false
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

fn parse_single_use_function_literal(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    literal_start: usize,
) -> Option<(usize, FunctionExpression)> {
    if let Some(function) = parse_function_expression(tokens, matching_close, literal_start) {
        return Some((literal_start, function));
    }

    // `parse_function_expression` intentionally models the common classic and
    // arrow forms only. Recognize an async arrow locally so this fold moves
    // the `async` modifier together with the literal instead of ever treating
    // its body as an ordinary function. A line terminator after `async` makes
    // it a different JavaScript grammar production, so fail closed there.
    if tokens.get(literal_start).map(|token| token.text) != Some("async") {
        return None;
    }
    let arrow_start = literal_start + 1;
    let gap = source.get(tokens[literal_start].end..tokens.get(arrow_start)?.start)?;
    if gap
        .chars()
        .any(|character| matches!(character, '\n' | '\r' | '\u{2028}' | '\u{2029}'))
    {
        return None;
    }
    let function = parse_function_expression(tokens, matching_close, arrow_start)?;
    function.is_arrow.then_some((literal_start, function))
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
            let Some((literal_start, function)) =
                parse_single_use_function_literal(source, &tokens, &matching_close, name_at + 2)
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
            if identifier_occurs(&tokens, literal_start, function.end + 1, name) {
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
                // Anonymous functions acquire the binding's name at their
                // declaration. Moving one to a property/value position can
                // infer a different name (or no name), which is observable
                // through Function#name. A called arrow cannot observe its
                // own function object, but an ordinary anonymous function can
                // do so through `arguments.callee`, so keep the latter bound.
                let procedure_iife = called && function.block_open.is_some();
                let nested =
                    use_is_in_nested_function(&tokens, &matching_close, scope_start, use_at);
                let capture_changes = nested
                    && function_literal_move_changes_capture(
                        &tokens,
                        &matching_close,
                        &matching_open,
                        literal_start,
                        function,
                        scope_start,
                        use_at,
                    );
                let allow = if member_or_new {
                    false
                } else if called {
                    (function.is_arrow || procedure_iife && function.named) && !capture_changes
                } else {
                    function.named && !nested
                };
                if allow {
                    let (from, to) = assignment_span_to_remove(&tokens, name_at, function.end);
                    if !replacement_overlaps(
                        &replacements,
                        tokens[use_at].start,
                        tokens[use_at].end,
                    ) && !replacement_overlaps(&replacements, from, to)
                    {
                        let literal =
                            &source[tokens[literal_start].start..tokens[function.end].end];
                        let rendered = if expression_arrow
                            || procedure_iife
                            || rematerialized_literal_needs_grouping(
                                &tokens,
                                use_at,
                                tokens[literal_start].kind,
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
