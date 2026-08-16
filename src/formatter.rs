use crate::config::{FormatConfig, NewlineStyle};
use crate::lexer::{lex_lossless, SyntaxElement, TokenKind, TriviaKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    pub message: String,
}

impl std::fmt::Display for FormatError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for FormatError {}

pub fn format_source(source: &str, config: &FormatConfig) -> Result<String, FormatError> {
    let elements = lex_lossless(source).map_err(|error| FormatError {
        message: error.to_string(),
    })?;
    let mut printer = Printer::new(config.line_width);
    for element in &elements {
        match element {
            SyntaxElement::Trivia(trivia) => match trivia.kind {
                TriviaKind::Whitespace => {}
                TriviaKind::LineComment => printer.line_comment(trivia.text),
                TriviaKind::BlockComment => printer.block_comment(trivia.text),
            },
            SyntaxElement::Token(token) => {
                printer.token(&token.kind, &source[token.span.start..token.span.end]);
            }
        }
    }
    let mut output = printer.finish();
    if config.organize_imports {
        output = organize_import_lines(&output);
    }
    if config.newline == NewlineStyle::Crlf {
        output = output.replace('\n', "\r\n");
    }
    Ok(output)
}

fn organize_import_lines(source: &str) -> String {
    let mut lines = source.lines().collect::<Vec<_>>();
    let Some(first) = lines.iter().position(|line| line.starts_with("import ")) else {
        return source.to_string();
    };
    let mut end = first;
    while end < lines.len() && (lines[end].starts_with("import ") || lines[end].is_empty()) {
        end += 1;
    }
    if lines[first..end]
        .iter()
        .any(|line| line.contains("//") || line.contains("/*"))
    {
        return source.to_string();
    }
    let mut imports = lines[first..end]
        .iter()
        .copied()
        .filter(|line| line.starts_with("import "))
        .collect::<Vec<_>>();
    imports.sort_unstable();
    imports.dedup();
    lines.splice(first..end, imports);
    let mut output = lines.join("\n");
    output.push('\n');
    output
}

struct Printer {
    output: String,
    indent: usize,
    line_start: bool,
    line_len: usize,
    paren_depth: usize,
    bracket_depth: usize,
    previous: Option<TokenClass>,
    previous_closed_inline: Option<bool>,
    previous_word: Option<String>,
    previous_prefix_update: bool,
    line_width: usize,
    inline_braces: Vec<bool>,
    in_import: bool,
    declaration_block_pending: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TokenClass {
    Word,
    Literal,
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    OpenBrace,
    CloseBrace,
    Dot,
    Comma,
    Colon,
    Semicolon,
    Operator,
}

impl Printer {
    fn new(line_width: usize) -> Self {
        Self {
            output: String::new(),
            indent: 0,
            line_start: true,
            line_len: 0,
            paren_depth: 0,
            bracket_depth: 0,
            previous: None,
            previous_closed_inline: None,
            previous_word: None,
            previous_prefix_update: false,
            line_width,
            inline_braces: Vec::new(),
            in_import: false,
            declaration_block_pending: false,
        }
    }

