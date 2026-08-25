//! Return a value instead of storing it first.
//!
//! `x = expr; return x` is what falls out of lowering a value that the IR
//! materialized into a named slot: the store is real in SSA, and by the time
//! the emitter has picked a JavaScript name the slot looks like a variable that
//! must be written before it is read. It is not -- when the only reads of that
//! binding are the store itself and the return, the store is a temporary whose
//! whole life is the next statement.
//!
//! The check is a resolution, not a pattern. Every occurrence of the name is
//! mapped to its declaration by [`BindingResolution`], and the fold applies
//! only when that declaration's complete use set is the declaration, the store
//! and the return. A name that escapes into a closure, is read earlier, or is
//! reassigned anywhere else fails that test and keeps its variable.

use crate::js_peephole::binding::{BindingResolution, Resolution};
use crate::js_peephole::rewrite::apply_token_rewrites;
use crate::js_peephole::token::{lex, matching_closers, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;

pub(crate) fn fold_returned_temporaries(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let resolution = BindingResolution::new(&tokens);

    let mut uses = std::collections::HashMap::<usize, Vec<usize>>::new();
    for index in 0..tokens.len() {
        if let Resolution::Bound(declaration) = resolution.resolve(index) {
            uses.entry(declaration).or_default().push(index);
        }
    }

    let mut replacements = Vec::<(usize, usize, String)>::new();
    for index in 0..tokens.len() {
        if tokens[index].text != "return"
            || tokens
                .get(index + 1)
                .is_none_or(|token| token.kind != TokenKind::Identifier)
            || !matches!(
                tokens.get(index + 2).map(|token| token.text),
                Some(";") | Some("}")
            )
        {
            continue;
        }
        let Resolution::Bound(declaration) = resolution.resolve(index + 1) else {
            continue;
        };
        let Some(sites) = uses.get(&declaration) else {
            continue;
        };
        let Some(statement) = preceding_statement(&tokens, &matching_close, index) else {
            continue;
        };
        let Some(store) = store_shape(&tokens, statement, index) else {
            continue;
        };
        // The store must target the very binding being returned. Checking the
        // spelling is not enough: `f(y){let x=…;y=2;return x}` has a store and
        // a return whose names merely look adjacent.
        if resolution.resolve(store.name) != Resolution::Bound(declaration) {
            continue;
        }
        // Earlier reads and writes are fine: after the store, the only read of
        // this binding is the return being folded, so nothing can observe the
        // store's effect on the variable. Two things must hold. Nothing may
        // read it between the store and the return, and no nested function may
        // close over it -- a closure created earlier can run later and would
        // see a variable that never got written.
        let accounted = [declaration, store.name, index + 1];
        let binding_scope = resolution.scope_index_at(declaration);
        let escapes = sites.iter().any(|site| {
            !accounted.contains(site)
                && (*site > store.name || resolution.scope_index_at(*site) != binding_scope)
        });
        if escapes {
            continue;
        }
        // `index - 1` is the statement's own `;`, which is not part of the value.
        let value = &source[tokens[store.value].start..tokens[index - 2].end];
        replacements.push((
            tokens[store.start].start,
            tokens[index + 1].end,
            format!("return {value}"),
        ));
    }
    Ok(apply_token_rewrites(source, replacements))
}

struct Store {
    /// First token of the statement to absorb.
    start: usize,
    /// The assigned name's token.
    name: usize,
    /// First token of the assigned expression.
    value: usize,
}

/// `NAME=…` or `var NAME=…` occupying the whole statement before the return.
fn store_shape(tokens: &[Token<'_>], statement: usize, return_at: usize) -> Option<Store> {
    let mut cursor = statement;
    if matches!(tokens[cursor].text, "var" | "let") {
        cursor += 1;
    }
    if tokens.get(cursor)?.kind != TokenKind::Identifier {
        return None;
    }
    if tokens.get(cursor + 1)?.text != "=" || tokens.get(cursor + 2)?.text == "=" {
        return None;
    }
    // The statement must end right where the return begins, so nothing else
    // runs between the store and the return.
    if return_at == 0 || tokens[return_at - 1].text != ";" {
        return None;
    }
    Some(Store {
        start: statement,
        name: cursor,
        value: cursor + 2,
    })
}

/// First token of the statement that ends immediately before `return`.
fn preceding_statement(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    return_at: usize,
) -> Option<usize> {
    if return_at == 0 || tokens[return_at - 1].text != ";" {
        return None;
    }
    let mut index = return_at - 1;
    while index > 0 {
        index -= 1;
        match tokens[index].text {
            ")" | "]" | "}" => {
                index = matching_close
                    .iter()
                    .position(|close| *close == Some(index))
                    .filter(|open| *open < index)?;
            }
            ";" | "{" => return Some(index + 1),
            _ => {}
        }
        if index == 0 {
            return Some(0);
        }
    }
    Some(0)
}

#[cfg(test)]
mod tests {
    use super::fold_returned_temporaries;

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
        let (folded, _) = fold_returned_temporaries(source).unwrap();
        assert_eq!(
            run(&format!("{source}\n{harness}")),
            run(&format!("{folded}\n{harness}")),
            "folded program diverged\n{folded}"
        );
    }

    #[test]
    fn returns_a_declared_temporary_directly() {
        let (out, count) =
            fold_returned_temporaries("var g=(e,t)=>{var r=e+t;return r}").unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "var g=(e,t)=>{return e+t}");
    }

    #[test]
    fn returns_a_stored_temporary_directly() {
        let (out, count) =
            fold_returned_temporaries("var g=e=>{var r;r=e?1:2;return r}").unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "var g=e=>{var r;return e?1:2}");
    }

    /// The store may only be absorbed when the binding is spoken nowhere else.
    #[test]
    fn refuses_a_temporary_that_is_read_elsewhere() {
        for source in [
            // read inside the stored expression
            "var g=e=>{var r=1;r=r+e;return r}",
            // captured by a closure
            "var g=e=>{var r=1;e(()=>r);r=2;return r}",
            // returned name is a parameter, not a local temporary
            "var g=e=>{e=e+1;return e}",
            // another statement runs between the store and the return
            "var g=e=>{var r=e+1;e();return r}",
        ] {
            let (out, count) = fold_returned_temporaries(source).unwrap();
            assert_eq!(count, 0, "folded an escaping temporary: {out}");
            assert_eq!(out, source);
        }
    }

    #[test]
    fn preserves_behavior_on_real_shapes() {
        same_behavior(
            "function pick(e,t){var r=e&&e.x||0===t;return r}",
            "console.log(pick({x:5},0),pick(null,1))",
        );
        same_behavior(
            "function build(e){var out;if(e){out=[e,1]}else{out=[0]}return out}",
            "console.log(JSON.stringify(build(7)),JSON.stringify(build(0)))",
        );
        same_behavior(
            "function chain(e){var r=e.map(x=>x*2);return r}",
            "console.log(chain([1,2,3]).join(','))",
        );
    }

    /// The store's name must *resolve* to the returned binding, not merely sit
    /// next to the return. This shape returned `2,mutate()` before the check
    /// was a resolution rather than a pattern.
    #[test]
    fn refuses_a_store_to_a_different_binding() {
        let source = "function f(y){let x={v:read(y)};y=2;return x}";
        let (out, count) = fold_returned_temporaries(source).unwrap();
        assert_eq!(count, 0, "absorbed a store to another binding: {out}");
        assert_eq!(out, source);

        same_behavior(
            "function read(v){return v}function f(y){let x={v:read(y)};y=2;return x}",
            "console.log(JSON.stringify(f(1)))",
        );
    }

    /// A read *before* the store is not an obstacle: after the store nothing
    /// observes the variable except the return being folded.
    #[test]
    fn absorbs_a_store_that_follows_an_earlier_read() {
        let (out, count) =
            fold_returned_temporaries("var g=e=>{var r=1;e(r);r=e+1;return r}").unwrap();
        assert_eq!(count, 1, "{out}");
        assert_eq!(out, "var g=e=>{var r=1;e(r);return e+1}");
        same_behavior(
            "function g(e){var r=1;log(r);r=e+1;return r}",
            "var seen=[];function log(v){seen.push(v)}console.log(g(4),seen.join(','))",
        );
    }

    #[test]
    fn leaves_a_bare_return_alone() {
        for source in ["var g=()=>{return}", "var g=e=>{return e}", "var g=()=>{var r=1;return 2}"] {
            let (out, count) = fold_returned_temporaries(source).unwrap();
            assert_eq!(count, 0, "{out}");
            assert_eq!(out, source);
        }
    }
}

