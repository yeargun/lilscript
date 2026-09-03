use crate::js_peephole::binding::{BindingResolution, Resolution};
use crate::js_peephole::rewrite::{
    apply_token_rewrites, assign_is_in_declaration, expression_has_top_level_token,
    identifier_occurs, is_property_identifier, is_statement_boundary, paren_depth_at,
    parse_bare_assign, rewrite_identifier_span, top_level_stop,
};
use crate::js_peephole::scope::{
    enclosing_block_start, enclosing_function_span, name_is_arguments_length_copy,
    name_is_declared_in_any_enclosing_scope, name_is_declared_in_visible_scope,
    name_is_nonnegative_length_copy, skip_nested_loop_or_function,
};
use crate::js_peephole::token::{lex, matching_closers, matching_openers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;
use std::collections::HashSet;

pub(crate) fn fold_index_postfix_updates(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let depths = paren_depth_at(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if depths[cursor] != 0 || tokens[cursor].kind != TokenKind::Identifier {
            cursor += 1;
            continue;
        }
        if tokens.get(cursor + 1).map(|token| token.text) == Some("[")
            && tokens
                .get(cursor + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 3).map(|token| token.text) == Some("]")
            && tokens.get(cursor + 4).map(|token| token.text) == Some("=")
        {
            let index = tokens[cursor + 2].text;
            let Some(stop) = top_level_stop(&tokens, cursor + 5, &[",", ";"]) else {
                cursor += 1;
                continue;
            };
            if identifier_occurs(&tokens, cursor + 5, stop, index) {
                cursor += 1;
                continue;
            }
            if tokens.get(stop + 1).map(|token| token.text) == Some(index)
                && tokens.get(stop + 2).map(|token| token.text) == Some("++")
            {
                let rhs = &source[tokens[cursor + 5].start..tokens[stop].start];
                let end = tokens[stop + 2].end;
                replacements.push((
                    tokens[cursor].start,
                    end,
                    format!("{}[{index}++]={rhs}", tokens[cursor].text),
                ));
                cursor = stop + 3;
                continue;
            }
        }
        if tokens.get(cursor + 1).map(|token| token.text) == Some("=")
            && tokens.get(cursor + 2).is_some_and(|token| {
                token.kind == TokenKind::Identifier || token.kind == TokenKind::Number
            })
            && tokens.get(cursor + 3).map(|token| token.text) == Some("+")
            && tokens.get(cursor + 4).map(|token| token.text) == Some("1")
            && tokens.get(cursor + 5).map(|token| token.text) == Some(";")
            && tokens.get(cursor + 2).map(|token| token.text) != Some(tokens[cursor].text)
        {
            let temp = tokens[cursor].text;
            let index = tokens[cursor + 2].text;
            if tokens
                .get(cursor + 6)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
                && tokens.get(cursor + 7).map(|token| token.text) == Some("[")
                && tokens.get(cursor + 8).map(|token| token.text) == Some(index)
                && tokens.get(cursor + 9).map(|token| token.text) == Some("]")
                && tokens.get(cursor + 10).map(|token| token.text) == Some("=")
            {
                if let Some(stop) = top_level_stop(&tokens, cursor + 11, &[";"]) {
                    if !identifier_occurs(&tokens, cursor + 11, stop, index)
                        && !identifier_occurs(&tokens, cursor + 11, stop, temp)
                    {
                        let mut restore = stop + 1;
                        if tokens
                            .get(restore)
                            .is_some_and(|token| token.kind == TokenKind::Identifier)
                            && tokens.get(restore).map(|token| token.text) != Some(index)
                            && tokens.get(restore).map(|token| token.text) != Some(temp)
                            && matches!(
                                tokens.get(restore + 1).map(|token| token.text),
                                Some("++") | Some("+=")
                            )
                        {
                            restore = if tokens.get(restore + 1).map(|token| token.text)
                                == Some("++")
                            {
                                restore + 3
                            } else if tokens.get(restore + 2).map(|token| token.text) == Some("1")
                                && tokens.get(restore + 3).map(|token| token.text) == Some(";")
                            {
                                restore + 4
                            } else {
                                restore
                            };
                        }
                        if tokens.get(restore).map(|token| token.text) == Some(index)
                            && tokens.get(restore + 1).map(|token| token.text) == Some("=")
                            && tokens.get(restore + 2).map(|token| token.text) == Some(temp)
                        {
                            let object = tokens[cursor + 6].text;
                            let rhs = &source[tokens[cursor + 11].start..tokens[stop].start];
                            let between = if restore > stop + 1 {
                                &source[tokens[stop + 1].start..tokens[restore].start]
                            } else {
                                ""
                            };
                            replacements.push((
                                tokens[cursor].start,
                                tokens[restore + 2].end,
                                format!("{object}[{index}++]={rhs};{between}"),
                            ));
                            cursor = restore + 3;
                            continue;
                        }
                    }
                }
            }
        }
        if tokens.get(cursor + 1).map(|token| token.text) == Some("=")
            && tokens
                .get(cursor + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 3).map(|token| token.text) == Some("[")
            && tokens
                .get(cursor + 4)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 5).map(|token| token.text) == Some("]")
        {
            let index = tokens[cursor + 4].text;
            let Some(stop) = top_level_stop(&tokens, cursor + 6, &[",", ";"]) else {
                cursor += 1;
                continue;
            };
            if stop != cursor + 6 {
                cursor += 1;
                continue;
            }
            if tokens.get(stop + 1).map(|token| token.text) == Some(index)
                && tokens.get(stop + 2).map(|token| token.text) == Some("++")
            {
                // An adjacent-expression fold can spell the update as
                // `var item=list[index];index++,receiver.field=item.field`.
                // Folding through the update while retaining that comma would
                // turn the member assignment into an invalid declarator:
                // `var item=list[index++],receiver.field=item.field`.
                // Consume the comma and restore a statement boundary when the
                // indexed assignment belongs to a declaration.
                let declaration_sequence = assign_is_in_declaration(&tokens, cursor + 1)
                    && tokens.get(stop + 3).map(|token| token.text) == Some(",");
                let end = if declaration_sequence {
                    tokens[stop + 3].end
                } else {
                    tokens[stop + 2].end
                };
                replacements.push((
                    tokens[cursor].start,
                    end,
                    format!(
                        "{}={}[{index}++]{}",
                        tokens[cursor].text,
                        tokens[cursor + 2].text,
                        if declaration_sequence { ";" } else { "" }
                    ),
                ));
                cursor = stop + 3;
                continue;
            }
        }
        cursor += 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_unit_counter_updates(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        let statement_update = tokens[cursor].kind == TokenKind::Identifier
            && matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | ")" | ","
            )
            && matches!(
                tokens.get(cursor + 3).map(|token| token.text),
                Some(";") | Some("}") | Some(")") | Some(",") | None
            );
        if statement_update
            && tokens.get(cursor + 1).map(|token| token.text) == Some("+=")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("1")
        {
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 2].end,
                format!("{}++", tokens[cursor].text),
            ));
            cursor += 3;
            continue;
        }
        if statement_update
            && tokens.get(cursor + 1).map(|token| token.text) == Some("-=")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("1")
        {
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 2].end,
                format!("{}--", tokens[cursor].text),
            ));
            cursor += 3;
            continue;
        }
        let self_update = tokens[cursor].kind == TokenKind::Identifier
            && matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | ")" | ","
            )
            && tokens.get(cursor + 1).map(|token| token.text) == Some("=")
            && tokens.get(cursor + 2).map(|token| token.text) == Some(tokens[cursor].text)
            && matches!(
                tokens.get(cursor + 3).map(|token| token.text),
                Some("+") | Some("-")
            )
            && tokens.get(cursor + 4).map(|token| token.text) == Some("1")
            && matches!(
                tokens.get(cursor + 5).map(|token| token.text),
                Some(";") | Some("}") | Some(")") | Some(",") | None
            );
        if self_update {
            let op = if tokens[cursor + 3].text == "+" {
                "++"
            } else {
                "--"
            };
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 4].end,
                format!("{}{op}", tokens[cursor].text),
            ));
            cursor += 5;
            continue;
        }
        let int32_self_update = tokens[cursor].kind == TokenKind::Identifier
            && matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | ")" | ","
            )
            && tokens.get(cursor + 1).map(|token| token.text) == Some("=")
            && tokens.get(cursor + 2).map(|token| token.text) == Some(tokens[cursor].text)
            && matches!(
                tokens.get(cursor + 3).map(|token| token.text),
                Some("+") | Some("-")
            )
            && tokens.get(cursor + 4).map(|token| token.text) == Some("1")
            && tokens.get(cursor + 5).map(|token| token.text) == Some("|")
            && tokens.get(cursor + 6).map(|token| token.text) == Some("0")
            && matches!(
                tokens.get(cursor + 7).map(|token| token.text),
                Some(";") | Some("}") | Some(")") | Some(",") | None
            );
        if int32_self_update {
            let op = if tokens[cursor + 3].text == "+" {
                "++"
            } else {
                "--"
            };
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 6].end,
                format!("{}{op}", tokens[cursor].text),
            ));
            cursor += 7;
            continue;
        }
        if let Some((end, rewritten)) = member_unit_int32_update(&tokens, cursor) {
            replacements.push((tokens[cursor].start, tokens[end].end, rewritten));
            cursor = end + 1;
            continue;
        }
        cursor += 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

