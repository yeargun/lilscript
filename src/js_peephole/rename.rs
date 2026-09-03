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
//! 2. Every scope in the artifact was resolved. Whole-artifact rewriting cannot
//!    leave an occurrence unchanged inside an unsupported nested scope.
//! 3. The replacement cannot capture a free/outer use or be captured by a fixed
//!    descendant function/class name.

use crate::js_peephole::binding::{BindingResolution, Resolution};
use crate::js_peephole::rewrite::{apply_token_rewrites, is_property_identifier};
use crate::js_peephole::token::{lex, template_has_substitution, Token, TokenKind};
use crate::js_peephole::JavaScriptParseError;
use std::collections::{HashMap, HashSet};

pub(crate) fn converge_local_names(source: &str) -> Result<(String, usize), JavaScriptParseError> {
    converge_names(source, &HashMap::new())
}

/// Apply one idiom conversion, expressed as declaration-token -> spelling.
///
/// The map's keys index the token stream of `source`, so it must be the exact
/// text the group was computed from.
pub(crate) fn converge_with_preferences(
    source: &str,
    preferences: &HashMap<usize, String>,
) -> Result<(String, usize), JavaScriptParseError> {
    converge_names(source, preferences)
}

/// Every repeated idiom's conversion, ranked by the novel text it would remove,
/// one map per idiom.
///
/// They are returned separately rather than merged because merging is what 059
/// measured failing: the whole assignment applied at once is a loss on every
/// port, while the individual idioms inside it have not been priced one by one.
/// The caller applies one, lets the codec rule on it, and recomputes -- so the
/// only conversions that survive are the ones that pay for themselves.
pub(crate) fn idiom_conversion_groups(
    source: &str,
) -> Result<Vec<HashMap<usize, String>>, JavaScriptParseError> {
    let tokens = lex(source)?;
    if tokens
        .iter()
        .any(|token| token.kind == TokenKind::Template && template_has_substitution(token.text))
    {
        return Ok(Vec::new());
    }
    let resolution = BindingResolution::new(&tokens);
    if resolution
        .function_scopes()
        .iter()
        .any(|(scope, _, _)| !resolution.scope_is_sound(*scope))
    {
        return Ok(Vec::new());
    }
    Ok(idiom_preference_groups(&tokens, &resolution))
}

