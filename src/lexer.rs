use logos::Logos;

use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use crate::span::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind<'src> {
    Int,
    Float,
    Number,
    String,
    Bool,
    Void,
    Auto,
    Func,
    Struct,
    Record,
    Enum,
    Class,
    Extends,
    Super,
    Return,
    Init,
    If,
    Else,
    While,
    For,
    In,
    Of,
    Break,
    Continue,
    Extern,
    Import,
    Export,
    From,
    As,
    Pure,
    True,
    False,
    Null,
    New,
    Is,
    Match,
    Async,
    Generator,
    Yield,
    Await,
    Throw,
    Try,
    Catch,
    Finally,

    FloatLiteral(f64),
    IntLiteral(i64),
    StringLiteral(&'src str),
    TemplateLiteral(TemplateId),
    Ident(&'src str),

    FatArrow,
    Ellipsis,
    ThinArrow,
    EqEq,
    BangEq,
    LessEq,
    GreaterEq,
    AndAnd,
    OrOr,
    QuestionQuestionEq,
    QuestionQuestion,
    QuestionDot,
    PlusPlus,
    MinusMinus,
    PlusEq,
    MinusEq,
    StarEq,
    SlashEq,
    PercentEq,
    CaretEq,
    AmpersandEq,
    PipeEq,
    ShiftLeftEq,
    ShiftRightEq,
    UnsignedShiftRightEq,

    Eq,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Ampersand,
    ShiftLeft,
    ShiftRight,
    UnsignedShiftRight,
    Bang,
    Less,
    Greater,
    Dot,
    Comma,
    Colon,
    Semicolon,
    LParen,
    RParen,
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    Question,
    At,
    Pipe,
}

// One generated grammar serves both inspection and admitted lexing. Template
// boundaries are resolved outside its callbacks so admission borrows stay local.
#[derive(Logos)]
#[logos(extras = Vec<TemplateToken>)]
#[logos(skip r"[ \t\n\f\r]+")]
#[logos(skip(r"//[^\n]*", allow_greedy = true))]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
enum RawToken<'src> {
    #[token("int", |_| TokenKind::Int)]
    #[token("float", |_| TokenKind::Float)]
    #[token("number", |_| TokenKind::Number)]
    #[token("string", |_| TokenKind::String)]
    #[token("bool", |_| TokenKind::Bool)]
    #[token("void", |_| TokenKind::Void)]
    #[token("auto", |_| TokenKind::Auto)]
    #[token("func", |_| TokenKind::Func)]
    #[token("struct", |_| TokenKind::Struct)]
    #[token("record", |_| TokenKind::Record)]
    #[token("enum", |_| TokenKind::Enum)]
    #[token("class", |_| TokenKind::Class)]
    #[token("extends", |_| TokenKind::Extends)]
    #[token("super", |_| TokenKind::Super)]
    #[token("return", |_| TokenKind::Return)]
    #[token("init", |_| TokenKind::Init)]
    #[token("if", |_| TokenKind::If)]
    #[token("else", |_| TokenKind::Else)]
    #[token("while", |_| TokenKind::While)]
    #[token("for", |_| TokenKind::For)]
    #[token("in", |_| TokenKind::In)]
    #[token("of", |_| TokenKind::Of)]
    #[token("break", |_| TokenKind::Break)]
    #[token("continue", |_| TokenKind::Continue)]
    #[token("extern", |_| TokenKind::Extern)]
    #[token("import", |_| TokenKind::Import)]
    #[token("export", |_| TokenKind::Export)]
    #[token("from", |_| TokenKind::From)]
    #[token("as", |_| TokenKind::As)]
    #[token("pure", |_| TokenKind::Pure)]
    #[token("true", |_| TokenKind::True)]
    #[token("false", |_| TokenKind::False)]
    #[token("null", |_| TokenKind::Null)]
    #[token("new", |_| TokenKind::New)]
    #[token("is", |_| TokenKind::Is)]
    #[token("match", |_| TokenKind::Match)]
    #[token("async", |_| TokenKind::Async)]
    #[token("generator", |_| TokenKind::Generator)]
    #[token("yield", |_| TokenKind::Yield)]
    #[token("await", |_| TokenKind::Await)]
    #[token("throw", |_| TokenKind::Throw)]
    #[token("try", |_| TokenKind::Try)]
    #[token("catch", |_| TokenKind::Catch)]
    #[token("finally", |_| TokenKind::Finally)]
    #[regex(r"[0-9]+\.[0-9]+([eE][+-]?[0-9]+)?", |lex| lex.slice().parse::<f64>().ok().map(TokenKind::FloatLiteral))]
    #[regex(r"0|[1-9][0-9]*", |lex| lex.slice().parse::<i64>().ok().map(TokenKind::IntLiteral))]
    #[regex(r#""([^"\\\n\r]|\\.)*""#, |lex| TokenKind::StringLiteral(lex.slice()))]
    #[regex(r"[A-Za-z_$][A-Za-z0-9_$]*", |lex| TokenKind::Ident(lex.slice()))]
    #[token("=>", |_| TokenKind::FatArrow)]
    #[token("...", |_| TokenKind::Ellipsis)]
    #[token("->", |_| TokenKind::ThinArrow)]
    #[token("==", |_| TokenKind::EqEq)]
    #[token("!=", |_| TokenKind::BangEq)]
    #[token("<=", |_| TokenKind::LessEq)]
    #[token(">=", |_| TokenKind::GreaterEq)]
    #[token("&&", |_| TokenKind::AndAnd)]
    #[token("||", |_| TokenKind::OrOr)]
    #[token("??=", |_| TokenKind::QuestionQuestionEq)]
    #[token("??", |_| TokenKind::QuestionQuestion)]
    #[token("?.", |_| TokenKind::QuestionDot)]
    #[token("++", |_| TokenKind::PlusPlus)]
    #[token("--", |_| TokenKind::MinusMinus)]
    #[token("+=", |_| TokenKind::PlusEq)]
    #[token("-=", |_| TokenKind::MinusEq)]
    #[token("*=", |_| TokenKind::StarEq)]
    #[token("/=", |_| TokenKind::SlashEq)]
    #[token("%=", |_| TokenKind::PercentEq)]
    #[token("^=", |_| TokenKind::CaretEq)]
    #[token("&=", |_| TokenKind::AmpersandEq)]
    #[token("|=", |_| TokenKind::PipeEq)]
    #[token("<<=", |_| TokenKind::ShiftLeftEq)]
    #[token(">>=", |_| TokenKind::ShiftRightEq)]
    #[token(">>>=", |_| TokenKind::UnsignedShiftRightEq)]
    #[token("=", |_| TokenKind::Eq)]
    #[token("+", |_| TokenKind::Plus)]
    #[token("-", |_| TokenKind::Minus)]
    #[token("*", |_| TokenKind::Star)]
    #[token("/", |_| TokenKind::Slash)]
    #[token("%", |_| TokenKind::Percent)]
    #[token("^", |_| TokenKind::Caret)]
    #[token("&", |_| TokenKind::Ampersand)]
    #[token("<<", |_| TokenKind::ShiftLeft)]
    #[token(">>", |_| TokenKind::ShiftRight)]
    #[token(">>>", |_| TokenKind::UnsignedShiftRight)]
    #[token("!", |_| TokenKind::Bang)]
    #[token("<", |_| TokenKind::Less)]
    #[token(">", |_| TokenKind::Greater)]
    #[token(".", |_| TokenKind::Dot)]
    #[token(",", |_| TokenKind::Comma)]
    #[token(":", |_| TokenKind::Colon)]
    #[token(";", |_| TokenKind::Semicolon)]
    #[token("(", |_| TokenKind::LParen)]
    #[token(")", |_| TokenKind::RParen)]
    #[token("{", |_| TokenKind::LBrace)]
    #[token("}", |_| TokenKind::RBrace)]
    #[token("[", |_| TokenKind::LBracket)]
    #[token("]", |_| TokenKind::RBracket)]
    #[token("?", |_| TokenKind::Question)]
    #[token("@", |_| TokenKind::At)]
    #[token("|", |_| TokenKind::Pipe)]
    Token(TokenKind<'src>),
    #[token("`")]
    TemplateStart,
}

impl<'src> Logos<'src> for TokenKind<'src> {
    type Extras = Vec<TemplateToken>;
    type Source = str;
    type Error = ();

    fn lex(lexer: &mut logos::Lexer<'src, Self>) -> Option<Result<Self, Self::Error>> {
        let current = std::mem::replace(lexer, Self::lexer(""));
        let mut raw = current.morph::<RawToken<'src>>();
        let result = RawToken::lex(&mut raw).map(|next| match next {
            Ok(RawToken::Token(kind)) => Ok(kind),
            Ok(RawToken::TemplateStart) => scan_template(&mut raw, None)
                .expect("inspection template scanner has no resource limit")
                .map(Self::TemplateLiteral)
                .ok_or(()),
            Err(error) => Err(error),
        });
        *lexer = raw.morph::<Self>();
        result
    }
}

