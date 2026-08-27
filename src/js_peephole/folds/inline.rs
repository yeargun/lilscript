//! Inline a function bound once and read once.
//!
//! Lowering names every value it materializes, so a helper used at a single
//! site still costs a declarator: `var Y=(a,b,c)=>E(l,a,b,c)` followed by one
//! `Y`. The name buys nothing -- there is no second reference to share it --
//! and the declarator is four bytes of pure overhead plus a spelling the
//! compressor has to learn.
//!
//! Creating a function is pure. It reads nothing, writes nothing and cannot
//! throw, so the creation may be delayed to wherever the value is read without
//! reordering anything observable. What is *not* free is identity: a closure
//! created once and read once is one object, and moving its creation inside a
//! loop or a nested function would build a fresh one each time round. Two
//! programs that differ only in closure identity still differ -- `removeEventListener`
//! and a `WeakMap` key both notice -- so the fold refuses whenever the read can
//! run a different number of times than the declaration:
//!
//! 1. The read sits in the same function scope as the declaration.
//! 2. No loop keyword stands between them.
//!
//! Both are decided from [`BindingResolution`], which maps every identifier to
//! the declaration it actually refers to, so a shadowed or reassigned name is
//! never mistaken for this one.

use crate::js_peephole::binding::{BindingResolution, Resolution};
use crate::js_peephole::rewrite::{
    apply_token_rewrites, is_property_identifier, is_statement_boundary,
};
use crate::js_peephole::scope::parse_function_expression;
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;
use std::collections::HashMap;

pub(crate) fn inline_single_use_functions(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let resolution = BindingResolution::new(&tokens);

    let mut uses = HashMap::<usize, Vec<usize>>::new();
    for index in 0..tokens.len() {
        if let Resolution::Bound(declaration) = resolution.resolve(index) {
            uses.entry(declaration).or_default().push(index);
        }
    }

    let mut rewrites = Vec::<(usize, usize, String)>::new();
    let mut claimed = Vec::<(usize, usize)>::new();
    let mut count = 0usize;

    for index in 0..tokens.len() {
        if !matches!(tokens[index].text, "var" | "let" | "const") {
            continue;
        }
        // Only a declaration standing as its own statement. A `for( var …` head
        // shares its list with the loop and is not ours to take apart.
        if index > 0 && !matches!(tokens[index - 1].text, ";" | "{" | "}" | ")") {
            continue;
        }
        let Some(list) = declarator_list(&tokens, &matching_close, index) else {
            continue;
        };
        for (position, declarator) in list.iter().enumerate() {
            let Declarator { name, value, .. } = *declarator;
            let Some((start, end)) = value else { continue };
            if !is_function_literal(&tokens, &matching_close, start, end) {
                continue;
            }
            let Resolution::Bound(declaration) = resolution.resolve(name) else {
                continue;
            };
            if declaration != name {
                continue;
            }
            let Some(sites) = uses.get(&declaration) else {
                continue;
            };
            let reads = sites
                .iter()
                .copied()
                .filter(|site| *site != declaration)
                .collect::<Vec<_>>();
            if reads.len() != 1 {
                continue;
            }
            let read = reads[0];
            // A recursive helper reads itself from inside its own body; there is
            // no site outside to move it to.
            if read <= end {
                continue;
            }
            if !is_plain_read(&tokens, read) {
                continue;
            }
            if resolution.scope_index_at(read) != resolution.scope_index_at(declaration) {
                continue;
            }
            if read_repeats(&tokens, &matching_close, declaration, read) {
                continue;
            }
            if !slots_without_parentheses(&tokens, read) {
                continue;
            }

            let Some(removal) = declarator_removal(&tokens, &list, position, index) else {
                continue;
            };
            if claimed
                .iter()
                .any(|(s, e)| removal.0 < *e && *s < removal.1)
            {
                continue;
            }
            claimed.push(removal);

            let text = source[tokens[start].start..tokens[end - 1].end].to_string();
            rewrites.push((removal.0, removal.1, String::new()));
            rewrites.push((tokens[read].start, tokens[read].end, text));
            count += 1;
        }
    }

    rewrites.sort_unstable_by_key(|(start, _, _)| *start);
    let (output, _) = apply_token_rewrites(source, rewrites);
    Ok((output, count))
}

