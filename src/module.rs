use crate::ast::ExprKind;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::stable_hash::StableHashMap as AHashMap;
use bumpalo::Bump;

use crate::ast::{
    ArrowBody, ClassMember, Expr, ExternClassMember, ForInitializer, FunctionDecl, Item, Param,
    Program, SourceNodeId, Stmt, TemplatePart,
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
    static_evaluation_order_from_admitted(&[root], module_count, dependencies, budget)
}

/// The canonical schedule of several roots (plan M3.3): one post-order walk
/// that visits the roots in their given order, each module once. With one
/// root it is that root's order; with several it is what ES modules evaluate
/// when every root is loaded in turn.
pub(crate) fn static_evaluation_order_from_admitted<I: IntoIterator<Item = usize>>(
    roots: &[usize],
    module_count: usize,
    dependencies: impl Fn(usize) -> I,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, StaticOrderError> {
    use crate::output_budget::AllocationClass::Scratch;
    budget.work(WorkKind::Analysis, 1)?;
    if roots.is_empty() || roots.iter().any(|&root| root >= module_count) {
        return Err(StaticOrderError::Invalid(
            "static module root is out of bounds",
        ));
    }
    let mut order = budget.vector(Scratch, module_count)?;
    let mut scope = budget.scope();
    let mut states = scope.filled(Scratch, module_count, 0u8)?;
    let mut stack = scope.vector(Scratch, module_count)?;
    for &root in roots {
        if states[root] != 0 {
            continue;
        }
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
    }
    Ok(order)
}

/// Static evaluation order from the roots, in their order, then every module
/// only `import()` reaches, in module order. Such a module is
/// initialization-free, so its place is unobservable; last, it runs after
/// everything it may read.
pub(crate) fn initialization_order_admitted<I: IntoIterator<Item = usize>>(
    roots: &[usize],
    module_count: usize,
    dependencies: impl Fn(usize) -> I,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, StaticOrderError> {
    use crate::output_budget::AllocationClass::Scratch;
    let mut order =
        static_evaluation_order_from_admitted(roots, module_count, dependencies, budget)?;
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

/// Each root's fresh evaluation order: the post-order ES modules run when
/// that root alone is loaded first (plan M3.3, "per-entry orders").
pub fn fresh_orders(roots: &[usize], dependencies: &[Vec<usize>]) -> Vec<Vec<usize>> {
    roots
        .iter()
        .map(|&root| {
            let mut order = Vec::new();
            let mut states = vec![0u8; dependencies.len()];
            let mut stack = vec![(root, 0usize)];
            states[root] = 1;
            while let Some((module, next)) = stack.last_mut() {
                let module = *module;
                if let Some(&child) = dependencies[module].get(*next) {
                    *next += 1;
                    if states[child] == 0 {
                        states[child] = 1;
                        stack.push((child, 0));
                    }
                } else {
                    stack.pop();
                    states[module] = 2;
                    order.push(module);
                }
            }
            order
        })
        .collect()
}

/// The static import cycles of a module graph: per module, the index of its
/// strongly connected component when that component is a cycle (several
/// modules, or one that imports itself). Tarjan's algorithm, iterative.
pub fn static_cycles(dependencies: &[Vec<usize>]) -> Vec<Option<u32>> {
    let count = dependencies.len();
    let mut index = vec![usize::MAX; count];
    let mut low = vec![0usize; count];
    let mut on_stack = vec![false; count];
    let mut stack = Vec::new();
    let mut cycles = vec![None; count];
    let mut next_index = 0;
    let mut next_cycle = 0u32;
    for start in 0..count {
        if index[start] != usize::MAX {
            continue;
        }
        let mut frames = vec![(start, 0usize)];
        index[start] = next_index;
        low[start] = next_index;
        next_index += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&mut (module, ref mut edge)) = frames.last_mut() {
            if let Some(&child) = dependencies[module].get(*edge) {
                *edge += 1;
                if index[child] == usize::MAX {
                    index[child] = next_index;
                    low[child] = next_index;
                    next_index += 1;
                    stack.push(child);
                    on_stack[child] = true;
                    frames.push((child, 0));
                } else if on_stack[child] {
                    low[module] = low[module].min(index[child]);
                }
                continue;
            }
            frames.pop();
            if let Some(&(parent, _)) = frames.last() {
                low[parent] = low[parent].min(low[module]);
            }
            if low[module] == index[module] {
                let mut members = Vec::new();
                loop {
                    let member = stack.pop().expect("the component's root is on the stack");
                    on_stack[member] = false;
                    members.push(member);
                    if member == module {
                        break;
                    }
                }
                let cyclic = members.len() > 1 || dependencies[module].contains(&module);
                if cyclic {
                    for member in members {
                        cycles[member] = Some(next_cycle);
                    }
                    next_cycle += 1;
                }
            }
        }
    }
    cycles
}

/// Where each root's walk enters each static cycle: the first module of the
/// cycle its depth-first walk reaches. Two roots that enter one cycle at
/// different modules evaluate it in different orders.
pub fn cycle_entries(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    cycles: &[Option<u32>],
) -> Vec<Vec<(u32, usize)>> {
    roots
        .iter()
        .map(|&root| {
            let mut entered: Vec<(u32, usize)> = Vec::new();
            let mut seen = vec![false; dependencies.len()];
            let mut stack = vec![(root, 0usize)];
            seen[root] = true;
            if let Some(cycle) = cycles[root] {
                entered.push((cycle, root));
            }
            while let Some((module, next)) = stack.last_mut() {
                let module = *module;
                if let Some(&child) = dependencies[module].get(*next) {
                    *next += 1;
                    if !seen[child] {
                        seen[child] = true;
                        if let Some(cycle) = cycles[child] {
                            if !entered.iter().any(|&(known, _)| known == cycle) {
                                entered.push((cycle, child));
                            }
                        }
                        stack.push((child, 0));
                    }
                } else {
                    stack.pop();
                }
            }
            entered
        })
        .collect()
}

/// The modules some root can load with `import()` without reaching them
/// statically (plan M3.3, design §5.2): the dynamic entries, ascending.
/// With one root, every `import()` target outside its static closure.
pub fn lazy_roots(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    dynamic: &[Vec<usize>],
) -> Vec<usize> {
    let count = dependencies.len();
    let mut lazy = vec![false; count];
    for (order, &root) in fresh_orders(roots, dependencies).iter().zip(roots) {
        let mut own = vec![false; count];
        for &module in order {
            own[module] = true;
        }
        let mut seen = vec![false; count];
        let mut pending = vec![root];
        while let Some(module) = pending.pop() {
            if std::mem::replace(&mut seen[module], true) {
                continue;
            }
            pending.extend(dependencies[module].iter().copied());
            for &target in dynamic.get(module).map_or(&[][..], Vec::as_slice) {
                if !own[target] {
                    lazy[target] = true;
                }
                pending.push(target);
            }
        }
    }
    (0..count).filter(|&module| lazy[module]).collect()
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
    /// The program's entries, sorted by name (plan M3.3): each is a root of
    /// the one module graph. Diagnostics that cite "the root" cite the first.
    pub roots: Vec<ModuleId>,
    /// Each root's entry name, in the same order.
    pub root_names: Vec<String>,
    /// Reachable through static imports from some root.
    pub eager: Vec<bool>,
}

impl<S> ModuleSet<S> {
    /// The first entry's module.
    pub fn root(&self) -> ModuleId {
        self.roots[0]
    }
}

/// One entry of a build: its name and its source file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntrySource {
    pub name: String,
    pub path: PathBuf,
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
    discover_configured_with_storage(
        &[EntrySource::of(root)],
        root_source,
        config,
        OwnedSources { budget },
    )
    .map(|(modules, ())| modules)
}

/// The factory owns both arenas. Original discovery ASTs are retained in
/// canonical module order; no source or syntax storage lives inside the graph.
/// `root_source`, when given, replaces the entry file's text (an editor's
/// unsaved buffer); every other module is read from disk.
pub(crate) fn discover_parsed_modules_admitted<'ast, 'src>(
    entries: &[EntrySource],
    root_source: Option<&str>,
    config: &ProjectConfig,
    sources: &'src StableSourceArena,
    syntax: &'ast AdmittedArena<'_>,
) -> Result<(ModuleSet<&'src str>, ParsedSources<'ast, 'src>), ModuleDiscoveryError> {
    discover_configured_with_storage(
        entries,
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
    entries: &[EntrySource],
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
    discover_with_storage(entries, root_source, resolver, storage)
}

fn discover_modules_inner(
    root: &Path,
    root_source: Option<&str>,
    package_resolver: Option<PackageResolver>,
    budget: &mut AllocationBudget<'_>,
) -> Result<ModuleSet, ModuleDiscoveryError> {
    discover_with_storage(
        &[EntrySource::of(root)],
        root_source,
        package_resolver,
        OwnedSources { budget },
    )
    .map(|(modules, ())| modules)
}

fn discover_with_storage<S: DiscoveryStorage>(
    entries: &[EntrySource],
    root_source: Option<&str>,
    package_resolver: Option<PackageResolver>,
    mut storage: S,
) -> Result<(ModuleSet<S::Source>, S::Parsed), ModuleDiscoveryError> {
    storage.work(1)?;
    let first = entries.first().ok_or_else(|| {
        ModuleError::new(
            "<entries>",
            "",
            Span::empty(0),
            "a build needs at least one entry".to_string(),
        )
    })?;
    let mut paths = Vec::with_capacity(entries.len());
    for entry in entries {
        storage.work(1)?;
        paths.push(canonical_module_path(&entry.path).map_err(|message| {
            ModuleError::new(
                &entry.path,
                if std::ptr::eq(entry, first) {
                    root_source.unwrap_or("")
                } else {
                    ""
                },
                Span::empty(0),
                message,
            )
        })?);
    }
    let mut overrides = AHashMap::default();
    if let Some(source) = root_source {
        overrides.insert(paths[0].clone(), storage.copy_override(source)?);
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
    // Entries in name order: the canonical schedule visits them so.
    let mut roots: Vec<ModuleId> = Vec::with_capacity(entries.len());
    for (entry, path) in entries.iter().zip(&paths) {
        let root = loader.visit(path, None)?;
        if let Some(position) = roots.iter().position(|&other| other == root) {
            return Err(ModuleError::new(
                &entry.path,
                "",
                Span::empty(0),
                format!(
                    "entries `{}` and `{}` name one module; an entry is a module's public surface once",
                    entries[position].name, entry.name
                ),
            )
            .into());
        }
        roots.push(root);
    }
    let mut offset = 0usize;
    for module in &mut loader.modules {
        loader.storage.work(1)?;
        module.offset = offset;
        offset = offset
            .saturating_add(module.source.as_ref().len())
            .saturating_add(1);
    }
    let mut eager = vec![false; loader.modules.len()];
    let mut pending = roots.clone();
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
            roots,
            root_names: entries.iter().map(|entry| entry.name.clone()).collect(),
            eager,
        },
        loader.storage.into_parsed(),
    ))
}

