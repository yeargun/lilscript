//! Admitted module-graph operations shared by checking and delivery. Returned
//! vectors belong to the caller's scratch scope; traversal stacks are released
//! between roots. Iterative walks keep source depth off the native stack.
use crate::compilation_policy::WorkKind::Analysis;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};

pub fn fresh_orders(roots: &[usize], dependencies: &[Vec<usize>]) -> Vec<Vec<usize>> {
    fresh_orders_admitted(roots, dependencies, &mut AllocationBudget::new(None))
        .expect("module graph fits memory and has valid edges")
}

pub(crate) fn fresh_orders_admitted(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Vec<usize>>, AllocationError> {
    let mut orders = budget.vector(Scratch, roots.len())?;
    for &root in roots {
        let order = super::static_evaluation_order_admitted(
            root,
            dependencies.len(),
            |module| dependencies[module].iter().copied(),
            budget,
        )
        .map_err(|error| match error {
            super::StaticOrderError::Resources(error) => error,
            super::StaticOrderError::Invalid(_) => AllocationError::Capacity,
        })?;
        orders.push(order);
    }
    Ok(orders)
}

/// Cyclic strongly connected components, in deterministic Tarjan completion
/// order. The component's existing stack segment supplies its members.
pub fn static_cycles(dependencies: &[Vec<usize>]) -> Vec<Option<u32>> {
    static_cycles_admitted(dependencies, &mut AllocationBudget::new(None))
        .expect("module graph fits memory and has valid edges")
}

pub(crate) fn static_cycles_admitted(
    dependencies: &[Vec<usize>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<u32>>, AllocationError> {
    let count = dependencies.len();
    let mut cycles = budget.filled(Scratch, count, None)?;
    let mut phase = budget.scope();
    let mut index = phase.filled(Scratch, count, usize::MAX)?;
    let mut low = phase.filled(Scratch, count, 0usize)?;
    let mut on_stack = phase.filled(Scratch, count, false)?;
    let mut stack = phase.vector(Scratch, count)?;
    let mut frames = phase.vector(Scratch, count)?;
    let mut next_index = 0;
    let mut next_cycle = 0u32;
    for start in 0..count {
        phase.work(Analysis, 1)?;
        if index[start] != usize::MAX {
            continue;
        }
        frames.push((start, 0usize));
        index[start] = next_index;
        low[start] = next_index;
        next_index += 1;
        stack.push(start);
        on_stack[start] = true;
        while let Some(&mut (module, ref mut edge)) = frames.last_mut() {
            phase.work(Analysis, 1)?;
            if let Some(&child) = dependencies[module].get(*edge) {
                *edge += 1;
                if child >= count {
                    return Err(AllocationError::Capacity);
                }
                if index[child] == usize::MAX {
                    index[child] = next_index;
                    low[child] = next_index;
                    next_index += 1;
                    stack.push(child);
                    on_stack[child] = true;
                    frames.push((child, 0));
                } else if on_stack[child] {
                    low[module] = low[module].min(index[child]);
                }
                continue;
            }
            frames.pop();
            if let Some(&(parent, _)) = frames.last() {
                low[parent] = low[parent].min(low[module]);
            }
            if low[module] == index[module] {
                let mut begin = stack.len();
                loop {
                    phase.work(Analysis, 1)?;
                    begin -= 1;
                    if stack[begin] == module {
                        break;
                    }
                }
                phase.work(Analysis, dependencies[module].len() as u64)?;
                let cyclic = stack.len() - begin > 1 || dependencies[module].contains(&module);
                for &member in &stack[begin..] {
                    phase.work(Analysis, 1)?;
                    on_stack[member] = false;
                    if cyclic {
                        cycles[member] = Some(next_cycle);
                    }
                }
                stack.truncate(begin);
                if cyclic {
                    next_cycle = next_cycle.checked_add(1).ok_or(AllocationError::Capacity)?;
                }
            }
        }
    }
    Ok(cycles)
}

/// First entry into each cycle in a root's source-ordered depth-first walk.
pub fn cycle_entries(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    cycles: &[Option<u32>],
) -> Vec<Vec<(u32, usize)>> {
    cycle_entries_admitted(
        roots,
        dependencies,
        cycles,
        &mut AllocationBudget::new(None),
    )
    .expect("module graph fits memory and has valid edges")
}

pub(crate) fn cycle_entries_admitted(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    cycles: &[Option<u32>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Vec<(u32, usize)>>, AllocationError> {
    let count = dependencies.len();
    let mut result = budget.vector(Scratch, roots.len())?;
    for &root in roots {
        if root >= count || cycles.len() != count {
            return Err(AllocationError::Capacity);
        }
        let mut entered = budget.vector(Scratch, count)?;
        let mut phase = budget.scope();
        let mut seen = phase.filled(Scratch, count, false)?;
        let mut known = phase.filled(Scratch, count, false)?;
        let mut stack = phase.vector(Scratch, count)?;
        seen[root] = true;
        stack.push((root, 0usize));
        if let Some(cycle) = cycles[root] {
            if cycle as usize >= count {
                return Err(AllocationError::Capacity);
            }
            known[cycle as usize] = true;
            entered.push((cycle, root));
        }
        while let Some((module, next)) = stack.last_mut() {
            phase.work(Analysis, 1)?;
            let module = *module;
            if let Some(&child) = dependencies[module].get(*next) {
                *next += 1;
                if child >= count {
                    return Err(AllocationError::Capacity);
                }
                if !std::mem::replace(&mut seen[child], true) {
                    if let Some(cycle) = cycles[child] {
                        if cycle as usize >= count {
                            return Err(AllocationError::Capacity);
                        }
                        if !std::mem::replace(&mut known[cycle as usize], true) {
                            entered.push((cycle, child));
                        }
                    }
                    stack.push((child, 0));
                }
            } else {
                stack.pop();
            }
        }
        result.push(entered);
    }
    Ok(result)
}

pub fn lazy_roots(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    dynamic: &[Vec<usize>],
) -> Vec<usize> {
    lazy_roots_admitted(
        roots,
        dependencies,
        dynamic,
        &mut AllocationBudget::new(None),
    )
    .expect("module graph fits memory and has valid edges")
}

pub(crate) fn lazy_roots_admitted(
    roots: &[usize],
    dependencies: &[Vec<usize>],
    dynamic: &[Vec<usize>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, AllocationError> {
    let mut result = budget.vector(Scratch, dependencies.len())?;
    let mut phase = budget.scope();
    let orders = fresh_orders_admitted(roots, dependencies, &mut phase)?;
    result.extend(lazy_roots_from_orders_admitted(
        roots,
        &orders,
        dependencies,
        dynamic,
        &mut phase,
    )?);
    Ok(result)
}

/// Delivery already owns fresh orders. Reuse them without a second static DFS.
pub(crate) fn lazy_roots_from_orders_admitted(
    roots: &[usize],
    orders: &[Vec<usize>],
    dependencies: &[Vec<usize>],
    dynamic: &[Vec<usize>],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<usize>, AllocationError> {
    let count = dependencies.len();
    let mut result = budget.vector(Scratch, count)?;
    let mut phase = budget.scope();
    let mut lazy = phase.filled(Scratch, count, false)?;
    let mut own = phase.filled(Scratch, count, false)?;
    let mut seen = phase.filled(Scratch, count, false)?;
    let mut pending = phase.vector(Scratch, count)?;
    if roots.len() != orders.len() {
        return Err(AllocationError::Capacity);
    }
    for (order, &root) in orders.iter().zip(roots) {
        phase.work(Analysis, count as u64 * 2 + order.len() as u64)?;
        own.fill(false);
        seen.fill(false);
        for &module in order {
            own[module] = true;
        }
        seen[root] = true;
        pending.push(root);
        while let Some(module) = pending.pop() {
            phase.work(Analysis, 1)?;
            for &target in &dependencies[module] {
                phase.work(Analysis, 1)?;
                if target >= count {
                    return Err(AllocationError::Capacity);
                }
                if !std::mem::replace(&mut seen[target], true) {
                    pending.push(target);
                }
            }
            for &target in dynamic.get(module).map_or(&[][..], Vec::as_slice) {
                phase.work(Analysis, 1)?;
                if target >= count {
                    return Err(AllocationError::Capacity);
                }
                if !own[target] {
                    lazy[target] = true;
                }
                if !std::mem::replace(&mut seen[target], true) {
                    pending.push(target);
                }
            }
        }
    }
    phase.work(Analysis, count as u64)?;
    result.extend((0..count).filter(|&module| lazy[module]));
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

    #[test]
    fn q2_graph_cycles_match_independent_transitive_closure() {
        for seed in 0..96u64 {
            let count = 12;
            let mut random = seed + 1;
            let mut graph = vec![Vec::new(); count];
            let mut closure = vec![vec![false; count]; count];
            for from in 0..count {
                for to in 0..count {
                    random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
                    if random >> 61 == 0 {
                        graph[from].push(to);
                        closure[from][to] = true;
                    }
                }
            }
            for through in 0..count {
                for from in 0..count {
                    for to in 0..count {
                        closure[from][to] |= closure[from][through] && closure[through][to];
                    }
                }
            }
            let cycles = static_cycles(&graph);
            for from in 0..count {
                assert_eq!(cycles[from].is_some(), closure[from][from], "seed {seed}");
                for to in 0..count {
                    assert_eq!(
                        cycles[from].is_some() && cycles[from] == cycles[to],
                        closure[from][to] && closure[to][from],
                        "seed {seed}, {from}, {to}"
                    );
                }
            }
        }
    }

    #[test]
    fn q2_graph_orders_cycles_and_lazy_roots_keep_source_order() {
        let graph = vec![vec![2], vec![3], vec![3], vec![2, 4], vec![4], vec![]];
        let dynamic = vec![vec![5], vec![5], vec![], vec![], vec![], vec![4]];
        assert_eq!(
            fresh_orders(&[0, 1], &graph),
            vec![vec![4, 3, 2, 0], vec![2, 4, 3, 1]]
        );
        let cycles = static_cycles(&graph);
        assert_eq!(
            cycle_entries(&[0, 1], &graph, &cycles),
            vec![
                vec![(cycles[2].unwrap(), 2), (cycles[4].unwrap(), 4)],
                vec![(cycles[3].unwrap(), 3), (cycles[4].unwrap(), 4)]
            ]
        );
        assert_eq!(lazy_roots(&[0, 1], &graph, &dynamic), vec![5]);
    }

    #[test]
    fn q2_graph_deep_walks_are_iterative_and_all_refusals_release_backing() {
        let count = 4096;
        let mut graph = (0..count)
            .map(|n| vec![(n + 1) % count])
            .collect::<Vec<_>>();
        let cycles = static_cycles(&graph);
        assert!(cycles.iter().all(|cycle| *cycle == Some(0)));
        graph[count - 1].clear();
        assert_eq!(fresh_orders(&[0], &graph)[0].len(), count);
        for bytes in [1, 64, 4096, 500_000] {
            let mut ledger = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 10_000_000,
                    optional_work: 0,
                    baseline_retained_bytes: 0,
                    retained_bytes: bytes,
                },
            )
            .unwrap();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                budget.retain(Scratch, 1).unwrap();
                {
                    let mut phase = budget.scope();
                    let result = static_cycles_admitted(&graph, &mut phase);
                    assert_eq!(result.is_ok(), bytes == 500_000);
                }
                assert_eq!(budget.retained_bytes(Scratch), 1);
            }
            assert_eq!(ledger.retained_bytes(), 0);
        }
    }
}
