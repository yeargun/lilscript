//! Original-source module checking. Shared declarations are never copied into
//! per-file models, and import aliases never allocate another value identity.
use super::*;
use crate::compilation_policy::WorkKind;
use crate::module::{ModuleId, ModuleSet};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleSemanticError {
    pub module: ModuleId,
    pub error: SemanticError,
}
impl fmt::Display for ModuleSemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module {}: {}", self.module, self.error)
    }
}
impl std::error::Error for ModuleSemanticError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmittedModuleSemanticError {
    pub module: ModuleId,
    pub error: AdmittedSemanticError,
}

impl From<ModuleSemanticError> for AdmittedModuleSemanticError {
    fn from(error: ModuleSemanticError) -> Self {
        Self {
            module: error.module,
            error: AdmittedSemanticError::Semantic(error.error),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceTarget {
    Value(SymbolId),
    Struct(NominalId),
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleImport<'src> {
    pub module: ModuleId,
    pub imported: &'src str,
    pub local: &'src str,
    pub target: InterfaceTarget,
    pub span: Span,
}
#[derive(Debug, Clone, Copy)]
pub struct ModuleExport<'src> {
    pub external: &'src str,
    pub target: InterfaceTarget,
    pub span: Span,
}
#[derive(Debug)]
pub struct ModuleInterface<'src> {
    pub module: ModuleId,
    pub dependencies: Vec<ModuleId>,
    /// Modules this one loads with `import()`, in source order.
    pub dynamic_dependencies: Vec<ModuleId>,
    pub imports: Vec<ModuleImport<'src>>,
    pub exports: Vec<ModuleExport<'src>>,
    /// One output specifier per `import extern` declaration, in source
    /// order: a resolved path relative to the root module's directory, as
    /// the legacy linker spells it, or the bare specifier.
    pub foreign_sources: Vec<String>,
}

#[derive(Debug)]
pub struct CheckedModules<'ast, 'src> {
    declarations: DeclarationTables<'src>,
    facts: Vec<ModuleFacts<'ast, 'src>>,
    interfaces: Vec<ModuleInterface<'src>>,
    initialization_order: Vec<ModuleId>,
    root: ModuleId,
}
impl<'ast, 'src> CheckedModules<'ast, 'src> {
    pub fn view(&self, module: ModuleId) -> Option<SemanticView<'_, 'ast, 'src>> {
        Some(SemanticView {
            declarations: &self.declarations,
            facts: self.facts.get(module)?,
        })
    }
    pub fn symbols(&self) -> &[Symbol<'src>] {
        &self.declarations.symbols
    }
    pub fn symbol_module(&self, symbol: SymbolId) -> Option<ModuleId> {
        self.declarations
            .symbol_modules
            .get(symbol.0 as usize)
            .copied()
            .flatten()
    }
    pub fn nominal_module(&self, nominal: NominalId) -> Option<ModuleId> {
        if nominal.is_class() {
            return None;
        }
        self.declarations.structs.get(nominal.index())?.module
    }
    pub fn source(&self, module: ModuleId) -> Option<&crate::ast::SourceIdentity> {
        self.facts.get(module).map(|facts| &facts.source)
    }
    pub fn interfaces(&self) -> &[ModuleInterface<'src>] {
        &self.interfaces
    }
    pub fn initialization_order(&self) -> &[ModuleId] {
        &self.initialization_order
    }
    pub fn root(&self) -> ModuleId {
        self.root
    }
}

fn error(module: ModuleId, span: Span, message: impl Into<String>) -> ModuleSemanticError {
    ModuleSemanticError {
        module,
        error: SemanticError::new(span, message),
    }
}

/// Checks original ASTs aligned with canonical discovery identities. Declaration
/// and body checking use the same Analyzer as `analyze`; only ownership,
/// interfaces and source-qualified initialization differ.
pub fn analyze_modules<'ast, 'src>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet,
) -> Result<CheckedModules<'ast, 'src>, ModuleSemanticError> {
    analyze_modules_in(programs, modules, &mut AllocationBudget::new(None)).map_err(|failure| {
        match failure.error {
            AdmittedSemanticError::Semantic(error) => ModuleSemanticError {
                module: failure.module,
                error,
            },
            AdmittedSemanticError::Resources(reason) => error(
                failure.module,
                programs[failure.module].span,
                format!("module checking allocation failed: {reason}"),
            ),
        }
    })
}

