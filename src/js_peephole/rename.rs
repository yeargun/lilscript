//! Converge local names so the same shape spells the same way everywhere.
//!
//! An LZ match costs about the same however long its text is, so one spelling
//! repeated many times is cheaper than several shorter spellings that each
//! appear once. Names assigned in the IR backend cannot converge: they are
//! chosen per function before the nesting layout is known, so each function
//! draws from a differently-shaped pool and the same two-parameter arrow is
//! emitted `(a,b)` here and `(c,d)` there. Measured on jQuery: 217
//! multi-parameter arrow headers across 69 distinct spellings.
//!
//! This pass runs on the final laid-out text, where [`BindingResolution`] knows
//! exactly which declaration every identifier refers to. Each function scope
//! then reassigns its own bindings from one canonical sequence -- parameters by
//! position, then the rest by descending use -- so headers converge on `(a,b)`,
//! `(a,b,c)` and bodies on the same short letters.
//!
//! Every rename must satisfy three conditions, all of them decided from the
//! resolution rather than from neighbouring text:
//!
//! 1. The old name is declared exactly once in the scope, so the uses to
//!    rewrite are precisely the tokens resolving to that declaration.
//! 2. The new name appears nowhere inside the scope's extent. A name absent
//!    from the whole extent cannot capture an inner reference, and no nested
//!    scope can shadow it out from under a use we are about to rewrite.
//! 3. No enclosing scope has already claimed the new name for a binding that
//!    reaches into this one.

use crate::js_peephole::binding::{BindingResolution, Resolution};
use crate::js_peephole::rewrite::{apply_token_rewrites, is_property_identifier};
use crate::js_peephole::token::{lex, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;
use std::collections::{HashMap, HashSet};

pub(crate) fn converge_local_names(source: &str) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    // Converging on a spelling the artifact does not already use trades one
    // kind of repetition for another and loses. Measured on jQuery: renaming to
    // `a,b,c` while the file's own identifiers are `e,t,n,r` converges headers
    // from 68 spellings to 50 and costs 350 Brotli bytes. The sequence has to
    // start from the characters this artifact already spends most of its
    // identifier bytes on.
    let alphabet = dominant_identifier_alphabet(&tokens);
    let resolution = BindingResolution::new(&tokens);

    let mut uses = HashMap::<usize, Vec<usize>>::new();
    for index in 0..tokens.len() {
        if let Resolution::Bound(declaration) = resolution.resolve(index) {
            uses.entry(declaration).or_default().push(index);
        }
    }

    // A scope may reuse a name an enclosing scope also uses, as long as nothing
    // inside the scope refers to that enclosing binding: binding the name again
    // shadows it, and shadowing is only a bug when something wanted the outer
    // one. The earlier rule here -- refuse any name that appears anywhere in the
    // scope's extent -- is a sufficient condition, not the real one, and it is
    // what kept jQuery at 68 header spellings where terser reaches 26. A nested
    // function that binds `e` for itself blocked `e` for its parent.
    //
    // So the question asked per scope is terser's: which names are spoken here
    // that resolve somewhere else? Those, and only those, are unavailable.
    let mut scopes = resolution.function_scopes();
    // Parents first, so an inner scope sees its enclosing bindings' final names.
    scopes.sort_unstable_by_key(|(_, start, end)| (*start, std::cmp::Reverse(*end)));

    let mut assigned = HashMap::<usize, String>::new();
    let mut rewrites = Vec::<(usize, usize, String)>::new();

    for (scope, start, end) in scopes {
        let declarations = resolution.declarations(scope);
        if declarations.is_empty() {
            continue;
        }
        let body = tokens
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, token)| token.text == "{" || token.text == "=>")
            .map_or(start, |(index, _)| index);

        // Names this scope may not take.
        let mut blocked = HashSet::<String>::new();
        for index in start..end.min(tokens.len()) {
            if tokens[index].kind != TokenKind::Identifier || is_property_identifier(&tokens, index)
            {
                continue;
            }
            match resolution.resolve(index) {
                // A global or an unresolved spelling read here would be captured
                // by a local of the same name.
                Resolution::Free | Resolution::Unresolved => {
                    blocked.insert(tokens[index].text.to_string());
                }
                Resolution::Bound(declaration) => {
                    if declaration < start || declaration >= end {
                        blocked.insert(
                            assigned
                                .get(&declaration)
                                .cloned()
                                .unwrap_or_else(|| tokens[declaration].text.to_string()),
                        );
                    }
                }
            }
        }

        // `(parameter position, -uses, declaration)`. Position leads because
        // header shape is what repeats: every first parameter is assigned before
        // any second parameter, so same-arity functions converge on one spelling.
        let mut renameable = Vec::<(usize, usize, usize)>::new();
        let mut position = 0usize;
        for (name, declaration) in declarations {
            let rank = if declaration < body {
                let rank = position;
                position += 1;
                rank
            } else {
                usize::MAX
            };
            let keeps_its_name = resolution.scope_index_at(declaration) == 0
                || !resolution.name_is_unambiguous(scope, name)
                || names_a_function_or_class(&tokens, declaration);
            if keeps_its_name {
                blocked.insert(name.to_string());
                assigned.insert(declaration, name.to_string());
            } else {
                let count = uses.get(&declaration).map_or(0, Vec::len);
                renameable.push((rank, usize::MAX - count, declaration));
            }
        }
        renameable.sort_unstable();

        let mut canonical = CanonicalNames::new(&alphabet);
        for (_, _, declaration) in renameable {
            let name = tokens[declaration].text;
            let replacement = loop {
                let candidate = canonical.next_name();
                if candidate.len() > 2 {
                    break None;
                }
                if is_reserved_word(&candidate) || blocked.contains(&candidate) {
                    continue;
                }
                break Some(candidate);
            };
            let Some(replacement) = replacement else {
                blocked.insert(name.to_string());
                assigned.insert(declaration, name.to_string());
                continue;
            };
            blocked.insert(replacement.clone());
            assigned.insert(declaration, replacement.clone());
            if replacement == name {
                continue;
            }
            for site in uses.get(&declaration).into_iter().flatten() {
                rewrites.push((tokens[*site].start, tokens[*site].end, replacement.clone()));
            }
            rewrites.push((
                tokens[declaration].start,
                tokens[declaration].end,
                replacement,
            ));
        }
    }

    rewrites.sort_unstable_by_key(|(start, _, _)| *start);
    rewrites.dedup_by_key(|(start, _, _)| *start);
    Ok(apply_token_rewrites(source, rewrites))
}

