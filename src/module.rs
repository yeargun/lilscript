use crate::ast::ExprKind;
use std::collections::hash_map::Entry;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};
use bumpalo::collections::Vec as BumpVec;
use bumpalo::Bump;

use crate::ast::{
    Argument, ArrayBinding, ArrayElement, ArrowBody, CatchBinding, CatchClause, ClassDecl,
    ClassMember, ConstructorDecl, DynamicExport, DynamicImportDecl, EnumDecl, ExportDecl, Expr,
    ExternClassDecl, ExternClassMember, ExternDecl, ExternGlobalDecl, FieldDecl, ForInitializer,
    ForeignImportDecl, FunctionDecl, Ident, ImportSpecifier, Item, MatchArm, MatchPattern,
    ModuleBinding, Param, ParameterType, Program, RecordBinding, RecordElement, RecordEntry, Stmt,
    StructDecl, TemplatePart, TypeKind, TypeRef, VarDecl,
};
use crate::compilation_policy::{BudgetLedger, WorkDomain, WorkKind};
use crate::config::ProjectConfig;
use crate::output_budget::{
    AllocationBudget, AllocationClass::Retained, AllocationError, RetainedCharge,
};
use crate::package::{load_package_resolver, PackageResolver};
use crate::parser::{parse_source, AdmittedArena, AdmittedParseError, ParseError, ParsedSources};
use crate::span::Span;

#[path = "module_source_arena.rs"]
mod source_arena;
pub(crate) use source_arena::StableSourceArena;

pub type ModuleId = usize;

/// Dependency-first evaluation order over the caller's canonical static graph.
/// Active edges close import cycles; each reachable module finishes once. The
/// stack borrows neighbor iterators instead of copying edges or using the Rust
/// call stack, so source checking and semantic publication share one schedule.
pub(crate) fn static_evaluation_order<I: IntoIterator<Item = usize>>(
    root: usize,
    module_count: usize,
    dependencies: impl Fn(usize) -> I,
) -> Result<Vec<usize>, &'static str> {
    static_evaluation_order_admitted(
        root,
        module_count,
        dependencies,
        &mut AllocationBudget::new(None),
    )
    .map_err(|error| match error {
        StaticOrderError::Invalid(reason) => reason,
        StaticOrderError::Resources(_) => "static module schedule exceeds capacity",
    })
}

#[derive(Debug)]
pub(crate) enum StaticOrderError {
    Invalid(&'static str),
    Resources(AllocationError),
}

impl From<AllocationError> for StaticOrderError {
    fn from(error: AllocationError) -> Self {
        Self::Resources(error)
    }
}

pub(crate) fn static_evaluation_order_admitted<I: IntoIterator<Item = usize>>(
    root: usize,
    module_count: usize,
    dependencies: impl Fn(usize) -> I,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, StaticOrderError> {
    use crate::output_budget::AllocationClass::Scratch;
    budget.work(WorkKind::Analysis, 1)?;
    if root >= module_count {
        return Err(StaticOrderError::Invalid(
            "static module root is out of bounds",
        ));
    }
    let mut order = budget.vector(Scratch, module_count)?;
    let mut scope = budget.scope();
    let mut states = scope.filled(Scratch, module_count, 0u8)?;
    let mut stack = scope.vector(Scratch, module_count)?;
    states[root] = 1;
    stack.push((root, dependencies(root).into_iter()));
    while let Some((module, neighbors)) = stack.last_mut() {
        scope.work(WorkKind::Analysis, 1)?;
        if let Some(child) = neighbors.next() {
            if child >= module_count {
                return Err(StaticOrderError::Invalid(
                    "static module dependency is out of bounds",
                ));
            }
            if states[child] == 0 {
                states[child] = 1;
                stack.push((child, dependencies(child).into_iter()));
            }
        } else {
            let module = *module;
            stack.pop();
            states[module] = 2;
            order.push(module);
        }
    }
    Ok(order)
}

/// Static evaluation order from `root`, then every module only `import()`
/// reaches, in module order. Such a module is initialization-free, so its
/// place is unobservable; last, it runs after everything it may read.
pub(crate) fn initialization_order_admitted<I: IntoIterator<Item = usize>>(
    root: usize,
    module_count: usize,
    dependencies: impl Fn(usize) -> I,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, StaticOrderError> {
    use crate::output_budget::AllocationClass::Scratch;
    let mut order = static_evaluation_order_admitted(root, module_count, dependencies, budget)?;
    if order.len() == module_count {
        return Ok(order);
    }
    let mut reached = budget.filled(Scratch, module_count, false)?;
    for &module in &order {
        reached[module] = true;
    }
    budget.work(WorkKind::Analysis, module_count as u64)?;
    for (module, reached) in reached.iter().enumerate() {
        if !reached {
            order.push(module);
        }
    }
    Ok(order)
}

#[derive(Debug, Clone)]
pub struct ModuleSource<S = String> {
    pub path: PathBuf,
    pub source: S,
    pub dependencies: Vec<ModuleId>,
    pub foreign_dependencies: Vec<ForeignModuleSource>,
    pub dynamic_dependencies: Vec<ModuleId>,
    pub offset: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForeignModuleSource {
    pub specifier: String,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ModuleSet<S = String> {
    pub modules: Vec<ModuleSource<S>>,
    pub dependency_order: Vec<ModuleId>,
    pub root: ModuleId,
    pub eager: Vec<bool>,
    pub for_of_specialize_family: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleError {
    pub path: PathBuf,
    pub source: String,
    pub span: Span,
    pub message: String,
}

impl ModuleError {
    pub(crate) fn new(
        path: impl Into<PathBuf>,
        source: impl Into<String>,
        span: Span,
        message: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
            span,
            message: message.into(),
        }
    }

    pub fn from_parse(path: &Path, source: &str, error: ParseError) -> Self {
        Self::new(path, source, error.span, error.message)
    }
}

impl std::fmt::Display for ModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.message)
    }
}

impl std::error::Error for ModuleError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Complete,
}

#[derive(Debug)]
pub(crate) enum ModuleDiscoveryError {
    Module(ModuleError),
    Resources(AllocationError),
}

impl From<ModuleError> for ModuleDiscoveryError {
    fn from(error: ModuleError) -> Self {
        Self::Module(error)
    }
}

impl From<AllocationError> for ModuleDiscoveryError {
    fn from(error: AllocationError) -> Self {
        Self::Resources(error)
    }
}

impl ModuleDiscoveryError {
    fn into_module(self, root: &Path) -> ModuleError {
        match self {
            Self::Module(error) => error,
            Self::Resources(error) => ModuleError::new(root, "", Span::empty(0), error.to_string()),
        }
    }
}

/// Only source String capacities, not module metadata or parser allocations.
/// The admitting factory drops its ModuleSet before releasing this charge.
#[derive(Debug)]
#[must_use = "retain until source buffers drop, then release into the admitting ledger"]
pub(crate) struct SourceBufferCharge(RetainedCharge<()>);

impl SourceBufferCharge {
    pub(crate) fn bytes(&self) -> u64 {
        self.0.bytes()
    }

    pub(crate) fn release(self, ledger: &mut BudgetLedger) -> Result<(), (Self, AllocationError)> {
        self.0
            .discard(&(), ledger)
            .map_err(|(charge, error)| (Self(charge), error))
    }
}

pub fn discover_modules(root: &Path) -> Result<ModuleSet, ModuleError> {
    discover_modules_inner(root, None, None, &mut AllocationBudget::new(None))
        .map_err(|error| error.into_module(root))
}

pub fn discover_modules_with_source(root: &Path, source: &str) -> Result<ModuleSet, ModuleError> {
    discover_modules_inner(root, Some(source), None, &mut AllocationBudget::new(None))
        .map_err(|error| error.into_module(root))
}

pub fn discover_modules_configured(
    root: &Path,
    config: &ProjectConfig,
) -> Result<ModuleSet, ModuleError> {
    discover_modules_configured_inner(root, None, config, &mut AllocationBudget::new(None))
        .map_err(|error| error.into_module(root))
}

pub fn discover_modules_configured_with_source(
    root: &Path,
    source: &str,
    config: &ProjectConfig,
) -> Result<ModuleSet, ModuleError> {
    discover_modules_configured_inner(root, Some(source), config, &mut AllocationBudget::new(None))
        .map_err(|error| error.into_module(root))
}

pub(crate) fn discover_modules_configured_admitted(
    root: &Path,
    config: &ProjectConfig,
    ledger: &mut BudgetLedger,
) -> Result<(ModuleSet, SourceBufferCharge), ModuleDiscoveryError> {
    let mut budget = AllocationBudget::new(Some((ledger, WorkDomain::Baseline)));
    let modules = discover_modules_configured_inner(root, None, config, &mut budget)?;
    let bytes = budget.retained_bytes(Retained);
    let charge = SourceBufferCharge(budget.detach_retained((), bytes)?);
    Ok((modules, charge))
}

fn discover_modules_configured_inner(
    root: &Path,
    root_source: Option<&str>,
    config: &ProjectConfig,
    budget: &mut AllocationBudget<'_>,
) -> Result<ModuleSet, ModuleDiscoveryError> {
    discover_configured_with_storage(root, root_source, config, OwnedSources { budget })
        .map(|(modules, ())| modules)
}

/// The factory owns both arenas. Original discovery ASTs are retained in
/// canonical module order; no source or syntax storage lives inside the graph.
/// `root_source`, when given, replaces the entry file's text (an editor's
/// unsaved buffer); every other module is read from disk.
pub(crate) fn discover_parsed_modules_admitted<'ast, 'src>(
    root: &Path,
    root_source: Option<&str>,
    config: &ProjectConfig,
    sources: &'src StableSourceArena,
    syntax: &'ast AdmittedArena<'_>,
) -> Result<(ModuleSet<&'src str>, ParsedSources<'ast, 'src>), ModuleDiscoveryError> {
    discover_configured_with_storage(
        root,
        root_source,
        config,
        RetainedSources {
            sources,
            syntax,
            parsed: syntax.parsed_sources(),
        },
    )
}

fn discover_configured_with_storage<S: DiscoveryStorage>(
    root: &Path,
    root_source: Option<&str>,
    config: &ProjectConfig,
    mut storage: S,
) -> Result<(ModuleSet<S::Source>, S::Parsed), ModuleDiscoveryError> {
    storage.work(1)?;
    let resolver = load_package_resolver(config).map_err(|error| {
        ModuleError::new(
            error.path,
            root_source.unwrap_or(""),
            Span::empty(0),
            error.message,
        )
    })?;
    let (mut modules, parsed) = discover_with_storage(root, root_source, resolver, storage)?;
    modules.for_of_specialize_family = config.optimization.for_of_specialize_family();
    Ok((modules, parsed))
}

fn discover_modules_inner(
    root: &Path,
    root_source: Option<&str>,
    package_resolver: Option<PackageResolver>,
    budget: &mut AllocationBudget<'_>,
) -> Result<ModuleSet, ModuleDiscoveryError> {
    discover_with_storage(root, root_source, package_resolver, OwnedSources { budget })
        .map(|(modules, ())| modules)
}

fn discover_with_storage<S: DiscoveryStorage>(
    root: &Path,
    root_source: Option<&str>,
    package_resolver: Option<PackageResolver>,
    mut storage: S,
) -> Result<(ModuleSet<S::Source>, S::Parsed), ModuleDiscoveryError> {
    storage.work(1)?;
    let root_path = canonical_module_path(root).map_err(|message| {
        ModuleError::new(root, root_source.unwrap_or(""), Span::empty(0), message)
    })?;
    let mut overrides = AHashMap::default();
    if let Some(source) = root_source {
        overrides.insert(root_path.clone(), storage.copy_override(source)?);
    }
    let mut loader = ModuleLoader {
        modules: Vec::new(),
        by_path: AHashMap::default(),
        states: Vec::new(),
        dependency_order: Vec::new(),
        overrides,
        package_resolver,
        storage,
    };
    let root = loader.visit(&root_path, None)?;
    let mut offset = 0usize;
    for module in &mut loader.modules {
        loader.storage.work(1)?;
        module.offset = offset;
        offset = offset
            .saturating_add(module.source.as_ref().len())
            .saturating_add(1);
    }
    let mut eager = vec![false; loader.modules.len()];
    let mut pending = vec![root];
    while let Some(module) = pending.pop() {
        loader.storage.work(1)?;
        if std::mem::replace(&mut eager[module], true) {
            continue;
        }
        pending.extend(loader.modules[module].dependencies.iter().copied());
    }
    Ok((
        ModuleSet {
            modules: loader.modules,
            dependency_order: loader.dependency_order,
            root,
            eager,
            for_of_specialize_family: 0,
        },
        loader.storage.into_parsed(),
    ))
}

#[cfg(test)]
#[path = "module_admission_tests.rs"]
mod admission_tests;

#[cfg(test)]
#[path = "module_parse_once_tests.rs"]
mod parse_once_tests;

fn read_module_source(
    path: &Path,
    budget: &mut AllocationBudget<'_>,
) -> Result<String, ModuleDiscoveryError> {
    let read_error = |error: io::Error| {
        ModuleError::new(
            path,
            "",
            Span::empty(0),
            format!("failed to read module {}: {error}", path.display()),
        )
    };
    budget.work(WorkKind::Analysis, 1)?;
    let mut file = fs::File::open(path).map_err(read_error)?;
    let mut source = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        // The fixed stack buffer lets reads discover length without speculative
        // heap growth. Every heap destination is admitted before bytes move.
        budget.work(WorkKind::Analysis, 1)?;
        let count = match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(read_error(error).into()),
        };
        budget.work(WorkKind::Analysis, count as u64)?;
        budget.extend_copy(Retained, &mut source, &chunk[..count])?;
    }
    String::from_utf8(source).map_err(|error| {
        drop(error);
        read_error(io::Error::new(
            io::ErrorKind::InvalidData,
            "stream did not contain valid UTF-8",
        ))
        .into()
    })
}

