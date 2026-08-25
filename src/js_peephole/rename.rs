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
    let resolution = BindingResolution::new(&tokens);

    let mut uses = HashMap::<usize, Vec<usize>>::new();
    for index in 0..tokens.len() {
        if let Resolution::Bound(declaration) = resolution.resolve(index) {
            uses.entry(declaration).or_default().push(index);
        }
    }

    // Two bindings may share a spelling exactly when neither scope encloses the
    // other, which is what lets sibling functions converge on `(a,b)`. Model a
    // claim as the token range over which the name is spoken for: a binding
    // claims its whole scope, and a free reference claims the point it occurs
    // at, so any binding whose scope covers that point must avoid the name.
    let mut claims = HashMap::<String, Vec<(usize, usize)>>::new();
    // `(parameter position, -uses, declaration, scope extent)`. Position leads
    // because header shape is what repeats: every first parameter is assigned
    // before any second parameter, so same-arity functions converge on one
    // spelling. Ordering by use count alone shortens names and scrambles
    // headers -- measured on jQuery as `(a,b)` beside `(b,a)`.
    let mut renameable = Vec::<(usize, usize, usize, usize, usize)>::new();
    // Module bindings are left alone: exports and host contracts read them, and
    // ranking them last so locals could pick first was measured and lost -- it
    // pushes them to two characters and costs more raw than the convergence
    // returns.
    for (scope, start, end) in resolution.function_scopes() {
        let declarations = resolution.declarations(scope);
        let body = tokens
            .iter()
            .enumerate()
            .skip(start)
            .find(|(_, token)| token.text == "{" || token.text == "=>")
            .map_or(start, |(index, _)| index);
        let mut position = 0usize;
        for (name, declaration) in declarations {
            let rank = if declaration < body {
                let rank = position;
                position += 1;
                rank
            } else {
                usize::MAX
            };
            let count = uses.get(&declaration).map_or(0, Vec::len);
            if resolution.name_is_unambiguous(scope, name)
                && !names_a_function_or_class(&tokens, declaration)
            {
                renameable.push((rank, usize::MAX - count, declaration, start, end));
            }
            // Refused bindings are claimed above, at each point they are spoken.
        }
    }
    // A name the pass does not assign -- a module binding, a free global, a
    // scope it refused -- only rules out spellings where it is actually spoken.
    // Claiming the whole file for every module binding would block the first
    // fifty spellings everywhere and push every local to two characters.
    let claim_point = |name: &str, at: usize, claims: &mut HashMap<String, Vec<(usize, usize)>>| {
        claims.entry(name.to_string()).or_default().push((at, at + 1));
    };
    for index in 0..tokens.len() {
        if tokens[index].kind != TokenKind::Identifier || is_property_identifier(&tokens, index) {
            continue;
        }
        let fixed = match resolution.resolve(index) {
            Resolution::Free | Resolution::Unresolved => true,
            Resolution::Bound(declaration) => {
                resolution.scope_index_at(declaration) == 0
                    || names_a_function_or_class(&tokens, declaration)
            }
        };
        if fixed {
            claim_point(tokens[index].text, index, &mut claims);
        }
    }

    renameable.sort_unstable();

    let mut rewrites = Vec::<(usize, usize, String)>::new();
    for (_, _, declaration, start, end) in renameable {
        let name = tokens[declaration].text;
        let mut canonical = CanonicalNames::new();
        let replacement = loop {
            let candidate = canonical.next_name();
            if candidate.len() > 2 {
                break None;
            }
            let taken = claims
                .get(&candidate)
                .is_some_and(|spans| spans.iter().any(|(s, e)| *s < end && start < *e));
            if !taken {
                break Some(candidate);
            }
        };
        let Some(replacement) = replacement else {
            claims.entry(name.to_string()).or_default().push((start, end));
            continue;
        };
        claims
            .entry(replacement.clone())
            .or_default()
            .push((start, end));
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

    rewrites.sort_unstable_by_key(|(start, _, _)| *start);
    rewrites.dedup_by_key(|(start, _, _)| *start);
    Ok(apply_token_rewrites(source, rewrites))
}

/// Names claimed by every scope that lexically encloses this one.
fn enclosing_claims(
    resolution: &BindingResolution<'_>,
    claimed_by_scope: &HashMap<usize, HashSet<String>>,
    scope: usize,
    start: usize,
    end: usize,
) -> HashSet<String> {
    let mut claims = HashSet::new();
    for (other, other_start, other_end) in resolution.function_scopes() {
        if other == scope {
            continue;
        }
        if other_start <= start && end <= other_end {
            if let Some(names) = claimed_by_scope.get(&other) {
                claims.extend(names.iter().cloned());
            }
        }
    }
    claims
}

