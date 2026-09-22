use crate::ast::ExprKind;
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::ast::{
    Argument, ArrayBinding, ArrayElement, ArrowBody, BinaryOp, CatchBinding, CatchClause, Expr,
    ForInitializer, FunctionDecl, Ident, Item, MatchArm, Param, ParameterType, Program,
    RecordBinding, RecordElement, RecordEntry, Stmt, TemplatePart, TypeKind, TypeRef, VarDecl,
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
    let source_nodes = crate::ast::SourceNodes::continuing(program.source_identity());
    let nodes = &source_nodes;
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
            extras.push(Item::Function(unrolled_clone(
                nodes, arena, func, site, n, salt,
            )));
        }
        extras.push(Item::Function(picker_function(
            nodes, arena, func, max_n, salt,
        )));
    }
    if extras.is_empty() {
        return program;
    }
    let mut items = BumpVec::new_in(arena);
    items.extend(program.items.iter().cloned());
    items.extend(extras);
    program.with_items(nodes, items.into_bump_slice())
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
    if func.params.len() < 2
        || func
            .params
            .iter()
            .any(|p| p.parameter.passing != crate::primitive::ParameterPassing::Value)
    {
        return None;
    }
    let first = func.params.first()?;
    if !matches!(first.parameter.ty.kind, TypeKind::Array(_)) {
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
                iterable:
                    Expr {
                        kind: ExprKind::Ident(ident),
                        ..
                    },
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
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    func: &FunctionDecl<'arena, 'src>,
    site: ForOfSite<'arena, 'src>,
    n: usize,
    salt: usize,
) -> FunctionDecl<'arena, 'src> {
    let delta = 0x0100_0000 + salt.saturating_mul(4096) + n.saturating_mul(256);
    let shifted = shift_function(nodes, arena, func, delta);
    let first = &shifted.params[0];
    let span = shifted.span;
    let index = generated_ident(format!("$i{n}"), salt + 32 + n);
    let element = ident_with_span(site.element.name, shift_span(site.element.span, delta));
    let mut elements = BumpVec::new_in(arena);
    for value in 0..n {
        elements.push(ArrayElement::Value(
            nodes.expression(ExprKind::Int(value as i64, span)),
        ));
    }
    let assign = Stmt::VarDecl(VarDecl {
        ty: shift_type(site.element_type, delta),
        name: element,
        initializer: Some(nodes.expression(ExprKind::Index {
            object: arena.alloc(nodes.expression(ExprKind::Ident(first.name))),
            index: arena.alloc(nodes.expression(ExprKind::Ident(index))),
            span,
        })),
        span,
    });
    let body_stmts = arena.alloc_slice_clone(&[assign, shift_stmt(nodes, arena, site.body, delta)]);
    let inline = Stmt::ForOf {
        element_type: TypeRef {
            kind: TypeKind::Int,
            span,
        },
        element: index,
        iterable: nodes.expression(ExprKind::ArrayLiteral {
            elements: elements.into_bump_slice(),
            span,
        }),
        body: arena.alloc(Stmt::Block {
            body: body_stmts,
            span,
        }),
        inline: true,
        span,
    };
    FunctionDecl {
        name: generated_ident(format!("{}${}", func.name.name, n), salt + n),
        body: replace_for_of(nodes, arena, shifted.body, first.name.name, inline),
        params: shifted.params,
        span,
        ..shifted
    }
}

