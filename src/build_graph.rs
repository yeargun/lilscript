//! Scoped graph reuse for editors and repeated build clients. The caller owns
//! the arena epoch, so syntax/checker references never outlive their source.
use super::*;
use crate::module::{ParsedModuleCache, SourceOverride};
use bumpalo::Bump;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct GraphSessionStats {
    pub parsed_modules: usize,
    pub reused_syntax_modules: usize,
    pub checked_modules: Vec<PathBuf>,
    pub reused_elaboration_modules: Vec<PathBuf>,
    pub source_syntax_arena_bytes: usize,
    pub requests_in_epoch: usize,
}

pub struct GraphSession<'a> {
    arena: &'a Bump,
    syntax: ParsedModuleCache<'a>,
    elaboration: crate::check::ElaborationCache<'a, 'a>,
    stats: GraphSessionStats,
}
impl<'a> GraphSession<'a> {
    pub fn new(arena: &'a Bump) -> Self {
        Self {
            arena,
            syntax: ParsedModuleCache::new(arena),
            elaboration: Default::default(),
            stats: Default::default(),
        }
    }
    pub fn stats(&self) -> &GraphSessionStats {
        &self.stats
    }
    /// Replace this session and its arena between requests. The byte threshold
    /// is a soft epoch boundary; one request can exceed it. The request cap also
    /// bounds obsolete small revisions when their arena fits below the threshold.
    pub fn needs_new_epoch(&self, config: &ProjectConfig) -> bool {
        self.stats.requests_in_epoch >= 64
            || self.arena.allocated_bytes() as u64 >= config.cache.frontend_cache_bytes
    }
    pub fn inspect<R>(
        &mut self,
        entries: &[EntrySource],
        overrides: &[SourceOverride<'_>],
        focus: &Path,
        config: &ProjectConfig,
        client: impl for<'v> FnOnce(&CheckedProgram<'v, 'a, 'a>) -> R,
    ) -> Result<R, ServiceError> {
        let entries = sorted_entries(entries)?;
        let mut frontend = Frontend::new(config, check_options())?;
        let source_contract = frontend.source_contract();
        let trap_index_reads = frontend.trap_index_reads();
        let policy = frontend
            .javascript
            .as_ref()
            .or(frontend.native.as_ref())
            .unwrap();
        let mut budget = AllocationBudget::new(Some((&mut frontend.ledger, WorkDomain::Baseline)));
        self.stats.requests_in_epoch += 1;
        self.elaboration.reused.clear();
        self.elaboration.checked.clear();
        self.stats.checked_modules.clear();
        self.stats.reused_elaboration_modules.clear();
        let (modules, syntax) = self
            .syntax
            .discover(&entries, overrides, config, &mut budget)
            .map_err(|error| match error {
                ModuleDiscoveryError::Module(error) => ServiceError::module("discovery", error),
                ModuleDiscoveryError::Resources(error) => {
                    ServiceError::resources("discovery resources", error)
                }
            })?;
        self.stats.parsed_modules = self.syntax.parsed;
        self.stats.reused_syntax_modules = self.syntax.reused;
        self.stats.source_syntax_arena_bytes = self.arena.allocated_bytes();
        budget
            .retain(
                crate::output_budget::AllocationClass::Scratch,
                self.arena.allocated_bytes() as u64,
            )
            .map_err(|error| ServiceError::resources("session syntax resources", error))?;
        if !config.cache.elaboration_reuse {
            self.elaboration.clear();
        }
        let focus = focus.canonicalize().unwrap_or_else(|_| focus.to_owned());
        let before = modules
            .modules
            .iter()
            .position(|module| module.path == focus)
            .unwrap_or(modules.root());
        let checked = crate::check::analyze_modules_cached(
            &syntax,
            &modules,
            source_contract,
            &mut budget,
            config
                .cache
                .elaboration_reuse
                .then_some(&mut self.elaboration),
            before,
        );
        if config.cache.elaboration_reuse {
            self.stats.checked_modules = self
                .elaboration
                .checked
                .iter()
                .map(|&id| modules.modules[id].path.clone())
                .collect();
            self.stats.reused_elaboration_modules = self
                .elaboration
                .reused
                .iter()
                .map(|&id| modules.modules[id].path.clone())
                .collect();
        } else {
            self.stats.checked_modules = modules
                .modules
                .iter()
                .map(|module| module.path.clone())
                .collect();
        }
        let checked = checked.map_err(|error| {
            let source = &modules.modules[error.module];
            native_check_error(&source.path, source.source, error.error)
        })?;
        let prepared = prepare_checked_graph(
            &modules,
            &syntax,
            &checked,
            GraphConversion {
                source_contract,
                hosts: policy.hosts(),
                defines: policy.defines(),
                javascript: policy.javascript_contract(),
                native: false,
                library: true,
                rules: None,
                trap_index_reads,
            },
            &mut budget,
        )?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client(&CheckedProgram {
                modules: &modules,
                syntax: &syntax,
                semantics: &checked,
                program: prepared.program.program(),
            })
        }));
        drop(budget);
        prepared.program.discard(&mut frontend.ledger);
        match result {
            Ok(value) => Ok(value),
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Work(PathBuf);
    impl Work {
        fn new(label: &str) -> Self {
            let root =
                std::env::temp_dir().join(format!("lilscript-d3-{label}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn write(&self, name: &str, text: &str) -> PathBuf {
            let path = self.0.join(name);
            std::fs::write(&path, text).unwrap();
            path
        }
    }
    impl Drop for Work {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn view(checked: &CheckedProgram<'_, '_, '_>) -> Vec<String> {
        checked
            .program
            .units()
            .iter()
            .map(|unit| format!("{:?}", unit.data().operations))
            .collect()
    }
    #[test]
    fn d3_module_reuse_matches_cold_conversion_and_invalidates_dependencies() {
        let work = Work::new("reuse");
        let dependency = work.write("dependency.lil", "export int twice(int n){return n+n;}");
        let old = "import {twice} from \"./dependency\";export int run(int n){return twice(n)+1;}";
        let changed = old.replace("+1", "+2");
        let root = work.write("main.lil", old);
        let entries = [EntrySource::of(&root)];
        let config = ProjectConfig::default();
        let arena = Bump::new();
        let mut session = GraphSession::new(&arena);
        let before = session
            .inspect(&entries, &[], &root, &config, view)
            .unwrap();
        let overlays = [SourceOverride {
            path: &root,
            source: &changed,
        }];
        let warm = session
            .inspect(&entries, &overlays, &root, &config, view)
            .unwrap();
        let cold = with_checked_graph(&entries, &overlays, &config, view).unwrap();
        assert_eq!(warm, cold);
        assert_ne!(warm, before);
        assert_eq!(session.stats().parsed_modules, 1);
        assert_eq!(
            session.stats().reused_elaboration_modules,
            vec![dependency.clone()]
        );
        let invalid = "export string twice(int n){return \"bad\";}";
        let overlays = [
            SourceOverride {
                path: &root,
                source: &changed,
            },
            SourceOverride {
                path: &dependency,
                source: invalid,
            },
        ];
        let warm = session
            .inspect(&entries, &overlays, &root, &config, |_| ())
            .unwrap_err();
        let cold = with_checked_graph(&entries, &overlays, &config, |_| ()).unwrap_err();
        assert_eq!(warm.diagnostic, cold.diagnostic);
        assert!(session.stats().reused_elaboration_modules.is_empty());
    }
    #[test]
    fn d3_configured_graph_checks_all_entries_and_all_unsaved_sources() {
        let work = Work::new("entries");
        let helper = work.write("helper.lil", "export int value=\"bad\";");
        let a = work.write(
            "a.lil",
            "import {value} from \"./helper\";export int read(){return value;}",
        );
        let b = work.write("b.lil", "export int other=\"bad\";");
        let mut config = ProjectConfig::default();
        config.config_dir = Some(work.0.clone());
        config
            .delivery
            .entries
            .insert("a".into(), PathBuf::from("a.lil"));
        config
            .delivery
            .entries
            .insert("b".into(), PathBuf::from("b.lil"));
        let entries = configured_entries(&helper, &config);
        assert_eq!(entries.len(), 2);
        let overlays = [
            SourceOverride {
                path: &helper,
                source: "export int value=3;",
            },
            SourceOverride {
                path: &b,
                source: "export int other=4;",
            },
        ];
        let arena = Bump::new();
        let mut session = GraphSession::new(&arena);
        let roots = session
            .inspect(&entries, &overlays, &a, &config, |checked| {
                checked.semantics.roots().len()
            })
            .unwrap();
        assert_eq!(roots, 2);
        let error = with_checked_program(&a, None, &config, |_| ()).unwrap_err();
        assert_eq!(error.diagnostic.unwrap().path, helper);
    }
    #[test]
    fn d3_reuse_controls_contract_changes_and_epoch_boundaries_are_explicit() {
        let work = Work::new("controls");
        work.write("d.lil", "export int inc(int n){return n+1;}");
        let root = work.write(
            "main.lil",
            "import {inc} from \"./d\";export int run(){return inc(3);}",
        );
        let entries = [EntrySource::of(&root)];
        let arena = Bump::new();
        let mut session = GraphSession::new(&arena);
        let mut config = ProjectConfig::default();
        session
            .inspect(&entries, &[], &root, &config, |_| ())
            .unwrap();
        config.cache.elaboration_reuse = false;
        session
            .inspect(&entries, &[], &root, &config, |_| ())
            .unwrap();
        assert_eq!(session.stats().parsed_modules, 2);
        assert_eq!(session.stats().reused_syntax_modules, 0);
        assert!(session.stats().reused_elaboration_modules.is_empty());
        config.cache.frontend_cache_bytes = 1;
        assert!(session.needs_new_epoch(&config));
        config.cache.elaboration_reuse = true;
        session
            .inspect(&entries, &[], &root, &config, |_| ())
            .unwrap();
        config.language.const_evaluation.steps = 1;
        let warm = session.inspect(&entries, &[], &root, &config, view);
        let cold = with_checked_graph(&entries, &[], &config, view);
        match (warm, cold) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => assert_eq!(a.to_string(), b.to_string()),
            other => panic!("{other:?}"),
        }
        assert!(session.stats().reused_elaboration_modules.is_empty());
    }

    #[test]
    fn d3_inheritance_diagnostics_keep_the_declaring_module_through_reuse() {
        let work = Work::new("class-diagnostic");
        work.write("base.lil", "export class Base { int value(){return 1;} }");
        let child = work.write("child.lil", "import {Base} from \"./base\";export class Child extends Base { int value(){return 2;} }");
        let root = work.write(
            "main.lil",
            "import {Child} from \"./child\";class Last {int unrelated;}export int answer=7;",
        );
        let entries = [EntrySource::of(&root)];
        let config = ProjectConfig::default();
        let arena = Bump::new();
        let mut session = GraphSession::new(&arena);
        let cold = with_checked_graph(&entries, &[], &config, |_| ()).unwrap_err();
        assert_eq!(cold.diagnostic.as_ref().unwrap().path, child);
        for _ in 0..2 {
            let warm = session
                .inspect(&entries, &[], &root, &config, |_| ())
                .unwrap_err();
            assert_eq!(warm.diagnostic.as_ref().unwrap().path, child);
            assert_eq!(warm.to_string(), cold.to_string());
        }
    }

    #[test]
    fn d3_cached_parsing_keeps_resource_admission_and_recovers_after_refusals() {
        let work = Work::new("resources");
        let root = work.write("main.lil", "export int answer(){return 7;}");
        let entries = [EntrySource::of(&root)];
        let arena = Bump::new();
        let mut session = GraphSession::new(&arena);
        let limited: ProjectConfig =
            toml::from_str("[policy.resources]\nretained_bytes=128\n").unwrap();
        let enormous = format!("export string text=\"{}\";", "x".repeat(65536));
        let error = session
            .inspect(
                &entries,
                &[SourceOverride {
                    path: &root,
                    source: &enormous,
                }],
                &root,
                &limited,
                |_| (),
            )
            .unwrap_err();
        assert!(error.resource.is_some(), "{error:?}");
        assert_eq!(
            arena.allocated_bytes(),
            0,
            "source growth must be admitted before allocation"
        );
        let normal = ProjectConfig::default();
        session
            .inspect(&entries, &[], &root, &normal, |_| ())
            .unwrap();
        let error = session
            .inspect(&entries, &[], &root, &limited, |_| ())
            .unwrap_err();
        assert!(
            error.resource.is_some(),
            "warm backing must still fit the request: {error:?}"
        );
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            session.inspect(&entries, &[], &root, &normal, |_| panic!("client unwind"))
        }));
        assert!(panic.is_err());
        session
            .inspect(&entries, &[], &root, &normal, |_| ())
            .unwrap();
    }
}