#[derive(Clone, Copy)]
pub(super) struct Declarator {
    /// Index of the bound name.
    pub(super) name: usize,
    /// Half-open token range of the initializer, when there is one.
    pub(super) value: Option<(usize, usize)>,
    /// Index of the `,` or `;` that closes this declarator.
    pub(super) close: usize,
}

/// Split `var a=1,b=2;` into its declarators. Returns `None` for anything that
/// is not a plain list of `name` / `name = value` -- a destructuring pattern
/// binds several names at once and is left alone.
pub(super) fn declarator_list(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    keyword: usize,
) -> Option<Vec<Declarator>> {
    let mut list = Vec::new();
    let mut cursor = keyword + 1;
    loop {
        let name = cursor;
        if tokens.get(name)?.kind != TokenKind::Identifier {
            return None;
        }
        cursor = name + 1;
        let value = if tokens.get(cursor)?.text == "=" {
            let start = cursor + 1;
            let end = declarator_end(tokens, matching_close, start)?;
            cursor = end;
            Some((start, end))
        } else {
            None
        };
        let close = cursor;
        match tokens.get(close)?.text {
            "," => {
                list.push(Declarator { name, value, close });
                cursor = close + 1;
            }
            ";" => {
                list.push(Declarator { name, value, close });
                return Some(list);
            }
            _ => return None,
        }
    }
}

/// Index of the `,` or `;` that ends the initializer starting at `from`.
fn declarator_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
) -> Option<usize> {
    let mut cursor = from;
    while cursor < tokens.len() {
        match tokens[cursor].text {
            "(" | "[" | "{" => cursor = matching_close[cursor]? + 1,
            "," | ";" => return Some(cursor),
            _ => cursor += 1,
        }
    }
    None
}

