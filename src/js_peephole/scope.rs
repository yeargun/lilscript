use crate::js_peephole::rewrite::{identifier_occurs, is_property_identifier, top_level_stop};
use crate::js_peephole::token::{matching_openers, Token, TokenKind};

pub(crate) fn nested_function_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scan: usize,
) -> Option<usize> {
    if tokens[scan].text == "function" {
        let mut index = scan + 1;
        if tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            index += 1;
        }
        if tokens.get(index).map(|token| token.text) != Some("(") {
            return None;
        }
        let close_paren = matching_close.get(index).copied().flatten()?;
        if tokens.get(close_paren + 1).map(|token| token.text) != Some("{") {
            return None;
        }
        return matching_close.get(close_paren + 1).copied().flatten();
    }
    if tokens[scan].text == "=>" && tokens.get(scan + 1).map(|token| token.text) == Some("{") {
        return matching_close.get(scan + 1).copied().flatten();
    }
    if tokens[scan].kind == TokenKind::Identifier
        && tokens.get(scan + 1).map(|token| token.text) == Some("(")
    {
        let close_paren = matching_close.get(scan + 1).copied().flatten()?;
        if tokens.get(close_paren + 1).map(|token| token.text) == Some("{") {
            return matching_close.get(close_paren + 1).copied().flatten();
        }
    }
    None
}

/// An arrow's parameter list precedes the `=>` token, so scans that skip
/// arrow bodies at `=>` still walk the parameters as ordinary identifiers.
/// A parameter is a fresh binding, never a read of an outer name.
pub(crate) fn identifier_is_arrow_parameter(tokens: &[Token<'_>], at: usize) -> bool {
    if tokens.get(at + 1).map(|token| token.text) == Some("=>") {
        return true;
    }
    let mut depth = 0i32;
    for index in at + 1..tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" if depth == 0 => {
                return tokens.get(index + 1).map(|token| token.text) == Some("=>");
            }
            ")" | "]" | "}" => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            ";" if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct FunctionExpression {
    pub end: usize,
    pub params_from: usize,
    pub params_to: usize,
    pub block_open: Option<usize>,
    pub is_arrow: bool,
    pub named: bool,
}

pub(crate) fn parse_function_expression(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    rhs: usize,
) -> Option<FunctionExpression> {
    if tokens.get(rhs).map(|token| token.text) == Some("function") {
        let mut index = rhs + 1;
        let named = tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Identifier);
        if named {
            index += 1;
        }
        if tokens.get(index).map(|token| token.text) != Some("(") {
            return None;
        }
        let close_paren = matching_close.get(index).copied().flatten()?;
        if tokens.get(close_paren + 1).map(|token| token.text) != Some("{") {
            return None;
        }
        let block_open = close_paren + 1;
        let end = matching_close.get(block_open).copied().flatten()?;
        return Some(FunctionExpression {
            end,
            params_from: index + 1,
            params_to: close_paren,
            block_open: Some(block_open),
            is_arrow: false,
            named,
        });
    }
    let (params_from, params_to, after_params) =
        if tokens.get(rhs).map(|token| token.text) == Some("(") {
            let close = matching_close.get(rhs).copied().flatten()?;
            (rhs + 1, close, close + 1)
        } else if tokens
            .get(rhs)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            (rhs, rhs + 1, rhs + 1)
        } else {
            return None;
        };
    if tokens.get(after_params).map(|token| token.text) != Some("=>") {
        return None;
    }
    let body = after_params + 1;
    if tokens.get(body).map(|token| token.text) == Some("{") {
        let end = matching_close.get(body).copied().flatten()?;
        return Some(FunctionExpression {
            end,
            params_from,
            params_to,
            block_open: Some(body),
            is_arrow: true,
            named: false,
        });
    }
    let stop = top_level_stop(tokens, body, &[",", ";", ")", "]", "}"]).unwrap_or(tokens.len());
    if stop <= body {
        return None;
    }
    Some(FunctionExpression {
        end: stop - 1,
        params_from,
        params_to,
        block_open: None,
        is_arrow: true,
        named: false,
    })
}