/// A position in the owning token stream's template storage. Ordinary tokens
/// have no reference-counted payload or destructor; lookahead copies handles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TemplateId(usize);

/// Boundary information belongs to lexical analysis. Offsets are relative to
/// `raw`, and the owning stream retains the payload through parsing/formatting.
#[derive(Debug, PartialEq)]
pub struct TemplateToken {
    span: Span,
    pub expressions: Box<[Span]>,
}

#[derive(Debug, PartialEq)]
pub struct Lexed<'src, T> {
    source: &'src str,
    items: Vec<T>,
    templates: Vec<TemplateToken>,
}

impl<'src, T> Lexed<'src, T> {
    pub fn template(&self, id: TemplateId) -> (&'src str, &[Span]) {
        let template = &self.templates[id.0];
        (
            &self.source[template.span.start..template.span.end],
            &template.expressions,
        )
    }
}

impl<T> std::ops::Deref for Lexed<'_, T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        &self.items
    }
}

impl<T> std::ops::DerefMut for Lexed<'_, T> {
    fn deref_mut(&mut self) -> &mut [T] {
        &mut self.items
    }
}

impl<'a, T> IntoIterator for &'a Lexed<'_, T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.items.iter()
    }
}

#[derive(Clone, Copy)]
enum TemplateFrame {
    Text,
    Expression { start: usize, depth: usize },
}

