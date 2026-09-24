//! Multi-file delivery of one finished module. Module state, initialization
//! and every function that touches either stay in the entry file. A function
//! whose free references are its own scope, hosts, foreign imports and other
//! such functions moves to its source module's chunk. Chunks only define
//! functions, so even cyclic chunk imports cannot observe an uninitialized
//! binding, and no file assigns a binding another file declares (ES imports
//! are read-only). A module only `import()` reaches travels in its own lazy
//! chunk when all of it moves; the entry then loads that file on demand.
//! Every name is the module's final name, unchanged.

use super::extract::OutputError;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass};

/// Which source modules may carry a delivered chunk, and how split mode
/// selects among them.
pub(crate) struct BundleSpec {
    /// Per module: may carry a chunk (never the entry module).
    pub allowed: Vec<bool>,
    /// Per module: the chunk file's stem.
    pub stems: Vec<String>,
    /// Every chunk file name ends in `.extension`, like the entry's.
    pub extension: &'static str,
    /// Per module: how many modules import it.
    pub importers: Vec<usize>,
    /// Per module: static imports reach it from the entry.
    pub eager: Vec<bool>,
    /// The syntax target has `import()`, so a lazy chunk can be loaded.
    pub dynamic_import: bool,
    /// Which lazy chunks the entry preloads.
    pub preload: crate::config::PreloadPolicy,
    /// Split's selection; without it every allowed module keeps its chunk.
    pub split: Option<crate::compilation_policy::SplitRule>,
    /// Per module import: its source is a host module the entry carries.
    pub hosted: Vec<bool>,
}

/// One delivered file: its source modules and root statements, in root order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryFile {
    pub name: String,
    pub modules: Vec<u32>,
    pub statements: Vec<usize>,
    /// This file is its module's lazy chunk, loaded only by `import()`.
    pub lazy: bool,
}

/// What one file imports from the others and exports to them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileLinks {
    /// (source file, bindings), by file then binding.
    pub imports: Vec<(usize, Vec<BindingId>)>,
    pub exports: Vec<BindingId>,
    /// Indices into `Module::imports` this file references.
    pub foreign: Vec<usize>,
    /// Lazy chunks this file loads with `import()`, as file indices.
    pub dynamic: Vec<usize>,
    /// A lazy chunk's namespace: each export name and its binding.
    pub namespace: Vec<(String, BindingId)>,
}

const NONE: usize = usize::MAX;

#[derive(Clone, Copy)]
enum Reference {
    Read(BindingId),
    Write(BindingId),
    /// `import()` of a source module.
    Load(u32),
}

/// Every binding a root statement's subtree reads or writes, and every
/// module it loads. A lazily delivered module's namespace members are read
/// in its own chunk, not here.
#[allow(clippy::too_many_arguments)]
fn visit_references<'m>(
    module: &'m Module,
    statement: &'m Statement,
    statements: &mut Vec<&'m Statement>,
    expressions: &mut Vec<ExprId>,
    lazy: &[bool],
    budget: &mut AllocationBudget<'_>,
    mut visit: impl FnMut(Reference),
) -> Result<(), OutputError> {
    statements.clear();
    expressions.clear();
    statements.push(statement);
    let region = |id: RegionId, statements: &mut Vec<&'m Statement>| {
        statements.extend(module.regions[id.index()].statements.iter());
    };
    loop {
        if let Some(expression) = expressions.pop() {
            budget.work(WorkKind::Analysis, 1)?;
            let node = &module.expressions[expression.index()];
            match node {
                Expr::Binding(binding) => visit(Reference::Read(*binding)),
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = module.expressions[target.index()] {
                        visit(Reference::Write(binding));
                    }
                }
                Expr::LoadModule {
                    module: loaded,
                    promise,
                    string,
                    ..
                } => {
                    visit(Reference::Load(*loaded));
                    if lazy.get(*loaded as usize).copied().unwrap_or(false) {
                        expressions.push(*promise);
                        expressions.push(*string);
                        continue;
                    }
                }
                _ => {}
            }
            if let Some(function) = node.created_function() {
                region(module.functions[function.index()].body, statements);
            }
            node.visit_children(|child| {
                expressions.push(child);
                Ok::<_, ()>(())
            })
            .unwrap();
            continue;
        }
        let Some(current) = statements.pop() else {
            return Ok(());
        };
        budget.work(WorkKind::Analysis, 1)?;
        current.visit_expressions(|root| expressions.push(root));
        match current {
            Statement::If { yes, no, .. } => {
                region(*yes, statements);
                if let Some(no) = no {
                    region(*no, statements);
                }
            }
            Statement::Loop { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. }
            | Statement::Block(body) => region(*body, statements),
            Statement::Try {
                body,
                catch,
                finally,
            } => {
                region(*body, statements);
                if let Some(catch) = catch {
                    region(catch.body, statements);
                }
                if let Some(finally) = finally {
                    region(*finally, statements);
                }
            }
            Statement::Function { function, .. } => {
                region(module.functions[function.index()].body, statements)
            }
            _ => {}
        }
    }
}

