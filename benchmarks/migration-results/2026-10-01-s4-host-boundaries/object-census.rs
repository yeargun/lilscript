use lilscript::lexer::{lex, TokenKind};
use std::io::BufRead;
fn main() {
    for path in std::io::stdin().lock().lines() {
        let path = path.unwrap();
        let source = std::fs::read_to_string(&path).unwrap();
        let tokens = lex(&source).unwrap_or_else(|error| panic!("{path}: {error}"));
        let positions: Vec<_> = tokens.windows(3).filter_map(|tokens| {
            (matches!(tokens[0].kind, TokenKind::Ident("object"))
                && matches!(tokens[1].kind, TokenKind::Ident(_) | TokenKind::From)
                && matches!(tokens[2].kind, TokenKind::LBrace | TokenKind::Extends | TokenKind::Less))
                .then_some(tokens[0].span.start)
        }).collect();
        println!("{:?}", positions);
    }
}
