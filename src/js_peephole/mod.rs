use serde::Serialize;

use crate::js_peephole::binding::{BindingResolution, Resolution};
pub(crate) use crate::js_peephole::folds::fold_constructor_prototype_tables_to_classes;
use crate::js_peephole::folds::*;
use crate::js_peephole::parse::{
    compound_assignment_rewrite, parse_expression_regions, syntax_metrics,
};
pub(crate) use crate::js_peephole::rename::converge_local_names;
use crate::js_peephole::rewrite::{
    apply_rewrites, apply_token_rewrites, assign_is_in_declaration, is_property_identifier,
    non_overlapping_rewrites,
};
use crate::js_peephole::scope::{
    identifier_is_arrow_parameter, identifier_is_catch_parameter, identifier_is_function_parameter,
    GeneratedBindingIndex,
};
use crate::js_peephole::token::{
    ascii_identifier_name_string, lex, matching_closers, scan_template_expression,
    validate_conditional_operators, validate_delimiters, Token, TokenKind,
};
use crate::js_syntax_target::{EcmaScriptEdition, JsSyntaxFeature};

mod folds;
pub(crate) use folds::{
    fold_constant_json_parse, fold_dead_identifier_copy_declarators, fold_dead_increment_snapshots,
    fold_expression_bodies, fold_fresh_empty_object_assign, fold_if_prefixed_returns,
    fold_nested_unguarded_ifs, fold_pristine_static_method_calls, fold_redundant_null_undefined_or,
    inline_single_use_functions,
};
mod binding;
mod liveness;
mod parse;
mod rename;
mod rewrite;
mod scope;
mod token;

#[cfg(test)]
mod keyword_space_tests;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
pub struct JavaScriptSyntaxMetrics {
    pub bytes: usize,
    pub tokens: usize,
    pub ast_nodes: usize,
    pub max_nesting: usize,
    pub functions: usize,
    pub calls: usize,
    pub branches: usize,
    pub loops: usize,
    pub parse_cost: u64,
    pub compile_cost: u64,
    pub estimated_memory_bytes: u64,
}