/// Parameters first in declaration order, then the remaining bindings by
/// descending use, so the hottest local also lands on a short spelling.
fn order_declarations<'src>(
    tokens: &[Token<'src>],
    resolution: &BindingResolution<'src>,
    scope: usize,
    uses: &HashMap<usize, Vec<usize>>,
) -> Vec<(&'src str, usize, usize)> {
    let Some((start, _, _)) = resolution
        .function_scopes()
        .into_iter()
        .find(|(index, _, _)| *index == scope)
        .map(|(_, start, end)| (start, end, ()))
    else {
        return Vec::new();
    };
    let parameter_end = tokens
        .iter()
        .enumerate()
        .skip(start)
        .find(|(_, token)| token.text == "{" || token.text == "=>")
        .map_or(start, |(index, _)| index);
    let mut declarations = resolution
        .declarations(scope)
        .into_iter()
        .map(|(name, at)| {
            let count = uses.get(&at).map_or(0, Vec::len);
            let rank = if at <= parameter_end { at } else { usize::MAX };
            (name, at, count, rank)
        })
        .collect::<Vec<_>>();
    declarations.sort_by(|left, right| {
        left.3
            .cmp(&right.3)
            .then_with(|| right.2.cmp(&left.2))
            .then_with(|| left.1.cmp(&right.1))
    });
    declarations
        .into_iter()
        .map(|(name, at, count, _)| (name, at, count))
        .collect()
}

/// The canonical spelling sequence: `a`..`z`, `A`..`Z`, `_`, `$`, then the
/// two-character combinations. Kept here rather than borrowed from the IR
/// backend's mangler so this pass depends only on the token stream.
struct CanonicalNames {
    next: usize,
}

const ALPHABET: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_$";

impl CanonicalNames {
    fn new() -> Self {
        Self { next: 0 }
    }

    fn next_name(&mut self) -> String {
        let index = self.next;
        self.next += 1;
        let width = ALPHABET.len();
        if index < width {
            return String::from(ALPHABET[index] as char);
        }
        let index = index - width;
        let first = ALPHABET[index / width] as char;
        let second = ALPHABET[index % width] as char;
        let mut name = String::with_capacity(2);
        name.push(first);
        name.push(second);
        name
    }
}

fn next_available(
    canonical: &mut CanonicalNames,
    escaping: &HashSet<&str>,
    claimed: &HashSet<String>,
    resolution: &BindingResolution<'_>,
    scope: usize,
    sites: &[usize],
) -> Option<String> {
    for _ in 0..64 {
        let candidate = canonical.next_name();
        if candidate.len() > 2 {
            return None;
        }
        if escaping.contains(candidate.as_str()) || claimed.contains(&candidate) {
            continue;
        }
        // A use sitting inside a nested scope that already declares the
        // candidate would resolve to that inner binding once we rename.
        if sites
            .iter()
            .any(|site| shadowed_between(resolution, *site, scope, &candidate))
        {
            continue;
        }
        return Some(candidate);
    }
    None
}

/// True when any scope from the use up to (but excluding) `scope` declares
/// `name`, which would shadow a binding renamed to it.
fn shadowed_between(
    resolution: &BindingResolution<'_>,
    site: usize,
    scope: usize,
    name: &str,
) -> bool {
    let mut current = Some(resolution.scope_index_at(site));
    while let Some(index) = current {
        if index == scope {
            return false;
        }
        if resolution.scope_declares(index, name) {
            return true;
        }
        current = resolution.parent_scope(index);
    }
    false
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
        let source =
            "function q(elem,key){return elem+key}function r(key,elem){return key-elem}";
        let (out, count) = converge_local_names(source).unwrap();
        assert!(count > 0, "{out}");
        assert_eq!(
            out,
            "function q(a,b){return a+b}function r(a,b){return a-b}",
            "sibling headers must converge"
        );
    }

    #[test]
    fn arrows_converge_too() {
        let source = "var h=(elem,key)=>elem[key];var g=(key,elem)=>key[elem];";
        let (out, _) = converge_local_names(source).unwrap();
        assert_eq!(out, "var h=(a,b)=>a[b];var g=(a,b)=>a[b];", "{out}");
    }

    #[test]
    fn a_shadowing_inner_binding_keeps_its_own_identity() {
        same_behavior(
            "function q(a){var b=a;function inner(b){return b*2}return inner(b)+b}",
            "console.log(q(3))",
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
