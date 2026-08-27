use crate::js_peephole::JavaScriptParseError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TokenKind {
    Identifier,
    Number,
    String,
    Template,
    Keyword,
    Punct,
    Regex,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Token<'src> {
    pub(crate) kind: TokenKind,
    pub(crate) text: &'src str,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

pub(crate) fn ascii_identifier_name_string(literal: &str) -> Option<&str> {
    let bytes = literal.as_bytes();
    if bytes.len() < 3
        || !matches!(bytes[0], b'\'' | b'"')
        || bytes.last().copied() != Some(bytes[0])
    {
        return None;
    }
    let identifier = &bytes[1..bytes.len() - 1];
    if !is_identifier_start(identifier[0])
        || !identifier[1..].iter().copied().all(is_identifier_continue)
    {
        return None;
    }
    Some(&literal[1..literal.len() - 1])
}

pub(crate) fn matching_closers(tokens: &[Token<'_>]) -> Vec<Option<usize>> {
    let mut matching_close = vec![None; tokens.len()];
    let mut stack = Vec::<usize>::new();
    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "(" | "[" | "{" => stack.push(index),
            ")" | "]" | "}" => {
                let Some(open) = stack.pop() else {
                    continue;
                };
                matching_close[open] = Some(index);
            }
            _ => {}
        }
    }
    matching_close
}

pub(crate) fn matching_openers(matching_close: &[Option<usize>]) -> Vec<Option<usize>> {
    let mut matching_open = vec![None; matching_close.len()];
    for (open, close) in matching_close.iter().enumerate() {
        if let Some(close) = *close {
            matching_open[close] = Some(open);
        }
    }
    matching_open
}

pub(crate) fn validate_delimiters(tokens: &[Token<'_>]) -> Result<usize, JavaScriptParseError> {
    let mut stack = Vec::new();
    let mut maximum = 0;
    for token in tokens {
        match token.text {
            "(" | "[" | "{" => {
                stack.push(token);
                maximum = maximum.max(stack.len());
            }
            ")" | "]" | "}" => {
                let Some(open) = stack.pop() else {
                    return Err(JavaScriptParseError {
                        offset: token.start,
                        message: "unmatched closing delimiter",
                        context: None,
                    });
                };
                let matches = matches!(
                    (open.text, token.text),
                    ("(", ")") | ("[", "]") | ("{", "}")
                );
                if !matches {
                    return Err(JavaScriptParseError {
                        offset: token.start,
                        message: "mismatched closing delimiter",
                        context: None,
                    });
                }
            }
            _ => {}
        }
    }
    if let Some(open) = stack.last() {
        return Err(JavaScriptParseError {
            offset: open.start,
            message: "unclosed delimiter",
            context: None,
        });
    }
    Ok(maximum)
}

/// How a still-open delimiter was opened. Only the distinction that decides
/// whether a `/` after the matching closer starts a regular expression or a
/// division is tracked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GroupKind {
    /// `(` of `if`/`while`/`for`/`with`/`switch`/`catch`. A statement — and
    /// therefore a regular-expression literal — may follow its `)`.
    ControlHeader,
    /// Any other `(`: a call argument list, parameter list, or grouping. Its
    /// `)` ends an expression, so a following `/` is division.
    Expression,
    Bracket,
    Brace,
}

/// A lexed token stream plus whether every `/` in it was classified from an
/// unambiguous predecessor.
///
/// Regex-versus-division is decided by the previous significant token. Every
/// predecessor this emitter produces resolves exactly — `)` through the group
/// stack, `]`/`++`/`--`/identifiers/literals as division, and every other
/// operator or statement keyword as a regex start. The single genuinely
/// ambiguous predecessor in the grammar is `}`, which ends a block (statement
/// position, regex) or an object/function expression (division). That case is
/// reported rather than guessed, so punctuation-shaped rewrites can refuse a
/// stream they cannot prove they read correctly.
#[derive(Debug, Clone)]
pub(crate) struct LexedSource<'src> {
    tokens: Vec<Token<'src>>,
    slash_classification_certain: bool,
}

