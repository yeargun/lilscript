use crate::js_peephole::rewrite::{
    apply_token_rewrites, conditional_test_needs_grouping, expression_has_top_level_token,
    identifier_occurs, non_overlapping_ranges, single_console_log_argument, top_level_stop,
    wrap_substituted_expression,
};
use crate::js_peephole::scope::{
    collect_same_scope_name_uses, enclosing_block_start, enclosing_function_span,
    name_use_is_mutated,
};
use crate::js_peephole::token::{lex, matching_closers, matching_openers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

fn is_console_log_open(tokens: &[Token<'_>], index: usize) -> bool {
    tokens.get(index).map(|token| token.text) == Some("console")
        && tokens.get(index + 1).map(|token| token.text) == Some(".")
        && tokens.get(index + 2).map(|token| token.text) == Some("log")
        && tokens.get(index + 3).map(|token| token.text) == Some("(")
}

pub(crate) fn fold_console_log_conditionals(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut index = 0usize;
    while index + 8 < tokens.len() {
        if tokens[index].text != "?" || !is_console_log_open(&tokens, index + 1) {
            index += 1;
            continue;
        }
        let then_open = index + 4;
        let Some(then_close) = matching_close[then_open] else {
            index += 1;
            continue;
        };
        if tokens.get(then_close + 1).map(|token| token.text) != Some(":")
            || !is_console_log_open(&tokens, then_close + 2)
        {
            index += 1;
            continue;
        }
        let else_open = then_close + 5;
        let Some(else_close) = matching_close[else_open] else {
            index += 1;
            continue;
        };
        let next = tokens.get(else_close + 1).map(|token| token.text);
        if next.is_some_and(|token| !matches!(token, ";" | "}" | "{")) {
            index += 1;
            continue;
        }
        let then_arg = source[tokens[then_open].end..tokens[then_close].start].to_string();
        let else_arg = source[tokens[else_open].end..tokens[else_close].start].to_string();
        if single_console_log_argument(&format!("console.log({then_arg})")).is_none()
            || single_console_log_argument(&format!("console.log({else_arg})")).is_none()
        {
            index += 1;
            continue;
        }
        let mut cond_start = 0usize;
        let mut depth = 0i32;
        let mut statement = true;
        for token_index in (0..index).rev() {
            match tokens[token_index].text {
                ")" | "]" | "}" => depth += 1,
                "(" | "[" | "{" => {
                    if depth == 0 {
                        if tokens[token_index].text != "{" {
                            statement = false;
                        }
                        cond_start = token_index + 1;
                        break;
                    }
                    depth -= 1;
                }
                ";" if depth == 0 => {
                    cond_start = token_index + 1;
                    break;
                }
                "," | ":" | "?" if depth == 0 => {
                    statement = false;
                    break;
                }
                _ => {}
            }
        }
        if !statement {
            index += 1;
            continue;
        }
        let condition = source[tokens[cond_start].start..tokens[index].start].trim();
        if condition.is_empty() {
            index += 1;
            continue;
        }
        let mut end = tokens[else_close].end;
        if tokens.get(else_close + 1).map(|token| token.text) == Some(";") {
            end = tokens[else_close + 1].end;
        }
        let replacement = format!("console.log({condition}?{then_arg}:{else_arg});");
        if replacement.len() < end - tokens[cond_start].start {
            replacements.push((tokens[cond_start].start, end, replacement));
        }
        index = else_close + 1;
    }
    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
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
        output.replace_range(start..end, &replacement);
    }
    Ok((output, count))
}

pub(crate) fn fold_coalesced_or_returns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 12 < tokens.len() {
        let name_index = if matches!(tokens[cursor].text, "var" | "let") {
            cursor + 1
        } else {
            cursor
        };
        if tokens
            .get(name_index)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_index + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[name_index].text;
        let Some(first_semi) = top_level_stop(&tokens, name_index + 2, &[";"]) else {
            cursor += 1;
            continue;
        };
        if tokens.get(first_semi + 1).map(|token| token.text) != Some("!")
            || tokens.get(first_semi + 2).map(|token| token.text) != Some(name)
            || tokens.get(first_semi + 3).map(|token| token.text) != Some("&&")
            || tokens.get(first_semi + 4).map(|token| token.text) != Some("(")
            || tokens.get(first_semi + 5).map(|token| token.text) != Some(name)
            || tokens.get(first_semi + 6).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(assign_close) = top_level_stop(&tokens, first_semi + 7, &[")"]) else {
            cursor += 1;
            continue;
        };
        if tokens.get(assign_close + 1).map(|token| token.text) != Some(";")
            || tokens.get(assign_close + 2).map(|token| token.text) != Some("return")
        {
            cursor += 1;
            continue;
        }
        if identifier_occurs(&tokens, first_semi + 7, assign_close, name) {
            cursor += 1;
            continue;
        }
        let left = &source[tokens[name_index + 2].start..tokens[first_semi].start];
        let right = &source[tokens[first_semi + 7].start..tokens[assign_close].start];
        let value = format!("{left}||{right}");
        let return_at = assign_close + 2;
        let comma_declarator = name_index
            .checked_sub(1)
            .is_some_and(|index| tokens[index].text == ",");
        let replace_from = if comma_declarator {
            tokens[name_index - 1].start
        } else {
            tokens[cursor].start
        };
        let prefix = if comma_declarator { ";" } else { "" };
        let replacement = if tokens.get(return_at + 1).map(|token| token.text) == Some(name) {
            let after_name = return_at + 2;
            if tokens.get(after_name).map(|token| token.text) == Some(".")
                && tokens
                    .get(after_name + 1)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens.get(after_name + 2).map(|token| token.text) == Some("(")
            {
                let Some(call_close) = matching_close.get(after_name + 2).copied().flatten() else {
                    cursor += 1;
                    continue;
                };
                if identifier_occurs(&tokens, after_name + 3, call_close, name) {
                    cursor += 1;
                    continue;
                }
                let end = tokens
                    .get(call_close + 1)
                    .filter(|token| token.text == ";")
                    .map(|token| token.end)
                    .unwrap_or(tokens[call_close].end);
                let suffix = &source[tokens[after_name].start..tokens[call_close].end];
                (
                    replace_from,
                    end,
                    format!("{prefix}return ({value}){suffix}"),
                )
            } else {
                let end = tokens
                    .get(after_name)
                    .filter(|token| token.text == ";")
                    .map(|token| token.end)
                    .unwrap_or(tokens[return_at + 1].end);
                (replace_from, end, format!("{prefix}return {value}"))
            }
        } else if tokens.get(return_at + 1).map(|token| token.text) == Some("!")
            && tokens
                .get(return_at + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(return_at + 3).map(|token| token.text) == Some(".")
            && tokens.get(return_at + 4).map(|token| token.text) == Some("test")
            && tokens.get(return_at + 5).map(|token| token.text) == Some("(")
            && tokens.get(return_at + 6).map(|token| token.text) == Some(name)
            && tokens.get(return_at + 7).map(|token| token.text) == Some(")")
        {
            let end = tokens
                .get(return_at + 8)
                .filter(|token| token.text == ";")
                .map(|token| token.end)
                .unwrap_or(tokens[return_at + 7].end);
            (
                replace_from,
                end,
                format!(
                    "{prefix}return!{}.test({value})",
                    tokens[return_at + 2].text
                ),
            )
        } else {
            cursor += 1;
            continue;
        };
        replacements.push(replacement);
        cursor = return_at + 4;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn expression_statement_texts<'src>(
    source: &'src str,
    tokens: &[Token<'src>],
    start: usize,
    end: usize,
) -> Option<Vec<String>> {
    let mut delimiters = Vec::new();
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => delimiters.push(token.text),
            ")" if delimiters.pop() != Some("(") => return None,
            "]" if delimiters.pop() != Some("[") => return None,
            "}" if delimiters.pop() != Some("{") => return None,
            _ => {}
        }
    }
    if !delimiters.is_empty() {
        return None;
    }
    let mut index = start;
    let mut exprs = Vec::new();
    while index < end {
        if tokens[index].text == ";" {
            index += 1;
            continue;
        }
        if matches!(
            tokens[index].text,
            ")" | "]"
                | "}"
                | ","
                | ":"
                | "else"
                | "case"
                | "default"
                | "if"
                | "for"
                | "while"
                | "var"
                | "let"
                | "const"
                | "function"
                | "return"
                | "switch"
                | "try"
                | "throw"
                | "class"
                | "do"
                | "break"
                | "continue"
                | "debugger"
        ) {
            return None;
        }
        let stop = top_level_stop(tokens, index, &[";"])
            .filter(|stop| *stop < end)
            .unwrap_or(end);
        if stop == index {
            return None;
        }
        let expr = source[tokens[index].start..tokens[stop].start].trim();
        if expr.is_empty() {
            return None;
        }
        exprs.push(expr.to_string());
        index = if stop < end { stop + 1 } else { end };
    }
    (!exprs.is_empty()).then_some(exprs)
}

pub(crate) fn fold_trailing_return_this(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for open in 1..tokens.len() {
        if tokens[open].text != "{" || !matches!(tokens[open - 1].text, ")" | "=>") {
            continue;
        }
        let Some(close) = matching_close.get(open).copied().flatten() else {
            continue;
        };
        let mut end = close;
        if tokens.get(close - 1).map(|token| token.text) == Some(";") {
            end = close - 1;
        }
        if end < open + 3
            || tokens.get(end - 1).map(|token| token.text) != Some("this")
            || tokens.get(end - 2).map(|token| token.text) != Some("return")
        {
            continue;
        }
        let stmts_end = end - 2;
        if stmts_end == open + 1 {
            continue;
        }
        let prefix = if tokens.get(open + 1).map(|token| token.text) == Some("if")
            && tokens.get(open + 2).map(|token| token.text) == Some("(")
        {
            let Some(cond_close) = matching_close.get(open + 2).copied().flatten() else {
                continue;
            };
            let cond_raw = &source[tokens[open + 3].start..tokens[cond_close].start];
            let cond = wrap_and_operand(cond_raw, &tokens, open + 3, cond_close);
            let (body_start, body_end) = if tokens.get(cond_close + 1).map(|token| token.text)
                == Some("{")
            {
                let Some(body_close) = matching_close.get(cond_close + 1).copied().flatten() else {
                    continue;
                };
                if body_close + 1 != stmts_end
                    && !(body_close + 2 == stmts_end
                        && tokens.get(body_close + 1).map(|token| token.text) == Some(";"))
                {
                    continue;
                }
                (cond_close + 2, body_close)
            } else {
                let Some(stop) = top_level_stop(&tokens, cond_close + 1, &[";"]) else {
                    continue;
                };
                if stop + 1 != stmts_end {
                    continue;
                }
                (cond_close + 1, stop)
            };
            let Some(exprs) = expression_statement_texts(source, &tokens, body_start, body_end)
            else {
                continue;
            };
            format!("{cond}&&({})", exprs.join(","))
        } else {
            let Some(exprs) = expression_statement_texts(source, &tokens, open + 1, stmts_end)
            else {
                continue;
            };
            if exprs.len() == 1 {
                exprs[0].clone()
            } else {
                exprs.join(",")
            }
        };
        replacements.push((
            tokens[open].start,
            tokens[close].end,
            format!("{{return {prefix},this}}"),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_if_prefix_guard_return(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for if_at in 0..tokens.len() {
        if tokens[if_at].text != "if" || tokens.get(if_at + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(cond_close) = matching_close.get(if_at + 1).copied().flatten() else {
            continue;
        };
        if tokens.get(cond_close + 1).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(cond_close + 1).copied().flatten() else {
            continue;
        };
        let mut depth = 0i32;
        let mut inner_if = None;
        let mut index = cond_close + 2;
        while index < body_close {
            match tokens[index].text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                "if" if depth == 0 => inner_if = Some(index),
                _ => {}
            }
            index += 1;
        }
        let Some(inner_if) = inner_if else {
            continue;
        };
        if tokens.get(inner_if + 1).map(|token| token.text) != Some("(") {
            continue;
        }
        let Some(inner_close) = matching_close.get(inner_if + 1).copied().flatten() else {
            continue;
        };
        if tokens.get(inner_close + 1).map(|token| token.text) != Some("return") {
            continue;
        }
        let Some(ret_end) = top_level_stop(&tokens, inner_close + 2, &[";", "}"]) else {
            continue;
        };
        if ret_end < body_close {
            let mut rest = ret_end;
            if tokens[rest].text == ";" {
                rest += 1;
            }
            if rest != body_close {
                continue;
            }
        }
        if prefix_has_statement_keyword(&tokens, cond_close + 2, inner_if) {
            continue;
        }
        let prefix = source[tokens[cond_close + 2].start..tokens[inner_if].start]
            .trim()
            .trim_end_matches(';');
        if prefix.is_empty() || prefix.contains(';') {
            continue;
        }
        let cond = &source[tokens[if_at + 2].start..tokens[cond_close].start];
        let inner = &source[tokens[inner_if + 2].start..tokens[inner_close].start];
        let ret = source[tokens[inner_close + 2].start..tokens[ret_end].start].trim();
        if ret.is_empty() {
            continue;
        }
        replacements.push((
            tokens[if_at].start,
            tokens[body_close].end,
            format!("if({cond}&&({prefix},{inner}))return {ret};"),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn prefix_has_statement_keyword(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "if" | "for" | "while" | "function" | "var" | "let" | "const" | "return" | "break"
            | "continue" | "switch" | "try" | "class"
                if depth == 0 =>
            {
                return true;
            }
            _ => {}
        }
    }
    false
}

pub(crate) fn fold_arrow_guard_returns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut matching_close = vec![None; tokens.len()];
    let mut stack = Vec::<usize>::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "(" | "[" | "{" => stack.push(index),
            ")" | "]" | "}" => {
                let Some(open) = stack.pop() else {
                    continue;
                };
                matching_close[open] = Some(index);
            }
            _ => {}
        }
    }

    let mut replacements = Vec::new();
    for body_open in 1..tokens.len() {
        if tokens[body_open].text != "{" || tokens[body_open - 1].text != "=>" {
            continue;
        }
        let Some(body_close) = matching_close[body_open] else {
            continue;
        };
        let if_index = body_open + 1;
        if tokens.get(if_index).map(|token| token.text) != Some("if")
            || tokens.get(if_index + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let condition_open = if_index + 1;
        let Some(condition_close) = matching_close[condition_open] else {
            continue;
        };
        let first_return = condition_close + 1;
        if tokens.get(first_return).map(|token| token.text) != Some("return") {
            continue;
        }
        let mut delimiters = Vec::<&str>::new();
        let mut first_semicolon = None;
        for (index, token) in tokens
            .iter()
            .enumerate()
            .take(body_close)
            .skip(first_return + 1)
        {
            match token.text {
                "(" | "[" | "{" => delimiters.push(token.text),
                ")" | "]" | "}" => {
                    delimiters.pop();
                }
                ";" if delimiters.is_empty() => {
                    first_semicolon = Some(index);
                    break;
                }
                _ => {}
            }
        }
        let Some(first_semicolon) = first_semicolon else {
            continue;
        };
        let second_return = first_semicolon + 1;
        if tokens.get(second_return).map(|token| token.text) != Some("return")
            || second_return + 1 >= body_close
        {
            continue;
        }
        let second_end = if tokens[body_close - 1].text == ";" {
            body_close - 1
        } else {
            body_close
        };
        if second_return + 1 >= second_end {
            continue;
        }

        let condition = &source[tokens[condition_open].end..tokens[condition_close].start];
        let condition =
            if conditional_test_needs_grouping(&tokens[condition_open + 1..condition_close]) {
                format!("({condition})")
            } else {
                condition.to_string()
            };
        // Bare `return;` is a value-producing `undefined` arm once the two
        // returns become one conditional expression. Never leave an empty
        // grammar arm (`condition?:value`) in a codec-scored candidate.
        let first = if first_return + 1 == first_semicolon {
            "void 0".to_string()
        } else {
            let first = &source[tokens[first_return + 1].start..tokens[first_semicolon].start];
            if expression_has_top_level_token(&tokens[first_return + 1..first_semicolon], ",") {
                format!("({first})")
            } else {
                first.to_string()
            }
        };
        let second = &source[tokens[second_return + 1].start..tokens[second_end].start];
        let second = if expression_has_top_level_token(&tokens[second_return + 1..second_end], ",")
        {
            format!("({second})")
        } else {
            second.to_string()
        };
        let replacement = format!("{condition}?{first}:{second}");
        let start = tokens[body_open].start;
        let end = tokens[body_close].end;
        if replacement.len() < end - start {
            replacements.push((start, end, replacement));
        }
    }
    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
    }
    // Nested candidates overlap; prefer the outermost non-overlapping set so
    // the largest guard/body wrapper disappears in this bounded pass.
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
        output.replace_range(start..end, &replacement);
    }
    Ok((output, count))
}

/// Index of the token that ends the statement starting at `start`, and whether
/// that token is the statement's own `;` rather than the enclosing block's `}`.
fn statement_terminator(tokens: &[Token<'_>], start: usize) -> Option<(usize, bool)> {
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate().skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    return Some((index, false));
                }
                depth -= 1;
            }
            ";" if depth == 0 => return Some((index, true)),
            "for" | "if" | "while" | "switch" | "try" | "var" | "let" | "const" | "return"
            | "do" | "with" | "debugger" | "break" | "continue" | "else"
                if depth == 0 =>
            {
                return Some((index, false));
            }
            _ => {}
        }
    }
    None
}