type ImportSites = Vec<(String, Span)>;

fn collect_imports(program: &Program<'_, '_>) -> (ImportSites, ImportSites, ImportSites) {
    let imports = program
        .imports
        .iter()
        .map(|import| (import.source.to_string(), import.span))
        .collect();
    let foreign = program
        .foreign_imports
        .iter()
        .map(|import| (import.source.to_string(), import.span))
        .collect();
    let mut dynamic = Vec::new();
    collect_program_dynamic_imports(program, &mut dynamic);
    let dynamic = dynamic
        .into_iter()
        .map(|(source, span)| (source.to_string(), span))
        .collect();
    (imports, foreign, dynamic)
}

fn discovery_parse_error(
    path: &Path,
    source: &str,
    error: AdmittedParseError,
) -> ModuleDiscoveryError {
    match error {
        AdmittedParseError::Syntax(error) => ModuleError::from_parse(path, source, error).into(),
        AdmittedParseError::Resource(error) => ModuleDiscoveryError::Resources(error),
    }
}

fn discover_imports(
    path: &Path,
    source: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<(ImportSites, ImportSites, ImportSites), ModuleDiscoveryError> {
    budget.with_ledger(|owner| match owner {
        Some((ledger, domain)) => {
            let arena = AdmittedArena::new(ledger, domain);
            let program = arena
                .parse(source)
                .map_err(|error| discovery_parse_error(path, source, error))?;
            Ok(collect_imports(&program))
        }
        None => {
            let arena = Bump::new();
            let program = parse_source(&arena, source)
                .map_err(|error| ModuleError::from_parse(path, source, error))?;
            Ok(collect_imports(&program))
        }
    })
}

/// Only source/syntax ownership varies. Resolution, graph identities and
/// traversal all remain in the one loader below.
trait DiscoveryStorage {
    type Source: AsRef<str>;
    type Parsed;
    fn work(&mut self, units: u64) -> Result<(), AllocationError>;
    fn read(&mut self, path: &Path) -> Result<Self::Source, ModuleDiscoveryError>;
    fn copy_override(&mut self, source: &str) -> Result<Self::Source, AllocationError>;
    fn imports(
        &mut self,
        path: &Path,
        source: &Self::Source,
    ) -> Result<(ImportSites, ImportSites, ImportSites), ModuleDiscoveryError>;
    fn into_parsed(self) -> Self::Parsed;
}

struct OwnedSources<'budget, 'ledger> {
    budget: &'budget mut AllocationBudget<'ledger>,
}

impl DiscoveryStorage for OwnedSources<'_, '_> {
    type Source = String;
    type Parsed = ();
    fn work(&mut self, units: u64) -> Result<(), AllocationError> {
        self.budget.work(WorkKind::Analysis, units)
    }
    fn read(&mut self, path: &Path) -> Result<String, ModuleDiscoveryError> {
        read_module_source(path, self.budget)
    }
    fn copy_override(&mut self, source: &str) -> Result<String, AllocationError> {
        self.budget.string(Retained, source)
    }
    fn imports(
        &mut self,
        path: &Path,
        source: &String,
    ) -> Result<(ImportSites, ImportSites, ImportSites), ModuleDiscoveryError> {
        discover_imports(path, source, self.budget)
    }
    fn into_parsed(self) {}
}

struct RetainedSources<'ast, 'src, 'ledger> {
    sources: &'src StableSourceArena,
    syntax: &'ast AdmittedArena<'ledger>,
    parsed: ParsedSources<'ast, 'src>,
}

impl<'ast, 'src> DiscoveryStorage for RetainedSources<'ast, 'src, '_> {
    type Source = &'src str;
    type Parsed = ParsedSources<'ast, 'src>;
    fn work(&mut self, units: u64) -> Result<(), AllocationError> {
        self.syntax.with_ledger(|ledger, domain| {
            ledger
                .charge(domain, WorkKind::Analysis, units)
                .map_err(Into::into)
        })
    }
    fn read(&mut self, path: &Path) -> Result<&'src str, ModuleDiscoveryError> {
        self.syntax.with_ledger(|ledger, domain| {
            let mut budget = AllocationBudget::new(Some((ledger, domain)));
            let text = read_module_source(path, &mut budget)?;
            let stored = budget.with_ledger(|owner| {
                let (ledger, _) = owner.expect("retained discovery owns the factory ledger");
                self.sources.store(&text, ledger)
            });
            // Source backing has its own persistent owner. Only the temporary
            // read buffer is released when this short allocation scope ends.
            drop(text);
            stored.map_err(Into::into)
        })
    }
    fn copy_override(&mut self, source: &str) -> Result<&'src str, AllocationError> {
        self.syntax
            .with_ledger(|ledger, _| self.sources.store(source, ledger))
    }
    fn imports(
        &mut self,
        path: &Path,
        source: &&'src str,
    ) -> Result<(ImportSites, ImportSites, ImportSites), ModuleDiscoveryError> {
        // Parsing and list growth happen outside any with_ledger callback.
        let program = self
            .syntax
            .parse(*source)
            .map_err(|error| discovery_parse_error(path, source, error))?;
        let imports = collect_imports(&program);
        self.parsed
            .push(program)
            .map_err(|error| discovery_parse_error(path, source, error))?;
        Ok(imports)
    }
    fn into_parsed(self) -> Self::Parsed {
        self.parsed
    }
}

struct ModuleLoader<S: DiscoveryStorage> {
    modules: Vec<ModuleSource<S::Source>>,
    by_path: AHashMap<PathBuf, ModuleId>,
    states: Vec<VisitState>,
    dependency_order: Vec<ModuleId>,
    overrides: AHashMap<PathBuf, S::Source>,
    package_resolver: Option<PackageResolver>,
    storage: S,
}

impl<S: DiscoveryStorage> ModuleLoader<S> {
    fn visit(
        &mut self,
        requested: &Path,
        import_site: Option<(ModuleId, Span)>,
    ) -> Result<ModuleId, ModuleDiscoveryError> {
        self.storage.work(1)?;
        let path = canonical_module_path(requested)
            .map_err(|message| self.import_error(import_site, requested, message))?;
        if let Some(&id) = self.by_path.get(&path) {
            if self.states[id] == VisitState::Visiting {
                // Module interfaces are linked to a fixed point below. Keep the
                // back-edge here and let each member of the SCC be emitted once.
                return Ok(id);
            }
            return Ok(id);
        }

        let source = match self.overrides.remove(&path) {
            Some(source) => source,
            None => self.storage.read(&path).map_err(|error| match error {
                ModuleDiscoveryError::Module(error) => {
                    self.import_error(import_site, &path, error.message).into()
                }
                resource => resource,
            })?,
        };
        self.storage.work(0)?;
        let (imports, foreign_imports, dynamic_imports) = self.storage.imports(&path, &source)?;

        let id = self.modules.len();
        self.by_path.insert(path.clone(), id);
        self.states.push(VisitState::Visiting);
        self.modules.push(ModuleSource {
            path: path.clone(),
            source,
            dependencies: Vec::with_capacity(imports.len()),
            foreign_dependencies: Vec::with_capacity(foreign_imports.len()),
            dynamic_dependencies: Vec::with_capacity(dynamic_imports.len()),
            offset: 0,
        });
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut dependencies = Vec::with_capacity(imports.len());
        for (specifier, span) in imports {
            self.storage.work(1)?;
            let dependency_path =
                self.resolve_import_path(parent, &specifier)
                    .map_err(|message| {
                        ModuleError::new(&path, self.modules[id].source.as_ref(), span, message)
                    })?;
            let dependency = self.visit(&dependency_path, Some((id, span)))?;
            dependencies.push(dependency);
        }
        self.modules[id].dependencies = dependencies;
        let mut foreign_dependencies = Vec::with_capacity(foreign_imports.len());
        for (specifier, span) in foreign_imports {
            self.storage.work(1)?;
            let resolved = self
                .resolve_foreign_import_path(parent, &specifier)
                .map_err(|message| {
                    ModuleError::new(&path, self.modules[id].source.as_ref(), span, message)
                })?;
            foreign_dependencies.push(ForeignModuleSource {
                specifier,
                path: resolved,
            });
        }
        self.modules[id].foreign_dependencies = foreign_dependencies;
        let mut dynamic_dependencies = Vec::with_capacity(dynamic_imports.len());
        for (specifier, span) in dynamic_imports {
            self.storage.work(1)?;
            let dependency_path =
                self.resolve_import_path(parent, &specifier)
                    .map_err(|message| {
                        ModuleError::new(&path, self.modules[id].source.as_ref(), span, message)
                    })?;
            let dependency = self.visit(&dependency_path, Some((id, span)))?;
            dynamic_dependencies.push(dependency);
        }
        self.modules[id].dynamic_dependencies = dynamic_dependencies;
        self.states[id] = VisitState::Complete;
        self.dependency_order.push(id);
        Ok(id)
    }

