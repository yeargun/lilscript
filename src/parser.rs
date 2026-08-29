use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::ast::{
    ArrayBinding, ArrayElement, ArrowBody, AssignmentOp, BinaryOp, CatchBinding, CatchClause,
    ClassDecl, ClassMember, ConstructorDecl, EnumDecl, ExportDecl, ExportKind, Expr,
    ExternClassDecl, ExternClassMember, ExternDecl, ExternGlobalDecl, FieldDecl, ForInitializer,
    ForeignImportDecl, FunctionDecl, Ident, ImportDecl, ImportSpecifier, Item, MatchArm,
    MatchPattern, Param, Program, RecordBinding, RecordElement, RecordEntry, Stmt, StructDecl,
    TemplatePart, TypeKind, TypeRef, UnaryOp, UpdateOp, VarDecl,
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
        let mut foreign_imports = BumpVec::new_in(self.arena);
        let mut exports = BumpVec::new_in(self.arena);

        while !self.is_at_end() {
            if self.check(|kind| matches!(kind, TokenKind::Import))
                && !self.check_next(|kind| matches!(kind, TokenKind::LParen))
            {
                self.advance();
                if self.match_kind(|kind| matches!(kind, TokenKind::Extern)) {
                    foreign_imports.push(self.parse_foreign_import_after_keyword()?);
                } else {
                    imports.push(self.parse_import_after_keyword()?);
                }
                continue;
            }
            if self.match_kind(|kind| matches!(kind, TokenKind::Export)) {
                let export_start = self.previous_span();
                if self.match_kind(|kind| matches!(kind, TokenKind::LBrace)) {
                    self.parse_export_list_after_open(export_start, &mut exports)?;
                    continue;
                }
                if matches!(self.peek_kind(), Some(TokenKind::Ident("constructor"))) {
                    self.advance();
                    let local =
                        self.expect_ident("expected class name after `export constructor`")?;
                    let exported = if self.match_kind(|kind| matches!(kind, TokenKind::As)) {
                        self.expect_ident("expected export alias after `as`")?
                    } else {
                        local
                    };
                    let semi = self.expect_semicolon()?;
                    exports.push(ExportDecl {
                        local,
                        exported,
                        kind: ExportKind::ConstructorValue,
                        span: export_start.merge(semi.span),
                    });
                    continue;
                }
                let item = self.parse_item()?;
                let local = exported_item_name(&item).ok_or_else(|| {
                    ParseError::new(item.span(), "only declarations can be exported")
                })?;
                exports.push(ExportDecl {
                    local,
                    exported: local,
                    kind: ExportKind::Binding,
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
            foreign_imports: foreign_imports.into_bump_slice(),
            dynamic_imports: &[],
            module_bindings: &[],
            exports: exports.into_bump_slice(),
            items: items.into_bump_slice(),
            span,
        })
    }

    fn parse_foreign_import_after_keyword(
        &mut self,
    ) -> Result<ForeignImportDecl<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        if let Some(TokenKind::StringLiteral(raw)) = self.peek_kind() {
            let source = strip_quotes(raw);
            self.advance();
            let semi = self.expect_semicolon()?;
            return Ok(ForeignImportDecl {
                specifiers: &[],
                source,
                span: start.merge(semi.span),
            });
        }
        let specifiers = self.parse_import_specifiers()?;
        let (source, end) = self.parse_import_source()?;
        Ok(ForeignImportDecl {
            specifiers,
            source,
            span: start.merge(end),
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

        let specifiers = self.parse_import_specifiers()?;
        let (source, end) = self.parse_import_source()?;
        Ok(ImportDecl {
            specifiers,
            source,
            span: start.merge(end),
        })
    }

    fn parse_import_specifiers(&mut self) -> Result<&'arena [ImportSpecifier<'src>], ParseError> {
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` after import",
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
        Ok(specifiers.into_bump_slice())
    }

    fn parse_import_source(&mut self) -> Result<(&'src str, Span), ParseError> {
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
        Ok((strip_quotes(raw), semi.span))
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
                    kind: ExportKind::Binding,
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
        let is_async = self.match_kind(|kind| matches!(kind, TokenKind::Async));
        let is_generator = self.match_kind(|kind| matches!(kind, TokenKind::Generator));
        if is_async && is_generator {
            return Err(self.error_here("async generators are not yet part of the portable core"));
        }
        if declared_pure && (is_async || is_generator) {
            return Err(self.error_here("async and generator functions cannot be declared `pure`"));
        }
        if self.match_kind(|kind| matches!(kind, TokenKind::Extern)) {
            if is_async || is_generator {
                return Err(self.error_here("externs must declare deferred return types directly"));
            }
            if self.match_kind(|kind| matches!(kind, TokenKind::Class)) {
                if declared_pure {
                    return Err(self.error_here("`pure` cannot modify an extern class declaration"));
                }
                return self
                    .parse_extern_class_after_keyword()
                    .map(Item::ExternClass);
            }
            return self.parse_extern_after_keyword(declared_pure);
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Struct)) {
            if declared_pure || is_async || is_generator {
                return Err(self.error_here("modifiers can only apply to functions"));
            }
            return self.parse_struct_after_keyword().map(Item::Struct);
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Enum)) {
            if declared_pure || is_async || is_generator {
                return Err(self.error_here("modifiers cannot apply to an enum declaration"));
            }
            return self.parse_enum_after_keyword().map(Item::Enum);
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Class)) {
            if declared_pure || is_async || is_generator {
                return Err(self.error_here("modifiers can only apply to functions"));
            }
            return self.parse_class_after_keyword(false).map(Item::Class);
        }
        if self.looks_like_object_declaration() {
            if declared_pure || is_async || is_generator {
                return Err(self.error_here("modifiers can only apply to functions"));
            }
            self.advance();
            return self.parse_class_after_keyword(true).map(Item::Class);
        }

        if self.looks_like_typed_binding() {
            let ty = self.parse_type()?;
            let name = self.expect_ident("expected declaration name")?;
            let type_params = self.parse_type_params()?;
            if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                return self
                    .parse_function_after_signature(
                        ty,
                        name,
                        type_params,
                        declared_pure,
                        is_async,
                        is_generator,
                    )
                    .map(Item::Function);
            }

            if !type_params.is_empty() {
                return Err(ParseError::new(
                    name.span,
                    "type parameters require a function declaration",
                ));
            }

            if declared_pure || is_async || is_generator {
                return Err(ParseError::new(
                    name.span,
                    "modifiers can only apply to functions",
                ));
            }

            return self
                .parse_var_decl_after_name(ty, name)
                .map(|decl| Item::Stmt(Stmt::VarDecl(decl)));
        }

        if declared_pure || is_async || is_generator {
            return Err(self.error_here("expected function declaration after modifier"));
        }
        self.parse_statement().map(Item::Stmt)
    }

    fn parse_extern_after_keyword(
        &mut self,
        declared_pure: bool,
    ) -> Result<Item<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        let ty = self.parse_type()?;
        if ty.is_auto() {
            return Err(ParseError::new(ty.span, "extern type cannot be `auto`"));
        }
        let name = self.expect_ident("expected extern function name")?;
        let type_params = self.parse_type_params()?;
        if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
            let params = self.parse_params_after_open()?;
            let semi = self.expect_semicolon()?;
            return Ok(Item::Extern(ExternDecl {
                declared_pure,
                return_type: ty,
                name,
                type_params,
                params,
                span: start.merge(semi.span),
            }));
        }
        if declared_pure {
            return Err(ParseError::new(
                name.span,
                "`pure` can only modify extern functions and methods",
            ));
        }
        if !type_params.is_empty() {
            return Err(ParseError::new(
                name.span,
                "type parameters require an extern function",
            ));
        }
        let semi = self.expect_semicolon()?;
        Ok(Item::ExternGlobal(ExternGlobalDecl {
            ty,
            name,
            span: start.merge(semi.span),
        }))
    }

    fn parse_extern_class_after_keyword(
        &mut self,
    ) -> Result<ExternClassDecl<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        let name = self.expect_ident("expected extern class name")?;
        let type_params = self.parse_type_params()?;
        let base = if self.match_kind(|kind| matches!(kind, TokenKind::Extends)) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(|kind| matches!(kind, TokenKind::LBrace), "expected `{`")?;
        let mut members = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated extern class declaration"));
            }
            if self.check(|kind| matches!(kind, TokenKind::Init)) {
                return Err(self.error_here("extern classes cannot declare `init`"));
            }
            let declared_pure = self.match_kind(|kind| matches!(kind, TokenKind::Pure));
            let is_async = self.match_kind(|kind| matches!(kind, TokenKind::Async));
            if is_async {
                return Err(self.error_here(
                    "`async` extern methods must declare a `Task<T>` return directly",
                ));
            }
            let ty = self.parse_type()?;
            let member_name = self.expect_property_ident("expected extern class member name")?;
            let member_type_params = self.parse_type_params()?;
            if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                let params = self.parse_params_after_open()?;
                let semi = self.expect_semicolon()?;
                members.push(ExternClassMember::Method(ExternDecl {
                    declared_pure,
                    return_type: ty,
                    name: member_name,
                    type_params: member_type_params,
                    params,
                    span: ty.span.merge(semi.span),
                }));
            } else {
                if declared_pure {
                    return Err(ParseError::new(
                        member_name.span,
                        "`pure` can only modify extern methods",
                    ));
                }
                if !member_type_params.is_empty() {
                    return Err(ParseError::new(
                        member_name.span,
                        "type parameters require an extern method",
                    ));
                }
                members.push(ExternClassMember::Field(
                    self.parse_field_decl_after_name(ty, member_name)?,
                ));
            }
        }
        let close = self.expect(|kind| matches!(kind, TokenKind::RBrace), "expected `}`")?;
        Ok(ExternClassDecl {
            name,
            type_params,
            base,
            members: members.into_bump_slice(),
            span: start.merge(close.span),
        })
    }

    fn parse_statement(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        if self.match_kind(|kind| matches!(kind, TokenKind::Return)) {
            return self.parse_return_after_keyword();
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Throw)) {
            let start = self.previous_span();
            let value = self.parse_expression()?;
            let semi = self.expect_semicolon()?;
            return Ok(Stmt::Throw {
                value,
                span: start.merge(semi.span),
            });
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Super)) {
            let start = self.previous_span();
            self.expect(
                |kind| matches!(kind, TokenKind::LParen),
                "expected `(` after `super`",
            )?;
            let (args, _) = self.parse_args_after_open()?;
            let semi = self.expect_semicolon()?;
            return Ok(Stmt::SuperCall {
                args,
                span: start.merge(semi.span),
            });
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Yield)) {
            let start = self.previous_span();
            let delegate = self.match_kind(|kind| matches!(kind, TokenKind::Star));
            let value = self.parse_expression()?;
            let semi = self.expect_semicolon()?;
            return Ok(Stmt::Yield {
                value,
                delegate,
                span: start.merge(semi.span),
            });
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::Try)) {
            return self.parse_try_after_keyword();
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

        if self.looks_like_inline_for() {
            self.advance();
            self.expect(
                |kind| matches!(kind, TokenKind::For),
                "expected `for` after `inline`",
            )?;
            return self.parse_for_after_keyword(true);
        }

        if self.match_kind(|kind| matches!(kind, TokenKind::For)) {
            return self.parse_for_after_keyword(false);
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

        if self.check(|kind| matches!(kind, TokenKind::Auto))
            && self.check_next(|kind| matches!(kind, TokenKind::LBracket | TokenKind::LBrace))
        {
            self.advance();
            let start = self.previous_span();
            return if self.match_kind(|kind| matches!(kind, TokenKind::LBracket)) {
                self.parse_array_destructure_after_open(start)
            } else {
                self.expect(
                    |kind| matches!(kind, TokenKind::LBrace),
                    "expected destructuring pattern",
                )?;
                self.parse_record_destructure_after_open(start)
            };
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

    fn parse_try_after_keyword(&mut self) -> Result<Stmt<'arena, 'src>, ParseError> {
        let start = self.previous_span();
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` after `try`",
        )?;
        let (body, body_span) = self.parse_block_after_open()?;

        let catch = if self.match_kind(|kind| matches!(kind, TokenKind::Catch)) {
            let catch_start = self.previous_span();
            let binding = if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                let ty = self.parse_type()?;
                let name = self.expect_ident("expected catch binding name")?;
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RParen),
                    "expected `)` after catch binding",
                )?;
                Some(CatchBinding {
                    ty,
                    name,
                    span: ty.span.merge(close.span),
                })
            } else {
                None
            };
            self.expect(
                |kind| matches!(kind, TokenKind::LBrace),
                "expected `{` after `catch`",
            )?;
            let (catch_body, catch_span) = self.parse_block_after_open()?;
            Some(CatchClause {
                binding,
                body: catch_body,
                span: catch_start.merge(catch_span),
            })
        } else {
            None
        };

        let finally = if self.match_kind(|kind| matches!(kind, TokenKind::Finally)) {
            self.expect(
                |kind| matches!(kind, TokenKind::LBrace),
                "expected `{` after `finally`",
            )?;
            Some(self.parse_block_after_open()?)
        } else {
            None
        };
        if catch.is_none() && finally.is_none() {
            return Err(ParseError::new(
                body_span,
                "`try` requires a `catch` or `finally` clause",
            ));
        }
        let end = finally.as_ref().map_or_else(
            || catch.as_ref().map_or(body_span, |clause| clause.span),
            |(_, span)| *span,
        );
        Ok(Stmt::Try {
            body,
            catch,
            finally: finally.map(|(body, _)| body),
            span: start.merge(end),
        })
    }

    fn parse_array_destructure_after_open(
        &mut self,
        start: Span,
    ) -> Result<Stmt<'arena, 'src>, ParseError> {
        let mut bindings = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBracket)) {
            if self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                bindings.push(ArrayBinding::Hole(self.previous_span()));
                continue;
            }
            if self.match_kind(|kind| matches!(kind, TokenKind::Ellipsis)) {
                let name = self.expect_ident("expected rest binding name")?;
                bindings.push(ArrayBinding::Rest(name));
                if self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    return Err(ParseError::new(
                        self.previous_span(),
                        "array rest binding must be last",
                    ));
                }
                break;
            }
            bindings.push(ArrayBinding::Name(
                self.expect_ident("expected array binding name")?,
            ));
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
        }
        if bindings.is_empty() {
            return Err(ParseError::new(
                start,
                "array destructuring requires a binding",
            ));
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RBracket),
            "expected `]` after array bindings",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Eq),
            "expected `=` after destructuring pattern",
        )?;
        let value = self.parse_expression()?;
        let semi = self.expect_semicolon()?;
        Ok(Stmt::ArrayDestructure {
            bindings: bindings.into_bump_slice(),
            value,
            span: start.merge(semi.span),
        })
    }

    fn parse_record_destructure_after_open(
        &mut self,
        start: Span,
    ) -> Result<Stmt<'arena, 'src>, ParseError> {
        let mut bindings = BumpVec::new_in(self.arena);
        let mut rest = None;
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.match_kind(|kind| matches!(kind, TokenKind::Ellipsis)) {
                rest = Some(self.expect_ident("expected record rest binding name")?);
                if self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    return Err(ParseError::new(
                        self.previous_span(),
                        "record rest binding must be last",
                    ));
                }
                break;
            }
            let token = self
                .advance()
                .ok_or_else(|| self.error_here("expected record binding key"))?;
            let (key, quoted) = match token.kind {
                TokenKind::StringLiteral(raw) => (
                    Ident {
                        name: strip_quotes(raw),
                        span: token.span,
                    },
                    true,
                ),
                kind => (
                    Ident {
                        name: property_identifier_name(kind).ok_or_else(|| {
                            ParseError::new(token.span, "expected record binding key")
                        })?,
                        span: token.span,
                    },
                    false,
                ),
            };
            let name = if self.match_kind(|kind| matches!(kind, TokenKind::Colon)) {
                self.expect_ident("expected record binding name")?
            } else {
                if quoted {
                    return Err(ParseError::new(
                        token.span,
                        "quoted record keys require a binding name",
                    ));
                }
                key
            };
            bindings.push(RecordBinding {
                key,
                name,
                span: key.span.merge(name.span),
            });
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
            if self.check(|kind| matches!(kind, TokenKind::RBrace)) {
                break;
            }
        }
        if bindings.is_empty() && rest.is_none() {
            return Err(ParseError::new(
                start,
                "record destructuring requires a binding",
            ));
        }
        self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after record bindings",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Eq),
            "expected `=` after destructuring pattern",
        )?;
        let value = self.parse_expression()?;
        let semi = self.expect_semicolon()?;
        Ok(Stmt::RecordDestructure {
            bindings: bindings.into_bump_slice(),
            rest,
            value,
            span: start.merge(semi.span),
        })
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

    fn parse_enum_after_keyword(&mut self) -> Result<EnumDecl<'arena, 'src>, ParseError> {
        let keyword_span = self.previous_span();
        let name = self.expect_ident("expected enum name")?;
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` after enum name",
        )?;
        let mut variants = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated enum declaration"));
            }
            variants.push(self.expect_property_ident("expected enum variant")?);
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
        }
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after enum variants",
        )?;
        if variants.is_empty() {
            return Err(ParseError::new(
                name.span,
                "an enum requires at least one variant",
            ));
        }
        Ok(EnumDecl {
            name,
            variants: variants.into_bump_slice(),
            span: keyword_span.merge(close.span),
        })
    }

    fn parse_class_after_keyword(
        &mut self,
        object: bool,
    ) -> Result<ClassDecl<'arena, 'src>, ParseError> {
        let keyword_span = self.previous_span();
        let name = self.expect_ident(if object {
            "expected object name"
        } else {
            "expected class name"
        })?;
        let type_params = self.parse_type_params()?;
        if object && !type_params.is_empty() {
            return Err(ParseError::new(
                name.span,
                "objects cannot declare type parameters",
            ));
        }
        let base = if self.match_kind(|kind| matches!(kind, TokenKind::Extends)) {
            if object {
                return Err(self.error_here("objects cannot extend a type"));
            }
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(|kind| matches!(kind, TokenKind::LBrace), "expected `{`")?;

        let mut members = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.is_at_end() {
                return Err(self.error_here("unterminated class declaration"));
            }

            if self.match_kind(|kind| matches!(kind, TokenKind::Init)) {
                if object {
                    return Err(self.error_here("objects cannot declare `init`"));
                }
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
            let is_async = self.match_kind(|kind| matches!(kind, TokenKind::Async));
            let is_generator = self.match_kind(|kind| matches!(kind, TokenKind::Generator));
            if is_async && is_generator {
                return Err(self.error_here("async generator methods are not yet supported"));
            }
            if declared_pure && (is_async || is_generator) {
                return Err(
                    self.error_here("async and generator methods cannot be declared `pure`")
                );
            }
            let ty = self.parse_type()?;
            let member_name = self.expect_property_ident("expected class member name")?;
            let type_params = self.parse_type_params()?;
            if self.match_kind(|kind| matches!(kind, TokenKind::LParen)) {
                let method = self.parse_function_after_signature(
                    ty,
                    member_name,
                    type_params,
                    declared_pure,
                    is_async,
                    is_generator,
                )?;
                members.push(ClassMember::Method(method));
            } else {
                if !type_params.is_empty() {
                    return Err(ParseError::new(
                        member_name.span,
                        "type parameters require a method declaration",
                    ));
                }
                if declared_pure || is_async || is_generator {
                    return Err(ParseError::new(
                        member_name.span,
                        "modifiers can only apply to methods",
                    ));
                }
                let field = self.parse_field_decl_after_name(ty, member_name)?;
                if object {
                    return Err(ParseError::new(
                        field.span,
                        "objects declare methods, not fields",
                    ));
                }
                members.push(ClassMember::Field(field));
            }
        }

        let close = self.expect(|kind| matches!(kind, TokenKind::RBrace), "expected `}`")?;
        Ok(ClassDecl {
            name,
            type_params,
            base,
            members: members.into_bump_slice(),
            object,
            span: keyword_span.merge(close.span),
        })
    }

    fn parse_field_decl(&mut self) -> Result<FieldDecl<'arena, 'src>, ParseError> {
        let ty = self.parse_type()?;
        let name = self.expect_property_ident("expected field name")?;
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
        is_async: bool,
        is_generator: bool,
    ) -> Result<FunctionDecl<'arena, 'src>, ParseError> {
        let params = self.parse_params_after_open()?;
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected function body",
        )?;
        let (body, body_span) = self.parse_block_after_open()?;
        Ok(FunctionDecl {
            declared_pure,
            is_async,
            is_generator,
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

    fn looks_like_inline_for(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Ident("inline")))
            && self.check_next(|kind| matches!(kind, TokenKind::For))
    }

    fn parse_for_after_keyword(&mut self, inline: bool) -> Result<Stmt<'arena, 'src>, ParseError> {
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
            if self.match_kind(|kind| matches!(kind, TokenKind::In)) {
                if inline {
                    return Err(ParseError::new(
                        start,
                        "`inline for` requires `for (T name of constList)`",
                    ));
                }
                let object = self.parse_expression()?;
                self.expect(
                    |kind| matches!(kind, TokenKind::RParen),
                    "expected `)` after for-in object",
                )?;
                let body = self.arena.alloc(self.parse_statement()?);
                return Ok(Stmt::ForIn {
                    key_type: ty,
                    key: name,
                    object,
                    span: start.merge(body.span()),
                    body,
                });
            }
            if self.match_kind(|kind| matches!(kind, TokenKind::Of)) {
                let iterable = self.parse_expression()?;
                self.expect(
                    |kind| matches!(kind, TokenKind::RParen),
                    "expected `)` after for-of iterable",
                )?;
                let body = self.arena.alloc(self.parse_statement()?);
                return Ok(Stmt::ForOf {
                    element_type: ty,
                    element: name,
                    iterable,
                    inline,
                    span: start.merge(body.span()),
                    body,
                });
            }
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
        if inline {
            return Err(ParseError::new(
                start,
                "`inline for` requires `for (T name of constList)`",
            ));
        }
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
        let first = self.parse_postfix_type()?;
        if !self.match_kind(|kind| matches!(kind, TokenKind::Pipe)) {
            return Ok(first);
        }

        let mut members = BumpVec::new_in(self.arena);
        members.push(first);
        loop {
            members.push(self.parse_postfix_type()?);
            if !self.match_kind(|kind| matches!(kind, TokenKind::Pipe)) {
                break;
            }
        }
        let span = members
            .first()
            .expect("union has a first type")
            .span
            .merge(members.last().expect("union has a last type").span);
        Ok(TypeRef {
            kind: TypeKind::Union(members.into_bump_slice()),
            span,
        })
    }

    fn parse_postfix_type(&mut self) -> Result<TypeRef<'arena, 'src>, ParseError> {
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
            TokenKind::Number => TypeRef {
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
            TokenKind::LParen => {
                let inner = self.parse_type()?;
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RParen),
                    "expected `)` after parenthesized type",
                )?;
                TypeRef {
                    kind: inner.kind,
                    span: token.span.merge(close.span),
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
            TokenKind::From => TypeRef::named("from", token.span),
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
            Some(TokenKind::QuestionQuestionEq) => AssignmentOp::Nullish,
            Some(TokenKind::PlusEq) => AssignmentOp::Add,
            Some(TokenKind::MinusEq) => AssignmentOp::Sub,
            Some(TokenKind::StarEq) => AssignmentOp::Mul,
            Some(TokenKind::SlashEq) => AssignmentOp::Div,
            Some(TokenKind::PercentEq) => AssignmentOp::Mod,
            Some(TokenKind::AmpersandEq) => AssignmentOp::BitAnd,
            Some(TokenKind::PipeEq) => AssignmentOp::BitOr,
            Some(TokenKind::CaretEq) => AssignmentOp::Xor,
            Some(TokenKind::ShiftLeftEq) => AssignmentOp::ShiftLeft,
            Some(TokenKind::ShiftRightEq) => AssignmentOp::ShiftRight,
            Some(TokenKind::UnsignedShiftRightEq) => AssignmentOp::UnsignedShiftRight,
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

        loop {
            if self.check(|kind| matches!(kind, TokenKind::Is)) {
                let precedence = 4;
                if precedence < min_precedence {
                    break;
                }
                self.advance();
                let target = self.parse_type()?;
                let value = self.arena.alloc(lhs);
                lhs = Expr::TypeCheck {
                    value,
                    target,
                    span: value.span().merge(target.span),
                };
                continue;
            }

            let Some((op, precedence)) = self.peek_binary_op() else {
                break;
            };
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
        if self.match_kind(|kind| matches!(kind, TokenKind::Await)) {
            let start = self.previous_span();
            let task = self.parse_unary_expression()?;
            let task = self.arena.alloc(task);
            return Ok(Expr::Await {
                task,
                span: start.merge(task.span()),
            });
        }

        let update = if self.match_kind(|kind| matches!(kind, TokenKind::PlusPlus)) {
            Some(UpdateOp::Increment)
        } else if self.match_kind(|kind| matches!(kind, TokenKind::MinusMinus)) {
            Some(UpdateOp::Decrement)
        } else {
            None
        };
        if let Some(op) = update {
            let op_span = self.previous_span();
            let target = self.parse_unary_expression()?;
            let target = self.arena.alloc(target);
            return Ok(Expr::Update {
                op,
                target,
                prefix: true,
                span: op_span.merge(target.span()),
            });
        }

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
            if self.match_kind(|kind| matches!(kind, TokenKind::QuestionDot)) {
                if self.match_kind(|kind| matches!(kind, TokenKind::LBracket)) {
                    let index = self.parse_expression()?;
                    let close = self.expect(
                        |kind| matches!(kind, TokenKind::RBracket),
                        "expected `]` after optional index",
                    )?;
                    let object = self.arena.alloc(expr);
                    let index = self.arena.alloc(index);
                    expr = Expr::OptionalIndex {
                        object,
                        index,
                        span: object.span().merge(close.span),
                    };
                } else {
                    let property =
                        self.expect_property_ident("expected property name after `?.`")?;
                    let object = self.arena.alloc(expr);
                    expr = Expr::OptionalMember {
                        object,
                        property,
                        span: object.span().merge(property.span),
                    };
                }
                continue;
            }
            if self.match_kind(|kind| matches!(kind, TokenKind::Dot)) {
                let property = self.expect_property_ident("expected property name after `.`")?;
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
            TokenKind::If => self.parse_if_expression(token.span),
            TokenKind::Match => self.parse_match_expression(token.span),
            TokenKind::Record => self.parse_record_literal(token.span),
            TokenKind::Import => {
                self.expect(
                    |kind| matches!(kind, TokenKind::LParen),
                    "expected `(` after dynamic `import`",
                )?;
                let path = self
                    .advance()
                    .ok_or_else(|| self.error_here("expected module path in dynamic `import`"))?;
                let TokenKind::StringLiteral(raw) = path.kind else {
                    return Err(ParseError::new(
                        path.span,
                        "dynamic `import` requires a static string module path",
                    ));
                };
                let close = self.expect(
                    |kind| matches!(kind, TokenKind::RParen),
                    "expected `)` after dynamic module path",
                )?;
                Ok(Expr::DynamicImport {
                    source: strip_quotes(raw),
                    span: token.span.merge(close.span),
                })
            }
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
                    if name == "object" {
                        let literal = self.parse_record_literal_after_open(token.span)?;
                        let Expr::RecordLiteral { entries, span } = literal else {
                            unreachable!();
                        };
                        return Ok(Expr::ObjectLiteral { entries, span });
                    }
                    return self.parse_struct_literal_after_open(ident);
                }
                Ok(Expr::Ident(ident))
            }
            TokenKind::From => {
                let ident = Ident {
                    name: "from",
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

    fn parse_if_expression(
        &mut self,
        keyword_span: Span,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        self.expect(
            |kind| matches!(kind, TokenKind::LParen),
            "expected `(` after expression `if`",
        )?;
        let condition = self.parse_expression()?;
        self.expect(
            |kind| matches!(kind, TokenKind::RParen),
            "expected `)` after expression-if condition",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` before expression-if value",
        )?;
        let then_value = self.parse_expression()?;
        self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after expression-if value",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::Else),
            "expression `if` requires `else`",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` before expression-if else value",
        )?;
        let else_value = self.parse_expression()?;
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after expression-if else value",
        )?;
        Ok(Expr::If {
            condition: self.arena.alloc(condition),
            then_value: self.arena.alloc(then_value),
            else_value: self.arena.alloc(else_value),
            span: keyword_span.merge(close.span),
        })
    }

    fn parse_match_expression(
        &mut self,
        keyword_span: Span,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        self.expect(
            |kind| matches!(kind, TokenKind::LParen),
            "expected `(` after `match`",
        )?;
        let value = self.parse_expression()?;
        self.expect(
            |kind| matches!(kind, TokenKind::RParen),
            "expected `)` after match value",
        )?;
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` before match arms",
        )?;
        let mut arms = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            let pattern_token = self
                .advance()
                .ok_or_else(|| self.error_here("expected match pattern"))?;
            let pattern = match pattern_token.kind {
                TokenKind::Ident("_") => MatchPattern::Wildcard(pattern_token.span),
                TokenKind::Ident(name) => {
                    let enum_name = Ident {
                        name,
                        span: pattern_token.span,
                    };
                    self.expect(
                        |kind| matches!(kind, TokenKind::Dot),
                        "expected `.` in enum pattern",
                    )?;
                    let variant = self.expect_property_ident("expected enum variant")?;
                    MatchPattern::EnumVariant {
                        enum_name,
                        variant,
                        span: enum_name.span.merge(variant.span),
                    }
                }
                TokenKind::IntLiteral(value) => MatchPattern::Int(value, pattern_token.span),
                TokenKind::StringLiteral(raw) => {
                    MatchPattern::String(strip_quotes(raw), pattern_token.span)
                }
                TokenKind::True => MatchPattern::Bool(true, pattern_token.span),
                TokenKind::False => MatchPattern::Bool(false, pattern_token.span),
                TokenKind::Minus => {
                    let literal = self.advance().ok_or_else(|| {
                        ParseError::new(pattern_token.span, "expected integer after `-`")
                    })?;
                    let TokenKind::IntLiteral(value) = literal.kind else {
                        return Err(ParseError::new(
                            literal.span,
                            "only integer literals may be negative match patterns",
                        ));
                    };
                    MatchPattern::Int(
                        value.checked_neg().ok_or_else(|| {
                            ParseError::new(literal.span, "negative match pattern is out of range")
                        })?,
                        pattern_token.span.merge(literal.span),
                    )
                }
                _ => {
                    return Err(ParseError::new(
                        pattern_token.span,
                        "expected enum, int, string, bool, or `_` match pattern",
                    ));
                }
            };
            self.expect(
                |kind| matches!(kind, TokenKind::FatArrow),
                "expected `=>` after match pattern",
            )?;
            let value = self.parse_expression()?;
            let span = pattern.span().merge(value.span());
            arms.push(MatchArm {
                pattern,
                value,
                span,
            });
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
        }
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after match arms",
        )?;
        if arms.is_empty() {
            return Err(ParseError::new(
                keyword_span.merge(close.span),
                "a match expression requires at least one arm",
            ));
        }
        Ok(Expr::Match {
            value: self.arena.alloc(value),
            arms: arms.into_bump_slice(),
            span: keyword_span.merge(close.span),
        })
    }

    fn parse_record_literal(
        &mut self,
        keyword_span: Span,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        self.expect(
            |kind| matches!(kind, TokenKind::LBrace),
            "expected `{` after `record`",
        )?;
        self.parse_record_literal_after_open(keyword_span)
    }

    fn parse_record_literal_after_open(
        &mut self,
        keyword_span: Span,
    ) -> Result<Expr<'arena, 'src>, ParseError> {
        let mut entries = BumpVec::new_in(self.arena);
        while !self.check(|kind| matches!(kind, TokenKind::RBrace)) {
            if self.match_kind(|kind| matches!(kind, TokenKind::Ellipsis)) {
                let spread = self.previous_span();
                let value = self.parse_expression()?;
                entries.push(RecordElement::Spread {
                    span: spread.merge(value.span()),
                    value,
                });
                if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                    break;
                }
                if self.check(|kind| matches!(kind, TokenKind::RBrace)) {
                    break;
                }
                continue;
            }
            let token = self
                .advance()
                .ok_or_else(|| self.error_here("expected record key"))?;
            let key = match token.kind {
                TokenKind::StringLiteral(raw) => Ident {
                    name: strip_quotes(raw),
                    span: token.span,
                },
                kind => Ident {
                    name: property_identifier_name(kind)
                        .ok_or_else(|| ParseError::new(token.span, "expected record key"))?,
                    span: token.span,
                },
            };
            self.expect(
                |kind| matches!(kind, TokenKind::Colon),
                "expected `:` after record key",
            )?;
            let value = self.parse_expression()?;
            entries.push(RecordElement::Entry(RecordEntry {
                key,
                span: key.span.merge(value.span()),
                value,
            }));
            if !self.match_kind(|kind| matches!(kind, TokenKind::Comma)) {
                break;
            }
            if self.check(|kind| matches!(kind, TokenKind::RBrace)) {
                break;
            }
        }
        let close = self.expect(
            |kind| matches!(kind, TokenKind::RBrace),
            "expected `}` after record entries",
        )?;
        Ok(Expr::RecordLiteral {
            entries: entries.into_bump_slice(),
            span: keyword_span.merge(close.span),
        })
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
                if self.match_kind(|kind| matches!(kind, TokenKind::Ellipsis)) {
                    let spread = self.previous_span();
                    let value = self.parse_expression()?;
                    elements.push(ArrayElement::Spread {
                        span: spread.merge(value.span()),
                        value,
                    });
                } else {
                    elements.push(ArrayElement::Value(self.parse_expression()?));
                }
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
        let mut saw_default = false;
        if !self.check(|kind| matches!(kind, TokenKind::RParen)) {
            loop {
                let ty = self.parse_type()?;
                let name = self.expect_ident("expected parameter name")?;
                let default = if self.match_kind(|kind| matches!(kind, TokenKind::Eq)) {
                    saw_default = true;
                    Some(self.parse_expression()?)
                } else {
                    if saw_default {
                        return Err(ParseError::new(
                            name.span,
                            "required parameters cannot follow defaulted parameters",
                        ));
                    }
                    None
                };
                let span = default.as_ref().map_or(ty.span.merge(name.span), |value| {
                    ty.span.merge(value.span())
                });
                params.push(Param {
                    ty,
                    name,
                    default,
                    span,
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

    fn looks_like_object_declaration(&self) -> bool {
        matches!(self.peek_kind(), Some(TokenKind::Ident("object")))
            && self.check_next(|kind| matches!(kind, TokenKind::Ident(_) | TokenKind::From))
            && matches!(
                self.tokens.get(self.cursor + 2).map(|token| &token.kind),
                Some(TokenKind::LBrace | TokenKind::Extends | TokenKind::Less)
            )
    }

    fn looks_like_typed_binding(&self) -> bool {
        let Some(type_end) = self.scan_type_end(self.cursor) else {
            return false;
        };
        matches!(
            self.tokens.get(type_end).map(|token| &token.kind),
            Some(TokenKind::Ident(_) | TokenKind::From)
        )
    }

    fn scan_type_end(&self, start: usize) -> Option<usize> {
        let mut index = start;
        match self.tokens.get(index).map(|token| &token.kind) {
            Some(
                TokenKind::Int
                | TokenKind::Float
                | TokenKind::Number
                | TokenKind::String
                | TokenKind::Bool
                | TokenKind::Void
                | TokenKind::Auto,
            ) => index += 1,
            Some(TokenKind::Ident(_) | TokenKind::From) => {
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
            Some(TokenKind::LParen) => {
                index = self.scan_type_end(index + 1)?;
                if !matches!(self.tokens.get(index)?.kind, TokenKind::RParen) {
                    return None;
                }
                index += 1;
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

        if matches!(
            self.tokens.get(index).map(|token| &token.kind),
            Some(TokenKind::Pipe)
        ) {
            index = self.scan_type_end(index + 1)?;
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
            TokenKind::QuestionQuestion => (BinaryOp::Nullish, 1),
            TokenKind::OrOr => (BinaryOp::Or, 1),
            TokenKind::AndAnd => (BinaryOp::And, 2),
            TokenKind::Pipe => (BinaryOp::BitOr, 3),
            TokenKind::Caret => (BinaryOp::Xor, 4),
            TokenKind::Ampersand => (BinaryOp::BitAnd, 5),
            TokenKind::EqEq => (BinaryOp::Eq, 6),
            TokenKind::BangEq => (BinaryOp::NotEq, 6),
            TokenKind::Less => (BinaryOp::Less, 7),
            TokenKind::LessEq => (BinaryOp::LessEq, 7),
            TokenKind::Greater => (BinaryOp::Greater, 7),
            TokenKind::GreaterEq => (BinaryOp::GreaterEq, 7),
            TokenKind::ShiftLeft => (BinaryOp::ShiftLeft, 8),
            TokenKind::ShiftRight => (BinaryOp::ShiftRight, 8),
            TokenKind::UnsignedShiftRight => (BinaryOp::UnsignedShiftRight, 8),
            TokenKind::Plus => (BinaryOp::Add, 9),
            TokenKind::Minus => (BinaryOp::Sub, 9),
            TokenKind::Star => (BinaryOp::Mul, 10),
            TokenKind::Slash => (BinaryOp::Div, 10),
            TokenKind::Percent => (BinaryOp::Mod, 10),
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
            TokenKind::From => Ok(Ident {
                name: "from",
                span: token.span,
            }),
            _ => Err(ParseError::new(token.span, message)),
        }
    }

    fn expect_property_ident(&mut self, message: &'static str) -> Result<Ident<'src>, ParseError> {
        let token = self.advance().ok_or_else(|| self.error_here(message))?;
        let name = property_identifier_name(token.kind)
            .ok_or_else(|| ParseError::new(token.span, message))?;
        Ok(Ident {
            name,
            span: token.span,
        })
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

    fn check_next(&self, predicate: impl FnOnce(&TokenKind<'src>) -> bool) -> bool {
        self.tokens
            .get(self.cursor + 1)
            .map(|token| &token.kind)
            .is_some_and(predicate)
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

fn property_identifier_name<'src>(kind: TokenKind<'src>) -> Option<&'src str> {
    Some(match kind {
        TokenKind::Ident(name) => name,
        TokenKind::Int => "int",
        TokenKind::Float => "float",
        TokenKind::Number => "number",
        TokenKind::String => "string",
        TokenKind::Bool => "bool",
        TokenKind::Void => "void",
        TokenKind::Auto => "auto",
        TokenKind::Func => "func",
        TokenKind::Struct => "struct",
        TokenKind::Record => "record",
        TokenKind::Enum => "enum",
        TokenKind::Class => "class",
        TokenKind::Extends => "extends",
        TokenKind::Super => "super",
        TokenKind::Return => "return",
        TokenKind::Init => "init",
        TokenKind::If => "if",
        TokenKind::Else => "else",
        TokenKind::While => "while",
        TokenKind::For => "for",
        TokenKind::In => "in",
        TokenKind::Of => "of",
        TokenKind::Break => "break",
        TokenKind::Continue => "continue",
        TokenKind::Extern => "extern",
        TokenKind::Import => "import",
        TokenKind::Export => "export",
        TokenKind::From => "from",
        TokenKind::As => "as",
        TokenKind::Pure => "pure",
        TokenKind::True => "true",
        TokenKind::False => "false",
        TokenKind::Null => "null",
        TokenKind::New => "new",
        TokenKind::Is => "is",
        TokenKind::Match => "match",
        TokenKind::Async => "async",
        TokenKind::Generator => "generator",
        TokenKind::Yield => "yield",
        TokenKind::Await => "await",
        TokenKind::Throw => "throw",
        TokenKind::Try => "try",
        TokenKind::Catch => "catch",
        TokenKind::Finally => "finally",
        _ => return None,
    })
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
        Item::Enum(decl) => Some(decl.name),
        Item::Struct(decl) => Some(decl.name),
        Item::Class(decl) => Some(decl.name),
        Item::ExternClass(decl) => Some(decl.name),
        Item::Function(decl) => Some(decl.name),
        Item::Extern(decl) => Some(decl.name),
        Item::ExternGlobal(decl) => Some(decl.name),
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
    fn parses_enum_and_match_expression() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "enum Status{Draft,Active,Sold}string label(Status value){return match(value){Status.Draft=>\"draft\",Status.Active=>\"active\",Status.Sold=>\"sold\"};}",
        )
        .unwrap();

        let Item::Enum(declaration) = &program.items[0] else {
            panic!("expected enum declaration");
        };
        assert_eq!(declaration.name.name, "Status");
        assert_eq!(declaration.variants.len(), 3);
        let Item::Function(function) = &program.items[1] else {
            panic!("expected function");
        };
        let Stmt::Return {
            value: Some(Expr::Match { arms, .. }),
            ..
        } = &function.body[0]
        else {
            panic!("expected match return value");
        };
        assert_eq!(arms.len(), 3);
    }

    #[test]
    fn parses_expression_if_with_a_required_else() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int choose(bool flag){return if(flag){1}else{if(false){2}else{3}};}",
        )
        .unwrap();
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function");
        };
        assert!(matches!(
            &function.body[0],
            Stmt::Return {
                value: Some(Expr::If { else_value, .. }),
                ..
            } if matches!(else_value, Expr::If { .. })
        ));

        let error = parse_source(&arena, "int choose(bool flag){return if(flag){1};}").unwrap_err();
        assert!(error.message.contains("requires `else`"), "{error}");
    }

    #[test]
    fn parses_scalar_literal_match_patterns() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string label(int value){return match(value){-1=>\"negative\",0=>\"zero\",_=>\"positive\"};}",
        )
        .unwrap();
        let Item::Function(function) = &program.items[0] else {
            panic!("expected function");
        };
        let Stmt::Return {
            value: Some(Expr::Match { arms, .. }),
            ..
        } = &function.body[0]
        else {
            panic!("expected match");
        };
        assert!(matches!(arms[0].pattern, MatchPattern::Int(-1, _)));
        assert!(matches!(arms[1].pattern, MatchPattern::Int(0, _)));
        assert!(matches!(arms[2].pattern, MatchPattern::Wildcard(_)));
    }

    #[test]
    fn parses_typed_for_of_statement() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2];for(int value of values){print(value);}",
        )
        .unwrap();
        assert!(matches!(
            &program.items[1],
            Item::Stmt(Stmt::ForOf { inline: false, .. })
        ));
    }

    #[test]
    fn parses_inline_for_over_const_list() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int total=0;inline for(int value of [1,2,3]){total+=value;}print(total);",
        )
        .unwrap();
        assert!(matches!(
            &program.items[1],
            Item::Stmt(Stmt::ForOf { inline: true, .. })
        ));
    }

    #[test]
    fn rejects_inline_for_in() {
        let arena = Bump::new();
        let error = parse_source(&arena, "inline for(string key in value){}").unwrap_err();
        assert!(error.message.contains("`inline for` requires"), "{error:?}");
    }

    #[test]
    fn parses_structural_record_literals() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "Record<int> values=record{alpha:1,\"beta-key\":2,};",
        )
        .unwrap();
        let Item::Stmt(Stmt::VarDecl(declaration)) = &program.items[0] else {
            panic!("expected record declaration");
        };
        let Some(Expr::RecordLiteral { entries, .. }) = &declaration.initializer else {
            panic!("expected record literal");
        };
        assert_eq!(entries.len(), 2);
        let RecordElement::Entry(second) = &entries[1] else {
            panic!("expected record entry");
        };
        assert_eq!(second.key.name, "beta-key");
    }

    #[test]
    fn parses_array_and_record_spreads_without_confusing_ranges() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] base=[1,2];int[] values=[0,...base,3];Record<int> source=record{a:1};Record<int> merged=record{...source,b:2};",
        )
        .unwrap();

        let Item::Stmt(Stmt::VarDecl(array)) = &program.items[1] else {
            panic!("expected array declaration");
        };
        let Some(Expr::ArrayLiteral { elements, .. }) = &array.initializer else {
            panic!("expected array literal");
        };
        assert!(matches!(elements[1], ArrayElement::Spread { .. }));

        let Item::Stmt(Stmt::VarDecl(record)) = &program.items[3] else {
            panic!("expected record declaration");
        };
        let Some(Expr::RecordLiteral { entries, .. }) = &record.initializer else {
            panic!("expected record literal");
        };
        assert!(matches!(entries[0], RecordElement::Spread { .. }));
    }

    #[test]
    fn parses_nullable_destructuring_bindings_and_array_rest() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2,3];auto [first,,third,...rest]=values;Record<int> source=record{a:1};auto {a,\"b-c\":bee,...remaining}=source;",
        )
        .unwrap();
        let Item::Stmt(Stmt::ArrayDestructure { bindings, .. }) = &program.items[1] else {
            panic!("expected array destructuring");
        };
        assert!(matches!(bindings[0], ArrayBinding::Name(_)));
        assert!(matches!(bindings[1], ArrayBinding::Hole(_)));
        assert!(matches!(bindings[3], ArrayBinding::Rest(_)));
        let Item::Stmt(Stmt::RecordDestructure { bindings, rest, .. }) = &program.items[3] else {
            panic!("expected record destructuring");
        };
        assert_eq!(bindings[1].key.name, "b-c");
        assert_eq!(bindings[1].name.name, "bee");
        assert_eq!(rest.as_ref().map(|name| name.name), Some("remaining"));

        let error = parse_source(&arena, "auto [first,...rest,last]=[1,2,3];").unwrap_err();
        assert!(
            error.message.contains("rest binding must be last"),
            "{error}"
        );
        let error = parse_source(&arena, "auto {a,...rest,b}=record{a:1,b:2};").unwrap_err();
        assert!(
            error.message.contains("rest binding must be last"),
            "{error}"
        );
    }

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
    fn parses_closed_object_methods() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "object Api{int add(int left,int right){return left+right;}}",
        )
        .unwrap();
        let Item::Class(decl) = &program.items[0] else {
            panic!("expected object declaration");
        };
        assert!(decl.object);
        assert_eq!(decl.name.name, "Api");
        assert_eq!(decl.members.len(), 1);
        assert!(matches!(&decl.members[0], ClassMember::Method(_)));
    }

    #[test]
    fn rejects_object_fields_and_constructors() {
        let arena = Bump::new();
        let field = parse_source(&arena, "object Api{int value;}").unwrap_err();
        assert!(field
            .message
            .contains("objects declare methods, not fields"));
        let init = parse_source(&arena, "object Api{init(){}}").unwrap_err();
        assert!(init.message.contains("objects cannot declare `init`"));
        let params = parse_source(&arena, "object Box<T>{int id(){return 1;}}").unwrap_err();
        assert!(params
            .message
            .contains("objects cannot declare type parameters"));
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
    fn parses_prefix_and_postfix_updates_at_unary_precedence() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int value=1;int first=++value*2;int second=value--; ",
        )
        .unwrap();
        let Item::Stmt(Stmt::VarDecl(first)) = &program.items[1] else {
            panic!("expected first declaration");
        };
        assert!(matches!(
            first.initializer,
            Some(Expr::Binary {
                lhs,
                op: BinaryOp::Mul,
                ..
            }) if matches!(lhs, Expr::Update { prefix: true, op: UpdateOp::Increment, .. })
        ));
        let Item::Stmt(Stmt::VarDecl(second)) = &program.items[2] else {
            panic!("expected second declaration");
        };
        assert!(matches!(
            second.initializer,
            Some(Expr::Update {
                prefix: false,
                op: UpdateOp::Decrement,
                ..
            })
        ));
    }

    #[test]
    fn parses_first_class_function_types() {
        let arena = Bump::new();
        let program = parse_source(&arena, "func(int)->int twice=(int x)=>x*2;").unwrap();
        assert!(matches!(&program.items[0], Item::Stmt(Stmt::VarDecl(_))));
    }

    #[test]
    fn parses_arrays_of_parenthesized_function_types() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "(func(int)->int)[] transforms=[];func(int)->int first=transforms[0];",
        )
        .unwrap();

        let Item::Stmt(Stmt::VarDecl(transforms)) = &program.items[0] else {
            panic!("expected callback array declaration");
        };
        assert!(matches!(
            transforms.ty.kind,
            TypeKind::Array(element) if matches!(element.kind, TypeKind::Function { .. })
        ));
        let Item::Stmt(Stmt::VarDecl(first)) = &program.items[1] else {
            panic!("expected callback declaration");
        };
        assert!(matches!(first.ty.kind, TypeKind::Function { .. }));
    }

    #[test]
    fn parses_trailing_defaulted_parameters() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"string greet(string name, string punctuation="!"){return name+punctuation;}"#,
        )
        .unwrap();

        let Item::Function(function) = &program.items[0] else {
            panic!("expected function");
        };
        assert!(function.params[0].default.is_none());
        assert!(matches!(
            function.params[1].default,
            Some(Expr::String("!", _))
        ));

        let error = parse_source(
            &arena,
            "int invalid(int first=1,int second){return second;}",
        )
        .unwrap_err();
        assert!(error
            .message
            .contains("required parameters cannot follow defaulted parameters"));
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
    fn parses_typed_foreign_module_imports() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"import extern { add as hostAdd, version } from "./host.ts";