fn converge_names(
    source: &str,
    preferences: &HashMap<usize, String>,
) -> Result<(String, usize), JavaScriptParseError> {
    let tokens = lex(source)?;
    // A template token swallows its `${...}` substitutions whole
    // (`scan_template` in token.rs), so an identifier referenced inside one is
    // invisible to the resolver: renaming its binding would leave that
    // occurrence pointing at a spelling that no longer exists. That hazard is
    // real, but it belongs to templates that *have* a substitution.
    //
    // The guard used to ask whether the artifact contained a backtick at all,
    // which is a much larger question. A template with no substitution is inert
    // text -- it mentions no binding and can capture none -- so refusing the
    // artifact for it disables the pass over every scope for nothing. Measured
    // on katexlil: 24 template literals, **none** holding a substitution, and
    // all 522 function scopes refused across 250 KB, on a port whose whole
    // remaining gap is how its identifiers are spelled (053, 056).
    let templates = tokens
        .iter()
        .filter(|token| {
            token.kind == TokenKind::Template && template_has_substitution(token.text)
        })
        .count();
    if templates > 0 {
        crate::timing::RENAME_TEMPLATED.event(templates as u64);
        return Ok((source.to_string(), 0));
    }
    // Converging on a spelling the artifact does not already use trades one
    // kind of repetition for another and loses. Measured on jQuery: renaming to
    // `a,b,c` while the file's own identifiers are `e,t,n,r` converges headers
    // from 68 spellings to 50 and costs 350 Brotli bytes. The sequence has to
    // start from the characters this artifact already spends most of its
    // identifier bytes on.
    let alphabet = dominant_identifier_alphabet(&tokens);
    let resolution = BindingResolution::new(&tokens);
    // A scope the resolver could not account for -- a destructured parameter
    // list -- resolves every use inside it, of outer bindings included, to
    // nothing, so a rename around it would leave those uses behind: one such
    // scope closes the whole rewrite. A name a function declares twice is
    // not that case. Its tokens all resolve `Unresolved`, which blocks the
    // spelling in every scope that contains one (below), and every other name
    // in the scope still resolves exactly. Asking for a total resolution here
    // instead treated the two alike, and the emitter's second `var t` in one
    // function -- six of them on jQuery, in both the committed artifact and
    // the tree build -- closed the rewrite over the whole artifact (041).
    let unsound = resolution
        .function_scopes()
        .iter()
        .filter(|(scope, _, _)| !resolution.scope_is_sound(*scope))
        .count();
    if unsound > 0 {
        crate::timing::RENAME_UNSOUND.event(unsound as u64);
        return Ok((source.to_string(), 0));
    }
    if !resolution.is_total() {
        crate::timing::RENAME_AMBIGUOUS.event(1);
    }

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
            // Named function/class expressions keep their spelling because
            // `.name` is observable. An outer binding renamed to that spelling
            // would be captured at uses inside the descendant scope.
            if names_a_function_or_class(&tokens, index) {
                blocked.insert(tokens[index].text.to_string());
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

        // `(parameter position, secondary, declaration)`. Position leads because
        // header shape is what repeats: every first parameter is assigned before
        // any second parameter, so same-arity functions converge on one spelling.
        //
        // The secondary term decides the *body* bindings, and which term is
        // right depends on the objective. Ranking by use count minimises the sum
        // of name lengths, which is what a raw objective wants. Under a
        // compressing objective a name's length is nearly free after its first
        // occurrence -- measured: a Brotli match costs the same whatever its
        // length (eps ~ 0), while novel text costs ~0.43 bytes per byte -- and
        // what pays instead is that a structurally identical function spells the
        // same way. Use counts differ between two such functions whenever their
        // bodies differ at all, so use-count ranking permutes their names apart
        // exactly where converging them would have paid most. Ordering by first
        // occurrence is a canonical form: two alpha-equivalent scopes have their
        // bindings in the same first-occurrence order by construction, so they
        // receive the same names with no clustering pass at all.
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
                renameable.push((rank, secondary_rank(&uses, declaration), declaration));
            }
        }
        renameable.sort_unstable();

        // A binding an idiom wants spelled a particular way gets that spelling
        // when the scope still has it free; the canonical sequence below then
        // fills every other binding as usual, skipping what was just claimed.
        // Blocking is already complete at this point -- names the scope keeps,
        // names it reads from outside, and descendant function names are all in
        // `blocked` -- so a preference honoured here is legal by the same proof
        // the canonical path relies on.
        let mut preferred = HashMap::<usize, String>::new();
        for (_, _, declaration) in &renameable {
            let Some(target) = preferences.get(declaration) else {
                continue;
            };
            if is_reserved_word(target) || blocked.contains(target) {
                continue;
            }
            blocked.insert(target.clone());
            preferred.insert(*declaration, target.clone());
        }

        let mut canonical = CanonicalNames::new(&alphabet);
        for (_, _, declaration) in renameable {
            if let Some(replacement) = preferred.remove(&declaration) {
                assigned.insert(declaration, replacement.clone());
                if replacement != tokens[declaration].text {
                    for site in uses.get(&declaration).into_iter().flatten() {
                        rewrites.push((
                            tokens[*site].start,
                            tokens[*site].end,
                            replacement.clone(),
                        ));
                    }
                    rewrites.push((
                        tokens[declaration].start,
                        tokens[declaration].end,
                        replacement,
                    ));
                }
                continue;
            }
            let name = tokens[declaration].text;
            let replacement = loop {
                let Some(candidate) = canonical.next_name() else {
                    break None;
                };
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

/// Widths of the token windows treated as idioms. Four tokens is the shortest
/// run worth a match; past ten the windows are literal tables rather than
/// idioms, and the cost grows with no return.
const IDIOM_WIDTHS: std::ops::RangeInclusive<usize> = 4..=10;
/// A window under this many bytes is already carried by the compressor; over the
/// upper bound it is data, not code.
const IDIOM_MIN_SPAN: usize = 12;
const IDIOM_MAX_SPAN: usize = 220;
/// A shape has to recur before converging it can pay for the rename.
const IDIOM_MIN_OCCURRENCES: usize = 4;

/// Sweep knobs for 059. `LILSCRIPT_IDIOM_MIN_OCC` raises the recurrence a shape
/// must show before it may move a name; `LILSCRIPT_IDIOM_MAX_BINDINGS` caps how
/// many bindings the census is allowed to claim, which bounds how far the
/// canonical sequence behind them is displaced.
fn idiom_min_occurrences() -> usize {
    static VALUE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("LILSCRIPT_IDIOM_MIN_OCC")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(IDIOM_MIN_OCCURRENCES)
    })
}