impl EntrySource {
    /// A single input, named by its file stem. The name becomes a file name
    /// and a label, so a character an entry name may not hold becomes `_`
    /// (a declared name is refused instead); any source file stays a valid
    /// input.
    pub fn of(path: &Path) -> Self {
        let stem = path
            .file_stem()
            .map(|stem| stem.to_string_lossy().into_owned())
            .unwrap_or_default();
        let mut name = stem
            .chars()
            .map(|c| {
                if c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-') {
                    c
                } else {
                    '_'
                }
            })
            .collect::<String>();
        if !crate::config::valid_entry_name(&name) {
            name = "main".to_string();
        }
        Self {
            name,
            path: path.to_path_buf(),
        }
    }
}

#[cfg(test)]
mod entry_tests {
    use super::*;

    /// An input's stem names its entry; any source file stays a valid input
    /// (plan M3.3: a declared name is refused instead).
    #[test]
    fn an_input_names_its_entry_by_a_valid_stem() {
        let name = |path: &str| EntrySource::of(Path::new(path)).name;
        assert_eq!(name("src/main.lil"), "main");
        assert_eq!(name("dir/my app.lil"), "my_app");
        assert_eq!(name("café.lil"), "caf_");
        assert_eq!(name("[id].lil"), "_id_");
        assert_eq!(name("a+b.lil"), "a_b");
        assert_eq!(name(".lil"), ".lil");
        assert_eq!(name(""), "main");
        assert!(crate::config::valid_entry_name(&name("main (1).lil")));
    }