/// The same checker owns admitted fixed facts, interfaces and initialization
/// order, including shared declaration vector backing. Declaration maps and
/// nested checker payloads remain uninstrumented.
pub(crate) fn with_analyzed_modules<'ast, 'src, S, R>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet<S>,
    budget: &mut AllocationBudget<'_>,
    client: impl FnOnce(&CheckedModules<'ast, 'src>, &mut AllocationBudget<'_>) -> R,
) -> Result<R, AdmittedModuleSemanticError> {
    let mut scope = budget.scope();
    let checked = analyze_modules_in(programs, modules, &mut scope)?;
    #[cfg(test)]
    let checked = AdmittedFactsOwner::new(checked, programs.len());
    let output = client(&checked, &mut scope);
    drop(checked);
    scope
        .finish_retained()
        .expect("checked-module callback transfers within its allocation owner");
    Ok(output)
}

fn resource(module: ModuleId, error: AllocationError) -> AdmittedModuleSemanticError {
    AdmittedModuleSemanticError {
        module,
        error: AdmittedSemanticError::Resources(error),
    }
}

fn analyze_modules_in<'ast, 'src, S>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet<S>,
    budget: &mut AllocationBudget<'_>,
) -> Result<CheckedModules<'ast, 'src>, AdmittedModuleSemanticError> {
    let initialization_order = validate_graph(programs, modules, budget)?;
    let mut facts = budget
        .vector(AllocationClass::Scratch, programs.len())
        .map_err(|error| resource(modules.root, error))?;
    for (module, program) in programs.iter().enumerate() {
        let source = ModuleFacts::new_admitted(program.source_identity(), budget)
            .map_err(|error| resource(module, error))?;
        budget
            .push(AllocationClass::Scratch, &mut facts, source)
            .map_err(|error| resource(module, error))?;
    }
    let mut interfaces = budget
        .vector(AllocationClass::Scratch, programs.len())
        .map_err(|error| resource(modules.root, error))?;
    for (module, (program, source)) in programs.iter().zip(&modules.modules).enumerate() {
        let dependencies = budget
            .copy_slice(AllocationClass::Scratch, &source.dependencies)
            .map_err(|error| resource(module, error))?;
        let dynamic_dependencies = budget
            .copy_slice(AllocationClass::Scratch, &source.dynamic_dependencies)
            .map_err(|error| resource(module, error))?;
        let import_count = program
            .imports
            .iter()
            .try_fold(0usize, |count, import| {
                budget.work(WorkKind::Analysis, 1)?;
                count
                    .checked_add(import.specifiers.len())
                    .ok_or(AllocationError::Capacity)
            })
            .map_err(|error| resource(module, error))?;
        let imports = budget
            .vector(AllocationClass::Scratch, import_count)
            .map_err(|error| resource(module, error))?;
        let exports = budget
            .vector(AllocationClass::Scratch, program.exports.len())
            .map_err(|error| resource(module, error))?;
        let root_directory = modules.modules[modules.root]
            .path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."));
        let mut foreign_sources = budget
            .vector(AllocationClass::Scratch, source.foreign_dependencies.len())
            .map_err(|error| resource(module, error))?;
        for dependency in &source.foreign_dependencies {
            let specifier = match &dependency.path {
                Some(path) => crate::module::relative_module_specifier(root_directory, path),
                None => dependency.specifier.clone(),
            };
            budget
                .work(WorkKind::Analysis, specifier.len() as u64 + 1)
                .map_err(|error| resource(module, error))?;
            let specifier = budget
                .string(AllocationClass::Scratch, &specifier)
                .map_err(|error| resource(module, error))?;
            budget
                .push(AllocationClass::Scratch, &mut foreign_sources, specifier)
                .map_err(|error| resource(module, error))?;
        }
        budget
            .push(
                AllocationClass::Scratch,
                &mut interfaces,
                ModuleInterface {
                    module,
                    dependencies,
                    dynamic_dependencies,
                    imports,
                    exports,
                    foreign_sources,
                },
            )
            .map_err(|error| resource(module, error))?;
    }
    let mut checked = CheckedModules {
        declarations: DeclarationTables::default(),
        facts,
        interfaces,
        initialization_order,
        root: modules.root,
    };
    let mut initialization = ModuleInitialization::default();
    let mut locals = Vec::with_capacity(programs.len());
    // Canonical declaration order is original module order, then source order.
    for (module, program) in programs.iter().enumerate() {
        locals.push(preflight_source(module, program)?);
        let mut analyzer = Analyzer::new(
            &mut checked.facts[module],
            &mut checked.declarations,
            &mut initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        analyzer
            .declare_nominal_types(program)
            .map_err(|error| AdmittedModuleSemanticError { module, error })?;
    }
    let mut aliases = InterfaceGraph::new(programs, &checked, &locals)?;
    aliases.propagate();
    aliases.install_types(programs, &mut checked)?;
    for (module, program) in programs.iter().enumerate() {
        let mut analyzer = Analyzer::new(
            &mut checked.facts[module],
            &mut checked.declarations,
            &mut initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        analyzer
            .define_enums(program)
            .and_then(|()| analyzer.define_structs(program))
            .map_err(|error| AdmittedModuleSemanticError { module, error })?;
    }
    super::struct_cycles::validate(&checked.declarations.structs).map_err(|(module, error)| {
        ModuleSemanticError {
            module: module.expect("module schema owner"),
            error,
        }
    })?;
    // Classes follow the single-source order: every module's declarations,
    // then one hierarchy resolution over the shared declaration tables. A
    // class-free graph skips the pass and its analyzer scratch entirely.
    let declares_classes = |program: &Program<'_, '_>| {
        program
            .items
            .iter()
            .any(|item| matches!(item, Item::Class(_) | Item::ExternClass(_)))
    };
    let last_with_classes = programs.iter().rposition(|program| declares_classes(program));
    for (module, program) in programs.iter().enumerate() {
        let Some(last) = last_with_classes else {
            break;
        };
        if !declares_classes(program) {
            continue;
        }
        let mut analyzer = Analyzer::new(
            &mut checked.facts[module],
            &mut checked.declarations,
            &mut initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        analyzer
            .define_classes(program)
            .and_then(|()| analyzer.define_extern_classes(program))
            .map_err(|error| AdmittedModuleSemanticError { module, error })?;
        if module == last {
            analyzer
                .resolve_class_hierarchies()
                .map_err(|error| AdmittedModuleSemanticError { module, error })?;
        }
    }
    let mut scopes = Vec::with_capacity(programs.len());
    // Value contracts are seeded into the same alias graph after canonical type
    // aliases and schemas exist. No source AST or schema table is copied.
    for (module, program) in programs.iter().enumerate() {
        let mut analyzer = Analyzer::new(
            &mut checked.facts[module],
            &mut checked.declarations,
            &mut initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        let registration = (|| {
            analyzer.declare_functions(program)?;
            for item in program.items {
                let Item::Stmt(Stmt::VarDecl(decl)) = item else {
                    continue;
                };
                if decl.ty.is_auto() {
                    continue;
                }
                let mut ty = analyzer.resolve_value_type(decl.ty, "module binding")?;
                strip_parameter_defaults_from_type(&mut ty);
                let symbol = analyzer.declare(decl.name, ty)?;
                analyzer.initialization.bindings.insert(
                    symbol,
                    ModuleBindingState {
                        declaration: decl.name.span,
                        owner: module,
                    },
                );
            }
            Ok(())
        })();
        registration.map_err(|error| AdmittedModuleSemanticError { module, error })?;
        scopes.push(analyzer.scopes.pop().expect("one module declaration scope"));
    }
    aliases.finish(programs, &mut checked, &mut scopes, &locals, budget)?;
    drop(aliases);
    drop(locals);
    // Each `import()` names its module; the namespace's members are that
    // module's runtime exports, resolved through its interface.
    for (module, program) in programs.iter().enumerate() {
        let dynamic = &modules.modules[module].dynamic_dependencies;
        if dynamic.is_empty() {
            continue;
        }
        let mut sites = Vec::new();
        crate::module::collect_program_dynamic_imports(program, &mut sites);
        if sites.len() != dynamic.len() {
            return Err(error(
                module,
                program.span,
                "internal dynamic module dependency mismatch",
            )
            .into());
        }
        for ((_, span), &target) in sites.iter().zip(dynamic) {
            budget
                .work(
                    WorkKind::Analysis,
                    checked.interfaces[target].exports.len() as u64 + 1,
                )
                .map_err(|error| resource(module, error))?;
            let id = u32::try_from(target)
                .map_err(|_| error(module, *span, "dynamic module id range"))?;
            let facts = &mut checked.facts[module];
            facts.dynamic_import_modules.insert(*span, id);
            for export in &checked.interfaces[target].exports {
                if let InterfaceTarget::Value(symbol) = export.target {
                    facts
                        .dynamic_export_symbols
                        .insert((id, export.external), symbol);
                }
            }
        }
    }

    for &module in &checked.initialization_order {
        let program = &programs[module];
        let mut analyzer = Analyzer::new(
            &mut checked.facts[module],
            &mut checked.declarations,
            &mut initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        analyzer.scopes[0] = std::mem::take(&mut scopes[module]);
        for item in program.items {
            if let Item::Stmt(Stmt::VarDecl(decl)) = item {
                if !decl.ty.is_auto() {
                    let symbol = analyzer.facts.identifier_symbols[&decl.name.span];
                    analyzer
                        .module_binding_declarations
                        .insert(decl.name.span, symbol);
                }
            }
        }
        analyzer
            .analyze_items(program)
            .and_then(|()| analyzer.finalize_module_parameter_defaults(module, program))
            .map_err(|error| AdmittedModuleSemanticError { module, error })?;
        // Later modules own their own resolved namespace. Keep only their
        // pending scopes, not this completed analyzer's maps.
        drop(analyzer);
    }
    for export in &checked.interfaces[checked.root].exports {
        let InterfaceTarget::Value(symbol) = export.target else {
            continue;
        };
        if checked.declarations.symbols[symbol.0 as usize]
            .ty
            .contains_mutable_reference_parameters()
        {
            return Err(error(
                checked.root,
                export.span,
                "public exports do not yet support mutable-reference callable contracts",
            )
            .into());
        }
    }
    #[cfg(debug_assertions)]
    assert!(checked
        .declarations
        .identifier_index_is_consistent(&checked.facts));
    Ok(checked)
}

fn validate_graph<S>(
    programs: &[Program<'_, '_>],
    modules: &ModuleSet<S>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<ModuleId>, AdmittedModuleSemanticError> {
    if programs.len() != modules.modules.len()
        || programs.is_empty()
        || modules.root >= programs.len()
        || modules.eager.len() != programs.len()
    {
        return Err(error(
            modules.root,
            Span::empty(0),
            "direct module source/graph ownership mismatch",
        )
        .into());
    }
    for (module, (program, source)) in programs.iter().zip(&modules.modules).enumerate() {
        budget
            .work(WorkKind::Analysis, 1)
            .map_err(|error| resource(module, error))?;
        if program.imports.len() != source.dependencies.len() {
            return Err(error(module, program.span, "direct module dependency mismatch").into());
        }
        for &target in &source.dependencies {
            budget
                .work(WorkKind::Analysis, 1)
                .map_err(|error| resource(module, error))?;
            if target >= programs.len() {
                return Err(
                    error(module, program.span, "direct module dependency mismatch").into(),
                );
            }
        }
        // A module reached only through `import()` runs no code when it
        // loads, so its place in the initialization order is unobservable.
        if !modules.eager[module] {
            if let Some(item) = program
                .items
                .iter()
                .find(|item| matches!(item, Item::Stmt(_)))
            {
                return Err(error(
                    module,
                    item.span(),
                    "lazy modules must be initialization-free; move top-level executable declarations into an exported function",
                )
                .into());
            }
        }
        for &target in &source.dynamic_dependencies {
            budget
                .work(WorkKind::Analysis, 1)
                .map_err(|error| resource(module, error))?;
            if target >= programs.len() {
                return Err(
                    error(module, program.span, "direct module dependency mismatch").into(),
                );
            }
        }
        if program.foreign_imports.len() != source.foreign_dependencies.len() {
            return Err(error(module, program.span, "foreign module dependency mismatch").into());
        }
        // An `import extern` local names the runtime source of a top-level
        // extern value declared in the same module; that declaration is its
        // static contract.
        for import in program.foreign_imports {
            for specifier in import.specifiers {
                budget
                    .work(WorkKind::Analysis, program.items.len() as u64 + 1)
                    .map_err(|error| resource(module, error))?;
                let backed = program.items.iter().any(|item| match item {
                    Item::Extern(declaration) => declaration.name.name == specifier.local.name,
                    Item::ExternGlobal(declaration) => {
                        declaration.name.name == specifier.local.name
                    }
                    _ => false,
                });
                if !backed {
                    return Err(error(
                        module,
                        specifier.local.span,
                        format!(
                            "foreign import `{}` requires a matching extern value declaration",
                            specifier.local.name
                        ),
                    )
                    .into());
                }
            }
        }
    }
    // Shared graph owner defines the source and core initialization order;
    // modules only `import()` reaches follow, in module order.
    let order = crate::module::initialization_order_admitted(
        modules.root,
        programs.len(),
        |module| modules.modules[module].dependencies.iter().copied(),
        budget,
    )
    .map_err(|failure| match failure {
        crate::module::StaticOrderError::Invalid(message) => {
            error(modules.root, programs[modules.root].span, message).into()
        }
        crate::module::StaticOrderError::Resources(reason) => resource(modules.root, reason),
    })?;
    // Discovery's post-order interleaves dynamic edges, so it matches the
    // static order only in a graph without them.
    let dynamic = modules
        .modules
        .iter()
        .any(|module| !module.dynamic_dependencies.is_empty());
    let mut same = order.len() == programs.len() && order.len() == modules.dependency_order.len();
    if same && !dynamic {
        for (actual, expected) in order.iter().zip(&modules.dependency_order) {
            budget
                .work(WorkKind::Analysis, 1)
                .map_err(|error| resource(modules.root, error))?;
            if actual != expected {
                same = false;
                break;
            }
        }
    }
    if !same {
        return Err(error(
            modules.root,
            programs[modules.root].span,
            "direct module initialization order does not match ordered dependencies",
        )
        .into());
    }
    Ok(order)
}

fn preflight_source<'ast, 'src>(
    module: ModuleId,
    program: &Program<'ast, 'src>,
) -> Result<AHashMap<&'src str, (Span, bool)>, ModuleSemanticError> {
    // Foreign imports are admitted by the graph check; each local is an
    // extern value declared in this module.
    if program
        .exports
        .iter()
        .any(|export| export.kind != crate::ast::ExportKind::Binding)
    {
        return Err(error(
            module,
            program.span,
            "direct module checking does not yet support nominal constructor exports",
        ));
    }
    let mut reserved = AHashMap::default();
    for item in program.items {
        let mut declare = |name: Ident<'src>, auto: bool| {
            if reserved.insert(name.name, (name.span, auto)).is_some() {
                return Err(error(
                    module,
                    name.span,
                    format!("duplicate module binding `{}`", name.name),
                ));
            }
            Ok(())
        };
        match item {
            Item::Function(decl) => declare(decl.name, false)?,
            // A generic extern is checked per call like any generic callee;
            // a host result carrying a private representation is refused by
            // its target's boundary rules.
            Item::Extern(decl) => declare(decl.name, false)?,
            Item::ExternGlobal(decl) => declare(decl.name, false)?,
            Item::Stmt(Stmt::VarDecl(decl)) => declare(decl.name, decl.ty.is_auto())?,
            Item::Stmt(Stmt::ArrayDestructure { bindings, .. }) => {
                for binding in *bindings {
                    if let ArrayBinding::Name(name) | ArrayBinding::Rest(name) = binding {
                        declare(*name, true)?;
                    }
                }
            }
            Item::Stmt(Stmt::RecordDestructure { bindings, rest, .. }) => {
                for binding in *bindings {
                    declare(binding.name, true)?;
                }
                if let Some(rest) = rest {
                    declare(*rest, true)?;
                }
            }
            _ => {}
        }
    }
    let mut aliases = AHashSet::default();
    for import in program.imports {
        for specifier in import.specifiers {
            if !aliases.insert(specifier.local.name) {
                return Err(error(
                    module,
                    specifier.local.span,
                    format!("duplicate module binding `{}`", specifier.local.name),
                ));
            }
        }
    }
    let mut exports = AHashSet::default();
    for export in program.exports {
        if !exports.insert(export.exported.name) {
            return Err(error(
                module,
                export.exported.span,
                format!("duplicate export `{}`", export.exported.name),
            ));
        }
    }
    Ok(reserved)
}

struct AliasNode {
    target: Option<InterfaceTarget>,
    consumers: Vec<usize>,
}

/// One producer/consumer graph, with canonical types seeded before value
/// contracts. Every node becomes ready once, across both phases.
struct InterfaceGraph<'src> {
    nodes: Vec<AliasNode>,
    exports: Vec<AHashMap<&'src str, usize>>,
    imports: Vec<AHashMap<&'src str, usize>>,
    values: Vec<(usize, usize, &'src str)>,
    ready: VecDeque<usize>,
    /// Class, extern class and enum names each module declares or imports.
    /// They resolve by name in the shared declaration tables and have no
    /// runtime export, so their exports and imports carry no node.
    type_names: Vec<AHashSet<&'src str>>,
}
impl<'src> InterfaceGraph<'src> {
    fn new<'ast>(
        programs: &[Program<'ast, 'src>],
        checked: &CheckedModules<'ast, 'src>,
        locals: &[AHashMap<&'src str, (Span, bool)>],
    ) -> Result<Self, ModuleSemanticError> {
        let mut graph = Self {
            nodes: Vec::new(),
            exports: vec![AHashMap::default(); programs.len()],
            imports: vec![AHashMap::default(); programs.len()],
            values: Vec::new(),
            ready: VecDeque::new(),
            type_names: vec![AHashSet::default(); programs.len()],
        };
        let type_exports = graph.type_names(programs, checked)?;
        for (module, program) in programs.iter().enumerate() {
            for export in program.exports {
                if type_exports[module].contains(export.exported.name) {
                    continue;
                }
                let node = graph.nodes.len();
                graph.exports[module].insert(export.exported.name, node);
                graph.nodes.push(AliasNode {
                    target: None,
                    consumers: Vec::new(),
                });
            }
            for import in program.imports {
                for specifier in import.specifiers {
                    if graph.type_names[module].contains(specifier.local.name) {
                        continue;
                    }
                    graph.imports[module].insert(specifier.local.name, graph.nodes.len());
                    graph.nodes.push(AliasNode {
                        target: None,
                        consumers: Vec::new(),
                    });
                }
            }
        }
        for (module, program) in programs.iter().enumerate() {
            for (index, import) in program.imports.iter().enumerate() {
                let dependency = checked.interfaces[module].dependencies[index];
                for specifier in import.specifiers {
                    if graph.type_names[module].contains(specifier.local.name) {
                        continue;
                    }
                    let Some(&producer) = graph.exports[dependency].get(specifier.imported.name)
                    else {
                        return Err(error(
                            module,
                            specifier.imported.span,
                            format!(
                                "module `{}` does not export `{}`",
                                import.source, specifier.imported.name
                            ),
                        ));
                    };
                    graph.nodes[producer]
                        .consumers
                        .push(graph.imports[module][specifier.local.name]);
                }
            }
            for export in program.exports {
                if type_exports[module].contains(export.exported.name) {
                    continue;
                }
                let node = graph.exports[module][export.exported.name];
                let local_value = locals[module].get(export.local.name);
                let local_type = checked.facts[module]
                    .struct_bindings
                    .get(export.local.name)
                    .copied();
                let declared_type = checked
                    .view(module)
                    .unwrap()
                    .export_target(export.local.span);
                let direct_value = local_value.is_some_and(|(span, _)| *span == export.local.span);
                let target = if let Some(target) = declared_type {
                    Some(target)
                } else if direct_value {
                    None
                } else if local_type.is_some() && local_value.is_some() {
                    return Err(error(
                        module,
                        export.local.span,
                        "ambiguous export names both a value and a struct type; export the declaration directly",
                    ));
                } else {
                    local_type.map(InterfaceTarget::Struct)
                };
                if let Some(target) = target {
                    graph.seed(node, target);
                } else if let Some((_, inferred)) = local_value {
                    if *inferred {
                        return Err(error(
                            module,
                            export.local.span,
                            "direct module checking does not yet support inferred export interfaces; add an explicit type",
                        ));
                    }
                    graph.values.push((node, module, export.local.name));
                } else {
                    let Some(&producer) = graph.imports[module].get(export.local.name) else {
                        return Err(error(
                            module,
                            export.local.span,
                            format!("cannot export unknown binding `{}`", export.local.name),
                        ));
                    };
                    graph.nodes[producer].consumers.push(node);
                }
            }
        }
        Ok(graph)
    }
    /// Finds every type-only name: declared classes, extern classes and
    /// enums, their exports, and imports and re-exports of those, to a fixed
    /// point. Returns each module's type-only export names. A renamed one is
    /// refused: the importer would resolve the declaration's own name.
    fn type_names<'ast>(
        &mut self,
        programs: &[Program<'ast, 'src>],
        checked: &CheckedModules<'ast, 'src>,
    ) -> Result<Vec<AHashSet<&'src str>>, ModuleSemanticError> {
        let mut exports = vec![AHashSet::default(); programs.len()];
        for (module, program) in programs.iter().enumerate() {
            for item in program.items {
                let name = match item {
                    Item::Class(declaration) => declaration.name.name,
                    Item::ExternClass(declaration) => declaration.name.name,
                    Item::Enum(declaration) => declaration.name.name,
                    _ => continue,
                };
                self.type_names[module].insert(name);
            }
        }
        loop {
            let mut changed = false;
            for (module, program) in programs.iter().enumerate() {
                for (index, import) in program.imports.iter().enumerate() {
                    let dependency = checked.interfaces[module].dependencies[index];
                    for specifier in import.specifiers {
                        if !exports[dependency].contains(specifier.imported.name) {
                            continue;
                        }
                        if specifier.local.name != specifier.imported.name {
                            return Err(error(
                                module,
                                specifier.local.span,
                                "a class or enum type import cannot be renamed yet",
                            ));
                        }
                        changed |= self.type_names[module].insert(specifier.local.name);
                    }
                }
                for export in program.exports {
                    if !self.type_names[module].contains(export.local.name) {
                        continue;
                    }
                    if export.local.name != export.exported.name {
                        return Err(error(
                            module,
                            export.exported.span,
                            "a class or enum type export cannot be renamed yet",
                        ));
                    }
                    changed |= exports[module].insert(export.exported.name);
                }
            }
            if !changed {
                return Ok(exports);
            }
        }
    }
    fn seed(&mut self, node: usize, target: InterfaceTarget) {
        assert!(self.nodes[node].target.is_none());
        self.nodes[node].target = Some(target);
        self.ready.push_back(node);
    }
    fn propagate(&mut self) {
        while let Some(index) = self.ready.pop_front() {
            let target = self.nodes[index].target.unwrap();
            for edge in 0..self.nodes[index].consumers.len() {
                let consumer = self.nodes[index].consumers[edge];
                if self.nodes[consumer].target.is_none() {
                    self.seed(consumer, target);
                } else {
                    debug_assert_eq!(self.nodes[consumer].target, Some(target));
                }
            }
        }
    }
    fn install_types<'ast>(
        &self,
        programs: &[Program<'ast, 'src>],
        checked: &mut CheckedModules<'ast, 'src>,
    ) -> Result<(), ModuleSemanticError> {
        for (module, program) in programs.iter().enumerate() {
            for import in program.imports {
                for specifier in import.specifiers {
                    let Some(&node) = self.imports[module].get(specifier.local.name) else {
                        continue;
                    };
                    let Some(InterfaceTarget::Struct(identity)) = self.nodes[node].target else {
                        continue;
                    };
                    let facts = &mut checked.facts[module];
                    if facts
                        .struct_bindings
                        .insert(specifier.local.name, identity)
                        .is_some()
                    {
                        return Err(error(
                            module,
                            specifier.local.span,
                            format!("duplicate module type binding `{}`", specifier.local.name),
                        ));
                    }
                    let declaration = checked.declarations.structs[identity.index()].declaration;
                    facts.binding_types.insert(
                        specifier.local.span,
                        BindingType::Inline(Type::Struct(declaration)),
                    );
                }
            }
        }
        Ok(())
    }
    fn finish<'ast>(
        &mut self,
        programs: &[Program<'ast, 'src>],
        checked: &mut CheckedModules<'ast, 'src>,
        scopes: &mut [AHashMap<&'src str, SymbolId>],
        locals: &[AHashMap<&'src str, (Span, bool)>],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AdmittedModuleSemanticError> {
        for index in 0..self.values.len() {
            let (node, module, name) = self.values[index];
            self.seed(node, InterfaceTarget::Value(scopes[module][name]));
        }
        self.propagate();
        for (module, program) in programs.iter().enumerate() {
            for (index, import) in program.imports.iter().enumerate() {
                let dependency = checked.interfaces[module].dependencies[index];
                for specifier in import.specifiers {
                    if self.type_names[module].contains(specifier.local.name) {
                        continue;
                    }
                    let target = self.nodes[self.imports[module][specifier.local.name]]
                        .target
                        .ok_or_else(|| {
                            error(
                                module,
                                specifier.imported.span,
                                format!(
                                    "cyclic module binding `{}` cannot be resolved",
                                    specifier.imported.name
                                ),
                            )
                        })?;
                    if let InterfaceTarget::Value(symbol) = target {
                        if locals[module].contains_key(specifier.local.name) {
                            return Err(error(
                                module,
                                specifier.local.span,
                                format!("duplicate module binding `{}`", specifier.local.name),
                            )
                            .into());
                        }
                        scopes[module].insert(specifier.local.name, symbol);
                        let facts = &mut checked.facts[module];
                        facts
                            .binding_types
                            .insert(specifier.local.span, BindingType::Symbol(symbol));
                        facts.record_identifier(
                            &mut checked.declarations,
                            specifier.local.span,
                            symbol,
                        );
                    }
                    budget
                        .push(
                            AllocationClass::Scratch,
                            &mut checked.interfaces[module].imports,
                            ModuleImport {
                                module: dependency,
                                imported: specifier.imported.name,
                                local: specifier.local.name,
                                target,
                                span: specifier.local.span,
                            },
                        )
                        .map_err(|error| resource(module, error))?;
                }
            }
            for export in program.exports {
                let Some(&node) = self.exports[module].get(export.exported.name) else {
                    continue;
                };
                let target = self.nodes[node]
                    .target
                    .ok_or_else(|| {
                        error(module, export.span, "cyclic export cannot be resolved")
                    })?;
                let direct_value = locals[module]
                    .get(export.local.name)
                    .is_some_and(|(span, _)| *span == export.local.span);
                let direct_type = matches!(
                    checked
                        .view(module)
                        .unwrap()
                        .export_target(export.local.span),
                    Some(InterfaceTarget::Struct(_))
                );
                if !direct_value
                    && !direct_type
                    && scopes[module].contains_key(export.local.name)
                    && checked.facts[module]
                        .struct_bindings
                        .contains_key(export.local.name)
                {
                    return Err(error(
                        module,
                        export.local.span,
                        "ambiguous export names both a value and a struct type; export the declaration directly",
                    ).into());
                }
                let facts = &mut checked.facts[module];
                match target {
                    InterfaceTarget::Value(symbol) => facts.record_identifier(
                        &mut checked.declarations,
                        export.local.span,
                        symbol,
                    ),
                    InterfaceTarget::Struct(identity) => {
                        facts.binding_types.insert(
                            export.local.span,
                            BindingType::Inline(Type::Struct(
                                checked.declarations.structs[identity.index()].declaration,
                            )),
                        );
                    }
                }
                budget
                    .push(
                        AllocationClass::Scratch,
                        &mut checked.interfaces[module].exports,
                        ModuleExport {
                            external: export.exported.name,
                            target,
                            span: export.span,
                        },
                    )
                    .map_err(|error| resource(module, error))?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "modules_tests.rs"]
mod tests;