fn idiom_max_bindings() -> usize {
    static VALUE: std::sync::OnceLock<usize> = std::sync::OnceLock::new();
    *VALUE.get_or_init(|| {
        std::env::var("LILSCRIPT_IDIOM_MAX_BINDINGS")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(usize::MAX)
    })
}

/// Which spelling each binding should take so that repeated idioms spell alike.
///
/// One linear pass per window width, hashing rather than materialising shapes,
/// so the whole census is `O(tokens x widths)` with no allocation per window.
/// Nothing here decides anything: it returns a preference the caller may or may
/// not be able to honour, and the artifact it produces is scored like any other.
fn idiom_preference_groups(
    tokens: &[Token<'_>],
    resolution: &BindingResolution,
) -> Vec<HashMap<usize, String>> {
    use std::hash::{Hash as _, Hasher as _};

    // A slot is a wildcard only where this pass could actually respell it. Free
    // names, properties, module bindings and function names have to match
    // verbatim or the census counts a rename that is not ours to make.
    let renameable: Vec<Option<usize>> = (0..tokens.len())
        .map(|index| {
            if tokens[index].kind != TokenKind::Identifier
                || is_property_identifier(tokens, index)
            {
                return None;
            }
            let Resolution::Bound(declaration) = resolution.resolve(index) else {
                return None;
            };
            let scope = resolution.scope_index_at(declaration);
            if scope == 0
                || !resolution.scope_is_sound(scope)
                || !resolution.name_is_unambiguous(scope, tokens[declaration].text)
                || names_a_function_or_class(tokens, declaration)
            {
                return None;
            }
            Some(declaration)
        })
        .collect();

    // (start token, width) of every window, grouped by shape then by spelling.
    struct Shape {
        occurrences: Vec<(usize, usize)>,
        spellings: HashMap<u64, usize>,
        span: usize,
    }
    let mut shapes: HashMap<u64, Shape> = HashMap::new();

    for width in IDIOM_WIDTHS {
        let mut last_end: HashMap<u64, usize> = HashMap::new();
        for start in 0..tokens.len().saturating_sub(width) {
            let from = tokens[start].start;
            let to = tokens[start + width - 1].end;
            if to <= from || to - from < IDIOM_MIN_SPAN || to - from > IDIOM_MAX_SPAN {
                continue;
            }
            let mut shape = std::collections::hash_map::DefaultHasher::new();
            let mut spelling = std::collections::hash_map::DefaultHasher::new();
            let mut slots: Vec<usize> = Vec::new();
            let mut wildcards = 0usize;
            for index in start..start + width {
                match renameable[index] {
                    Some(declaration) => {
                        let slot = slots
                            .iter()
                            .position(|held| *held == declaration)
                            .unwrap_or_else(|| {
                                slots.push(declaration);
                                slots.len() - 1
                            });
                        0u8.hash(&mut shape);
                        slot.hash(&mut shape);
                        tokens[index].text.hash(&mut spelling);
                        wildcards += 1;
                    }
                    None => {
                        // A literal's value differs between two members of one
                        // idiom far more often than its structure does, and the
                        // run around it still matches, so only the kind is keyed.
                        match tokens[index].kind {
                            TokenKind::Number
                            | TokenKind::String
                            | TokenKind::Template
                            | TokenKind::Regex => {
                                1u8.hash(&mut shape);
                                std::mem::discriminant(&tokens[index].kind).hash(&mut shape);
                            }
                            _ => {
                                2u8.hash(&mut shape);
                                tokens[index].text.hash(&mut shape);
                            }
                        }
                    }
                }
            }
            if wildcards == 0 {
                continue;
            }
            let key = shape.finish();
            // Occurrences of one shape must not overlap each other, or a window
            // sliding through a single long literal counts itself many times.
            if last_end.get(&key).is_some_and(|previous| from < *previous) {
                continue;
            }
            last_end.insert(key, to);
            let entry = shapes.entry(key).or_insert_with(|| Shape {
                occurrences: Vec::new(),
                spellings: HashMap::new(),
                span: to - from,
            });
            entry.occurrences.push((start, width));
            *entry.spellings.entry(spelling.finish()).or_insert(0) += 1;
        }
    }

    // Rank by the novel text converging the shape would remove. Ordering is by
    // value then by first position, so the result does not depend on hash order.
    let mut ranked: Vec<(usize, usize, u64)> = shapes
        .iter()
        .filter(|(_, shape)| shape.occurrences.len() >= idiom_min_occurrences())
        .map(|(key, shape)| {
            let best = shape.spellings.values().copied().max().unwrap_or(0);
            let convertible = shape.occurrences.len().saturating_sub(best);
            (convertible * shape.span, shape.occurrences[0].0, *key)
        })
        .filter(|(value, _, _)| *value > 0)
        .collect();
    ranked.sort_unstable_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));

    let slots_of = |start: usize, width: usize| -> Vec<usize> {
        let mut slots: Vec<usize> = Vec::new();
        for index in start..start + width {
            if let Some(declaration) = renameable[index] {
                if !slots.contains(&declaration) {
                    slots.push(declaration);
                }
            }
        }
        slots
    };

    let mut groups: Vec<HashMap<usize, String>> = Vec::new();
    for (_, _, key) in ranked {
        let Some(shape) = shapes.get(&key) else {
            continue;
        };
        let Some((target_spelling, _)) = shape
            .spellings
            .iter()
            .max_by_key(|(spelling, count)| (**count, std::cmp::Reverse(**spelling)))
        else {
            continue;
        };
        // The names the winning spelling uses, slot by slot.
        let Some(target) = shape
            .occurrences
            .iter()
            .find(|(start, width)| {
                let mut spelling = std::collections::hash_map::DefaultHasher::new();
                for index in *start..*start + *width {
                    if renameable[index].is_some() {
                        tokens[index].text.hash(&mut spelling);
                    }
                }
                spelling.finish() == *target_spelling
            })
            .map(|(start, width)| {
                slots_of(*start, *width)
                    .into_iter()
                    .map(|declaration| tokens[declaration].text.to_string())
                    .collect::<Vec<_>>()
            })
        else {
            continue;
        };

        // One idiom, one group. Overlap is tracked only inside it: the caller
        // re-derives the groups after every conversion it accepts, so two idioms
        // never need to agree in advance.
        let mut wanted: HashMap<usize, String> = HashMap::new();
        let mut claimed: Vec<bool> = vec![false; tokens.len()];
        for (start, width) in &shape.occurrences {
            if wanted.len() >= idiom_max_bindings() {
                break;
            }
            if claimed[*start..*start + *width].iter().any(|token| *token) {
                continue;
            }
            let slots = slots_of(*start, *width);
            if slots.len() != target.len() {
                continue;
            }
            // A binding has one name. An occurrence whose slot is already
            // committed to a different spelling cannot be converted at all, so
            // it is skipped rather than half-applied.
            if slots.iter().zip(&target).any(|(declaration, name)| {
                wanted.get(declaration).is_some_and(|held| held != name)
            }) {
                continue;
            }
            for token in claimed[*start..*start + *width].iter_mut() {
                *token = true;
            }
            for (declaration, name) in slots.into_iter().zip(&target) {
                wanted.entry(declaration).or_insert_with(|| name.clone());
            }
        }
        if !wanted.is_empty() {
            groups.push(wanted);
        }
    }
    groups
}

