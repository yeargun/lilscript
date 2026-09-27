//! The admission parse (plan task M2.5; architecture A5, L11 and §17).
//!
//! Every delivered JavaScript file is parsed again, by Oxc, an independent
//! standards parser, and the structure it reads must be the structure the
//! printer printed. A syntax error or a different structure refuses the
//! artifact with a diagnostic, before any score can select it.
//!
//! Both sides reduce to one canonical form, [`Canon`]: the printed tree through
//! `js::admission`, the delivered text through this module's walker over Oxc's
//! AST. The form keeps what a parse decides, namely which tokens make which
//! statement and expression and how they nest, and drops what a printer may
//! spell either way, as a closed list of equivalences (the smart constructors
//! on [`Canon`], applied identically to both sides):
//! - every literal is `Lit`, so `!0`, `void 0`, `-5`, `1e3` and a string printed
//!   as its observed truthiness compare equal; `!`, `-` and `void` of a literal
//!   fold to `Lit`;
//! - `x|<literal>` is `x` wherever it appears: integer normalization is
//!   printed or omitted by facts, not by grammar;
//! - `(0,x)` is `x`, and `{n:f}.n` with `f` a function is `f` (the printer's
//!   call-unbinding and name-suppressing wrappers);
//! - `x op= y` and `++x` are `x = x op y`;
//! - an expression statement `a&&b` is `if(a)b`, `a||b` is `if(!a)b`, and
//!   `x=c?a:b` is `if(c)x=a;else x=b`; a statement's outer `void` is dropped;
//! - `if(c)return a;else return b` and `if(c)return a;return b` are
//!   `return c?a:b`;
//! - `let a=1,b=2` is two declarations; `for(let i=v;…)` is
//!   `{let i=v;for(;…)}`; `while(c)` is `for(;c;)`; a body is its statement
//!   list, braced or not; an arrow's expression body is a `return`.
//!
//! Identifiers are not compared (naming is not structure), and a
//! destructuring declaration (the host-module prelude, foreign text) is opaque.
//! What the printer never emits (labels, `switch`, classes' fields, optional
//! chains) is kept by kind, so its appearance is a difference.

use sha2::{Digest, Sha256};

