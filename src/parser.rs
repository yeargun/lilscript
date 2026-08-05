use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::ast::{
    ArrowBody, AssignmentOp, BinaryOp, ClassDecl, ClassMember, ConstructorDecl, ExportDecl, Expr,
    ExternDecl, FieldDecl, ForInitializer, FunctionDecl, Ident, ImportDecl, ImportSpecifier, Item,
    Param, Program, Stmt, StructDecl, TemplatePart, TypeKind, TypeRef, UnaryOp, UpdateOp, VarDecl,
};
use crate::lexer::{lex, LexError, Token, TokenKind};
use crate::span::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub span: Span,
    pub message: String,
}

impl ParseError {
    fn new(span: Span, message: impl Into<String>) -> Self {
        Self {
            span,
            message: message.into(),
        }
    }

    pub const fn span(&self) -> Span {
        self.span
    }
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(value: LexError) -> Self {
        Self {
            span: value.span,
            message: value.message,
        }
    }
}

pub fn parse_source<'arena, 'src>(
    arena: &'arena Bump,
    source: &'src str,
) -> Result<Program<'arena, 'src>, ParseError> {
    Parser::new(arena, source)?.parse_program()
}

pub struct Parser<'arena, 'src> {
    arena: &'arena Bump,
    tokens: Vec<Token<'src>>,
    cursor: usize,
    source_len: usize,
}

impl<'arena, 'src> Parser<'arena, 'src> {
    pub fn new(arena: &'arena Bump, source: &'src str) -> Result<Self, ParseError> {
        Self::new_fragment(arena, source, 0)
    }

    fn new_fragment(
        arena: &'arena Bump,
        source: &'src str,
        base_offset: usize,
    ) -> Result<Self, ParseError> {
        let mut tokens = lex(source).map_err(|error| ParseError {
            span: Span::new(error.span.start + base_offset, error.span.end + base_offset),
            message: error.message,
        })?;
        if base_offset != 0 {
            for token in &mut tokens {
                token.span =
                    Span::new(token.span.start + base_offset, token.span.end + base_offset);
            }
        }
        Ok(Self {
            arena,
            tokens,
            cursor: 0,
            source_len: source.len() + base_offset,
        })
    }

    pub fn parse_program(mut self) -> Result<Program<'arena, 'src>, ParseError> {
        let start = self.peek_span().unwrap_or_else(|| Span::empty(0));
        let mut items = BumpVec::new_in(self.arena);
        let mut imports = BumpVec::new_in(self.arena);
        let mut exports = BumpVec::new_in(self.arena);

        while !self.is_at_end() {
            if self.match_kind(|kind| matches!(kind, TokenKind::Import)) {
                imports.push(self.parse_import_after_keyword()?);
                continue;
            }
            if self.match_kind(|kind| matches!(kind, TokenKind::Export)) {
                let export_start = self.previous_span();
                if self.match_kind(|kind| matches!(kind, TokenKind::LBrace)) {
                    self.parse_export_list_after_open(export_start, &mut exports)?;
                    continue;
                }
                let item = self.parse_item()?;
                let local = exported_item_name(&item).ok_or_else(|| {
                    ParseError::new(item.span(), "only declarations can be exported")
                })?;
                exports.push(ExportDecl {
                    local,
                    exported: local,
                    span: export_start.merge(item.span()),
                });
                items.push(item);
                continue;
            }
            items.push(self.parse_item()?);
        }

        let end = items
            .last()
            .map(Item::span)
            .unwrap_or_else(|| Span::empty(self.source_len));
        let span = if items.is_empty() {
            Span::empty(0)
        } else {
            start.merge(end)
        };

        Ok(Program {
            imports: imports.into_bump_slice(),
            exports: exports.into_bump_slice(),
            items: items.into_bump_slice(),
            span,
        })
    }