/// Preserve the value of a self-update used as an expression while shortening
/// its spelling. Unlike postfix `x++`, `x+=1` returns the newly assigned value
/// just like `x=x+1`.
pub(crate) fn fold_expression_self_assignments(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        let expression_self_update = tokens[cursor].kind == TokenKind::Identifier
            && !crate::js_peephole::rewrite::is_property_identifier(&tokens, cursor)
            && tokens.get(cursor + 1).map(|token| token.text) == Some("=")
            && tokens.get(cursor + 2).map(|token| token.text) == Some(tokens[cursor].text)
            && matches!(
                tokens.get(cursor + 3).map(|token| token.text),
                Some("+") | Some("-")
            )
            && tokens
                .get(cursor + 4)
                .is_some_and(|token| matches!(token.kind, TokenKind::Number | TokenKind::String))
            && matches!(
                tokens.get(cursor + 5).map(|token| token.text),
                Some(";") | Some("}") | Some(")") | Some(",") | None
            );
        if !expression_self_update {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[cursor + 4].end,
            format!(
                "{}{}={}",
                tokens[cursor].text,
                tokens[cursor + 3].text,
                tokens[cursor + 4].text
            ),
        ));
        cursor += 5;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn member_unit_int32_update(tokens: &[Token<'_>], cursor: usize) -> Option<(usize, String)> {
    if !matches!(
        cursor
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";"),
        ";" | "{" | "}" | ")" | ","
    ) {
        return None;
    }
    let object = tokens.get(cursor)?;
    if object.text != "this" && object.kind != TokenKind::Identifier {
        return None;
    }
    if tokens.get(cursor + 1).map(|token| token.text) != Some(".")
        || !tokens
            .get(cursor + 2)
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
        || tokens.get(cursor + 3).map(|token| token.text) != Some("=")
        || tokens.get(cursor + 4).map(|token| token.text) != Some(object.text)
        || tokens.get(cursor + 5).map(|token| token.text) != Some(".")
        || tokens.get(cursor + 6).map(|token| token.text) != Some(tokens[cursor + 2].text)
        || !matches!(
            tokens.get(cursor + 7).map(|token| token.text),
            Some("+") | Some("-")
        )
        || tokens.get(cursor + 8).map(|token| token.text) != Some("1")
    {
        return None;
    }
    if tokens.get(cursor + 9).map(|token| token.text) != Some("|")
        || tokens.get(cursor + 10).map(|token| token.text) != Some("0")
        || !matches!(
            tokens.get(cursor + 11).map(|token| token.text),
            Some(";") | Some("}") | Some(")") | Some(",") | None
        )
    {
        return None;
    }
    let end = cursor + 10;
    let op = if tokens[cursor + 7].text == "+" {
        "++"
    } else {
        "--"
    };
    Some((
        end,
        format!("{}.{}{op}", object.text, tokens[cursor + 2].text),
    ))
}

