use crate::js_peephole::rewrite::is_property_identifier;
use crate::js_peephole::scope::nested_function_end;
use crate::js_peephole::token::{Token, TokenKind};

#[derive(Clone, Copy, PartialEq, Eq)]
enum MemberKey<'a> {
    Ident(&'a str),
    Computed,
}

pub(crate) fn identifier_is_assigned_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
    name: &str,
) -> bool {
    let mut index = from;
    let mut assigning_closure = false;
    let mut called = false;
    while index < to {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if immediately_invoked_after(tokens, close) {
                if let Some(body) = nested_function_body_start(tokens, matching_close, index) {
                    if identifier_is_assigned_between(
                        tokens,
                        matching_close,
                        body,
                        close.min(to),
                        name,
                    ) {
                        return true;
                    }
                }
            } else if nested_function_body_assigns(tokens, matching_close, index, close, name) {
                assigning_closure = true;
            }
            index = close + 1;
            continue;
        }
        if tokens[index].kind == TokenKind::Identifier
            && tokens[index].text == name
            && !is_property_identifier(tokens, index)
            && identifier_use_is_assignment(tokens, index)
        {
            return true;
        }
        if token_starts_call(tokens, index) {
            called = true;
        }
        index += 1;
    }
    assigning_closure && called
}

pub(crate) fn source_receiver_overwritten_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    expr_from: usize,
    expr_to: usize,
    from: usize,
    to: usize,
) -> bool {
    let mut assigned_checked = Vec::<&str>::new();
    for index in expr_from..expr_to {
        if !is_member_receiver(tokens, index) {
            continue;
        }
        let name = tokens[index].text;
        if tokens[index].kind == TokenKind::Identifier && !assigned_checked.contains(&name) {
            assigned_checked.push(name);
            if identifier_is_assigned_between(tokens, matching_close, from, to, name) {
                return true;
            }
        }
        let (path, _) = member_chain_from(tokens, matching_close, index, expr_to);
        if path.is_empty() {
            continue;
        }
        if member_path_is_written_between(tokens, matching_close, from, to, name, &path) {
            return true;
        }
    }
    false
}