    fn import_error(
        &self,
        site: Option<(ModuleId, Span)>,
        requested: &Path,
        message: String,
    ) -> ModuleError {
        match site {
            Some((parent, span)) => {
                let parent = &self.modules[parent];
                ModuleError::new(&parent.path, parent.source.as_ref(), span, message)
            }
            None => ModuleError::new(requested, "", Span::empty(0), message),
        }
    }
    fn resolve_import_path(&self, parent: &Path, specifier: &str) -> Result<PathBuf, String> {
        if specifier.starts_with('.') {
            return Ok(parent.join(specifier));
        }
        if Path::new(specifier).is_absolute() {
            return Err(format!("module path `{specifier}` must not be absolute"));
        }
        let resolver = self.package_resolver.as_ref().ok_or_else(|| {
            format!("package import `{specifier}` requires a locked dependency in lilscript.toml")
        })?;
        resolver
            .resolve(parent, specifier)
            .map_err(|error| error.message)
    }

    fn resolve_foreign_import_path(
        &self,
        parent: &Path,
        specifier: &str,
    ) -> Result<Option<PathBuf>, String> {
        if !specifier.starts_with('.') {
            if Path::new(specifier).is_absolute() {
                return Err(format!(
                    "foreign module path `{specifier}` must not be absolute"
                ));
            }
            return Ok(None);
        }
        let requested = parent.join(specifier);
        let extension = requested.extension().and_then(|value| value.to_str());
        let path = if extension.is_some() {
            requested
        } else {
            ["ts", "mts", "js", "mjs", "tsx", "jsx"]
                .into_iter()
                .map(|extension| requested.with_extension(extension))
                .find(|candidate| candidate.is_file())
                .ok_or_else(|| format!("cannot resolve foreign module `{specifier}`"))?
        };
        let extension = path.extension().and_then(|value| value.to_str());
        if !matches!(extension, Some("js" | "jsx" | "mjs" | "ts" | "tsx" | "mts")) {
            return Err(format!(
                "foreign module `{specifier}` must use a JavaScript or TypeScript extension"
            ));
        }
        path.canonicalize()
            .map(Some)
            .map_err(|error| format!("cannot resolve foreign module `{specifier}`: {error}"))
    }
}

fn canonical_module_path(path: &Path) -> Result<PathBuf, String> {
    let with_extension = if path.extension().is_none() {
        path.with_extension("lil")
    } else {
        path.to_path_buf()
    };
    if with_extension.extension().and_then(|value| value.to_str()) != Some("lil") {
        return Err(format!(
            "LilScript modules must use the `.lil` extension: {}",
            with_extension.display()
        ));
    }
    with_extension.canonicalize().map_err(|error| {
        format!(
            "cannot resolve LilScript module {}: {error}",
            with_extension.display()
        )
    })
}

#[cfg(test)]
fn resolve_import_path(parent: &Path, specifier: &str) -> Result<PathBuf, String> {
    let path = Path::new(specifier);
    if path.is_absolute() || !specifier.starts_with('.') {
        return Err(format!(
            "module path `{specifier}` must be relative and begin with `./` or `../`"
        ));
    }
    Ok(parent.join(path))
}