pub(crate) fn lex(source: &str) -> Result<Vec<Token<'_>>, JavaScriptParseError> {
    Ok(lex_classified(source)?.tokens)
}

/// Lex only when every `/` was classified from an unambiguous predecessor.
///
/// Punctuation-shaped rewrites select operators positionally, so a `/` read as
/// division when it opened a regular-expression body (or the reverse) would
/// corrupt the artifact. Returning `None` keeps the caller's input untouched.
pub(crate) fn lex_certainly(source: &str) -> Result<Option<Vec<Token<'_>>>, JavaScriptParseError> {
    let lexed = lex_classified(source)?;
    Ok(lexed.slash_classification_certain.then_some(lexed.tokens))
}

pub(crate) fn lex_classified(source: &str) -> Result<LexedSource<'_>, JavaScriptParseError> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::with_capacity(source.len() / 2);
    let mut groups = Vec::<GroupKind>::new();
    // A program starts at statement position, where `/` opens a regex.
    let mut regex_allowed = true;
    let mut slash_classification_certain = true;
    let mut cursor = 0;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if byte.is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if source[cursor..].starts_with("//") {
            cursor += 2;
            while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                cursor += 1;
            }
            continue;
        }
        if source[cursor..].starts_with("/*") {
            let start = cursor;
            cursor += 2;
            while cursor + 1 < bytes.len() && &source[cursor..cursor + 2] != "*/" {
                cursor += 1;
            }
            if cursor + 1 >= bytes.len() {
                return Err(JavaScriptParseError {
                    offset: start,
                    message: "unterminated block comment",
                    context: None,
                });
            }
            cursor += 2;
            continue;
        }
        let start = cursor;
        let kind = if is_identifier_start(byte) {
            cursor += 1;
            while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
                cursor += 1;
            }
            let word = &source[start..cursor];
            if is_keyword(word) {
                // Only the value keywords end an expression. Every other
                // keyword introduces one, so a following `/` opens a regex.
                regex_allowed = !matches!(word, "this" | "super" | "true" | "false" | "null");
                TokenKind::Keyword
            } else {
                regex_allowed = false;
                TokenKind::Identifier
            }
        } else if byte.is_ascii_digit()
            || (byte == b'.' && bytes.get(cursor + 1).is_some_and(u8::is_ascii_digit))
        {
            cursor = scan_number(bytes, cursor);
            regex_allowed = false;
            TokenKind::Number
        } else if matches!(byte, b'\'' | b'"') {
            cursor = scan_quoted(bytes, cursor, byte)?;
            regex_allowed = false;
            TokenKind::String
        } else if byte == b'`' {
            cursor = scan_template(bytes, cursor)?;
            regex_allowed = false;
            TokenKind::Template
        } else if byte == b'/' && regex_allowed {
            match scan_regex(bytes, cursor) {
                Some(end) => {
                    cursor = end;
                    regex_allowed = false;
                    TokenKind::Regex
                }
                // A body that runs past a line terminator is not a regular
                // expression. Fall back to punctuation and record that this
                // `/` was not read with certainty.
                None => {
                    slash_classification_certain = false;
                    cursor += punctuation_width(&source[cursor..]);
                    regex_allowed = true;
                    TokenKind::Punct
                }
            }
        } else {
            let width = punctuation_width(&source[cursor..]);
            let text = &source[cursor..cursor + width];
            cursor += width;
            match text {
                "(" => {
                    let header = tokens.last().is_some_and(|token: &Token<'_>| {
                        token.kind == TokenKind::Keyword
                            && matches!(
                                token.text,
                                "if" | "while" | "for" | "with" | "switch" | "catch"
                            )
                    });
                    groups.push(if header {
                        GroupKind::ControlHeader
                    } else {
                        GroupKind::Expression
                    });
                    regex_allowed = true;
                }
                "[" => {
                    groups.push(GroupKind::Bracket);
                    regex_allowed = true;
                }
                "{" => {
                    groups.push(GroupKind::Brace);
                    regex_allowed = true;
                }
                ")" | "]" | "}" => {
                    let opened = groups.pop();
                    regex_allowed = match opened {
                        Some(GroupKind::ControlHeader) => true,
                        Some(GroupKind::Expression | GroupKind::Bracket) => false,
                        // A block's `}` sits at statement position, an object
                        // or function expression's `}` does not. Assume the
                        // statement reading and flag the stream only when a
                        // `/` actually depends on the guess.
                        Some(GroupKind::Brace) | None => {
                            if next_significant_byte(bytes, cursor) == Some(b'/') {
                                slash_classification_certain = false;
                            }
                            true
                        }
                    };
                }
                // A postfix update ends an expression; nothing else that this
                // arm can see does.
                "++" | "--" => {}
                _ => regex_allowed = true,
            }
            TokenKind::Punct
        };
        tokens.push(Token {
            kind,
            text: &source[start..cursor],
            start,
            end: cursor,
        });
    }
    Ok(LexedSource {
        tokens,
        slash_classification_certain,
    })
}

