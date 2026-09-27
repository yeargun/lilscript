//! Delivery placement (plan M3.3; design `out/design/m3.3/design.md` §7).
//!
//! One formed tree, several files. Placement runs once per formed tree,
//! after the target rules and before naming, and is stored on the tree
//! (`Module::delivery`): naming and printing read it, nothing recomputes it.
//!
//! **The reference.** `preserve-modules` is what ES modules evaluate for the
//! source: every module a file, each importing what its source imported.
//! Every other plan is a motion from it that a simulator checks.
//!
//! **Labels.** One bit per static entry (name order), then one per module
//! only `import()` reaches. An anchored statement is labelled with the
//! entries that evaluate its module; a definition with the union of the
//! labels of what references it, and of the entries that export it.
//!
//! **`split`.** The fallback plan is legal by construction: one file per
//! (label, module) group of anchored statements; each definition joins the
//! latest same-label group it depends on, or a definitions-only file of its
//! label; one facade per entry imports the anchored files carrying its bit in
//! its own module order and re-exports its names. Merges are then tried, and
//! one is kept only when every entry still runs exactly the anchored
//! statements of the modules it reaches, in its own order, and the file graph
//! stays acyclic.
//!
//! **`single`.** One file per entry. With one entry there is no plan and
//! today's single-file printer runs; with several, each entry's file is the
//! fallback plan's files in that entry's evaluation order.
//!
//! A write to a binding another file declares becomes a call of a setter
//! that file declares (`$x(v)`, `let $x=v=>x=v`): ES imports are read-only.
//!
//! Prior art: esbuild `f6058f8 internal/linker/linker.go:3875`
//! (`computeChunks`: one chunk per entry bitset), `:560` (acyclic chunk
//! imports as an assertion), `:1096` (entries import chunks for their side
//! effects); Rolldown `5c676e5 code_splitting.rs:213-220` (order by execution
//! position, never by bitset value), `compute_cross_chunk_links.rs:38-46`
//! (no internal export named `then`); Closure `0da58e1 JSChunkGraph.java:374`
//! (a definition's chunk covers its users), `CrossChunkCodeMotion.java:40-66`
//! (mutually referencing definitions move together),
//! `ConvertChunksToESModules.java:81-84` (cross-chunk assignment, refused
//! there, a setter here).

use super::extract::OutputError;
use super::*;
use crate::compilation_policy::{DeliveryContract, WorkKind};
use crate::config::DeliveryMode;
use crate::output_budget::AllocationBudget;

const NONE: usize = usize::MAX;

/// A set of entries: bit `i` is static entry `i` for `i` below the static
/// count, then the lazily loaded modules in order. Trailing zero words are
/// trimmed, so equal sets compare equal.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct EntrySet(Vec<u64>);

impl EntrySet {
    pub fn single(bit: usize) -> Self {
        let mut set = Self::default();
        set.insert(bit);
        set
    }
    pub fn insert(&mut self, bit: usize) {
        let word = bit / 64;
        if self.0.len() <= word {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (bit % 64);
    }
    pub fn remove(&mut self, bit: usize) {
        if let Some(word) = self.0.get_mut(bit / 64) {
            *word &= !(1 << (bit % 64));
        }
        self.trim();
    }
    fn trim(&mut self) {
        while self.0.last() == Some(&0) {
            self.0.pop();
        }
    }
    pub fn contains(&self, bit: usize) -> bool {
        self.0
            .get(bit / 64)
            .is_some_and(|word| word & (1 << (bit % 64)) != 0)
    }
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    /// Adds `other`; whether this set grew.
    pub fn union_with(&mut self, other: &Self) -> bool {
        if self.0.len() < other.0.len() {
            self.0.resize(other.0.len(), 0);
        }
        let mut grew = false;
        for (word, other) in self.0.iter_mut().zip(&other.0) {
            let joined = *word | other;
            grew |= joined != *word;
            *word = joined;
        }
        grew
    }
    pub fn is_superset(&self, other: &Self) -> bool {
        other
            .0
            .iter()
            .enumerate()
            .all(|(index, word)| self.0.get(index).copied().unwrap_or(0) & word == *word)
    }
    pub fn bits(&self) -> impl Iterator<Item = usize> + '_ {
        self.0.iter().enumerate().flat_map(|(index, &word)| {
            (0..64)
                .filter(move |bit| word & (1 << bit) != 0)
                .map(move |bit| index * 64 + bit)
        })
    }
    /// Whether every bit is below `statics`: only static entries.
    pub fn only_static(&self, statics: usize) -> bool {
        self.bits().all(|bit| bit < statics)
    }
}

impl std::fmt::Debug for EntrySet {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_set().entries(self.bits()).finish()
    }
}

/// Labels compare as their sorted bit lists, which, bits being entries in
/// name order, is their entry names compared lexicographically (never an
/// integer value; Rolldown's warning, `code_splitting.rs:213-220`).
impl Ord for EntrySet {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.bits().cmp(other.bits())
    }
}
impl PartialOrd for EntrySet {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// The program's entries and module graph as placement reads them (design
/// §6 "an `EntryGraph` projection"): modules and bits, never the Program.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntryGraph {
    /// Static entries in name order: name and module.
    pub entries: Vec<(String, u32)>,
    /// Modules only `import()` reaches that some `import()` loads, by first
    /// load in the canonical schedule: bit `entries.len() + i`.
    pub dynamic: Vec<u32>,
    /// Per module: its static imports, in source order.
    pub imports: Vec<Vec<u32>>,
    /// Per module: its position in the canonical schedule.
    pub position: Vec<u32>,
    /// Per static entry, then per dynamic entry: its fresh evaluation order.
    pub orders: Vec<Vec<u32>>,
    /// Per module: the entries whose static closure holds it.
    pub reach: Vec<EntrySet>,
    /// Per module: its static import cycle.
    pub cycles: Vec<Option<u32>>,
    /// Modules importing a host module the output carries: host code runs
    /// for the entries that reach one of them.
    pub host_importers: Vec<u32>,
}

impl EntryGraph {
    fn statics(&self) -> usize {
        self.entries.len()
    }
    fn bits(&self) -> usize {
        self.entries.len() + self.dynamic.len()
    }
    /// The bit of a lazily loaded module, if it is one.
    fn dynamic_bit(&self, module: u32) -> Option<usize> {
        self.dynamic
            .iter()
            .position(|&loaded| loaded == module)
            .map(|index| self.entries.len() + index)
    }
    /// The label host code carries.
    fn host_label(&self) -> EntrySet {
        let mut label = EntrySet::default();
        for &module in &self.host_importers {
            if let Some(reach) = self.reach.get(module as usize) {
                label.union_with(reach);
            }
        }
        label
    }
}

/// A file's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileRole {
    /// An entry's facade (or, in `single`, its whole file): named by
    /// `entry_names`; nothing imports it.
    Entry(u32),
    /// A lazily loaded module's file, which `import()` loads.
    Lazy(u32),
    /// Code several entries share.
    Shared,
    /// A source module's file in `preserve-modules`.
    Module(u32),
    /// Definitions only, of one label.
    Definitions,
    /// Delivered host code.
    Host,
}

/// What one file imports, exports and loads.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FileLinks {
    /// Static imports in evaluation order: the file and the bindings it
    /// imports from it; none for an import only for its effects.
    pub imports: Vec<(u32, Vec<BindingId>)>,
    /// Bindings other files import from this one, under their own names.
    pub exports: Vec<BindingId>,
    /// Public names: (name, binding, the file declaring it). A facade
    /// re-exports what another file declares.
    pub public: Vec<(String, BindingId, u32)>,
    /// `Module::imports` this file prints as foreign imports.
    pub foreign: Vec<usize>,
    /// `Module::imports` of carried host modules this file declares (the
    /// host file only, when host code was not lowered into the tree).
    pub hosted: Vec<usize>,
    /// Files this file loads with `import()`.
    pub dynamic: Vec<u32>,
}

/// One delivered file of a plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedFile {
    pub role: FileRole,
    pub label: EntrySet,
    /// Source modules whose statements it holds, ascending.
    pub modules: Vec<u32>,
    /// Root statements, in print order.
    pub statements: Vec<u32>,
    /// Holds an anchored statement: loading it has effects.
    pub anchored: bool,
    pub links: FileLinks,
}

/// One entry of a delivery: its facade and every file it loads, in
/// evaluation order (the rows the objective scores, design §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryDelivery {
    pub name: String,
    pub file: u32,
    /// Loaded by `import()`, after the entries that load it.
    pub dynamic: bool,
    /// Files this entry loads (for a lazily loaded one: beyond what its
    /// first importer already loaded), in evaluation order.
    pub closure: Vec<u32>,
}

/// Where every root statement is delivered (design §7.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeliveryPlan {
    pub mode: DeliveryMode,
    pub files: Vec<PlannedFile>,
    pub entries: Vec<EntryDelivery>,
    /// Setters placement created: (assigned binding, its setter).
    pub setters: Vec<(BindingId, BindingId)>,
    /// Static entries; the rest of `entries` are lazily loaded.
    pub statics: usize,
}

impl DeliveryPlan {
    /// The file a lazily loaded module is delivered in, for `import()`.
    pub fn lazy_file(&self, module: u32) -> Option<usize> {
        self.files
            .iter()
            .position(|file| file.role == FileRole::Lazy(module))
    }
}

// ---------------------------------------------------------------- facts

