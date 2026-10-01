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

use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use oxc_ast::{AstKind, ast};
use oxc_ast_visit::Visit;
use oxc_span::{ContentEq, GetSpan};
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
    reads: Vec<HostRead>,
    /// Exported names and the local bindings they export.
    exports: Vec<(String, String)>,
    volatile: Vec<String>,
    /// Other host modules this one imports: index, then each imported name
    /// (`None` for a namespace import) and its local binding.
    imports: Vec<(usize, Vec<(Option<String>, String)>)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HostRead {
    start: usize,
    end: usize,
    import: usize,
    binding: usize,
    shorthand: bool,
}

impl HostModule {
    fn write_import(
        &self,
        import: usize,
        binding: usize,
        names: &[String],
        text: &mut String,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let (source, bindings) = &self.imports[import];
        let imported = bindings[binding].0.as_ref().expect("named import read");
        budget.push_str(Retained, text, &names[*source])?;
        budget.push_char(Retained, text, '[')?;
        let quoted = serde_json::to_string(imported).expect("string key");
        budget.push_str(Retained, text, &quoted)?;
        budget.push_char(Retained, text, ']')
    }
    fn write_body(
        &self,
        names: &[String],
        text: &mut String,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let mut cursor = 0;
        for read in &self.reads {
            budget.push_str(Retained, text, &self.body[cursor..read.start])?;
            if read.shorthand {
                budget.push_str(Retained, text, &self.body[read.start..read.end])?;
                budget.push_char(Retained, text, ':')?;
            }
            // Imported function calls are unbound, including tagged calls.
            budget.push_str(Retained, text, "(0,")?;
            self.write_import(read.import, read.binding, names, text, budget)?;
            budget.push_char(Retained, text, ')')?;
            cursor = read.end;
        }
        budget.push_str(Retained, text, &self.body[cursor..])
    }
    fn write_exports(
        &self,
        names: &[String],
        text: &mut String,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        for (position, (exported, local)) in self.exports.iter().enumerate() {
            if position != 0 {
                budget.push_char(Retained, text, ',')?;
            }
            budget.push_str(Retained, text, "get ")?;
            budget.push_str(
                Retained,
                text,
                &serde_json::to_string(exported).expect("export key"),
            )?;
            budget.push_str(Retained, text, "(){return ")?;
            let imported = self
                .imports
                .iter()
                .enumerate()
                .find_map(|(import, (_, bindings))| {
                    bindings
                        .iter()
                        .position(|(name, binding)| name.is_some() && binding == local)
                        .map(|binding| (import, binding))
                });
            if let Some((import, binding)) = imported {
                self.write_import(import, binding, names, text, budget)?;
            } else {
                budget.push_str(Retained, text, local)?;
            }
            budget.push_char(Retained, text, '}')?;
        }
        Ok(())
    }

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
            bytes += text(&module.specifier)
                + text(&module.stem)
                + text(&module.body)
                + (module.reads.capacity() * std::mem::size_of::<HostRead>()) as u64;
            bytes += (module.volatile.capacity() * std::mem::size_of::<String>()) as u64;
            bytes += module.volatile.iter().map(text).sum::<u64>();
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
        self.expression_in(strict, &mut AllocationBudget::new(None))
            .expect("host expression")
    }
    pub(crate) fn expression_in(
        &self,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<String, AllocationError> {
        self.expression_selected_in(strict, 0..self.modules.len(), budget)
    }
    pub(crate) fn expression_selected_in(
        &self, strict: bool, requested: impl Iterator<Item=usize>, budget: &mut AllocationBudget<'_>,
    ) -> Result<String, AllocationError> {
        budget.retained_phase(|budget| {
            let mut selected = budget.filled(Scratch, self.modules.len(), false)?;
            let mut pending = Vec::new();
            let mut order = Vec::new();
            for root in requested {
                budget.push(Scratch, &mut pending, (root, false))?;
                while let Some((index, finish)) = pending.pop() {
                    budget.work(WorkKind::Analysis, 1)?;
                    if finish { budget.push(Scratch, &mut order, index)?; continue; }
                    if std::mem::replace(&mut selected[index], true) { continue; }
                    budget.push(Scratch, &mut pending, (index, true))?;
                    for &(dependency, _) in self.modules[index].imports.iter().rev() { budget.push(Scratch, &mut pending, (dependency, false))?; }
                }
            }
            let names = self.scope_names_in(budget)?;
            let mut text = budget.string(Retained, "(()=>{")?;
            if strict {
                budget.push_str(Retained, &mut text, "\"use strict\";")?;
            }
            budget.push_str(Retained, &mut text, "let ")?;
            let mut comma = false;
            for &index in &order {
                let module = &self.modules[index];
                if std::mem::replace(&mut comma, true) { budget.push_char(Retained, &mut text, ',')?; }
                budget.push_str(Retained, &mut text, &names[index])?;
                budget.push_str(Retained, &mut text, "=(function(){")?;
                for (source, bindings) in &module.imports {
                    for (imported, local) in bindings {
                        match imported {
                            None => {
                                budget.push_str(Retained, &mut text, "const ")?;
                                budget.push_str(Retained, &mut text, local)?;
                                budget.push_char(Retained, &mut text, '=')?;
                                budget.push_str(Retained, &mut text, &names[*source])?;
                                budget.push_char(Retained, &mut text, ';')?;
                            }
                            Some(_) => {}
                        }
                    }
                }
                module.write_body(&names, &mut text, budget)?;
                budget.push_str(Retained, &mut text, ";return{")?;
                module.write_exports(&names, &mut text, budget)?;
                budget.push_str(Retained, &mut text, "}})()")?;
            }
            budget.push_str(Retained, &mut text, ";return[")?;
            for index in 0..self.modules.len() {
                if index != 0 {
                    budget.push_char(Retained, &mut text, ',')?;
                }
                if selected[index] { budget.push_str(Retained, &mut text, &names[index])?; }
                else { budget.push_str(Retained, &mut text, "void 0")?; }
            }
            budget.push_str(Retained, &mut text, "]})()")?;
            drop((selected, pending, order));
            Ok(text)
        })
    }

    /// With one module that imports no other, `(()=>{...;return{...}})()`:
    /// its namespace object directly.
    pub fn single(&self, strict: bool) -> Option<String> {
        self.single_in(strict, &mut AllocationBudget::new(None))
            .expect("single host expression")
    }
    pub(crate) fn single_in(
        &self,
        strict: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<String>, AllocationError> {
        let [module] = self.modules.as_slice() else {
            return Ok(None);
        };
        if !module.imports.is_empty() {
            return Ok(None);
        }
        budget.retained_phase(|budget| {
            let mut text = budget.string(Retained, "(function(){")?;
            if strict {
                budget.push_str(Retained, &mut text, "\"use strict\";")?;
            }
            budget.push_str(Retained, &mut text, &module.body)?;
            budget.push_str(Retained, &mut text, ";return{")?;
            module.write_exports(&[], &mut text, budget)?;
            budget.push_str(Retained, &mut text, "}})()")?;
            Ok(Some(text))
        })
    }

    /// Scope-local names for each module's namespace, distinct from every
    /// identifier host code spells.
    fn scope_names_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<String>, AllocationError> {
        let mut names = budget.vector(Scratch, self.modules.len())?;
        let mut counter = 0usize;
        while names.len() < self.modules.len() {
            let candidate = budget.format(Scratch, format_args!("h{counter}"))?;
            let scan = self.reserved.iter().map(String::len).sum::<usize>()
                + self
                    .modules
                    .iter()
                    .map(|module| {
                        module.body.len()
                            + module
                                .imports
                                .iter()
                                .flat_map(|(_, bindings)| bindings)
                                .map(|(_, local)| local.len())
                                .sum::<usize>()
                    })
                    .sum::<usize>();
            budget.work(WorkKind::Render, scan as u64)?;
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
            } else {
                let bytes = candidate.capacity() as u64;
                drop(candidate);
                budget.release(Scratch, bytes)?;
            }
        }
        Ok(names)
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
        let root = visit(
            root_directory,
            request,
            edition,
            &mut Vec::new(),
            &mut paths,
            &mut delivery.modules,
            &mut reserved,
        )?;
        // This boundary still binds carried imports as local values. A
        // mutable public host binding stays an external module (auto), or
        // receives the existing explicit embedding diagnostic. Internal host
        // dependencies use the live namespace read recipe below.
        if !delivery.modules[root].volatile.is_empty() {
            return Err("host module exports a mutable binding across the embedded source boundary; retain this module as external".into());
        }
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
            ));
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
    let mut volatile = analyzed.volatile;
    for (exported, local) in &analyzed.exports {
        for (source, bindings) in &imports {
            for (imported, binding) in bindings {
                if binding == local
                    && imported
                        .as_ref()
                        .is_some_and(|name| modules[*source].volatile.contains(name))
                {
                    volatile.push(exported.clone());
                }
            }
        }
    }
    volatile.sort_unstable();
    volatile.dedup();
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
        reads: analyzed.reads,
        exports: analyzed.exports,
        volatile,
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
    struct Features {
        edition: crate::js_syntax_target::EcmaScriptEdition,
        found: Option<F>,
    }
    impl<'a> Visit<'a> for Features {
        fn enter_node(&mut self, node: AstKind<'a>) {
            let feature = match node {
                AstKind::ChainExpression(_) => Some(F::OptionalChain),
                AstKind::LogicalExpression(node) if node.operator.as_str() == "??" => {
                    Some(F::NullishCoalescing)
                }
                AstKind::AssignmentExpression(node)
                    if matches!(node.operator.as_str(), "??=" | "||=" | "&&=") =>
                {
                    Some(F::LogicalAssignment)
                }
                AstKind::PropertyDefinition(_)
                | AstKind::PrivateIdentifier(_)
                | AstKind::StaticBlock(_)
                | AstKind::AccessorProperty(_) => Some(F::ClassFields),
                AstKind::AwaitExpression(_) => Some(F::AsyncAwait),
                AstKind::Function(node) if node.r#async => Some(F::AsyncAwait),
                AstKind::ArrowFunctionExpression(node) if node.r#async => Some(F::AsyncAwait),
                AstKind::ObjectExpression(node)
                    if node
                        .properties
                        .iter()
                        .any(|item| matches!(item, ast::ObjectPropertyKind::SpreadProperty(_))) =>
                {
                    Some(F::ObjectRestSpread)
                }
                AstKind::ObjectPattern(node) if node.rest.is_some() => Some(F::ObjectRestSpread),
                AstKind::ObjectAssignmentTarget(node) if node.rest.is_some() => {
                    Some(F::ObjectRestSpread)
                }
                AstKind::CatchClause(node) if node.param.is_none() => Some(F::OptionalCatchBinding),
                AstKind::ImportExpression(_) => Some(F::DynamicImport),
                _ => None,
            };
            if let Some(feature) = feature.filter(|feature| !self.edition.allows(*feature)) {
                self.found.get_or_insert(feature);
            }
        }
    }
    let arena = oxc_allocator::Allocator::default();
    let tree = parse_with(&arena, source, false, true)?;
    let mut features = Features {
        edition,
        found: None,
    };
    features.visit_program(&tree);
    Ok(features.found)
}