/// A `return` argument spelled as one conditional arm.
///
/// An absent argument is `undefined` once the two returns become one
/// expression, and a top-level comma binds looser than a conditional arm.
fn conditional_return_arm(source: &str, tokens: &[Token<'_>], start: usize, end: usize) -> String {
    if start >= end {
        return "void 0".to_string();
    }
    let text = &source[tokens[start].start..tokens[end - 1].end];
    if expression_has_top_level_token(&tokens[start..end], ",") {
        format!("({text})")
    } else {
        text.to_string()
    }
}

fn spans_line_terminator(source: &str) -> bool {
    source.contains(['\n', '\r', '\u{2028}', '\u{2029}'])
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EarlyExitKind {
    Return,
    Continue,
}

/// Invert a function- or loop-body guard whose only arm exits that body, then
/// place the remaining suffix under the inverted condition.
///
/// `if(C)return;S` becomes `if(!C){S}` at function-body level, and
/// `if(C)continue;S` receives the same spelling at loop-body level. The
/// original completion skips exactly `S`; the replacement executes exactly
/// `S` when the guard is false. This is the broad structural family behind a
/// large part of Terser's `if_return` win on generated jQuery.
///
/// Moving a lexical declaration into the new block can change its TDZ and
/// visibility before the guard. Generated `var` declarations are unaffected,
/// but `let`, `const`, class, and function declarations make the proposal
/// ineligible. Non-body-level guards and labelled exits are likewise refused.
pub(crate) fn fold_early_exit_guards(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let matching_open = matching_openers(&matching_close);
        let mut replacements = Vec::<(usize, usize, String)>::new();

        for guard in 0..tokens.len() {
            if tokens[guard].text != "if"
                || tokens.get(guard + 1).map(|token| token.text) != Some("(")
                || guard
                    .checked_sub(1)
                    .is_some_and(|previous| !matches!(tokens[previous].text, "{" | "}" | ";"))
            {
                continue;
            }
            let condition_open = guard + 1;
            let Some(condition_close) = matching_close[condition_open] else {
                continue;
            };
            let Some(container_open) = enclosing_block_start(&matching_close, guard) else {
                continue;
            };
            let Some(container_close) = matching_close[container_open] else {
                continue;
            };

            let arm_start = condition_close + 1;
            let (exit, after_guard) = if tokens.get(arm_start).map(|token| token.text) == Some("{")
            {
                let Some(arm_close) = matching_close[arm_start] else {
                    continue;
                };
                let Some(exit) = bare_early_exit(&tokens, arm_start + 1, Some(arm_close)) else {
                    continue;
                };
                (exit, arm_close + 1)
            } else {
                let Some(exit) = bare_early_exit(&tokens, arm_start, None) else {
                    continue;
                };
                (exit, arm_start + 2)
            };
            if tokens.get(after_guard).map(|token| token.text) == Some("else")
                || after_guard >= container_close
            {
                continue;
            }

            let function_body = enclosing_function_span(&tokens, &matching_close, guard);
            let valid_container = match exit {
                EarlyExitKind::Return => function_body.is_some_and(|(body, close)| {
                    body == container_open && close == container_close
                }),
                EarlyExitKind::Continue => {
                    block_is_loop_body(&tokens, &matching_open, container_open)
                }
            };
            if !valid_container
                || suffix_has_scope_changing_declaration(&tokens, after_guard, container_close)
            {
                continue;
            }

            let condition = negate_early_exit_condition(
                &output,
                &tokens,
                &matching_close,
                condition_open + 1,
                condition_close,
            );
            let suffix = &output[tokens[after_guard].start..tokens[container_close].start];
            replacements.push((
                tokens[guard].start,
                tokens[container_close].start,
                format!("if({condition}){{{suffix}}}"),
            ));
        }

        if replacements.is_empty() {
            return Ok((output, folded));
        }
        // Guards in one body all extend to the same closing brace. Rewrite
        // the rightmost one first; the next round can then invert its parent
        // without overlapping byte ranges or changing enumeration order.
        replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
        let mut retained = Vec::<(usize, usize, String)>::new();
        for replacement in replacements.into_iter().rev() {
            if retained
                .last()
                .is_none_or(|(start, _, _)| replacement.1 <= *start)
            {
                retained.push(replacement);
            }
        }
        folded += retained.len();
        for (start, end, replacement) in retained {
            output.replace_range(start..end, &replacement);
        }
    }
}

/// Turn a loop-body guard whose braced arm finishes with `continue` into an
/// `if`/`else` that reaches the loop backedge by ordinary fallthrough.
///
/// `if(C){E;continue}S` becomes `if(C){E}else{S}`. On the true path `E`
/// completes at the end of the loop body instead of executing an explicit
/// continue; on the false path the old suffix remains the only work. This is
/// three raw bytes smaller per guard and repeated scanner-style guard ladders
/// become nested `else` chains one level per round.
///
/// Only direct statements of a braced loop body are eligible. Moving a
/// top-level lexical declaration from the loop body into the new `else` block
/// could change TDZ or visibility, so the same conservative declaration gate
/// used by bare early-exit inversion applies to the suffix.
pub(crate) fn fold_continue_tail_guards(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_continue_tail_guards_with_orientation(source, false)
}

/// The same loop-tail proof as [`fold_continue_tail_guards`], with the
/// continuation suffix in the first arm: `if(!C){S}else{E}`. It is often
/// raw-larger when `C` needs grouping, but scanner ladders and transfer-codec
/// dictionaries can strongly prefer this orientation, so it remains an
/// independent whole-artifact proposal.
pub(crate) fn fold_inverted_continue_tail_guards(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    fold_continue_tail_guards_with_orientation(source, true)
}

fn fold_continue_tail_guards_with_orientation(
    source: &str,
    inverted: bool,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let matching_open = matching_openers(&matching_close);
        let mut replacements = Vec::<(usize, usize, String)>::new();

        for guard in 0..tokens.len() {
            if tokens[guard].text != "if"
                || tokens.get(guard + 1).map(|token| token.text) != Some("(")
                || guard
                    .checked_sub(1)
                    .is_some_and(|previous| !matches!(tokens[previous].text, "{" | "}" | ";"))
            {
                continue;
            }
            let condition_open = guard + 1;
            let Some(condition_close) = matching_close[condition_open] else {
                continue;
            };
            let arm_open = condition_close + 1;
            if tokens.get(arm_open).map(|token| token.text) != Some("{") {
                continue;
            }
            let Some(arm_close) = matching_close[arm_open] else {
                continue;
            };
            let Some(last_in_arm) = arm_close.checked_sub(1) else {
                continue;
            };
            let continue_index = if tokens[last_in_arm].text == "continue" {
                last_in_arm
            } else if tokens[last_in_arm].text == ";"
                && last_in_arm
                    .checked_sub(1)
                    .is_some_and(|previous| tokens[previous].text == "continue")
            {
                last_in_arm - 1
            } else {
                continue;
            };
            // The final token can still be controlled by an unbraced nested
            // statement (`if(B)continue`). Removing that token would erase
            // the nested condition, not merely replace the arm's terminal
            // backedge. A direct statement starts at the arm boundary or
            // after another complete statement/block.
            if continue_index.checked_sub(1).is_some_and(|previous| {
                previous != arm_open && !matches!(tokens[previous].text, ";" | "}")
            }) {
                continue;
            }
            if continue_index <= arm_open + 1
                || tokens.get(arm_close + 1).map(|token| token.text) == Some("else")
            {
                continue;
            }

            let Some(container_open) = enclosing_block_start(&matching_close, guard) else {
                continue;
            };
            let Some(container_close) = matching_close[container_open] else {
                continue;
            };
            let after_guard = arm_close + 1;
            if after_guard >= container_close
                || !block_is_loop_body(&tokens, &matching_open, container_open)
                || suffix_has_scope_changing_declaration(&tokens, after_guard, container_close)
            {
                continue;
            }

            let condition = if inverted {
                negate_early_exit_condition(
                    &output,
                    &tokens,
                    &matching_close,
                    condition_open + 1,
                    condition_close,
                )
            } else {
                output[tokens[condition_open].end..tokens[condition_close].start].to_string()
            };
            let prefix = &output[tokens[arm_open].end..tokens[continue_index].start];
            let suffix = &output[tokens[after_guard].start..tokens[container_close].start];
            let replacement = if inverted {
                format!("if({condition}){{{suffix}}}else{{{prefix}}}")
            } else {
                format!("if({condition}){{{prefix}}}else{{{suffix}}}")
            };
            if inverted || replacement.len() < tokens[container_close].start - tokens[guard].start {
                replacements.push((
                    tokens[guard].start,
                    tokens[container_close].start,
                    replacement,
                ));
            }
        }

        if replacements.is_empty() {
            return Ok((output, folded));
        }
        // Every candidate in one loop extends through the same body suffix.
        // Rewrite the rightmost guard first, then let the next round wrap the
        // resulting `if`/`else` as the suffix of the preceding guard.
        replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
        let mut retained = Vec::<(usize, usize, String)>::new();
        for replacement in replacements.into_iter().rev() {
            if retained
                .last()
                .is_none_or(|(start, _, _)| replacement.1 <= *start)
            {
                retained.push(replacement);
            }
        }
        folded += retained.len();
        for (start, end, replacement) in retained {
            output.replace_range(start..end, &replacement);
        }
    }
}

