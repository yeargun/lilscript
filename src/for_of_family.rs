use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::ast::{
    ArrayBinding, ArrayElement, ArrowBody, BinaryOp, CatchBinding, CatchClause, Expr,
    ForInitializer, FunctionDecl, Ident, Item, MatchArm, Param, Program, RecordBinding,
    RecordElement, RecordEntry, Stmt, TemplatePart, TypeKind, TypeRef, VarDecl,
};
use crate::span::Span;

pub fn expand_for_of_families<'arena, 'src>(
    arena: &'arena Bump,
    program: Program<'arena, 'src>,
    max_n: usize,
) -> Program<'arena, 'src> {
    if max_n == 0 {
        return program;
    }
    let mut extras = BumpVec::new_in(arena);
    let mut family_index = 0usize;
    for item in program.items {
        let Item::Function(func) = item else {
            continue;
        };
        let Some(site) = eligible_for_of(func) else {
            continue;
        };
        let salt = family_index.saturating_mul(64);
        family_index += 1;
        for n in 1..=max_n {
            extras.push(Item::Function(unrolled_clone(arena, func, site, n, salt)));
        }
        extras.push(Item::Function(picker_function(arena, func, max_n, salt)));
    }
    if extras.is_empty() {
        return program;
    }
    let mut items = BumpVec::new_in(arena);
    items.extend(program.items.iter().cloned());
    items.extend(extras);
    Program {
        items: items.into_bump_slice(),
        ..program
    }
}

fn intern_name(name: String) -> &'static str {
    Box::leak(name.into_boxed_str())
}

fn generated_ident(name: String, salt: usize) -> Ident<'static> {
    let name = intern_name(name);
    Ident {
        name,
        span: Span::new(0x7000_0000 + salt, 0x7000_0000 + salt + name.len()),
    }
}

#[derive(Clone, Copy)]
struct ForOfSite<'arena, 'src> {
    element_type: TypeRef<'arena, 'src>,
    element: Ident<'src>,
    body: &'arena Stmt<'arena, 'src>,
}

fn eligible_for_of<'arena, 'src>(
    func: &FunctionDecl<'arena, 'src>,
) -> Option<ForOfSite<'arena, 'src>> {
    if func.params.len() < 2 {
        return None;
    }
    let first = func.params.first()?;
    if !matches!(first.ty.kind, TypeKind::Array(_)) {
        return None;
    }
    find_for_of_over(func.body, first.name.name)
}

fn find_for_of_over<'arena, 'src>(
    stmts: &'arena [Stmt<'arena, 'src>],
    name: &'src str,
) -> Option<ForOfSite<'arena, 'src>> {
    for stmt in stmts {
        match stmt {
            Stmt::ForOf {
                element_type,
                element,
                iterable: Expr::Ident(ident),
                body,
                inline: false,
                ..
            } if ident.name == name => {
                return Some(ForOfSite {
                    element_type: *element_type,
                    element: *element,
                    body,
                });
            }
            Stmt::Block { body, .. } => {
                if let Some(site) = find_for_of_over(body, name) {
                    return Some(site);
                }
            }
            _ => {}
        }
    }
    None
}

fn unrolled_clone<'arena, 'src>(
    arena: &'arena Bump,
    func: &FunctionDecl<'arena, 'src>,
    site: ForOfSite<'arena, 'src>,
    n: usize,
    salt: usize,
) -> FunctionDecl<'arena, 'src> {
    let delta = 0x0100_0000 + salt.saturating_mul(4096) + n.saturating_mul(256);
    let shifted = shift_function(arena, func, delta);
    let first = &shifted.params[0];
    let span = shifted.span;
    let index = generated_ident(format!("$i{n}"), salt + 32 + n);
    let element = ident_with_span(site.element.name, shift_span(site.element.span, delta));
    let mut elements = BumpVec::new_in(arena);
    for value in 0..n {
        elements.push(ArrayElement::Value(Expr::Int(value as i64, span)));
    }
    let assign = Stmt::VarDecl(VarDecl {
        ty: shift_type(site.element_type, delta),
        name: element,
        initializer: Some(Expr::Index {
            object: arena.alloc(Expr::Ident(first.name)),
            index: arena.alloc(Expr::Ident(index)),
            span,
        }),
        span,
    });
    let body_stmts = arena.alloc_slice_clone(&[assign, shift_stmt(arena, site.body, delta)]);
    let inline = Stmt::ForOf {
        element_type: TypeRef {
            kind: TypeKind::Int,
            span,
        },
        element: index,
        iterable: Expr::ArrayLiteral {
            elements: elements.into_bump_slice(),
            span,
        },
        body: arena.alloc(Stmt::Block {
            body: body_stmts,
            span,
        }),
        inline: true,
        span,
    };
    FunctionDecl {
        name: generated_ident(format!("{}${}", func.name.name, n), salt + n),
        body: replace_for_of(arena, shifted.body, first.name.name, inline),
        params: shifted.params,
        span,
        ..shifted
    }
}