/// A function, in both its declaration and expression forms.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CanonFunction {
    pub arrow: bool,
    pub asynchronous: bool,
    pub generator: bool,
    pub strict: bool,
    /// One entry per parameter: its default, if it has one.
    pub parameters: Vec<Option<Canon>>,
    pub rest: bool,
    pub body: Vec<Canon>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Canon {
    // Statements.
    Let(Option<Box<Canon>>),
    /// A destructuring declaration, compared by kind only.
    Pattern,
    Expr(Box<Canon>),
    Return(Option<Box<Canon>>),
    Throw(Box<Canon>),
    If(Box<Canon>, Vec<Canon>, Option<Vec<Canon>>),
    /// `for (; test; update) body`.
    Loop(Option<Box<Canon>>, Option<Box<Canon>>, Vec<Canon>),
    Block(Vec<Canon>),
    ForIn(Box<Canon>, Vec<Canon>),
    ForOf(Box<Canon>, Vec<Canon>),
    /// Body, catch (whether it binds, body), finally.
    Try(Vec<Canon>, Option<(bool, Vec<Canon>)>, Option<Vec<Canon>>),
    Break,
    Continue,
    FunctionDeclaration(Box<CanonFunction>),
    /// An import declaration with this many specifiers.
    Import(usize),
    /// An export list with this many specifiers.
    Export(usize),
    // Expressions.
    Lit,
    Regex,
    Ident,
    This,
    Super,
    Unary(&'static str, Box<Canon>),
    Binary(&'static str, Box<Canon>, Box<Canon>),
    Assign(Box<Canon>, Box<Canon>),
    Conditional(Box<Canon>, Box<Canon>, Box<Canon>),
    Call(Box<Canon>, Vec<Canon>),
    New(Box<Canon>, Vec<Canon>),
    /// Object and computed key; `None` is a literal key (`.k`, `["k"]`, `[0]`).
    Member(Box<Canon>, Option<Box<Canon>>),
    Sequence(Vec<Canon>),
    Template(Vec<Canon>),
    Array(Vec<Canon>),
    Spread(Box<Canon>),
    /// Entries: a computed key (`None` for a literal or identifier key) and the value.
    Object(Vec<(Option<Canon>, Canon)>),
    Function(Box<CanonFunction>),
    /// Heritage and methods (the constructor first when printed first).
    Class(Option<Box<Canon>>, Vec<CanonFunction>),
    Await(Box<Canon>),
    Yield(bool, Option<Box<Canon>>),
    ImportCall(Box<Canon>),
    /// A construct the printer never emits, by kind.
    Other(&'static str),
}

fn boxed(value: Canon) -> Box<Canon> {
    Box::new(value)
}

impl Canon {
    /// `!`, `-` and `void` of a literal are a literal: `!0`, `-5`, `void 0`.
    pub(crate) fn unary(operator: &'static str, value: Canon) -> Canon {
        if value == Canon::Lit && matches!(operator, "!" | "-" | "void") {
            return Canon::Lit;
        }
        Canon::Unary(operator, boxed(value))
    }

    /// `x|<literal>` is `x`: integer normalization is a fact, not grammar.
    pub(crate) fn binary(operator: &'static str, left: Canon, right: Canon) -> Canon {
        if operator == "|" && right == Canon::Lit {
            return left;
        }
        Canon::Binary(operator, boxed(left), boxed(right))
    }

    /// `(0,x)` is `x`: the printer's call-unbinding and name-suppressing wrapper.
    pub(crate) fn sequence(mut values: Vec<Canon>) -> Canon {
        if values.len() == 2 && values[0] == Canon::Lit {
            return values.pop().unwrap();
        }
        Canon::Sequence(values)
    }

    /// `{n:f}.n` with `f` a function is `f`: the printer's name wrapper.
    pub(crate) fn member(object: Canon, key: Option<Canon>) -> Canon {
        if key.is_none() {
            if let Canon::Object(entries) = &object {
                if let [(None, Canon::Function(_))] = entries.as_slice() {
                    let Canon::Object(mut entries) = object else {
                        unreachable!()
                    };
                    return entries.pop().unwrap().1;
                }
            }
        }
        Canon::Member(boxed(object), key.map(boxed))
    }

    /// A computed member or property key that is a literal is a literal key.
    pub(crate) fn key(key: Canon) -> Option<Canon> {
        (key != Canon::Lit).then_some(key)
    }

    /// An expression statement: its outer `void` dropped, and the statement
    /// spellings of `if` (`a&&b`, `a||b`, `x=c?a:b`) read back as `if`.
    pub(crate) fn statement(value: Canon) -> Canon {
        let mut value = value;
        loop {
            match value {
                Canon::Unary("void", inner) => value = *inner,
                other => {
                    value = other;
                    break;
                }
            }
        }
        match value {
            Canon::Binary("&&", left, right) => {
                Canon::If(left, vec![Canon::statement(*right)], None)
            }
            Canon::Binary("||", left, right) => Canon::If(
                boxed(Canon::unary("!", *left)),
                vec![Canon::statement(*right)],
                None,
            ),
            Canon::Assign(target, value) if *target == Canon::Ident => match *value {
                Canon::Conditional(condition, yes, no) => Canon::If(
                    condition,
                    vec![Canon::statement(Canon::Assign(boxed(Canon::Ident), yes))],
                    Some(vec![Canon::statement(Canon::Assign(
                        boxed(Canon::Ident),
                        no,
                    ))]),
                ),
                value => Canon::Expr(boxed(Canon::Assign(target, boxed(value)))),
            },
            value => Canon::Expr(boxed(value)),
        }
    }

    /// `if(c)return a;else return b` is `return c?a:b`.
    pub(crate) fn if_statement(condition: Canon, yes: Vec<Canon>, no: Option<Vec<Canon>>) -> Canon {
        let single_return = |list: &[Canon]| matches!(list, [Canon::Return(Some(_))]);
        match no {
            Some(no) if single_return(&yes) && single_return(&no) => {
                let returned = |list: Vec<Canon>| match list.into_iter().next() {
                    Some(Canon::Return(Some(value))) => value,
                    _ => unreachable!("a single return"),
                };
                Canon::Return(Some(boxed(Canon::Conditional(
                    boxed(condition),
                    returned(yes),
                    returned(no),
                ))))
            }
            no => Canon::If(boxed(condition), yes, no),
        }
    }
}

/// A statement list: `if(c)return a;return b` is `return c?a:b`, applied from
/// the end so a chain folds whole; empty statements are dropped.
pub(crate) fn statement_list(mut statements: Vec<Canon>) -> Vec<Canon> {
    statements.retain(|statement| *statement != Canon::Other("empty"));
    let mut index = statements.len();
    while index >= 2 {
        index -= 1;
        let foldable = matches!(
            (&statements[index - 1], &statements[index]),
            (Canon::If(_, yes, None), Canon::Return(Some(_)))
                if matches!(yes.as_slice(), [Canon::Return(Some(_))])
        );
        if foldable {
            let Canon::Return(Some(b)) = statements.remove(index) else {
                unreachable!()
            };
            let Canon::If(condition, yes, None) = statements.remove(index - 1) else {
                unreachable!()
            };
            let Some(Canon::Return(Some(a))) = yes.into_iter().next() else {
                unreachable!()
            };
            statements.insert(
                index - 1,
                Canon::Return(Some(boxed(Canon::Conditional(condition, a, b)))),
            );
        }
    }
    statements
}

// ---------------------------------------------------------------- digest

/// Tags of the serialized form. Stable only within one binary: a digest is
/// compared with one computed by the same compiler, never stored.
fn write(canon: &Canon, out: &mut Vec<u8>) {
    let tag = |out: &mut Vec<u8>, tag: u8| out.push(tag);
    let count =
        |out: &mut Vec<u8>, count: usize| out.extend_from_slice(&(count as u32).to_le_bytes());
    let text = |out: &mut Vec<u8>, text: &str| {
        count(out, text.len());
        out.extend_from_slice(text.as_bytes());
    };
    let list = |out: &mut Vec<u8>, items: &[Canon]| {
        count(out, items.len());
        for item in items {
            write(item, out);
        }
    };
    let option = |out: &mut Vec<u8>, item: Option<&Canon>| match item {
        Some(item) => {
            tag(out, 1);
            write(item, out);
        }
        None => tag(out, 0),
    };
    match canon {
        Canon::Let(value) => {
            tag(out, 1);
            option(out, value.as_deref());
        }
        Canon::Pattern => tag(out, 2),
        Canon::Expr(value) => {
            tag(out, 3);
            write(value, out);
        }
        Canon::Return(value) => {
            tag(out, 4);
            option(out, value.as_deref());
        }
        Canon::Throw(value) => {
            tag(out, 5);
            write(value, out);
        }
        Canon::If(condition, yes, no) => {
            tag(out, 6);
            write(condition, out);
            list(out, yes);
            match no {
                Some(no) => {
                    tag(out, 1);
                    list(out, no);
                }
                None => tag(out, 0),
            }
        }
        Canon::Loop(test, update, body) => {
            tag(out, 7);
            option(out, test.as_deref());
            option(out, update.as_deref());
            list(out, body);
        }
        Canon::Block(body) => {
            tag(out, 8);
            list(out, body);
        }
        Canon::ForIn(object, body) => {
            tag(out, 9);
            write(object, out);
            list(out, body);
        }
        Canon::ForOf(iterable, body) => {
            tag(out, 10);
            write(iterable, out);
            list(out, body);
        }
        Canon::Try(body, catch, finally) => {
            tag(out, 11);
            list(out, body);
            match catch {
                Some((binds, body)) => {
                    tag(out, 1 + u8::from(*binds));
                    list(out, body);
                }
                None => tag(out, 0),
            }
            match finally {
                Some(body) => {
                    tag(out, 1);
                    list(out, body);
                }
                None => tag(out, 0),
            }
        }
        Canon::Break => tag(out, 12),
        Canon::Continue => tag(out, 13),
        Canon::FunctionDeclaration(function) => {
            tag(out, 14);
            write_function(function, out);
        }
        Canon::Import(specifiers) => {
            tag(out, 15);
            count(out, *specifiers);
        }
        Canon::Export(specifiers) => {
            tag(out, 16);
            count(out, *specifiers);
        }
        Canon::Lit => tag(out, 17),
        Canon::Regex => tag(out, 18),
        Canon::Ident => tag(out, 19),
        Canon::This => tag(out, 20),
        Canon::Super => tag(out, 21),
        Canon::Unary(operator, value) => {
            tag(out, 22);
            text(out, operator);
            write(value, out);
        }
        Canon::Binary(operator, left, right) => {
            tag(out, 23);
            text(out, operator);
            write(left, out);
            write(right, out);
        }
        Canon::Assign(target, value) => {
            tag(out, 24);
            write(target, out);
            write(value, out);
        }
        Canon::Conditional(condition, yes, no) => {
            tag(out, 25);
            write(condition, out);
            write(yes, out);
            write(no, out);
        }
        Canon::Call(callee, arguments) => {
            tag(out, 26);
            write(callee, out);
            list(out, arguments);
        }
        Canon::New(callee, arguments) => {
            tag(out, 27);
            write(callee, out);
            list(out, arguments);
        }
        Canon::Member(object, key) => {
            tag(out, 28);
            write(object, out);
            option(out, key.as_deref());
        }
        Canon::Sequence(values) => {
            tag(out, 29);
            list(out, values);
        }
        Canon::Template(values) => {
            tag(out, 30);
            list(out, values);
        }
        Canon::Array(values) => {
            tag(out, 31);
            list(out, values);
        }
        Canon::Spread(value) => {
            tag(out, 32);
            write(value, out);
        }
        Canon::Object(entries) => {
            tag(out, 33);
            count(out, entries.len());
            for (key, value) in entries {
                option(out, key.as_ref());
                write(value, out);
            }
        }
        Canon::Function(function) => {
            tag(out, 34);
            write_function(function, out);
        }
        Canon::Class(base, methods) => {
            tag(out, 35);
            option(out, base.as_deref());
            count(out, methods.len());
            for method in methods {
                write_function(method, out);
            }
        }
        Canon::Await(value) => {
            tag(out, 36);
            write(value, out);
        }
        Canon::Yield(delegate, value) => {
            tag(out, 37 + u8::from(*delegate));
            option(out, value.as_deref());
        }
        Canon::ImportCall(source) => {
            tag(out, 39);
            write(source, out);
        }
        Canon::Other(kind) => {
            tag(out, 40);
            text(out, kind);
        }
    }
}

fn write_function(function: &CanonFunction, out: &mut Vec<u8>) {
    out.push(
        u8::from(function.arrow)
            | u8::from(function.asynchronous) << 1
            | u8::from(function.generator) << 2
            | u8::from(function.strict) << 3
            | u8::from(function.rest) << 4,
    );
    out.extend_from_slice(&(function.parameters.len() as u32).to_le_bytes());
    for parameter in &function.parameters {
        match parameter {
            Some(default) => {
                out.push(1);
                write(default, out);
            }
            None => out.push(0),
        }
    }
    out.extend_from_slice(&(function.body.len() as u32).to_le_bytes());
    for statement in &function.body {
        write(statement, out);
    }
}

/// How many top-level statements share one diagnostic bucket at most.
pub(crate) const BUCKETS: usize = 16;

/// The structural digest of one delivered file: the whole program's digest,
/// its top-level statement count, and digests of the top-level statements in
/// 16 contiguous buckets, so a mismatch names where it starts. Fixed size: it
/// lives inline in an artifact record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StructureDigest {
    pub digest: [u8; 32],
    pub roots: u32,
    pub buckets: [u64; BUCKETS],
}

fn bucket_of(index: usize, roots: usize) -> usize {
    (index * BUCKETS / roots.max(1)).min(BUCKETS - 1)
}

pub(crate) fn digest(program: &[Canon]) -> StructureDigest {
    let mut whole = Sha256::new();
    let mut buckets: [Sha256; BUCKETS] = std::array::from_fn(|_| Sha256::new());
    let mut bytes = Vec::new();
    for (index, statement) in program.iter().enumerate() {
        bytes.clear();
        write(statement, &mut bytes);
        whole.update(&bytes);
        buckets[bucket_of(index, program.len())].update(&bytes);
    }
    whole.update((program.len() as u64).to_le_bytes());
    let buckets = buckets.map(|bucket| {
        let digest = bucket.finalize();
        u64::from_le_bytes(digest[..8].try_into().unwrap())
    });
    StructureDigest {
        digest: whole.finalize().into(),
        roots: program.len() as u32,
        buckets,
    }
}

/// The work a digest of `bytes` of text costs, in analysis units: parsing and
/// the walk are linear in the text (charged by the caller before parsing).
pub(crate) fn work_units(bytes: usize) -> u64 {
    bytes as u64
}

// ---------------------------------------------------------------- Oxc side

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, ArrowFunctionBody, AssignmentTarget, BindingPattern,
    ClassElement, Expression, ForStatementInit, FormalParameters, Function, FunctionBody,
    ObjectPropertyKind, Program, PropertyKey, SimpleAssignmentTarget, Statement,
    VariableDeclaration,
};

/// Oxc's AST of the delivered text, in the canonical form, with the byte span
/// of the source statement each top-level canonical statement came from.
pub(crate) fn canonical_program(program: &Program<'_>) -> (Vec<Canon>, Vec<(u32, u32)>) {
    let mut statements = Vec::new();
    let mut spans = Vec::new();
    for directive in &program.directives {
        statements.push(Canon::Expr(Box::new(Canon::Lit)));
        spans.push((directive.span.start, directive.span.end));
    }
    for statement in &program.body {
        let before = statements.len();
        oxc_statement(statement, &mut statements);
        let span = oxc_span::GetSpan::span(statement);
        spans.extend(std::iter::repeat_n(
            (span.start, span.end),
            statements.len() - before,
        ));
    }
    // The list folds (`if(c)return a;return b`) never apply at the top level,
    // where `return` is not allowed; empty statements are dropped with spans.
    let mut kept = Vec::with_capacity(statements.len());
    let mut kept_spans = Vec::with_capacity(spans.len());
    for (statement, span) in statements.into_iter().zip(spans) {
        if statement != Canon::Other("empty") {
            kept.push(statement);
            kept_spans.push(span);
        }
    }
    (kept, kept_spans)
}

fn oxc_list<'a>(statements: impl IntoIterator<Item = &'a Statement<'a>>) -> Vec<Canon> {
    let mut out = Vec::new();
    for statement in statements {
        oxc_statement(statement, &mut out);
    }
    statement_list(out)
}

