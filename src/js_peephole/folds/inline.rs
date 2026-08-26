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
use crate::js_peephole::rewrite::apply_token_rewrites;
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
}