fn replace_for_of<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
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
                    iterable:
                        Expr {
                            kind: ExprKind::Ident(ident),
                            ..
                        },
                    inline: false,
                    ..
                } if ident.name == name => {
                    out.push(shift_stmt(nodes, arena, &replacement, 0));
                    replaced = true;
                    continue;
                }
                Stmt::Block { body, span } if find_for_of_over(body, name).is_some() => {
                    out.push(Stmt::Block {
                        body: replace_for_of(nodes, arena, body, name, replacement.clone()),
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
    nodes: &crate::ast::SourceNodes,
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
            parameter: shift_parameter_type(param.parameter, delta),
            name: generated_ident(param.name.name.to_string(), salt + 50 + index),
            default: param
                .default
                .as_ref()
                .map(|value| shift_expr(nodes, arena, value, delta)),
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
        initializer: Some(nodes.expression(ExprKind::Member {
            object: arena.alloc(nodes.expression(ExprKind::Ident(keys))),
            property: Ident {
                name: "length",
                span,
            },
            span,
        })),
        span,
    });
    let mut branch = Stmt::Return {
        value: Some(wrapper_arrow(nodes, arena, keys, func.name, rest, span)),
        span,
    };
    for n in (1..=max_n).rev() {
        let clone = generated_ident(format!("{}${}", func.name.name, n), salt + n);
        branch = Stmt::If {
            condition: nodes.expression(ExprKind::Binary {
                op: BinaryOp::Eq,
                lhs: arena.alloc(nodes.expression(ExprKind::Ident(n_ident))),
                rhs: arena.alloc(nodes.expression(ExprKind::Int(n as i64, span))),
                span,
            }),
            then_branch: arena.alloc(Stmt::Return {
                value: Some(wrapper_arrow(nodes, arena, keys, clone, rest, span)),
                span,
            }),
            else_branch: Some(arena.alloc(branch)),
            span,
        };
    }
    FunctionDecl {
        name: generated_ident(format!("{}$pick", func.name.name), salt),
        params: arena.alloc_slice_clone(&[Param {
            parameter: shift_parameter_type(func.params[0].parameter, delta),
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
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    keys: Ident<'src>,
    callee: Ident<'src>,
    rest: &[Param<'arena, 'src>],
    span: Span,
) -> Expr<'arena, 'src> {
    let mut args = BumpVec::new_in(arena);
    args.push(Argument {
        expression: nodes.expression(ExprKind::Ident(keys)),
        passing: crate::primitive::ParameterPassing::Value,
        span: keys.span,
    });
    for param in rest {
        args.push(Argument {
            expression: nodes.expression(ExprKind::Ident(param.name)),
            passing: param.parameter.passing,
            span: param.name.span,
        });
    }
    nodes.expression(ExprKind::ArrowFunction {
        params: shift_params(nodes, arena, rest, 0),
        body: ArrowBody::Expr(arena.alloc(nodes.expression(ExprKind::Call {
            callee: arena.alloc(nodes.expression(ExprKind::Ident(callee))),
            args: args.into_bump_slice(),
            span,
        }))),
        span,
    })
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

fn shift_parameter_type<'arena, 'src>(
    parameter: ParameterType<'arena, 'src>,
    delta: usize,
) -> ParameterType<'arena, 'src> {
    ParameterType {
        ty: shift_type(parameter.ty, delta),
        passing: parameter.passing,
        span: shift_span(parameter.span, delta),
    }
}

fn shift_arguments<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    arguments: &[Argument<'arena, 'src>],
    delta: usize,
) -> &'arena [Argument<'arena, 'src>] {
    let mut out = BumpVec::with_capacity_in(arguments.len(), arena);
    for argument in arguments {
        out.push(Argument {
            expression: shift_expr(nodes, arena, &argument.expression, delta),
            passing: argument.passing,
            span: shift_span(argument.span, delta),
        });
    }
    out.into_bump_slice()
}

// Copying a parameter into a new callable also creates a new default
// expression occurrence, even when its diagnostic coordinates stay the same.
fn shift_params<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    params: &[Param<'arena, 'src>],
    delta: usize,
) -> &'arena [Param<'arena, 'src>] {
    let mut out = BumpVec::with_capacity_in(params.len(), arena);
    for param in params {
        out.push(Param {
            parameter: shift_parameter_type(param.parameter, delta),
            name: shift_ident(param.name, delta),
            default: param
                .default
                .as_ref()
                .map(|value| shift_expr(nodes, arena, value, delta)),
            span: shift_span(param.span, delta),
        });
    }
    out.into_bump_slice()
}

fn shift_function<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    func: &FunctionDecl<'arena, 'src>,
    delta: usize,
) -> FunctionDecl<'arena, 'src> {
    let params = shift_params(nodes, arena, func.params, delta);
    FunctionDecl {
        name: shift_ident(func.name, delta),
        params,
        body: shift_stmts(nodes, arena, func.body, delta),
        return_type: shift_type(func.return_type, delta),
        span: shift_span(func.span, delta),
        ..func.clone()
    }
}

fn shift_stmts<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    stmts: &'arena [Stmt<'arena, 'src>],
    delta: usize,
) -> &'arena [Stmt<'arena, 'src>] {
    let mut out = BumpVec::new_in(arena);
    for stmt in stmts {
        out.push(shift_stmt(nodes, arena, stmt, delta));
    }
    out.into_bump_slice()
}

