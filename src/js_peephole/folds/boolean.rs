use crate::js_peephole::parse::{infix_precedence, Expression, ExpressionParser};
use crate::js_peephole::rewrite::{
    apply_token_rewrites, assign_is_in_declaration, conditional_test_needs_grouping,
    identifier_occurs, is_statement_boundary, next_statement_end,
    parenthesized_expression_has_postfix_continuation, single_console_log_argument, top_level_stop,
};
use crate::js_peephole::scope::{
    enclosing_block_end, enclosing_block_start, enclosing_function_range,
    name_is_arguments_length_copy, name_is_nonnegative_length_copy, nested_function_end,
    outermost_function_body_start, parse_function_expression,
};
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

/// Fold a braced `if`/`else` whose arms contain only expression statements
/// into a conditional expression. Comma grouping preserves the sequencing in
/// multi-statement arms. The untouched artifact remains a final codec-scored
/// candidate, so this raw-byte rewrite is never forced when gzip or Brotli
/// prefers the structured spelling.
pub(crate) fn fold_expression_branches(
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
    for if_index in 0..tokens.len() {
        if tokens[if_index].text != "if"
            || tokens.get(if_index + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let condition_open = if_index + 1;
        let Some(condition_close) = matching_close[condition_open] else {
            continue;
        };
        let then_open = condition_close + 1;
        if tokens.get(then_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(then_close) = matching_close[then_open] else {
            continue;
        };
        if tokens.get(then_close + 1).map(|token| token.text) != Some("else") {
            continue;
        }
        let else_open = then_close + 2;
        if tokens.get(else_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(else_close) = matching_close[else_open] else {
            continue;
        };
        let Some(then_value) =
            expression_statement_sequence(source, &tokens, then_open + 1, then_close)
        else {
            continue;
        };
        let Some(else_value) =
            expression_statement_sequence(source, &tokens, else_open + 1, else_close)
        else {
            continue;
        };
        let condition = &source[tokens[condition_open].end..tokens[condition_close].start];
        let condition =
            if conditional_test_needs_grouping(&tokens[condition_open + 1..condition_close]) {
                format!("({condition})")
            } else {
                condition.to_string()
            };
        let mut end = tokens[else_close].end;
        if tokens.get(else_close + 1).map(|token| token.text) == Some(";") {
            end = tokens[else_close + 1].end;
        }
        let replacement = if let (Some(then_arg), Some(else_arg)) = (
            single_console_log_argument(&then_value),
            single_console_log_argument(&else_value),
        ) {
            format!("console.log({condition}?{then_arg}:{else_arg});")
        } else {
            format!("{condition}?{then_value}:{else_value};")
        };
        if replacement.len() < end - tokens[if_index].start {
            replacements.push((tokens[if_index].start, end, replacement));
        }
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

fn expression_statement_sequence(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
) -> Option<String> {
    let mut cursor = start;
    let mut expressions = Vec::<String>::new();
    while cursor < end {
        while cursor < end && tokens[cursor].text == ";" {
            cursor += 1;
        }
        if cursor == end {
            break;
        }
        let mut depth = 0usize;
        let mut statement_end = end;
        for (index, token) in tokens.iter().enumerate().take(end).skip(cursor) {
            match token.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth = depth.saturating_sub(1),
                ";" if depth == 0 => {
                    statement_end = index;
                    break;
                }
                _ => {}
            }
        }
        if statement_end == cursor {
            cursor += 1;
            continue;
        }
        let mut parser = ExpressionParser::new(&tokens[cursor..statement_end]);
        let expression = parser.parse_complete()?;
        let rendered = source[tokens[cursor].start..tokens[statement_end - 1].end].to_string();
        expressions.push(if matches!(expression, Expression::Sequence(_)) {
            format!("({rendered})")
        } else {
            rendered
        });
        cursor = statement_end + usize::from(statement_end < end);
    }
    match expressions.len() {
        0 => None,
        1 => expressions.pop(),
        _ => Some(format!("({})", expressions.join(","))),
    }
}

/// Replace `!(left == right)` with `left != right` (and the strict/inverse
/// forms) without changing operand evaluation. JavaScript defines both
/// inequality operators as the logical inverse of their matching equality
/// operation, so coercion, side effects, exceptions, and left-to-right order
/// are identical. The surrounding candidate without this spelling remains in
/// the compiler's exact-codec frontier.
pub(crate) fn fold_negated_equalities(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut index = 0usize;

    while index + 3 < tokens.len() {
        if tokens[index].text != "!" || tokens[index + 1].text != "(" {
            index += 1;
            continue;
        }
        let open = index + 1;
        let Some(close) = matching_close[open] else {
            index += 1;
            continue;
        };
        if close <= open + 2 {
            index += 1;
            continue;
        }
        if parenthesized_expression_has_postfix_continuation(&tokens, close)
            || tokens
                .get(index.wrapping_sub(1))
                .is_some_and(|token| token.text == "new")
            || matches!(tokens[open + 1].text, "{" | "function" | "class" | "async")
        {
            index += 1;
            continue;
        }
        // The compact lexer deliberately does not distinguish division from
        // regular-expression literals. A regex body may itself contain an
        // equality token, so refuse the whole local rewrite rather than risk
        // selecting an operator from inside `/.../`.
        if tokens[open + 1..close]
            .iter()
            .any(|token| matches!(token.text, "/" | "/="))
        {
            index += 1;
            continue;
        }
        let Some((operator_index, inverse)) = root_equality_inverse(&tokens[open + 1..close])
        else {
            index += 1;
            continue;
        };
        let operator_index = open + 1 + operator_index;
        let inner_start = tokens[open + 1].start;
        let inner_end = tokens[close - 1].end;
        let mut inverse_expression = String::with_capacity(inner_end - inner_start);
        inverse_expression.push_str(&source[inner_start..tokens[operator_index].start]);
        inverse_expression.push_str(inverse);
        inverse_expression.push_str(&source[tokens[operator_index].end..inner_end]);
        if equality_replacement_requires_grouping(&tokens, index, close) {
            inverse_expression = format!("({inverse_expression})");
        }
        replacements.push((tokens[index].start, tokens[close].end, inverse_expression));
        index = close + 1;
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

/// Find the root equality operator of a parenthesized expression. Operators
/// with lower precedence prove that equality is only a child, while equal
/// precedence is left-associative and therefore makes the last equality token
/// the root.
fn root_equality_inverse(tokens: &[Token<'_>]) -> Option<(usize, &'static str)> {
    let mut depth = 0usize;
    let mut root = None;
    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "(" | "[" | "{" => {
                depth += 1;
                continue;
            }
            ")" | "]" | "}" => {
                depth = depth.checked_sub(1)?;
                continue;
            }
            _ if depth != 0 => continue,
            _ => {}
        }

        let inverse = match token.text {
            "==" => Some("!="),
            "!=" => Some("=="),
            "===" => Some("!=="),
            "!==" => Some("==="),
            _ => None,
        };
        if let Some(inverse) = inverse {
            root = Some((index, inverse));
            continue;
        }
        if matches!(token.text, "?" | ":" | "=>" | "yield")
            || infix_precedence(token.text).is_some_and(|(precedence, _, _)| precedence < 10)
        {
            return None;
        }
    }
    (depth == 0).then_some(root).flatten()
}

fn equality_replacement_requires_grouping(tokens: &[Token<'_>], bang: usize, close: usize) -> bool {
    let tighter_or_equal = |token: &Token<'_>| {
        infix_precedence(token.text).is_some_and(|(precedence, _, _)| precedence >= 10)
    };
    let prefix_parent = tokens.get(bang.wrapping_sub(1)).is_some_and(|token| {
        matches!(
            token.text,
            "!" | "~" | "+" | "-" | "." | "typeof" | "void" | "delete" | "await" | "new"
        ) || tighter_or_equal(token)
    });
    let postfix_parent = tokens.get(close + 1).is_some_and(tighter_or_equal);
    prefix_parent || postfix_parent
}

/// Fold `value=expression;if(value){...}` into
/// `if(value=expression){...}`. The assignment result already undergoes the
/// same JavaScript truthiness conversion in both forms, and the binding keeps
/// the same value on both the taken and untaken paths.
pub(crate) fn fold_assignment_guards(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let is_statement_start =
        |index: usize| index == 0 || matches!(tokens[index - 1].text, "{" | "}" | ";");
    let mut start = 0usize;
    while start + 6 < tokens.len() {
        if tokens[start].kind != TokenKind::Identifier
            || tokens[start + 1].text != "="
            || !is_statement_start(start)
        {
            start += 1;
            continue;
        }
        let name = tokens[start].text;
        let mut depth = 0i32;
        let mut semicolon = None;
        for (index, token) in tokens.iter().enumerate().skip(start + 2) {
            match token.text {
                "(" | "[" | "{" => depth += 1,
                ")" | "]" | "}" => depth -= 1,
                ";" if depth == 0 => {
                    semicolon = Some(index);
                    break;
                }
                _ => {}
            }
            if depth < 0 {
                break;
            }
        }
        let Some(semi) = semicolon else {
            start += 1;
            continue;
        };
        if tokens.get(semi + 1).map(|token| token.text) != Some("if")
            || tokens.get(semi + 2).map(|token| token.text) != Some("(")
            || tokens.get(semi + 3).map(|token| (token.kind, token.text))
                != Some((TokenKind::Identifier, name))
            || tokens.get(semi + 4).map(|token| token.text) != Some(")")
        {
            start += 1;
            continue;
        }
        replacements.push((
            tokens[start].start,
            tokens[semi + 4].end,
            format!("if({})", &source[tokens[start].start..tokens[semi].start]),
        ));
        start = semi + 5;
    }
    if replacements.is_empty() {
        return Ok((source.to_string(), 0));
    }
    let count = replacements.len();
    let mut output = source.to_string();
    // Reverse order keeps all earlier source offsets stable.
    for (start, end, replacement) in replacements.into_iter().rev() {
        output.replace_range(start..end, &replacement);
    }
    Ok((output, count))
}

pub(crate) fn fold_guarded_assign_into_call_predicate(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut start = 0usize;
    while start + 10 < tokens.len() {
        if !is_generated_statement_start(&tokens, start) {
            start += 1;
            continue;
        }
        let Some((and_and, name_at, rhs_close, semi)) =
            guarded_assign_statement_tail(&tokens, &matching_close, start)
        else {
            start += 1;
            continue;
        };
        if tokens.get(semi + 1).map(|token| token.text) != Some("if")
            || tokens.get(semi + 2).map(|token| token.text) != Some("(")
            || tokens
                .get(semi + 3)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(semi + 4).map(|token| token.text) != Some("(")
            || tokens.get(semi + 5).map(|token| token.text) != Some(tokens[name_at].text)
            || tokens.get(semi + 6).map(|token| token.text) != Some(")")
            || tokens.get(semi + 7).map(|token| token.text) != Some(")")
        {
            start += 1;
            continue;
        }
        let name = tokens[name_at].text;
        if identifier_occurs(&tokens, start, and_and, name)
            || statement_has_top_level_disjunction(&tokens, start, and_and)
        {
            start += 1;
            continue;
        }
        let if_close = semi + 7;
        let then_end = if tokens.get(if_close + 1).map(|token| token.text) == Some("{") {
            let Some(close) = matching_close.get(if_close + 1).copied().flatten() else {
                start += 1;
                continue;
            };
            close
        } else {
            let Some(stop) = top_level_stop(&tokens, if_close + 1, &[";"]) else {
                start += 1;
                continue;
            };
            stop
        };
        let stmt_end = if tokens.get(then_end + 1).map(|token| token.text) == Some("else") {
            let else_at = then_end + 2;
            let else_end = if tokens.get(else_at).map(|token| token.text) == Some("{") {
                let Some(close) = matching_close.get(else_at).copied().flatten() else {
                    start += 1;
                    continue;
                };
                close
            } else if tokens.get(else_at).map(|token| token.text) == Some("if") {
                start += 1;
                continue;
            } else {
                let Some(stop) = top_level_stop(&tokens, else_at, &[";"]) else {
                    start += 1;
                    continue;
                };
                stop
            };
            if identifier_occurs(&tokens, then_end + 1, else_end + 1, name) {
                start += 1;
                continue;
            }
            else_end
        } else {
            then_end
        };
        let scope_end = enclosing_block_end(&matching_close, start).unwrap_or(tokens.len());
        if identifier_occurs(&tokens, stmt_end + 1, scope_end, name) {
            start += 1;
            continue;
        }
        let cond = &source[tokens[start].start..tokens[and_and].start];
        let rhs = &source[tokens[name_at + 2].start..tokens[rhs_close].start];
        let pred = tokens[semi + 3].text;
        replacements.push((
            tokens[start].start,
            tokens[if_close].end,
            format!("if({pred}({name}={cond}&&{rhs}))"),
        ));
        start = if_close + 1;
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

fn is_generated_statement_start(tokens: &[Token<'_>], index: usize) -> bool {
    index == 0 || matches!(tokens[index - 1].text, "{" | "}" | ";")
}

fn statement_has_top_level_disjunction(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for token in &tokens[start..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "||" | "??" | "?" | "," | "=" if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn guarded_assign_statement_tail(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
) -> Option<(usize, usize, usize, usize)> {
    let mut depth = 0i32;
    let mut last_and_and = None;
    for index in start..tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                depth -= 1;
                if depth < 0 {
                    return None;
                }
            }
            "&&" if depth == 0 => last_and_and = Some(index),
            ";" if depth == 0 => {
                let and_and = last_and_and?;
                if tokens.get(and_and + 1).map(|token| token.text) != Some("(") {
                    return None;
                }
                let name_at = and_and + 2;
                if tokens.get(name_at).map(|token| token.kind) != Some(TokenKind::Identifier)
                    || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
                {
                    return None;
                }
                let rhs_close = matching_close.get(and_and + 1).copied().flatten()?;
                if rhs_close + 1 != index {
                    return None;
                }
                return Some((and_and, name_at, rhs_close, index));
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn fold_guarded_and_addends(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 12 < tokens.len() {
        if tokens[cursor].text == "!"
            && tokens
                .get(cursor + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(cursor + 2).map(|token| token.text) == Some("||")
            && tokens.get(cursor + 3).map(|token| token.text) == Some("(")
            && tokens.get(cursor + 4).map(|token| token.text) == Some(tokens[cursor + 1].text)
            && tokens.get(cursor + 5).map(|token| token.text) == Some("=")
        {
            let name = tokens[cursor + 1].text;
            if let Some(assign_close) = top_level_stop(&tokens, cursor + 6, &[")"]) {
                if tokens.get(assign_close + 1).map(|token| token.text) == Some(";")
                    && tokens
                        .get(assign_close + 2)
                        .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && tokens.get(assign_close + 3).map(|token| token.text) == Some(".")
                    && tokens
                        .get(assign_close + 4)
                        .is_some_and(|token| token.kind == TokenKind::Identifier)
                    && tokens.get(assign_close + 5).map(|token| token.text) == Some("=")
                    && tokens.get(assign_close + 6).map(|token| token.text) == Some(name)
                {
                    let value = &source[tokens[cursor + 6].start..tokens[assign_close].start];
                    replacements.push((
                        tokens[cursor].start,
                        tokens[assign_close + 6].end,
                        format!(
                            "{}.{}={name}&&{value}",
                            tokens[assign_close + 2].text,
                            tokens[assign_close + 4].text
                        ),
                    ));
                    cursor = assign_close + 7;
                    continue;
                }
            }
        }
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
            || tokens.get(first_semi + 3).map(|token| token.text) != Some("||")
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
        if tokens.get(assign_close + 1).map(|token| token.text) != Some(";") {
            cursor += 1;
            continue;
        }
        let target_at = assign_close + 2;
        let Some(target) = tokens
            .get(target_at)
            .filter(|token| token.kind == TokenKind::Identifier)
            .map(|token| token.text)
        else {
            cursor += 1;
            continue;
        };
        if target == name {
            cursor += 1;
            continue;
        }
        let cond = &source[tokens[name_index + 2].start..tokens[first_semi].start];
        let value = &source[tokens[first_semi + 7].start..tokens[assign_close].start];
        if identifier_occurs(&tokens, name_index + 2, first_semi, name)
            || identifier_occurs(&tokens, first_semi + 7, assign_close, name)
        {
            cursor += 1;
            continue;
        }
        let comma_declarator = name_index
            .checked_sub(1)
            .is_some_and(|index| tokens[index].text == ",");
        let replace_from = if comma_declarator {
            tokens[name_index - 1].start
        } else {
            tokens[cursor].start
        };
        let prefix = if comma_declarator { ";" } else { "" };
        let replacement = if tokens.get(target_at + 1).map(|token| token.text) == Some("+=")
            && tokens.get(target_at + 2).map(|token| token.text) == Some(name)
        {
            Some((
                replace_from,
                tokens[target_at + 2].end,
                format!("{prefix}{target}+={cond}&&{value}"),
            ))
        } else if tokens.get(target_at + 1).map(|token| token.text) == Some(".")
            && tokens
                .get(target_at + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(target_at + 3).map(|token| token.text) == Some("=")
            && tokens.get(target_at + 4).map(|token| token.text) == Some(name)
        {
            Some((
                replace_from,
                tokens[target_at + 4].end,
                format!(
                    "{prefix}{target}.{}={cond}&&{value}",
                    tokens[target_at + 2].text
                ),
            ))
        } else if tokens.get(target_at + 1).map(|token| token.text) == Some("=")
            && tokens.get(target_at + 2).map(|token| token.text) == Some(target)
            && tokens.get(target_at + 3).map(|token| token.text) == Some("+")
            && tokens.get(target_at + 4).map(|token| token.text) == Some(name)
        {
            Some((
                replace_from,
                tokens[target_at + 4].end,
                format!("{prefix}{target}={target}+({cond}&&{value})"),
            ))
        } else {
            None
        };
        let Some(replacement) = replacement else {
            cursor += 1;
            continue;
        };
        replacements.push(replacement);
        cursor = target_at + 3;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

pub(crate) fn fold_assigned_truthy_ternaries(
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
        // `(name=EXPR)?name:FALLBACK` folds to `EXPR||FALLBACK` only when the
        // parenthesized assignment is the *whole* ternary condition. In
        // `"string"==typeof e&&""!=(e=e.trim())?e:null` the group is an operand
        // of `!=`, the condition's value is a boolean, and folding returns
        // `true` where the source returned the trimmed string.
        if !condition_starts_at(&tokens, open) {
            continue;
        }
        let Some(close) = matching_close.get(open).copied().flatten() else {
            continue;
        };
        let name = tokens[open + 1].text;
        if tokens.get(close + 1).map(|token| token.text) != Some("?")
            || tokens.get(close + 2).map(|token| token.text) != Some(name)
            || tokens.get(close + 3).map(|token| token.text) != Some(":")
        {
            continue;
        }
        let fallback_at = close + 4;
        let Some(fallback_end) = complete_primary_end(&tokens, fallback_at) else {
            continue;
        };
        if identifier_occurs(&tokens, fallback_at, fallback_end + 1, name)
            || name_is_read_after_statement(&tokens, fallback_end + 1, name)
        {
            continue;
        }
        let expr = &source[tokens[open + 3].start..tokens[close].start];
        let fallback = &source[tokens[fallback_at].start..tokens[fallback_end].end];
        replacements.push((
            tokens[open].start,
            tokens[fallback_end].end,
            format!("{expr}||{fallback}"),
        ));
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

/// True when a parenthesized group at `open` begins a fresh expression rather
/// than continuing one. Anything else -- an operator, `typeof`, `!` -- means
/// the group is an operand and its value is not the enclosing expression's.
fn condition_starts_at(tokens: &[Token<'_>], open: usize) -> bool {
    match open.checked_sub(1) {
        None => true,
        Some(previous) => matches!(
            tokens[previous].text,
            "return" | "=" | "(" | "[" | "," | ";" | "{" | "}" | ":" | "?" | "=>"
        ),
    }
}

fn complete_primary_end(tokens: &[Token<'_>], start: usize) -> Option<usize> {
    let token = tokens.get(start)?;
    if matches!(
        token.kind,
        TokenKind::String | TokenKind::Number | TokenKind::Keyword | TokenKind::Identifier
    ) {
        return Some(start);
    }
    if token.text == "(" || token.text == "[" {
        return matching_closers(tokens).get(start).copied().flatten();
    }
    None
}

fn name_is_read_after_statement(tokens: &[Token<'_>], start: usize, name: &str) -> bool {
    let mut index = start;
    while index < tokens.len()
        && matches!(
            tokens[index].text,
            ":" | ")" | "," | ";" | "?" | "&&" | "||"
        )
    {
        if tokens[index].text == ";" {
            index += 1;
            break;
        }
        index += 1;
    }
    let mut depth = 0i32;
    while index < tokens.len() {
        match tokens[index].text {
            "{" => depth += 1,
            "}" if depth == 0 => return false,
            "}" => depth -= 1,
            "function" if depth == 0 => return false,
            _ if tokens[index].kind == TokenKind::Identifier && tokens[index].text == name => {
                return true;
            }
            _ => {}
        }
        index += 1;
    }
    false
}

pub(crate) fn flip_false_equalities(source: &str) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        if tokens[cursor].text == "==="
            && tokens.get(cursor + 1).map(|token| token.text) == Some("!")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("1")
        {
            let Some(left_start) = equality_left_start(&tokens, &matching_close, cursor) else {
                cursor += 1;
                continue;
            };
            let left = &source[tokens[left_start].start..tokens[cursor].start];
            replacements.push((
                tokens[left_start].start,
                tokens[cursor + 2].end,
                format!("!1==={left}"),
            ));
            cursor += 3;
            continue;
        }
        cursor += 1;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn equality_left_start(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    eq_at: usize,
) -> Option<usize> {
    let prev = eq_at.checked_sub(1)?;
    let mut start = if tokens[prev].text == ")" {
        let open = matching_close
            .iter()
            .enumerate()
            .find_map(|(open, close)| (*close == Some(prev)).then_some(open))?;
        if open == 0 || tokens[open - 1].kind != TokenKind::Identifier {
            return None;
        }
        open - 1
    } else if tokens[prev].kind == TokenKind::Identifier {
        prev
    } else {
        return None;
    };
    loop {
        if start >= 2
            && tokens[start - 1].text == "."
            && tokens[start - 2].kind == TokenKind::Identifier
        {
            start -= 2;
            continue;
        }
        if start >= 2 && tokens[start - 1].text == "." && tokens[start - 2].text == "]" {
            let close = start - 2;
            let Some(open) = matching_close
                .iter()
                .enumerate()
                .find_map(|(open, end)| (*end == Some(close)).then_some(open))
            else {
                break;
            };
            if open == 0 || tokens[open - 1].kind != TokenKind::Identifier {
                break;
            }
            start = open - 1;
            continue;
        }
        break;
    }
    Some(start)
}

pub(crate) fn fold_arguments_length_eq_zero_to_not(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 2 < tokens.len() {
        let (start, name_at) = if tokens[cursor].text == "0"
            && tokens.get(cursor + 1).map(|token| token.text) == Some("==")
            && tokens
                .get(cursor + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            (cursor, cursor + 2)
        } else if tokens[cursor].kind == TokenKind::Identifier
            && tokens.get(cursor + 1).map(|token| token.text) == Some("==")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("0")
        {
            (cursor, cursor)
        } else {
            cursor += 1;
            continue;
        };
        let name = tokens[name_at].text;
        let prev = start
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";");
        if matches!(prev, "+" | "-" | "*" | "/" | "%" | "**" | "++" | "--")
            || !name_is_arguments_length_copy(&tokens, &matching_close, start, name)
        {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[start].start,
            tokens[start + 2].end,
            format!("!{name}"),
        ));
        cursor = start + 3;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_integer_neq_zero_in_boolean(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 2 < tokens.len() {
        let (start, name_at) = if tokens[cursor].text == "0"
            && tokens.get(cursor + 1).map(|token| token.text) == Some("!=")
            && tokens
                .get(cursor + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            (cursor, cursor + 2)
        } else if tokens[cursor].kind == TokenKind::Identifier
            && tokens.get(cursor + 1).map(|token| token.text) == Some("!=")
            && tokens.get(cursor + 2).map(|token| token.text) == Some("0")
        {
            (cursor, cursor)
        } else {
            cursor += 1;
            continue;
        };
        if !comparison_sits_in_boolean_context(&tokens, start) {
            cursor += 1;
            continue;
        }
        let name = tokens[name_at].text;
        let prev = start
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";");
        if matches!(prev, "+" | "-" | "*" | "/" | "%" | "**" | "++" | "--")
            || !name_is_zero_based_int(&tokens, &matching_close, start, name)
        {
            cursor += 1;
            continue;
        }
        replacements.push((tokens[start].start, tokens[start + 2].end, name.to_string()));
        cursor = start + 3;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn comparison_sits_in_boolean_context(tokens: &[Token<'_>], start: usize) -> bool {
    match tokens.get(start + 3).map(|token| token.text) {
        Some("?") | Some("&&") | Some("||") => true,
        Some(")") => {
            start >= 2
                && tokens[start - 1].text == "("
                && matches!(tokens[start - 2].text, "if" | "while")
        }
        _ => false,
    }
}

fn name_is_zero_based_int(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    before: usize,
    name: &str,
) -> bool {
    if name_is_arguments_length_copy(tokens, matching_close, before, name) {
        return true;
    }
    if name_is_local_zero_counter(tokens, matching_close, before, name) {
        return true;
    }
    param_is_zero_based_int(tokens, matching_close, before, name)
}

fn name_is_local_zero_counter(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    before: usize,
    name: &str,
) -> bool {
    let mut proven = false;
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
            && tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("=")
            && tokens.get(index + 2).map(|token| token.text) == Some("0")
        {
            proven = true;
            index += 3;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && tokens.get(index + 1).map(|token| token.text) == Some("++")
        {
            index += 2;
            continue;
        }
        if tokens[index].text == "++" && tokens.get(index + 1).map(|token| token.text) == Some(name)
        {
            index += 2;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && matches!(
                tokens.get(index + 1).map(|token| token.text),
                Some("=") | Some("--") | Some("+=") | Some("-=")
            )
        {
            proven = false;
        }
        index += 1;
    }
    proven
}

fn param_is_zero_based_int(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    before: usize,
    name: &str,
) -> bool {
    let Some(arrow) = (0..before).rev().find(|&index| {
        tokens[index].text == "=>"
            && enclosing_block_start(matching_close, before).is_some_and(|open| open > index)
    }) else {
        return false;
    };
    if tokens.get(arrow - 1).map(|token| token.text) != Some(")") {
        return false;
    }
    let Some(param_open) = (0..arrow - 1)
        .rev()
        .find(|&index| matching_close.get(index).copied().flatten() == Some(arrow - 1))
    else {
        return false;
    };
    let params = tokens[param_open + 1..arrow - 1]
        .iter()
        .filter(|token| token.kind == TokenKind::Identifier)
        .map(|token| token.text)
        .collect::<Vec<_>>();
    let Some(param_index) = params.iter().position(|param| *param == name) else {
        return false;
    };
    if tokens.get(param_open - 1).map(|token| token.text) != Some("=")
        || param_open < 2
        || tokens[param_open - 2].kind != TokenKind::Identifier
    {
        return false;
    }
    let callee = tokens[param_open - 2].text;
    let (scan_start, scan_end) = enclosing_function_range(tokens, matching_close, param_open - 2)
        .unwrap_or((0, tokens.len()));
    let mut saw_call = false;
    let mut index = scan_start;
    while index + 1 < scan_end {
        if tokens[index].text == callee
            && tokens.get(index + 1).map(|token| token.text) == Some("(")
        {
            let Some(close) = matching_close.get(index + 1).copied().flatten() else {
                return false;
            };
            let mut args = Vec::new();
            let mut arg_start = index + 2;
            let mut depth = 0i32;
            let mut cursor = index + 2;
            while cursor < close {
                match tokens[cursor].text {
                    "(" | "[" | "{" => depth += 1,
                    ")" | "]" | "}" => depth -= 1,
                    "," if depth == 0 => {
                        args.push((arg_start, cursor));
                        arg_start = cursor + 1;
                    }
                    _ => {}
                }
                cursor += 1;
            }
            args.push((arg_start, close));
            let Some((start, end)) = args.get(param_index).copied() else {
                return false;
            };
            if end != start + 1 {
                return false;
            }
            if tokens[start].text != "0"
                && (tokens[start].kind != TokenKind::Identifier
                    || !name_is_local_zero_counter(
                        tokens,
                        matching_close,
                        start,
                        tokens[start].text,
                    ))
            {
                return false;
            }
            saw_call = true;
            index = close + 1;
            continue;
        }
        index += 1;
    }
    saw_call
}

pub(crate) fn fold_predicate_reassign_same_expr(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 3).map(|token| token.text) != Some("(")
            || !matches!(
                cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";"),
                ";" | "{" | "}" | "var" | "let" | "const" | ","
            )
        {
            cursor += 1;
            continue;
        }
        let Some(close) = matching_close.get(cursor + 3).copied().flatten() else {
            cursor += 1;
            continue;
        };
        if !matches!(
            tokens.get(close + 1).map(|token| token.text),
            Some(";") | Some(",")
        ) {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        if tokens.get(close + 2).map(|token| token.text) != Some(name)
            || tokens.get(close + 3).map(|token| token.text) != Some("&&")
            || tokens.get(close + 4).map(|token| token.text) != Some("(")
            || tokens.get(close + 5).map(|token| token.text) != Some(name)
            || tokens.get(close + 6).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let Some(assign_close) = matching_close.get(close + 4).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let first = &source[tokens[cursor + 4].start..tokens[close].start];
        let second = &source[tokens[close + 7].start..tokens[assign_close].start];
        if first != second {
            cursor += 1;
            continue;
        }
        let mut end = tokens[assign_close].end;
        let mut folded = format!("={}({})&&{}", tokens[cursor + 2].text, first, first);
        if tokens.get(close + 1).map(|token| token.text) == Some(";")
            && tokens.get(assign_close + 1).map(|token| token.text) == Some(",")
            && assign_is_in_declaration(&tokens, cursor)
        {
            end = tokens[assign_close + 1].end;
            folded.push(';');
        }
        replacements.push((tokens[cursor + 1].start, end, folded));
        cursor = assign_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_conditional_assigned_false_phi(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    for colon in 0..tokens.len() {
        if tokens[colon].text != ":"
            || tokens
                .get(colon + 1)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(colon + 2).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let name = tokens[colon + 1].text;
        let false_end = if tokens.get(colon + 3).map(|token| token.text) == Some("!")
            && tokens.get(colon + 4).map(|token| token.text) == Some("1")
        {
            colon + 4
        } else if tokens.get(colon + 3).map(|token| token.text) == Some("false") {
            colon + 3
        } else {
            continue;
        };
        if tokens.get(false_end + 1).map(|token| token.text) != Some(",")
            || tokens.get(false_end + 2).map(|token| token.text) != Some(name)
            || tokens.get(false_end + 3).map(|token| token.text) != Some("&&")
        {
            continue;
        }
        let Some(qmark) = matching_question_mark(&tokens, colon) else {
            continue;
        };
        let mut then_start = qmark + 1;
        let mut then_end = colon;
        if tokens.get(then_start).map(|token| token.text) == Some("(")
            && matching_close.get(then_start).copied().flatten() == Some(colon - 1)
        {
            then_start += 1;
            then_end = colon - 1;
        }
        if tokens
            .get(then_start)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens[then_start].text != name
            || tokens.get(then_start + 1).map(|token| token.text) != Some("=")
            || then_start + 2 >= then_end
        {
            continue;
        }
        let rest_at = false_end + 4;
        let stmt_end = next_statement_end(&tokens, rest_at);
        if identifier_occurs(&tokens, rest_at, stmt_end, name)
            || name_is_read_after_statement(&tokens, stmt_end, name)
        {
            continue;
        }
        let expr = &source[tokens[then_start + 2].start..tokens[then_end - 1].end];
        if expression_has_top_level_and_break(expr) {
            continue;
        }
        replacements.push((
            tokens[qmark].start,
            tokens[false_end + 3].end,
            format!("&&{expr}&&"),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn matching_question_mark(tokens: &[Token<'_>], colon: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut nested = 0i32;
    let mut index = colon;
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
            ":" if depth == 0 => nested += 1,
            "?" if depth == 0 => {
                if nested == 0 {
                    return Some(index);
                }
                nested -= 1;
            }
            ";" if depth == 0 => return None,
            _ => {}
        }
    }
    None
}

fn expression_has_top_level_and_break(source: &str) -> bool {
    let Ok(tokens) = lex(source) else {
        return true;
    };
    let mut depth = 0i32;
    for token in &tokens {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "||" | "??" | "?" | "," | "=" if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

pub(crate) fn fold_boolean_context_double_not(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if !matches!(tokens[cursor].text, "&&" | "||")
            || tokens.get(cursor + 1).map(|token| token.text) != Some("!")
            || tokens.get(cursor + 2).map(|token| token.text) != Some("!")
        {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor + 1].start,
            tokens[cursor + 3].start,
            String::new(),
        ));
        cursor += 3;
    }
    let (output, count) = apply_token_rewrites(source, replacements);
    Ok((output, count))
}

fn and_chain_can_drop_parens(tokens: &[Token<'_>], from: usize, close: usize) -> bool {
    if from >= close {
        return false;
    }
    let mut depth = 0i32;
    let mut saw_and = false;
    for token in &tokens[from..close] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "&&" if depth == 0 => saw_and = true,
            "||" | "??" | "?" | "," | "=" | "=>" | "void" | "typeof" | "new" if depth == 0 => {
                return false;
            }
            _ => {}
        }
    }
    saw_and
}

fn postfix_needs_grouping(tokens: &[Token<'_>], close: usize) -> bool {
    matches!(
        tokens.get(close + 1).map(|token| token.text),
        Some(".") | Some("[") | Some("(") | Some("++") | Some("--") | Some("?.") | Some("`")
    )
}

pub(crate) fn fold_redundant_and_parens(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if tokens[cursor].text == "(" {
            let Some(close) = matching_close.get(cursor).copied().flatten() else {
                cursor += 1;
                continue;
            };
            if tokens.get(close + 1).map(|token| token.text) == Some("&&")
                && and_chain_can_drop_parens(&tokens, cursor + 1, close)
            {
                let prev = cursor
                    .checked_sub(1)
                    .map(|index| tokens[index].text)
                    .unwrap_or(";");
                if !matches!(
                    prev,
                    "!" | "typeof"
                        | "void"
                        | "delete"
                        | "await"
                        | "new"
                        | "return"
                        | "throw"
                        | "++"
                        | "--"
                        | "."
                        | "["
                ) {
                    replacements.push((tokens[cursor].start, tokens[cursor].end, String::new()));
                    replacements.push((tokens[close].start, tokens[close].end, String::new()));
                    cursor = close + 1;
                    continue;
                }
            }
        }
        if tokens[cursor].text == "&&"
            && tokens.get(cursor + 1).map(|token| token.text) == Some("(")
        {
            let Some(close) = matching_close.get(cursor + 1).copied().flatten() else {
                cursor += 1;
                continue;
            };
            if and_chain_can_drop_parens(&tokens, cursor + 2, close)
                && !postfix_needs_grouping(&tokens, close)
            {
                replacements.push((
                    tokens[cursor + 1].start,
                    tokens[cursor + 1].end,
                    String::new(),
                ));
                replacements.push((tokens[close].start, tokens[close].end, String::new()));
                cursor = close + 1;
                continue;
            }
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_statement_negated_ors(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if !is_statement_boundary(&tokens, cursor)
            || tokens[cursor].text != "!"
            || tokens
                .get(cursor + 1)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 2).map(|token| token.text) != Some("||")
            || tokens.get(cursor + 3).map(|token| token.text) != Some("(")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor + 1].text;
        let Some(close) = matching_close.get(cursor + 3).copied().flatten() else {
            cursor += 1;
            continue;
        };
        if !statement_follows_paren(&tokens, close) {
            cursor += 1;
            continue;
        }
        let rhs = &source[tokens[cursor + 4].start..tokens[close].start];
        let end_at = if tokens.get(close + 1).map(|token| token.text) == Some(";") {
            close + 1
        } else {
            close
        };
        replacements.push((
            tokens[cursor].start,
            tokens[end_at].end,
            format!("{name}&&({rhs});"),
        ));
        cursor = end_at + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_chained_comma_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        if !matches!(
            cursor
                .checked_sub(1)
                .map(|index| tokens[index].text)
                .unwrap_or(";"),
            "(" | ","
        ) || tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
            || assign_is_in_declaration(&tokens, cursor)
        {
            cursor += 1;
            continue;
        }
        let Some(comma) = top_level_stop(&tokens, cursor + 2, &[","]) else {
            cursor += 1;
            continue;
        };
        if tokens
            .get(comma + 1)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(comma + 2).map(|token| token.text) != Some("=")
            || tokens.get(comma + 3).map(|token| token.text) != Some(tokens[cursor].text)
            || !matches!(
                tokens.get(comma + 4).map(|token| token.text),
                Some(")") | Some(",") | Some(";")
            )
        {
            cursor += 1;
            continue;
        }
        let rhs = &source[tokens[cursor + 2].start..tokens[comma].start];
        let other = tokens[comma + 1].text;
        let first = tokens[cursor].text;
        replacements.push((
            tokens[cursor].start,
            tokens[comma + 3].end,
            format!("{other}={first}={rhs}"),
        ));
        cursor = comma + 4;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_statement_or_assigns(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if !is_statement_boundary(&tokens, cursor) {
            cursor += 1;
            continue;
        }
        if let Some((start, end, replacement, next)) =
            statement_or_assign_rewrite(source, &tokens, &matching_close, cursor)
        {
            replacements.push((start, end, replacement));
            cursor = next;
            continue;
        }
        cursor += 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

/// `a=a||RHS` only guards `RHS` when `RHS` is a single expression. A comma
/// sequence binds looser than `||`, so folding `!a&&(a={},a.x=1)` into
/// `a=a||{},a.x=1` runs `a.x=1` even when `a` was already truthy. Every
/// rewrite in `statement_or_assign_rewrite` therefore refuses a right-hand
/// side that still contains a top-level comma.
fn spans_top_level_comma(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    let mut depth = 0i32;
    for token in tokens.iter().take(end).skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn statement_or_assign_rewrite(
    source: &str,
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    cursor: usize,
) -> Option<(usize, usize, String, usize)> {
    if tokens[cursor].text == "!"
        && tokens
            .get(cursor + 1)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(cursor + 2).map(|token| token.text) == Some("&&")
        && tokens.get(cursor + 3).map(|token| token.text) == Some("(")
    {
        let name = tokens[cursor + 1].text;
        let close = matching_close.get(cursor + 3).copied().flatten()?;
        if tokens.get(cursor + 4).map(|token| token.text) == Some(name)
            && tokens.get(cursor + 5).map(|token| token.text) == Some("=")
            && statement_follows_paren(tokens, close)
            && !spans_top_level_comma(tokens, cursor + 6, close)
        {
            let rhs = &source[tokens[cursor + 6].start..tokens[close].start];
            let end_at = if tokens.get(close + 1).map(|token| token.text) == Some(";") {
                close + 1
            } else {
                close
            };
            return Some((
                tokens[cursor].start,
                tokens[end_at].end,
                format!("{name}={name}||{rhs};"),
                end_at + 1,
            ));
        }
    }
    if tokens[cursor].kind == TokenKind::Identifier
        && tokens.get(cursor + 1).map(|token| token.text) == Some("||")
        && tokens.get(cursor + 2).map(|token| token.text) == Some("(")
    {
        let name = tokens[cursor].text;
        let close = matching_close.get(cursor + 2).copied().flatten()?;
        if tokens.get(cursor + 3).map(|token| token.text) == Some(name)
            && tokens.get(cursor + 4).map(|token| token.text) == Some("=")
            && statement_follows_paren(tokens, close)
            && !spans_top_level_comma(tokens, cursor + 5, close)
        {
            let rhs = &source[tokens[cursor + 5].start..tokens[close].start];
            let end_at = if tokens.get(close + 1).map(|token| token.text) == Some(";") {
                close + 1
            } else {
                close
            };
            return Some((
                tokens[cursor].start,
                tokens[end_at].end,
                format!("{name}={name}||{rhs};"),
                end_at + 1,
            ));
        }
    }
    if tokens[cursor].text != "if" || tokens.get(cursor + 1).map(|token| token.text) != Some("(") {
        return None;
    }
    let cond_close = matching_close.get(cursor + 1).copied().flatten()?;
    if tokens.get(cursor + 2).map(|token| token.text) != Some("!")
        || tokens
            .get(cursor + 3)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || cond_close != cursor + 4
    {
        return None;
    }
    let name = tokens[cursor + 3].text;
    let after = cond_close + 1;
    if tokens.get(after).map(|token| token.text) == Some(name)
        && tokens.get(after + 1).map(|token| token.text) == Some("=")
    {
        let semi = top_level_stop(tokens, after + 2, &[";"])?;
        if tokens.get(semi + 1).map(|token| token.text) == Some("else")
            || spans_top_level_comma(tokens, after + 2, semi)
        {
            return None;
        }
        let rhs = &source[tokens[after + 2].start..tokens[semi].start];
        return Some((
            tokens[cursor].start,
            tokens[semi].end,
            format!("{name}={name}||{rhs};"),
            semi + 1,
        ));
    }
    if tokens.get(after).map(|token| token.text) != Some("{") {
        return None;
    }
    let block_close = matching_close.get(after).copied().flatten()?;
    if tokens.get(block_close + 1).map(|token| token.text) == Some("else") {
        return None;
    }
    if tokens.get(after + 1).map(|token| token.text) == Some(name)
        && tokens.get(after + 2).map(|token| token.text) == Some("=")
    {
        let semi = top_level_stop(tokens, after + 3, &[";"])?;
        let after_assign = if tokens.get(semi).map(|token| token.text) == Some(";") {
            semi + 1
        } else {
            semi
        };
        if after_assign != block_close || spans_top_level_comma(tokens, after + 3, semi) {
            return None;
        }
        let rhs = &source[tokens[after + 3].start..tokens[semi].start];
        return Some((
            tokens[cursor].start,
            tokens[block_close].end,
            format!("{name}={name}||{rhs};"),
            block_close + 1,
        ));
    }
    if tokens.get(after + 1).map(|token| token.text) != Some("var")
        || tokens
            .get(after + 2)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(after + 3).map(|token| token.text) != Some("=")
    {
        return None;
    }
    let temp = tokens[after + 2].text;
    let first_semi = top_level_stop(tokens, after + 4, &[";"])?;
    if tokens.get(first_semi + 1).map(|token| token.text) != Some(name)
        || tokens.get(first_semi + 2).map(|token| token.text) != Some("=")
        || tokens.get(first_semi + 3).map(|token| token.text) != Some(temp)
    {
        return None;
    }
    let after_copy = if tokens.get(first_semi + 4).map(|token| token.text) == Some(";") {
        first_semi + 5
    } else {
        first_semi + 4
    };
    if after_copy != block_close || spans_top_level_comma(tokens, after + 4, first_semi) {
        return None;
    }
    let rhs = &source[tokens[after + 4].start..tokens[first_semi].start];
    Some((
        tokens[cursor].start,
        tokens[block_close].end,
        format!("{name}={name}||{rhs};"),
        block_close + 1,
    ))
}

fn statement_follows_paren(tokens: &[Token<'_>], close: usize) -> bool {
    matches!(
        tokens.get(close + 1).map(|token| token.text),
        Some(";") | Some("}") | None
    )
}

fn side_effect_free_lvalue(tokens: &[Token<'_>], start: usize, eq_at: usize) -> bool {
    match eq_at.checked_sub(start) {
        Some(1) => tokens[start].kind == TokenKind::Identifier,
        Some(3) => {
            tokens[start].kind == TokenKind::Identifier
                && tokens[start + 1].text == "."
                && tokens[start + 2].kind == TokenKind::Identifier
        }
        Some(4) => {
            tokens[start].kind == TokenKind::Identifier
                && tokens[start + 1].text == "["
                && matches!(
                    tokens[start + 2].kind,
                    TokenKind::Identifier | TokenKind::Number
                )
                && tokens[start + 3].text == "]"
        }
        _ => false,
    }
}

fn assign_arm(tokens: &[Token<'_>], start: usize, end: usize) -> Option<(usize, usize)> {
    let mut depth = 0i32;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "=" if depth == 0 && tokens.get(index + 1).map(|token| token.text) != Some("=") => {
                return side_effect_free_lvalue(tokens, start, index).then_some((start, index));
            }
            _ => {}
        }
    }
    None
}

fn ternary_colon(tokens: &[Token<'_>], question: usize) -> Option<usize> {
    let mut depth = 0i32;
    let mut nested = 0i32;
    for index in question + 1..tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    return None;
                }
                depth -= 1;
            }
            "?" if depth == 0
                && !matches!(
                    tokens.get(index + 1).map(|token| token.text),
                    Some(".") | Some("[")
                ) =>
            {
                nested += 1;
            }
            ":" if depth == 0 => {
                if nested == 0 {
                    return Some(index);
                }
                nested -= 1;
            }
            _ => {}
        }
    }
    None
}

fn ternary_end(tokens: &[Token<'_>], after_colon: usize) -> usize {
    let mut depth = 0i32;
    let mut nested = 0i32;
    for index in after_colon..tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => {
                if depth == 0 {
                    return index;
                }
                depth -= 1;
            }
            "?" if depth == 0
                && !matches!(
                    tokens.get(index + 1).map(|token| token.text),
                    Some(".") | Some("[")
                ) =>
            {
                nested += 1;
            }
            ":" if depth == 0 && nested > 0 => nested -= 1,
            "," | ";" if depth == 0 && nested == 0 => return index,
            _ => {}
        }
    }
    tokens.len()
}

fn ternary_condition_start(tokens: &[Token<'_>], question: usize) -> usize {
    let mut depth = 0i32;
    for index in (0..question).rev() {
        match tokens[index].text {
            ")" | "]" | "}" => {
                // Inside one expression a closer is never directly followed
                // by a token that starts a new primary expression. Such a
                // pair (e.g. `}name` after an if/else block or `)name` after
                // a control header) is a statement boundary, not nesting.
                if depth == 0 && index + 1 < question && token_starts_primary(&tokens[index + 1]) {
                    return index + 1;
                }
                depth += 1;
            }
            "(" | "[" | "{" => {
                if depth == 0 {
                    return index + 1;
                }
                depth -= 1;
            }
            _ if depth == 0
                && matches!(
                    tokens[index].text,
                    "," | ";" | "=" | "=>" | "?" | ":" | "&&" | "||" | "??" | "return" | "throw"
                ) =>
            {
                return index + 1;
            }
            _ => {}
        }
    }
    0
}

fn ternary_logical_condition_start(tokens: &[Token<'_>], question: usize) -> usize {
    let mut start = ternary_condition_start(tokens, question);
    while let Some(operator) = start
        .checked_sub(1)
        .filter(|index| matches!(tokens[*index].text, "&&" | "||" | "??"))
    {
        let earlier = ternary_condition_start(tokens, operator);
        if earlier == start {
            break;
        }
        start = earlier;
    }
    start
}

fn inverted_disjunction_condition(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
) -> Option<String> {
    let mut depth = 0i32;
    let mut term_start = start;
    let mut terms = Vec::new();
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "||" if depth == 0 => {
                terms.push((term_start, index));
                term_start = index + 1;
            }
            "&&" | "??" if depth == 0 => return None,
            _ => {}
        }
    }
    if terms.is_empty() || term_start >= end {
        return None;
    }
    terms.push((term_start, end));
    let inverted = terms
        .into_iter()
        .map(|(from, to)| inverted_condition_term(source, tokens, from, to))
        .collect::<Option<Vec<_>>>()?;
    Some(inverted.join("&&"))
}

fn inverted_condition_term(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
) -> Option<String> {
    if start >= end {
        return None;
    }
    let mut depth = 0i32;
    let mut equality = None;
    for (index, token) in tokens.iter().enumerate().take(end).skip(start) {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "==" | "!=" | "===" | "!==" if depth == 0 && equality.replace(index).is_some() => {
                return None;
            }
            _ => {}
        }
    }
    if let Some(operator) = equality {
        let replacement = match tokens[operator].text {
            "==" => "!=",
            "!=" => "==",
            "===" => "!==",
            "!==" => "===",
            _ => unreachable!(),
        };
        return Some(format!(
            "{}{replacement}{}",
            &source[tokens[start].start..tokens[operator].start],
            &source[tokens[operator].end..tokens[end - 1].end]
        ));
    }
    if tokens[start].text != "!" || start + 1 >= end {
        return None;
    }
    depth = 0;
    for token in &tokens[start + 1..end] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "=" | "+=" | "-=" | "*=" | "/=" | "%=" | "**=" | "<<=" | ">>=" | ">>>=" | "&="
            | "^=" | "|=" | "&&=" | "||=" | "??=" | "?" | ":" | "&&" | "||" | "??" | "|" | "^"
            | "&" | "<" | "<=" | ">" | ">=" | "in" | "instanceof" | "<<" | ">>" | ">>>" | "+"
            | "-" | "*" | "/" | "%" | "**"
                if depth == 0 =>
            {
                return None
            }
            _ => {}
        }
    }
    Some(source[tokens[start + 1].start..tokens[end - 1].end].to_string())
}

/// Replace boolean-valued conditional arms with native short-circuit boolean
/// expressions. Unknown conditions are explicitly coerced, so the rewrite
/// preserves the conditional's `true`/`false` result instead of leaking a
/// truthy or falsy operand value through `&&`/`||`.
pub(crate) fn fold_boolean_conditional_values(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let Some((start, end, rewritten)) = boolean_conditional_value_rewrites(&output)?
            .into_iter()
            .next()
        else {
            return Ok((output, folded));
        };
        output.replace_range(start..end, &rewritten);
        folded += 1;
    }
}

pub(crate) fn boolean_conditional_value_variants(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    let mut variants = Vec::new();
    for (start, end, rewritten) in boolean_conditional_value_rewrites(source)? {
        let mut variant = source.to_string();
        variant.replace_range(start..end, &rewritten);
        if !variants.contains(&variant) {
            variants.push(variant);
        }
    }
    Ok(variants)
}

/// Remove a leading logical negation by exchanging the conditional arms.
///
/// `!test?yes:no` and `test?no:yes` have identical truthiness and evaluation
/// order. Keeping this as an objective-scored late spelling lets dictionary
/// codecs decide whether the arm topology is preferable for the artifact.
pub(crate) fn fold_negated_conditional_arms(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let mut replacement = None;
        for question in (0..tokens.len()).rev() {
            if tokens[question].text != "?"
                || matches!(
                    tokens.get(question + 1).map(|token| token.text),
                    Some(".") | Some("[")
                )
            {
                continue;
            }
            let condition_start = ternary_condition_start(&tokens, question);
            let condition_is_complete_negation = tokens
                .get(condition_start)
                .is_some_and(|token| token.text == "!")
                && condition_start + 1 < question
                && condition_start
                    .checked_sub(1)
                    .is_none_or(|previous| !matches!(tokens[previous].text, "&&" | "||" | "??"));
            let (condition_start, condition) = if condition_is_complete_negation {
                (
                    condition_start,
                    output[tokens[condition_start + 1].start..tokens[question].start].to_string(),
                )
            } else {
                let full_start = ternary_logical_condition_start(&tokens, question);
                let Some(condition) =
                    inverted_disjunction_condition(&output, &tokens, full_start, question)
                else {
                    continue;
                };
                (full_start, condition)
            };
            let Some(colon) = ternary_colon(&tokens, question) else {
                continue;
            };
            let end = ternary_end(&tokens, colon + 1);
            if question + 1 >= colon || colon + 1 >= end {
                continue;
            }
            let then_value = &output[tokens[question + 1].start..tokens[colon].start];
            let else_value = &output[tokens[colon + 1].start..tokens[end - 1].end];
            let separator = if tokens[condition_start].start > 0
                && output.as_bytes()[tokens[condition_start].start - 1].is_ascii_alphanumeric()
                && condition
                    .as_bytes()
                    .first()
                    .is_some_and(|byte| byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$'))
            {
                " "
            } else {
                ""
            };
            replacement = Some((
                tokens[condition_start].start,
                tokens[end - 1].end,
                format!("{separator}{condition}?{else_value}:{then_value}"),
            ));
            break;
        }
        let Some((start, end, rewritten)) = replacement else {
            return Ok((output, folded));
        };
        output.replace_range(start..end, &rewritten);
        folded += 1;
    }
}

fn boolean_conditional_value_rewrites(
    source: &str,
) -> Result<Vec<(usize, usize, String)>, JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::new();
    for question in (0..tokens.len()).rev() {
        if tokens[question].text != "?"
            || matches!(
                tokens.get(question + 1).map(|token| token.text),
                Some(".") | Some("[")
            )
        {
            continue;
        }
        let Some(colon) = ternary_colon(&tokens, question) else {
            continue;
        };
        let else_end = ternary_end(&tokens, colon + 1);
        let condition_start = ternary_logical_condition_start(&tokens, question);
        if condition_start >= question || colon <= question + 1 || else_end <= colon + 1 {
            continue;
        }

        let then_constant = constant_boolean_tokens(&tokens, &matching_close, question + 1, colon);
        let else_constant = constant_boolean_tokens(&tokens, &matching_close, colon + 1, else_end);
        let condition_boolean = parsed_boolean_expression(&tokens, condition_start, question);

        let condition = &source[tokens[condition_start].start..tokens[question].start];
        let then_value = &source[tokens[question + 1].start..tokens[colon].start];
        let else_value = &source[tokens[colon + 1].start..tokens[else_end - 1].end];
        let boolean_condition = render_booleanized_operand(
            condition,
            &tokens,
            condition_start,
            question,
            condition_boolean,
        );
        let negated_condition =
            render_negated_operand(condition, &tokens, condition_start, question);

        let rewritten = match (then_constant, else_constant) {
            (Some(true), Some(false)) => Some(boolean_condition),
            (Some(false), Some(true)) => Some(negated_condition),
            (Some(true), _) => Some(format!(
                "{}||{}",
                boolean_condition,
                render_logical_operand(else_value, &tokens, colon + 1, else_end, false)
            )),
            (Some(false), _) => Some(format!(
                "{}&&{}",
                negated_condition,
                render_logical_operand(else_value, &tokens, colon + 1, else_end, true)
            )),
            (_, Some(false)) => Some(format!(
                "{}&&{}",
                boolean_condition,
                render_logical_operand(then_value, &tokens, question + 1, colon, true)
            )),
            (_, Some(true)) => Some(format!(
                "{}||{}",
                negated_condition,
                render_logical_operand(then_value, &tokens, question + 1, colon, false)
            )),
            _ => None,
        };
        let Some(rewritten) = rewritten else {
            continue;
        };
        replacements.push((
            tokens[condition_start].start,
            tokens[else_end - 1].end,
            rewritten,
        ));
    }
    Ok(replacements)
}

/// Factor a repeated conditional arm through the condition.
///
/// `A?X:B?X:Y` is `(A||B)?X:Y`, while `A?(B?X:Y):Y` is
/// `(A&&B)?X:Y`. JavaScript evaluates the retained arm at exactly the same
/// point on every path; only the two tests are joined. Sequence-valued tests
/// are parenthesized by [`render_logical_operand`].
pub(crate) fn fold_common_conditional_arms(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let mut output = source.to_string();
    let mut folded = 0usize;
    loop {
        let tokens = lex(&output)?;
        let matching_close = matching_closers(&tokens);
        let mut replacement = None;

        for question in (0..tokens.len()).rev() {
            if tokens[question].text != "?"
                || matches!(
                    tokens.get(question + 1).map(|token| token.text),
                    Some(".") | Some("[")
                )
            {
                continue;
            }
            let Some(colon) = ternary_colon(&tokens, question) else {
                continue;
            };
            let end = ternary_end(&tokens, colon + 1);
            let condition_start = ternary_logical_condition_start(&tokens, question);
            if condition_start >= question || end <= colon + 1 {
                continue;
            }

            let outer_condition = &output[tokens[condition_start].start..tokens[question].start];
            let outer_then =
                strip_parenthesized_range(&tokens, &matching_close, question + 1, colon);
            let outer_else = strip_parenthesized_range(&tokens, &matching_close, colon + 1, end);

            if let Some(inner_question) = root_ternary_question(&tokens, outer_else.0, outer_else.1)
            {
                let Some(inner_colon) = ternary_colon(&tokens, inner_question) else {
                    continue;
                };
                let inner_end = ternary_end(&tokens, inner_colon + 1);
                if inner_end != outer_else.1 {
                    continue;
                }
                let inner_then = strip_parenthesized_range(
                    &tokens,
                    &matching_close,
                    inner_question + 1,
                    inner_colon,
                );
                if expression_range_text(&output, &tokens, outer_then)
                    == expression_range_text(&output, &tokens, inner_then)
                {
                    let inner_condition =
                        &output[tokens[outer_else.0].start..tokens[inner_question].start];
                    let left = render_logical_operand(
                        outer_condition,
                        &tokens,
                        condition_start,
                        question,
                        false,
                    );
                    let right = render_logical_operand(
                        inner_condition,
                        &tokens,
                        outer_else.0,
                        inner_question,
                        false,
                    );
                    let then_value = expression_range_text(&output, &tokens, outer_then);
                    let else_value = expression_range_text(
                        &output,
                        &tokens,
                        strip_parenthesized_range(
                            &tokens,
                            &matching_close,
                            inner_colon + 1,
                            inner_end,
                        ),
                    );
                    replacement = Some((
                        tokens[condition_start].start,
                        tokens[end - 1].end,
                        format!("{left}||{right}?{then_value}:{else_value}"),
                    ));
                    break;
                }
            }

            if let Some(inner_question) = root_ternary_question(&tokens, outer_then.0, outer_then.1)
            {
                let Some(inner_colon) = ternary_colon(&tokens, inner_question) else {
                    continue;
                };
                let inner_end = ternary_end(&tokens, inner_colon + 1);
                if inner_end != outer_then.1 {
                    continue;
                }
                let inner_else =
                    strip_parenthesized_range(&tokens, &matching_close, inner_colon + 1, inner_end);
                if expression_range_text(&output, &tokens, inner_else)
                    == expression_range_text(&output, &tokens, outer_else)
                {
                    let inner_condition =
                        &output[tokens[outer_then.0].start..tokens[inner_question].start];
                    let left = render_logical_operand(
                        outer_condition,
                        &tokens,
                        condition_start,
                        question,
                        true,
                    );
                    let right = render_logical_operand(
                        inner_condition,
                        &tokens,
                        outer_then.0,
                        inner_question,
                        true,
                    );
                    let then_value = expression_range_text(
                        &output,
                        &tokens,
                        strip_parenthesized_range(
                            &tokens,
                            &matching_close,
                            inner_question + 1,
                            inner_colon,
                        ),
                    );
                    let else_value = expression_range_text(&output, &tokens, outer_else);
                    replacement = Some((
                        tokens[condition_start].start,
                        tokens[end - 1].end,
                        format!("{left}&&{right}?{then_value}:{else_value}"),
                    ));
                    break;
                }
            }
        }

        let Some((start, end, rewritten)) = replacement else {
            return Ok((output, folded));
        };
        output.replace_range(start..end, &rewritten);
        folded += 1;
    }
}

fn strip_parenthesized_range(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    mut start: usize,
    mut end: usize,
) -> (usize, usize) {
    while start < end
        && tokens[start].text == "("
        && matching_close.get(start).copied().flatten() == end.checked_sub(1)
    {
        start += 1;
        end -= 1;
    }
    (start, end)
}

fn root_ternary_question(tokens: &[Token<'_>], start: usize, end: usize) -> Option<usize> {
    let mut depth = 0usize;
    for index in start..end {
        match tokens[index].text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.checked_sub(1)?,
            "?" if depth == 0
                && !matches!(
                    tokens.get(index + 1).map(|token| token.text),
                    Some(".") | Some("[")
                ) =>
            {
                return Some(index);
            }
            _ => {}
        }
    }
    None
}

fn expression_range_text<'a>(
    source: &'a str,
    tokens: &[Token<'_>],
    (start, end): (usize, usize),
) -> &'a str {
    &source[tokens[start].start..tokens[end - 1].end]
}

fn parsed_boolean_expression(tokens: &[Token<'_>], start: usize, end: usize) -> bool {
    if start >= end {
        return false;
    }
    let mut parser = ExpressionParser::new(&tokens[start..end]);
    parser
        .parse_complete()
        .is_some_and(|expression| expression_is_boolean(&expression))
}

fn expression_is_boolean(expression: &Expression<'_>) -> bool {
    match expression {
        Expression::Literal(value) => matches!(*value, "true" | "false"),
        Expression::Unary { operator, .. } => matches!(*operator, "!" | "delete"),
        Expression::Binary { operator, lhs, rhs } => match *operator {
            "==" | "!=" | "===" | "!==" | "<" | "<=" | ">" | ">=" | "in" | "instanceof" => true,
            "&&" | "||" | "??" => expression_is_boolean(lhs) && expression_is_boolean(rhs),
            _ => false,
        },
        Expression::Assignment { operator, rhs, .. } => {
            *operator == "=" && expression_is_boolean(rhs)
        }
        Expression::Conditional {
            then_value,
            else_value,
            ..
        } => expression_is_boolean(then_value) && expression_is_boolean(else_value),
        Expression::Sequence(values) => values.last().is_some_and(expression_is_boolean),
        Expression::Identifier(_)
        | Expression::Call { .. }
        | Expression::Member { .. }
        | Expression::Array(_) => false,
    }
}

fn constant_boolean_tokens(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    mut start: usize,
    mut end: usize,
) -> Option<bool> {
    while start < end
        && tokens[start].text == "("
        && matching_close.get(start).copied().flatten() == end.checked_sub(1)
    {
        start += 1;
        end -= 1;
    }
    match &tokens[start..end] {
        [token] if token.text == "true" => Some(true),
        [token] if token.text == "false" => Some(false),
        [not, zero] if not.text == "!" && zero.text == "0" => Some(true),
        [not, one] if not.text == "!" && one.text == "1" => Some(false),
        _ => None,
    }
}

fn render_booleanized_operand(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
    already_boolean: bool,
) -> String {
    if already_boolean {
        return render_logical_operand(source, tokens, start, end, false);
    }
    render_unary_operand("!!", source, tokens, start, end)
}

fn render_negated_operand(source: &str, tokens: &[Token<'_>], start: usize, end: usize) -> String {
    render_unary_operand("!", source, tokens, start, end)
}

fn render_unary_operand(
    operator: &str,
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
) -> String {
    let mut parser = ExpressionParser::new(&tokens[start..end]);
    let simple = parser.parse_complete().is_some_and(|expression| {
        matches!(
            expression,
            Expression::Identifier(_)
                | Expression::Literal(_)
                | Expression::Unary { .. }
                | Expression::Call { .. }
                | Expression::Member { .. }
                | Expression::Array(_)
        )
    });
    if simple {
        format!("{operator}{source}")
    } else {
        format!("{operator}({source})")
    }
}

fn render_logical_operand(
    source: &str,
    tokens: &[Token<'_>],
    start: usize,
    end: usize,
    for_and: bool,
) -> String {
    let mut parser = ExpressionParser::new(&tokens[start..end]);
    let needs_grouping = parser
        .parse_complete()
        .is_none_or(|expression| match expression {
            Expression::Assignment { .. }
            | Expression::Conditional { .. }
            | Expression::Sequence(_) => true,
            Expression::Binary { operator, .. } => {
                operator == "??" || (for_and && operator == "||")
            }
            _ => false,
        });
    if needs_grouping {
        format!("({source})")
    } else {
        source.to_string()
    }
}

fn token_starts_primary(token: &Token<'_>) -> bool {
    matches!(
        token.kind,
        TokenKind::Identifier | TokenKind::Number | TokenKind::String | TokenKind::Regex
    ) || matches!(
        token.text,
        "!" | "typeof" | "new" | "void" | "this" | "null" | "true" | "false" | "undefined"
    )
}

pub(crate) fn fold_same_lvalue_ternary(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor < tokens.len() {
        if tokens[cursor].text != "?"
            || matches!(
                tokens.get(cursor + 1).map(|token| token.text),
                Some(".") | Some("[")
            )
        {
            cursor += 1;
            continue;
        }
        let Some(colon) = ternary_colon(&tokens, cursor) else {
            cursor += 1;
            continue;
        };
        let else_end = ternary_end(&tokens, colon + 1);
        let Some((then_l_from, then_eq)) = assign_arm(&tokens, cursor + 1, colon) else {
            cursor += 1;
            continue;
        };
        let Some((else_l_from, else_eq)) = assign_arm(&tokens, colon + 1, else_end) else {
            cursor += 1;
            continue;
        };
        let then_lvalue = &source[tokens[then_l_from].start..tokens[then_eq].start];
        let else_lvalue = &source[tokens[else_l_from].start..tokens[else_eq].start];
        if then_lvalue != else_lvalue {
            cursor += 1;
            continue;
        }
        let cond_start = ternary_logical_condition_start(&tokens, cursor);
        if cond_start >= cursor {
            cursor += 1;
            continue;
        }
        let cond = &source[tokens[cond_start].start..tokens[cursor].start];
        let then_rhs = &source[tokens[then_eq + 1].start..tokens[colon].start];
        let else_rhs = &source[tokens[else_eq + 1].start..tokens[else_end].start];
        replacements.push((
            tokens[cond_start].start,
            tokens[else_end].start,
            format!("{then_lvalue}={cond}?{then_rhs}:{else_rhs}"),
        ));
        cursor = else_end;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn rhs_is_pure_value(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    stop: usize,
) -> bool {
    if from >= stop {
        return false;
    }
    if let Some(function) = parse_function_expression(tokens, matching_close, from) {
        return function.end + 1 == stop;
    }
    if from + 1 == stop
        && matches!(
            tokens[from].kind,
            TokenKind::Identifier | TokenKind::Number | TokenKind::String | TokenKind::Regex
        )
    {
        return true;
    }
    from + 1 == stop
        && matches!(
            tokens[from].text,
            "this" | "null" | "true" | "false" | "undefined"
        )
}

pub(crate) fn fold_or_reassign_to_ternary(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 8 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let prev = cursor
            .checked_sub(1)
            .map(|index| tokens[index].text)
            .unwrap_or(";");
        if !matches!(prev, ";" | "{" | "}" | "," | "var" | "let" | "const") {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let rhs1_from = cursor + 2;
        let Some(rhs1_stop) = top_level_stop(&tokens, rhs1_from, &[",", ";"]) else {
            cursor += 1;
            continue;
        };
        if !matches!(tokens[rhs1_stop].text, "," | ";")
            || !rhs_is_pure_value(&tokens, &matching_close, rhs1_from, rhs1_stop)
        {
            cursor += 1;
            continue;
        }
        let cond_at = rhs1_stop + 1;
        let (cond_from, after_cond) = if tokens.get(cond_at).map(|token| token.text) == Some("!")
            && tokens
                .get(cond_at + 1)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            (cond_at, cond_at + 2)
        } else if tokens
            .get(cond_at)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            (cond_at, cond_at + 1)
        } else {
            cursor += 1;
            continue;
        };
        if tokens.get(after_cond).map(|token| token.text) != Some("||")
            || tokens.get(after_cond + 1).map(|token| token.text) != Some("(")
            || tokens.get(after_cond + 2).map(|token| token.text) != Some(name)
            || tokens.get(after_cond + 3).map(|token| token.text) != Some("=")
        {
            cursor += 1;
            continue;
        }
        let paren_open = after_cond + 1;
        let Some(paren_close) = matching_close.get(paren_open).copied().flatten() else {
            cursor += 1;
            continue;
        };
        let rhs2_from = after_cond + 4;
        // The parenthesized group must be exactly `name=rhs2`: a top-level
        // comma would splice extra guarded expressions into the ternary spot
        // and run them unconditionally. The ternary also evaluates rhs2
        // before `name` is written, so any read of `name` inside it would see
        // the pre-assignment value instead of rhs1.
        let group_comma =
            top_level_stop(&tokens, rhs2_from, &[",", ";"]).is_some_and(|stop| stop < paren_close);
        // In a declarator, everything after the folded initializer up to the
        // statement end would turn into further declarators, so the group
        // must already end the statement.
        let declarator = matches!(prev, "var" | "let" | "const");
        let statement_ends = matches!(
            tokens.get(paren_close + 1).map(|token| token.text),
            Some(";") | Some("}") | None
        );
        if rhs2_from >= paren_close
            || group_comma
            || (declarator && !statement_ends)
            || identifier_occurs(&tokens, cond_from, after_cond, name)
            || identifier_occurs(&tokens, rhs1_from, rhs1_stop, name)
            || identifier_occurs(&tokens, rhs2_from, paren_close, name)
        {
            cursor += 1;
            continue;
        }
        let cond = &source[tokens[cond_from].start..tokens[after_cond - 1].end];
        let rhs1 = &source[tokens[rhs1_from].start..tokens[rhs1_stop].start];
        let rhs2 = &source[tokens[rhs2_from].start..tokens[paren_close].start];
        replacements.push((
            tokens[cursor].start,
            tokens[paren_close].end,
            format!("{name}={cond}?{rhs1}:{rhs2}"),
        ));
        cursor = paren_close + 1;
    }
    Ok(apply_token_rewrites(source, replacements))
}

pub(crate) fn fold_redundant_null_undefined_or(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 6 < tokens.len() {
        let Some((name, first_end)) = match_nullish_ident_check(&tokens, cursor) else {
            cursor += 1;
            continue;
        };
        if tokens.get(first_end).map(|token| token.text) != Some("||") {
            cursor += 1;
            continue;
        }
        let Some((other, second_end)) = match_nullish_ident_check(&tokens, first_end + 1) else {
            cursor += 1;
            continue;
        };
        if name != other {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[second_end - 1].end,
            format!("{name}==null"),
        ));
        cursor = second_end;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn match_nullish_ident_check<'tok>(
    tokens: &'tok [Token<'tok>],
    at: usize,
) -> Option<(&'tok str, usize)> {
    if tokens.get(at).map(|token| token.text) == Some("null")
        && matches!(
            tokens.get(at + 1).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens
            .get(at + 2)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return Some((tokens[at + 2].text, at + 3));
    }
    if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && matches!(
            tokens.get(at + 1).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens.get(at + 2).map(|token| token.text) == Some("null")
    {
        return Some((tokens[at].text, at + 3));
    }
    if tokens.get(at).map(|token| token.text) == Some("void")
        && tokens.get(at + 1).map(|token| token.text) == Some("0")
        && matches!(
            tokens.get(at + 2).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens
            .get(at + 3)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return Some((tokens[at + 3].text, at + 4));
    }
    if tokens
        .get(at)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && matches!(
            tokens.get(at + 1).map(|token| token.text),
            Some("==") | Some("===")
        )
        && tokens.get(at + 2).map(|token| token.text) == Some("void")
        && tokens.get(at + 3).map(|token| token.text) == Some("0")
    {
        return Some((tokens[at].text, at + 4));
    }
    None
}

pub(crate) fn fold_ident_ternary_to_or(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 4 < tokens.len() {
        if tokens[cursor].kind != TokenKind::Identifier
            || tokens.get(cursor + 1).map(|token| token.text) != Some("?")
            || tokens.get(cursor + 2).map(|token| token.text) != Some(tokens[cursor].text)
            || tokens.get(cursor + 3).map(|token| token.text) != Some(":")
            || ternary_logical_condition_start(&tokens, cursor + 1) != cursor
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor].text;
        let else_from = cursor + 4;
        if !simple_or_arm(&tokens, else_from) {
            cursor += 1;
            continue;
        }
        let else_end = else_from + simple_or_arm_width(&tokens, else_from);
        let else_expr = &source[tokens[else_from].start..tokens[else_end - 1].end];
        replacements.push((
            tokens[cursor].start,
            tokens[else_end - 1].end,
            format!("{name}||{else_expr}"),
        ));
        cursor = else_end;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn simple_or_arm(tokens: &[Token<'_>], from: usize) -> bool {
    matches!(
        tokens.get(from).map(|token| token.text),
        Some("[]" | "null" | "true" | "false" | "undefined")
    ) || (tokens.get(from).map(|token| token.text) == Some("[")
        && tokens.get(from + 1).map(|token| token.text) == Some("]"))
        || (tokens
            .get(from)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && !matches!(
                tokens.get(from + 1).map(|token| token.text),
                Some("=")
                    | Some("+=")
                    | Some("-=")
                    | Some("*=")
                    | Some("/=")
                    | Some("%=")
                    | Some("<<=")
                    | Some(">>=")
                    | Some(">>>=")
                    | Some("&=")
                    | Some("|=")
                    | Some("^=")
                    | Some("&&=")
                    | Some("||=")
                    | Some("??=")
            ))
        || tokens
            .get(from)
            .is_some_and(|token| token.kind == TokenKind::Number)
        || tokens
            .get(from)
            .is_some_and(|token| token.kind == TokenKind::String)
}

pub(crate) fn fold_or_assignment_parens(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 3 < tokens.len() {
        if !matches!(tokens[cursor].text, "||" | "&&") {
            cursor += 1;
            continue;
        }
        let Some((assign_from, eq_at)) = assignment_lvalue_eq(&tokens, &matching_close, cursor + 1)
        else {
            cursor += 1;
            continue;
        };
        let Some(stop) = top_level_stop(&tokens, eq_at + 1, &[",", ";", ")", "]", "}"]) else {
            cursor += 1;
            continue;
        };
        if stop <= eq_at + 1 {
            cursor += 1;
            continue;
        }
        let from = tokens[assign_from].start;
        let to = tokens[stop - 1].end;
        replacements.push((from, to, format!("({})", &source[from..to])));
        cursor = stop;
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn assignment_lvalue_eq(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
) -> Option<(usize, usize)> {
    let first = tokens.get(start)?;
    if first.kind != TokenKind::Identifier && first.text != "this" {
        return None;
    }
    let mut index = start + 1;
    loop {
        match tokens.get(index).map(|token| token.text) {
            Some(".") => {
                if tokens.get(index + 1)?.kind != TokenKind::Identifier {
                    return None;
                }
                index += 2;
            }
            Some("[") => {
                let close = matching_close.get(index).copied().flatten()?;
                if close <= index {
                    return None;
                }
                index = close + 1;
            }
            Some("=") => return Some((start, index)),
            _ => return None,
        }
    }
}

fn simple_or_arm_width(tokens: &[Token<'_>], from: usize) -> usize {
    if tokens.get(from).map(|token| token.text) == Some("[")
        && tokens.get(from + 1).map(|token| token.text) == Some("]")
    {
        2
    } else {
        1
    }
}

pub(crate) fn fold_not_gt_zero_length(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut cursor = 0usize;
    while cursor + 5 < tokens.len() {
        if tokens[cursor].text != "!"
            || tokens.get(cursor + 1).map(|token| token.text) != Some("(")
            || tokens
                .get(cursor + 2)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor + 3).map(|token| token.text) != Some(">")
            || tokens.get(cursor + 4).map(|token| token.text) != Some("0")
            || tokens.get(cursor + 5).map(|token| token.text) != Some(")")
        {
            cursor += 1;
            continue;
        }
        let name = tokens[cursor + 2].text;
        if !name_is_nonnegative_length_copy(&tokens, &matching_close, cursor, name) {
            cursor += 1;
            continue;
        }
        replacements.push((
            tokens[cursor].start,
            tokens[cursor + 5].end,
            format!("!{name}"),
        ));
        cursor += 6;
    }
    Ok(apply_token_rewrites(source, replacements))
}

#[cfg(test)]
mod tests {
    use super::fold_statement_or_assigns;

    /// `||` binds tighter than `,`, so the guarded body of
    /// `!m&&(m={},m.handled=!0,m.type="generic")` cannot become the right-hand
    /// side of `m=m||…`: the property stores would run even when `m` was
    /// already truthy, overwriting a caller-supplied object. Every shape this
    /// fold recognizes has to refuse a comma sequence.
    /// `(e=e.trim())?e:null` may become `e.trim()||null` because the condition's
    /// value *is* `e`. Once the assignment is only an operand -- `""!=(e=e.trim())`
    /// -- the condition evaluates to a boolean and the fold would return `true`
    /// where the source returned the string.
    #[test]
    fn assigned_truthy_ternary_needs_the_assignment_to_be_the_whole_condition() {
        use super::fold_assigned_truthy_ternaries;
        for source in [
            r#"function W(e){return "string"==typeof e&&""!=(e=e.trim())?e:null}"#,
            r#"function W(e){return ""!=(e=e.trim())?e:null}"#,
            r#"function W(e){return !(e=e.trim())?e:null}"#,
            r#"function W(e,t){return t+(e=e.trim())?e:null}"#,
        ] {
            let (out, count) = fold_assigned_truthy_ternaries(source).unwrap();
            assert_eq!(count, 0, "folded an operand as if it were the condition: {out}");
            assert_eq!(out, source);
        }

        for (source, expected) in [
            (
                r#"function W(e){return (e=e.trim())?e:null}"#,
                r#"function W(e){return e.trim()||null}"#,
            ),
            (
                r#"function W(e){var x=(e=e.trim())?e:null;return x}"#,
                r#"function W(e){var x=e.trim()||null;return x}"#,
            ),
        ] {
            let (out, count) = fold_assigned_truthy_ternaries(source).unwrap();
            assert_eq!(count, 1, "{out}");
            assert_eq!(out, expected);
        }
    }

    #[test]
    fn or_assign_refuses_comma_sequenced_right_hand_sides() {
        for source in [
            r#"function f(m){!m&&(m={},m.handled=!0,m.type="generic");return m}"#,
            r#"function f(m){m||(m={},m.handled=!0,m.type="generic");return m}"#,
            r#"function f(m){if(!m)m={},m.handled=!0;return m}"#,
            r#"function f(m){if(!m){m={},m.handled=!0;}return m}"#,
            r#"function f(m){if(!m){var t={},u=1;m=t;}return m}"#,
        ] {
            let (out, count) = fold_statement_or_assigns(source).unwrap();
            assert_eq!(count, 0, "rewrote a comma sequence: {out}");
            assert_eq!(out, source);
        }
    }

    /// The single-expression forms still fold; the comma guard must not turn
    /// the whole fold off.
    #[test]
    fn or_assign_still_folds_single_expression_right_hand_sides() {
        for (source, expected) in [
            (
                r#"function f(m){!m&&(m={handled:!0});return m}"#,
                r#"function f(m){m=m||{handled:!0};return m}"#,
            ),
            (
                r#"function f(m){m||(m={handled:!0});return m}"#,
                r#"function f(m){m=m||{handled:!0};return m}"#,
            ),
            (
                r#"function f(m){if(!m)m={handled:!0};return m}"#,
                r#"function f(m){m=m||{handled:!0};return m}"#,
            ),
            (
                r#"function f(m){if(!m){m=g(1,2);}return m}"#,
                r#"function f(m){m=m||g(1,2);return m}"#,
            ),
            (
                r#"function f(m){if(!m){var t=g(1,2);m=t;}return m}"#,
                r#"function f(m){m=m||g(1,2);return m}"#,
            ),
        ] {
            let (out, count) = fold_statement_or_assigns(source).unwrap();
            assert_eq!(count, 1, "{out}");
            assert_eq!(out, expected);
        }
    }
}