extern int hostAdd(int left,int right);
extern string version;"#,
        )
        .unwrap();

        assert!(program.imports.is_empty());
        assert_eq!(program.foreign_imports.len(), 1);
        let import = &program.foreign_imports[0];
        assert_eq!(import.source, "./host.ts");
        assert_eq!(import.specifiers[0].imported.name, "add");
        assert_eq!(import.specifiers[0].local.name, "hostAdd");
        assert_eq!(import.specifiers[1].local.name, "version");
    }

    #[test]
    fn parses_side_effect_foreign_imports() {
        let arena = Bump::new();
        let program = parse_source(&arena, r#"import extern "./setup.ts";"#).unwrap();
        assert_eq!(program.foreign_imports.len(), 1);
        assert!(program.foreign_imports[0].specifiers.is_empty());
        assert_eq!(program.foreign_imports[0].source, "./setup.ts");
    }

    #[test]
    fn parses_extern_classes_methods_and_globals() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern class Document{string title;pure Element? querySelector(string selector);}\
             extern Document document;",
        )
        .unwrap();

        let Item::ExternClass(class) = &program.items[0] else {
            panic!("expected extern class declaration");
        };
        assert_eq!(class.name.name, "Document");
        assert!(matches!(class.members[0], ExternClassMember::Field(_)));
        assert!(matches!(
            &class.members[1],
            ExternClassMember::Method(method)
                if method.name.name == "querySelector" && method.declared_pure
        ));
        assert!(matches!(
            &program.items[1],
            Item::ExternGlobal(global) if global.name.name == "document"
        ));
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
    fn treats_from_as_a_contextual_import_keyword() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"import {from as importedFrom} from "./source";
