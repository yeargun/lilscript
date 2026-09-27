//! The program's entries as delivery reads them (plan M3.3, design §5-§6).
//!
//! A program has one module graph and several roots: its entries, in name
//! order. The canonical schedule is the post-order over the roots in that
//! order; each entry also has its own fresh order, what ES modules evaluate
//! when it is loaded first. Placement reads this projection, never the
//! Program (L2).
use super::*;
use crate::js::delivery::{EntryGraph, EntrySet};

/// The entry graph of `program` for its delivery. `carried` names the host
/// modules the output carries: their code runs for the entries that reach
/// a module importing one.
pub(super) fn entry_graph(
    program: &Program<'_>,
    carried: &[String],
    paths: &[String],
) -> EntryGraph {
    let modules = program.modules();
    let count = modules.len();
    let imports = modules
        .iter()
        .map(|module| {
            module
                .dependencies
                .iter()
                .map(|id| id.index() as u32)
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let graph = imports
        .iter()
        .map(|targets| targets.iter().map(|&t| t as usize).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    // Canonical positions, from the initialization schedule.
    let mut position = vec![u32::MAX; count];
    for (index, &initializer) in program.initialization().iter().enumerate() {
        if let Some(data) = program.unit(initializer) {
            position[data.module.index()] = index as u32;
        }
    }
    let entries = program
        .entries()
        .iter()
        .map(|entry| (entry.name.clone(), entry.module.index() as u32))
        .collect::<Vec<_>>();
    let roots = entries
        .iter()
        .map(|&(_, module)| module as usize)
        .collect::<Vec<_>>();
    let mut orders = crate::module::fresh_orders(&roots, &graph);
    // A module an entry can load with `import()` without reaching it
    // statically is a dynamic entry (design §5.2): loading it evaluates what
    // that entry has not, so it gets its own file. With one entry this is
    // every module only `import()` reaches; with several, a module another
    // entry imports statically can still be lazy for this one.
    let dynamic_graph = modules
        .iter()
        .map(|module| {
            module
                .dynamic_dependencies
                .iter()
                .map(|id| id.index())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut lazy = vec![false; count];
    for module in crate::module::lazy_roots(&roots, &graph, &dynamic_graph) {
        lazy[module] = true;
    }
    // By first load in the schedule.
    let mut by_position = (0..count).collect::<Vec<_>>();
    by_position.sort_unstable_by_key(|&module| position[module]);
    let mut dynamic: Vec<u32> = Vec::new();
    for &module in &by_position {
        for &target in &dynamic_graph[module] {
            if lazy[target] && !dynamic.contains(&(target as u32)) {
                dynamic.push(target as u32);
            }
        }
    }
    let dynamic_roots = dynamic
        .iter()
        .map(|&module| module as usize)
        .collect::<Vec<_>>();
    orders.extend(crate::module::fresh_orders(&dynamic_roots, &graph));
    let mut reach = vec![EntrySet::default(); count];
    for (bit, order) in orders.iter().enumerate() {
        for &module in order {
            reach[module].insert(bit);
        }
    }
    let host_importers = modules
        .iter()
        .enumerate()
        .filter(|(_, module)| {
            module
                .foreign_imports
                .iter()
                .any(|import| carried.iter().any(|carried| *carried == import.source))
        })
        .map(|(index, _)| index as u32)
        .collect();
    EntryGraph {
        entries,
        dynamic,
        imports,
        position,
        orders: orders
            .into_iter()
            .map(|order| order.into_iter().map(|module| module as u32).collect())
            .collect(),
        reach,
        cycles: crate::module::static_cycles(&graph),
        host_importers,
        paths: (0..count)
            .map(|module| {
                paths
                    .get(module)
                    .cloned()
                    .unwrap_or_else(|| format!("m{module}"))
            })
            .collect(),
    }
}