fn shift_stmt<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
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
                .map(|value| shift_expr(nodes, arena, value, delta)),
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
                value: shift_expr(nodes, arena, value, delta),
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
                value: shift_expr(nodes, arena, value, delta),
                span: shift_span(*span, delta),
            }
        }
        Stmt::Expr(value) => Stmt::Expr(shift_expr(nodes, arena, value, delta)),
        Stmt::Return { value, span } => Stmt::Return {
            value: value
                .as_ref()
                .map(|value| shift_expr(nodes, arena, value, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::Throw { value, span } => Stmt::Throw {
            value: shift_expr(nodes, arena, value, delta),
            span: shift_span(*span, delta),
        },
        Stmt::SuperCall { args, span } => Stmt::SuperCall {
            args: shift_arguments(nodes, arena, args, delta),
            span: shift_span(*span, delta),
        },
        Stmt::Yield {
            value,
            delegate,
            span,
        } => Stmt::Yield {
            value: shift_expr(nodes, arena, value, delta),
            delegate: *delegate,
            span: shift_span(*span, delta),
        },
        Stmt::Try {
            body,
            catch,
            finally,
            span,
        } => Stmt::Try {
            body: shift_stmts(nodes, arena, body, delta),
            catch: catch.as_ref().map(|clause| CatchClause {
                binding: clause.binding.map(|binding| CatchBinding {
                    ty: shift_type(binding.ty, delta),
                    name: shift_ident(binding.name, delta),
                    span: shift_span(binding.span, delta),
                }),
                body: shift_stmts(nodes, arena, clause.body, delta),
                span: shift_span(clause.span, delta),
            }),
            finally: finally.map(|body| shift_stmts(nodes, arena, body, delta)),
            span: shift_span(*span, delta),
        },
        Stmt::Block { body, span } => Stmt::Block {
            body: shift_stmts(nodes, arena, body, delta),
            span: shift_span(*span, delta),
        },
        Stmt::If {
            condition,
            then_branch,
            else_branch,
            span,
        } => Stmt::If {
            condition: shift_expr(nodes, arena, condition, delta),
            then_branch: arena.alloc(shift_stmt(nodes, arena, then_branch, delta)),
            else_branch: else_branch
                .map(|branch| &*arena.alloc(shift_stmt(nodes, arena, branch, delta))),
            span: shift_span(*span, delta),
        },
        Stmt::While {
            condition,
            body,
            span,
        } => Stmt::While {
            condition: shift_expr(nodes, arena, condition, delta),
            body: arena.alloc(shift_stmt(nodes, arena, body, delta)),
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
                        .map(|value| shift_expr(nodes, arena, value, delta)),
                    span: shift_span(decl.span, delta),
                }),
                ForInitializer::Expr(value) => {
                    ForInitializer::Expr(shift_expr(nodes, arena, value, delta))
                }
            }),
            condition: condition
                .as_ref()
                .map(|value| shift_expr(nodes, arena, value, delta)),
            update: update
                .as_ref()
                .map(|value| shift_expr(nodes, arena, value, delta)),
            body: arena.alloc(shift_stmt(nodes, arena, body, delta)),
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
            object: shift_expr(nodes, arena, object, delta),
            body: arena.alloc(shift_stmt(nodes, arena, body, delta)),
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
            iterable: shift_expr(nodes, arena, iterable, delta),
            body: arena.alloc(shift_stmt(nodes, arena, body, delta)),
            inline: *inline,
            span: shift_span(*span, delta),
        },
        Stmt::Break(span) => Stmt::Break(shift_span(*span, delta)),
        Stmt::Continue(span) => Stmt::Continue(shift_span(*span, delta)),
    }
}

fn shift_exprs<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    exprs: &'arena [Expr<'arena, 'src>],
    delta: usize,
) -> &'arena [Expr<'arena, 'src>] {
    let mut out = BumpVec::new_in(arena);
    for expr in exprs {
        out.push(shift_expr(nodes, arena, expr, delta));
    }
    out.into_bump_slice()
}

