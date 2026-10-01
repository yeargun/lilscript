//! One admitted projection of the checked module graph for delivery. Static
//! orders are computed once and reused to discover lazy roots. The projection
//! is immutable and released after placement; target edits keep their owner.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use crate::js::delivery::{EntryGraph, EntrySet};
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

pub(super) fn with_entry_graph<R, E: From<AllocationError>>(
    program: &Program<'_>,
    hosts: Option<&crate::host_modules::HostDelivery>,
    integrated_hosts: bool,
    paths: &[String],
    budget: &mut AllocationBudget<'_>,
    inspect: impl FnOnce(&EntryGraph, &mut AllocationBudget<'_>) -> Result<R, E>,
) -> Result<R, E> {
    budget.with_temporary(
        |budget| entry_graph(program, hosts, integrated_hosts, paths, budget).map_err(E::from),
        |graph, budget| inspect(graph, budget),
    )
}

fn entry_graph(
    program: &Program<'_>,
    hosts: Option<&crate::host_modules::HostDelivery>,
    integrated_hosts: bool,
    paths: &[String],
    budget: &mut AllocationBudget<'_>,
) -> Result<EntryGraph, AllocationError> {
    let mut phase = budget.scope();
    let modules = program.modules();
    let integrated = hosts.filter(|_| integrated_hosts);
    let language_and_hosts = modules.len().checked_add(integrated.map_or(0, |hosts| hosts.modules.len())).ok_or(AllocationError::Capacity)?;
    let mut foreign: Vec<(String, u32)> = Vec::new();
    for module in modules {
        for import in &module.foreign_imports {
            phase.work(Analysis, foreign.len() as u64 + 1)?;
            if integrated.is_some_and(|hosts| hosts.position(&import.source).is_some()) || foreign.iter().any(|(source, _)| source == &import.source) { continue; }
            let owner = u32::try_from(language_and_hosts + foreign.len()).map_err(|_| AllocationError::Capacity)?;
            let source = phase.string(Retained, &import.source)?;
            phase.push(Retained, &mut foreign, (source, owner))?;
        }
    }
    let count = language_and_hosts.checked_add(foreign.len()).ok_or(AllocationError::Capacity)?;
    if count > u32::MAX as usize / 2 { return Err(AllocationError::Capacity); }
    let mut imports = phase.vector(Retained, count)?;
    let mut graph = phase.vector(Scratch, count)?;
    let mut dynamic_graph = phase.vector(Scratch, count)?;
    for module in modules {
        phase.work(
            Analysis,
            (module.dependencies.len() + module.dynamic_dependencies.len()) as u64,
        )?;
        let mut targets = phase.vector(Retained, module.dependencies.len())?;
        let mut edges = phase.vector(Scratch, module.dependencies.len())?;
        let mut ordered = phase.vector(Scratch, module.dependencies.len() + module.foreign_imports.len())?;
        ordered.extend(module.dependencies.iter().zip(&module.dependency_spans).map(|(id, span)| (span.start, id.index())));
        if let Some(hosts) = integrated {
            for import in &module.foreign_imports {
                phase.work(Analysis, hosts.modules.len() as u64)?;
                if let Some(index) = hosts.position(&import.source) { ordered.push((import.span.start, modules.len() + index)); }
            }
        }
        for import in &module.foreign_imports {
            if let Some((_, owner)) = foreign.iter().find(|(source, _)| source == &import.source) { ordered.push((import.span.start, *owner as usize)); }
        }
        phase.work(Analysis, (ordered.len() as u64).saturating_mul(usize::BITS as u64))?;
        ordered.sort_unstable(); ordered.dedup();
        for &(_, id) in &ordered {
            phase.push(Retained, &mut targets, id as u32)?;
            phase.push(Scratch, &mut edges, id)?;
        }
        drop(ordered);
        imports.push(targets);
        graph.push(edges);
        let mut dynamic = phase.vector(Scratch, module.dynamic_dependencies.len())?;
        dynamic.extend(module.dynamic_dependencies.iter().map(|id| id.index()));
        dynamic_graph.push(dynamic);
    }
    if let Some(hosts) = integrated {
        for host in &hosts.modules {
            let mut edges = phase.vector(Scratch, host.imports().len())?;
            let mut targets = phase.vector(Retained, host.imports().len())?;
            for &(dependency, _) in host.imports() {
                edges.push(modules.len() + dependency);
                targets.push((modules.len() + dependency) as u32);
            }
            graph.push(edges); imports.push(targets); dynamic_graph.push(Vec::new());
        }
    }
    for _ in &foreign { graph.push(Vec::new()); imports.push(Vec::new()); dynamic_graph.push(Vec::new()); }
    let mut position = phase.filled(Retained, count, u32::MAX)?;
    phase.work(Analysis, program.initialization().len() as u64)?;
    for (index, &initializer) in program.initialization().iter().enumerate() {
        if let Some(data) = program.unit(initializer) {
            position[data.module.index()] = index as u32;
        }
    }
    let mut entries = phase.vector(Retained, program.entries().len())?;
    let mut roots = phase.vector(Scratch, program.entries().len())?;
    for entry in program.entries() {
        entries.push((
            phase.string(Retained, &entry.name)?,
            entry.module.index() as u32,
        ));
        roots.push(entry.module.index());
    }
    let mut orders = crate::module::fresh_orders_admitted(&roots, &graph, &mut phase)?;
    // Every dynamic request owns a cached namespace, including a module an
    // entry already initialized. Snapshot objects would lose live bindings and
    // repeated-import identity even when no new effects need to execute.
    let mut lazy = phase.filled(Scratch, count, false)?;
    for targets in &dynamic_graph {
        for &target in targets { phase.work(Analysis, 1)?; lazy[target] = true; }
    }
    let mut by_position = phase.vector(Scratch, count)?;
    by_position.extend(0..count);
    phase.work(
        Analysis,
        (count as u64).saturating_mul(u64::from(usize::BITS - count.leading_zeros()) + 1),
    )?;
    by_position.sort_unstable_by_key(|&module| position[module]);
    let mut dynamic = phase.vector(Retained, count)?;
    let mut dynamic_roots = phase.vector(Scratch, count)?;
    for &module in &by_position {
        for &target in &dynamic_graph[module] {
            phase.work(Analysis, 1)?;
            if std::mem::replace(&mut lazy[target], false) {
                dynamic.push(target as u32);
                dynamic_roots.push(target);
            }
        }
    }
    let dynamic_orders = crate::module::fresh_orders_admitted(&dynamic_roots, &graph, &mut phase)?;
    phase.reserve_vec(Scratch, &mut orders, dynamic_orders.len())?;
    orders.extend(dynamic_orders);
    let mut reach = phase.vector(Retained, count)?;
    reach.resize_with(count, EntrySet::default);
    let mut output_orders = phase.vector(Retained, orders.len())?;
    for (bit, order) in orders.iter().enumerate() {
        let mut output = phase.vector(Retained, order.len())?;
        for &module in order {
            phase.work(Analysis, 1)?;
            reach[module].insert_admitted(bit, Retained, &mut phase)?;
            output.push(module as u32);
        }
        output_orders.push(output);
    }
    if integrated_hosts || !foreign.is_empty() {
        position.fill(u32::MAX);
        let mut next = 0u32;
        for order in &orders {
            for &module in order {
                if position[module] == u32::MAX { position[module] = next; next += 1; }
            }
        }
    }
    let mut host_importers = phase.vector(Retained, count)?;
    for (index, module) in modules.iter().enumerate() {
        let mut carried_here = false;
        for import in &module.foreign_imports {
            for name in hosts.into_iter().flat_map(|hosts| hosts.modules.iter().map(|host| host.specifier.as_str())).filter(|_| !integrated_hosts) {
                phase.work(Analysis, name.len().min(import.source.len()) as u64 + 1)?;
                if name == import.source {
                    carried_here = true;
                    break;
                }
            }
            if carried_here {
                break;
            }
        }
        if carried_here {
            host_importers.push(index as u32);
        }
    }
    let cycles = crate::module::static_cycles_admitted(&graph, &mut phase)?;
    phase.promote(crate::output_budget::vector_bytes(&cycles)?)?;
    let mut output_paths = phase.vector(Retained, count)?;
    for module in 0..count {
        let path = match paths.get(module) {
            Some(path) => phase.string(Retained, path)?,
            None if module >= language_and_hosts => {
                let mut path = String::new();
                phase.write_fmt(Retained, &mut path, format_args!("extern{}", module - language_and_hosts))?;
                path
            },
            None if module >= modules.len() => phase.string(Retained, &integrated.unwrap().modules[module - modules.len()].stem)?,
            None => {
                // A usize needs at most 20 decimal digits. Admit before format.
                let mut path = phase.vector(Retained, 21)?;
                use std::io::Write;
                write!(&mut path, "m{module}").expect("preallocated vector writer");
                String::from_utf8(path).expect("ASCII module name")
            }
        };
        output_paths.push(path);
    }
    let result = EntryGraph {
        entries,
        dynamic,
        imports,
        position,
        orders: output_orders,
        reach,
        cycles,
        host_importers,
        foreign,
        paths: output_paths,
    };
    // Do not let scratch buffers outlive their reservations.
    drop((
        graph,
        dynamic_graph,
        roots,
        orders,
        lazy,
        by_position,
        dynamic_roots,
    ));
    phase.finish_retained()?;
    Ok(result)
}