    fn token(&mut self, kind: &TokenKind<'_>, text: &str) {
        let class = token_class(kind);
        let prefix_update = matches!(text, "++" | "--") && self.update_is_prefix();
        if self.previous == Some(TokenClass::CloseBrace)
            && self.previous_closed_inline == Some(false)
            && text != "else"
            && class != TokenClass::CloseBrace
        {
            self.newline();
        }
        let closing_inline =
            class == TokenClass::CloseBrace && self.inline_braces.last().copied().unwrap_or(false);
        if class == TokenClass::CloseBrace && !closing_inline {
            self.indent = self.indent.saturating_sub(1);
            self.newline();
        }
        if self.needs_space(class, text) {
            self.space();
        }
        self.write(text);
        let mut closed_inline = None;
        match class {
            TokenClass::OpenParen => self.paren_depth += 1,
            TokenClass::CloseParen => self.paren_depth = self.paren_depth.saturating_sub(1),
            TokenClass::OpenBracket => self.bracket_depth += 1,
            TokenClass::CloseBracket => self.bracket_depth = self.bracket_depth.saturating_sub(1),
            TokenClass::OpenBrace => {
                let inline = self.in_import
                    || (!self.declaration_block_pending
                        && !matches!(self.previous, Some(TokenClass::CloseParen))
                        && !matches!(self.previous_word.as_deref(), Some("else")));
                self.inline_braces.push(inline);
                if inline {
                    self.space();
                } else {
                    self.indent += 1;
                    self.newline();
                }
                self.declaration_block_pending = false;
            }
            TokenClass::CloseBrace => {
                let inline = self.inline_braces.pop().unwrap_or(false);
                closed_inline = Some(inline);
                if inline {
                    while self.output.ends_with(' ') {
                        self.output.pop();
                        self.line_len = self.line_len.saturating_sub(1);
                    }
                }
            }
            TokenClass::Semicolon if self.paren_depth == 0 => {
                self.in_import = false;
                self.newline();
            }
            TokenClass::Comma
                if self.line_len >= self.line_width
                    && self.paren_depth + self.bracket_depth > 0 =>
            {
                self.newline();
            }
            _ => {}
        }
        self.previous = Some(class);
        self.previous_closed_inline = closed_inline;
        self.previous_word = (class == TokenClass::Word).then(|| text.to_string());
        self.previous_prefix_update = prefix_update;
        if matches!(kind, TokenKind::Import) {
            self.in_import = true;
        }
        if matches!(kind, TokenKind::Struct | TokenKind::Class) {
            self.declaration_block_pending = true;
        }
    }

    fn needs_space(&self, current: TokenClass, text: &str) -> bool {
        let Some(previous) = self.previous else {
            return false;
        };
        if self.line_start {
            return false;
        }
        if matches!(text, "++" | "--") && !self.update_is_prefix() {
            return false;
        }
        if self.previous_prefix_update {
            return false;
        }
        if text == "else" && previous == TokenClass::CloseBrace {
            return true;
        }
        if current == TokenClass::CloseBrace && self.inline_braces.last().copied().unwrap_or(false)
        {
            return true;
        }
        if previous == TokenClass::CloseBrace && current == TokenClass::Word {
            return true;
        }
        if current == TokenClass::OpenParen {
            return self
                .previous_word
                .as_deref()
                .is_some_and(|word| matches!(word, "if" | "for" | "while"));
        }
        if matches!(
            current,
            TokenClass::CloseParen
                | TokenClass::CloseBracket
                | TokenClass::Comma
                | TokenClass::Semicolon
                | TokenClass::Dot
        ) || matches!(
            previous,
            TokenClass::OpenParen | TokenClass::OpenBracket | TokenClass::Dot
        ) {
            return false;
        }
        if current == TokenClass::OpenBracket {
            return false;
        }
        if current == TokenClass::OpenBrace {
            return self.in_import
                || self.declaration_block_pending
                || matches!(previous, TokenClass::CloseParen)
                || matches!(self.previous_word.as_deref(), Some("else"));
        }
        if previous == TokenClass::OpenBrace && self.inline_braces.last().copied().unwrap_or(false)
        {
            return false;
        }
        matches!(current, TokenClass::Operator)
            || matches!(
                previous,
                TokenClass::Operator | TokenClass::Comma | TokenClass::Colon
            )
            || matches!(
                (previous, current),
                (
                    TokenClass::Word | TokenClass::Literal,
                    TokenClass::Word | TokenClass::Literal
                )
            )
    }

    fn update_is_prefix(&self) -> bool {
        self.previous.is_none()
            || self.line_start
            || matches!(
                self.previous,
                Some(
                    TokenClass::OpenParen
                        | TokenClass::OpenBracket
                        | TokenClass::OpenBrace
                        | TokenClass::Comma
                        | TokenClass::Colon
                        | TokenClass::Semicolon
                        | TokenClass::Operator
                )
            )
            || self.previous_word.as_deref() == Some("return")
    }

    fn line_comment(&mut self, text: &str) {
        if !self.line_start {
            self.space();
        }
        self.write(text.trim_end());
        self.newline();
    }

    fn block_comment(&mut self, text: &str) {
        if !self.line_start {
            self.space();
        }
        if text.contains('\n') {
            for (index, line) in text.lines().enumerate() {
                if index != 0 {
                    self.newline();
                }
                self.write(line.trim());
            }
            self.newline();
        } else {
            self.write(text);
            self.space();
        }
    }