struct TemplateScratch<'budget, 'ledger> {
    frames: Vec<TemplateFrame>,
    expressions: Vec<Span>,
    boxed: Option<Box<[Span]>>,
    budget: Option<&'budget mut AllocationBudget<'ledger>>,
}

impl TemplateScratch<'_, '_> {
    fn advance(&mut self, cursor: &mut usize, count: usize) -> Result<(), AllocationError> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.work(WorkKind::Analysis, count as u64)?;
        }
        *cursor = cursor.checked_add(count).ok_or(AllocationError::Capacity)?;
        Ok(())
    }

    fn frame(&mut self, frame: TemplateFrame) -> Result<(), AllocationError> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.push(AllocationClass::Scratch, &mut self.frames, frame)
        } else {
            self.frames.push(frame);
            Ok(())
        }
    }

    fn expression(&mut self, span: Span) -> Result<(), AllocationError> {
        if let Some(budget) = self.budget.as_deref_mut() {
            budget.push(AllocationClass::Scratch, &mut self.expressions, span)
        } else {
            self.expressions.push(span);
            Ok(())
        }
    }

    fn publish(
        mut self,
        templates: &mut Vec<TemplateToken>,
        span: Span,
    ) -> Result<TemplateId, AllocationError> {
        if let Some(budget) = self.budget.as_deref_mut() {
            // The exact destination is admitted while the growing source Vec
            // stays live. Equal length/capacity makes boxing allocation-free.
            let exact = budget.copy_slice(AllocationClass::Scratch, &self.expressions)?;
            self.boxed = Some(exact.into_boxed_slice());
            budget.work(WorkKind::Render, 1)?;
            budget.reserve_vec(AllocationClass::Scratch, templates, 1)?;
        } else {
            self.boxed = Some(std::mem::take(&mut self.expressions).into_boxed_slice());
        }
        let id = TemplateId(templates.len());
        templates.push(TemplateToken {
            span,
            expressions: self.boxed.take().expect("completed template expressions"),
        });
        Ok(id)
    }
}