pub(crate) fn collect_program_dynamic_imports<'ast, 'src>(
    program: &Program<'ast, 'src>,
    imports: &mut Vec<(&'src str, Span)>,
) {
    for item in program.items {
        match item {
            Item::Enum(_) | Item::Struct(_) | Item::ExternGlobal(_) => {}
            Item::Class(class) => {
                for member in class.members {
                    match member {
                        ClassMember::Field(_) => {}
                        ClassMember::Constructor(constructor) => {
                            collect_param_dynamic_imports(constructor.params, imports);
                            collect_stmt_dynamic_imports(constructor.body, imports);
                        }
                        ClassMember::Method(function) => {
                            collect_function_dynamic_imports(function, imports)
                        }
                    }
                }
            }
            Item::ExternClass(class) => {
                for member in class.members {
                    if let ExternClassMember::Method(method) = member {
                        collect_param_dynamic_imports(method.params, imports);
                    }
                }
            }
            Item::Function(function) => collect_function_dynamic_imports(function, imports),
            Item::Extern(declaration) => collect_param_dynamic_imports(declaration.params, imports),
            Item::Stmt(statement) => {
                collect_stmt_dynamic_imports(std::slice::from_ref(statement), imports)
            }
        }
    }
}

fn collect_function_dynamic_imports<'ast, 'src>(
    function: &FunctionDecl<'ast, 'src>,
    imports: &mut Vec<(&'src str, Span)>,
) {
    collect_param_dynamic_imports(function.params, imports);
    collect_stmt_dynamic_imports(function.body, imports);
}

fn collect_param_dynamic_imports<'ast, 'src>(
    params: &[Param<'ast, 'src>],
    imports: &mut Vec<(&'src str, Span)>,
) {
    for default in params.iter().filter_map(|param| param.default.as_ref()) {
        collect_expr_dynamic_imports(default, imports);
    }
}

fn collect_stmt_dynamic_imports<'ast, 'src>(
    statements: &[Stmt<'ast, 'src>],
    imports: &mut Vec<(&'src str, Span)>,
) {
    for statement in statements {
        match statement {
            Stmt::VarDecl(declaration) => {
                if let Some(initializer) = &declaration.initializer {
                    collect_expr_dynamic_imports(initializer, imports);
                }
            }
            Stmt::ArrayDestructure { value, .. } | Stmt::RecordDestructure { value, .. } => {
                collect_expr_dynamic_imports(value, imports)
            }
            Stmt::Expr(expression) => collect_expr_dynamic_imports(expression, imports),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    collect_expr_dynamic_imports(value, imports);
                }
            }
            Stmt::Throw { value, .. } => collect_expr_dynamic_imports(value, imports),
            Stmt::SuperCall { args, .. } => {
                for argument in *args {
                    collect_expr_dynamic_imports(&argument.expression, imports);
                }
            }
            Stmt::Yield { value, .. } => collect_expr_dynamic_imports(value, imports),
            Stmt::Try {
                body,
                catch,
                finally,
                ..
            } => {
                collect_stmt_dynamic_imports(body, imports);
                if let Some(clause) = catch {
                    collect_stmt_dynamic_imports(clause.body, imports);
                }
                if let Some(body) = finally {
                    collect_stmt_dynamic_imports(body, imports);
                }
            }
            Stmt::Block { body, .. } => collect_stmt_dynamic_imports(body, imports),
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => {
                collect_expr_dynamic_imports(condition, imports);
                collect_stmt_dynamic_imports(std::slice::from_ref(*then_branch), imports);
                if let Some(else_branch) = else_branch {
                    collect_stmt_dynamic_imports(std::slice::from_ref(*else_branch), imports);
                }
            }
            Stmt::While {
                condition, body, ..
            } => {
                collect_expr_dynamic_imports(condition, imports);
                collect_stmt_dynamic_imports(std::slice::from_ref(*body), imports);
            }
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                ..
            } => {
                if let Some(initializer) = initializer {
                    match initializer {
                        ForInitializer::VarDecl(declaration) => {
                            if let Some(value) = &declaration.initializer {
                                collect_expr_dynamic_imports(value, imports);
                            }
                        }
                        ForInitializer::Expr(expression) => {
                            collect_expr_dynamic_imports(expression, imports)
                        }
                    }
                }
                if let Some(condition) = condition {
                    collect_expr_dynamic_imports(condition, imports);
                }
                if let Some(update) = update {
                    collect_expr_dynamic_imports(update, imports);
                }
                collect_stmt_dynamic_imports(std::slice::from_ref(*body), imports);
            }
            Stmt::ForIn { object, body, .. } => {
                collect_expr_dynamic_imports(object, imports);
                collect_stmt_dynamic_imports(std::slice::from_ref(*body), imports);
            }
            Stmt::ForOf { iterable, body, .. } => {
                collect_expr_dynamic_imports(iterable, imports);
                collect_stmt_dynamic_imports(std::slice::from_ref(*body), imports);
            }
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn collect_expr_dynamic_imports<'ast, 'src>(
    expression: &Expr<'ast, 'src>,
    imports: &mut Vec<(&'src str, Span)>,
) {
    match expression {
        Expr {
            kind: ExprKind::DynamicImport { source, span },
            ..
        } => imports.push((source, *span)),
        Expr {
            kind: ExprKind::ArrayLiteral { elements, .. },
            ..
        } => {
            for element in *elements {
                collect_expr_dynamic_imports(element.value(), imports);
            }
        }
        Expr {
            kind: ExprKind::RecordLiteral { entries, .. },
            ..
        }
        | Expr {
            kind: ExprKind::ObjectLiteral { entries, .. },
            ..
        } => {
            for entry in *entries {
                collect_expr_dynamic_imports(entry.value(), imports);
            }
        }
        Expr {
            kind: ExprKind::StructLiteral { values, .. },
            ..
        } => {
            for value in *values {
                collect_expr_dynamic_imports(value, imports);
            }
        }
        Expr {
            kind: ExprKind::New { args, .. },
            ..
        } => {
            for argument in *args {
                collect_expr_dynamic_imports(&argument.expression, imports);
            }
        }
        Expr {
            kind: ExprKind::Member { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::OptionalMember { object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Unary { expr: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Await { task: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::TypeCheck { value: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Update { target: object, .. },
            ..
        } => collect_expr_dynamic_imports(object, imports),
        Expr {
            kind: ExprKind::Call { callee, args, .. },
            ..
        } => {
            collect_expr_dynamic_imports(callee, imports);
            for argument in *args {
                collect_expr_dynamic_imports(&argument.expression, imports);
            }
        }
        Expr {
            kind: ExprKind::ArrowFunction { params, body, .. },
            ..
        } => {
            collect_param_dynamic_imports(params, imports);
            match body {
                ArrowBody::Expr(expression) => collect_expr_dynamic_imports(expression, imports),
                ArrowBody::Block(statements) => collect_stmt_dynamic_imports(statements, imports),
            }
        }
        Expr {
            kind: ExprKind::Binary { lhs, rhs, .. },
            ..
        }
        | Expr {
            kind:
                ExprKind::Index {
                    object: lhs,
                    index: rhs,
                    ..
                },
            ..
        }
        | Expr {
            kind:
                ExprKind::OptionalIndex {
                    object: lhs,
                    index: rhs,
                    ..
                },
            ..
        }
        | Expr {
            kind:
                ExprKind::Assignment {
                    target: lhs,
                    value: rhs,
                    ..
                },
            ..
        } => {
            collect_expr_dynamic_imports(lhs, imports);
            collect_expr_dynamic_imports(rhs, imports);
        }
        Expr {
            kind: ExprKind::Template { parts, .. },
            ..
        } => {
            for part in *parts {
                if let TemplatePart::Expr(expression) = part {
                    collect_expr_dynamic_imports(expression, imports);
                }
            }
        }
        Expr {
            kind: ExprKind::Match { value, arms, .. },
            ..
        } => {
            collect_expr_dynamic_imports(value, imports);
            for arm in *arms {
                collect_expr_dynamic_imports(&arm.value, imports);
            }
        }
        Expr {
            kind:
                ExprKind::If {
                    condition,
                    then_value,
                    else_value,
                    ..
                },
            ..
        } => {
            collect_expr_dynamic_imports(condition, imports);
            collect_expr_dynamic_imports(then_value, imports);
            collect_expr_dynamic_imports(else_value, imports);
        }
        Expr {
            kind: ExprKind::Int(..),
            ..
        }
        | Expr {
            kind: ExprKind::Float(..),
            ..
        }
        | Expr {
            kind: ExprKind::String(..),
            ..
        }
        | Expr {
            kind: ExprKind::Bool(..),
            ..
        }
        | Expr {
            kind: ExprKind::Null(..),
            ..
        }
        | Expr {
            kind: ExprKind::Ident(..),
            ..
        } => {}
    }
}

pub fn parse_modules<'arena, 'src>(
    arena: &'arena Bump,
    modules: &'src ModuleSet,
) -> Result<Vec<Program<'arena, 'src>>, ModuleError> {
    modules
        .modules
        .iter()
        .map(|module| {
            parse_source(arena, &module.source)
                .map(|program| {
                    crate::for_of_family::expand_for_of_families(
                        arena,
                        program,
                        modules.for_of_specialize_family,
                    )
                })
                .map_err(|error| ModuleError::from_parse(&module.path, &module.source, error))
        })
        .collect()
}

pub fn link_modules<'arena>(
    arena: &'arena Bump,
    modules: &'arena ModuleSet,
    programs: &'arena [Program<'arena, 'arena>],
) -> Result<Program<'arena, 'arena>, ModuleError> {
    let source = crate::ast::SourceNodes::default();
    for (module_id, program) in programs.iter().enumerate() {
        if modules.eager[module_id] {
            continue;
        }
        if let Some(item) = program
            .items
            .iter()
            .find(|item| matches!(item, Item::Stmt(_)))
        {
            return Err(module_error_at(
                modules,
                module_id,
                item.span(),
                "lazy modules must be initialization-free; move top-level executable declarations into an exported function",
            ));
        }
    }

    let mut bindings = Vec::with_capacity(programs.len());
    for (module_id, program) in programs.iter().enumerate() {
        let mut module_bindings = AHashMap::default();
        let mut object_names = AHashSet::default();
        for item in program.items {
            for name in top_level_names(item) {
                let keep_source_name = module_id == modules.root
                    || matches!(item, Item::Extern(_) | Item::ExternGlobal(_))
                    || matches!(item, Item::Class(decl) if decl.object
                        && program.exports.iter().any(|export| export.local.name == decl.name.name));
                let internal = if keep_source_name {
                    name.name
                } else {
                    let generated = format!("$m{module_id}${}", name.name);
                    &*arena.alloc_str(&generated)
                };
                if let Some(previous) = module_bindings.insert(name.name, internal) {
                    let merge_object = matches!(item, Item::Class(decl) if decl.object)
                        && object_names.contains(name.name)
                        && previous == internal;
                    if !merge_object {
                        return Err(module_error_at(
                            modules,
                            module_id,
                            name.span,
                            format!("duplicate module binding `{}`", name.name),
                        ));
                    }
                }
                if matches!(item, Item::Class(decl) if decl.object) {
                    object_names.insert(name.name);
                }
            }
        }
        bindings.push(module_bindings);
    }

    let mut reserved = bindings
        .iter()
        .map(|bindings| bindings.keys().copied().collect::<AHashSet<_>>())
        .collect::<Vec<_>>();
    for (module_id, program) in programs.iter().enumerate() {
        if program.imports.len() != modules.modules[module_id].dependencies.len() {
            return Err(module_error_at(
                modules,
                module_id,
                program.span,
                "internal module dependency mismatch",
            ));
        }
        if program.foreign_imports.len() != modules.modules[module_id].foreign_dependencies.len() {
            return Err(module_error_at(
                modules,
                module_id,
                program.span,
                "internal foreign module dependency mismatch",
            ));
        }
        for import in program.imports {
            for specifier in import.specifiers {
                if !reserved[module_id].insert(specifier.local.name) {
                    return Err(module_error_at(
                        modules,
                        module_id,
                        specifier.local.span,
                        format!("duplicate module binding `{}`", specifier.local.name),
                    ));
                }
            }
        }
        let mut foreign_locals = AHashSet::default();
        for import in program.foreign_imports {
            for specifier in import.specifiers {
                if !foreign_locals.insert(specifier.local.name) {
                    return Err(module_error_at(
                        modules,
                        module_id,
                        specifier.local.span,
                        format!(
                            "duplicate foreign import binding `{}`",
                            specifier.local.name
                        ),
                    ));
                }
                if !program.items.iter().any(|item| {
                    matches!(
                        item,
                        Item::Extern(decl) if decl.name.name == specifier.local.name
                    ) || matches!(
                        item,
                        Item::ExternGlobal(decl) if decl.name.name == specifier.local.name
                    ) || matches!(
                        item,
                        Item::ExternClass(decl) if decl.name.name == specifier.local.name
                    )
                }) {
                    return Err(module_error_at(
                        modules,
                        module_id,
                        specifier.local.span,
                        format!(
                            "foreign import `{}` requires a matching extern declaration",
                            specifier.local.name
                        ),
                    ));
                }
            }
        }
        let mut exported_names = AHashSet::default();
        for export in program.exports {
            if !exported_names.insert(export.exported.name) {
                return Err(module_error_at(
                    modules,
                    module_id,
                    export.exported.span,
                    format!("duplicate export `{}`", export.exported.name),
                ));
            }
        }
    }

    // Resolve module interfaces to a fixed point. A dynamic edge may legally
    // contain a static edge back to an already loaded module, so dependency
    // order alone is insufficient for live bindings in that graph shape.
    let mut exports = vec![AHashMap::default(); programs.len()];
    loop {
        let mut changed = false;
        for (module_id, program) in programs.iter().enumerate() {
            for (import_index, import) in program.imports.iter().enumerate() {
                let dependency = modules.modules[module_id].dependencies[import_index];
                for specifier in import.specifiers {
                    if bindings[module_id].contains_key(specifier.local.name) {
                        continue;
                    }
                    if let Some(&internal) = exports[dependency].get(specifier.imported.name) {
                        bindings[module_id].insert(specifier.local.name, internal);
                        changed = true;
                    }
                }
            }
            for export in program.exports {
                if exports[module_id].contains_key(export.exported.name) {
                    continue;
                }
                if let Some(&internal) = bindings[module_id].get(export.local.name) {
                    exports[module_id].insert(export.exported.name, internal);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    for (module_id, program) in programs.iter().enumerate() {
        for (import_index, import) in program.imports.iter().enumerate() {
            let dependency = modules.modules[module_id].dependencies[import_index];
            for specifier in import.specifiers {
                if bindings[module_id].contains_key(specifier.local.name) {
                    continue;
                }
                let declared = programs[dependency]
                    .exports
                    .iter()
                    .any(|export| export.exported.name == specifier.imported.name);
                let message = if declared {
                    format!(
                        "cyclic module binding `{}` cannot be resolved",
                        specifier.imported.name
                    )
                } else {
                    format!(
                        "module `{}` does not export `{}`",
                        import.source, specifier.imported.name
                    )
                };
                return Err(module_error_at(
                    modules,
                    module_id,
                    specifier.imported.span,
                    message,
                ));
            }
        }
        for export in program.exports {
            if !exports[module_id].contains_key(export.exported.name) {
                return Err(module_error_at(
                    modules,
                    module_id,
                    export.local.span,
                    format!(
                        "cannot export unknown module binding `{}`",
                        export.local.name
                    ),
                ));
            }
        }
    }

    let mut items = BumpVec::new_in(arena);
    let mut module_bindings = BumpVec::new_in(arena);
    let mut constructor_values = BumpVec::new_in(arena);
    let mut seen_constructor_values = AHashSet::default();
    let mut seen_externs = AHashMap::<&str, ExternDecl<'arena, 'arena>>::default();
    for &module_id in &modules.dependency_order {
        let mut cloner = ModuleCloner::new(
            &source,
            arena,
            &bindings[module_id],
            modules.modules[module_id].offset,
        );
        for constructor in programs[module_id].constructor_values {
            let constructor = cloner.global_ident(*constructor);
            if seen_constructor_values.insert(constructor.name) {
                constructor_values.push(constructor);
            }
        }
        for item in programs[module_id].items {
            let cloned = cloner.clone_item(item);
            if let Item::Stmt(Stmt::VarDecl(decl)) = &cloned {
                if !decl.ty.is_auto() {
                    let module = &modules.modules[module_id];
                    module_bindings.push(ModuleBinding {
                        name: decl.name,
                        ty: decl.ty,
                        module_span: Span::new(module.offset, module.offset + module.source.len()),
                    });
                }
            }
            if let Item::Extern(decl) = &cloned {
                match seen_externs.entry(decl.name.name) {
                    Entry::Vacant(entry) => {
                        entry.insert(decl.clone());
                    }
                    Entry::Occupied(entry) => {
                        if !extern_contracts_match(entry.get(), decl) {
                            return Err(module_error_at(
                                modules,
                                module_id,
                                item.span(),
                                format!(
                                    "extern `{}` has conflicting declarations across modules",
                                    decl.name.name
                                ),
                            ));
                        }
                        continue;
                    }
                }
            }
            items.push(cloned);
        }
    }
    let items = items.into_bump_slice();
    let root = modules.root;
    let root_directory = modules.modules[root]
        .path
        .parent()
        .unwrap_or_else(|| Path::new("."));
    let mut linked_foreign_imports = BumpVec::new_in(arena);
    let mut foreign_bindings = AHashMap::<&str, (&str, &str)>::default();
    for (module_id, program) in programs.iter().enumerate() {
        for (import, dependency) in program
            .foreign_imports
            .iter()
            .zip(&modules.modules[module_id].foreign_dependencies)
        {
            let source = if let Some(path) = &dependency.path {
                arena.alloc_str(&relative_module_specifier(root_directory, path))
            } else {
                arena.alloc_str(&dependency.specifier)
            };
            let mut specifiers = BumpVec::new_in(arena);
            for specifier in import.specifiers {
                if let Some((previous_source, previous_imported)) =
                    foreign_bindings.get(specifier.local.name)
                {
                    if *previous_source != source || *previous_imported != specifier.imported.name {
                        return Err(module_error_at(
                            modules,
                            module_id,
                            specifier.local.span,
                            format!(
                                "foreign binding `{}` is imported from conflicting modules",
                                specifier.local.name
                            ),
                        ));
                    }
                    continue;
                }
                foreign_bindings.insert(specifier.local.name, (source, specifier.imported.name));
                let offset = modules.modules[module_id].offset;
                specifiers.push(ImportSpecifier {
                    imported: Ident {
                        name: specifier.imported.name,
                        span: Span::new(
                            specifier.imported.span.start + offset,
                            specifier.imported.span.end + offset,
                        ),
                    },
                    local: Ident {
                        name: specifier.local.name,
                        span: Span::new(
                            specifier.local.span.start + offset,
                            specifier.local.span.end + offset,
                        ),
                    },
                });
            }
            if !specifiers.is_empty() || import.specifiers.is_empty() {
                let offset = modules.modules[module_id].offset;
                linked_foreign_imports.push(ForeignImportDecl {
                    specifiers: specifiers.into_bump_slice(),
                    source,
                    span: Span::new(import.span.start + offset, import.span.end + offset),
                });
            }
        }
    }
    let mut linked_exports = BumpVec::new_in(arena);
    for export in programs[root].exports {
        let internal = bindings[root]
            .get(export.local.name)
            .copied()
            .ok_or_else(|| {
                module_error_at(
                    modules,
                    root,
                    export.local.span,
                    format!(
                        "cannot export unknown module binding `{}`",
                        export.local.name
                    ),
                )
            })?;
        let offset = modules.modules[root].offset;
        linked_exports.push(ExportDecl {
            local: Ident {
                name: internal,
                span: Span::new(
                    export.local.span.start + offset,
                    export.local.span.end + offset,
                ),
            },
            exported: Ident {
                name: export.exported.name,
                span: Span::new(
                    export.exported.span.start + offset,
                    export.exported.span.end + offset,
                ),
            },
            kind: export.kind,
            span: Span::new(export.span.start + offset, export.span.end + offset),
        });
    }
    let mut linked_dynamic_imports = BumpVec::new_in(arena);
    for (module_id, program) in programs.iter().enumerate() {
        let mut imports = Vec::new();
        collect_program_dynamic_imports(program, &mut imports);
        if imports.len() != modules.modules[module_id].dynamic_dependencies.len() {
            return Err(module_error_at(
                modules,
                module_id,
                program.span,
                "internal dynamic module dependency mismatch",
            ));
        }
        for ((source, span), dependency) in imports
            .into_iter()
            .zip(&modules.modules[module_id].dynamic_dependencies)
        {
            let mut exported = exports[*dependency]
                .iter()
                .map(|(name, binding)| (*name, *binding))
                .collect::<Vec<_>>();
            exported.sort_unstable_by_key(|(name, _)| *name);
            let mut dynamic_exports = BumpVec::new_in(arena);
            dynamic_exports.extend(
                exported
                    .into_iter()
                    .map(|(exported, binding)| DynamicExport { exported, binding }),
            );
            linked_dynamic_imports.push(DynamicImportDecl {
                module: u32::try_from(*dependency).map_err(|_| {
                    module_error_at(
                        modules,
                        module_id,
                        span,
                        "module graph exceeds the supported dynamic module id range",
                    )
                })?,
                source,
                span: Span::new(
                    span.start + modules.modules[module_id].offset,
                    span.end + modules.modules[module_id].offset,
                ),
                exports: dynamic_exports.into_bump_slice(),
            });
        }
    }

    let span = items
        .first()
        .zip(items.last())
        .map_or(Span::empty(0), |(first, last)| {
            first.span().merge(last.span())
        });
    Ok(Program::new(
        source.finish(),
        crate::ast::ProgramData {
            imports: &[],
            foreign_imports: linked_foreign_imports.into_bump_slice(),
            dynamic_imports: linked_dynamic_imports.into_bump_slice(),
            module_bindings: module_bindings.into_bump_slice(),
            constructor_values: constructor_values.into_bump_slice(),
            exports: linked_exports.into_bump_slice(),
            items,
            span,
        },
    ))
}

pub(crate) fn relative_module_specifier(from: &Path, to: &Path) -> String {
    let from = from.components().collect::<Vec<_>>();
    let to = to.components().collect::<Vec<_>>();
    let common = from
        .iter()
        .zip(&to)
        .take_while(|(left, right)| left == right)
        .count();
    let mut parts = Vec::new();
    parts.extend(std::iter::repeat_n("..".to_string(), from.len() - common));
    parts.extend(
        to[common..]
            .iter()
            .map(|component| component.as_os_str().to_string_lossy().into_owned()),
    );
    let joined = parts.join("/");
    if joined.starts_with("..") {
        joined
    } else {
        format!("./{joined}")
    }
}

fn extern_contracts_match(left: &ExternDecl<'_, '_>, right: &ExternDecl<'_, '_>) -> bool {
    left.declared_pure == right.declared_pure
        && type_contracts_match(left.return_type, right.return_type)
        && left.params.len() == right.params.len()
        && left.params.iter().zip(right.params).all(|(left, right)| {
            left.parameter.passing == right.parameter.passing
                && type_contracts_match(left.parameter.ty, right.parameter.ty)
        })
}

fn type_contracts_match(left: TypeRef<'_, '_>, right: TypeRef<'_, '_>) -> bool {
    match (left.kind, right.kind) {
        (TypeKind::Int, TypeKind::Int)
        | (TypeKind::Float, TypeKind::Float)
        | (TypeKind::String, TypeKind::String)
        | (TypeKind::Bool, TypeKind::Bool)
        | (TypeKind::Void, TypeKind::Void)
        | (TypeKind::Auto, TypeKind::Auto) => true,
        (
            TypeKind::Named {
                name: left,
                args: left_args,
            },
            TypeKind::Named {
                name: right,
                args: right_args,
            },
        ) => {
            left == right
                && left_args.len() == right_args.len()
                && left_args
                    .iter()
                    .zip(right_args)
                    .all(|(left, right)| type_contracts_match(*left, *right))
        }
        (TypeKind::Array(left), TypeKind::Array(right)) => type_contracts_match(*left, *right),
        (TypeKind::Nullable(left), TypeKind::Nullable(right)) => {
            type_contracts_match(*left, *right)
        }
        (
            TypeKind::Function {
                params: left_params,
                return_type: left_return,
            },
            TypeKind::Function {
                params: right_params,
                return_type: right_return,
            },
        ) => {
            left_params.len() == right_params.len()
                && left_params.iter().zip(right_params).all(|(left, right)| {
                    left.passing == right.passing && type_contracts_match(left.ty, right.ty)
                })
                && type_contracts_match(*left_return, *right_return)
        }
        _ => false,
    }
}

fn top_level_names<'src>(item: &Item<'_, 'src>) -> Vec<Ident<'src>> {
    match item {
        Item::Enum(decl) => vec![decl.name],
        Item::Struct(decl) => vec![decl.name],
        Item::Class(decl) => vec![decl.name],
        Item::ExternClass(decl) => vec![decl.name],
        Item::Function(decl) => vec![decl.name],
        Item::Extern(decl) => vec![decl.name],
        Item::ExternGlobal(decl) => vec![decl.name],
        Item::Stmt(Stmt::VarDecl(decl)) => vec![decl.name],
        Item::Stmt(Stmt::ArrayDestructure { bindings, .. }) => bindings
            .iter()
            .filter_map(|binding| match binding {
                ArrayBinding::Hole(_) => None,
                ArrayBinding::Name(name) | ArrayBinding::Rest(name) => Some(*name),
            })
            .collect(),
        Item::Stmt(Stmt::RecordDestructure { bindings, rest, .. }) => bindings
            .iter()
            .map(|binding| binding.name)
            .chain(rest.iter().copied())
            .collect(),
        Item::Stmt(_) => Vec::new(),
    }
}

fn module_error_at(
    modules: &ModuleSet,
    module_id: ModuleId,
    span: Span,
    message: impl Into<String>,
) -> ModuleError {
    let module = &modules.modules[module_id];
    ModuleError::new(&module.path, &module.source, span, message)
}

pub fn locate_linked_span(modules: &ModuleSet, span: Span) -> (&ModuleSource, Span) {
    let module = modules
        .modules
        .iter()
        .rev()
        .find(|module| span.start >= module.offset)
        .unwrap_or(&modules.modules[modules.root]);
    (
        module,
        Span::new(
            span.start
                .saturating_sub(module.offset)
                .min(module.source.len()),
            span.end
                .saturating_sub(module.offset)
                .min(module.source.len()),
        ),
    )
}

struct ModuleCloner<'arena, 'map> {
    source: &'map crate::ast::SourceNodes,
    arena: &'arena Bump,
    globals: &'map AHashMap<&'arena str, &'arena str>,
    scopes: Vec<AHashSet<&'arena str>>,
    offset: usize,
}

impl<'arena, 'map> ModuleCloner<'arena, 'map> {
    fn new(
        source: &'map crate::ast::SourceNodes,
        arena: &'arena Bump,
        globals: &'map AHashMap<&'arena str, &'arena str>,
        offset: usize,
    ) -> Self {
        Self {
            source,
            arena,
            globals,
            scopes: Vec::new(),
            offset,
        }
    }

    fn clone_item(&mut self, item: &Item<'arena, 'arena>) -> Item<'arena, 'arena> {
        match item {
            Item::Enum(decl) => Item::Enum(EnumDecl {
                name: self.global_ident(decl.name),
                variants: self.clone_idents(decl.variants),
                span: self.span(decl.span),
            }),
            Item::Struct(decl) => Item::Struct(self.clone_struct(decl)),
            Item::Class(decl) => Item::Class(self.clone_class(decl)),
            Item::ExternClass(decl) => Item::ExternClass(self.clone_extern_class(decl)),
            Item::Function(decl) => Item::Function(self.clone_function(decl, true)),
            Item::Extern(decl) => Item::Extern(ExternDecl {
                declared_pure: decl.declared_pure,
                return_type: self.clone_type(decl.return_type),
                name: self.global_ident(decl.name),
                type_params: self.clone_idents(decl.type_params),
                params: self.clone_params(decl.params),
                span: self.span(decl.span),
            }),
            Item::ExternGlobal(decl) => Item::ExternGlobal(ExternGlobalDecl {
                ty: self.clone_type(decl.ty),
                name: self.global_ident(decl.name),
                span: self.span(decl.span),
            }),
            Item::Stmt(stmt) => Item::Stmt(self.clone_stmt(stmt, true)),
        }
    }

    fn clone_struct(&mut self, decl: &StructDecl<'arena, 'arena>) -> StructDecl<'arena, 'arena> {
        StructDecl {
            name: self.global_ident(decl.name),
            type_params: self.clone_idents(decl.type_params),
            fields: self.clone_fields(decl.fields),
            span: self.span(decl.span),
        }
    }

    fn clone_class(&mut self, decl: &ClassDecl<'arena, 'arena>) -> ClassDecl<'arena, 'arena> {
        let mut members = BumpVec::new_in(self.arena);
        for member in decl.members {
            members.push(match member {
                ClassMember::Field(field) => ClassMember::Field(self.clone_field(field)),
                ClassMember::Constructor(constructor) => {
                    self.push_scope();
                    let params = self.clone_params_and_declare(constructor.params);
                    let body = self.clone_statements(constructor.body);
                    self.pop_scope();
                    ClassMember::Constructor(ConstructorDecl {
                        params,
                        body,
                        span: self.span(constructor.span),
                    })
                }
                ClassMember::Method(method) => {
                    ClassMember::Method(self.clone_function(method, false))
                }
            });
        }
        ClassDecl {
            name: self.global_ident(decl.name),
            type_params: self.clone_idents(decl.type_params),
            base: decl.base.map(|base| self.clone_type(base)),
            members: members.into_bump_slice(),
            object: decl.object,
            span: self.span(decl.span),
        }
    }

    fn clone_extern_class(
        &mut self,
        decl: &ExternClassDecl<'arena, 'arena>,
    ) -> ExternClassDecl<'arena, 'arena> {
        let mut members = BumpVec::new_in(self.arena);
        for member in decl.members {
            members.push(match member {
                ExternClassMember::Field(field) => {
                    ExternClassMember::Field(self.clone_field(field))
                }
                ExternClassMember::Method(method) => ExternClassMember::Method(ExternDecl {
                    declared_pure: method.declared_pure,
                    return_type: self.clone_type(method.return_type),
                    name: self.plain_ident(method.name),
                    type_params: self.clone_idents(method.type_params),
                    params: self.clone_params(method.params),
                    span: self.span(method.span),
                }),
                ExternClassMember::Constructor(constructor) => {
                    ExternClassMember::Constructor(crate::ast::ExternConstructorDecl {
                        params: self.clone_params(constructor.params),
                        span: self.span(constructor.span),
                    })
                }
            });
        }
        ExternClassDecl {
            name: self.global_ident(decl.name),
            type_params: self.clone_idents(decl.type_params),
            base: decl.base.map(|base| self.clone_type(base)),
            members: members.into_bump_slice(),
            span: self.span(decl.span),
        }
    }

    fn clone_function(
        &mut self,
        decl: &FunctionDecl<'arena, 'arena>,
        global_name: bool,
    ) -> FunctionDecl<'arena, 'arena> {
        self.push_scope();
        let params = self.clone_params_and_declare(decl.params);
        let body = self.clone_statements(decl.body);
        self.pop_scope();
        FunctionDecl {
            region: decl.region,
            declared_pure: decl.declared_pure,
            is_async: decl.is_async,
            is_generator: decl.is_generator,
            return_type: self.clone_type(decl.return_type),
            name: if global_name {
                self.global_ident(decl.name)
            } else {
                self.plain_ident(decl.name)
            },
            type_params: self.clone_idents(decl.type_params),
            params,
            body,
            span: self.span(decl.span),
        }
    }

    fn clone_fields(
        &mut self,
        fields: &[FieldDecl<'arena, 'arena>],
    ) -> &'arena [FieldDecl<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(fields.iter().map(|field| self.clone_field(field)));
        cloned.into_bump_slice()
    }

    fn clone_field(&self, field: &FieldDecl<'arena, 'arena>) -> FieldDecl<'arena, 'arena> {
        FieldDecl {
            ty: self.clone_type(field.ty),
            name: self.plain_ident(field.name),
            span: self.span(field.span),
        }
    }

    fn clone_params(
        &mut self,
        params: &[Param<'arena, 'arena>],
    ) -> &'arena [Param<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(params.iter().map(|param| Param {
            parameter: self.clone_parameter_type(param.parameter),
            name: self.plain_ident(param.name),
            default: param.default.as_ref().map(|value| self.clone_expr(value)),
            span: self.span(param.span),
        }));
        cloned.into_bump_slice()
    }

    fn clone_params_and_declare(
        &mut self,
        params: &[Param<'arena, 'arena>],
    ) -> &'arena [Param<'arena, 'arena>] {
        let cloned = self.clone_params(params);
        for param in cloned {
            self.declare_local(param.name.name);
        }
        cloned
    }

    /// Arrow defaults share the arrow's complete parameter scope. Predeclare
    /// every name so self/forward references remain parameter-bound for the
    /// semantic earlier-only check instead of being renamed to a same-name
    /// module global. Named callable defaults retain outer-scope binding rules.
    fn clone_arrow_params_and_declare(
        &mut self,
        params: &[Param<'arena, 'arena>],
    ) -> &'arena [Param<'arena, 'arena>] {
        for param in params {
            self.declare_local(param.name.name);
        }
        self.clone_params(params)
    }

    fn clone_statements(
        &mut self,
        statements: &[Stmt<'arena, 'arena>],
    ) -> &'arena [Stmt<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        for statement in statements {
            cloned.push(self.clone_stmt(statement, false));
        }
        cloned.into_bump_slice()
    }

    fn clone_stmt(&mut self, stmt: &Stmt<'arena, 'arena>, top_level: bool) -> Stmt<'arena, 'arena> {
        match stmt {
            Stmt::VarDecl(decl) => Stmt::VarDecl(self.clone_var_decl(decl, top_level)),
            Stmt::ArrayDestructure {
                bindings,
                value,
                span,
            } => {
                let value = self.clone_expr(value);
                let mut cloned = BumpVec::new_in(self.arena);
                for binding in *bindings {
                    cloned.push(match binding {
                        ArrayBinding::Hole(span) => ArrayBinding::Hole(self.span(*span)),
                        ArrayBinding::Name(name) => {
                            ArrayBinding::Name(self.clone_binding_ident(*name, top_level))
                        }
                        ArrayBinding::Rest(name) => {
                            ArrayBinding::Rest(self.clone_binding_ident(*name, top_level))
                        }
                    });
                }
                Stmt::ArrayDestructure {
                    bindings: cloned.into_bump_slice(),
                    value,
                    span: self.span(*span),
                }
            }
            Stmt::RecordDestructure {
                bindings,
                rest,
                value,
                span,
            } => {
                let value = self.clone_expr(value);
                let mut cloned = BumpVec::new_in(self.arena);
                for binding in *bindings {
                    cloned.push(RecordBinding {
                        key: self.plain_ident(binding.key),
                        name: self.clone_binding_ident(binding.name, top_level),
                        span: self.span(binding.span),
                    });
                }
                Stmt::RecordDestructure {
                    bindings: cloned.into_bump_slice(),
                    rest: rest.map(|name| self.clone_binding_ident(name, top_level)),
                    value,
                    span: self.span(*span),
                }
            }
            Stmt::Expr(expr) => Stmt::Expr(self.clone_expr(expr)),
            Stmt::Return { value, span } => Stmt::Return {
                value: value.as_ref().map(|value| self.clone_expr(value)),
                span: self.span(*span),
            },
            Stmt::Throw { value, span } => Stmt::Throw {
                value: self.clone_expr(value),
                span: self.span(*span),
            },
            Stmt::SuperCall { args, span } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for argument in *args {
                    cloned.push(self.clone_argument(argument));
                }
                Stmt::SuperCall {
                    args: cloned.into_bump_slice(),
                    span: self.span(*span),
                }
            }
            Stmt::Yield {
                value,
                delegate,
                span,
            } => Stmt::Yield {
                value: self.clone_expr(value),
                delegate: *delegate,
                span: self.span(*span),
            },
            Stmt::Try {
                body,
                catch,
                finally,
                span,
            } => {
                self.push_scope();
                let body = self.clone_statements(body);
                self.pop_scope();
                let catch = catch.as_ref().map(|clause| {
                    self.push_scope();
                    let binding = clause.binding.map(|binding| {
                        let binding = CatchBinding {
                            ty: self.clone_type(binding.ty),
                            name: self.plain_ident(binding.name),
                            span: self.span(binding.span),
                        };
                        self.declare_local(binding.name.name);
                        binding
                    });
                    let body = self.clone_statements(clause.body);
                    self.pop_scope();
                    CatchClause {
                        binding,
                        body,
                        span: self.span(clause.span),
                    }
                });
                let finally = finally.map(|body| {
                    self.push_scope();
                    let body = self.clone_statements(body);
                    self.pop_scope();
                    body
                });
                Stmt::Try {
                    body,
                    catch,
                    finally,
                    span: self.span(*span),
                }
            }
            Stmt::Block { body, span } => {
                self.push_scope();
                let body = self.clone_statements(body);
                self.pop_scope();
                Stmt::Block {
                    body,
                    span: self.span(*span),
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => Stmt::If {
                condition: self.clone_expr(condition),
                then_branch: self.clone_scoped_stmt(then_branch),
                else_branch: else_branch.map(|branch| self.clone_scoped_stmt(branch)),
                span: self.span(*span),
            },
            Stmt::While {
                condition,
                body,
                span,
            } => Stmt::While {
                condition: self.clone_expr(condition),
                body: self.clone_scoped_stmt(body),
                span: self.span(*span),
            },
            Stmt::For {
                initializer,
                condition,
                update,
                body,
                span,
            } => {
                self.push_scope();
                let initializer = initializer.as_ref().map(|initializer| match initializer {
                    ForInitializer::VarDecl(decl) => {
                        ForInitializer::VarDecl(self.clone_var_decl(decl, false))
                    }
                    ForInitializer::Expr(expr) => ForInitializer::Expr(self.clone_expr(expr)),
                });
                let condition = condition.as_ref().map(|expr| self.clone_expr(expr));
                let update = update.as_ref().map(|expr| self.clone_expr(expr));
                let body = self.arena.alloc(self.clone_stmt(body, false));
                self.pop_scope();
                Stmt::For {
                    initializer,
                    condition,
                    update,
                    body,
                    span: self.span(*span),
                }
            }
            Stmt::ForIn {
                key_type,
                key,
                object,
                body,
                span,
            } => {
                self.push_scope();
                let key_type = self.clone_type(*key_type);
                let key = self.plain_ident(*key);
                self.declare_local(key.name);
                let object = self.clone_expr(object);
                let body = self.arena.alloc(self.clone_stmt(body, false));
                self.pop_scope();
                Stmt::ForIn {
                    key_type,
                    key,
                    object,
                    body,
                    span: self.span(*span),
                }
            }
            Stmt::ForOf {
                element_type,
                element,
                iterable,
                body,
                inline,
                span,
            } => {
                self.push_scope();
                let element_type = self.clone_type(*element_type);
                let iterable = self.clone_expr(iterable);
                let element = self.plain_ident(*element);
                self.declare_local(element.name);
                let body = self.arena.alloc(self.clone_stmt(body, false));
                self.pop_scope();
                Stmt::ForOf {
                    element_type,
                    element,
                    iterable,
                    body,
                    inline: *inline,
                    span: self.span(*span),
                }
            }
            Stmt::Break(span) => Stmt::Break(self.span(*span)),
            Stmt::Continue(span) => Stmt::Continue(self.span(*span)),
        }
    }

    fn clone_scoped_stmt(
        &mut self,
        statement: &Stmt<'arena, 'arena>,
    ) -> &'arena Stmt<'arena, 'arena> {
        self.push_scope();
        let cloned = self.clone_stmt(statement, false);
        self.pop_scope();
        self.arena.alloc(cloned)
    }

    fn clone_var_decl(
        &mut self,
        decl: &VarDecl<'arena, 'arena>,
        top_level: bool,
    ) -> VarDecl<'arena, 'arena> {
        let initializer = decl
            .initializer
            .as_ref()
            .map(|initializer| self.clone_expr(initializer));
        let name = if top_level {
            self.global_ident(decl.name)
        } else {
            let name = self.plain_ident(decl.name);
            self.declare_local(name.name);
            name
        };
        VarDecl {
            ty: self.clone_type(decl.ty),
            name,
            initializer,
            span: self.span(decl.span),
        }
    }

    fn clone_binding_ident(&mut self, ident: Ident<'arena>, top_level: bool) -> Ident<'arena> {
        if top_level {
            self.global_ident(ident)
        } else {
            let ident = self.plain_ident(ident);
            self.declare_local(ident.name);
            ident
        }
    }

    fn clone_expr(&mut self, expr: &Expr<'arena, 'arena>) -> Expr<'arena, 'arena> {
        match expr {
            Expr {
                kind: ExprKind::Int(value, span),
                ..
            } => self
                .source
                .expression(ExprKind::Int(*value, self.span(*span))),
            Expr {
                kind: ExprKind::Float(value, span),
                ..
            } => self
                .source
                .expression(ExprKind::Float(*value, self.span(*span))),
            Expr {
                kind: ExprKind::String(value, span),
                ..
            } => self
                .source
                .expression(ExprKind::String(value, self.span(*span))),
            Expr {
                kind: ExprKind::Bool(value, span),
                ..
            } => self
                .source
                .expression(ExprKind::Bool(*value, self.span(*span))),
            Expr {
                kind: ExprKind::Null(span),
                ..
            } => self.source.expression(ExprKind::Null(self.span(*span))),
            Expr {
                kind: ExprKind::Ident(ident),
                ..
            } => self
                .source
                .expression(ExprKind::Ident(self.reference_ident(*ident))),
            Expr {
                kind: ExprKind::ArrayLiteral { elements, span },
                ..
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for element in *elements {
                    cloned.push(match element {
                        ArrayElement::Value(value) => ArrayElement::Value(self.clone_expr(value)),
                        ArrayElement::Spread { value, span } => ArrayElement::Spread {
                            value: self.clone_expr(value),
                            span: self.span(*span),
                        },
                    });
                }
                self.source.expression(ExprKind::ArrayLiteral {
                    elements: cloned.into_bump_slice(),
                    span: self.span(*span),
                })
            }
            Expr {
                kind: ExprKind::RecordLiteral { entries, span },
                ..
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for entry in *entries {
                    cloned.push(match entry {
                        RecordElement::Entry(entry) => RecordElement::Entry(RecordEntry {
                            key: self.plain_ident(entry.key),
                            value: self.clone_expr(&entry.value),
                            span: self.span(entry.span),
                        }),
                        RecordElement::Spread { value, span } => RecordElement::Spread {
                            value: self.clone_expr(value),
                            span: self.span(*span),
                        },
                    });
                }
                self.source.expression(ExprKind::RecordLiteral {
                    entries: cloned.into_bump_slice(),
                    span: self.span(*span),
                })
            }
            Expr {
                kind: ExprKind::ObjectLiteral { entries, span },
                ..
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for entry in *entries {
                    cloned.push(match entry {
                        RecordElement::Entry(entry) => RecordElement::Entry(RecordEntry {
                            key: self.plain_ident(entry.key),
                            value: self.clone_expr(&entry.value),
                            span: self.span(entry.span),
                        }),
                        RecordElement::Spread { value, span } => RecordElement::Spread {
                            value: self.clone_expr(value),
                            span: self.span(*span),
                        },
                    });
                }
                self.source.expression(ExprKind::ObjectLiteral {
                    entries: cloned.into_bump_slice(),
                    span: self.span(*span),
                })
            }
            Expr {
                kind: ExprKind::StructLiteral { name, values, span },
                ..
            } => self.source.expression(ExprKind::StructLiteral {
                name: self.global_ident(*name),
                values: self.clone_exprs(values),
                span: self.span(*span),
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
            } => self.source.expression(ExprKind::New {
                class: self.global_ident(*class),
                type_args: self.clone_types(type_args),
                args: self.clone_arguments(args),
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::DynamicImport { source, span },
                ..
            } => self.source.expression(ExprKind::DynamicImport {
                source,
                span: self.span(*span),
            }),
            Expr {
                kind:
                    ExprKind::Member {
                        object,
                        property,
                        span,
                    },
                ..
            } => self.source.expression(ExprKind::Member {
                object: self.arena.alloc(self.clone_expr(object)),
                property: self.plain_ident(*property),
                span: self.span(*span),
            }),
            Expr {
                kind:
                    ExprKind::OptionalMember {
                        object,
                        property,
                        span,
                    },
                ..
            } => self.source.expression(ExprKind::OptionalMember {
                object: self.arena.alloc(self.clone_expr(object)),
                property: self.plain_ident(*property),
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::Call { callee, args, span },
                ..
            } => self.source.expression(ExprKind::Call {
                callee: self.arena.alloc(self.clone_expr(callee)),
                args: self.clone_arguments(args),
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::ArrowFunction { params, body, span },
                ..
            } => {
                self.push_scope();
                let params = self.clone_arrow_params_and_declare(params);
                let body = match body {
                    ArrowBody::Expr(expr) => {
                        ArrowBody::Expr(self.arena.alloc(self.clone_expr(expr)))
                    }
                    ArrowBody::Block(body) => ArrowBody::Block(self.clone_statements(body)),
                };
                self.pop_scope();
                self.source.expression(ExprKind::ArrowFunction {
                    params,
                    body,
                    span: self.span(*span),
                })
            }
            Expr {
                kind: ExprKind::Unary { op, expr, span },
                ..
            } => self.source.expression(ExprKind::Unary {
                op: *op,
                expr: self.arena.alloc(self.clone_expr(expr)),
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::Await { task, span },
                ..
            } => self.source.expression(ExprKind::Await {
                task: self.arena.alloc(self.clone_expr(task)),
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::Binary { op, lhs, rhs, span },
                ..
            } => self.source.expression(ExprKind::Binary {
                op: *op,
                lhs: self.arena.alloc(self.clone_expr(lhs)),
                rhs: self.arena.alloc(self.clone_expr(rhs)),
                span: self.span(*span),
            }),
            Expr {
                kind:
                    ExprKind::TypeCheck {
                        value,
                        target,
                        span,
                    },
                ..
            } => self.source.expression(ExprKind::TypeCheck {
                value: self.arena.alloc(self.clone_expr(value)),
                target: self.clone_type(*target),
                span: self.span(*span),
            }),
            Expr {
                kind:
                    ExprKind::Index {
                        object,
                        index,
                        span,
                    },
                ..
            } => self.source.expression(ExprKind::Index {
                object: self.arena.alloc(self.clone_expr(object)),
                index: self.arena.alloc(self.clone_expr(index)),
                span: self.span(*span),
            }),
            Expr {
                kind:
                    ExprKind::OptionalIndex {
                        object,
                        index,
                        span,
                    },
                ..
            } => self.source.expression(ExprKind::OptionalIndex {
                object: self.arena.alloc(self.clone_expr(object)),
                index: self.arena.alloc(self.clone_expr(index)),
                span: self.span(*span),
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
            } => self.source.expression(ExprKind::If {
                condition: self.arena.alloc(self.clone_expr(condition)),
                then_value: self.arena.alloc(self.clone_expr(then_value)),
                else_value: self.arena.alloc(self.clone_expr(else_value)),
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::Match { value, arms, span },
                ..
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for arm in *arms {
                    let pattern = match arm.pattern {
                        MatchPattern::EnumVariant {
                            enum_name,
                            variant,
                            span,
                        } => MatchPattern::EnumVariant {
                            enum_name: self.global_ident(enum_name),
                            variant: self.plain_ident(variant),
                            span: self.span(span),
                        },
                        MatchPattern::Int(value, span) => MatchPattern::Int(value, self.span(span)),
                        MatchPattern::String(value, span) => {
                            MatchPattern::String(value, self.span(span))
                        }
                        MatchPattern::Bool(value, span) => {
                            MatchPattern::Bool(value, self.span(span))
                        }
                        MatchPattern::Wildcard(span) => MatchPattern::Wildcard(self.span(span)),
                    };
                    cloned.push(MatchArm {
                        pattern,
                        value: self.clone_expr(&arm.value),
                        span: self.span(arm.span),
                    });
                }
                self.source.expression(ExprKind::Match {
                    value: self.arena.alloc(self.clone_expr(value)),
                    arms: cloned.into_bump_slice(),
                    span: self.span(*span),
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
            } => self.source.expression(ExprKind::Assignment {
                op: *op,
                target: self.arena.alloc(self.clone_expr(target)),
                value: self.arena.alloc(self.clone_expr(value)),
                span: self.span(*span),
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
            } => self.source.expression(ExprKind::Update {
                op: *op,
                target: self.arena.alloc(self.clone_expr(target)),
                prefix: *prefix,
                span: self.span(*span),
            }),
            Expr {
                kind: ExprKind::Template { parts, span },
                ..
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                for part in *parts {
                    cloned.push(match part {
                        TemplatePart::String(value, span) => {
                            TemplatePart::String(value, self.span(*span))
                        }
                        TemplatePart::Expr(expr) => TemplatePart::Expr(self.clone_expr(expr)),
                    });
                }
                self.source.expression(ExprKind::Template {
                    parts: cloned.into_bump_slice(),
                    span: self.span(*span),
                })
            }
        }
    }

    fn clone_parameter_type(
        &self,
        parameter: ParameterType<'arena, 'arena>,
    ) -> ParameterType<'arena, 'arena> {
        ParameterType {
            ty: self.clone_type(parameter.ty),
            passing: parameter.passing,
            span: self.span(parameter.span),
        }
    }

    fn clone_argument(&mut self, argument: &Argument<'arena, 'arena>) -> Argument<'arena, 'arena> {
        Argument {
            expression: self.clone_expr(&argument.expression),
            passing: argument.passing,
            span: self.span(argument.span),
        }
    }

    fn clone_arguments(
        &mut self,
        arguments: &[Argument<'arena, 'arena>],
    ) -> &'arena [Argument<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(
            arguments
                .iter()
                .map(|argument| self.clone_argument(argument)),
        );
        cloned.into_bump_slice()
    }

    fn clone_exprs(
        &mut self,
        expressions: &[Expr<'arena, 'arena>],
    ) -> &'arena [Expr<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        for expression in expressions {
            cloned.push(self.clone_expr(expression));
        }
        cloned.into_bump_slice()
    }

    fn clone_type(&self, ty: TypeRef<'arena, 'arena>) -> TypeRef<'arena, 'arena> {
        let kind = match ty.kind {
            TypeKind::Named { name, args } => TypeKind::Named {
                name: self.globals.get(name).copied().unwrap_or(name),
                args: self.clone_types(args),
            },
            TypeKind::Array(element) => {
                TypeKind::Array(self.arena.alloc(self.clone_type(*element)))
            }
            TypeKind::Nullable(inner) => {
                TypeKind::Nullable(self.arena.alloc(self.clone_type(*inner)))
            }
            TypeKind::Union(members) => TypeKind::Union(self.clone_types(members)),
            TypeKind::Function {
                params,
                return_type,
            } => {
                let mut cloned = BumpVec::new_in(self.arena);
                cloned.extend(params.iter().map(|param| self.clone_parameter_type(*param)));
                TypeKind::Function {
                    params: cloned.into_bump_slice(),
                    return_type: self.arena.alloc(self.clone_type(*return_type)),
                }
            }
            primitive => primitive,
        };
        TypeRef {
            kind,
            span: self.span(ty.span),
        }
    }

    fn clone_types(&self, types: &[TypeRef<'arena, 'arena>]) -> &'arena [TypeRef<'arena, 'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(types.iter().map(|ty| self.clone_type(*ty)));
        cloned.into_bump_slice()
    }

    fn clone_idents(&self, idents: &[Ident<'arena>]) -> &'arena [Ident<'arena>] {
        let mut cloned = BumpVec::new_in(self.arena);
        cloned.extend(idents.iter().map(|ident| self.plain_ident(*ident)));
        cloned.into_bump_slice()
    }

    fn global_ident(&self, ident: Ident<'arena>) -> Ident<'arena> {
        Ident {
            name: self.globals.get(ident.name).copied().unwrap_or(ident.name),
            span: self.span(ident.span),
        }
    }

    fn reference_ident(&self, ident: Ident<'arena>) -> Ident<'arena> {
        if self
            .scopes
            .iter()
            .rev()
            .any(|scope| scope.contains(ident.name))
        {
            self.plain_ident(ident)
        } else {
            self.global_ident(ident)
        }
    }

    fn plain_ident(&self, ident: Ident<'arena>) -> Ident<'arena> {
        Ident {
            name: ident.name,
            span: self.span(ident.span),
        }
    }

    fn span(&self, span: Span) -> Span {
        Span::new(span.start + self.offset, span.end + self.offset)
    }

    fn push_scope(&mut self) {
        self.scopes.push(AHashSet::default());
    }

    fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    fn declare_local(&mut self, name: &'arena str) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.insert(name);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_cloning_keeps_reference_modes_and_fresh_argument_occurrences() {
        use crate::primitive::ParameterPassing::MutableReference;
        let arena = Bump::new();
        let syntax = parse_source(
            &arena,
            "void forward(ref int value,func(ref int)->void action){action(ref value);}",
        )
        .unwrap();
        let Item::Function(original) = &syntax.items[0] else {
            panic!("function")
        };
        let source = crate::ast::SourceNodes::continuing(syntax.source_identity());
        let globals = AHashMap::default();
        let mut cloner = ModuleCloner::new(&source, &arena, &globals, 31);
        let cloned = cloner.clone_function(original, false);
        assert_eq!(cloned.params[0].parameter.passing, MutableReference);
        let TypeKind::Function { params, .. } = cloned.params[1].parameter.ty.kind else {
            panic!("callable type")
        };
        assert_eq!(params[0].passing, MutableReference);
        let Stmt::Expr(Expr {
            kind:
                ExprKind::Call {
                    args: original_args,
                    ..
                },
            ..
        }) = &original.body[0]
        else {
            panic!("original call")
        };
        let Stmt::Expr(Expr {
            kind: ExprKind::Call { args, .. },
            ..
        }) = &cloned.body[0]
        else {
            panic!("cloned call")
        };
        assert_eq!(args[0].passing, MutableReference);
        assert_ne!(args[0].expression.id, original_args[0].expression.id);
        assert_eq!(args[0].span.start, original_args[0].span.start + 31);
        assert_eq!(
            args[0].expression.span().start,
            original_args[0].expression.span().start + 31
        );
    }

    #[test]
    fn static_order_preserves_edges_cycles_and_single_initialization() {
        let edges = [vec![1, 2, 1], vec![0, 3], vec![3], vec![]];
        let visited = std::cell::RefCell::new(Vec::new());
        let order = static_evaluation_order(0, edges.len(), |module| {
            visited.borrow_mut().push(module);
            edges[module].iter().copied()
        })
        .unwrap();
        assert_eq!(order, [3, 1, 2, 0]);
        assert_eq!(*visited.borrow(), [0, 1, 3, 2]);
        assert_eq!(
            static_evaluation_order(2, edges.len(), |module| edges[module].iter().copied())
                .unwrap(),
            [3, 2]
        );
        assert!(
            static_evaluation_order(4, edges.len(), |module| edges[module].iter().copied())
                .is_err()
        );
        assert!(static_evaluation_order(0, 1, |_| [1]).is_err());
    }

    #[test]
    fn static_order_uses_an_iterative_stack_for_deep_import_chains() {
        let count = 20_000;
        let order = static_evaluation_order(0, count, |module| {
            (module + 1 < count).then_some(module + 1)
        })
        .unwrap();
        assert_eq!(order.len(), count);
        assert!(order.into_iter().eq((0..count).rev()));
    }

    #[test]
    fn resolves_extensionless_relative_imports() {
        assert_eq!(
            resolve_import_path(Path::new("/tmp/project"), "./math").unwrap(),
            PathBuf::from("/tmp/project/math")
        );
    }

    #[test]
    fn linked_arrow_default_keeps_earlier_parameter_binding() {
        let root_source = "import {run} from \"./dep.lil\";print(run());";
        let dependency_source =
            "int seed=9;export int run(){return ((int seed=1,int value=seed)=>value)(7);}";
        let modules = ModuleSet {
            modules: vec![
                ModuleSource {
                    path: PathBuf::from("/virtual/root.lil"),
                    source: root_source.to_string(),
                    dependencies: vec![1],
                    foreign_dependencies: Vec::new(),
                    dynamic_dependencies: Vec::new(),
                    offset: 0,
                },
                ModuleSource {
                    path: PathBuf::from("/virtual/dep.lil"),
                    source: dependency_source.to_string(),
                    dependencies: Vec::new(),
                    foreign_dependencies: Vec::new(),
                    dynamic_dependencies: Vec::new(),
                    offset: root_source.len() + 1,
                },
            ],
            dependency_order: vec![1, 0],
            root: 0,
            eager: vec![true, true],
            for_of_specialize_family: 0,
        };
        let arena = Bump::new();
        let modules = arena.alloc(modules);
        let programs = parse_modules(&arena, modules).unwrap();
        let programs = arena.alloc_slice_fill_iter(programs);
        let linked = link_modules(&arena, modules, programs).unwrap();

        let run = linked
            .items
            .iter()
            .find_map(|item| match item {
                Item::Function(function) if function.name.name.ends_with("$run") => Some(function),
                _ => None,
            })
            .expect("linked dependency function");
        let Some(Stmt::Return {
            value:
                Some(Expr {
                    kind:
                        ExprKind::Call {
                            callee:
                                Expr {
                                    kind: ExprKind::ArrowFunction { params, .. },
                                    ..
                                },
                            ..
                        },
                    ..
                }),
            ..
        }) = run.body.first()
        else {
            panic!("expected returned immediate arrow call")
        };
        let Some(Expr {
            kind: ExprKind::Ident(default),
            ..
        }) = &params[1].default
        else {
            panic!("expected identifier default")
        };
        assert_eq!(default.name, "seed");
    }

    #[test]
    fn exported_objects_keep_source_names_and_merge_across_modules() {
        let root_source = "import {Api} from \"./core.lil\";import \"./ajax.lil\";print(Api.add(1,2)+Api.mul(3,4));";
        let core_source = "export object Api{int add(int left,int right){return left+right;}}";
        let ajax_source = "export object Api{int mul(int left,int right){return left*right;}}";
        let modules = ModuleSet {
            modules: vec![
                ModuleSource {
                    path: PathBuf::from("/virtual/root.lil"),
                    source: root_source.to_string(),
                    dependencies: vec![1, 2],
                    foreign_dependencies: Vec::new(),
                    dynamic_dependencies: Vec::new(),
                    offset: 0,
                },
                ModuleSource {
                    path: PathBuf::from("/virtual/core.lil"),
                    source: core_source.to_string(),
                    dependencies: Vec::new(),
                    foreign_dependencies: Vec::new(),
                    dynamic_dependencies: Vec::new(),
                    offset: root_source.len() + 1,
                },
                ModuleSource {
                    path: PathBuf::from("/virtual/ajax.lil"),
                    source: ajax_source.to_string(),
                    dependencies: Vec::new(),
                    foreign_dependencies: Vec::new(),
                    dynamic_dependencies: Vec::new(),
                    offset: root_source.len() + 1 + core_source.len() + 1,
                },
            ],
            dependency_order: vec![1, 2, 0],
            root: 0,
            eager: vec![true, true, true],
            for_of_specialize_family: 0,
        };
        let arena = Bump::new();
        let modules = arena.alloc(modules);
        let programs = parse_modules(&arena, modules).unwrap();
        let programs = arena.alloc_slice_fill_iter(programs);
        let linked = link_modules(&arena, modules, programs).unwrap();
        let objects = linked
            .items
            .iter()
            .filter_map(|item| match item {
                Item::Class(decl) if decl.object => Some(decl.name.name),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(objects, vec!["Api", "Api"]);
    }

    #[test]
    fn linked_arrow_defaults_reject_self_and_forward_parameter_references() {
        let root_source = "import {run} from \"./dep.lil\";print(run());";
        for dependency_source in [
            "int value=9;export int run(){return ((int value=value)=>value)();}",
            "int later=9;export int run(){return ((int value=later,int later=1)=>value)();}",
        ] {
            let modules = ModuleSet {
                modules: vec![
                    ModuleSource {
                        path: PathBuf::from("/virtual/root.lil"),
                        source: root_source.to_string(),
                        dependencies: vec![1],
                        foreign_dependencies: Vec::new(),
                        dynamic_dependencies: Vec::new(),
                        offset: 0,
                    },
                    ModuleSource {
                        path: PathBuf::from("/virtual/dep.lil"),
                        source: dependency_source.to_string(),
                        dependencies: Vec::new(),
                        foreign_dependencies: Vec::new(),
                        dynamic_dependencies: Vec::new(),
                        offset: root_source.len() + 1,
                    },
                ],
                dependency_order: vec![1, 0],
                root: 0,
                eager: vec![true, true],
                for_of_specialize_family: 0,
            };
            let arena = Bump::new();
            let modules = arena.alloc(modules);
            let programs = parse_modules(&arena, modules).unwrap();
            let programs = arena.alloc_slice_fill_iter(programs);
            let linked = link_modules(&arena, modules, programs).unwrap();
            let error = crate::semantic::analyze(&linked).unwrap_err();
            assert!(
                error
                    .message
                    .contains("parameter defaults can only reference earlier parameters"),
                "{error}"
            );
        }
    }
}