/// Skip whitespace and comments to report the next code byte after `cursor`.
pub(crate) fn next_significant_byte(bytes: &[u8], mut cursor: usize) -> Option<u8> {
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if byte.is_ascii_whitespace() {
            cursor += 1;
            continue;
        }
        if byte == b'/' && bytes.get(cursor + 1) == Some(&b'/') {
            cursor += 2;
            while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                cursor += 1;
            }
            continue;
        }
        if byte == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            cursor += 2;
            while cursor + 1 < bytes.len() && &bytes[cursor..cursor + 2] != b"*/" {
                cursor += 1;
            }
            cursor = (cursor + 2).min(bytes.len());
            continue;
        }
        return Some(byte);
    }
    None
}

/// Scan a regular-expression literal body and flags from its opening `/`.
///
/// `\` escapes the next character, a `[...]` class holds an unescaped `/`, and
/// a line terminator inside the body means this `/` was not a regex after all.
pub(crate) fn scan_regex(bytes: &[u8], start: usize) -> Option<usize> {
    let mut cursor = start + 1;
    let mut in_class = false;
    loop {
        let byte = *bytes.get(cursor)?;
        match byte {
            b'\n' | b'\r' => return None,
            b'\\' => {
                // A trailing backslash cannot escape a line terminator here.
                if matches!(bytes.get(cursor + 1), None | Some(b'\n') | Some(b'\r')) {
                    return None;
                }
                cursor += 2;
            }
            b'[' => {
                in_class = true;
                cursor += 1;
            }
            b']' => {
                in_class = false;
                cursor += 1;
            }
            b'/' if !in_class => {
                cursor += 1;
                break;
            }
            _ => cursor += 1,
        }
    }
    // An empty body lexes as a line comment, never as a regex.
    if cursor == start + 2 {
        return None;
    }
    while cursor < bytes.len() && is_identifier_continue(bytes[cursor]) {
        cursor += 1;
    }
    Some(cursor)
}

pub(crate) fn scan_quoted(
    bytes: &[u8],
    start: usize,
    quote: u8,
) -> Result<usize, JavaScriptParseError> {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            byte if byte == quote => return Ok(cursor + 1),
            b'\n' | b'\r' if quote != b'`' => {
                return Err(JavaScriptParseError {
                    offset: start,
                    message: "unterminated string literal",
                    context: None,
                });
            }
            _ => cursor += 1,
        }
    }
    Err(JavaScriptParseError {
        offset: start,
        message: "unterminated string literal",
        context: None,
    })
}

