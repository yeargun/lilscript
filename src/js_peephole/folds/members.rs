use crate::js_peephole::rewrite::{apply_token_rewrites, identifier_occurs, top_level_stop};
use crate::js_peephole::scope::{
    collect_unbound_name_uses, enclosing_block_end, function_binds_name, nested_function_end,
    own_body_has_this_or_arguments, parse_function_expression, simple_identifier_params,
};
use crate::js_peephole::token::{lex, matching_closers, matching_openers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn flatten_associative_string_concats(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if tokens[cursor].kind == TokenKind::String
            && tokens.get(cursor + 1).map(|token| token.text) == Some("+")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("(")
            && tokens
                .get(cursor + 3)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 4).map(|token| token.text) == Some("+")
            && tokens.get(cursor + 5).map(|token| token.kind) == Some(TokenKind::String)
            && tokens.get(cursor + 6).map(|token| token.text) == Some(")")
        {
            replacements.push((
                tokens[cursor].start,
                tokens[cursor + 6].end,
                format!(
                    "{}+{}+{}",
                    tokens[cursor].text,
                    tokens[cursor + 3].text,
                    tokens[cursor + 5].text
                ),
            ));
            cursor += 7;
            continue;
        }
        cursor += 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn is_object_name(token: &Token<'_>) -> bool {
    token.kind == TokenKind::Identifier || token.text == "this"
}

fn length_binding<'a>(tokens: &[Token<'a>], name_at: usize) -> Option<(&'a str, &'a str, usize)> {
    if tokens.get(name_at)?.kind != TokenKind::Identifier || tokens.get(name_at + 1)?.text != "=" {
        return None;
    }
    let object_at = if tokens.get(name_at + 2)?.text == "+"
        && is_object_name(tokens.get(name_at + 3)?)
        && tokens.get(name_at + 4)?.text == "."
        && tokens.get(name_at + 5)?.text == "length"
    {
        name_at + 3
    } else if is_object_name(tokens.get(name_at + 2)?)
        && tokens.get(name_at + 3)?.text == "."
        && tokens.get(name_at + 4)?.text == "length"
    {
        name_at + 2
    } else {
        return None;
    };
    Some((tokens[name_at].text, tokens[object_at].text, object_at + 2))
}

fn collect_length_bindings<'a>(
    tokens: &[Token<'a>],
    start: usize,
    end: usize,
    out: &mut Vec<(&'a str, &'a str)>,
) {
    let mut index = start;
    while index < end {
        if matches!(tokens[index].text, "var" | "let" | "const" | ",") {
            index += 1;
            continue;
        }
        if let Some((name, object, length_at)) = length_binding(tokens, index) {
            let after = length_at + 1;
            if after < end && !matches!(tokens[after].text, ",") {
                index += 1;
                continue;
            }
            out.retain(|(existing, _)| *existing != name);
            out.push((name, object));
            index = after;
            continue;
        }
        index += 1;
    }
}

fn previous_statement_range(tokens: &[Token<'_>], for_at: usize) -> Option<(usize, usize)> {
    let semi = for_at.checked_sub(1)?;
    if tokens[semi].text != ";" {
        return None;
    }
    let mut depth = 0i32;
    let mut index = semi;
    while index > 0 {
        index -= 1;
        match tokens[index].text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" => {
                if depth == 0 {
                    return Some((index + 1, semi));
                }
                depth -= 1;
            }
            ";" if depth == 0 => return Some((index + 1, semi)),
            _ => {}
        }
    }
    (depth == 0).then_some((0, semi))
}