/// Elide braces around one statement used as an `if`, `else`, or loop body.
///
/// Expression-like statements and `var`/exit statements are direct leaves;
/// nested control statements are followed recursively so an outer wrapper can
/// disappear too. Lexical declarations stay braced, and an `if` consequence
/// stays braced whenever its nested statement could capture the outer
/// `else`. A missing trailing semicolon is restored when the closing brace was
/// its ASI boundary.
pub(crate) fn fold_single_statement_control_braces(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let matching_open = matching_openers(&matching_close);
        let mut replacements = Vec::<(usize, usize, String)>::new();

        for block_open in 0..tokens.len() {
            if tokens[block_open].text != "{"
                || !is_simple_control_body_block(&tokens, &matching_open, block_open)
            {
                continue;
            }
            let Some(block_close) = matching_close[block_open] else {
                continue;
            };
            let statement_start = block_open + 1;
            let Some(can_absorb_else) = block_single_statement_can_absorb_else(
                &output,
                &tokens,
                &matching_close,
                statement_start,
                block_close,
            ) else {
                continue;
            };
            if can_absorb_else
                && control_body_block_has_following_else(
                    &tokens,
                    &matching_open,
                    block_open,
                    block_close,
                )
            {
                continue;
            }

            let mut replacement = output[tokens[block_open].end..tokens[block_close].start]
                .trim()
                .to_string();
            if block_open
                .checked_sub(1)
                .is_some_and(|previous| matches!(tokens[previous].text, "else" | "do"))
            {
                replacement.insert(0, ' ');
            }
            let last_before_close = block_close.checked_sub(1);
            let has_terminator = last_before_close
                .and_then(|index| tokens.get(index))
                .map(|token| token.text)
                == Some(";");
            let statement_ends_without_semicolon = last_before_close.is_some_and(|index| {
                tokens.get(index).is_some_and(|token| token.text == "}")
                    && closing_brace_terminates_statement(&tokens, &matching_open, index)
            }) || (tokens[statement_start].text == "do"
                && last_before_close
                    .and_then(|index| tokens.get(index))
                    .is_some_and(|token| token.text == ")"));
            let next_terminates_by_grammar = tokens
                .get(block_close + 1)
                .is_none_or(|token| matches!(token.text, "}" | ";"));
            if !has_terminator && !statement_ends_without_semicolon && !next_terminates_by_grammar {
                replacement.push(';');
            }
            let start = tokens[block_open].start;
            let end = tokens[block_close].end;
            if replacement.len() < end - start {
                replacements.push((start, end, replacement));
            }
        }

        if replacements.is_empty() {
            return Ok((output, folded));
        }
        let retained = non_overlapping_ranges(replacements);
        folded += retained.len();
        for (start, end, replacement) in retained.into_iter().rev() {
            output.replace_range(start..end, &replacement);
        }
    }
}