/// A statement body: a block is its statements (one level), anything else is
/// a one-statement list.
fn oxc_body(statement: &Statement<'_>) -> Vec<Canon> {
    match statement {
        Statement::BlockStatement(block) => oxc_list(&block.body),
        statement => oxc_list(std::iter::once(statement)),
    }
}

fn oxc_declaration(declaration: &VariableDeclaration<'_>, out: &mut Vec<Canon>) {
    for declarator in &declaration.declarations {
        if !matches!(declarator.id, BindingPattern::BindingIdentifier(_)) {
            out.push(Canon::Pattern);
            continue;
        }
        let kind = match declaration.kind {
            oxc_ast::ast::VariableDeclarationKind::Let => None,
            _ => Some(Canon::Other("var-or-const")),
        };
        if let Some(kind) = kind {
            out.push(kind);
        }
        out.push(Canon::Let(
            declarator
                .init
                .as_ref()
                .map(|init| Box::new(oxc_expression(init))),
        ));
    }
}

fn oxc_statement(statement: &Statement<'_>, out: &mut Vec<Canon>) {
    let canon = match statement {
        Statement::BlockStatement(block) => Canon::Block(oxc_list(&block.body)),
        Statement::BreakStatement(statement) => {
            if statement.label.is_some() {
                Canon::Other("labeled-break")
            } else {
                Canon::Break
            }
        }
        Statement::ContinueStatement(statement) => {
            if statement.label.is_some() {
                Canon::Other("labeled-continue")
            } else {
                Canon::Continue
            }
        }
        Statement::EmptyStatement(_) => Canon::Other("empty"),
        Statement::ExpressionStatement(statement) => {
            Canon::statement(oxc_expression(&statement.expression))
        }
        Statement::IfStatement(statement) => Canon::if_statement(
            oxc_expression(&statement.test),
            oxc_body(&statement.consequent),
            statement.alternate.as_ref().map(oxc_body),
        ),
        Statement::WhileStatement(statement) => Canon::Loop(
            Some(Box::new(oxc_expression(&statement.test))),
            None,
            oxc_body(&statement.body),
        ),
        Statement::ForStatement(statement) => {
            let looped = Canon::Loop(
                statement
                    .test
                    .as_ref()
                    .map(|test| Box::new(oxc_expression(test))),
                statement
                    .update
                    .as_ref()
                    .map(|update| Box::new(oxc_expression(update))),
                oxc_body(&statement.body),
            );
            match &statement.init {
                None => looped,
                Some(ForStatementInit::VariableDeclaration(declaration)) => {
                    let mut block = Vec::new();
                    oxc_declaration(declaration, &mut block);
                    block.push(looped);
                    Canon::Block(block)
                }
                Some(init) => Canon::Block(vec![
                    Canon::statement(
                        init.as_expression()
                            .map_or(Canon::Other("for-init"), oxc_expression),
                    ),
                    looped,
                ]),
            }
        }
        Statement::ForInStatement(statement) => Canon::ForIn(
            Box::new(oxc_expression(&statement.right)),
            oxc_body(&statement.body),
        ),
        Statement::ForOfStatement(statement) => {
            if statement.r#await {
                Canon::Other("for-await")
            } else {
                Canon::ForOf(
                    Box::new(oxc_expression(&statement.right)),
                    oxc_body(&statement.body),
                )
            }
        }
        Statement::ReturnStatement(statement) => Canon::Return(
            statement
                .argument
                .as_ref()
                .map(|value| Box::new(oxc_expression(value))),
        ),
        Statement::ThrowStatement(statement) => {
            Canon::Throw(Box::new(oxc_expression(&statement.argument)))
        }
        Statement::TryStatement(statement) => Canon::Try(
            oxc_list(&statement.block.body),
            statement
                .handler
                .as_ref()
                .map(|handler| (handler.param.is_some(), oxc_list(&handler.body.body))),
            statement
                .finalizer
                .as_ref()
                .map(|finalizer| oxc_list(&finalizer.body)),
        ),
        Statement::VariableDeclaration(declaration) => {
            oxc_declaration(declaration, out);
            return;
        }
        Statement::FunctionDeclaration(function) => {
            Canon::FunctionDeclaration(Box::new(oxc_function(function)))
        }
        Statement::ClassDeclaration(_) => Canon::Other("class-declaration"),
        Statement::ImportDeclaration(declaration) => Canon::Import(
            declaration
                .specifiers
                .as_ref()
                .map_or(0, |specifiers| specifiers.len()),
        ),
        Statement::ExportNamedDeclaration(declaration) => {
            Canon::Export(declaration.specifiers.len())
        }
        Statement::DebuggerStatement(_) => Canon::Other("debugger"),
        Statement::DoWhileStatement(_) => Canon::Other("do-while"),
        Statement::LabeledStatement(_) => Canon::Other("label"),
        Statement::SwitchStatement(_) => Canon::Other("switch"),
        Statement::WithStatement(_) => Canon::Other("with"),
        _ => Canon::Other("declaration"),
    };
    out.push(canon);
}