/// The root statement declaring each root binding.
fn root_owners(
    module: &Module,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, OutputError> {
    let mut owners = budget.filled(AllocationClass::Scratch, module.bindings.len(), NONE)?;
    for (index, statement) in module.regions[module.root.index()]
        .statements
        .iter()
        .enumerate()
    {
        budget.work(WorkKind::Analysis, 1)?;
        if let Statement::Let { binding, .. } | Statement::Function { binding, .. } = statement {
            owners[binding.index()] = index;
        }
    }
    Ok(owners)
}

/// Root statements that may move to a chunk before module permissions:
/// `let f=<function>` that nothing assigns again, whose subtree writes no
/// root binding and loads no module, with the other root statements each
/// statement reads. Also every loaded module's namespace members.
pub(crate) struct Partition {
    candidate: Vec<bool>,
    reads: Vec<Vec<usize>>,
    owners: Vec<usize>,
    /// Per source module: its namespace, when some `import()` loads it.
    namespaces: Vec<Option<Vec<(String, BindingId)>>>,
}

pub(crate) fn partition(
    module: &Module,
    modules: usize,
    hosted: &[bool],
    budget: &mut AllocationBudget<'_>,
) -> Result<Partition, OutputError> {
    let root = &module.regions[module.root.index()].statements;
    let owners = root_owners(module, budget)?;
    // A carried host module's bindings exist only in the entry.
    let mut entry_only = budget.filled(AllocationClass::Scratch, module.bindings.len(), false)?;
    for (index, import) in module.imports.iter().enumerate() {
        if hosted.get(index).copied().unwrap_or(false) {
            entry_only[import.binding.index()] = true;
        }
    }
    let mut candidate = budget.filled(AllocationClass::Scratch, root.len(), false)?;
    for (index, statement) in root.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        if let Statement::Let {
            value: Some(value), ..
        } = statement
        {
            candidate[index] = matches!(module.expressions[value.index()], Expr::Function(_));
        }
    }
    let mut namespaces: Vec<Option<Vec<(String, BindingId)>>> =
        budget.vector(AllocationClass::Scratch, modules)?;
    namespaces.resize_with(modules, || None);
    budget.work(WorkKind::Analysis, module.expressions.len() as u64)?;
    for expression in &module.expressions {
        let Expr::LoadModule {
            module: loaded,
            members,
            ..
        } = expression
        else {
            continue;
        };
        let slot = namespaces
            .get_mut(*loaded as usize)
            .ok_or("a module load names an unknown module")?;
        if slot.is_some() {
            continue;
        }
        let mut namespace = Vec::with_capacity(members.len());
        for (name, value) in members {
            budget.work(WorkKind::Analysis, 1)?;
            let Expr::Binding(binding) = module.expressions[value.index()] else {
                return Err("a namespace member is not a binding".into());
            };
            namespace.push((name.clone(), binding));
        }
        *slot = Some(namespace);
    }
    // Reads of other root statements, per statement; any root write or a
    // write to a candidate's binding disqualifies, as does loading a module.
    let mut reads: Vec<Vec<usize>> = budget.vector(AllocationClass::Scratch, root.len())?;
    let mut regions = Vec::new();
    let mut expressions = Vec::new();
    let mut written = budget.filled(AllocationClass::Scratch, root.len(), false)?;
    for (index, statement) in root.iter().enumerate() {
        let mut own = Vec::new();
        let mut stays = false;
        visit_references(
            module,
            statement,
            &mut regions,
            &mut expressions,
            &[],
            budget,
            |reference| match reference {
                Reference::Read(binding) => {
                    let owner = owners[binding.index()];
                    if owner != NONE && owner != index {
                        own.push(owner);
                    }
                    if entry_only[binding.index()] {
                        stays = true;
                    }
                }
                Reference::Write(binding) => {
                    let owner = owners[binding.index()];
                    if owner != NONE {
                        stays = true;
                        written[owner] = true;
                    }
                }
                Reference::Load(_) => stays = true,
            },
        )?;
        if stays {
            candidate[index] = false;
        }
        own.sort_unstable();
        own.dedup();
        reads.push(own);
    }
    for (index, written) in written.iter().enumerate() {
        if *written {
            candidate[index] = false;
        }
    }
    Ok(Partition {
        candidate,
        reads,
        owners,
        namespaces,
    })
}

