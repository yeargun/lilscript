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