/// What placement reads of each root statement: the structural reference
/// sets (reads and writes of root bindings, including inside functions it
/// creates, and the modules it loads) and its row.
struct Facts {
    count: usize,
    /// Per binding: the root statement declaring it.
    owners: Vec<usize>,
    /// Per binding: the `Module::imports` row binding it.
    foreign: Vec<usize>,
    /// Per statement: (binding, owner statement, write) it references in
    /// another statement, sorted.
    references: Vec<Vec<(BindingId, usize, bool)>>,
    /// Per statement: foreign imports it references.
    imports: Vec<Vec<usize>>,
    /// Per statement: lazily loaded modules it loads.
    loads: Vec<Vec<u32>>,
    /// Per statement: its effective anchor.
    anchored: Vec<bool>,
    rows: Vec<RootRow>,
}

#[derive(Clone, Copy)]
enum Reference {
    Read(BindingId),
    Write(BindingId, bool),
    Load(u32, bool),
}

/// Every binding a root statement's subtree reads or writes (and whether
/// the write is outside the functions it creates), and every module it
/// loads. A lazily delivered module's namespace members are read in its
/// own file, not here.
fn visit_references(
    module: &Module,
    statement: &Statement,
    lazy: &dyn Fn(u32) -> bool,
    budget: &mut AllocationBudget<'_>,
    mut visit: impl FnMut(Reference),
) -> Result<(), OutputError> {
    let mut statements: Vec<(&Statement, bool)> = vec![(statement, false)];
    let mut expressions: Vec<(ExprId, bool)> = Vec::new();
    loop {
        if let Some((expression, nested)) = expressions.pop() {
            budget.work(WorkKind::Analysis, 1)?;
            let node = &module.expressions[expression.index()];
            match node {
                Expr::Binding(binding) => visit(Reference::Read(*binding)),
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = module.expressions[target.index()] {
                        visit(Reference::Write(binding, nested));
                    }
                }
                Expr::LoadModule {
                    module: loaded,
                    promise,
                    string,
                    ..
                } => {
                    visit(Reference::Load(*loaded, nested));
                    if lazy(*loaded) {
                        expressions.push((*promise, nested));
                        expressions.push((*string, nested));
                        continue;
                    }
                }
                _ => {}
            }
            for function in node.created_functions() {
                statements.extend(
                    module.regions[module.functions[function.index()].body.index()]
                        .statements
                        .iter()
                        .map(|statement| (statement, true)),
                );
            }
            node.visit_children(|child| {
                expressions.push((child, nested));
                Ok::<_, ()>(())
            })
            .unwrap();
            continue;
        }
        let Some((current, nested)) = statements.pop() else {
            return Ok(());
        };
        budget.work(WorkKind::Analysis, 1)?;
        current.visit_expressions(|root| expressions.push((root, nested)));
        let mut region = |id: RegionId| {
            statements.extend(
                module.regions[id.index()]
                    .statements
                    .iter()
                    .map(|statement| (statement, nested)),
            );
        };
        match current {
            Statement::If { yes, no, .. } => {
                region(*yes);
                if let Some(no) = no {
                    region(*no);
                }
            }
            Statement::Loop { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. }
            | Statement::Block(body) => region(*body),
            Statement::Try {
                body,
                catch,
                finally,
            } => {
                region(*body);
                if let Some(catch) = catch {
                    region(catch.body);
                }
                if let Some(finally) = finally {
                    region(*finally);
                }
            }
            Statement::Function { function, .. } => {
                statements.extend(
                    module.regions[module.functions[function.index()].body.index()]
                        .statements
                        .iter()
                        .map(|statement| (statement, true)),
                );
            }
            _ => {}
        }
    }
}

impl Facts {
    fn collect(
        module: &Module,
        lazy: &dyn Fn(u32) -> bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, OutputError> {
        let root = &module.regions[module.root.index()].statements;
        if module.root_rows.len() != root.len() {
            return Err("root statements and their rows disagree".into());
        }
        let mut owners = vec![NONE; module.bindings.len()];
        for (index, statement) in root.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if let Statement::Let { binding, .. } | Statement::Function { binding, .. } = statement
            {
                owners[binding.index()] = index;
            }
        }
        let mut foreign = vec![NONE; module.bindings.len()];
        for (index, import) in module.imports.iter().enumerate() {
            foreign[import.binding.index()] = index;
        }
        let mut references = Vec::with_capacity(root.len());
        let mut imports = Vec::with_capacity(root.len());
        let mut loads = Vec::with_capacity(root.len());
        let mut anchored = Vec::with_capacity(root.len());
        for (index, statement) in root.iter().enumerate() {
            let mut own: Vec<(BindingId, usize, bool)> = Vec::new();
            let mut foreign_used = Vec::new();
            let mut loaded = Vec::new();
            let mut writes_root = false;
            let mut loads_here = false;
            visit_references(module, statement, lazy, budget, |reference| match reference {
                Reference::Read(binding) => {
                    let owner = owners[binding.index()];
                    if owner != NONE && owner != index {
                        own.push((binding, owner, false));
                    }
                    if foreign[binding.index()] != NONE {
                        foreign_used.push(foreign[binding.index()]);
                    }
                }
                Reference::Write(binding, nested) => {
                    let owner = owners[binding.index()];
                    if owner != NONE && owner != index {
                        own.push((binding, owner, true));
                        if !nested {
                            writes_root = true;
                        }
                    }
                }
                Reference::Load(target, nested) => {
                    if !nested {
                        loads_here = true;
                    }
                    if lazy(target) {
                        loaded.push(target);
                    }
                }
            })?;
            own.sort_unstable();
            own.dedup();
            foreign_used.sort_unstable();
            foreign_used.dedup();
            loaded.sort_unstable();
            loaded.dedup();
            // The row's anchor, checked against the tree (design §6): a
            // definition declares its binding, assigns no other root
            // binding outside the functions it creates and loads nothing.
            let row = module.root_rows[index];
            let declares = matches!(
                statement,
                Statement::Let { .. } | Statement::Function { .. }
            );
            let definition =
                row.anchor == Anchor::Definition && declares && !writes_root && !loads_here;
            debug_assert!(
                row.anchor == Anchor::Anchored || definition,
                "root statement {index} is a definition row that does not only define"
            );
            references.push(own);
            imports.push(foreign_used);
            loads.push(loaded);
            anchored.push(!definition);
        }
        Ok(Self {
            count: root.len(),
            owners,
            foreign,
            references,
            imports,
            loads,
            anchored,
            rows: module.root_rows.clone(),
        })
    }

    /// The module whose evaluation an anchored statement belongs to; host
    /// code belongs to none (`None`).
    fn module_of(&self, statement: usize) -> Option<u32> {
        (self.rows[statement].origin != RowOrigin::Host).then_some(self.rows[statement].module)
    }
}

// ---------------------------------------------------------------- atoms and labels

/// Statements that must share a file: one binding per atom, a binding with
/// the writers that must stay with it.
struct Atoms {
    of: Vec<usize>,
    /// Per atom (by representative): its members, ascending.
    members: Vec<Vec<usize>>,
    /// Per atom: holds an anchored statement.
    anchored: Vec<bool>,
    /// Per atom: the module its anchored statements belong to (`HOST` for
    /// host code), when anchored.
    group: Vec<Option<u32>>,
}

const HOST_GROUP: u32 = u32::MAX;

fn find(parents: &mut [usize], mut index: usize) -> usize {
    while parents[index] != index {
        parents[index] = parents[parents[index]];
        index = parents[index];
    }
    index
}

impl Atoms {
    /// Write groups (design §7.3): a binding assigned outside its
    /// declaration joins every writer that is a definition (a function whose
    /// body assigns it) and every writer in its declaring module, unless
    /// that would put two modules' anchored statements in one atom. Any
    /// other writer is a cross-file write, which becomes a setter call.
    /// `preserve` keeps every statement alone: files are modules there.
    fn build(facts: &Facts, graph: &EntryGraph, preserve: bool) -> Self {
        let count = facts.count;
        let mut parents = (0..count).collect::<Vec<_>>();
        let group_of = |statement: usize| -> Option<u32> {
            facts.anchored[statement].then(|| match facts.module_of(statement) {
                None => HOST_GROUP,
                Some(module) => graph
                    .cycles
                    .get(module as usize)
                    .copied()
                    .flatten()
                    .map_or(module, |cycle| u32::MAX - 1 - cycle),
            })
        };
        let mut groups: Vec<Option<u32>> = (0..count).map(group_of).collect();
        if !preserve {
            for writer in 0..count {
                for &(_, owner, write) in &facts.references[writer] {
                    if !write {
                        continue;
                    }
                    let same_module = facts.rows[writer].module == facts.rows[owner].module
                        && facts.rows[writer].origin == facts.rows[owner].origin;
                    if facts.anchored[writer] && !same_module {
                        continue;
                    }
                    let (left, right) = (find(&mut parents, writer), find(&mut parents, owner));
                    if left == right {
                        continue;
                    }
                    match (groups[left], groups[right]) {
                        (Some(a), Some(b)) if a != b => continue,
                        (a, b) => {
                            parents[left] = right;
                            groups[right] = a.or(b);
                        }
                    }
                }
            }
        }
        let mut of = vec![0; count];
        let mut members = vec![Vec::new(); count];
        let mut anchored = vec![false; count];
        let mut group = vec![None; count];
        for statement in 0..count {
            let atom = find(&mut parents, statement);
            of[statement] = atom;
            members[atom].push(statement);
            anchored[atom] |= facts.anchored[statement];
            group[atom] = groups[atom];
        }
        Self {
            of,
            members,
            anchored,
            group,
        }
    }
}

