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
use crate::output_budget::AllocationClass::{Retained, Scratch};
use crate::output_budget::{vector_bytes, AllocationBudget};

const NONE: usize = usize::MAX;

/// A set of entries: bit `i` is static entry `i` for `i` below the static
/// count, then the lazily loaded modules in order. Trailing zero words are
/// trimmed, so equal sets compare equal.
#[derive(Clone, Default, PartialEq, Eq, Hash)]
pub struct EntrySet(Vec<u64>);

impl EntrySet {
    pub(super) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        self.copy_in(Retained, budget)
    }

    fn copy_in(
        &self,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        Ok(Self(budget.copy_slice(class, &self.0)?))
    }
    fn single_in(
        bit: usize,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut value = Self::default();
        value.insert_admitted(bit, class, budget)?;
        Ok(value)
    }
    fn union_in(
        &mut self,
        other: &Self,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(WorkKind::Analysis, other.0.len() as u64)?;
        if self.0.len() < other.0.len() {
            let additional = other.0.len() - self.0.len();
            budget.reserve_vec(class, &mut self.0, additional)?;
            self.0.resize(other.0.len(), 0);
        }
        Ok(self.union_with(other))
    }
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
    pub(crate) fn insert_admitted(
        &mut self,
        bit: usize,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        budget.work(WorkKind::Analysis, 1)?;
        let word = bit / 64;
        if self.0.len() <= word {
            let additional = word + 1 - self.0.len();
            budget.reserve_vec(class, &mut self.0, additional)?;
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (bit % 64);
        Ok(())
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
    /// Every statically known `import()` target, by first request in the
    /// canonical schedule: bit `entries.len() + i`. Already initialized
    /// targets still need their shared, live namespace.
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
    /// External module requests are independent, effectful leaves in the graph.
    pub foreign: Vec<(String, u32)>,
    /// Per module: its source path relative to the common source
    /// directory, without extension (`[path]` in file name templates).
    pub paths: Vec<String>,
}

impl EntryGraph {
    fn statics(&self) -> usize {
        self.entries.len()
    }
    /// The bit of a lazily loaded module, if it is one.
    fn dynamic_bit(&self, module: u32) -> Option<usize> {
        self.dynamic
            .iter()
            .position(|&loaded| loaded == module)
            .map(|index| self.entries.len() + index)
    }
    /// The label host code carries.
    fn host_label(
        &self,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<EntrySet, AllocationError> {
        let mut label = EntrySet::default();
        for &module in &self.host_importers {
            if let Some(reach) = self.reach.get(module as usize) {
                label.union_in(reach, class, budget)?;
            }
        }
        Ok(label)
    }
}

/// A file's role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
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
    /// Debugging metadata for a JavaScript file, by delivered file index.
    SourceMap(u32),
}

impl FileRole {
    /// The role's name in the manifest and the build report.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Entry(_) => "entry",
            Self::Lazy(_) => "lazy",
            Self::Shared => "shared",
            Self::Module(_) => "module",
            Self::Definitions => "definitions",
            Self::Host => "host",
            Self::SourceMap(_) => "source-map",
        }
    }
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
    /// One opaque host module, indexed in `Module::carried`. Its file exports
    /// an internal default namespace and the immutable foreign bindings.
    pub host: Option<usize>,
    /// Files this file loads with `import()`.
    pub dynamic: Vec<u32>,
}

/// A source module instantiated within a single delivered file. Eager modules
/// retain native root cells; lazy modules keep their cells in suspended lexical
/// activations. Dependencies are source-module ids in authored order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InlineModule {
    pub source: u32,
    /// An opaque carried body evaluated at this source graph position.
    pub host: Option<usize>,
    pub publish: bool,
    pub eager: bool,
    pub dependencies: Vec<u32>,
    pub members: Vec<(String, BindingId)>,
}
impl InlineModule {
    fn heap_bytes(&self) -> Result<u64, AllocationError> {
        Ok(vector_bytes(&self.dependencies)? + vector_bytes(&self.members)?
            + self.members.iter().map(|(name, _)| name.capacity() as u64).sum::<u64>())
    }
    fn clone_in(&self, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        Ok(Self { source: self.source, host: self.host, publish: self.publish, eager: self.eager,
            dependencies: budget.copy_slice(Retained, &self.dependencies)?,
            members: super::cloning::map(&self.members, budget, |(name, binding), budget|
                Ok((budget.string(Retained, name)?, *binding)))? })
    }
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
    pub initializers: Vec<InlineModule>,
    /// Holds an anchored statement: loading it has effects.
    pub anchored: bool,
    pub links: FileLinks,
}

/// One entry of a delivery: its facade and every file it loads, in
/// evaluation order (the rows the objective scores, design §10).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    /// Static entry names, bit order.
    pub entry_names: Vec<String>,
    /// How every file is named: its template and fields.
    pub naming: Vec<FileName>,
    pub preload: crate::config::PreloadPolicy,
    pub format: crate::config::JavaScriptFormat,
    pub container: crate::compilation_policy::ContainerContract,
    pub request_bytes: u64,
    pub depth_bytes: u64,
}

/// One file's name before a content hash is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileName {
    pub template: String,
    pub name: String,
    pub index: usize,
    pub path: String,
    pub ext: String,
}

impl DeliveryPlan {
    /// The file a lazily loaded module is delivered in, for `import()`.
    pub fn lazy_file(&self, module: u32) -> Option<usize> {
        self.files
            .iter()
            .position(|file| file.role == FileRole::Lazy(module))
    }

    /// Whether some file's name is its content hash.
    pub fn needs_hash(&self) -> bool {
        self.naming
            .iter()
            .any(|name| super::names::needs_hash(&name.template))
    }

    /// Every file's name; `hashes` gives each file's hex digest when a
    /// template needs one.
    pub fn file_names(&self, hashes: Option<&[String]>) -> Vec<String> {
        self.file_names_in(hashes, Retained, &mut AllocationBudget::new(None))
            .expect("file names")
    }
    pub(crate) fn file_names_in(
        &self,
        hashes: Option<&[String]>,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<String>, AllocationError> {
        let mut names = budget.vector(class, self.naming.len())?;
        for (index, name) in self.naming.iter().enumerate() {
            names.push(super::names::expand_in(
                &name.template,
                &super::names::Fields {
                    name: &name.name,
                    index: name.index,
                    path: &name.path,
                    ext: &name.ext,
                    hash: hashes.map_or("", |hashes| hashes[index].as_str()),
                },
                class,
                budget,
            )?);
        }
        Ok(names)
    }

    /// Each file a static entry's facade preloads (`preload`): the lazily
    /// loaded files its closure loads, or every one.
    pub fn preloads(&self, file: usize) -> Vec<usize> {
        self.preloads_in(file, Retained, &mut AllocationBudget::new(None))
            .expect("preload list")
    }
    pub(crate) fn preloads_in(
        &self,
        file: usize,
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<usize>, AllocationError> {
        let FileRole::Entry(entry) = self.files[file].role else {
            return Ok(Vec::new());
        };
        let mut loaded = Vec::new();
        match self.preload {
            crate::config::PreloadPolicy::None => {}
            crate::config::PreloadPolicy::All => {
                for (index, file) in self.files.iter().enumerate() {
                    budget.work(WorkKind::Render, 1)?;
                    if matches!(file.role, FileRole::Lazy(_)) {
                        budget.push(class, &mut loaded, index)?;
                    }
                }
            }
            crate::config::PreloadPolicy::Entry => {
                if let Some(delivery) = self.entries.get(entry as usize) {
                    for &member in &delivery.closure {
                        for &target in &self.files[member as usize].links.dynamic {
                            budget.work(WorkKind::Render, loaded.len() as u64 + 1)?;
                            if !loaded.contains(&(target as usize)) {
                                budget.push(class, &mut loaded, target as usize)?;
                            }
                        }
                    }
                }
            }
        }
        Ok(loaded)
    }

    /// What an artifact keeps of this plan once its tree is gone.
    pub fn layout(&self) -> DeliveredLayout {
        self.layout_in(&mut AllocationBudget::new(None))
            .expect("delivered layout")
    }
    pub(crate) fn layout_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<DeliveredLayout, AllocationError> {
        budget.retained_phase(|budget| {
            // Package sideEffects metadata is a loading contract, including
            // transitive imports and generated preludes. A pure facade over
            // an effectful dependency must not authorize dropping that load.
            let mut effects = budget.vector(Scratch, self.files.len())?;
            let mut users: Vec<Vec<usize>> = defaults_in(self.files.len(), Scratch, budget)?;
            let mut pending = budget.vector(Scratch, self.files.len())?;
            for (index, file) in self.files.iter().enumerate() {
                budget.work(WorkKind::Analysis, 1)?;
                let preload = self.preload != crate::config::PreloadPolicy::None
                    && matches!(file.role, FileRole::Entry(_))
                    && self.files.iter().any(|file| matches!(file.role, FileRole::Lazy(_)));
                let observable = file.anchored || !file.links.foreign.is_empty()
                    || !file.links.hosted.is_empty() || file.links.host.is_some() || preload
                    || self.format != crate::config::JavaScriptFormat::Esm;
                effects.push(observable);
                if observable { pending.push(index); }
                for &(dependency, _) in &file.links.imports {
                    budget.push(Scratch, &mut users[dependency as usize], index)?;
                }
            }
            let mut cursor = 0;
            while cursor < pending.len() {
                let source = pending[cursor]; cursor += 1;
                for &user in &users[source] {
                    budget.work(WorkKind::Analysis, 1)?;
                    if !std::mem::replace(&mut effects[user], true) { pending.push(user); }
                }
            }
            let entries = super::cloning::map(&self.entries, budget, |entry, budget| {
                Ok(EntryDelivery {
                    name: budget.string(Retained, &entry.name)?,
                    file: entry.file,
                    dynamic: entry.dynamic,
                    closure: budget.copy_slice(Retained, &entry.closure)?,
                })
            })?;
            let entry_names = super::cloning::map(&self.entry_names, budget, |name, budget| {
                budget.string(Retained, name)
            })?;
            let mut file_index = 0;
            let files = super::cloning::map(&self.files, budget, |file, budget| {
                let anchored = effects[file_index]; file_index += 1;
                Ok(LayoutFile {
                    role: file.role,
                    label: collect_in(file.label.bits(), Retained, budget)?,
                    modules: budget.copy_slice(Retained, &file.modules)?,
                    anchored,
                    imports: collect_in(
                        file.links.imports.iter().map(|&(source, _)| source),
                        Retained,
                        budget,
                    )?,
                    dynamic: budget.copy_slice(Retained, &file.links.dynamic)?,
                })
            })?;
            Ok(DeliveredLayout {
                mode: self.mode,
                format: self.format,
                entries,
                entry_names,
                files,
                request_bytes: self.request_bytes,
                depth_bytes: self.depth_bytes,
                setters: self.setters.len(),
            })
        })
    }
}

/// What an artifact keeps of its delivery plan: every file's role, label,
/// source modules and links (as file positions), and every entry's closure
/// (design §10: the rows the objective scores, and manifest v3).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct DeliveredLayout {
    pub mode: DeliveryMode,
    pub format: crate::config::JavaScriptFormat,
    pub entries: Vec<EntryDelivery>,
    /// Static entry names, bit order: a label's bits name these, then the
    /// lazily loaded entries of `entries`.
    pub entry_names: Vec<String>,
    pub files: Vec<LayoutFile>,
    pub request_bytes: u64,
    pub depth_bytes: u64,
    /// Setters placement created for writes across files (design §7.3).
    pub setters: usize,
}

/// One delivered file of a layout.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LayoutFile {
    pub role: FileRole,
    /// Entry bits: static entries in name order, then lazily loaded ones.
    pub label: Vec<usize>,
    pub modules: Vec<u32>,
    /// Loading it runs code (the manifest's `side_effects`).
    pub anchored: bool,
    /// Static imports in evaluation order, and files loaded by `import()`.
    pub imports: Vec<u32>,
    pub dynamic: Vec<u32>,
}