impl JavaScriptSyntaxMetrics {
    pub fn startup_score(self, parse_weight: u32, compile_weight: u32, memory_weight: u32) -> u64 {
        self.parse_cost
            .saturating_mul(u64::from(parse_weight))
            .saturating_add(self.compile_cost.saturating_mul(u64::from(compile_weight)))
            .saturating_add(
                self.estimated_memory_bytes
                    .saturating_mul(u64::from(memory_weight)),
            )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeepholeResult {
    pub code: String,
    pub metrics: JavaScriptSyntaxMetrics,
    pub rewrites: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum GeneratedJavaScriptExportKind {
    Value,
    Function,
    Constructor,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratedJavaScriptExportWitness {
    pub name: String,
    pub kind: GeneratedJavaScriptExportKind,
    pub arity: Option<usize>,
    pub constructible: Option<bool>,
    pub fields: Vec<String>,
    pub methods: Vec<GeneratedJavaScriptMethodWitness>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GeneratedJavaScriptMethodWitness {
    pub name: String,
    pub arity: usize,
    pub is_async: bool,
    pub is_generator: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedJavaScriptPropertyOccurrence {
    pub name: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GeneratedJavaScriptBindingKind {
    Bound,
    Free,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedJavaScriptBindingOccurrence {
    pub name: String,
    pub start: usize,
    pub end: usize,
    pub kind: GeneratedJavaScriptBindingKind,
    pub declaration_start: Option<usize>,
    pub declaration_end: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JavaScriptParseError {
    offset: usize,
    message: &'static str,
    context: Option<String>,
}

impl JavaScriptParseError {
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// Quote the generated text around the fault.
    ///
    /// A byte offset into an artifact the user never sees is not a diagnosis.
    /// The validators run where the generated source is in hand, so the excerpt
    /// is attached there and travels with the error to the command line.
    fn with_source(mut self, source: &str) -> Self {
        if self.context.is_some() || self.offset > source.len() {
            return self;
        }
        let start = source[..self.offset]
            .char_indices()
            .rev()
            .nth(140)
            .map_or(0, |(index, _)| index);
        let end = source[self.offset..]
            .char_indices()
            .nth(90)
            .map_or(source.len(), |(index, _)| self.offset + index);
        self.context = Some(format!(
            "{}<<<HERE>>>{}",
            &source[start..self.offset],
            &source[self.offset..end]
        ));
        self
    }
}

impl std::fmt::Display for JavaScriptParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{} at generated byte {}",
            self.message, self.offset
        )?;
        if let Some(context) = &self.context {
            write!(formatter, "\n  generated: ...{context}...")?;
        }
        Ok(())
    }
}

impl std::error::Error for JavaScriptParseError {}

/// Returns a codec candidate that spells function-body-leading generated
/// declarations with `let` instead of `var`.
///
/// LilScript's IR emitter places a function's local declaration before every
/// executable body operation, assigns every emitted local name uniquely, and
/// cannot emit direct `eval` or `with`. Within that generated subset, the
/// leading function block and the function's `var` scope are equivalent when
/// the declaration has no initializer or only literal initializers. The
/// spellings can still interact quite differently with a surrounding gzip or
/// Brotli dictionary, so the compiler scores both complete programs.
///
/// This is intentionally a lexical, additive variant rather than a peephole
/// rewrite. Strings, templates, and comments are never inspected, parameter
/// lists must use the emitter's simple identifier form, declarations with a
/// hoisting-sensitive initializer or later `var` are rejected, and callers
/// always retain the original source as a candidate.
pub(crate) fn function_leading_declaration_variant(source: &str) -> Option<String> {
    let tokens = lex(source).ok()?;
    let matching_close = matching_closers(&tokens);
    let mut replacements = Vec::<(usize, usize)>::new();

    for (function_index, token) in tokens.iter().enumerate() {
        if token.text != "function" {
            continue;
        }

        let mut cursor = function_index + 1;
        if tokens.get(cursor).map(|token| token.text) == Some("*") {
            cursor += 1;
        }
        if tokens.get(cursor).map(|token| token.kind) == Some(TokenKind::Identifier) {
            cursor += 1;
        }
        if tokens.get(cursor).map(|token| token.text) != Some("(") {
            continue;
        }
        let parameters_open = cursor;
        let Some(parameters_close) = matching_close[parameters_open] else {
            continue;
        };
        let simple_parameters = tokens[parameters_open + 1..parameters_close]
            .iter()
            .enumerate()
            .all(|(index, token)| {
                if index % 2 == 0 {
                    token.kind == TokenKind::Identifier
                } else {
                    token.text == ","
                }
            });
        if !simple_parameters {
            continue;
        }

        let body_open = parameters_close + 1;
        if tokens.get(body_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close[body_open] else {
            continue;
        };
        let declaration_index = body_open + 1;
        if declaration_index >= body_close || tokens[declaration_index].text != "var" {
            continue;
        }

        let mut declaration_cursor = declaration_index + 1;
        let mut declared_names = Vec::<&str>::new();
        let mut declaration_end = None;
        while declaration_cursor < body_close {
            let Some(name) = tokens.get(declaration_cursor) else {
                break;
            };
            if name.kind != TokenKind::Identifier
                || declared_names.contains(&name.text)
                || tokens[parameters_open + 1..parameters_close]
                    .iter()
                    .any(|parameter| parameter.text == name.text)
            {
                break;
            }
            declared_names.push(name.text);
            declaration_cursor += 1;

            if tokens.get(declaration_cursor).map(|token| token.text) == Some("=") {
                declaration_cursor += 1;
                let initializer_start = declaration_cursor;
                while declaration_cursor < body_close
                    && !matches!(tokens[declaration_cursor].text, "," | ";")
                {
                    declaration_cursor += 1;
                }
                if !is_hoist_independent_literal_initializer(
                    &tokens[initializer_start..declaration_cursor],
                ) {
                    break;
                }
            }

            match tokens.get(declaration_cursor).map(|token| token.text) {
                Some(",") => declaration_cursor += 1,
                Some(";") => {
                    declaration_end = Some(declaration_cursor);
                    break;
                }
                _ => break,
            }
        }
        let Some(declaration_end) = declaration_end else {
            continue;
        };
        if tokens[declaration_end + 1..body_close]
            .iter()
            .any(|token| token.text == "var")
        {
            continue;
        }
        // A following function/class declaration can legally merge with a
        // `var` binding but conflicts with the lexical candidate. Generated
        // code normally has neither here; rejecting the whole uncommon shape
        // keeps the proof independent of declaration-instantiation rules.
        if tokens[declaration_end + 1..body_close]
            .iter()
            .any(|token| matches!(token.text, "function" | "class"))
        {
            continue;
        }

        let replacement = (
            tokens[declaration_index].start,
            tokens[declaration_index].end,
        );
        if !replacements.contains(&replacement) {
            replacements.push(replacement);
        }
    }

    if replacements.is_empty() {
        return None;
    }
    let mut variant = source.to_string();
    for (start, end) in replacements.into_iter().rev() {
        variant.replace_range(start..end, "let");
    }
    Some(variant)
}

fn is_hoist_independent_literal_initializer(tokens: &[Token<'_>]) -> bool {
    match tokens {
        [literal]
            if matches!(literal.kind, TokenKind::Number | TokenKind::String)
                || matches!(literal.text, "true" | "false" | "null") =>
        {
            true
        }
        [operator, literal]
            if matches!(operator.text, "+" | "-") && literal.kind == TokenKind::Number =>
        {
            true
        }
        _ => false,
    }
}

pub fn analyze_generated_javascript(
    source: &str,
) -> Result<JavaScriptSyntaxMetrics, JavaScriptParseError> {
    analyze_generated_javascript_inner(source).map_err(|error| {
        dump_rejected_generated_javascript(source, &error);
        error.with_source(source)
    })
}

pub fn generated_javascript_export_names(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut names = std::collections::BTreeSet::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].text != "export" {
            index += 1;
            continue;
        }
        let open = index + 1;
        if tokens.get(open).is_none_or(|token| token.text != "{") {
            return Err(JavaScriptParseError {
                offset: tokens[index].start,
                message: "unsupported generated export form",
                context: None,
            }
            .with_source(source));
        }
        let close = matching_close[open].ok_or_else(|| JavaScriptParseError {
            offset: tokens[open].start,
            message: "unclosed generated export clause",
            context: None,
        })?;
        let mut cursor = open + 1;
        while cursor < close {
            if tokens[cursor].text == "," {
                cursor += 1;
                continue;
            }
            let mut exported =
                generated_export_name(&tokens[cursor]).ok_or_else(|| JavaScriptParseError {
                    offset: tokens[cursor].start,
                    message: "invalid generated export name",
                    context: None,
                })?;
            cursor += 1;
            if cursor < close && tokens[cursor].text == "as" {
                cursor += 1;
                let token = tokens.get(cursor).ok_or_else(|| JavaScriptParseError {
                    offset: tokens[close].start,
                    message: "missing generated export alias",
                    context: None,
                })?;
                exported = generated_export_name(token).ok_or_else(|| JavaScriptParseError {
                    offset: token.start,
                    message: "invalid generated export alias",
                    context: None,
                })?;
                cursor += 1;
            }
            if !names.insert(exported.to_string()) {
                return Err(JavaScriptParseError {
                    offset: tokens[cursor.saturating_sub(1)].start,
                    message: "duplicate generated export name",
                    context: None,
                }
                .with_source(source));
            }
            if cursor < close && tokens[cursor].text != "," {
                return Err(JavaScriptParseError {
                    offset: tokens[cursor].start,
                    message: "invalid generated export clause",
                    context: None,
                }
                .with_source(source));
            }
        }
        index = close + 1;
    }
    Ok(names.into_iter().collect())
}

pub fn generated_javascript_export_witnesses(
    source: &str,
) -> Result<Vec<GeneratedJavaScriptExportWitness>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let resolution = BindingResolution::new(&tokens);
    let mut witnesses = Vec::new();
    for (local_index, name) in generated_export_pairs(&tokens, &matching_close, source)? {
        let Resolution::Bound(declaration) = resolution.resolve(local_index) else {
            return Err(generated_import_error(
                tokens[local_index].start,
                "unresolved generated export binding",
                source,
            ));
        };
        let (kind, arity, constructible, fields, methods) = classify_generated_export_binding(
            &tokens,
            &matching_close,
            &resolution,
            declaration,
            &mut std::collections::BTreeSet::new(),
        );
        witnesses.push(GeneratedJavaScriptExportWitness {
            name,
            kind,
            arity,
            constructible,
            fields,
            methods,
        });
    }
    witnesses.sort();
    Ok(witnesses)
}

fn generated_export_pairs(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    source: &str,
) -> Result<Vec<(usize, String)>, JavaScriptParseError> {
    let mut pairs = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "export" {
            continue;
        }
        let open = index + 1;
        if tokens.get(open).is_none_or(|token| token.text != "{") {
            return Err(generated_import_error(
                token.start,
                "unsupported generated export form",
                source,
            ));
        }
        let close = matching_close[open].ok_or_else(|| {
            generated_import_error(token.start, "unclosed generated export clause", source)
        })?;
        let mut cursor = open + 1;
        while cursor < close {
            if tokens[cursor].text == "," {
                cursor += 1;
                continue;
            }
            let local_index = cursor;
            let mut exported = generated_export_name(&tokens[cursor]).ok_or_else(|| {
                generated_import_error(
                    tokens[cursor].start,
                    "invalid generated export name",
                    source,
                )
            })?;
            cursor += 1;
            if cursor < close && tokens[cursor].text == "as" {
                cursor += 1;
                let alias = tokens.get(cursor).ok_or_else(|| {
                    generated_import_error(
                        tokens[close].start,
                        "missing generated export alias",
                        source,
                    )
                })?;
                exported = generated_export_name(alias).ok_or_else(|| {
                    generated_import_error(alias.start, "invalid generated export alias", source)
                })?;
                cursor += 1;
            }
            pairs.push((local_index, exported.to_string()));
        }
    }
    Ok(pairs)
}

fn classify_generated_export_binding(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    resolution: &BindingResolution<'_>,
    declaration: usize,
    visited: &mut std::collections::BTreeSet<usize>,
) -> (
    GeneratedJavaScriptExportKind,
    Option<usize>,
    Option<bool>,
    Vec<String>,
    Vec<GeneratedJavaScriptMethodWitness>,
) {
    if !visited.insert(declaration) {
        return (
            GeneratedJavaScriptExportKind::Value,
            None,
            None,
            Vec::new(),
            Vec::new(),
        );
    }
    let previous = declaration
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .map(|token| token.text);
    if previous == Some("function")
        || (previous == Some("*")
            && declaration
                .checked_sub(2)
                .and_then(|index| tokens.get(index))
                .is_some_and(|token| token.text == "function"))
    {
        let function = declaration - usize::from(previous == Some("*"));
        let open = declaration + 1;
        return (
            GeneratedJavaScriptExportKind::Function,
            generated_parameter_arity(tokens, matching_close, open),
            Some(!generated_function_is_async_or_generator(tokens, function)),
            Vec::new(),
            Vec::new(),
        );
    }
    if previous == Some("class") {
        let (arity, fields, methods) = generated_class_shape(
            tokens,
            matching_close,
            resolution,
            declaration,
            &mut std::collections::BTreeSet::new(),
        );
        return (
            GeneratedJavaScriptExportKind::Constructor,
            arity,
            Some(true),
            fields,
            methods,
        );
    }
    if tokens.get(declaration + 1).map(|token| token.text) != Some("=") {
        return (
            GeneratedJavaScriptExportKind::Value,
            None,
            None,
            Vec::new(),
            Vec::new(),
        );
    }
    let rhs = declaration + 2;
    if tokens.get(rhs + 1).map(|token| token.text) == Some("=>") {
        return (
            GeneratedJavaScriptExportKind::Function,
            Some(1),
            Some(false),
            Vec::new(),
            Vec::new(),
        );
    }
    if tokens.get(rhs).map(|token| token.text) == Some("(") {
        if let Some(close) = matching_close[rhs] {
            if tokens.get(close + 1).map(|token| token.text) == Some("=>") {
                return (
                    GeneratedJavaScriptExportKind::Function,
                    generated_parameter_arity(tokens, matching_close, rhs),
                    Some(false),
                    Vec::new(),
                    Vec::new(),
                );
            }
        }
    }
    let search_end = tokens.len().min(rhs + 12);
    for index in rhs..search_end {
        if tokens[index].text == "function" {
            let mut next = index + 1;
            if tokens.get(next).is_some_and(|token| token.text == "*") {
                next += 1;
            }
            let named = tokens
                .get(next)
                .is_some_and(|token| token.kind == TokenKind::Identifier);
            let open = next + usize::from(named);
            return (
                GeneratedJavaScriptExportKind::Function,
                generated_parameter_arity(tokens, matching_close, open),
                Some(!generated_function_is_async_or_generator(tokens, index)),
                Vec::new(),
                Vec::new(),
            );
        }
        if tokens[index].text == "class" {
            let class_name = index + 1;
            let (arity, fields, methods) = generated_class_shape(
                tokens,
                matching_close,
                resolution,
                class_name,
                &mut std::collections::BTreeSet::new(),
            );
            return (
                GeneratedJavaScriptExportKind::Constructor,
                arity,
                Some(true),
                fields,
                methods,
            );
        }
        if matches!(tokens[index].text, ";" | "export") {
            break;
        }
    }
    if tokens
        .get(rhs)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
    {
        if let Resolution::Bound(target) = resolution.resolve(rhs) {
            return classify_generated_export_binding(
                tokens,
                matching_close,
                resolution,
                target,
                visited,
            );
        }
    }
    (
        GeneratedJavaScriptExportKind::Value,
        None,
        None,
        Vec::new(),
        Vec::new(),
    )
}

fn generated_function_is_async_or_generator(tokens: &[Token<'_>], function: usize) -> bool {
    tokens
        .get(function + 1)
        .is_some_and(|token| token.text == "*")
        || function
            .checked_sub(1)
            .and_then(|index| tokens.get(index))
            .is_some_and(|token| token.text == "async")
}

fn generated_parameter_arity(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    open: usize,
) -> Option<usize> {
    if tokens.get(open).map(|token| token.text) != Some("(") {
        return None;
    }
    let close = matching_close[open]?;
    let mut arity = 0usize;
    let mut segment = false;
    let mut depth = 0i32;
    for token in &tokens[open + 1..close] {
        if depth == 0 {
            if matches!(token.text, "=" | "...") {
                return Some(arity);
            }
            if token.text == "," {
                arity += usize::from(segment);
                segment = false;
                continue;
            }
            segment = true;
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth -= 1,
            _ => {}
        }
    }
    Some(arity + usize::from(segment))
}

fn generated_class_shape(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    resolution: &BindingResolution<'_>,
    class_name: usize,
    visited: &mut std::collections::BTreeSet<usize>,
) -> (
    Option<usize>,
    Vec<String>,
    Vec<GeneratedJavaScriptMethodWitness>,
) {
    if !visited.insert(class_name) {
        return (None, Vec::new(), Vec::new());
    }
    let Some(open) = (class_name + 1..tokens.len()).find(|index| tokens[*index].text == "{") else {
        return (None, Vec::new(), Vec::new());
    };
    let Some(close) = matching_close[open] else {
        return (None, Vec::new(), Vec::new());
    };
    let mut arity = Some(0);
    let mut fields = Vec::new();
    let mut methods = Vec::new();
    let mut cursor = open + 1;
    while cursor < close {
        if matches!(tokens[cursor].text, ";" | ",") {
            cursor += 1;
            continue;
        }
        let mut is_async = false;
        let mut is_generator = false;
        if tokens[cursor].text == "static" {
            cursor += 1;
        }
        if cursor < close && tokens[cursor].text == "async" {
            is_async = true;
            cursor += 1;
        }
        if cursor < close && tokens[cursor].text == "*" {
            is_generator = true;
            cursor += 1;
        }
        let Some(name) = tokens.get(cursor) else {
            break;
        };
        let params = cursor + 1;
        if tokens.get(params).map(|token| token.text) != Some("(") {
            if tokens
                .get(params)
                .is_some_and(|token| matches!(token.text, "=" | ";"))
            {
                if let Some(name) = generated_export_name(name) {
                    fields.push(name.to_string());
                }
            }
            cursor += 1;
            continue;
        }
        let method_arity = generated_parameter_arity(tokens, matching_close, params).unwrap_or(0);
        if name.text == "constructor" {
            arity = Some(method_arity);
        } else if let Some(name) = generated_export_name(name) {
            methods.push(GeneratedJavaScriptMethodWitness {
                name: name.to_string(),
                arity: method_arity,
                is_async,
                is_generator,
            });
        }
        let Some(params_close) = matching_close[params] else {
            break;
        };
        let body = params_close + 1;
        cursor = if tokens.get(body).map(|token| token.text) == Some("{") {
            matching_close[body].map_or(body + 1, |body_close| body_close + 1)
        } else {
            body
        };
    }
    if tokens.get(class_name + 1).map(|token| token.text) == Some("extends") {
        let base = class_name + 2;
        if let Resolution::Bound(base_declaration) = resolution.resolve(base) {
            let (_, base_fields, base_methods) = generated_class_shape(
                tokens,
                matching_close,
                resolution,
                base_declaration,
                visited,
            );
            for field in base_fields.into_iter().rev() {
                if !fields.contains(&field) {
                    fields.insert(0, field);
                }
            }
            for method in base_methods {
                if !methods.iter().any(|existing| {
                    let existing: &GeneratedJavaScriptMethodWitness = existing;
                    existing.name == method.name
                }) {
                    methods.push(method);
                }
            }
        }
    }
    fields.dedup();
    methods.sort();
    (arity, fields, methods)
}

pub fn generated_javascript_static_imports(
    source: &str,
) -> Result<Vec<(String, Vec<String>)>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut imports = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "import" {
            continue;
        }
        let Some(next) = tokens.get(index + 1) else {
            return Err(generated_import_error(
                token.start,
                "incomplete generated import",
                source,
            ));
        };
        if next.text == "(" {
            continue;
        }
        if next.kind == TokenKind::String {
            let source_name = unescape_js_string(next.text).ok_or_else(|| {
                generated_import_error(next.start, "invalid generated import source", source)
            })?;
            imports.push((source_name, Vec::new()));
            continue;
        }
        if next.text != "{" {
            return Err(generated_import_error(
                next.start,
                "unsupported generated import form",
                source,
            ));
        }
        let close = matching_close[index + 1].ok_or_else(|| {
            generated_import_error(next.start, "unclosed generated import clause", source)
        })?;
        let mut names = Vec::new();
        let mut cursor = index + 2;
        while cursor < close {
            if tokens[cursor].text == "," {
                cursor += 1;
                continue;
            }
            let imported = generated_export_name(&tokens[cursor]).ok_or_else(|| {
                generated_import_error(
                    tokens[cursor].start,
                    "invalid generated import name",
                    source,
                )
            })?;
            names.push(imported.to_string());
            cursor += 1;
            if cursor < close && tokens[cursor].text == "as" {
                cursor += 1;
                let alias = tokens.get(cursor).ok_or_else(|| {
                    generated_import_error(
                        tokens[close].start,
                        "missing generated import alias",
                        source,
                    )
                })?;
                if generated_export_name(alias).is_none() {
                    return Err(generated_import_error(
                        alias.start,
                        "invalid generated import alias",
                        source,
                    ));
                }
                cursor += 1;
            }
            if cursor < close && tokens[cursor].text != "," {
                return Err(generated_import_error(
                    tokens[cursor].start,
                    "invalid generated import clause",
                    source,
                ));
            }
        }
        let from = tokens.get(close + 1);
        let source_token = tokens.get(close + 2);
        if from.is_none_or(|token| token.text != "from")
            || source_token.is_none_or(|token| token.kind != TokenKind::String)
        {
            return Err(generated_import_error(
                tokens[close].end,
                "missing generated import source",
                source,
            ));
        }
        let source_token = source_token.unwrap();
        let source_name = unescape_js_string(source_token.text).ok_or_else(|| {
            generated_import_error(
                source_token.start,
                "invalid generated import source",
                source,
            )
        })?;
        names.sort();
        names.dedup();
        imports.push((source_name, names));
    }
    imports.sort();
    Ok(imports)
}

pub fn generated_javascript_bit_or_zero_count(source: &str) -> Result<usize, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    Ok(tokens
        .windows(2)
        .filter(|pair| {
            pair[0].text == "|" && pair[1].kind == TokenKind::Number && pair[1].text == "0"
        })
        .count())
}

pub fn generated_javascript_static_property_names(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    let mut names = generated_javascript_static_property_occurrences(source)?
        .into_iter()
        .map(|property| property.name)
        .collect::<Vec<_>>();
    names.sort();
    names.dedup();
    Ok(names)
}

pub fn generated_javascript_static_property_occurrences(
    source: &str,
) -> Result<Vec<GeneratedJavaScriptPropertyOccurrence>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let class_names = class_element_name_occurrences(&tokens, &matching_close);
    let mut properties = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if matches!(token.kind, TokenKind::Identifier | TokenKind::Keyword)
            && (is_property_identifier(&tokens, index)
                || class_names.get(index).copied().unwrap_or(false))
        {
            properties.push(GeneratedJavaScriptPropertyOccurrence {
                name: token.text.to_string(),
                start: token.start,
                end: token.end,
            });
            continue;
        }
        if token.kind != TokenKind::String {
            continue;
        }
        let previous = index.checked_sub(1).and_then(|index| tokens.get(index));
        let next = tokens.get(index + 1);
        let static_property = previous.is_some_and(|token| token.text == "[")
            && next.is_some_and(|token| token.text == "]")
            || next.is_some_and(|token| token.text == ":")
                && previous.is_some_and(|token| matches!(token.text, "{" | ","));
        if static_property {
            if let Some(name) = unescape_js_string(token.text) {
                properties.push(GeneratedJavaScriptPropertyOccurrence {
                    name,
                    start: token.start,
                    end: token.end,
                });
            }
        }
    }
    Ok(properties)
}

