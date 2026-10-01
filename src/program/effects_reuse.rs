//! Exact dependency keys and deterministic dirty components for source effects.
//! A previous analysis is an optimization hint, never evidence for a different
//! table, body, callee, capture/storage resolution or initialization answer.
use super::*;

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct Stats {
    pub(super) visited: u64,
    pub(super) reused: u64,
    pub(super) components_reused: u64,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct AccessKey(Vec<u64>);

impl AccessKey {
    fn new(program: &Program<'_>, unit: UnitId, access: &ProgramInitialization) -> Self {
        let data = program.unit(unit).unwrap();
        let mut words = vec![0; data.operations.len().div_ceil(64)];
        for (index, operation) in data.operations.iter().enumerate() {
            if access_cell(data, operation).is_some_and(|cell| {
                access.initialized(program, unit, OpId::from_index(index).unwrap(), cell)
            }) {
                words[index / 64] |= 1 << (index % 64);
            }
        }
        Self(words)
    }

    pub(super) fn initialized(&self, operation: usize) -> bool {
        self.0[operation / 64] & (1 << (operation % 64)) != 0
    }
}

#[derive(Debug)]
pub(super) struct SummaryPass {
    pub(super) units: Vec<Fact<UnitEffects>>,
    pub(super) roots: Vec<Vec<Root>>,
    pub(super) statements: Vec<Vec<Effects>>,
    pub(super) access: Vec<AccessKey>,
}

pub(super) struct Previous<'a> {
    deps: &'a Deps,
    graph: &'a CallGraph,
    pass: &'a SummaryPass,
}

impl<'a> Previous<'a> {
    pub(super) fn new(deps: &'a Deps, graph: &'a CallGraph, pass: &'a SummaryPass) -> Self {
        Self { deps, graph, pass }
    }

    fn matches(
        &self,
        program: &Program<'_>,
        graph: &CallGraph,
        component: &[UnitId],
        current: &SummaryPass,
    ) -> bool {
        if self.deps.tables != program.tables_revision
            || component.is_empty()
            || self.graph.component_of(component[0]) != component
        {
            return false;
        }
        component.iter().all(|&unit| {
            let index = unit.index();
            self.deps.units.get(index) == Some(&(unit, program.units[index].revision()))
                && graph.same_effect_calls(self.graph, unit)
                && self.pass.access.get(index) == current.access.get(index)
                && graph.calls_from(unit).iter().all(|edge| {
                    if graph.same_component(unit, edge.callee) {
                        return true;
                    }
                    let callee = edge.callee.index();
                    // The transfer also reads the callee's parameter shape;
                    // equality of its effect bits alone cannot establish it.
                    self.deps.units.get(callee)
                        == Some(&(edge.callee, program.units[callee].revision()))
                        && same_summary(&self.pass.units[callee], &current.units[callee])
                })
        })
    }
}

fn same_summary(left: &Fact<UnitEffects>, right: &Fact<UnitEffects>) -> bool {
    match (left, right) {
        (Fact::Known(left, _), Fact::Known(right, _)) => left == right,
        (Fact::Unknown(left), Fact::Unknown(right)) => left == right,
        (Fact::Truncated(left), Fact::Truncated(right)) => left == right,
        _ => false,
    }
}

pub(super) fn summary_deps(program: &Program<'_>, graph: &CallGraph, unit: UnitId) -> Deps {
    Deps {
        tables: program.tables_revision,
        units: std::iter::once(unit)
            .chain(graph.calls_from(unit).iter().map(|edge| edge.callee))
            .map(|unit| (unit, program.units[unit.index()].revision()))
            .collect(),
    }
}

