//! Multi-file delivery of one finished module. Module state, initialization
//! and every function that touches either stay in the entry file. A function
//! whose free references are its own scope, hosts, foreign imports and other
//! such functions moves to its source module's chunk. Chunks only define
//! functions, so even cyclic chunk imports cannot observe an uninitialized
//! binding, and no file assigns a binding another file declares (ES imports
//! are read-only). Every name is the module's final name, unchanged.

use super::*;
use crate::compilation_policy::WorkKind;
use super::extract::OutputError;
use crate::output_budget::{AllocationBudget, AllocationClass};

/// Which source modules may carry a delivered chunk, and how split mode
/// selects among them.
pub(crate) struct BundleSpec {
    /// Per module: may carry a chunk (never the entry module).
    pub allowed: Vec<bool>,
    /// Per module: the chunk file's stem.
    pub stems: Vec<String>,
    /// Per module: how many modules import it.
    pub importers: Vec<usize>,
    /// Split's selection; without it every allowed module keeps its chunk.
    pub split: Option<crate::compilation_policy::SplitRule>,
}

/// One delivered file: its source modules and root statements, in root order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryFile {
    pub name: String,
    pub modules: Vec<u32>,
    pub statements: Vec<usize>,
}

/// What one file imports from the others and exports to them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileLinks {
    /// (source file, bindings), by file then binding.
    pub imports: Vec<(usize, Vec<BindingId>)>,
    pub exports: Vec<BindingId>,
    /// Indices into `Module::imports` this file references.
    pub foreign: Vec<usize>,
}

const NONE: usize = usize::MAX;

/// Every binding a root statement's subtree reads or writes.
fn visit_references<'m>(
    module: &'m Module,
    statement: &'m Statement,
    statements: &mut Vec<&'m Statement>,
    expressions: &mut Vec<ExprId>,
    budget: &mut AllocationBudget<'_>,
    mut visit: impl FnMut(BindingId, bool),
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
                Expr::Binding(binding) => visit(*binding, false),
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = module.expressions[target.index()] {
                        visit(binding, true);
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
/// `let f=<function>` that nothing assigns again and whose subtree writes no
/// root binding, with the other root statements each statement reads.
pub(crate) struct Partition {
    candidate: Vec<bool>,
    reads: Vec<Vec<usize>>,
}

pub(crate) fn partition(
    module: &Module,
    budget: &mut AllocationBudget<'_>,
) -> Result<Partition, OutputError> {
    let root = &module.regions[module.root.index()].statements;
    let owners = root_owners(module, budget)?;
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
    // Reads of other root statements, per statement; any root write or a
    // write to a candidate's binding disqualifies.
    let mut reads: Vec<Vec<usize>> = budget.vector(AllocationClass::Scratch, root.len())?;
    let mut regions = Vec::new();
    let mut expressions = Vec::new();
    let mut written = budget.filled(AllocationClass::Scratch, root.len(), false)?;
    for (index, statement) in root.iter().enumerate() {
        let mut own = Vec::new();
        let mut writes_root = false;
        visit_references(
            module,
            statement,
            &mut regions,
            &mut expressions,
            budget,
            |binding, write| {
                let owner = owners[binding.index()];
                if owner == NONE {
                    return;
                }
                if write {
                    writes_root = true;
                    written[owner] = true;
                } else if owner != index {
                    own.push(owner);
                }
            },
        )?;
        if writes_root {
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
    Ok(Partition { candidate, reads })
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

/// Each file's shortest import distance from the entry (file 0).
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
        for (source, _) in &links[file].imports {
            budget.work(WorkKind::Analysis, 1)?;
            if depths[*source] == NONE {
                depths[*source] = depths[file] + 1;
                queue.push(*source);
            }
        }
    }
    Ok(depths)
}

/// Each file's imports, exports and foreign imports. Fails on a cross-file
/// write, which an ES module cannot express.
pub(crate) fn link(
    module: &Module,
    files: &[DeliveryFile],
    entry: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<FileLinks>, OutputError> {
    let root = &module.regions[module.root.index()].statements;
    let owners = root_owners(module, budget)?;
    let mut file_of = budget.filled(AllocationClass::Scratch, root.len(), NONE)?;
    for (index, file) in files.iter().enumerate() {
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
                budget,
                |binding, write| {
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
    // The entry re-exports the public surface, wherever it is declared.
    for export in &module.exports {
        budget.work(WorkKind::Analysis, 1)?;
        if foreign_of[export.binding.index()] != NONE {
            links[entry].foreign.push(foreign_of[export.binding.index()]);
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
    }
    Ok(links)
}

/// The entry first, then one chunk per source module that keeps any
/// chunkable statement, in first-statement order; every other statement
/// stays in the entry.
pub(crate) fn plan_files(
    module: &Module,
    chunkable: &[bool],
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