/// The candidates whose source module may carry a chunk and that read only
/// other such statements (greatest fixpoint), so no chunk imports the entry.
pub(crate) fn chunkable(
    module: &Module,
    partition: &Partition,
    allowed: &[bool],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<bool>, OutputError> {
    let count = partition.candidate.len();
    if module.root_modules.len() != count {
        return Err("root statements do not record their source modules".into());
    }
    let mut chunkable = budget.filled(AllocationClass::Scratch, count, false)?;
    for index in 0..count {
        budget.work(WorkKind::Analysis, 1)?;
        chunkable[index] = partition.candidate[index]
            && allowed
                .get(module.root_modules[index] as usize)
                .copied()
                .unwrap_or(false);
    }
    loop {
        let mut changed = false;
        for index in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            if chunkable[index] && partition.reads[index].iter().any(|&read| !chunkable[read]) {
                chunkable[index] = false;
                changed = true;
            }
        }
        if !changed {
            return Ok(chunkable);
        }
    }
}

/// Modules delivered in their own lazy chunk: some `import()` loads them,
/// no static import reaches them, and every root statement and namespace
/// member of theirs moves, so the entry never names what the chunk holds.
pub(crate) fn lazy_modules(
    module: &Module,
    partition: &Partition,
    chunkable: &[bool],
    spec: &BundleSpec,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<bool>, OutputError> {
    let modules = partition.namespaces.len();
    let mut lazy = budget.filled(AllocationClass::Scratch, modules, false)?;
    if !spec.dynamic_import {
        return Ok(lazy);
    }
    let mut moves = budget.filled(AllocationClass::Scratch, modules, true)?;
    let mut owns = budget.filled(AllocationClass::Scratch, modules, false)?;
    for (index, &source) in module.root_modules.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        let source = source as usize;
        if source < modules {
            owns[source] = true;
            moves[source] &= chunkable[index];
        }
    }
    for source in 0..modules {
        budget.work(WorkKind::Analysis, 1)?;
        let Some(namespace) = &partition.namespaces[source] else {
            continue;
        };
        lazy[source] = owns[source]
            && moves[source]
            && !spec.eager.get(source).copied().unwrap_or(true)
            && spec.allowed.get(source).copied().unwrap_or(false)
            && namespace.iter().all(|(_, binding)| {
                let owner = partition.owners[binding.index()];
                owner != NONE && chunkable[owner]
            });
    }
    Ok(lazy)
}