export int from(int value){return value;}
int result=from(3);"#,
        )
        .unwrap();

        assert_eq!(program.imports[0].specifiers[0].imported.name, "from");
        assert_eq!(program.imports[0].specifiers[0].local.name, "importedFrom");
        let Item::Function(function) = &program.items[0] else {
            panic!("expected exported function");
        };
        assert_eq!(function.name.name, "from");
        let Item::Stmt(Stmt::VarDecl(binding)) = &program.items[1] else {
            panic!("expected variable declaration");
        };
        assert!(matches!(binding.initializer, Some(Expr::Call { .. })));
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

    #[test]
    fn parses_number_as_the_web_numeric_type() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "number total=1;number scale(number value){return value*2;}",
        )
        .unwrap();
        let Item::Stmt(Stmt::VarDecl(total)) = &program.items[0] else {
            panic!("expected number binding");
        };
        assert!(matches!(total.ty.kind, TypeKind::Float));
        let Item::Function(scale) = &program.items[1] else {
            panic!("expected number function");
        };
        assert!(matches!(scale.return_type.kind, TypeKind::Float));
        assert!(matches!(scale.params[0].ty.kind, TypeKind::Float));
    }

    #[test]
    fn parses_nullish_coalescing_and_assignment() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string? optional=null;string value=optional??\"fallback\";optional??=\"stored\";",
        )
        .unwrap();

        let Item::Stmt(Stmt::VarDecl(value)) = &program.items[1] else {
            panic!("expected coalesced variable declaration");
        };
        assert!(matches!(
            value.initializer,
            Some(Expr::Binary {
                op: BinaryOp::Nullish,
                ..
            })
        ));
        let Item::Stmt(Stmt::Expr(Expr::Assignment { op, .. })) = &program.items[2] else {
            panic!("expected nullish assignment expression");
        };
        assert_eq!(*op, AssignmentOp::Nullish);
    }

    #[test]
    fn parses_optional_member_and_index_access() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[]? values=null;int? length=values?.length;int? first=values?.[0];",
        )
        .unwrap();
        let Item::Stmt(Stmt::VarDecl(length)) = &program.items[1] else {
            panic!("expected optional member binding");
        };
        assert!(matches!(
            length.initializer,
            Some(Expr::OptionalMember { .. })
        ));
        let Item::Stmt(Stmt::VarDecl(first)) = &program.items[2] else {
            panic!("expected optional index binding");
        };
        assert!(matches!(
            first.initializer,
            Some(Expr::OptionalIndex { .. })
        ));
    }

    #[test]
    fn parses_union_types_with_postfix_precedence() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "string|int value=1;(string|int)[] values=[value];").unwrap();

        let Item::Stmt(Stmt::VarDecl(value)) = &program.items[0] else {
            panic!("expected union binding");
        };
        assert!(matches!(value.ty.kind, TypeKind::Union(members) if members.len() == 2));
        let Item::Stmt(Stmt::VarDecl(values)) = &program.items[1] else {
            panic!("expected union array binding");
        };
        assert!(matches!(
            values.ty.kind,
            TypeKind::Array(element)
                if matches!(element.kind, TypeKind::Union(members) if members.len() == 2)
        ));
    }

    #[test]
    fn parses_union_type_guards() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "bool isText(string|int value){return value is string;}",
        )
        .unwrap();

        let Item::Function(function) = &program.items[0] else {
            panic!("expected function");
        };
        let Stmt::Return {
            value: Some(Expr::TypeCheck { target, .. }),
            ..
        } = &function.body[0]
        else {
            panic!("expected type guard return");
        };
        assert!(matches!(target.kind, TypeKind::String));
    }

    #[test]
    fn parses_only_statically_resolvable_dynamic_imports() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "auto task=import(\"./feature\");task.then((auto module)=>module.run());",
        )
        .unwrap();
        let Item::Stmt(Stmt::VarDecl(binding)) = &program.items[0] else {
            panic!("expected dynamic import binding");
        };
        assert!(matches!(
            binding.initializer,
            Some(Expr::DynamicImport {
                source: "./feature",
                ..
            })
        ));
        assert!(
            parse_source(&arena, "string path=\"./feature\";auto task=import(path);")
                .unwrap_err()
                .message
                .contains("static string")
        );
    }

    #[test]
    fn parses_async_exceptions_and_optional_catch_bindings() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "async int run(){try{throw null;}catch(auto error){return await Task.resolve(1);}finally{print(\"done\");}}try{print(1);}catch{}",
        )
        .unwrap();
        let Item::Function(function) = &program.items[0] else {
            panic!("expected async function");
        };
        assert!(function.is_async);
        let Stmt::Try {
            catch: Some(clause),
            finally: Some(_),
            ..
        } = &function.body[0]
        else {
            panic!("expected try/catch/finally");
        };
        assert_eq!(clause.binding.expect("catch binding").name.name, "error");
        assert!(matches!(clause.body[0], Stmt::Return { .. }));
        let Item::Stmt(Stmt::Try {
            catch: Some(clause),
            ..
        }) = &program.items[1]
        else {
            panic!("expected binding-free catch");
        };
        assert!(clause.binding.is_none());
        assert!(parse_source(&arena, "try{print(1);}")
            .unwrap_err()
            .message
            .contains("requires a `catch` or `finally`"));
    }

    #[test]
    fn parses_class_inheritance_and_super_constructor_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Base<T>{T value;init(T value){this.value=value;}}class Child extends Base<int>{init(int value){super(value);}}",
        )
        .unwrap();
        let Item::Class(child) = &program.items[1] else {
            panic!("expected derived class");
        };
        assert!(matches!(
            child.base.expect("base type").kind,
            TypeKind::Named { name: "Base", args } if args.len() == 1
        ));
        let ClassMember::Constructor(constructor) = &child.members[0] else {
            panic!("expected constructor");
        };
        assert!(matches!(constructor.body[0], Stmt::SuperCall { .. }));
    }

    #[test]
    fn parses_generators_yield_and_delegated_yield() {
        let arena = Bump::new();
        let program =
            parse_source(&arena, "generator int values(){yield 1;yield* [2,3];}").unwrap();
        let Item::Function(function) = &program.items[0] else {
            panic!("expected generator function");
        };
        assert!(function.is_generator);
        assert!(matches!(
            function.body[0],
            Stmt::Yield {
                delegate: false,
                ..
            }
        ));
        assert!(matches!(
            function.body[1],
            Stmt::Yield { delegate: true, .. }
        ));
    }

    #[test]
    fn accepts_reserved_identifier_names_for_host_properties() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Ast{bool generator;bool async;}auto a=value.default;auto b=value?.catch;auto c=value.extends;auto d=value.super;auto e=value.import;auto f=value.true;auto g=value.int;Record<int> keys=record{generator:1,async:2};",
        )
        .unwrap();
        let properties = program
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Stmt(Stmt::VarDecl(VarDecl {
                    initializer: Some(Expr::Member { property, .. }),
                    ..
                }))
                | Item::Stmt(Stmt::VarDecl(VarDecl {
                    initializer: Some(Expr::OptionalMember { property, .. }),
                    ..
                })) => Some(property.name),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            properties,
            ["default", "catch", "extends", "super", "import", "true", "int"]
        );
        let Item::Class(class) = &program.items[0] else {
            panic!("expected class declaration");
        };
        assert!(
            matches!(&class.members[0], ClassMember::Field(field) if field.name.name == "generator")
        );
        assert!(
            matches!(&class.members[1], ClassMember::Field(field) if field.name.name == "async")
        );
        let Item::Stmt(Stmt::VarDecl(VarDecl {
            initializer: Some(Expr::RecordLiteral { entries, .. }),
            ..
        })) = &program.items[8]
        else {
            panic!("expected record declaration");
        };
        assert!(
            matches!(&entries[0], RecordElement::Entry(entry) if entry.key.name == "generator")
        );
        assert!(matches!(&entries[1], RecordElement::Entry(entry) if entry.key.name == "async"));
    }
}