/// The byte range to delete so the list stays well formed: the whole statement
/// when this is the only declarator, otherwise the declarator and one comma.
fn declarator_removal(
    tokens: &[Token<'_>],
    list: &[Declarator],
    position: usize,
    keyword: usize,
) -> Option<(usize, usize)> {
    let declarator = list[position];
    if list.len() == 1 {
        return Some((tokens[keyword].start, tokens[declarator.close].end));
    }
    if position == 0 {
        // `var a=…,b=…` -> `var b=…`: take the name through its comma.
        return Some((tokens[declarator.name].start, tokens[declarator.close].end));
    }
    // `var a=…,b=…` -> `var a=…`: take the preceding comma through the value.
    let previous = list[position - 1].close;
    Some((tokens[previous].start, tokens[declarator.close].start))
}

/// True when the read can run more times than the declaration, because some
/// loop starting after the declaration encloses it. A loop that surrounds both
/// -- or one that lives in an unrelated function in between -- changes nothing:
/// the two run together, however often that is.
fn read_repeats(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    declaration: usize,
    read: usize,
) -> bool {
    for keyword in declaration..read {
        if !matches!(tokens[keyword].text, "for" | "while" | "do") {
            continue;
        }
        if keyword > 0 && matches!(tokens[keyword - 1].text, "." | "?.") {
            continue;
        }
        let Some(end) = loop_extent(tokens, matching_close, keyword) else {
            // An extent we cannot bound is one we cannot clear.
            return true;
        };
        if read < end {
            return true;
        }
    }
    false
}

/// Exclusive end of the loop statement beginning at `keyword`.
fn loop_extent(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    keyword: usize,
) -> Option<usize> {
    let body = if tokens[keyword].text == "do" {
        keyword + 1
    } else {
        let head = keyword + 1;
        if tokens.get(head)?.text != "(" {
            return None;
        }
        matching_close[head]? + 1
    };
    let end = statement_extent(tokens, matching_close, body)?;
    if tokens[keyword].text != "do" {
        return Some(end);
    }
    // `do … while (…)` carries its test after the body.
    let mut cursor = end;
    if tokens.get(cursor)?.text != "while" {
        return None;
    }
    cursor += 1;
    if tokens.get(cursor)?.text != "(" {
        return None;
    }
    cursor = matching_close[cursor]? + 1;
    if tokens.get(cursor).map(|token| token.text) == Some(";") {
        cursor += 1;
    }
    Some(cursor)
}

/// Exclusive end of the statement beginning at `from`, block or not.
fn statement_extent(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
) -> Option<usize> {
    if tokens.get(from)?.text == "{" {
        return Some(matching_close[from]? + 1);
    }
    let mut cursor = from;
    while cursor < tokens.len() {
        match tokens[cursor].text {
            "(" | "[" | "{" => cursor = matching_close[cursor]? + 1,
            ";" => return Some(cursor + 1),
            "}" => return Some(cursor),
            _ => cursor += 1,
        }
    }
    None
}

/// True when the initializer is exactly one function or arrow literal.
fn is_function_literal(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    start: usize,
    end: usize,
) -> bool {
    if start >= end {
        return false;
    }
    let mut head = start;
    if tokens[head].text == "async" {
        head += 1;
    }
    if head >= end {
        return false;
    }
    if tokens[head].text == "function" {
        // The body's `}` has to be the last token, or the initializer is a
        // larger expression that merely begins with a function.
        return tokens[end - 1].text == "}"
            && matching_close
                .iter()
                .enumerate()
                .any(|(open, close)| open > head && *close == Some(end - 1));
    }
    // `x => …` or `(…) => …`, and nothing wrapped around it.
    let arrow = if tokens[head].kind == TokenKind::Identifier {
        head + 1
    } else if tokens[head].text == "(" {
        match matching_close[head] {
            Some(close) => close + 1,
            None => return false,
        }
    } else {
        return false;
    };
    arrow < end && tokens[arrow].text == "=>"
}

/// A read, not a write: `Y` on its own, never `Y=…`, `Y++` or `Y+=…`.
fn is_plain_read(tokens: &[Token<'_>], at: usize) -> bool {
    let after_is_write = tokens.get(at + 1).is_some_and(|token| {
        token.text == "="
            || token.text == "++"
            || token.text == "--"
            || (token.text.len() >= 2
                && token.text.ends_with('=')
                && !matches!(token.text, "==" | "===" | "!=" | "!==" | "<=" | ">="))
    });
    let before_is_write = at > 0 && matches!(tokens[at - 1].text, "++" | "--");
    !after_is_write && !before_is_write
}

/// True when a function literal dropped at this site needs no parentheses of
/// its own: the neighbours already delimit it, so nothing can extend an arrow
/// body or start a statement with `function`.
fn slots_without_parentheses(tokens: &[Token<'_>], at: usize) -> bool {
    let before = match at.checked_sub(1) {
        Some(previous) => tokens[previous].text,
        None => return false,
    };
    let after = match tokens.get(at + 1) {
        Some(token) => token.text,
        None => return false,
    };
    matches!(before, "(" | "," | "[" | ":" | "=" | "return" | "?")
        && matches!(after, ")" | "," | "]" | ";" | "}" | ":")
}

/// Replace `function f(a,b){g(a,b)}` and `f=(a,b)=>{g(a,b)}` with calls to `g`
/// when every use of `f` is a call. A one-line arity wrapper is larger than
/// writing the callee at each site, and the wrapper's function object is
/// unobservable if nothing reads `f` except by calling it.
pub(crate) fn fold_forwarding_call_wrappers(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let resolution = BindingResolution::new(&tokens);
    let mut replacements = Vec::new();
    for index in 0..tokens.len() {
        let Some((name_at, callee, delete_start, delete_end)) =
            forwarding_wrapper(&tokens, &matching_close, index)
        else {
            continue;
        };
        let Some(uses) = forwarding_wrapper_call_uses(&tokens, &resolution, name_at) else {
            continue;
        };
        for use_at in uses {
            replacements.push((tokens[use_at].start, tokens[use_at].end, callee.to_string()));
        }
        replacements.push((delete_start, delete_end, String::new()));
    }
    Ok(apply_token_rewrites(source, replacements))
}

fn forwarding_wrapper_call_uses(
    tokens: &[Token<'_>],
    resolution: &BindingResolution<'_>,
    name_at: usize,
) -> Option<Vec<usize>> {
    let name = tokens[name_at].text;
    let home = match resolution.resolve(name_at) {
        Resolution::Bound(declaration) => declaration,
        Resolution::Free => name_at,
        Resolution::Unresolved => return None,
    };
    let free_home = matches!(resolution.resolve(name_at), Resolution::Free);
    let mut uses = Vec::new();
    for index in 0..tokens.len() {
        if index == name_at || index == home {
            continue;
        }
        if tokens[index].kind != TokenKind::Identifier || tokens[index].text != name {
            continue;
        }
        match resolution.resolve(index) {
            Resolution::Bound(declaration) if !free_home && declaration == home => {
                if !identifier_is_call_use(tokens, index) {
                    return None;
                }
                uses.push(index);
            }
            Resolution::Free if free_home => {
                if !identifier_is_call_use(tokens, index) {
                    return None;
                }
                uses.push(index);
            }
            Resolution::Unresolved => return None,
            _ => {}
        }
    }
    if uses.is_empty() {
        None
    } else {
        Some(uses)
    }
}

fn identifier_is_call_use(tokens: &[Token<'_>], index: usize) -> bool {
    tokens.get(index + 1).map(|token| token.text) == Some("(")
        && tokens.get(index.wrapping_sub(1)).map(|token| token.text) != Some("new")
}

fn forwarding_wrapper<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    index: usize,
) -> Option<(usize, &'a str, usize, usize)> {
    if let Some(found) = forwarding_declaration(tokens, matching_close, index) {
        return Some(found);
    }
    forwarding_assignment(tokens, matching_close, index)
}

fn forwarding_declaration<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    function_at: usize,
) -> Option<(usize, &'a str, usize, usize)> {
    if tokens.get(function_at).map(|token| token.text) != Some("function")
        || !is_statement_boundary(tokens, function_at)
        || tokens
            .get(function_at + 1)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(function_at + 2).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let name_at = function_at + 1;
    let expr = parse_function_expression(tokens, matching_close, function_at)?;
    let params = simple_forwarding_params(tokens, expr.params_from, expr.params_to)?;
    let callee = forwarding_callee(tokens, matching_close, &expr, &params)?;
    if callee == tokens[name_at].text {
        return None;
    }
    Some((
        name_at,
        callee,
        tokens[function_at].start,
        tokens[expr.end].end,
    ))
}

fn assignment_list_boundary(tokens: &[Token<'_>], name_at: usize) -> bool {
    let prev = name_at
        .checked_sub(1)
        .map(|index| tokens[index].text)
        .unwrap_or(";");
    matches!(
        prev,
        ";" | "{" | "}" | "," | "var" | "let" | "const" | "else"
    )
}

fn forwarding_assignment<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    name_at: usize,
) -> Option<(usize, &'a str, usize, usize)> {
    if tokens
        .get(name_at)
        .is_none_or(|token| token.kind != TokenKind::Identifier)
        || is_property_identifier(tokens, name_at)
        || !assignment_list_boundary(tokens, name_at)
        || tokens.get(name_at + 1).map(|token| token.text) != Some("=")
        || tokens.get(name_at + 2).map(|token| token.text) == Some("=")
    {
        return None;
    }
    let expr = parse_function_expression(tokens, matching_close, name_at + 2)?;
    if expr.named {
        return None;
    }
    let params = simple_forwarding_params(tokens, expr.params_from, expr.params_to)?;
    let callee = forwarding_callee(tokens, matching_close, &expr, &params)?;
    if callee == tokens[name_at].text {
        return None;
    }
    let mut delete_end = tokens[expr.end].end;
    if tokens.get(expr.end + 1).map(|token| token.text) == Some(",") {
        delete_end = tokens[expr.end + 1].end;
    }
    Some((name_at, callee, tokens[name_at].start, delete_end))
}

fn simple_forwarding_params<'a>(
    tokens: &'a [Token<'a>],
    from: usize,
    to: usize,
) -> Option<Vec<&'a str>> {
    if from == to {
        return Some(Vec::new());
    }
    let mut params = Vec::new();
    let mut cursor = from;
    loop {
        if tokens
            .get(cursor)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(cursor).map(|token| token.text) == Some("...")
        {
            return None;
        }
        params.push(tokens[cursor].text);
        cursor += 1;
        if cursor == to {
            break;
        }
        if tokens.get(cursor).map(|token| token.text) != Some(",") {
            return None;
        }
        cursor += 1;
        if cursor == to {
            return None;
        }
    }
    Some(params)
}