pub(crate) fn simple_identifier_params(tokens: &[Token<'_>], from: usize, to: usize) -> bool {
    if from == to {
        return true;
    }
    let mut expect_name = true;
    for token in &tokens[from..to] {
        if expect_name {
            if token.kind != TokenKind::Identifier {
                return false;
            }
            expect_name = false;
        } else if token.text == "," {
            expect_name = true;
        } else {
            return false;
        }
    }
    !expect_name
}

pub(crate) fn function_binds_name(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    body: usize,
    end: usize,
    name: &str,
) -> bool {
    if function_scope_declares(tokens, matching_open, body, end, name) {
        return true;
    }
    let mut cursor = body + 1;
    while cursor < end {
        if let Some(close) = nested_function_end(tokens, matching_close, cursor) {
            cursor = close + 1;
            continue;
        }
        if !matches!(tokens[cursor].text, "let" | "const") {
            cursor += 1;
            continue;
        }
        cursor += 1;
        let mut delimiter_depth = 0usize;
        let mut expects_name = true;
        while cursor < end {
            let token = tokens[cursor];
            if delimiter_depth == 0 && token.text == ";" {
                break;
            }
            if expects_name && token.kind == TokenKind::Identifier {
                if token.text == name {
                    return true;
                }
                expects_name = false;
            }
            match token.text {
                ")" | "]" | "}" if delimiter_depth == 0 => break,
                "(" | "[" | "{" => delimiter_depth += 1,
                ")" | "]" | "}" => delimiter_depth -= 1,
                "," if delimiter_depth == 0 => expects_name = true,
                _ => {}
            }
            cursor += 1;
        }
    }
    false
}

pub(crate) fn enclosing_block_end(matching_close: &[Option<usize>], at: usize) -> Option<usize> {
    matching_close
        .iter()
        .enumerate()
        .filter_map(|(open, close)| {
            let close = (*close)?;
            (open < at && at < close).then_some(close)
        })
        .min()
}

pub(crate) fn enclosing_block_start(matching_close: &[Option<usize>], at: usize) -> Option<usize> {
    matching_close
        .iter()
        .enumerate()
        .filter_map(|(open, close)| {
            let close = (*close)?;
            (open < at && at < close).then_some(open)
        })
        .max()
}

/// `var` declarations hoist to the enclosing function (or module), and the
/// emitter's name pool reuses hoisted bindings far outside the declaring
/// block, so the use scan must cover the whole function span: a declarator
/// with no reads in its own block may still be the sole declaration for a
/// name assigned and read elsewhere.
pub(crate) fn name_is_used_in_scope(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name_at: usize,
    after: usize,
    name: &str,
) -> bool {
    let (scope_start, scope_end) = enclosing_function_span(tokens, matching_close, name_at)
        .map(|(body, end)| (body + 1, end))
        .unwrap_or((0, tokens.len()));
    identifier_occurs(tokens, scope_start, name_at, name)
        || identifier_occurs(tokens, after, scope_end, name)
}

pub(crate) fn name_use_is_mutated(tokens: &[Token<'_>], use_at: usize) -> bool {
    matches!(
        tokens.get(use_at + 1).map(|token| token.text),
        Some("=") | Some("++") | Some("--") | Some("+=") | Some("-=")
    ) && tokens.get(use_at + 2).map(|token| token.text) != Some("=")
        || matches!(
            use_at.checked_sub(1).map(|index| tokens[index].text),
            Some("++") | Some("--") | Some("new")
        )
}

fn nested_function_binds_name(
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
    if tokens.get(body).map(|token| token.text) != Some("{") {
        return false;
    }
    function_binds_name(tokens, matching_close, matching_open, body, close, name)
}

fn catch_block_binds_name(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    body: usize,
    name: &str,
) -> bool {
    let Some(close_paren) = body.checked_sub(1) else {
        return false;
    };
    if tokens[close_paren].text != ")" {
        return false;
    }
    let Some(open_paren) = matching_open.get(close_paren).copied().flatten() else {
        return false;
    };
    if open_paren
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.text)
        != Some("catch")
    {
        return false;
    }
    tokens[open_paren + 1..close_paren]
        .iter()
        .any(|token| token.kind == TokenKind::Identifier && token.text == name)
}

pub(crate) fn name_is_declared_in_enclosing_catch(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    let matching_open = matching_openers(matching_close);
    let mut cursor = enclosing_block_start(matching_close, at);
    while let Some(body) = cursor {
        if catch_block_binds_name(tokens, &matching_open, body, name) {
            return true;
        }
        cursor = enclosing_block_start(matching_close, body);
    }
    false
}

pub(crate) fn name_is_declared_in_enclosing_expression_arrow(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    let matching_open = matching_openers(matching_close);
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "=>" || tokens.get(index + 1).map(|next| next.text) == Some("{") {
            continue;
        }
        let body_end =
            top_level_stop(tokens, index + 1, &[",", ";", ")", "]", "}"]).unwrap_or(tokens.len());
        if at > index
            && at < body_end
            && expression_arrow_binds_name(tokens, &matching_open, index, name)
        {
            return true;
        }
    }
    false
}