fn oxc_parameters(parameters: &FormalParameters<'_>) -> (Vec<Option<Canon>>, bool) {
    let list = parameters
        .items
        .iter()
        .map(|parameter| {
            if !matches!(parameter.pattern, BindingPattern::BindingIdentifier(_)) {
                Some(Canon::Other("parameter-pattern"))
            } else {
                parameter
                    .initializer
                    .as_ref()
                    .map(|default| oxc_expression(default))
            }
        })
        .collect();
    (list, parameters.rest.is_some())
}

fn oxc_function_body(body: Option<&FunctionBody<'_>>) -> (bool, Vec<Canon>) {
    let Some(body) = body else {
        return (false, Vec::new());
    };
    let mut strict = false;
    let mut statements = Vec::new();
    for directive in &body.directives {
        if directive.directive.as_str() == "use strict" {
            strict = true;
        } else {
            statements.push(Canon::Expr(Box::new(Canon::Lit)));
        }
    }
    for statement in &body.statements {
        oxc_statement(statement, &mut statements);
    }
    (strict, statement_list(statements))
}

fn oxc_function(function: &Function<'_>) -> CanonFunction {
    let (parameters, rest) = oxc_parameters(&function.params);
    let (strict, body) = oxc_function_body(function.body.as_deref());
    CanonFunction {
        arrow: false,
        asynchronous: function.r#async,
        generator: function.generator,
        strict,
        parameters,
        rest,
        body,
    }
}