impl Drop for TemplateScratch<'_, '_> {
    fn drop(&mut self) {
        let bytes = vector_capacity_bytes(&self.frames)
            + vector_capacity_bytes(&self.expressions)
            + self
                .boxed
                .as_ref()
                .map_or(0, |spans| std::mem::size_of_val(&**spans) as u64);
        drop(std::mem::take(&mut self.frames));
        drop(std::mem::take(&mut self.expressions));
        drop(self.boxed.take());
        if let Some(budget) = self.budget.as_deref_mut() {
            budget
                .release(AllocationClass::Scratch, bytes)
                .expect("template scratch owns its admitted backing");
        }
    }
}

fn scan_template<'src>(
    lex: &mut logos::Lexer<'src, RawToken<'src>>,
    budget: Option<&mut AllocationBudget<'_>>,
) -> Result<Option<TemplateId>, AllocationError> {
    let start = lex.span().start;
    let source = &lex.source()[start..];
    let bytes = source.as_bytes();
    let mut scratch = TemplateScratch {
        frames: Vec::new(),
        expressions: Vec::new(),
        boxed: None,
        budget,
    };
    scratch.frame(TemplateFrame::Text)?;
    let mut cursor = 1;
    while cursor < bytes.len() {
        match *scratch.frames.last().expect("open template frame") {
            TemplateFrame::Text => match bytes[cursor] {
                b'\\' => scratch.advance(&mut cursor, 2)?,
                b'`' => {
                    scratch.advance(&mut cursor, 1)?;
                    scratch.frames.pop();
                    if scratch.frames.is_empty() {
                        let id =
                            scratch.publish(&mut lex.extras, Span::new(start, start + cursor))?;
                        lex.bump(cursor - 1);
                        return Ok(Some(id));
                    }
                }
                b'$' if bytes.get(cursor + 1) == Some(&b'{') => {
                    scratch.advance(&mut cursor, 2)?;
                    scratch.frame(TemplateFrame::Expression {
                        start: cursor,
                        depth: 1,
                    })?;
                }
                _ => scratch.advance(&mut cursor, 1)?,
            },
            TemplateFrame::Expression { start, depth } => match bytes[cursor] {
                b'"' => {
                    scratch.advance(&mut cursor, 1)?;
                    loop {
                        let Some(byte) = bytes.get(cursor) else {
                            return Ok(None);
                        };
                        match *byte {
                            b'\\' => scratch.advance(&mut cursor, 2)?,
                            b'"' => {
                                scratch.advance(&mut cursor, 1)?;
                                break;
                            }
                            _ => scratch.advance(&mut cursor, 1)?,
                        }
                    }
                }
                b'/' if bytes.get(cursor + 1) == Some(&b'/') => {
                    scratch.advance(&mut cursor, 2)?;
                    while cursor < bytes.len() && !matches!(bytes[cursor], b'\n' | b'\r') {
                        scratch.advance(&mut cursor, 1)?;
                    }
                }
                b'/' if bytes.get(cursor + 1) == Some(&b'*') => {
                    scratch.advance(&mut cursor, 2)?;
                    loop {
                        let Some(pair) = bytes.get(cursor..cursor + 2) else {
                            return Ok(None);
                        };
                        if pair == b"*/" {
                            break;
                        }
                        scratch.advance(&mut cursor, 1)?;
                    }
                    scratch.advance(&mut cursor, 2)?;
                }
                b'`' => {
                    scratch.frame(TemplateFrame::Text)?;
                    scratch.advance(&mut cursor, 1)?;
                }
                b'{' => {
                    *scratch.frames.last_mut().unwrap() = TemplateFrame::Expression {
                        start,
                        depth: depth.checked_add(1).ok_or(AllocationError::Capacity)?,
                    };
                    scratch.advance(&mut cursor, 1)?;
                }
                b'}' => {
                    if depth == 1 {
                        scratch.frames.pop();
                        if scratch.frames.len() == 1 {
                            scratch.expression(Span::new(start, cursor))?;
                        }
                    } else {
                        *scratch.frames.last_mut().unwrap() = TemplateFrame::Expression {
                            start,
                            depth: depth - 1,
                        };
                    }
                    scratch.advance(&mut cursor, 1)?;
                }
                _ => scratch.advance(&mut cursor, 1)?,
            },
        }
    }
    Ok(None)
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmittedLexError {
    Syntax(LexError),
    Resource(AllocationError),
}

pub fn lex(source: &str) -> Result<Lexed<'_, Token<'_>>, LexError> {
    match lex_core(source, None) {
        Ok((tokens, _)) => Ok(tokens),
        Err(AdmittedLexError::Syntax(error)) => Err(error),
        Err(AdmittedLexError::Resource(_)) => {
            unreachable!("inspection lexer has no resource owner")
        }
    }
}

/// Tokens, template payloads and scanner scratch share the caller's resource
/// owner. Each stream prepays its input-byte tariff; Logos calls themselves
/// are not preemptible. Diagnostic strings remain unmetered.
pub(crate) fn lex_admitted<'src>(
    source: &'src str,
    budget: &mut AllocationBudget<'_>,
) -> Result<(Lexed<'src, Token<'src>>, u64), AdmittedLexError> {
    lex_core(source, Some(budget))
}