pub(crate) fn identifier_is_function_parameter(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> bool {
    for (open, close) in matching_close.iter().enumerate() {
        let Some(close) = *close else {
            continue;
        };
        if tokens[open].text != "(" || at <= open || at >= close {
            continue;
        }
        if !matches!(
            tokens.get(close + 1).map(|token| token.text),
            Some("{") | Some("=>")
        ) || paren_close_is_control_header(tokens, close)
        {
            continue;
        }
        return true;
    }
    false
}

pub(crate) fn identifier_is_catch_parameter(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> bool {
    for (open, close) in matching_close.iter().enumerate() {
        let Some(close) = *close else {
            continue;
        };
        if tokens[open].text != "(" || at <= open || at >= close {
            continue;
        }
        if open
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .map(|token| token.text)
            == Some("catch")
        {
            return true;
        }
    }
    false
}

pub(crate) fn name_is_bound_as_non_enclosing_function_local(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    let matching_open = matching_openers(matching_close);
    for open in 0..tokens.len() {
        if tokens[open].text != "{" {
            continue;
        }
        let Some(before) = open.checked_sub(1) else {
            continue;
        };
        let is_function = tokens[before].text == "=>"
            || (tokens[before].text == ")" && !paren_close_is_control_header(tokens, before));
        if !is_function {
            continue;
        }
        let Some(end) = matching_close.get(open).copied().flatten() else {
            continue;
        };
        if at > open && at < end {
            continue;
        }
        if function_binds_name(tokens, matching_close, &matching_open, open, end, name) {
            return true;
        }
    }
    false
}

fn expression_arrow_binds_name(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    arrow: usize,
    name: &str,
) -> bool {
    let Some(before_arrow) = arrow.checked_sub(1) else {
        return false;
    };
    let range = if tokens[before_arrow].text == ")" {
        matching_open
            .get(before_arrow)
            .copied()
            .flatten()
            .map(|open| (open + 1, before_arrow))
    } else if tokens[before_arrow].kind == TokenKind::Identifier {
        Some((before_arrow, before_arrow + 1))
    } else {
        None
    };
    range.is_some_and(|(start, finish)| {
        tokens[start..finish]
            .iter()
            .any(|token| token.kind == TokenKind::Identifier && token.text == name)
    })
}

pub(crate) fn collect_same_scope_name_uses(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name: &str,
    start: usize,
    end: usize,
    skip: usize,
) -> (Vec<usize>, bool) {
    let matching_open = matching_openers(matching_close);
    let mut uses = Vec::new();
    let mut nested_use = false;
    let mut scan = start;
    while scan < end {
        if scan != skip {
            if let Some(close) = nested_function_end(tokens, matching_close, scan) {
                if !nested_function_binds_name(
                    tokens,
                    matching_close,
                    &matching_open,
                    scan,
                    close,
                    name,
                ) && identifier_occurs(tokens, scan, close + 1, name)
                {
                    nested_use = true;
                }
                scan = close + 1;
                continue;
            }
            if tokens[scan].text == "=>"
                && tokens.get(scan + 1).map(|token| token.text) != Some("{")
            {
                let body_end =
                    top_level_stop(tokens, scan + 1, &[",", ";", ")", "]", "}"]).unwrap_or(end);
                if !expression_arrow_binds_name(tokens, &matching_open, scan, name)
                    && identifier_occurs(tokens, scan + 1, body_end, name)
                {
                    nested_use = true;
                }
                scan = body_end;
                continue;
            }
        }
        if scan != skip
            && tokens[scan].kind == TokenKind::Identifier
            && tokens[scan].text == name
            && !is_property_identifier(tokens, scan)
        {
            uses.push(scan);
        }
        scan += 1;
    }
    (uses, nested_use)
}

pub(crate) fn collect_unbound_name_uses(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name: &str,
    start: usize,
    end: usize,
    skip: usize,
) -> Vec<usize> {
    let matching_open = matching_openers(matching_close);
    let mut uses = Vec::new();
    let mut scan = start;
    while scan < end {
        if scan != skip {
            if let Some(close) = nested_function_end(tokens, matching_close, scan) {
                if nested_function_binds_name(
                    tokens,
                    matching_close,
                    &matching_open,
                    scan,
                    close,
                    name,
                ) {
                    scan = close + 1;
                    continue;
                }
            }
            if tokens[scan].text == "=>"
                && tokens.get(scan + 1).map(|token| token.text) != Some("{")
                && expression_arrow_binds_name(tokens, &matching_open, scan, name)
            {
                scan = top_level_stop(tokens, scan + 1, &[",", ";", ")", "]", "}"]).unwrap_or(end);
                continue;
            }
        }
        if scan != skip
            && tokens[scan].kind == TokenKind::Identifier
            && tokens[scan].text == name
            && !is_property_identifier(tokens, scan)
        {
            uses.push(scan);
        }
        scan += 1;
    }
    uses
}

pub(crate) fn use_is_in_nested_function(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    use_at: usize,
) -> bool {
    let mut scan = from;
    while scan < use_at {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            if use_at <= close {
                return true;
            }
            scan = close + 1;
            continue;
        }
        if tokens[scan].text == "=>" && tokens.get(scan + 1).map(|token| token.text) != Some("{") {
            let body_end =
                top_level_stop(tokens, scan + 1, &[",", ";", ")", "]", "}"]).unwrap_or(use_at + 1);
            if use_at < body_end {
                return true;
            }
            scan = body_end;
            continue;
        }
        scan += 1;
    }
    false
}

/// Whether moving an expression from `from` to `use_at` would place a free
/// reference named `name` underneath a function binding that did not enclose
/// its original position. This is the capture check required by literal
/// rematerialization: `let n=x=>h(S,x);return h=>n(h)` must not become
/// `return h=>(x=>h(S,x))(h)`.
pub(crate) fn name_is_bound_in_nested_function_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    use_at: usize,
    name: &str,
) -> bool {
    let matching_open = matching_openers(matching_close);
    let mut scan = from;
    while scan < use_at {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            if use_at <= close {
                if nested_function_binds_name(
                    tokens,
                    matching_close,
                    &matching_open,
                    scan,
                    close,
                    name,
                ) {
                    return true;
                }
                // The use can be inside a still deeper function.
                scan += 1;
                continue;
            }
            scan = close + 1;
            continue;
        }
        if tokens[scan].text == "=>" && tokens.get(scan + 1).map(|token| token.text) != Some("{") {
            let body_end =
                top_level_stop(tokens, scan + 1, &[",", ";", ")", "]", "}"]).unwrap_or(use_at + 1);
            if use_at < body_end {
                if expression_arrow_binds_name(tokens, &matching_open, scan, name) {
                    return true;
                }
                scan += 1;
                continue;
            }
            scan = body_end;
            continue;
        }
        scan += 1;
    }
    false
}