fn is_member_receiver(tokens: &[Token<'_>], index: usize) -> bool {
    if is_property_identifier(tokens, index) {
        return false;
    }
    tokens[index].kind == TokenKind::Identifier || tokens[index].text == "this"
}

fn identifier_use_is_assignment(tokens: &[Token<'_>], index: usize) -> bool {
    if next_is_member_start(tokens, index) {
        return false;
    }
    let next = tokens.get(index + 1).map(|token| token.text);
    let prev = index.checked_sub(1).map(|prev| tokens[prev].text);
    next.is_some_and(is_value_assignment_punct) || matches!(prev, Some("++") | Some("--"))
}

fn next_is_member_start(tokens: &[Token<'_>], index: usize) -> bool {
    tokens.get(index + 1).is_some_and(|token| token.text == "[")
        || skip_member_start(tokens, index + 1).is_some()
}

fn is_value_assignment_punct(text: &str) -> bool {
    matches!(text, "++" | "--")
        || (text.ends_with('=')
            && !matches!(text, "==" | "===" | "!=" | "!==" | "<=" | ">=" | "=>"))
}

fn skip_member_start(tokens: &[Token<'_>], index: usize) -> Option<usize> {
    let text = tokens.get(index)?.text;
    if text == "." || text == "?." {
        return Some(index + 1);
    }
    if text == "?" && tokens.get(index + 1).map(|token| token.text) == Some(".") {
        return Some(index + 2);
    }
    None
}

fn member_chain_from<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    ident_at: usize,
    until: usize,
) -> (Vec<MemberKey<'a>>, usize) {
    let mut index = ident_at + 1;
    let mut path = Vec::new();
    while index < until {
        if let Some(after_dot) = skip_member_start(tokens, index) {
            if after_dot >= until {
                break;
            }
            if matches!(
                tokens[after_dot].kind,
                TokenKind::Identifier | TokenKind::Keyword
            ) {
                path.push(MemberKey::Ident(tokens[after_dot].text));
                index = after_dot + 1;
                continue;
            }
            break;
        }
        if tokens[index].text == "[" {
            let Some(close) = matching_close[index] else {
                break;
            };
            if close >= until {
                break;
            }
            path.push(MemberKey::Computed);
            index = close + 1;
            continue;
        }
        break;
    }
    (path, index)
}

fn member_path_is_written_between(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    from: usize,
    to: usize,
    name: &str,
    read_path: &[MemberKey<'_>],
) -> bool {
    let mut index = from;
    let mut assigning_closure = false;
    let mut called = false;
    while index < to {
        if let Some(close) = nested_function_end(tokens, matching_close, index) {
            if immediately_invoked_after(tokens, close) {
                if let Some(body) = nested_function_body_start(tokens, matching_close, index) {
                    if member_path_is_written_between(
                        tokens,
                        matching_close,
                        body,
                        close.min(to),
                        name,
                        read_path,
                    ) {
                        return true;
                    }
                }
            } else if nested_function_body_writes_member(
                tokens,
                matching_close,
                index,
                close,
                name,
                read_path,
            ) {
                assigning_closure = true;
            }
            index = close + 1;
            continue;
        }
        if is_member_receiver(tokens, index) && tokens[index].text == name {
            if let Some(write_path) = assigned_member_path_from(tokens, matching_close, index, to) {
                if path_write_invalidates_read(&write_path, read_path) {
                    return true;
                }
            }
        }
        if token_starts_call(tokens, index) {
            called = true;
        }
        index += 1;
    }
    assigning_closure && called
}

fn immediately_invoked_after(tokens: &[Token<'_>], close: usize) -> bool {
    let mut index = close + 1;
    if tokens.get(index).map(|token| token.text) == Some(")") {
        index += 1;
    }
    tokens.get(index).map(|token| token.text) == Some("(")
}

fn nested_function_body_start(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scan: usize,
) -> Option<usize> {
    if tokens[scan].text == "=>" {
        return Some(scan + 2);
    }
    if tokens[scan].text == "function" {
        let mut index = scan + 1;
        if tokens
            .get(index)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
        {
            index += 1;
        }
        let close_paren = matching_close.get(index).copied().flatten()?;
        return Some(close_paren + 2);
    }
    if tokens[scan].kind == TokenKind::Identifier
        && tokens.get(scan + 1).map(|token| token.text) == Some("(")
    {
        let close_paren = matching_close.get(scan + 1).copied().flatten()?;
        return Some(close_paren + 2);
    }
    None
}

fn nested_function_body_assigns(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scan: usize,
    close: usize,
    name: &str,
) -> bool {
    nested_function_body_start(tokens, matching_close, scan).is_some_and(|body| {
        identifier_is_assigned_between(tokens, matching_close, body, close, name)
    })
}

fn nested_function_body_writes_member(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    scan: usize,
    close: usize,
    name: &str,
    read_path: &[MemberKey<'_>],
) -> bool {
    nested_function_body_start(tokens, matching_close, scan).is_some_and(|body| {
        member_path_is_written_between(tokens, matching_close, body, close, name, read_path)
    })
}

fn token_starts_call(tokens: &[Token<'_>], index: usize) -> bool {
    if tokens[index].text == "new" {
        return true;
    }
    if tokens[index].text != "(" {
        return false;
    }
    index
        .checked_sub(1)
        .and_then(|prev| tokens.get(prev))
        .is_some_and(|prev| prev.kind == TokenKind::Identifier || matches!(prev.text, ")" | "]"))
}

fn assigned_member_path_from<'a>(
    tokens: &'a [Token<'a>],
    matching_close: &[Option<usize>],
    ident_at: usize,
    until: usize,
) -> Option<Vec<MemberKey<'a>>> {
    let (path, after) = member_chain_from(tokens, matching_close, ident_at, until);
    if path.is_empty() {
        return None;
    }
    let suffix_assign = tokens
        .get(after)
        .is_some_and(|token| is_value_assignment_punct(token.text));
    let prefix_assign = ident_at
        .checked_sub(1)
        .is_some_and(|prev| matches!(tokens[prev].text, "++" | "--" | "delete"));
    (suffix_assign || prefix_assign).then_some(path)
}

fn path_write_invalidates_read(write: &[MemberKey<'_>], read: &[MemberKey<'_>]) -> bool {
    if write.is_empty() {
        return false;
    }
    if write.iter().any(|key| matches!(key, MemberKey::Computed))
        || read.iter().any(|key| matches!(key, MemberKey::Computed))
    {
        return true;
    }
    write.len() <= read.len()
        && write
            .iter()
            .zip(read.iter())
            .all(|(left, right)| left == right)
}

#[cfg(test)]
mod tests {
    use super::source_receiver_overwritten_between;
    use crate::js_peephole::token::{lex, matching_closers};

    fn snapshot_overwritten(source: &str) -> bool {
        let tokens = lex(source).expect("lex");
        let matching_close = matching_closers(&tokens);
        let mut index = 0usize;
        while index + 2 < tokens.len() {
            if tokens[index].text == "var"
                && tokens[index + 1].text == "d"
                && tokens[index + 2].text == "="
            {
                let expr_from = index + 3;
                let expr_to = (expr_from..tokens.len())
                    .find(|&at| tokens[at].text == ";")
                    .expect("rhs semicolon");
                let from = expr_to + 1;
                let to = (from..tokens.len())
                    .find(|&at| tokens[at].text == "return")
                    .expect("return");
                return source_receiver_overwritten_between(
                    &tokens,
                    &matching_close,
                    expr_from,
                    expr_to,
                    from,
                    to,
                );
            }
            index += 1;
        }
        panic!("no var d=");
    }

    #[test]
    fn member_assignment_overwrites_the_snapshotted_property() {
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href;b.href=x;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href;b.href+=x;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b){var d=b.href;b.href++;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b){var d=b.href;++b.href;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b){var d=b.href;delete b.href;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href.c;b.href=x;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b[k];b.href=x;return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href;b[k]=x;return d}"
        ));
    }

    #[test]
    fn sibling_or_nested_writes_do_not_overwrite_an_object_snapshot() {
        assert!(!snapshot_overwritten(
            "function f(b,x){var d=b.href;b.src=x;return d}"
        ));
        assert!(!snapshot_overwritten(
            "function f(b,x){var d=b.href;b.href.c=x;return d}"
        ));
        assert!(!snapshot_overwritten(
            "function f(b,x){var d=b;b.href=x;return d}"
        ));
    }

    #[test]
    fn rebinding_the_receiver_still_overwrites_a_member_snapshot() {
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href;b=x;return d}"
        ));
    }

    #[test]
    fn invoked_nested_function_overwrites_a_captured_receiver() {
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href;(()=>{b=x})();return d}"
        ));
        assert!(snapshot_overwritten(
            "function f(b,x){var d=b.href;var r=()=>{b=x};r();return d}"
        ));
        assert!(!snapshot_overwritten(
            "function f(b,x){var d=b.href;var r=()=>{b=x};return d}"
        ));
    }
}