/// Each file's shortest import distance from the entry (file 0), through
/// static and dynamic imports.
pub(crate) fn depths(
    links: &[FileLinks],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, OutputError> {
    let mut depths = budget.filled(AllocationClass::Scratch, links.len(), NONE)?;
    let mut queue = budget.vector(AllocationClass::Scratch, links.len())?;
    if !links.is_empty() {
        depths[0] = 0;
        queue.push(0usize);
    }
    let mut next = 0;
    while let Some(&file) = queue.get(next) {
        next += 1;
        let targets = links[file]
            .imports
            .iter()
            .map(|(source, _)| *source)
            .chain(links[file].dynamic.iter().copied());
        for source in targets {
            budget.work(WorkKind::Analysis, 1)?;
            if depths[source] == NONE {
                depths[source] = depths[file] + 1;
                queue.push(source);
            }
        }
    }
    Ok(depths)
}

/// Each file's imports, exports and foreign imports, and the lazy chunks it
/// loads. Fails on a cross-file write, which an ES module cannot express.
pub(crate) fn link(
    module: &Module,
    files: &[DeliveryFile],
    entry: usize,
    partition: &Partition,
    lazy: &[bool],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<FileLinks>, OutputError> {
    let root = &module.regions[module.root.index()].statements;
    let owners = &partition.owners;
    let mut file_of = budget.filled(AllocationClass::Scratch, root.len(), NONE)?;
    let mut lazy_file = budget.filled(AllocationClass::Scratch, lazy.len(), NONE)?;
    for (index, file) in files.iter().enumerate() {
        if file.lazy {
            if let Some(&source) = file.modules.first() {
                lazy_file[source as usize] = index;
            }
        }
        for &statement in &file.statements {
            budget.work(WorkKind::Analysis, 1)?;
            if statement >= root.len() || file_of[statement] != NONE {
                return Err("a root statement is delivered twice or not at all".into());
            }
            file_of[statement] = index;
        }
    }
    if file_of.contains(&NONE) {
        return Err("a root statement is not delivered".into());
    }
    let mut foreign_of = budget.filled(AllocationClass::Scratch, module.bindings.len(), NONE)?;
    for (index, import) in module.imports.iter().enumerate() {
        foreign_of[import.binding.index()] = index;
    }
    let mut links: Vec<FileLinks> = budget.vector(AllocationClass::Scratch, files.len())?;
    links.resize_with(files.len(), FileLinks::default);
    let mut needed: Vec<(usize, usize, BindingId)> = Vec::new();
    let mut regions = Vec::new();
    let mut expressions = Vec::new();
    for (index, file) in files.iter().enumerate() {
        for &statement in &file.statements {
            let mut failure = None;
            visit_references(
                module,
                &root[statement],
                &mut regions,
                &mut expressions,
                lazy,
                budget,
                |reference| {
                    let (binding, write) = match reference {
                        Reference::Read(binding) => (binding, false),
                        Reference::Write(binding) => (binding, true),
                        Reference::Load(loaded) => {
                            let target = lazy_file.get(loaded as usize).copied().unwrap_or(NONE);
                            if target != NONE {
                                links[index].dynamic.push(target);
                            }
                            return;
                        }
                    };
                    if foreign_of[binding.index()] != NONE {
                        links[index].foreign.push(foreign_of[binding.index()]);
                        return;
                    }
                    let owner = owners[binding.index()];
                    if owner == NONE || file_of[owner] == index {
                        return;
                    }
                    if write {
                        failure = Some("a file assigns a binding another file declares");
                        return;
                    }
                    needed.push((index, file_of[owner], binding));
                },
            )?;
            if let Some(failure) = failure {
                return Err(failure.into());
            }
        }
    }
    // A lazy chunk exports its namespace, importing any member it does not
    // itself declare.
    for (index, file) in files.iter().enumerate() {
        if !file.lazy {
            continue;
        }
        let Some(Some(namespace)) = file
            .modules
            .first()
            .and_then(|source| partition.namespaces.get(*source as usize))
        else {
            return Err("a lazy chunk has no namespace".into());
        };
        for (name, binding) in namespace {
            budget.work(WorkKind::Analysis, 1)?;
            let owner = owners[binding.index()];
            if owner == NONE {
                return Err("a namespace member has no declaration".into());
            }
            if file_of[owner] != index {
                needed.push((index, file_of[owner], *binding));
            }
            links[index].namespace.push((name.clone(), *binding));
        }
    }
    // The entry re-exports the public surface, wherever it is declared.
    for export in &module.exports {
        budget.work(WorkKind::Analysis, 1)?;
        if foreign_of[export.binding.index()] != NONE {
            links[entry]
                .foreign
                .push(foreign_of[export.binding.index()]);
            continue;
        }
        let owner = owners[export.binding.index()];
        if owner != NONE && file_of[owner] != entry {
            needed.push((entry, file_of[owner], export.binding));
        }
    }
    budget.work(WorkKind::Analysis, needed.len() as u64)?;
    needed.sort_unstable();
    needed.dedup();
    for &(importer, source, binding) in &needed {
        if source == entry {
            return Err("a chunk imports from the entry".into());
        }
        let imports = &mut links[importer].imports;
        match imports.last_mut() {
            Some((last, bindings)) if *last == source => bindings.push(binding),
            _ => imports.push((source, vec![binding])),
        }
        links[source].exports.push(binding);
    }
    for link in &mut links {
        link.exports.sort_unstable();
        link.exports.dedup();
        link.foreign.sort_unstable();
        link.foreign.dedup();
        link.dynamic.sort_unstable();
        link.dynamic.dedup();
    }
    Ok(links)
}

/// The entry first, then one chunk per source module that keeps any
/// chunkable statement, in first-statement order; every other statement
/// stays in the entry. A lazy module's chunk is marked lazy.
pub(crate) fn plan_files(
    module: &Module,
    chunkable: &[bool],
    lazy: &[bool],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<DeliveryFile>, OutputError> {
    let root = &module.regions[module.root.index()].statements;
    if module.root_modules.len() != root.len() || chunkable.len() != root.len() {
        return Err("root statements do not record their source modules".into());
    }
    let mut files = vec![DeliveryFile {
        name: String::new(),
        modules: Vec::new(),
        statements: Vec::new(),
        lazy: false,
    }];
    let modules = module
        .root_modules
        .iter()
        .max()
        .map_or(0, |&module| module as usize + 1);
    let mut chunk_of = budget.filled(AllocationClass::Scratch, modules, NONE)?;
    for (index, &source) in module.root_modules.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        let source = source as usize;
        let file = if chunkable[index] {
            if chunk_of[source] == NONE {
                chunk_of[source] = files.len();
                files.push(DeliveryFile {
                    name: String::new(),
                    modules: vec![source as u32],
                    statements: Vec::new(),
                    lazy: lazy.get(source).copied().unwrap_or(false),
                });
            }
            chunk_of[source]
        } else {
            0
        };
        files[file].statements.push(index);
        if file == 0 && !files[0].modules.contains(&(source as u32)) {
            files[0].modules.push(source as u32);
        }
    }
    files[0].modules.sort_unstable();
    Ok(files)
}