    fn write(&mut self, text: &str) {
        if self.line_start {
            for _ in 0..self.indent {
                self.output.push_str("  ");
                self.line_len += 2;
            }
            self.line_start = false;
        }
        self.output.push_str(text);
        self.line_len += text.chars().count();
    }

    fn space(&mut self) {
        if !self.line_start && !self.output.ends_with([' ', '\n']) {
            self.output.push(' ');
            self.line_len += 1;
        }
    }

    fn newline(&mut self) {
        while self.output.ends_with(' ') {
            self.output.pop();
        }
        if !self.output.is_empty() && !self.output.ends_with('\n') {
            self.output.push('\n');
        }
        self.line_start = true;
        self.line_len = 0;
    }

    fn finish(mut self) -> String {
        self.newline();
        self.output
    }
}

fn token_class(kind: &TokenKind<'_>) -> TokenClass {
    match kind {
        TokenKind::Ident(_)
        | TokenKind::Int
        | TokenKind::Float
        | TokenKind::Number
        | TokenKind::String
        | TokenKind::Bool
        | TokenKind::Void
        | TokenKind::Auto
        | TokenKind::Func
        | TokenKind::Struct
        | TokenKind::Enum
        | TokenKind::Record
        | TokenKind::Class
        | TokenKind::Return
        | TokenKind::Init
        | TokenKind::If
        | TokenKind::Else
        | TokenKind::While
        | TokenKind::For
        | TokenKind::Of
        | TokenKind::Break
        | TokenKind::Continue
        | TokenKind::Extern
        | TokenKind::Import
        | TokenKind::Export
        | TokenKind::From
        | TokenKind::As
        | TokenKind::Pure
        | TokenKind::New
        | TokenKind::Is => TokenClass::Word,
        TokenKind::Match => TokenClass::Word,
        TokenKind::IntLiteral(_)
        | TokenKind::FloatLiteral(_)
        | TokenKind::StringLiteral(_)
        | TokenKind::TemplateLiteral(_)
        | TokenKind::True
        | TokenKind::False
        | TokenKind::Null => TokenClass::Literal,
        TokenKind::LParen => TokenClass::OpenParen,
        TokenKind::RParen => TokenClass::CloseParen,
        TokenKind::LBracket => TokenClass::OpenBracket,
        TokenKind::RBracket => TokenClass::CloseBracket,
        TokenKind::LBrace => TokenClass::OpenBrace,
        TokenKind::RBrace => TokenClass::CloseBrace,
        TokenKind::Dot => TokenClass::Dot,
        TokenKind::Comma => TokenClass::Comma,
        TokenKind::Colon => TokenClass::Colon,
        TokenKind::Semicolon => TokenClass::Semicolon,
        _ => TokenClass::Operator,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formatting_is_idempotent_and_preserves_comments() {
        let input = "int x=1;// keep\nif(x>0){x+=1;}";
        let once = format_source(input, &FormatConfig::default()).unwrap();
        let twice = format_source(&once, &FormatConfig::default()).unwrap();
        assert_eq!(once, twice);
        assert!(once.contains("// keep"));
        assert!(once.contains("if (x > 0)"));
    }

    #[test]
    fn organizes_leading_imports() {
        let input = "import { z } from \"./z\";\nimport { a } from \"./a\";\nprint(a);";
        let output = format_source(input, &FormatConfig::default()).unwrap();
        assert!(output.find("./a").unwrap() < output.find("./z").unwrap());
    }

    #[test]
    fn separates_a_closed_function_from_the_next_statement() {
        let input = "pure int square(int value){return value*value;}print(square(4));";
        let output = format_source(input, &FormatConfig::default()).unwrap();
        assert!(output.contains("}\nprint(square(4));"));
        assert_eq!(
            output,
            format_source(&output, &FormatConfig::default()).unwrap()
        );
    }

    #[test]
    fn keeps_prefix_and_postfix_updates_attached() {
        let input =
            "int value=1;print(++value);print(value++);int next=value++ + ++value;return ++value;";
        let output = format_source(input, &FormatConfig::default()).unwrap();
        assert!(output.contains("print(++value);"), "{output}");
        assert!(output.contains("print(value++);"), "{output}");
        assert!(output.contains("int next = value++ + ++value;"), "{output}");
        assert!(output.contains("return ++value;"), "{output}");
    }
}