pub(crate) fn name_is_declared_in_any_enclosing_scope(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    name_is_declared_in_any_enclosing_function_scope(tokens, matching_close, at, name)
        || name_is_declared_at_module_scope(tokens, matching_close, name)
}

pub(crate) fn name_is_declared_in_any_enclosing_function_scope(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    if name_is_declared_in_visible_scope(tokens, matching_close, at, name) {
        return true;
    }
    let matching_open = matching_openers(matching_close);
    let mut cursor = enclosing_function_span(tokens, matching_close, at).map(|(body, _)| body);
    while let Some(body) = cursor {
        let Some(outer) = enclosing_function_span(tokens, matching_close, body) else {
            break;
        };
        if function_binds_name(
            tokens,
            matching_close,
            &matching_open,
            outer.0,
            outer.1,
            name,
        ) {
            return true;
        }
        if outer.0 >= body {
            break;
        }
        cursor = Some(outer.0);
    }
    false
}

pub(crate) fn name_is_visible_generated_binding(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    if name_is_declared_in_enclosing_expression_arrow(tokens, matching_close, at, name)
        || name_is_declared_in_enclosing_catch(tokens, matching_close, at, name)
        || name_is_declared_at_module_scope(tokens, matching_close, name)
    {
        return true;
    }
    let matching_open = matching_openers(matching_close);
    let Some((body, end)) = enclosing_function_span(tokens, matching_close, at) else {
        return false;
    };
    if function_directly_binds_name(tokens, matching_close, &matching_open, body, end, name) {
        return true;
    }
    let mut cursor = Some(body);
    while let Some(body) = cursor {
        let Some(outer) = enclosing_function_span(tokens, matching_close, body) else {
            break;
        };
        if function_directly_binds_name(
            tokens,
            matching_close,
            &matching_open,
            outer.0,
            outer.1,
            name,
        ) {
            return true;
        }
        if outer.0 >= body {
            break;
        }
        cursor = Some(outer.0);
    }
    false
}