pub(crate) fn fold_for_false_breaks(source: &str) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for for_at in 0..tokens.len() {
        if tokens[for_at].text != "for"
            || tokens.get(for_at + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(header_close) = matching_close.get(for_at + 1).copied().flatten() else {
            continue;
        };
        let mut semis = Vec::new();
        let mut depth = 0i32;
        for (index, token) in tokens
            .iter()
            .enumerate()
            .take(header_close)
            .skip(for_at + 2)
        {
            match token.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                ";" if depth == 0 => semis.push(index),
                _ => {}
            }
        }
        if semis.len() != 2 {
            continue;
        }
        let cond = &source[tokens[semis[0]].end..tokens[semis[1]].start];
        let inc = &source[tokens[semis[1]].end..tokens[header_close].start];
        let init = &source[tokens[for_at + 1].end..tokens[semis[0]].start];
        let body_at = header_close + 1;
        let (test_start, test_end, body_end) =
            if tokens.get(body_at).map(|token| token.text) == Some("{") {
                let Some(body_close) = matching_close.get(body_at).copied().flatten() else {
                    continue;
                };
                let Some((test_start, test_end, after_break)) =
                    exclusive_if_break(&tokens, &matching_close, body_at + 1)
                else {
                    continue;
                };
                if after_break != body_close {
                    continue;
                }
                (test_start, test_end, tokens[body_close].end)
            } else {
                let Some((test_start, test_end, after_break)) =
                    exclusive_if_break(&tokens, &matching_close, body_at)
                else {
                    continue;
                };
                let end = tokens
                    .get(after_break)
                    .filter(|token| token.text == ";")
                    .map(|token| token.end)
                    .unwrap_or(tokens[after_break - 1].end);
                (test_start, test_end, end)
            };
        let Some(cond_extra) = false_equality_condition(source, &tokens, test_start, test_end)
        else {
            continue;
        };
        let new_cond = if cond.trim().is_empty() {
            cond_extra
        } else {
            format!("{cond}&&{cond_extra}")
        };
        replacements.push((
            tokens[for_at].start,
            body_end,
            format!("for({init};{new_cond};{inc});"),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn exclusive_if_break(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    if_at: usize,
) -> Option<(usize, usize, usize)> {
    if tokens.get(if_at).map(|token| token.text) != Some("if")
        || tokens.get(if_at + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let if_close = matching_close.get(if_at + 1).copied().flatten()?;
    let after_break = if tokens.get(if_close + 1).map(|token| token.text) == Some("break") {
        if tokens.get(if_close + 2).map(|token| token.text) == Some(";") {
            if_close + 3
        } else {
            if_close + 2
        }
    } else if tokens.get(if_close + 1).map(|token| token.text) == Some("{") {
        let block_close = matching_close.get(if_close + 1).copied().flatten()?;
        if tokens.get(if_close + 2).map(|token| token.text) != Some("break") {
            return None;
        }
        let after_inner = if tokens.get(if_close + 3).map(|token| token.text) == Some(";") {
            if_close + 4
        } else {
            if_close + 3
        };
        if after_inner != block_close {
            return None;
        }
        block_close + 1
    } else {
        return None;
    };
    Some((if_at + 2, if_close, after_break))
}

fn false_equality_condition(
    source: &str,
    tokens: &[Token<'_>],
    test_open: usize,
    test_close: usize,
) -> Option<String> {
    let test = &tokens[test_open..test_close];
    let eq_at = if test.len() >= 3
        && test[test.len() - 1].text == "1"
        && test[test.len() - 2].text == "!"
        && matches!(test[test.len() - 3].text, "===" | "==")
    {
        test.len() - 3
    } else if test.len() >= 2
        && matches!(test[test.len() - 1].text, "false" | "!1")
        && matches!(test[test.len() - 2].text, "===" | "==")
    {
        test.len() - 2
    } else {
        return None;
    };
    let left = &source[test[0].start..test[eq_at].start];
    Some(format!("!1!=={left}"))
}

pub(crate) fn fold_nullish_index_walks(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for for_at in 0..tokens.len() {
        if tokens[for_at].text != "for"
            || tokens.get(for_at + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(header_close) = matching_close.get(for_at + 1).copied().flatten() else {
            continue;
        };
        let header = &source[tokens[for_at + 1].end..tokens[header_close].start];
        if header != ";!0;" && header != ";;" {
            continue;
        }
        let body_open = header_close + 1;
        if tokens.get(body_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(body_open).copied().flatten() else {
            continue;
        };
        if let Some(folded) = fold_index_walk_from_tokens(source, &tokens, body_open, body_close) {
            replacements.push((tokens[for_at].start, tokens[body_close].end, folded));
        }
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn fold_index_walk_from_tokens(
    source: &str,
    tokens: &[Token<'_>],
    body_open: usize,
    body_close: usize,
) -> Option<String> {
    let start = body_open + 1;
    if let Some(folded) = fold_postfix_index_walk(source, tokens, start, body_close) {
        return Some(folded);
    }
    fold_temp_index_walk(source, tokens, start, body_close)
}

fn consume_falsy_break(tokens: &[Token<'_>], at: usize, name: &str) -> Option<usize> {
    if tokens.get(at).map(|token| token.text) != Some("if")
        || tokens.get(at + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let after_test = if tokens.get(at + 2).map(|token| token.text) == Some("!")
        && tokens.get(at + 3).map(|token| token.text) == Some(name)
        && tokens.get(at + 4).map(|token| token.text) == Some(")")
    {
        at + 5
    } else if tokens.get(at + 2).map(|token| token.text) == Some(name)
        && matches!(
            tokens.get(at + 3).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens.get(at + 4).map(|token| token.text) == Some("null")
        && tokens.get(at + 5).map(|token| token.text) == Some(")")
    {
        at + 6
    } else {
        return None;
    };
    if tokens.get(after_test).map(|token| token.text) != Some("break") {
        return None;
    }
    Some(
        if tokens.get(after_test + 1).map(|token| token.text) == Some(";") {
            after_test + 2
        } else {
            after_test + 1
        },
    )
}

fn fold_postfix_index_walk(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    body_close: usize,
) -> Option<String> {
    if tokens
        .get(start)
        .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(start + 1).map(|token| token.text) != Some("=")
        || tokens
            .get(start + 2)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(start + 3).map(|token| token.text) != Some("[")
        || tokens
            .get(start + 4)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(start + 5).map(|token| token.text) != Some("++")
        || tokens.get(start + 6).map(|token| token.text) != Some("]")
        || tokens.get(start + 7).map(|token| token.text) != Some(";")
    {
        return None;
    }
    let node = tokens[start].text;
    let array = tokens[start + 2].text;
    let index = tokens[start + 4].text;
    let body_start = consume_falsy_break(tokens, start + 8, node)?;
    if body_start >= body_close {
        return None;
    }
    let body = source[tokens[body_start].start..tokens[body_close].start].trim_end_matches(';');
    Some(format!(
        "for(;{node}={array}[{index}++];){}",
        braced_loop_body(body)
    ))
}

fn braced_loop_body(body: &str) -> String {
    let body = body.trim();
    if body.is_empty() {
        return "{}".to_string();
    }
    format!("{{{body}}}")
}

fn fold_temp_index_walk(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    body_close: usize,
) -> Option<String> {
    if tokens
        .get(start)
        .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(start + 1).map(|token| token.text) != Some("=")
        || tokens
            .get(start + 2)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(start + 3).map(|token| token.text) != Some("+")
        || tokens.get(start + 4).map(|token| token.text) != Some("1")
        || tokens.get(start + 5).map(|token| token.text) != Some(";")
    {
        return None;
    }
    let temp = tokens[start].text;
    let index = tokens[start + 2].text;
    if temp == index {
        return None;
    }
    let read_at = start + 6;
    if tokens
        .get(read_at)
        .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(read_at + 1).map(|token| token.text) != Some("=")
        || tokens
            .get(read_at + 2)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(read_at + 3).map(|token| token.text) != Some("[")
        || tokens.get(read_at + 4).map(|token| token.text) != Some(index)
        || tokens.get(read_at + 5).map(|token| token.text) != Some("]")
        || tokens.get(read_at + 6).map(|token| token.text) != Some(";")
    {
        return None;
    }
    let node = tokens[read_at].text;
    let array = tokens[read_at + 2].text;
    let body_start = consume_falsy_break(tokens, read_at + 7, node)?;
    if body_start >= body_close {
        return None;
    }
    let restore_at = if tokens.get(body_close - 1).map(|token| token.text) == Some(";") {
        body_close - 4
    } else {
        body_close - 3
    };
    if tokens.get(restore_at).map(|token| token.text) != Some(index)
        || tokens.get(restore_at + 1).map(|token| token.text) != Some("=")
        || tokens.get(restore_at + 2).map(|token| token.text) != Some(temp)
    {
        return None;
    }
    if identifier_occurs(tokens, body_start, restore_at, temp) {
        return None;
    }
    if node == index {
        let body = rewrite_identifier_span(source, tokens, body_start, restore_at, node, temp)
            .trim_end_matches(';')
            .to_string();
        Some(format!(
            "for(;{temp}={array}[{index}++];){}",
            braced_loop_body(&body)
        ))
    } else if identifier_occurs(tokens, body_start, restore_at, index) {
        None
    } else {
        let body = source[tokens[body_start].start..tokens[restore_at].start].trim_end_matches(';');
        Some(format!(
            "for(;{node}={array}[{index}++];){}",
            braced_loop_body(body)
        ))
    }
}

fn while_is_do_while(
    tokens: &[Token<'_>],
    matching_open: &[Option<usize>],
    while_at: usize,
) -> bool {
    if while_at == 0 || tokens[while_at - 1].text != "}" {
        return false;
    }
    matching_open
        .get(while_at - 1)
        .copied()
        .flatten()
        .and_then(|open| open.checked_sub(1))
        .is_some_and(|before| tokens[before].text == "do")
}

fn match_postfix_increment<'a>(
    tokens: &'a [Token<'a>],
    start: usize,
    end: usize,
) -> Option<(usize, &'a str)> {
    let span = tokens.get(start..end)?;
    match span {
        [name, plusplus] if name.kind == TokenKind::Identifier && plusplus.text == "++" => {
            Some((start, name.text))
        }
        [name, plusplus, semi]
            if name.kind == TokenKind::Identifier && plusplus.text == "++" && semi.text == ";" =>
        {
            Some((start, name.text))
        }
        _ => None,
    }
}

fn last_top_level_comma(tokens: &[Token<'_>], from: usize, to: usize) -> Option<usize> {
    let mut last = None;
    let mut depth = 0i32;
    for index in from..to {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => last = Some(index),
            _ => {}
        }
    }
    last
}

fn body_has_same_level_continue(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
) -> bool {
    let mut scan = from;
    while scan < to {
        if let Some(close) = skip_nested_loop_or_function(tokens, matching_close, scan) {
            scan = close + 1;
            continue;
        }
        if tokens[scan].text == "continue" {
            return true;
        }
        scan += 1;
    }
    false
}

fn condition_updates_name(tokens: &[Token<'_>], from: usize, to: usize, name: &str) -> bool {
    for index in from..to {
        if tokens[index].kind != TokenKind::Identifier || tokens[index].text != name {
            continue;
        }
        if is_property_identifier(tokens, index) {
            continue;
        }
        if matches!(
            tokens.get(index + 1).map(|token| token.text),
            Some(
                "++" | "--"
                    | "="
                    | "+="
                    | "-="
                    | "*="
                    | "/="
                    | "%="
                    | "<<="
                    | ">>="
                    | ">>>="
                    | "&="
                    | "|="
                    | "^="
                    | "&&="
                    | "||="
                    | "??="
            )
        ) {
            return true;
        }
        if matches!(
            index
                .checked_sub(1)
                .map(|previous| tokens[previous].text)
                .as_deref(),
            Some("++" | "--")
        ) {
            return true;
        }
    }
    false
}

/// The increment must be a statement of the loop body itself, not the tail of
/// an `if`/`else` arm, a ternary arm or an arrow body inside it: lifting the
/// `h++` that ends an `else` arm into the `for` header leaves the other arm's
/// `h++` in place and runs it twice (047: `if(!c)h++;else t=…,h++` became
/// `for(;…;h++){if(c)…;else}`). Walk back from the increment to the last
/// body-level boundary; any control keyword or `?`/`:`/`=>` at depth 0 on the
/// way means the increment is nested.
fn increment_is_body_level(tokens: &[Token<'_>], body_from: usize, name_at: usize) -> bool {
    let mut depth = 0i32;
    let mut index = name_at;
    while index > body_from {
        index -= 1;
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    // An unmatched opener between the body start and the increment
                    // means the increment sits inside a nested block or group — an
                    // `else{…}` arm included (that arm's `{` is not the body's, and
                    // lifting its `i++` while the other arm keeps one ran it twice).
                    return index + 1 == body_from;
                }
                depth -= 1;
            }
            ";" if depth == 0 => return true,
            "if" | "else" | "for" | "while" | "do" | "switch" | "case" | "default" | "try"
            | "catch" | "finally" | "with" | "=>" | "?" | ":"
                if depth == 0 =>
            {
                return false;
            }
            _ => {}
        }
    }
    true
}

fn increment_can_lift(tokens: &[Token<'_>], name_at: usize) -> bool {
    if is_property_identifier(tokens, name_at) {
        return false;
    }
    if name_at == 0 || tokens[name_at - 1].text != ":" {
        return true;
    }
    colon_closes_ternary(tokens, name_at - 1)
}

fn colon_closes_ternary(tokens: &[Token<'_>], colon_at: usize) -> bool {
    let mut depth = 0i32;
    let mut index = colon_at;
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
            "?" if depth == 0 => return true,
            ";" if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

fn matching_ternary_question(tokens: &[Token<'_>], colon_at: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut index = colon_at;
    while index > 0 {
        index -= 1;
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            "?" if depth == 0 => return Some(index),
            ";" if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

fn wrap_lifted_loop_body(body: &str) -> String {
    let body = body.trim_end_matches([',', ';']).trim();
    if body.is_empty() {
        ";".to_string()
    } else if body.contains(';') || body.contains('{') {
        format!("{{{body};}}")
    } else {
        format!("{body};")
    }
}

fn loop_body_after_lifted_increment(
    source: &str,
    tokens: &[Token<'_>],
    body_from: usize,
    incr_at: usize,
) -> String {
    if incr_at <= body_from {
        return ";".to_string();
    }
    if incr_at > 0 && tokens[incr_at - 1].text == ":" {
        if let Some(question) = matching_ternary_question(tokens, incr_at - 1) {
            if question >= body_from {
                let left = source[tokens[body_from].start..tokens[question].start].trim_end();
                let mid = source[tokens[question].end..tokens[incr_at - 1].start].trim();
                let body = if left.is_empty() {
                    mid.to_string()
                } else if mid.is_empty() {
                    left.to_string()
                } else {
                    format!("{left}&&{mid}")
                };
                return wrap_lifted_loop_body(&body);
            }
        }
    }
    wrap_lifted_loop_body(&source[tokens[body_from].start..tokens[incr_at].start])
}

pub(crate) fn fold_while_trailing_increments(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let matching_open = matching_openers(&matching_close);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for while_at in 0..tokens.len() {
        if tokens[while_at].text != "while"
            || tokens.get(while_at + 1).map(|token| token.text) != Some("(")
            || while_at
                .checked_sub(1)
                .is_some_and(|prev| matches!(tokens[prev].text, "." | "?."))
            || while_is_do_while(&tokens, &matching_open, while_at)
        {
            continue;
        }
        let Some(header_close) = matching_close.get(while_at + 1).copied().flatten() else {
            continue;
        };
        let cond = source[tokens[while_at + 1].end..tokens[header_close].start].trim();
        if cond.is_empty() {
            continue;
        }
        let body_at = header_close + 1;
        let (body_from, incr_at, name, replace_end) = if tokens.get(body_at).map(|token| token.text)
            == Some("{")
        {
            let Some(body_close) = matching_close.get(body_at).copied().flatten() else {
                continue;
            };
            let last = if tokens.get(body_close - 1).map(|token| token.text) == Some(";")
                && tokens.get(body_close - 2).map(|token| token.text) == Some("++")
            {
                body_close - 3
            } else if tokens.get(body_close - 1).map(|token| token.text) == Some("++") {
                body_close - 2
            } else {
                continue;
            };
            if last <= body_at + 1
                || tokens
                    .get(last)
                    .is_none_or(|token| token.kind != TokenKind::Identifier)
                || !increment_can_lift(&tokens, last)
                || !increment_is_body_level(&tokens, body_at + 1, last)
            {
                continue;
            }
            (body_at + 1, last, tokens[last].text, tokens[body_close].end)
        } else {
            let stop = top_level_stop(&tokens, body_at, &[";", "}"]).unwrap_or(tokens.len());
            if stop <= body_at {
                continue;
            }
            let last_comma = last_top_level_comma(&tokens, body_at, stop);
            let incr_from = last_comma.map(|comma| comma + 1).unwrap_or(body_at);
            let Some((incr_at, name)) = match_postfix_increment(&tokens, incr_from, stop) else {
                continue;
            };
            if !increment_can_lift(&tokens, incr_at)
                || !increment_is_body_level(&tokens, body_at, incr_at)
            {
                continue;
            }
            let replace_end = if stop < tokens.len() && tokens[stop].text == ";" {
                tokens[stop].end
            } else {
                tokens[incr_at + 1].end
            };
            (body_at, incr_at, name, replace_end)
        };
        if condition_updates_name(&tokens, while_at + 2, header_close, name)
            || body_has_same_level_continue(&tokens, &matching_close, body_from, incr_at)
        {
            continue;
        }
        let body = loop_body_after_lifted_increment(source, &tokens, body_from, incr_at);
        replacements.push((
            tokens[while_at].start,
            replace_end,
            format!("for(;{cond};{name}++){body}"),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_for_trailing_increments(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for for_at in 0..tokens.len() {
        if tokens[for_at].text != "for"
            || tokens.get(for_at + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(header_close) = matching_close.get(for_at + 1).copied().flatten() else {
            continue;
        };
        let mut semis = Vec::new();
        let mut depth = 0i32;
        for (index, token) in tokens
            .iter()
            .enumerate()
            .take(header_close)
            .skip(for_at + 2)
        {
            match token.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                ";" if depth == 0 => semis.push(index),
                _ => {}
            }
        }
        if semis.len() != 2 || semis[1] + 1 != header_close {
            continue;
        }
        let body_at = header_close + 1;
        if tokens.get(body_at).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(body_at).copied().flatten() else {
            continue;
        };
        let last = if tokens.get(body_close - 1).map(|token| token.text) == Some(";")
            && tokens.get(body_close - 2).map(|token| token.text) == Some("++")
        {
            body_close - 3
        } else if tokens.get(body_close - 1).map(|token| token.text) == Some("++") {
            body_close - 2
        } else {
            continue;
        };
        if last <= body_at + 1
            || tokens
                .get(last)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !increment_can_lift(&tokens, last)
            || !increment_is_body_level(&tokens, body_at + 1, last)
        {
            continue;
        }
        let name = tokens[last].text;
        let mut scan = body_at + 1;
        let mut same_level_continue = false;
        while scan < last {
            if let Some(close) = skip_nested_loop_or_function(&tokens, &matching_close, scan) {
                scan = close + 1;
                continue;
            }
            if tokens[scan].text == "continue" {
                same_level_continue = true;
                break;
            }
            scan += 1;
        }
        if same_level_continue {
            continue;
        }
        let init = &source[tokens[for_at + 1].end..tokens[semis[0]].start];
        let cond = &source[tokens[semis[0]].end..tokens[semis[1]].start];
        let body = loop_body_after_lifted_increment(source, &tokens, body_at + 1, last);
        replacements.push((
            tokens[for_at].start,
            tokens[body_close].end,
            format!("for({init};{cond};{name}++){body}"),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn for_header_semicolons(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    for_at: usize,
) -> Option<(usize, usize, usize)> {
    if tokens.get(for_at).map(|token| token.text) != Some("for")
        || tokens.get(for_at + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let header_close = matching_close.get(for_at + 1).copied().flatten()?;
    let mut semis = Vec::new();
    let mut depth = 0i32;
    for (index, token) in tokens
        .iter()
        .enumerate()
        .take(header_close)
        .skip(for_at + 2)
    {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            ";" if depth == 0 => semis.push(index),
            _ => {}
        }
    }
    if semis.len() != 2 {
        return None;
    }
    Some((header_close, semis[0], semis[1]))
}

fn is_unit_increment(tokens: &[Token<'_>], start: usize, end: usize, name: &str) -> bool {
    let span = &tokens[start..end];
    matches!(
        span,
        [name_tok, plusplus]
            if name_tok.text == name && plusplus.text == "++"
    ) || matches!(
        span,
        [plusplus, name_tok]
            if plusplus.text == "++" && name_tok.text == name
    )
}

fn cheap_literal_rhs(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let span = &tokens[start..end];
    match span {
        [bang, digit] if bang.text == "!" && matches!(digit.text, "0" | "1") => true,
        [token]
            if matches!(
                token.text,
                "0" | "1" | "true" | "false" | "null" | "undefined"
            ) || token.kind == TokenKind::String =>
        {
            true
        }
        [open, close] if matches!((open.text, close.text), ("[", "]") | ("{", "}")) => true,
        _ => false,
    }
}

pub(crate) fn fold_prefix_increment_for_bounds(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        if !is_statement_boundary(&tokens, cursor) {
            cursor += 1;
            continue;
        }
        let (name, inc_end) = if tokens[cursor].kind == TokenKind::Identifier
            && tokens.get(cursor + 1).map(|token| token.text) == Some("++")
            && tokens.get(cursor + 2).map(|token| token.text) == Some(";")
        {
            (tokens[cursor].text, cursor + 3)
        } else if tokens[cursor].text == "++"
            && tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 2).map(|token| token.text) == Some(";")
        {
            (tokens[cursor + 1].text, cursor + 3)
        } else {
            cursor += 1;
            continue;
        };
        let for_at = inc_end;
        let Some((header_close, first_semi, second_semi)) =
            for_header_semicolons(&tokens, &matching_close, for_at)
        else {
            cursor += 1;
            continue;
        };
        if first_semi != for_at + 2 {
            cursor += 1;
            continue;
        }
        if tokens.get(first_semi + 1).map(|token| token.text) != Some(name)
            || tokens.get(first_semi + 2).map(|token| token.text) != Some("<")
        {
            cursor += 1;
            continue;
        }
        if !is_unit_increment(&tokens, second_semi + 1, header_close, name) {
            cursor += 1;
            continue;
        }
        // Everything after `name <` up to the condition's end is not one operand: `<` binds
        // tighter than `&&`, `||`, `?`, `,` and `=`, so `i<n&&ok` is `(i<n)&&ok` and the tail
        // belongs to the loop's condition, not to the comparison. Parenthesising it turned the
        // test into `i<(n&&ok)` — a comparison against a boolean, with a different iteration
        // count (cnlil's run-merge loop never merged; finer 049). The text goes through as it
        // stands, which reproduces the original parse with the increment lifted.
        let bound = source[tokens[first_semi + 3].start..tokens[second_semi].start].to_string();
        replacements.push((
            tokens[cursor].start,
            tokens[header_close].end,
            format!("for(;++{name}<{bound};)"),
        ));
        cursor = header_close + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_increment_infinite_for_bounds(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        if !is_statement_boundary(&tokens, cursor) {
            cursor += 1;
            continue;
        }
        let (name, inc_end) = if tokens[cursor].kind == TokenKind::Identifier
            && tokens.get(cursor + 1).map(|token| token.text) == Some("++")
            && tokens.get(cursor + 2).map(|token| token.text) == Some(";")
        {
            (tokens[cursor].text, cursor + 3)
        } else if tokens[cursor].text == "++"
            && tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 2).map(|token| token.text) == Some(";")
        {
            (tokens[cursor + 1].text, cursor + 3)
        } else {
            cursor += 1;
            continue;
        };
        let for_at = inc_end;
        let Some((header_close, first_semi, second_semi)) =
            for_header_semicolons(&tokens, &matching_close, for_at)
        else {
            cursor += 1;
            continue;
        };
        if first_semi != for_at + 2
            || first_semi + 1 != second_semi
            || !is_unit_increment(&tokens, second_semi + 1, header_close, name)
        {
            cursor += 1;
            continue;
        }
        let body_at = header_close + 1;
        if tokens.get(body_at).map(|token| token.text) != Some("{") {
            cursor += 1;
            continue;
        }
        let Some(body_close) = matching_close.get(body_at).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let mut scan = body_at + 1;
        let mut kept_prefix = "";
        if tokens.get(scan).map(|token| token.text) == Some("var")
            && tokens
                .get(scan + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(scan + 2).map(|token| token.text) == Some("=")
            && tokens.get(scan + 3).map(|token| token.text) == Some(name)
            && tokens.get(scan + 4).map(|token| token.text) == Some(";")
        {
            kept_prefix = &source[tokens[scan].start..tokens[scan + 4].end];
            scan += 5;
        }
        if tokens.get(scan).map(|token| token.text) != Some("if")
            || tokens.get(scan + 1).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let Some(if_close) = matching_close.get(scan + 1).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let test = &tokens[scan + 2..if_close];
        // The break test has to be the comparison itself: `if(i>=n&&!ok)break` exits on the
        // conjunction, so its negation is `i<n||ok`, which this rewrite cannot spell.
        if expression_has_top_level_token(test, "&&")
            || expression_has_top_level_token(test, "||")
            || expression_has_top_level_token(test, "?")
            || expression_has_top_level_token(test, ",")
        {
            cursor += 1;
            continue;
        }
        let bound = if matches!(
            test,
            [ident, op, ..] if ident.text == name && op.text == ">="
        ) {
            &source[tokens[scan + 4].start..tokens[if_close].start]
        } else if test.len() >= 5
            && test[0].text == "!"
            && test[1].text == "("
            && test[2].text == name
            && test[3].text == "<"
            && test.last().is_some_and(|token| token.text == ")")
        {
            &source[test[4].start..test[test.len() - 1].start]
        } else {
            cursor += 1;
            continue;
        };
        let after_if = if tokens.get(if_close + 1).map(|token| token.text) == Some("break") {
            if tokens.get(if_close + 2).map(|token| token.text) == Some(";") {
                if_close + 3
            } else {
                if_close + 2
            }
        } else if tokens.get(if_close + 1).map(|token| token.text) == Some("{") {
            let Some(block_close) = matching_close.get(if_close + 1).copied().flatten() else {
                cursor += 1;
                continue;
            };
            if tokens.get(if_close + 2).map(|token| token.text) != Some("break") {
                cursor += 1;
                continue;
            }
            let after_break = if tokens.get(if_close + 3).map(|token| token.text) == Some(";") {
                if_close + 4
            } else {
                if_close + 3
            };
            if after_break != block_close {
                cursor += 1;
                continue;
            }
            block_close + 1
        } else {
            cursor += 1;
            continue;
        };
        let rest = &source[tokens[after_if].start..tokens[body_close].start];
        replacements.push((
            tokens[cursor].start,
            tokens[body_close].end,
            format!("for(;++{name}<{bound};){{{kept_prefix}{rest}}}"),
        ));
        cursor = body_close + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_while_true_unit_increment_bounds(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 12 < tokens.len() {
        if !is_statement_boundary(&tokens, cursor) {
            cursor += 1;
            continue;
        }
        let body_at = if tokens[cursor].text == "while"
            && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("!")
            && tokens.get(cursor + 3).map(|token| token.text) == Some("0")
            && tokens.get(cursor + 4).map(|token| token.text) == Some(")")
            && tokens.get(cursor + 5).map(|token| token.text) == Some("{")
        {
            cursor + 5
        } else if tokens[cursor].text == "for"
            && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
            && tokens.get(cursor + 2).map(|token| token.text) == Some(";")
            && tokens.get(cursor + 3).map(|token| token.text) == Some("!")
            && tokens.get(cursor + 4).map(|token| token.text) == Some("0")
            && tokens.get(cursor + 5).map(|token| token.text) == Some(";")
            && tokens.get(cursor + 6).map(|token| token.text) == Some(")")
            && tokens.get(cursor + 7).map(|token| token.text) == Some("{")
        {
            cursor + 7
        } else {
            cursor += 1;
            continue;
        };
        let Some(body_close) = matching_close.get(body_at).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let (name, inc_end, prefix) = if tokens
            .get(body_at + 1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(body_at + 2).map(|token| token.text) == Some("++")
            && tokens.get(body_at + 3).map(|token| token.text) == Some(";")
        {
            (tokens[body_at + 1].text, body_at + 4, "++")
        } else if tokens.get(body_at + 1).map(|token| token.text) == Some("++")
            && tokens
                .get(body_at + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(body_at + 3).map(|token| token.text) == Some(";")
        {
            (tokens[body_at + 2].text, body_at + 4, "++")
        } else if tokens
            .get(body_at + 1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(body_at + 2).map(|token| token.text) == Some("--")
            && tokens.get(body_at + 3).map(|token| token.text) == Some(";")
        {
            (tokens[body_at + 1].text, body_at + 4, "--")
        } else {
            cursor += 1;
            continue;
        };
        if tokens.get(inc_end).map(|token| token.text) != Some("if")
            || tokens.get(inc_end + 1).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let Some(if_close) = matching_close.get(inc_end + 1).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let after_if = if tokens.get(if_close + 1).map(|token| token.text) == Some("break") {
            if tokens.get(if_close + 2).map(|token| token.text) == Some(";") {
                if_close + 3
            } else {
                if_close + 2
            }
        } else {
            cursor += 1;
            continue;
        };
        let test = &tokens[inc_end + 2..if_close];
        if expression_has_top_level_token(test, "&&")
            || expression_has_top_level_token(test, "||")
            || expression_has_top_level_token(test, "?")
            || expression_has_top_level_token(test, ",")
        {
            cursor += 1;
            continue;
        }
        let header = if prefix == "++"
            && matches!(test, [ident, op, ..] if ident.text == name && matches!(op.text, ">=" | ">"))
        {
            let bound = &source[tokens[inc_end + 4].start..tokens[if_close].start];
            if test[1].text == ">=" {
                format!("for(;{prefix}{name}<{bound};)")
            } else {
                format!("for(;{prefix}{name}<={bound};)")
            }
        } else if prefix == "--"
            && matches!(test, [ident, op, zero] if ident.text == name && op.text == "<" && zero.text == "0")
        {
            format!("for(;{prefix}{name}>=0;)")
        } else {
            cursor += 1;
            continue;
        };
        let rest = &source[tokens[after_if].start..tokens[body_close].start];
        replacements.push((
            tokens[cursor].start,
            tokens[body_close].end,
            format!("{header}{{{rest}}}"),
        ));
        cursor = body_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_assigned_index_for_conditions(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for for_at in 0..tokens.len() {
        let Some((header_close, first_semi, second_semi)) =
            for_header_semicolons(&tokens, &matching_close, for_at)
        else {
            continue;
        };
        let cond = &tokens[first_semi + 1..second_semi];
        let always = matches!(cond, [bang, zero] if bang.text == "!" && zero.text == "0")
            || matches!(cond, [token] if token.text == "true");
        if !always {
            continue;
        }
        let body_at = header_close + 1;
        if tokens.get(body_at).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(body_at).copied().flatten() else {
            continue;
        };
        if tokens.get(body_at + 1).map(|token| token.kind) != Some(TokenKind::Identifier)
            || tokens.get(body_at + 2).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let name = tokens[body_at + 1].text;
        let Some(assign_semi) = top_level_stop(&tokens, body_at + 3, &[";"]) else {
            continue;
        };
        let if_at = assign_semi + 1;
        if tokens.get(if_at).map(|token| token.text) != Some("if")
            || tokens.get(if_at + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(if_close) = matching_close.get(if_at + 1).copied().flatten() else {
            continue;
        };
        let test = &tokens[if_at + 2..if_close];
        let new_cond = if matches!(
            test,
            [ident, lt, zero]
                if ident.text == name && lt.text == "<" && zero.text == "0"
        ) || matches!(
            test,
            [bang, open, ident, gt, minus, one, close]
                if bang.text == "!"
                    && open.text == "("
                    && ident.text == name
                    && gt.text == ">"
                    && minus.text == "-"
                    && one.text == "1"
                    && close.text == ")"
        ) {
            format!(
                "({name}={})>-1",
                &source[tokens[body_at + 3].start..tokens[assign_semi].start]
            )
        } else {
            continue;
        };
        let after_if = if tokens.get(if_close + 1).map(|token| token.text) == Some("break") {
            if tokens.get(if_close + 2).map(|token| token.text) == Some(";") {
                if_close + 3
            } else {
                if_close + 2
            }
        } else if tokens.get(if_close + 1).map(|token| token.text) == Some("{") {
            let Some(block_close) = matching_close.get(if_close + 1).copied().flatten() else {
                continue;
            };
            if tokens.get(if_close + 2).map(|token| token.text) != Some("break") {
                continue;
            }
            let after_break = if tokens.get(if_close + 3).map(|token| token.text) == Some(";") {
                if_close + 4
            } else {
                if_close + 3
            };
            if after_break != block_close {
                continue;
            }
            block_close + 1
        } else {
            continue;
        };
        let init = elide_zero_for_init(
            &source[tokens[for_at + 2].start..tokens[first_semi].start],
            name,
        );
        let inc = &source[tokens[second_semi].end..tokens[header_close].start];
        let rest = &source[tokens[after_if].start..tokens[body_close].start];
        replacements.push((
            tokens[for_at].start,
            tokens[body_close].end,
            format!("for({init};{new_cond};{inc}){{{rest}}}"),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn elide_zero_for_init(init: &str, name: &str) -> String {
    let trimmed = init.trim();
    for decl in ["var ", "let "] {
        if let Some(rest) = trimmed.strip_prefix(decl) {
            if rest == format!("{name}=0") || rest == format!("{name}=0.0") {
                return format!("{decl}{name}");
            }
        }
    }
    if trimmed == format!("{name}=0") || trimmed == format!("{name}=0.0") {
        return String::new();
    }
    init.to_string()
}

pub(crate) fn fold_index_scan_for_headers(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for for_at in 0..tokens.len() {
        let Some((header_close, first_semi, second_semi)) =
            for_header_semicolons(&tokens, &matching_close, for_at)
        else {
            continue;
        };
        let init = &tokens[for_at + 2..first_semi];
        let (decl, name, zero_init) = match init {
            [decl, ident, eq, zero]
                if matches!(decl.text, "var" | "let")
                    && ident.kind == TokenKind::Identifier
                    && eq.text == "="
                    && zero.text == "0" =>
            {
                (decl.text, ident.text, true)
            }
            [decl, ident]
                if matches!(decl.text, "var" | "let") && ident.kind == TokenKind::Identifier =>
            {
                (decl.text, ident.text, false)
            }
            _ => continue,
        };
        if tokens.get(first_semi + 1).map(|token| token.text) != Some("(")
            || tokens.get(first_semi + 2).map(|token| token.text) != Some(name)
            || tokens.get(first_semi + 3).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let Some(paren_close) = matching_close.get(first_semi + 1).copied().flatten() else {
            continue;
        };
        if paren_close >= second_semi {
            continue;
        }
        let after = &tokens[paren_close + 1..second_semi];
        let ge_zero = matches!(after, [op, zero] if op.text == ">=" && zero.text == "0");
        let gt_minus1 = matches!(
            after,
            [gt, minus, one] if gt.text == ">" && minus.text == "-" && one.text == "1"
        ) || matches!(
            after,
            [gt, num] if gt.text == ">" && num.text == "-1"
        );
        if !ge_zero && !gt_minus1 || !zero_init && !ge_zero {
            continue;
        }
        let assign = &source[tokens[first_semi + 1].start..tokens[paren_close].end];
        let inc = &source[tokens[second_semi].end..tokens[header_close].start];
        replacements.push((
            tokens[for_at].start,
            tokens[header_close].end,
            format!("for({decl} {name};{assign}>-1;{inc})"),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_arguments_length_countdown_for(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let matching_open = matching_openers(&matching_close);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for at in 0..tokens.len() {
        if tokens[at].text == "while"
            && tokens.get(at + 1).map(|token| token.text) == Some("(")
            && at
                .checked_sub(1)
                .is_none_or(|prev| !matches!(tokens[prev].text, "." | "?."))
            && !while_is_do_while(&tokens, &matching_open, at)
        {
            if let Some(header_close) = matching_close.get(at + 1).copied().flatten() {
                if tokens
                    .get(at + 2)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && tokens.get(at + 3).map(|token| token.text) == Some(">")
                    && tokens.get(at + 4).map(|token| token.text) == Some("0")
                    && at + 5 == header_close
                {
                    let name = tokens[at + 2].text;
                    if name_is_nonnegative_length_copy(&tokens, &matching_close, header_close, name)
                    {
                        push_countdown_header(
                            &tokens,
                            at,
                            header_close,
                            name,
                            "",
                            &mut replacements,
                        );
                    }
                }
            }
            continue;
        }
        let Some((header_close, first_semi, second_semi)) =
            for_header_semicolons(&tokens, &matching_close, at)
        else {
            continue;
        };
        if second_semi + 1 != header_close {
            continue;
        }
        if tokens
            .get(first_semi + 1)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(first_semi + 2).map(|token| token.text) != Some(">")
            || tokens.get(first_semi + 3).map(|token| token.text) != Some("0")
            || first_semi + 4 != second_semi
        {
            continue;
        }
        let name = tokens[first_semi + 1].text;
        if !name_is_nonnegative_length_copy(&tokens, &matching_close, header_close, name) {
            continue;
        }
        let init = &source[tokens[at + 2].start..tokens[first_semi].start];
        push_countdown_header(&tokens, at, header_close, name, init, &mut replacements);
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn push_countdown_header(
    tokens: &[Token<'_>],
    header_at: usize,
    header_close: usize,
    name: &str,
    init: &str,
    replacements: &mut Vec<(usize, usize, String)>,
) {
    let braced = tokens.get(header_close + 1).map(|token| token.text) == Some("{");
    let dec_at = if braced {
        header_close + 2
    } else {
        header_close + 1
    };
    let Some(after_dec) = match_ident_decrement(tokens, dec_at, name) else {
        return;
    };
    if !matches!(
        tokens.get(after_dec).map(|token| token.text),
        Some(",") | Some(";") | Some("}") | None
    ) {
        return;
    }
    if braced {
        if tokens.get(after_dec).map(|token| token.text) == Some(";") {
            replacements.push((
                tokens[header_at].start,
                tokens[after_dec].end,
                format!("for({init};{name}--;){{"),
            ));
        } else if tokens.get(after_dec).map(|token| token.text) == Some("}") {
            replacements.push((
                tokens[header_at].start,
                tokens[after_dec].end,
                format!("for({init};{name}--);"),
            ));
        }
        return;
    }
    let end = if tokens.get(after_dec).map(|token| token.text) == Some(",") {
        tokens[after_dec].end
    } else {
        tokens[after_dec - 1].end
    };
    replacements.push((
        tokens[header_at].start,
        end,
        format!("for({init};{name}--;)"),
    ));
}

fn match_ident_decrement(tokens: &[Token<'_>], at: usize, name: &str) -> Option<usize> {
    if tokens.get(at).map(|token| token.text) == Some("--")
        && tokens.get(at + 1).map(|token| token.text) == Some(name)
    {
        return Some(at + 2);
    }
    if tokens.get(at).map(|token| token.text) == Some(name)
        && tokens.get(at + 1).map(|token| token.text) == Some("--")
    {
        return Some(at + 2);
    }
    if tokens.get(at).map(|token| token.text) == Some(name)
        && tokens.get(at + 1).map(|token| token.text) == Some("=")
        && tokens.get(at + 2).map(|token| token.text) == Some(name)
        && tokens.get(at + 3).map(|token| token.text) == Some("-")
        && tokens.get(at + 4).map(|token| token.text) == Some("1")
    {
        return Some(at + 5);
    }
    if tokens.get(at).map(|token| token.text) == Some(name)
        && tokens.get(at + 1).map(|token| token.text) == Some("-=")
        && tokens.get(at + 2).map(|token| token.text) == Some("1")
    {
        return Some(at + 3);
    }
    None
}

pub(crate) fn fold_arguments_length_zero_after_decrement(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("--")
            || tokens.get(cursor + 2).map(|token| token.text) != Some(",")
            || !statement_expression_context(&tokens, &matching_close, cursor)
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let (eq_at, ident_at) = if tokens.get(cursor + 3).map(|token| token.text) == Some("0")
            && tokens.get(cursor + 4).map(|token| token.text) == Some("==")
            && tokens.get(cursor + 5).map(|token| token.text) == Some(name)
        {
            (cursor + 4, cursor + 5)
        } else if tokens.get(cursor + 3).map(|token| token.text) == Some(name)
            && tokens.get(cursor + 4).map(|token| token.text) == Some("==")
            && tokens.get(cursor + 5).map(|token| token.text) == Some("0")
        {
            (cursor + 4, cursor + 3)
        } else {
            cursor += 1;
            continue;
        };
        if tokens.get(ident_at.max(eq_at) + 1).map(|token| token.text) != Some("&&") {
            cursor += 1;
            continue;
        }
        if !name_is_arguments_length_copy(&tokens, &matching_close, cursor, name) {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[ident_at.max(eq_at) + 1].end,
            format!("--{name}||"),
        ));
        cursor = ident_at.max(eq_at) + 2;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn statement_expression_context(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> bool {
    if enclosing_block_start(matching_close, at)
        .is_some_and(|open| matches!(tokens[open].text, "(" | "["))
    {
        return false;
    }
    let mut depth = 0i32;
    for index in (0..at).rev() {
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    return tokens[index].text == "{";
                }
                depth -= 1;
            }
            ";" if depth == 0 => return true,
            "return" | "throw" | "yield" | "=>" | "void" | "case" if depth == 0 => return false,
            _ => {}
        }
    }
    true
}

pub(crate) fn fold_void_prefix_updates(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 1 < tokens.len() {
        if tokens[cursor].text == "--"
            && tokens[cursor + 1].kind == TokenKind::Identifier
            && matches!(
                tokens.get(cursor + 2).map(|token| token.text),
                Some(";") | Some(",") | Some("}") | None
            )
            && matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                "&&" | "||" | "," | ";" | "{"
            )
        {
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 1].end,
                format!("{}--", tokens[cursor + 1].text),
            ));
            cursor += 2;
            continue;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_redundant_loop_body_braces(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for index in 0..tokens.len() {
        if !matches!(tokens[index].text, "for" | "while")
            || tokens.get(index + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let Some(header_close) = matching_close.get(index + 1).copied().flatten() else {
            continue;
        };
        if tokens.get(header_close + 1).map(|token| token.text) != Some("{") {
            continue;
        }
        let body_open = header_close + 1;
        let Some(body_close) = matching_close.get(body_open).copied().flatten() else {
            continue;
        };
        if body_close <= body_open + 1 {
            continue;
        }
        let mut body_end = body_close;
        if tokens.get(body_close - 1).map(|token| token.text) == Some(";") {
            body_end = body_close - 1;
        }
        if !safe_braceless_loop_body(&tokens, body_open + 1, body_end) {
            continue;
        }
        let body = source[tokens[body_open + 1].start..tokens[body_end].start].trim();
        if body.is_empty() {
            continue;
        }
        replacements.push((
            tokens[body_open].start,
            tokens[body_close].end,
            format!("{body};"),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn safe_braceless_loop_body(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    if start >= end {
        return false;
    }
    if matches!(
        tokens[start].text,
        "if" | "else"
            | "var"
            | "let"
            | "const"
            | "function"
            | "class"
            | "return"
            | "throw"
            | "try"
            | "switch"
            | "with"
            | "do"
    ) {
        return false;
    }
    if matches!(tokens[start].text, "for" | "while") {
        if tokens.get(start + 1).map(|token| token.text) != Some("(") {
            return false;
        }
        let matching_close = matching_closers(tokens);
        let Some(header_close) = matching_close.get(start + 1).copied().flatten() else {
            return false;
        };
        if header_close >= end {
            return false;
        }
        let rest = header_close + 1;
        if rest >= end {
            return false;
        }
        if tokens[rest].text == "{" {
            return false;
        }
        return safe_braceless_loop_body(tokens, rest, end);
    }
    let mut depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            ";" if depth == 0 => return false,
            _ => {}
        }
        if depth < 0 {
            return false;
        }
    }
    depth == 0
}

fn let_for_init_names_escape(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    name_at: usize,
    semi: usize,
    header_close: usize,
) -> bool {
    let mut declared = vec![tokens[name_at].text];
    let mut at = name_at + 1;
    let mut depth = 0i32;
    while at < semi {
        match tokens[at].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => {
                if let Some(name) = tokens
                    .get(at + 1)
                    .filter(|t| t.kind == TokenKind::Identifier)
                {
                    declared.push(name.text);
                }
            }
            _ => {}
        }
        at += 1;
    }
    let body_start = header_close + 1;
    let body_end = if tokens.get(body_start).map(|token| token.text) == Some("{") {
        matching_close.get(body_start).copied().flatten()
    } else {
        top_level_stop(tokens, body_start, &[";"])
    };
    let Some(body_end) = body_end else {
        return true;
    };
    // The moved binding would die at the loop statement, so any later read in
    // the enclosing block (including nested functions and blocks, which scan
    // at non-negative depth) keeps the declaration outside. Leaving the block
    // exits the binding's original scope and ends the scan.
    let mut depth = 0i32;
    for token in &tokens[body_end + 1..] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            _ => {
                if token.kind == TokenKind::Identifier && declared.contains(&token.text) {
                    return true;
                }
            }
        }
    }
    false
}

pub(crate) fn fold_prior_assign_into_for_init(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        let mut assigns = Vec::new();
        let mut scan = cursor;
        while let Some(assign) = parse_bare_assign(source, &tokens, scan) {
            assigns.push(assign);
            scan = assign.3;
        }
        if !assigns.is_empty() {
            if let Some((header_close, first_semi, _)) =
                for_header_semicolons(&tokens, &matching_close, scan)
            {
                let init_empty = first_semi == scan + 2;
                let init_starts_decl = matches!(
                    tokens.get(scan + 2).map(|token| token.text),
                    Some("var") | Some("let") | Some("const")
                );
                if init_empty {
                    let init_parts = assigns
                        .iter()
                        .map(|(_, name, rhs, _)| format!("{name}={rhs}"))
                        .collect::<Vec<_>>();
                    let rest = &source[tokens[first_semi].start..tokens[header_close].end];
                    replacements.push((
                        tokens[assigns[0].0].start,
                        tokens[header_close].end,
                        format!(
                            "for({}{}{rest}",
                            for_init_var_prefix(
                                &tokens,
                                &matching_close,
                                assigns[0].0,
                                assigns.iter().map(|(_, name, _, _)| *name),
                            ),
                            init_parts.join(",")
                        ),
                    ));
                    cursor = header_close + 1;
                    continue;
                }
                if !init_starts_decl {
                    let init_start = scan + 2;
                    let first_init = if tokens[init_start].kind == TokenKind::Identifier
                        && tokens.get(init_start + 1).map(|token| token.text) == Some("=")
                    {
                        top_level_stop(&tokens, init_start + 2, &[",", ";"]).and_then(|rhs_end| {
                            cheap_literal_rhs(&tokens, init_start + 2, rhs_end).then(|| {
                                (
                                    tokens[init_start].text,
                                    &source[tokens[init_start + 2].start..tokens[rhs_end].start],
                                    rhs_end,
                                )
                            })
                        })
                    } else {
                        None
                    };
                    if let Some((init_name, init_rhs, rhs_end)) = first_init {
                        if let Some((_, last_name, last_rhs, _)) = assigns.last() {
                            if *last_rhs == init_rhs {
                                let mut init_parts = assigns
                                    .iter()
                                    .map(|(_, name, rhs, _)| format!("{name}={rhs}"))
                                    .collect::<Vec<_>>();
                                init_parts.pop();
                                init_parts.push(format!("{last_name}={init_name}={init_rhs}"));
                                let rest = &source[tokens[rhs_end].start..tokens[header_close].end];
                                replacements.push((
                                    tokens[assigns[0].0].start,
                                    tokens[header_close].end,
                                    format!(
                                        "for({}{}{rest}",
                                        for_init_var_prefix(
                                            &tokens,
                                            &matching_close,
                                            assigns[0].0,
                                            assigns.iter().map(|(_, name, _, _)| *name),
                                        ),
                                        init_parts.join(",")
                                    ),
                                ));
                                cursor = header_close + 1;
                                continue;
                            }
                        }
                    }
                }
            }
        }

        let (decl, name_at) = if matches!(tokens[cursor].text, "var" | "let")
            && tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            (format!("{} ", tokens[cursor].text), cursor + 1)
        } else if tokens[cursor].kind == TokenKind::Identifier {
            (String::new(), cursor)
        } else {
            cursor += 1;
            continue;
        };
        if !is_statement_boundary(&tokens, cursor)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(semi) = top_level_stop(&tokens, name_at + 2, &[";"]) else {
            cursor += 1;
            continue;
        };
        if tokens.get(semi + 1).map(|token| token.text) != Some("for")
            || tokens.get(semi + 2).map(|token| token.text) != Some("(")
            || tokens.get(semi + 3).map(|token| token.text) != Some(";")
        {
            cursor += 1;
            continue;
        }
        let Some(header_close) = matching_close.get(semi + 2).copied().flatten() else {
            cursor += 1;
            continue;
        };
        // A `let` moved into the for-init narrows its scope to the loop
        // statement, so any read of a declared name after the loop would
        // become a ReferenceError. `var` stays function-scoped and is safe.
        if decl.starts_with("let")
            && let_for_init_names_escape(&tokens, &matching_close, name_at, semi, header_close)
        {
            cursor += 1;
            continue;
        }
        let rest = &source[tokens[semi + 3].start..tokens[header_close].end];
        let init = &source[tokens[name_at].start..tokens[semi].start];
        replacements.push((
            tokens[cursor].start,
            tokens[header_close].end,
            format!("for({decl}{init}{rest}"),
        ));
        cursor = header_close + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    let (output, declared) = declare_undeclared_for_init_assigns(&output)?;
    Ok((output, count + declared))
}

/// ident-05: a mixed `for` initializer (`f=[],f.push(...)`) is not a `var`
/// declarator list. If those writes resolve to a module binding rather than a
/// local or an enclosing-function capture, declare a fresh local before the
/// loop so the write cannot clobber the outer function.
fn declare_undeclared_for_init_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let resolution = BindingResolution::new(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut declared = HashSet::<(usize, &str)>::new();
    for for_at in 0..tokens.len() {
        let Some((_, first_semi, _)) = for_header_semicolons(&tokens, &matching_close, for_at)
        else {
            continue;
        };
        let init_from = for_at + 2;
        if first_semi <= init_from {
            continue;
        }
        let targets = for_init_assign_targets(&tokens, init_from, first_semi);
        if targets.is_empty() {
            continue;
        }
        let Some(needed) = names_needing_function_local_var(&tokens, &resolution, &targets) else {
            continue;
        };
        let scope = resolution.scope_index_at(for_at);
        let mut names = Vec::new();
        for name in needed {
            if declared.insert((scope, name)) {
                names.push(name);
            }
        }
        if names.is_empty() {
            continue;
        }
        let keyword = if var_declaration_would_land_at_module(&tokens, &matching_close, for_at) {
            "let"
        } else {
            "var"
        };
        replacements.push((
            tokens[for_at].start,
            tokens[for_at].start,
            format!("{keyword} {};", names.join(",")),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn for_init_assign_targets(
    tokens: &[Token<'_>],
    init_from: usize,
    first_semi: usize,
) -> Vec<usize> {
    if matches!(
        tokens.get(init_from).map(|token| token.text),
        Some("var") | Some("let") | Some("const")
    ) {
        return Vec::new();
    }
    let mut targets = Vec::new();
    let mut depth = 0i32;
    let mut at_part_start = true;
    for index in init_from..first_semi {
        match tokens[index].text {
            "(" | "[" | "{" => {
                depth += 1;
                at_part_start = false;
            }
            ")" | "]" | "}" => {
                depth -= 1;
                at_part_start = false;
            }
            "," if depth == 0 => at_part_start = true,
            _ => {
                if at_part_start
                    && depth == 0
                    && tokens[index].kind == TokenKind::Identifier
                    && tokens.get(index + 1).map(|token| token.text) == Some("=")
                    && tokens.get(index + 2).map(|token| token.text) != Some("=")
                {
                    targets.push(index);
                }
                at_part_start = false;
            }
        }
    }
    targets
}

fn names_needing_function_local_var<'src>(
    tokens: &[Token<'src>],
    resolution: &BindingResolution<'src>,
    targets: &[usize],
) -> Option<Vec<&'src str>> {
    let mut needed = Vec::new();
    for &at in targets {
        match resolution.resolve(at) {
            Resolution::Unresolved => return None,
            Resolution::Free => {
                if !needed.contains(&tokens[at].text) {
                    needed.push(tokens[at].text);
                }
            }
            Resolution::Bound(declaration) => {
                let assign_scope = resolution.scope_index_at(at);
                let decl_scope = resolution.scope_index_at(declaration);
                if assign_scope == decl_scope {
                    continue;
                }
                if declaration_is_enclosing_function_capture(resolution, assign_scope, decl_scope) {
                    continue;
                }
                if !needed.contains(&tokens[at].text) {
                    needed.push(tokens[at].text);
                }
            }
        }
    }
    Some(needed)
}

fn declaration_is_enclosing_function_capture(
    resolution: &BindingResolution<'_>,
    assign_scope: usize,
    decl_scope: usize,
) -> bool {
    if decl_scope == 0 {
        return false;
    }
    let mut parent = resolution.parent_scope(assign_scope);
    while let Some(scope) = parent {
        if scope == decl_scope {
            return true;
        }
        parent = resolution.parent_scope(scope);
    }
    false
}

fn var_declaration_would_land_at_module(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> bool {
    let mut cursor = enclosing_function_span(tokens, matching_close, at);
    while let Some((body, _)) = cursor {
        let is_arrow = body
            .checked_sub(1)
            .is_some_and(|before| tokens[before].text == "=>");
        if !is_arrow {
            return false;
        }
        cursor = enclosing_function_span(tokens, matching_close, body);
    }
    true
}

fn for_init_var_prefix<'a>(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    names: impl IntoIterator<Item = &'a str>,
) -> &'static str {
    let names = names.into_iter().collect::<Vec<_>>();
    let mut undeclared = false;
    let mut outer_capture = false;
    for name in names {
        let visible = name_is_declared_in_visible_scope(tokens, matching_close, at, name);
        let enclosing = name_is_declared_in_any_enclosing_scope(tokens, matching_close, at, name);
        if !enclosing {
            undeclared = true;
        } else if !visible {
            outer_capture = true;
        }
    }
    // `var` in a nested for-init is a fresh function-local binding. Captures
    // from an outer function are already declared; prefixing `var` shadows
    // them so later readers (`fired:()=>!!ss`) keep the uninitialized outer.
    if undeclared && !outer_capture {
        "var "
    } else {
        ""
    }
}

/// Rotates the generated SSA spelling `flag=true;while(flag){...;flag=next}`
/// into `do{...}while(next)`. The proof deliberately requires the synthetic
/// flag to occur only at those three sites and rejects `continue`, whose
/// bottom-condition timing would otherwise differ.
pub(crate) fn rotate_proven_initial_true_loops(
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

    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0;
    while cursor < tokens.len().saturating_sub(9) {
        let start = cursor;
        let Some(name) = tokens
            .get(start)
            .filter(|token| token.kind == TokenKind::Identifier)
            .map(|token| token.text)
        else {
            cursor += 1;
            continue;
        };
        if tokens.get(start + 1).map(|token| token.text) != Some("=") {
            cursor += 1;
            continue;
        }
        let while_index = if tokens.get(start + 2).map(|token| token.text) == Some("true")
            && tokens.get(start + 3).map(|token| token.text) == Some(";")
        {
            start + 4
        } else if tokens.get(start + 2).map(|token| token.text) == Some("!")
            && tokens.get(start + 3).map(|token| token.text) == Some("0")
            && tokens.get(start + 4).map(|token| token.text) == Some(";")
        {
            start + 5
        } else {
            cursor += 1;
            continue;
        };
        if tokens.get(while_index).map(|token| token.text) != Some("while")
            || tokens.get(while_index + 1).map(|token| token.text) != Some("(")
            || tokens.get(while_index + 2).map(|token| token.text) != Some(name)
            || tokens.get(while_index + 3).map(|token| token.text) != Some(")")
            || tokens.get(while_index + 4).map(|token| token.text) != Some("{")
        {
            cursor += 1;
            continue;
        }
        let body_open = while_index + 4;
        let Some(body_close) = matching_close[body_open] else {
            cursor += 1;
            continue;
        };
        if tokens[body_open + 1..body_close]
            .iter()
            .any(|token| token.text == "continue")
            || tokens[start..body_close]
                .iter()
                .filter(|token| token.kind == TokenKind::Identifier && token.text == name)
                .count()
                != 3
        {
            cursor = body_close + 1;
            continue;
        }

        let mut delimiters = Vec::<&str>::new();
        let mut top_level_semicolons = Vec::new();
        for (index, token) in tokens
            .iter()
            .enumerate()
            .take(body_close)
            .skip(body_open + 1)
        {
            match token.text {
                "(" | "[" | "{" => delimiters.push(token.text),
                ")" | "]" | "}" => {
                    delimiters.pop();
                }
                ";" if delimiters.is_empty() => top_level_semicolons.push(index),
                _ => {}
            }
        }
        // Standard-grammar emission may omit the update statement's final
        // semicolon immediately before `}`. Treat the block close as the same
        // statement boundary so this later peephole accepts both scored forms.
        let terminal_semicolon = top_level_semicolons
            .last()
            .copied()
            .filter(|semicolon| semicolon + 1 == body_close);
        let final_end = terminal_semicolon.unwrap_or(body_close);
        let final_start = top_level_semicolons
            .iter()
            .rev()
            .nth(usize::from(terminal_semicolon.is_some()))
            .map_or(body_open + 1, |index| index + 1);
        if tokens.get(final_start).map(|token| token.text) != Some(name)
            || tokens.get(final_start + 1).map(|token| token.text) != Some("=")
            || final_start + 2 >= final_end
        {
            cursor = body_close + 1;
            continue;
        }

        let mut replacement = String::new();
        replacement.push_str("do{");
        let mut retained_body = &source[tokens[body_open].end..tokens[final_start].start];
        if retained_body.ends_with(';') {
            retained_body = &retained_body[..retained_body.len() - 1];
        }
        replacement.push_str(retained_body);
        replacement.push_str("}while(");
        replacement.push_str(&source[tokens[final_start + 2].start..tokens[final_end].start]);
        replacement.push_str(");");
        let replaced_start = tokens[start].start;
        let replaced_end = tokens[body_close].end;
        if replacement.len() < replaced_end - replaced_start {
            replacements.push((replaced_start, replaced_end, replacement));
        }
        cursor = body_close + 1;
    }

    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let count = replacements.len();
    let mut output = source.to_string();
    for (start, end, replacement) in replacements.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok((output, count))
}
