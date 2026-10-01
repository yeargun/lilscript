//! Original-source module checking. Shared declarations are never copied into
//! per-file models, and import aliases never allocate another value identity.
use super::*;
use crate::compilation_policy::WorkKind;
use crate::module::{ModuleId, ModuleSet};
use std::collections::VecDeque;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleCheckError {
    pub module: ModuleId,
    pub error: CheckError,
}
impl fmt::Display for ModuleCheckError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "module {}: {}", self.module, self.error)
    }
}
impl std::error::Error for ModuleCheckError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AdmittedModuleCheckError {
    pub module: ModuleId,
    pub error: AdmittedCheckError,
}

impl From<ModuleCheckError> for AdmittedModuleCheckError {
    fn from(error: ModuleCheckError) -> Self {
        Self {
            module: error.module,
            error: AdmittedCheckError::Semantic(error.error),
        }
    }
}

/// What an import or export names: a value's symbol, or a nominal type
/// (struct, class, extern class or enum) by its identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InterfaceTarget {
    Value(SymbolId),
    Type(NominalId),
}

#[derive(Debug, Clone, Copy)]
pub struct ModuleImport<'src> {
    pub module: ModuleId,
    pub imported: &'src str,
    pub local: &'src str,
    pub target: InterfaceTarget,
    pub span: Span,
    /// The local binding's identifier (M4.4).
    pub node: SourceNodeId,
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
    /// the old route's linker spelled it, or the bare specifier.
    pub foreign_sources: Vec<String>,
}