fn closing_brace_terminates_statement(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    close: usize,
) -> bool {
    let Some(open) = matching_open.get(close).copied().flatten() else {
        return false;
    };
    let Some(previous) = open.checked_sub(1) else {
        return true;
    };
    match tokens[previous].text {
        "else" | "do" | "try" | "catch" | "finally" | "{" | ";" => true,
        ")" => parenthesis_introduces_statement_block(tokens, matching_open, previous),
        _ => false,
    }
}

fn parenthesis_introduces_statement_block(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    close_paren: usize,
) -> bool {
    let Some(open) = matching_open.get(close_paren).copied().flatten() else {
        return false;
    };
    let Some(previous) = open.checked_sub(1) else {
        return false;
    };
    match tokens[previous].text {
        "if" | "for" | "while" | "with" | "switch" | "catch" => true,
        "await" => previous
            .checked_sub(1)
            .is_some_and(|index| tokens[index].text == "for"),
        _ => false,
    }
}

fn is_simple_control_body_block(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    block_open: usize,
) -> bool {
    let Some(previous) = block_open.checked_sub(1) else {
        return false;
    };
    if matches!(tokens[previous].text, "else" | "do") {
        return true;
    }
    if tokens[previous].text != ")" {
        return false;
    }
    let Some(header_open) = matching_open[previous] else {
        return false;
    };
    header_open
        .checked_sub(1)
        .is_some_and(|keyword| matches!(tokens[keyword].text, "if" | "for" | "while"))
}

fn block_single_statement_can_absorb_else(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
) -> Option<bool> {
    if start >= end || spans_line_terminator(&source[tokens[start].start..tokens[end].start]) {
        return None;
    }
    let statement = statement_extent(tokens, matching_close, start, end)?;
    (statement.end == end).then_some(statement.can_absorb_else_after_elision)
}

#[derive(Clone, Copy)]
struct StatementExtent {
    end: usize,
    can_absorb_else: bool,
    can_absorb_else_after_elision: bool,
}

fn statement_extent(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    limit: usize,
) -> Option<StatementExtent> {
    if start >= limit {
        return None;
    }
    match tokens[start].text {
        "{" => {
            let close = matching_close[start]?;
            if close >= limit {
                return None;
            }
            let can_absorb_else_after_elision =
                statement_extent(tokens, matching_close, start + 1, close).is_some_and(
                    |statement| statement.end == close && statement.can_absorb_else_after_elision,
                );
            Some(StatementExtent {
                end: close + 1,
                can_absorb_else: false,
                can_absorb_else_after_elision,
            })
        }
        "if" => {
            let condition_open = start + 1;
            if tokens.get(condition_open).map(|token| token.text) != Some("(") {
                return None;
            }
            let condition_close = matching_close[condition_open]?;
            let consequent = statement_extent(tokens, matching_close, condition_close + 1, limit)?;
            if tokens.get(consequent.end).map(|token| token.text) != Some("else") {
                return Some(StatementExtent {
                    end: consequent.end,
                    can_absorb_else: true,
                    can_absorb_else_after_elision: true,
                });
            }
            let alternative = statement_extent(tokens, matching_close, consequent.end + 1, limit)?;
            Some(StatementExtent {
                end: alternative.end,
                can_absorb_else: alternative.can_absorb_else,
                can_absorb_else_after_elision: alternative.can_absorb_else_after_elision,
            })
        }
        "for" | "while" | "with" => {
            let mut header_open = start + 1;
            if tokens[start].text == "for"
                && tokens.get(header_open).map(|token| token.text) == Some("await")
            {
                header_open += 1;
            }
            if tokens.get(header_open).map(|token| token.text) != Some("(") {
                return None;
            }
            let header_close = matching_close[header_open]?;
            statement_extent(tokens, matching_close, header_close + 1, limit)
        }
        "do" => {
            let body = statement_extent(tokens, matching_close, start + 1, limit)?;
            if tokens.get(body.end).map(|token| token.text) != Some("while")
                || tokens.get(body.end + 1).map(|token| token.text) != Some("(")
            {
                return None;
            }
            let condition_close = matching_close[body.end + 1]?;
            let end = condition_close
                + 1
                + usize::from(tokens.get(condition_close + 1).map(|token| token.text) == Some(";"));
            (end <= limit).then_some(StatementExtent {
                end,
                can_absorb_else: false,
                can_absorb_else_after_elision: false,
            })
        }
        "switch" => {
            if tokens.get(start + 1).map(|token| token.text) != Some("(") {
                return None;
            }
            let condition_close = matching_close[start + 1]?;
            let body_open = condition_close + 1;
            if tokens.get(body_open).map(|token| token.text) != Some("{") {
                return None;
            }
            let body_close = matching_close[body_open]?;
            (body_close < limit).then_some(StatementExtent {
                end: body_close + 1,
                can_absorb_else: false,
                can_absorb_else_after_elision: false,
            })
        }
        "try" => try_statement_extent(tokens, matching_close, start, limit),
        "let" | "const" | "class" | "function" | "async" | "import" | "export" => None,
        _ => simple_statement_extent(tokens, matching_close, start, limit),
    }
}

fn try_statement_extent(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    limit: usize,
) -> Option<StatementExtent> {
    let body_open = start + 1;
    if tokens.get(body_open).map(|token| token.text) != Some("{") {
        return None;
    }
    let mut cursor = matching_close[body_open]? + 1;
    let mut handled = false;
    if tokens.get(cursor).map(|token| token.text) == Some("catch") {
        handled = true;
        cursor += 1;
        if tokens.get(cursor).map(|token| token.text) == Some("(") {
            cursor = matching_close[cursor]? + 1;
        }
        if tokens.get(cursor).map(|token| token.text) != Some("{") {
            return None;
        }
        cursor = matching_close[cursor]? + 1;
    }
    if tokens.get(cursor).map(|token| token.text) == Some("finally") {
        handled = true;
        cursor += 1;
        if tokens.get(cursor).map(|token| token.text) != Some("{") {
            return None;
        }
        cursor = matching_close[cursor]? + 1;
    }
    (handled && cursor <= limit).then_some(StatementExtent {
        end: cursor,
        can_absorb_else: false,
        can_absorb_else_after_elision: false,
    })
}

fn simple_statement_extent(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    limit: usize,
) -> Option<StatementExtent> {
    let mut index = start;
    while index < limit {
        match tokens[index].text {
            "(" | "[" | "{" => {
                let close = matching_close[index]?;
                if close >= limit {
                    return None;
                }
                index = close + 1;
            }
            ";" => {
                return Some(StatementExtent {
                    end: index + 1,
                    can_absorb_else: false,
                    can_absorb_else_after_elision: false,
                });
            }
            _ => index += 1,
        }
    }
    Some(StatementExtent {
        end: limit,
        can_absorb_else: false,
        can_absorb_else_after_elision: false,
    })
}

fn control_body_block_has_following_else(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    block_open: usize,
    block_close: usize,
) -> bool {
    if tokens.get(block_close + 1).map(|token| token.text) != Some("else") {
        return false;
    }
    let Some(header_close) = block_open.checked_sub(1) else {
        return false;
    };
    if tokens.get(header_close).map(|token| token.text) != Some(")") {
        return false;
    }
    let Some(header_open) = matching_open[header_close] else {
        return false;
    };
    header_open
        .checked_sub(1)
        .is_some_and(|keyword| tokens[keyword].text == "if")
}

fn bare_early_exit(
    tokens: &[Token<'_>],
    start: usize,
    exact_end: Option<usize>,
) -> Option<EarlyExitKind> {
    let exit = match tokens.get(start)?.text {
        "return" => EarlyExitKind::Return,
        "continue" => EarlyExitKind::Continue,
        _ => return None,
    };
    // Requiring the explicit terminator excludes return values, continue
    // labels, ASI sensitivity, and every multi-statement braced arm.
    (tokens.get(start + 1)?.text == ";" && exact_end.is_none_or(|exact_end| start + 2 == exact_end))
        .then_some(exit)
}

fn block_is_loop_body(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    block_open: usize,
) -> bool {
    let Some(previous) = block_open.checked_sub(1) else {
        return false;
    };
    if tokens[previous].text == "do" {
        return true;
    }
    if tokens[previous].text != ")" {
        return false;
    }
    let Some(header_open) = matching_open[previous] else {
        return false;
    };
    header_open
        .checked_sub(1)
        .is_some_and(|keyword| matches!(tokens[keyword].text, "for" | "while"))
}

fn suffix_has_scope_changing_declaration(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut brace_depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "{" => brace_depth += 1,
            "}" => brace_depth -= 1,
            "let" | "const" | "class" | "function" if brace_depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn negate_early_exit_condition(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
) -> String {
    let raw = &source[tokens[start].start..tokens[end].start];
    let leading_not_covers_condition = tokens.get(start).map(|token| token.text) == Some("!")
        && (end == start + 2
            || (tokens.get(start + 1).map(|token| token.text) == Some("(")
                && matching_close.get(start + 1).copied().flatten() == end.checked_sub(1)));
    if leading_not_covers_condition {
        return source[tokens[start].end..tokens[end].start].to_string();
    }

    let mut depth = 0i32;
    let mut equality = None;
    let mut ambiguous = false;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "==" | "!=" | "===" | "!==" if depth == 0 => {
                if equality.replace(index).is_some() {
                    ambiguous = true;
                }
            }
            "&&" | "||" | "??" | "?" | ":" | "," if depth == 0 => ambiguous = true,
            _ => {}
        }
    }
    if !ambiguous {
        if let Some(operator) = equality {
            let flipped = match tokens[operator].text {
                "==" => "!=",
                "!=" => "==",
                "===" => "!==",
                "!==" => "===",
                _ => unreachable!(),
            };
            return format!(
                "{}{}{}",
                &source[tokens[start].start..tokens[operator].start],
                flipped,
                &source[tokens[operator].end..tokens[end].start],
            );
        }
    }
    // `!raw` is the negation only when `raw` is already a single operand of
    // unary precedence. A condition that merely *starts* with a parenthesised
    // group is not: in `(a==null)||typeof a!="object"` the group covers the
    // first operand alone, so dropping the added parentheses negates that
    // operand and leaves the disjunction — and the rest of it — standing.
    let parenthesized_condition = tokens.get(start).map(|token| token.text) == Some("(")
        && matching_close.get(start).copied().flatten() == end.checked_sub(1);
    if end == start + 1 || parenthesized_condition {
        format!("!{raw}")
    } else {
        format!("!({raw})")
    }
}