fn function_directly_binds_name(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    body: usize,
    end: usize,
    name: &str,
) -> bool {
    let parameter_range = body.checked_sub(1).and_then(|before_body| {
        if tokens[before_body].text == "=>" {
            let before_arrow = before_body.checked_sub(1)?;
            if tokens[before_arrow].text == ")" {
                matching_open[before_arrow].map(|open| (open + 1, before_arrow))
            } else {
                Some((before_arrow, before_arrow + 1))
            }
        } else if tokens[before_body].text == ")" {
            matching_open[before_body].map(|open| (open + 1, before_body))
        } else {
            None
        }
    });
    if parameter_range.is_some_and(|(start, finish)| {
        tokens[start..finish]
            .iter()
            .any(|token| token.kind == TokenKind::Identifier && token.text == name)
    }) {
        return true;
    }
    let mut cursor = body + 1;
    while cursor < end {
        if tokens[cursor].text == "function" {
            let mut index = cursor + 1;
            if tokens.get(index).map(|token| token.text) == Some("*") {
                index += 1;
            }
            if tokens
                .get(index)
                .is_some_and(|token| token.kind == TokenKind::Identifier && token.text == name)
            {
                return true;
            }
        }
        if let Some(close) = nested_function_end(tokens, matching_close, cursor) {
            cursor = close + 1;
            continue;
        }
        if !matches!(tokens[cursor].text, "var" | "let" | "const") {
            cursor += 1;
            continue;
        }
        cursor += 1;
        let mut delimiter_depth = 0usize;
        let mut expects_name = true;
        while cursor < end {
            let token = tokens[cursor];
            if delimiter_depth == 0 && token.text == ";" {
                break;
            }
            if let Some(close) = nested_function_end(tokens, matching_close, cursor) {
                cursor = close + 1;
                expects_name = false;
                continue;
            }
            if expects_name && token.kind == TokenKind::Identifier {
                if token.text == name {
                    return true;
                }
                expects_name = false;
            }
            match token.text {
                ")" | "]" | "}" if delimiter_depth == 0 => break,
                "(" | "[" | "{" => delimiter_depth += 1,
                ")" | "]" | "}" => delimiter_depth -= 1,
                "," if delimiter_depth == 0 => expects_name = true,
                _ => {}
            }
            cursor += 1;
        }
    }
    false
}

fn name_is_declared_at_module_scope(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name: &str,
) -> bool {
    let mut scan = 0usize;
    while scan < tokens.len() {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            if tokens[scan].text == "function" {
                let mut index = scan + 1;
                if tokens.get(index).map(|token| token.text) == Some("*") {
                    index += 1;
                }
                if tokens
                    .get(index)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && tokens[index].text == name
                {
                    return true;
                }
            }
            scan = close + 1;
            continue;
        }
        if tokens[scan].text == "class" {
            let mut body = scan + 1;
            if tokens
                .get(body)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                if tokens[body].text == name {
                    return true;
                }
                body += 1;
            }
            if tokens.get(body).map(|token| token.text) == Some("extends") {
                body += 2;
            }
            if tokens.get(body).map(|token| token.text) == Some("{") {
                if let Some(close) = matching_close.get(body).copied().flatten() {
                    scan = close + 1;
                    continue;
                }
            }
        }
        if matches!(tokens[scan].text, "let" | "var" | "const") {
            if let Some(end) = module_var_declaration_end(tokens, matching_close, scan, name) {
                if end.1 {
                    return true;
                }
                scan = end.0;
                continue;
            }
        }
        scan += 1;
    }
    false
}