    fn parse_import_after_keyword(&mut self) -> Result<ImportDecl<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        if let Some(TokenKind::StringLiteral(raw)) = self.peek_kind() {
            let source = strip_quotes(raw);
            self.advance();
            let semi = self.expect_semicolon()?;
            return Ok(ImportDecl {
                specifiers: &[],
                source,
                span: start.merge(semi.span),
            });
        }

        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` or a module path after `import`",
        )?;
        let mut specifiers = BumpVec::new_in(self.arena);
        if !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            loop {
                let imported = self.expect_ident("expected imported name")?;
                let local = if self.match_kind(|kind| matches!(kind, TokenKind::As)) {
                    self.expect_ident("expected local alias after `as`")?
                } else {
                    imported
                };
                specifiers.push(ImportSpecifier { imported, local });
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after import names",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::From),
            "expected `from` after import names",
        )?;
        let path = self
            .advance()
            .ok_or_else(|| self.error_here("expected module path after `from`"))?;
        let TokenKind::StringLiteral(raw) = path.kind else {
            return Err(ParseError::new(
                path.span,
                "expected string module path after `from`",
            ));
        };
        let semi = self.expect_semicolon()?;
        Ok(ImportDecl {
            specifiers: specifiers.into_bump_slice(),
            source: strip_quotes(raw),
            span: start.merge(semi.span),
        })
    }

    fn parse_export_list_after_open(
        &mut self,
        start: Span,
        exports: &mut BumpVec<'arena, ExportDecl<'src>>,
    ) -> Result<(), ParseError> {
        if !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            loop {
                let local = self.expect_ident("expected exported name")?;
                let exported = if self.match_kind(|kind| matches!(kind, TokenKind::As)) {
                    self.expect_ident("expected export alias after `as`")?
                } else {
                    local
                };
                exports.push(ExportDecl {
                    local,
                    exported,
                    span: start.merge(exported.span),
                });
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after export names",
        )?;
        self.expect_semicolon()?;
        Ok(())
    }

    fn parse_item(&mut self) -> Result<Item<'arena, 'src>, ParseError> {
        let declared_pure = self.match_kind(|kind| matches!(kind, TokenKind::Pure));
        if self.match_kind(|kind| matches!(kind, TokenKind::Extern)) {
            return self
                .parse_extern_after_keyword(declared_pure)
                .map(Item::Extern);
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Struct)) {
            if declared_pure {
                return Err(self.error_here("`pure` can only modify functions and externs"));
            }
            return self.parse_struct_after_keyword().map(Item::Struct);
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Class)) {
            if declared_pure {
                return Err(self.error_here("`pure` can only modify functions and externs"));
            }
            return self.parse_class_after_keyword().map(Item::Class);
        }

        if self.looks_like_typed_binding() {
            let ty = self.parse_type()?;
            let name = self.expect_ident("expected declaration name")?;
            let type_params = self.parse_type_params()?;
            if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                return self
                    .parse_function_after_signature(ty, name, type_params, declared_pure)
                    .map(Item::Function);
            }

            if !type_params.is_empty() {
                return Err(ParseError::new(
                    name.span,
                    "type parameters require a function declaration",
                ));
            }

            if declared_pure {
                return Err(ParseError::new(
                    name.span,
                    "`pure` can only modify functions and externs",
                ));
            }

            return self
                .parse_var_decl_after_name(ty, name)
                .map(|decl| Item::Stmt(Stmt::VarDecl(decl)));
        }

        if declared_pure {
            return Err(self.error_here("expected function declaration after `pure`"));
        }
        self.parse_statement().map(Item::Stmt)
    }

    fn parse_extern_after_keyword(
        &mut self,
        declared_pure: bool,
    ) -> Result<ExternDecl<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        let return_type = self.parse_type()?;
        if return_type.is_auto() {
            return Err(ParseError::new(
                return_type.span,
                "extern return type cannot be `auto`",
            ));
        }
        let name = self.expect_ident("expected extern function name")?;
        let type_params = self.parse_type_params()?;
        self.expect(
            |kind| matches!(kind, TokenKind::LParen),
            "expected `(` after extern function name",
        )?;
        let params = self.parse_params_after_open()?;
        let semi = self.expect_semicolon()?;
        Ok(ExternDecl {
            declared_pure,
            return_type,
            name,
            type_params,
            params,
            span: start.merge(semi.span),
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        if self.match_kind(|kind| matches!(kind, TokenKind::Return)) {
            return self.parse_return_after_keyword();
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::LBrace)) {
            return self
                .parse_block_after_open()
                .map(|(body, span)| Stmt::Block { body, span });
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::If)) {
            return self.parse_if_after_keyword();
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::While)) {
            return self.parse_while_after_keyword();
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::For)) {
            return self.parse_for_after_keyword();
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Break)) {
            let start = self.previous_span();
            let semi = self.expect_semicolon()?;
            return Ok(Stmt::Break(start.merge(semi.span)));
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Continue)) {
            let start = self.previous_span();
            let semi = self.expect_semicolon()?;
            return Ok(Stmt::Continue(start.merge(semi.span)));
        }

        if self.looks_like_typed_binding() {
            let ty = self.parse_type()?;
            let name = self.expect_ident("expected variable name")?;
            return self.parse_var_decl_after_name(ty, name).map(Stmt::VarDecl);
        }

        let expr = self.parse_expression()?;
        self.expect_semicolon()?;
        Ok(Stmt::Expr(expr))
    }

    fn parse_struct_after_keyword(&mut self) -> Result<StructDecl<'arena, 'src>, ParseError> {
        let keyword_span = self.previous_span();
        let name = self.expect_ident("expected struct name")?;
        let type_params = self.parse_type_params()?;
        self.expect(|kind| matches!(kind, TokenKind::LBrace), "expected `{`")?;

        let mut fields = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated struct declaration"));
            }
            fields.push(self.parse_field_decl()?);
        }

        let close = self.expect(|kind| matches!(kind, TokenKind::RBrace), "expected `}`")?;
        Ok(StructDecl {
            name,
            type_params,
            fields: fields.into_bump_slice(),
            span: keyword_span.merge(close.span),
        })
    }

    fn parse_class_after_keyword(&mut self) -> Result<ClassDecl<'arena, 'src>, ParseError> {
        let keyword_span = self.previous_span();
        let name = self.expect_ident("expected class name")?;
        let type_params = self.parse_type_params()?;
        self.expect(|kind| matches!(kind, TokenKind::LBrace), "expected `{`")?;

        let mut members = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated class declaration"));
            }

            if self.match_kind(|kind| matches!(kind, TokenKind::Init)) {
                let start = self.previous_span();
                self.expect(
                    |kind| matches!(kind, TokenKind::LParen),
                    "expected `(` after `init`",
                )?;
                let params = self.parse_params_after_open()?;
                self.expect(
                    |kind| matches!(kind, TokenKind::LBrace),
                    "expected constructor body",
                )?;
                let (body, body_span) = self.parse_block_after_open()?;
                members.push(ClassMember::Constructor(ConstructorDecl {
                    params,
                    body,
                    span: start.merge(body_span),
                }));
                continue;
            }

            let declared_pure = self.match_kind(|kind| matches!(kind, TokenKind::Pure));
            let ty = self.parse_type()?;
            let member_name = self.expect_ident("expected class member name")?;
            let type_params = self.parse_type_params()?;
            if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                let method = self.parse_function_after_signature(
                    ty,
                    member_name,
                    type_params,
                    declared_pure,
                )?;
                members.push(ClassMember::Method(method));
            } else {
                if !type_params.is_empty() {
                    return Err(ParseError::new(
                        member_name.span,
                        "type parameters require a method declaration",
                    ));
                }
                if declared_pure {
                    return Err(ParseError::new(
                        member_name.span,
                        "`pure` can only modify methods",
                    ));
                }
                let field = self.parse_field_decl_after_name(ty, member_name)?;
                members.push(ClassMember::Field(field));
            }
        }

        let close = self.expect(|kind| matches!(kind, TokenKind::RBrace), "expected `}`")?;
        Ok(ClassDecl {
            name,
            type_params,
            members: members.into_bump_slice(),
            span: keyword_span.merge(close.span),
        })
    }

    fn parse_field_decl(&mut self) -> Result<FieldDecl<'arena, 'src>, ParseError> {
        let ty = self.parse_type()?;
        let name = self.expect_ident("expected field name")?;
        self.parse_field_decl_after_name(ty, name)
    }

    fn parse_field_decl_after_name(
        &mut self,
        ty: TypeRef<'arena, 'src>,
        name: Ident<'src>,
    ) -> Result<FieldDecl<'arena, 'src>, ParseError> {
        let semi = self.expect_semicolon()?;
        Ok(FieldDecl {
            ty,
            name,
            span: ty.span.merge(semi.span),
        })
    }

    fn parse_function_after_signature(
        &mut self,
        return_type: TypeRef<'arena, 'src>,
        name: Ident<'src>,
        type_params: &'arena [Ident<'src>],
        declared_pure: bool,
    ) -> Result<FunctionDecl<'arena, 'src>, ParseError> {
        let params = self.parse_params_after_open()?;
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected function body",
        )?;
        let (body, body_span) = self.parse_block_after_open()?;
        Ok(FunctionDecl {
            declared_pure,
            return_type,
            name,
            type_params,
            params,
            body,
            span: return_type.span.merge(body_span),
        })
    }

    fn parse_var_decl_after_name(
        &mut self,
        ty: TypeRef<'arena, 'src>,
        name: Ident<'src>,
    ) -> Result<VarDecl<'arena, 'src>, ParseError> {
        let initializer = if self.match_kind(|kind| matches!(kind, TokenKind::Eq)) {
            Some(self.parse_expression()?)
        } else {
            None
        };
        let semi = self.expect_semicolon()?;
        Ok(VarDecl {
            ty,
            name,
            initializer,
            span: ty.span.merge(semi.span),
        })
    }

    fn parse_return_after_keyword(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        if self.check(|kind| matches!(kind, TokenKind::Semicolon)) {
            let semi = self.expect_semicolon()?;
            return Ok(Stmt::Return {
                value: None,
                span: start.merge(semi.span),
            });
        }

        let value = self.parse_expression()?;
        let semi = self.expect_semicolon()?;
        Ok(Stmt::Return {
            value: Some(value),
            span: start.merge(semi.span),
        })
    }

    fn parse_if_after_keyword(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        self.expect(
            |kind| matches!(kind, TokenKind::LParen),
            "expected `(` after `if`",
        )?;
        let condition = self.parse_expression()?;
        self.expect(
            |kind| matches!(kind, TokenKind::RParen),
            "expected `)` after condition",
        )?;
        let then_branch = self.arena.alloc(self.parse_statement()?);
        let else_branch = if self.match_kind(|kind| matches!(kind, TokenKind::Else)) {
            Some(&*self.arena.alloc(self.parse_statement()?))
        } else {
            None
        };
        let end = else_branch.map_or_else(|| then_branch.span(), |branch| branch.span());
        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
            span: start.merge(end),
        })
    }

    fn parse_while_after_keyword(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        self.expect(
            |kind| matches!(kind, TokenKind::LParen),
            "expected `(` after `while`",
        )?;
        let condition = self.parse_expression()?;
        self.expect(
            |kind| matches!(kind, TokenKind::RParen),
            "expected `)` after condition",
        )?;
        let body = self.arena.alloc(self.parse_statement()?);
        Ok(Stmt::While {
            condition,
            span: start.merge(body.span()),
            body,
        })
    }

    fn parse_for_after_keyword(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        self.expect(
            |kind| matches!(kind, TokenKind::LParen),
            "expected `(` after `for`",
        )?;

        let initializer = if self.match_kind(|kind| matches!(kind, TokenKind::Semicolon)) {
            None
        } else if self.looks_like_typed_binding() {
            let ty = self.parse_type()?;
            let name = self.expect_ident("expected variable name")?;
            Some(ForInitializer::VarDecl(
                self.parse_var_decl_after_name(ty, name)?,
            ))
        } else {
            let expression = self.parse_expression()?;
            self.expect_semicolon()?;
            Some(ForInitializer::Expr(expression))
        };

        let condition = if self.match_kind(|kind| matches!(kind, TokenKind::Semicolon)) {
            None
        } else {
            let expression = self.parse_expression()?;
            self.expect_semicolon()?;
            Some(expression)
        };

        let update = if self.check(|kind| matches!(kind, TokenKind::RParen)) {
            None
        } else {
            Some(self.parse_expression()?)
        };
        self.expect(
            |kind| matches!(kind, TokenKind::RParen),
            "expected `)` after for clauses",
        )?;
        let body = self.arena.alloc(self.parse_statement()?);
        Ok(Stmt::For {
            initializer,
            condition,
            update,
            span: start.merge(body.span()),
            body,
        })
    }

    fn parse_block_after_open(
        &mut self,
    ) -> Result<(&'arena [Stmt<'arena, 'src>], Span), ParseError> {
        let start = self.previous_span();
        let mut statements = BumpVec::new_in(self.arena);

        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated block"));
            }
            statements.push(self.parse_statement()?);
        }

        let close = self.expect(|kind| matches!(kind, TokenKind::RBrace), "expected `}`")?;
        Ok((statements.into_bump_slice(), start.merge(close.span)))
    }

    fn parse_type_params(&mut self) -> Result<&'arena [Ident<'src>], ParseError> {
        if !self.match_kind(|kind| matches!(kind, TokenKind::Less)) {
            return Ok(&[]);
        }
        let mut params = BumpVec::new_in(self.arena);
        loop {
            params.push(self.expect_ident("expected type parameter name")?);
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Greater),
            "expected `>` after type parameters",
        )?;
        Ok(params.into_bump_slice())
    }

    fn parse_type_args(&mut self) -> Result<&'arena [TypeRef<'arena, 'src>], ParseError> {
        if !self.match_kind(|kind| matches!(kind, TokenKind::Less)) {
            return Ok(&[]);
        }
        let mut args = BumpVec::new_in(self.arena);
        loop {
            args.push(self.parse_type()?);
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
        }
        self.expect(
            |kind| matches!(kind, TokenKind::Greater),
            "expected `>` after type arguments",
        )?;
        Ok(args.into_bump_slice())
    }

    fn parse_type(&mut self) -> Result<TypeRef<'arena, 'src>, ParseError> {
        let token = self
            .advance()
            .ok_or_else(|| self.error_here("expected type"))?;
        let mut ty = match token.kind {
            TokenKind::Int => TypeRef {
                kind: TypeKind::Int,
                span: token.span,
            },
            TokenKind::Float => TypeRef {
                kind: TypeKind::Float,
                span: token.span,
            },
            TokenKind::String => TypeRef {
                kind: TypeKind::String,
                span: token.span,
            },
            TokenKind::Bool => TypeRef {
                kind: TypeKind::Bool,
                span: token.span,
            },
            TokenKind::Void => TypeRef {
                kind: TypeKind::Void,
                span: token.span,
            },
            TokenKind::Auto => TypeRef {
                kind: TypeKind::Auto,
                span: token.span,
            },
            TokenKind::Func => {
                self.expect(
                    |kind| matches!(kind, TokenKind::LParen),
                    "expected `(` after `func`",
                )?;
                let mut params = BumpVec::new_in(self.arena);
                if !self.check(|kind| matches!(kind, TokenKind::RParen)) {
                    loop {
                        params.push(self.parse_type()?);
                        if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                            break;
                        }
                    }
                }
                self.expect(
                    |kind| matches!(kind, TokenKind::RParen),
                    "expected `)` after function parameter types",
                )?;
                self.expect(
                    |kind| matches!(kind, TokenKind::ThinArrow),
                    "expected `->` before function return type",
                )?;
                let return_type = self.parse_type()?;
                let return_type = self.arena.alloc(return_type);
                TypeRef {
                    kind: TypeKind::Function {
                        params: params.into_bump_slice(),
                        return_type,
                    },
                    span: token.span.merge(return_type.span),
                }
            }
            TokenKind::Ident(name) => {
                let args = self.parse_type_args()?;
                let span = args
                    .last()
                    .map_or(token.span, |argument| token.span.merge(argument.span));
                TypeRef {
                    kind: TypeKind::Named { name, args },
                    span,
                }
            }
            _ => return Err(ParseError::new(token.span, "expected type")),
        };

        loop {
            if self.match_kind(|kind| matches!(kind, TokenKind::LBracket)) {
                let open = self.previous_span();
                let close =
                    self.expect(|kind| matches!(kind, TokenKind::RBracket), "expected `]`")?;
                let element = self.arena.alloc(ty);
                ty = TypeRef {
                    kind: TypeKind::Array(element),
                    span: open.merge(close.span).merge(element.span),
                };
            } else if self.match_kind(|kind| matches!(kind, TokenKind::Question)) {
                let question = self.previous_span();
                let inner = self.arena.alloc(ty);
                ty = TypeRef {
                    kind: TypeKind::Nullable(inner),
                    span: inner.span.merge(question),
                };
            } else {
                break;
            }
        }

        Ok(ty)
    }

    fn parse_expression(&mut self) -> Result<Expr<'arena, 'src>, ParseError> {
        self.parse_assignment_expression()
    }

    fn parse_assignment_expression(&mut self) -> Result<Expr<'arena, 'src>, ParseError> {
        let target = self.parse_binary_expression(0)?;
        let op = match self.peek_kind() {
            Some(TokenKind::Eq) => AssignmentOp::Assign,
            Some(TokenKind::PlusEq) => AssignmentOp::Add,
            Some(TokenKind::MinusEq) => AssignmentOp::Sub,
            Some(TokenKind::StarEq) => AssignmentOp::Mul,
            Some(TokenKind::SlashEq) => AssignmentOp::Div,
            Some(TokenKind::PercentEq) => AssignmentOp::Mod,
            _ => return Ok(target),
        };
        self.advance();
        let value = self.parse_assignment_expression()?;
        let target = self.arena.alloc(target);
        let value = self.arena.alloc(value);
        Ok(Expr::Assignment {
            op,
            target,
            value,
            span: target.span().merge(value.span()),
        })
    }

    fn parse_binary_expression(
        &mut self,
        min_precedence: u8,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        let mut lhs = self.parse_unary_expression()?;

        while let Some((op, precedence)) = self.peek_binary_op() {
            if precedence < min_precedence {
                break;
            }

            self.advance();
            let rhs = self.parse_binary_expression(precedence + 1)?;
            let lhs_ref = self.arena.alloc(lhs);
            let rhs_ref = self.arena.alloc(rhs);
            lhs = Expr::Binary {
                op,
                lhs: lhs_ref,
                rhs: rhs_ref,
                span: lhs_ref.span().merge(rhs_ref.span()),
            };
        }

        Ok(lhs)
    }

    fn parse_unary_expression(&mut self) -> Result<Expr<'arena, 'src>, ParseError> {
        if self.match_kind(|kind| matches!(kind, TokenKind::Bang)) {
            let op_span = self.previous_span();
            let expr = self.parse_unary_expression()?;
            let expr_ref = self.arena.alloc(expr);
            return Ok(Expr::Unary {
                op: UnaryOp::Not,
                expr: expr_ref,
                span: op_span.merge(expr_ref.span()),
            });
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Minus)) {
            let op_span = self.previous_span();
            let expr = self.parse_unary_expression()?;
            let expr_ref = self.arena.alloc(expr);
            return Ok(Expr::Unary {
                op: UnaryOp::Neg,
                expr: expr_ref,
                span: op_span.merge(expr_ref.span()),
            });
        }

        self.parse_postfix_expression()
    }

    fn parse_postfix_expression(&mut self) -> Result<Expr<'arena, 'src>, ParseError> {
        let mut expr = self.parse_primary_expression()?;

        loop {
            if self.match_kind(|kind| matches!(kind, TokenKind::Dot)) {
                let property = self.expect_ident("expected property name after `.`")?;
                let object = self.arena.alloc(expr);
                expr = Expr::Member {
                    object,
                    property,
                    span: object.span().merge(property.span),
                };
                continue;
            }

            if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                let (args, close_span) = self.parse_args_after_open()?;
                let callee = self.arena.alloc(expr);
                expr = Expr::Call {
                    callee,
                    args,
                    span: callee.span().merge(close_span),
                };
                continue;
            }

            if self.match_kind(|kind| matches!(kind, TokenKind::LBracket)) {
                let index = self.parse_expression()?;
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RBracket),
                    "expected `]` after index",
                )?;
                let object = self.arena.alloc(expr);
                let index = self.arena.alloc(index);
                expr = Expr::Index {
                    object,
                    index,
                    span: object.span().merge(close.span),
                };
                continue;
            }

            let update = if self.match_kind(|kind| matches!(kind, TokenKind::PlusPlus)) {
                Some(UpdateOp::Increment)
            } else if self.match_kind(|kind| matches!(kind, TokenKind::MinusMinus)) {
                Some(UpdateOp::Decrement)
            } else {
                None
            };
            if let Some(op) = update {
                let target = self.arena.alloc(expr);
                expr = Expr::Update {
                    op,
                    target,
                    prefix: false,
                    span: target.span().merge(self.previous_span()),
                };
                continue;
            }

            break;
        }

        Ok(expr)
    }

    fn parse_primary_expression(&mut self) -> Result<Expr<'arena, 'src>, ParseError> {
        if self.check(|kind| matches!(kind, TokenKind::LParen)) && self.is_arrow_function_start() {
            return self.parse_arrow_function();
        }

        let token = self
            .advance()
            .ok_or_else(|| self.error_here("expected expression"))?;
        match token.kind {
            TokenKind::IntLiteral(value) => Ok(Expr::Int(value, token.span)),
            TokenKind::FloatLiteral(value) => Ok(Expr::Float(value, token.span)),
            TokenKind::StringLiteral(raw) => Ok(Expr::String(strip_quotes(raw), token.span)),
            TokenKind::TemplateLiteral(raw) => self.parse_template_literal(raw, token.span),
            TokenKind::True => Ok(Expr::Bool(true, token.span)),
            TokenKind::False => Ok(Expr::Bool(false, token.span)),
            TokenKind::Null => Ok(Expr::Null(token.span)),
            TokenKind::New => {
                let class = self.expect_ident("expected class name after `new`")?;
                let type_args = self.parse_type_args()?;
                self.expect(
                    |kind| matches!(kind, TokenKind::LParen),
                    "expected `(` after class name",
                )?;
                let (args, close_span) = self.parse_args_after_open()?;
                Ok(Expr::New {
                    class,
                    type_args,
                    args,
                    span: token.span.merge(close_span),
                })
            }
            TokenKind::Ident(name) => {
                let ident = Ident {
                    name,
                    span: token.span,
                };
                if self.match_kind(|kind| matches!(kind, TokenKind::LBrace)) {
                    return self.parse_struct_literal_after_open(ident);
                }
                Ok(Expr::Ident(ident))
            }
            TokenKind::LParen => {
                let expr = self.parse_expression()?;
                self.expect(|kind| matches!(kind, TokenKind::RParen), "expected `)`")?;
                Ok(expr)
            }
            TokenKind::LBracket => self.parse_array_literal_after_open(token.span),
            _ => Err(ParseError::new(token.span, "expected expression")),
        }
    }

    fn parse_template_literal(
        &mut self,
        raw: &'src str,
        span: Span,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        let content = raw
            .strip_prefix('`')
            .and_then(|value| value.strip_suffix('`'))
            .unwrap_or(raw);
        let content_offset = span.start + usize::from(raw.starts_with('`'));
        let bytes = content.as_bytes();
        let mut parts = BumpVec::new_in(self.arena);
        let mut segment_start = 0usize;
        let mut cursor = 0usize;

        while cursor + 1 < bytes.len() {
            if bytes[cursor] == b'\\' {
                cursor = (cursor + 2).min(bytes.len());
                continue;
            }
            if bytes[cursor] != b'$' || bytes[cursor + 1] != b'{' {
                cursor += 1;
                continue;
            }

            if segment_start < cursor {
                parts.push(TemplatePart::String(
                    &content[segment_start..cursor],
                    Span::new(content_offset + segment_start, content_offset + cursor),
                ));
            }

            let expression_start = cursor + 2;
            let expression_end = find_template_expression_end(content, expression_start)
                .ok_or_else(|| ParseError::new(span, "unterminated template interpolation"))?;
            let expression_source = &content[expression_start..expression_end];
            let mut parser = Parser::new_fragment(
                self.arena,
                expression_source,
                content_offset + expression_start,
            )?;
            let expression = parser.parse_expression()?;
            if !parser.is_at_end() {
                return Err(parser.error_here("unexpected token in template interpolation"));
            }
            parts.push(TemplatePart::Expr(expression));
            cursor = expression_end + 1;
            segment_start = cursor;
        }

        if segment_start < content.len() {
            parts.push(TemplatePart::String(
                &content[segment_start..],
                Span::new(
                    content_offset + segment_start,
                    content_offset + content.len(),
                ),
            ));
        }

        Ok(Expr::Template {
            parts: parts.into_bump_slice(),
            span,
        })
    }

    fn parse_array_literal_after_open(
        &mut self,
        open: Span,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        let mut elements = BumpVec::new_in(self.arena);

        if !self.check(|kind| matches!(kind, TokenKind::RBracket)) {
            loop {
                elements.push(self.parse_expression()?);
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
                if self.check(|kind| matches!(kind, TokenKind::RBracket)) {
                    break;
                }
            }
        }

        let close = self.expect(|kind| matches!(kind, TokenKind::RBracket), "expected `]`")?;
        Ok(Expr::ArrayLiteral {
            elements: elements.into_bump_slice(),
            span: open.merge(close.span),
        })
    }

    fn parse_struct_literal_after_open(
        &mut self,
        name: Ident<'src>,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        let open = self.previous_span();
        let mut values = BumpVec::new_in(self.arena);

        if !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            loop {
                values.push(self.parse_expression()?);
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
                if self.check(|kind| matches!(kind, TokenKind::RBrace)) {
                    break;
                }
            }
        }

        let close = self.expect(|kind| matches!(kind, TokenKind::RBrace), "expected `}`")?;
        Ok(Expr::StructLiteral {
            name,
            values: values.into_bump_slice(),
            span: name.span.merge(open).merge(close.span),
        })
    }

    fn parse_arrow_function(&mut self) -> Result<Expr<'arena, 'src>, ParseError> {
        let open = self.expect(|kind| matches!(kind, TokenKind::LParen), "expected `(`")?;
        let params = self.parse_params_after_open()?;
        self.expect(|kind| matches!(kind, TokenKind::FatArrow), "expected `=>`")?;

        let (body, body_span) = if self.match_kind(|kind| matches!(kind, TokenKind::LBrace)) {
            let (body, span) = self.parse_block_after_open()?;
            (ArrowBody::Block(body), span)
        } else {
            let expr = self.parse_expression()?;
            let span = expr.span();
            (ArrowBody::Expr(self.arena.alloc(expr)), span)
        };

        Ok(Expr::ArrowFunction {
            params,
            body,
            span: open.span.merge(body_span),
        })
    }

    fn parse_params_after_open(&mut self) -> Result<&'arena [Param<'arena, 'src>], ParseError> {
        let mut params = BumpVec::new_in(self.arena);
        if !self.check(|kind| matches!(kind, TokenKind::RParen)) {
            loop {
                let ty = self.parse_type()?;
                let name = self.expect_ident("expected parameter name")?;
                params.push(Param {
                    ty,
                    name,
                    span: ty.span.merge(name.span),
                });
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
            }
        }
        self.expect(|kind| matches!(kind, TokenKind::RParen), "expected `)`")?;
        Ok(params.into_bump_slice())
    }

    fn parse_args_after_open(
        &mut self,
    ) -> Result<(&'arena [Expr<'arena, 'src>], Span), ParseError> {
        let mut args = BumpVec::new_in(self.arena);
        if !self.check(|kind| matches!(kind, TokenKind::RParen)) {
            loop {
                args.push(self.parse_expression()?);
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
                if self.check(|kind| matches!(kind, TokenKind::RParen)) {
                    break;
                }
            }
        }
        let close = self.expect(|kind| matches!(kind, TokenKind::RParen), "expected `)`")?;
        Ok((args.into_bump_slice(), close.span))
    }

    fn looks_like_typed_binding(&self) -> bool {
        let Some(type_end) = self.scan_type_end(self.cursor) else {
            return false;
        };
        matches!(
            self.tokens.get(type_end).map(|token| &token.kind),
            Some(TokenKind::Ident(_))
        )
    }

    fn scan_type_end(&self, start: usize) -> Option<usize> {
        let mut index = start;
        match self.tokens.get(index).map(|token| &token.kind) {
            Some(
                TokenKind::Int
                | TokenKind::Float
                | TokenKind::String
                | TokenKind::Bool
                | TokenKind::Void
                | TokenKind::Auto,
            ) => index += 1,
            Some(TokenKind::Ident(_)) => {
                index += 1;
                if matches!(
                    self.tokens.get(index).map(|token| &token.kind),
                    Some(TokenKind::Less)
                ) {
                    index += 1;
                    loop {
                        index = self.scan_type_end(index)?;
                        if matches!(
                            self.tokens.get(index).map(|token| &token.kind),
                            Some(TokenKind::Comma)
                        ) {
                            index += 1;
                            continue;
                        }
                        break;
                    }
                    if !matches!(
                        self.tokens.get(index).map(|token| &token.kind),
                        Some(TokenKind::Greater)
                    ) {
                        return None;
                    }
                    index += 1;
                }
            }
            Some(TokenKind::Func) => {
                index += 1;
                if !matches!(self.tokens.get(index)?.kind, TokenKind::LParen) {
                    return None;
                }
                index += 1;
                if !matches!(self.tokens.get(index)?.kind, TokenKind::RParen) {
                    loop {
                        index = self.scan_type_end(index)?;
                        if matches!(self.tokens.get(index)?.kind, TokenKind::Comma) {
                            index += 1;
                            continue;
                        }
                        break;
                    }
                }
                if !matches!(self.tokens.get(index)?.kind, TokenKind::RParen) {
                    return None;
                }
                index += 1;
                if !matches!(self.tokens.get(index)?.kind, TokenKind::ThinArrow) {
                    return None;
                }
                index = self.scan_type_end(index + 1)?;
            }
            _ => return None,
        }

        loop {
            if matches!(
                (
                    self.tokens.get(index).map(|token| &token.kind),
                    self.tokens.get(index + 1).map(|token| &token.kind),
                ),
                (Some(TokenKind::LBracket), Some(TokenKind::RBracket))
            ) {
                index += 2;
            } else if matches!(
                self.tokens.get(index).map(|token| &token.kind),
                Some(TokenKind::Question)
            ) {
                index += 1;
            } else {
                break;
            }
        }

        Some(index)
    }

    fn is_arrow_function_start(&self) -> bool {
        let mut depth = 0usize;
        for index in self.cursor..self.tokens.len() {
            match &self.tokens[index].kind {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        return matches!(
                            self.tokens.get(index + 1).map(|token| &token.kind),
                            Some(TokenKind::FatArrow)
                        );
                    }
                }
                _ => {}
            }
        }
        false
    }

    fn peek_binary_op(&self) -> Option<(BinaryOp, u8)> {
        let op = match self.peek_kind()? {
            TokenKind::OrOr => (BinaryOp::Or, 1),
            TokenKind::AndAnd => (BinaryOp::And, 2),
            TokenKind::EqEq => (BinaryOp::Eq, 3),
            TokenKind::BangEq => (BinaryOp::NotEq, 3),
            TokenKind::Less => (BinaryOp::Less, 4),
            TokenKind::LessEq => (BinaryOp::LessEq, 4),
            TokenKind::Greater => (BinaryOp::Greater, 4),
            TokenKind::GreaterEq => (BinaryOp::GreaterEq, 4),
            TokenKind::Plus => (BinaryOp::Add, 5),
            TokenKind::Minus => (BinaryOp::Sub, 5),
            TokenKind::Star => (BinaryOp::Mul, 6),
            TokenKind::Slash => (BinaryOp::Div, 6),
            TokenKind::Percent => (BinaryOp::Mod, 6),
            _ => return None,
        };
        Some(op)
    }

    fn expect_ident(&mut self, message: &'static str) -> Result<Ident<'src>, ParseError> {
        let token = self.advance().ok_or_else(|| self.error_here(message))?;
        match token.kind {
            TokenKind::Ident(name) => Ok(Ident {
                name,
                span: token.span,
            }),
            _ => Err(ParseError::new(token.span, message)),
        }
    }

    fn expect_semicolon(&mut self) -> Result<Token<'src>, ParseError> {
        self.expect(
            |kind| matches!(kind, TokenKind::Semicolon),
            "expected `;` after statement",
        )
    }

    fn expect(
        &mut self,
        predicate: impl FnOnce(&TokenKind<'src>) -> bool,
        message: &'static str,
    ) -> Result<Token<'src>, ParseError> {
        let token = self.advance().ok_or_else(|| self.error_here(message))?;
        if predicate(&token.kind) {
            Ok(token)
        } else {
            Err(ParseError::new(token.span, message))
        }
    }

    fn match_kind(&mut self, predicate: impl FnOnce(&TokenKind<'src>) -> bool) -> bool {
        if self.check(predicate) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn check(&self, predicate: impl FnOnce(&TokenKind<'src>) -> bool) -> bool {
        self.peek_kind().is_some_and(predicate)
    }

    fn advance(&mut self) -> Option<Token<'src>> {
        let token = self.tokens.get(self.cursor).cloned()?;
        self.cursor += 1;
        Some(token)
    }

    fn previous_span(&self) -> Span {
        self.tokens
            .get(self.cursor.saturating_sub(1))
            .map(|token| token.span)
            .unwrap_or_else(|| Span::empty(0))
    }

    fn peek_kind(&self) -> Option<&TokenKind<'src>> {
        self.tokens.get(self.cursor).map(|token| &token.kind)
    }

    fn peek_span(&self) -> Option<Span> {
        self.tokens.get(self.cursor).map(|token| token.span)
    }

    fn is_at_end(&self) -> bool {
        self.cursor >= self.tokens.len()
    }

    fn error_here(&self, message: impl Into<String>) -> ParseError {
        ParseError::new(
            self.peek_span()
                .unwrap_or_else(|| Span::empty(self.source_len)),
            message,
        )
    }
}