/// Collapse a guarded return and the return that follows it into a single
/// conditional return.
///
/// `if(C)return A;return B` and `return C?A:B` evaluate `C`, then exactly one
/// of `A` or `B`, and complete with the same value. Nothing else can run
/// between them: the guarded arm returns, so the following statement is
/// reached exactly when `C` is falsy.
///
/// The `if` must be a statement of the enclosing block. An `if` that is the
/// unbraced body of a loop or a labelled statement is refused, because the
/// fused return would then leave the loop on the first iteration instead of
/// continuing it. An `else` arm is refused implicitly: the token after the
/// guarded return is then `else` rather than `return`.
///
/// Longer ladders collapse one level per round, from the tail inward, so
/// `if(C1)return A;if(C2)return B;return C` reaches `return C1?A:C2?B:C`
/// through the right-associative spelling with no added parentheses.
pub(crate) fn fold_conditional_return_tails(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let mut replacements = Vec::<(usize, usize, String)>::new();

        for guard in 0..tokens.len() {
            if tokens[guard].text != "if" {
                continue;
            }
            let condition_open = guard + 1;
            if tokens.get(condition_open).map(|token| token.text) != Some("(") {
                continue;
            }
            let Some(condition_close) = matching_close[condition_open] else {
                continue;
            };

            // The guarded arm is a lone `return`, either bare or braced.
            let braced = tokens.get(condition_close + 1).map(|token| token.text) == Some("{");
            let guarded_return = if braced {
                condition_close + 2
            } else {
                condition_close + 1
            };
            if tokens.get(guarded_return).map(|token| token.text) != Some("return") {
                continue;
            }
            let Some((guarded_end, _)) = statement_terminator(&tokens, guarded_return + 1) else {
                continue;
            };
            let next_statement = if braced {
                let Some(brace_close) = matching_close[condition_close + 1] else {
                    continue;
                };
                // Exactly one statement inside the braces, with at most a
                // trailing `;`.
                if guarded_end != brace_close
                    && !(guarded_end + 1 == brace_close && tokens[guarded_end].text == ";")
                {
                    continue;
                }
                brace_close + 1
            } else {
                if tokens[guarded_end].text != ";" {
                    continue;
                }
                guarded_end + 1
            };
            // An `else` arm belongs to this `if`, so the fused return replaces
            // one whole statement with one whole statement and may sit
            // wherever the `if` did. A fall-through tail is a separate
            // statement of the enclosing block, so the `if` has to be one too:
            // as the unbraced body of a loop or a labelled statement, the
            // fused return would leave the loop instead of continuing it.
            let alternative = tokens.get(next_statement).map(|token| token.text) == Some("else");
            if !alternative
                && guard.checked_sub(1).is_some_and(|previous| {
                    !matches!(tokens[previous].text, "{" | "}" | ";" | "else")
                })
            {
                continue;
            }
            let tail_braced =
                alternative && tokens.get(next_statement + 1).map(|token| token.text) == Some("{");
            let tail_return = next_statement + usize::from(alternative) + usize::from(tail_braced);
            if tokens.get(tail_return).map(|token| token.text) != Some("return") {
                continue;
            }
            let Some((terminator, semicolon)) = statement_terminator(&tokens, tail_return + 1)
            else {
                continue;
            };
            // The replacement is an expression statement, so a braced arm's
            // own `}` disappears with it.
            let (tail_end, end) = if tail_braced {
                let Some(brace_close) = matching_close[next_statement + 1] else {
                    continue;
                };
                if terminator != brace_close
                    && !(terminator + 1 == brace_close && tokens[terminator].text == ";")
                {
                    continue;
                }
                (terminator, tokens[brace_close].end)
            } else if semicolon {
                (terminator, tokens[terminator].end)
            } else {
                (terminator, tokens[terminator].start)
            };

            // `return` followed by a line terminator is already `return;`, so
            // splicing across one would change what the argument is.
            let start = tokens[guard].start;
            if spans_line_terminator(&output[start..end]) {
                continue;
            }

            let condition = &output[tokens[condition_open].end..tokens[condition_close].start];
            let condition =
                if conditional_test_needs_grouping(&tokens[condition_open + 1..condition_close]) {
                    format!("({condition})")
                } else {
                    condition.to_string()
                };
            let guarded = conditional_return_arm(&output, &tokens, guarded_return + 1, guarded_end);
            let tail = conditional_return_arm(&output, &tokens, tail_return + 1, tail_end);
            let mut replacement = format!("return {condition}?{guarded}:{tail}");
            if semicolon && !tail_braced {
                replacement.push(';');
            }
            if replacement.len() < end - start {
                replacements.push((start, end, replacement));
            }
        }

        if replacements.is_empty() {
            return Ok((output, folded));
        }
        // Fusing the tail first lets the guard before it fuse in the next
        // round, so keep the rightmost of any overlapping pair.
        replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
        let mut retained = Vec::<(usize, usize, String)>::new();
        for replacement in replacements.into_iter().rev() {
            if retained
                .last()
                .is_none_or(|(start, _, _)| replacement.1 <= *start)
            {
                retained.push(replacement);
            }
        }
        folded += retained.len();
        for (start, end, replacement) in retained {
            output.replace_range(start..end, &replacement);
        }
    }
}

/// Fold a value-returning guard over an expression-only function/block tail.
///
/// `if(C)return A;E;return B` and `return C?A:(E,B)` perform the same
/// evaluations in the same order and return from the same paths.  Keeping
/// this separate from [`fold_conditional_return_tails`] matters for transfer
/// objectives: the comma/conditional spelling is usually shorter, but a
/// repeated statement-shaped tail can still be better input for a codec.
pub(crate) fn fold_guard_return_expression_suffixes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let mut replacement = None;

        // Prefer the rightmost candidate.  A following round can then fold
        // the newly exposed parent guard without overlapping token ranges.
        for guard in (0..tokens.len()).rev() {
            if tokens[guard].text != "if"
                || tokens.get(guard + 1).map(|token| token.text) != Some("(")
                || guard
                    .checked_sub(1)
                    .is_some_and(|previous| !matches!(tokens[previous].text, "{" | "}" | ";"))
            {
                continue;
            }
            let condition_open = guard + 1;
            let Some(condition_close) = matching_close[condition_open] else {
                continue;
            };

            let arm_start = condition_close + 1;
            let (guarded_return, guarded_end, after_guard) =
                if tokens.get(arm_start).map(|token| token.text) == Some("{") {
                    let Some(arm_close) = matching_close[arm_start] else {
                        continue;
                    };
                    let return_at = arm_start + 1;
                    if tokens.get(return_at).map(|token| token.text) != Some("return") {
                        continue;
                    }
                    let Some((return_end, _)) = statement_terminator(&tokens, return_at + 1) else {
                        continue;
                    };
                    if return_end != arm_close
                        && !(return_end + 1 == arm_close && tokens[return_end].text == ";")
                    {
                        continue;
                    }
                    (return_at, return_end, arm_close + 1)
                } else {
                    if tokens.get(arm_start).map(|token| token.text) != Some("return") {
                        continue;
                    }
                    let Some((return_end, semicolon)) =
                        statement_terminator(&tokens, arm_start + 1)
                    else {
                        continue;
                    };
                    if !semicolon {
                        continue;
                    }
                    (arm_start, return_end, return_end + 1)
                };
            if tokens.get(after_guard).map(|token| token.text) == Some("else") {
                continue;
            }

            let Some(container_open) = enclosing_block_start(&matching_close, guard) else {
                continue;
            };
            let Some(container_close) = matching_close[container_open] else {
                continue;
            };
            if after_guard >= container_close {
                continue;
            }
            let Some((prefixes, tail_return, tail_end, tail_semicolon)) =
                expression_suffix_return(&output, &tokens, after_guard, container_close)
            else {
                continue;
            };
            // The adjacent-return family already owns the empty-prefix case.
            // Requiring a real expression also prevents two late proposals
            // from spelling the same candidate under different identities.
            if prefixes.is_empty() {
                continue;
            }

            let mut end = if tail_semicolon {
                tokens[tail_end].end
            } else {
                tokens[tail_end].start
            };
            while end < tokens[container_close].start
                && output
                    .as_bytes()
                    .get(end)
                    .is_some_and(u8::is_ascii_whitespace)
            {
                end += 1;
            }
            let start = tokens[guard].start;
            if spans_line_terminator(&output[start..end]) {
                continue;
            }

            let condition = &output[tokens[condition_open].end..tokens[condition_close].start];
            let condition =
                if conditional_test_needs_grouping(&tokens[condition_open + 1..condition_close]) {
                    format!("({condition})")
                } else {
                    condition.to_string()
                };
            let guarded = conditional_return_arm(&output, &tokens, guarded_return + 1, guarded_end);
            let tail = conditional_return_arm(&output, &tokens, tail_return + 1, tail_end);
            let mut suffix = prefixes;
            suffix.push(tail);
            let suffix = format!("({})", suffix.join(","));
            let mut rewritten = format!("return {condition}?{guarded}:{suffix}");
            if tail_semicolon {
                rewritten.push(';');
            }
            if rewritten.len() < end - start {
                replacement = Some((start, end, rewritten));
                break;
            }
        }

        let Some((start, end, rewritten)) = replacement else {
            return Ok((output, folded));
        };
        output.replace_range(start..end, &rewritten);
        folded += 1;
    }
}