fn forwarding_callee<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    expr: &crate::js_peephole::scope::FunctionExpression,
    params: &[&str],
) -> Option<&'a str> {
    let (body, body_end) = if let Some(block_open) = expr.block_open {
        (block_open + 1, expr.end)
    } else {
        let mut arrow = expr.params_to;
        while arrow < expr.end && tokens.get(arrow).map(|token| token.text) != Some("=>") {
            arrow += 1;
        }
        if tokens.get(arrow).map(|token| token.text) != Some("=>") {
            return None;
        }
        (arrow + 1, expr.end + 1)
    };
    let mut body = body;
    if tokens.get(body).map(|token| token.text) == Some("return") {
        body += 1;
    }
    if tokens
        .get(body)
        .is_none_or(|token| token.kind != TokenKind::Identifier)
        || tokens.get(body + 1).map(|token| token.text) != Some("(")
    {
        return None;
    }
    let callee = tokens[body].text;
    let args_open = body + 1;
    let args_close = matching_close.get(args_open).copied().flatten()?;
    let mut arg_at = args_open + 1;
    for (index, param) in params.iter().enumerate() {
        if tokens.get(arg_at).map(|token| token.text) != Some(*param) {
            return None;
        }
        arg_at += 1;
        if index + 1 < params.len() {
            if tokens.get(arg_at).map(|token| token.text) != Some(",") {
                return None;
            }
            arg_at += 1;
        }
    }
    if arg_at != args_close {
        return None;
    }
    let mut after = args_close + 1;
    if tokens.get(after).map(|token| token.text) == Some(";") {
        after += 1;
    }
    if after != body_end {
        return None;
    }
    Some(callee)
}