/// Move a temporary into the statement that reads it.
///
/// `var a = e.x; return g(a)` keeps a name alive for one statement. Emitting
/// `return g(e.x)` is shorter and, more to the point, replaces a novel
/// `var a=` with a member read the compressor has probably seen before.
///
/// The hazard is evaluation order: the initializer runs *before* the next
/// statement today, and inlining moves it to wherever the use sits. That is
/// only invisible when nothing between the start of that statement and the use
/// can run — no call, no assignment, no update, no `new`, no `await`. A use
/// buried behind `g(h(), a)` would start observing `h()` first, so it is
/// refused.
///
/// The initializer must also be a pure read. Anything that can throw or mutate
/// keeps its statement, because moving it past even a property access would
/// reorder observable effects.
pub(crate) fn fold_single_use_temporaries(
    source: &str,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let resolution = BindingResolution::new(&tokens);

    let mut uses = std::collections::HashMap::<usize, Vec<usize>>::new();
    for index in 0..tokens.len() {
        if let Resolution::Bound(declaration) = resolution.resolve(index) {
            uses.entry(declaration).or_default().push(index);
        }
    }

    let mut replacements = Vec::<(usize, usize, String)>::new();
    for index in 0..tokens.len() {
        // `var NAME = …;` occupying a whole statement.
        if tokens[index].text != "var" && tokens[index].text != "let" {
            continue;
        }
        if index > 0 && !matches!(tokens[index - 1].text, ";" | "{" | "}") {
            continue;
        }
        let name = index + 1;
        if tokens.get(name).is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name + 1).map(|token| token.text) != Some("=")
        {
            continue;
        }
        let Some(end) = statement_end(&tokens, &matching_close, name + 2) else {
            continue;
        };
        if tokens.get(end).map(|token| token.text) != Some(";") {
            continue;
        }
        if !is_pure_read(&tokens, name + 2, end) {
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
        let reads = sites.iter().filter(|site| **site != declaration).collect::<Vec<_>>();
        if reads.len() != 1 {
            continue;
        }
        let use_at = *reads[0];
        // The read must live in the statement that immediately follows, and in
        // the same scope: a closure would keep the binding alive. That
        // statement may be closed by `}` rather than `;` when it is the last
        // one in a body.
        let next_end = statement_end(&tokens, &matching_close, end + 1)
            .or_else(|| block_end(&tokens, &matching_close, end + 1));
        let Some(next_end) = next_end else {
            continue;
        };
        if use_at <= end || use_at >= next_end {
            continue;
        }
        if resolution.scope_index_at(use_at) != resolution.scope_index_at(declaration) {
            continue;
        }
        if !nothing_runs_before(&tokens, end + 1, use_at) {
            continue;
        }
        let value = &source[tokens[name + 2].start..tokens[end - 1].end];
        // A use already wrapped by its own delimiters needs no extra pair.
        let delimited = use_at > 0
            && tokens[use_at - 1].text == "("
            && tokens.get(use_at + 1).map(|token| token.text) == Some(")");
        let grouped = !delimited && needs_grouping(&tokens, name + 2, end);
        let replacement = if grouped {
            format!("({value})")
        } else {
            value.to_string()
        };
        replacements.push((tokens[index].start, tokens[end].end, String::new()));
        replacements.push((tokens[use_at].start, tokens[use_at].end, replacement));
    }
    Ok(apply_token_rewrites(source, replacements))
}