pub(crate) fn fold_cached_length_conditions(
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
        if semis.len() != 2 {
            continue;
        }
        let mut bindings = Vec::<(&str, &str)>::new();
        collect_length_bindings(&tokens, for_at + 2, semis[0], &mut bindings);
        if let Some((start, end)) = previous_statement_range(&tokens, for_at) {
            collect_length_bindings(&tokens, start, end, &mut bindings);
        }
        if bindings.is_empty() {
            continue;
        }
        let mut index = semis[0] + 1;
        while index + 2 < semis[1] {
            if is_object_name(&tokens[index])
                && tokens[index + 1].text == "."
                && tokens[index + 2].text == "length"
                && tokens.get(index + 3).map(|token| token.text) != Some("=")
            {
                let object = tokens[index].text;
                if let Some((name, _)) = bindings.iter().rev().find(|(_, obj)| *obj == object) {
                    replacements.push((
                        tokens[index].start,
                        tokens[index + 2].end,
                        (*name).to_string(),
                    ));
                    index += 3;
                    continue;
                }
            }
            index += 1;
        }
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_cached_member_reads(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        let name_at = if matches!(tokens[cursor].text, "var" | "let") {
            cursor + 1
        } else {
            cursor
        };
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
            || !tokens.get(name_at + 2).is_some_and(is_object_name)
            || tokens.get(name_at + 3).map(|token| token.text) != Some(".")
            || tokens
                .get(name_at + 4)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                tokens.get(name_at + 5).map(|token| token.text),
                Some(",") | Some(";") | Some("}") | None
            )
        {
            cursor += 1;
            continue;
        }
        let name = tokens[name_at].text;
        let object = tokens[name_at + 2].text;
        let prop = tokens[name_at + 4].text;
        let matching_open = matching_openers(&matching_close);
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        let mut scan = name_at + 5;
        if matches!(
            tokens.get(scan).map(|token| token.text),
            Some(",") | Some(";")
        ) {
            scan += 1;
        }
        while scan + 2 < scope_end {
            if let Some(close) = nested_function_end(&tokens, &matching_close, scan) {
                if let Some(body) = matching_open.get(close).copied().flatten() {
                    let rebinds_this_or_arguments = tokens[scan].text != "=>";
                    if rebinds_this_or_arguments && matches!(object, "this" | "arguments")
                        || function_binds_name(
                            &tokens,
                            &matching_close,
                            &matching_open,
                            body,
                            close,
                            object,
                        )
                        || function_binds_name(
                            &tokens,
                            &matching_close,
                            &matching_open,
                            body,
                            close,
                            name,
                        )
                    {
                        scan = close + 1;
                        continue;
                    }
                } else {
                    scan = close + 1;
                    continue;
                }
            }
            if tokens[scan].kind == TokenKind::Identifier && tokens[scan].text == name {
                if tokens.get(scan + 1).map(|token| token.text) == Some("=")
                    && tokens.get(scan + 2).map(|token| token.text) != Some("=")
                {
                    break;
                }
            }
            let prev = scan
                .checked_sub(1)
                .map(|index| tokens[index].text)
                .unwrap_or(";");
            if is_object_name(&tokens[scan]) && tokens[scan].text == object {
                // Any store through the object can change the cached member:
                // rebinding the object, a computed access, deleting the
                // property, or writing `object.prop` directly (including
                // compound assignments and increments).
                let next = tokens.get(scan + 1).map(|token| token.text);
                if prev == "delete"
                    || next == Some("[")
                    || next.is_some_and(|text| {
                        text.ends_with('=')
                            && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
                    })
                {
                    break;
                }
                if next == Some(".") && tokens.get(scan + 2).map(|token| token.text) == Some(prop) {
                    let after = tokens.get(scan + 3).map(|token| token.text);
                    if after.is_some_and(|text| {
                        matches!(text, "++" | "--")
                            || text.ends_with('=')
                                && !matches!(
                                    text,
                                    "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>"
                                )
                    }) {
                        break;
                    }
                    if prev != ":" {
                        replacements.push((
                            tokens[scan].start,
                            tokens[scan + 2].end,
                            name.to_string(),
                        ));
                        scan += 3;
                        continue;
                    }
                }
            }
            scan += 1;
        }
        cursor = name_at + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_repeated_member_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 10 < tokens.len() {
        let prev = cursor
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";");
        if !matches!(prev, ";" | "{" | "}")
            || !is_object_name(&tokens[cursor])
            || tokens.get(cursor + 1).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 3).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 4)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 5).map(|token| token.text) != Some(";")
            || tokens.get(cursor + 6).map(|token| token.text) != Some(tokens[cursor].text)
            || tokens.get(cursor + 7).map(|token| token.text) != Some(".")
            || tokens
                .get(cursor + 8)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens[cursor + 8].text == tokens[cursor + 2].text
            || tokens.get(cursor + 9).map(|token| token.text) != Some("=")
            || tokens.get(cursor + 10).map(|token| token.text) != Some(tokens[cursor + 4].text)
        {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[cursor + 10].end,
            format!(
                "{}.{}={}.{}={}",
                tokens[cursor].text,
                tokens[cursor + 2].text,
                tokens[cursor].text,
                tokens[cursor + 8].text,
                tokens[cursor + 4].text
            ),
        ));
        cursor += 11;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_single_property_objects(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 10 < tokens.len() {
        let name_at = if matches!(tokens[cursor].text, "var" | "let") {
            cursor + 1
        } else {
            cursor
        };
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
            || tokens.get(name_at + 2).map(|token| token.text) != Some("{")
            || tokens.get(name_at + 3).map(|token| token.text) != Some("}")
            || tokens.get(name_at + 4).map(|token| token.text) != Some(";")
            || tokens.get(name_at + 5).map(|token| token.text) != Some(tokens[name_at].text)
            || tokens.get(name_at + 6).map(|token| token.text) != Some(".")
            || tokens
                .get(name_at + 7)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 8).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[name_at].text;
        let Some(value_stop) = top_level_stop(&tokens, name_at + 9, &[",", ";"]) else {
            cursor += 1;
            continue;
        };
        if identifier_occurs(&tokens, name_at + 9, value_stop, name) {
            cursor += 1;
            continue;
        }
        let prop = tokens[name_at + 7].text;
        let value = &source[tokens[name_at + 9].start..tokens[value_stop].start];
        let matching_close = matching_closers(&tokens);
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        let uses = collect_unbound_name_uses(
            &tokens,
            &matching_close,
            name,
            value_stop + 1,
            scope_end,
            name_at,
        );
        if uses.len() != 1 {
            cursor += 1;
            continue;
        }
        let use_at = uses[0];
        replacements.push((tokens[cursor].start, tokens[value_stop].end, String::new()));
        replacements.push((
            tokens[use_at].start,
            tokens[use_at].end,
            format!("{{{prop}:{value}}}"),
        ));
        cursor = value_stop + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn enclosing_brace(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
) -> Option<usize> {
    matching_close
        .iter()
        .enumerate()
        .filter_map(|(open, close)| {
            let close = (*close)?;
            (open < at && at < close && tokens[open].text == "{").then_some(open)
        })
        .max()
}

fn brace_is_object_literal(tokens: &[Token<'_>], open: usize) -> bool {
    let prev = open
        .checked_sub(1)
        .map(|index| tokens[index].text)
        .unwrap_or(";");
    if prev == ":" {
        let colon = open - 1;
        for index in (0..colon).rev() {
            match tokens[index].text {
                "case" | "default" => return false,
                ";" | "{" | "}" => break,
                _ => {}
            }
        }
        return true;
    }
    matches!(
        prev,
        "=" | "(" | "[" | "," | "?" | "return" | "&&" | "||" | "??" | "!"
    )
}

pub(crate) fn fold_object_property_functions_to_methods(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some(":")
        {
            cursor += 1;
            continue;
        }
        let Some(open) = enclosing_brace(&tokens, &matching_close, cursor) else {
            cursor += 1;
            continue;
        };
        if !brace_is_object_literal(&tokens, open) {
            cursor += 1;
            continue;
        }
        let Some(function) = parse_function_expression(&tokens, &matching_close, cursor + 2) else {
            cursor += 1;
            continue;
        };
        let Some(block_open) = function.block_open else {
            cursor += 1;
            continue;
        };
        if function.named
            || !simple_identifier_params(&tokens, function.params_from, function.params_to)
        {
            cursor += 1;
            continue;
        }
        if function.is_arrow
            && own_body_has_this_or_arguments(&tokens, &matching_close, block_open, function.end)
        {
            cursor += 1;
            continue;
        }
        if !matches!(
            tokens.get(function.end + 1).map(|token| token.text),
            Some(",") | Some("}")
        ) {
            cursor += 1;
            continue;
        }
        let key = tokens[cursor].text;
        let params = if function.params_from == function.params_to {
            String::new()
        } else {
            source[tokens[function.params_from].start..tokens[function.params_to - 1].end]
                .to_string()
        };
        let body = &source[tokens[block_open + 1].start..tokens[function.end].start];
        replacements.push((
            tokens[cursor].start,
            tokens[function.end].end,
            format!("{key}({params}){{{body}}}"),
        ));
        cursor = function.end + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}