pub(crate) fn name_is_module_var_binding(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name: &str,
) -> bool {
    let mut scan = 0usize;
    while scan < tokens.len() {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            scan = close + 1;
            continue;
        }
        if tokens[scan].text == "class" {
            let mut body = scan + 1;
            if tokens
                .get(body)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                body += 1;
            }
            if tokens.get(body).map(|token| token.text) == Some("extends") {
                body += 2;
            }
            if tokens.get(body).map(|token| token.text) == Some("{") {
                if let Some(close) = matching_close.get(body).copied().flatten() {
                    scan = close + 1;
                    continue;
                }
            }
        }
        if matches!(tokens[scan].text, "let" | "var" | "const") {
            if let Some(end) = module_var_declaration_end(tokens, matching_close, scan, name) {
                if end.1 {
                    return true;
                }
                scan = end.0;
                continue;
            }
        }
        scan += 1;
    }
    false
}

fn module_var_declaration_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scan: usize,
    name: &str,
) -> Option<(usize, bool)> {
    let mut index = scan + 1;
    let mut delimiter_depth = 0usize;
    let mut expects_name = true;
    let mut found = false;
    while index < tokens.len() {
        let token = tokens[index];
        if delimiter_depth == 0 && token.text == ";" {
            return Some((index, found));
        }
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            index = close + 1;
            expects_name = false;
            continue;
        }
        if expects_name && token.kind == TokenKind::Identifier {
            if token.text == name {
                found = true;
            }
            expects_name = false;
        }
        match token.text {
            ")" | "]" | "}" if delimiter_depth == 0 => return Some((index, found)),
            "(" | "[" | "{" => delimiter_depth += 1,
            ")" | "]" | "}" => delimiter_depth -= 1,
            "," if delimiter_depth == 0 => expects_name = true,
            _ => {}
        }
        index += 1;
    }
    Some((index, found))
}

pub(crate) fn name_is_declared_in_visible_scope(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> bool {
    let matching_open = matching_openers(matching_close);
    if let Some((body, end)) = enclosing_function_span(tokens, matching_close, at) {
        return function_binds_name(tokens, matching_close, &matching_open, body, end, name);
    }
    let mut scan = 0usize;
    while scan < at {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            if tokens[scan].text == "function" {
                let mut index = scan + 1;
                if tokens.get(index).map(|token| token.text) == Some("*") {
                    index += 1;
                }
                if tokens
                    .get(index)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && tokens[index].text == name
                {
                    return true;
                }
            }
            scan = close + 1;
            continue;
        }
        if tokens[scan].text == "class" {
            let mut body = scan + 1;
            if tokens
                .get(body)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                if tokens[body].text == name {
                    return true;
                }
                body += 1;
            }
            if tokens.get(body).map(|token| token.text) == Some("extends") {
                body += 2;
            }
            if tokens.get(body).map(|token| token.text) == Some("{") {
                if let Some(close) = matching_close.get(body).copied().flatten() {
                    scan = close + 1;
                    continue;
                }
            }
        }
        if matches!(tokens[scan].text, "let" | "var" | "const") {
            let index = scan + 1;
            if tokens
                .get(index)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens[index].text == name
            {
                return true;
            }
        }
        scan += 1;
    }
    false
}

