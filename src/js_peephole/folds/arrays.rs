use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

/// Write an array that is built by consecutive pushes as the array literal it is.
///
/// `x=[];x.push(a);x.push(b)` and `x=[a,b]` produce the same array. The pushes
/// have to be the statements that directly follow the empty literal, so nothing
/// runs in between that could observe the half-built array, and no pushed value
/// may read `x` itself — `x=[x.length]` would then see an array that no longer
/// exists at that point. `push` returns the new length; in statement position
/// that value is discarded, which is the only position this fold accepts.
///
/// A comma-sequence push that is the last operand of a grouping
/// (`(x=[],x.push(a))`) ends at `)` rather than `;` or `,`; the grouping stays.
///
/// The rewrite keeps the **binding's** terminator, not the last push's, except
/// when that last push is grouping-final. They differ whenever the pushes
/// continue as a comma sequence (`let k=[];k.push(a),k.push(b),f=k;`): splicing
/// that trailing comma in after a declarator turns the next assignment into a
/// declarator of its own, which redeclares whatever name it assigns. A comma
/// binding whose run ends at `;` is refused for the mirror reason — the
/// statement that follows may be a declaration, which no comma can join.
///
/// `Array.prototype.push` honours an inherited index setter and an array
/// literal does not, so this runs only under `assume_pristine_builtins`.
pub(crate) fn fold_fresh_empty_array_pushes(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let statements = statement_positions(&tokens);
    let mut replacements = Vec::<(usize, usize, String)>::new();
    let mut index = 0usize;
    while index + 4 < tokens.len() {
        let Some((name, terminator, mut cursor)) = empty_array_binding(&tokens, &statements, index)
        else {
            index += 1;
            continue;
        };
        let mut items = Vec::<&str>::new();
        let mut last_separator = terminator;
        while let Some((after, item, separator)) = pushed_element(
            &tokens,
            &matching_close,
            &statements,
            source,
            cursor,
            name,
            terminator,
        ) {
            items.push(item);
            last_separator = separator;
            cursor = after;
            if separator == ")" {
                break;
            }
        }
        if items.is_empty() {
            index += 1;
            continue;
        }
        let grouping_final = last_separator == ")";
        if terminator == "," && last_separator != "," && !grouping_final {
            index += 1;
            continue;
        }
        let emit_terminator = if grouping_final { "" } else { terminator };
        replacements.push((
            tokens[index + 2].end,
            tokens[cursor - 1].end,
            format!("{}]{emit_terminator}", items.join(",")),
        ));
        index = cursor;
    }
    Ok(apply_token_rewrites(source, replacements))
}

/// Whether each token sits somewhere a statement can.
///
/// Paren depth alone is the wrong question: a function passed as a call
/// argument holds ordinary statements while the call's parenthesis is still
/// open. What matters is the *innermost* bracket open at this token, and only
/// two kinds disqualify it — an array literal, and a `for` header, where
/// `k=[];k.push(1),x;` is a loop condition rather than a statement list. A
/// brace re-opens statement position, so an enclosing function body clears the
/// context.
fn statement_positions(tokens: &[Token<'_>]) -> Vec<bool> {
    let mut allowed = Vec::with_capacity(tokens.len());
    let mut stack = Vec::<bool>::new();
    for (index, token) in tokens.iter().enumerate() {
        allowed.push(stack.last().copied().unwrap_or(true));
        match token.text {
            "{" => stack.push(true),
            "[" => stack.push(false),
            "(" => {
                let head = index
                    .checked_sub(1)
                    .is_some_and(|previous| tokens[previous].text == "for");
                stack.push(!head && stack.last().copied().unwrap_or(true));
            }
            "}" | "]" | ")" => {
                stack.pop();
            }
            _ => {}
        }
    }
    allowed
}

/// `NAME=[]` followed by `;` or `,`, in statement position.
fn empty_array_binding<'src>(
    tokens: &[Token<'src>],
    statements: &[bool],
    index: usize,
) -> Option<(&'src str, &'src str, usize)> {
    let name_token = tokens.get(index)?;
    if name_token.kind != TokenKind::Identifier || !statements[index] {
        return None;
    }
    if index
        .checked_sub(1)
        .is_some_and(|previous| matches!(tokens[previous].text, "." | "?."))
    {
        return None;
    }
    if tokens.get(index + 1)?.text != "="
        || tokens.get(index + 2)?.text != "["
        || tokens.get(index + 3)?.text != "]"
    {
        return None;
    }
    let terminator = tokens.get(index + 4)?.text;
    if !matches!(terminator, ";" | ",") {
        return None;
    }
    Some((name_token.text, terminator, index + 5))
}

/// `NAME.push(ARG)` followed by `;` or `,`, where `ARG` never reads `NAME`.
///
/// A comma-sequence push that closes a grouping (`(NAME=[],NAME.push(ARG))`)
/// ends at the group's `)`. The `)` is left in place.
fn pushed_element<'src>(
    tokens: &[Token<'src>],
    matching_close: &[Option<usize>],
    statements: &[bool],
    source: &'src str,
    cursor: usize,
    name: &str,
    binding_terminator: &str,
) -> Option<(usize, &'src str, &'src str)> {
    if tokens.get(cursor)?.kind != TokenKind::Identifier
        || tokens[cursor].text != name
        || !statements[cursor]
        || cursor
            .checked_sub(1)
            .is_some_and(|previous| matches!(tokens[previous].text, "." | "?."))
        || tokens.get(cursor + 1)?.text != "."
        || tokens.get(cursor + 2)?.text != "push"
        || tokens.get(cursor + 3)?.text != "("
    {
        return None;
    }
    let close = matching_close.get(cursor + 3).copied().flatten()?;
    let separator = tokens.get(close + 1)?.text;
    let (after, separator) = if matches!(separator, ";" | ",") {
        (close + 2, separator)
    } else if binding_terminator == "," && separator == ")" {
        (close + 1, ")")
    } else {
        return None;
    };
    // One element per call: a spread or a second argument is a different
    // operation, and `push()` with no argument is not an element at all.
    if close == cursor + 4 || tokens[cursor + 4].text == "..." {
        return None;
    }
    if argument_has_root_comma(tokens, cursor + 4, close)
        || reads_binding(tokens, cursor + 4, close, name)
    {
        return None;
    }
    Some((
        after,
        &source[tokens[cursor + 4].start..tokens[close].start],
        separator,
    ))
}

fn argument_has_root_comma(tokens: &[Token<'_>], from: usize, to: usize) -> bool {
    let mut depth = 0i32;
    for token in &tokens[from..to] {
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            "," if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

fn reads_binding(tokens: &[Token<'_>], from: usize, to: usize, name: &str) -> bool {
    tokens[from..to].iter().enumerate().any(|(offset, token)| {
        token.kind == TokenKind::Identifier
            && token.text == name
            && (offset == 0 || !matches!(tokens[from + offset - 1].text, "." | "?."))
    })
}
