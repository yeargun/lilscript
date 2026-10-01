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
    fn new(
        program: &Program<'_>,
        unit: UnitId,
        access: &ProgramInitialization,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let data = program.unit(unit).unwrap();
        let mut words = budget.filled(Retained, data.operations.len().div_ceil(64), 0)?;
        for (index, operation) in data.operations.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if access_cell(data, operation).is_some_and(|cell| {
                access.initialized(program, unit, OpId::from_index(index).unwrap(), cell)
            }) {
                words[index / 64] |= 1 << (index % 64);
            }
        }
        Ok(Self(words))
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
    record: bool,
    admissions: Vec<Option<crate::admission_replay::Receipt>>,
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
        if (current.record && !self.pass.record)
            || self.deps.tables != program.tables_revision
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

pub(super) fn summary_deps(
    program: &Program<'_>,
    graph: &CallGraph,
    unit: UnitId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Deps, AllocationError> {
    Ok(Deps {
        tables: program.tables_revision,
        units: storage::collect(
            std::iter::once(unit)
                .chain(graph.calls_from(unit).iter().map(|edge| edge.callee))
                .map(|unit| (unit, program.units[unit.index()].revision())),
            Retained,
            budget,
        )?,
    })
}

fn release_summary(
    summary: Fact<UnitEffects>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    if let Fact::Known(_, deps) = summary {
        storage::release(deps.units, Retained, budget)?;
    }
    Ok(())
}

/// Callees precede callers. Every completed component and stable recursive
/// member has a deterministic admission stage, in both physical reuse modes.
/// This preserves logical limits while skipping only proved physical work.
pub(super) fn summarize_units(
    program: &Program<'_>,
    graph: &CallGraph,
    access: &ProgramInitialization,
    record: bool,
    enabled: bool,
    first: Option<Previous<'_>>,
    second: Option<Previous<'_>>,
    stats: &mut Stats,
    budget: &mut AllocationBudget<'_>,
) -> Result<SummaryPass, AllocationError> {
    budget.retained_phase(|budget| {
        let count = program.units.len();
        let mut declared = budget.filled(Scratch, count, false)?;
        for cell in program.cells.iter() {
            budget.work(WorkKind::Analysis, 1)?;
            if let CellBinding::Function(unit) = cell.binding {
                declared[unit.index()] |= cell.declared_pure;
            }
        }
        let mut keys = budget.vector(Retained, count)?;
        for unit in &program.units {
            let key = AccessKey::new(program, unit.id(), access, budget)?;
            budget.push(Retained, &mut keys, key)?;
        }
        let mut pass = SummaryPass {
            units: storage::collect(
                (0..count).map(|_| Fact::Truncated(Limit::Iterations)),
                Retained,
                budget,
            )?,
            roots: storage::collect((0..count).map(|_| Vec::new()), Retained, budget)?,
            statements: storage::collect((0..count).map(|_| Vec::new()), Retained, budget)?,
            access: keys,
            record,
            admissions: budget.filled(Retained, graph.components().len(), None)?,
        };
        let mut dirty = budget.filled(Scratch, count, false)?;
        let mut members = budget.filled(Scratch, count, None)?;
        for (component_index, component) in graph.components().iter().enumerate() {
            let previous = first
                .as_ref()
                .filter(|old| old.matches(program, graph, component, &pass))
                .or_else(|| {
                    second
                        .as_ref()
                        .filter(|old| old.matches(program, graph, component, &pass))
                });
            if let Some(old) = previous {
                let mut admission = old.pass.admissions[old.graph.component_index(component[0])];
                // A complete pass can borrow structural summaries while omitting
                // their per-statement output. Preserve the conservative cold
                // work/peak stage, but release backing this result does not own.
                if !record && old.pass.record {
                    let omitted = component.iter().try_fold(0u64, |total, unit| {
                        total
                            .checked_add(crate::output_budget::vector_bytes(
                                &old.pass.statements[unit.index()],
                            )?)
                            .ok_or(AllocationError::Capacity)
                    })?;
                    admission = admission.and_then(|stage| stage.without_retained(omitted));
                }
                if admission.is_some() || !budget.is_accounted() {
                    let build = || {
                        if enabled {
                            let _timing = crate::timing::SOURCE_EFFECT_REUSE.scope(0);
                            stats.components_reused += 1;
                            stats.reused += component.len() as u64;
                            for &unit in component {
                                let index = unit.index();
                                pass.units[index] = old.pass.units[index].clone();
                                pass.roots[index] = old.pass.roots[index].clone();
                                if record {
                                    pass.statements[index] = old.pass.statements[index].clone();
                                }
                            }
                            Ok(())
                        } else {
                            solve_component(
                                program,
                                graph,
                                component,
                                &mut pass,
                                &declared,
                                &mut dirty,
                                &mut members,
                                false,
                                stats,
                                &mut AllocationBudget::new(None),
                            )
                        }
                    };
                    if let Some(stage) = admission {
                        budget.replay(&stage, build)?;
                    } else {
                        let mut build = build;
                        build()?;
                    }
                    pass.admissions[component_index] = admission;
                    continue;
                }
            }
            let (_, admission) = budget.record(|budget| {
                solve_component(
                    program,
                    graph,
                    component,
                    &mut pass,
                    &declared,
                    &mut dirty,
                    &mut members,
                    enabled,
                    stats,
                    budget,
                )
            })?;
            pass.admissions[component_index] = admission;
        }
        Ok(pass)
    })
}

#[allow(clippy::too_many_arguments)]
fn solve_component(
    program: &Program<'_>,
    graph: &CallGraph,
    component: &[UnitId],
    pass: &mut SummaryPass,
    declared: &[bool],
    dirty: &mut [bool],
    members: &mut [Option<crate::admission_replay::Receipt>],
    enabled: bool,
    stats: &mut Stats,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let recursive = component.first().is_some_and(|&unit| graph.recursive(unit));
    for &unit in component {
        let index = unit.index();
        dirty[index] = true;
        members[index] = None;
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
            let changed_input = std::mem::replace(&mut dirty[index], false);
            let record =
                pass.record && program.units[index].data().kind == UnitKind::ModuleInitialization;
            let output = if !changed_input && members[index].is_some() {
                let stage = members[index].unwrap();
                let result = budget.replay(&stage, || {
                    if enabled {
                        Ok(None)
                    } else {
                        let _timing = crate::timing::SOURCE_EFFECT_UNIT.scope(0);
                        summarize(
                            program,
                            graph,
                            &pass.units,
                            unit,
                            declared[index],
                            &pass.access[index],
                            record,
                            &mut AllocationBudget::new(None),
                        )
                        .map(Some)
                    }
                })?;
                if result.is_none() {
                    budget.release(Retained, stage.live_bytes())?;
                    stats.reused += 1;
                    continue;
                }
                stats.visited += 1;
                result.unwrap()
            } else if !changed_input && enabled && !budget.is_accounted() {
                stats.reused += 1;
                continue;
            } else {
                let _timing = crate::timing::SOURCE_EFFECT_UNIT.scope(0);
                stats.visited += 1;
                let (result, admission) = budget.record(|budget| {
                    summarize(
                        program,
                        graph,
                        &pass.units,
                        unit,
                        declared[index],
                        &pass.access[index],
                        record,
                        budget,
                    )
                })?;
                members[index] = admission;
                result
            };
            let (mut summary, roots, statements) = output;
            if let Fact::Known(summary, _) = &mut summary {
                summary.effects.may_diverge |= recursive;
            }
            if pass.units[index] != summary {
                changed = true;
                for caller in graph.callers_of(unit) {
                    if graph.same_component(unit, caller.caller) {
                        dirty[caller.caller.index()] = true;
                    }
                }
            }
            release_summary(std::mem::replace(&mut pass.units[index], summary), budget)?;
            storage::release(
                std::mem::replace(&mut pass.roots[index], roots),
                Retained,
                budget,
            )?;
            storage::release(
                std::mem::replace(&mut pass.statements[index], statements),
                Retained,
                budget,
            )?;
        }
        if !changed || !recursive {
            settled = true;
            break;
        }
    }
    if !settled {
        for &unit in component {
            release_summary(
                std::mem::replace(
                    &mut pass.units[unit.index()],
                    Fact::Truncated(Limit::Iterations),
                ),
                budget,
            )?;
        }
    }
    Ok(())
}