#[cfg(test)]
mod tests {
    use super::inline_single_use_functions;

    fn run(source: &str) -> String {
        let output = std::process::Command::new("node")
            .arg("-e")
            .arg(source)
            .output()
            .expect("node must execute generated JavaScript");
        assert!(
            output.status.success(),
            "node failed:\n{}\nsource:\n{source}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).expect("node stdout must be UTF-8")
    }

    fn same_behavior(source: &str, harness: &str) {
        let (folded, _) = inline_single_use_functions(source).unwrap();
        assert_eq!(
            run(&format!("{source}\n{harness}")),
            run(&format!("{folded}\n{harness}")),
            "diverged\n{folded}"
        );
    }

    #[test]
    fn moves_a_lone_arrow_into_its_only_reference() {
        let (out, count) =
            inline_single_use_functions("function f(){var Y=(a,b)=>a+b;return g(Y)}").unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "function f(){return g((a,b)=>a+b)}");
    }

    #[test]
    fn takes_one_declarator_out_of_a_list() {
        let (out, _) =
            inline_single_use_functions("function f(){var p=1,Y=(a)=>a,q=2;return g(Y,p,q)}")
                .unwrap();
        assert_eq!(
            out, "function f(){var p=1,q=2;return g((a)=>a,p,q)}",
            "{out}"
        );
    }

    #[test]
    fn takes_the_first_declarator_out_of_a_list() {
        let (out, _) =
            inline_single_use_functions("function f(){var Y=(a)=>a,q=2;return g(Y,q)}").unwrap();
        assert_eq!(out, "function f(){var q=2;return g((a)=>a,q)}", "{out}");
    }