/// Fold expression-prefixed return arms and an expression/return tail.
///
/// `if(C){E;return A}F;return B` becomes
/// `return C?(E,A):(F,B)`. Both spellings evaluate `C` first, then exactly
/// one sequence in the same order. This complements the lone-return folds:
/// generated warning/logging calls and assignments commonly precede the
/// value returned by a branch.
pub(crate) fn fold_expression_return_branches(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let mut replacement = None;

        for guard in (0..tokens.len()).rev() {
            if tokens[guard].text != "if"
                || tokens.get(guard + 1).map(|token| token.text) != Some("(")
                || guard
                    .checked_sub(1)
                    .is_some_and(|previous| !matches!(tokens[previous].text, "{" | "}" | ";"))
            {
                continue;
            }
            let condition_open = guard + 1;
            let Some(condition_close) = matching_close[condition_open] else {
                continue;
            };
            let arm_open = condition_close + 1;
            if tokens.get(arm_open).map(|token| token.text) != Some("{") {
                continue;
            }
            let Some(arm_close) = matching_close[arm_open] else {
                continue;
            };
            if tokens.get(arm_close + 1).map(|token| token.text) == Some("else") {
                continue;
            }
            let Some((guard_prefix, guard_return, guard_end, _)) =
                expression_suffix_return(&output, &tokens, arm_open + 1, arm_close)
            else {
                continue;
            };
            // The lone-return family already owns an empty guarded prefix.
            if guard_prefix.is_empty() {
                continue;
            }

            let Some(container_open) = enclosing_block_start(&matching_close, guard) else {
                continue;
            };
            let Some(container_close) = matching_close[container_open] else {
                continue;
            };
            let after_guard = arm_close + 1;
            if after_guard >= container_close {
                continue;
            }
            let Some((tail_prefix, tail_return, tail_end, tail_semicolon)) =
                expression_suffix_return(&output, &tokens, after_guard, container_close)
            else {
                continue;
            };

            let mut end = if tail_semicolon {
                tokens[tail_end].end
            } else {
                tokens[tail_end].start
            };
            while end < tokens[container_close].start
                && output
                    .as_bytes()
                    .get(end)
                    .is_some_and(u8::is_ascii_whitespace)
            {
                end += 1;
            }
            let start = tokens[guard].start;
            if spans_line_terminator(&output[start..end]) {
                continue;
            }

            let condition = &output[tokens[condition_open].end..tokens[condition_close].start];
            let condition =
                if conditional_test_needs_grouping(&tokens[condition_open + 1..condition_close]) {
                    format!("({condition})")
                } else {
                    condition.to_string()
                };
            let guarded = return_sequence_arm(
                guard_prefix,
                conditional_return_arm(&output, &tokens, guard_return + 1, guard_end),
            );
            let tail = return_sequence_arm(
                tail_prefix,
                conditional_return_arm(&output, &tokens, tail_return + 1, tail_end),
            );
            let mut rewritten = format!("return {condition}?{guarded}:{tail}");
            if tail_semicolon {
                rewritten.push(';');
            }
            if rewritten.len() < end - start {
                replacement = Some((start, end, rewritten));
                break;
            }
        }

        let Some((start, end, rewritten)) = replacement else {
            return Ok((output, folded));
        };
        output.replace_range(start..end, &rewritten);
        folded += 1;
    }
}

fn return_sequence_arm(mut prefix: Vec<String>, returned: String) -> String {
    prefix.push(returned);
    if prefix.len() == 1 {
        prefix.pop().expect("one return expression")
    } else {
        format!("({})", prefix.join(","))
    }
}

fn expression_suffix_return(
    source: &str,
    tokens: &[Token<'_>],
    mut cursor: usize,
    block_close: usize,
) -> Option<(Vec<String>, usize, usize, bool)> {
    let mut expressions = Vec::new();
    while cursor < block_close {
        while cursor < block_close && tokens[cursor].text == ";" {
            cursor += 1;
        }
        if cursor >= block_close {
            return None;
        }
        if tokens[cursor].text == "return" {
            let (end, semicolon) = statement_terminator(tokens, cursor + 1)?;
            let mut after = end + usize::from(semicolon);
            while after < block_close && tokens[after].text == ";" {
                after += 1;
            }
            return (after == block_close).then_some((expressions, cursor, end, semicolon));
        }
        let semicolon = top_level_stop(tokens, cursor, &[";"])?;
        if semicolon >= block_close {
            return None;
        }
        let mut statement = expression_statement_texts(source, tokens, cursor, semicolon)?;
        if statement.len() != 1 {
            return None;
        }
        expressions.push(statement.pop()?);
        cursor = semicolon + 1;
    }
    None
}

/// Move the expression-only suffix of a block into its terminal return.
///
/// `E1;E2;return V` becomes `return E1,E2,V`. This is deliberately a late
/// proposal rather than part of the emitter's broad comma-expression option:
/// raw size is normally unchanged, while gzip and Brotli may prefer either
/// token topology. The configured whole-artifact codec makes that decision
/// without also committing to commas everywhere else in the bundle.
pub(crate) fn fold_expression_suffix_returns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let replacements = expression_suffix_return_rewrites(source)?;
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

/// Return one candidate per independently selectable suffix-to-return rewrite.
///
/// Dictionary codecs are contextual: applying every raw-neutral sequence fold
/// can lose even when a few individual sites win.  Keeping the rewrite ranges
/// here, beside the grammar proof that produced them, lets the compiler score
/// those sites against the configured whole-artifact objective without trying
/// to reconstruct edits from an all-at-once textual diff.
pub(crate) fn expression_suffix_return_variants(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    let replacements = expression_suffix_return_rewrites(source)?;
    let mut variants = Vec::with_capacity(replacements.len());
    for (start, end, replacement) in replacements {
        let mut variant = source.to_string();
        variant.replace_range(start..end, &replacement);
        if !variants.contains(&variant) {
            variants.push(variant);
        }
    }
    Ok(variants)
}

fn expression_suffix_return_rewrites(
    source: &str,
) -> Result<Vec<(usize, usize, String)>, JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();

    for return_at in 0..tokens.len() {
        if tokens[return_at].text != "return" {
            continue;
        }
        let Some(block_open) = enclosing_block_start(&matching_close, return_at) else {
            continue;
        };
        let Some(block_close) = matching_close[block_open] else {
            continue;
        };
        let Some((return_end, semicolon)) = statement_terminator(&tokens, return_at + 1) else {
            continue;
        };
        let mut after_return = return_end + usize::from(semicolon);
        while after_return < block_close && tokens[after_return].text == ";" {
            after_return += 1;
        }
        if after_return != block_close || return_at + 1 >= return_end {
            continue;
        }
        if spans_line_terminator(&source[tokens[return_at].end..tokens[return_at + 1].start]) {
            continue;
        }

        let mut suffix_start = None;
        for start in block_open + 1..return_at {
            if tokens[start].text == ";"
                || enclosing_block_start(&matching_close, start) != Some(block_open)
                || (start != block_open + 1 && !matches!(tokens[start - 1].text, ";" | "}"))
            {
                continue;
            }
            // A leading string expression can be a directive prologue. Leave
            // it outside the sequence so strictness and other directives keep
            // their grammar-level meaning.
            if start == block_open + 1
                && matches!(tokens[start].kind, TokenKind::String | TokenKind::Template)
            {
                continue;
            }
            if expression_statement_texts(source, &tokens, start, return_at).is_some() {
                suffix_start = Some(start);
                break;
            }
        }
        let Some(suffix_start) = suffix_start else {
            continue;
        };
        let Some(mut expressions) =
            expression_statement_texts(source, &tokens, suffix_start, return_at)
        else {
            continue;
        };
        expressions.push(conditional_return_arm(
            source,
            &tokens,
            return_at + 1,
            return_end,
        ));
        let mut rewritten = format!("return {}", expressions.join(","));
        let end = if semicolon {
            rewritten.push(';');
            tokens[return_end].end
        } else {
            tokens[return_end].start
        };
        replacements.push((tokens[suffix_start].start, end, rewritten));
    }
    Ok(replacements)
}

/// Spell an arrow whose body is a lone `return` as a concise body.
///
/// `=>{return X}` and `=>X` produce the same value. A body that begins with
/// `{` would be read as a block, and a top-level comma binds looser than the
/// arrow body, so both are parenthesized.
pub(crate) fn fold_single_return_arrow_bodies(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();

    for body_open in 1..tokens.len() {
        if tokens[body_open].text != "{" || tokens[body_open - 1].text != "=>" {
            continue;
        }
        let (Some(body_close), Some(value)) =
            (matching_close[body_open], tokens.get(body_open + 1))
        else {
            continue;
        };
        if value.text != "return" {
            continue;
        }
        let Some((terminator, _)) = statement_terminator(&tokens, body_open + 2) else {
            continue;
        };
        if terminator != body_close
            && !(terminator + 1 == body_close && tokens[terminator].text == ";")
        {
            continue;
        }
        // `=>{}` already returns `undefined` in fewer bytes than `=>void 0`.
        if body_open + 2 >= terminator {
            continue;
        }
        let start = tokens[body_open].start;
        let end = tokens[body_close].end;
        if spans_line_terminator(&source[start..end]) {
            continue;
        }
        let text = &source[tokens[body_open + 2].start..tokens[terminator - 1].end];
        let replacement = if tokens[body_open + 2].text == "{"
            || expression_has_top_level_token(&tokens[body_open + 2..terminator], ",")
        {
            format!("({text})")
        } else {
            text.to_string()
        };
        if replacement.len() < end - start {
            replacements.push((start, end, replacement));
        }
    }

    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let retained = non_overlapping_ranges(replacements);
    let count = retained.len();
    let mut output = source.to_string();
    for (start, end, replacement) in retained.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok((output, count))
}