/// Index of the `;` closing a statement that starts at `from`.
fn statement_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
) -> Option<usize> {
    let mut index = from;
    while index < tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => index = matching_close.get(index).copied().flatten()? + 1,
            ";" => return Some(index),
            "}" => return None,
            _ => index += 1,
        }
    }
    None
}

/// Index of the `}` closing the body a statement starting at `from` sits in,
/// for the last statement of a block, which carries no `;`.
fn block_end(tokens: &[Token<'_>], matching_close: &[Option<usize>], from: usize) -> Option<usize> {
    let mut index = from;
    while index < tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => index = matching_close.get(index).copied().flatten()? + 1,
            ";" => return Some(index),
            "}" => return Some(index),
            _ => index += 1,
        }
    }
    None
}

/// A read that cannot throw, call, or assign: identifiers, literals, member
/// paths and operators only.
fn is_pure_read(tokens: &[Token<'_>], from: usize, to: usize) -> bool {
    if from >= to {
        return false;
    }
    let mut index = from;
    while index < to {
        let token = &tokens[index];
        match token.kind {
            TokenKind::Identifier | TokenKind::Number | TokenKind::String => {}
            TokenKind::Keyword if matches!(token.text, "null" | "true" | "false" | "void") => {}
            TokenKind::Punct
                if matches!(
                    token.text,
                    "." | "+" | "-" | "*" | "&&" | "||" | "??" | "!" | "==" | "!=" | "==="
                        | "!==" | "<" | ">" | "<=" | ">=" | "?" | ":"
                ) => {}
            _ => return false,
        }
        // A call turns a member path into an invocation.
        if token.text == "(" {
            return false;
        }
        index += 1;
    }
    // A bare identifier is a copy; `fold_identifier_copies` owns that shape.
    to > from + 1
}

/// True when nothing between `from` and `use_at` can execute.
///
/// An *open* `(` has run nothing yet -- a call evaluates its arguments before
/// it invokes anything -- so `return g(a)` still reaches `a` first. A closed
/// group is different: by the time `g(h(),a)` reaches `a`, `h()` has already
/// run, so any `)` before the use disqualifies the move, as does an
/// assignment, an update, `new`, `await` or `yield`.
fn nothing_runs_before(tokens: &[Token<'_>], from: usize, use_at: usize) -> bool {
    for index in from..use_at {
        let text = tokens[index].text;
        let assigns = text.ends_with('=')
            && !matches!(text, "==" | "!=" | "===" | "!==" | "<=" | ">=");
        // A member access is itself observable -- `receiver.invoke(v)` runs the
        // `invoke` getter before the argument, so moving a read into `v`'s slot
        // would swap two getters.
        if matches!(text, ")" | "]" | "[" | "." | "?." | "++" | "--" | "new" | "await" | "yield")
            || assigns
        {
            return false;
        }
    }
    true
}

/// The moved value needs parentheses when it binds looser than its new context.
fn needs_grouping(tokens: &[Token<'_>], from: usize, to: usize) -> bool {
    tokens[from..to].iter().any(|token| {
        matches!(
            token.text,
            "?" | ":" | "&&" | "||" | "??" | "+" | "-" | "*" | "==" | "!=" | "===" | "!=="
                | "<" | ">" | "<=" | ">="
        )
    })
}

#[cfg(test)]
mod collapse_tests {
    use super::fold_single_use_temporaries;

    fn run(source: &str) -> String {
        let output = std::process::Command::new("node").arg("-e").arg(source).output().unwrap();
        assert!(output.status.success(), "node failed:\n{}\n{source}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    }

    fn same_behavior(source: &str, harness: &str) {
        let (folded, _) = fold_single_use_temporaries(source).unwrap();
        assert_eq!(
            run(&format!("{source}\n{harness}")),
            run(&format!("{folded}\n{harness}")),
            "diverged\n{folded}"
        );
    }

    #[test]
    fn moves_a_member_read_into_its_only_use() {
        let (out, count) =
            fold_single_use_temporaries("function f(e){var a=e.x;return g(a)}").unwrap();
        assert_eq!(count, 2, "{out}");
        assert_eq!(out, "function f(e){return g(e.x)}");
    }

    /// Evaluation order is the whole hazard: the initializer runs before the
    /// next statement today, so it may only move to a spot nothing precedes.
    #[test]
    fn refuses_when_something_could_run_first() {
        for source in [
            // `h()` would start observing the world before `e.x` is read
            "function f(e){var a=e.x;return g(h(),a)}",
            // a statement in between could change what `e.x` yields
            "function f(e){var a=e.x;h();return g(a)}",
            // an assignment in between
            "function f(e){var a=e.x;e.y=1;return g(a)}",
            // read twice
            "function f(e){var a=e.x;return g(a,a)}",
            // read from a closure that outlives the statement
            "function f(e){var a=e.x;return ()=>a}",
            // the initializer calls, so moving it reorders effects
            "function f(e){var a=e.x();return g(a)}",
            // the callee is itself a member read, and it runs before the
            // argument -- moving the read here would swap two getters
            "function f(){var v=input.value;receiver.invoke(v)}",
        ] {
            let (out, count) = fold_single_use_temporaries(source).unwrap();
            assert_eq!(count, 0, "unsafe collapse: {out}");
            assert_eq!(out, source);
        }
    }

    #[test]
    fn preserves_behavior() {
        same_behavior(
            "function f(e){var a=e.x;return [a,1]}",
            "console.log(JSON.stringify(f({x:9})))",
        );
        same_behavior(
            "function f(e,t){var a=e.x||t;return a?a:0}",
            "console.log(f({x:0},5),f({x:2},5))",
        );
    }
}
