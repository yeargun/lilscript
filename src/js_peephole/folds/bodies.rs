//! Spell a body as an expression when it only ever was one.
//!
//! An arrow whose block is a run of expression statements ending in a return
//! carries three tokens of pure syntax: the two braces and the `return`. The
//! same body as a sequence -- `(a(),b(),c)` -- costs one pair of parentheses
//! and says the same thing, so the swap is six bytes and, more usefully, turns
//! several differently-shaped bodies into one shape the compressor has seen.
//!
//! The same holds for a statement body under `if`, `else`, `for` or `while`:
//! braces around statements that are all expressions are two bytes spent to
//! separate what a comma already separates.
//!
//! Neither rewrite may change what runs or what a body evaluates to:
//!
//! * A block containing a declaration keeps its braces. `var` would escape to
//!   the enclosing function and `let` has a scope that the sequence does not.
//! * A block containing control flow keeps its braces: `break`, `continue`,
//!   `return` and the rest are statements with no expression spelling.
//! * An arrow body without a trailing `return` keeps its braces. `()=>{f()}`
//!   evaluates to `undefined` and `()=>f()` does not, and nothing here can see
//!   whether the caller reads the result.

use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{lex, matching_closers, Token};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn fold_expression_bodies(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut rewrites = Vec::<(usize, usize, String)>::new();
    let mut count = 0usize;

    for index in 0..tokens.len() {
        if tokens[index].text != "{" {
            continue;
        }
        let Some(close) = matching_close[index] else {
            continue;
        };
        let Some(previous) = index.checked_sub(1) else {
            continue;
        };
        let Some(statements) = expression_statements(&tokens, &matching_close, index, close) else {
            continue;
        };
        if statements.is_empty() {
            continue;
        }

        if tokens[previous].text == "=>" {
            let Some(body) = arrow_body(source, &tokens, &statements) else {
                continue;
            };
            rewrites.push((tokens[index].start, tokens[close].end, body));
            count += 1;
        } else if opens_a_statement_body(&tokens, &matching_close, previous) {
            // `return` has no expression spelling here, so a body that ends in
            // one is left alone; the arrow case above is the only place a
            // return becomes a value.
            if statements.iter().any(|(start, _)| tokens[*start].text == "return") {
                continue;
            }
            let joined = statements
                .iter()
                .map(|(start, end)| &source[tokens[*start].start..tokens[*end - 1].end])
                .collect::<Vec<_>>()
                .join(",");
            // `else` is a word: dropping the brace after it would weld the
            // keyword to whatever the body starts with.
            let separator = if needs_separator(tokens[previous].text, &joined) { " " } else { "" };
            rewrites.push((
                tokens[index].start,
                tokens[close].end,
                format!("{separator}{joined};"),
            ));
            count += 1;
        }
    }

    // Braces nest, and rewriting an outer body would move the text an inner
    // rewrite is addressed to. Keep only the outermost of any overlapping pair.
    rewrites.sort_unstable_by_key(|(start, end, _)| (*start, std::cmp::Reverse(*end)));
    let mut kept = Vec::<(usize, usize, String)>::new();
    for rewrite in rewrites {
        if kept.last().is_some_and(|(_, end, _)| rewrite.0 < *end) {
            count -= 1;
            continue;
        }
        kept.push(rewrite);
    }
    Ok((apply_token_rewrites(source, kept).0, count))
}

/// The statements of a block, as half-open token ranges, when every one of them
/// is an expression statement -- optionally with a `return` last, which only the
/// arrow case accepts.
fn expression_statements(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    open: usize,
    close: usize,
) -> Option<Vec<(usize, usize)>> {
    let mut statements = Vec::new();
    let mut cursor = open + 1;
    while cursor < close {
        if tokens[cursor].text == ";" {
            // An empty statement carries nothing; drop it.
            cursor += 1;
            continue;
        }
        let start = cursor;
        let leads_a_statement = matches!(
            tokens[start].text,
            "var" | "let" | "const" | "if" | "for" | "while" | "do" | "switch" | "try" | "throw"
                | "break" | "continue" | "function" | "class" | "debugger" | "with" | "{"
        );
        if leads_a_statement {
            return None;
        }
        // A `return` may only be last, and only an arrow can use it.
        let returning = tokens[start].text == "return";
        let mut end = start;
        while end < close {
            match tokens[end].text {
                "(" | "[" | "{" => end = matching_close[end]? + 1,
                ";" => break,
                _ => end += 1,
            }
        }
        if returning {
            // `return;` has no value to become an expression.
            if end == start + 1 {
                return None;
            }
            // Anything after a return is unreachable; leave the block alone
            // rather than reason about it.
            let mut after = end;
            while after < close && tokens[after].text == ";" {
                after += 1;
            }
            if after != close {
                return None;
            }
        }
        statements.push((start, end));
        cursor = if end < close && tokens[end].text == ";" { end + 1 } else { end };
    }
    Some(statements)
}