pub fn generated_javascript_dynamic_property_occurrences(
    source: &str,
) -> Result<Vec<(usize, usize)>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let mut occurrences = Vec::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.text != "[" || index == 0 {
            continue;
        }
        let previous = &tokens[index - 1];
        let optional = previous.text == "."
            && index > 1
            && tokens[index - 2].text == "?"
            && tokens[index - 2].end == previous.start;
        let receiver = matches!(
            previous.kind,
            TokenKind::Identifier
                | TokenKind::Number
                | TokenKind::String
                | TokenKind::Template
                | TokenKind::Regex
        ) || matches!(previous.text, ")" | "]")
            || optional;
        if !receiver {
            continue;
        }
        let Some(close) = matching_close[index] else {
            continue;
        };
        if close == index + 2 && tokens[index + 1].kind == TokenKind::String {
            continue;
        }
        occurrences.push((token.start, tokens[close].end));
    }
    Ok(occurrences)
}

pub fn generated_javascript_free_identifiers(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let resolution = BindingResolution::new(&tokens);
    let mut names = std::collections::BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        if token.kind == TokenKind::Identifier
            && identifier_occurrence_is_clear_binding(&tokens, index)
            && matches!(resolution.resolve(index), Resolution::Free)
        {
            names.insert(token.text.to_string());
        }
    }
    Ok(names.into_iter().collect())
}

pub fn generated_javascript_binding_occurrences(
    source: &str,
) -> Result<Vec<GeneratedJavaScriptBindingOccurrence>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let class_names = class_element_name_occurrences(&tokens, &matching_close);
    let resolution = BindingResolution::new(&tokens);
    Ok(tokens
        .iter()
        .enumerate()
        .filter(|(index, token)| {
            token.kind == TokenKind::Identifier
                && !is_property_identifier(&tokens, *index)
                && !class_names.get(*index).copied().unwrap_or(false)
        })
        .map(|(index, token)| {
            let (kind, declaration_start, declaration_end) = match resolution.resolve(index) {
                Resolution::Bound(declaration) => (
                    GeneratedJavaScriptBindingKind::Bound,
                    Some(tokens[declaration].start),
                    Some(tokens[declaration].end),
                ),
                Resolution::Free => (GeneratedJavaScriptBindingKind::Free, None, None),
                Resolution::Unresolved => (GeneratedJavaScriptBindingKind::Unresolved, None, None),
            };
            GeneratedJavaScriptBindingOccurrence {
                name: token.text.to_string(),
                start: token.start,
                end: token.end,
                kind,
                declaration_start,
                declaration_end,
            }
        })
        .collect())
}

pub fn generated_javascript_template_literals(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let mut templates = lex(source)?
        .into_iter()
        .filter(|token| token.kind == TokenKind::Template)
        .map(|token| token.text.to_string())
        .collect::<Vec<_>>();
    templates.sort();
    Ok(templates)
}

pub fn validate_generated_javascript_syntax_floor(
    source: &str,
    edition: EcmaScriptEdition,
) -> Result<(), JavaScriptParseError> {
    analyze_generated_javascript(source)?;
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let class_names = class_element_name_occurrences(&tokens, &matching_close);
    for (index, token) in tokens.iter().enumerate() {
        let optional_chain = token.text == "?"
            && tokens
                .get(index + 1)
                .is_some_and(|next| next.text == "." && next.start == token.end);
        let feature = if optional_chain {
            Some(JsSyntaxFeature::OptionalChain)
        } else if generated_object_rest_or_spread(&tokens, index) {
            Some(JsSyntaxFeature::ObjectRestSpread)
        } else {
            match token.text {
                "??" => Some(JsSyntaxFeature::NullishCoalescing),
                "&&=" | "||=" | "??=" => Some(JsSyntaxFeature::LogicalAssignment),
                "await" if !is_property_identifier(&tokens, index) => {
                    Some(JsSyntaxFeature::AsyncAwait)
                }
                "async" if generated_async_function_or_arrow(&tokens, &matching_close, index) => {
                    Some(JsSyntaxFeature::AsyncAwait)
                }
                "catch" if tokens.get(index + 1).is_some_and(|next| next.text == "{") => {
                    Some(JsSyntaxFeature::OptionalCatchBinding)
                }
                "values"
                    if index > 1
                        && tokens[index - 1].text == "."
                        && tokens[index - 2].text == "Object" =>
                {
                    Some(JsSyntaxFeature::ObjectValues)
                }
                "hasOwn"
                    if index > 1
                        && tokens[index - 1].text == "."
                        && tokens[index - 2].text == "Object" =>
                {
                    Some(JsSyntaxFeature::ObjectHasOwn)
                }
                _ if class_names.get(index).copied().unwrap_or(false)
                    && tokens
                        .get(index + 1)
                        .is_some_and(|next| matches!(next.text, "=" | ";")) =>
                {
                    Some(JsSyntaxFeature::ClassFields)
                }
                _ => None,
            }
        };
        if feature.is_some_and(|feature| !edition.allows(feature)) {
            return Err(JavaScriptParseError {
                offset: token.start,
                message: "generated JavaScript exceeds configured syntax floor",
                context: None,
            }
            .with_source(source));
        }
    }
    Ok(())
}

fn generated_async_function_or_arrow(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    index: usize,
) -> bool {
    let next = index + 1;
    if tokens
        .get(next)
        .is_some_and(|token| token.text == "function")
        || (tokens
            .get(next)
            .is_some_and(|token| token.kind == TokenKind::Identifier)
            && tokens.get(next + 1).is_some_and(|token| token.text == "=>"))
    {
        return true;
    }
    tokens.get(next).is_some_and(|token| token.text == "(")
        && matching_close[next]
            .and_then(|close| tokens.get(close + 1))
            .is_some_and(|token| token.text == "=>")
}

fn generated_object_rest_or_spread(tokens: &[Token<'_>], index: usize) -> bool {
    if tokens.get(index).is_none_or(|token| token.text != ".")
        || tokens.get(index + 1).is_none_or(|token| token.text != ".")
        || tokens.get(index + 2).is_none_or(|token| token.text != ".")
    {
        return false;
    }
    let mut depth = 0i32;
    for token in tokens[..index].iter().rev() {
        match token.text {
            ")" | "]" | "}" => depth += 1,
            "(" | "[" | "{" if depth > 0 => depth -= 1,
            "{" if depth == 0 => return true,
            "(" | "[" if depth == 0 => return false,
            _ => {}
        }
    }
    false
}

fn generated_import_error(
    offset: usize,
    message: &'static str,
    source: &str,
) -> JavaScriptParseError {
    JavaScriptParseError {
        offset,
        message,
        context: None,
    }
    .with_source(source)
}

fn generated_export_name<'src>(token: &Token<'src>) -> Option<&'src str> {
    match token.kind {
        TokenKind::Identifier | TokenKind::Keyword => Some(token.text),
        TokenKind::String => ascii_identifier_name_string(token.text),
        _ => None,
    }
}

/// Write a rejected candidate out for inspection.
///
/// A validator that fails closed keeps invalid JavaScript from shipping, but it
/// also destroys the only copy of the program that shows why. Setting
/// `LILSCRIPT_DUMP_REJECTED` to a directory keeps each rejected candidate.
fn dump_rejected_generated_javascript(source: &str, error: &JavaScriptParseError) {
    let Some(directory) = std::env::var_os("LILSCRIPT_DUMP_REJECTED") else {
        return;
    };
    let path = std::path::Path::new(&directory).join(format!(
        "rejected-{}-{}.js",
        error.message.replace(' ', "-"),
        error.offset
    ));
    let _ = std::fs::write(path, source);
}

fn analyze_generated_javascript_inner(
    source: &str,
) -> Result<JavaScriptSyntaxMetrics, JavaScriptParseError> {
    let tokens = lex(source)?;
    let delimiter_nesting = validate_delimiters(&tokens)?;
    validate_generated_declaration_syntax(source, &tokens)?;
    validate_generated_bare_arrow_parameters(&tokens)?;
    validate_conditional_operators(&tokens)?;
    validate_unique_top_level_bindings(&tokens)?;
    validate_class_body_members(&tokens)?;
    validate_for_heads(&tokens)?;
    validate_resolved_generated_bindings(&tokens)?;
    let parsed = parse_expression_regions(&tokens);
    Ok(syntax_metrics(source, &tokens, &parsed, delimiter_nesting))
}

/// The tokenizer marks contextual words such as `of` and `as` as keywords even
/// though they are legal generated binding names. Reject only words that can
/// never occupy a binding position.
fn is_generated_binding_name(token: &Token<'_>) -> bool {
    match token.kind {
        TokenKind::Identifier => true,
        TokenKind::Keyword => !matches!(
            token.text,
            "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "debugger"
                | "default"
                | "delete"
                | "do"
                | "else"
                | "export"
                | "extends"
                | "false"
                | "finally"
                | "for"
                | "function"
                | "if"
                | "import"
                | "in"
                | "instanceof"
                | "new"
                | "null"
                | "return"
                | "super"
                | "switch"
                | "this"
                | "throw"
                | "true"
                | "try"
                | "typeof"
                | "var"
                | "void"
                | "while"
                | "with"
        ),
        _ => false,
    }
}

