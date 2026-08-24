use crate::js_peephole::rewrite::{
    apply_token_rewrites, assign_is_in_declaration, identifier_is_read, identifier_occurs,
    is_property_identifier, is_statement_boundary, next_statement_end, replacement_overlaps,
    top_level_stop,
};
use crate::js_peephole::scope::{
    enclosing_block_end, enclosing_function_span, function_scope_declares,
    name_is_declared_in_any_enclosing_function_scope, name_is_declared_in_any_enclosing_scope,
    name_is_module_var_binding, name_is_used_in_scope,
};
use crate::js_peephole::token::{lex, lex_certainly, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

/// Move declaration-only function-local `var` bindings before the initialized
/// bindings in the same declaration while retaining initializer order.
///
/// ECMAScript instantiates every `var` binding as `undefined` before executing
/// any initializer. An uninitialized declarator therefore performs no runtime
/// operation at its textual position, including in a `for` initializer. This
/// reorder preserves initializer order, suspension, exceptions, closure and
/// direct-eval visibility, but can give gzip and Brotli a better declaration
/// layout. Top-level declarations are excluded because script-mode `var` can
/// create global-object properties in source order. The untouched artifact
/// remains the other exact-codec leaf.
///
/// This deliberately refuses `let`/`const` (TDZ ordering is observable),
/// destructuring, `for-in`/`for-of`, ASI-dependent boundaries, and comments in
/// the separators whose association would otherwise move. The compact lexer
/// cannot distinguish regex punctuation, so callers must provide the same
/// emitter proof used by the other generated-syntax canonicalizations.
pub(crate) fn reorder_uninitialized_var_declarators(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    if source.contains(['\u{2028}', '\u{2029}']) {
        // ECMAScript treats both Unicode separators as line terminators. The
        // compact lexer deliberately keeps them as punctuation, so fail
        // closed instead of risking an ASI boundary.
        return Ok((source.to_string(), 0));
    }
    let Some(tokens) = lex_certainly(source)? else {
        return Ok((source.to_string(), 0));
    };
    let matching_close = matching_closers(&tokens);
    let mut matching_open = vec![None; tokens.len()];
    for (open, close) in matching_close.iter().enumerate() {
        if let Some(close) = close {
            matching_open[*close] = Some(open);
        }
    }
    let function_bodies = (0..tokens.len())
        .filter(|index| tokens[*index].text == "{")
        .filter_map(|body| {
            let end = matching_close[body]?;
            if body
                .checked_sub(1)
                .is_some_and(|previous| tokens[previous].text == "=>")
            {
                return Some((body, end));
            }
            let close_params = body.checked_sub(1)?;
            if tokens[close_params].text != ")" {
                return None;
            }
            let open_params = matching_open[close_params]?;
            function_header_precedes_parameters(&tokens, open_params).then_some((body, end))
        })
        .collect::<Vec<_>>();
    let mut replacements = Vec::<(usize, usize, String)>::new();

    for var_index in 0..tokens.len() {
        if tokens[var_index].text != "var"
            || !tokens
                .get(var_index + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            // Top-level script `var` bindings can create global-object
            // properties in declaration order. Function-local `var`
            // bindings have no corresponding property-order observation.
            || !function_bodies
                .iter()
                .any(|(start, end)| *start < var_index && var_index < *end)
        {
            continue;
        }

        let mut delimiters = Vec::<&str>::new();
        let mut segment_start = var_index + 1;
        let mut segments = Vec::<(usize, usize, bool)>::new();
        let mut separator_commas = Vec::<usize>::new();
        let mut cursor = segment_start;
        let mut complete = false;
        while cursor < tokens.len() {
            if delimiters.is_empty()
                && cursor > segment_start
                && source[tokens[cursor - 1].end..tokens[cursor].start]
                    .bytes()
                    .any(|byte| matches!(byte, b'\n' | b'\r'))
            {
                // A line terminator can end the declaration through ASI. Do
                // not consume a following comma expression while searching
                // for a later explicit semicolon.
                break;
            }
            match tokens[cursor].text {
                "(" | "[" | "{" => delimiters.push(tokens[cursor].text),
                ")" | "]" | "}" if !delimiters.is_empty() => {
                    delimiters.pop();
                }
                "," if delimiters.is_empty() => {
                    let Some(initialized) = simple_var_declarator(&tokens, segment_start, cursor)
                    else {
                        break;
                    };
                    segments.push((segment_start, cursor, initialized));
                    separator_commas.push(cursor);
                    segment_start = cursor + 1;
                }
                ";" if delimiters.is_empty() => {
                    let Some(initialized) = simple_var_declarator(&tokens, segment_start, cursor)
                    else {
                        break;
                    };
                    segments.push((segment_start, cursor, initialized));
                    complete = true;
                    break;
                }
                // A multi-binding declaration cannot be a valid for-in/of
                // initializer. Refusing here also prevents treating the
                // iterable expression as part of a declarator.
                "in" | "of" if delimiters.is_empty() => break,
                // The generated candidate normally includes a semicolon.
                // Do not infer line-sensitive ASI boundaries from token gaps.
                ")" | "]" | "}" if delimiters.is_empty() => break,
                _ => {}
            }
            cursor += 1;
        }
        if !complete || segments.len() < 2 {
            continue;
        }

        let declaration_prefix = &source[tokens[var_index].end..tokens[segments[0].0].start];
        if !declaration_prefix
            .bytes()
            .all(|byte| byte.is_ascii_whitespace())
        {
            continue;
        }
        let clean_separators = separator_commas.iter().enumerate().all(|(index, comma)| {
            let left_end = tokens[segments[index].1 - 1].end;
            let right_start = tokens[segments[index + 1].0].start;
            tokens[*comma].start >= left_end
                && tokens[*comma].end <= right_start
                && source[left_end..right_start]
                    .bytes()
                    .all(|byte| byte == b',' || byte.is_ascii_whitespace())
        });
        if !clean_separators {
            continue;
        }

        let first_initialized = segments.iter().position(|segment| segment.2);
        let last_uninitialized = segments.iter().rposition(|segment| !segment.2);
        if !first_initialized
            .zip(last_uninitialized)
            .is_some_and(|(initialized, uninitialized)| initialized < uninitialized)
        {
            continue;
        }

        let ordered = segments
            .iter()
            .filter(|segment| !segment.2)
            .chain(segments.iter().filter(|segment| segment.2));
        let replacement = ordered
            .map(|(start, end, _)| &source[tokens[*start].start..tokens[*end - 1].end])
            .collect::<Vec<_>>()
            .join(",");
        replacements.push((
            tokens[segments[0].0].start,
            tokens[segments.last().expect("multiple segments").1 - 1].end,
            replacement,
        ));
    }

    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
    }
    replacements.sort_unstable_by_key(|(start, end, _)| (*start, *end));
    let mut retained = Vec::with_capacity(replacements.len());
    let mut last_end = 0usize;
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

fn function_header_precedes_parameters(tokens: &[Token<'_>], open_params: usize) -> bool {
    let Some(before_params) = open_params.checked_sub(1) else {
        return false;
    };
    let function = if tokens[before_params].text == "function" {
        // Anonymous ordinary functions and methods literally named
        // `function` both introduce a function-local `var` scope.
        Some(before_params)
    } else if tokens[before_params].text == "*" {
        before_params
            .checked_sub(1)
            .filter(|index| tokens[*index].text == "function")
    } else if tokens[before_params].kind == TokenKind::Identifier {
        let before_name = before_params.checked_sub(1);
        before_name
            .and_then(|index| {
                if tokens[index].text == "function" {
                    Some(index)
                } else if tokens[index].text == "*" {
                    index
                        .checked_sub(1)
                        .filter(|function| tokens[*function].text == "function")
                } else {
                    Some(before_params)
                }
            })
            .or(Some(before_params))
    } else {
        None
    };
    function.is_some_and(|index| {
        !index
            .checked_sub(1)
            .is_some_and(|previous| matches!(tokens[previous].text, "." | "?." | "#"))
    })
}

fn simple_var_declarator(tokens: &[Token<'_>], start: usize, end: usize) -> Option<bool> {
    if start >= end || tokens[start].kind != TokenKind::Identifier {
        return None;
    }
    match end - start {
        1 => Some(false),
        _ if tokens[start + 1].text == "=" => Some(true),
        _ => None,
    }
}

pub(crate) fn strip_unused_simple_declarators(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if !matches!(tokens[cursor].text, "let" | "var") {
            cursor += 1;
            continue;
        }
        let mut name_at = cursor + 1;
        let mut kept = Vec::<(usize, usize)>::new();
        let mut any_removed = false;
        loop {
            if tokens
                .get(name_at)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
                || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
                || tokens
                    .get(name_at + 2)
                    .is_none_or(|token| token.kind != TokenKind::Identifier)
            {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                kept.push((name_at, stop));
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            if tokens.get(name_at + 3).map(|token| token.text) == Some(".")
                && tokens
                    .get(name_at + 4)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                && matches!(
                    tokens.get(name_at + 5).map(|token| token.text),
                    Some(",") | Some(";")
                )
            {
                let stop = name_at + 5;
                // A syntactically plain member read can still invoke a getter
                // or Proxy trap. Without ownership/purity proof, an unused
                // destination does not make the read removable.
                kept.push((name_at, stop));
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            if tokens.get(name_at + 3).map(|token| token.text) == Some("[")
                && tokens.get(name_at + 4).is_some_and(|token| {
                    token.kind == TokenKind::Identifier || token.kind == TokenKind::Number
                })
                && tokens.get(name_at + 5).map(|token| token.text) == Some("]")
                && matches!(
                    tokens.get(name_at + 6).map(|token| token.text),
                    Some(",") | Some(";")
                )
            {
                let stop = name_at + 6;
                kept.push((name_at, stop));
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            if tokens.get(name_at + 3).map(|token| token.text) == Some(".") {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                kept.push((name_at, stop));
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let rhs_end = name_at + 2;
            if !matches!(
                tokens.get(rhs_end + 1).map(|token| token.text),
                Some(",") | Some(";")
            ) {
                let Some(stop) = top_level_stop(&tokens, name_at, &[",", ";"]) else {
                    break;
                };
                kept.push((name_at, stop));
                if tokens[stop].text == ";" {
                    break;
                }
                name_at = stop + 1;
                continue;
            }
            let name = tokens[name_at].text;
            let stop = rhs_end + 1;
            if name_is_used_in_scope(&tokens, &matching_close, name_at, stop + 1, name) {
                kept.push((name_at, stop));
            } else {
                any_removed = true;
            }
            if tokens[stop].text == ";" {
                break;
            }
            name_at = stop + 1;
        }
        if any_removed {
            // The statement must end in a same-depth semicolon: a declaration
            // closed by `}` through ASI has no `;` of its own, and scanning
            // past the brace would splice the next statement into the rewrite.
            let Some(semi) = top_level_stop(&tokens, cursor + 1, &[";"]) else {
                cursor += 1;
                continue;
            };
            let replacement = if kept.is_empty() {
                String::new()
            } else {
                let decls = kept
                    .iter()
                    .map(|(start, end)| {
                        source[tokens[*start].start..tokens[*end].start].to_string()
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                format!("{} {decls};", tokens[cursor].text)
            };
            replacements.push((tokens[cursor].start, tokens[semi].end, replacement));
            cursor = semi + 1;
            continue;
        }
        cursor += 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn strip_unused_for_init_vars(
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
        if tokens.get(for_at + 2).map(|token| token.text) != Some("var")
            && tokens.get(for_at + 2).map(|token| token.text) != Some("let")
        {
            continue;
        }
        if matching_close.get(for_at + 1).copied().flatten().is_none() {
            continue;
        }
        if tokens
            .get(for_at + 3)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(for_at + 4).map(|token| token.text) != Some(",")
        {
            continue;
        }
        let name = tokens[for_at + 3].text;
        let scope_end = enclosing_block_end(&matching_close, for_at).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, for_at + 4, scope_end, name) {
            continue;
        }
        replacements.push((
            tokens[for_at + 3].start,
            tokens[for_at + 4].end,
            String::new(),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_dead_initializer_reassigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        let decl = if matches!(tokens[cursor].text, "var" | "let")
            && tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            Some((tokens[cursor].text, cursor + 1))
        } else {
            None
        };
        let Some((kind, name_at)) = decl else {
            cursor += 1;
            continue;
        };
        if tokens.get(name_at + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(name_at + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 3).map(|token| token.text) != Some(";")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[name_at].text;
        if tokens.get(name_at + 4).map(|token| token.text) != Some(name)
            || tokens.get(name_at + 5).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(semi) = top_level_stop(&tokens, name_at + 6, &[";"]) else {
            cursor += 1;
            continue;
        };
        if identifier_occurs(&tokens, name_at + 6, semi, name) {
            cursor += 1;
            continue;
        }
        let rhs = &source[tokens[name_at + 6].start..tokens[semi].start];
        replacements.push((
            tokens[cursor].start,
            tokens[semi].end,
            format!("{kind} {name}={rhs};"),
        ));
        cursor = semi + 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn strip_void_initializer_before_write(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for (name_at, _, _) in uninitialized_var_declarators(&tokens) {
        if name_at + 3 >= tokens.len()
            || tokens[name_at + 1].text != "="
            || tokens[name_at + 2].text != "void"
            || tokens[name_at + 3].text != "0"
        {
            continue;
        }
        let name = tokens[name_at].text;
        let scope_end = enclosing_block_end(&matching_close, name_at).unwrap_or(tokens.len());
        let first_use = (name_at + 4..scope_end).find(|&index| {
            tokens[index].kind == TokenKind::Identifier && tokens[index].text == name
        });
        let Some(use_at) = first_use else {
            continue;
        };
        if tokens.get(use_at + 1).map(|token| token.text) != Some("=")
            || tokens.get(use_at + 2).map(|token| token.text) == Some("=")
        {
            continue;
        }
        let Some(rhs_end) = top_level_stop(&tokens, use_at + 2, &[",", ";", "}"]) else {
            continue;
        };
        // A syntactic assignment is not necessarily a write-before-read. In
        // `var x=void 0;x=condition?value:x`, the initializer is the reset
        // observed by the false arm. This matters in loops because `var x`
        // alone only initializes the binding once, on function entry.
        if identifier_occurs(&tokens, use_at + 2, rhs_end, name) {
            continue;
        }
        replacements.push((
            tokens[name_at + 1].start,
            tokens[name_at + 3].end,
            String::new(),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_uninitialized_var_into_assign(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut var_index = 0usize;
    while var_index < tokens.len() {
        if tokens[var_index].text != "var"
            || var_index
                .checked_sub(2)
                .is_some_and(|index| tokens[index].text == "for" && tokens[index + 1].text == "(")
        {
            var_index += 1;
            continue;
        }
        let mut delimiters = Vec::<&str>::new();
        let mut segment_start = var_index + 1;
        let mut segments = Vec::<(usize, usize)>::new();
        let mut cursor = segment_start;
        let mut semicolon = None;
        while cursor < tokens.len() {
            match tokens[cursor].text {
                "(" | "[" | "{" => delimiters.push(tokens[cursor].text),
                ")" | "]" | "}" => {
                    if delimiters.pop().is_none() {
                        break;
                    }
                }
                "," if delimiters.is_empty() => {
                    segments.push((segment_start, cursor));
                    segment_start = cursor + 1;
                }
                ";" if delimiters.is_empty() => {
                    segments.push((segment_start, cursor));
                    semicolon = Some(cursor);
                    break;
                }
                _ => {}
            }
            cursor += 1;
        }
        let Some(semicolon) = semicolon else {
            var_index += 1;
            continue;
        };
        let assign_at = semicolon + 1;
        if tokens
            .get(assign_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(assign_at + 1).map(|token| token.text) != Some("=")
            || tokens.get(assign_at + 2).map(|token| token.text) == Some("=")
        {
            var_index = semicolon + 1;
            continue;
        }
        let name = tokens[assign_at].text;
        if !matches!(tokens[assign_at + 2].text, "{" | "[" | "function" | "(") {
            var_index = semicolon + 1;
            continue;
        }
        let Some(segment_index) = segments
            .iter()
            .enumerate()
            .find_map(|(index, (start, end))| {
                (tokens[*start].kind == TokenKind::Identifier
                    && tokens[*start].text == name
                    && *end == start + 1)
                    .then_some(index)
            })
        else {
            var_index = semicolon + 1;
            continue;
        };
        if segments.iter().enumerate().any(|(index, (start, end))| {
            index != segment_index && identifier_occurs(&tokens, *start, *end, name)
        }) {
            var_index = semicolon + 1;
            continue;
        }
        let Some(stop) = top_level_stop(&tokens, assign_at + 2, &[";", ",", "}"]) else {
            var_index = semicolon + 1;
            continue;
        };
        let rhs = &source[tokens[assign_at + 2].start..tokens[stop].start];
        if rhs.is_empty() {
            var_index = semicolon + 1;
            continue;
        }
        let mut parts = Vec::new();
        for (index, (start, end)) in segments.iter().enumerate() {
            if index == segment_index {
                continue;
            }
            parts.push(source[tokens[*start].start..tokens[*end].start].to_string());
        }
        parts.push(format!("{name}={rhs}"));
        let replace_end = match tokens[stop].text {
            ";" | "," => tokens[stop].end,
            _ => tokens[stop].start,
        };
        replacements.push((
            tokens[var_index].start,
            replace_end,
            format!("var {};", parts.join(",")),
        ));
        var_index = stop + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_void_then_reassign(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        if !(is_statement_boundary(&tokens, cursor)
            || tokens
                .get(cursor.wrapping_sub(1))
                .is_some_and(|token| token.text == ","))
            || tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens.get(cursor + 2).map(|token| token.text) != Some("void")
            || tokens.get(cursor + 3).map(|token| token.text) != Some("0")
            || tokens.get(cursor + 4).map(|token| token.text) != Some(";")
            || tokens.get(cursor + 5).map(|token| token.text) != Some(tokens[cursor].text)
            || tokens.get(cursor + 6).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(rhs_end) = top_level_stop(&tokens, cursor + 7, &[",", ";", "}"]) else {
            cursor += 1;
            continue;
        };
        let name = tokens[cursor].text;
        // Keep the undefined reset when the replacement assignment can read
        // the binding before producing its new value. The jQuery event walk
        // has exactly this shape: `handle=void 0;handle=events?read():handle`.
        if identifier_occurs(&tokens, cursor + 7, rhs_end, name) {
            cursor = rhs_end + 1;
            continue;
        }
        replacements.push((
            tokens[cursor + 1].start,
            tokens[cursor + 6].start,
            String::new(),
        ));
        cursor += 7;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_guarded_uninitialized_assign(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut var_index = 0usize;
    while var_index < tokens.len() {
        if tokens[var_index].text != "var" {
            var_index += 1;
            continue;
        }
        let Some(semi) = top_level_stop(&tokens, var_index + 1, &[";"]) else {
            var_index += 1;
            continue;
        };
        if tokens
            .get(semi + 1)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(semi + 2).map(|token| token.text) != Some("&&")
            || tokens.get(semi + 3).map(|token| token.text) != Some("(")
        {
            var_index = semi + 1;
            continue;
        }
        let cond = tokens[semi + 1].text;
        let Some(paren_close) = matching_close.get(semi + 3).copied().flatten() else {
            var_index = semi + 1;
            continue;
        };
        if tokens.get(semi + 4).map(|token| token.kind) != Some(TokenKind::Identifier)
            || tokens.get(semi + 5).map(|token| token.text) != Some("=")
            || tokens.get(semi + 6).map(|token| token.text) != Some(cond)
            || tokens.get(semi + 7).map(|token| token.text) != Some(".")
        {
            var_index = semi + 1;
            continue;
        }
        let name = tokens[semi + 4].text;
        if !uninitialized_var_segment(&tokens, var_index + 1, semi, name)
            || identifier_is_read(&tokens, var_index + 1, semi, name)
        {
            var_index = semi + 1;
            continue;
        }
        let rhs = &source[tokens[semi + 6].start..tokens[paren_close].start];
        let mut parts = Vec::new();
        let mut cursor = var_index + 1;
        while cursor < semi {
            let Some(stop) = top_level_stop(&tokens, cursor, &[",", ";"]) else {
                break;
            };
            if tokens[cursor].kind == TokenKind::Identifier
                && tokens[cursor].text == name
                && stop == cursor + 1
            {
                cursor = stop + 1;
                continue;
            }
            parts.push(source[tokens[cursor].start..tokens[stop].start].to_string());
            cursor = stop + 1;
        }
        parts.push(format!("{name}={cond}&&{rhs}"));
        let mut end = tokens[paren_close].end;
        let mut rendered = format!("var {}", parts.join(","));
        if tokens.get(paren_close + 1).map(|token| token.text) == Some(",") {
            end = tokens[paren_close + 1].end;
            rendered.push(';');
        }
        replacements.push((tokens[var_index].start, end, rendered));
        var_index = paren_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn uninitialized_var_segment(tokens: &[Token<'_>], start: usize, semi: usize, name: &str) -> bool {
    let mut cursor = start;
    while cursor < semi {
        let Some(stop) = top_level_stop(tokens, cursor, &[",", ";"]) else {
            return false;
        };
        if tokens[cursor].kind == TokenKind::Identifier
            && tokens[cursor].text == name
            && stop == cursor + 1
        {
            return true;
        }
        cursor = stop + 1;
    }
    false
}

pub(crate) fn merge_adjacent_declarations(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut merged = 0;
    loop {
        let tokens = lex(&output)?;
        let mut candidate = None;
        for declaration in 0..tokens.len() {
            let kind = tokens[declaration].text;
            if !matches!(kind, "let" | "var" | "const")
                || declaration
                    .checked_sub(1)
                    .is_some_and(|previous| !matches!(tokens[previous].text, ";" | "{" | "}"))
            {
                continue;
            }
            let mut delimiters = Vec::<&str>::new();
            for index in declaration + 1..tokens.len().saturating_sub(1) {
                match tokens[index].text {
                    "(" | "[" | "{" => delimiters.push(tokens[index].text),
                    ")" | "]" | "}" => {
                        delimiters.pop();
                    }
                    ";" if delimiters.is_empty() => {
                        if tokens[index + 1].text == kind
                            && tokens
                                .get(index + 2)
                                .is_some_and(|token| token.kind == TokenKind::Identifier)
                        {
                            candidate = Some((tokens[index].start, tokens[index + 2].start));
                        }
                        break;
                    }
                    _ => {}
                }
            }
            if candidate.is_some() {
                break;
            }
        }
        let Some((start, end)) = candidate else {
            break;
        };
        output.replace_range(start..end, ",");
        merged += 1;
    }
    Ok((output, merged))
}

pub(crate) fn remove_unused_standalone_vars(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut removed = 0;
    loop {
        let tokens = lex(&output)?;
        let mut matching_open = vec![None; tokens.len()];
        let mut matching_close = vec![None; tokens.len()];
        let mut stack = Vec::<usize>::new();
        for (index, token) in tokens.iter().enumerate() {
            match token.text {
                "(" | "[" | "{" => stack.push(index),
                ")" | "]" | "}" => {
                    let Some(open) = stack.pop() else {
                        continue;
                    };
                    matching_open[index] = Some(open);
                    matching_close[open] = Some(index);
                }
                _ => {}
            }
        }
        let function_bodies = (0..tokens.len())
            .filter(|index| tokens[*index].text == "{")
            .filter_map(|body| {
                let end = matching_close[body]?;
                let arrow = body
                    .checked_sub(1)
                    .is_some_and(|previous| tokens[previous].text == "=>");
                if arrow {
                    let before_arrow = body.checked_sub(2)?;
                    let parameter_start = if tokens[before_arrow].text == ")" {
                        matching_open[before_arrow]?
                    } else {
                        before_arrow
                    };
                    return Some((body, end, parameter_start));
                }
                let close_params = body.checked_sub(1)?;
                (tokens[close_params].text == ")").then_some(())?;
                let open_params = matching_open[close_params]?;
                let before = open_params.checked_sub(1)?;
                let is_function = tokens[before].text == "function"
                    || before
                        .checked_sub(1)
                        .is_some_and(|index| tokens[index].text == "function")
                    || tokens[before].kind == TokenKind::Identifier;
                is_function.then_some((body, end, open_params))
            })
            .collect::<Vec<_>>();

        let candidate = uninitialized_var_declarators(&tokens).into_iter().find_map(
            |(index, remove_start, remove_end)| {
                let (scope_start, scope_end) = function_bodies
                    .iter()
                    .copied()
                    .filter(|(start, end, _)| *start < index && index < *end)
                    .max_by_key(|(start, _, _)| *start)
                    .map(|(start, end, _)| (start, end))
                    .unwrap_or((usize::MAX, tokens.len()));
                let first = if scope_start == usize::MAX {
                    0
                } else {
                    scope_start + 1
                };
                let name = tokens[index].text;
                let shadowing_nested_scopes = function_bodies
                    .iter()
                    .copied()
                    .filter(|(start, end, _)| first <= *start && *end < scope_end)
                    .filter(|(start, end, _)| {
                        function_scope_declares(&tokens, &matching_open, *start, *end, name)
                    })
                    .collect::<Vec<_>>();
                let occurrences = tokens[first..scope_end]
                    .iter()
                    .enumerate()
                    .filter(|(offset, token)| {
                        let token_index = first + *offset;
                        token.kind == TokenKind::Identifier
                            && token.text == name
                            && !shadowing_nested_scopes
                                .iter()
                                .any(|(_, end, parameter_start)| {
                                    *parameter_start <= token_index && token_index < *end
                                })
                    })
                    .count();
                (occurrences == 1).then_some((remove_start, remove_end))
            },
        );
        let Some((start, end)) = candidate else {
            break;
        };
        output.replace_range(start..end, "");
        removed += 1;
    }
    Ok((output, removed))
}

/// A declarator is removable when nothing observes dropping its initializer.
/// Uninitialized bindings qualify trivially; so does a binding initialized to a
/// single literal token, because evaluating a literal cannot be observed.
///
/// The string pool is the common producer of the second shape: it hoists a
/// literal into a binding, and a later decision to spell the uses differently
/// (`o["k"]` collapsing to `o.k`) can strand the binding with no readers.
fn declarator_initializer_is_inert(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    match end - start {
        1 => true,
        3 => {
            tokens[start + 1].text == "="
                && match tokens[start + 2].kind {
                    TokenKind::Number | TokenKind::String | TokenKind::Regex => true,
                    TokenKind::Keyword => {
                        matches!(tokens[start + 2].text, "true" | "false" | "null")
                    }
                    _ => false,
                }
        }
        4 => {
            tokens[start + 1].text == "="
                && tokens[start + 2].text == "void"
                && matches!(
                    tokens[start + 3].kind,
                    TokenKind::Number | TokenKind::String
                )
        }
        _ => false,
    }
}

/// Returns removable `var`/`let`/`const` declarators with the byte range that
/// removes just that declarator (or the complete declaration when it is the
/// only one).
fn uninitialized_var_declarators(tokens: &[Token<'_>]) -> Vec<(usize, usize, usize)> {
    let mut candidates = Vec::new();
    for var_index in 0..tokens.len() {
        if !matches!(tokens[var_index].text, "var" | "let" | "const") {
            continue;
        }
        // `for ( let i = 0 ; ... )` ends its declaration on the header's own
        // semicolon, so removing the declaration would eat a required `;`.
        if var_index
            .checked_sub(2)
            .is_some_and(|index| tokens[index].text == "for" && tokens[index + 1].text == "(")
        {
            continue;
        }
        let mut delimiters = Vec::<&str>::new();
        let mut segment_start = var_index + 1;
        let mut segments = Vec::<(usize, usize, Option<usize>)>::new();
        let mut cursor = segment_start;
        let mut semicolon = None;
        while cursor < tokens.len() {
            match tokens[cursor].text {
                "(" | "[" | "{" => delimiters.push(tokens[cursor].text),
                ")" | "]" | "}" => {
                    if delimiters.pop().is_none() {
                        break;
                    }
                }
                "," if delimiters.is_empty() => {
                    segments.push((segment_start, cursor, Some(cursor)));
                    segment_start = cursor + 1;
                }
                ";" if delimiters.is_empty() => {
                    segments.push((segment_start, cursor, None));
                    semicolon = Some(cursor);
                    break;
                }
                _ => {}
            }
            cursor += 1;
        }
        let Some(semicolon) = semicolon else {
            continue;
        };
        for (segment_index, (start, end, following_comma)) in segments.iter().enumerate() {
            if tokens[*start].kind != TokenKind::Identifier
                || !declarator_initializer_is_inert(tokens, *start, *end)
            {
                continue;
            }
            let (remove_start, remove_end) = if segments.len() == 1 {
                (tokens[var_index].start, tokens[semicolon].end)
            } else if segment_index == 0 {
                (
                    tokens[*start].start,
                    tokens[following_comma.expect("a non-final segment has a comma")].end,
                )
            } else {
                (tokens[start - 1].start, tokens[end - 1].end)
            };
            candidates.push((*start, remove_start, remove_end));
        }
    }
    candidates
}

pub(crate) fn reuse_dead_var_binding(source: &str) -> Result<(String, bool), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut matching_open = vec![None; tokens.len()];
    let mut matching_close = vec![None; tokens.len()];
    let mut stack = Vec::<usize>::new();
    let mut brace_depth = vec![0usize; tokens.len()];
    let mut depth = 0usize;
    for (index, token) in tokens.iter().enumerate() {
        brace_depth[index] = depth;
        match token.text {
            "(" | "[" | "{" => {
                stack.push(index);
                if token.text == "{" {
                    depth += 1;
                }
            }
            ")" | "]" | "}" => {
                let Some(open) = stack.pop() else {
                    continue;
                };
                matching_open[index] = Some(open);
                matching_close[open] = Some(index);
                if token.text == "}" {
                    depth = depth.saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    let function_bodies = (0..tokens.len())
        .filter(|index| tokens[*index].text == "{")
        .filter_map(|body| {
            let end = matching_close[body]?;
            if body
                .checked_sub(1)
                .is_some_and(|previous| tokens[previous].text == "=>")
            {
                let before_arrow = body.checked_sub(2)?;
                let (parameter_start, parameter_end) = if tokens[before_arrow].text == ")" {
                    let open = matching_open[before_arrow]?;
                    (open + 1, before_arrow)
                } else {
                    (before_arrow, before_arrow + 1)
                };
                return Some((body, end, parameter_start, parameter_end));
            }
            let close_params = body.checked_sub(1)?;
            (tokens[close_params].text == ")").then_some(())?;
            let open_params = matching_open[close_params]?;
            let before = open_params.checked_sub(1)?;
            let is_function = tokens[before].text == "function"
                || before
                    .checked_sub(1)
                    .is_some_and(|index| tokens[index].text == "function")
                || tokens[before].kind == TokenKind::Identifier;
            is_function.then_some((body, end, open_params + 1, close_params))
        })
        .collect::<Vec<_>>();
    let arrow_parameter_ranges = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| token.text == "=>")
        .filter_map(|(arrow, _)| {
            let before_arrow = arrow.checked_sub(1)?;
            if tokens[before_arrow].text == ")" {
                let open = matching_open[before_arrow]?;
                Some((open + 1, before_arrow))
            } else {
                Some((before_arrow, before_arrow + 1))
            }
        })
        .collect::<Vec<_>>();

    for second_var in 0..tokens.len().saturating_sub(2) {
        if tokens[second_var].text != "var"
            || tokens[second_var + 1].kind != TokenKind::Identifier
            || tokens[second_var + 2].text != "="
            || second_var
                .checked_sub(1)
                .is_some_and(|previous| !matches!(tokens[previous].text, ";" | "{" | "}"))
        {
            continue;
        }
        let Some((scope_start, scope_end, _, _)) = function_bodies
            .iter()
            .copied()
            .filter(|(start, end, _, _)| *start < second_var && second_var < *end)
            .max_by_key(|(start, _, _, _)| *start)
        else {
            continue;
        };
        // Removing the `var` keyword is valid only for a single-declarator
        // statement.  In `var next=value,i=0,tmp`, doing so would turn `i`
        // and `tmp` into undeclared assignments (and can overwrite a mangled
        // top-level function in modules).
        if var_declaration_has_multiple_declarators(&tokens, second_var, scope_end) {
            continue;
        }
        let declaration_depth = brace_depth[second_var];
        let second_name = tokens[second_var + 1].text;
        let Some(first_var) = (scope_start + 1..second_var).rev().find(|index| {
            tokens[*index].text == "var"
                && tokens.get(*index + 1).is_some_and(|token| {
                    token.kind == TokenKind::Identifier && token.text != second_name
                })
                && tokens
                    .get(*index + 2)
                    .is_some_and(|token| token.text == "=")
                && brace_depth[*index] == declaration_depth
        }) else {
            continue;
        };
        let first_name = tokens[first_var + 1].text;
        // A use inside an already-created nested function remains live for as
        // long as that closure can escape. Textual last-use alone is therefore
        // insufficient: reusing the binding would mutate the closure's capture.
        let captured_before_second = function_bodies.iter().any(|(body, end, _, _)| {
            *body > first_var
                && *body < second_var
                && *end <= scope_end
                && tokens[*body..=*end]
                    .iter()
                    .any(|token| token.kind == TokenKind::Identifier && token.text == first_name)
        });
        if captured_before_second {
            continue;
        }
        if tokens[second_var + 2..scope_end]
            .iter()
            .any(|token| token.kind == TokenKind::Identifier && token.text == first_name)
        {
            continue;
        }
        if tokens[scope_start + 1..second_var]
            .iter()
            .any(|token| token.kind == TokenKind::Identifier && token.text == second_name)
        {
            continue;
        }

        let mut replacements = vec![(
            tokens[second_var].start,
            tokens[second_var + 1].end,
            first_name,
        )];
        let mut safe = true;
        for index in second_var + 2..scope_end {
            let token = tokens[index];
            if token.kind != TokenKind::Identifier || token.text != second_name {
                continue;
            }
            let nested_parameter =
                arrow_parameter_ranges
                    .iter()
                    .any(|(parameter_start, parameter_end)| {
                        *parameter_start <= index && index < *parameter_end
                    })
                    || function_bodies
                        .iter()
                        .any(|(_, _, parameter_start, parameter_end)| {
                            *parameter_start <= index && index < *parameter_end
                        });
            let declaration = index.checked_sub(1).is_some_and(|previous| {
                matches!(
                    tokens[previous].text,
                    "var" | "let" | "const" | "function" | "class" | "catch"
                )
            });
            let property = index
                .checked_sub(1)
                .is_some_and(|previous| matches!(tokens[previous].text, "." | "?."))
                || tokens.get(index + 1).is_some_and(|next| next.text == ":");
            if nested_parameter || declaration || property {
                safe = false;
                break;
            }
            replacements.push((token.start, token.end, first_name));
        }
        if !safe || replacements.len() == 1 {
            continue;
        }
        let mut output = String::with_capacity(source.len().saturating_sub(4));
        let mut cursor = 0;
        for (start, end, replacement) in replacements {
            output.push_str(&source[cursor..start]);
            output.push_str(replacement);
            cursor = end;
        }
        output.push_str(&source[cursor..]);
        return Ok((output, true));
    }
    Ok((source.to_string(), false))
}

fn var_declaration_has_multiple_declarators(
    tokens: &[Token<'_>],
    var_index: usize,
    scope_end: usize,
) -> bool {
    let mut delimiter_depth = 0usize;
    for token in tokens
        .iter()
        .take(scope_end)
        .skip(var_index.saturating_add(1))
    {
        match token.text {
            "(" | "[" | "{" => delimiter_depth += 1,
            ")" | "]" | "}" => delimiter_depth = delimiter_depth.saturating_sub(1),
            "," if delimiter_depth == 0 => return true,
            ";" if delimiter_depth == 0 => return false,
            _ => {}
        }
    }
    true
}

pub(crate) fn declare_implicit_assignment_bindings(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut declared = Vec::<(usize, &str)>::new();
    for at in 0..tokens.len() {
        if tokens[at].kind != TokenKind::Identifier
            || tokens.get(at + 1).map(|token| token.text) != Some("=")
            || tokens.get(at + 2).map(|token| token.text) == Some("=")
        {
            continue;
        }
        if at.checked_sub(1).is_some_and(|previous| {
            matches!(tokens[previous].text, "var" | "let" | "const" | "." | "?.")
        }) {
            continue;
        }
        let comma_sequence_assignment = is_comma_sequence_assignment_target(&tokens, at);
        let statement_like = is_statement_boundary(&tokens, at)
            || is_bare_for_init_assignment(&tokens, at)
            || is_chained_assignment_target(&tokens, at)
            || comma_sequence_assignment;
        let expression_embedded = matches!(
            tokens.get(at.saturating_sub(1)).map(|token| token.text),
            Some("(" | "&&" | "||" | "?" | ":")
        );
        if !statement_like && !expression_embedded {
            continue;
        }
        let name = tokens[at].text;
        let Some((function_body, _)) = enclosing_function_span(&tokens, &matching_close, at) else {
            if !statement_like
                || declared
                    .iter()
                    .any(|(body, declared_name)| *body == usize::MAX && *declared_name == name)
                || name_is_declared_in_any_enclosing_scope(&tokens, &matching_close, at, name)
            {
                continue;
            }
            replacements.push((0, 0, format!("var {name};")));
            declared.push((usize::MAX, name));
            continue;
        };
        if declared
            .iter()
            .any(|(body, declared_name)| *body == function_body && *declared_name == name)
        {
            continue;
        }
        if name_is_declared_in_any_enclosing_function_scope(&tokens, &matching_close, at, name) {
            continue;
        }
        if name_is_module_var_binding(&tokens, &matching_close, name)
            && !is_chained_assignment_target(&tokens, at)
        {
            continue;
        }
        let insert_at = if is_bare_for_init_assignment(&tokens, at) {
            tokens[at - 2].start
        } else if is_chained_assignment_target(&tokens, at) || comma_sequence_assignment {
            statement_start(&tokens, at)
        } else if statement_like {
            tokens[at].start
        } else if tokens[function_body].text == "{" {
            tokens[function_body].end
        } else {
            continue;
        };
        replacements.push((insert_at, insert_at, format!("var {name};")));
        declared.push((function_body, name));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn is_bare_for_init_assignment(tokens: &[Token<'_>], at: usize) -> bool {
    at >= 2 && tokens[at - 2].text == "for" && tokens[at - 1].text == "("
}

fn is_chained_assignment_target(tokens: &[Token<'_>], at: usize) -> bool {
    at >= 1
        && tokens[at].kind == TokenKind::Identifier
        && tokens[at - 1].text == "="
        && tokens.get(at + 1).map(|token| token.text) == Some("=")
        && tokens.get(at + 2).map(|token| token.text) != Some("=")
}

fn is_comma_sequence_assignment_target(tokens: &[Token<'_>], at: usize) -> bool {
    at >= 1
        && tokens[at].kind == TokenKind::Identifier
        && tokens[at - 1].text == ","
        && tokens.get(at + 1).map(|token| token.text) == Some("=")
        && tokens.get(at + 2).map(|token| token.text) != Some("=")
}

fn statement_start(tokens: &[Token<'_>], at: usize) -> usize {
    let mut index = at;
    while index > 0 && !is_statement_boundary(tokens, index) {
        index -= 1;
    }
    tokens[index].start
}

fn simple_pure_rhs_end(tokens: &[Token<'_>], at: usize) -> Option<usize> {
    if matches!(
        tokens.get(at).map(|token| token.kind),
        Some(TokenKind::Number | TokenKind::String)
    ) || matches!(
        tokens.get(at).map(|token| token.text),
        Some("true" | "false" | "null" | "undefined")
    ) {
        return Some(at);
    }
    if !tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier || token.text == "this")
    {
        return None;
    }
    let mut end = at;
    while tokens.get(end + 1).map(|token| token.text) == Some(".")
        && tokens
            .get(end + 2)
            .is_some_and(|token| matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword))
    {
        end += 2;
    }
    Some(end)
}

pub(crate) fn fold_dead_pure_identifier_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 2 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || is_property_identifier(&tokens, cursor)
            || assign_is_in_declaration(&tokens, cursor)
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(rhs_end) = simple_pure_rhs_end(&tokens, cursor + 2) else {
            cursor += 1;
            continue;
        };
        let name = tokens[cursor].text;
        let first_after = rhs_end + 1;
        let mut after = first_after;
        if !matches!(tokens.get(after).map(|token| token.text), Some("," | ";")) {
            cursor += 1;
            continue;
        }
        let mut survivor = None;
        let mut scan = after + 1;
        let mut depth = 0i32;
        let mut crossed_other = false;
        while scan < tokens.len() {
            match tokens[scan].text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => {
                    depth -= 1;
                    if depth < 0 {
                        break;
                    }
                }
                _ => {}
            }
            if depth != 0 {
                scan += 1;
                continue;
            }
            if tokens[scan].text == name
                && !is_property_identifier(&tokens, scan)
                && tokens.get(scan + 1).map(|token| token.text) == Some("=")
                && !assign_is_in_declaration(&tokens, scan)
            {
                if identifier_is_read(&tokens, after + 1, scan, name) {
                    break;
                }
                let Some(next_rhs) = simple_pure_rhs_end(&tokens, scan + 2) else {
                    break;
                };
                let next_after = next_rhs + 1;
                if !matches!(
                    tokens.get(next_after).map(|token| token.text),
                    Some("," | ";")
                ) {
                    break;
                }
                survivor = Some(scan);
                after = next_after;
                scan = next_after + 1;
                continue;
            }
            if matches!(
                tokens[scan].text,
                "if" | "else"
                    | "for"
                    | "while"
                    | "do"
                    | "switch"
                    | "try"
                    | "catch"
                    | "finally"
                    | "return"
                    | "throw"
                    | "break"
                    | "continue"
                    | "?"
            ) {
                break;
            }
            if !matches!(tokens[scan].text, "," | ";") {
                if survivor.is_some() {
                    break;
                }
                crossed_other = true;
            }
            scan += 1;
        }
        let Some(write_at) = survivor else {
            cursor += 1;
            continue;
        };
        if !crossed_other {
            replacements.push((tokens[cursor].start, tokens[write_at].start, String::new()));
        } else if cursor > 0 && tokens[cursor - 1].text == "," {
            if tokens[first_after].text == ";" {
                replacements.push((
                    tokens[cursor - 1].start,
                    tokens[first_after].end,
                    ";".to_string(),
                ));
            } else {
                replacements.push((
                    tokens[cursor - 1].start,
                    tokens[first_after].start,
                    String::new(),
                ));
            }
        } else if tokens[first_after].text == "," {
            replacements.push((tokens[cursor].start, tokens[first_after].end, String::new()));
        } else if tokens.get(first_after + 1).map(|token| token.text) == Some(",") {
            replacements.push((
                tokens[cursor].start,
                tokens[first_after + 1].end,
                String::new(),
            ));
        } else {
            replacements.push((tokens[cursor].start, tokens[first_after].end, String::new()));
        }
        cursor = write_at;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_unread_prototype_aliases(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        let Some(alias) = prototype_alias_at(&tokens, cursor) else {
            cursor += 1;
            continue;
        };
        if prototype_alias_is_live_before_rewrite(&tokens, alias.after + 1, alias.name) {
            cursor += 1;
            continue;
        }
        let preceded_by_comma = cursor > 0 && tokens[cursor - 1].text == ",";
        let preceded_by_decl =
            cursor > 0 && matches!(tokens[cursor - 1].text, "var" | "let" | "const");
        let start = if preceded_by_comma || (preceded_by_decl && tokens[alias.after].text == ";") {
            tokens[cursor - 1].start
        } else {
            tokens[cursor].start
        };
        let mut end_token = alias.after;
        let mut scan = alias.after + 1;
        while let Some(next) = prototype_alias_at(&tokens, scan) {
            if prototype_alias_is_live_before_rewrite(&tokens, next.after + 1, next.name) {
                break;
            }
            end_token = next.after;
            scan = next.after + 1;
        }
        let replacement = if preceded_by_comma {
            tokens[end_token].text.to_string()
        } else {
            String::new()
        };
        if !replacement_overlaps(&replacements, start, tokens[end_token].end) {
            replacements.push((start, tokens[end_token].end, replacement));
        }
        cursor = end_token + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

struct PrototypeAlias<'a> {
    name: &'a str,
    after: usize,
}

fn prototype_alias_at<'a>(tokens: &'a [Token<'a>], cursor: usize) -> Option<PrototypeAlias<'a>> {
    if tokens.get(cursor)?.kind != TokenKind::Identifier
        || is_property_identifier(tokens, cursor)
        || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
        || tokens.get(cursor + 2).map(|token| token.kind) != Some(TokenKind::Identifier)
        || tokens.get(cursor + 3).map(|token| token.text) != Some(".")
        || tokens.get(cursor + 4).map(|token| token.text) != Some("prototype")
    {
        return None;
    }
    let after = cursor + 5;
    if !matches!(tokens.get(after).map(|token| token.text), Some("," | ";")) {
        return None;
    }
    Some(PrototypeAlias {
        name: tokens[cursor].text,
        after,
    })
}

fn prototype_alias_is_live_before_rewrite(tokens: &[Token<'_>], start: usize, name: &str) -> bool {
    let matching_close = matching_closers(tokens);
    let mut index = start;
    while index < tokens.len() {
        if matches!(
            tokens[index].text,
            "if" | "else"
                | "for"
                | "while"
                | "do"
                | "switch"
                | "try"
                | "catch"
                | "finally"
                | "return"
                | "throw"
        ) {
            let end = next_statement_end(tokens, index + 1);
            if identifier_is_read(tokens, index + 1, end, name) {
                return true;
            }
            index = end + 1;
            continue;
        }
        if tokens[index].text == "function" || tokens[index].text == "class" {
            if let Some(next) = skip_function_or_class(tokens, &matching_close, index) {
                index = next;
                continue;
            }
            return true;
        }
        match tokens[index].text {
            "(" | "[" | "{" => {
                let Some(close) = matching_close.get(index).copied().flatten() else {
                    return true;
                };
                if identifier_is_read(tokens, index + 1, close, name) {
                    return true;
                }
                index = close + 1;
                continue;
            }
            ")" | "]" | "}" => return true,
            _ => {}
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && !is_property_identifier(tokens, index)
        {
            return tokens.get(index + 1).map(|token| token.text) != Some("=");
        }
        index += 1;
    }
    false
}

fn skip_function_or_class(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
) -> Option<usize> {
    let mut index = start + 1;
    if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Identifier) {
        index += 1;
    }
    if tokens.get(index).map(|token| token.text) == Some("extends") {
        index += 1;
        if tokens.get(index).map(|token| token.kind) == Some(TokenKind::Identifier) {
            index += 1;
        }
    }
    if tokens.get(index).map(|token| token.text) == Some("(") {
        index = matching_close.get(index).copied().flatten()? + 1;
    }
    if tokens.get(index).map(|token| token.text) == Some("{") {
        return Some(matching_close.get(index).copied().flatten()? + 1);
    }
    None
}