fn replace_for_of<'arena, 'src>(
    arena: &'arena Bump,
    stmts: &'arena [Stmt<'arena, 'src>],
    name: &'src str,
    replacement: Stmt<'arena, 'src>,
) -> &'arena [Stmt<'arena, 'src>] {
    let mut out = BumpVec::new_in(arena);
    let mut replaced = false;
    for stmt in stmts {
        if !replaced {
            match stmt {
                Stmt::ForOf {
                    iterable: Expr::Ident(ident),
                    inline: false,
                    ..
                } if ident.name == name => {
                    out.push(replacement.clone());
                    replaced = true;
                    continue;
                }
                Stmt::Block { body, span } if find_for_of_over(body, name).is_some() => {
                    out.push(Stmt::Block {
                        body: replace_for_of(arena, body, name, replacement.clone()),
                        span: *span,
                    });
                    replaced = true;
                    continue;
                }
                _ => {}
            }
        }
        out.push(stmt.clone());
    }
    out.into_bump_slice()
}

fn picker_function<'arena, 'src>(
    arena: &'arena Bump,
    func: &FunctionDecl<'arena, 'src>,
    max_n: usize,
    salt: usize,
) -> FunctionDecl<'arena, 'src> {
    let delta = 0x0180_0000 + salt.saturating_mul(4096);
    let keys = generated_ident(func.params[0].name.name.to_string(), salt + 40);
    let rest: Vec<Param<'arena, 'src>> = func.params[1..]
        .iter()
        .enumerate()
        .map(|(index, param)| Param {
            ty: shift_type(param.ty, delta),
            name: generated_ident(param.name.name.to_string(), salt + 50 + index),
            default: param
                .default
                .as_ref()
                .map(|value| shift_expr(arena, value, delta)),
            span: shift_span(param.span, delta),
        })
        .collect();
    let rest = arena.alloc_slice_clone(&rest);
    let span = shift_span(func.span, delta);
    let n_ident = generated_ident("$n".to_string(), salt + 16);
    let decl_n = Stmt::VarDecl(VarDecl {
        ty: TypeRef {
            kind: TypeKind::Int,
            span,
        },
        name: n_ident,
        initializer: Some(Expr::Member {
            object: arena.alloc(Expr::Ident(keys)),
            property: Ident {
                name: "length",
                span,
            },
            span,
        }),
        span,
    });
    let mut branch = Stmt::Return {
        value: Some(wrapper_arrow(arena, keys, func.name, rest, span)),
        span,
    };
    for n in (1..=max_n).rev() {
        let clone = generated_ident(format!("{}${}", func.name.name, n), salt + n);
        branch = Stmt::If {
            condition: Expr::Binary {
                op: BinaryOp::Eq,
                lhs: arena.alloc(Expr::Ident(n_ident)),
                rhs: arena.alloc(Expr::Int(n as i64, span)),
                span,
            },
            then_branch: arena.alloc(Stmt::Return {
                value: Some(wrapper_arrow(arena, keys, clone, rest, span)),
                span,
            }),
            else_branch: Some(arena.alloc(branch)),
            span,
        };
    }
    FunctionDecl {
        name: generated_ident(format!("{}$pick", func.name.name), salt),
        params: arena.alloc_slice_clone(&[Param {
            ty: shift_type(func.params[0].ty, delta),
            name: keys,
            default: None,
            span,
        }]),
        body: arena.alloc_slice_clone(&[decl_n, branch]),
        return_type: js_type(span),
        ..func.clone()
    }
}

fn wrapper_arrow<'arena, 'src>(
    arena: &'arena Bump,
    keys: Ident<'src>,
    callee: Ident<'src>,
    rest: &[Param<'arena, 'src>],
    span: Span,
) -> Expr<'arena, 'src> {
    let mut args = BumpVec::new_in(arena);
    args.push(Expr::Ident(keys));
    for param in rest {
        args.push(Expr::Ident(param.name));
    }
    Expr::ArrowFunction {
        params: arena.alloc_slice_clone(rest),
        body: ArrowBody::Expr(arena.alloc(Expr::Call {
            callee: arena.alloc(Expr::Ident(callee)),
            args: args.into_bump_slice(),
            span,
        })),
        span,
    }
}

