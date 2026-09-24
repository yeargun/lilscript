//! Delivery of foreign JavaScript and TypeScript host modules (008-D3).
//!
//! A relative `import extern` names a file the output must carry. Its
//! TypeScript is stripped by erasing the spans of type-only syntax (each
//! erased byte becomes a space, line breaks stay), then the module is
//! compacted token by token, keeping a line break wherever one separated two
//! tokens, so automatic semicolon insertion cannot change. Both steps are
//! checked by parsing: the stripped text must parse as JavaScript, and the
//! compacted text must parse to the same tree. Host modules the output
//! imports, and the host modules they import, become one expression that
//! evaluates each module once, in dependency order, in its own scope.

use serde_json::Value;
use std::path::{Path, PathBuf};

/// One delivered host module.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HostModule {
    /// The specifier the output spells, relative to the root module's
    /// directory.
    pub specifier: String,
    /// The module's file stem, for a delivered file name.
    pub stem: String,
    /// The compacted module body without its imports and `export` keywords.
    body: String,
    /// Exported names and the local bindings they export.
    exports: Vec<(String, String)>,
    /// Other host modules this one imports: index, then each imported name
    /// (`None` for a namespace import) and its local binding.
    imports: Vec<(usize, Vec<(Option<String>, String)>)>,
}

impl HostModule {
    /// The module's delivered body size.
    pub fn delivered_bytes(&self) -> usize {
        self.body.len()
    }

    /// The compacted body, without its imports and `export` keywords.
    pub(crate) fn body(&self) -> &str {
        &self.body
    }

    /// Exported names and the local bindings they export.
    pub(crate) fn exports(&self) -> &[(String, String)] {
        &self.exports
    }

    /// Other host modules this one imports: index, then each imported name
    /// (`None` for a namespace import) and its local binding.
    pub(crate) fn imports(&self) -> &[(usize, Vec<(Option<String>, String)>)] {
        &self.imports
    }
}

/// Every host module an output needs, evaluated by one expression.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct HostDelivery {
    pub modules: Vec<HostModule>,
    /// Identifiers host code reads without declaring. An output binding in
    /// scope of the embedded code must not take one of these names.
    pub reserved: Vec<String>,
}

impl HostDelivery {
    pub fn is_empty(&self) -> bool {
        self.modules.is_empty()
    }

    /// Storage this delivery keeps for a compilation's lifetime.
    pub fn retained_bytes(&self) -> u64 {
        let text = |value: &String| value.capacity() as u64;
        let mut bytes = (self.modules.capacity() * std::mem::size_of::<HostModule>()
            + self.reserved.capacity() * std::mem::size_of::<String>())
            as u64;
        bytes += self.reserved.iter().map(text).sum::<u64>();
        for module in &self.modules {
            bytes += text(&module.specifier) + text(&module.stem) + text(&module.body);
            for (exported, local) in &module.exports {
                bytes += text(exported) + text(local);
            }
            for (_, bindings) in &module.imports {
                for (imported, local) in bindings {
                    bytes += imported.as_ref().map_or(0, text) + text(local);
                }
            }
        }
        bytes
    }

    /// The delivered module for an output import source, if any.
    pub fn position(&self, specifier: &str) -> Option<usize> {
        self.modules
            .iter()
            .position(|module| module.specifier == specifier)
    }

    /// `(()=>{...;return[h0,h1]})()`: every module, evaluated once in
    /// dependency order, each in its own function scope; the result lists
    /// each module's namespace object by position.
    pub fn expression(&self, strict: bool) -> String {
        let names = self.scope_names();
        let mut text = String::from("(()=>{");
        if strict {
            text.push_str("\"use strict\";");
        }
        text.push_str("let ");
        for (index, module) in self.modules.iter().enumerate() {
            if index != 0 {
                text.push(',');
            }
            text.push_str(&names[index]);
            text.push_str("=(()=>{");
            for (source, bindings) in &module.imports {
                for (imported, local) in bindings {
                    match imported {
                        None => {
                            text.push_str("const ");
                            text.push_str(local);
                            text.push('=');
                            text.push_str(&names[*source]);
                            text.push(';');
                        }
                        Some(imported) => {
                            text.push_str("const{");
                            text.push_str(imported);
                            if imported != local {
                                text.push(':');
                                text.push_str(local);
                            }
                            text.push_str("}=");
                            text.push_str(&names[*source]);
                            text.push(';');
                        }
                    }
                }
            }
            text.push_str(&module.body);
            text.push_str(";return{");
            for (position, (exported, local)) in module.exports.iter().enumerate() {
                if position != 0 {
                    text.push(',');
                }
                if exported == local {
                    text.push_str(local);
                } else {
                    text.push_str(exported);
                    text.push(':');
                    text.push_str(local);
                }
            }
            text.push_str("}})()");
        }
        text.push_str(";return[");
        for index in 0..self.modules.len() {
            if index != 0 {
                text.push(',');
            }
            text.push_str(&names[index]);
        }
        text.push_str("]})()");
        text
    }