#[derive(Debug)]
pub struct CheckedModules<'ast, 'src> {
    declarations: DeclarationTables<'src>,
    facts: Vec<ModuleFacts<'ast, 'src>>,
    interfaces: Vec<ModuleInterface<'src>>,
    initialization_order: Vec<ModuleId>,
    /// The entries' modules, sorted by entry name (plan M3.3).
    roots: Vec<ModuleId>,
    /// Each root's entry name.
    root_names: Vec<String>,
}
impl<'ast, 'src> CheckedModules<'ast, 'src> {
    pub fn view(&self, module: ModuleId) -> Option<CheckedView<'_, 'ast, 'src>> {
        Some(CheckedView {
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
    /// The module whose scope declares a nominal.
    pub fn nominal_module(&self, nominal: NominalId) -> Option<ModuleId> {
        match nominal.kind() {
            NominalKind::Struct => self.declarations.structs.get(nominal.index())?.module,
            NominalKind::Class => self.declarations.classes.get(nominal.index())?.module,
            NominalKind::Enum => self.declarations.enums.get(nominal.index())?.module,
        }
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
    /// The first entry's module: the one diagnostics cite as the root.
    pub fn root(&self) -> ModuleId {
        self.roots[0]
    }
    /// Every entry's module, in entry name order.
    pub fn roots(&self) -> &[ModuleId] {
        &self.roots
    }
    /// Each entry's name, in the order of `roots`.
    pub fn root_names(&self) -> &[String] {
        &self.root_names
    }
}

fn error(module: ModuleId, span: Span, message: impl Into<String>) -> ModuleCheckError {
    ModuleCheckError {
        module,
        error: CheckError::new(span, message),
    }
}

/// Checks original ASTs aligned with canonical discovery identities. Declaration
/// and body checking use the same Analyzer as `analyze`; only ownership,
/// interfaces and source-qualified initialization differ.
pub fn analyze_modules<'ast, 'src>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet,
) -> Result<CheckedModules<'ast, 'src>, ModuleCheckError> {
    analyze_modules_in(programs, modules, &mut AllocationBudget::new(None)).map_err(|failure| {
        match failure.error {
            AdmittedCheckError::Semantic(error) => ModuleCheckError {
                module: failure.module,
                error,
            },
            AdmittedCheckError::Resources(reason) => error(
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
) -> Result<R, AdmittedModuleCheckError> {
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

/// One source as a module graph of one module: no imports, and its
/// `import extern` edges as unresolved specifiers.
pub(super) fn analyze_source_in<'ast, 'src>(
    program: &Program<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<CheckedModules<'ast, 'src>, AdmittedModuleCheckError> {
    let modules = ModuleSet {
        modules: vec![crate::module::ModuleSource {
            path: std::path::PathBuf::from("<source>"),
            source: (),
            dependencies: Vec::new(),
            foreign_dependencies: program
                .foreign_imports
                .iter()
                .map(|import| crate::module::ForeignModuleSource {
                    specifier: import.source.to_string(),
                    path: None,
                })
                .collect(),
            dynamic_dependencies: Vec::new(),
            offset: 0,
        }],
        dependency_order: vec![0],
        roots: vec![0],
        root_names: vec!["main".to_string()],
        eager: vec![true],
    };
    analyze_modules_in(std::slice::from_ref(program), &modules, budget)
}

impl<'ast, 'src> CheckedModules<'ast, 'src> {
    /// The checked module of a one-module graph.
    pub(super) fn into_single(self) -> CheckedModule<'ast, 'src> {
        debug_assert_eq!(self.facts.len(), 1);
        CheckedModule {
            declarations: self.declarations,
            facts: self.facts.into_iter().next().expect("one checked source"),
        }
    }
}

fn resource(module: ModuleId, error: AllocationError) -> AdmittedModuleCheckError {
    AdmittedModuleCheckError {
        module,
        error: AdmittedCheckError::Resources(error),
    }
}

/// The module-graph checker: the one checking entry. A single source is a
/// graph of one module (`check::analyze`). Each phase consumes the previous
/// phase's product, so the order is fixed by construction:
///
/// 1. graph: sources match discovery; the initialization order;
/// 2. declarations: every nominal gets its identity in its module's type
///    scope, and type imports bind the exporter's identities;
/// 3. schemas: enums, structs, classes and extern classes, then class
///    hierarchies over every module's classes;
/// 4. signatures: functions and typed module bindings, then every value
///    import and export, and `import()` namespaces;
/// 5. bodies, in initialization order.
fn analyze_modules_in<'ast, 'src, S>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet<S>,
    budget: &mut AllocationBudget<'_>,
) -> Result<CheckedModules<'ast, 'src>, AdmittedModuleCheckError> {
    let graph = graph_phase(programs, modules, budget)?;
    let declared = declaration_phase(programs, graph, budget)?;
    let schemas = schema_phase(programs, declared, budget)?;
    let signatures = signature_phase(programs, modules, schemas, budget)?;
    body_phase(programs, signatures, budget)
}

/// Phase 1's product: the checked graph's interfaces (dependencies only) and
/// its initialization order, with empty per-source facts.
struct GraphPhase<'ast, 'src> {
    checked: CheckedModules<'ast, 'src>,
}

/// Phase 2's product: every nominal declared and every type import bound.
struct DeclarationPhase<'ast, 'src> {
    checked: CheckedModules<'ast, 'src>,
    initialization: ModuleInitialization,
    aliases: InterfaceGraph<'src>,
    locals: Vec<AHashMap<&'src str, (Span, bool)>>,
}

/// Phase 3's product: every nominal's schema and class hierarchy, and each
/// module's value scope so far: its classes' constructor values.
struct SchemaPhase<'ast, 'src> {
    declared: DeclarationPhase<'ast, 'src>,
    scopes: Vec<AHashMap<&'src str, SymbolId>>,
}

/// Phase 4's product: each module's top-level value scope, with its
/// imports, and the resolved interfaces.
struct SignaturePhase<'ast, 'src> {
    checked: CheckedModules<'ast, 'src>,
    initialization: ModuleInitialization,
    scopes: Vec<AHashMap<&'src str, SymbolId>>,
    /// Exports of `auto` bindings, `(module, export index)`: published once
    /// their module's bodies are checked.
    inferred: Vec<(usize, usize)>,
}

fn graph_phase<'ast, 'src, S>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet<S>,
    budget: &mut AllocationBudget<'_>,
) -> Result<GraphPhase<'ast, 'src>, AdmittedModuleCheckError> {
    let initialization_order = validate_graph(programs, modules, budget)?;
    let mut facts = budget
        .vector(AllocationClass::Scratch, programs.len())
        .map_err(|error| resource(modules.root(), error))?;
    for (module, program) in programs.iter().enumerate() {
        let source = ModuleFacts::new_admitted(program.source_identity(), budget)
            .map_err(|error| resource(module, error))?;
        budget
            .push(AllocationClass::Scratch, &mut facts, source)
            .map_err(|error| resource(module, error))?;
    }
    let mut interfaces = budget
        .vector(AllocationClass::Scratch, programs.len())
        .map_err(|error| resource(modules.root(), error))?;
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
        let root_directory = modules.modules[modules.root()]
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
    Ok(GraphPhase {
        checked: CheckedModules {
            declarations: DeclarationTables::default(),
            facts,
            interfaces,
            initialization_order,
            roots: modules.roots.clone(),
            root_names: modules.root_names.clone(),
        },
    })
}

fn declaration_phase<'ast, 'src>(
    programs: &[Program<'ast, 'src>],
    graph: GraphPhase<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<DeclarationPhase<'ast, 'src>, AdmittedModuleCheckError> {
    let GraphPhase { mut checked } = graph;
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
            .map_err(|error| AdmittedModuleCheckError { module, error })?;
    }
    let mut aliases = InterfaceGraph::new(programs, &checked, &locals)?;
    aliases.propagate();
    aliases.install_types(programs, &mut checked, budget)?;
    // Internal exports grant visibility. Only delivery roots publish an ABI;
    // actual constructor-value observations are seeded after body checking.
    for (module, program) in programs.iter().enumerate() {
        for export in program.exports {
            if export.kind != crate::ast::ExportKind::ConstructorValue {
                continue;
            }
            let class = checked.facts[module]
                .type_bindings
                .get(export.local.name)
                .copied()
                .filter(|identity| identity.is_class())
                .ok_or_else(|| {
                    error(
                        module,
                        export.local.span,
                        format!("constructor export `{}` is not a class", export.local.name),
                    )
                })?;
            let info = &mut checked.declarations.classes[class.index()];
            if info.external {
                return Err(error(
                    module,
                    export.local.span,
                    "constructor exports require a non-extern class",
                )
                .into());
            }
            info.published |= checked.roots.contains(&module);
        }
    }
    Ok(DeclarationPhase {
        checked,
        initialization,
        aliases,
        locals,
    })
}

fn schema_phase<'ast, 'src>(
    programs: &[Program<'ast, 'src>],
    declared: DeclarationPhase<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<SchemaPhase<'ast, 'src>, AdmittedModuleCheckError> {
    let mut declared = declared;
    let DeclarationPhase {
        checked,
        initialization,
        ..
    } = &mut declared;
    for (module, program) in programs.iter().enumerate() {
        let mut analyzer = Analyzer::new(
            &mut checked.facts[module],
            &mut checked.declarations,
            initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        analyzer
            .define_enums(program)
            .and_then(|()| analyzer.define_structs(program))
            .map_err(|error| AdmittedModuleCheckError { module, error })?;
    }
    super::struct_cycles::validate(&checked.declarations.structs).map_err(|(module, error)| {
        ModuleCheckError {
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
    let last_with_classes = programs
        .iter()
        .rposition(|program| declares_classes(program));
    let mut scopes = vec![AHashMap::default(); programs.len()];
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
            initialization,
            Some(module),
            budget,
        )
        .map_err(|error| resource(module, error))?;
        analyzer
            .define_classes(program)
            .and_then(|()| analyzer.define_extern_classes(program))
            .map_err(|error| AdmittedModuleCheckError { module, error })?;
        if module == last {
            analyzer
                .resolve_class_hierarchies()
                .map_err(|error| AdmittedModuleCheckError { module, error })?;
        }
        scopes[module] = analyzer.scopes.pop().expect("one module declaration scope");
    }
    checked
        .declarations
        .mark_observed_classes()
        .map_err(|(module, error)| ModuleCheckError {
            module: module.expect("class declaration owner"),
            error,
        })?;
    Ok(SchemaPhase { declared, scopes })
}

fn signature_phase<'ast, 'src, S>(
    programs: &[Program<'ast, 'src>],
    modules: &ModuleSet<S>,
    schemas: SchemaPhase<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<SignaturePhase<'ast, 'src>, AdmittedModuleCheckError> {
    let DeclarationPhase {
        mut checked,
        mut initialization,
        mut aliases,
        locals,
    } = schemas.declared;
    let mut schema_scopes = schemas.scopes;
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
        analyzer.scopes[0] = std::mem::take(&mut schema_scopes[module]);
        let registration = (|| {
            analyzer.declare_functions(program)?;
            for item in program.items {
                let Item::Stmt(Stmt::VarDecl(decl, ..)) = item else {
                    continue;
                };
                if decl.ty.is_auto() {
                    continue;
                }
                let mut ty = analyzer.resolve_value_type(decl.ty, "module binding")?;
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
        registration.map_err(|error| AdmittedModuleCheckError { module, error })?;
        scopes.push(analyzer.scopes.pop().expect("one module declaration scope"));
    }
    aliases.finish(programs, &mut checked, &mut scopes, &locals, budget)?;
    let inferred = aliases
        .inferred
        .iter()
        .map(|&(_, module, index)| (module, index))
        .collect();
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
        for ((_, span, import), &target) in sites.iter().zip(dynamic) {
            budget
                .work(
                    WorkKind::Analysis,
                    checked.interfaces[target].exports.len() as u64 + 1,
                )
                .map_err(|error| resource(module, error))?;
            let id = u32::try_from(target)
                .map_err(|_| error(module, *span, "dynamic module id range"))?;
            let facts = &mut checked.facts[module];
            facts.dynamic_import_modules.insert(*import, id);
            for export in &checked.interfaces[target].exports {
                if let InterfaceTarget::Value(symbol) = export.target {
                    facts
                        .dynamic_export_symbols
                        .insert((id, export.external), symbol);
                }
            }
        }
    }
    Ok(SignaturePhase {
        checked,
        initialization,
        scopes,
        inferred,
    })
}

fn body_phase<'ast, 'src>(
    programs: &[Program<'ast, 'src>],
    signatures: SignaturePhase<'ast, 'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<CheckedModules<'ast, 'src>, AdmittedModuleCheckError> {
    let SignaturePhase {
        mut checked,
        mut initialization,
        mut scopes,
        inferred,
    } = signatures;
    for index in 0..checked.initialization_order.len() {
        let module = checked.initialization_order[index];
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
            if let Item::Stmt(Stmt::VarDecl(decl, ..)) = item {
                if !decl.ty.is_auto() {
                    let symbol = analyzer.facts.identifier_symbols[&decl.name.id];
                    analyzer
                        .module_binding_declarations
                        .insert(decl.name.id, symbol);
                }
            }
        }
        analyzer
            .analyze_items(program)
            .map_err(|error| AdmittedModuleCheckError { module, error })?;
        let mut inferred_exports = Vec::new();
        for &(owner, index) in &inferred {
            if owner != module {
                continue;
            }
            let export = &program.exports[index];
            let symbol = analyzer.scopes[0]
                .get(export.local.name)
                .copied()
                .ok_or_else(|| {
                    error(module, export.local.span, "inferred export has no binding")
                })?;
            analyzer.record_identifier(export.local.id, symbol);
            inferred_exports.push(ModuleExport {
                external: export.exported.name,
                target: InterfaceTarget::Value(symbol),
                span: export.span,
            });
        }
        // Later modules own their own resolved namespace. Keep only their
        // pending scopes, not this completed analyzer's maps.
        drop(analyzer);
        if !inferred_exports.is_empty() {
            let exports = &mut checked.interfaces[module].exports;
            for export in inferred_exports {
                budget
                    .push(AllocationClass::Scratch, exports, export)
                    .map_err(|error| resource(module, error))?;
            }
            // Source order, as every other export is published.
            exports.sort_by_key(|export| export.span.start);
        }
    }
    // A first-class constructor observation can leave through an untyped host
    // operation. Preserve its identity and prototype using canonical bindings,
    // including imported aliases and used dynamic namespace members.
    checked
        .declarations
        .mark_constructor_observations(&checked.facts, budget)
        .map_err(|error| resource(checked.roots[0], error))?;
    checked
        .declarations
        .mark_observed_classes()
        .map_err(|(module, error)| ModuleCheckError {
            module: module.expect("class declaration owner"),
            error,
        })?;
    // Identity tests anywhere keep their classes (R13).
    checked.declarations.mark_tested_classes();
    for &root in &checked.roots {
        for export in &checked.interfaces[root].exports {
            let InterfaceTarget::Value(symbol) = export.target else {
                continue;
            };
            // A root's export crosses to its consumer (R6).
            checked
                .declarations
                .reflect(&checked.declarations.symbols[symbol.0 as usize].ty);
            if checked.declarations.symbols[symbol.0 as usize]
                .ty
                .contains_mutable_reference_parameters()
            {
                return Err(error(
                    root,
                    export.span,
                    "public exports do not yet support mutable-reference callable contracts",
                )
                .into());
            }
        }
    }
    // Crossings anywhere reflect their nominals, closed over fields (R6).
    checked.declarations.close_reflected();
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
) -> Result<Vec<ModuleId>, AdmittedModuleCheckError> {
    if programs.len() != modules.modules.len()
        || programs.is_empty()
        || modules.roots.is_empty()
        || modules.roots.len() != modules.root_names.len()
        || modules.roots.iter().any(|&root| root >= programs.len())
        || modules.eager.len() != programs.len()
    {
        return Err(error(
            0,
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
    // Shared graph owner defines the source and core initialization order:
    // the post-order over the entries in name order (the canonical
    // schedule); modules only `import()` reaches follow, in module order.
    let root = modules.root();
    let order = crate::module::initialization_order_admitted(
        &modules.roots,
        programs.len(),
        |module| modules.modules[module].dependencies.iter().copied(),
        budget,
    )
    .map_err(|failure| match failure {
        crate::module::StaticOrderError::Invalid(message) => {
            error(root, programs[root].span, message).into()
        }
        crate::module::StaticOrderError::Resources(reason) => resource(root, reason),
    })?;
    // Two entries entering one static cycle at different modules evaluate
    // it in different orders; one schedule cannot serve both (plan M3.3a
    // refusal, lifted by M3.3d). A module one entry loads with `import()`
    // and does not reach statically is an entry of its own here.
    if modules.roots.len() > 1 {
        let graph = modules
            .modules
            .iter()
            .map(|module| module.dependencies.clone())
            .collect::<Vec<_>>();
        let dynamic = modules
            .modules
            .iter()
            .map(|module| module.dynamic_dependencies.clone())
            .collect::<Vec<_>>();
        budget
            .work(
                WorkKind::Analysis,
                graph.len() as u64 * modules.roots.len() as u64,
            )
            .map_err(|error| resource(root, error))?;
        let cycles = crate::module::static_cycles(&graph);
        let mut roots = modules.roots.clone();
        let mut names = modules
            .root_names
            .iter()
            .map(|name| format!("entries `{name}`"))
            .collect::<Vec<_>>();
        for module in crate::module::lazy_roots(&modules.roots, &graph, &dynamic) {
            roots.push(module);
            names.push(format!(
                "`import(\"{}\")`",
                modules.modules[module].path.display()
            ));
        }
        let entered = crate::module::cycle_entries(&roots, &graph, &cycles);
        for (first, first_entries) in entered.iter().enumerate() {
            for (second, second_entries) in entered.iter().enumerate().skip(first + 1) {
                for &(cycle, at) in first_entries {
                    let Some(&(_, other)) =
                        second_entries.iter().find(|&&(known, _)| known == cycle)
                    else {
                        continue;
                    };
                    if other == at {
                        continue;
                    }
                    let members = (0..graph.len())
                        .filter(|&module| cycles[module] == Some(cycle))
                        .map(|module| modules.modules[module].path.display().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let (first_name, second_name) = (
                        names[first].trim_start_matches("entries "),
                        names[second].trim_start_matches("entries "),
                    );
                    return Err(error(
                        roots[second],
                        programs[roots[second]].span,
                        format!(
                            "entries {first_name} and {second_name} enter the import cycle of {members} at different modules, so they evaluate it in different orders; one entry must import the cycle through the same module as the other"
                        ),
                    )
                    .into());
                }
            }
        }
    }
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
                .map_err(|error| resource(modules.root(), error))?;
            if actual != expected {
                same = false;
                break;
            }
        }
    }
    if !same {
        return Err(error(
            root,
            programs[root].span,
            "direct module initialization order does not match ordered dependencies",
        )
        .into());
    }
    Ok(order)
}

fn preflight_source<'ast, 'src>(
    module: ModuleId,
    program: &Program<'ast, 'src>,
) -> Result<AHashMap<&'src str, (Span, bool)>, ModuleCheckError> {
    // Foreign imports are admitted by the graph check; each local is an
    // extern value declared in this module.
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
            Item::Stmt(Stmt::VarDecl(decl, ..)) => declare(decl.name, decl.ty.is_auto())?,
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
    // A class's constructor may share its external name with the export of
    // the class's type: one name, a type and its runtime value.
    let mut exports = AHashSet::default();
    for export in program.exports {
        let constructor = export.kind == crate::ast::ExportKind::ConstructorValue;
        if !exports.insert((export.exported.name, constructor)) {
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

/// One producer/consumer graph, with nominal types seeded before value
/// contracts. Every node becomes ready once, across both phases. Imports and
/// exports of every nominal kind are nodes like values: a type import binds
/// the exporter's identity in the importer's scope, under the local alias.
struct InterfaceGraph<'src> {
    nodes: Vec<AliasNode>,
    exports: Vec<AHashMap<&'src str, usize>>,
    /// `export constructor` nodes, by external name: a name may also export
    /// the class's type.
    constructors: Vec<AHashMap<&'src str, usize>>,
    imports: Vec<AHashMap<&'src str, usize>>,
    /// A dual import retains both namespaces. Its primary node carries the
    /// nominal; this node carries the explicitly exported constructor value.
    constructor_imports: Vec<AHashMap<&'src str, usize>>,
    values: Vec<(usize, usize, &'src str)>,
    /// Exports of `auto` bindings: `(node, module, export index)`. Their
    /// type is known only once the module's bodies are checked, so no other
    /// module may import them; they are published after the bodies.
    inferred: Vec<(usize, usize, usize)>,
    ready: VecDeque<usize>,
}
impl<'src> InterfaceGraph<'src> {
    fn new<'ast>(
        programs: &[Program<'ast, 'src>],
        checked: &CheckedModules<'ast, 'src>,
        locals: &[AHashMap<&'src str, (Span, bool)>],
    ) -> Result<Self, ModuleCheckError> {
        let mut graph = Self {
            nodes: Vec::new(),
            exports: vec![AHashMap::default(); programs.len()],
            constructors: vec![AHashMap::default(); programs.len()],
            imports: vec![AHashMap::default(); programs.len()],
            constructor_imports: vec![AHashMap::default(); programs.len()],
            values: Vec::new(),
            inferred: Vec::new(),
            ready: VecDeque::new(),
        };
        for (module, program) in programs.iter().enumerate() {
            for export in program.exports {
                let node = graph.nodes.len();
                if export.kind == crate::ast::ExportKind::ConstructorValue {
                    graph.constructors[module].insert(export.exported.name, node);
                } else {
                    graph.exports[module].insert(export.exported.name, node);
                }
                graph.nodes.push(AliasNode {
                    target: None,
                    consumers: Vec::new(),
                });
            }
            for import in program.imports {
                for specifier in import.specifiers {
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
                    let Some(&producer) = graph.exports[dependency]
                        .get(specifier.imported.name)
                        .or_else(|| graph.constructors[dependency].get(specifier.imported.name))
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
                    if let (Some(_), Some(&constructor)) = (
                        graph.exports[dependency].get(specifier.imported.name),
                        graph.constructors[dependency].get(specifier.imported.name),
                    ) {
                        let node = graph.nodes.len();
                        graph.nodes.push(AliasNode {
                            target: None,
                            consumers: Vec::new(),
                        });
                        graph.constructor_imports[module].insert(specifier.local.name, node);
                        graph.nodes[constructor].consumers.push(node);
                    }
                }
            }
            for (index, export) in program.exports.iter().enumerate() {
                if export.kind == crate::ast::ExportKind::ConstructorValue {
                    let node = graph.constructors[module][export.exported.name];
                    // Local values are seeded after schemas. An imported
                    // constructor follows its value edge, including through
                    // barrels and cycles; validate its nominal after the type
                    // namespace has propagated.
                    if checked.facts[module]
                        .type_bindings
                        .contains_key(export.local.name)
                    {
                        graph.values.push((node, module, export.local.name));
                    } else if let Some(&producer) = graph.constructor_imports[module]
                        .get(export.local.name)
                        .or_else(|| graph.imports[module].get(export.local.name))
                    {
                        graph.nodes[producer].consumers.push(node);
                    } else {
                        return Err(error(
                            module,
                            export.local.span,
                            format!("constructor export `{}` is not a class", export.local.name),
                        ));
                    }
                    continue;
                }
                let node = graph.exports[module][export.exported.name];
                let local_value = locals[module].get(export.local.name);
                let local_type = checked.facts[module]
                    .type_bindings
                    .get(export.local.name)
                    .copied();
                let declared_type = checked.view(module).unwrap().export_target(export.local.id);
                let direct_value = local_value.is_some_and(|(span, _)| *span == export.local.span);
                let target = if let Some(target) = declared_type {
                    Some(target)
                } else if direct_value {
                    None
                } else if local_type.is_some() && local_value.is_some() {
                    return Err(error(
                        module,
                        export.local.span,
                        "ambiguous export names both a value and a type; export the declaration directly",
                    ));
                } else {
                    local_type.map(InterfaceTarget::Type)
                };
                if let Some(target) = target {
                    graph.seed(node, target);
                } else if let Some((_, inferred)) = local_value {
                    if *inferred {
                        graph.inferred.push((node, module, index));
                    } else {
                        graph.values.push((node, module, export.local.name));
                    }
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
        for &(node, module, index) in &graph.inferred {
            if !graph.nodes[node].consumers.is_empty() {
                return Err(error(
                    module,
                    programs[module].exports[index].local.span,
                    "direct module checking does not yet support inferred export interfaces; add an explicit type",
                ));
            }
        }
        Ok(graph)
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
    /// Binds every imported nominal in its importer's type scope, under the
    /// local name. Types resolve before any schema or signature is read.
    fn install_types<'ast>(
        &self,
        programs: &[Program<'ast, 'src>],
        checked: &mut CheckedModules<'ast, 'src>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AdmittedModuleCheckError> {
        for (module, program) in programs.iter().enumerate() {
            for import in program.imports {
                for specifier in import.specifiers {
                    let Some(&node) = self.imports[module].get(specifier.local.name) else {
                        continue;
                    };
                    let Some(InterfaceTarget::Type(identity)) = self.nodes[node].target else {
                        continue;
                    };
                    let ty = checked
                        .view(module)
                        .and_then(|view| view.nominal_type(identity))
                        .expect("an interface names a declared nominal");
                    let ty = checked
                        .declarations
                        .types
                        .intern(&ty, budget)
                        .map_err(|error| resource(module, error))?;
                    let facts = &mut checked.facts[module];
                    if facts
                        .type_bindings
                        .insert(specifier.local.name, identity)
                        .is_some()
                    {
                        return Err(error(
                            module,
                            specifier.local.span,
                            format!("duplicate module type binding `{}`", specifier.local.name),
                        )
                        .into());
                    }
                    facts
                        .binding_types
                        .insert(specifier.local.id, BindingType::Inline(ty));
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
    ) -> Result<(), AdmittedModuleCheckError> {
        for index in 0..self.values.len() {
            let (node, module, name) = self.values[index];
            self.seed(node, InterfaceTarget::Value(scopes[module][name]));
        }
        self.propagate();
        for (module, program) in programs.iter().enumerate() {
            for (index, import) in program.imports.iter().enumerate() {
                let dependency = checked.interfaces[module].dependencies[index];
                for specifier in import.specifiers {
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
                    let constructor = self.constructor_imports[module]
                        .get(specifier.local.name)
                        .map(|&node| {
                            self.nodes[node].target.ok_or_else(|| {
                                error(
                                    module,
                                    specifier.imported.span,
                                    "cyclic constructor import cannot be resolved",
                                )
                            })
                        })
                        .transpose()?;
                    for target in [Some(target), constructor].into_iter().flatten() {
                        if let InterfaceTarget::Value(symbol) = target {
                            if locals[module].contains_key(specifier.local.name)
                                || scopes[module].contains_key(specifier.local.name)
                            {
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
                                .insert(specifier.local.id, BindingType::Symbol(symbol));
                            facts.record_identifier(
                                &mut checked.declarations,
                                specifier.local.id,
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
                                    node: specifier.local.id,
                                },
                            )
                            .map_err(|error| resource(module, error))?;
                    }
                }
            }
            for export in program.exports {
                let nodes = if export.kind == crate::ast::ExportKind::ConstructorValue {
                    &self.constructors[module]
                } else {
                    &self.exports[module]
                };
                let Some(&node) = nodes.get(export.exported.name) else {
                    continue;
                };
                if self.inferred.iter().any(|&(inferred, ..)| inferred == node) {
                    continue;
                }
                let target = self.nodes[node].target.ok_or_else(|| {
                    error(module, export.span, "cyclic export cannot be resolved")
                })?;
                if export.kind == crate::ast::ExportKind::ConstructorValue
                    && !matches!(target, InterfaceTarget::Value(_))
                {
                    return Err(error(
                        module,
                        export.local.span,
                        "constructor re-export requires an imported constructor value",
                    )
                    .into());
                }
                let direct_value = locals[module]
                    .get(export.local.name)
                    .is_some_and(|(span, _)| *span == export.local.span);
                let direct_type = matches!(
                    checked.view(module).unwrap().export_target(export.local.id),
                    Some(InterfaceTarget::Type(_))
                );
                if !direct_value
                    && !direct_type
                    && scopes[module].contains_key(export.local.name)
                    && checked.facts[module]
                        .type_bindings
                        .get(export.local.name)
                        .is_some_and(|identity| identity.is_struct())
                {
                    return Err(error(
                        module,
                        export.local.span,
                        "ambiguous export names both a value and a type; export the declaration directly",
                    ).into());
                }
                let facts = &mut checked.facts[module];
                match target {
                    InterfaceTarget::Value(symbol) => {
                        facts.record_identifier(&mut checked.declarations, export.local.id, symbol)
                    }
                    InterfaceTarget::Type(identity) => {
                        let ty = CheckedView {
                            declarations: &checked.declarations,
                            facts: &checked.facts[module],
                        }
                        .nominal_type(identity)
                        .expect("an interface names a declared nominal");
                        let ty = checked
                            .declarations
                            .types
                            .intern(&ty, budget)
                            .map_err(|error| resource(module, error))?;
                        // A class declaration's own name keeps its
                        // constructor binding.
                        checked.facts[module]
                            .binding_types
                            .entry(export.local.id)
                            .or_insert(BindingType::Inline(ty));
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