/// Reject malformed declarator lists that the delimiter-only parser would
/// otherwise accept. Generated bindings are identifiers or destructuring
/// patterns; after one of those, only an initializer or a declaration
/// separator may follow on the same line. In particular, a logical expression
/// cannot become a declarator merely because a rewrite moved a root comma.
fn validate_generated_declaration_syntax(
    source: &str,
    tokens: &[Token<'_>],
) -> Result<(), JavaScriptParseError> {
    let matching_close = matching_closers(tokens);
    for declaration in 0..tokens.len() {
        if !matches!(tokens[declaration].text, "var" | "let" | "const")
            || declaration.checked_sub(1).is_some_and(|previous| {
                matches!(tokens[previous].text, "." | "?.")
            })
            // Keyword-named object/class members are not declarations.
            || matches!(tokens.get(declaration + 1).map(|token| token.text), Some(":" | "("))
        {
            continue;
        }

        let mut cursor = declaration + 1;
        'declarators: loop {
            let Some(binding) = tokens.get(cursor) else {
                return Err(JavaScriptParseError {
                    offset: tokens[declaration].end,
                    message: "missing generated variable declarator",
                    context: None,
                });
            };
            let binding_end = if is_generated_binding_name(binding) {
                cursor + 1
            } else if matches!(binding.text, "{" | "[") {
                matching_close
                    .get(cursor)
                    .copied()
                    .flatten()
                    .map(|close| close + 1)
                    .ok_or(JavaScriptParseError {
                        offset: binding.start,
                        message: "unclosed generated binding pattern",
                        context: None,
                    })?
            } else {
                return Err(JavaScriptParseError {
                    offset: binding.start,
                    message: "invalid generated variable declarator",
                    context: None,
                });
            };
            cursor = binding_end;

            match tokens.get(cursor).map(|token| token.text) {
                Some("=") => {
                    cursor += 1;
                    if cursor >= tokens.len() {
                        return Err(JavaScriptParseError {
                            offset: tokens[cursor - 1].end,
                            message: "missing generated variable initializer",
                            context: None,
                        });
                    }
                    while cursor < tokens.len() {
                        match tokens[cursor].text {
                            "(" | "[" | "{" => {
                                cursor = matching_close
                                    .get(cursor)
                                    .copied()
                                    .flatten()
                                    .map(|close| close + 1)
                                    .ok_or(JavaScriptParseError {
                                        offset: tokens[cursor].start,
                                        message: "unclosed generated variable initializer",
                                        context: None,
                                    })?;
                            }
                            "," => {
                                cursor += 1;
                                continue 'declarators;
                            }
                            ";" | ")" | "}" => break 'declarators,
                            _ => cursor += 1,
                        }
                    }
                    break;
                }
                Some(",") => {
                    cursor += 1;
                    continue;
                }
                Some(";") | Some(")") | Some("}") | Some("in") | Some("of") | None => break,
                Some(_) => {
                    let next = &tokens[cursor];
                    if source[tokens[binding_end - 1].end..next.start]
                        .bytes()
                        .any(|byte| matches!(byte, b'\n' | b'\r'))
                    {
                        break;
                    }
                    return Err(JavaScriptParseError {
                        offset: next.start,
                        message: "invalid generated variable declarator suffix",
                        context: None,
                    });
                }
            }
        }
    }
    Ok(())
}

/// A bare arrow parameter must be a binding identifier. The tokenizer marks
/// contextual words such as `async` as keywords too, so reject only words
/// that can never name a generated binding rather than requiring the broader
/// `Identifier` token class. This catches value propagation into the binding
/// position (for example, `a=>` becoming `null=>`) independently of the fold
/// that produced it.
fn validate_generated_bare_arrow_parameters(
    tokens: &[Token<'_>],
) -> Result<(), JavaScriptParseError> {
    for arrow in 1..tokens.len() {
        if tokens[arrow].text != "=>" || tokens[arrow - 1].text == ")" {
            continue;
        }
        let parameter = &tokens[arrow - 1];
        if !is_generated_binding_name(parameter) {
            return Err(JavaScriptParseError {
                offset: parameter.start,
                message: "invalid generated bare arrow parameter",
                context: None,
            });
        }
    }
    Ok(())
}

/// Reject duplicate bindings in the generated module scope.
///
/// The emitter intentionally gives every module-scope function, global, and
/// entry local a unique spelling. Treating that invariant as a candidate
/// admission check is useful because the aggressive cross-scope mangler and
/// post-emission spelling search are optional proposals: a broken proposal
/// must lose to the conservative pinned candidate before JavaScript reaches a
/// runtime. This is deliberately narrower than a general JavaScript parser;
/// it recognizes the simple declarations emitted by LilScript and leaves
/// nested lexical scopes alone.
fn validate_unique_top_level_bindings(tokens: &[Token<'_>]) -> Result<(), JavaScriptParseError> {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ModuleBindingKind {
        Var,
        Lexical,
    }

    fn insert<'src>(
        declared: &mut std::collections::BTreeMap<&'src str, (usize, ModuleBindingKind)>,
        token: Token<'src>,
        kind: ModuleBindingKind,
    ) -> Result<(), JavaScriptParseError> {
        match declared.entry(token.text) {
            std::collections::btree_map::Entry::Occupied(existing) => {
                // `var` may redeclare `var` in the same scope. Lexical names
                // (`let`/`const`/`class`/`function`) and mixed var/lexical
                // pairs are SyntaxErrors in modules, so those stay fatal.
                if existing.get().1 == ModuleBindingKind::Var && kind == ModuleBindingKind::Var {
                    return Ok(());
                }
                Err(JavaScriptParseError {
                    offset: token.start,
                    message: "duplicate generated top-level binding",
                    context: Some(token.text.to_string()),
                })
            }
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert((token.start, kind));
                Ok(())
            }
        }
    }

    let mut declared = std::collections::BTreeMap::<&str, (usize, ModuleBindingKind)>::new();
    let mut brace_depth = 0usize;
    let mut index = 0usize;
    while index < tokens.len() {
        let token = tokens[index];
        if brace_depth == 0 {
            match token.text {
                "let" | "const" | "var" => {
                    let kind = if token.text == "var" {
                        ModuleBindingKind::Var
                    } else {
                        ModuleBindingKind::Lexical
                    };
                    let mut scan = index + 1;
                    let mut paren_depth = 0usize;
                    let mut bracket_depth = 0usize;
                    let mut initializer_brace_depth = 0usize;
                    let mut expects_name = true;
                    while scan < tokens.len() {
                        let current = tokens[scan];
                        let at_declarator_level =
                            paren_depth == 0 && bracket_depth == 0 && initializer_brace_depth == 0;
                        if at_declarator_level && matches!(current.text, ";" | ")") {
                            break;
                        }
                        if at_declarator_level
                            && !expects_name
                            && matches!(current.text, "of" | "in")
                        {
                            break;
                        }
                        if at_declarator_level
                            && expects_name
                            && is_generated_binding_name(&current)
                        {
                            insert(&mut declared, current, kind)?;
                            expects_name = false;
                        }
                        match current.text {
                            "(" => paren_depth += 1,
                            ")" => paren_depth = paren_depth.saturating_sub(1),
                            "[" => bracket_depth += 1,
                            "]" => bracket_depth = bracket_depth.saturating_sub(1),
                            "{" => initializer_brace_depth += 1,
                            "}" => {
                                initializer_brace_depth = initializer_brace_depth.saturating_sub(1)
                            }
                            "," if at_declarator_level => expects_name = true,
                            "=" if at_declarator_level => expects_name = false,
                            _ => {}
                        }
                        scan += 1;
                    }
                }
                "function" => {
                    if generated_class_or_function_is_declaration(tokens, index) {
                        let mut name = index + 1;
                        if tokens.get(name).map(|token| token.text) == Some("*") {
                            name += 1;
                        }
                        if let Some(name) = tokens
                            .get(name)
                            .copied()
                            .filter(|name| name.kind == TokenKind::Identifier)
                        {
                            insert(&mut declared, name, ModuleBindingKind::Lexical)?;
                        }
                    }
                }
                "class" => {
                    if generated_class_or_function_is_declaration(tokens, index) {
                        if let Some(name) = tokens
                            .get(index + 1)
                            .copied()
                            .filter(|name| name.kind == TokenKind::Identifier)
                        {
                            insert(&mut declared, name, ModuleBindingKind::Lexical)?;
                        }
                    }
                }
                _ => {}
            }
        }
        match token.text {
            "{" => brace_depth += 1,
            "}" => brace_depth = brace_depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }
    Ok(())
}