    /// With one module that imports no other, `(()=>{...;return{...}})()`:
    /// its namespace object directly.
    pub fn single(&self, strict: bool) -> Option<String> {
        let [module] = self.modules.as_slice() else {
            return None;
        };
        if !module.imports.is_empty() {
            return None;
        }
        let mut text = String::from("(()=>{");
        if strict {
            text.push_str("\"use strict\";");
        }
        text.push_str(&module.body);
        text.push_str(";return{");
        for (position, (exported, local)) in module.exports.iter().enumerate() {
            if position != 0 {
                text.push(',');
            }
            if exported == local {
                text.push_str(local);
            } else {
                text.push_str(exported);
                text.push(':');
                text.push_str(local);
            }
        }
        text.push_str("}})()");
        Some(text)
    }

    /// Scope-local names for each module's namespace, distinct from every
    /// identifier host code spells.
    fn scope_names(&self) -> Vec<String> {
        let mut names = Vec::with_capacity(self.modules.len());
        let mut counter = 0usize;
        while names.len() < self.modules.len() {
            let candidate = format!("h{counter}");
            counter += 1;
            let taken = self.reserved.iter().any(|name| *name == candidate)
                || self.modules.iter().any(|module| {
                    module.body.contains(&candidate)
                        || module
                            .imports
                            .iter()
                            .flat_map(|(_, bindings)| bindings)
                            .any(|(_, local)| *local == candidate)
                });
            if !taken {
                names.push(candidate);
            }
        }
        names
    }
}

/// Read, strip, compact and link every relative host module `requests`
/// names, with the host modules they import. `root_directory` spells each
/// module's specifier as the output imports it.
pub(crate) fn deliver(
    root_directory: &Path,
    requests: &[PathBuf],
    edition: crate::js_syntax_target::EcmaScriptEdition,
) -> Result<HostDelivery, String> {
    deliver_with_files(root_directory, requests, edition).map(|(delivery, _)| delivery)
}

/// The files `deliver` carries, canonical, in delivery order.
pub(crate) fn delivered_files(
    root_directory: &Path,
    requests: &[PathBuf],
    edition: crate::js_syntax_target::EcmaScriptEdition,
) -> Result<Vec<PathBuf>, String> {
    deliver_with_files(root_directory, requests, edition).map(|(_, files)| files)
}

fn deliver_with_files(
    root_directory: &Path,
    requests: &[PathBuf],
    edition: crate::js_syntax_target::EcmaScriptEdition,
) -> Result<(HostDelivery, Vec<PathBuf>), String> {
    let mut delivery = HostDelivery::default();
    let mut paths: Vec<PathBuf> = Vec::new();
    let mut reserved = std::collections::BTreeSet::new();
    for request in requests {
        visit(
            root_directory,
            request,
            edition,
            &mut Vec::new(),
            &mut paths,
            &mut delivery.modules,
            &mut reserved,
        )?;
    }
    delivery.reserved = reserved.into_iter().collect();
    Ok((delivery, paths))
}