    /// A module is a dynamic entry when some root can `import()` it without
    /// reaching it statically (design §5.2); one root: only `import()` reaches it.
    #[test]
    fn lazy_roots_are_lazy_for_some_root() {
        // 0 and 1 are roots; 0 imports 2; 1 imports 2 with `import()` and
        // 3 with `import()`; 3 imports 2.
        let dependencies = vec![vec![2], vec![], vec![], vec![2]];
        let dynamic = vec![vec![], vec![2, 3], vec![], vec![]];
        assert_eq!(lazy_roots(&[0, 1], &dependencies, &dynamic), vec![2, 3]);
        // Root 0 alone reaches 2 statically: only 3 would be lazy, and 0
        // imports nothing lazily.
        assert_eq!(
            lazy_roots(&[0], &dependencies, &dynamic),
            Vec::<usize>::new()
        );
        // Root 1 alone: 2 and 3 are both outside its static closure.
        assert_eq!(lazy_roots(&[1], &dependencies, &dynamic), vec![2, 3]);
    }
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
        .map(|(source, span, _)| (source.to_string(), span))
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
    imports: &mut Vec<(&'src str, Span, SourceNodeId)>,
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
    imports: &mut Vec<(&'src str, Span, SourceNodeId)>,
) {
    collect_param_dynamic_imports(function.params, imports);
    collect_stmt_dynamic_imports(function.body, imports);
}

fn collect_param_dynamic_imports<'ast, 'src>(
    params: &[Param<'ast, 'src>],
    imports: &mut Vec<(&'src str, Span, SourceNodeId)>,
) {
    for default in params.iter().filter_map(|param| param.default.as_ref()) {
        collect_expr_dynamic_imports(default, imports);
    }
}

fn collect_stmt_dynamic_imports<'ast, 'src>(
    statements: &[Stmt<'ast, 'src>],
    imports: &mut Vec<(&'src str, Span, SourceNodeId)>,
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
    imports: &mut Vec<(&'src str, Span, SourceNodeId)>,
) {
    match expression {
        Expr {
            kind: ExprKind::DynamicImport { source, span },
            ..
        } => imports.push((source, *span, expression.id)),
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
        }
        | Expr {
            kind: ExprKind::Cast { value: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::Convert { value: object, .. },
            ..
        }
        | Expr {
            kind: ExprKind::DynamicUnary { expr: object, .. },
            ..
        } => collect_expr_dynamic_imports(object, imports),
        Expr {
            kind: ExprKind::Construct { callee, args, .. },
            ..
        } => {
            collect_expr_dynamic_imports(callee, imports);
            for argument in *args {
                collect_expr_dynamic_imports(&argument.expression, imports);
            }
        }
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
        }
        | Expr {
            kind: ExprKind::DynamicBinary { lhs, rhs, .. },
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
                .map_err(|error| ModuleError::from_parse(&module.path, &module.source, error))
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