/// How body bindings are ranked within a scope. See the ranking site above.
///
/// `LILSCRIPT_NAME_ORDER` selects it for A/B measurement: `uses` (the incumbent,
/// descending use count), `decl` (declaration order) or `first` (order of first
/// occurrence anywhere in the scope, declaration included).
#[derive(Clone, Copy, PartialEq, Eq)]
enum NameOrder {
    Uses,
    Declaration,
    FirstOccurrence,
}

fn name_order() -> NameOrder {
    static ORDER: std::sync::OnceLock<NameOrder> = std::sync::OnceLock::new();
    *ORDER.get_or_init(|| match std::env::var("LILSCRIPT_NAME_ORDER").as_deref() {
        Ok("decl") => NameOrder::Declaration,
        Ok("first") => NameOrder::FirstOccurrence,
        _ => NameOrder::Uses,
    })
}

fn secondary_rank(uses: &HashMap<usize, Vec<usize>>, declaration: usize) -> usize {
    match name_order() {
        NameOrder::Uses => usize::MAX - uses.get(&declaration).map_or(0, Vec::len),
        // `declaration` already tiebreaks the sort, so declaration order needs
        // no secondary term of its own.
        NameOrder::Declaration => 0,
        NameOrder::FirstOccurrence => uses
            .get(&declaration)
            .and_then(|sites| sites.iter().copied().min())
            .map_or(declaration, |first| first.min(declaration)),
    }
}

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

    fn next_name(&mut self) -> Option<String> {
        let index = self.next;
        self.next += 1;
        let width = self.alphabet.len();
        if index < width {
            return Some(String::from(self.alphabet[index] as char));
        }
        let index = index - width;
        if index >= width.saturating_mul(width) {
            return None;
        }
        let first = self.alphabet[index / width] as char;
        let second = self.alphabet[index % width] as char;
        let mut name = String::with_capacity(2);
        name.push(first);
        name.push(second);
        Some(name)
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
pub(crate) fn names_a_function_or_class(tokens: &[Token<'_>], declaration: usize) -> bool {
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
    use super::{converge_local_names, CanonicalNames, ALPHABET};

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

    /// 057 — an inert template must not disable the pass. katexlil carries 24
    /// template literals and not one substitution, and every one of its 522
    /// function scopes was refused for them.
    #[test]
    fn a_template_without_a_substitution_does_not_stop_the_rename() {
        let source =
            "var doc=`\\\\hdashline`;function q(elem,key){return elem+key}             function r(key,elem){return key-elem}";
        let (out, count) = converge_local_names(source).unwrap();
        assert!(count > 0, "an inert template must not close the rewrite: {out}");
        assert!(out.contains("`\\\\hdashline`"), "the template text is untouched: {out}");
        assert!(out.contains("function q(e,k)") && out.contains("function r(e,k)"), "{out}");
    }

    /// The hazard the guard exists for: the lexer swallows `${...}` whole, so a
    /// rename cannot see the occurrence inside it. Refusing is still correct.
    #[test]
    fn a_template_with_a_substitution_still_stops_the_rename() {
        let source = "function q(elem,key){return `${elem}:${key}`}                      function r(key,elem){return key-elem}";
        let (out, count) = converge_local_names(source).unwrap();
        assert_eq!(count, 0, "a substitution must close the rewrite");
        assert_eq!(out, source, "the artifact is returned untouched");
    }

    /// An escaped dollar is a literal, not a substitution.
    #[test]
    fn an_escaped_dollar_is_not_a_substitution() {
        use crate::js_peephole::token::template_has_substitution;
        assert!(!template_has_substitution("`plain`"));
        assert!(!template_has_substitution("`\\${notASubstitution}`"));
        assert!(template_has_substitution("`${x}`"));
        assert!(template_has_substitution("`a ${x} b`"));
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

    /// ident-05 remaining shape: an inlined IIFE local must not steal a name
    /// a nested body still uses to reach the enclosing binding.
    #[test]
    fn an_iife_local_does_not_steal_an_outer_binding_a_nested_body_still_reads() {
        let source = concat!(
            "function go(callback){",
            "return (function(){",
            "var notFn={};",
            "(function(){callback()})();",
            "return notFn",
            "})()}",
        );
        same_behavior(
            source,
            "go(function(){console.log('called')});console.log(typeof go(function(){}))",
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
    fn an_unresolved_descendant_disables_whole_artifact_convergence() {
        let source = "function good(longName){return longName+1}function blocked({x}){return x}";
        let (out, count) = converge_local_names(source).unwrap();
        assert_eq!(
            count, 0,
            "an unresolved scope must close the whole rewrite: {out}"
        );
        assert_eq!(out, source);
    }

    /// A function that declares one name twice (the emitter's `var t` inside a
    /// branch after `var t` at the top) keeps that name and still converges
    /// the rest. Six such functions closed the rewrite over all of jQuery.
    #[test]
    fn a_name_declared_twice_keeps_its_spelling_while_its_scope_still_converges() {
        let source = concat!(
            "function q(elem,key){var t=elem.x;if(!t){var r,t=\"\",a=0;",
            "for(;a<key;a++)t+=a}return t+elem.y}",
            "function w(key,elem){return key-elem}",
        );
        same_behavior(source, "console.log(q({x:0,y:'!'},3),q({x:2,y:'?'},1),w(5,2))");
        let (out, count) = converge_local_names(source).unwrap();
        assert!(count > 0, "the duplicate must not close the rewrite: {out}");
        assert!(
            out.contains("var t=") && out.contains(",t=\"\""),
            "the duplicate keeps its name: {out}"
        );
        // `w` binds no `t`, so it may take it; `q` may not.
        let header = out
            .split("function q(")
            .nth(1)
            .and_then(|rest| rest.split(')').next())
            .unwrap_or_default();
        assert!(!header.split(',').any(|name| name == "t"), "captured `t`: {out}");
    }

    /// And an ancestor may not take the duplicated name for a binding the
    /// inner scope reads.
    #[test]
    fn an_ancestor_avoids_a_name_a_descendant_declares_twice() {
        let source = concat!(
            "function outer(alpha){",
            "function inner(beta){var t=beta;if(beta>1){var t=beta*2}return t+alpha}",
            "return inner(1)+inner(3)}",
        );
        same_behavior(source, "console.log(outer(10))");
        let (out, _) = converge_local_names(source).unwrap();
        assert!(!out.contains("function outer(t)"), "captured by the inner `t`: {out}");
    }

    #[test]
    fn a_template_expression_disables_convergence_until_it_has_binding_identity() {
        let source = "function f(longName){return`${longName+1}`}";
        let (out, count) = converge_local_names(source).unwrap();
        assert_eq!(
            count, 0,
            "template bindings must not be partially renamed: {out}"
        );
        assert_eq!(out, source);
    }

    #[test]
    fn a_fixed_descendant_name_cannot_capture_a_renamed_outer_binding() {
        let source = "function outer(eeee){return function e(){return+eeee}}";
        same_behavior(source, "console.log(outer(7)())");
        let (out, _) = converge_local_names(source).unwrap();
        assert!(
            !out.contains("outer(e)"),
            "fixed descendant captured outer: {out}"
        );
    }

    #[test]
    fn canonical_names_report_exhaustion_after_two_characters() {
        let mut names = CanonicalNames::new(ALPHABET);
        for _ in 0..ALPHABET.len() + ALPHABET.len() * ALPHABET.len() {
            assert!(names.next_name().is_some());
        }
        assert_eq!(names.next_name(), None);
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