fn oxc_arguments(arguments: &[Argument<'_>]) -> Vec<Canon> {
    arguments
        .iter()
        .map(|argument| match argument {
            Argument::SpreadElement(spread) => {
                Canon::Spread(Box::new(oxc_expression(&spread.argument)))
            }
            argument => argument
                .as_expression()
                .map_or(Canon::Other("argument"), oxc_expression),
        })
        .collect()
}

fn oxc_simple_target(target: &SimpleAssignmentTarget<'_>) -> Canon {
    match target {
        SimpleAssignmentTarget::AssignmentTargetIdentifier(_) => Canon::Ident,
        target => target
            .as_member_expression()
            .map_or(Canon::Other("assignment-target"), |member| {
                oxc_expression(member.as_expression())
            }),
    }
}

fn oxc_target(target: &AssignmentTarget<'_>) -> Canon {
    target
        .as_simple_assignment_target()
        .map_or(Canon::Other("pattern-assignment"), oxc_simple_target)
}

fn oxc_property_key(key: &PropertyKey<'_>, computed: bool) -> Option<Canon> {
    match key {
        PropertyKey::StaticIdentifier(_) => None,
        PropertyKey::PrivateIdentifier(_) => Some(Canon::Other("private-key")),
        key => {
            let expression = key
                .as_expression()
                .map_or(Canon::Other("key"), oxc_expression);
            if computed {
                Canon::key(expression)
            } else {
                // `"k":` and `5:` are literal keys.
                None
            }
        }
    }
}

fn oxc_expression(expression: &Expression<'_>) -> Canon {
    match expression {
        Expression::BooleanLiteral(_)
        | Expression::NullLiteral(_)
        | Expression::NumericLiteral(_)
        | Expression::BigIntLiteral(_)
        | Expression::StringLiteral(_) => Canon::Lit,
        Expression::RegExpLiteral(_) => Canon::Regex,
        Expression::TemplateLiteral(template) => {
            Canon::Template(template.expressions.iter().map(oxc_expression).collect())
        }
        Expression::Identifier(_) => Canon::Ident,
        Expression::Super(_) => Canon::Super,
        Expression::ThisExpression(_) => Canon::This,
        Expression::ArrayExpression(array) => Canon::Array(
            array
                .elements
                .iter()
                .map(|element| match element {
                    ArrayExpressionElement::SpreadElement(spread) => {
                        Canon::Spread(Box::new(oxc_expression(&spread.argument)))
                    }
                    ArrayExpressionElement::Elision(_) => Canon::Other("hole"),
                    element => element
                        .as_expression()
                        .map_or(Canon::Other("element"), oxc_expression),
                })
                .collect(),
        ),
        Expression::ArrowFunctionExpression(arrow) => {
            let (parameters, rest) = oxc_parameters(&arrow.params);
            let (strict, body) = match &arrow.body {
                ArrowFunctionBody::FunctionBody(body) => oxc_function_body(Some(body)),
                body => (
                    false,
                    vec![Canon::Return(Some(Box::new(
                        body.as_expression()
                            .map_or(Canon::Other("arrow-body"), oxc_expression),
                    )))],
                ),
            };
            Canon::Function(Box::new(CanonFunction {
                arrow: true,
                asynchronous: arrow.r#async,
                generator: false,
                strict,
                parameters,
                rest,
                body,
            }))
        }
        Expression::AssignmentExpression(assignment) => {
            let target = oxc_target(&assignment.left);
            let value = oxc_expression(&assignment.right);
            let operator = assignment.operator;
            if operator == oxc_ast::ast::AssignmentOperator::Assign {
                Canon::Assign(Box::new(target), Box::new(value))
            } else {
                let binary = operator
                    .to_binary_operator()
                    .map(|binary| binary.as_str())
                    .or_else(|| {
                        operator
                            .to_logical_operator()
                            .map(|logical| logical.as_str())
                    })
                    .unwrap_or("?=");
                Canon::Assign(
                    Box::new(target.clone()),
                    Box::new(Canon::binary(binary, target, value)),
                )
            }
        }
        Expression::UpdateExpression(update) => {
            let target = oxc_simple_target(&update.argument);
            let operator = if update.operator == oxc_ast::ast::UpdateOperator::Increment {
                "+"
            } else {
                "-"
            };
            Canon::Assign(
                Box::new(target.clone()),
                Box::new(Canon::binary(operator, target, Canon::Lit)),
            )
        }
        Expression::AwaitExpression(value) => {
            Canon::Await(Box::new(oxc_expression(&value.argument)))
        }
        Expression::BinaryExpression(binary) => Canon::binary(
            binary.operator.as_str(),
            oxc_expression(&binary.left),
            oxc_expression(&binary.right),
        ),
        Expression::LogicalExpression(logical) => Canon::binary(
            logical.operator.as_str(),
            oxc_expression(&logical.left),
            oxc_expression(&logical.right),
        ),
        Expression::CallExpression(call) => {
            if call.optional {
                Canon::Other("optional-call")
            } else {
                Canon::Call(
                    Box::new(oxc_expression(&call.callee)),
                    oxc_arguments(&call.arguments),
                )
            }
        }
        Expression::NewExpression(new) => Canon::New(
            Box::new(oxc_expression(&new.callee)),
            oxc_arguments(&new.arguments),
        ),
        Expression::ChainExpression(_) => Canon::Other("chain"),
        Expression::ClassExpression(class) => {
            let mut methods = Vec::new();
            for element in &class.body.body {
                match element {
                    ClassElement::MethodDefinition(method) => {
                        methods.push(oxc_function(&method.value))
                    }
                    _ => methods.push(CanonFunction {
                        arrow: false,
                        asynchronous: false,
                        generator: false,
                        strict: false,
                        parameters: vec![Some(Canon::Other("class-element"))],
                        rest: false,
                        body: Vec::new(),
                    }),
                }
            }
            Canon::Class(
                class
                    .super_class
                    .as_ref()
                    .map(|base| Box::new(oxc_expression(base))),
                methods,
            )
        }
        Expression::ConditionalExpression(conditional) => Canon::Conditional(
            Box::new(oxc_expression(&conditional.test)),
            Box::new(oxc_expression(&conditional.consequent)),
            Box::new(oxc_expression(&conditional.alternate)),
        ),
        Expression::FunctionExpression(function) => {
            Canon::Function(Box::new(oxc_function(function)))
        }
        Expression::ImportExpression(import) => {
            if import.options.is_some() {
                Canon::Other("import-options")
            } else {
                Canon::ImportCall(Box::new(oxc_expression(&import.source)))
            }
        }
        Expression::ObjectExpression(object) => Canon::Object(
            object
                .properties
                .iter()
                .map(|property| match property {
                    ObjectPropertyKind::ObjectProperty(property) => {
                        if property.method || property.kind != oxc_ast::ast::PropertyKind::Init {
                            (
                                Some(Canon::Other("method-or-accessor")),
                                oxc_expression(&property.value),
                            )
                        } else {
                            (
                                oxc_property_key(&property.key, property.computed),
                                oxc_expression(&property.value),
                            )
                        }
                    }
                    ObjectPropertyKind::SpreadProperty(spread) => (
                        Some(Canon::Other("spread-key")),
                        Canon::Spread(Box::new(oxc_expression(&spread.argument))),
                    ),
                })
                .collect(),
        ),
        Expression::ParenthesizedExpression(inner) => oxc_expression(&inner.expression),
        Expression::SequenceExpression(sequence) => {
            Canon::sequence(sequence.expressions.iter().map(oxc_expression).collect())
        }
        Expression::TaggedTemplateExpression(_) => Canon::Other("tagged-template"),
        Expression::UnaryExpression(unary) => {
            Canon::unary(unary.operator.as_str(), oxc_expression(&unary.argument))
        }
        Expression::YieldExpression(value) => Canon::Yield(
            value.delegate,
            value
                .argument
                .as_ref()
                .map(|value| Box::new(oxc_expression(value))),
        ),
        Expression::StaticMemberExpression(member) => {
            if member.optional {
                Canon::Other("optional-member")
            } else {
                Canon::member(oxc_expression(&member.object), None)
            }
        }
        Expression::ComputedMemberExpression(member) => {
            if member.optional {
                Canon::Other("optional-member")
            } else {
                Canon::member(
                    oxc_expression(&member.object),
                    Canon::key(oxc_expression(&member.expression)),
                )
            }
        }
        Expression::PrivateFieldExpression(_) => Canon::Other("private-field"),
        Expression::PrivateInExpression(_) => Canon::Other("private-in"),
        Expression::ImportMeta(_) => Canon::Other("import-meta"),
        Expression::NewTarget(_) => Canon::Other("new-target"),
        _ => Canon::Other("typescript-or-jsx"),
    }
}

// ---------------------------------------------------------------- admission

/// Why the independent parse refused a delivered file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Refusal(pub String);

fn excerpt(text: &str, at: usize) -> String {
    let at = at.min(text.len());
    let mut start = at.saturating_sub(40);
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let mut end = (at + 80).min(text.len());
    while !text.is_char_boundary(end) {
        end += 1;
    }
    text[start..end].replace('\n', " ")
}

/// Parse `text` as a script or a module and return its canonical program and
/// the spans of its top-level canonical statements, or the parser's refusal.
pub(crate) fn parse_canonical(
    text: &str,
    module: bool,
) -> Result<(Vec<Canon>, Vec<(u32, u32)>), Refusal> {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if module {
        oxc_span::SourceType::mjs()
    } else {
        oxc_span::SourceType::script()
    };
    let parsed = oxc_parser::Parser::new(&allocator, text, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            preserve_parens: false,
            enable_ident_hashes: false,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        let (message, at) = parsed.diagnostics.first().map_or_else(
            || ("the parser could not recover".to_string(), 0),
            |diagnostic| {
                let at = diagnostic
                    .labels
                    .first()
                    .map_or(0, |label| label.offset() as usize);
                (diagnostic.message.to_string(), at)
            },
        );
        return Err(Refusal(format!(
            "the delivered {} does not parse: {message} at byte {at}: `{}`",
            if module { "module" } else { "script" },
            excerpt(text, at)
        )));
    }
    if !module {
        // Oxc leaves module syntax in a script to semantic analysis; a script
        // that imports or exports is refused here.
        if let Some(statement) = parsed
            .program
            .body
            .iter()
            .find(|statement| statement.as_module_declaration().is_some())
        {
            let at = oxc_span::GetSpan::span(statement).start as usize;
            return Err(Refusal(format!(
                "the delivered script imports or exports at byte {at}: `{}`",
                excerpt(text, at)
            )));
        }
    }
    Ok(canonical_program(&parsed.program))
}

/// Admit one delivered file: it parses under its execution, and its structure
/// is the printed tree's (`expected`, from `js::admission`), or it is refused
/// with where the two first differ.
pub(crate) fn admit(expected: &StructureDigest, text: &str, module: bool) -> Result<(), Refusal> {
    let (program, spans) = parse_canonical(text, module)?;
    let found = digest(&program);
    if found == *expected {
        return Ok(());
    }
    let what = if module { "module" } else { "script" };
    if found.roots != expected.roots {
        return Err(Refusal(format!(
            "the delivered {what} parses to {} top-level statements, but the printed tree has {}: {}",
            found.roots,
            expected.roots,
            first_difference(expected, &found, &spans, text)
        )));
    }
    Err(Refusal(format!(
        "the delivered {what} parses to a different structure than the printed tree: {}",
        first_difference(expected, &found, &spans, text)
    )))
}

fn first_difference(
    expected: &StructureDigest,
    found: &StructureDigest,
    spans: &[(u32, u32)],
    text: &str,
) -> String {
    let bucket = (0..BUCKETS)
        .find(|&bucket| expected.buckets[bucket] != found.buckets[bucket])
        .unwrap_or(0);
    let roots = spans.len();
    let first = (0..roots).find(|&index| bucket_of(index, roots) == bucket);
    match first {
        Some(index) => {
            let last = (index..roots)
                .take_while(|&index| bucket_of(index, roots) == bucket)
                .last()
                .unwrap_or(index);
            let (start, _) = spans[index];
            let (_, end) = spans[last];
            format!(
                "first in top-level statements {}..={} (bytes {start}..{end}): `{}`",
                index + 1,
                last + 1,
                excerpt(text, start as usize)
            )
        }
        None => "the file has no top-level statement there".to_string(),
    }
}

#[cfg(test)]
#[path = "admission_parse_tests.rs"]
mod tests;