/// `&&`'s own tightness in [`binary_operator_tightness`].
const AND_TIGHTNESS: u8 = 4;

/// Parenthesise an operand of `&&` that binds looser than `&&` does.
///
/// This is the precedence relation, not a list of operators to remember. The
/// list it replaces named plain `=` but none of `+=`, `-=`, `*=`, … , so
/// `if(h>0)a+=.25;` folded to `h>0&&a+=.25` — an assignment to an rvalue, which
/// no JavaScript engine accepts. Asking the precedence table instead makes the
/// whole family correct at once, including the assignment operators, `=>`, and
/// anything added to the table later.
fn wrap_and_operand(expr: &str, tokens: &[Token<'_>], start: usize, end: usize) -> String {
    if operand_binds_looser_than_and(tokens, start, end)
        || leading_bare_assignment(tokens, start, end)
    {
        format!("({expr})")
    } else {
        expr.to_string()
    }
}

fn operand_binds_looser_than_and(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            // An arrow body would swallow everything to its right, so the
            // function has to be grouped even though it is not an operator.
            "=>" if depth == 0 => return true,
            operator if depth == 0 => {
                if crate::js_peephole::rewrite::binary_operator_tightness(operator)
                    .is_some_and(|tightness| tightness < AND_TIGHTNESS)
                {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn leading_bare_assignment(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut index = start;
    while index < end && tokens[index].text == "(" {
        index += 1;
    }
    index + 1 < end && tokens[index].kind == TokenKind::Identifier && tokens[index + 1].text == "="
}

fn single_expression_statement(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            ";" if depth == 0 && index + 1 < end => return false,
            "var" | "let" | "const" | "return" | "if" | "for" | "while" | "function" | "throw"
            | "continue" | "break" | "try" | "switch" | "class"
                if depth == 0 =>
            {
                return false;
            }
            _ => {}
        }
    }
    depth == 0
}

/// Forwarding `name=rhs` into a later use replays the rhs at the use site, so
/// every value the rhs reads must be provably unchanged across the gap. A
/// bare identifier or literal only needs its identifiers unassigned; a rhs
/// with member reads or calls can be invalidated through aliases (e.g.
/// `m=t.length` before a `t.splice(...)`), so any reappearance of its
/// identifiers or any call in the gap forfeits the fold.
fn rhs_holds_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    rhs_from: usize,
    stop: usize,
    use_at: usize,
) -> bool {
    let impure = tokens[rhs_from..stop]
        .iter()
        .any(|token| matches!(token.text, "." | "[" | "("));
    for gap in stop + 1..use_at {
        if impure && tokens[gap].text == "(" {
            return false;
        }
    }
    !crate::js_peephole::liveness::source_receiver_overwritten_between(
        tokens,
        matching_close,
        rhs_from,
        stop,
        stop + 1,
        use_at,
    )
}

fn assign_is_statement_level(tokens: &[Token<'_>], body_open: usize, name_at: usize) -> bool {
    let mut paren = 0i32;
    let mut brace = 0i32;
    for token in &tokens[body_open + 1..name_at] {
        match token.text {
            "(" | "[" => paren += 1,
            ")" | "]" => paren -= 1,
            "{" => brace += 1,
            "}" => brace -= 1,
            _ => {}
        }
    }
    paren == 0 && brace == 0
}

fn if_without_else(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    if_at: usize,
) -> Option<(usize, usize, usize, usize)> {
    if tokens[if_at].text != "if" || tokens.get(if_at + 1).map(|token| token.text) != Some("(") {
        return None;
    }
    let cond_close = matching_close.get(if_at + 1).copied().flatten()?;
    if tokens.get(cond_close + 1).map(|token| token.text) != Some("{") {
        return None;
    }
    let body_close = matching_close.get(cond_close + 1).copied().flatten()?;
    if tokens.get(body_close + 1).map(|token| token.text) == Some("else") {
        return None;
    }
    Some((if_at + 1, cond_close, cond_close + 1, body_close))
}

pub(crate) fn fold_single_use_if_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for if_at in 0..tokens.len() {
        let Some((_, _, body_open, body_close)) = if_without_else(&tokens, &matching_close, if_at)
        else {
            continue;
        };
        let mut cursor = body_close;
        while cursor > body_open + 1 {
            cursor -= 1;
            let (name_at, rhs_from, after_rhs) = if tokens[cursor].kind == TokenKind::Identifier
                && tokens.get(cursor + 1).map(|token| token.text) == Some("=")
                && tokens.get(cursor + 2).map(|token| token.text) != Some("=")
            {
                let after_rhs = if cursor > 0 && matches!(tokens[cursor - 1].text, "var" | "let") {
                    cursor - 1
                } else {
                    cursor
                };
                (cursor, cursor + 2, after_rhs)
            } else {
                continue;
            };
            if !assign_is_statement_level(&tokens, body_open, name_at) {
                continue;
            }
            // Only a statement-opening assignment can be deleted whole: a
            // chained `s=name=rhs` would leave `s=` dangling, a comma
            // sequence would leave a trailing comma, and a brace-less
            // control body (`if(c)name=rhs;`) only executes conditionally.
            if !matches!(tokens[after_rhs - 1].text, ";" | "{" | "}") {
                continue;
            }
            let Some(stop) = top_level_stop(&tokens, rhs_from, &[",", ";", "}"]) else {
                continue;
            };
            if stop >= body_close {
                continue;
            }
            let name = tokens[name_at].text;
            let (uses, nested_use) = collect_same_scope_name_uses(
                &tokens,
                &matching_close,
                name,
                stop + 1,
                body_close,
                name_at,
            );
            if nested_use || uses.len() != 1 || name_use_is_mutated(&tokens, uses[0]) {
                continue;
            }
            let after_if = body_close + 1;
            let scope_end = enclosing_function_span(&tokens, &matching_close, if_at)
                .map(|(_, close)| close)
                .unwrap_or(tokens.len());
            if identifier_occurs(&tokens, after_if, scope_end, name) {
                continue;
            }
            let rhs = &source[tokens[rhs_from].start..tokens[stop].start];
            let use_at = uses[0];
            if !rhs_holds_between(&tokens, &matching_close, rhs_from, stop, use_at) {
                continue;
            }
            let assign_from = if matches!(tokens[after_rhs].text, "var" | "let") {
                tokens[after_rhs].start
            } else {
                tokens[name_at].start
            };
            let assign_to = if matches!(tokens[stop].text, "," | ";") {
                tokens[stop].end
            } else {
                tokens[stop].start
            };
            replacements.push((
                tokens[use_at].start,
                tokens[use_at].end,
                wrap_substituted_expression(&tokens, use_at, rhs),
            ));
            replacements.push((assign_from, assign_to, String::new()));
            break;
        }
    }
    Ok(apply_token_rewrites(source, replacements))
}

/// Terser `if_return` + `sequences`: `if(C){E;return V}` → `if(C)return E,V`.
///
/// The suffix-to-return rewrite is raw-neutral as a block tail, so it stays
/// search-only. The same comma under an `if` also drops the braces, which is
/// always shorter and matches the official Terser micromark spelling.
pub(crate) fn fold_if_prefixed_returns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let mut replacements = Vec::<(usize, usize, String)>::new();

        for if_at in 0..tokens.len() {
            if tokens[if_at].text != "if"
                || tokens.get(if_at + 1).map(|token| token.text) != Some("(")
            {
                continue;
            }
            let cond_open = if_at + 1;
            let Some(cond_close) = matching_close[cond_open] else {
                continue;
            };
            if tokens.get(cond_close + 1).map(|token| token.text) != Some("{") {
                continue;
            }
            let body_open = cond_close + 1;
            let Some(body_close) = matching_close[body_open] else {
                continue;
            };
            let Some((prefix, return_at, return_end, _)) =
                expression_suffix_return(&output, &tokens, body_open + 1, body_close)
            else {
                continue;
            };
            if return_at + 1 >= return_end && !prefix.is_empty() {
                continue;
            }
            let start = tokens[if_at].start;
            let end = tokens[body_close].end;
            if spans_line_terminator(&output[start..end]) {
                continue;
            }
            let cond = &output[tokens[cond_open].end..tokens[cond_close].start];
            let value = if return_at + 1 >= return_end {
                String::new()
            } else {
                output[tokens[return_at + 1].start..tokens[return_end - 1].end].to_string()
            };
            let mut expressions = prefix;
            if !value.is_empty() {
                expressions.push(value);
            }
            let mut rewritten = if expressions.is_empty() {
                format!("if({cond})return")
            } else {
                format!("if({cond})return {}", expressions.join(","))
            };
            let next = tokens.get(body_close + 1).map(|token| token.text);
            if !matches!(next, Some("}") | Some(";") | None) {
                rewritten.push(';');
            }
            if rewritten.len() < end - start {
                replacements.push((start, end, rewritten));
            }
        }

        if replacements.is_empty() {
            return Ok((output, folded));
        }
        let retained = non_overlapping_ranges(replacements);
        folded += retained.len();
        for (start, end, replacement) in retained.into_iter().rev() {
            output.replace_range(start..end, &replacement);
        }
    }
}