impl DeliveredLayout {
    /// Each entry's row (design §10): the bytes of the files it loads, plus
    /// the declared cost of every file beyond the first and of every static
    /// import level beyond the first. `sizes` holds one size per file.
    pub fn rows(&self, sizes: &[usize]) -> Vec<u64> {
        self.rows_in(sizes, Retained, &mut AllocationBudget::new(None))
            .expect("delivered rows")
    }
    pub(crate) fn rows_in(
        &self,
        sizes: &[usize],
        class: AllocationClass,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<u64>, AllocationError> {
        let mut rows = budget.vector(class, self.entries.len())?;
        for entry in &self.entries {
            budget.work(WorkKind::Render, entry.closure.len() as u64)?;
            let bytes = entry.closure.iter().try_fold(0u64, |total, &file| {
                total
                    .checked_add(sizes.get(file as usize).copied().unwrap_or(0) as u64)
                    .ok_or(AllocationError::Capacity)
            })?;
            let requests = (entry.closure.iter().filter(|&&file| !matches!(self.files[file as usize].role, FileRole::SourceMap(_))).count() as u64).saturating_sub(1);
            let depth = if self.depth_bytes == 0 {
                0
            } else {
                self.depth_in(entry.file as usize, budget)?
            };
            rows.push(
                bytes
                    .saturating_add(self.request_bytes.saturating_mul(requests))
                    .saturating_add(self.depth_bytes.saturating_mul(depth.saturating_sub(1))),
            );
        }
        Ok(rows)
    }

    /// The same depth-first cycle convention as the old recursive walk, with
    /// admitted explicit frames rather than an unbounded native stack.
    fn depth_in(
        &self,
        file: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<u64, AllocationError> {
        if file >= self.files.len() {
            return Ok(0);
        }
        let mut phase = budget.scope();
        let mut memo = phase.filled(Scratch, self.files.len(), None::<u64>)?;
        let mut stack = phase.copy_slice(Scratch, &[(file, 0usize, 0u64)])?;
        memo[file] = Some(0);
        while let Some(&(file, next, depth)) = stack.last() {
            phase.work(WorkKind::Render, 1)?;
            if let Some(&source) = self.files[file].imports.get(next) {
                let source = source as usize;
                stack.last_mut().unwrap().1 += 1;
                match memo.get(source).ok_or(AllocationError::Capacity)? {
                    Some(depth) => {
                        let parent = stack.last_mut().unwrap();
                        parent.2 = depth.saturating_add(1).max(parent.2);
                    }
                    None => {
                        memo[source] = Some(0);
                        phase.push(Scratch, &mut stack, (source, 0, 0))?;
                    }
                }
            } else {
                stack.pop();
                memo[file] = Some(depth);
                match stack.last_mut() {
                    Some(parent) => parent.2 = parent.2.max(depth.saturating_add(1)),
                    None => return Ok(depth),
                }
            }
        }
        Ok(0)
    }

    pub(crate) fn heap_bytes(&self) -> Result<u64, AllocationError> {
        let mut bytes = 0u64;
        let mut add = |n: u64| -> Result<(), AllocationError> {
            bytes = bytes.checked_add(n).ok_or(AllocationError::Capacity)?;
            Ok(())
        };
        add(vector_bytes(&self.entries)?)?;
        add(vector_bytes(&self.entry_names)?)?;
        add(vector_bytes(&self.files)?)?;
        for entry in &self.entries {
            add(entry.name.capacity() as u64)?;
            add(vector_bytes(&entry.closure)?)?;
        }
        for name in &self.entry_names {
            add(name.capacity() as u64)?;
        }
        for file in &self.files {
            add(vector_bytes(&file.label)?)?;
            add(vector_bytes(&file.modules)?)?;
            add(vector_bytes(&file.imports)?)?;
            add(vector_bytes(&file.dynamic)?)?;
        }
        Ok(bytes)
    }

    /// A label as entry names.
    pub fn label_names(&self, label: &[usize]) -> Vec<String> {
        let statics = self.entry_names.len();
        label
            .iter()
            .map(|&bit| {
                self.entry_names.get(bit).cloned().unwrap_or_else(|| {
                    self.entries
                        .iter()
                        .filter(|entry| entry.dynamic)
                        .nth(bit - statics)
                        .map_or_else(|| format!("lazy-{bit}"), |entry| entry.name.clone())
                })
            })
            .collect()
    }
}

fn defaults_in<T: Default>(
    length: usize,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<T>, AllocationError> {
    budget.work(WorkKind::Analysis, length as u64)?;
    let mut values = budget.vector(class, length)?;
    values.resize_with(length, T::default);
    Ok(values)
}

fn collect_in<T>(
    values: impl IntoIterator<Item = T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<T>, AllocationError> {
    let values = values.into_iter();
    let (minimum, maximum) = values.size_hint();
    let capacity = if maximum == Some(minimum) { minimum } else { 0 };
    let mut output = budget.vector(class, capacity)?;
    for value in values {
        budget.push(class, &mut output, value)?;
    }
    Ok(output)
}

fn extend_in<T>(
    target: &mut Vec<T>,
    values: impl IntoIterator<Item = T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    for value in values {
        budget.push(class, target, value)?;
    }
    Ok(())
}

fn release_vec<T>(
    value: Vec<T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes = vector_bytes(&value)?;
    drop(value);
    budget.release(class, bytes)
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
    /// Per statement: every module its `import()`s load, lazily delivered
    /// or built in place.
    every_load: Vec<Vec<u32>>,
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

/// A single statement's references. The walk and its result have one
/// child owner, so a failed traversal cannot leave uncharged partial vectors.
#[derive(Default)]
struct StatementReferences {
    own: Vec<(BindingId, usize, bool)>,
    foreign: Vec<usize>,
    loads: Vec<u32>,
    every: Vec<u32>,
    writes_root: bool,
    loads_here: bool,
}

impl StatementReferences {
    fn add(
        &mut self,
        reference: Reference,
        owners: &[usize],
        foreign: &[usize],
        index: usize,
        lazy: &dyn Fn(u32) -> bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        match reference {
            Reference::Read(binding) => {
                let owner = owners[binding.index()];
                if owner != NONE && owner != index {
                    budget.push(Retained, &mut self.own, (binding, owner, false))?;
                }
                if foreign[binding.index()] != NONE {
                    budget.push(Retained, &mut self.foreign, foreign[binding.index()])?;
                }
            }
            Reference::Write(binding, nested) => {
                let owner = owners[binding.index()];
                if owner != NONE && owner != index {
                    budget.push(Retained, &mut self.own, (binding, owner, true))?;
                    self.writes_root |= !nested;
                }
            }
            Reference::Load(target, nested) => {
                self.loads_here |= !nested;
                if lazy(target) {
                    budget.push(Retained, &mut self.loads, target)?;
                }
                budget.push(Retained, &mut self.every, target)?;
            }
        }
        Ok(())
    }
}

fn statement_references(
    module: &Module,
    statement: &Statement,
    owners: &[usize],
    foreign: &[usize],
    index: usize,
    lazy: &dyn Fn(u32) -> bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<StatementReferences, OutputError> {
    let mut phase = budget.scope();
    let result = {
        let budget = &mut phase;
        let mut result = StatementReferences::default();
        let mut statements = budget.copy_slice(Scratch, &[(statement, false)])?;
        let mut expressions: Vec<(ExprId, bool)> = Vec::new();
        loop {
            if let Some((expression, nested)) = expressions.pop() {
                budget.work(WorkKind::Analysis, 1)?;
                let node = &module.expressions[expression.index()];
                let mut add =
                    |reference| result.add(reference, owners, foreign, index, lazy, budget);
                match node {
                    Expr::Binding(binding) => add(Reference::Read(*binding))?,
                    Expr::Assign { target, .. } => {
                        if let Expr::Binding(binding) = module.expressions[target.index()] {
                            add(Reference::Write(binding, nested))?;
                        }
                    }
                    Expr::LoadModule {
                        module: loaded,
                        promise,
                        string,
                        ..
                    } => {
                        add(Reference::Load(*loaded, nested))?;
                        if lazy(*loaded) {
                            budget.push(Scratch, &mut expressions, (*promise, nested))?;
                            budget.push(Scratch, &mut expressions, (*string, nested))?;
                            continue;
                        }
                    }
                    _ => {}
                }
                for function in node.created_functions() {
                    extend_in(
                        &mut statements,
                        module.regions[module.functions[function.index()].body.index()]
                            .statements
                            .iter()
                            .map(|s| (s, true)),
                        Scratch,
                        budget,
                    )?;
                }
                node.visit_children(|child| {
                    budget.push(Scratch, &mut expressions, (child, nested))
                })?;
                continue;
            }
            let Some((current, nested)) = statements.pop() else {
                break;
            };
            budget.work(WorkKind::Analysis, 1)?;
            let mut status = Ok(());
            current.visit_expressions(|root| {
                if status.is_ok() {
                    status = budget.push(Scratch, &mut expressions, (root, nested));
                }
            });
            status?;
            let mut region = |id: RegionId, nested: bool| {
                extend_in(
                    &mut statements,
                    module.regions[id.index()]
                        .statements
                        .iter()
                        .map(|s| (s, nested)),
                    Scratch,
                    budget,
                )
            };
            match current {
                Statement::If { yes, no, .. } => {
                    region(*yes, nested)?;
                    if let Some(no) = no {
                        region(*no, nested)?;
                    }
                }
                Statement::Loop { body, .. }
                | Statement::ForIn { body, .. }
                | Statement::ForOf { body, .. }
                | Statement::Block(body) => region(*body, nested)?,
                Statement::Try {
                    body,
                    catch,
                    finally,
                } => {
                    region(*body, nested)?;
                    if let Some(catch) = catch {
                        region(catch.body, nested)?;
                    }
                    if let Some(finally) = finally {
                        region(*finally, nested)?;
                    }
                }
                Statement::Function { function, .. } => {
                    region(module.functions[function.index()].body, true)?
                }
                _ => {}
            }
        }
        result
    };
    phase.finish_retained()?;
    Ok(result)
}

impl Facts {
    fn collect(
        module: &Module,
        lazy: &dyn Fn(u32) -> bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, OutputError> {
        let mut phase = budget.scope();
        let budget = &mut phase;
        let root = &module.regions[module.root.index()].statements;
        if module.root_rows.len() != root.len() {
            return Err("root statements and their rows disagree".into());
        }
        let mut owners = budget.filled(Retained, module.bindings.len(), NONE)?;
        for (index, statement) in root.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if let Statement::Let { binding, .. } | Statement::Function { binding, .. } = statement
            {
                owners[binding.index()] = index;
            }
        }
        let mut foreign = budget.filled(Retained, module.bindings.len(), NONE)?;
        for (index, import) in module.imports.iter().enumerate() {
            foreign[import.binding.index()] = index;
        }
        let mut references = budget.vector(Retained, root.len())?;
        let mut imports = budget.vector(Retained, root.len())?;
        let mut loads = budget.vector(Retained, root.len())?;
        let mut every_load = budget.vector(Retained, root.len())?;
        let mut anchored = budget.vector(Retained, root.len())?;
        for (index, statement) in root.iter().enumerate() {
            let StatementReferences {
                mut own,
                foreign: mut foreign_used,
                loads: mut loaded,
                mut every,
                writes_root,
                loads_here,
            } = statement_references(module, statement, &owners, &foreign, index, lazy, budget)?;
            own.sort_unstable();
            own.dedup();
            foreign_used.sort_unstable();
            foreign_used.dedup();
            loaded.sort_unstable();
            loaded.dedup();
            every.sort_unstable();
            every.dedup();
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
                "root statement {index} is a definition row that does not only define: {statement:?}; writes_root={writes_root}, loads_here={loads_here}"
            );
            references.push(own);
            imports.push(foreign_used);
            loads.push(loaded);
            every_load.push(every);
            anchored.push(!definition);
        }
        let result = Self {
            count: root.len(),
            owners,
            foreign,
            references,
            imports,
            loads,
            every_load,
            anchored,
            rows: budget.copy_slice(Retained, &module.root_rows)?,
        };
        phase.finish_retained()?;
        Ok(result)
    }

    /// Lowered host modules have their own nodes in the same evaluation graph.
    fn module_of(&self, statement: usize) -> Option<u32> {
        Some(self.rows[statement].module)
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
    fn build(
        facts: &Facts,
        graph: &EntryGraph,
        preserve: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, OutputError> {
        let mut phase = budget.scope();
        let budget = &mut phase;
        let count = facts.count;
        let mut parents = collect_in(0..count, Scratch, budget)?;
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
        let mut groups: Vec<Option<u32>> = collect_in((0..count).map(group_of), Scratch, budget)?;
        for statement in 0..count {
            if let Some(binding) = facts.rows[statement].completes {
                let owner = facts.owners.get(binding.index()).copied().unwrap_or(NONE);
                // A target rule can inline the entire private declaration
                // into its only public view; there is then no separate atom.
                if owner == NONE { continue; }
                if facts.rows[owner].module != facts.rows[statement].module {
                    return Err("completion row without a definition in its source module".into());
                }
                let (left, right) = (find(&mut parents, statement), find(&mut parents, owner));
                parents[left] = right;
                groups[right] = groups[left].or(groups[right]);
            }
        }
        if !preserve {
            for writer in 0..count {
                for &(_, owner, write) in &facts.references[writer] {
                    if !write {
                        continue;
                    }
                    // A writer in another module never joins: its write
                    // becomes a setter call. Joining a definition writer
                    // would carry its references to its own module's state
                    // into the declaring module's file, against the import
                    // order (a file cycle).
                    let same_module = facts.rows[writer].module == facts.rows[owner].module
                        && facts.rows[writer].origin == facts.rows[owner].origin;
                    if !same_module {
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
        let mut of = budget.filled(Retained, count, 0)?;
        let mut members = defaults_in(count, Retained, budget)?;
        let mut anchored = budget.filled(Retained, count, false)?;
        let mut group = budget.filled(Retained, count, None)?;
        for statement in 0..count {
            let atom = find(&mut parents, statement);
            of[statement] = atom;
            budget.push(Retained, &mut members[atom], statement)?;
            anchored[atom] |= facts.anchored[statement];
            group[atom] = groups[atom];
        }
        let result = Self {
            of,
            members,
            anchored,
            group,
        };
        drop(parents);
        drop(groups);
        phase.finish_retained()?;
        Ok(result)
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
    budget.retained_phase(|budget| {
        let count = facts.count;
        let mut labels: Vec<EntrySet> = defaults_in(count, Scratch, budget)?;
        let host = graph.host_label(Scratch, budget)?;
        let empty = EntrySet::default();
        let reach = |statement: usize| -> &EntrySet {
            match facts.module_of(statement) {
                None => &host,
                Some(module) => graph.reach.get(module as usize).unwrap_or(&empty),
            }
        };
        for statement in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            if facts.anchored[statement] {
                let atom = atoms.of[statement];
                let own = reach(statement);
                labels[atom].union_in(own, Scratch, budget)?;
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
                        labels[atom].insert_admitted(0, Scratch, budget)?;
                    }
                }
            }
        } else {
            for (entry, public) in module.entries.iter().enumerate() {
                for &position in &public.exports {
                    let binding = module.exports[position as usize].binding;
                    if let Some(atom) = owner_atom(binding) {
                        if !atoms.anchored[atom] {
                            labels[atom].insert_admitted(entry, Scratch, budget)?;
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
                            labels[atom].insert_admitted(bit, Scratch, budget)?;
                        }
                    }
                }
            }
        }
        // Definitions: the union of their referencers' labels (least fixpoint).
        let mut referenced = defaults_in(count, Scratch, budget)?;
        for statement in 0..count {
            let from = atoms.of[statement];
            for &(_, owner, _) in &facts.references[statement] {
                let to = atoms.of[owner];
                if to != from && !atoms.anchored[to] {
                    budget.push(Scratch, &mut referenced[from], to)?;
                }
            }
        }
        for list in &mut referenced {
            list.sort_unstable();
            list.dedup();
        }
        let mut pending = collect_in(
            (0..count).filter(|&atom| atoms.of[atom] == atom),
            Scratch,
            budget,
        )?;
        while let Some(atom) = pending.pop() {
            budget.work(WorkKind::Analysis, 1 + referenced[atom].len() as u64)?;
            if labels[atom].is_empty() {
                continue;
            }
            for &target in &referenced[atom] {
                let (source, destination) = if atom < target {
                    let (before, after) = labels.split_at_mut(target);
                    (&before[atom], &mut after[0])
                } else {
                    let (before, after) = labels.split_at_mut(atom);
                    (&after[0], &mut before[target])
                };
                if destination.union_in(source, Scratch, budget)? {
                    budget.push(Scratch, &mut pending, target)?;
                }
            }
        }
        // A definition nothing reaches (dead code the tree kept) goes where its
        // module's code goes.
        for statement in 0..count {
            let atom = atoms.of[statement];
            if labels[atom].is_empty() {
                let own = reach(statement);
                labels[atom].union_in(own, Scratch, budget)?;
            }
            if labels[atom].is_empty() && statics > 0 {
                labels[atom].insert_admitted(0, Scratch, budget)?;
            }
        }
        // Already loaded (Rolldown `dynamic_already_loaded`): content whose
        // label holds a lazily loaded module and every entry that loads it is
        // loaded before that module is.
        let mut importers: Vec<EntrySet> = defaults_in(graph.dynamic.len(), Scratch, budget)?;
        for _ in 0..4 {
            for (index, importers) in importers.iter_mut().enumerate() {
                importers.0.clear();
                let target = graph.dynamic[index];
                for statement in 0..count {
                    if facts.loads[statement].contains(&target) {
                        importers.union_in(&labels[atoms.of[statement]], Scratch, budget)?;
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
        let mut per_statement = budget.vector(Retained, count)?;
        for statement in 0..count {
            per_statement.push(labels[atoms.of[statement]].clone_in(budget)?);
        }
        Ok(per_statement)
    })
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

impl FileLinks {
    fn heap_bytes(&self) -> Result<u64, AllocationError> {
        let mut bytes = vector_bytes(&self.imports)?
            + vector_bytes(&self.exports)?
            + vector_bytes(&self.public)?
            + vector_bytes(&self.foreign)?
            + vector_bytes(&self.hosted)?
            + vector_bytes(&self.dynamic)?;
        for (_, bindings) in &self.imports {
            bytes = bytes
                .checked_add(vector_bytes(bindings)?)
                .ok_or(AllocationError::Capacity)?;
        }
        for (name, ..) in &self.public {
            bytes = bytes
                .checked_add(name.capacity() as u64)
                .ok_or(AllocationError::Capacity)?;
        }
        Ok(bytes)
    }
    pub(super) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut imports = budget.vector(Retained, self.imports.len())?;
        for (file, bindings) in &self.imports {
            imports.push((*file, budget.copy_slice(Retained, bindings)?));
        }
        let mut public = budget.vector(Retained, self.public.len())?;
        for (name, binding, file) in &self.public {
            public.push((budget.string(Retained, name)?, *binding, *file));
        }
        Ok(Self {
            imports,
            public,
            exports: budget.copy_slice(Retained, &self.exports)?,
            foreign: budget.copy_slice(Retained, &self.foreign)?,
            hosted: budget.copy_slice(Retained, &self.hosted)?,
            host: self.host,
            dynamic: budget.copy_slice(Retained, &self.dynamic)?,
        })
    }
    fn replace_in(
        &mut self,
        next: Self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let bytes = self.heap_bytes()?;
        drop(std::mem::replace(self, next));
        budget.release(Retained, bytes)
    }
}
impl PlannedFile {
    fn heap_bytes(&self) -> Result<u64, AllocationError> {
        Ok(vector_bytes(&self.label.0)?
            + vector_bytes(&self.modules)?
            + vector_bytes(&self.statements)?
            + vector_bytes(&self.initializers)?
            + self.initializers.iter().try_fold(0u64, |sum, row| Ok::<_, AllocationError>(sum + row.heap_bytes()?))?
            + self.links.heap_bytes()?)
    }
    pub(super) fn clone_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        Ok(Self {
            role: self.role,
            label: self.label.clone_in(budget)?,
            modules: budget.copy_slice(Retained, &self.modules)?,
            statements: budget.copy_slice(Retained, &self.statements)?,
            initializers: super::cloning::map(&self.initializers, budget, |row, budget| row.clone_in(budget))?,
            anchored: self.anchored,
            links: self.links.clone_in(budget)?,
        })
    }
}
impl Layout {
    fn heap_bytes(&self) -> Result<u64, AllocationError> {
        let mut bytes = vector_bytes(&self.files)? + vector_bytes(&self.file_of)?;
        for file in &self.files {
            bytes = bytes
                .checked_add(file.heap_bytes()?)
                .ok_or(AllocationError::Capacity)?;
        }
        Ok(bytes)
    }
    fn clone_in(&self, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        budget.retained_phase(|budget| {
            let mut files = budget.vector(Retained, self.files.len())?;
            for file in &self.files {
                files.push(file.clone_in(budget)?);
            }
            Ok(Self {
                files,
                file_of: budget.copy_slice(Retained, &self.file_of)?,
            })
        })
    }
}

/// Everything `link` reads besides the layout.
struct Context<'a> {
    module: &'a Module,
    facts: &'a Facts,
    graph: &'a EntryGraph,
    /// `Module::imports` rows of carried host modules printed as host text
    /// (not lowered into the tree): the host file declares them.
    hosted: &'a [bool],
    /// Per entry bit (static entries, then dynamic ones): each module's
    /// position in that entry's own order.
    positions: &'a [Vec<usize>],
    /// Each lazily loaded module's namespace: (name, binding) per member.
    namespaces: &'a [(u32, Vec<(String, BindingId)>)],
    preserve: bool,
}

impl Context<'_> {
    fn foreign_file(&self, layout: &Layout, import: usize) -> Option<usize> {
        let source = self.module.imports[import].source.as_unicode()?;
        let owner = self.graph.foreign.iter().find(|(name, _)| name == source)?.1;
        layout.files.iter().position(|file| file.role == FileRole::Module(owner))
    }
    /// A file's first point in the canonical schedule.
    fn first_point(&self, file: &PlannedFile) -> usize {
        file.statements
            .iter()
            .map(|&s| s as usize)
            .min()
            .unwrap_or(NONE)
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
fn export_positions(module: &Module, entry: usize) -> impl Iterator<Item = usize> + '_ {
    let exports = module
        .entries
        .get(entry)
        .map(|entry| entry.exports.as_slice());
    (0..exports.map_or(module.exports.len(), <[u32]>::len))
        .map(move |index| exports.map_or(index, |exports| exports[index] as usize))
}

fn link(
    context: &Context<'_>,
    layout: &mut Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    let links = budget.retained_phase(|budget| {
        let module = context.module;
        let facts = context.facts;
        let host_file = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Host);
        let count = layout.files.len();
        // Link construction cannot consult the previous links: on the first
        // pass they are empty, and a definition-only module may import effects
        // several edges away. Close source effects once over the static graph.
        let mut effectful = budget.filled(Scratch, context.graph.imports.len(), false)?;
        let mut callers: Vec<Vec<usize>> = defaults_in(effectful.len(), Scratch, budget)?;
        for (importer, imports) in context.graph.imports.iter().enumerate() {
            for &imported in imports {
                budget.push(Scratch, &mut callers[imported as usize], importer)?;
            }
        }
        for (statement, row) in facts.rows.iter().enumerate() {
            if facts.anchored[statement] { effectful[row.module as usize] = true; }
        }
        for &(_, owner) in &context.graph.foreign { effectful[owner as usize] = true; }
        let mut pending = collect_in((0..effectful.len()).filter(|&module| effectful[module]), Scratch, budget)?;
        while let Some(module) = pending.pop() {
            for &caller in &callers[module] {
                budget.work(WorkKind::Analysis, 1)?;
                if !std::mem::replace(&mut effectful[caller], true) {
                    budget.push(Scratch, &mut pending, caller)?;
                }
            }
        }
        // Per file: (source file, binding) it needs.
        let mut needs: Vec<Vec<(usize, BindingId)>> = defaults_in(count, Scratch, budget)?;
        let mut foreign: Vec<Vec<usize>> = defaults_in(count, Scratch, budget)?;
        let mut dynamic: Vec<Vec<u32>> = defaults_in(count, Scratch, budget)?;
        for (index, file) in layout.files.iter().enumerate() {
            if let FileRole::Module(owner) = file.role {
                if let Some((source, _)) = context.graph.foreign.iter().find(|(_, module)| *module == owner) {
                    for (import, row) in module.imports.iter().enumerate() {
                        if row.source.as_unicode() == Some(source.as_str()) && !context.hosted[import] { budget.push(Retained, &mut foreign[index], import)?; }
                    }
                }
            }
            for &statement in &file.statements {
                let statement = statement as usize;
                for &(binding, owner, _) in &facts.references[statement] {
                    let source = layout.file_of[owner];
                    if source == index {
                        continue;
                    }
                    budget.push(Scratch, &mut needs[index], (source, binding))?;
                }
                for &import in &facts.imports[statement] {
                    match host_file.filter(|_| context.hosted[import]).or_else(|| context.foreign_file(layout, import)) {
                        Some(host) if host != index => budget.push(
                            Scratch,
                            &mut needs[index],
                            (host, module.imports[import].binding),
                        )?,
                        Some(_) => {}
                        None => budget.push(Retained, &mut foreign[index], import)?,
                    }
                }
                for &loaded in &facts.loads[statement] {
                    if let Some(target) = dynamic_file(layout, loaded) {
                        budget.push(Retained, &mut dynamic[index], target as u32)?;
                    }
                }
            }
        }
        // Public names: each entry's exports, and each lazily loaded module's
        // namespace, from the file declaring each binding.
        let mut public: Vec<Vec<(String, BindingId, u32)>> = defaults_in(count, Scratch, budget)?;
        // Per file: (source file, binding) it re-exports with `export{…}from`.
        let mut reexports: Vec<Vec<(usize, BindingId)>> = defaults_in(count, Scratch, budget)?;
        let declaring = |binding: BindingId, layout: &Layout| -> Option<usize> {
            let owner = facts.owners[binding.index()];
            if owner != NONE {
                return Some(layout.file_of[owner]);
            }
            let import = facts.foreign[binding.index()];
            if import != NONE {
                return host_file.filter(|_| context.hosted[import]).or_else(|| context.foreign_file(layout, import));
            }
            None
        };
        for (index, file) in layout.files.iter().enumerate() {
            match file.role {
                FileRole::Entry(entry) => {
                    for position in export_positions(module, entry as usize) {
                        let export = &module.exports[position];
                        let binding = export.binding;
                        match declaring(binding, layout) {
                            Some(source) => {
                                let name = budget.string(Retained, &export.name)?;
                                budget.push(
                                    Retained,
                                    &mut public[index],
                                    (name, binding, source as u32),
                                )?;
                                if source != index {
                                    budget.push(
                                        Scratch,
                                        &mut reexports[index],
                                        (source, binding),
                                    )?;
                                }
                            }
                            None => {
                                // A foreign value re-exported as it is imported.
                                let import = facts.foreign[binding.index()];
                                if import == NONE {
                                    return Err("an export names no delivered binding".into());
                                }
                                budget.push(Retained, &mut foreign[index], import)?;
                                let name = budget.string(Retained, &export.name)?;
                                budget.push(
                                    Retained,
                                    &mut public[index],
                                    (name, binding, index as u32),
                                )?;
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
                            let name = budget.string(Retained, name)?;
                            budget.push(
                                Retained,
                                &mut public[index],
                                (name, binding, source as u32),
                            )?;
                            if source != index {
                                budget.push(Scratch, &mut reexports[index], (source, binding))?;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        // Imports in evaluation order.
        let mut links = budget.vector(Retained, count)?;
        for index in 0..count {
            let next = budget.retained_phase(|budget| {
                let needed = &mut needs[index];
                needed.sort_unstable();
                needed.dedup();
                // A file it only re-exports from is still a module it requests.
                let mut sources = collect_in(
                    needed
                        .iter()
                        .chain(&reexports[index])
                        .map(|&(source, _)| source),
                    Scratch,
                    budget,
                )?;
                sources.sort_unstable();
                sources.dedup();
                let file = &layout.files[index];
                let mut order: Vec<usize> = Vec::new();
                // A facade: an entry's file, or (design §7.5, §7.8) a lazily loaded
                // module's file, whose load evaluates what its module reaches.
                let facade = match file.role {
                    FileRole::Entry(entry) if !context.preserve || file.statements.is_empty() => {
                        Some(entry as usize)
                    }
                    FileRole::Lazy(module) if !context.preserve => {
                        context.graph.dynamic_bit(module)
                    }
                    _ => None,
                };
                match file.role {
                    FileRole::Entry(entry) if context.preserve && file.statements.is_empty() => {
                        // A facade enters the source graph through its root,
                        // not through the first module in post-order. Entering
                        // a cycle at that dependency would reverse its effects.
                        let own = context.graph.entries[entry as usize].1;
                        if let Some(source) = layout.files.iter().position(|file| file.role == FileRole::Module(own)) {
                            budget.push(Scratch, &mut order, source)?;
                        } else {
                            preserve_imports(context, layout, index, own, &sources, &effectful, &mut order, budget)?;
                        }
                        for &source in &sources {
                            if !order.contains(&source) { budget.push(Scratch, &mut order, source)?; }
                        }
                    }
                    _ if facade.is_some() => {
                        // Every anchored file carrying its bit, in its own module
                        // order, then the files holding its public bindings.
                        let entry = facade.unwrap_or_default();
                        let mut anchored = collect_in(
                            (0..count)
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
                                }),
                            Scratch,
                            budget,
                        )?;
                        anchored.sort_unstable();
                        extend_in(
                            &mut order,
                            anchored.iter().map(|&(.., other)| other),
                            Scratch,
                            budget,
                        )?;
                        let mut rest = collect_in(
                            sources
                                .iter()
                                .copied()
                                // Formation may retain unused definitions
                                // when optional DCE is off. Their entry label
                                // still owns them, even with no binding import
                                // leading to their file. Preserve that ownership
                                // before single-file flattening follows links.
                                .chain((0..count).filter(|&other| {
                                    other != index
                                        && !layout.files[other].anchored
                                        && !layout.files[other].statements.is_empty()
                                        && layout.files[other].label.contains(entry)
                                }))
                                .filter(|source| !order.contains(source))
                                .map(|source| (context.first_point(&layout.files[source]), source)),
                            Scratch,
                            budget,
                        )?;
                        rest.sort_unstable();
                        rest.dedup();
                        extend_in(
                            &mut order,
                            rest.iter().map(|&(_, source)| source),
                            Scratch,
                            budget,
                        )?;
                    }
                    FileRole::Lazy(own)
                        if context.preserve
                            && file.statements.is_empty()
                            && layout
                                .files
                                .iter()
                                .any(|other| other.role == FileRole::Module(own)) =>
                    {
                        // A facade over a module file: loading it evaluates that
                        // file, then re-exports what the namespace names.
                        let module_file = layout
                            .files
                            .iter()
                            .position(|other| other.role == FileRole::Module(own))
                            .unwrap_or(index);
                        budget.push(Scratch, &mut order, module_file)?;
                        let mut rest = collect_in(
                            sources
                                .iter()
                                .copied()
                                .filter(|&source| source != module_file)
                                .map(|source| (context.first_point(&layout.files[source]), source)),
                            Scratch,
                            budget,
                        )?;
                        rest.sort_unstable();
                        extend_in(
                            &mut order,
                            rest.iter().map(|&(_, source)| source),
                            Scratch,
                            budget,
                        )?;
                    }
                    FileRole::Module(own) | FileRole::Lazy(own) if context.preserve => {
                        preserve_imports(
                            context, layout, index, own, &sources, &effectful, &mut order, budget,
                        )?;
                    }
                    FileRole::Entry(entry) => {
                        let own = context.graph.entries[entry as usize].1;
                        preserve_imports(
                            context, layout, index, own, &sources, &effectful, &mut order, budget,
                        )?;
                    }
                    _ => {
                        // Definitions-only files first, then anchored files by their
                        // first point (Rolldown's `exec_order`).
                        let mut sorted = collect_in(
                            sources.iter().map(|&source| {
                                let other = &layout.files[source];
                                (other.anchored, context.first_point(other), source)
                            }),
                            Scratch,
                            budget,
                        )?;
                        sorted.sort_unstable();
                        extend_in(
                            &mut order,
                            sorted.iter().map(|&(.., source)| source),
                            Scratch,
                            budget,
                        )?;
                    }
                }
                let mut imports = budget.vector(Retained, order.len())?;
                for source in order {
                    let bindings = collect_in(
                        needed
                            .iter()
                            .filter(|&&(from, _)| from == source)
                            .map(|&(_, binding)| binding),
                        Retained,
                        budget,
                    )?;
                    imports.push((source as u32, bindings));
                }
                let mut foreign_imports = std::mem::take(&mut foreign[index]);
                foreign_imports.sort_unstable();
                foreign_imports.dedup();
                let mut loads = std::mem::take(&mut dynamic[index]);
                loads.sort_unstable();
                loads.dedup();
                Ok::<_, OutputError>(FileLinks {
                    imports,
                    exports: Vec::new(),
                    public: std::mem::take(&mut public[index]),
                    foreign: foreign_imports,
                    hosted: Vec::new(),
                    host: match file.role {
                        FileRole::Module(owner) => context.graph.foreign.iter().find(|(_, module)| *module == owner)
                            .and_then(|(source, _)| module.carried.iter().position(|name| name == source)),
                        _ => None,
                    },
                    dynamic: loads,
                })
            })?;
            links.push(next);
        }
        for (index, file) in layout.files.iter().enumerate() {
            if let FileRole::Module(owner) = file.role {
                if let Some((source, _)) = context.graph.foreign.iter().find(|(_, module)| *module == owner) {
                    for (import, row) in module.imports.iter().enumerate() {
                        if context.hosted[import] && row.source.as_unicode() == Some(source.as_str()) {
                            budget.push(Retained, &mut links[index].hosted, import)?;
                        }
                    }
                }
            }
        }
        // Exports: what importers name or re-export.
        for index in 0..count {
            for import in 0..links[index].imports.len() {
                let source = links[index].imports[import].0 as usize;
                for slot in 0..links[index].imports[import].1.len() {
                    let binding = links[index].imports[import].1[slot];
                    budget.push(Retained, &mut links[source].exports, binding)?;
                }
            }
            for &(source, binding) in &reexports[index] {
                budget.push(Retained, &mut links[source].exports, binding)?;
            }
        }
        for link in &mut links {
            link.exports.sort_unstable();
            link.exports.dedup();
        }
        if let Some(host) = host_file {
            let mut hosted = collect_in(
                (0..module.imports.len()).filter(|&import| context.hosted[import]),
                Retained,
                budget,
            )?;
            hosted.sort_unstable();
            links[host].hosted = hosted;
        }
        Ok::<_, OutputError>(links)
    })?;
    let outer_bytes = vector_bytes(&links)?;
    for (file, link) in layout.files.iter_mut().zip(links) {
        file.links.replace_in(link, budget)?;
    }
    budget.release(Retained, outer_bytes)?;
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
    effectful: &[bool],
    order: &mut Vec<usize>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    // Host code evaluates before the modules that import it.
    if context.graph.host_importers.contains(&own) {
        if let Some(host) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Host)
        {
            if host != index {
                budget.push(Scratch, order, host)?;
            }
        }
    }
    let file_of_module = |module: u32| -> Option<usize> {
        layout.files.iter().position(|file| {
            matches!(file.role, FileRole::Module(m) | FileRole::Lazy(m) if m == module)
                || matches!(file.role, FileRole::Entry(e) if context.graph.entries[e as usize].1 == module && !file.statements.is_empty())
        })
    };
    let mut pending = collect_in(
        context.graph.imports[own as usize].iter().rev().copied(),
        Scratch,
        budget,
    )?;
    let mut seen = budget.filled(Scratch, context.graph.imports.len(), false)?;
    while let Some(target) = pending.pop() {
        if std::mem::replace(&mut seen[target as usize], true) {
            continue;
        }
        match file_of_module(target) {
            Some(file) if file != index => {
                if !order.contains(&file)
                    && (sources.contains(&file) || effectful[target as usize])
                {
                    budget.push(Scratch, order, file)?;
                }
            }
            Some(_) => {}
            None => {
                // An empty module: its own imports take its place.
                extend_in(
                    &mut pending,
                    context.graph.imports[target as usize].iter().rev().copied(),
                    Scratch,
                    budget,
                )?;
            }
        }
    }
    for &source in sources {
        if !order.contains(&source) {
            budget.push(Scratch, order, source)?;
        }
    }
    release_vec(pending, Scratch, budget)?;
    release_vec(seen, Scratch, budget)?;
    Ok(())
}

/// The file `import()` of a dynamic entry loads: its lazy file, or in
/// `preserve-modules` its module's file when other files import that
/// module statically as well.
fn dynamic_file(layout: &Layout, module: u32) -> Option<usize> {
    layout
        .files
        .iter()
        .position(|file| file.role == FileRole::Lazy(module))
        .or_else(|| {
            layout
                .files
                .iter()
                .position(|file| file.role == FileRole::Module(module))
        })
}

// ---------------------------------------------------------------- the simulator

/// Static imports in ECMAScript evaluation order. The returned order is
/// retained; the traversal stack and visitation state die in this phase.
fn evaluation(
    layout: &Layout,
    start: usize,
    loaded: &[bool],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, AllocationError> {
    budget.retained_phase(|budget| {
        let mut order = Vec::new();
        if loaded.get(start).copied().unwrap_or(false) {
            return Ok(order);
        }
        let mut state = budget.filled(Scratch, layout.files.len(), 0u8)?;
        let mut stack = budget.copy_slice(Scratch, &[(start, 0usize)])?;
        state[start] = 1;
        while let Some((file, next)) = stack.last_mut() {
            budget.work(WorkKind::Analysis, 1)?;
            let file = *file;
            if let Some(&(source, _)) = layout.files[file].links.imports.get(*next) {
                *next += 1;
                let source = source as usize;
                if state[source] == 0 && !loaded.get(source).copied().unwrap_or(false) {
                    state[source] = 1;
                    budget.push(Scratch, &mut stack, (source, 0))?;
                }
            } else {
                stack.pop();
                state[file] = 2;
                budget.push(Retained, &mut order, file)?;
            }
        }
        Ok(order)
    })
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
            Self::P4Order => {
                "delivery plan P4: an entry runs its modules' effects out of source order"
            }
            Self::P5Closure => {
                "delivery plan P5: an entry runs effects of a module it does not reach"
            }
            Self::P6Cycle => "delivery plan P6: the file graph has a cycle",
            Self::P7Facade => {
                "delivery plan P7: a file imports an entry facade, or a lazy file statically"
            }
            Self::P8Surface => "delivery plan P8: an entry's public names differ from its exports",
            Self::P9Modules => {
                "delivery plan P9: preserve-modules moved a statement out of its module's file"
            }
            Self::P10Reference => {
                "delivery plan P10: a file references a binding it neither declares nor imports"
            }
        }
    }
}

#[derive(Debug)]
enum VerificationError {
    Plan(PlanFailure),
    Admission(AllocationError),
}
impl From<PlanFailure> for VerificationError {
    fn from(error: PlanFailure) -> Self {
        Self::Plan(error)
    }
}
impl From<AllocationError> for VerificationError {
    fn from(error: AllocationError) -> Self {
        Self::Admission(error)
    }
}
impl From<VerificationError> for OutputError {
    fn from(error: VerificationError) -> Self {
        match error {
            VerificationError::Plan(failure) => Self::Invalid(failure.describe()),
            VerificationError::Admission(error) => Self::Admission(error),
        }
    }
}

/// Each static entry's anchored statements in its own evaluation order.
fn reference_orders(
    context: &Context<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Vec<usize>>, AllocationError> {
    budget.retained_phase(|budget| {
        let facts = context.facts;
        let host = context.graph.host_label(Scratch, budget)?;
        let mut orders = budget.vector(Retained, context.graph.statics())?;
        for entry in 0..context.graph.statics() {
            orders.push(budget.retained_phase(|budget| {
                let mut statements = collect_in(
                    (0..facts.count)
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
                        }),
                    Scratch,
                    budget,
                )?;
                statements.sort_unstable();
                collect_in(
                    statements.iter().map(|&(_, statement)| statement),
                    Retained,
                    budget,
                )
            })?);
        }
        Ok(orders)
    })
}

/// Whether the file graph is acyclic, no file imports a facade, and every
/// static entry runs exactly its reference order (P4–P7).
fn simulate(
    context: &Context<'_>,
    layout: &Layout,
    reference: &[Vec<usize>],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), VerificationError> {
    let mut phase = budget.scope();
    let budget = &mut phase;
    // P6: acyclic static imports.
    let count = layout.files.len();
    let mut state = budget.filled(Scratch, count, 0u8)?;
    let mut stack = Vec::new();
    for start in 0..count {
        if state[start] != 0 {
            continue;
        }
        stack.clear();
        budget.push(Scratch, &mut stack, (start, 0usize))?;
        state[start] = 1;
        while let Some((file, next)) = stack.last_mut() {
            let file = *file;
            if let Some(&(source, _)) = layout.files[file].links.imports.get(*next) {
                *next += 1;
                let source = source as usize;
                match state[source] {
                    0 => {
                        state[source] = 1;
                        budget.push(Scratch, &mut stack, (source, 0))?;
                    }
                    1 => {
                        // A preserved source SCC is a legal native module
                        // cycle. Hoisted rows were established before facts
                        // and placement; artificial cross-component cycles
                        // remain forbidden.
                        let cycle = |file: usize| {
                            let [module] = layout.files[file].modules.as_slice() else { return None; };
                            context.graph.cycles[*module as usize]
                        };
                        if !context.preserve || cycle(file).is_none() || cycle(file) != cycle(source) {
                            return Err(PlanFailure::P6Cycle.into());
                        }
                    },
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
                return Err(PlanFailure::P7Facade.into());
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
            return Err(PlanFailure::P8Surface.into());
        };
        let mut run_phase = budget.scope();
        let budget = &mut run_phase;
        let mut actual = budget.vector(Scratch, expected.len())?;
        for file in evaluation(layout, facade, &[], budget)? {
            extend_in(
                &mut actual,
                layout.files[file]
                    .statements
                    .iter()
                    .map(|&statement| statement as usize)
                    .filter(|&statement| context.facts.anchored[statement]),
                Scratch,
                budget,
            )?;
        }
        if actual != *expected {
            return Err(order_failure(&actual, expected, budget)?.into());
        }
    }
    simulate_dynamic(context, layout, budget)
}

/// P4 when two runs hold the same statements in another order, else P5.
fn order_failure(
    actual: &[usize],
    expected: &[usize],
    budget: &mut AllocationBudget<'_>,
) -> Result<PlanFailure, AllocationError> {
    let mut phase = budget.scope();
    let mut sorted_actual = phase.copy_slice(Scratch, actual)?;
    sorted_actual.sort_unstable();
    let mut sorted_expected = phase.copy_slice(Scratch, expected)?;
    sorted_expected.sort_unstable();
    Ok(if sorted_actual == sorted_expected {
        PlanFailure::P4Order
    } else {
        PlanFailure::P5Closure
    })
}

/// P4 and P5 for lazily loaded entries (design §7.6, "given loaded(d)"):
/// from each static entry, every `import()` its files, or the files a lazy
/// load brought in, can run evaluates exactly the anchored statements of
/// the target's modules nothing loaded before it has evaluated, in the
/// target's own module order. A later load that finds some of those
/// modules evaluated skips whole files (a file's anchored statements share
/// one label), so the restriction of an accepted order stays in order.
fn simulate_dynamic(
    context: &Context<'_>,
    layout: &Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), VerificationError> {
    let mut phase = budget.scope();
    let budget = &mut phase;
    let graph = context.graph;
    if graph.dynamic.is_empty() {
        return Ok(());
    }
    let facts = context.facts;
    let statics = graph.statics();
    let host = graph.host_label(Scratch, budget)?;
    let bit_of = |file: usize| -> Option<usize> {
        match layout.files[file].role {
            FileRole::Lazy(module) | FileRole::Module(module) => graph.dynamic_bit(module),
            _ => None,
        }
    };
    for entry in 0..statics {
        let Some(facade) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Entry(entry as u32))
        else {
            return Err(PlanFailure::P8Surface.into());
        };
        let mut entry_phase = budget.scope();
        let budget = &mut entry_phase;
        let mut loaded_files = budget.filled(Scratch, layout.files.len(), false)?;
        let mut pending: Vec<(usize, EntrySet, Vec<bool>)> = Vec::new();
        for file in evaluation(layout, facade, &[], budget)? {
            loaded_files[file] = true;
        }
        let loaded_bits = EntrySet::single_in(entry, Scratch, budget)?;
        for file in 0..layout.files.len() {
            if loaded_files[file] {
                for &target in &layout.files[file].links.dynamic {
                    let bits = loaded_bits.copy_in(Scratch, budget)?;
                    let files = budget.copy_slice(Scratch, &loaded_files)?;
                    budget.push(Scratch, &mut pending, (target as usize, bits, files))?;
                }
            }
        }
        // Each lazy file once per static entry, first by the shortest chain
        // of loads that reaches it (the least that is loaded before it).
        let mut visited = budget.filled(Scratch, layout.files.len(), false)?;
        let mut cursor = 0;
        while cursor < pending.len() {
            let (target, mut bits, mut files) = std::mem::take(&mut pending[cursor]);
            cursor += 1;
            if std::mem::replace(&mut visited[target], true) {
                release_vec(bits.0, Scratch, budget)?;
                release_vec(files, Scratch, budget)?;
                continue;
            }
            let Some(bit) = bit_of(target) else {
                release_vec(bits.0, Scratch, budget)?;
                release_vec(files, Scratch, budget)?;
                continue;
            };
            let run = evaluation(layout, target, &files, budget)?;
            let actual = collect_in(
                run.iter()
                    .flat_map(|&file| layout.files[file].statements.iter())
                    .map(|&statement| statement as usize)
                    .filter(|&statement| facts.anchored[statement]),
                Scratch,
                budget,
            )?;
            let evaluated = |reach: &EntrySet| reach.bits().any(|known| bits.contains(known));
            let mut expected = collect_in(
                (0..facts.count)
                    .filter(|&statement| facts.anchored[statement])
                    .filter_map(|statement| {
                        let position = match facts.module_of(statement) {
                            None => (host.contains(bit) && !evaluated(&host)).then_some(0),
                            Some(module) => {
                                let reach = graph.reach.get(module as usize)?;
                                (reach.contains(bit) && !evaluated(reach))
                                    .then(|| context.positions[bit].get(module as usize).copied())
                                    .flatten()
                                    .filter(|&position| position != NONE)
                                    .map(|position| position + 1)
                            }
                        };
                        position.map(|position| (position, statement))
                    }),
                Scratch,
                budget,
            )?;
            expected.sort_unstable();
            let expected_order = collect_in(
                expected.iter().map(|&(_, statement)| statement),
                Scratch,
                budget,
            )?;
            if actual != expected_order {
                return Err(order_failure(&actual, &expected_order, budget)?.into());
            }
            release_vec(actual, Scratch, budget)?;
            release_vec(expected, Scratch, budget)?;
            release_vec(expected_order, Scratch, budget)?;
            for &file in &run {
                files[file] = true;
            }
            bits.insert_admitted(bit, Scratch, budget)?;
            for &file in &run {
                for &next in &layout.files[file].links.dynamic {
                    let next_bits = bits.copy_in(Scratch, budget)?;
                    let next_files = budget.copy_slice(Scratch, &files)?;
                    budget.push(
                        Scratch,
                        &mut pending,
                        (next as usize, next_bits, next_files),
                    )?;
                }
            }
            release_vec(run, Retained, budget)?;
            release_vec(bits.0, Scratch, budget)?;
            release_vec(files, Scratch, budget)?;
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
        initializers: Vec::new(),
        anchored: false,
        links: FileLinks::default(),
    }
}

/// Give each file its statements, in tree order, and derive its modules
/// and whether it is anchored.
fn settle(
    facts: &Facts,
    layout: &mut Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    for file in &mut layout.files {
        file.statements.clear();
        file.modules.clear();
        if let FileRole::Module(owner) = file.role {
            if !file.links.foreign.is_empty() || !file.links.hosted.is_empty() || file.links.host.is_some() { budget.push(Retained, &mut file.modules, owner)?; }
        }
    }
    for (statement, &index) in layout.file_of.iter().enumerate() {
        if index != NONE {
            budget.push(
                Retained,
                &mut layout.files[index].statements,
                statement as u32,
            )?;
        }
    }
    for file in &mut layout.files {
        file.anchored = !file.links.foreign.is_empty() || !file.links.hosted.is_empty() || file.links.host.is_some() || file.statements.iter().any(|&s| facts.anchored[s as usize]);
        extend_in(
            &mut file.modules,
            file.statements
                .iter()
                .filter(|&&s| facts.rows[s as usize].origin != RowOrigin::Synthetic)
                .map(|&s| facts.rows[s as usize].module),
            Retained,
            budget,
        )?;
        file.modules.sort_unstable();
        file.modules.dedup();
    }
    Ok(())
}

/// The facades: one per static entry, then one per lazily loaded module.
fn facades(
    graph: &EntryGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<PlannedFile>, AllocationError> {
    let mut files = budget.vector(Retained, graph.statics() + graph.dynamic.len())?;
    for entry in 0..graph.statics() {
        files.push(file(
            FileRole::Entry(entry as u32),
            EntrySet::single_in(entry, Retained, budget)?,
        ));
    }
    for (index, &module) in graph.dynamic.iter().enumerate() {
        files.push(file(
            FileRole::Lazy(module),
            EntrySet::single_in(graph.statics() + index, Retained, budget)?,
        ));
    }
    Ok(files)
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
    budget.retained_phase(|budget| {
        let count = facts.count;
        let mut files = facades(graph, budget)?;
        let mut file_of = budget.filled(Retained, count, NONE)?;
        // Groups in canonical order: by their module's place in the canonical
        // schedule (a cycle's earliest member; host code first), then by their
        // first statement. File positions then follow the schedule.
        let rank = |group: u32| -> u32 {
            if group == HOST_GROUP {
                return 0;
            }
            match cycle_of_group(group) {
                Some(cycle) => (0..graph.cycles.len())
                    .filter(|&module| graph.cycles[module] == Some(cycle))
                    .map(|module| graph.position[module].saturating_add(1))
                    .min()
                    .unwrap_or(u32::MAX),
                None => graph
                    .position
                    .get(group as usize)
                    .map_or(u32::MAX, |position| position.saturating_add(1)),
            }
        };
        let mut keys: Vec<(u32, usize, &EntrySet, u32)> = Vec::new();
        for statement in 0..count {
            budget.work(WorkKind::Analysis, 1)?;
            let atom = atoms.of[statement];
            let Some(group) = atoms.group[atom].filter(|_| atoms.anchored[atom]) else {
                continue;
            };
            let label = &labels[statement];
            if !keys
                .iter()
                .any(|(_, _, known, other)| *known == label && *other == group)
            {
                budget.push(Scratch, &mut keys, (rank(group), statement, label, group))?;
            }
        }
        keys.sort_unstable_by(|left, right| (left.0, left.1).cmp(&(right.0, right.1)));
        let mut group_file: Vec<((&EntrySet, u32), usize)> = budget.vector(Scratch, keys.len())?;
        for (_, _, label, group) in keys {
            let role = if group == HOST_GROUP {
                FileRole::Host
            } else {
                FileRole::Shared
            };
            let label_copy = label.clone_in(budget)?;
            budget.push(Retained, &mut files, file(role, label_copy))?;
            group_file.push(((label, group), files.len() - 1));
        }
        let group_module: Vec<Option<u32>> = {
            let mut modules = budget.filled(Scratch, files.len(), None)?;
            for ((_, group), index) in &group_file {
                modules[*index] = Some(*group);
            }
            modules
        };
        for statement in 0..count {
            let atom = atoms.of[statement];
            let Some(group) = atoms.group[atom].filter(|_| atoms.anchored[atom]) else {
                continue;
            };
            let key = (&labels[statement], group);
            let index = group_file
                .iter()
                .find(|(known, _)| *known == key)
                .map(|&(_, index)| index)
                .ok_or("a group has no file")?;
            for &member in &atoms.members[atom] {
                file_of[member] = index;
            }
        }
        // Which group modules each group's module statically reaches.
        let reaches = reachability(graph, budget)?;
        let group_reaches = |from: usize, to: usize| -> bool {
            match (group_module[from], group_module[to]) {
                (_, Some(HOST_GROUP)) => true,
                (Some(from), Some(to)) => {
                    let members = |group: u32| {
                        let cycle = cycle_of_group(group);
                        (0..if cycle.is_some() {
                            graph.cycles.len()
                        } else {
                            0
                        })
                            .filter(move |&module| graph.cycles[module] == cycle)
                            .chain(
                                (cycle.is_none() && group != HOST_GROUP).then_some(group as usize),
                            )
                    };
                    members(from).any(|module| members(to).all(|target| reaches[module][target]))
                }
                _ => false,
            }
        };
        // Definitions (design §7.5): every anchored group each depends on,
        // directly or through other definitions, and the latest of its own
        // label.
        let mut needs: Vec<Vec<usize>> = defaults_in(count, Scratch, budget)?;
        for statement in 0..count {
            let atom = atoms.of[statement];
            if atoms.anchored[atom] {
                continue;
            }
            for &(_, owner, _) in &facts.references[statement] {
                if atoms.anchored[atoms.of[owner]] && !needs[atom].contains(&file_of[owner]) {
                    budget.push(Scratch, &mut needs[atom], file_of[owner])?;
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
                    if atoms.anchored[target] || target == atom {
                        continue;
                    }
                    for index in 0..needs[target].len() {
                        let group = needs[target][index];
                        if !needs[atom].contains(&group) {
                            budget.push(Scratch, &mut needs[atom], group)?;
                            changed = true;
                        }
                    }
                }
            }
            budget.work(WorkKind::Analysis, count as u64)?;
            if !changed {
                break;
            }
        }
        // Definitions-only files, one per label and set of anchored groups its
        // definitions need: a group a definition needs never imports that
        // definition's file (references follow source import paths), so these
        // files close no cycle.
        let mut definitions: Vec<((&EntrySet, usize), usize)> = Vec::new();
        for statement in 0..count {
            let atom = atoms.of[statement];
            if atoms.anchored[atom] || file_of[statement] != NONE {
                continue;
            }
            needs[atom].sort_unstable();
            let label = &labels[statement];
            // The latest group of its own label, when that group's module
            // reaches every group the definition depends on: joining it then
            // adds no evaluation order the source lacks.
            let latest = needs[atom]
                .iter()
                .copied()
                .filter(|&group| files[group].label == *label)
                .max();
            let index = match latest.filter(|&group| {
                needs[atom]
                    .iter()
                    .all(|&other| other == group || group_reaches(group, other))
            }) {
                Some(group) => group,
                None => match definitions
                    .iter()
                    .find(|((known, needed), _)| *known == label && needs[*needed] == needs[atom])
                {
                    Some(&(_, index)) => index,
                    None => {
                        let copied = label.clone_in(budget)?;
                        budget.push(Retained, &mut files, file(FileRole::Definitions, copied))?;
                        budget.push(Scratch, &mut definitions, ((label, atom), files.len() - 1))?;
                        files.len() - 1
                    }
                },
            };
            for &member in &atoms.members[atom] {
                file_of[member] = index;
            }
        }
        let mut layout = Layout { files, file_of };
        settle(facts, &mut layout, budget)?;
        let reach_bytes = reaches
            .iter()
            .try_fold(vector_bytes(&reaches)?, |bytes, row| {
                bytes
                    .checked_add(vector_bytes(row)?)
                    .ok_or(AllocationError::Capacity)
            })?;
        drop(reaches);
        budget.release(Retained, reach_bytes)?;
        Ok(layout)
    })
}

/// The static import cycle a group stands for, if it is one.
fn cycle_of_group(group: u32) -> Option<u32> {
    (group != HOST_GROUP && group > u32::MAX / 2).then(|| u32::MAX - 1 - group)
}

/// Per module, every module its static imports reach (itself included).
fn reachability(
    graph: &EntryGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Vec<bool>>, AllocationError> {
    budget.retained_phase(|budget| {
        let count = graph.imports.len();
        let mut reaches = budget.vector(Retained, count)?;
        let mut pending = Vec::new();
        for start in 0..count {
            let mut row = budget.filled(Retained, count, false)?;
            budget.push(Scratch, &mut pending, start)?;
            while let Some(module) = pending.pop() {
                budget.work(WorkKind::Analysis, 1)?;
                if std::mem::replace(&mut row[module], true) {
                    continue;
                }
                extend_in(
                    &mut pending,
                    graph.imports[module].iter().map(|&target| target as usize),
                    Scratch,
                    budget,
                )?;
            }
            reaches.push(row);
        }
        Ok(reaches)
    })
}

/// Move one file into another, preserving canonical statement order.
fn merge(
    facts: &Facts,
    layout: &mut Layout,
    from: usize,
    into: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    for index in &mut layout.file_of {
        if *index == from {
            *index = into;
        }
    }
    settle(facts, layout, budget)?;
    if layout.files[into].role == FileRole::Definitions && layout.files[into].anchored {
        layout.files[into].role = FileRole::Shared;
    }
    Ok(())
}

/// Remove empty non-facades in place. Their payload, but not the surviving
/// vector's capacity, is released. Links are regenerated by the caller.
fn compact(layout: &mut Layout, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
    let released = {
        let mut phase = budget.scope();
        let mut renumber = phase.filled(Scratch, layout.files.len(), NONE)?;
        let mut next = 0;
        let mut released = 0u64;
        for (index, file) in layout.files.iter().enumerate() {
            if !file.statements.is_empty() || !file.links.foreign.is_empty() || !file.links.hosted.is_empty() || file.links.host.is_some()
                || matches!(file.role, FileRole::Entry(_) | FileRole::Lazy(_))
            {
                renumber[index] = next;
                next += 1;
            } else {
                released = released
                    .checked_add(file.heap_bytes()?)
                    .ok_or(AllocationError::Capacity)?;
            }
        }
        let mut index = 0;
        layout.files.retain(|_| {
            let keep = renumber[index] != NONE;
            index += 1;
            keep
        });
        for index in &mut layout.file_of {
            if *index != NONE {
                *index = renumber[*index];
            }
        }
        released
    };
    budget.release(Retained, released)
}

/// Keep a merge only after ordinary link and simulation admission. Resource
/// refusal aborts the producer; it is never misreported as an illegal merge.
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
    let mut phase = budget.scope();
    let mut trial = layout.clone_in(&mut phase)?;
    merge(context.facts, &mut trial, from, into, &mut phase)?;
    link(context, &mut trial, &mut phase)?;
    match simulate(context, &trial, reference, &mut phase) {
        Ok(()) => {
            phase.finish_retained()?;
            let released = layout.heap_bytes()?;
            drop(std::mem::replace(layout, trial));
            budget.release(Retained, released)?;
            Ok(true)
        }
        Err(VerificationError::Plan(_)) => Ok(false),
        Err(error) => Err(error.into()),
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
    export_placement: crate::config::ExportPlacement,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    // 1. Definitions-only files.
    let mut index = 0;
    while index < layout.files.len() {
        if layout.files[index].role == FileRole::Definitions
            && !layout.files[index].statements.is_empty()
        {
            let label = &layout.files[index].label;
            let target = (0..layout.files.len())
                .filter(|&other| {
                    other != index
                        && layout.files[other].label == *label
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
    let mut order = collect_in(
        (0..layout.files.len())
            .filter(|&file| {
                matches!(
                    layout.files[file].role,
                    FileRole::Shared | FileRole::Definitions
                ) && !layout.files[file].statements.is_empty()
            })
            .map(|file| (context.first_point(&layout.files[file]), file)),
        Scratch,
        budget,
    )?;
    order.sort_unstable();
    let mut previous: Vec<usize> = Vec::new();
    for &(_, file) in &order {
        if layout.files[file].statements.is_empty() {
            continue;
        }
        match previous
            .iter_mut()
            .find(|last| layout.files[**last].label == layout.files[file].label)
        {
            Some(last) => {
                if try_merge(context, layout, reference, file, *last, budget)? {
                    continue;
                }
                *last = file;
            }
            None => budget.push(Scratch, &mut previous, file)?,
        }
    }
    // 3 and 4. Files one entry alone loads into its facade.
    for facade in 0..layout.files.len() {
        if export_placement == crate::config::ExportPlacement::Facade && matches!(layout.files[facade].role, FileRole::Entry(_)) { continue; }
        let bit = match layout.files[facade].role {
            FileRole::Entry(entry) => entry as usize,
            FileRole::Lazy(module) => match context.graph.dynamic_bit(module) {
                Some(bit) => bit,
                None => continue,
            },
            _ => continue,
        };

        let mut candidates = collect_in(
            (0..layout.files.len())
                .filter(|&file| {
                    file != facade
                        && layout.files[file].label.bits().eq(std::iter::once(bit))
                        && !layout.files[file].statements.is_empty()
                        && matches!(
                            layout.files[file].role,
                            FileRole::Shared | FileRole::Definitions | FileRole::Host
                        )
                })
                .map(|file| (context.first_point(&layout.files[file]), file)),
            Scratch,
            budget,
        )?;
        candidates.sort_unstable();
        for &(_, file) in &candidates {
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
        release_vec(candidates, Scratch, budget)?;
    }
    release_vec(order, Scratch, budget)?;
    release_vec(previous, Scratch, budget)?;
    Ok(())
}

/// `preserve-modules` (design §7.4): every source module a file; host code
/// its own file; a definition a rule created in its only user's file, the
/// latest module file it references, or a definitions-only file. An entry
/// module another module imports keeps its module file and gets a facade.
fn preserve_layout(
    module: &Module,
    facts: &Facts,
    labels: &[EntrySet],
    graph: &EntryGraph,
    export_placement: crate::config::ExportPlacement,
    budget: &mut AllocationBudget<'_>,
) -> Result<Layout, OutputError> {
    budget.retained_phase(|budget| {
        let count = facts.count;
        let modules = graph.imports.len();
        let mut files: Vec<PlannedFile> = Vec::new();
        let mut module_file = budget.filled(Scratch, modules, NONE)?;
        let mut file_of = budget.filled(Retained, count, NONE)?;
        let imported = {
            let mut imported = budget.filled(Scratch, modules, false)?;
            for imports in &graph.imports {
                for &target in imports {
                    imported[target as usize] = true;
                }
            }
            imported
        };
        // An entry module another file imports, statically or with `import()`,
        // keeps its module file, and its entry file is a facade (P7).
        let shared = |module: u32| export_placement == crate::config::ExportPlacement::Facade || imported[module as usize] || graph.dynamic.contains(&module);
        // Entry files first, in entry order; then module files by position.
        for (entry, (_, root)) in graph.entries.iter().enumerate() {
            if !shared(*root) {
                let label = graph.reach[*root as usize].clone_in(budget)?;
                budget.push(
                    Retained,
                    &mut files,
                    file(FileRole::Entry(entry as u32), label),
                )?;
                module_file[*root as usize] = files.len() - 1;
            }
        }
        for (entry, (_, root)) in graph.entries.iter().enumerate() {
            if shared(*root) {
                let label = EntrySet::single_in(entry, Retained, budget)?;
                budget.push(
                    Retained,
                    &mut files,
                    file(FileRole::Entry(entry as u32), label),
                )?;
            }
        }
        // A module only `import()` loads is its own lazy file. One other files
        // also import statically, or an entry's module, is a module file, and
        // `import()` loads a facade re-exporting its namespace (below): the
        // module file's internal export names and the namespace's public names
        // could meet in one export list.
        let is_root = |module: u32| graph.entries.iter().any(|(_, root)| *root == module);
        let role_of = |module: u32| {
            if graph.dynamic.contains(&module) && !imported[module as usize] && !is_root(module) {
                FileRole::Lazy(module)
            } else {
                FileRole::Module(module)
            }
        };
        for statement in 0..count {
            let row = facts.rows[statement];
            let target = match row.origin {
                RowOrigin::Source | RowOrigin::Host => {
                    let module = row.module as usize;
                    if module_file[module] == NONE {
                        let label = graph.reach[module].clone_in(budget)?;
                        budget.push(Retained, &mut files, file(role_of(row.module), label))?;
                        module_file[module] = files.len() - 1;
                    }
                    module_file[module]
                }
                RowOrigin::Synthetic => continue,
            };
            file_of[statement] = target;
        }
        // Even an unused foreign binding retains loading and link validation.
        // Its facade keeps the request in the authored dependency order.
        for (source, owner) in &graph.foreign {
            let label = graph.reach[*owner as usize].clone_in(budget)?;
            if label.is_empty() { continue; }
            let mut foreign_file = file(FileRole::Module(*owner), label);
            foreign_file.links.host = module.carried.iter().position(|name| name == source);
            for (index, import) in module.imports.iter().enumerate() {
                if import.source.as_unicode() == Some(source.as_str()) {
                    let carried = module.carried.iter().any(|name| name == source);
                    budget.push(Retained, if carried { &mut foreign_file.links.hosted } else { &mut foreign_file.links.foreign }, index)?;
                }
            }
            module_file[*owner as usize] = files.len();
            budget.push(Retained, &mut files, foreign_file)?;
        }
        // Every entry and lazily loaded module has its file, even when empty.
        for &loaded in &graph.dynamic {
            if module_file[loaded as usize] == NONE {
                let label = graph.reach[loaded as usize].clone_in(budget)?;
                budget.push(Retained, &mut files, file(role_of(loaded), label))?;
                module_file[loaded as usize] = files.len() - 1;
            }
            if role_of(loaded) != FileRole::Lazy(loaded) {
                let bit = graph
                    .dynamic_bit(loaded)
                    .ok_or("a lazily loaded module has no entry bit")?;
                let label = EntrySet::single_in(bit, Retained, budget)?;
                budget.push(Retained, &mut files, file(FileRole::Lazy(loaded), label))?;
            }
        }
        // Rule-created definitions: their users' file when they have one.
        let mut users: Vec<Vec<usize>> = defaults_in(count, Scratch, budget)?;
        for statement in 0..count {
            for &(_, owner, _) in &facts.references[statement] {
                budget.push(Scratch, &mut users[owner], statement)?;
            }
        }
        let mut definitions = NONE;
        let mut unresolved = collect_in(
            (0..count).filter(|&statement| file_of[statement] == NONE),
            Scratch,
            budget,
        )?;
        let mut next = Vec::new();
        let mut targets = Vec::new();
        for _ in 0..=count {
            next.clear();
            for &statement in &unresolved {
                targets.clear();
                extend_in(
                    &mut targets,
                    users[statement].iter().map(|&user| file_of[user]),
                    Scratch,
                    budget,
                )?;
                if targets.contains(&NONE) {
                    budget.push(Scratch, &mut next, statement)?;
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
                                    && facts.rows[owner].origin != RowOrigin::Synthetic
                            })
                            .max_by_key(|&&(_, owner, _)| {
                                graph.position[facts.rows[owner].module as usize]
                            })
                            .map(|&(_, owner, _)| file_of[owner]);
                        match latest {
                            Some(file) => file,
                            None => {
                                if definitions == NONE {
                                    let label = labels[statement].clone_in(budget)?;
                                    budget.push(
                                        Retained,
                                        &mut files,
                                        file(FileRole::Definitions, label),
                                    )?;
                                    definitions = files.len() - 1;
                                } else {
                                    files[definitions].label.union_in(
                                        &labels[statement],
                                        Retained,
                                        budget,
                                    )?;
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
                        let label = labels[statement].clone_in(budget)?;
                        budget.push(Retained, &mut files, file(FileRole::Definitions, label))?;
                        definitions = files.len() - 1;
                    }
                    file_of[statement] = definitions;
                }
                break;
            }
            std::mem::swap(&mut unresolved, &mut next);
            if unresolved.is_empty() {
                break;
            }
        }
        let _ = module;
        let mut layout = Layout { files, file_of };
        settle(facts, &mut layout, budget)?;
        Ok(layout)
    })
}

/// Fold each entry facade into its module's file when no other file
/// imports that file; whether any facade folded.
fn fold_unimported_entry_modules(
    facts: &Facts,
    layout: &mut Layout,
    graph: &EntryGraph,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    let mut folded = false;
    for (entry, (_, root)) in graph.entries.iter().enumerate() {
        // `import()` of an entry module loads its module file, never the
        // entry file (P7).
        if graph.dynamic.contains(root) {
            continue;
        }
        let Some(facade) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Entry(entry as u32))
        else {
            continue;
        };
        let Some(own) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Module(*root))
        else {
            continue;
        };
        let imported = layout.files.iter().enumerate().any(|(index, file)| {
            index != facade
                && file
                    .links
                    .imports
                    .iter()
                    .any(|&(source, _)| source as usize == own)
        });
        if imported {
            continue;
        }
        let label = layout.files[own].label.clone_in(budget)?;
        layout.files[own].role = FileRole::Entry(entry as u32);
        layout.files[facade].role = FileRole::Definitions;
        let old = std::mem::replace(&mut layout.files[facade].label, label);
        release_vec(old.0, Retained, budget)?;
        merge(facts, layout, facade, own, budget)?;
        folded = true;
    }
    Ok(folded)
}

/// `single` with several entries (design §7.7): each entry's file holds the
/// fallback plan's files it loads, in its evaluation order. Every entry
/// gets its own instance of what it runs.
fn single(
    context: &Context<'_>,
    mut layout: Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<Layout, OutputError> {
    link(context, &mut layout, budget)?;
    let result = budget.retained_phase(|budget| {
        let mut files = budget.vector(Retained, context.graph.statics())?;
        for entry in 0..context.graph.statics() {
            budget.work(WorkKind::Analysis, layout.files.len() as u64)?;
            let facade = layout
                .files
                .iter()
                .position(|file| file.role == FileRole::Entry(entry as u32))
                .ok_or("an entry has no facade")?;
            let mut entry_file = file(
                FileRole::Entry(entry as u32),
                EntrySet::single_in(entry, Retained, budget)?,
            );
            let mut order = evaluation(&layout, facade, &[], budget)?;
            let mut seen = budget.filled(Scratch, layout.files.len(), false)?;
            for &index in &order { seen[index] = true; }
            let mut cursor = 0;
            while cursor < order.len() {
                let index = order[cursor]; cursor += 1;
                for &target in &layout.files[index].links.dynamic {
                    let loaded = evaluation(&layout, target as usize, &[], budget)?;
                    for &next in &loaded {
                        if !std::mem::replace(&mut seen[next], true) { budget.push(Retained, &mut order, next)?; }
                    }
                    release_vec(loaded, Retained, budget)?;
                }
            }
            release_vec(seen, Scratch, budget)?;
            for &index in &order {
                budget.extend_copy(
                    Retained,
                    &mut entry_file.statements,
                    &layout.files[index].statements,
                )?;
                entry_file.anchored |= layout.files[index].anchored;
                budget.extend_copy(
                    Retained,
                    &mut entry_file.modules,
                    &layout.files[index].modules,
                )?;
            }
            release_vec(order, Retained, budget)?;
            let mut positions = budget.filled(Scratch, context.graph.imports.len(), NONE)?;
            for (position, &module) in context.graph.orders[entry].iter().enumerate() { positions[module as usize] = position; }
            budget.work(WorkKind::Analysis, (entry_file.statements.len() as u64).saturating_mul(usize::BITS as u64))?;
            entry_file.statements.sort_by_key(|&index| {
                let index = index as usize;
                let statement = &context.module.regions[context.module.root.index()].statements[index];
                let instantiated = match statement {
                    Statement::Function { .. } => true,
                    Statement::Let { binding, value: Some(value) } => context.module.settled.get(binding.index()).copied().flatten() == Some(0)
                        && matches!(context.module.expressions[value.index()], Expr::Function(_)),
                    _ => false,
                };
                let position = context.facts.module_of(index).map_or(0, |module| positions[module as usize].saturating_add(1));
                (u8::from(!instantiated), position, index)
            });
            release_vec(positions, Scratch, budget)?;
            entry_file.modules.sort_unstable();
            entry_file.modules.dedup();
            // Static opaque modules can be evaluated between source owners in
            // one file. Lazy opaque bodies still require split delivery; the
            // activation runtime below continues to own typed lazy modules.
            let ordered_hosts = context.graph.dynamic.is_empty() && !context.module.carried.is_empty();
            if !context.graph.dynamic.is_empty() || ordered_hosts {
                let mut needed = budget.filled(Scratch, context.graph.imports.len(), false)?;
                let mut pending = budget.copy_slice(Scratch, &entry_file.modules)?;
                budget.extend_copy(Scratch, &mut pending, &context.graph.orders[entry])?;
                for &statement in &entry_file.statements {
                    budget.extend_copy(Scratch, &mut pending, &context.facts.every_load[statement as usize])?;
                }
                while let Some(source) = pending.pop() {
                    if std::mem::replace(&mut needed[source as usize], true) { continue; }
                    budget.extend_copy(Scratch, &mut pending, &context.graph.imports[source as usize])?;
                }
                for (source, &include) in needed.iter().enumerate() {
                    if !include { continue; }
                    let members = context.namespaces.iter().find(|(owner, _)| *owner as usize == source).map_or(&[][..], |(_, members)| members.as_slice());
                    let host = if ordered_hosts { context.graph.foreign.iter()
                        .find(|(_, owner)| *owner as usize == source)
                        .and_then(|(name, _)| context.module.carried.iter().position(|carried| carried == name)) } else { None };
                    let row = InlineModule { source: source as u32, host, publish: context.graph.dynamic.contains(&(source as u32)), eager: context.graph.reach[source].contains(entry),
                        dependencies: budget.copy_slice(Retained, &context.graph.imports[source])?,
                        members: super::cloning::map(members, budget, |(name, binding), budget| Ok((budget.string(Retained, name)?, *binding)))? };
                    budget.push(Retained, &mut entry_file.initializers, row)?;
                }
                if ordered_hosts {
                    let mut position = budget.filled(Scratch, context.graph.imports.len(), NONE)?;
                    for (index, &source) in context.graph.orders[entry].iter().enumerate() { position[source as usize] = index; }
                    budget.work(WorkKind::Analysis, (entry_file.initializers.len() as u64).saturating_mul(usize::BITS as u64))?;
                    entry_file.initializers.sort_by_key(|part| position[part.source as usize]);
                    release_vec(position, Scratch, budget)?;
                }
                entry_file.anchored = entry_file.statements.iter().any(|&statement| {
                    let row = context.facts.rows[statement as usize];
                    context.facts.anchored[statement as usize]
                        && (row.origin == RowOrigin::Synthetic || context.graph.reach[row.module as usize].contains(entry))
                });
                release_vec(needed, Scratch, budget)?;
                release_vec(pending, Scratch, budget)?;
            }
            files.push(entry_file);
        }
        Ok::<_, OutputError>(Layout {
            files,
            file_of: budget.filled(Retained, layout.file_of.len(), NONE)?,
        })
    })?;
    let released = layout.heap_bytes()?;
    drop(layout);
    budget.release(Retained, released)?;
    Ok(result)
}

/// `import()` built in place (`single`, or an ECMAScript target without
/// dynamic import) evaluates nothing when it runs: the namespace is made of
/// what the entry already delivered. So a module an entry reaches only
/// through `import()` may hold no anchored statement: running it with the
/// entry, or never, would both break DL2 and DL3 (plan M3.3d lowers such
/// modules to functions run once, before the promise resolves).
fn check_in_place_loads(
    facts: &Facts,
    graph: &EntryGraph,
    layout: &Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    let mut phase = budget.scope();
    let budget = &mut phase;
    let modules = graph.imports.len();
    let mut anchored_in = budget.filled(Scratch, modules, false)?;
    let mut statements_of = defaults_in(modules, Scratch, budget)?;
    for statement in 0..facts.count {
        let row = facts.rows[statement];
        if row.origin == RowOrigin::Synthetic || row.module as usize >= modules {
            continue;
        }
        budget.push(Scratch, &mut statements_of[row.module as usize], statement)?;
        anchored_in[row.module as usize] |= facts.anchored[statement];
    }
    for entry in 0..graph.statics() {
        budget.work(WorkKind::Analysis, (facts.count + modules) as u64)?;
        let Some(facade) = layout
            .files
            .iter()
            .position(|file| file.role == FileRole::Entry(entry as u32))
        else {
            continue;
        };
        let mut entry_phase = budget.scope();
        let budget = &mut entry_phase;
        let mut pending: Vec<u32> = Vec::new();
        for file in evaluation(layout, facade, &[], budget)? {
            for &statement in &layout.files[file].statements {
                budget.extend_copy(Scratch, &mut pending, &facts.every_load[statement as usize])?;
            }
        }
        let mut seen = budget.filled(Scratch, modules, false)?;
        let mut stack = Vec::new();
        while let Some(target) = pending.pop() {
            budget.push(Scratch, &mut stack, target as usize)?;
            while let Some(module) = stack.pop() {
                if module >= modules
                    || std::mem::replace(&mut seen[module], true)
                    || graph.reach[module].contains(entry)
                {
                    continue;
                }
                if anchored_in[module] {
                    return Err(OutputError::Invalid(
                        "an entry reaches a module only through `import()`, and that module runs code when it loads: this delivery builds `import()` in place, so it cannot run that code lazily (plan M3.3d). Deliver with `delivery.mode = \"split\"` or `\"preserve-modules\"` on a target with dynamic import, or import the module statically",
                    ));
                }
                for &statement in &statements_of[module] {
                    budget.extend_copy(Scratch, &mut pending, &facts.every_load[statement])?;
                }
                extend_in(
                    &mut stack,
                    graph.imports[module].iter().map(|&target| target as usize),
                    Scratch,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

/// `single`'s links: no file imports another; each prints the foreign
/// imports and host bindings its statements use and publishes its entry's
/// names.
fn link_single(
    context: &Context<'_>,
    layout: &mut Layout,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), OutputError> {
    let module = context.module;
    let facts = context.facts;
    for file in &mut layout.files {
        let FileRole::Entry(entry) = file.role else {
            return Err("a single-file entry plan holds a file that is no entry".into());
        };
        let links = budget.retained_phase(|budget| {
            let mut foreign = Vec::new();
            let mut hosted = Vec::new();
            let mut prior_effect = false;
            for &owner in &context.graph.orders[entry as usize] {
                if let Some((source, _)) = context.graph.foreign.iter().find(|(_, module)| *module == owner) {
                    let ordered_host = file.initializers.iter().any(|part| part.source == owner && part.host.is_some());
                    if prior_effect && !ordered_host { return Err(OutputError::Invalid("a foreign module must evaluate after a source initializer; single-file static imports cannot preserve that order: use split/preserve-modules, or embed the host module")); }
                    // The embedded body is an ordering barrier too. A later
                    // native import cannot be moved ahead of it.
                    prior_effect |= ordered_host;
                    for (index, import) in module.imports.iter().enumerate() {
                        if import.source.as_unicode() == Some(source.as_str()) { budget.push(Retained, if context.hosted[index] { &mut hosted } else { &mut foreign }, index)?; }
                    }
                } else {
                    prior_effect |= context.facts.rows.iter().enumerate().any(|(index, row)| row.module == owner && context.facts.anchored[index]);
                }
            }
            for part in &file.initializers {
                if !part.eager && context.graph.foreign.iter().any(|(_, owner)| *owner == part.source) {
                    return Err(OutputError::Invalid("a lazy foreign module cannot be eagerly imported into a single file; use split/preserve-modules, or embed a host module supported by typed lowering"));
                }
            }
            for &statement in &file.statements {
                for &import in &facts.imports[statement as usize] {
                    if context.hosted[import] {
                        budget.push(Retained, &mut hosted, import)?;
                    } else {
                        budget.push(Retained, &mut foreign, import)?;
                    }
                }
            }
            let mut public = Vec::new();
            for position in export_positions(module, entry as usize) {
                let export = &module.exports[position];
                let import = facts.foreign[export.binding.index()];
                if import != NONE {
                    if context.hosted[import] {
                        budget.push(Retained, &mut hosted, import)?;
                    } else {
                        budget.push(Retained, &mut foreign, import)?;
                    }
                }
                let name = budget.string(Retained, &export.name)?;
                budget.push(Retained, &mut public, (name, export.binding, 0))?;
            }
            let mut seen = budget.filled(Scratch, module.imports.len(), false)?;
            foreign.retain(|&index| !std::mem::replace(&mut seen[index], true));
            seen.fill(false);
            hosted.retain(|&index| !std::mem::replace(&mut seen[index], true));
            drop(seen);
            Ok::<_, OutputError>(FileLinks {
                imports: Vec::new(),
                exports: Vec::new(),
                public,
                foreign,
                hosted,
                host: None,
                dynamic: Vec::new(),
            })
        })?;
        file.links.replace_in(links, budget)?;
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
fn assignments(
    module: &Module,
    statement: &Statement,
    binding: BindingId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<ExprId>, AllocationError> {
    budget.retained_phase(|budget| {
        let mut found = Vec::new();
        let mut statements = budget.copy_slice(Scratch, &[statement])?;
        let mut expressions: Vec<ExprId> = Vec::new();
        loop {
            if let Some(expression) = expressions.pop() {
                let node = &module.expressions[expression.index()];
                if let Expr::Assign { target, .. } = node {
                    if module.expressions[target.index()] == Expr::Binding(binding) {
                        budget.push(Retained, &mut found, expression)?;
                    }
                }
                for function in node.created_functions() {
                    extend_in(
                        &mut statements,
                        module.regions[module.functions[function.index()].body.index()]
                            .statements
                            .iter(),
                        Scratch,
                        budget,
                    )?;
                }
                node.visit_children(|child| budget.push(Scratch, &mut expressions, child))?;
                continue;
            }
            let Some(current) = statements.pop() else {
                return Ok(found);
            };
            let mut status = Ok(());
            current.visit_expressions(|root| {
                if status.is_ok() {
                    status = budget.push(Scratch, &mut expressions, root);
                }
            });
            status?;
            let mut region = |id: RegionId| {
                extend_in(
                    &mut statements,
                    module.regions[id.index()].statements.iter(),
                    Scratch,
                    budget,
                )
            };
            match current {
                Statement::If { yes, no, .. } => {
                    region(*yes)?;
                    if let Some(no) = no {
                        region(*no)?;
                    }
                }
                Statement::Loop { body, .. }
                | Statement::ForIn { body, .. }
                | Statement::ForOf { body, .. }
                | Statement::Block(body) => region(*body)?,
                Statement::Try {
                    body,
                    catch,
                    finally,
                } => {
                    region(*body)?;
                    if let Some(catch) = catch {
                        region(catch.body)?;
                    }
                    if let Some(finally) = finally {
                        region(*finally)?;
                    }
                }
                Statement::Function { function, .. } => {
                    region(module.functions[function.index()].body)?
                }
                _ => {}
            }
        }
    })
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
                    budget.push(Scratch, &mut writes, (binding, owner, statement as usize))?;
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
                        spelling: budget.string(Retained, "v")?,
                        pinned: false,
                        class: None,
                        defined: false,
                    },
                    budget,
                )?;
                let target = module.expression_in(Expr::Binding(binding), None, budget)?;
                let read = module.expression_in(Expr::Binding(value), None, budget)?;
                let assign = module.expression_in(
                    Expr::Assign {
                        target,
                        value: read,
                    },
                    None,
                    budget,
                )?;
                budget.push(
                    crate::output_budget::AllocationClass::Retained,
                    &mut module.regions[body.index()].statements,
                    Statement::Return(Some(assign)),
                )?;
                let function =
                    FunctionId::try_new(module.functions.len()).ok_or("too many functions")?;
                let parameters = budget.copy_slice(Retained, &[value])?;
                budget.push(
                    crate::output_budget::AllocationClass::Retained,
                    &mut module.functions,
                    Function {
                        rest: false,
                        parameters: parameters,
                        body,
                        arrow: true,
                        name: FunctionName::Unobserved,
                        strict: false,
                        length: None,
                        suspension: Suspension::None,
                    },
                )?;
                let created = module.expression_in(Expr::Function(function), None, budget)?;
                let spelling = budget.format(
                    Retained,
                    format_args!("set_{}", module.bindings[binding.index()].spelling),
                )?;
                let setter = module.binding_in(
                    Binding {
                        source_symbol: None,
                        scope: root_scope,
                        spelling,
                        pinned: false,
                        class: None,
                        defined: false,
                    },
                    budget,
                )?;
                let root = module.root.index();
                let statement = module.regions[root].statements.len();
                module.push_root_admitted(
                    Statement::Let {
                        binding: setter,
                        value: Some(created),
                    },
                    RootRow::synthetic(facts.rows[owner].module),
                    budget,
                )?;
                // Beside the declaration, in its file.
                let home = layout.file_of[owner];
                budget.push(Retained, &mut layout.file_of, home)?;
                let position = layout.files[home]
                    .statements
                    .iter()
                    .position(|&placed| placed as usize == owner)
                    .map_or(layout.files[home].statements.len(), |at| at + 1);
                budget.reserve_vec(Retained, &mut layout.files[home].statements, 1)?;
                layout.files[home]
                    .statements
                    .insert(position, statement as u32);
                budget.push(Retained, &mut setters, (binding, setter))?;
                setter
            }
        };
        // Rewrite this writer's assignments.
        let found = assignments(
            module,
            &module.regions[module.root.index()].statements[writer],
            binding,
            budget,
        )?;
        for &assign in &found {
            let Expr::Assign { target, value } = module.expressions[assign.index()] else {
                continue;
            };
            module.set_expression(target, Expr::Binding(setter));
            let arguments = budget.copy_slice(Retained, &[value])?;
            module.set_expression(
                assign,
                Expr::Call {
                    callee: target,
                    arguments,
                    invocation: crate::primitive::Invocation::Reference,
                },
            );
        }
        release_vec(found, Retained, budget)?;
    }
    release_vec(writes, Scratch, budget)?;
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
    ext: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<DeliveryPlan>, OutputError> {
    let statics = graph.statics();
    if statics>1 && matches!(contract.format,crate::config::JavaScriptFormat::Iife|crate::config::JavaScriptFormat::Umd)
        && contract.container.global.as_ref().is_none_or(|name|!name.contains("[name]")&&!name.contains("[index]")) {
        return Err(OutputError::Invalid("several global entries require [name] or [index] in delivery.global"));
    }
    let simple_container=(matches!(contract.format,crate::config::JavaScriptFormat::Esm|crate::config::JavaScriptFormat::Bare)
        || (!contract.library && contract.format==crate::config::JavaScriptFormat::Iife))
        && contract.container.annotations == crate::config::ConsumerAnnotations::Off
        && contract.container.source_maps == crate::config::SourceMaps::Off
        && !module.root_rows.iter().any(|row| row.origin == RowOrigin::Host)
        && module.imports.is_empty();
    if statics==0 || (statics==1 && contract.mode==DeliveryMode::Single && simple_container && graph.dynamic.is_empty()) {
        return Ok(None);
    }
    let lazy_files = contract.mode != DeliveryMode::Single && dynamic_import;
    // Trimming is an admitted temporary projection. Its strings, adjacency
    // rows and labels cannot escape with the delivered target.
    if !lazy_files && contract.mode != DeliveryMode::Single && !graph.dynamic.is_empty() {
        return budget.with_temporary(
            |budget| {
                let mut entries = budget.vector(Retained, graph.entries.len())?;
                for (name, module) in &graph.entries {
                    entries.push((budget.string(Retained, name)?, *module));
                }
                let mut imports = budget.vector(Retained, graph.imports.len())?;
                for row in &graph.imports {
                    imports.push(budget.copy_slice(Retained, row)?);
                }
                let mut orders = budget.vector(Retained, statics)?;
                for row in &graph.orders[..statics] {
                    orders.push(budget.copy_slice(Retained, row)?);
                }
                let mut reach = budget.vector(Retained, graph.reach.len())?;
                for label in &graph.reach {
                    let mut label = label.clone_in(budget)?;
                    for index in 0..graph.dynamic.len() {
                        label.remove(statics + index);
                    }
                    reach.push(label);
                }
                let mut paths = budget.vector(Retained, graph.paths.len())?;
                for path in &graph.paths {
                    paths.push(budget.string(Retained, path)?);
                }
                Ok::<_, OutputError>(EntryGraph {
                    entries,
                    dynamic: Vec::new(),
                    imports,
                    orders,
                    reach,
                    paths,
                    position: budget.copy_slice(Retained, &graph.position)?,
                    cycles: budget.copy_slice(Retained, &graph.cycles)?,
                    host_importers: budget.copy_slice(Retained, &graph.host_importers)?,
                    foreign: super::cloning::map(&graph.foreign, budget, |(source, owner), budget| Ok((budget.string(Retained, source)?, *owner)))?,
                })
            },
            |graph, budget| plan_graph(module, graph, contract, lazy_files, ext, budget),
        );
    }
    plan_graph(module, graph, contract, lazy_files, ext, budget)
}

struct PlanningInput {
    facts: Facts,
    atoms: Atoms,
    labels: Vec<EntrySet>,
    hosted: Vec<bool>,
    positions: Vec<Vec<usize>>,
    namespaces: Vec<(u32, Vec<(String, BindingId)>)>,
    reference: Vec<Vec<usize>>,
}
impl PlanningInput {
    fn context<'a>(
        &'a self,
        module: &'a Module,
        facts: &'a Facts,
        graph: &'a EntryGraph,
        preserve: bool,
    ) -> Context<'a> {
        Context {
            module,
            facts,
            graph,
            hosted: &self.hosted,
            positions: &self.positions,
            namespaces: &self.namespaces,
            preserve,
        }
    }
}
fn plan_graph(
    module: &mut Module,
    graph: &EntryGraph,
    contract: &DeliveryContract,
    lazy_files: bool,
    ext: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<DeliveryPlan>, OutputError> {
    let is_lazy = |target: u32| (lazy_files || contract.mode == DeliveryMode::Single) && graph.dynamic.contains(&target);
    let preserve = contract.mode == DeliveryMode::PreserveModules || !graph.foreign.is_empty()
        || contract.mode == DeliveryMode::Split && graph.cycles.iter().any(Option::is_some);
    if preserve && contract.mode != DeliveryMode::Single {
        // A source function exists during module instantiation, before any
        // member of a cycle evaluates. A lexical function expression would
        // create an artificial TDZ in a separately delivered module.
        // Single-file functions already share one instantiation scope. Keep
        // their allocated bindings: two source modules may declare the same
        // observable function name. Ordered hosts hoist these initializers,
        // and lazy single-file modules instantiate their own local functions.
        let root = module.root.index();
        for index in 0..module.regions[root].statements.len() {
            budget.work(WorkKind::Analysis, 1)?;
            if graph.cycles[module.root_rows[index].module as usize].is_none() && graph.foreign.is_empty() { continue; }
            if matches!(module.regions[root].statements[index], Statement::Function { .. }) {
                module.root_rows[index].hoisted = true;
                continue;
            }
            let Statement::Let { binding, value: Some(value) } = module.regions[root].statements[index] else { continue; };
            if module.settled.get(binding.index()).copied().flatten() != Some(0) { continue; }
            let Expr::Function(function) = module.expressions[value.index()] else { continue; };
            module.functions[function.index()].arrow = false;
            module.regions[root].statements[index] = Statement::Function { binding, function };
            module.root_rows[index].hoisted = true;
        }
    }
    if contract.mode == DeliveryMode::Single && !graph.dynamic.is_empty() {
        if !module.reserved.iter().any(|name| name == "Object") {
            let name = budget.string(Retained, "Object")?;
            budget.push(Retained, &mut module.reserved, name)?;
        }
        for index in 0..module.regions[module.root.index()].statements.len() {
            budget.work(WorkKind::Analysis, 1)?;
            let Statement::Let { binding, value: Some(value) } = module.regions[module.root.index()].statements[index] else { continue; };
            if module.settled.get(binding.index()).copied().flatten() != Some(0) { continue; }
            let Expr::Function(function) = module.expressions[value.index()] else { continue; };
            module.functions[function.index()].arrow = false;
            if let Some(name) = module.functions[function.index()].name.exact().and_then(|name| name.as_unicode()) {
                if !module.reserved.iter().any(|reserved| reserved == name) {
                    let name = budget.string(Retained, name)?;
                    budget.push(Retained, &mut module.reserved, name)?;
                }
            }
        }
    }
    // Build before borrowing the mutable module in the consumer closure.
    let input = |module: &Module, budget: &mut AllocationBudget<'_>| {
        let facts = Facts::collect(module, &is_lazy, budget)?;
        let atoms = Atoms::build(&facts, graph, preserve, budget)?;
        let labels = labels(module, &facts, &atoms, graph, budget)?;
        let hosted = collect_in(
            module.imports.iter().map(|import| {
                import
                    .source
                    .as_unicode()
                    .is_some_and(|source| module.carried.iter().any(|carried| carried == source))
            }),
            Retained,
            budget,
        )?;
        let mut positions = budget.vector(Retained, graph.orders.len())?;
        for order in &graph.orders {
            let mut row = budget.filled(Retained, graph.imports.len(), NONE)?;
            for (position, &module) in order.iter().enumerate() {
                row[module as usize] = position;
            }
            positions.push(row);
        }
        let namespaces = namespaces(module, graph, budget)?;
        let mut input = PlanningInput {
            facts,
            atoms,
            labels,
            hosted,
            positions,
            namespaces,
            reference: Vec::new(),
        };
        input.reference = reference_orders(
            &input.context(module, &input.facts, graph, preserve),
            budget,
        )?;
        Ok::<_, OutputError>(input)
    };
    budget.with_temporary_context(module, input, |input, module, budget| {
        let context = input.context(module, &input.facts, graph, preserve);
        let mut layout = match contract.mode {
            DeliveryMode::Split if !preserve => {
                let mut layout = fallback(&input.facts, &input.atoms, &input.labels, graph, budget)?;
                link(&context, &mut layout, budget)?;
                simulate(&context, &layout, &input.reference, budget)?;
                coalesce(&context, &mut layout, &input.reference, contract.export_placement, budget)?;
                compact(&mut layout, budget)?;
                link(&context, &mut layout, budget)?;
                layout
            }
            DeliveryMode::PreserveModules | DeliveryMode::Split => {
                let mut layout = preserve_layout(module, &input.facts, &input.labels, graph, contract.export_placement, budget)?;
                compact(&mut layout, budget)?;
                link(&context, &mut layout, budget)?;
                if contract.export_placement != crate::config::ExportPlacement::Facade && fold_unimported_entry_modules(&input.facts, &mut layout, graph, budget)? {
                    compact(&mut layout, budget)?;
                    link(&context, &mut layout, budget)?;
                }
                simulate(&context, &layout, &input.reference, budget)?;
                layout
            }
            DeliveryMode::Single => {
                let layout = if graph.foreign.is_empty() { fallback(&input.facts, &input.atoms, &input.labels, graph, budget)? }
                    else { preserve_layout(module, &input.facts, &input.labels, graph, contract.export_placement, budget)? };
                let mut layout = single(&context, layout, budget)?;
                link_single(&context, &mut layout, budget)?;
                layout
            }
        };
        if !lazy_files && contract.mode != DeliveryMode::Single { check_in_place_loads(&input.facts, graph, &layout, budget)?; }
        let setters = if contract.mode == DeliveryMode::Single { Vec::new() }
            else { create_setters(module, &input.facts, &mut layout, budget)? };
        if layout.files.iter().any(|file| matches!(file.role, FileRole::Lazy(_))) && !module.reserved.iter().any(|name| name == "then") {
            let name = budget.string(Retained, "then")?;
            budget.push(Retained, &mut module.reserved, name)?;
        }
        if setters.is_empty() {
            finish_plan(input, module, &input.facts, graph, contract, layout, setters, ext, budget).map(Some)
        } else {
            budget.with_temporary(|budget| Facts::collect(module, &is_lazy, budget), |facts, budget| {
                link(&input.context(module, facts, graph, preserve), &mut layout, budget)?;
                finish_plan(input, module, facts, graph, contract, layout, setters, ext, budget).map(Some)
            })
        }
    })
}
fn finish_plan(
    input: &PlanningInput,
    module: &Module,
    facts: &Facts,
    graph: &EntryGraph,
    contract: &DeliveryContract,
    layout: Layout,
    setters: Vec<(BindingId, BindingId)>,
    ext: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<DeliveryPlan, OutputError> {
    let context = input.context(
        module,
        facts,
        graph,
        contract.mode == DeliveryMode::PreserveModules || !graph.foreign.is_empty() || contract.mode == DeliveryMode::Split && graph.cycles.iter().any(Option::is_some),
    );
    verify(&context, &layout, &input.reference, contract.mode, budget)?;
    let entries = closures(graph, &layout, contract.mode, budget)?;
    let naming = file_naming(&layout, graph, contract, ext, budget)?;
    let mut entry_names = budget.vector(Retained, graph.entries.len())?;
    for (name, _) in &graph.entries {
        entry_names.push(budget.string(Retained, name)?);
    }
    release_vec(layout.file_of, Retained, budget)?;
    Ok(DeliveryPlan {
        mode: contract.mode,
        files: layout.files,
        entries,
        setters,
        statics: graph.statics(),
        entry_names,
        naming,
        preload: contract.preload,
        format: contract.format,
        container: contract.container.clone_in(budget)?,
        request_bytes: contract.request_bytes,
        depth_bytes: contract.depth_bytes,
    })
}

/// Each file's name template and fields (design §8): entries by
/// `entry_names`, `preserve-modules` module files by `module_names`, every
/// other file by `chunk_names` with its position among them as `[index]`.
/// Two files whose names cannot differ are refused here; hashed names are
/// checked when they are known.
fn file_naming(
    layout: &Layout,
    graph: &EntryGraph,
    contract: &DeliveryContract,
    ext: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<FileName>, OutputError> {
    budget.retained_phase(|budget| {
    let path_of = |module: u32, budget: &mut AllocationBudget<'_>| {
        match graph.paths.get(module as usize) {
            Some(path) => budget.string(Retained, path),
            None => budget.format(Retained, format_args!("m{module}")),
        }
    };
    // In `preserve-modules` a module's file is named by its path, except
    // two internal files: an entry module's own file behind its entry file,
    // and a facade `import()` loads over a module file. Both are chunks, so
    // neither takes the entry file's name or its module file's.
    let internal = |role: FileRole| match role {
        FileRole::Module(module) => graph.entries.iter().any(|(_, root)| *root == module),
        FileRole::Lazy(module) => layout
            .files
            .iter()
            .any(|other| other.role == FileRole::Module(module)),
        _ => false,
    };
    let mut chunks = 0;
    let mut naming = budget.vector(Retained, layout.files.len())?;
    for (index, file) in layout.files.iter().enumerate() {
        let first = file.modules.first().copied();
        let named = match file.role {
            FileRole::Entry(entry) => {
                let (name, root) = &graph.entries[entry as usize];
                FileName {
                    template: budget.string(Retained, contract.entry_names())?,
                    name: budget.string(Retained, name)?,
                    index: entry as usize,
                    path: path_of(*root, budget)?,
                    ext: budget.string(Retained, ext)?,
                }
            }
            FileRole::Module(module) | FileRole::Lazy(module)
                if contract.mode == DeliveryMode::PreserveModules && !internal(file.role) =>
            {
                let path = path_of(module, budget)?;
                FileName {
                    template: budget.string(Retained, contract.module_names())?,
                    name: budget.string(Retained, path.rsplit('/').next().unwrap_or(&path))?,
                    index,
                    path,
                    ext: budget.string(Retained, ext)?,
                }
            }
            role => {
                let path = match role {
                    FileRole::Lazy(module) | FileRole::Module(module) => path_of(module, budget)?,
                    FileRole::Host => budget.string(Retained, "host")?,
                    _ => match first { Some(module) => path_of(module, budget)?, None => budget.string(Retained, "chunk")? },
                };
                chunks += 1;
                FileName {
                    template: budget.string(Retained, contract.chunk_names())?,
                    name: budget.string(Retained, path.rsplit('/').next().unwrap_or(&path))?,
                    index: chunks - 1,
                    path,
                    ext: budget.string(Retained, ext)?,
                }
            }
        };
        naming.push(named);
    }
    if !naming
        .iter()
        .any(|name| super::names::needs_hash(&name.template))
    {
        let mut names = budget.vector(Scratch, naming.len())?;
        for name in &naming {
            names.push(super::names::expand_in(&name.template, &super::names::Fields {
                name: &name.name, index: name.index, path: &name.path, ext: &name.ext, hash: "",
            }, Scratch, budget)?);
        }
        names.sort_unstable();
        if names.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(OutputError::Invalid(
                "two delivered files get one name: give `delivery.chunk_names` an [index] or [hash] and keep entry names distinct",
            ));
        }
    }
    Ok(naming)
    })
}

/// Each lazily loaded module's namespace, from its first `import()`.
fn namespaces(
    module: &Module,
    graph: &EntryGraph,
    budget: &mut AllocationBudget<'_>,
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
        if !graph.dynamic.contains(target) || namespaces.iter().any(|(known, _)| known == target) {
            continue;
        }
        let mut namespace = budget.vector(Retained, members.len())?;
        for (name, value) in members {
            let Expr::Binding(binding) = module.expressions[value.index()] else {
                return Err("a namespace member is not a binding".into());
            };
            namespace.push((budget.string(Retained, name)?, binding));
        }
        budget.push(Retained, &mut namespaces, (*target, namespace))?;
    }
    Ok(namespaces)
}

/// Each entry's files in evaluation order: a static entry's from its
/// facade; a lazily loaded module's beyond what every entry loading it has
/// already loaded.
fn closures(
    graph: &EntryGraph,
    layout: &Layout,
    mode: DeliveryMode,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<EntryDelivery>, AllocationError> {
    budget.retained_phase(|budget| {
        let mut entries = Vec::new();
        let mut loaded_by: Vec<Vec<bool>> = Vec::new();
        for (entry, (name, _)) in graph.entries.iter().enumerate() {
            let facade = layout
                .files
                .iter()
                .position(|file| file.role == FileRole::Entry(entry as u32))
                .unwrap_or(0);
            let closure = if mode == DeliveryMode::Single {
                budget.copy_slice(Retained, &[facade])?
            } else {
                evaluation(layout, facade, &[], budget)?
            };
            let mut loaded = budget.filled(Scratch, layout.files.len(), false)?;
            for &file in &closure {
                loaded[file] = true;
            }
            budget.push(Scratch, &mut loaded_by, loaded)?;
            let name = budget.string(Retained, name)?;
            let entry_closure =
                collect_in(closure.iter().map(|&file| file as u32), Retained, budget)?;
            budget.push(
                Retained,
                &mut entries,
                EntryDelivery {
                    name,
                    file: facade as u32,
                    dynamic: false,
                    closure: entry_closure,
                },
            )?;
            release_vec(closure, Retained, budget)?;
        }
        // Dynamic entries in bit order: a label's dynamic bits name them so.
        for &module in &graph.dynamic {
            let Some(index) = dynamic_file(layout, module) else {
                continue;
            };
            // Loaded by every static entry that can load it.
            let importers = collect_in(
                (0..graph.statics()).filter(|&entry| {
                    entries[entry].closure.iter().any(|&file| {
                        layout.files[file as usize]
                            .links
                            .dynamic
                            .contains(&(index as u32))
                    })
                }),
                Scratch,
                budget,
            )?;
            let mut loaded = budget.filled(Scratch, layout.files.len(), !importers.is_empty())?;
            for &entry in &importers {
                for (slot, &was) in loaded.iter_mut().zip(&loaded_by[entry]) {
                    *slot &= was;
                }
            }
            let closure = evaluation(layout, index, &loaded, budget)?;
            // A lazily loaded entry is named by its source module.
            let name = match graph.paths.get(module as usize) {
                Some(path) => budget.string(Retained, path)?,
                None => budget.format(Retained, format_args!("lazy-{module}"))?,
            };
            let entry_closure =
                collect_in(closure.iter().map(|&file| file as u32), Retained, budget)?;
            budget.push(
                Retained,
                &mut entries,
                EntryDelivery {
                    name,
                    file: index as u32,
                    dynamic: true,
                    closure: entry_closure,
                },
            )?;
            release_vec(closure, Retained, budget)?;
            release_vec(importers, Scratch, budget)?;
            release_vec(loaded, Scratch, budget)?;
        }
        Ok(entries)
    })
}

/// The plan verifier (design §7.11): a failure is an internal error that
/// names its assertion, never a size choice.
fn verify(
    context: &Context<'_>,
    layout: &Layout,
    reference: &[Vec<usize>],
    mode: DeliveryMode,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), VerificationError> {
    let mut phase = budget.scope();
    let budget = &mut phase;
    let facts = context.facts;
    let module = context.module;
    // P1: each statement once (split, preserve-modules), or once in each
    // single file whose entry runs it.
    let mut seen = budget.filled(Scratch, facts.count, 0usize)?;
    let mut own = budget.filled(Scratch, facts.count, false)?;
    for file in &layout.files {
        own.fill(false);
        for &statement in &file.statements {
            let statement = statement as usize;
            if statement >= facts.count || std::mem::replace(&mut own[statement], true) {
                return Err(PlanFailure::P1Placement.into());
            }
            seen[statement] += 1;
        }
    }
    if mode != DeliveryMode::Single && seen.iter().any(|&times| times != 1) {
        return Err(PlanFailure::P1Placement.into());
    }
    if mode == DeliveryMode::Single && seen.iter().any(|&times| times == 0) {
        return Err(PlanFailure::P1Placement.into());
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
            return Err(PlanFailure::P1Placement.into());
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
                        return Err(PlanFailure::P2Write.into());
                    }
                    let imported = file.links.imports.iter().any(|(from, bindings)| {
                        *from as usize == source && bindings.contains(&binding)
                    });
                    if !imported {
                        return Err(PlanFailure::P10Reference.into());
                    }
                }
            }
            // P3: every imported binding is exported by its file.
            for (source, bindings) in &file.links.imports {
                let exports = &layout.files[*source as usize].links.exports;
                if bindings.iter().any(|binding| !exports.contains(binding)) {
                    return Err(PlanFailure::P3Link.into());
                }
            }
        }
        simulate(context, layout, reference, budget)?;
    }
    // P8: each entry publishes exactly its names.
    for (index, file) in layout.files.iter().enumerate() {
        let FileRole::Entry(entry) = file.role else {
            continue;
        };
        let expected = export_positions(module, entry as usize).map(|position| {
            let export = &module.exports[position];
            (export.name.as_str(), export.binding)
        });
        let actual = file
            .links
            .public
            .iter()
            .map(|(name, binding, _)| (name.as_str(), *binding));
        if !actual.eq(expected) {
            return Err(PlanFailure::P8Surface.into());
        }
        let _ = index;
    }
    // P9: preserve-modules keeps every source statement in its module's
    // file, and every entry module is a file.
    if mode == DeliveryMode::PreserveModules {
        for (statement, &file) in layout.file_of.iter().enumerate() {
            let row = facts.rows[statement];
            if row.origin == RowOrigin::Synthetic {
                continue;
            }
            let home = match layout.files[file].role {
                FileRole::Module(owner) | FileRole::Lazy(owner) => owner == row.module,
                FileRole::Entry(entry) => context.graph.entries[entry as usize].1 == row.module,
                _ => false,
            };
            if !home {
                return Err(PlanFailure::P9Modules.into());
            }
        }
    }
    Ok(())
}

/// `[delivery] lazy_functions`: the functions a file creates while it loads
/// are spelled `function`. An engine parses an arrow's body together with the
/// code that creates it (V8 defers only `function` bodies, outside a script's
/// top level), so a module of arrows parses all of its code at import; a
/// `function` body is pre-scanned and parsed when it is first called.
///
/// A root binding of a source function, settled at instantiation, becomes the
/// function's declaration, as the source declared it. Any other function the
/// root's frame creates becomes a function expression in place. A function
/// that reads a frame (`this`, `arguments`, `super`, direct `eval`) keeps its
/// arrow, and the functions its body creates are the root frame's. Returns
/// whether the tree changed.
pub(crate) fn spell_lazy_functions(module: &mut Module) -> bool {
    let mut changed = false;
    let root = module.root.index();
    for index in 0..module.regions[root].statements.len() {
        let Statement::Let { binding, value: Some(value) } = module.regions[root].statements[index] else { continue; };
        let Expr::Function(function) = module.expressions[value.index()] else { continue; };
        if !module.functions[function.index()].arrow
            || module.settled.get(binding.index()).copied().flatten() != Some(0)
            || !module.frame_free(function)
        {
            continue;
        }
        module.functions[function.index()].arrow = false;
        module.regions[root].statements[index] = Statement::Function { binding, function };
        changed = true;
    }
    let mut arrows = Vec::new();
    let mut regions = vec![module.root];
    let mut expressions = Vec::new();
    while let Some(region) = regions.pop() {
        for statement in &module.regions[region.index()].statements {
            statement.visit_expressions(|root| expressions.push(root));
            statement.visit_regions(|child| regions.push(child));
        }
        while let Some(id) = expressions.pop() {
            let expression = &module.expressions[id.index()];
            if let Expr::Function(function) = *expression {
                if module.functions[function.index()].arrow {
                    if module.frame_free(function) {
                        arrows.push(function);
                    } else {
                        regions.push(module.functions[function.index()].body);
                    }
                }
            }
            let _ = expression.visit_children(|child| {
                expressions.push(child);
                Ok::<_, ()>(())
            });
        }
    }
    for function in arrows {
        module.functions[function.index()].arrow = false;
        changed = true;
    }
    changed
}

#[cfg(test)]
#[path = "delivery_admission_tests.rs"]
mod admission_tests;