/// Parameters first in declaration order, then the remaining bindings by

/// The canonical spelling sequence: `a`..`z`, `A`..`Z`, `_`, `$`, then the
/// two-character combinations. Kept here rather than borrowed from the IR
/// backend's mangler so this pass depends only on the token stream.
struct CanonicalNames<'a> {
    next: usize,
    alphabet: &'a [u8],
}

const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";

/// The mangling alphabet ordered by how much of this artifact's identifier text
/// each character already carries, so a converged name reuses a byte the codec
/// has seen rather than introducing one it has not.
fn dominant_identifier_alphabet(tokens: &[Token<'_>]) -> Vec<u8> {
    let mut weight = [0usize; 256];
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Identifier || is_property_identifier(tokens, index) {
            continue;
        }
        for byte in token.text.bytes() {
            weight[byte as usize] += 1;
        }
    }
    let mut alphabet = ALPHABET.to_vec();
    alphabet.sort_by(|left, right| {
        weight[*right as usize]
            .cmp(&weight[*left as usize])
            .then_with(|| left.cmp(right))
    });
    alphabet
}

impl<'a> CanonicalNames<'a> {
    fn new(alphabet: &'a [u8]) -> Self {
        Self { next: 0, alphabet }
    }

    fn next_name(&mut self) -> String {
        let index = self.next;
        self.next += 1;
        let width = self.alphabet.len();
        if index < width {
            return String::from(self.alphabet[index] as char);
        }
        let index = index - width;
        let first = self.alphabet[index / width] as char;
        let second = self.alphabet[index % width] as char;
        let mut name = String::with_capacity(2);
        name.push(first);
        name.push(second);
        name
    }
}