/// Callees precede callers. Reusing a whole SCC preserves the recursive least
/// fixed point; individual members are never seeded from a previous fixed point.
pub(super) fn summarize_units(
    program: &Program<'_>,
    graph: &CallGraph,
    access: &ProgramInitialization,
    record: bool,
    enabled: bool,
    first: Option<Previous<'_>>,
    second: Option<Previous<'_>>,
    stats: &mut Stats,
) -> SummaryPass {
    let count = program.units.len();
    let mut declared = vec![false; count];
    for cell in program.cells.iter() {
        if let CellBinding::Function(unit) = cell.binding {
            declared[unit.index()] |= cell.declared_pure;
        }
    }
    let mut pass = SummaryPass {
        units: (0..count)
            .map(|_| Fact::Truncated(Limit::Iterations))
            .collect(),
        roots: vec![Vec::new(); count],
        statements: vec![Vec::new(); count],
        access: program
            .units
            .iter()
            .map(|unit| AccessKey::new(program, unit.id(), access))
            .collect(),
    };
    let mut dirty = vec![false; count];
    for component in graph.components() {
        let reused = first
            .as_ref()
            .filter(|old| old.matches(program, graph, component, &pass))
            .or_else(|| {
                second
                    .as_ref()
                    .filter(|old| old.matches(program, graph, component, &pass))
            });
        if let Some(old) = reused.filter(|_| enabled) {
            let _timing = crate::timing::SOURCE_EFFECT_REUSE.scope(0);
            stats.components_reused += 1;
            stats.reused += component.len() as u64;
            for &unit in component {
                let index = unit.index();
                pass.units[index] = match &old.pass.units[index] {
                    Fact::Known(summary, _) => {
                        Fact::Known(summary.clone(), summary_deps(program, graph, unit))
                    }
                    other => other.clone(),
                };
                pass.roots[index] = old.pass.roots[index].clone();
                if record {
                    pass.statements[index] = old.pass.statements[index].clone();
                }
            }
            continue;
        }
        let recursive = component.first().is_some_and(|&unit| graph.recursive(unit));
        for &unit in component {
            let index = unit.index();
            dirty[index] = true;
            pass.units[index] = Fact::Known(
                UnitEffects {
                    effects: Effects {
                        may_diverge: recursive,
                        ..Effects::NONE
                    },
                    result_primitive: Some(ParameterSet::EMPTY),
                    declared_pure: declared[index],
                },
                Deps {
                    tables: program.tables_revision,
                    units: Vec::new(),
                },
            );
        }
        let mut settled = false;
        for _ in 0..if recursive { COMPONENT_ITERATIONS } else { 1 } {
            let mut changed = false;
            for &unit in component {
                let index = unit.index();
                if !std::mem::replace(&mut dirty[index], false) && enabled {
                    stats.reused += 1;
                    continue;
                }
                let _timing = crate::timing::SOURCE_EFFECT_UNIT.scope(0);
                stats.visited += 1;
                let operations = (record
                    && program.units[index].data().kind == UnitKind::ModuleInitialization)
                    .then(|| {
                        pass.statements[index].clear();
                        &mut pass.statements[index]
                    });
                let (summary, roots) = summarize(
                    program,
                    graph,
                    &pass.units,
                    unit,
                    declared[index],
                    &pass.access[index],
                    operations,
                );
                let summary = match summary {
                    Fact::Known(mut summary, deps) => {
                        summary.effects.may_diverge |= recursive;
                        Fact::Known(summary, deps)
                    }
                    other => other,
                };
                if pass.units[index] != summary {
                    changed = true;
                    for caller in graph.callers_of(unit) {
                        if graph.same_component(unit, caller.caller) {
                            dirty[caller.caller.index()] = true;
                        }
                    }
                }
                pass.units[index] = summary;
                pass.roots[index] = roots;
            }
            // Preserve the original round ceiling and truncation behavior.
            if !changed || !recursive {
                settled = true;
                break;
            }
        }
        if !settled {
            for &unit in component {
                pass.units[unit.index()] = Fact::Truncated(Limit::Iterations);
            }
        }
    }
    pass
}