fn js_type(span: Span) -> TypeRef<'static, 'static> {
    TypeRef {
        kind: TypeKind::Named {
            name: "JsValue",
            args: &[],
        },
        span,
    }
}

fn shift_span(span: Span, delta: usize) -> Span {
    Span::new(
        span.start.saturating_add(delta),
        span.end.saturating_add(delta),
    )
}

fn ident_with_span(name: &str, span: Span) -> Ident<'_> {
    Ident { name, span }
}

fn shift_ident(ident: Ident<'_>, delta: usize) -> Ident<'_> {
    Ident {
        name: ident.name,
        span: shift_span(ident.span, delta),
    }
}

fn shift_type<'arena, 'src>(ty: TypeRef<'arena, 'src>, delta: usize) -> TypeRef<'arena, 'src> {
    TypeRef {
        kind: ty.kind,
        span: shift_span(ty.span, delta),
    }
}

fn shift_function<'arena, 'src>(
    arena: &'arena Bump,
    func: &FunctionDecl<'arena, 'src>,
    delta: usize,
) -> FunctionDecl<'arena, 'src> {
    let params: Vec<Param<'arena, 'src>> = func
        .params
        .iter()
        .map(|param| Param {
            ty: shift_type(param.ty, delta),
            name: shift_ident(param.name, delta),
            default: param
                .default
                .as_ref()
                .map(|value| shift_expr(arena, value, delta)),
            span: shift_span(param.span, delta),
        })
        .collect();
    FunctionDecl {
        name: shift_ident(func.name, delta),
        params: arena.alloc_slice_clone(&params),
        body: shift_stmts(arena, func.body, delta),
        return_type: shift_type(func.return_type, delta),
        span: shift_span(func.span, delta),
        ..func.clone()
    }
}

fn shift_stmts<'arena, 'src>(
    arena: &'arena Bump,
    stmts: &'arena [Stmt<'arena, 'src>],
    delta: usize,
) -> &'arena [Stmt<'arena, 'src>] {
    let mut out = BumpVec::new_in(arena);
    for stmt in stmts {
        out.push(shift_stmt(arena, stmt, delta));
    }
    out.into_bump_slice()
}

