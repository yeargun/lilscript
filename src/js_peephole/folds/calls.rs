use crate::js_peephole::rewrite::{
    apply_token_rewrites, identifier_occurs, is_property_identifier, next_statement_end,
    top_level_stop,
};
use crate::js_peephole::scope::{
    enclosing_function_span, function_binds_name, name_is_declared_in_any_enclosing_function_scope,
    parse_function_expression, simple_identifier_params,
};
use crate::js_peephole::token::{
    lex, matching_closers, matching_openers, Token, TokenKind,
};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn fold_comma_assign_into_trailing_call_arg(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].text != "("
            || tokens
                .get(cursor + 1)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 2).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor + 1].text;
        let Some(paren_close) = matching_close.get(cursor).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let Some(comma) = top_level_stop(&tokens, cursor + 3, &[","]) else {
            cursor += 1;
            continue;
        };
        if comma >= paren_close {
            cursor += 1;
            continue;
        }
        let rhs = &source[tokens[cursor + 3].start..tokens[comma].start];
        if rhs.is_empty() || identifier_occurs(&tokens, cursor + 3, comma, name) {
            cursor += 1;
            continue;
        }
        if tokens.get(paren_close - 1).map(|token| token.text) != Some(")")
            || tokens.get(paren_close - 2).map(|token| token.text) != Some(name)
            || !matches!(
                tokens.get(paren_close - 3).map(|token| token.text),
                Some(",") | Some("(")
            )
        {
            cursor += 1;
            continue;
        }
        if identifier_occurs(&tokens, comma + 1, paren_close - 2, name) {
            cursor += 1;
            continue;
        }
        // Dropping the assignment is only sound when nothing reads the name
        // after this group.
        let scope_end = enclosing_function_span(&tokens, &matching_close, cursor)
            .map(|(_, close)| close)
            .unwrap_or(tokens.len());
        if identifier_occurs(&tokens, paren_close + 1, scope_end, name) {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[paren_close].end,
            format!(
                "{}{rhs})",
                &source[tokens[comma + 1].start..tokens[paren_close - 2].start]
            ),
        ));
        cursor = paren_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_omissible_trailing_false_args(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut slice_last_params = std::collections::HashMap::<&str, ()>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens.get(cursor + 2).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let Some(param_close) = matching_close.get(cursor + 2).copied().flatten() else {
            cursor += 1;
            continue;
        };
        if tokens.get(param_close + 1).map(|token| token.text) != Some("=>")
            || tokens.get(param_close + 2).map(|token| token.text) != Some("{")
        {
            cursor += 1;
            continue;
        }
        let Some(body_close) = matching_close.get(param_close + 2).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let mut params = Vec::new();
        let mut index = cursor + 3;
        while index < param_close {
            if tokens[index].kind == TokenKind::Identifier {
                params.push(tokens[index].text);
            }
            index += 1;
        }
        let Some(&last) = params.last() else {
            cursor += 1;
            continue;
        };
        if last_param_is_only_slice_start(
            &tokens,
            cursor + 3,
            param_close,
            param_close + 3,
            body_close,
            last,
        ) {
            slice_last_params.insert(tokens[cursor].text, ());
        }
        cursor = body_close + 1;
    }
    if slice_last_params.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || !slice_last_params.contains_key(tokens[cursor].text)
            || tokens.get(cursor + 1).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let Some(call_close) = matching_close.get(cursor + 1).copied().flatten() else {
            cursor += 1;
            continue;
        };
        if tokens.get(call_close - 1).map(|token| token.text) == Some("1")
            && tokens.get(call_close - 2).map(|token| token.text) == Some("!")
            && tokens.get(call_close - 3).map(|token| token.text) == Some(",")
        {
            replacements.push((
                tokens[call_close - 3].start,
                tokens[call_close - 1].end,
                String::new(),
            ));
        }
        cursor = call_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn last_param_is_only_slice_start(
    tokens: &[Token<'_>],
    param_start: usize,
    param_end: usize,
    body_start: usize,
    body_end: usize,
    name: &str,
) -> bool {
    if identifier_occurs(tokens, param_start, param_end, name)
        && tokens[param_start..param_end]
            .iter()
            .filter(|token| token.kind == TokenKind::Identifier && token.text == name)
            .count()
            != 1
    {
        return false;
    }
    let mut uses = 0usize;
    let mut index = body_start;
    while index < body_end {
        if tokens[index].kind == TokenKind::Identifier && tokens[index].text == name {
            uses += 1;
            if index < 3
                || tokens.get(index - 1).map(|token| token.text) != Some("(")
                || tokens.get(index - 2).map(|token| token.text) != Some("slice")
                || tokens.get(index - 3).map(|token| token.text) != Some(".")
                || tokens.get(index + 1).map(|token| token.text) != Some(")")
            {
                return false;
            }
        }
        index += 1;
    }
    uses > 0
}

pub(crate) fn fold_adjacent_expression_statements(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_adjacent_expression_statements_at(source, false)
}

/// Top-level sequencing runs once at the very end of the pipeline: joining
/// module-level statements earlier would hide statement-shaped patterns from
/// the other folds, and the comma spelling is byte-neutral raw while
/// compressing measurably better under Brotli and gzip.
pub(crate) fn fold_top_level_adjacent_expression_statements(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_adjacent_expression_statements_at(source, true)
}

fn fold_adjacent_expression_statements_at(
    source: &str,
    top_level: bool,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut for_header_depth = 0i32;
    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "(" | "[" => {
                depth_paren += 1;
                if token.text == "("
                    && index > 0
                    && tokens[index - 1].text == "for"
                    && for_header_depth == 0
                {
                    for_header_depth = depth_paren;
                }
            }
            ")" | "]" => {
                if token.text == ")" && depth_paren == for_header_depth {
                    for_header_depth = 0;
                }
                depth_paren -= 1;
            }
            "{" => depth_brace += 1,
            "}" => depth_brace -= 1,
            ";" if (if top_level {
                depth_brace == 0
            } else {
                depth_brace > 0
            }) && (for_header_depth == 0 || depth_paren < for_header_depth) =>
            {
                if matches!(
                    tokens.get(index + 1).map(|token| token.text),
                    Some("if")
                        | Some("else")
                        | Some("for")
                        | Some("while")
                        | Some("var")
                        | Some("let")
                        | Some("const")
                        | Some("function")
                        | Some("return")
                        | Some("throw")
                        | Some("try")
                        | Some("switch")
                        | Some("do")
                        | Some("with")
                        | Some("break")
                        | Some("continue")
                        | Some("class")
                        | Some("debugger")
                        | Some("import")
                        | Some("export")
                        | Some("case")
                        | Some("default")
                ) {
                    continue;
                }
                let left_start = previous_statement_start(&tokens, index);
                let right_end = next_statement_end(&tokens, index + 1);
                // A string literal opening the left statement may be a
                // directive prologue entry ("use strict"); sequencing it
                // would demote the directive to a plain expression.
                if tokens
                    .get(left_start)
                    .is_some_and(|token| token.kind == TokenKind::String)
                {
                    continue;
                }
                if is_expression_statement_span(&tokens, left_start, index)
                    && is_expression_statement_span(&tokens, index + 1, right_end)
                {
                    replacements.push((token.start, token.end, ",".to_string()));
                }
            }
            _ => {}
        }
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn previous_statement_start(tokens: &[Token<'_>], semi: usize) -> usize {
    let mut depth_paren = 0i32;
    let mut depth_brace = 0i32;
    let mut index = semi;
    while index > 0 {
        index -= 1;
        match tokens[index].text {
            ")" | "]" => depth_paren += 1,
            "(" | "[" => depth_paren -= 1,
            "}" => depth_brace += 1,
            "{" => {
                if depth_brace == 0 && depth_paren == 0 {
                    return index + 1;
                }
                depth_brace -= 1;
            }
            ";" if depth_paren == 0 && depth_brace == 0 => return index + 1,
            _ => {}
        }
    }
    0
}

/// Beta-reduce a generated arrow IIFE when every substituted argument is a
/// primary and every parameter is a simple identifier.
pub(crate) fn fold_identity_arrow_iife(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_return_only_iife(source, false)
}

/// Offer an unnamed, zero-argument classic return-only IIFE as an independent
/// late candidate. Keeping this out of the canonical pass lets each configured
/// whole-artifact objective decide whether removing the function boundary is a
/// compression win.
pub(crate) fn fold_zero_argument_return_iife(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_return_only_iife(source, true)
}

/// Move a private top-level function declaration to its only direct call.
///
/// The declaration and every non-property occurrence of its name prove that
/// function identity is unobservable. Free names are checked against every
/// enclosing function scope at the call site before the body moves, avoiding
/// the capture bug in a purely textual `reduce_funcs` rewrite. The anonymous
/// expression may be allocated on repeated calls, but the named function was
/// neither address-taken nor observable; configured whole-artifact scoring
/// decides whether that size/runtime trade is worthwhile.
pub(crate) fn fold_single_use_function_expressions(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    #[derive(Clone)]
    struct Candidate {
        declaration_start: usize,
        declaration_end: usize,
        call_start: usize,
        call_end: usize,
        replacement: String,
    }

    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let matching_open = matching_openers(&matching_close);
    let mut candidates = Vec::<Candidate>::new();

    for function_at in 0..tokens.len() {
        if tokens[function_at].text != "function"
            || enclosing_function_span(&tokens, &matching_close, function_at).is_some()
        {
            continue;
        }
        let mut name_at = function_at + 1;
        if tokens.get(name_at).map(|token| token.text) == Some("*") {
            // Generator calls have distinct suspension and allocation
            // behavior; leave them to the IR-level proof.
            continue;
        }
        let Some(name) = tokens
            .get(name_at)
            .filter(|token| token.kind == TokenKind::Identifier)
            .map(|token| token.text)
        else {
            continue;
        };
        name_at += 1;
        if tokens.get(name_at).map(|token| token.text) != Some("(") {
            continue;
        }
        let params_open = name_at;
        let Some(params_close) = matching_close.get(params_open).copied().flatten() else {
            continue;
        };
        if !simple_identifier_params(&tokens, params_open + 1, params_close) {
            continue;
        }
        let body_open = params_close + 1;
        if tokens.get(body_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(body_open).copied().flatten() else {
            continue;
        };
        if tokens[body_open + 1..body_close].iter().any(|token| {
            matches!(token.text, "function" | "=>" | "class" | "arguments" | "super")
                || token.kind == TokenKind::Template
        }) {
            continue;
        }

        let references = tokens
            .iter()
            .enumerate()
            .filter(|(index, token)| {
                token.kind == TokenKind::Identifier
                    && token.text == name
                    && tokens
                        .get(index.saturating_sub(1))
                        .is_none_or(|previous| previous.text != ".")
                    && tokens.get(index + 1).is_none_or(|next| next.text != ":")
            })
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        if references.len() != 2 || !references.contains(&(function_at + 1)) {
            continue;
        }
        let Some(call_at) = references
            .into_iter()
            .find(|reference| *reference != function_at + 1)
        else {
            continue;
        };
        if tokens.get(call_at + 1).map(|token| token.text) != Some("(")
            || call_at
                .checked_sub(1)
                .and_then(|index| tokens.get(index))
                .is_some_and(|token| matches!(token.text, "new" | "." | "?."))
        {
            continue;
        }
        let Some(call_close) = matching_close.get(call_at + 1).copied().flatten() else {
            continue;
        };

        let mut capture_safe = true;
        for (index, token) in tokens
            .iter()
            .enumerate()
            .take(body_close)
            .skip(body_open + 1)
        {
            if token.kind != TokenKind::Identifier
                || token.text == name
                || is_property_identifier(&tokens, index)
                || function_binds_name(
                    &tokens,
                    &matching_close,
                    &matching_open,
                    body_open,
                    body_close,
                    token.text,
                )
            {
                continue;
            }
            let previous = index
                .checked_sub(1)
                .and_then(|previous| tokens.get(previous))
                .map(|previous| previous.text);
            let next = tokens.get(index + 1).map(|next| next.text);
            if matches!(previous, Some("{") | Some(","))
                && matches!(next, Some("}") | Some(",") | Some("("))
                || name_is_declared_in_any_enclosing_function_scope(
                    &tokens,
                    &matching_close,
                    call_at,
                    token.text,
                )
            {
                capture_safe = false;
                break;
            }
        }
        if !capture_safe {
            continue;
        }

        let params = &source[tokens[params_open].start..tokens[params_close].end];
        let body = &source[tokens[body_open].start..tokens[body_close].end];
        let args = &source[tokens[call_at + 1].start..tokens[call_close].end];
        candidates.push(Candidate {
            declaration_start: tokens[function_at].start,
            declaration_end: tokens[body_close].end,
            call_start: tokens[call_at].start,
            call_end: tokens[call_close].end,
            replacement: format!("(function{params}{body}){args}"),
        });
    }

    // If one eligible helper is called from another eligible helper, deleting
    // both declarations would strand the inner call in the moved outer body.
    // Keep only independent sites in this round; a later cleanup round can
    // reconsider the remaining declaration against the already-moved code.
    let spans = candidates
        .iter()
        .map(|candidate| (candidate.declaration_start, candidate.declaration_end))
        .collect::<Vec<_>>();
    candidates.retain(|candidate| {
        spans.iter().all(|(start, end)| {
            (*start == candidate.declaration_start && *end == candidate.declaration_end)
                || candidate.call_start < *start
                || candidate.call_start >= *end
        })
    });
    let mut replacements = Vec::with_capacity(candidates.len() * 2);
    for candidate in candidates {
        replacements.push((
            candidate.declaration_start,
            candidate.declaration_end,
            String::new(),
        ));
        replacements.push((
            candidate.call_start,
            candidate.call_end,
            candidate.replacement,
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn fold_return_only_iife(
    source: &str,
    classic_only: bool,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if tokens[cursor].text != "(" {
            cursor += 1;
            continue;
        }
        if cursor
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .is_some_and(|token| {
                token.kind == TokenKind::Identifier
                    || matches!(token.text, ")" | "]" | "this" | "new")
            })
        {
            cursor += 1;
            continue;
        }
        let Some(function) = parse_function_expression(&tokens, &matching_close, cursor + 1) else {
            cursor += 1;
            continue;
        };
        let accepted_kind = if classic_only {
            !function.is_arrow && function.params_from == function.params_to
        } else {
            function.is_arrow
        };
        if !accepted_kind
            || function.named
            || !simple_identifier_params(&tokens, function.params_from, function.params_to)
        {
            cursor += 1;
            continue;
        }
        let grouping_close = function.end + 1;
        if tokens.get(grouping_close).map(|token| token.text) != Some(")")
            || tokens.get(grouping_close + 1).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let call_open = grouping_close + 1;
        let Some(call_close) = matching_close.get(call_open).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let params = identifier_list(&tokens, function.params_from, function.params_to);
        let Some(args) = simple_arg_list(source, &tokens, call_open + 1, call_close) else {
            cursor += 1;
            continue;
        };
        if params.len() != args.len() {
            cursor += 1;
            continue;
        }
        let Some((body, body_from, body_to)) =
            arrow_iife_body_span(source, &tokens, &matching_close, &function)
        else {
            cursor += 1;
            continue;
        };
        if body_assigns_any(&tokens, body_from, body_to, &params)
            || body_has_this_or_arguments(&tokens, &matching_close, body_from, body_to)
            || body_has_nested_function(&tokens, &matching_close, body_from, body_to)
            || (!function.is_arrow && body_has_new_target(&tokens, body_from, body_to))
        {
            cursor += 1;
            continue;
        }
        let rewritten = substitute_idents(body, &tokens, body_from, body_to, &params, &args);
        if rewritten.is_empty() {
            cursor += 1;
            continue;
        }
        // The IIFE is a call expression, while its returned body can have much
        // lower precedence. Keep grouping where a surrounding operator can
        // capture part of the body; return/assignment/argument positions can
        // use the expression directly and avoid inert parentheses.
        let rewritten = if beta_reduction_needs_grouping(
            &tokens,
            cursor,
            call_close,
            body_from,
            body_to,
        ) {
            format!("({rewritten})")
        } else {
            rewritten
        };
        replacements.push((
            tokens[cursor].start,
            tokens[call_close].end,
            rewritten,
        ));
        cursor = call_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn beta_reduction_needs_grouping(
    tokens: &[Token<'_>],
    start: usize,
    call_close: usize,
    body_from: usize,
    body_to: usize,
) -> bool {
    let previous = start.checked_sub(1).and_then(|index| tokens.get(index));
    if previous.is_some_and(|token| {
        matches!(
            token.text,
            "!" | "~"
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "**"
                | "<<"
                | ">>"
                | ">>>"
                | "<"
                | "<="
                | ">"
                | ">="
                | "=="
                | "!="
                | "==="
                | "!=="
                | "&"
                | "^"
                | "|"
                | "&&"
                | "||"
                | "??"
                | "typeof"
                | "void"
                | "delete"
                | "await"
                | "new"
        )
    }) {
        return true;
    }
    if tokens.get(call_close + 1).is_some_and(|token| {
        matches!(
            token.text,
            "." | "["
                | "("
                | "?."
                | "+"
                | "-"
                | "*"
                | "/"
                | "%"
                | "**"
                | "<<"
                | ">>"
                | ">>>"
                | "<"
                | "<="
                | ">"
                | ">="
                | "=="
                | "!="
                | "==="
                | "!=="
                | "&"
                | "^"
                | "|"
                | "&&"
                | "||"
                | "??"
        )
    }) {
        return true;
    }
    let mut depth = 0i32;
    for token in &tokens[body_from..body_to] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 && previous.is_none_or(|token| token.text != "return") => {
                return true;
            }
            _ => {}
        }
    }
    false
}

fn body_has_new_target(tokens: &[Token<'_>], from: usize, to: usize) -> bool {
    tokens[from..to].windows(3).any(|window| {
        window[0].text == "new" && window[1].text == "." && window[2].text == "target"
    })
}

fn identifier_list<'tok>(tokens: &'tok [Token<'tok>], from: usize, to: usize) -> Vec<&'tok str> {
    tokens[from..to]
        .iter()
        .filter(|token| token.kind == TokenKind::Identifier)
        .map(|token| token.text)
        .collect()
}

fn simple_arg_list<'src>(
    source: &'src str,
    tokens: &[Token<'_>],
    from: usize,
    close: usize,
) -> Option<Vec<&'src str>> {
    if from == close {
        return Some(Vec::new());
    }
    let mut args = Vec::new();
    let mut start = from;
    let mut depth = 0i32;
    for index in from..close {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => {
                args.push(simple_primary(source, tokens, start, index)?);
                start = index + 1;
            }
            _ => {}
        }
    }
    args.push(simple_primary(source, tokens, start, close)?);
    Some(args)
}

fn simple_primary<'src>(
    source: &'src str,
    tokens: &[Token<'_>],
    from: usize,
    to: usize,
) -> Option<&'src str> {
    if from >= to {
        return None;
    }
    if from + 1 == to
        && matches!(
            tokens[from].kind,
            TokenKind::Identifier | TokenKind::Number | TokenKind::String
        )
    {
        return Some(&source[tokens[from].start..tokens[from].end]);
    }
    if from + 1 == to && matches!(tokens[from].text, "null" | "true" | "false" | "undefined") {
        return Some(&source[tokens[from].start..tokens[from].end]);
    }
    if from + 2 == to && tokens[from].text == "void" && tokens[from + 1].text == "0" {
        return Some(&source[tokens[from].start..tokens[to - 1].end]);
    }
    None
}

fn arrow_iife_body_span<'src>(
    source: &'src str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    function: &crate::js_peephole::scope::FunctionExpression,
) -> Option<(&'src str, usize, usize)> {
    if let Some(block_open) = function.block_open {
        let close = matching_close.get(block_open).copied().flatten()?;
        if tokens.get(block_open + 1).map(|token| token.text) != Some("return") {
            return None;
        }
        let end = if tokens.get(close - 1).map(|token| token.text) == Some(";") {
            close - 1
        } else {
            close
        };
        if block_open + 2 >= end {
            return None;
        }
        return Some((
            &source[tokens[block_open + 2].start..tokens[end - 1].end],
            block_open + 2,
            end,
        ));
    }
    let body_from = arrow_body_token_from(tokens, function);
    if body_from > function.end {
        return None;
    }
    Some((
        &source[tokens[body_from].start..tokens[function.end].end],
        body_from,
        function.end + 1,
    ))
}

fn arrow_body_token_from(
    tokens: &[Token<'_>],
    function: &crate::js_peephole::scope::FunctionExpression,
) -> usize {
    if tokens.get(function.params_to).map(|token| token.text) == Some("=>") {
        function.params_to + 1
    } else {
        function.params_to + 2
    }
}

fn body_assigns_any(tokens: &[Token<'_>], from: usize, to: usize, names: &[&str]) -> bool {
    let mut index = from;
    while index < to {
        if tokens[index].kind == TokenKind::Identifier
            && names.contains(&tokens[index].text)
            && matches!(
                tokens.get(index + 1).map(|token| token.text),
                Some("=") | Some("++") | Some("--") | Some("+=") | Some("-=")
            )
        {
            return true;
        }
        index += 1;
    }
    false
}

fn body_has_nested_function(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> bool {
    let mut index = from;
    while index < to {
        if crate::js_peephole::scope::nested_function_end(tokens, matching_close, index).is_some()
            || tokens[index].text == "=>"
            || tokens[index].text == "function"
        {
            return true;
        }
        index += 1;
    }
    false
}

fn body_has_this_or_arguments(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> bool {
    let mut index = from;
    while index < to {
        if let Some(close) =
            crate::js_peephole::scope::nested_function_end(tokens, matching_close, index)
        {
            index = close + 1;
            continue;
        }
        if matches!(tokens[index].text, "this" | "arguments") {
            return true;
        }
        index += 1;
    }
    false
}

fn substitute_idents(
    body: &str,
    tokens: &[Token<'_>],
    body_from: usize,
    body_to: usize,
    params: &[&str],
    args: &[&str],
) -> String {
    let mut output = String::new();
    let mut cursor = 0usize;
    let body_start = tokens[body_from].start;
    for index in body_from..body_to {
        if tokens[index].kind != TokenKind::Identifier {
            continue;
        }
        let Some(arg) = params
            .iter()
            .position(|name| *name == tokens[index].text)
            .map(|slot| args[slot])
        else {
            continue;
        };
        if tokens.get(index.wrapping_sub(1)).map(|token| token.text) == Some(".") {
            continue;
        }
        let local = tokens[index].start - body_start;
        output.push_str(&body[cursor..local]);
        output.push_str(arg);
        cursor = local + tokens[index].text.len();
    }
    output.push_str(&body[cursor..]);
    output
}

fn is_expression_statement_span(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let start = (start..end)
        .find(|&index| tokens[index].text != ";")
        .unwrap_or(end);
    if start >= end {
        return false;
    }
    !matches!(
        tokens[start].text,
        "if" | "else"
            | "for"
            | "while"
            | "var"
            | "let"
            | "const"
            | "function"
            | "class"
            | "return"
            | "throw"
            | "try"
            | "switch"
            | "do"
            | "with"
            | "break"
            | "continue"
            | "debugger"
            | "case"
            | "default"
    )
}

pub(crate) fn fold_same_receiver_method_call(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if (tokens[cursor].kind != TokenKind::Identifier && tokens[cursor].text != "this")
            || is_property_identifier(&tokens, cursor)
        {
            cursor += 1;
            continue;
        }
        if tokens.get(cursor + 1).map(|token| token.text) != Some(".")
            || !tokens
                .get(cursor + 2)
                .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
            || tokens.get(cursor + 3).map(|token| token.text) != Some(".")
            || tokens.get(cursor + 4).map(|token| token.text) != Some("call")
            || tokens.get(cursor + 5).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let open = cursor + 5;
        let Some(close) = matching_close.get(open).copied().flatten() else {
            cursor += 1;
            continue;
        };
        if tokens.get(open + 1).map(|token| token.text) != Some(tokens[cursor].text) {
            cursor += 1;
            continue;
        }
        if tokens.get(open + 2).map(|token| token.text) != Some(",")
            && open + 2 != close
        {
            cursor += 1;
            continue;
        }
        let args = if open + 2 == close {
            String::new()
        } else {
            source[tokens[open + 3].start..tokens[close].start].to_string()
        };
        replacements.push((
            tokens[cursor].start,
            tokens[close].end,
            format!("{}.{}({args})", tokens[cursor].text, tokens[cursor + 2].text),
        ));
        cursor = close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}