fn find_template_expression_end(content: &str, start: usize) -> Option<usize> {
    let bytes = content.as_bytes();
    let mut cursor = start;
    let mut depth = 1usize;
    let mut quote = None;

    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if let Some(active_quote) = quote {
            if byte == b'\\' {
                cursor += 2;
                continue;
            }
            if byte == active_quote {
                quote = None;
            }
            cursor += 1;
            continue;
        }

        match byte {
            b'"' | b'`' => quote = Some(byte),
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor);
                }
            }
            _ => {}
        }
        cursor += 1;
    }
    None
}

fn exported_item_name<'src>(item: &Item<'_, 'src>) -> Option<Ident<'src>> {
    match item {
        Item::Struct(decl) => Some(decl.name),
        Item::Class(decl) => Some(decl.name),
        Item::Function(decl) => Some(decl.name),
        Item::Extern(decl) => Some(decl.name),
        Item::Stmt(Stmt::VarDecl(decl)) => Some(decl.name),
        Item::Stmt(_) => None,
    }
}

fn strip_quotes(raw: &str) -> &str {
    raw.strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .unwrap_or(raw)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_struct_and_struct_literal_decl() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point { int x; int y; } Point p = Point{10, 20};",
        )
        .unwrap();

        assert_eq!(program.items.len(), 2);
        assert!(matches!(&program.items[0], Item::Struct(_)));
        assert!(matches!(&program.items[1], Item::Stmt(Stmt::VarDecl(_))));
    }

    #[test]
    fn parses_array_method_with_typed_arrow() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] numbers = [1, 2, 3]; auto doubled = numbers.map((int x) => x * 2);",
        )
        .unwrap();

        assert_eq!(program.items.len(), 2);
    }

    #[test]
    fn parses_class_construction() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Vector { float x; float length() { return this.x; } } Vector v = new Vector();",
        )
        .unwrap();

        assert_eq!(program.items.len(), 2);
        assert!(matches!(&program.items[0], Item::Class(_)));
    }

    #[test]
    fn parses_control_flow_assignment_and_templates() {
        let arena = Bump::new();
        let source = "int sum=0; for(int i=0;i<3;i++){sum+=i;} if(sum==3){print(`sum=${sum}`);}";
        let program = parse_source(&arena, source).unwrap();
        assert_eq!(program.items.len(), 3);
        assert!(matches!(&program.items[1], Item::Stmt(Stmt::For { .. })));
        assert!(matches!(&program.items[2], Item::Stmt(Stmt::If { .. })));
    }

    #[test]
    fn parses_first_class_function_types() {
        let arena = Bump::new();
        let program = parse_source(&arena, "func(int)->int twice=(int x)=>x*2;").unwrap();
        assert!(matches!(&program.items[0], Item::Stmt(Stmt::VarDecl(_))));
    }

    #[test]
    fn parses_typed_extern_declarations() {
        let arena = Bump::new();
        let program = parse_source(&arena, "extern int hostAdd(int left,int right);").unwrap();
        let Item::Extern(extern_decl) = &program.items[0] else {
            panic!("expected extern declaration");
        };
        assert_eq!(extern_decl.name.name, "hostAdd");
        assert_eq!(extern_decl.params.len(), 2);
    }

    #[test]
    fn parses_imports_exports_aliases_and_pure_functions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"import { square as sq, Point } from "./math";
