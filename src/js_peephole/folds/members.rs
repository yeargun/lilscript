use crate::js_peephole::rewrite::{
    apply_token_rewrites, identifier_occurs, is_property_identifier, is_statement_boundary,
    top_level_stop,
};
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
        // A property write like `e.currentTarget=o.elem` is not a cache
        // binding: no variable named after the property exists at all.
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || is_property_identifier(&tokens, name_at)
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
                                && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>")
                    }) {
                        break;
                    }
                    // `object.method(...)` binds `this` to `object`. A cached
                    // function value called as `fn(...)` does not.
                    if after == Some("(") {
                        scan += 3;
                        continue;
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
            || !function.is_arrow
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

fn object_constructor_aliases<'a>(tokens: &'a [Token<'a>]) -> std::collections::HashSet<&'a str> {
    let mut names = std::collections::HashSet::from(["Object"]);
    let mut index = 0usize;
    while index + 2 < tokens.len() {
        if tokens[index].kind == TokenKind::Identifier
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("Object")
        {
            names.insert(tokens[index].text);
        }
        if matches!(tokens[index].text, "var" | "let" | "const")
            && tokens
                .get(index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(index + 2).map(|token| token.text) == Some("=")
            && tokens.get(index + 3).map(|token| token.text) == Some("Object")
        {
            names.insert(tokens[index + 1].text);
        }
        index += 1;
    }
    names
}

fn object_assign_open(tokens: &[Token<'_>], objects: &std::collections::HashSet<&str>, at: usize) -> Option<usize> {
    if tokens.get(at).map(|token| token.text) == Some("Object")
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("assign")
        && tokens.get(at + 3).map(|token| token.text) == Some("(")
    {
        return Some(at + 3);
    }
    if tokens
        .get(at)
        .is_some_and(|token| objects.contains(token.text))
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens.get(at + 2).map(|token| token.text) == Some("assign")
        && tokens.get(at + 3).map(|token| token.text) == Some("(")
    {
        return Some(at + 3);
    }
    None
}

fn assign_result_unused(tokens: &[Token<'_>], call_at: usize, close: usize) -> bool {
    let prev = call_at
        .checked_sub(1)
        .map(|index| tokens[index].text)
        .unwrap_or(";");
    if matches!(prev, "=" | ":" | "return" | "=>") {
        return false;
    }
    if prev == "(" {
        if let Some(open) = call_at.checked_sub(1) {
            if open > 0 {
                let before_paren = tokens[open - 1].text;
                if matches!(before_paren, ")" | "]" | "}")
                    || tokens[open - 1].kind == TokenKind::Identifier
                {
                    return false;
                }
            }
        }
    }
    matches!(
        tokens.get(close + 1).map(|token| token.text),
        None | Some(";") | Some(",") | Some(")") | Some("}")
    )
}

fn parse_plain_assign_props<'a>(
    source: &'a str,
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    open: usize,
) -> Option<Vec<(String, String)>> {
    let close = matching_close.get(open).copied().flatten()?;
    let mut props = Vec::new();
    let mut index = open + 1;
    while index < close {
        if tokens[index].text == "," {
            index += 1;
            continue;
        }
        if tokens[index].text == "..."
            || tokens[index].text == "["
            || tokens[index].text == "__proto__"
        {
            return None;
        }
        if matches!(tokens[index].text, "get" | "set")
            && tokens.get(index + 1).map(|token| token.text) != Some(":")
        {
            return None;
        }
        let key = if tokens[index].kind == TokenKind::Identifier
            || tokens[index].kind == TokenKind::Keyword
        {
            tokens[index].text.to_string()
        } else if let Some(name) = crate::js_peephole::token::ascii_identifier_name_string(tokens[index].text)
        {
            name.to_string()
        } else {
            return None;
        };
        if tokens.get(index + 1).map(|token| token.text) != Some(":") {
            return None;
        }
        let value_start = index + 2;
        let Some(stop) = top_level_stop(tokens, value_start, &[",", "}"]) else {
            return None;
        };
        if stop <= value_start {
            return None;
        }
        let value = source[tokens[value_start].start..tokens[stop].start].to_string();
        if value.trim().is_empty() {
            return None;
        }
        props.push((key, value));
        index = stop;
    }
    (!props.is_empty()).then_some(props)
}

pub(crate) fn fold_object_assign_literal_to_writes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let objects = object_constructor_aliases(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        let Some(open) = object_assign_open(&tokens, &objects, cursor) else {
            cursor += 1;
            continue;
        };
        let Some(close) = matching_close.get(open).copied().flatten() else {
            cursor += 1;
            continue;
        };
        if !assign_result_unused(&tokens, cursor, close) {
            cursor += 1;
            continue;
        }
        let recv_at = open + 1;
        let recv_ok = tokens.get(recv_at).map(|token| token.text) == Some("this")
            || tokens
                .get(recv_at)
                .is_some_and(|token| token.kind == TokenKind::Identifier);
        if !recv_ok || tokens.get(recv_at + 1).map(|token| token.text) != Some(",") {
            cursor += 1;
            continue;
        }
        if tokens.get(recv_at + 2).map(|token| token.text) != Some("{") {
            cursor += 1;
            continue;
        }
        let Some(props) =
            parse_plain_assign_props(source, &tokens, &matching_close, recv_at + 2)
        else {
            cursor += 1;
            continue;
        };
        if matching_close
            .get(recv_at + 2)
            .copied()
            .flatten()
            .is_some_and(|lit_close| lit_close + 1 != close)
        {
            cursor += 1;
            continue;
        }
        let recv = tokens[recv_at].text;
        let mut writes = String::new();
        for (index, (key, value)) in props.iter().enumerate() {
            if index != 0 {
                writes.push(',');
            }
            writes.push_str(recv);
            writes.push('.');
            writes.push_str(key);
            writes.push('=');
            writes.push_str(value);
        }
        replacements.push((tokens[cursor].start, tokens[close].end, writes));
        cursor = close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn method_key_spelling(key: &str) -> String {
    if matches!(key, "get" | "set" | "async") {
        format!("\"{key}\"")
    } else {
        key.to_string()
    }
}

fn member_function_assign_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> Option<usize> {
    if tokens.get(at).map(|token| token.text) != Some(name)
        || is_property_identifier(tokens, at)
        || tokens.get(at + 1).map(|token| token.text) != Some(".")
        || !tokens
            .get(at + 2)
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
        || tokens.get(at + 3).map(|token| token.text) != Some("=")
    {
        return None;
    }
    let function = parse_function_expression(tokens, matching_close, at + 4)?;
    if function.is_arrow || function.named || function.block_open.is_none() {
        return None;
    }
    Some(function.end)
}

fn parse_member_function_assign(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> Option<(usize, String)> {
    let end = member_function_assign_end(tokens, matching_close, at, name)?;
    let function = parse_function_expression(tokens, matching_close, at + 4)?;
    let block_open = function.block_open?;
    debug_assert_eq!(function.end, end);
    let key = tokens[at + 2].text;
    let params = if function.params_from == function.params_to {
        String::new()
    } else {
        source[tokens[function.params_from].start..tokens[function.params_to - 1].end].to_string()
    };
    let body = &source[tokens[block_open + 1].start..tokens[function.end].start];
    Some((
        function.end,
        format!("{}({params}){{{body}}}", method_key_spelling(key)),
    ))
}

fn collect_member_function_run(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    name: &str,
) -> Option<(usize, String)> {
    let mut methods = Vec::new();
    let mut cursor = start;
    let mut last_end = start;
    while let Some((end, method)) =
        parse_member_function_assign(source, tokens, matching_close, cursor, name)
    {
        methods.push(method);
        last_end = end;
        let next = end + 1;
        if matches!(tokens.get(next).map(|token| token.text), Some("," | ";"))
            && parse_member_function_assign(source, tokens, matching_close, next + 1, name)
                .is_some()
        {
            cursor = next + 1;
            continue;
        }
        break;
    }
    if methods.is_empty() {
        return None;
    }
    Some((last_end, methods.join(",")))
}

fn empty_object_inits<'a>(tokens: &'a [Token<'a>]) -> Vec<(usize, &'a str, usize)> {
    let mut inits = Vec::new();
    let mut index = 0usize;
    while index + 3 < tokens.len() {
        if tokens[index].kind == TokenKind::Identifier
            && !is_property_identifier(tokens, index)
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("{")
            && tokens.get(index + 3).map(|token| token.text) == Some("}")
        {
            inits.push((index, tokens[index].text, index + 3));
            index += 4;
            continue;
        }
        index += 1;
    }
    inits
}

fn top_level_value_use(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
    name: &str,
) -> bool {
    let mut index = start;
    while index < end {
        if let Some(nested) = nested_function_end(tokens, matching_close, index) {
            index = nested + 1;
            continue;
        }
        if tokens[index].text == name
            && !is_property_identifier(tokens, index)
            && member_function_assign_end(tokens, matching_close, index, name).is_none()
        {
            return true;
        }
        index += 1;
    }
    false
}

pub(crate) fn fold_empty_object_method_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for (name_at, name, empty_close) in empty_object_inits(&tokens) {
        let after_empty = empty_close + 1;
        if tokens.get(after_empty).map(|token| token.text) == Some(";") {
            if let Some((end, methods)) =
                collect_member_function_run(source, &tokens, &matching_close, after_empty + 1, name)
            {
                replacements.push((
                    tokens[empty_close - 1].start,
                    tokens[empty_close].end,
                    format!("{{{methods}}}"),
                ));
                replacements.push((tokens[after_empty].end, tokens[end].end, String::new()));
                continue;
            }
        }
        let mut scan = after_empty;
        while scan < tokens.len() {
            if tokens[scan].text != name {
                scan += 1;
                continue;
            }
            let Some((end, methods)) =
                collect_member_function_run(source, &tokens, &matching_close, scan, name)
            else {
                scan += 1;
                continue;
            };
            if top_level_value_use(&tokens, &matching_close, after_empty, scan, name) {
                scan = end + 1;
                continue;
            }
            replacements.push((tokens[name_at + 1].start, tokens[empty_close].end, String::new()));
            replacements.push((
                tokens[scan].start,
                tokens[end].end,
                format!("{name}={{{methods}}}"),
            ));
            break;
        }
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn parse_push_call<'a>(
    source: &'a str,
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> Option<(usize, &'a str)> {
    if tokens.get(at).map(|token| token.text) != Some(name)
        || is_property_identifier(tokens, at)
        || tokens.get(at + 1).map(|token| token.text) != Some(".")
        || tokens.get(at + 2).map(|token| token.text) != Some("push")
        || tokens.get(at + 3).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let close = matching_close.get(at + 3).copied().flatten()?;
    if close == at + 4 {
        return None;
    }
    Some((
        close,
        &source[tokens[at + 4].start..tokens[close].start],
    ))
}

pub(crate) fn fold_consecutive_array_pushes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        let Some(name) = tokens.get(cursor).map(|token| token.text) else {
            cursor += 1;
            continue;
        };
        if tokens[cursor].kind != TokenKind::Identifier && tokens[cursor].text != "this" {
            cursor += 1;
            continue;
        }
        let Some((first_close, first_args)) =
            parse_push_call(source, &tokens, &matching_close, cursor, name)
        else {
            cursor += 1;
            continue;
        };
        let mut args = vec![first_args.to_string()];
        let mut last = first_close;
        let mut next = first_close + 1;
        while tokens.get(next).map(|token| token.text) == Some(",") {
            let Some((close, extra)) =
                parse_push_call(source, &tokens, &matching_close, next + 1, name)
            else {
                break;
            };
            args.push(extra.to_string());
            last = close;
            next = close + 1;
        }
        if args.len() > 1 {
            replacements.push((
                tokens[cursor].start,
                tokens[last].end,
                format!("{name}.push({})", args.join(",")),
            ));
            cursor = last + 1;
            continue;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn empty_array_assign(tokens: &[Token<'_>], at: usize) -> Option<usize> {
    let name_at = if matches!(tokens.get(at).map(|token| token.text), Some("var" | "let" | "const"))
    {
        at + 1
    } else {
        at
    };
    if tokens
        .get(name_at)
        .is_none_or(|token| token.kind != TokenKind::Identifier)
        || is_property_identifier(tokens, name_at)
        || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        || tokens.get(name_at + 2).map(|token| token.text) != Some("[")
        || tokens.get(name_at + 3).map(|token| token.text) != Some("]")
    {
        return None;
    }
    Some(name_at)
}

pub(crate) fn fold_push_built_arrays(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        let Some(name_at) = empty_array_assign(&tokens, cursor) else {
            cursor += 1;
            continue;
        };
        let name = tokens[name_at].text;
        let after_empty = name_at + 4;
        if !matches!(
            tokens.get(after_empty).map(|token| token.text),
            Some("," | ";")
        ) {
            cursor += 1;
            continue;
        }
        let mut elems = Vec::new();
        let mut last = after_empty;
        let mut next = after_empty + 1;
        while let Some((close, args)) =
            parse_push_call(source, &tokens, &matching_close, next, name)
        {
            elems.push(args.to_string());
            last = close;
            if matches!(tokens.get(close + 1).map(|token| token.text), Some("," | ";"))
            {
                next = close + 2;
                continue;
            }
            break;
        }
        if elems.is_empty() {
            cursor += 1;
            continue;
        }
        let consume_at = if matches!(tokens.get(last + 1).map(|token| token.text), Some("," | ";"))
        {
            last + 2
        } else {
            cursor += 1;
            continue;
        };
        let Some((consume_end, name_use)) =
            consume_name_as_value(&tokens, &matching_close, consume_at, name)
        else {
            cursor += 1;
            continue;
        };
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, consume_end + 1, scope_end, name)
            && !later_assign_resets(&tokens, consume_end + 1, name)
        {
            cursor += 1;
            continue;
        }
        let literal = format!("[{}]", elems.join(","));
        let mut consumed = String::new();
        consumed.push_str(&source[tokens[consume_at].start..tokens[name_use].start]);
        consumed.push_str(&literal);
        consumed.push_str(&source[tokens[name_use].end..tokens[consume_end].end]);
        let (from, consumed) = if cursor > 0 && tokens[cursor - 1].text == "," {
            (tokens[cursor - 1].start, format!(";{consumed}"))
        } else {
            (tokens[cursor].start, consumed)
        };
        replacements.push((from, tokens[consume_end].end, consumed));
        cursor = consume_end + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_while_push_to_map(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 16 < tokens.len() {
        let Some(name_at) = empty_array_assign(&tokens, cursor) else {
            cursor += 1;
            continue;
        };
        let name = tokens[name_at].text;
        if !matches!(
            tokens.get(name_at + 4).map(|token| token.text),
            Some("," | ";")
        ) {
            cursor += 1;
            continue;
        }
        let index_at = name_at + 5;
        if tokens
            .get(index_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(index_at + 1).map(|token| token.text) != Some("=")
            || tokens.get(index_at + 2).map(|token| token.text) != Some("0")
            || !matches!(
                tokens.get(index_at + 3).map(|token| token.text),
                Some("," | ";")
            )
            || tokens.get(index_at + 4).map(|token| token.text) != Some("while")
            || tokens.get(index_at + 5).map(|token| token.text) != Some("(")
            || tokens.get(index_at + 6).map(|token| token.text) != Some(tokens[index_at].text)
            || tokens.get(index_at + 7).map(|token| token.text) != Some("<")
        {
            cursor += 1;
            continue;
        }
        let index = tokens[index_at].text;
        let arr_at = index_at + 8;
        let Some(arr_end) = simple_member_root_end(&tokens, arr_at) else {
            cursor += 1;
            continue;
        };
        if tokens.get(arr_end + 1).map(|token| token.text) != Some(".")
            || tokens.get(arr_end + 2).map(|token| token.text) != Some("length")
            || tokens.get(arr_end + 3).map(|token| token.text) != Some(")")
        {
            cursor += 1;
            continue;
        }
        let mut push_at = arr_end + 4;
        let mut loop_close = None;
        if tokens.get(push_at).map(|token| token.text) == Some("{") {
            loop_close = matching_close.get(push_at).copied().flatten();
            push_at += 1;
        }
        if tokens.get(push_at).map(|token| token.text) != Some(name)
            || tokens.get(push_at + 1).map(|token| token.text) != Some(".")
            || tokens.get(push_at + 2).map(|token| token.text) != Some("push")
            || tokens.get(push_at + 3).map(|token| token.text) != Some("(")
            || tokens
                .get(push_at + 4)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(push_at + 5).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let mapper = tokens[push_at + 4].text;
        let Some(inner_close) = matching_close.get(push_at + 5).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let Some(push_close) = matching_close.get(push_at + 3).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let arr = &source[tokens[arr_at].start..tokens[arr_end].end];
        let inner = &source[tokens[push_at + 6].start..tokens[inner_close].start];
        if inner != format!("{arr}[{index}]") || inner_close + 1 != push_close {
            cursor += 1;
            continue;
        }
        let incr_at = if matches!(
            tokens.get(push_close + 1).map(|token| token.text),
            Some("," | ";")
        ) {
            push_close + 2
        } else {
            cursor += 1;
            continue;
        };
        let incr_end = if tokens.get(incr_at).map(|token| token.text) == Some(index)
            && tokens.get(incr_at + 1).map(|token| token.text) == Some("++")
        {
            incr_at + 1
        } else {
            cursor += 1;
            continue;
        };
        let after_loop = if let Some(close) = loop_close {
            if incr_end + 1 != close
                && !(tokens.get(incr_end + 1).map(|token| token.text) == Some(";")
                    && incr_end + 2 == close)
            {
                cursor += 1;
                continue;
            }
            close + 1
        } else {
            incr_end + 1
        };
        let consume_at = if matches!(
            tokens.get(after_loop).map(|token| token.text),
            Some("," | ";")
        ) {
            after_loop + 1
        } else {
            after_loop
        };
        let Some((consume_end, name_use)) =
            consume_name_as_value(&tokens, &matching_close, consume_at, name)
        else {
            cursor += 1;
            continue;
        };
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, consume_end + 1, scope_end, index)
            || identifier_occurs(&tokens, consume_end + 1, scope_end, name)
        {
            cursor += 1;
            continue;
        }
        let mapped = format!("{arr}.map({mapper})");
        let mut consumed = String::new();
        consumed.push_str(&source[tokens[consume_at].start..tokens[name_use].start]);
        consumed.push_str(&mapped);
        consumed.push_str(&source[tokens[name_use].end..tokens[consume_end].end]);
        let (from, consumed) = if cursor > 0 && tokens[cursor - 1].text == "," {
            (tokens[cursor - 1].start, format!(";{consumed}"))
        } else {
            (tokens[cursor].start, consumed)
        };
        replacements.push((from, tokens[consume_end].end, consumed));
        cursor = consume_end + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn simple_member_root_end(tokens: &[Token<'_>], at: usize) -> Option<usize> {
    if !tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == "this")
    {
        return None;
    }
    if tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens
            .get(at + 2)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(at + 3).map(|token| token.text) == Some(".")
        && tokens.get(at + 4).map(|token| token.text) == Some("length")
    {
        return Some(at + 2);
    }
    Some(at)
}

fn later_assign_resets(tokens: &[Token<'_>], start: usize, name: &str) -> bool {
    tokens[start..].iter().enumerate().any(|(offset, token)| {
        token.text == name
            && !is_property_identifier(tokens, start + offset)
            && tokens.get(start + offset + 1).map(|token| token.text) == Some("=")
    })
}

fn consume_name_as_value(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    at: usize,
    name: &str,
) -> Option<(usize, usize)> {
    if tokens.get(at).map(|token| token.text) == Some(name)
        && !is_property_identifier(tokens, at)
        && matches!(
            tokens.get(at + 1).map(|token| token.text),
            Some(";" | "," | ")" | "}" | "]") | None
        )
    {
        return Some((at, at));
    }
    if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == "this")
        && !is_property_identifier(tokens, at)
    {
        let eq = if tokens.get(at + 1).map(|token| token.text) == Some("=") {
            at + 1
        } else if tokens.get(at + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(at + 2)
                .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
            && tokens.get(at + 3).map(|token| token.text) == Some("=")
        {
            at + 3
        } else {
            0
        };
        if eq != 0
            && tokens.get(eq + 1).map(|token| token.text) == Some(name)
            && !is_property_identifier(tokens, eq + 1)
        {
            return Some((eq + 1, eq + 1));
        }
    }
    let open = if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == "this")
        && tokens.get(at + 1).map(|token| token.text) == Some("(")
    {
        at + 1
    } else if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == "this")
        && tokens.get(at + 1).map(|token| token.text) == Some(".")
        && tokens
            .get(at + 2)
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
        && tokens.get(at + 3).map(|token| token.text) == Some("(")
    {
        at + 3
    } else {
        return None;
    };
    let close = matching_close.get(open).copied().flatten()?;
    let mut name_use = None;
    let mut index = open + 1;
    while index < close {
        if tokens[index].text == name && !is_property_identifier(tokens, index) {
            if name_use.is_some() {
                return None;
            }
            name_use = Some(index);
        }
        index += 1;
    }
    Some((close, name_use?))
}

fn push_only_function_body<'a>(
    source: &'a str,
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    body_open: usize,
    body_close: usize,
) -> Option<(&'a str, String)> {
    let mut index = body_open + 1;
    let mut target = None;
    let mut elems = Vec::new();
    while index < body_close {
        if matches!(tokens[index].text, ";" | ",") {
            index += 1;
            continue;
        }
        let name = tokens[index].text;
        let (close, args) = parse_push_call(source, tokens, matching_close, index, name)?;
        match target {
            None => target = Some(name),
            Some(existing) if existing == name => {}
            _ => return None,
        }
        elems.push(args.to_string());
        index = close + 1;
    }
    let name = target?;
    if elems.is_empty() {
        return None;
    }
    Some((name, elems.join(",")))
}

pub(crate) fn fold_push_only_init_function(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if tokens[cursor].text != "function"
            || !tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            || tokens.get(cursor + 2).map(|token| token.text) != Some("(")
            || tokens.get(cursor + 3).map(|token| token.text) != Some(")")
            || tokens.get(cursor + 4).map(|token| token.text) != Some("{")
            || !is_statement_boundary(&tokens, cursor)
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor + 1].text;
        let Some(body_close) = matching_close.get(cursor + 4).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let Some((array, elems)) =
            push_only_function_body(source, &tokens, &matching_close, cursor + 4, body_close)
        else {
            cursor += 1;
            continue;
        };
        let mut calls = Vec::new();
        let mut other_use = false;
        let mut index = 0usize;
        while index < tokens.len() {
            if index == cursor + 1 {
                index += 1;
                continue;
            }
            if tokens[index].text == name && !is_property_identifier(&tokens, index) {
                if tokens.get(index + 1).map(|token| token.text) == Some("(")
                    && tokens.get(index + 2).map(|token| token.text) == Some(")")
                {
                    calls.push(index);
                    index += 3;
                    continue;
                }
                other_use = true;
                break;
            }
            index += 1;
        }
        if other_use || calls.len() != 1 {
            cursor = body_close + 1;
            continue;
        }
        let call_at = calls[0];
        let Some(empty_at) = tokens.iter().enumerate().find_map(|(index, token)| {
            (token.text == array
                && !is_property_identifier(&tokens, index)
                && tokens.get(index + 1).map(|token| token.text) == Some("=")
                && tokens.get(index + 2).map(|token| token.text) == Some("[")
                && tokens.get(index + 3).map(|token| token.text) == Some("]"))
            .then_some(index)
        }) else {
            cursor = body_close + 1;
            continue;
        };
        replacements.push((
            tokens[empty_at + 2].start,
            tokens[empty_at + 3].end,
            format!("[{elems}]"),
        ));
        let call_end = if tokens.get(call_at + 3).map(|token| token.text) == Some(";") {
            tokens[call_at + 3].end
        } else {
            tokens[call_at + 2].end
        };
        let call_start = if call_at > 0 && tokens[call_at - 1].text == "," {
            tokens[call_at - 1].start
        } else {
            tokens[call_at].start
        };
        replacements.push((call_start, call_end, String::new()));
        let fn_end = if tokens.get(body_close + 1).map(|token| token.text) == Some(";") {
            tokens[body_close + 1].end
        } else {
            tokens[body_close].end
        };
        replacements.push((tokens[cursor].start, fn_end, String::new()));
        cursor = body_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

#[cfg(test)]
mod tests {
    use super::fold_object_assign_literal_to_writes;

    #[test]
    fn expands_unused_object_assign_literal() {
        let source = "function f(c){Object.assign(this,{a:c,f:new Set,K:0});return this}";
        let (out, count) = fold_object_assign_literal_to_writes(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("this.a=c,this.f=new Set,this.K=0"), "{out}");
        assert!(!out.contains("Object.assign"), "{out}");
    }

    #[test]
    fn keeps_object_assign_when_result_is_used() {
        let source = "function f(c){var e=Object.assign(this,{a:c});return e}";
        let (out, count) = fold_object_assign_literal_to_writes(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert!(out.contains("Object.assign"), "{out}");
    }
}