/// `(E1,…,En,R)` for an arrow whose block ends in `return R`.
fn arrow_body(
    source: &str,
    tokens: &[Token<'_>],
    statements: &[(usize, usize)],
) -> Option<String> {
    let (last_start, last_end) = *statements.last()?;
    if tokens[last_start].text != "return" {
        return None;
    }
    let value = &source[tokens[last_start + 1].start..tokens[last_end - 1].end];
    let mut parts = statements[..statements.len() - 1]
        .iter()
        .map(|(start, end)| &source[tokens[*start].start..tokens[*end - 1].end])
        .collect::<Vec<_>>();
    parts.push(value);
    Some(format!("({})", parts.join(",")))
}

/// Two tokens that would read as one need a space between them.
fn needs_separator(before: &str, after: &str) -> bool {
    let tail = before.chars().next_back();
    let head = after.chars().next();
    match (tail, head) {
        (Some(tail), Some(head)) => {
            let wordish = |c: char| c.is_alphanumeric() || c == '_' || c == '$';
            wordish(tail) && wordish(head)
        }
        _ => false,
    }
}

/// True when the token before a `{` makes that brace a statement body rather
/// than an object literal, a class body or a function body.
fn opens_a_statement_body(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    previous: usize,
) -> bool {
    if tokens[previous].text == "else" {
        return true;
    }
    if tokens[previous].text != ")" {
        return false;
    }
    // Walk back over the head to the keyword that owns it. A `)` belonging to a
    // call or a parameter list is not a statement head.
    let Some(open) = matching_close
        .iter()
        .position(|close| *close == Some(previous))
    else {
        return false;
    };
    open.checked_sub(1)
        .is_some_and(|keyword| matches!(tokens[keyword].text, "if" | "for" | "while"))
}

#[cfg(test)]
mod tests {
    use super::fold_expression_bodies;

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
        let (folded, _) = fold_expression_bodies(source).unwrap();
        assert_eq!(
            run(&format!("{source}\n{harness}")),
            run(&format!("{folded}\n{harness}")),
            "diverged\n{folded}"
        );
    }

    #[test]
    fn an_arrow_body_becomes_a_sequence() {
        let (out, count) = fold_expression_bodies("var f=()=>{q();return !(null==D)&&D}").unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "var f=()=>(q(),!(null==D)&&D)");
    }

    #[test]
    fn a_single_return_becomes_the_value() {
        let (out, _) = fold_expression_bodies("var f=(a)=>{return a+1}").unwrap();
        assert_eq!(out, "var f=(a)=>(a+1)", "{out}");
    }

    /// `()=>{f()}` evaluates to undefined and `()=>f()` does not.
    #[test]
    fn an_arrow_without_a_return_keeps_its_braces() {
        let source = "var f=()=>{q()}";
        let (out, count) = fold_expression_bodies(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn a_declaration_keeps_its_braces() {
        let source = "var f=()=>{var a=1;return a}";
        let (out, count) = fold_expression_bodies(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn control_flow_keeps_its_braces() {
        let source = "var f=(a)=>{if(a)g();return a}";
        let (out, count) = fold_expression_bodies(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn a_loop_body_of_expressions_drops_its_braces() {
        let (out, count) = fold_expression_bodies("for(var i in a){b=a[i];c.push(b)}").unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "for(var i in a)b=a[i],c.push(b);");
    }

    #[test]
    fn an_if_body_of_expressions_drops_its_braces() {
        let (out, _) = fold_expression_bodies("if(a){b=1;c=2}else{d=3}").unwrap();
        assert_eq!(out, "if(a)b=1,c=2;else d=3;", "{out}");
    }

    /// A `return` inside a statement body has no expression spelling, so those
    /// braces stay even though every other statement is an expression.
    #[test]
    fn a_statement_body_that_returns_keeps_its_braces() {
        let source = "function f(a){if(a){g();return 1}return 2}";
        let (out, count) = fold_expression_bodies(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn an_object_literal_is_not_a_body() {
        let source = "var o={a:1,b:2}";
        let (out, count) = fold_expression_bodies(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn a_function_body_is_not_an_arrow_body() {
        let source = "var f=function(){q();return 1}";
        let (out, count) = fold_expression_bodies(source).unwrap();
        assert_eq!(count, 0, "{out}");
        assert_eq!(out, source);
    }

    #[test]
    fn behavior_survives_a_nest_of_bodies() {
        same_behavior(
            "var t=[];var f=(a)=>{t.push(a);return a*2};for(var i=0;i<3;i++){t.push(i);f(i)}",
            "console.log(t.join(','),f(5))",
        );
    }

    #[test]
    fn a_dangling_else_is_not_created() {
        same_behavior(
            "function f(a,b){var r='';if(a){if(b)r='ab'}else{r='x'}return r}",
            "console.log(f(1,0)+'|'+f(0,0)+'|'+f(1,1))",
        );
    }

    #[test]
    fn sequence_order_is_preserved() {
        same_behavior(
            "var log=[];var f=()=>{log.push(1);log.push(2);return log.length};",
            "console.log(f(),log.join(''))",
        );
    }
}