fn visit(
    root_directory: &Path,
    path: &Path,
    edition: crate::js_syntax_target::EcmaScriptEdition,
    stack: &mut Vec<PathBuf>,
    paths: &mut Vec<PathBuf>,
    modules: &mut Vec<HostModule>,
    reserved: &mut std::collections::BTreeSet<String>,
) -> Result<usize, String> {
    let path = path
        .canonicalize()
        .map_err(|error| format!("cannot read host module `{}`: {error}", path.display()))?;
    if let Some(index) = paths.iter().position(|known| *known == path) {
        return Ok(index);
    }
    if stack.contains(&path) {
        return Err(format!(
            "host module `{}` imports itself through a cycle",
            path.display()
        ));
    }
    let source = std::fs::read_to_string(&path)
        .map_err(|error| format!("cannot read host module `{}`: {error}", path.display()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    let typescript = match extension {
        "ts" | "mts" => true,
        "js" | "mjs" => false,
        _ => {
            return Err(format!(
                "host module `{}` must be a .ts, .mts, .js or .mjs file",
                path.display()
            ))
        }
    };
    let stripped = if typescript {
        strip_typescript(&source)?
    } else {
        source.clone()
    };
    let analyzed = analyze(&stripped)?;
    if let Some(feature) = syntax_beyond(&stripped, edition)? {
        return Err(format!(
            "host module `{}` uses {}, beyond the {} syntax target",
            path.display(),
            feature.construct(),
            edition.name()
        ));
    }
    stack.push(path.clone());
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut imports = Vec::new();
    for (specifier, bindings) in &analyzed.imports {
        let dependency = resolve(parent, specifier)?;
        let index = visit(
            root_directory,
            &dependency,
            edition,
            stack,
            paths,
            modules,
            reserved,
        )?;
        let bindings = bindings
            .iter()
            .map(|(imported, local)| {
                local
                    .clone()
                    .map(|local| (imported.clone(), local))
                    .ok_or_else(|| "host import without a local binding".to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        imports.push((index, bindings));
    }
    stack.pop();
    reserved.extend(analyzed.free);
    let stem = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("host")
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    modules.push(HostModule {
        specifier: crate::module::relative_module_specifier(root_directory, &path),
        stem,
        body: analyzed.body,
        exports: analyzed.exports,
        imports,
    });
    paths.push(path);
    Ok(modules.len() - 1)
}

/// A host module's own relative import, resolved as the linker resolves a
/// foreign import.
fn resolve(parent: &Path, specifier: &str) -> Result<PathBuf, String> {
    if !specifier.starts_with('.') {
        return Err(format!(
            "host module import `{specifier}` names a package; only relative host modules are delivered"
        ));
    }
    let requested = parent.join(specifier);
    if requested.extension().is_some() && requested.is_file() {
        return Ok(requested);
    }
    ["ts", "mts", "js", "mjs"]
        .into_iter()
        .map(|extension| requested.with_extension(extension))
        .find(|candidate| candidate.is_file())
        .ok_or_else(|| format!("cannot resolve host module import `{specifier}`"))
}

/// The first syntax feature the module uses that `edition` lacks.
fn syntax_beyond(
    source: &str,
    edition: crate::js_syntax_target::EcmaScriptEdition,
) -> Result<Option<crate::js_syntax_target::JsSyntaxFeature>, String> {
    use crate::js_syntax_target::JsSyntaxFeature as F;
    let tree = parse_tree(source, false)?;
    let mut found = None;
    walk(&tree, &mut |node| {
        let operator = node.get("operator").and_then(Value::as_str);
        let feature = match kind(node) {
            "ChainExpression" => Some(F::OptionalChain),
            "LogicalExpression" if operator == Some("??") => Some(F::NullishCoalescing),
            "AssignmentExpression" if matches!(operator, Some("??=" | "||=" | "&&=")) => {
                Some(F::LogicalAssignment)
            }
            "PropertyDefinition" | "PrivateIdentifier" | "StaticBlock" => Some(F::ClassFields),
            "AwaitExpression" => Some(F::AsyncAwait),
            "FunctionDeclaration" | "FunctionExpression" | "ArrowFunctionExpression"
                if node.get("async").and_then(Value::as_bool) == Some(true) =>
            {
                Some(F::AsyncAwait)
            }
            "ObjectExpression" | "ObjectPattern"
                if node
                    .get("properties")
                    .and_then(Value::as_array)
                    .is_some_and(|items| {
                        items
                            .iter()
                            .any(|item| matches!(kind(item), "SpreadElement" | "RestElement"))
                    }) =>
            {
                Some(F::ObjectRestSpread)
            }
            "CatchClause" if node.get("param").is_none_or(Value::is_null) => {
                Some(F::OptionalCatchBinding)
            }
            "ImportExpression" => Some(F::DynamicImport),
            _ => None,
        };
        if let Some(feature) = feature.filter(|feature| !edition.allows(*feature)) {
            found.get_or_insert(feature);
        }
        Ok(true)
    })?;
    Ok(found)
}

fn parse_tree(source: &str, typescript: bool) -> Result<Value, String> {
    parse_with(source, typescript, true)
}

/// A delivered module body's ESTree, without grouping parentheses, which
/// change no meaning: `(a.b)()` still calls with `a` as its receiver.
pub(crate) fn parse_program(source: &str) -> Result<Value, String> {
    parse_with(source, false, false)
}

fn parse_with(source: &str, typescript: bool, preserve_parens: bool) -> Result<Value, String> {
    let allocator = oxc_allocator::Allocator::default();
    let source_type = if typescript {
        oxc_span::SourceType::ts().with_module(true)
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(&allocator, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            preserve_parens,
            ..oxc_parser::ParseOptions::default()
        })
        .parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics.first().map_or_else(
            || "the parser could not recover".to_string(),
            ToString::to_string,
        ));
    }
    serde_json::from_str(&parsed.program.to_estree_json(typescript, false))
        .map_err(|error| format!("host module tree: {error}"))
}

fn span(node: &Value) -> Option<(usize, usize)> {
    Some((
        node.get("start")?.as_u64()? as usize,
        node.get("end")?.as_u64()? as usize,
    ))
}

fn kind(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or("")
}

/// Every object node below `node`, depth first, until `visit` declines.
fn walk<'v>(
    node: &'v Value,
    visit: &mut dyn FnMut(&'v Value) -> Result<bool, String>,
) -> Result<(), String> {
    match node {
        Value::Object(fields) => {
            if fields.contains_key("type") && !visit(node)? {
                return Ok(());
            }
            for value in fields.values() {
                walk(value, visit)?;
            }
        }
        Value::Array(items) => {
            for item in items {
                walk(item, visit)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// TypeScript to JavaScript by erasing the spans of type-only syntax.
pub(crate) fn strip_typescript(source: &str) -> Result<String, String> {
    let tree = parse_tree(source, true).map_err(|error| format!("host module: {error}"))?;
    let mut erase: Vec<(usize, usize)> = Vec::new();
    let refuse = |what: &str| {
        Err(format!(
            "host module uses {what}, which has runtime semantics"
        ))
    };
    let mut visit = |node: &Value| -> Result<bool, String> {
        let (start, end) = span(node).ok_or("host module node without a span")?;
        let child = |name: &str| node.get(name).filter(|value| !value.is_null());
        match kind(node) {
            "TSTypeAnnotation"
            | "TSTypeParameterDeclaration"
            | "TSTypeParameterInstantiation"
            | "TSInterfaceDeclaration"
            | "TSTypeAliasDeclaration"
            | "TSDeclareFunction"
            | "TSNamespaceExportDeclaration"
            | "TSIndexSignature" => {
                erase.push((start, end));
                return Ok(false);
            }
            "TSAsExpression" | "TSSatisfiesExpression" | "TSNonNullExpression" => {
                let (_, inner) = child("expression").and_then(span).ok_or("type assertion")?;
                erase.push((inner, end));
            }
            "TSTypeAssertion" => {
                let (inner, _) = child("expression").and_then(span).ok_or("type assertion")?;
                erase.push((start, inner));
            }
            "TSEnumDeclaration" => return refuse("an enum"),
            "TSParameterProperty" => return refuse("a parameter property"),
            "TSImportEqualsDeclaration" | "TSExportAssignment" => {
                return refuse("CommonJS module syntax")
            }
            "TSModuleDeclaration" => {
                if node.get("declare").and_then(Value::as_bool) == Some(true) {
                    erase.push((start, end));
                    return Ok(false);
                }
                return refuse("a namespace");
            }
            "Decorator" => return refuse("a decorator"),
            name if name.starts_with("JSX") => return refuse("JSX"),
            "ImportDeclaration" | "ExportNamedDeclaration" | "ExportAllDeclaration"
                if matches!(
                    node.get("importKind")
                        .or_else(|| node.get("exportKind"))
                        .and_then(Value::as_str),
                    Some("type")
                ) =>
            {
                erase.push((start, end));
                return Ok(false);
            }
            "ExportNamedDeclaration"
                if child("declaration").is_some_and(|declaration| {
                    matches!(
                        kind(declaration),
                        "TSInterfaceDeclaration" | "TSTypeAliasDeclaration" | "TSDeclareFunction"
                    ) || declaration.get("declare").and_then(Value::as_bool) == Some(true)
                }) =>
            {
                erase.push((start, end));
                return Ok(false);
            }
            "VariableDeclaration" | "ClassDeclaration" | "FunctionDeclaration"
                if node.get("declare").and_then(Value::as_bool) == Some(true) =>
            {
                erase.push((start, end));
                return Ok(false);
            }
            "ImportSpecifier" | "ExportSpecifier"
                if matches!(
                    node.get("importKind")
                        .or_else(|| node.get("exportKind"))
                        .and_then(Value::as_str),
                    Some("type")
                ) =>
            {
                return refuse("an inline type-only specifier");
            }
            "Identifier" if node.get("optional").and_then(Value::as_bool) == Some(true) => {
                let name = node.get("name").and_then(Value::as_str).unwrap_or("");
                if name == "this" {
                    return refuse("a `this` parameter");
                }
                let limit = child("typeAnnotation")
                    .and_then(span)
                    .map_or(end, |(at, _)| at);
                let mark = source[start..limit].find('?').ok_or("optional marker")?;
                erase.push((start + mark, start + mark + 1));
            }
            "Identifier"
                if node.get("name").and_then(Value::as_str) == Some("this")
                    && child("typeAnnotation").is_some() =>
            {
                return refuse("a `this` parameter");
            }
            "VariableDeclarator" if node.get("definite").and_then(Value::as_bool) == Some(true) => {
                return refuse("a definite assignment assertion");
            }
            "PropertyDefinition"
            | "MethodDefinition"
            | "AccessorProperty"
            | "TSAbstractPropertyDefinition"
            | "TSAbstractMethodDefinition" => {
                let flagged = ["optional", "definite", "override", "readonly", "declare"]
                    .iter()
                    .any(|flag| node.get(*flag).and_then(Value::as_bool) == Some(true))
                    || node
                        .get("accessibility")
                        .is_some_and(|value| !value.is_null())
                    || kind(node).starts_with("TSAbstract");
                if flagged {
                    return refuse("a class member modifier");
                }
            }
            "ClassDeclaration" | "ClassExpression" => {
                let implements = node
                    .get("implements")
                    .and_then(Value::as_array)
                    .is_some_and(|list| !list.is_empty());
                if implements || node.get("abstract").and_then(Value::as_bool) == Some(true) {
                    return refuse("an implements clause or abstract class");
                }
            }
            _ => {}
        }
        Ok(true)
    };
    walk(&tree, &mut visit)?;
    let mut bytes = source.as_bytes().to_vec();
    for (start, end) in erase {
        let mut at = start;
        while at < end {
            // U+2028 and U+2029 are line terminators; keep all three bytes.
            if bytes[at] == 0xE2
                && at + 2 < end
                && bytes[at + 1] == 0x80
                && matches!(bytes[at + 2], 0xA8 | 0xA9)
            {
                at += 3;
                continue;
            }
            if !matches!(bytes[at], b'\n' | b'\r') {
                bytes[at] = b' ';
            }
            at += 1;
        }
    }
    let stripped = String::from_utf8(bytes).map_err(|_| "host module is not UTF-8".to_string())?;
    parse_tree(&stripped, false)
        .map_err(|error| format!("host module does not strip to JavaScript: {error}"))?;
    Ok(stripped)
}

/// Remove comments and every space and line break not needed to separate
/// two tokens. Where automatic semicolon insertion ended a statement, an
/// explicit `;` takes the line break's place (none before `}` or at the
/// end), so the text is one line. The result must parse to the same tree.
pub(crate) fn compact(source: &str) -> Result<String, String> {
    let allocator = oxc_allocator::Allocator::default();
    let parsed = oxc_parser::Parser::new(&allocator, source, oxc_span::SourceType::mjs())
        .with_config(oxc_parser::config::TokensParserConfig)
        .parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return Err("host module does not parse as a JavaScript module".to_string());
    }
    let original = parse_tree(source, false)?;
    let mut semicolons = Vec::new();
    implicit_semicolons(&original, source, false, &mut semicolons);
    semicolons.sort_unstable();
    semicolons.dedup();
    let mut pending = semicolons.into_iter().peekable();
    let mut text = String::with_capacity(source.len());
    let mut previous: Option<(&str, bool)> = None;
    for token in parsed.tokens.iter() {
        if token.kind() == oxc_parser::Kind::Eof {
            break;
        }
        let start = token.start() as usize;
        let piece = &source[start..token.end() as usize];
        if piece.is_empty() {
            continue;
        }
        let mut ended = false;
        while pending.next_if(|&at| at <= start).is_some() {
            ended = true;
        }
        if ended && previous.is_some() && piece != "}" && piece != ";" {
            text.push(';');
            previous = Some((";", false));
        }
        if let Some((last, number)) = previous {
            if separated(last, piece, number) {
                text.push(' ');
            }
        }
        text.push_str(piece);
        previous = Some((piece, token.kind().is_number()));
    }
    let compacted = parse_tree(&text, false)
        .map_err(|error| format!("compacted host module does not parse: {error}"))?;
    if without_positions(original) != without_positions(compacted) {
        return Err("compacting a host module changed its tree".to_string());
    }
    Ok(text)
}

/// The end of every statement whose `;` automatic semicolon insertion
/// supplied. A `for` head's declaration is followed by the head's own `;`.
fn implicit_semicolons(node: &Value, source: &str, head: bool, ends: &mut Vec<usize>) {
    match node {
        Value::Array(items) => {
            for item in items {
                implicit_semicolons(item, source, head, ends);
            }
        }
        Value::Object(fields) => {
            let terminated = matches!(
                kind(node),
                "ExpressionStatement"
                    | "ReturnStatement"
                    | "ThrowStatement"
                    | "BreakStatement"
                    | "ContinueStatement"
                    | "DebuggerStatement"
                    | "DoWhileStatement"
                    | "ImportDeclaration"
                    | "ExportAllDeclaration"
                    | "PropertyDefinition"
            ) || (kind(node) == "VariableDeclaration" && !head)
                || (kind(node) == "ExportNamedDeclaration"
                    && fields.get("declaration").is_none_or(Value::is_null))
                || (kind(node) == "ExportDefaultDeclaration"
                    && !matches!(
                        fields.get("declaration").map(kind),
                        Some("FunctionDeclaration" | "ClassDeclaration")
                    ));
            if terminated {
                if let Some((_, end)) = span(node) {
                    if !source[..end].trim_end().ends_with(';') {
                        ends.push(end);
                    }
                }
            }
            let loop_head = matches!(
                kind(node),
                "ForStatement" | "ForInStatement" | "ForOfStatement"
            );
            for (key, value) in fields {
                let head = loop_head && matches!(key.as_str(), "init" | "left");
                implicit_semicolons(value, source, head, ends);
            }
        }
        _ => {}
    }
}

/// Whether two adjacent tokens need a space to stay two tokens.
fn separated(last: &str, next: &str, number: bool) -> bool {
    let word = |c: char| c.is_alphanumeric() || c == '_' || c == '$' || c == '\\' || !c.is_ascii();
    let tail = last.chars().next_back().unwrap_or(' ');
    let head = next.chars().next().unwrap_or(' ');
    (word(tail) && word(head))
        || (tail == '+' && head == '+')
        || (tail == '-' && (head == '-' || head == '>'))
        || (tail == '/' && (head == '/' || head == '*'))
        || (tail == '<' && head == '!')
        || (number && head == '.')
}

fn without_positions(mut value: Value) -> Value {
    fn strip(value: &mut Value) {
        match value {
            Value::Object(fields) => {
                fields.remove("start");
                fields.remove("end");
                fields.remove("range");
                for field in fields.values_mut() {
                    strip(field);
                }
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    strip(&mut value);
    value
}

struct Analyzed {
    /// Compacted body without imports and `export` keywords.
    body: String,
    exports: Vec<(String, String)>,
    /// Relative imports: specifier, then imported (None: namespace) and local.
    imports: Vec<(String, Vec<(Option<String>, Option<String>)>)>,
    free: Vec<String>,
}

fn name(node: &Value) -> Option<String> {
    match kind(node) {
        "Identifier" => node.get("name").and_then(Value::as_str).map(str::to_string),
        "Literal" => node
            .get("value")
            .and_then(Value::as_str)
            .map(str::to_string),
        _ => None,
    }
}

/// The module's imports and exports, and its body without them.
fn analyze(source: &str) -> Result<Analyzed, String> {
    let tree = parse_tree(source, false)?;
    let body = tree
        .get("body")
        .and_then(Value::as_array)
        .ok_or("host module has no body")?;
    let mut erase = Vec::new();
    let mut exports = Vec::new();
    let mut imports: Vec<(String, Vec<(Option<String>, Option<String>)>)> = Vec::new();
    let mut declared_kinds = std::collections::BTreeMap::new();
    for statement in body {
        let (start, end) = span(statement).ok_or("host statement span")?;
        match kind(statement) {
            "VariableDeclaration" => {
                let variable = statement.get("kind").and_then(Value::as_str).unwrap_or("");
                for declarator in statement
                    .get("declarations")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    if let Some(local) = declarator.get("id").and_then(name) {
                        declared_kinds.insert(local, variable.to_string());
                    }
                }
            }
            "ImportDeclaration" => {
                let specifier = statement
                    .get("source")
                    .and_then(|source| source.get("value"))
                    .and_then(Value::as_str)
                    .ok_or("host import source")?
                    .to_string();
                let mut bindings = Vec::new();
                for item in statement
                    .get("specifiers")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                {
                    let local = item.get("local").and_then(name);
                    match kind(item) {
                        "ImportSpecifier" => {
                            bindings.push((item.get("imported").and_then(name), local))
                        }
                        "ImportNamespaceSpecifier" => bindings.push((None, local)),
                        _ => return Err("host module uses a default import".to_string()),
                    }
                }
                imports.push((specifier, bindings));
                erase.push((start, end));
            }
            "ExportNamedDeclaration" => {
                if statement
                    .get("source")
                    .is_some_and(|source| !source.is_null())
                {
                    return Err("host module re-exports another module".to_string());
                }
                if let Some(declaration) = statement
                    .get("declaration")
                    .filter(|value| !value.is_null())
                {
                    let (inner, _) = span(declaration).ok_or("host export span")?;
                    erase.push((start, inner));
                    match kind(declaration) {
                        "FunctionDeclaration" | "ClassDeclaration" => {
                            let local = declaration
                                .get("id")
                                .and_then(name)
                                .ok_or("host export name")?;
                            exports.push((local.clone(), local));
                        }
                        "VariableDeclaration" => {
                            if declaration.get("kind").and_then(Value::as_str) != Some("const") {
                                return Err("host module exports a mutable binding".to_string());
                            }
                            for declarator in declaration
                                .get("declarations")
                                .and_then(Value::as_array)
                                .into_iter()
                                .flatten()
                            {
                                let local = declarator
                                    .get("id")
                                    .and_then(name)
                                    .ok_or("host module exports a destructuring pattern")?;
                                exports.push((local.clone(), local));
                            }
                        }
                        _ => {
                            return Err("host module exports an unsupported declaration".to_string())
                        }
                    }
                } else {
                    for item in statement
                        .get("specifiers")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                    {
                        let local = item
                            .get("local")
                            .and_then(name)
                            .ok_or("host export local")?;
                        let exported = item
                            .get("exported")
                            .and_then(name)
                            .ok_or("host export name")?;
                        exports.push((exported, local));
                    }
                    erase.push((start, end));
                }
            }
            "ExportDefaultDeclaration" | "ExportAllDeclaration" => {
                return Err("host module uses a default or star export".to_string())
            }
            _ => {}
        }
    }
    for (_, local) in &exports {
        if declared_kinds
            .get(local)
            .is_some_and(|kind| kind != "const")
        {
            return Err("host module exports a mutable binding".to_string());
        }
    }
    // Identifiers read without a declaration anywhere in the module; a name
    // declared in any scope is taken as declared, which only reserves less.
    let mut declared = std::collections::BTreeSet::new();
    let mut referenced = std::collections::BTreeSet::new();
    references(&tree, &mut declared, &mut referenced)?;
    let free = referenced
        .into_iter()
        .filter(|name| !declared.contains(name))
        .collect();
    let mut bytes = source.as_bytes().to_vec();
    for (start, end) in erase {
        for byte in &mut bytes[start..end] {
            if !matches!(*byte, b'\n' | b'\r') {
                *byte = b' ';
            }
        }
    }
    let body = String::from_utf8(bytes).map_err(|_| "host module is not UTF-8".to_string())?;
    let body = compact(&body)?;
    Ok(Analyzed {
        body,
        exports,
        imports,
        free,
    })
}

/// Every identifier `node` reads, and every name it declares. Member
/// names, property and method keys, labels and export spellings are not
/// reads.
fn references(
    node: &Value,
    declared: &mut std::collections::BTreeSet<String>,
    referenced: &mut std::collections::BTreeSet<String>,
) -> Result<(), String> {
    let items = match node {
        Value::Array(items) => {
            for item in items {
                references(item, declared, referenced)?;
            }
            return Ok(());
        }
        Value::Object(fields) => fields,
        _ => return Ok(()),
    };
    let computed = items.get("computed").and_then(Value::as_bool) == Some(true);
    let skip: &[&str] = match kind(node) {
        "MetaProperty" => return Err("host module reads import.meta".to_string()),
        "Identifier" => {
            if let Some(found) = name(node) {
                referenced.insert(found);
            }
            return Ok(());
        }
        "FunctionDeclaration" | "FunctionExpression" | "ClassDeclaration" | "ClassExpression" => {
            collect_pattern(items.get("id"), declared);
            &[]
        }
        "VariableDeclarator" => {
            collect_pattern(items.get("id"), declared);
            &[]
        }
        "CatchClause" => {
            collect_pattern(items.get("param"), declared);
            &[]
        }
        "MemberExpression" if !computed => &["property"],
        "Property" | "MethodDefinition" | "PropertyDefinition" | "AccessorProperty"
            if !computed =>
        {
            &["key"]
        }
        "LabeledStatement" | "BreakStatement" | "ContinueStatement" => &["label"],
        "ImportSpecifier" | "ImportNamespaceSpecifier" | "ImportDefaultSpecifier" => {
            collect_pattern(items.get("local"), declared);
            return Ok(());
        }
        "ExportSpecifier" => &["exported"],
        _ => &[],
    };
    if let Some(params) = items.get("params").and_then(Value::as_array) {
        for param in params {
            collect_pattern(Some(param), declared);
        }
    }
    for (key, value) in items {
        if !skip.contains(&key.as_str()) {
            references(value, declared, referenced)?;
        }
    }
    Ok(())
}

fn collect_pattern(node: Option<&Value>, declared: &mut std::collections::BTreeSet<String>) {
    let Some(node) = node.filter(|node| !node.is_null()) else {
        return;
    };
    let _ = walk(node, &mut |inner| {
        match kind(inner) {
            "Identifier" => {
                if let Some(found) = name(inner) {
                    declared.insert(found);
                }
            }
            // Default values and computed keys are expressions, not names.
            "AssignmentPattern" => {
                collect_pattern(inner.get("left"), declared);
                return Ok(false);
            }
            "Property" if inner.get("computed").and_then(Value::as_bool) != Some(true) => {
                collect_pattern(inner.get("value"), declared);
                return Ok(false);
            }
            _ => {}
        }
        Ok(true)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typescript_strips_to_erased_spans_and_keeps_line_breaks() {
        let source = "export function f<T>(a: any, b?: number): T {\n  return a as T\n}\nexport const x: number = 1!;\ninterface I { a: string }\nexport type U = I;\n";
        let stripped = strip_typescript(source).unwrap();
        assert_eq!(stripped.len(), source.len());
        assert_eq!(stripped.lines().count(), source.lines().count());
        assert!(
            !stripped.contains(':')
                && !stripped.contains("as T")
                && !stripped.contains("interface")
        );
        let compacted = compact(&stripped).unwrap();
        assert_eq!(
            compacted,
            "export function f(a,b){return a}export const x=1;"
        );
    }

    #[test]
    fn runtime_typescript_is_refused() {
        for source in [
            "export enum E { A }",
            "export class C { constructor(private a: number) {} }",
            "namespace N { export const a = 1 }",
        ] {
            assert!(strip_typescript(source).is_err(), "{source}");
        }
    }

    #[test]
    fn compaction_keeps_the_tokens_apart_that_need_it() {
        let source = "let a = 1, b = a + +a - -a\nlet c = a / /x/.source.length\nlet d = 1 .toString()\nreturn_: for (const e of [a]) { break return_ }\nlet f = `x${ a }y`\nlet g = a\n++b";
        let compacted = compact(source).unwrap();
        assert!(compacted.contains("a+ +a- -a;"), "{compacted}");
        assert!(compacted.contains("a/ /x/"), "{compacted}");
        assert!(compacted.contains("1 .toString"), "{compacted}");
        assert!(compacted.ends_with("let g=a;++b"), "{compacted}");
        assert!(!compacted.contains('\n'), "{compacted}");
        // A restricted production keeps its meaning.
        assert_eq!(
            compact("function f(){return\n1}").unwrap(),
            "function f(){return;1}"
        );
        assert_eq!(
            compact("for (let i = 0\n; i < 1\n; i++) {}").unwrap(),
            "for(let i=0;i<1;i++){}"
        );
    }

    #[test]
    fn host_modules_become_one_scoped_expression() {
        let directory = std::env::temp_dir().join(format!("lilscript-host-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("math.ts"), "export function sum(left: number, right: number): number {\n  return left + right\n}\n").unwrap();
        std::fs::write(directory.join("host.ts"), "import { sum } from \"./math.ts\";\n// a comment\nexport function add(left: number, right: number): number {\n  return sum(left, right)\n}\nexport const ZERO: number = 0\n").unwrap();
        let delivery = deliver(
            &directory,
            &[directory.join("host.ts")],
            crate::js_syntax_target::EcmaScriptEdition::Es2022,
        )
        .unwrap();
        assert_eq!(delivery.modules.len(), 2);
        assert_eq!(delivery.position("./host.ts"), Some(1));
        assert!(delivery.reserved.is_empty(), "{:?}", delivery.reserved);
        let free = analyze(
            "let a = window.document.body; a.b.c(Array.isArray(a) ? 1 : 2); x: for (;;) break x",
        )
        .unwrap()
        .free;
        assert_eq!(free, ["Array", "window"]);
        let expression = delivery.expression(false);
        assert_eq!(
            expression,
            "(()=>{let h0=(()=>{function sum(left,right){return left+right};return{sum}})(),\
             h1=(()=>{const{sum}=h0;function add(left,right){return sum(left,right)}const ZERO=0;return{add,ZERO}})();\
             return[h0,h1]})()"
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