/// A borrowed typed AST. The caller owns the arena for the entire visit;
/// host lowering never serializes/reparses an intermediate ESTree tree.
pub(crate) fn parse_program<'a>(
    arena: &'a oxc_allocator::Allocator,
    source: &'a str,
) -> Result<ast::Program<'a>, String> {
    parse_with(arena, source, false, false)
}
fn parse_with<'a>(
    arena: &'a oxc_allocator::Allocator,
    source: &'a str,
    typescript: bool,
    preserve_parens: bool,
) -> Result<ast::Program<'a>, String> {
    let source_type = if typescript {
        oxc_span::SourceType::ts().with_module(true)
    } else {
        oxc_span::SourceType::mjs()
    };
    let parsed = oxc_parser::Parser::new(arena, source, source_type)
        .with_options(oxc_parser::ParseOptions {
            parse_regular_expression: true,
            preserve_parens,
            ..Default::default()
        })
        .parse();
    if parsed.panicked || !parsed.diagnostics.is_empty() {
        return Err(parsed.diagnostics.first().map_or_else(
            || "the parser could not recover".to_string(),
            ToString::to_string,
        ));
    }
    Ok(parsed.program)
}

/// Type-only spans are blanked without changing line terminators. Runtime
/// TypeScript still requires a lowering and receives an explicit diagnostic.
pub(crate) fn strip_typescript(source: &str) -> Result<String, String> {
    struct Erase<'s> {
        source: &'s str,
        spans: Vec<(usize, usize)>,
        error: Option<String>,
        erased_until: u32,
    }
    impl Erase<'_> {
        fn refuse(&mut self, what: &str) {
            self.error.get_or_insert_with(|| {
                format!("host module uses {what}, which has runtime semantics")
            });
        }
        fn erase(&mut self, span: oxc_span::Span) {
            self.spans.push((span.start as usize, span.end as usize));
            self.erased_until = span.end;
        }
    }
    impl<'a> Visit<'a> for Erase<'_> {
        fn enter_node(&mut self, node: AstKind<'a>) {
            let span = node.span();
            if self.error.is_some()
                || (span.start < self.erased_until && span.end <= self.erased_until)
            {
                return;
            }
            match node {
                AstKind::TSTypeAnnotation(_)
                | AstKind::TSTypeParameterDeclaration(_)
                | AstKind::TSTypeParameterInstantiation(_)
                | AstKind::TSInterfaceDeclaration(_)
                | AstKind::TSTypeAliasDeclaration(_)
                | AstKind::TSNamespaceExportDeclaration(_)
                | AstKind::TSIndexSignature(_) => self.erase(span),
                AstKind::TSAsExpression(node) => self
                    .spans
                    .push((node.expression.span().end as usize, span.end as usize)),
                AstKind::TSSatisfiesExpression(node) => self
                    .spans
                    .push((node.expression.span().end as usize, span.end as usize)),
                AstKind::TSNonNullExpression(node) => self
                    .spans
                    .push((node.expression.span().end as usize, span.end as usize)),
                AstKind::TSTypeAssertion(node) => self
                    .spans
                    .push((span.start as usize, node.expression.span().start as usize)),
                AstKind::TSEnumDeclaration(_) => self.refuse("an enum"),
                AstKind::FormalParameter(node)
                    if node.accessibility.is_some() || node.readonly || node.r#override =>
                {
                    self.refuse("a parameter property")
                }
                AstKind::TSImportEqualsDeclaration(_) | AstKind::TSExportAssignment(_) => {
                    self.refuse("CommonJS module syntax")
                }
                AstKind::TSModuleDeclaration(node) if node.declare => self.erase(span),
                AstKind::TSGlobalDeclaration(_) => self.erase(span),
                AstKind::TSModuleDeclaration(_) => self.refuse("a namespace"),
                AstKind::Decorator(_) => self.refuse("a decorator"),
                AstKind::JSXElement(_) | AstKind::JSXFragment(_) => self.refuse("JSX"),
                AstKind::ImportDeclaration(node)
                    if node.import_kind == ast::ImportOrExportKind::Type =>
                {
                    self.erase(span)
                }
                AstKind::ExportNamedDeclaration(node)
                    if node.export_kind == ast::ImportOrExportKind::Type =>
                {
                    self.erase(span)
                }
                AstKind::ExportFromDeclaration(node)
                    if node.export_kind == ast::ImportOrExportKind::Type =>
                {
                    self.erase(span)
                }
                AstKind::ExportAllDeclaration(node)
                    if node.export_kind == ast::ImportOrExportKind::Type =>
                {
                    self.erase(span)
                }
                AstKind::ExportDeclaration(node) if erased_declaration(&node.declaration) => {
                    self.erase(span)
                }
                AstKind::VariableDeclaration(node) if node.declare => self.erase(span),
                AstKind::Class(node) if node.declare => self.erase(span),
                AstKind::Function(node) if node.declare || node.body.is_none() => self.erase(span),
                AstKind::ImportSpecifier(node)
                    if node.import_kind == ast::ImportOrExportKind::Type =>
                {
                    self.refuse("an inline type-only specifier")
                }
                AstKind::ExportSpecifier(node)
                    if node.export_kind == ast::ImportOrExportKind::Type =>
                {
                    self.refuse("an inline type-only specifier")
                }
                AstKind::Function(node) if node.this_param.is_some() => {
                    self.refuse("a `this` parameter")
                }
                AstKind::FormalParameter(node) if node.optional => {
                    let start = node.pattern.span().end as usize;
                    let end =
                        node.type_annotation
                            .as_ref()
                            .map_or(span.end, |ty| ty.span.start) as usize;
                    if let Some(mark) = self.source[start..end].find('?') {
                        self.spans.push((start + mark, start + mark + 1));
                    } else {
                        self.error = Some("host module optional marker".into());
                    }
                }
                AstKind::VariableDeclarator(node) if node.definite => {
                    self.refuse("a definite assignment assertion")
                }
                AstKind::PropertyDefinition(node)
                    if node.optional
                        || node.definite
                        || node.r#override
                        || node.readonly
                        || node.declare
                        || node.accessibility.is_some()
                        || node.r#type
                            == ast::PropertyDefinitionType::TSAbstractPropertyDefinition =>
                {
                    self.refuse("a class member modifier")
                }
                AstKind::MethodDefinition(node)
                    if node.optional
                        || node.r#override
                        || node.accessibility.is_some()
                        || node.r#type == ast::MethodDefinitionType::TSAbstractMethodDefinition =>
                {
                    self.refuse("a class member modifier")
                }
                AstKind::AccessorProperty(node)
                    if node.definite
                        || node.r#override
                        || node.accessibility.is_some()
                        || node.r#type == ast::AccessorPropertyType::TSAbstractAccessorProperty =>
                {
                    self.refuse("a class member modifier")
                }
                AstKind::Class(node) if !node.implements.is_empty() || node.r#abstract => {
                    self.refuse("an implements clause or abstract class")
                }
                _ => {}
            }
        }
    }
    let arena = oxc_allocator::Allocator::default();
    let tree =
        parse_with(&arena, source, true, true).map_err(|error| format!("host module: {error}"))?;
    let mut erase = Erase {
        source,
        spans: Vec::new(),
        error: None,
        erased_until: 0,
    };
    erase.visit_program(&tree);
    if let Some(error) = erase.error {
        return Err(error);
    }
    let stripped = erase_spans(source, &erase.spans)?;
    parse_with(&arena, &stripped, false, true)
        .map_err(|error| format!("host module does not strip to JavaScript: {error}"))?;
    Ok(stripped)
}
fn erased_declaration(node: &ast::Declaration<'_>) -> bool {
    match node {
        ast::Declaration::TSInterfaceDeclaration(_)
        | ast::Declaration::TSTypeAliasDeclaration(_) => true,
        ast::Declaration::VariableDeclaration(node) => node.declare,
        ast::Declaration::ClassDeclaration(node) => node.declare,
        ast::Declaration::FunctionDeclaration(node) => node.declare || node.body.is_none(),
        ast::Declaration::TSModuleDeclaration(node) => node.declare,
        _ => false,
    }
}
fn erase_spans(source: &str, spans: &[(usize, usize)]) -> Result<String, String> {
    let mut bytes = source.as_bytes().to_vec();
    for &(start, end) in spans {
        let mut at = start;
        while at < end {
            if bytes[at] == 0xe2
                && at + 2 < end
                && bytes[at + 1] == 0x80
                && matches!(bytes[at + 2], 0xa8 | 0xa9)
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
    String::from_utf8(bytes).map_err(|_| "host module is not UTF-8".into())
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
    let original = parse_with(&allocator, source, false, true)?;
    let mut semicolons = Vec::new();
    implicit_semicolons(&original, source, &mut semicolons);
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
    let compacted = parse_with(&allocator, &text, false, true)
        .map_err(|error| format!("compacted host module does not parse: {error}"))?;
    if !original.content_eq(&compacted) {
        return Err("compacting a host module changed its tree".to_string());
    }
    Ok(text)
}

/// The end of every statement whose `;` automatic semicolon insertion
/// supplied. A `for` head's declaration is followed by the head's own `;`.
fn implicit_semicolons(program: &ast::Program<'_>, source: &str, ends: &mut Vec<usize>) {
    struct Semicolons<'s> {
        source: &'s str,
        ends: &'s mut Vec<usize>,
        heads: Vec<oxc_span::Span>,
    }
    impl<'a> Visit<'a> for Semicolons<'_> {
        fn enter_node(&mut self, node: AstKind<'a>) {
            let span = node.span();
            match node {
                AstKind::ForStatement(node) => {
                    if let Some(ast::ForStatementInit::VariableDeclaration(node)) = &node.init {
                        self.heads.push(node.span);
                    }
                }
                AstKind::ForInStatement(node) => {
                    if let ast::ForStatementLeft::VariableDeclaration(node) = &node.left {
                        self.heads.push(node.span);
                    }
                }
                AstKind::ForOfStatement(node) => {
                    if let ast::ForStatementLeft::VariableDeclaration(node) = &node.left {
                        self.heads.push(node.span);
                    }
                }
                _ => {}
            }
            let terminated = match node {
                AstKind::Directive(_)
                | AstKind::ExpressionStatement(_)
                | AstKind::ReturnStatement(_)
                | AstKind::ThrowStatement(_)
                | AstKind::BreakStatement(_)
                | AstKind::ContinueStatement(_)
                | AstKind::DebuggerStatement(_)
                | AstKind::DoWhileStatement(_)
                | AstKind::ImportDeclaration(_)
                | AstKind::ExportAllDeclaration(_)
                | AstKind::ExportFromDeclaration(_)
                | AstKind::PropertyDefinition(_)
                | AstKind::ExportNamedDeclaration(_) => true,
                AstKind::VariableDeclaration(_) => !self.heads.contains(&span),
                AstKind::ExportDefaultDeclaration(node) => {
                    node.declaration.as_expression().is_some()
                }
                _ => false,
            };
            if terminated && !self.source[..span.end as usize].trim_end().ends_with(';') {
                self.ends.push(span.end as usize);
            }
        }
    }
    Semicolons {
        source,
        ends,
        heads: Vec::new(),
    }
    .visit_program(program);
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

struct Analyzed {
    /// Compacted body without imports and `export` keywords.
    body: String,
    reads: Vec<HostRead>,
    exports: Vec<(String, String)>,
    volatile: Vec<String>,
    /// Relative imports: specifier, then imported (None: namespace) and local.
    imports: Vec<(String, Vec<(Option<String>, Option<String>)>)>,
    free: Vec<String>,
}

fn export_name(node: &ast::ModuleExportName<'_>) -> Result<String, String> {
    match node {
        ast::ModuleExportName::IdentifierName(node) => Ok(node.name.to_string()),
        ast::ModuleExportName::IdentifierReference(node) => Ok(node.name.to_string()),
        ast::ModuleExportName::StringLiteral(node) if !node.lone_surrogates => {
            Ok(node.value.to_string())
        }
        _ => Err("host export name contains an unsupported surrogate".into()),
    }
}

fn analyze(source: &str) -> Result<Analyzed, String> {
    let arena = oxc_allocator::Allocator::default();
    let tree = parse_with(&arena, source, false, true)?;
    let checked = oxc_semantic::SemanticBuilder::new()
        .with_check_syntax_error(true)
        .build(&tree);
    if let Some(error) = checked.diagnostics.first() {
        return Err(format!("host module: {error}"));
    }
    let mut erase = Vec::new();
    let mut exports = Vec::new();
    let mut imports = Vec::new();
    let mut import_source = String::new();
    for statement in &tree.body {
        let span = statement.span();
        match statement {
            ast::Statement::ImportDeclaration(node) => {
                if node.phase.is_some() || node.with_clause.is_some() {
                    return Err("host module uses import phases or attributes".into());
                }
                let mut bindings = Vec::new();
                for item in node.specifiers.iter().flatten() {
                    let (imported, local) = match item {
                        ast::ImportDeclarationSpecifier::ImportSpecifier(item) => (
                            Some(export_name(&item.imported)?),
                            item.local.name.to_string(),
                        ),
                        ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(item) => {
                            (None, item.local.name.to_string())
                        }
                        ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(item) => {
                            (Some("default".into()), item.local.name.to_string())
                        }
                    };
                    bindings.push((imported, Some(local)));
                }
                imports.push((node.source.value.to_string(), bindings));
                import_source.push_str(&source[span.start as usize..span.end as usize]);
                import_source.push(';');
                erase.push((span.start as usize, span.end as usize));
            }
            ast::Statement::ExportDeclaration(node) => {
                erase.push((span.start as usize, node.declaration.span().start as usize));
                match &node.declaration {
                    ast::Declaration::FunctionDeclaration(node) => {
                        let local = node.id.as_ref().ok_or("host export name")?.name.to_string();
                        exports.push((local.clone(), local));
                    }
                    ast::Declaration::ClassDeclaration(node) => {
                        let local = node.id.as_ref().ok_or("host export name")?.name.to_string();
                        exports.push((local.clone(), local));
                    }
                    ast::Declaration::VariableDeclaration(node) => {
                        for declaration in &node.declarations {
                            let ast::BindingPattern::BindingIdentifier(local) = &declaration.id
                            else {
                                return Err("host module exports a destructuring pattern".into());
                            };
                            exports.push((local.name.to_string(), local.name.to_string()));
                        }
                    }
                    _ => return Err("host module exports an unsupported declaration".into()),
                }
            }
            ast::Statement::ExportNamedDeclaration(node) => {
                for item in &node.specifiers {
                    exports.push((export_name(&item.exported)?, export_name(&item.local)?));
                }
                erase.push((span.start as usize, span.end as usize));
            }
            ast::Statement::ExportFromDeclaration(_) => {
                return Err("host module re-exports another module".into());
            }
            ast::Statement::ExportDefaultDeclaration(_)
            | ast::Statement::ExportAllDeclaration(_) => {
                return Err("host module uses a default or star export".into());
            }
            _ => {}
        }
    }
    struct Context {
        invalid: Option<&'static str>,
    }
    impl<'a> Visit<'a> for Context {
        fn enter_node(&mut self, node: AstKind<'a>) {
            if matches!(node, AstKind::ImportMeta(_)) {
                self.invalid = Some("host module reads import.meta");
            }
        }
    }
    let mut context = Context { invalid: None };
    context.visit_program(&tree);
    if let Some(error) = context.invalid {
        return Err(error.into());
    }
    // Binding-aware lookup: a nested declaration must not hide an outer free
    // read when reserving names in the embedding scope.
    let free: Vec<String> = checked
        .semantic
        .scoping()
        .root_unresolved_references()
        .keys()
        .map(|name| name.to_string())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    if free.iter().any(|name| name == "arguments") {
        return Err("host module reads unbound arguments; retain this module as external".into());
    }
    if !imports.is_empty() && free.iter().any(|name| name == "eval") {
        return Err(
            "host module with imports uses eval, whose lexical lookup cannot be relocated".into(),
        );
    }
    let scoping = checked.semantic.scoping();
    let mut volatile = Vec::new();
    for (exported, local) in &exports {
        if scoping
            .get_root_binding(local.as_str().into())
            .is_some_and(|symbol| scoping.symbol_is_mutated(symbol))
        {
            volatile.push(exported.clone());
        }
    }
    let body = compact(&erase_spans(source, &erase)?)?;
    let reads = import_reads(&import_source, &body)?;
    Ok(Analyzed {
        body,
        exports,
        volatile,
        imports,
        free,
        reads,
    })
}

/// Named import references in the compacted body, with lexical shadowing
/// resolved by the same typed semantic owner. This is delivery metadata, not
/// an executable string template or a second optimizer.
fn import_reads(prefix: &str, body: &str) -> Result<Vec<HostRead>, String> {
    if prefix.is_empty() {
        return Ok(Vec::new());
    }
    let source = format!("{prefix}{body}");
    let arena = oxc_allocator::Allocator::default();
    let tree = parse_program(&arena, &source)?;
    let checked = oxc_semantic::SemanticBuilder::new()
        .with_build_nodes(true)
        .with_check_syntax_error(true)
        .build(&tree);
    if let Some(error) = checked.diagnostics.first() {
        return Err(format!("host module: {error}"));
    }
    let mut reads = Vec::new();
    let mut import = 0;
    for statement in &tree.body {
        let ast::Statement::ImportDeclaration(node) = statement else {
            continue;
        };
        for (binding, specifier) in node.specifiers.iter().flatten().enumerate() {
            let local = match specifier {
                ast::ImportDeclarationSpecifier::ImportSpecifier(item) => &item.local,
                ast::ImportDeclarationSpecifier::ImportDefaultSpecifier(item) => &item.local,
                ast::ImportDeclarationSpecifier::ImportNamespaceSpecifier(_) => continue,
            };
            let symbol = local.symbol_id.get().ok_or("host import symbol")?;
            for reference in checked.semantic.scoping().get_resolved_references(symbol) {
                if reference.is_write() {
                    return Err("host module assigns to an import".into());
                }
                let id = reference.node_id();
                let span = checked.semantic.nodes().get_node(id).kind().span();
                if span.start < prefix.len() as u32 {
                    continue;
                }
                let shorthand = matches!(checked.semantic.nodes().parent_kind(id), AstKind::ObjectProperty(node) if node.shorthand);
                reads.push(HostRead {
                    start: span.start as usize - prefix.len(),
                    end: span.end as usize - prefix.len(),
                    import,
                    binding,
                    shorthand,
                });
            }
        }
        import += 1;
    }
    reads.sort_unstable_by_key(|read| read.start);
    Ok(reads)
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
        let output = std::process::Command::new("node").args(["--input-type=module", "-e", &format!("const [a,b]={expression};if(a.sum(2,3)!==5||b.add(4,5)!==9||b.ZERO!==0)throw Error('host linkage');")]).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let _ = std::fs::remove_dir_all(directory);
    }
    #[test]
    fn d2_host_text_uses_live_imports_and_resolves_shadowing() {
        let directory =
            std::env::temp_dir().join(format!("lilscript-d2-host-live-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("state.js"), "export let value=1;export function change(){value++;}export function receiver(){return this;}").unwrap();
        std::fs::write(directory.join("entry.js"), "import {value,change,receiver} from './state.js';const moduleThis=this;export function run(){const first={value};change();const shadow=(value)=>value+8;return [first.value,value,shadow(2),receiver()===undefined,moduleThis===undefined];}").unwrap();
        let delivery = deliver(
            &directory,
            &[directory.join("entry.js")],
            crate::js_syntax_target::EcmaScriptEdition::Es2022,
        )
        .unwrap();
        let expression = delivery.expression(true);
        let output = std::process::Command::new("node").args(["--input-type=module", "-e", &format!("const [,m]={expression};const value=m.run();if(JSON.stringify(value)!=='[1,2,10,true,true]')throw Error(JSON.stringify(value));")]).output().unwrap();
        assert!(
            output.status.success(),
            "{}\n{expression}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            deliver(
                &directory,
                &[directory.join("state.js")],
                crate::js_syntax_target::EcmaScriptEdition::Es2022
            )
            .unwrap_err()
            .contains("mutable binding")
        );
        assert_eq!(
            analyze(
                "function local(window){return window;}export function read(){return window.x;}"
            )
            .unwrap()
            .free,
            ["window"]
        );
        let _ = std::fs::remove_dir_all(directory);
    }
}