import "./startup.lil";
export pure int squared(int value) { return sq(value); }
export { Point };"#,
        )
        .unwrap();

        assert_eq!(program.imports.len(), 2);
        assert_eq!(program.imports[0].source, "./math");
        assert_eq!(program.imports[0].specifiers[0].local.name, "sq");
        assert!(program.imports[1].specifiers.is_empty());
        assert_eq!(program.exports.len(), 2);
        assert_eq!(program.exports[0].local.name, "squared");
        assert_eq!(program.exports[0].exported.name, "squared");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected exported function");
        };
        assert!(function.declared_pure);
    }

    #[test]
    fn parses_export_aliases() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int internalValue=7;export {internalValue as publicValue};",
        )
        .unwrap();

        assert_eq!(program.exports.len(), 1);
        assert_eq!(program.exports[0].local.name, "internalValue");
        assert_eq!(program.exports[0].exported.name, "publicValue");
    }

    #[test]
    fn parses_generic_functions_and_applied_class_types() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "T identity<T>(T value){return value;}class Box<T>{T value;init(T value){this.value=value;}}Box<int> box=new Box<int>(7);",
        )
        .unwrap();

        let Item::Function(function) = &program.items[0] else {
            panic!("expected generic function");
        };
        assert_eq!(function.type_params[0].name, "T");
        let Item::Class(class) = &program.items[1] else {
            panic!("expected generic class");
        };
        assert_eq!(class.type_params[0].name, "T");
        let Item::Stmt(Stmt::VarDecl(binding)) = &program.items[2] else {
            panic!("expected applied class binding");
        };
        assert!(matches!(
            binding.ty.kind,
            TypeKind::Named { name: "Box", args } if args.len() == 1
        ));
    }

    #[test]
    fn parses_nullable_types_and_null_literals() {
        let arena = Bump::new();
        let program = parse_source(&arena, "string? label=null;int?[] values=[null,1];").unwrap();

        let Item::Stmt(Stmt::VarDecl(label)) = &program.items[0] else {
            panic!("expected nullable binding");
        };
        assert!(matches!(label.ty.kind, TypeKind::Nullable(_)));
        assert!(matches!(label.initializer, Some(Expr::Null(_))));
        let Item::Stmt(Stmt::VarDecl(values)) = &program.items[1] else {
            panic!("expected nullable array binding");
        };
        assert!(matches!(
            values.ty.kind,
            TypeKind::Array(element) if matches!(element.kind, TypeKind::Nullable(_))
        ));
    }
}
