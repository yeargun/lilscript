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
    #[token("class")]
    Class,
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
    fn reports_unknown_input() {
        let err = lex("@").unwrap_err();
        assert_eq!(err.span, Span::new(0, 1));
    }
}