struct TokenBuffer<'budget, 'ledger, 'src> {
    lexer: Option<logos::Lexer<'src, RawToken<'src>>>,
    items: Option<Vec<Token<'src>>>,
    budget: Option<&'budget mut AllocationBudget<'ledger>>,
}

impl<'budget, 'ledger, 'src> TokenBuffer<'budget, 'ledger, 'src> {
    fn new(source: &'src str, budget: Option<&'budget mut AllocationBudget<'ledger>>) -> Self {
        Self {
            lexer: Some(RawToken::lexer(source)),
            items: Some(Vec::new()),
            budget,
        }
    }

    fn next(&mut self) -> Result<Option<Result<TokenKind<'src>, ()>>, AdmittedLexError> {
        let lexer = self.lexer.as_mut().expect("live raw lexer");
        let next = lexer.next();
        if let Some(budget) = self.budget.as_deref_mut() {
            budget
                .work(WorkKind::Analysis, 0)
                .map_err(AdmittedLexError::Resource)?;
        }
        Ok(match next {
            Some(Ok(RawToken::Token(kind))) => Some(Ok(kind)),
            Some(Ok(RawToken::TemplateStart)) => Some(
                scan_template(lexer, self.budget.as_deref_mut())
                    .map_err(AdmittedLexError::Resource)?
                    .map(TokenKind::TemplateLiteral)
                    .ok_or(()),
            ),
            Some(Err(error)) => Some(Err(error)),
            None => None,
        })
    }

    fn push(&mut self, token: Token<'src>) -> Result<(), AdmittedLexError> {
        let items = self.items.as_mut().expect("live lexical token buffer");
        if let Some(budget) = self.budget.as_deref_mut() {
            budget
                .push(AllocationClass::Scratch, items, token)
                .map_err(AdmittedLexError::Resource)
        } else {
            items.push(token);
            Ok(())
        }
    }

    fn into_parts(mut self) -> (Lexed<'src, Token<'src>>, u64) {
        let items = self.items.take().expect("live lexical token buffer");
        let lexer = self.lexer.take().expect("live raw lexer");
        let bytes = token_bytes(&items) + template_bytes(&lexer.extras);
        (
            Lexed {
                source: lexer.source(),
                items,
                templates: lexer.extras,
            },
            bytes,
        )
    }
}

impl Drop for TokenBuffer<'_, '_, '_> {
    fn drop(&mut self) {
        let bytes = self.items.as_ref().map_or(0, token_bytes)
            + self
                .lexer
                .as_ref()
                .map_or(0, |lexer| template_bytes(&lexer.extras));
        drop(self.items.take());
        drop(self.lexer.take());
        if let Some(budget) = self.budget.as_deref_mut() {
            budget
                .release(AllocationClass::Scratch, bytes)
                .expect("lexical token and template storage owns its capacity");
        }
    }
}