/// A reordered alphabet can spell a keyword in two characters, which a fixed
/// `ab`, `ac` sequence never reaches. `do`, `if` and `in` are the only reserved
/// words that short; `of` and `as` are contextual and legal as identifiers.
fn is_reserved_word(name: &str) -> bool {
    matches!(name, "do" | "if" | "in")
}

/// `.name` on a function or class is observable, so those bindings keep their
/// spelling however hot they are.
fn names_a_function_or_class(tokens: &[Token<'_>], declaration: usize) -> bool {
    let mut cursor = declaration;
    if cursor > 0 && tokens[cursor - 1].text == "*" {
        cursor -= 1;
    }
    cursor
        .checked_sub(1)
        .is_some_and(|previous| matches!(tokens[previous].text, "function" | "class"))
}

/// A name that is already the shortest it can be still has to be reserved, so
/// a later binding in the same scope does not take it. Used by the tests to
/// document that a no-op rename is not a skipped one.
#[allow(dead_code)]
pub(crate) fn identifier_is_renameable(tokens: &[Token<'_>], at: usize) -> bool {
    tokens[at].kind == TokenKind::Identifier && !is_property_identifier(tokens, at)
}

#[cfg(test)]
mod tests {
    use super::converge_local_names;

    /// Run the renamed program and the original under Node and require the same
    /// observable result. Convergence is worthless if it changes behavior.
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
        let (renamed, _) = converge_local_names(source).unwrap();
        let before = run(&format!("{source}\n{harness}"));
        let after = run(&format!("{renamed}\n{harness}"));
        assert_eq!(before, after, "renamed program diverged\n{renamed}");
    }

    #[test]
    fn sibling_functions_converge_on_one_header_spelling() {
        let source = "function q(elem,key){return elem+key}function r(key,elem){return key-elem}";
        let (out, count) = converge_local_names(source).unwrap();
        assert!(count > 0, "{out}");
        // The letters come from the artifact's own identifier text -- here
        // `elem` and `key` -- so a converged name reuses a byte the codec has
        // already seen. What matters is that both headers agree.
        assert_eq!(
            out, "function q(e,k){return e+k}function r(e,k){return e-k}",
            "sibling headers must converge"
        );
    }

    #[test]
    fn arrows_converge_too() {
        let source = "var h=(elem,key)=>elem[key];var g=(key,elem)=>key[elem];";
        let (out, _) = converge_local_names(source).unwrap();
        assert_eq!(out, "var h=(e,k)=>e[k];var g=(e,k)=>e[k];", "{out}");
    }

    #[test]
    fn a_shadowing_inner_binding_keeps_its_own_identity() {
        same_behavior(
            "function q(a){var b=a;function inner(b){return b*2}return inner(b)+b}",
            "console.log(q(3))",
        );
    }

    #[test]
    fn a_named_iife_does_not_steal_later_reads_of_an_outer_binding() {
        same_behavior(
            concat!(
                "function factory(){var fire=()=>7;",
                "return function(){",
                "(function fire(n){if(n)fire(n-1)})(1);",
                "return fire()}}",
            ),
            "console.log(factory()())",
        );
    }

    #[test]
    fn a_name_present_in_the_scope_is_never_claimed() {
        // `a` is a free reference to a global inside q, so q's parameter cannot
        // become `a` -- that would capture the global.
        let source = "function q(zz){return a+zz}";
        let (out, _) = converge_local_names(source).unwrap();
        assert!(out.contains("a+"), "the free `a` must survive: {out}");
        assert!(!out.contains("function q(a)"), "captured a global: {out}");
    }

    #[test]
    fn closures_over_outer_names_keep_resolving_to_them() {
        same_behavior(
            "function outer(first,second){var total=first+second;return function(extra){return total+extra}}",
            "console.log(outer(1,2)(4))",
        );
    }

    #[test]
    fn catch_parameters_do_not_collide_with_locals() {
        same_behavior(
            "function q(input){var out=0;try{out=input.x.y}catch(err){out=String(err).length>0?-1:0}return out}",
            "console.log(q({}))",
        );
    }

    /// The rule this pass turns on: a scope may take a name an inner scope also
    /// binds, because binding it again shadows it. The old test -- refuse any
    /// name that appears in the extent -- forbade this and cost the convergence.
    #[test]
    fn a_parent_may_reuse_a_name_an_inner_scope_binds_for_itself() {
        same_behavior(
            "function outer(alpha){function inner(beta){return beta*2}return inner(3)+alpha}",
            "console.log(outer(5))",
        );
        let (out, _) = converge_local_names(
            "function outer(alpha){function inner(beta){return beta*2}return inner(3)+alpha}",
        )
        .unwrap();
        // Whichever letter the input's own text makes first, both scopes take
        // it: that is the point.
        let outer = out
            .split("function outer(")
            .nth(1)
            .and_then(|rest| rest.split(')').next());
        let inner = out
            .split("function inner(")
            .nth(1)
            .and_then(|rest| rest.split(')').next());
        assert_eq!(outer, inner, "parent and child may share a name: {out}");
        assert!(outer.is_some_and(|name| name.len() == 1), "{out}");
    }

    /// And may not when the inner scope actually reads the outer binding.
    #[test]
    fn a_parent_name_read_inside_is_still_refused() {
        same_behavior(
            "function outer(alpha){function inner(beta){return beta+alpha}return inner(3)}",
            "console.log(outer(5))",
        );
        let (out, _) = converge_local_names(
            "function outer(alpha){function inner(beta){return beta+alpha}return inner(3)}",
        )
        .unwrap();
        assert!(
            !out.contains("function inner(e)") || !out.contains("function outer(e)"),
            "a read of the outer binding must keep the names apart: {out}"
        );
    }

    /// A reordered alphabet can spell `if`, `in` or `do`.
    #[test]
    fn a_two_character_keyword_is_never_assigned() {
        let mut source = String::from("function f(");
        for index in 0..70 {
            if index > 0 {
                source.push(',');
            }
            source.push_str(&format!("p{index}"));
        }
        source.push_str("){return ");
        for index in 0..70 {
            if index > 0 {
                source.push('+');
            }
            source.push_str(&format!("p{index}"));
        }
        source.push_str("}");
        let (out, _) = converge_local_names(&source).unwrap();
        for word in ["if", "in", "do"] {
            assert!(
                !out.contains(&format!("({word},"))
                    && !out.contains(&format!(",{word},"))
                    && !out.contains(&format!(",{word})")),
                "assigned the reserved word `{word}`: {out}"
            );
        }
    }

    #[test]
    fn destructured_parameters_are_left_alone() {
        let source = "function q({a,b}){return a+b}";
        let (out, count) = converge_local_names(source).unwrap();
        assert_eq!(count, 0, "unresolved scopes must not be rewritten: {out}");
        assert_eq!(out, source);
    }

    #[test]
    fn property_names_are_never_renamed() {
        let source = "function q(value){return value.value+value.key}";
        let (out, _) = converge_local_names(source).unwrap();
        assert!(out.contains(".value"), "{out}");
        assert!(out.contains(".key"), "{out}");
    }

    #[test]
    fn nested_scopes_do_not_reuse_an_ancestor_claim_that_reaches_them() {
        same_behavior(
            "function outer(alpha){var beta=alpha*2;return function inner(gamma){return alpha+beta+gamma}}",
            "console.log(outer(5)(1))",
        );
    }

    #[test]
    fn default_initializers_still_see_earlier_parameters() {
        same_behavior(
            "function q(first,second=first+1){return first*second}",
            "console.log(q(3))",
        );
    }

    #[test]
    fn a_class_method_body_converges_without_touching_members() {
        same_behavior(
            "class K{constructor(seed){this.seed=seed}scale(factor){return this.seed*factor}}",
            "console.log(new K(4).scale(3))",
        );
    }
}