pub(crate) fn skip_nested_loop_or_function(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scan: usize,
) -> Option<usize> {
    if let Some(close) = nested_function_end(tokens, matching_close, scan) {
        return Some(close);
    }
    if matches!(tokens[scan].text, "for" | "while")
        && tokens.get(scan + 1).map(|token| token.text) == Some("(")
    {
        let header_close = matching_close.get(scan + 1).copied().flatten()?;
        let after = header_close + 1;
        if tokens.get(after).map(|token| token.text) == Some("{") {
            return matching_close.get(after).copied().flatten();
        }
        return top_level_stop(tokens, after, &[";"]);
    }
    if tokens[scan].text == "do" && tokens.get(scan + 1).map(|token| token.text) == Some("{") {
        return matching_close.get(scan + 1).copied().flatten();
    }
    None
}

pub(crate) fn outermost_function_body_start(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> usize {
    let mut start = 0usize;
    let mut open = enclosing_block_start(matching_close, at);
    while let Some(block) = open {
        if block
            .checked_sub(1)
            .is_some_and(|before| matches!(tokens[before].text, "=>" | ")"))
        {
            start = block + 1;
        }
        open = enclosing_block_start(matching_close, block);
    }
    start
}

pub(crate) fn enclosing_function_range(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<(usize, usize)> {
    let mut open = enclosing_block_start(matching_close, at);
    while let Some(block) = open {
        if block
            .checked_sub(1)
            .is_some_and(|before| matches!(tokens[before].text, "=>" | ")"))
        {
            let close = matching_close.get(block).copied().flatten()?;
            return Some((block + 1, close));
        }
        open = enclosing_block_start(matching_close, block);
    }
    None
}

pub(crate) fn enclosing_function_span(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<(usize, usize)> {
    let mut open = enclosing_block_start(matching_close, at)?;
    loop {
        let close = matching_close.get(open).copied().flatten()?;
        if let Some(before) = open.checked_sub(1) {
            if tokens[before].text == "=>" {
                return Some((open, close));
            }
            // `){` opens a function body only when the parenthesis is a
            // parameter list: a control header (`while(…){`, `if(…){`, …)
            // has the same shape but its block is not a scope for `var`.
            if tokens[before].text == ")" && !paren_close_is_control_header(tokens, before) {
                return Some((open, close));
            }
        }
        open = enclosing_block_start(matching_close, open)?;
    }
}

fn paren_close_is_control_header(tokens: &[Token<'_>], close: usize) -> bool {
    let mut depth = 0i32;
    let mut index = close;
    loop {
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                depth -= 1;
                if depth == 0 {
                    return index.checked_sub(1).is_some_and(|before| {
                        matches!(
                            tokens[before].text,
                            "while" | "for" | "if" | "switch" | "catch" | "with"
                        )
                    });
                }
            }
            _ => {}
        }
        let Some(previous) = index.checked_sub(1) else {
            return false;
        };
        index = previous;
    }
}

pub(crate) fn function_scope_declares(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    body: usize,
    end: usize,
    name: &str,
) -> bool {
    let parameter_range = body.checked_sub(1).and_then(|before_body| {
        if tokens[before_body].text == "=>" {
            let before_arrow = before_body.checked_sub(1)?;
            if tokens[before_arrow].text == ")" {
                matching_open[before_arrow].map(|open| (open + 1, before_arrow))
            } else {
                Some((before_arrow, before_arrow + 1))
            }
        } else if tokens[before_body].text == ")" {
            matching_open[before_body].map(|open| (open + 1, before_body))
        } else {
            None
        }
    });
    if parameter_range.is_some_and(|(start, finish)| {
        tokens[start..finish]
            .iter()
            .any(|token| token.kind == TokenKind::Identifier && token.text == name)
    }) {
        return true;
    }

    let mut cursor = body + 1;
    while cursor < end {
        if tokens[cursor].text != "var" {
            cursor += 1;
            continue;
        }
        cursor += 1;
        let mut delimiter_depth = 0usize;
        let mut expects_name = true;
        while cursor < end {
            let token = tokens[cursor];
            if delimiter_depth == 0 && token.text == ";" {
                break;
            }
            if expects_name && token.kind == TokenKind::Identifier {
                if token.text == name {
                    return true;
                }
                expects_name = false;
            }
            match token.text {
                ")" | "]" | "}" if delimiter_depth == 0 => break,
                "(" | "[" | "{" => delimiter_depth += 1,
                ")" | "]" | "}" => delimiter_depth -= 1,
                "," if delimiter_depth == 0 => expects_name = true,
                _ => {}
            }
            cursor += 1;
        }
    }
    false
}

pub(crate) fn name_is_arguments_length_copy(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    before: usize,
    name: &str,
) -> bool {
    let mut proven = std::collections::HashSet::<&str>::new();
    let mut index = outermost_function_body_start(tokens, matching_close, before);
    while index < before {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if before <= close {
                index += 1;
            } else {
                index = close + 1;
            }
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("arguments")
            && tokens.get(index + 3).map(|token| token.text) == Some(".")
            && tokens.get(index + 4).map(|token| token.text) == Some("length")
        {
            proven.insert(tokens[index].text);
            index += 5;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens
                .get(index + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && matches!(
                tokens.get(index + 3).map(|token| token.text),
                Some(",") | Some(";") | Some(")") | None
            )
        {
            let dest = tokens[index].text;
            let src = tokens[index + 2].text;
            if proven.contains(src) {
                proven.insert(dest);
            } else {
                proven.remove(dest);
            }
            index += 3;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && matches!(
                tokens.get(index + 1).map(|token| token.text),
                Some("=") | Some("++") | Some("--") | Some("+=") | Some("-=")
            )
        {
            proven.remove(name);
        }
        if tokens[index].text == "--" && tokens.get(index + 1).map(|token| token.text) == Some(name)
        {
            proven.remove(name);
        }
        index += 1;
    }
    proven.contains(name)
}

pub(crate) fn name_is_array_length_copy(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    before: usize,
    name: &str,
) -> bool {
    let mut arrays = std::collections::HashSet::<&str>::new();
    let mut proven = std::collections::HashSet::<&str>::new();
    let mut index = outermost_function_body_start(tokens, matching_close, before);
    let mut scan = index;
    while scan + 3 < before {
        if let Some(close) = nested_function_end(tokens, matching_close, scan) {
            scan = if before <= close { scan + 1 } else { close + 1 };
            continue;
        }
        if tokens[scan].kind == TokenKind::Identifier
            && tokens.get(scan + 1).map(|token| token.text) == Some(".")
            && matches!(
                tokens.get(scan + 2).map(|token| token.text),
                Some("push" | "pop" | "shift" | "unshift" | "splice" | "sort")
            )
            && tokens.get(scan + 3).map(|token| token.text) == Some("(")
        {
            arrays.insert(tokens[scan].text);
        }
        scan += 1;
    }
    while index < before {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if before <= close {
                index += 1;
            } else {
                index = close + 1;
            }
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("[")
        {
            if let Some(close) = matching_close.get(index + 2).copied().flatten() {
                arrays.insert(tokens[index].text);
                proven.remove(tokens[index].text);
                index = close + 1;
                continue;
            }
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens
                .get(index + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 3).map(|token| token.text) == Some(".")
            && tokens.get(index + 4).map(|token| token.text) == Some("length")
        {
            let dest = tokens[index].text;
            let src = tokens[index + 2].text;
            arrays.remove(dest);
            if arrays.contains(src) {
                proven.insert(dest);
            } else {
                proven.remove(dest);
            }
            index += 5;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens
                .get(index + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && matches!(
                tokens.get(index + 3).map(|token| token.text),
                Some(",") | Some(";") | Some(")") | None
            )
        {
            let dest = tokens[index].text;
            let src = tokens[index + 2].text;
            if proven.contains(src) {
                proven.insert(dest);
            } else {
                proven.remove(dest);
            }
            if arrays.contains(src) {
                arrays.insert(dest);
            } else {
                arrays.remove(dest);
            }
            index += 3;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && matches!(
                tokens.get(index + 1).map(|token| token.text),
                Some("=") | Some("++") | Some("--") | Some("+=") | Some("-=")
            )
        {
            proven.remove(name);
        }
        if tokens[index].text == "--" && tokens.get(index + 1).map(|token| token.text) == Some(name)
        {
            proven.remove(name);
        }
        index += 1;
    }
    proven.contains(name)
}

pub(crate) fn name_is_nonnegative_length_copy(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    before: usize,
    name: &str,
) -> bool {
    name_is_arguments_length_copy(tokens, matching_close, before, name)
        || name_is_array_length_copy(tokens, matching_close, before, name)
}