/// Terser `conditionals`: `if(C)if(D)S` and `if(C){if(D)S}` → `if(C&&D)S`
/// when neither `if` has an `else`.
pub(crate) fn fold_nested_unguarded_ifs(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let mut replacements = Vec::<(usize, usize, String)>::new();

        for if_at in 0..tokens.len() {
            if tokens[if_at].text != "if"
                || tokens.get(if_at + 1).map(|token| token.text) != Some("(")
            {
                continue;
            }
            let outer_cond_open = if_at + 1;
            let Some(outer_cond_close) = matching_close[outer_cond_open] else {
                continue;
            };
            let after_outer_cond = outer_cond_close + 1;
            let braced = tokens.get(after_outer_cond).map(|token| token.text) == Some("{");
            let (inner_if, replace_end, next) = if braced {
                let Some(body_close) = matching_close[after_outer_cond] else {
                    continue;
                };
                if tokens.get(body_close + 1).map(|token| token.text) == Some("else") {
                    continue;
                }
                let mut cursor = after_outer_cond + 1;
                while cursor < body_close && tokens[cursor].text == ";" {
                    cursor += 1;
                }
                if tokens.get(cursor).map(|token| token.text) != Some("if") {
                    continue;
                }
                let Some((inner_end, inner_else)) =
                    skip_if_statement(&tokens, &matching_close, cursor)
                else {
                    continue;
                };
                if inner_else {
                    continue;
                }
                let mut rest = inner_end;
                while rest < body_close && tokens[rest].text == ";" {
                    rest += 1;
                }
                if rest != body_close {
                    continue;
                }
                (
                    cursor,
                    tokens[body_close].end,
                    tokens.get(body_close + 1).map(|token| token.text),
                )
            } else if tokens.get(after_outer_cond).map(|token| token.text) == Some("if") {
                let Some((inner_end, inner_else)) =
                    skip_if_statement(&tokens, &matching_close, after_outer_cond)
                else {
                    continue;
                };
                if inner_else {
                    continue;
                }
                (
                    after_outer_cond,
                    tokens[inner_end - 1].end,
                    tokens.get(inner_end).map(|token| token.text),
                )
            } else {
                continue;
            };
            if tokens.get(inner_if + 1).map(|token| token.text) != Some("(") {
                continue;
            }
            let Some(inner_cond_close) = matching_close[inner_if + 1] else {
                continue;
            };
            let Some((inner_end, _)) = skip_if_statement(&tokens, &matching_close, inner_if) else {
                continue;
            };
            let start = tokens[if_at].start;
            if spans_line_terminator(&output[start..replace_end]) {
                continue;
            }
            let left = wrap_and_operand(
                &output[tokens[outer_cond_open].end..tokens[outer_cond_close].start],
                &tokens,
                outer_cond_open + 1,
                outer_cond_close,
            );
            let right = wrap_and_operand(
                &output[tokens[inner_if + 2].start..tokens[inner_cond_close].start],
                &tokens,
                inner_if + 2,
                inner_cond_close,
            );
            let body = &output[tokens[inner_cond_close + 1].start..tokens[inner_end - 1].end];
            let mut rewritten = format!("if({left}&&{right}){body}");
            if !matches!(next, Some("}") | Some("else") | Some(";") | None)
                && !rewritten.ends_with(';')
                && !rewritten.ends_with('}')
            {
                rewritten.push(';');
            }
            if rewritten.len() < replace_end - start {
                replacements.push((start, replace_end, rewritten));
            }
        }

        if replacements.is_empty() {
            return Ok((output, folded));
        }
        let retained = non_overlapping_ranges(replacements);
        folded += retained.len();
        for (start, end, replacement) in retained.into_iter().rev() {
            output.replace_range(start..end, &replacement);
        }
    }
}

fn skip_if_statement(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    if_at: usize,
) -> Option<(usize, bool)> {
    if tokens.get(if_at).map(|token| token.text) != Some("if")
        || tokens.get(if_at + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let cond_close = matching_close.get(if_at + 1).copied().flatten()?;
    let after_body = skip_control_body(tokens, matching_close, cond_close + 1)?;
    if tokens.get(after_body).map(|token| token.text) == Some("else") {
        let after_else = skip_control_body(tokens, matching_close, after_body + 1)?;
        Some((after_else, true))
    } else {
        Some((after_body, false))
    }
}

fn skip_control_body(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
) -> Option<usize> {
    match tokens.get(start).map(|token| token.text) {
        Some("{") => matching_close
            .get(start)
            .copied()
            .flatten()
            .map(|close| close + 1),
        Some("if") => skip_if_statement(tokens, matching_close, start).map(|(end, _)| end),
        Some("return" | "throw" | "break" | "continue" | "debugger" | "var" | "let" | "const") => {
            let (end, semicolon) = statement_terminator(tokens, start + 1)?;
            Some(if semicolon { end + 1 } else { end })
        }
        Some(_) => {
            let (end, semicolon) = statement_terminator(tokens, start)?;
            Some(if semicolon { end + 1 } else { end })
        }
        None => None,
    }
}

pub(crate) fn fold_if_expression_to_and(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for if_at in 0..tokens.len() {
        let Some((cond_open, cond_close, body_open, body_close)) =
            if_without_else(&tokens, &matching_close, if_at)
        else {
            continue;
        };
        let mut start = body_open + 1;
        let mut end = body_close;
        while start < end && tokens[start].text == ";" {
            start += 1;
        }
        while end > start && tokens[end - 1].text == ";" {
            end -= 1;
        }
        if start >= end || !single_expression_statement(&tokens, start, end) {
            continue;
        }
        let cond = wrap_and_operand(
            &source[tokens[cond_open + 1].start..tokens[cond_close].start],
            &tokens,
            cond_open + 1,
            cond_close,
        );
        let body = wrap_and_operand(
            &source[tokens[start].start..tokens[end - 1].end],
            &tokens,
            start,
            end,
        );
        replacements.push((
            tokens[if_at].start,
            tokens[body_close].end,
            format!("{cond}&&{body};"),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_try_if_return_alternatives(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for try_at in 0..tokens.len() {
        if tokens[try_at].text != "try"
            || tokens.get(try_at + 1).map(|token| token.text) != Some("{")
        {
            continue;
        }
        let Some(try_close) = matching_close.get(try_at + 1).copied().flatten() else {
            continue;
        };
        if tokens.get(try_close + 1).map(|token| token.text) != Some("catch") {
            continue;
        }
        if tokens.get(try_at + 2).map(|token| token.text) != Some("if")
            || tokens.get(try_at + 3).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(outer_cond_close) = matching_close.get(try_at + 3).copied().flatten() else {
            continue;
        };
        if tokens.get(outer_cond_close + 1).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(outer_body_close) = matching_close.get(outer_cond_close + 1).copied().flatten()
        else {
            continue;
        };
        let mut cursor = outer_cond_close + 2;
        if !matches!(
            tokens.get(cursor).map(|token| token.text),
            Some("var") | Some("let")
        ) || tokens
            .get(cursor + 1)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 2).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let name = tokens[cursor + 1].text;
        let Some(first_rhs_end) = top_level_stop(&tokens, cursor + 3, &[";"]) else {
            continue;
        };
        let first_rhs = &source[tokens[cursor + 3].start..tokens[first_rhs_end].start];
        cursor = first_rhs_end + 1;
        if tokens.get(cursor).map(|token| token.text) != Some("if")
            || tokens.get(cursor + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(first_pred_close) = matching_close.get(cursor + 1).copied().flatten() else {
            continue;
        };
        if tokens
            .get(cursor + 2)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 3).map(|token| token.text) != Some("(")
            || tokens.get(cursor + 4).map(|token| token.text) != Some(name)
        {
            continue;
        }
        let Some(pred_call_close) = matching_close.get(cursor + 3).copied().flatten() else {
            continue;
        };
        if pred_call_close != cursor + 5 || first_pred_close != pred_call_close + 1 {
            continue;
        }
        let pred = tokens[cursor + 2].text;
        if tokens.get(first_pred_close + 1).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(first_then_close) = matching_close.get(first_pred_close + 1).copied().flatten()
        else {
            continue;
        };
        let first_then = source[tokens[first_pred_close + 2].start..tokens[first_then_close].start]
            .trim()
            .trim_end_matches(';')
            .trim_end_matches("return")
            .trim_end_matches(';')
            .trim();
        if first_then.is_empty() || first_then.contains("return") {
            continue;
        }
        cursor = first_then_close + 1;
        if tokens.get(cursor).map(|token| token.text) != Some(name)
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let Some(second_rhs_end) = top_level_stop(&tokens, cursor + 2, &[";"]) else {
            continue;
        };
        let second_rhs = &source[tokens[cursor + 2].start..tokens[second_rhs_end].start];
        cursor = second_rhs_end + 1;
        if tokens.get(cursor).map(|token| token.text) != Some("if")
            || tokens.get(cursor + 1).map(|token| token.text) != Some("(")
            || tokens.get(cursor + 2).map(|token| token.text) != Some(pred)
            || tokens.get(cursor + 3).map(|token| token.text) != Some("(")
            || tokens.get(cursor + 4).map(|token| token.text) != Some(name)
        {
            continue;
        }
        let Some(second_pred_close) = matching_close.get(cursor + 1).copied().flatten() else {
            continue;
        };
        if tokens.get(second_pred_close + 1).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(second_then_close) = matching_close.get(second_pred_close + 1).copied().flatten()
        else {
            continue;
        };
        if second_then_close + 1 != outer_body_close {
            continue;
        }
        let second_then = source
            [tokens[second_pred_close + 2].start..tokens[second_then_close].start]
            .trim()
            .trim_end_matches(';')
            .trim_end_matches("return")
            .trim_end_matches(';')
            .trim();
        if second_then.is_empty() || second_then.contains("return") {
            continue;
        }
        let fallback = source[tokens[outer_body_close + 1].start..tokens[try_close].start]
            .trim()
            .trim_end_matches(';');
        if fallback.is_empty() {
            continue;
        }
        let outer_cond = &source[tokens[try_at + 4].start..tokens[outer_cond_close].start];
        replacements.push((
            tokens[try_at + 2].start,
            tokens[try_close].start,
            format!(
                "var {name};{outer_cond}&&{pred}({name}={first_rhs})?{first_then}:{outer_cond}&&{pred}({name}={second_rhs})?{second_then}:{fallback}"
            ),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}