fn shift_expr<'arena, 'src>(
    nodes: &crate::ast::SourceNodes,
    arena: &'arena Bump,
    expr: &Expr<'arena, 'src>,
    delta: usize,
) -> Expr<'arena, 'src> {
    match expr {
        Expr {
            kind: ExprKind::Int(value, span),
            ..
        } => nodes.expression(ExprKind::Int(*value, shift_span(*span, delta))),
        Expr {
            kind: ExprKind::Float(value, span),
            ..
        } => nodes.expression(ExprKind::Float(*value, shift_span(*span, delta))),
        Expr {
            kind: ExprKind::String(value, span),
            ..
        } => nodes.expression(ExprKind::String(value, shift_span(*span, delta))),
        Expr {
            kind: ExprKind::Bool(value, span),
            ..
        } => nodes.expression(ExprKind::Bool(*value, shift_span(*span, delta))),
        Expr {
            kind: ExprKind::Null(span),
            ..
        } => nodes.expression(ExprKind::Null(shift_span(*span, delta))),
        Expr {
            kind: ExprKind::Ident(ident),
            ..
        } => nodes.expression(ExprKind::Ident(shift_ident(*ident, delta))),
        Expr {
            kind: ExprKind::ArrayLiteral { elements, span },
            ..
        } => {
            let mut out = BumpVec::new_in(arena);
            for element in *elements {
                out.push(match element {
                    ArrayElement::Value(value) => {
                        ArrayElement::Value(shift_expr(nodes, arena, value, delta))
                    }
                    ArrayElement::Spread { value, span } => ArrayElement::Spread {
                        value: shift_expr(nodes, arena, value, delta),
                        span: shift_span(*span, delta),
                    },
                });
            }
            nodes.expression(ExprKind::ArrayLiteral {
                elements: out.into_bump_slice(),
                span: shift_span(*span, delta),
            })
        }
        Expr {
            kind: ExprKind::RecordLiteral { entries, span },
            ..
        } => {
            let mut out = BumpVec::new_in(arena);
            for entry in *entries {
                out.push(match entry {
                    RecordElement::Entry(entry) => RecordElement::Entry(RecordEntry {
                        key: shift_ident(entry.key, delta),
                        value: shift_expr(nodes, arena, &entry.value, delta),
                        span: shift_span(entry.span, delta),
                    }),
                    RecordElement::Spread { value, span } => RecordElement::Spread {
                        value: shift_expr(nodes, arena, value, delta),
                        span: shift_span(*span, delta),
                    },
                });
            }
            nodes.expression(ExprKind::RecordLiteral {
                entries: out.into_bump_slice(),
                span: shift_span(*span, delta),
            })
        }
        Expr {
            kind: ExprKind::ObjectLiteral { entries, span },
            ..
        } => {
            let mut out = BumpVec::new_in(arena);
            for entry in *entries {
                out.push(match entry {
                    RecordElement::Entry(entry) => RecordElement::Entry(RecordEntry {
                        key: shift_ident(entry.key, delta),
                        value: shift_expr(nodes, arena, &entry.value, delta),
                        span: shift_span(entry.span, delta),
                    }),
                    RecordElement::Spread { value, span } => RecordElement::Spread {
                        value: shift_expr(nodes, arena, value, delta),
                        span: shift_span(*span, delta),
                    },
                });
            }
            nodes.expression(ExprKind::ObjectLiteral {
                entries: out.into_bump_slice(),
                span: shift_span(*span, delta),
            })
        }
        Expr {
            kind: ExprKind::StructLiteral { name, values, span },
            ..
        } => nodes.expression(ExprKind::StructLiteral {
            name: shift_ident(*name, delta),
            values: shift_exprs(nodes, arena, values, delta),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::New {
                    class,
                    type_args,
                    args,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::New {
            class: shift_ident(*class, delta),
            type_args,
            args: shift_arguments(nodes, arena, args, delta),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::DynamicImport { source, span },
            ..
        } => nodes.expression(ExprKind::DynamicImport {
            source,
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::Member {
                    object,
                    property,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::Member {
            object: arena.alloc(shift_expr(nodes, arena, object, delta)),
            property: shift_ident(*property, delta),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::OptionalMember {
                    object,
                    property,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::OptionalMember {
            object: arena.alloc(shift_expr(nodes, arena, object, delta)),
            property: shift_ident(*property, delta),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::Call { callee, args, span },
            ..
        } => nodes.expression(ExprKind::Call {
            callee: arena.alloc(shift_expr(nodes, arena, callee, delta)),
            args: shift_arguments(nodes, arena, args, delta),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::ArrowFunction { params, body, span },
            ..
        } => nodes.expression(ExprKind::ArrowFunction {
            params: shift_params(nodes, arena, params, delta),
            body: match body {
                ArrowBody::Expr(value) => {
                    ArrowBody::Expr(arena.alloc(shift_expr(nodes, arena, value, delta)))
                }
                ArrowBody::Block(stmts) => {
                    ArrowBody::Block(shift_stmts(nodes, arena, stmts, delta))
                }
            },
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::Unary { op, expr, span },
            ..
        } => nodes.expression(ExprKind::Unary {
            op: *op,
            expr: arena.alloc(shift_expr(nodes, arena, expr, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::Await { task, span },
            ..
        } => nodes.expression(ExprKind::Await {
            task: arena.alloc(shift_expr(nodes, arena, task, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::Binary { op, lhs, rhs, span },
            ..
        } => nodes.expression(ExprKind::Binary {
            op: *op,
            lhs: arena.alloc(shift_expr(nodes, arena, lhs, delta)),
            rhs: arena.alloc(shift_expr(nodes, arena, rhs, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::TypeCheck {
                    value,
                    target,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::TypeCheck {
            value: arena.alloc(shift_expr(nodes, arena, value, delta)),
            target: shift_type(*target, delta),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::Index {
                    object,
                    index,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::Index {
            object: arena.alloc(shift_expr(nodes, arena, object, delta)),
            index: arena.alloc(shift_expr(nodes, arena, index, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::OptionalIndex {
                    object,
                    index,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::OptionalIndex {
            object: arena.alloc(shift_expr(nodes, arena, object, delta)),
            index: arena.alloc(shift_expr(nodes, arena, index, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::If {
                    condition,
                    then_value,
                    else_value,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::If {
            condition: arena.alloc(shift_expr(nodes, arena, condition, delta)),
            then_value: arena.alloc(shift_expr(nodes, arena, then_value, delta)),
            else_value: arena.alloc(shift_expr(nodes, arena, else_value, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::Match { value, arms, span },
            ..
        } => {
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
                    value: shift_expr(nodes, arena, &arm.value, delta),
                    span: shift_span(arm.span, delta),
                });
            }
            nodes.expression(ExprKind::Match {
                value: arena.alloc(shift_expr(nodes, arena, value, delta)),
                arms: out.into_bump_slice(),
                span: shift_span(*span, delta),
            })
        }
        Expr {
            kind:
                ExprKind::Assignment {
                    op,
                    target,
                    value,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::Assignment {
            op: *op,
            target: arena.alloc(shift_expr(nodes, arena, target, delta)),
            value: arena.alloc(shift_expr(nodes, arena, value, delta)),
            span: shift_span(*span, delta),
        }),
        Expr {
            kind:
                ExprKind::Update {
                    op,
                    target,
                    prefix,
                    span,
                },
            ..
        } => nodes.expression(ExprKind::Update {
            op: *op,
            target: arena.alloc(shift_expr(nodes, arena, target, delta)),
            prefix: *prefix,
            span: shift_span(*span, delta),
        }),
        Expr {
            kind: ExprKind::Template { parts, span },
            ..
        } => {
            let mut out = BumpVec::new_in(arena);
            for part in *parts {
                out.push(match part {
                    TemplatePart::String(text, span) => {
                        TemplatePart::String(text, shift_span(*span, delta))
                    }
                    TemplatePart::Expr(value) => {
                        TemplatePart::Expr(shift_expr(nodes, arena, value, delta))
                    }
                });
            }
            nodes.expression(ExprKind::Template {
                parts: out.into_bump_slice(),
                span: shift_span(*span, delta),
            })
        }
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn a_reference_parameter_cannot_be_captured_by_a_generated_picker() {
        let arena = Bump::new();
        let source = crate::parse_source(
            &arena,
            "void sum(int[] values,ref int total){for(int value of values){total+=value;}}",
        )
        .unwrap();
        let expanded = expand_for_of_families(&arena, source, 4);
        assert_eq!(expanded.items.len(), 1);
        let Item::Function(function) = &expanded.items[0] else {
            panic!("function")
        };
        assert_eq!(
            function.params[1].parameter.passing,
            crate::primitive::ParameterPassing::MutableReference
        );
    }

    #[test]
    fn generated_callable_defaults_get_distinct_source_occurrences() {
        let arena = Bump::new();
        let nodes = crate::ast::SourceNodes::default();
        let span = Span::empty(0);
        let default = nodes.expression(ExprKind::Int(7, span));
        let original = default.id;
        let parameter = Param {
            parameter: ParameterType {
                ty: TypeRef {
                    kind: TypeKind::Int,
                    span,
                },
                passing: crate::primitive::ParameterPassing::Value,
                span,
            },
            name: Ident {
                name: "value",
                span,
            },
            default: Some(default),
            span,
        };
        let first = shift_params(&nodes, &arena, std::slice::from_ref(&parameter), 0);
        let second = shift_params(&nodes, &arena, std::slice::from_ref(&parameter), 0);
        let first = first[0].default.as_ref().unwrap();
        let second = second[0].default.as_ref().unwrap();
        assert_eq!(first.span(), second.span());
        assert_ne!(first.id, second.id);
        assert_ne!(first.id, original);
        assert_ne!(second.id, original);
        assert_eq!(nodes.finish().len(), 3);
    }
}