/// Every atom's label (design §7.2).
fn labels(
    module: &Module,
    facts: &Facts,
    atoms: &Atoms,
    graph: &EntryGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<EntrySet>, OutputError> {
    let count = facts.count;
    let mut labels = vec![EntrySet::default(); count];
    let host = graph.host_label();
    let reach = |statement: usize| -> EntrySet {
        match facts.module_of(statement) {
            None => host.clone(),
            Some(module) => graph.reach.get(module as usize).cloned().unwrap_or_default(),
        }
    };
    for statement in 0..count {
        budget.work(WorkKind::Analysis, 1)?;
        if facts.anchored[statement] {
            let atom = atoms.of[statement];
            let own = reach(statement);
            labels[atom].union_with(&own);
        }
    }
    // Seeds: each entry's public cells; each lazily loaded module's
    // namespace members.
    let owner_atom = |binding: BindingId| -> Option<usize> {
        let owner = facts.owners[binding.index()];
        (owner != NONE).then(|| atoms.of[owner])
    };
    let statics = graph.statics();
    if module.entries.is_empty() {
        for export in &module.exports {
            if let Some(atom) = owner_atom(export.binding) {
                if !atoms.anchored[atom] {
                    labels[atom].insert(0);
                }
            }
        }
    } else {
        for (entry, public) in module.entries.iter().enumerate() {
            for &position in &public.exports {
                let binding = module.exports[position as usize].binding;
                if let Some(atom) = owner_atom(binding) {
                    if !atoms.anchored[atom] {
                        labels[atom].insert(entry);
                    }
                }
            }
        }
    }
    for expression in &module.expressions {
        let Expr::LoadModule {
            module: loaded,
            members,
            ..
        } = expression
        else {
            continue;
        };
        let Some(bit) = graph.dynamic_bit(*loaded) else {
            continue;
        };
        for (_, value) in members {
            if let Expr::Binding(binding) = module.expressions[value.index()] {
                if let Some(atom) = owner_atom(binding) {
                    if !atoms.anchored[atom] {
                        labels[atom].insert(bit);
                    }
                }
            }
        }
    }
    // Definitions: the union of their referencers' labels (least fixpoint).
    let mut referenced: Vec<Vec<usize>> = vec![Vec::new(); count];
    for statement in 0..count {
        let from = atoms.of[statement];
        for &(_, owner, _) in &facts.references[statement] {
            let to = atoms.of[owner];
            if to != from && !atoms.anchored[to] {
                referenced[from].push(to);
            }
        }
    }
    for list in &mut referenced {
        list.sort_unstable();
        list.dedup();
    }
    let mut pending = (0..count)
        .filter(|&atom| atoms.of[atom] == atom)
        .collect::<Vec<_>>();
    while let Some(atom) = pending.pop() {
        budget.work(WorkKind::Analysis, 1 + referenced[atom].len() as u64)?;
        let label = labels[atom].clone();
        if label.is_empty() {
            continue;
        }
        for &target in &referenced[atom] {
            if labels[target].union_with(&label) {
                pending.push(target);
            }
        }
    }
    // A definition nothing reaches (dead code the tree kept) goes where its
    // module's code goes.
    for statement in 0..count {
        let atom = atoms.of[statement];
        if labels[atom].is_empty() {
            let own = reach(statement);
            labels[atom].union_with(&own);
        }
        if labels[atom].is_empty() && statics > 0 {
            labels[atom].insert(0);
        }
    }
    // Already loaded (Rolldown `dynamic_already_loaded`): content whose
    // label holds a lazily loaded module and every entry that loads it is
    // loaded before that module is.
    let mut importers = vec![EntrySet::default(); graph.dynamic.len()];
    for _ in 0..4 {
        for (index, importers) in importers.iter_mut().enumerate() {
            *importers = EntrySet::default();
            let target = graph.dynamic[index];
            for statement in 0..count {
                if facts.loads[statement].contains(&target) {
                    importers.union_with(&labels[atoms.of[statement]]);
                }
            }
            importers.remove(statics + index);
        }
        let mut changed = false;
        for atom in 0..count {
            if atoms.of[atom] != atom {
                continue;
            }
            for (index, importers) in importers.iter().enumerate() {
                let bit = statics + index;
                if labels[atom].contains(bit)
                    && !importers.is_empty()
                    && labels[atom].is_superset(importers)
                {
                    labels[atom].remove(bit);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }
    let mut per_statement = Vec::with_capacity(count);
    for statement in 0..count {
        per_statement.push(labels[atoms.of[statement]].clone());
    }
    Ok(per_statement)
}

// ---------------------------------------------------------------- links

/// What placement decides a file holds, before links: the working state
/// every plan is built and merged in.
#[derive(Clone)]
struct Layout {
    files: Vec<PlannedFile>,
    /// Per root statement: its file (`split`, `preserve-modules`).
    file_of: Vec<usize>,
}

/// Everything `link` reads besides the layout.
struct Context<'a> {
    module: &'a Module,
    facts: &'a Facts,
    graph: &'a EntryGraph,
    labels: &'a [EntrySet],
    /// `Module::imports` rows of carried host modules printed as host text
    /// (not lowered into the tree): the host file declares them.
    hosted: &'a [bool],
    /// Per binding: its setter, once placement created it.
    setters: &'a [Option<BindingId>],
    /// Per static entry: each module's position in its own order.
    positions: Vec<Vec<usize>>,
    /// Each lazily loaded module's namespace: (name, binding) per member.
    namespaces: Vec<(u32, Vec<(String, BindingId)>)>,
    preserve: bool,
}

impl Context<'_> {
    /// A file's first point in the canonical schedule.
    fn first_point(&self, file: &PlannedFile) -> usize {
        file.statements.iter().map(|&s| s as usize).min().unwrap_or(NONE)
    }

    /// Where an anchored file sits in a static entry's own order: its
    /// earliest module (host code first).
    fn entry_position(&self, entry: usize, file: &PlannedFile) -> usize {
        let mut best = NONE;
        for &statement in &file.statements {
            let statement = statement as usize;
            if !self.facts.anchored[statement] {
                continue;
            }
            let position = match self.facts.module_of(statement) {
                None => 0,
                Some(module) => self.positions[entry]
                    .get(module as usize)
                    .copied()
                    .unwrap_or(NONE)
                    .saturating_add(1),
            };
            best = best.min(position);
        }
        best
    }
}

/// Every file's imports, exports, foreign imports, loads and public names,
/// from what its statements reference (design §7.10).
fn link(context: &Context<'_>, layout: &mut Layout) -> Result<(), OutputError> {
    let module = context.module;
    let facts = context.facts;
    let host_file = layout
        .files
        .iter()
        .position(|file| file.role == FileRole::Host);
    let count = layout.files.len();
    // Per file: (source file, binding) it needs.
    let mut needs: Vec<Vec<(usize, BindingId)>> = vec![Vec::new(); count];
    let mut foreign: Vec<Vec<usize>> = vec![Vec::new(); count];
    let mut dynamic: Vec<Vec<u32>> = vec![Vec::new(); count];
    for (index, file) in layout.files.iter().enumerate() {
        for &statement in &file.statements {
            let statement = statement as usize;
            for &(binding, owner, write) in &facts.references[statement] {
                let source = layout.file_of[owner];
                if source == index {
                    continue;
                }
                if write {
                    match context.setters[binding.index()] {
                        Some(setter) => needs[index].push((source, setter)),
                        // Before setters exist, a write still needs its file.
                        None => needs[index].push((source, binding)),
                    }
                } else {
                    needs[index].push((source, binding));
                }
            }
            for &import in &facts.imports[statement] {
                match host_file.filter(|_| context.hosted[import]) {
                    Some(host) if host != index => {
                        needs[index].push((host, module.imports[import].binding))
                    }
                    Some(_) => {}
                    None => foreign[index].push(import),
                }
            }
            for &loaded in &facts.loads[statement] {
                if let Some(target) = layout
                    .files
                    .iter()
                    .position(|file| file.role == FileRole::Lazy(loaded))
                {
                    dynamic[index].push(target as u32);
                }
            }
        }
    }
    // Public names: each entry's exports, and each lazily loaded module's
    // namespace, from the file declaring each binding.
    let mut public: Vec<Vec<(String, BindingId, u32)>> = vec![Vec::new(); count];
    let declaring = |binding: BindingId, layout: &Layout| -> Option<usize> {
        let owner = facts.owners[binding.index()];
        if owner != NONE {
            return Some(layout.file_of[owner]);
        }
        let import = facts.foreign[binding.index()];
        if import != NONE && context.hosted[import] {
            return host_file;
        }
        None
    };
    for (index, file) in layout.files.iter().enumerate() {
        match file.role {
            FileRole::Entry(entry) => {
                let exports: Vec<usize> = if module.entries.is_empty() {
                    (0..module.exports.len()).collect()
                } else {
                    module.entries[entry as usize]
                        .exports
                        .iter()
                        .map(|&position| position as usize)
                        .collect()
                };
                for position in exports {
                    let export = &module.exports[position];
                    let binding = export.binding;
                    match declaring(binding, layout) {
                        Some(source) => {
                            public[index].push((export.name.clone(), binding, source as u32));
                            if source != index {
                                needs[index].push((source, binding));
                            }
                        }
                        None => {
                            // A foreign value re-exported as it is imported.
                            let import = facts.foreign[binding.index()];
                            if import == NONE {
                                return Err("an export names no delivered binding".into());
                            }
                            foreign[index].push(import);
                            public[index].push((export.name.clone(), binding, index as u32));
                        }
                    }
                }
            }
            FileRole::Lazy(loaded) => {
                let Some((_, members)) = context
                    .namespaces
                    .iter()
                    .find(|(target, _)| *target == loaded)
                else {
                    return Err("a lazily loaded file has no namespace".into());
                };
                for (name, binding) in members {
                    let binding = *binding;
                    let source = declaring(binding, layout)
                        .ok_or("a namespace member has no declaration")?;
                    if !public[index].iter().any(|(known, ..)| known == name) {
                        public[index].push((name.clone(), binding, source as u32));
                    }
                    if source != index {
                        needs[index].push((source, binding));
                    }
                }
            }
            _ => {}
        }
    }
    // Imports in evaluation order.
    let mut links: Vec<FileLinks> = Vec::with_capacity(count);
    for index in 0..count {
        let mut needed = std::mem::take(&mut needs[index]);
        needed.sort_unstable();
        needed.dedup();
        let mut sources: Vec<usize> = needed.iter().map(|&(source, _)| source).collect();
        sources.dedup();
        let file = &layout.files[index];
        let mut order: Vec<usize> = Vec::new();
        match file.role {
            FileRole::Entry(entry) if !context.preserve || file.statements.is_empty() => {
                // A facade: every anchored file carrying its bit, in its own
                // module order, then the files holding its public bindings.
                let entry = entry as usize;
                let mut anchored = (0..count)
                    .filter(|&other| {
                        other != index
                            && layout.files[other].anchored
                            && layout.files[other].label.contains(entry)
                    })
                    .map(|other| {
                        (
                            context.entry_position(entry, &layout.files[other]),
                            context.first_point(&layout.files[other]),
                            other,
                        )
                    })
                    .collect::<Vec<_>>();
                anchored.sort_unstable();
                order.extend(anchored.into_iter().map(|(.., other)| other));
                let mut rest = sources
                    .iter()
                    .copied()
                    .filter(|source| !order.contains(source))
                    .map(|source| (context.first_point(&layout.files[source]), source))
                    .collect::<Vec<_>>();
                rest.sort_unstable();
                order.extend(rest.into_iter().map(|(_, source)| source));
            }
            FileRole::Module(own) | FileRole::Lazy(own) if context.preserve => {
                order = preserve_imports(context, layout, index, own, &sources);
            }
            FileRole::Entry(entry) => {
                let own = context.graph.entries[entry as usize].1;
                order = preserve_imports(context, layout, index, own, &sources);
            }
            _ => {
                // Definitions-only files first, then anchored files by their
                // first point (Rolldown's `exec_order`).
                let mut sorted = sources
                    .iter()
                    .map(|&source| {
                        let other = &layout.files[source];
                        (other.anchored, context.first_point(other), source)
                    })
                    .collect::<Vec<_>>();
                sorted.sort_unstable();
                order.extend(sorted.into_iter().map(|(.., source)| source));
            }
        }
        let mut imports = Vec::with_capacity(order.len());
        for source in order {
            let bindings = needed
                .iter()
                .filter(|&&(from, _)| from == source)
                .map(|&(_, binding)| binding)
                .collect::<Vec<_>>();
            imports.push((source as u32, bindings));
        }
        let mut foreign_imports = std::mem::take(&mut foreign[index]);
        foreign_imports.sort_unstable();
        foreign_imports.dedup();
        let mut loads = std::mem::take(&mut dynamic[index]);
        loads.sort_unstable();
        loads.dedup();
        links.push(FileLinks {
            imports,
            exports: Vec::new(),
            public: std::mem::take(&mut public[index]),
            foreign: foreign_imports,
            hosted: Vec::new(),
            dynamic: loads,
        });
    }
    // Exports: what importers name.
    for index in 0..count {
        for (source, bindings) in links[index].imports.clone() {
            links[source as usize].exports.extend(bindings);
        }
    }
    for link in &mut links {
        link.exports.sort_unstable();
        link.exports.dedup();
    }
    if let Some(host) = host_file {
        let mut hosted = (0..module.imports.len())
            .filter(|&import| context.hosted[import])
            .collect::<Vec<_>>();
        hosted.sort_unstable();
        links[host].hosted = hosted;
    }
    for (file, link) in layout.files.iter_mut().zip(links) {
        file.links = link;
    }
    Ok(())
}

/// A `preserve-modules` file's imports: its module's source imports, in
/// source order (an empty module stands for the modules it imports), named
/// where it needs bindings and bare where the target still has effects;
/// then the imports the source lacked, which are order-neutral.
fn preserve_imports(
    context: &Context<'_>,
    layout: &Layout,
    index: usize,
    own: u32,
    sources: &[usize],
) -> Vec<usize> {
    let mut order = Vec::new();
    // Host code evaluates before the modules that import it.
    if context.graph.host_importers.contains(&own) {
        if let Some(host) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Host)
        {
            if host != index {
                order.push(host);
            }
        }
    }
    let file_of_module = |module: u32| -> Option<usize> {
        layout.files.iter().position(|file| {
            matches!(file.role, FileRole::Module(m) | FileRole::Lazy(m) if m == module)
                || matches!(file.role, FileRole::Entry(e) if context.graph.entries[e as usize].1 == module && !file.statements.is_empty())
        })
    };
    let mut pending: Vec<u32> = context.graph.imports[own as usize]
        .iter()
        .rev()
        .copied()
        .collect();
    let mut seen = vec![false; context.graph.imports.len()];
    while let Some(target) = pending.pop() {
        if std::mem::replace(&mut seen[target as usize], true) {
            continue;
        }
        match file_of_module(target) {
            Some(file) if file != index => {
                if !order.contains(&file)
                    && (sources.contains(&file) || closure_anchored(layout, file))
                {
                    order.push(file);
                }
            }
            Some(_) => {}
            None => {
                // An empty module: its own imports take its place.
                pending.extend(context.graph.imports[target as usize].iter().rev());
            }
        }
    }
    for &source in sources {
        if !order.contains(&source) {
            order.push(source);
        }
    }
    order
}

/// Whether loading a file runs anything: it or a file it imports holds an
/// anchored statement.
fn closure_anchored(layout: &Layout, file: usize) -> bool {
    let mut seen = vec![false; layout.files.len()];
    let mut pending = vec![file];
    while let Some(current) = pending.pop() {
        if std::mem::replace(&mut seen[current], true) {
            continue;
        }
        if layout.files[current].anchored {
            return true;
        }
        pending.extend(
            layout.files[current]
                .links
                .imports
                .iter()
                .map(|&(source, _)| source as usize),
        );
    }
    false
}

// ---------------------------------------------------------------- the simulator

/// The files an entry's facade loads, in ES module evaluation order: a
/// post-order walk of static imports in listed order.
fn evaluation(layout: &Layout, start: usize, loaded: &[bool]) -> Vec<usize> {
    let mut order = Vec::new();
    let mut state = vec![0u8; layout.files.len()];
    let mut stack = vec![(start, 0usize)];
    if loaded.get(start).copied().unwrap_or(false) {
        return order;
    }
    state[start] = 1;
    while let Some((file, next)) = stack.last_mut() {
        let file = *file;
        if let Some(&(source, _)) = layout.files[file].links.imports.get(*next) {
            *next += 1;
            let source = source as usize;
            if state[source] == 0 && !loaded.get(source).copied().unwrap_or(false) {
                state[source] = 1;
                stack.push((source, 0));
            }
        } else {
            stack.pop();
            state[file] = 2;
            order.push(file);
        }
    }
    order
}

/// Why a plan is refused (the verifier's assertions, design §7.11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanFailure {
    P1Placement,
    P2Write,
    P3Link,
    P4Order,
    P5Closure,
    P6Cycle,
    P7Facade,
    P8Surface,
    P9Modules,
    P10Reference,
}