pub(crate) fn scan_template(bytes: &[u8], start: usize) -> Result<usize, JavaScriptParseError> {
    let mut cursor = start + 1;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            b'`' => return Ok(cursor + 1),
            b'$' if bytes.get(cursor + 1) == Some(&b'{') => {
                cursor = scan_template_expression(bytes, cursor + 2, start)?;
            }
            _ => cursor += 1,
        }
    }
    Err(JavaScriptParseError {
        offset: start,
        message: "unterminated template literal",
        context: None,
    })
}

pub(crate) fn scan_template_expression(
    bytes: &[u8],
    mut cursor: usize,
    template_start: usize,
) -> Result<usize, JavaScriptParseError> {
    let mut brace_depth = 1usize;
    while cursor < bytes.len() {
        match bytes[cursor] {
            b'\'' | b'"' => cursor = scan_quoted(bytes, cursor, bytes[cursor])?,
            b'`' => cursor = scan_template(bytes, cursor)?,
            b'{' => {
                brace_depth += 1;
                cursor += 1;
            }
            b'}' => {
                brace_depth -= 1;
                cursor += 1;
                if brace_depth == 0 {
                    return Ok(cursor);
                }
            }
            _ => cursor += 1,
        }
    }
    Err(JavaScriptParseError {
        offset: template_start,
        message: "unterminated template interpolation",
        context: None,
    })
}

pub(crate) fn scan_number(bytes: &[u8], start: usize) -> usize {
    let mut cursor = start;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        let exponent_sign = matches!(byte, b'+' | b'-')
            && cursor > start
            && matches!(bytes[cursor - 1], b'e' | b'E');
        if byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_') || exponent_sign {
            cursor += 1;
        } else {
            break;
        }
    }
    cursor
}

pub(crate) fn punctuation_width(source: &str) -> usize {
    let bytes = source.as_bytes();
    match bytes.first().copied() {
        Some(b'>') => match bytes.get(1).copied() {
            Some(b'>') => match bytes.get(2).copied() {
                Some(b'>') if bytes.get(3) == Some(&b'=') => 4,
                Some(b'>' | b'=') => 3,
                _ => 2,
            },
            Some(b'=') => 2,
            _ => 1,
        },
        Some(b'=') => match bytes.get(1).copied() {
            Some(b'=') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'=' | b'>') => 2,
            _ => 1,
        },
        Some(b'!') => match bytes.get(1).copied() {
            Some(b'=') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'=') => 2,
            _ => 1,
        },
        Some(b'*') => match bytes.get(1).copied() {
            Some(b'*') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'*' | b'=') => 2,
            _ => 1,
        },
        Some(b'<') => match bytes.get(1).copied() {
            Some(b'<') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'<' | b'=') => 2,
            _ => 1,
        },
        Some(b'&') => match bytes.get(1).copied() {
            Some(b'&') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'&' | b'=') => 2,
            _ => 1,
        },
        Some(b'|') => match bytes.get(1).copied() {
            Some(b'|') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'|' | b'=') => 2,
            _ => 1,
        },
        Some(b'?') => match bytes.get(1).copied() {
            Some(b'?') if bytes.get(2) == Some(&b'=') => 3,
            Some(b'?') => 2,
            _ => 1,
        },
        Some(b'+') => match bytes.get(1).copied() {
            Some(b'+' | b'=') => 2,
            _ => 1,
        },
        Some(b'-') => match bytes.get(1).copied() {
            Some(b'-' | b'=') => 2,
            _ => 1,
        },
        Some(b'/' | b'%' | b'^') => match bytes.get(1).copied() {
            Some(b'=') => 2,
            _ => 1,
        },
        Some(_) => source.chars().next().map_or(1, char::len_utf8),
        None => 1,
    }
}

pub(crate) const fn is_identifier_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || matches!(byte, b'_' | b'$')
}

pub(crate) const fn is_identifier_continue(byte: u8) -> bool {
    is_identifier_start(byte) || byte.is_ascii_digit()
}