fn vector_capacity_bytes<T>(items: &Vec<T>) -> u64 {
    // Vec capacity already satisfies the platform's valid allocation layout.
    (items.capacity() * std::mem::size_of::<T>()) as u64
}

fn token_bytes(items: &Vec<Token<'_>>) -> u64 {
    vector_capacity_bytes(items)
}

fn template_bytes(templates: &Vec<TemplateToken>) -> u64 {
    vector_capacity_bytes(templates)
        + templates
            .iter()
            .map(|template| std::mem::size_of_val(&*template.expressions) as u64)
            .sum::<u64>()
}

fn lex_core<'src>(
    source: &'src str,
    budget: Option<&mut AllocationBudget<'_>>,
) -> Result<(Lexed<'src, Token<'src>>, u64), AdmittedLexError> {
    let mut tokens = TokenBuffer::new(source, budget);
    if let Some(budget) = tokens.budget.as_deref_mut() {
        budget
            .work(WorkKind::Analysis, source.len() as u64)
            .map_err(AdmittedLexError::Resource)?;
    }

    while let Some(next) = tokens.next()? {
        let span = Span::from(tokens.lexer.as_ref().unwrap().span());
        match next {
            Ok(kind) => tokens.push(Token { kind, span })?,
            Err(_) => {
                return Err(AdmittedLexError::Syntax(LexError {
                    span,
                    message: "unrecognized token".to_string(),
                }));
            }
        }
    }
    Ok(tokens.into_parts())
}

/// Returns the same tokens as [`lex`] while retaining every skipped byte as
/// classified trivia. Token and trivia spans partition the complete source.
pub fn lex_lossless(source: &str) -> Result<Lexed<'_, SyntaxElement<'_>>, LexError> {
    let Lexed {
        items: tokens,
        templates,
        ..
    } = lex(source)?;
    let mut elements = Vec::with_capacity(tokens.len() * 2 + 1);
    let mut cursor = 0;
    for token in tokens {
        append_trivia(source, cursor, token.span.start, &mut elements)?;
        cursor = token.span.end;
        elements.push(SyntaxElement::Token(token));
    }
    append_trivia(source, cursor, source.len(), &mut elements)?;
    Ok(Lexed {
        source,
        items: elements,
        templates,
    })
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
#[path = "lexer_admission_tests.rs"]
mod admission_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_templates_retain_boundaries_across_comments_quotes_and_physical_lines() {
        let source = " /* prefix */ `first\r\n${`nested ${value}`} ${\"}\" /* } ` */} ${(() => { // } `\nreturn 3; })()}`;";
        let tokens = lex(source).unwrap();
        let TokenKind::TemplateLiteral(id) = tokens[0].kind else {
            panic!("template token missing")
        };
        let (raw, expressions) = tokens.template(id);
        let parts: Vec<_> = expressions
            .iter()
            .map(|span| &raw[span.start..span.end])
            .collect();
        assert_eq!(
            parts,
            [
                "`nested ${value}`",
                "\"}\" /* } ` */",
                "(() => { // } `\nreturn 3; })()"
            ]
        );
        assert_eq!(tokens[1].kind, TokenKind::Semicolon);
        let rebuilt = lex_lossless(source)
            .unwrap()
            .iter()
            .map(|part| match part {
                SyntaxElement::Token(token) => &source[token.span.start..token.span.end],
                SyntaxElement::Trivia(trivia) => trivia.text,
            })
            .collect::<String>();
        assert_eq!(rebuilt, source);
    }

    #[test]
    fn malformed_template_nesting_fails_without_consuming_a_partial_literal() {
        for source in [
            "`missing",
            "`missing ${1`",
            "`missing ${\"quote}",
            "`missing ${ /* comment }`",
            "`missing ${`nested`",
        ] {
            assert!(lex(source).is_err(), "{source}");
        }
    }
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
        let err = lex("\u{5c}").unwrap_err();
        assert_eq!(err.span, Span::new(0, 1));
    }

    #[test]
    fn lexes_the_region_attribute_sigil() {
        let tokens = lex("@pool int f() { return 0; }").unwrap();
        assert!(matches!(tokens[0].kind, TokenKind::At));
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
