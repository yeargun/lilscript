use logos::Logos;

use crate::span::Span;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\n\f\r]+")]
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
pub enum TokenKind<'src> {
    #[token("int")]
    Int,
    #[token("float")]
    Float,
    #[token("number")]
    Number,
    #[token("string")]
    String,
    #[token("bool")]
    Bool,
    #[token("void")]
    Void,
    #[token("auto")]
    Auto,
    #[token("func")]
    Func,
    #[token("struct")]
    Struct,
    #[token("record")]
    Record,
    #[token("enum")]
    Enum,
    #[token("class")]
    Class,
    #[token("extends")]
    Extends,
    #[token("super")]
    Super,
    #[token("return")]
    Return,
    #[token("init")]
    Init,
    #[token("if")]
    If,
    #[token("else")]
    Else,
    #[token("while")]
    While,
    #[token("for")]
    For,
    #[token("in")]
    In,
    #[token("of")]
    Of,
    #[token("break")]
    Break,
    #[token("continue")]
    Continue,
    #[token("extern")]
    Extern,
    #[token("import")]
    Import,
    #[token("export")]
    Export,
    #[token("from")]
    From,
    #[token("as")]
    As,
    #[token("pure")]
    Pure,
    #[token("true")]
    True,
    #[token("false")]
    False,
    #[token("null")]
    Null,
    #[token("new")]
    New,
    #[token("is")]
    Is,
    #[token("match")]
    Match,
    #[token("async")]
    Async,
    #[token("generator")]
    Generator,
    #[token("yield")]
    Yield,
    #[token("await")]
    Await,
    #[token("throw")]
    Throw,
    #[token("try")]
    Try,
    #[token("catch")]
    Catch,
    #[token("finally")]
    Finally,

    #[regex(r"[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok())]
    FloatLiteral(f64),
    #[regex(r"0|[1-9][0-9]*", |lex| lex.slice().parse::<i64>().ok())]
    IntLiteral(i64),
    #[regex(r#""([^"\\\n\r]|\\.)*""#, |lex| lex.slice())]
    StringLiteral(&'src str),
    #[regex(r#"`([^`\\\n\r]|\\.)*`"#, |lex| lex.slice())]
    TemplateLiteral(&'src str),
    #[regex(r"[A-Za-z_$][A-Za-z0-9_$]*", |lex| lex.slice())]
    Ident(&'src str),

    #[token("=>")]
    FatArrow,
    #[token("...")]
    Ellipsis,
    #[token("->")]
    ThinArrow,
    #[token("==")]
    EqEq,
    #[token("!=")]
    BangEq,
    #[token("<=")]
    LessEq,
    #[token(">=")]
    GreaterEq,
    #[token("&&")]
    AndAnd,
    #[token("||")]
    OrOr,
    #[token("??=")]
    QuestionQuestionEq,
    #[token("??")]
    QuestionQuestion,
    #[token("?.")]
    QuestionDot,
    #[token("++")]
    PlusPlus,
    #[token("--")]
    MinusMinus,
    #[token("+=")]
    PlusEq,
    #[token("-=")]
    MinusEq,
    #[token("*=")]
    StarEq,
    #[token("/=")]
    SlashEq,
    #[token("%=")]
    PercentEq,
    #[token("^=")]
    CaretEq,
    #[token("&=")]
    AmpersandEq,
    #[token("|=")]
    PipeEq,
    #[token("<<=")]
    ShiftLeftEq,
    #[token(">>=")]
    ShiftRightEq,
    #[token(">>>=")]
    UnsignedShiftRightEq,

    #[token("=")]
    Eq,
    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("^")]
    Caret,
    #[token("&")]
    Ampersand,
    #[token("<<")]
    ShiftLeft,
    #[token(">>")]
    ShiftRight,
    #[token(">>>")]
    UnsignedShiftRight,
    #[token("!")]
    Bang,
    #[token("<")]
    Less,
    #[token(">")]
    Greater,
    #[token(".")]
    Dot,
    #[token(",")]
    Comma,
    #[token(":")]
    Colon,
    #[token(";")]
    Semicolon,
    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("{")]
    LBrace,
    #[token("}")]
    RBrace,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token("?")]
    Question,
    #[token("|")]
    Pipe,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token<'src> {
    pub kind: TokenKind<'src>,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriviaKind {
    Whitespace,
    LineComment,
    BlockComment,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trivia<'src> {
    pub kind: TriviaKind,
    pub text: &'src str,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SyntaxElement<'src> {
    Token(Token<'src>),
    Trivia(Trivia<'src>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for LexError {}

pub fn lex(source: &str) -> Result<Vec<Token<'_>>, LexError> {
    let mut lexer = TokenKind::lexer(source);
    let mut tokens = Vec::new();

    while let Some(next) = lexer.next() {
        let span = Span::from(lexer.span());
        match next {
            Ok(kind) => tokens.push(Token { kind, span }),
            Err(_) => {
                return Err(LexError {
                    span,
                    message: "unrecognized token".to_string(),
                });
            }
        }
    }

    Ok(tokens)
}

/// Returns the same tokens as [`lex`] while retaining every skipped byte as
/// classified trivia. Token and trivia spans partition the complete source.
pub fn lex_lossless(source: &str) -> Result<Vec<SyntaxElement<'_>>, LexError> {
    let tokens = lex(source)?;
    let mut elements = Vec::with_capacity(tokens.len() * 2 + 1);
    let mut cursor = 0;
    for token in tokens {
        append_trivia(source, cursor, token.span.start, &mut elements)?;
        cursor = token.span.end;
        elements.push(SyntaxElement::Token(token));
    }
    append_trivia(source, cursor, source.len(), &mut elements)?;
    Ok(elements)
}

fn append_trivia<'src>(
    source: &'src str,
    start: usize,
    end: usize,
    elements: &mut Vec<SyntaxElement<'src>>,
) -> Result<(), LexError> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    while cursor < end {
        let item_start = cursor;
        let kind = if bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
            while cursor < end && bytes[cursor].is_ascii_whitespace() {
                cursor += 1;
            }
            TriviaKind::Whitespace
        } else if bytes[cursor..end].starts_with(b"//") {
            cursor += 2;
            while cursor < end && bytes[cursor] != b'\n' {
                cursor += 1;
            }
            TriviaKind::LineComment
        } else if bytes[cursor..end].starts_with(b"/*") {
            cursor += 2;
            while cursor + 1 < end && !bytes[cursor..end].starts_with(b"*/") {
                cursor += 1;
            }
            if cursor + 1 >= end {
                return Err(LexError {
                    span: Span::new(item_start, end),
                    message: "unterminated block comment".to_string(),
                });
            }
            cursor += 2;
            TriviaKind::BlockComment
        } else {
            return Err(LexError {
                span: Span::new(cursor, (cursor + 1).min(end)),
                message: "unclassified trivia".to_string(),
            });
        };
        elements.push(SyntaxElement::Trivia(Trivia {
            kind,
            text: &source[item_start..cursor],
            span: Span::new(item_start, cursor),
        }));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_core_tokens() {
        let tokens = lex(r#"struct Point { int x; string label = "p"; }"#).unwrap();
        assert!(matches!(&tokens[0].kind, TokenKind::Struct));
        assert!(matches!(&tokens[1].kind, TokenKind::Ident("Point")));
        assert!(tokens
            .iter()
            .any(|token| matches!(&token.kind, TokenKind::StringLiteral("\"p\""))));
    }

    #[test]
    fn lexes_bitwise_and_shift_assignments_longest_first() {
        let tokens = lex("a&=b;a|=b;a<<=b;a>>=b;a>>>=b;a>>>b;").unwrap();
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::AmpersandEq)));
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::PipeEq)));
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::ShiftLeftEq)));
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::ShiftRightEq)));
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::UnsignedShiftRightEq)));
        assert!(tokens
            .iter()
            .any(|token| matches!(token.kind, TokenKind::UnsignedShiftRight)));
    }

    #[test]
    fn reports_unknown_input() {
        let err = lex("@").unwrap_err();
        assert_eq!(err.span, Span::new(0, 1));
    }

    #[test]
    fn lossless_lexing_partitions_comments_and_whitespace() {
        let source = "int x = 1; // value\n/* tail */";
        let elements = lex_lossless(source).unwrap();
        let rebuilt = elements
            .iter()
            .map(|element| match element {
                SyntaxElement::Token(token) => &source[token.span.start..token.span.end],
                SyntaxElement::Trivia(trivia) => trivia.text,
            })
            .collect::<String>();
        assert_eq!(rebuilt, source);
        assert!(elements.iter().any(|element| matches!(
            element,
            SyntaxElement::Trivia(Trivia {
                kind: TriviaKind::LineComment,
                ..
            })
        )));
    }
}