impl PlanFailure {
    pub fn describe(self) -> &'static str {
        match self {
            Self::P1Placement => "delivery plan P1: a root statement is not delivered exactly once",
            Self::P2Write => "delivery plan P2: a file assigns a binding another file declares",
            Self::P3Link => "delivery plan P3: an import names nothing its file exports",
            Self::P4Order => "delivery plan P4: an entry runs its modules' effects out of source order",
            Self::P5Closure => "delivery plan P5: an entry runs effects of a module it does not reach",
            Self::P6Cycle => "delivery plan P6: the file graph has a cycle",
            Self::P7Facade => "delivery plan P7: a file imports an entry facade, or a lazy file statically",
            Self::P8Surface => "delivery plan P8: an entry's public names differ from its exports",
            Self::P9Modules => "delivery plan P9: preserve-modules moved a statement out of its module's file",
            Self::P10Reference => "delivery plan P10: a file references a binding it neither declares nor imports",
        }
    }
}

/// Each static entry's anchored statements in its own evaluation order
/// (ρ): those of the modules it reaches, by its module order, then tree
/// order; host code first.
fn reference_orders(context: &Context<'_>) -> Vec<Vec<usize>> {
    let facts = context.facts;
    let host = context.graph.host_label();
    (0..context.graph.statics())
        .map(|entry| {
            let mut statements = (0..facts.count)
                .filter(|&statement| facts.anchored[statement])
                .filter_map(|statement| {
                    let position = match facts.module_of(statement) {
                        None => host.contains(entry).then_some(0),
                        Some(module) => context.positions[entry]
                            .get(module as usize)
                            .copied()
                            .filter(|&position| position != NONE)
                            .map(|position| position + 1),
                    };
                    position.map(|position| (position, statement))
                })
                .collect::<Vec<_>>();
            statements.sort_unstable();
            statements.into_iter().map(|(_, statement)| statement).collect()
        })
        .collect()
}

