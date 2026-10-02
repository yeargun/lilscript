//! A single checkpoint at a dependency-scheduled module boundary. The cache
//! owns one declaration table, not a copy per module. Exact syntax/interface
//! guards deliberately invalidate more than the minimum dependency closure.
use super::*;

#[derive(Clone)]
pub(super) struct SignatureKey<'ast, 'src> {
    syntax: Vec<Program<'ast, 'src>>,
    interfaces: Vec<ModuleInterface<'src>>,
    order: Vec<ModuleId>,
    roots: Vec<ModuleId>,
    names: Vec<String>,
    contract: crate::config::LanguageConfig,
}
impl<'ast, 'src> SignatureKey<'ast, 'src> {
    pub(super) fn new(
        programs: &[Program<'ast, 'src>],
        phase: &SignaturePhase<'ast, 'src>,
    ) -> Self {
        Self {
            syntax: programs.to_vec(),
            interfaces: phase.checked.interfaces.clone(),
            order: phase.checked.initialization_order.clone(),
            roots: phase.checked.roots.clone(),
            names: phase.checked.root_names.clone(),
            contract: phase.checked.declarations.source_contract,
        }
    }
    fn compatible(&self, other: &Self, prefix: usize) -> bool {
        self.contract == other.contract
            && self.order == other.order
            && self.roots == other.roots
            && self.names == other.names
            && self.interfaces == other.interfaces
            && self.syntax.len() == other.syntax.len()
            && self.order.iter().take(prefix).all(|&module| {
                self.syntax[module]
                    .source_identity()
                    .same(other.syntax[module].source_identity())
            })
            && self
                .syntax
                .iter()
                .zip(&other.syntax)
                .all(|(a, b)| same_signatures(a, b))
    }
}

/// Only top-level function bodies may change without rebuilding the declaration
/// checkpoint. All node ids, spans, default expressions, imports, schemas and
/// top-level initializers still compare exactly. Source identity additionally
/// proves every reused module body itself is unchanged. This also covers cycles:
/// their peers' declaration contracts remain part of this guard.
fn same_signatures(a: &Program<'_, '_>, b: &Program<'_, '_>) -> bool {
    if a.source_identity().same(b.source_identity()) {
        return true;
    }
    a.imports == b.imports
        && a.foreign_imports == b.foreign_imports
        && a.exports == b.exports
        && a.items.len() == b.items.len()
        && a.items.iter().zip(b.items).all(|(a, b)| match (a, b) {
            (Item::Function(a), Item::Function(b)) => {
                let mut a = a.clone();
                let mut b = b.clone();
                a.body = &[];
                b.body = &[];
                a == b
            }
            _ => a == b,
        })
}

struct Checkpoint<'ast, 'src> {
    key: SignatureKey<'ast, 'src>,
    prefix: usize,
    declarations: DeclarationTables<'src>,
    initialization: ModuleInitialization,
    facts: Vec<(ModuleId, ModuleFacts<'ast, 'src>, ModuleInterface<'src>)>,
}

#[derive(Default)]
pub(crate) struct ElaborationCache<'ast, 'src> {
    checkpoint: Option<Checkpoint<'ast, 'src>>,
    pub(crate) reused: Vec<ModuleId>,
    pub(crate) checked: Vec<ModuleId>,
}
impl<'ast, 'src> ElaborationCache<'ast, 'src> {
    pub(super) fn restore(
        &mut self,
        key: &SignatureKey<'ast, 'src>,
        checked: &mut CheckedModules<'ast, 'src>,
        initialization: &mut ModuleInitialization,
    ) -> usize {
        self.reused.clear();
        self.checked.clear();
        let Some(saved) = self.checkpoint.take() else {
            return 0;
        };
        if !saved.key.compatible(key, saved.prefix) {
            return 0;
        }
        checked.declarations = saved.declarations;
        *initialization = saved.initialization;
        for (module, facts, interface) in saved.facts {
            checked.facts[module] = facts;
            checked.interfaces[module] = interface;
            self.reused.push(module);
        }
        saved.prefix
    }
    pub(super) fn save(
        &mut self,
        key: SignatureKey<'ast, 'src>,
        prefix: usize,
        checked: &CheckedModules<'ast, 'src>,
        initialization: &ModuleInitialization,
    ) {
        if prefix == 0 {
            self.checkpoint = None;
            return;
        }
        self.checkpoint = Some(Checkpoint {
            key,
            prefix,
            declarations: checked.declarations.clone(),
            initialization: initialization.clone(),
            facts: checked
                .initialization_order
                .iter()
                .take(prefix)
                .map(|&module| {
                    (
                        module,
                        checked.facts[module].clone(),
                        checked.interfaces[module].clone(),
                    )
                })
                .collect(),
        });
    }
    pub(crate) fn clear(&mut self) {
        self.checkpoint = None;
        self.reused.clear();
        self.checked.clear();
    }
}