fn generated_class_or_function_is_declaration(tokens: &[Token<'_>], index: usize) -> bool {
    !matches!(
        index
            .checked_sub(1)
            .and_then(|previous| tokens.get(previous).map(|token| token.text)),
        Some(
            "=" | "("
                | ","
                | ":"
                | "["
                | "!"
                | "?"
                | "&&"
                | "||"
                | "return"
                | "void"
                | "typeof"
                | "new"
                | "yield"
                | "await"
        )
    )
}

fn class_body_spans(tokens: &[Token<'_>], matching_close: &[Option<usize>]) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut index = 0usize;
    while index < tokens.len() {
        if tokens[index].text == "class" {
            let mut body = index + 1;
            if tokens
                .get(body)
                .is_some_and(|token| token.kind == TokenKind::Identifier)
            {
                body += 1;
            }
            if tokens.get(body).map(|token| token.text) == Some("extends") {
                body += 2;
            }
            if tokens.get(body).map(|token| token.text) == Some("{") {
                if let Some(close) = matching_close.get(body).copied().flatten() {
                    spans.push((body, close));
                    index = close + 1;
                    continue;
                }
            }
        }
        index += 1;
    }
    spans
}

/// Search may propose a class member that starts with a compact boolean
/// (`}!1{...}`). That is not a method, field, or static block, so the
/// candidate must lose instead of reaching a runtime parse error.
fn class_body_has_dotted_method(tokens: &[Token<'_>], index: usize) -> bool {
    tokens
        .get(index)
        .is_some_and(|token| token.kind == TokenKind::Identifier)
        && tokens.get(index + 1).map(|token| token.text) == Some(".")
        && tokens.get(index + 2).is_some_and(|token| {
            token.kind == TokenKind::Identifier || token.kind == TokenKind::Keyword
        })
        && tokens.get(index + 3).map(|token| token.text) == Some("(")
}

fn class_body_token_is_field_initializer(tokens: &[Token<'_>], open: usize, index: usize) -> bool {
    tokens[open + 1..index]
        .iter()
        .rev()
        .take_while(|token| !matches!(token.text, ";" | "{" | "}"))
        .any(|token| token.text == "=")
}

fn validate_class_body_members(tokens: &[Token<'_>]) -> Result<(), JavaScriptParseError> {
    let matching_close = matching_closers(tokens);
    for (open, close) in class_body_spans(tokens, &matching_close) {
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut brace = 0i32;
        for index in open + 1..close {
            if paren == 0
                && bracket == 0
                && brace == 0
                && ((tokens[index].text == "!"
                    && !class_body_token_is_field_initializer(tokens, open, index))
                    || (tokens[index].text == ","
                        && class_body_token_is_field_initializer(tokens, open, index))
                    || matches!(tokens[index].text, "var" | "let" | "const")
                    || class_body_has_dotted_method(tokens, index))
            {
                return Err(JavaScriptParseError {
                    offset: tokens[index].start,
                    message: "invalid generated class element",
                    context: None,
                });
            }
            match tokens[index].text {
                "(" => paren += 1,
                ")" => paren -= 1,
                "[" => bracket += 1,
                "]" => bracket -= 1,
                "{" => brace += 1,
                "}" => brace -= 1,
                _ => {}
            }
        }
    }
    Ok(())
}

fn validate_for_heads(tokens: &[Token<'_>]) -> Result<(), JavaScriptParseError> {
    let matching_close = matching_closers(tokens);
    for index in 0..tokens.len() {
        if tokens[index].text != "for" || tokens.get(index + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let open = index + 1;
        let Some(close) = matching_close.get(open).copied().flatten() else {
            continue;
        };
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut brace = 0i32;
        let mut semis = 0u8;
        for token in tokens.iter().take(close).skip(open + 1) {
            if paren == 0 && bracket == 0 && brace == 0 {
                if matches!(token.text, "return" | "throw" | "break" | "continue") {
                    return Err(JavaScriptParseError {
                        offset: token.start,
                        message: "statement keyword in generated for head",
                        context: None,
                    });
                } else if token.text == ";" {
                    semis = semis.saturating_add(1);
                } else if semis >= 2 && matches!(token.text, "var" | "let" | "const" | "function") {
                    return Err(JavaScriptParseError {
                        offset: token.start,
                        message: "invalid generated for-update clause",
                        context: None,
                    });
                }
            }
            match token.text {
                "(" => paren += 1,
                ")" => paren -= 1,
                "[" => bracket += 1,
                "]" => bracket -= 1,
                "{" => brace += 1,
                "}" => brace -= 1,
                _ => {}
            }
        }
    }
    Ok(())
}

fn generated_identifier_is_binding(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
    index: usize,
) -> bool {
    if identifier_is_arrow_parameter(tokens, index)
        || identifier_is_function_parameter(tokens, matching_close, index)
        || identifier_is_catch_parameter(tokens, matching_close, index)
    {
        return true;
    }
    let previous = index
        .checked_sub(1)
        .map(|prev| tokens[prev].text)
        .unwrap_or(";");
    if matches!(
        previous,
        "var" | "let" | "const" | "function" | "class" | "catch"
    ) {
        return true;
    }
    if previous == "*"
        && index
            .checked_sub(2)
            .is_some_and(|prev| tokens[prev].text == "function")
    {
        return true;
    }
    previous == "," && assign_is_in_declaration(tokens, index)
}

fn generated_identifier_is_ambient(name: &str) -> bool {
    matches!(
        name,
        "undefined"
            | "NaN"
            | "Infinity"
            | "Math"
            | "Object"
            | "Array"
            | "String"
            | "Number"
            | "Boolean"
            | "Date"
            | "RegExp"
            | "Error"
            | "TypeError"
            | "RangeError"
            | "SyntaxError"
            | "Map"
            | "Set"
            | "WeakMap"
            | "WeakSet"
            | "JSON"
            | "console"
            | "document"
            | "window"
            | "globalThis"
            | "global"
            | "self"
            | "Promise"
            | "Symbol"
            | "Reflect"
            | "Proxy"
            | "Intl"
            | "parseInt"
            | "parseFloat"
            | "isNaN"
            | "isFinite"
            | "encodeURIComponent"
            | "decodeURIComponent"
            | "encodeURI"
            | "decodeURI"
            | "eval"
            | "Function"
            | "Uint8Array"
            | "Uint16Array"
            | "Uint32Array"
            | "Int8Array"
            | "Int16Array"
            | "Int32Array"
            | "Float32Array"
            | "Float64Array"
            | "ArrayBuffer"
            | "DataView"
            | "BigInt"
            | "setTimeout"
            | "clearTimeout"
            | "setInterval"
            | "clearInterval"
            | "crypto"
            | "performance"
            | "fetch"
            | "URL"
            | "Buffer"
            | "process"
            | "arguments"
    )
}

/// A candidate may not win by emitting a name that is bound in one function
/// and read from another. Host and language globals stay legal; a local that
/// leaked across a function boundary is the ident-05 failure mode.
fn class_element_name_occurrences(
    tokens: &[Token<'_>],
    matching_close: &[Option<usize>],
) -> Vec<bool> {
    let mut names = vec![false; tokens.len()];
    for (open, close) in class_body_spans(tokens, matching_close) {
        let mut paren = 0i32;
        let mut bracket = 0i32;
        let mut brace = 0i32;
        let mut at_element_start = true;
        for index in open + 1..close {
            if paren == 0 && bracket == 0 && brace == 0 {
                if tokens[index].text == ";" {
                    at_element_start = true;
                    continue;
                }
                if at_element_start {
                    if matches!(tokens[index].text, "static" | "async" | "get" | "set" | "*") {
                        continue;
                    }
                    if tokens[index].kind == TokenKind::Identifier {
                        names[index] = true;
                    }
                    at_element_start = false;
                }
            }
            match tokens[index].text {
                "(" => paren += 1,
                ")" => paren -= 1,
                "[" => bracket += 1,
                "]" => bracket -= 1,
                "{" => brace += 1,
                "}" => {
                    brace -= 1;
                    if paren == 0 && bracket == 0 && brace == 0 {
                        at_element_start = true;
                    }
                }
                _ => {}
            }
        }
    }
    names
}

fn validate_resolved_generated_bindings(tokens: &[Token<'_>]) -> Result<(), JavaScriptParseError> {
    let matching_close = matching_closers(tokens);
    let bindings = GeneratedBindingIndex::new(tokens, &matching_close);
    let class_element_names = class_element_name_occurrences(tokens, &matching_close);
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Identifier
            || is_property_identifier(tokens, index)
            || generated_identifier_is_module_syntax(tokens, index)
            || class_element_names.get(index).copied().unwrap_or(false)
            || bindings.identifier_is_binding(index)
            || generated_identifier_is_ambient(token.text)
            || bindings.name_is_visible(index, token.text)
        {
            continue;
        }
        if bindings.name_is_bound_as_non_enclosing_function_local(index, token.text) {
            return Err(JavaScriptParseError {
                offset: token.start,
                message: "unresolved generated identifier",
                context: None,
            });
        }
    }
    Ok(())
}

fn generated_identifier_is_module_syntax(tokens: &[Token<'_>], index: usize) -> bool {
    let mut cursor = index;
    while cursor > 0 {
        cursor -= 1;
        match tokens[cursor].text {
            ";" | "}" => return false,
            "{" => {
                return match tokens.get(cursor.wrapping_sub(1)).map(|token| token.text) {
                    Some("import") => true,
                    Some("export") => tokens
                        .get(index.wrapping_sub(1))
                        .is_some_and(|token| token.text == "as"),
                    _ => false,
                };
            }
            _ => {}
        }
    }
    false
}

fn visit_single_character_identifiers(tokens: &[Token<'_>], mut visit: impl FnMut(u8)) {
    for token in tokens {
        if token.kind == TokenKind::Identifier && token.text.len() == 1 {
            visit(token.text.as_bytes()[0]);
        } else if token.kind == TokenKind::Template {
            if let Ok(names) = template_expression_identifier_names(token.text) {
                for name in names {
                    if name.len() == 1 {
                        visit(name.as_bytes()[0]);
                    }
                }
            }
        }
    }
}

pub fn single_character_identifiers(source: &str) -> Result<Vec<u8>, JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut identifiers = Vec::new();
    visit_single_character_identifiers(&tokens, |byte| identifiers.push(byte));
    identifiers.sort_unstable();
    identifiers.dedup();
    Ok(identifiers)
}

/// Returns one-byte identifier spellings whose every occurrence resolves to
/// a generated binding and can therefore participate in a whole-program
/// bijective rename. Unlike the broader entropy probe, this deliberately
/// rejects ambient reads, property names, shorthand properties, labels, and
/// template substitutions that the lightweight scope proof cannot resolve.
pub fn single_character_resolved_binding_identifiers(
    source: &str,
) -> Result<Vec<u8>, JavaScriptParseError> {
    let tokens = lex(source)?;
    let resolution = BindingResolution::new(&tokens);
    let template_names = template_identifier_names_in_tokens(&tokens)?;
    let mut candidates = std::collections::BTreeSet::new();
    let mut rejected = std::collections::BTreeSet::new();

    for (index, token) in tokens.iter().enumerate() {
        if token.kind == TokenKind::Identifier && token.text.len() == 1 {
            let byte = token.text.as_bytes()[0];
            candidates.insert(byte);
            if !identifier_occurrence_is_clear_binding(&tokens, index)
                || !matches!(resolution.resolve(index), Resolution::Bound(_))
                || template_names.contains(token.text)
            {
                rejected.insert(byte);
            }
        }
    }
    Ok(candidates
        .difference(&rejected)
        .copied()
        .collect::<Vec<_>>())
}

pub fn single_character_identifier_use_counts(
    source: &str,
) -> Result<[usize; 128], JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut counts = [0usize; 128];
    visit_single_character_identifiers(&tokens, |byte| {
        counts[byte as usize] += 1;
    });
    Ok(counts)
}

fn collect_parameter_binding_names<'src>(
    tokens: &[Token<'src>],
    from: usize,
    to: usize,
    declared: &mut std::collections::BTreeSet<&'src str>,
) {
    let mut depth = 0usize;
    let mut expects_name = true;
    for token in &tokens[from..to] {
        if depth == 0 && expects_name && token.kind == TokenKind::Identifier {
            declared.insert(token.text);
            expects_name = false;
        }
        match token.text {
            "(" | "[" | "{" => depth += 1,
            ")" | "]" | "}" => depth = depth.saturating_sub(1),
            "," if depth == 0 => expects_name = true,
            "=" if depth == 0 => expects_name = false,
            _ => {}
        }
    }
}

/// Count the ASCII characters contributed by identifier tokens whose names
/// are declared somewhere in the generated artifact. The entropy mangler uses
/// this to remove incumbent binding spellings from its character-frequency
/// seed while retaining keywords, property names, string contents, and host
/// globals as compression context.
pub fn declared_identifier_character_use_counts(
    source: &str,
) -> Result<[usize; 128], JavaScriptParseError> {
    let tokens = lex(source)?;
    let matching_close = matching_closers(&tokens);
    let matching_open = crate::js_peephole::token::matching_openers(&matching_close);
    let mut declared = std::collections::BTreeSet::<&str>::new();

    for (index, token) in tokens.iter().enumerate() {
        match token.text {
            "let" | "var" | "const" => {
                let mut scan = index + 1;
                let mut depth = 0usize;
                let mut expects_name = true;
                while scan < tokens.len() {
                    let current = tokens[scan];
                    if depth == 0
                        && (current.text == ";"
                            || current.text == "of"
                            || current.text == "in"
                            || current.text == ")")
                    {
                        break;
                    }
                    if depth == 0 && expects_name && current.kind == TokenKind::Identifier {
                        declared.insert(current.text);
                        expects_name = false;
                    }
                    match current.text {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => depth = depth.saturating_sub(1),
                        "," if depth == 0 => expects_name = true,
                        "=" if depth == 0 => expects_name = false,
                        _ => {}
                    }
                    scan += 1;
                }
            }
            "function" => {
                let mut scan = index + 1;
                if tokens
                    .get(scan)
                    .is_some_and(|candidate| candidate.kind == TokenKind::Identifier)
                {
                    declared.insert(tokens[scan].text);
                    scan += 1;
                }
                if tokens.get(scan).map(|candidate| candidate.text) == Some("(") {
                    if let Some(close) = matching_close.get(scan).copied().flatten() {
                        collect_parameter_binding_names(&tokens, scan + 1, close, &mut declared);
                    }
                }
            }
            "class" => {
                if let Some(name) = tokens
                    .get(index + 1)
                    .filter(|candidate| candidate.kind == TokenKind::Identifier)
                {
                    declared.insert(name.text);
                }
            }
            "catch" => {
                let open = index + 1;
                if tokens.get(open).map(|candidate| candidate.text) == Some("(") {
                    if let Some(close) = matching_close.get(open).copied().flatten() {
                        collect_parameter_binding_names(&tokens, open + 1, close, &mut declared);
                    }
                }
            }
            "=>" => {
                if let Some(previous) = index.checked_sub(1) {
                    if tokens[previous].kind == TokenKind::Identifier {
                        declared.insert(tokens[previous].text);
                    } else if tokens[previous].text == ")" {
                        if let Some(open) = matching_open.get(previous).copied().flatten() {
                            collect_parameter_binding_names(
                                &tokens,
                                open + 1,
                                previous,
                                &mut declared,
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    let mut counts = [0usize; 128];
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Identifier
            || !declared.contains(token.text)
            || crate::js_peephole::rewrite::is_property_identifier(&tokens, index)
        {
            continue;
        }
        for byte in token.text.bytes().filter(|byte| byte.is_ascii()) {
            counts[byte as usize] += 1;
        }
    }
    Ok(counts)
}

fn identifier_occurrence_is_clear_binding(tokens: &[Token<'_>], index: usize) -> bool {
    let previous = index
        .checked_sub(1)
        .map(|prev| tokens[prev].text)
        .unwrap_or(";");
    let next = tokens.get(index + 1).map(|token| token.text).unwrap_or(";");
    if previous == "." || next == ":" {
        return false;
    }
    !matches!(previous, "{" | ",") || !matches!(next, "}" | "," | "(")
}

pub fn identifier_name_is_clear_binding(
    source: &str,
    name: &str,
) -> Result<bool, JavaScriptParseError> {
    if name.is_empty() {
        return Ok(false);
    }
    let tokens = lex(source)?;
    if template_identifier_names_in_tokens(&tokens)?.contains(name) {
        return Ok(false);
    }
    let resolution = BindingResolution::new(&tokens);
    let mut seen = false;
    for (index, token) in tokens.iter().enumerate() {
        if token.kind != TokenKind::Identifier || token.text != name {
            continue;
        }
        seen = true;
        if !identifier_occurrence_is_clear_binding(&tokens, index)
            || !matches!(resolution.resolve(index), Resolution::Bound(_))
        {
            return Ok(false);
        }
    }
    Ok(seen)
}

fn template_identifier_names_in_tokens(
    tokens: &[Token<'_>],
) -> Result<std::collections::BTreeSet<String>, JavaScriptParseError> {
    let mut names = std::collections::BTreeSet::new();
    for token in tokens {
        if token.kind == TokenKind::Template {
            names.extend(template_expression_identifier_names(token.text)?);
        }
    }
    Ok(names)
}

fn template_expression_identifier_names(
    template: &str,
) -> Result<std::collections::BTreeSet<String>, JavaScriptParseError> {
    let bytes = template.as_bytes();
    let mut names = std::collections::BTreeSet::new();
    let mut cursor = 1usize;
    while cursor + 1 < bytes.len() {
        if bytes[cursor] == b'\\' {
            cursor = (cursor + 2).min(bytes.len());
            continue;
        }
        if bytes[cursor] != b'$' || bytes[cursor + 1] != b'{' {
            cursor += 1;
            continue;
        }
        let expression_start = cursor + 2;
        let expression_end = scan_template_expression(bytes, expression_start, 0)?;
        let expression = &template[expression_start..expression_end - 1];
        for token in lex(expression)? {
            if token.kind == TokenKind::Identifier {
                names.insert(token.text.to_string());
            } else if token.kind == TokenKind::Template {
                names.extend(template_expression_identifier_names(token.text)?);
            }
        }
        cursor = expression_end;
    }
    Ok(names)
}

pub fn two_character_identifier_use_counts(
    source: &str,
) -> Result<Vec<(String, usize)>, JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut counts = std::collections::BTreeMap::<String, usize>::new();
    for token in &tokens {
        if token.kind == TokenKind::Identifier && token.text.len() == 2 {
            *counts.entry(token.text.to_string()).or_insert(0) += 1;
        }
    }
    Ok(counts.into_iter().collect())
}

pub fn remap_identifier(
    source: &str,
    from: &str,
    to: &str,
) -> Result<String, JavaScriptParseError> {
    if from == to || from.is_empty() || to.is_empty() {
        return Ok(source.to_string());
    }
    if !identifier_name_is_clear_binding(source, from)? {
        return Ok(source.to_string());
    }
    let tokens = lex(source)?;
    let mut replacements = Vec::new();
    for token in &tokens {
        if token.kind == TokenKind::Identifier && token.text == from {
            replacements.push((token.start, token.end, to.to_string()));
        }
    }
    if replacements.is_empty() {
        return Ok(source.to_string());
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for (start, end, replacement) in replacements {
        output.push_str(&source[cursor..start]);
        output.push_str(&replacement);
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

pub fn single_character_name_is_clear_binding(
    source: &str,
    name: u8,
) -> Result<bool, JavaScriptParseError> {
    let name_text = [name];
    let name = std::str::from_utf8(&name_text).unwrap_or("");
    identifier_name_is_clear_binding(source, name)
}

pub fn remap_single_character_identifiers(
    source: &str,
    mapping: &[u8; 128],
) -> Result<String, JavaScriptParseError> {
    let tokens = lex(source)?;
    let mut replacements = Vec::new();
    for token in &tokens {
        if token.kind == TokenKind::Identifier && token.text.len() == 1 {
            let byte = token.text.as_bytes()[0];
            let replacement = mapping[byte as usize];
            if replacement != byte {
                replacements.push((token.start, token.end, replacement));
            }
        } else if token.kind == TokenKind::Template {
            for (offset, window) in token.text.as_bytes().windows(4).enumerate() {
                if window[0] != b'$'
                    || window[1] != b'{'
                    || !window[2].is_ascii_alphabetic()
                    || window[3] != b'}'
                {
                    continue;
                }
                let replacement = mapping[window[2] as usize];
                if replacement != window[2] {
                    replacements.push((
                        token.start + offset + 2,
                        token.start + offset + 3,
                        replacement,
                    ));
                }
            }
        }
    }
    if replacements.is_empty() {
        return Ok(source.to_string());
    }
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for (start, end, replacement) in replacements {
        output.push_str(&source[cursor..start]);
        output.push(char::from(replacement));
        cursor = end;
    }
    output.push_str(&source[cursor..]);
    Ok(output)
}

/// Produces one candidate for each bijective swap of two one-byte local
/// bindings inside one simple top-level function. The rewrite is deliberately
/// scope-local: module bindings and locals in sibling functions retain their
/// spelling, allowing the whole-artifact codec scorer to optimize the same
/// per-scope namespace that JavaScript engines expose.
pub fn function_local_binding_swap_variants(
    source: &str,
) -> Result<Vec<String>, JavaScriptParseError> {
    let tokens = lex(source)?;
    let resolution = BindingResolution::new(&tokens);
    if !resolution.is_total() || tokens.iter().any(|token| token.kind == TokenKind::Template) {
        return Ok(Vec::new());
    }
    let matching_close = matching_closers(&tokens);
    let mut functions = Vec::<std::collections::BTreeMap<u8, Vec<usize>>>::new();

    for cursor in 0..tokens.len() {
        if tokens[cursor].text != "function"
            || crate::js_peephole::scope::enclosing_function_span(&tokens, &matching_close, cursor)
                .is_some()
        {
            continue;
        }
        let mut name = cursor + 1;
        if tokens.get(name).map(|token| token.text) == Some("*") {
            name += 1;
        }
        if tokens
            .get(name)
            .is_none_or(|token| token.kind != TokenKind::Identifier)
            || tokens.get(name + 1).map(|token| token.text) != Some("(")
        {
            continue;
        }
        let params_open = name + 1;
        let Some(params_close) = matching_close.get(params_open).copied().flatten() else {
            continue;
        };
        let body_open = params_close + 1;
        if tokens.get(body_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(body_open).copied().flatten() else {
            continue;
        };
        if !crate::js_peephole::scope::simple_identifier_params(
            &tokens,
            params_open + 1,
            params_close,
        ) || tokens[body_open + 1..body_close].iter().any(|token| {
            matches!(token.text, "function" | "=>" | "class" | "catch")
                || token.kind == TokenKind::Template
        }) {
            continue;
        }

        let mut bindings = std::collections::BTreeSet::<u8>::new();
        for token in &tokens[params_open + 1..params_close] {
            if token.kind == TokenKind::Identifier && token.text.len() == 1 {
                bindings.insert(token.text.as_bytes()[0]);
            }
        }
        let mut declaration_scan = body_open + 1;
        let mut declarations_are_simple = true;
        while declaration_scan < body_close {
            if !matches!(tokens[declaration_scan].text, "var" | "let" | "const") {
                declaration_scan += 1;
                continue;
            }
            declaration_scan += 1;
            let mut delimiter_depth = 0usize;
            let mut expects_name = true;
            while declaration_scan < body_close {
                let token = tokens[declaration_scan];
                if delimiter_depth == 0 && matches!(token.text, ";" | "of" | "in" | ")") {
                    break;
                }
                if delimiter_depth == 0 && expects_name {
                    if token.kind != TokenKind::Identifier {
                        declarations_are_simple = false;
                        break;
                    }
                    if token.text.len() == 1 {
                        bindings.insert(token.text.as_bytes()[0]);
                    }
                    expects_name = false;
                }
                match token.text {
                    "(" | "[" | "{" => delimiter_depth += 1,
                    ")" | "]" | "}" if delimiter_depth > 0 => delimiter_depth -= 1,
                    "," if delimiter_depth == 0 => expects_name = true,
                    "=" if delimiter_depth == 0 => expects_name = false,
                    _ => {}
                }
                declaration_scan += 1;
            }
            if !declarations_are_simple {
                break;
            }
        }
        if !declarations_are_simple || bindings.len() < 2 {
            continue;
        }

        let mut occurrences = bindings
            .iter()
            .copied()
            .map(|binding| (binding, Vec::new()))
            .collect::<std::collections::BTreeMap<_, _>>();
        let mut rejected = std::collections::BTreeSet::new();
        for index in params_open + 1..body_close {
            let token = tokens[index];
            if token.kind != TokenKind::Identifier || token.text.len() != 1 {
                continue;
            }
            let byte = token.text.as_bytes()[0];
            if !bindings.contains(&byte) {
                continue;
            }
            if !identifier_occurrence_is_clear_binding(&tokens, index)
                || !matches!(resolution.resolve(index), Resolution::Bound(declaration) if declaration > params_open && declaration < body_close)
            {
                rejected.insert(byte);
            } else {
                occurrences.entry(byte).or_default().push(index);
            }
        }
        occurrences.retain(|binding, uses| !uses.is_empty() && !rejected.contains(binding));
        if occurrences.len() >= 2 {
            functions.push(occurrences);
        }
    }

    // A helper moved to its only call creates a nested IIFE. Optimize that
    // caller as one lexical namespace permutation: every declaration and
    // resolved local reference in the complete function tree is swapped,
    // while a spelling with even one module/global or property occurrence is
    // frozen. A bijection over the whole tree preserves shadowing and capture
    // relationships, including sibling and nested functions.
    for cursor in 0..tokens.len() {
        if tokens[cursor].text != "function"
            || crate::js_peephole::scope::enclosing_function_span(&tokens, &matching_close, cursor)
                .is_some()
        {
            continue;
        }
        let mut name = cursor + 1;
        if tokens.get(name).map(|token| token.text) == Some("*") {
            name += 1;
        }
        let Some(function_name) = tokens
            .get(name)
            .filter(|token| token.kind == TokenKind::Identifier)
            .map(|token| token.text)
        else {
            continue;
        };
        if tokens.get(name + 1).map(|token| token.text) != Some("(") {
            continue;
        }
        let params_open = name + 1;
        let Some(params_close) = matching_close.get(params_open).copied().flatten() else {
            continue;
        };
        let body_open = params_close + 1;
        if tokens.get(body_open).map(|token| token.text) != Some("{") {
            continue;
        }
        let Some(body_close) = matching_close.get(body_open).copied().flatten() else {
            continue;
        };
        if !tokens[body_open + 1..body_close]
            .iter()
            .any(|token| matches!(token.text, "function" | "=>"))
        {
            continue;
        }

        let mut occurrences = std::collections::BTreeMap::<u8, Vec<usize>>::new();
        let mut rejected = std::collections::BTreeSet::<u8>::new();
        if function_name.len() == 1 {
            rejected.insert(function_name.as_bytes()[0]);
        }
        for index in params_open + 1..body_close {
            let token = tokens[index];
            if token.kind == TokenKind::Template {
                for window in token.text.as_bytes().windows(4) {
                    if window[0] == b'$'
                        && window[1] == b'{'
                        && window[2].is_ascii_alphabetic()
                        && window[3] == b'}'
                    {
                        rejected.insert(window[2]);
                    }
                }
                continue;
            }
            if token.kind != TokenKind::Identifier || token.text.len() != 1 {
                continue;
            }
            let byte = token.text.as_bytes()[0];
            if !identifier_occurrence_is_clear_binding(&tokens, index)
                || !matches!(resolution.resolve(index), Resolution::Bound(declaration) if declaration > params_open && declaration < body_close)
            {
                rejected.insert(byte);
            } else {
                occurrences.entry(byte).or_default().push(index);
            }
        }
        occurrences.retain(|binding, uses| !uses.is_empty() && !rejected.contains(binding));
        if occurrences.len() >= 2 {
            functions.push(occurrences);
        }
    }

    let mut variants = Vec::new();
    for occurrences in functions {
        let names = occurrences.keys().copied().collect::<Vec<_>>();
        for (left_index, left) in names.iter().copied().enumerate() {
            for right in names.iter().copied().skip(left_index + 1) {
                let mut replacements = Vec::new();
                for index in &occurrences[&left] {
                    replacements.push((
                        tokens[*index].start,
                        tokens[*index].end,
                        char::from(right).to_string(),
                    ));
                }
                for index in &occurrences[&right] {
                    replacements.push((
                        tokens[*index].start,
                        tokens[*index].end,
                        char::from(left).to_string(),
                    ));
                }
                variants.push(apply_token_rewrites(source, replacements).0);
            }
        }
    }
    variants.sort();
    variants.dedup();
    Ok(variants)
}

/// Parses generated JavaScript, applies semantics-preserving local rewrites,
/// and derives deterministic engine-startup proxies from the parsed program.
///
/// The parser is deliberately conservative. Unsupported expressions still
/// contribute to token metrics but are never rewritten.
pub fn optimize_generated_javascript(source: &str) -> Result<PeepholeResult, JavaScriptParseError> {
    optimize_generated_javascript_with(source, true, false)
}

/// [`optimize_generated_javascript`] under an explicit builtins contract.
///
/// A few rewrites are equivalent only when the language's own prototypes have
/// not been reshaped — rebuilding an array from its pushes is one, because
/// `Array.prototype.push` honours an inherited index setter and an array
/// literal does not. Those folds run only when the project has declared
/// `javascript.assume_pristine_builtins`.
pub fn optimize_generated_javascript_assuming(
    source: &str,
    pristine_builtins: bool,
) -> Result<PeepholeResult, JavaScriptParseError> {
    optimize_generated_javascript_with(source, true, pristine_builtins)
}

/// Same local rewrites as [`optimize_generated_javascript`], except folds that
/// move or erase function bindings. Search-off must still minify declarations
/// and increments when the full pass would cross the function-count boundary.
pub fn optimize_generated_javascript_preserving_functions(
    source: &str,
) -> Result<PeepholeResult, JavaScriptParseError> {
    optimize_generated_javascript_with(source, false, false)
}

/// [`optimize_generated_javascript_preserving_functions`] under the same
/// builtins contract as [`optimize_generated_javascript_assuming`].
pub fn optimize_generated_javascript_preserving_functions_assuming(
    source: &str,
    pristine_builtins: bool,
) -> Result<PeepholeResult, JavaScriptParseError> {
    optimize_generated_javascript_with(source, false, pristine_builtins)
}

fn optimize_generated_javascript_with(
    source: &str,
    elide_functions: bool,
    pristine_builtins: bool,
) -> Result<PeepholeResult, JavaScriptParseError> {
    let first = optimize_generated_javascript_pass(source, elide_functions, pristine_builtins)?;
    if first.rewrites == 0 || !constructor_table_remains(&first.code) {
        return Ok(first);
    }
    match optimize_generated_javascript_pass(&first.code, elide_functions, pristine_builtins) {
        Ok(second) if second.rewrites > 0 => Ok(PeepholeResult {
            code: second.code,
            metrics: second.metrics,
            rewrites: first.rewrites.saturating_add(second.rewrites),
        }),
        Ok(_) | Err(_) => Ok(first),
    }
}

fn constructor_table_remains(source: &str) -> bool {
    source.contains(".prototype")
        && (source.contains("(0,function") || source.contains("=function("))
}

fn optimize_generated_javascript_pass(
    source: &str,
    elide_functions: bool,
    pristine_builtins: bool,
) -> Result<PeepholeResult, JavaScriptParseError> {
    let tokens = lex(source)?;
    validate_delimiters(&tokens)?;
    validate_generated_declaration_syntax(source, &tokens)?;
    validate_generated_bare_arrow_parameters(&tokens)?;
    let parsed = parse_expression_regions(&tokens);
    let mut compound = parsed
        .iter()
        .filter_map(|region| compound_assignment_rewrite(&tokens, region))
        .collect::<Vec<_>>();
    compound.sort_unstable_by_key(|rewrite| (rewrite.start, rewrite.end));
    compound.dedup_by_key(|rewrite| (rewrite.start, rewrite.end));
    compound = non_overlapping_rewrites(compound);

    let mut session = RewriteSession::new(apply_rewrites(source, &compound));
    session.rewrites += compound.len();
    session.run_if(elide_functions, fold_value_binding_iife)?;
    session.run_if(
        elide_functions,
        fold_constructor_prototype_tables_to_classes,
    )?;
    session.run(fold_indexed_arguments_to_formals)?;
    session.run(fold_undefined_defaults_into_formals)?;
    session.run(fold_arguments_length_formal_copies)?;
    session.run(fold_arguments_slice_to_rest)?;
    session.run(rotate_proven_initial_true_loops)?;
    session.run_flag(reuse_dead_var_binding)?;
    session.run(remove_unused_standalone_vars)?;
    session.run(merge_adjacent_declarations)?;
    session.run(fold_assignment_guards)?;
    session.run(fold_guarded_assign_into_call_predicate)?;
    session.run(fold_index_postfix_updates)?;
    session.run(fold_guarded_and_addends)?;
    session.run(fold_unit_counter_updates)?;
    session.run(fold_while_true_unit_increment_bounds)?;
    session.run(fold_int32_coercions)?;
    session.repeat(fold_identifier_copies, 8)?;
    session.run(strip_unused_simple_declarators)?;
    session.repeat(fold_single_use_literal_bindings, 8)?;
    session.run_if(elide_functions, fold_single_use_function_values)?;
    session.run_if(elide_functions, fold_single_use_function_values)?;
    session.run(fold_typeof_identifier_caches)?;
    session.run(fold_coalesced_or_returns)?;
    session.run(flatten_associative_string_concats)?;
    session.run(fold_known_string_coercions)?;
    session.run(fold_for_false_breaks)?;
    session.run(fold_nullish_index_walks)?;
    session.repeat(fold_while_trailing_increments, 4)?;
    session.run(fold_for_trailing_increments)?;
    session.run(strip_unused_for_init_vars)?;
    session.run(fold_chained_identifier_assigns)?;
    session.run_if(elide_functions, fold_single_use_function_values)?;
    session.run(fold_prefix_increment_for_bounds)?;
    session.run(fold_increment_infinite_for_bounds)?;
    session.run(fold_dead_initializer_reassigns)?;
    session.run(fold_void_then_reassign)?;
    session.run(strip_void_initializer_before_write)?;
    session.run(remove_unused_standalone_vars)?;
    session.run(fold_assigned_index_for_conditions)?;
    session.run(fold_index_scan_for_headers)?;
    session.run(fold_statement_or_assigns)?;
    session.run(fold_statement_negated_ors)?;
    session.run(fold_chained_comma_assigns)?;
    session.run(fold_adjacent_expression_statements)?;
    session.repeat(fold_redundant_loop_body_braces, 4)?;
    session.run(fold_prior_assign_into_for_init)?;
    session.run(fold_arguments_length_countdown_for)?;
    session.run(fold_arguments_length_zero_after_decrement)?;
    session.run(fold_predicate_reassign_same_expr)?;
    session.run(fold_arguments_length_eq_zero_to_not)?;
    session.run(fold_integer_neq_zero_in_boolean)?;
    session.run(fold_guarded_uninitialized_assign)?;
    session.run(fold_identity_arrow_iife)?;
    session.run(fold_empty_ternary_then_comma)?;
    session.repeat(fold_single_use_if_assigns, 6)?;
    session.run(fold_if_prefixed_returns)?;
    session.run(fold_nested_unguarded_ifs)?;
    session.run(fold_if_expression_to_and)?;
    session.run(fold_copied_member_presence)?;
    session.run(fold_try_if_return_alternatives)?;
    session.run(fold_if_prefix_guard_return)?;
    session.run(fold_omissible_trailing_false_args)?;
    session.run(fold_boolean_context_double_not)?;
    session.run(fold_redundant_and_parens)?;
    session.run(flip_false_equalities)?;
    session.run_if(elide_functions, fold_single_use_function_values)?;
    session.run(fold_single_use_regex_bindings)?;
    session.run_if(
        elide_functions,
        fold_constructor_prototype_tables_to_classes,
    )?;
    session.run(fold_indexed_arguments_to_formals)?;
    session.run(fold_undefined_defaults_into_formals)?;
    session.run(fold_arguments_length_formal_copies)?;
    session.run(fold_arguments_slice_to_rest)?;
    session.run(fold_dead_pure_identifier_assigns)?;
    session.run(fold_conditional_assigned_false_phi)?;
    session.run(remove_unused_standalone_vars)?;
    session.run(fold_uninitialized_var_into_assign)?;
    session.repeat(fold_expression_branches, 4)?;
    session.run(fold_copied_member_presence)?;
    session.run(fold_same_lvalue_ternary)?;
    session.run(fold_or_reassign_to_ternary)?;
    session.repeat(fold_single_use_if_assigns, 4)?;
    session.run(fold_if_prefixed_returns)?;
    session.run(fold_nested_unguarded_ifs)?;
    session.run(fold_if_expression_to_and)?;
    session.run(fold_copied_member_presence)?;
    session.run(fold_integer_neq_zero_in_boolean)?;
    session.run(fold_assigned_truthy_ternaries)?;
    session.run(fold_console_log_conditionals)?;
    session.run(fold_arrow_guard_returns)?;
    session.run(fold_conditional_return_tails)?;
    session.run(fold_returned_temporaries)?;
    session.run(fold_trailing_return_this)?;
    session.run(fold_single_return_arrow_bodies)?;
    session.run(fold_adjacent_expression_statements)?;
    session.repeat(fold_redundant_loop_body_braces, 4)?;
    session.run(fold_arguments_length_countdown_for)?;
    session.run(fold_adjacent_expression_statements)?;
    session.run(fold_arguments_length_zero_after_decrement)?;
    session.run(fold_void_prefix_updates)?;
    session.run(fold_negated_equalities)?;
    session.run(fold_redundant_null_undefined_or)?;
    session.run(reorder_uninitialized_var_declarators)?;
    session.run_if(elide_functions, fold_single_use_function_values)?;
    session.repeat(fold_forwarding_call_wrappers, 4)?;
    session.run(fold_int32_coercions)?;
    session.run(canonicalize_leaf_syntax)?;
    session.run(fold_known_string_coercions)?;
    session.run(elide_separating_keyword_spaces)?;

    canonical_late_generated_javascript_cleanup_into(&mut session)?;
    session.run(fold_if_prefixed_returns)?;
    session.run(fold_nested_unguarded_ifs)?;
    session.run(fold_conditional_return_tails)?;
    session.run(fold_unit_counter_updates)?;
    session.run(fold_while_true_unit_increment_bounds)?;
    session.run(fold_int32_coercions)?;
    session.run(fold_top_level_adjacent_expression_statements)?;
    session.run(fold_or_assignment_parens)?;
    session.run(declare_implicit_assignment_bindings)?;
    session.repeat(fold_forwarding_call_wrappers, 4)?;
    session.run(fold_int32_coercions)?;
    session.run(split_fused_keyword_identifiers)?;
    session.run(strip_stale_set_prototype_of)?;
    session.run(terminate_bare_prototype_before_statement)?;
    session.run_if(
        elide_functions,
        fold_constructor_prototype_tables_to_classes,
    )?;
    session.run(strip_stale_set_prototype_of)?;
    session.run(terminate_bare_prototype_before_statement)?;
    session.run(fold_dead_pure_identifier_assigns)?;
    session.run(fold_unread_prototype_aliases)?;
    session.run(fold_dead_identifier_copy_declarators)?;
    session.run(remove_unused_standalone_vars)?;
    session.run(fold_or_empty_object_assign)?;
    session.run_if(pristine_builtins, fold_fresh_empty_object_assign)?;
    if pristine_builtins {
        session.repeat(fold_fresh_empty_array_pushes, 4)?;
    }
    session.run(fold_named_class_identity)?;
    session.run(rewrite_class_ctor_identity_to_new_target)?;
    session.run_if(elide_functions, fold_value_binding_iife)?;
    session.run(fold_undefined_defaults_into_formals)?;
    session.run(drop_redundant_class_constructor_guards)?;
    session.run(remove_unused_standalone_vars)?;
    session.run(drop_orphaned_class_identity_guards)?;
    session.repeat(fold_single_use_temporaries, 4)?;
    session.run(fold_returned_temporaries)?;
    session.run(remove_unused_standalone_vars)?;
    session.run(hoist_async_arrow_method_bodies)?;
    session.run(drop_pure_regex_expression_statements)?;
    session.run(fold_empty_comma_operators)?;
    session.run(elide_asi_safe_semicolons)?;

    let final_tokens = if session.rewrites == 0 {
        tokens
    } else {
        lex(&session.code)?
    };
    let final_nesting = validate_delimiters(&final_tokens)?;
    validate_generated_declaration_syntax(&session.code, &final_tokens)?;
    validate_generated_bare_arrow_parameters(&final_tokens)?;
    validate_unique_top_level_bindings(&final_tokens)?;
    validate_class_body_members(&final_tokens)?;
    let final_parsed = parse_expression_regions(&final_tokens);
    let metrics = syntax_metrics(&session.code, &final_tokens, &final_parsed, final_nesting);
    Ok(PeepholeResult {
        code: session.code,
        metrics,
        rewrites: session.rewrites,
    })
}

pub fn repair_fused_keyword_identifiers(source: &str) -> Result<String, JavaScriptParseError> {
    split_fused_keyword_identifiers(source).map(|(code, _)| code)
}

pub fn late_generated_javascript_cleanup(source: &str) -> Result<String, JavaScriptParseError> {
    let mut session = RewriteSession::new(source.to_string());
    late_generated_javascript_cleanup_into(&mut session)?;
    Ok(session.code)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LateJavaScriptCleanupPass {
    ConditionalReturnTails,
    GuardReturnExpressionSuffixes,
    ExpressionReturnBranches,
    ExpressionSuffixReturns,
    SequenceAssignmentFirstUse,
    StatementAssignmentFirstUse,
    NegatedConditionalArms,
    BooleanConditionalValues,
    UnitCounterUpdates,
    CommonConditionalArms,
    EarlyExitGuards,
    ContinueTailGuards,
    InvertedContinueTailGuards,
    SingleStatementControlBraces,
    NegatedEqualities,
    RedundantNullUndefinedOr,
    IdentTernaryToOr,
    OrAssignmentParens,
    NotGtZeroLength,
    IdentityArrowIife,
    ZeroArgumentReturnIife,
    SingleUseFunctionExpressions,
    EmptyTernaryThenComma,
    UnusedStandaloneVars,
    ArgumentsLengthCountdownFor,
    CanonicalLeafSyntax,
    SameBindingStrictEquality,
}

impl LateJavaScriptCleanupPass {
    pub(crate) const ALL: [Self; 21] = [
        Self::ConditionalReturnTails,
        Self::GuardReturnExpressionSuffixes,
        Self::NegatedConditionalArms,
        Self::BooleanConditionalValues,
        Self::UnitCounterUpdates,
        Self::EarlyExitGuards,
        Self::ContinueTailGuards,
        Self::InvertedContinueTailGuards,
        Self::SingleStatementControlBraces,
        Self::NegatedEqualities,
        Self::RedundantNullUndefinedOr,
        Self::IdentTernaryToOr,
        Self::OrAssignmentParens,
        Self::NotGtZeroLength,
        Self::IdentityArrowIife,
        Self::ZeroArgumentReturnIife,
        Self::EmptyTernaryThenComma,
        Self::UnusedStandaloneVars,
        Self::ArgumentsLengthCountdownFor,
        Self::CanonicalLeafSyntax,
        Self::SameBindingStrictEquality,
    ];

    const fn objective_only(self) -> bool {
        matches!(
            self,
            Self::GuardReturnExpressionSuffixes
                | Self::ExpressionReturnBranches
                | Self::ExpressionSuffixReturns
                | Self::SequenceAssignmentFirstUse
                | Self::StatementAssignmentFirstUse
                | Self::NegatedConditionalArms
                | Self::BooleanConditionalValues
                | Self::UnitCounterUpdates
                | Self::CommonConditionalArms
                | Self::ZeroArgumentReturnIife
                | Self::SingleUseFunctionExpressions
                | Self::SameBindingStrictEquality
        )
    }
}

/// Apply one late syntax proposal and close any implicit-assignment bindings
/// it exposes. The compiler uses these independently so raw, gzip, and Brotli
/// can retain different subsets under their exact whole-artifact objective.
pub(crate) fn late_generated_javascript_cleanup_pass(
    source: &str,
    pass: LateJavaScriptCleanupPass,
) -> Result<String, JavaScriptParseError> {
    let mut session = RewriteSession::new(source.to_string());
    late_generated_javascript_cleanup_pass_into(&mut session, pass)?;
    session.run(declare_implicit_assignment_bindings)?;
    Ok(session.code)
}

/// Produce candidates that apply one independently proven local rewrite.
///
/// Most late passes intentionally expose one all-sites spelling. Sequence
/// topology is different: it is often raw-neutral, and dictionary codecs can
/// prefer a sparse subset of sites. The compiler walks these local variants
/// with its exact configured scorer instead of treating Terser-style global
/// sequence normalization as universally beneficial.
pub(crate) fn late_generated_javascript_cleanup_local_variants(
    source: &str,
    pass: LateJavaScriptCleanupPass,
) -> Result<Vec<String>, JavaScriptParseError> {
    let mut variants = match pass {
        LateJavaScriptCleanupPass::ExpressionSuffixReturns => {
            expression_suffix_return_variants(source)?
        }
        LateJavaScriptCleanupPass::BooleanConditionalValues => {
            boolean_conditional_value_variants(source)?
        }
        _ => Vec::new(),
    };
    for variant in &mut variants {
        let mut session = RewriteSession::new(std::mem::take(variant));
        session.run(declare_implicit_assignment_bindings)?;
        *variant = session.code;
    }
    Ok(variants)
}

fn late_generated_javascript_cleanup_pass_into(
    session: &mut RewriteSession,
    pass: LateJavaScriptCleanupPass,
) -> Result<(), JavaScriptParseError> {
    match pass {
        LateJavaScriptCleanupPass::ConditionalReturnTails => {
            session.run(fold_conditional_return_tails)?
        }
        LateJavaScriptCleanupPass::GuardReturnExpressionSuffixes => {
            session.run(fold_guard_return_expression_suffixes)?
        }
        LateJavaScriptCleanupPass::ExpressionReturnBranches => {
            session.run(fold_expression_return_branches)?
        }
        LateJavaScriptCleanupPass::ExpressionSuffixReturns => {
            session.run(fold_expression_suffix_returns)?
        }
        LateJavaScriptCleanupPass::SequenceAssignmentFirstUse => {
            session.run(fold_sequence_assignments_into_first_use)?
        }
        LateJavaScriptCleanupPass::StatementAssignmentFirstUse => {
            session.run(fold_statement_assignments_into_first_use)?
        }
        LateJavaScriptCleanupPass::NegatedConditionalArms => {
            session.run(fold_negated_conditional_arms)?
        }
        LateJavaScriptCleanupPass::BooleanConditionalValues => {
            session.run(fold_boolean_conditional_values)?
        }
        LateJavaScriptCleanupPass::UnitCounterUpdates => {
            session.run(fold_unit_counter_updates)?;
            session.run(fold_while_true_unit_increment_bounds)?;
            session.run(fold_expression_self_assignments)?
        }
        LateJavaScriptCleanupPass::CommonConditionalArms => {
            session.run(fold_common_conditional_arms)?
        }
        LateJavaScriptCleanupPass::EarlyExitGuards => session.run(fold_early_exit_guards)?,
        LateJavaScriptCleanupPass::ContinueTailGuards => session.run(fold_continue_tail_guards)?,
        LateJavaScriptCleanupPass::InvertedContinueTailGuards => {
            session.run(fold_inverted_continue_tail_guards)?
        }
        LateJavaScriptCleanupPass::SingleStatementControlBraces => {
            session.run(fold_single_statement_control_braces)?
        }
        LateJavaScriptCleanupPass::NegatedEqualities => session.run(fold_negated_equalities)?,
        LateJavaScriptCleanupPass::RedundantNullUndefinedOr => {
            session.run(fold_redundant_null_undefined_or)?
        }
        LateJavaScriptCleanupPass::IdentTernaryToOr => session.run(fold_ident_ternary_to_or)?,
        LateJavaScriptCleanupPass::OrAssignmentParens => session.run(fold_or_assignment_parens)?,
        LateJavaScriptCleanupPass::NotGtZeroLength => session.run(fold_not_gt_zero_length)?,
        LateJavaScriptCleanupPass::IdentityArrowIife => session.run(fold_identity_arrow_iife)?,
        LateJavaScriptCleanupPass::ZeroArgumentReturnIife => {
            session.run(fold_zero_argument_return_iife)?
        }
        LateJavaScriptCleanupPass::SingleUseFunctionExpressions => {
            session.run(fold_single_use_function_expressions)?
        }
        LateJavaScriptCleanupPass::EmptyTernaryThenComma => {
            session.run(fold_empty_ternary_then_comma)?
        }
        LateJavaScriptCleanupPass::UnusedStandaloneVars => {
            session.run(remove_unused_standalone_vars)?
        }
        LateJavaScriptCleanupPass::ArgumentsLengthCountdownFor => {
            session.run(fold_arguments_length_countdown_for)?
        }
        LateJavaScriptCleanupPass::CanonicalLeafSyntax => session.run(canonicalize_leaf_syntax)?,
        LateJavaScriptCleanupPass::SameBindingStrictEquality => {
            session.run(fold_same_binding_strict_equality)?
        }
    }
    Ok(())
}

fn late_generated_javascript_cleanup_into(
    session: &mut RewriteSession,
) -> Result<(), JavaScriptParseError> {
    for pass in LateJavaScriptCleanupPass::ALL {
        late_generated_javascript_cleanup_pass_into(session, pass)?;
    }
    session.run(declare_implicit_assignment_bindings)?;
    Ok(())
}

fn canonical_late_generated_javascript_cleanup_into(
    session: &mut RewriteSession,
) -> Result<(), JavaScriptParseError> {
    for pass in LateJavaScriptCleanupPass::ALL {
        if !pass.objective_only() {
            late_generated_javascript_cleanup_pass_into(session, pass)?;
        }
    }
    session.run(declare_implicit_assignment_bindings)?;
    Ok(())
}

struct RewriteSession {
    code: String,
    rewrites: usize,
}

impl RewriteSession {
    fn new(code: String) -> Self {
        Self { code, rewrites: 0 }
    }

    fn run(
        &mut self,
        fold: impl Fn(&str) -> Result<(String, usize), JavaScriptParseError>,
    ) -> Result<(), JavaScriptParseError> {
        let (next, count) = fold(&self.code)?;
        trace_fold(&fold, &self.code, &next);
        self.code = next;
        self.rewrites += count;
        Ok(())
    }

    fn run_if(
        &mut self,
        enabled: bool,
        fold: impl Fn(&str) -> Result<(String, usize), JavaScriptParseError>,
    ) -> Result<(), JavaScriptParseError> {
        if enabled {
            self.run(fold)
        } else {
            Ok(())
        }
    }

    fn run_flag(
        &mut self,
        fold: impl Fn(&str) -> Result<(String, bool), JavaScriptParseError>,
    ) -> Result<(), JavaScriptParseError> {
        let (next, changed) = fold(&self.code)?;
        trace_fold(&fold, &self.code, &next);
        self.code = next;
        self.rewrites += usize::from(changed);
        Ok(())
    }

    fn repeat(
        &mut self,
        fold: impl Fn(&str) -> Result<(String, usize), JavaScriptParseError>,
        max_rounds: usize,
    ) -> Result<(), JavaScriptParseError> {
        for _ in 0..max_rounds {
            let (next, count) = fold(&self.code)?;
            trace_fold(&fold, &self.code, &next);
            self.code = next;
            self.rewrites += count;
            if count == 0 {
                break;
            }
        }
        Ok(())
    }
}

fn trace_fold<F: ?Sized>(_fold: &F, before: &str, after: &str) {
    if before == after {
        return;
    }
    if std::env::var_os("LILSCRIPT_PEEPHOLE_TRACE").is_some() {
        eprintln!("[peephole] {}\n{after}", std::any::type_name::<F>());
    }
    verify_fold(std::any::type_name::<F>(), before, after);
}

/// Name the fold that first breaks a generated-JavaScript invariant.
///
/// A pipeline this long reports only the offset of the final artifact, which
/// says nothing about which rewrite introduced the fault. Under
/// `LILSCRIPT_PEEPHOLE_VERIFY` every fold is re-validated against the input it
/// was given, so a fold is blamed only for an invariant its own output broke.
fn verify_fold(fold: &str, before: &str, after: &str) {
    if std::env::var_os("LILSCRIPT_PEEPHOLE_VERIFY").is_none() {
        return;
    }
    let was = analyze_generated_javascript(before)
        .err()
        .map(|e| e.message);
    let Err(error) = analyze_generated_javascript(after) else {
        return;
    };
    if was == Some(error.message) {
        return;
    }
    let start = after[..error.offset]
        .char_indices()
        .rev()
        .nth(200)
        .map_or(0, |(i, _)| i);
    let end = after[error.offset..]
        .char_indices()
        .nth(120)
        .map_or(after.len(), |(i, _)| error.offset + i);
    eprintln!(
        "[peephole:verify] {fold} introduced `{}` at {}\n  ...{}>>>{}",
        error.message,
        error.offset,
        &after[start..error.offset],
        &after[error.offset..end],
    );
}
