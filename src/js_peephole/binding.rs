//! Total use-to-binding resolution for generated JavaScript.
//!
//! The rewrite passes historically answered scope questions with independent
//! token scans -- "is this name declared in an enclosing block", "is it bound in
//! a nested function between here and there". Each such predicate is a separate
//! approximation, each fails *open*, and three miscompiles in this lane came
//! from folds that matched text without resolving it.
//!
//! This module answers one question exactly instead: for every identifier
//! token, which token declares it? A consumer that can ask that does not need
//! to guess from neighbouring punctuation.
//!
//! Two properties make it safe to build rewrites on:
//!
//! * **Total.** Every identifier resolves to [`Resolution::Bound`],
//!   [`Resolution::Free`], or [`Resolution::Unresolved`]. There is no "maybe"
//!   for a caller to interpret.
//! * **Fail-closed.** Anything the scanner cannot account for marks its scope
//!   unresolved, and unresolved scopes report `Resolution::Unresolved` for every
//!   name in them. A consumer must refuse to rewrite those; it may never read
//!   the absence of a binding as evidence that a name is free.
//!
//! Scope granularity is the function, not the block. Declarations from nested
//! blocks are attributed to the enclosing function scope, and a function that
//! declares one name twice is marked unresolved rather than guessed at, because
//! only block scoping could tell those bindings apart.

use crate::js_peephole::rewrite::{is_property_identifier, is_statement_boundary};
use crate::js_peephole::token::{matching_closers, matching_openers, Token, TokenKind};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Resolution {
    /// The identifier resolves to the declaration at this token index.
    Bound(usize),
    /// No declaration in any enclosing scope: a global, a host name, or an
    /// import the module does not bind.
    Free,
    /// The scanner could not account for this scope. Consumers must not rewrite
    /// anything here.
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScopeKind {
    Module,
    Function,
    Catch,
}

#[derive(Debug)]
struct Scope<'src> {
    kind: ScopeKind,
    /// First token of the scope. For a function this is the parameter list, not
    /// the body brace: a parameter is declared *before* the body opens, and
    /// attributing it to the enclosing scope is the bug that makes naive
    /// renamers unsound.
    start: usize,
    end: usize,
    parent: Option<u32>,
    declarations: HashMap<&'src str, usize>,
    /// Names this scope declares more than once. Only block scoping could tell
    /// those bindings apart, so the name is refused here while every other name
    /// in the scope still resolves exactly. Poisoning the whole scope would let
    /// one reused temporary silence a file.
    ambiguous: std::collections::HashSet<&'src str>,
    /// False when the scanner could not account for the scope's own shape --
    /// a destructured parameter list, say -- so no name in it can be trusted.
    sound: bool,
}

pub(crate) struct BindingResolution<'src> {
    scopes: Vec<Scope<'src>>,
    scope_of_token: Vec<u32>,
    resolution: Vec<Resolution>,
}

