//! Editor/session syntax ownership over the ordinary module loader. The arena
//! belongs to a bounded session epoch; no self-referential owner or leaked text.
use super::*;
use crate::parser::BorrowedArena;
use std::collections::{BTreeMap, BTreeSet};

struct CachedModule<'a> {
    source: &'a str,
    syntax: Program<'a, 'a>,
}
pub(crate) struct ParsedModuleCache<'a> {
    arena: &'a Bump,
    modules: BTreeMap<PathBuf, CachedModule<'a>>,
    pub(crate) parsed: usize,
    pub(crate) reused: usize,
}
impl<'a> ParsedModuleCache<'a> {
    pub(crate) fn new(arena: &'a Bump) -> Self {
        Self {
            arena,
            modules: BTreeMap::new(),
            parsed: 0,
            reused: 0,
        }
    }
    pub(crate) fn discover(
        &mut self,
        entries: &[EntrySource],
        overrides: &[SourceOverride<'_>],
        config: &ProjectConfig,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(ModuleSet<&'a str>, Vec<Program<'a, 'a>>), ModuleDiscoveryError> {
        self.parsed = 0;
        self.reused = 0;
        if !config.cache.elaboration_reuse {
            self.modules.clear();
        }
        let resolver = load_package_resolver(config)
            .map_err(|error| ModuleError::new(error.path, "", Span::empty(0), error.message))?;
        let arena = BorrowedArena::new(self.arena, budget)?;
        let result = discover_with_storage(
            entries,
            None,
            overrides,
            resolver,
            CachedSources {
                cache: self,
                syntax: Vec::new(),
                arena,
            },
        )?;
        let reachable = result
            .0
            .modules
            .iter()
            .map(|module| module.path.clone())
            .collect::<BTreeSet<_>>();
        self.modules.retain(|path, _| reachable.contains(path));
        Ok(result)
    }
    fn store(
        &mut self,
        path: &Path,
        text: &str,
        arena: &BorrowedArena<'a, '_, '_>,
    ) -> Result<&'a str, AllocationError> {
        if let Some(known) = self.modules.get(path) {
            if known.source == text {
                return Ok(known.source);
            }
        }
        arena.store(text)
    }
}
struct CachedSources<'a, 'cache, 'budget, 'ledger> {
    cache: &'cache mut ParsedModuleCache<'a>,
    syntax: Vec<Program<'a, 'a>>,
    arena: BorrowedArena<'a, 'budget, 'ledger>,
}
impl<'a> DiscoveryStorage for CachedSources<'a, '_, '_, '_> {
    type Source = &'a str;
    type Parsed = Vec<Program<'a, 'a>>;
    fn work(&mut self, units: u64) -> Result<(), AllocationError> {
        self.arena
            .with_budget(|budget| budget.work(WorkKind::Analysis, units))
    }
    fn read(&mut self, path: &Path) -> Result<&'a str, ModuleDiscoveryError> {
        let text = self
            .arena
            .with_budget(|budget| read_module_source(path, budget))?;
        let result = self.cache.store(path, &text, &self.arena);
        let bytes = text.capacity() as u64;
        drop(text);
        self.arena
            .with_budget(|budget| budget.release(Retained, bytes))?;
        result.map_err(Into::into)
    }
    fn copy_override(&mut self, path: &Path, text: &str) -> Result<&'a str, AllocationError> {
        let path = path.canonicalize().unwrap_or_else(|_| path.to_owned());
        self.cache.store(&path, text, &self.arena)
    }
    fn imports(
        &mut self,
        path: &Path,
        source: &&'a str,
    ) -> Result<(ImportSites, ImportSites, ImportSites), ModuleDiscoveryError> {
        let program = if let Some(known) = self
            .cache
            .modules
            .get(path)
            .filter(|known| known.source == *source)
        {
            self.cache.reused += 1;
            known.syntax.clone()
        } else {
            let syntax = self
                .arena
                .parse(source)
                .map_err(|error| discovery_parse_error(path, source, error))?;
            self.cache.parsed += 1;
            self.cache.modules.insert(
                path.to_owned(),
                CachedModule {
                    source,
                    syntax: syntax.clone(),
                },
            );
            syntax
        };
        let imports = collect_imports(&program);
        self.syntax.push(program);
        Ok(imports)
    }
    fn into_parsed(self) -> Self::Parsed {
        self.syntax
    }
}
