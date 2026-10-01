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
    carried: &[String],
    paths: &[String],
    budget: &mut AllocationBudget<'_>,
    inspect: impl FnOnce(&EntryGraph, &mut AllocationBudget<'_>) -> Result<R, E>,
) -> Result<R, E> {
    use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
    let before = budget.retained_bytes(Retained);
    let graph = entry_graph(program, carried, paths, budget)?;
    let bytes = budget.retained_bytes(Retained) - before;
    let outcome = catch_unwind(AssertUnwindSafe(|| inspect(&graph, budget)));
    drop(graph);
    budget.release(Retained, bytes)?;
    match outcome {
        Ok(result) => result,
        Err(payload) => resume_unwind(payload),
    }
}

fn entry_graph(
    program: &Program<'_>,
    carried: &[String],
    paths: &[String],
    budget: &mut AllocationBudget<'_>,
) -> Result<EntryGraph, AllocationError> {
    let mut phase = budget.scope();
    let modules = program.modules();
    let count = modules.len();
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
        for id in &module.dependencies {
            targets.push(id.index() as u32);
            edges.push(id.index());
        }
        imports.push(targets);
        graph.push(edges);
        let mut dynamic = phase.vector(Scratch, module.dynamic_dependencies.len())?;
        dynamic.extend(module.dynamic_dependencies.iter().map(|id| id.index()));
        dynamic_graph.push(dynamic);
    }
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
    let mut lazy = phase.filled(Scratch, count, false)?;
    for module in crate::module::lazy_roots_from_orders_admitted(
        &roots,
        &orders,
        &graph,
        &dynamic_graph,
        &mut phase,
    )? {
        lazy[module] = true;
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
    let mut host_importers = phase.vector(Retained, count)?;
    for (index, module) in modules.iter().enumerate() {
        let mut carried_here = false;
        for import in &module.foreign_imports {
            for name in carried {
                phase.work(Analysis, name.len().min(import.source.len()) as u64 + 1)?;
                if *name == import.source {
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