impl<'src> BindingResolution<'src> {
    pub(crate) fn new(tokens: &[Token<'src>]) -> Self {
        let matching_close = matching_closers(tokens);
        let matching_open = matching_openers(&matching_close);
        let mut scopes = vec![Scope {
            kind: ScopeKind::Module,
            start: 0,
            end: tokens.len(),
            parent: None,
            declarations: HashMap::new(),
            ambiguous: std::collections::HashSet::new(),
            sound: true,
        }];
        collect_scopes(tokens, &matching_close, &matching_open, &mut scopes);

        // Innermost scope per token. Scopes are discovered outermost-first, so a
        // later (deeper) scope overwrites its ancestors over its own range.
        let mut scope_of_token = vec![0u32; tokens.len()];
        for (index, scope) in scopes.iter().enumerate().skip(1) {
            for token in scope.start..scope.end.min(tokens.len()) {
                scope_of_token[token] = index as u32;
            }
        }
        assign_named_function_expression_scopes(tokens, &scopes, &mut scope_of_token);

        collect_declarations(tokens, &matching_close, &scope_of_token, &mut scopes);
        let resolution = resolve(tokens, &scopes, &scope_of_token);
        Self {
            scopes,
            scope_of_token,
            resolution,
        }
    }

    pub(crate) fn resolve(&self, at: usize) -> Resolution {
        self.resolution
            .get(at)
            .copied()
            .unwrap_or(Resolution::Unresolved)
    }

    /// The innermost scope containing this token, as `(start, end, kind)`.
    pub(crate) fn scope_at(&self, at: usize) -> Option<(usize, usize, ScopeKind)> {
        let scope = *self.scope_of_token.get(at)?;
        let scope = &self.scopes[scope as usize];
        Some((scope.start, scope.end, scope.kind))
    }

    /// Every function scope as `(index, start, end)`, outermost first.
    pub(crate) fn function_scopes(&self) -> Vec<(usize, usize, usize)> {
        self.scopes
            .iter()
            .enumerate()
            .filter(|(_, scope)| scope.kind != ScopeKind::Module)
            .map(|(index, scope)| (index, scope.start, scope.end))
            .collect()
    }

    /// Declarations owned by a scope, as `(name, declaring token)`.
    pub(crate) fn declarations(&self, scope: usize) -> Vec<(&'src str, usize)> {
        self.scopes
            .get(scope)
            .map(|scope| {
                let mut out = scope
                    .declarations
                    .iter()
                    .map(|(name, at)| (*name, *at))
                    .collect::<Vec<_>>();
                out.sort_unstable_by_key(|(_, at)| *at);
                out
            })
            .unwrap_or_default()
    }

    pub(crate) fn scope_is_sound(&self, scope: usize) -> bool {
        self.scopes.get(scope).is_some_and(|scope| scope.sound)
    }

    pub(crate) fn parent_scope(&self, scope: usize) -> Option<usize> {
        self.scopes.get(scope)?.parent.map(|parent| parent as usize)
    }

    pub(crate) fn scope_declares(&self, scope: usize, name: &str) -> bool {
        self.scopes
            .get(scope)
            .is_some_and(|scope| scope.declarations.contains_key(name))
    }

    /// The innermost scope index containing this token.
    pub(crate) fn scope_index_at(&self, at: usize) -> usize {
        self.scope_of_token.get(at).copied().unwrap_or(0) as usize
    }

    /// True when the name is declared exactly once in this scope, so every use
    /// that resolves here refers to that one binding.
    pub(crate) fn name_is_unambiguous(&self, scope: usize, name: &str) -> bool {
        self.scopes
            .get(scope)
            .is_some_and(|scope| scope.sound && !scope.ambiguous.contains(name))
    }

    /// True when every scope was accounted for. A consumer that rewrites whole
    /// artifacts should check this before it starts.
    pub(crate) fn is_total(&self) -> bool {
        self.scopes
            .iter()
            .all(|scope| scope.sound && scope.ambiguous.is_empty())
    }
}

/// A named function expression owns its name inside the function rather than
/// declaring it in the surrounding scope:
///
/// `var outer=()=>1;(function outer(){ outer() })();outer()`
///
/// The name token is written before the parameter list, so the ordinary scope
/// range does not cover it. Attribute that declaration token to the function's
/// own scope explicitly. Function declarations keep their name in the parent.
fn assign_named_function_expression_scopes(
    tokens: &[Token<'_>],
    scopes: &[Scope<'_>],
    scope_of_token: &mut [u32],
) {
    for function_at in 0..tokens.len() {
        if tokens[function_at].text != "function" || function_is_declaration(tokens, function_at) {
            continue;
        }
        let mut name_at = function_at + 1;
        if tokens.get(name_at).map(|token| token.text) == Some("*") {
            name_at += 1;
        }
        if tokens
            .get(name_at)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name_at + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let params_open = name_at + 1;
        let Some((scope, _)) = scopes
            .iter()
            .enumerate()
            .find(|(_, scope)| scope.kind == ScopeKind::Function && scope.start == params_open)
        else {
            continue;
        };
        scope_of_token[name_at] = scope as u32;
    }
}

fn function_is_declaration(tokens: &[Token<'_>], function_at: usize) -> bool {
    let mut head = function_at;
    while head > 0 && matches!(tokens[head - 1].text, "async" | "default" | "export") {
        head -= 1;
    }
    is_statement_boundary(tokens, head)
}

/// Walk the token stream once, opening a scope at each function-like head.
fn collect_scopes<'src>(
    tokens: &[Token<'src>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    scopes: &mut Vec<Scope<'src>>,
) {
    let mut pending = Vec::<(usize, usize, ScopeKind)>::new();
    for index in 0..tokens.len() {
        if let Some(span) = function_scope_at(tokens, matching_close, matching_open, index) {
            pending.push(span);
        } else if tokens[index].text == "catch" {
            if let Some(span) = catch_scope_at(tokens, matching_close, index) {
                pending.push(span);
            }
        }
    }
    // Outermost-first, so `scope_of_token` ends up holding the innermost.
    pending.sort_by(|left, right| left.0.cmp(&right.0).then_with(|| right.1.cmp(&left.1)));
    for (start, end, kind) in pending {
        let parent = scopes
            .iter()
            .enumerate()
            .filter(|(_, scope)| scope.start <= start && end <= scope.end)
            .max_by_key(|(_, scope)| scope.start)
            .map(|(index, _)| index as u32);
        scopes.push(Scope {
            kind,
            start,
            end,
            parent,
            declarations: HashMap::new(),
            ambiguous: std::collections::HashSet::new(),
            sound: true,
        });
    }
}

/// `(params_start, body_end, Function)` for a function head at `index`.
fn function_scope_at(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    matching_open: &[Option<usize>],
    index: usize,
) -> Option<(usize, usize, ScopeKind)> {
    // `function [name] (params) {body}`
    if tokens[index].text == "function" {
        let mut cursor = index + 1;
        if tokens.get(cursor).map(|token| token.text) == Some("*") {
            cursor += 1;
        }
        if tokens
            .get(cursor)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            cursor += 1;
        }
        if tokens.get(cursor).map(|token| token.text) != Some("(") {
            return None;
        }
        let params_close = matching_close.get(cursor).copied().flatten()?;
        if tokens.get(params_close + 1).map(|token| token.text) != Some("{") {
            return None;
        }
        let body_end = matching_close.get(params_close + 1).copied().flatten()?;
        return Some((cursor, body_end + 1, ScopeKind::Function));
    }

    // `=>` with either `(params)` or a bare identifier in front.
    if tokens[index].text == "=>" {
        let previous = index.checked_sub(1)?;
        let start = if tokens[previous].text == ")" {
            matching_open.get(previous).copied().flatten()?
        } else if tokens[previous].kind == TokenKind::Identifier {
            previous
        } else {
            return None;
        };
        let end = if tokens.get(index + 1).map(|token| token.text) == Some("{") {
            matching_close.get(index + 1).copied().flatten()? + 1
        } else {
            arrow_expression_end(tokens, matching_close, index + 1)?
        };
        return Some((start, end, ScopeKind::Function));
    }

    // Method shorthand in a class body or object literal: `name(params){body}`.
    if tokens[index].kind == TokenKind::Identifier
        && tokens.get(index + 1).map(|token| token.text) == Some("(")
        && !is_property_identifier(tokens, index)
    {
        let opens_member = index
            .checked_sub(1)
            .is_some_and(|previous| matches!(tokens[previous].text, "{" | "}" | ";" | ","))
            || index
                .checked_sub(1)
                .is_some_and(|previous| tokens[previous].text == "async");
        if !opens_member {
            return None;
        }
        let params_close = matching_close.get(index + 1).copied().flatten()?;
        if tokens.get(params_close + 1).map(|token| token.text) != Some("{") {
            return None;
        }
        let body_end = matching_close.get(params_close + 1).copied().flatten()?;
        return Some((index + 1, body_end + 1, ScopeKind::Function));
    }
    None
}

fn catch_scope_at(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    index: usize,
) -> Option<(usize, usize, ScopeKind)> {
    if tokens.get(index + 1).map(|token| token.text) != Some("(") {
        return None;
    }
    let params_close = matching_close.get(index + 1).copied().flatten()?;
    if tokens.get(params_close + 1).map(|token| token.text) != Some("{") {
        return None;
    }
    let body_end = matching_close.get(params_close + 1).copied().flatten()?;
    Some((index + 1, body_end + 1, ScopeKind::Catch))
}

/// A concise arrow body runs to the first top-level `,` `;` `)` `]` `}` — the
/// same stop set the rest of the peephole uses for expression extents.
fn arrow_expression_end(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
) -> Option<usize> {
    let mut index = from;
    while index < tokens.len() {
        match tokens[index].text {
            "(" | "[" | "{" => {
                index = matching_close.get(index).copied().flatten()? + 1;
                continue;
            }
            "," | ";" | ")" | "]" | "}" => return Some(index),
            _ => index += 1,
        }
    }
    Some(tokens.len())
}

fn collect_declarations<'src>(
    tokens: &[Token<'src>],
    matching_close: &[Option<usize>],
    scope_of_token: &[u32],
    scopes: &mut [Scope<'src>],
) {
    // Parameters belong to the scope their list opens. Gathered first so the
    // scan borrows `scopes` immutably, then applied in one mutable pass.
    let parameters = scopes
        .iter()
        .enumerate()
        .skip(1)
        .map(|(index, scope)| (index, parameter_names(tokens, matching_close, scope.start)))
        .collect::<Vec<_>>();
    for (owner, (names, understood)) in parameters {
        if !understood {
            scopes[owner].sound = false;
        }
        for (name, at) in names {
            if scopes[owner].declarations.insert(name, at).is_some() {
                scopes[owner].ambiguous.insert(name);
            }
        }
    }

    // `var` / `let` / `const` / `function` / `class` attach to the nearest
    // function scope. Block scoping is not modelled, so one function declaring
    // the same name twice is marked unresolved rather than guessed at: only
    // block scope could tell those two bindings apart.
    for index in 0..tokens.len() {
        let declared = match tokens[index].text {
            "var" | "let" | "const" => declarator_names(tokens, matching_close, index),
            "import" => import_names(tokens, matching_close, index),
            "function" | "class" => {
                let mut cursor = index + 1;
                if tokens.get(cursor).map(|token| token.text) == Some("*") {
                    cursor += 1;
                }
                if tokens
                    .get(cursor)
                    .is_some_and(|token| token.kind == TokenKind::Identifier)
                {
                    vec![cursor]
                } else {
                    Vec::new()
                }
            }
            _ => continue,
        };
        for at in declared {
            let scope = enclosing_function_scope(scopes, scope_of_token, at);
            let name = tokens[at].text;
            if scopes[scope]
                .declarations
                .insert(name, at)
                .is_some_and(|previous| previous != at)
            {
                scopes[scope].ambiguous.insert(name);
            }
        }
    }
}

fn import_names(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    index: usize,
) -> Vec<usize> {
    let open = index + 1;
    if tokens.get(open).is_none_or(|token| token.text != "{") {
        return Vec::new();
    }
    let Some(close) = matching_close.get(open).copied().flatten() else {
        return Vec::new();
    };
    let mut names = Vec::new();
    let mut cursor = open + 1;
    while cursor < close {
        if tokens[cursor].kind != TokenKind::Identifier {
            cursor += 1;
            continue;
        }
        if tokens
            .get(cursor + 1)
            .is_some_and(|token| token.text == "as")
        {
            if tokens
                .get(cursor + 2)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                names.push(cursor + 2);
            }
        } else {
            names.push(cursor);
        }
        while cursor < close && tokens[cursor].text != "," {
            cursor += 1;
        }
    }
    names
}

fn enclosing_function_scope(scopes: &[Scope<'_>], scope_of_token: &[u32], at: usize) -> usize {
    let mut scope = scope_of_token.get(at).copied().unwrap_or(0) as usize;
    while scopes[scope].kind == ScopeKind::Catch {
        match scopes[scope].parent {
            Some(parent) => scope = parent as usize,
            None => break,
        }
    }
    scope
}

/// Identifier tokens declared by a parameter list opening at `open`, plus
/// whether the list was fully understood.
fn parameter_names<'src>(
    tokens: &[Token<'src>],
    matching_close: &[Option<usize>],
    open: usize,
) -> (Vec<(&'src str, usize)>, bool) {
    // A bare-identifier arrow parameter: `x => …`.
    if tokens
        .get(open)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        return (vec![(tokens[open].text, open)], true);
    }
    if tokens.get(open).map(|token| token.text) != Some("(") {
        return (Vec::new(), false);
    }
    let Some(close) = matching_close.get(open).copied().flatten() else {
        return (Vec::new(), false);
    };
    let mut names = Vec::new();
    let mut index = open + 1;
    let mut expect_name = true;
    while index < close {
        match tokens[index].text {
            "," => {
                expect_name = true;
                index += 1;
            }
            "=" => {
                // Skip a default initializer: it is an expression, not a binding.
                let mut depth = 0i32;
                index += 1;
                while index < close {
                    match tokens[index].text {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => depth -= 1,
                        "," if depth == 0 => break,
                        _ => {}
                    }
                    index += 1;
                }
                expect_name = false;
            }
            "[" | "{" | "..." => return (names, false),
            _ => {
                if expect_name && tokens[index].kind == TokenKind::Identifier {
                    names.push((tokens[index].text, index));
                    expect_name = false;
                    index += 1;
                } else if expect_name {
                    return (names, false);
                } else {
                    index += 1;
                }
            }
        }
    }
    (names, true)
}

/// Identifier tokens bound by a `var` / `let` / `const` statement at `index`.
fn declarator_names(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    index: usize,
) -> Vec<usize> {
    let mut names = Vec::new();
    let mut cursor = index + 1;
    let mut expect_name = true;
    while cursor < tokens.len() {
        match tokens[cursor].text {
            ";" => break,
            "," => {
                expect_name = true;
                cursor += 1;
            }
            "(" | "[" | "{" => {
                match matching_close.get(cursor).copied().flatten() {
                    Some(close) => cursor = close + 1,
                    None => break,
                }
                expect_name = false;
            }
            "=" => {
                expect_name = false;
                cursor += 1;
            }
            "in" | "of" => break,
            _ => {
                if expect_name && tokens[cursor].kind == TokenKind::Identifier {
                    names.push(cursor);
                    expect_name = false;
                }
                cursor += 1;
            }
        }
    }
    names
}

fn resolve(tokens: &[Token<'_>], scopes: &[Scope<'_>], scope_of_token: &[u32]) -> Vec<Resolution> {
    let mut out = vec![Resolution::Free; tokens.len()];
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Identifier || is_property_identifier(tokens, index) {
            out[index] = Resolution::Free;
            continue;
        }
        let mut scope = Some(scope_of_token[index] as usize);
        let mut resolution = Resolution::Free;
        while let Some(current) = scope {
            if !scopes[current].sound || scopes[current].ambiguous.contains(token.text) {
                resolution = Resolution::Unresolved;
                break;
            }
            if let Some(at) = scopes[current].declarations.get(token.text) {
                resolution = Resolution::Bound(*at);
                break;
            }
            scope = scopes[current].parent.map(|parent| parent as usize);
        }
        out[index] = resolution;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{BindingResolution, Resolution};
    use crate::js_peephole::token::{lex, TokenKind};

    /// `(token index, text)` for every identifier, with its resolution.
    fn resolved(source: &str) -> Vec<(usize, String, Resolution)> {
        let tokens = lex(source).unwrap();
        let resolution = BindingResolution::new(&tokens);
        tokens
            .iter()
            .enumerate()
            .filter(|(_, token)| token.kind == TokenKind::Identifier)
            .map(|(index, token)| (index, token.text.to_string(), resolution.resolve(index)))
            .collect()
    }

    fn binding_of(source: &str, occurrence: usize) -> Resolution {
        resolved(source)[occurrence].2
    }

    /// The bug that made the first renamer unsound: a parameter token sits
    /// before the body brace, so a scope model keyed on the body attributes it
    /// to the enclosing function.
    #[test]
    fn parameters_belong_to_their_own_function() {
        let source = "function q(elem,key){return elem+key}function r(key,elem){return key-elem}";
        let all = resolved(source);
        // q, elem(decl), key(decl), elem(use), key(use), r, key(decl), elem(decl), key(use), elem(use)
        let elem_decl_q = all[1].0;
        let key_decl_q = all[2].0;
        assert_eq!(all[3].2, Resolution::Bound(elem_decl_q), "{all:?}");
        assert_eq!(all[4].2, Resolution::Bound(key_decl_q), "{all:?}");
        let key_decl_r = all[6].0;
        let elem_decl_r = all[7].0;
        assert_eq!(all[8].2, Resolution::Bound(key_decl_r), "{all:?}");
        assert_eq!(all[9].2, Resolution::Bound(elem_decl_r), "{all:?}");
        // The two functions' `elem` must be different bindings.
        assert_ne!(elem_decl_q, elem_decl_r);
    }

    #[test]
    fn arrow_parameters_resolve_in_both_spellings() {
        let all = resolved("var h=(a,b)=>a[b];var g=c=>c.x;");
        let a_decl = all[1].0;
        let b_decl = all[2].0;
        assert_eq!(all[3].2, Resolution::Bound(a_decl), "{all:?}");
        assert_eq!(all[4].2, Resolution::Bound(b_decl), "{all:?}");
        let c_decl = all[6].0;
        assert_eq!(all[7].2, Resolution::Bound(c_decl), "{all:?}");
    }

    #[test]
    fn an_inner_parameter_shadows_an_outer_binding() {
        let source = "function q(a){var b=a;function inner(b){return b}return inner(b)}";
        let all = resolved(source);
        let outer_b = all.iter().find(|(_, text, _)| text == "b").unwrap().0;
        let inner_b_decl = all
            .iter()
            .filter(|(_, text, _)| text == "b")
            .nth(1)
            .unwrap()
            .0;
        assert_ne!(outer_b, inner_b_decl);
        // `return b` inside `inner` resolves to the parameter, not the outer var.
        let inner_use = all
            .iter()
            .filter(|(_, text, _)| text == "b")
            .nth(2)
            .unwrap();
        assert_eq!(inner_use.2, Resolution::Bound(inner_b_decl), "{all:?}");
        // `inner(b)` at the tail resolves to the outer var.
        let outer_use = all
            .iter()
            .filter(|(_, text, _)| text == "b")
            .last()
            .unwrap();
        assert_eq!(outer_use.2, Resolution::Bound(outer_b), "{all:?}");
    }

    #[test]
    fn a_named_function_expression_owns_only_its_recursive_name() {
        let source = concat!(
            "function q(){var fire=()=>7;",
            "(function fire(n){if(n)fire(n-1)})(1);",
            "return fire()}",
        );
        let all = resolved(source);
        let fire = all
            .iter()
            .filter(|(_, text, _)| text == "fire")
            .collect::<Vec<_>>();
        assert_eq!(fire.len(), 4, "{all:?}");
        let outer_declaration = fire[0].0;
        let expression_declaration = fire[1].0;
        assert_eq!(
            fire[2].2,
            Resolution::Bound(expression_declaration),
            "{all:?}"
        );
        assert_eq!(fire[3].2, Resolution::Bound(outer_declaration), "{all:?}");
    }

    #[test]
    fn globals_and_host_names_are_free() {
        let all = resolved("function q(a){return JSON.stringify(a)}");
        let json = all.iter().find(|(_, text, _)| text == "JSON").unwrap();
        assert_eq!(json.2, Resolution::Free, "{all:?}");
    }

    #[test]
    fn property_names_never_resolve_to_a_binding() {
        let all = resolved("function q(a){return a.a}");
        let property = all.last().unwrap();
        assert_eq!(property.2, Resolution::Free, "{all:?}");
    }

    #[test]
    fn catch_parameters_bind_inside_the_handler() {
        let all = resolved("function q(){try{f()}catch(e){return e}}");
        let decl = all.iter().find(|(_, text, _)| text == "e").unwrap().0;
        let use_at = all
            .iter()
            .filter(|(_, text, _)| text == "e")
            .nth(1)
            .unwrap();
        assert_eq!(use_at.2, Resolution::Bound(decl), "{all:?}");
    }

    #[test]
    fn a_default_initializer_is_an_expression_not_a_binding() {
        let all = resolved("function q(a,b=a){return b}");
        let a_decl = all[1].0;
        // The `a` inside the default reads the earlier parameter.
        let inside_default = all[3].clone();
        assert_eq!(inside_default.1, "a");
        assert_eq!(inside_default.2, Resolution::Bound(a_decl), "{all:?}");
    }

    /// Fail-closed is the property consumers rely on. A construct the scanner
    /// cannot account for must poison its scope, never report `Free`.
    #[test]
    fn unaccounted_parameter_shapes_poison_their_scope() {
        for source in [
            "function q({a,b}){return a+b}",
            "function q([a,b]){return a+b}",
            "var h=({a})=>a;",
        ] {
            let all = resolved(source);
            assert!(
                all.iter().any(|(_, _, res)| *res == Resolution::Unresolved),
                "expected a poisoned scope for {source}: {all:?}"
            );
            assert!(
                !all.iter()
                    .any(|(_, text, res)| text == "a" && *res == Resolution::Free),
                "a destructured name must never read as free: {source}"
            );
        }
    }

    #[test]
    fn one_function_declaring_a_name_twice_is_unresolved() {
        let all = resolved("function q(a){if(a){let b=1;g(b)}else{let b=2;g(b)}}");
        assert!(
            all.iter().any(|(_, _, res)| *res == Resolution::Unresolved),
            "block-scoped reuse must poison rather than merge: {all:?}"
        );
    }

    #[test]
    fn module_bindings_resolve_from_inside_functions() {
        let all = resolved("var top=1;function q(x){return top+x}");
        let top_decl = all[0].0;
        let use_at = all
            .iter()
            .filter(|(_, text, _)| text == "top")
            .nth(1)
            .unwrap();
        assert_eq!(use_at.2, Resolution::Bound(top_decl), "{all:?}");
    }

    #[test]
    fn class_methods_open_their_own_scope() {
        let all = resolved("class C{m(a){return a}n(a){return a}}");
        let first_decl = all
            .iter()
            .filter(|(_, text, _)| text == "a")
            .next()
            .unwrap()
            .0;
        let second_decl = all
            .iter()
            .filter(|(_, text, _)| text == "a")
            .nth(2)
            .unwrap()
            .0;
        assert_ne!(first_decl, second_decl, "{all:?}");
        let second_use = all
            .iter()
            .filter(|(_, text, _)| text == "a")
            .nth(3)
            .unwrap();
        assert_eq!(second_use.2, Resolution::Bound(second_decl), "{all:?}");
    }

    #[test]
    fn every_identifier_gets_a_resolution() {
        let source = "var a=1;function q(b){return a+b+JSON.parse(b).c}class K{m(){return new K}}";
        let all = resolved(source);
        assert!(!all.is_empty());
        for (index, text, resolution) in &all {
            assert!(
                matches!(
                    resolution,
                    Resolution::Bound(_) | Resolution::Free | Resolution::Unresolved
                ),
                "token {index} ({text}) had no resolution"
            );
        }
    }
}
