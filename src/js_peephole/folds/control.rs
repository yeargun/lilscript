use crate::js_peephole::rewrite::{
    apply_token_rewrites, conditional_test_needs_grouping, expression_has_top_level_token,
    identifier_occurs, non_overlapping_ranges, single_console_log_argument, top_level_stop,
};
use crate::js_peephole::scope::{
    collect_same_scope_name_uses, enclosing_function_span, name_use_is_mutated,
};
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
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
    let mut index = start;
    let mut exprs = Vec::new();
    while index < end {
        if tokens[index].text == ";" {
            index += 1;
            continue;
        }
        if matches!(
            tokens[index].text,
            "if" | "for"
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
            let cond =
                wrap_and_operand(cond_raw, &tokens, open + 3, cond_close, AND_LHS_NEEDS_WRAP);
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

const AND_LHS_NEEDS_WRAP: &[&str] = &["?", "=", ",", "||", "??"];
const AND_RHS_NEEDS_WRAP: &[&str] = &["=", "?", ",", "||", "??"];

fn wrap_and_operand(
    expr: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
    needles: &[&str],
) -> String {
    if top_level_token_in(tokens, start, end, needles) {
        format!("({expr})")
    } else {
        expr.to_string()
    }
}

fn top_level_token_in(tokens: &[Token<'_>], start: usize, end: usize, needles: &[&str]) -> bool {
    let mut depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            text if depth == 0 && needles.contains(&text) => return true,
            _ => {}
        }
    }
    false
}

fn single_expression_statement(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for index in start..end {
        match tokens[index].text {
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
            // A `)`, `else`, or `do` directly before the assignment makes it
            // the brace-less dependent statement of a control header (e.g.
            // `if(c)name=rhs;`), so it only executes conditionally and its
            // value cannot be forwarded into the following use.
            if after_rhs > body_open + 1
                && matches!(tokens[after_rhs - 1].text, ")" | "else" | "do")
            {
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
            let assign_from = if matches!(tokens[after_rhs].text, "var" | "let") {
                tokens[after_rhs].start
            } else {
                tokens[name_at].start
            };
            let assign_to = if tokens[stop].text == "," {
                tokens[stop].end
            } else if tokens[stop].text == ";" {
                tokens[stop].end
            } else {
                tokens[stop].start
            };
            replacements.push((tokens[use_at].start, tokens[use_at].end, rhs.to_string()));
            replacements.push((assign_from, assign_to, String::new()));
            break;
        }
    }
    Ok(apply_token_rewrites(source, replacements))
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
            AND_LHS_NEEDS_WRAP,
        );
        let body = wrap_and_operand(
            &source[tokens[start].start..tokens[end - 1].end],
            &tokens,
            start,
            end,
            AND_RHS_NEEDS_WRAP,
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