    #[test]
    fn a_function_read_twice_keeps_its_name() {
        let source = "function f(){var Y=(a)=>a;return g(Y,Y)}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    /// Closure identity is observable. Moving the creation inside a loop would
    /// build a new function per iteration, and a program that compares them --
    /// or removes a listener by reference -- would see the difference.
    #[test]
    fn a_read_inside_a_loop_keeps_its_name() {
        let source = "function f(){var Y=(a)=>a,seen=[];for(var i=0;i<2;i++)seen.push(Y);return seen[0]===seen[1]}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "a loop must not multiply the closure: {out}");
        assert_eq!(out, source);
        same_behavior(source, "console.log(f())");
    }

    /// Same hazard through a nested function: one closure today, one per call
    /// after the move.
    #[test]
    fn a_read_inside_a_nested_function_keeps_its_name() {
        let source = "function f(){var Y=(a)=>a;return ()=>Y}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
        same_behavior(source, "var h=f();console.log(h()===h())");
    }

    #[test]
    fn a_reassigned_binding_keeps_its_name() {
        let source = "function f(c){var Y=(a)=>a;if(c)Y=(b)=>b*2;return g(Y)}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn a_recursive_helper_keeps_its_name() {
        let source = "function f(){var Y=(a)=>a<1?1:Y(a-1);return g(Y)}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    /// A shadowing binding means the outer name is not what the inner site
    /// reads, and the resolution is what decides that -- not the spelling.
    #[test]
    fn a_shadowed_name_is_not_mistaken_for_this_one() {
        same_behavior(
            "function f(){var Y=(a)=>a+1;function inner(Y){return Y*2}return inner(3)+Y(1)}",
            "console.log(f())",
        );
    }

    #[test]
    fn an_initializer_that_is_not_a_bare_function_is_left_alone() {
        let source = "function f(){var Y=cond?(a)=>a:null;return g(Y)}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    /// An arrow body swallows whatever follows it, so a site that is not
    /// already delimited is refused rather than silently reparsed.
    #[test]
    fn an_undelimited_site_is_refused() {
        let source = "function f(){var Y=(a)=>a;return Y||q}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "an arrow would swallow `||q`: {out}");
        assert_eq!(out, source);
    }

    #[test]
    fn a_function_expression_moves_too() {
        let (out, count) =
            inline_single_use_functions("function f(){var Y=function(a){return a};return g(Y)}")
                .unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "function f(){return g(function(a){return a})}");
    }

    #[test]
    fn object_literal_members_are_the_common_shape() {
        same_behavior(
            "function f(){var Y=(a,b)=>a+b,Z=(a)=>a*2;return {add:Y,twice:Z}}",
            "var o=f();console.log(o.add(1,2),o.twice(4))",
        );
    }

    #[test]
    fn a_destructuring_declaration_is_left_alone() {
        let source = "function f(o){var {a,b}=o,Y=(x)=>x;return g(Y,a,b)}";
        let (out, count) = inline_single_use_functions(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn forwarding_call_wrappers_become_direct_calls() {
        use super::fold_forwarding_call_wrappers;
        let source = "function ie(e){throw e}function l(e){ie(e)}function N(e,a){ie(e,a)}function f(){l(30);N(31,1)}";
        let (out, count) = fold_forwarding_call_wrappers(source).unwrap();
        assert!(count >= 2, "{out}");
        assert!(out.contains("ie(30)"), "{out}");
        assert!(out.contains("ie(31,1)"), "{out}");
        assert!(!out.contains("function l("), "{out}");
        assert!(!out.contains("function N("), "{out}");
        assert!(out.contains("function ie("), "{out}");
    }

    #[test]
    fn forwarding_call_wrappers_keep_non_call_uses() {
        use super::fold_forwarding_call_wrappers;
        let source = "function ie(e){throw e}function l(e){ie(e)}export{l as die}";
        let (out, count) = fold_forwarding_call_wrappers(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn forwarding_call_wrappers_skip_member_callees() {
        use super::fold_forwarding_call_wrappers;
        let source = "function S(e,a,t){f.defineProperty(e,a,t)}S(x,y,z)";
        let (out, count) = fold_forwarding_call_wrappers(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn forwarding_arrow_wrappers_become_direct_calls() {
        use super::fold_forwarding_call_wrappers;
        let source = "fa=function(e){throw e};p=a=>{fa(a)},H=(a,b)=>{fa(a,b)};function f(){p(30);H(31,1)}";
        let (out, count) = fold_forwarding_call_wrappers(source).unwrap();
        assert!(count >= 2, "{out}");
        assert!(out.contains("fa(30)"), "{out}");
        assert!(out.contains("fa(31,1)"), "{out}");
        assert!(!out.contains("p=a=>"), "{out}");
        assert!(!out.contains("H=(a,b)=>"), "{out}");
        assert!(out.contains("fa=function"), "{out}");
    }

    #[test]
    fn forwarding_bare_param_arrow_wrappers_become_direct_calls() {
        use super::fold_forwarding_call_wrappers;
        let source = "let da=a=>a;p=a=>{fa(a)},gb=a=>a;function f(){p(35)}";
        let (out, count) = fold_forwarding_call_wrappers(source).unwrap();
        assert!(count >= 1, "{out}");
        assert!(out.contains("fa(35)"), "{out}");
        assert!(!out.contains("p=a=>"), "{out}");
        assert!(out.contains("gb=a=>a"), "{out}");
    }
}