/// Whether the file graph is acyclic, no file imports a facade, and every
/// static entry runs exactly its reference order (P4–P7).
fn simulate(
    context: &Context<'_>,
    layout: &Layout,
    reference: &[Vec<usize>],
) -> Result<(), PlanFailure> {
    // P6: acyclic static imports.
    let count = layout.files.len();
    let mut state = vec![0u8; count];
    for start in 0..count {
        if state[start] != 0 {
            continue;
        }
        let mut stack = vec![(start, 0usize)];
        state[start] = 1;
        while let Some((file, next)) = stack.last_mut() {
            let file = *file;
            if let Some(&(source, _)) = layout.files[file].links.imports.get(*next) {
                *next += 1;
                let source = source as usize;
                match state[source] {
                    0 => {
                        state[source] = 1;
                        stack.push((source, 0));
                    }
                    1 => return Err(PlanFailure::P6Cycle),
                    _ => {}
                }
            } else {
                stack.pop();
                state[file] = 2;
            }
        }
    }
    // P7: nothing imports a facade; a lazy file is only loaded.
    for file in &layout.files {
        for &(source, _) in &file.links.imports {
            if matches!(
                layout.files[source as usize].role,
                FileRole::Entry(_) | FileRole::Lazy(_)
            ) {
                return Err(PlanFailure::P7Facade);
            }
        }
    }
    // P4 and P5: every static entry's anchored statements.
    for (entry, expected) in reference.iter().enumerate() {
        let Some(facade) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Entry(entry as u32))
        else {
            return Err(PlanFailure::P8Surface);
        };
        let mut actual = Vec::with_capacity(expected.len());
        for file in evaluation(layout, facade, &[]) {
            actual.extend(
                layout.files[file]
                    .statements
                    .iter()
                    .map(|&statement| statement as usize)
                    .filter(|&statement| context.facts.anchored[statement]),
            );
        }
        if actual != *expected {
            let mut sorted_actual = actual.clone();
            sorted_actual.sort_unstable();
            let mut sorted_expected = expected.clone();
            sorted_expected.sort_unstable();
            return Err(if sorted_actual == sorted_expected {
                PlanFailure::P4Order
            } else {
                PlanFailure::P5Closure
            });
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- plans

fn file(role: FileRole, label: EntrySet) -> PlannedFile {
    PlannedFile {
        role,
        label,
        modules: Vec::new(),
        statements: Vec::new(),
        anchored: false,
        links: FileLinks::default(),
    }
}

/// Give each file its statements, in tree order, and derive its modules
/// and whether it is anchored.
fn settle(facts: &Facts, layout: &mut Layout) {
    for file in &mut layout.files {
        file.statements.clear();
    }
    for (statement, &index) in layout.file_of.iter().enumerate() {
        if index != NONE {
            layout.files[index].statements.push(statement as u32);
        }
    }
    for file in &mut layout.files {
        file.anchored = file
            .statements
            .iter()
            .any(|&statement| facts.anchored[statement as usize]);
        let mut modules = file
            .statements
            .iter()
            .filter(|&&statement| facts.rows[statement as usize].origin == RowOrigin::Source)
            .map(|&statement| facts.rows[statement as usize].module)
            .collect::<Vec<_>>();
        modules.sort_unstable();
        modules.dedup();
        file.modules = modules;
    }
}

/// The facades: one per static entry, then one per lazily loaded module.
fn facades(graph: &EntryGraph) -> Vec<PlannedFile> {
    let mut files = (0..graph.statics())
        .map(|entry| file(FileRole::Entry(entry as u32), EntrySet::single(entry)))
        .collect::<Vec<_>>();
    for (index, &module) in graph.dynamic.iter().enumerate() {
        files.push(file(
            FileRole::Lazy(module),
            EntrySet::single(graph.statics() + index),
        ));
    }
    files
}

/// `split`'s fallback plan, legal by construction (design §7.5): a file per
/// (label, module) group of anchored atoms; each definition in the latest
/// same-label group it references, directly or through same-label
/// definitions, or in its label's definitions-only file.
fn fallback(
    facts: &Facts,
    atoms: &Atoms,
    labels: &[EntrySet],
    graph: &EntryGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<Layout, OutputError> {
    let count = facts.count;
    let mut files = facades(graph);
    let mut file_of = vec![NONE; count];
    // Groups, by first point.
    let mut group_file: Vec<((EntrySet, u32), usize)> = Vec::new();
    for statement in 0..count {
        budget.work(WorkKind::Analysis, 1)?;
        let atom = atoms.of[statement];
        let Some(group) = atoms.group[atom].filter(|_| atoms.anchored[atom]) else {
            continue;
        };
        let key = (labels[statement].clone(), group);
        let index = match group_file.iter().find(|(known, _)| *known == key) {
            Some(&(_, index)) => index,
            None => {
                let role = if group == HOST_GROUP {
                    FileRole::Host
                } else {
                    FileRole::Shared
                };
                files.push(file(role, key.0.clone()));
                group_file.push((key, files.len() - 1));
                files.len() - 1
            }
        };
        for &member in &atoms.members[atom] {
            file_of[member] = index;
        }
    }
    // Definitions: the latest same-label group each depends on.
    let mut latest = vec![NONE; count];
    for statement in 0..count {
        let atom = atoms.of[statement];
        if atoms.anchored[atom] {
            continue;
        }
        for &(_, owner, _) in &facts.references[statement] {
            let target = atoms.of[owner];
            if atoms.anchored[target] && labels[owner] == labels[statement] {
                let group = file_of[owner];
                if latest[atom] == NONE || group > latest[atom] {
                    latest[atom] = group;
                }
            }
        }
    }
    loop {
        let mut changed = false;
        for statement in 0..count {
            let atom = atoms.of[statement];
            if atoms.anchored[atom] {
                continue;
            }
            for &(_, owner, _) in &facts.references[statement] {
                let target = atoms.of[owner];
                if !atoms.anchored[target]
                    && target != atom
                    && labels[owner] == labels[statement]
                    && latest[target] != NONE
                    && (latest[atom] == NONE || latest[target] > latest[atom])
                {
                    latest[atom] = latest[target];
                    changed = true;
                }
            }
        }
        budget.work(WorkKind::Analysis, count as u64)?;
        if !changed {
            break;
        }
    }
    let mut definitions: Vec<(EntrySet, usize)> = Vec::new();
    for statement in 0..count {
        let atom = atoms.of[statement];
        if atoms.anchored[atom] || file_of[statement] != NONE {
            continue;
        }
        let index = if latest[atom] != NONE {
            latest[atom]
        } else {
            let label = &labels[statement];
            match definitions.iter().find(|(known, _)| known == label) {
                Some(&(_, index)) => index,
                None => {
                    files.push(file(FileRole::Definitions, label.clone()));
                    definitions.push((label.clone(), files.len() - 1));
                    files.len() - 1
                }
            }
        };
        for &member in &atoms.members[atom] {
            file_of[member] = index;
        }
    }
    let mut layout = Layout { files, file_of };
    settle(facts, &mut layout);
    Ok(layout)
}

/// Move every statement of file `from` into file `into`.
fn merge(facts: &Facts, layout: &mut Layout, from: usize, into: usize) {
    for index in layout.file_of.iter_mut() {
        if *index == from {
            *index = into;
        }
    }
    settle(facts, layout);
}

/// Drop files left without statements, except facades and the lazy files
/// `import()` loads, renumbering the rest.
fn compact(layout: &mut Layout) {
    let keep = layout
        .files
        .iter()
        .map(|file| {
            !file.statements.is_empty()
                || matches!(file.role, FileRole::Entry(_) | FileRole::Lazy(_))
        })
        .collect::<Vec<_>>();
    let mut renumber = vec![NONE; layout.files.len()];
    let mut next = 0;
    for (index, &kept) in keep.iter().enumerate() {
        if kept {
            renumber[index] = next;
            next += 1;
        }
    }
    let files = std::mem::take(&mut layout.files);
    layout.files = files
        .into_iter()
        .zip(&keep)
        .filter(|(_, &kept)| kept)
        .map(|(file, _)| file)
        .collect();
    for index in layout.file_of.iter_mut() {
        if *index != NONE {
            *index = renumber[*index];
        }
    }
}

/// Try one merge: keep it when the plan still links and simulates.
fn try_merge(
    context: &Context<'_>,
    layout: &mut Layout,
    reference: &[Vec<usize>],
    from: usize,
    into: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, OutputError> {
    budget.work(
        WorkKind::Analysis,
        (layout.file_of.len() + layout.files.len() * reference.len().max(1)) as u64,
    )?;
    let mut trial = layout.clone();
    merge(context.facts, &mut trial, from, into);
    link(context, &mut trial)?;
    if simulate(context, &trial, reference).is_ok() {
        *layout = trial;
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Coalescing (design §7.6), each merge kept only when the simulator
/// accepts it: definitions-only files into the first file of their label
/// that references them; each label's files into one; files only one entry
/// loads, and nothing else imports, into its facade; the same for lazy
/// files.
fn coalesce(
    context: &Context<'_>,
    layout: &mut Layout,
    reference: &[Vec<usize>],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    // 1. Definitions-only files.
    let mut index = 0;
    while index < layout.files.len() {
        if layout.files[index].role == FileRole::Definitions
            && !layout.files[index].statements.is_empty()
        {
            let label = layout.files[index].label.clone();
            let target = (0..layout.files.len())
                .filter(|&other| {
                    other != index
                        && layout.files[other].label == label
                        && matches!(layout.files[other].role, FileRole::Shared | FileRole::Host)
                        && layout.files[other]
                            .links
                            .imports
                            .iter()
                            .any(|&(source, _)| source as usize == index)
                })
                .min_by_key(|&other| context.first_point(&layout.files[other]));
            if let Some(target) = target {
                try_merge(context, layout, reference, index, target, budget)?;
            }
        }
        index += 1;
    }
    // 2. Each label's files into the previous one of that label.
    let mut order = (0..layout.files.len())
        .filter(|&file| {
            matches!(
                layout.files[file].role,
                FileRole::Shared | FileRole::Definitions
            ) && !layout.files[file].statements.is_empty()
        })
        .map(|file| (context.first_point(&layout.files[file]), file))
        .collect::<Vec<_>>();
    order.sort_unstable();
    let mut previous: Vec<(EntrySet, usize)> = Vec::new();
    for (_, file) in order {
        if layout.files[file].statements.is_empty() {
            continue;
        }
        let label = layout.files[file].label.clone();
        match previous.iter_mut().find(|(known, _)| *known == label) {
            Some((_, last)) => {
                if try_merge(context, layout, reference, file, *last, budget)? {
                    continue;
                }
                *last = file;
            }
            None => previous.push((label, file)),
        }
    }
    // 3 and 4. Files one entry alone loads into its facade.
    for facade in 0..layout.files.len() {
        let bit = match layout.files[facade].role {
            FileRole::Entry(entry) => entry as usize,
            FileRole::Lazy(module) => match context.graph.dynamic_bit(module) {
                Some(bit) => bit,
                None => continue,
            },
            _ => continue,
        };
        let only = EntrySet::single(bit);
        let mut candidates = (0..layout.files.len())
            .filter(|&file| {
                file != facade
                    && layout.files[file].label == only
                    && !layout.files[file].statements.is_empty()
                    && matches!(
                        layout.files[file].role,
                        FileRole::Shared | FileRole::Definitions | FileRole::Host
                    )
            })
            .map(|file| (context.first_point(&layout.files[file]), file))
            .collect::<Vec<_>>();
        candidates.sort_unstable();
        for (_, file) in candidates {
            let imported_elsewhere = layout.files.iter().enumerate().any(|(other, planned)| {
                other != facade
                    && planned
                        .links
                        .imports
                        .iter()
                        .any(|&(source, _)| source as usize == file)
            });
            if !imported_elsewhere {
                try_merge(context, layout, reference, file, facade, budget)?;
            }
        }
    }
    Ok(())
}

/// `preserve-modules` (design §7.4): every source module a file; host code
/// its own file; a definition a rule created in its only user's file, the
/// latest module file it references, or a definitions-only file. An entry
/// module another module imports keeps its module file and gets a facade.
fn preserve(
    module: &Module,
    facts: &Facts,
    labels: &[EntrySet],
    graph: &EntryGraph,
) -> Result<Layout, OutputError> {
    let count = facts.count;
    let modules = graph.imports.len();
    let mut files: Vec<PlannedFile> = Vec::new();
    let mut module_file = vec![NONE; modules];
    let mut file_of = vec![NONE; count];
    let imported = {
        let mut imported = vec![false; modules];
        for imports in &graph.imports {
            for &target in imports {
                imported[target as usize] = true;
            }
        }
        imported
    };
    // Entry files first, in entry order; then module files by position.
    for (entry, (_, root)) in graph.entries.iter().enumerate() {
        if !imported[*root as usize] {
            files.push(file(
                FileRole::Entry(entry as u32),
                graph.reach[*root as usize].clone(),
            ));
            module_file[*root as usize] = files.len() - 1;
        }
    }
    for (entry, (_, root)) in graph.entries.iter().enumerate() {
        if imported[*root as usize] {
            files.push(file(FileRole::Entry(entry as u32), EntrySet::single(entry)));
        }
    }
    let mut by_position = (0..modules as u32).collect::<Vec<_>>();
    by_position.sort_unstable_by_key(|&module| graph.position[module as usize]);
    let mut host = NONE;
    for statement in 0..count {
        let row = facts.rows[statement];
        let target = match row.origin {
            RowOrigin::Source => {
                let module = row.module as usize;
                if module_file[module] == NONE {
                    let role = if graph.dynamic.contains(&row.module) {
                        FileRole::Lazy(row.module)
                    } else {
                        FileRole::Module(row.module)
                    };
                    files.push(file(role, graph.reach[module].clone()));
                    module_file[module] = files.len() - 1;
                }
                module_file[module]
            }
            RowOrigin::Host => {
                if host == NONE {
                    files.push(file(FileRole::Host, graph.host_label()));
                    host = files.len() - 1;
                }
                host
            }
            RowOrigin::Synthetic => continue,
        };
        file_of[statement] = target;
    }
    // Every entry and lazily loaded module has its file, even when empty.
    for &loaded in &graph.dynamic {
        if module_file[loaded as usize] == NONE {
            files.push(file(
                FileRole::Lazy(loaded),
                graph.reach[loaded as usize].clone(),
            ));
            module_file[loaded as usize] = files.len() - 1;
        }
    }
    // Rule-created definitions: their users' file when they have one.
    let mut users: Vec<Vec<usize>> = vec![Vec::new(); count];
    for statement in 0..count {
        for &(_, owner, _) in &facts.references[statement] {
            users[owner].push(statement);
        }
    }
    let mut definitions = NONE;
    let mut unresolved = (0..count)
        .filter(|&statement| file_of[statement] == NONE)
        .collect::<Vec<_>>();
    for _ in 0..=count {
        let mut next = Vec::new();
        for &statement in &unresolved {
            let mut targets = users[statement]
                .iter()
                .map(|&user| file_of[user])
                .collect::<Vec<_>>();
            if targets.contains(&NONE) {
                next.push(statement);
                continue;
            }
            targets.sort_unstable();
            targets.dedup();
            file_of[statement] = match targets.as_slice() {
                [only] => *only,
                _ => {
                    // The latest module file it references, else one file
                    // of such definitions.
                    let latest = facts.references[statement]
                        .iter()
                        .filter(|&&(_, owner, _)| {
                            file_of[owner] != NONE
                                && facts.rows[owner].origin == RowOrigin::Source
                        })
                        .max_by_key(|&&(_, owner, _)| {
                            graph.position[facts.rows[owner].module as usize]
                        })
                        .map(|&(_, owner, _)| file_of[owner]);
                    match latest {
                        Some(file) => file,
                        None => {
                            if definitions == NONE {
                                files.push(file(FileRole::Definitions, labels[statement].clone()));
                                definitions = files.len() - 1;
                            } else {
                                files[definitions].label.union_with(&labels[statement]);
                            }
                            definitions
                        }
                    }
                }
            };
        }
        if next.len() == unresolved.len() {
            // Definitions only other such definitions use: one file.
            for &statement in &next {
                if definitions == NONE {
                    files.push(file(FileRole::Definitions, labels[statement].clone()));
                    definitions = files.len() - 1;
                }
                file_of[statement] = definitions;
            }
            break;
        }
        unresolved = next;
        if unresolved.is_empty() {
            break;
        }
    }
    let _ = module;
    let mut layout = Layout { files, file_of };
    settle(facts, &mut layout);
    Ok(layout)
}

/// `single` with several entries (design §7.7): each entry's file holds the
/// fallback plan's files it loads, in its evaluation order. Every entry
/// gets its own instance of what it runs.
fn single(
    context: &Context<'_>,
    mut layout: Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<Layout, OutputError> {
    link(context, &mut layout)?;
    let mut files = Vec::with_capacity(context.graph.statics());
    for entry in 0..context.graph.statics() {
        budget.work(WorkKind::Analysis, layout.files.len() as u64)?;
        let facade = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Entry(entry as u32))
            .ok_or("an entry has no facade")?;
        let mut entry_file = file(FileRole::Entry(entry as u32), EntrySet::single(entry));
        for index in evaluation(&layout, facade, &[]) {
            entry_file
                .statements
                .extend(layout.files[index].statements.iter().copied());
            entry_file.anchored |= layout.files[index].anchored;
            entry_file
                .modules
                .extend(layout.files[index].modules.iter().copied());
        }
        entry_file.modules.sort_unstable();
        entry_file.modules.dedup();
        files.push(entry_file);
    }
    Ok(Layout {
        files,
        file_of: vec![NONE; layout.file_of.len()],
    })
}

/// `single`'s links: no file imports another; each prints the foreign
/// imports and host bindings its statements use and publishes its entry's
/// names.
fn link_single(context: &Context<'_>, layout: &mut Layout) -> Result<(), OutputError> {
    let module = context.module;
    let facts = context.facts;
    for file in &mut layout.files {
        let FileRole::Entry(entry) = file.role else {
            return Err("a single-file entry plan holds a file that is no entry".into());
        };
        let mut foreign = Vec::new();
        let mut hosted = Vec::new();
        for &statement in &file.statements {
            for &import in &facts.imports[statement as usize] {
                if context.hosted[import] {
                    hosted.push(import);
                } else {
                    foreign.push(import);
                }
            }
        }
        let exports: Vec<usize> = if module.entries.is_empty() {
            (0..module.exports.len()).collect()
        } else {
            module.entries[entry as usize]
                .exports
                .iter()
                .map(|&position| position as usize)
                .collect()
        };
        let mut public = Vec::with_capacity(exports.len());
        for position in exports {
            let export = &module.exports[position];
            let import = facts.foreign[export.binding.index()];
            if import != NONE {
                if context.hosted[import] {
                    hosted.push(import);
                } else {
                    foreign.push(import);
                }
            }
            public.push((export.name.clone(), export.binding, 0));
        }
        foreign.sort_unstable();
        foreign.dedup();
        hosted.sort_unstable();
        hosted.dedup();
        file.links = FileLinks {
            imports: Vec::new(),
            exports: Vec::new(),
            public,
            foreign,
            hosted,
            dynamic: Vec::new(),
        };
    }
    for (index, file) in layout.files.iter_mut().enumerate() {
        for public in &mut file.links.public {
            public.2 = index as u32;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------- setters

/// Every assignment of `binding` in `statement`'s subtree.
fn assignments(module: &Module, statement: &Statement, binding: BindingId, found: &mut Vec<ExprId>) {
    let mut statements = vec![statement];
    let mut expressions: Vec<ExprId> = Vec::new();
    loop {
        if let Some(expression) = expressions.pop() {
            let node = &module.expressions[expression.index()];
            if let Expr::Assign { target, .. } = node {
                if module.expressions[target.index()] == Expr::Binding(binding) {
                    found.push(expression);
                }
            }
            for function in node.created_functions() {
                statements.extend(
                    module.regions[module.functions[function.index()].body.index()]
                        .statements
                        .iter(),
                );
            }
            let _ = node.visit_children(|child| {
                expressions.push(child);
                Ok::<_, ()>(())
            });
            continue;
        }
        let Some(current) = statements.pop() else {
            return;
        };
        current.visit_expressions(|root| expressions.push(root));
        let mut region = |id: RegionId| statements.extend(module.regions[id.index()].statements.iter());
        match current {
            Statement::If { yes, no, .. } => {
                region(*yes);
                if let Some(no) = no {
                    region(*no);
                }
            }
            Statement::Loop { body, .. }
            | Statement::ForIn { body, .. }
            | Statement::ForOf { body, .. }
            | Statement::Block(body) => region(*body),
            Statement::Try {
                body,
                catch,
                finally,
            } => {
                region(*body);
                if let Some(catch) = catch {
                    region(catch.body);
                }
                if let Some(finally) = finally {
                    region(*finally);
                }
            }
            Statement::Function { function, .. } => {
                region(module.functions[function.index()].body)
            }
            _ => {}
        }
    }
}

/// Each write to a binding another file declares becomes a call of a setter
/// declared beside the binding (design §7.3): `x=v` is `$x(v)`, and
/// `let $x=v=>x=v` returns `v`, as the assignment did. The setter joins the
/// binding's file right after its declaration. A journaled target edit
/// before naming: the assignment's own nodes become the call.
fn create_setters(
    module: &mut Module,
    facts: &Facts,
    layout: &mut Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<(BindingId, BindingId)>, OutputError> {
    let mut writes: Vec<(BindingId, usize, usize)> = Vec::new();
    for (index, file) in layout.files.iter().enumerate() {
        for &statement in &file.statements {
            for &(binding, owner, write) in &facts.references[statement as usize] {
                if write && layout.file_of[owner] != index {
                    writes.push((binding, owner, statement as usize));
                }
            }
        }
    }
    writes.sort_unstable();
    writes.dedup();
    let mut setters: Vec<(BindingId, BindingId)> = Vec::new();
    let root_scope = module.regions[module.root.index()].scope;
    for &(binding, owner, writer) in &writes {
        budget.work(WorkKind::Edit, 1)?;
        let setter = match setters.iter().find(|&&(known, _)| known == binding) {
            Some(&(_, setter)) => setter,
            None => {
                let body = module.region_in(root_scope, budget)?;
                let body_scope = module.regions[body.index()].scope;
                let value = module.binding_in(
                    Binding {
                        source_symbol: None,
                        scope: body_scope,
                        spelling: "v".to_string(),
                        pinned: false,
                    },
                    budget,
                )?;
                let target = module.expression_in(Expr::Binding(binding), None, budget)?;
                let read = module.expression_in(Expr::Binding(value), None, budget)?;
                let assign =
                    module.expression_in(Expr::Assign { target, value: read }, None, budget)?;
                budget.push(
                    crate::output_budget::AllocationClass::Retained,
                    &mut module.regions[body.index()].statements,
                    Statement::Return(Some(assign)),
                )?;
                let function =
                    FunctionId::try_new(module.functions.len()).ok_or("too many functions")?;
                budget.push(
                    crate::output_budget::AllocationClass::Retained,
                    &mut module.functions,
                    Function {
                        parameters: vec![value],
                        body,
                        arrow: true,
                        name: FunctionName::Unobserved,
                        strict: false,
                        length: None,
                        suspension: Suspension::None,
                    },
                )?;
                let created = module.expression_in(Expr::Function(function), None, budget)?;
                let spelling = format!("set_{}", module.bindings[binding.index()].spelling);
                let setter = module.binding_in(
                    Binding {
                        source_symbol: None,
                        scope: root_scope,
                        spelling,
                        pinned: false,
                    },
                    budget,
                )?;
                let root = module.root.index();
                let statement = module.regions[root].statements.len();
                budget.push(
                    crate::output_budget::AllocationClass::Retained,
                    &mut module.regions[root].statements,
                    Statement::Let {
                        binding: setter,
                        value: Some(created),
                    },
                )?;
                budget.push(
                    crate::output_budget::AllocationClass::Retained,
                    &mut module.root_rows,
                    RootRow::synthetic(facts.rows[owner].module),
                )?;
                // Beside the declaration, in its file.
                let home = layout.file_of[owner];
                layout.file_of.push(home);
                let position = layout.files[home]
                    .statements
                    .iter()
                    .position(|&placed| placed as usize == owner)
                    .map_or(layout.files[home].statements.len(), |at| at + 1);
                layout.files[home]
                    .statements
                    .insert(position, statement as u32);
                setters.push((binding, setter));
                setter
            }
        };
        // Rewrite this writer's assignments.
        let mut found = Vec::new();
        let statement = module.regions[module.root.index()].statements[writer].clone();
        assignments(module, &statement, binding, &mut found);
        for assign in found {
            let Expr::Assign { target, value } = module.expressions[assign.index()] else {
                continue;
            };
            module.expressions[target.index()] = Expr::Binding(setter);
            module.expressions[assign.index()] = Expr::Call {
                callee: target,
                arguments: vec![value],
                invocation: crate::primitive::Invocation::Reference,
            };
        }
    }
    Ok(setters)
}

// ---------------------------------------------------------------- the plan

/// Place one formed tree's root statements in files (design §7), or `None`
/// for one entry in one file, which today's printer delivers. Setters are
/// the one edit placement makes; the plan is verified before it is kept.
pub(crate) fn plan(
    module: &mut Module,
    graph: &EntryGraph,
    contract: &DeliveryContract,
    dynamic_import: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<DeliveryPlan>, OutputError> {
    let statics = graph.statics();
    if statics == 0 || (statics == 1 && contract.mode == DeliveryMode::Single) {
        return Ok(None);
    }
    let lazy_files = contract.mode != DeliveryMode::Single && dynamic_import;
    // Without lazy files, `import()` builds its namespace in place: no
    // module is a dynamic entry.
    let trimmed;
    let graph = if lazy_files || graph.dynamic.is_empty() {
        graph
    } else {
        let mut copy = graph.clone();
        for reach in &mut copy.reach {
            for index in 0..copy.dynamic.len() {
                reach.remove(statics + index);
            }
        }
        copy.orders.truncate(statics);
        copy.dynamic.clear();
        trimmed = copy;
        &trimmed
    };
    let is_lazy = |target: u32| lazy_files && graph.dynamic.contains(&target);
    let mut facts = Facts::collect(module, &is_lazy, budget)?;
    let preserve = contract.mode == DeliveryMode::PreserveModules;
    let atoms = Atoms::build(&facts, graph, preserve);
    let labels = labels(module, &facts, &atoms, graph, budget)?;
    let hosted = module
        .imports
        .iter()
        .map(|import| {
            import
                .source
                .as_unicode()
                .is_some_and(|source| module.carried.iter().any(|carried| carried == source))
        })
        .collect::<Vec<_>>();
    let positions = graph
        .orders
        .iter()
        .take(statics)
        .map(|order| {
            let mut positions = vec![NONE; graph.imports.len()];
            for (position, &module) in order.iter().enumerate() {
                positions[module as usize] = position;
            }
            positions
        })
        .collect::<Vec<_>>();
    let namespaces = namespaces(module, graph)?;
    let no_setters = vec![None; module.bindings.len()];
    let context = Context {
        module,
        facts: &facts,
        graph,
        labels: &labels,
        hosted: &hosted,
        setters: &no_setters,
        positions: positions.clone(),
        namespaces: namespaces.clone(),
        preserve,
    };
    let reference = reference_orders(&context);
    let mut layout = match contract.mode {
        DeliveryMode::Split => {
            let mut layout = fallback(&facts, &atoms, &labels, graph, budget)?;
            link(&context, &mut layout)?;
            if let Err(failure) = simulate(&context, &layout, &reference) {
                return Err(OutputError::Invalid(failure.describe()));
            }
            coalesce(&context, &mut layout, &reference, budget)?;
            compact(&mut layout);
            link(&context, &mut layout)?;
            layout
        }
        DeliveryMode::PreserveModules => {
            let mut layout = preserve(module, &facts, &labels, graph)?;
            compact(&mut layout);
            link(&context, &mut layout)?;
            match simulate(&context, &layout, &reference) {
                Ok(()) => {}
                Err(PlanFailure::P6Cycle) => {
                    return Err(OutputError::Invalid(
                        "preserve-modules delivery of a static import cycle waits for its hoisting constraint on the tree (plan M3.3d)",
                    ))
                }
                Err(failure) => return Err(OutputError::Invalid(failure.describe())),
            }
            layout
        }
        DeliveryMode::Single => {
            let layout = fallback(&facts, &atoms, &labels, graph, budget)?;
            let mut layout = single(&context, layout, budget)?;
            link_single(&context, &mut layout)?;
            layout
        }
    };
    drop(context);
    // Setters, then the facts of the edited tree.
    let setters = if contract.mode == DeliveryMode::Single {
        Vec::new()
    } else {
        create_setters(module, &facts, &mut layout, budget)?
    };
    if !setters.is_empty() {
        facts = Facts::collect(module, &is_lazy, budget)?;
        let labels = {
            let mut extended = labels.clone();
            while extended.len() < facts.count {
                extended.push(EntrySet::default());
            }
            extended
        };
        let no_setters = vec![None; module.bindings.len()];
        let context = Context {
            module,
            facts: &facts,
            graph,
            labels: &labels,
            hosted: &hosted,
            setters: &no_setters,
            positions: positions.clone(),
            namespaces: namespaces.clone(),
            preserve,
        };
        link(&context, &mut layout)?;
    }
    // Lazily loaded files' internal exports are never named `then`: a
    // namespace with `then` is a thenable (Rolldown
    // `compute_cross_chunk_links.rs:38-46`).
    if layout
        .files
        .iter()
        .any(|file| matches!(file.role, FileRole::Lazy(_)))
        && !module.reserved.iter().any(|name| name == "then")
    {
        module.reserved.push("then".to_string());
    }
    let no_setters = vec![None; module.bindings.len()];
    let labels_final = {
        let mut extended = labels.clone();
        while extended.len() < facts.count {
            extended.push(EntrySet::default());
        }
        extended
    };
    let context = Context {
        module,
        facts: &facts,
        graph,
        labels: &labels_final,
        hosted: &hosted,
        setters: &no_setters,
        positions,
        namespaces,
        preserve,
    };
    verify(&context, &layout, &reference, contract.mode)
        .map_err(|failure| OutputError::Invalid(failure.describe()))?;
    let entries = closures(graph, &layout, contract.mode);
    Ok(Some(DeliveryPlan {
        mode: contract.mode,
        files: layout.files,
        entries,
        setters,
        statics,
    }))
}

/// Each lazily loaded module's namespace, from its first `import()`.
fn namespaces(
    module: &Module,
    graph: &EntryGraph,
) -> Result<Vec<(u32, Vec<(String, BindingId)>)>, OutputError> {
    let mut namespaces: Vec<(u32, Vec<(String, BindingId)>)> = Vec::new();
    for expression in &module.expressions {
        let Expr::LoadModule {
            module: target,
            members,
            ..
        } = expression
        else {
            continue;
        };
        if !graph.dynamic.contains(target)
            || namespaces.iter().any(|(known, _)| known == target)
        {
            continue;
        }
        let mut namespace = Vec::with_capacity(members.len());
        for (name, value) in members {
            let Expr::Binding(binding) = module.expressions[value.index()] else {
                return Err("a namespace member is not a binding".into());
            };
            namespace.push((name.clone(), binding));
        }
        namespaces.push((*target, namespace));
    }
    Ok(namespaces)
}

/// Each entry's files in evaluation order: a static entry's from its
/// facade; a lazily loaded module's beyond what every entry loading it has
/// already loaded.
fn closures(graph: &EntryGraph, layout: &Layout, mode: DeliveryMode) -> Vec<EntryDelivery> {
    let mut entries = Vec::new();
    let mut loaded_by: Vec<Vec<bool>> = Vec::new();
    for (entry, (name, _)) in graph.entries.iter().enumerate() {
        let facade = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Entry(entry as u32))
            .unwrap_or(0);
        let closure = if mode == DeliveryMode::Single {
            vec![facade]
        } else {
            evaluation(layout, facade, &[])
        };
        let mut loaded = vec![false; layout.files.len()];
        for &file in &closure {
            loaded[file] = true;
        }
        loaded_by.push(loaded);
        entries.push(EntryDelivery {
            name: name.clone(),
            file: facade as u32,
            dynamic: false,
            closure: closure.into_iter().map(|file| file as u32).collect(),
        });
    }
    for (index, file) in layout.files.iter().enumerate() {
        let FileRole::Lazy(module) = file.role else {
            continue;
        };
        // Loaded by every static entry that can load it.
        let importers = (0..graph.statics())
            .filter(|&entry| {
                entries[entry].closure.iter().any(|&file| {
                    layout.files[file as usize]
                        .links
                        .dynamic
                        .contains(&(index as u32))
                })
            })
            .collect::<Vec<_>>();
        let mut loaded = vec![!importers.is_empty(); layout.files.len()];
        for &entry in &importers {
            for (slot, &was) in loaded.iter_mut().zip(&loaded_by[entry]) {
                *slot &= was;
            }
        }
        let closure = evaluation(layout, index, &loaded);
        let name = graph
            .dynamic_bit(module)
            .map_or_else(|| format!("lazy-{module}"), |bit| format!("lazy-{bit}"));
        entries.push(EntryDelivery {
            name,
            file: index as u32,
            dynamic: true,
            closure: closure.into_iter().map(|file| file as u32).collect(),
        });
    }
    entries
}

/// The plan verifier (design §7.11): a failure is an internal error that
/// names its assertion, never a size choice.
fn verify(
    context: &Context<'_>,
    layout: &Layout,
    reference: &[Vec<usize>],
    mode: DeliveryMode,
) -> Result<(), PlanFailure> {
    let facts = context.facts;
    let module = context.module;
    // P1: each statement once (split, preserve-modules), or once in each
    // single file whose entry runs it.
    let mut seen = vec![0usize; facts.count];
    for file in &layout.files {
        let mut own = vec![false; facts.count];
        for &statement in &file.statements {
            let statement = statement as usize;
            if statement >= facts.count || std::mem::replace(&mut own[statement], true) {
                return Err(PlanFailure::P1Placement);
            }
            seen[statement] += 1;
        }
    }
    if mode != DeliveryMode::Single && seen.iter().any(|&times| times != 1) {
        return Err(PlanFailure::P1Placement);
    }
    if mode == DeliveryMode::Single && seen.iter().any(|&times| times == 0) {
        return Err(PlanFailure::P1Placement);
    }
    let file_of = |statement: usize| -> Option<usize> {
        layout
            .files
            .iter()
            .position(|file| file.statements.contains(&(statement as u32)))
    };
    if mode != DeliveryMode::Single {
        let placed = layout
            .file_of
            .iter()
            .enumerate()
            .all(|(statement, &file)| file_of(statement) == Some(file));
        if !placed {
            return Err(PlanFailure::P1Placement);
        }
        // P2 and P10: every reference is to the file's own binding or an
        // imported one; no file assigns another file's binding.
        for (index, file) in layout.files.iter().enumerate() {
            for &statement in &file.statements {
                for &(binding, owner, write) in &facts.references[statement as usize] {
                    let source = layout.file_of[owner];
                    if source == index {
                        continue;
                    }
                    if write {
                        return Err(PlanFailure::P2Write);
                    }
                    let imported = file.links.imports.iter().any(|(from, bindings)| {
                        *from as usize == source && bindings.contains(&binding)
                    });
                    if !imported {
                        return Err(PlanFailure::P10Reference);
                    }
                }
            }
            // P3: every imported binding is exported by its file.
            for (source, bindings) in &file.links.imports {
                let exports = &layout.files[*source as usize].links.exports;
                if bindings.iter().any(|binding| !exports.contains(binding)) {
                    return Err(PlanFailure::P3Link);
                }
            }
        }
        simulate(context, layout, reference)?;
    }
    // P8: each entry publishes exactly its names.
    for (index, file) in layout.files.iter().enumerate() {
        let FileRole::Entry(entry) = file.role else {
            continue;
        };
        let expected: Vec<(&str, BindingId)> = if module.entries.is_empty() {
            module
                .exports
                .iter()
                .map(|export| (export.name.as_str(), export.binding))
                .collect()
        } else {
            module.entries[entry as usize]
                .exports
                .iter()
                .map(|&position| {
                    let export = &module.exports[position as usize];
                    (export.name.as_str(), export.binding)
                })
                .collect()
        };
        let actual = file
            .links
            .public
            .iter()
            .map(|(name, binding, _)| (name.as_str(), *binding))
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(PlanFailure::P8Surface);
        }
        let _ = index;
    }
    // P9: preserve-modules keeps every source statement in its module's
    // file, and every entry module is a file.
    if mode == DeliveryMode::PreserveModules {
        for (statement, &file) in layout.file_of.iter().enumerate() {
            let row = facts.rows[statement];
            if row.origin != RowOrigin::Source {
                continue;
            }
            let home = match layout.files[file].role {
                FileRole::Module(owner) | FileRole::Lazy(owner) => owner == row.module,
                FileRole::Entry(entry) => context.graph.entries[entry as usize].1 == row.module,
                _ => false,
            };
            if !home {
                return Err(PlanFailure::P9Modules);
            }
        }
    }
    Ok(())
}