pub(crate) fn is_keyword(identifier: &str) -> bool {
    matches!(
        identifier,
        "as" | "async"
            | "await"
            | "break"
            | "case"
            | "catch"
            | "class"
            | "const"
            | "continue"
            | "debugger"
            | "default"
            | "delete"
            | "do"
            | "else"
            | "export"
            | "extends"
            | "false"
            | "finally"
            | "for"
            | "from"
            | "function"
            | "get"
            | "if"
            | "import"
            | "in"
            | "instanceof"
            | "let"
            | "new"
            | "null"
            | "of"
            | "return"
            | "set"
            | "static"
            | "super"
            | "switch"
            | "this"
            | "throw"
            | "true"
            | "try"
            | "typeof"
            | "undefined"
            | "var"
            | "void"
            | "while"
            | "with"
            | "yield"
    )
}

/// Frames track the conditional operator per delimiter, because `?` and `:`
/// pair inside the innermost group and nowhere else.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ColonFrame {
    /// A `{` holding `key: value` pairs.
    ObjectLiteral,
    /// A `{` holding statements, where a `:` can only be a label or a `case`.
    Block,
    /// `(` or `[`.
    Group,
}

/// A `:` must answer a `?`, name a property, or label a statement. Anything
/// else is a malformed conditional, which balanced delimiters cannot detect:
/// `a||((b,c):d)?e:f` nests correctly and parses as nothing at all.
///
/// This is not a parser. It reports only the unambiguous failure -- a `:` with
/// no `?` to pair with, in a position where no other `:` is legal -- so a
/// candidate is never rejected for a spelling this does not model.
pub(crate) fn validate_conditional_operators(
    tokens: &[Token<'_>],
) -> Result<(), JavaScriptParseError> {
    let mut frames: Vec<(ColonFrame, usize)> = Vec::new();
    // Statement context for the innermost frame: whether a `case` is open, and
    // the index of the previous significant token.
    let mut case_open = vec![false];
    let mut previous: Option<usize> = None;

    for index in 0..tokens.len() {
        let text = tokens[index].text;
        match text {
            "(" | "[" => {
                frames.push((ColonFrame::Group, 0));
                case_open.push(false);
            }
            "{" => {
                let kind = if previous.is_some_and(|at| opens_object_literal(tokens, at)) {
                    ColonFrame::ObjectLiteral
                } else {
                    ColonFrame::Block
                };
                frames.push((kind, 0));
                case_open.push(false);
            }
            ")" | "]" | "}" => {
                if let Some((_, pending)) = frames.pop() {
                    if pending > 0 {
                        return Err(JavaScriptParseError {
                            offset: tokens[index].start,
                            message: "conditional `?` without `:`",
                            context: None,
                        });
                    }
                }
                case_open.pop();
                if case_open.is_empty() {
                    case_open.push(false);
                }
            }
            "?" => {
                // `?.` reaches the lexer as `?` then `.`; optional chaining is
                // not a conditional and has no `:` to pair with.
                let optional_chain = tokens
                    .get(index + 1)
                    .is_some_and(|next| next.text == "." && next.start == tokens[index].end);
                if optional_chain {
                    previous = Some(index);
                    continue;
                }
                if let Some((_, pending)) = frames.last_mut() {
                    *pending += 1;
                } else {
                    // Top level: track with a synthetic frame so the pairing
                    // still balances.
                    frames.push((ColonFrame::Group, 1));
                    case_open.push(false);
                }
            }
            "case" => {
                if let Some(open) = case_open.last_mut() {
                    *open = true;
                }
            }
            ";" => {
                if let Some(open) = case_open.last_mut() {
                    *open = false;
                }
            }
            ":" => {
                let pending = frames.last().map_or(0, |(_, pending)| *pending);
                if pending > 0 {
                    if let Some((_, pending)) = frames.last_mut() {
                        *pending -= 1;
                    }
                    if frames
                        .last()
                        .is_some_and(|(kind, pending)| *kind == ColonFrame::Group && *pending == 0)
                        && frames.len() > 1
                    {
                        // A synthetic top-level frame is finished with.
                    }
                    previous = Some(index);
                    continue;
                }
                let frame = frames.last().map(|(kind, _)| *kind);
                let labelled = case_open.last().copied().unwrap_or(false)
                    || previous.is_some_and(|at| tokens[at].text == "default")
                    || previous.is_some_and(|at| {
                        tokens[at].kind == TokenKind::Identifier
                            && at.checked_sub(1).is_none_or(|before| {
                                matches!(tokens[before].text, ";" | "{" | "}" | ")")
                            })
                    });
                let allowed = matches!(frame, Some(ColonFrame::ObjectLiteral)) || labelled;
                if !allowed {
                    return Err(JavaScriptParseError {
                        offset: tokens[index].start,
                        message: "`:` without a conditional `?`",
                        context: None,
                    });
                }
                if let Some(open) = case_open.last_mut() {
                    *open = false;
                }
            }
            _ => {}
        }
        previous = Some(index);
    }
    Ok(())
}

/// True when a `{` following this token opens an object literal rather than a
/// block. Everything that ends an expression is followed by a block; everything
/// that expects one is followed by a literal.
pub(crate) fn opens_object_literal(tokens: &[Token<'_>], at: usize) -> bool {
    matches!(
        tokens[at].text,
        "(" | "," | "[" | "=" | ":" | "?" | "return" | "&&" | "||" | "??" | "!" | "+" | "-" | "*"
            | "/" | "%" | "==" | "!=" | "===" | "!==" | "<" | ">" | "<=" | ">=" | "typeof" | "in"
            | "instanceof" | "new" | "case" | "..." | "of" | "void" | "delete" | "yield" | "await"
            | "+=" | "-=" | "*=" | "/=" | "|=" | "&=" | "^=" | "||=" | "&&=" | "??="
            // A `{` after a binding keyword is a destructuring pattern, which
            // spells `key: target` exactly as a literal does. `export default`
            // is followed by a value, so its `{` is a literal too.
            | "const" | "let" | "var" | "default"
    )
}

#[cfg(test)]
mod conditional_operator_tests {
    use super::{lex, validate_conditional_operators};

    fn check(source: &str) -> Result<(), &'static str> {
        let tokens = lex(source).expect("lexes");
        validate_conditional_operators(&tokens).map_err(|error| error.message)
    }

    #[test]
    fn accepts_ordinary_conditionals() {
        for source in [
            "var a=b?c:d;",
            "var a=b?c?d:e:f;",
            "f(a?b:c,d);",
            "var o={k:a?b:c,j:2};",
            "var o={a:1,b:{c:2}};",
            "a?(b,c):d;",
            "x=y||(z?1:2);",
        ] {
            assert_eq!(check(source), Ok(()), "{source}");
        }
    }

    #[test]
    fn accepts_colons_that_are_not_conditionals() {
        for source in [
            "switch(a){case 1:b();break;default:c()}",
            "outer:for(;;){break outer}",
            "const {\"~std\": x, ...rest}=y;",
            "function f({a:b}){return b}",
            "export default {a:1};",
            "var a=b?.c;",
            "var a=b?.c??d;",
            "var a=b?.c??d?e:f;",
        ] {
            assert_eq!(check(source), Ok(()), "{source}");
        }
    }

    /// The shape that shipped: parentheses balance, so the delimiter check sees
    /// nothing, and the program is not JavaScript.
    #[test]
    fn rejects_a_colon_with_no_question() {
        assert_eq!(
            check("var a=b&&c||((d,e):f&&g)?h:i;"),
            Err("`:` without a conditional `?`")
        );
    }

    #[test]
    fn rejects_a_question_with_no_colon() {
        assert_eq!(check("f(a?b);"), Err("conditional `?` without `:`"));
    }
}