fn shift_stmt<'arena, 'src>(
    arena: &'arena Bump,
    stmt: &Stmt<'arena, 'src>,
    delta: usize,
) -> Stmt<'arena, 'src> {
    match stmt {
        Stmt::VarDecl(decl) => Stmt::VarDecl(VarDecl {
            ty: shift_type(decl.ty, delta),
            name: shift_ident(decl.name, delta),
            initializer: decl
                .initializer
                .as_ref()
                .map(|value| shift_expr(arena, value, delta)),
            span: shift_span(decl.span, delta),
        }),
        Stmt::ArrayDestructure {
            bindings,
            value,
            span,
        } => {
            let mut cloned = BumpVec::new_in(arena);
            for binding in *bindings {
                cloned.push(match *binding {
                    ArrayBinding::Hole(span) => ArrayBinding::Hole(shift_span(span, delta)),
                    ArrayBinding::Name(name) => ArrayBinding::Name(shift_ident(name, delta)),
                    ArrayBinding::Rest(name) => ArrayBinding::Rest(shift_ident(name, delta)),
                });
            }
            Stmt::ArrayDestructure {
                bindings: cloned.into_bump_slice(),
                value: shift_expr(arena, value, delta),
                span: shift_span(*span, delta),
            }
        }
        Stmt::RecordDestructure {
            bindings,
            rest,
            value,
            span,
        } => {
            let mut cloned = BumpVec::new_in(arena);
            for binding in *bindings {
                cloned.push(RecordBinding {
                    key: shift_ident(binding.key, delta),
                    name: shift_ident(binding.name, delta),
                    span: shift_span(binding.span, delta),
                });
            }
            Stmt::RecordDestructure {
                bindings: cloned.into_bump_slice(),
                rest: rest.map(|name| shift_ident(name, delta)),
                value: shift_expr(arena, value, delta),
                span: shift_span(*span, delta),
            }
        }
        Stmt::Expr(value) => Stmt::Expr(shift_expr(arena, value, delta)),
        Stmt::Return { value, span } => Stmt::Return {
            value: value.as_ref().map(|value| shift_expr(arena, value, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::Throw { value, span } => Stmt::Throw {
            value: shift_expr(arena, value, delta),
            span: shift_span(*span, delta),
        },
        Stmt::SuperCall { args, span } => Stmt::SuperCall {
            args: shift_exprs(arena, args, delta),
            span: shift_span(*span, delta),
        },
        Stmt::Yield {
            value,
            delegate,
            span,
        } => Stmt::Yield {
            value: shift_expr(arena, value, delta),
            delegate: *delegate,
            span: shift_span(*span, delta),
        },
        Stmt::Try {
            body,
            catch,
            finally,
            span,
        } => Stmt::Try {
            body: shift_stmts(arena, body, delta),
            catch: catch.as_ref().map(|clause| CatchClause {
                binding: clause.binding.map(|binding| CatchBinding {
                    ty: shift_type(binding.ty, delta),
                    name: shift_ident(binding.name, delta),
                    span: shift_span(binding.span, delta),
                }),
                body: shift_stmts(arena, clause.body, delta),
                span: shift_span(clause.span, delta),
            }),
            finally: finally.map(|body| shift_stmts(arena, body, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::Block { body, span } => Stmt::Block {
            body: shift_stmts(arena, body, delta),
            span: shift_span(*span, delta),
        },
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Stmt::If {
            condition: shift_expr(arena, condition, delta),
            then_branch: arena.alloc(shift_stmt(arena, then_branch, delta)),
            else_branch: else_branch.map(|branch| &*arena.alloc(shift_stmt(arena, branch, delta))),
            span: shift_span(*span, delta),
        },
        Stmt::While {
            condition,
            body,
            span,
        } => Stmt::While {
            condition: shift_expr(arena, condition, delta),
            body: arena.alloc(shift_stmt(arena, body, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::For {
            initializer,
            condition,
            update,
            body,
            span,
        } => Stmt::For {
            initializer: initializer.as_ref().map(|init| match init {
                ForInitializer::VarDecl(decl) => ForInitializer::VarDecl(VarDecl {
                    ty: shift_type(decl.ty, delta),
                    name: shift_ident(decl.name, delta),
                    initializer: decl
                        .initializer
                        .as_ref()
                        .map(|value| shift_expr(arena, value, delta)),
                    span: shift_span(decl.span, delta),
                }),
                ForInitializer::Expr(value) => {
                    ForInitializer::Expr(shift_expr(arena, value, delta))
                }
            }),
            condition: condition
                .as_ref()
                .map(|value| shift_expr(arena, value, delta)),
            update: update.as_ref().map(|value| shift_expr(arena, value, delta)),
            body: arena.alloc(shift_stmt(arena, body, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::ForIn {
            key_type,
            key,
            object,
            body,
            span,
        } => Stmt::ForIn {
            key_type: shift_type(*key_type, delta),
            key: shift_ident(*key, delta),
            object: shift_expr(arena, object, delta),
            body: arena.alloc(shift_stmt(arena, body, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::ForOf {
            element_type,
            element,
            iterable,
            body,
            inline,
            span,
        } => Stmt::ForOf {
            element_type: shift_type(*element_type, delta),
            element: shift_ident(*element, delta),
            iterable: shift_expr(arena, iterable, delta),
            body: arena.alloc(shift_stmt(arena, body, delta)),
            inline: *inline,
            span: shift_span(*span, delta),
        },
        Stmt::Break(span) => Stmt::Break(shift_span(*span, delta)),
        Stmt::Continue(span) => Stmt::Continue(shift_span(*span, delta)),
    }
}

fn shift_exprs<'arena, 'src>(
    arena: &'arena Bump,
    exprs: &'arena [Expr<'arena, 'src>],
    delta: usize,
) -> &'arena [Expr<'arena, 'src>] {
    let mut out = BumpVec::new_in(arena);
    for expr in exprs {
        out.push(shift_expr(arena, expr, delta));
    }
    out.into_bump_slice()
}

fn shift_expr<'arena, 'src>(
    arena: &'arena Bump,
    expr: &Expr<'arena, 'src>,
    delta: usize,
) -> Expr<'arena, 'src> {
    match expr {
        Expr::Int(value, span) => Expr::Int(*value, shift_span(*span, delta)),
        Expr::Float(value, span) => Expr::Float(*value, shift_span(*span, delta)),
        Expr::String(value, span) => Expr::String(value, shift_span(*span, delta)),
        Expr::Bool(value, span) => Expr::Bool(*value, shift_span(*span, delta)),
        Expr::Null(span) => Expr::Null(shift_span(*span, delta)),
        Expr::Ident(ident) => Expr::Ident(shift_ident(*ident, delta)),
        Expr::ArrayLiteral { elements, span } => {
            let mut out = BumpVec::new_in(arena);
            for element in *elements {
                out.push(match element {
                    ArrayElement::Value(value) => {
                        ArrayElement::Value(shift_expr(arena, value, delta))
                    }
                    ArrayElement::Spread { value, span } => ArrayElement::Spread {
                        value: shift_expr(arena, value, delta),
                        span: shift_span(*span, delta),
                    },
                });
            }
            Expr::ArrayLiteral {
                elements: out.into_bump_slice(),
                span: shift_span(*span, delta),
            }
        }
        Expr::RecordLiteral { entries, span } => {
            let mut out = BumpVec::new_in(arena);
            for entry in *entries {
                out.push(match entry {
                    RecordElement::Entry(entry) => RecordElement::Entry(RecordEntry {
                        key: shift_ident(entry.key, delta),
                        value: shift_expr(arena, &entry.value, delta),
                        span: shift_span(entry.span, delta),
                    }),
                    RecordElement::Spread { value, span } => RecordElement::Spread {
                        value: shift_expr(arena, value, delta),
                        span: shift_span(*span, delta),
                    },
                });
            }
            Expr::RecordLiteral {
                entries: out.into_bump_slice(),
                span: shift_span(*span, delta),
            }
        }
        Expr::ObjectLiteral { entries, span } => {
            let mut out = BumpVec::new_in(arena);
            for entry in *entries {
                out.push(match entry {
                    RecordElement::Entry(entry) => RecordElement::Entry(RecordEntry {
                        key: shift_ident(entry.key, delta),
                        value: shift_expr(arena, &entry.value, delta),
                        span: shift_span(entry.span, delta),
                    }),
                    RecordElement::Spread { value, span } => RecordElement::Spread {
                        value: shift_expr(arena, value, delta),
                        span: shift_span(*span, delta),
                    },
                });
            }
            Expr::ObjectLiteral {
                entries: out.into_bump_slice(),
                span: shift_span(*span, delta),
            }
        }
        Expr::StructLiteral { name, values, span } => Expr::StructLiteral {
            name: shift_ident(*name, delta),
            values: shift_exprs(arena, values, delta),
            span: shift_span(*span, delta),
        },
        Expr::New {
            class,
            type_args,
            args,
            span,
        } => Expr::New {
            class: shift_ident(*class, delta),
            type_args,
            args: shift_exprs(arena, args, delta),
            span: shift_span(*span, delta),
        },
        Expr::DynamicImport { source, span } => Expr::DynamicImport {
            source,
            span: shift_span(*span, delta),
        },
        Expr::Member {
            object,
            property,
            span,
        } => Expr::Member {
            object: arena.alloc(shift_expr(arena, object, delta)),
            property: shift_ident(*property, delta),
            span: shift_span(*span, delta),
        },
        Expr::OptionalMember {
            object,
            property,
            span,
        } => Expr::OptionalMember {
            object: arena.alloc(shift_expr(arena, object, delta)),
            property: shift_ident(*property, delta),
            span: shift_span(*span, delta),
        },
        Expr::Call { callee, args, span } => Expr::Call {
            callee: arena.alloc(shift_expr(arena, callee, delta)),
            args: shift_exprs(arena, args, delta),
            span: shift_span(*span, delta),
        },
        Expr::ArrowFunction { params, body, span } => {
            let shifted: Vec<Param<'arena, 'src>> = params
                .iter()
                .map(|param| Param {
                    ty: shift_type(param.ty, delta),
                    name: shift_ident(param.name, delta),
                    default: param
                        .default
                        .as_ref()
                        .map(|value| shift_expr(arena, value, delta)),
                    span: shift_span(param.span, delta),
                })
                .collect();
            Expr::ArrowFunction {
                params: arena.alloc_slice_clone(&shifted),
                body: match body {
                    ArrowBody::Expr(value) => {
                        ArrowBody::Expr(arena.alloc(shift_expr(arena, value, delta)))
                    }
                    ArrowBody::Block(stmts) => ArrowBody::Block(shift_stmts(arena, stmts, delta)),
                },
                span: shift_span(*span, delta),
            }
        }
        Expr::Unary { op, expr, span } => Expr::Unary {
            op: *op,
            expr: arena.alloc(shift_expr(arena, expr, delta)),
            span: shift_span(*span, delta),
        },
        Expr::Await { task, span } => Expr::Await {
            task: arena.alloc(shift_expr(arena, task, delta)),
            span: shift_span(*span, delta),
        },
        Expr::Binary { op, lhs, rhs, span } => Expr::Binary {
            op: *op,
            lhs: arena.alloc(shift_expr(arena, lhs, delta)),
            rhs: arena.alloc(shift_expr(arena, rhs, delta)),
            span: shift_span(*span, delta),
        },
        Expr::TypeCheck {
            value,
            target,
            span,
        } => Expr::TypeCheck {
            value: arena.alloc(shift_expr(arena, value, delta)),
            target: shift_type(*target, delta),
            span: shift_span(*span, delta),
        },
        Expr::Index {
            object,
            index,
            span,
        } => Expr::Index {
            object: arena.alloc(shift_expr(arena, object, delta)),
            index: arena.alloc(shift_expr(arena, index, delta)),
            span: shift_span(*span, delta),
        },
        Expr::OptionalIndex {
            object,
            index,
            span,
        } => Expr::OptionalIndex {
            object: arena.alloc(shift_expr(arena, object, delta)),
            index: arena.alloc(shift_expr(arena, index, delta)),
            span: shift_span(*span, delta),
        },
        Expr::If {
            condition,
            then_value,
            else_value,
            span,
        } => Expr::If {
            condition: arena.alloc(shift_expr(arena, condition, delta)),
            then_value: arena.alloc(shift_expr(arena, then_value, delta)),
            else_value: arena.alloc(shift_expr(arena, else_value, delta)),
            span: shift_span(*span, delta),
        },
        Expr::Match { value, arms, span } => {
            let mut out = BumpVec::new_in(arena);
            for arm in *arms {
                let pattern = match arm.pattern {
                    crate::ast::MatchPattern::EnumVariant {
                        enum_name,
                        variant,
                        span,
                    } => crate::ast::MatchPattern::EnumVariant {
                        enum_name: shift_ident(enum_name, delta),
                        variant: shift_ident(variant, delta),
                        span: shift_span(span, delta),
                    },
                    crate::ast::MatchPattern::Int(value, span) => {
                        crate::ast::MatchPattern::Int(value, shift_span(span, delta))
                    }
                    crate::ast::MatchPattern::String(value, span) => {
                        crate::ast::MatchPattern::String(value, shift_span(span, delta))
                    }
                    crate::ast::MatchPattern::Bool(value, span) => {
                        crate::ast::MatchPattern::Bool(value, shift_span(span, delta))
                    }
                    crate::ast::MatchPattern::Wildcard(span) => {
                        crate::ast::MatchPattern::Wildcard(shift_span(span, delta))
                    }
                };
                out.push(MatchArm {
                    pattern,
                    value: shift_expr(arena, &arm.value, delta),
                    span: shift_span(arm.span, delta),
                });
            }
            Expr::Match {
                value: arena.alloc(shift_expr(arena, value, delta)),
                arms: out.into_bump_slice(),
                span: shift_span(*span, delta),
            }
        }
        Expr::Assignment {
            op,
            target,
            value,
            span,
        } => Expr::Assignment {
            op: *op,
            target: arena.alloc(shift_expr(arena, target, delta)),
            value: arena.alloc(shift_expr(arena, value, delta)),
            span: shift_span(*span, delta),
        },
        Expr::Update {
            op,
            target,
            prefix,
            span,
        } => Expr::Update {
            op: *op,
            target: arena.alloc(shift_expr(arena, target, delta)),
            prefix: *prefix,
            span: shift_span(*span, delta),
        },
        Expr::Template { parts, span } => {
            let mut out = BumpVec::new_in(arena);
            for part in *parts {
                out.push(match part {
                    TemplatePart::String(text, span) => {
                        TemplatePart::String(text, shift_span(*span, delta))
                    }
                    TemplatePart::Expr(value) => {
                        TemplatePart::Expr(shift_expr(arena, value, delta))
                    }
                });
            }
            Expr::Template {
                parts: out.into_bump_slice(),
                span: shift_span(*span, delta),
            }
        }
    }
}
