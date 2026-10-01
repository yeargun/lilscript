//! Admitted postorder planning followed by an in-place arena permutation.
//! Nested expression payloads move with their nodes; no string, argument list
//! or class body is cloned just to restore the arena's topological order.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use crate::output_budget::AllocationClass::{Retained, Scratch};

struct Plan {
    map: Vec<Option<ExprId>>,
    reached: Vec<bool>,
    permutation: Vec<usize>,
    live: usize,
}

impl Plan {
    fn new(module: &Module, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        let mut phase = budget.scope();
        let mut map = phase.filled(Retained, module.expressions.len(), None)?;
        let mut reached = phase.filled(Retained, module.regions.len(), false)?;
        let mut permutation = phase.vector(Retained, module.expressions.len())?;
        let mut regions = phase.vector(Scratch, 1)?;
        regions.push(module.root);
        let mut pending = phase.vector(Scratch, 0)?;
        let mut live = 0;
        while let Some(region) = regions.pop() {
            phase.work(Analysis, 1)?;
            if std::mem::replace(&mut reached[region.index()], true) {
                continue;
            }
            for statement in &module.regions[region.index()].statements {
                phase.work(Analysis, 1)?;
                // A statement has at most two expression roots and three
                // child regions. Reserve before the infallible visitors.
                phase.reserve_vec(Scratch, &mut pending, 2)?;
                phase.reserve_vec(Scratch, &mut regions, 4)?;
                statement.visit_expressions(|root| pending.push((root, false)));
                statement.visit_regions(|child| regions.push(child));
                if let Statement::Function { function, .. } = statement {
                    regions.push(module.functions[function.index()].body);
                }
                while let Some((id, expanded)) = pending.pop() {
                    phase.work(Analysis, 1)?;
                    if map[id.index()].is_some() {
                        continue;
                    }
                    if !expanded {
                        phase.push(Scratch, &mut pending, (id, true))?;
                        let expression = &module.expressions[id.index()];
                        for function in expression.created_functions() {
                            phase.push(
                                Scratch,
                                &mut regions,
                                module.functions[function.index()].body,
                            )?;
                        }
                        expression.visit_children(|child| {
                            if map[child.index()].is_none() {
                                phase.push(Scratch, &mut pending, (child, false))?;
                            }
                            Ok::<_, AllocationError>(())
                        })?;
                    } else {
                        map[id.index()] =
                            Some(ExprId::try_new(live).ok_or(AllocationError::Capacity)?);
                        live += 1;
                    }
                }
            }
        }
        // Give dead nodes distinct destinations after the live prefix. The
        // resulting permutation can be consumed by swaps without a second
        // expression arena or any nested allocation.
        let mut dead = live;
        phase.work(
            Analysis,
            (map.len() + module.behaviours.len() + module.observed_literals.len()) as u64,
        )?;
        for destination in &map {
            permutation.push(destination.map_or_else(
                || {
                    let slot = dead;
                    dead += 1;
                    slot
                },
                ExprId::index,
            ));
        }
        // Charge the bounded mutation pass before the first mutation. Its
        // expression edges were counted by the traversal above.
        phase.work(Analysis, (map.len() * 2 + module.regions.len()) as u64)?;
        drop((regions, pending));
        phase.finish_retained()?;
        Ok(Self {
            map,
            reached,
            permutation,
            live,
        })
    }

    fn apply(
        &mut self,
        module: &mut Module,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        // This is the last allocation/work admission. Refusal preserves the tree
        // and journal; the remaining moves only release existing reservations.
        module.journal.renumber(&self.map, budget)?;
        for (index, expression) in module.expressions.iter_mut().enumerate() {
            if self.map[index].is_some() {
                expression.remap_children(|child| self.map[child.index()].expect("a live operand"));
            }
        }
        for index in 0..self.permutation.len() {
            while self.permutation[index] != index {
                let target = self.permutation[index];
                module.expressions.swap(index, target);
                module.origins.swap(index, target);
                if !module.authored_expressions.is_empty() {
                    module.authored_expressions.swap(index, target);
                }
                if !module.spelling_nodes.is_empty() {
                    module.spelling_nodes.swap(index, target);
                }
                self.permutation.swap(index, target);
            }
        }
        module.expressions.truncate(self.live);
        module.origins.truncate(self.live);
        module.authored_expressions.truncate(self.live);
        module.spelling_nodes.truncate(self.live);
        for (index, region) in module.regions.iter_mut().enumerate() {
            if !self.reached[index] {
                region.statements.clear();
                continue;
            }
            for statement in &mut region.statements {
                statement
                    .remap_expressions(|id| self.map[id.index()].expect("a live statement root"));
            }
        }
        module
            .observed_literals
            .retain_mut(|alternative| alternative.remap(&self.map));
        module
            .observed_literals
            .sort_unstable_by_key(|alternative| alternative.expression());
        let mut released = 0u64;
        module.behaviours.retain_mut(|row| {
            let Some(expression) = self.map[row.expression.index()] else {
                released += row.node.payload_bytes().expect("admitted behavior payload");
                return false;
            };
            let mut whole = true;
            row.node
                .remap_children(|child| match self.map[child.index()] {
                    Some(child) => child,
                    None => {
                        whole = false;
                        child
                    }
                });
            row.expression = expression;
            if !whole { released += row.node.payload_bytes().expect("admitted behavior payload"); }
            whole
        });
        module.behaviours.sort_unstable_by_key(|row| row.expression);
        budget.release(Retained, released)?;
        Ok(())
    }
}

impl Module {
    /// Restore postorder and remove unreachable nodes. Arena backing stays
    /// with its current owner, so repeated renumbering does not duplicate it.
    pub(crate) fn renumber(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        use std::panic::{catch_unwind, resume_unwind, AssertUnwindSafe};
        let before = budget.retained_bytes(Retained);
        let mut plan = Plan::new(self, budget)?;
        let bytes = budget.retained_bytes(Retained) - before;
        let result = catch_unwind(AssertUnwindSafe(|| plan.apply(self, budget)));
        drop(plan);
        budget.release(Retained, bytes)?;
        match result {
            Ok(result) => result,
            Err(payload) => resume_unwind(payload),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};

    fn fixture() -> Module {
        let mut module = Module::default();
        module.expression(Expr::Regex("/unused/".into()), None);
        let callee = module.expression(Expr::Host("capture".into()), None);
        let placeholder = module.expression(Expr::Literal(Literal::Null), None);
        let call = module.expression(
            Expr::Call {
                callee,
                arguments: vec![placeholder],
                invocation: Invocation::Value,
            },
            None,
        );
        let text = module.expression(
            Expr::Literal(Literal::String("retained payload".repeat(20).into())),
            None,
        );
        let array = module.expression(Expr::Array(vec![text]), None);
        if let Expr::Call { arguments, .. } = &mut module.expressions[call.index()] {
            arguments[0] = array;
        }
        module.regions[0].statements.push(Statement::Evaluate(call));
        module
    }
    fn ledger(work: u64, bytes: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: work,
                optional_work: 0,
                baseline_retained_bytes: 0,
                retained_bytes: bytes,
            },
        )
        .unwrap()
    }

    #[test]
    fn q2_renumber_moves_payloads_and_preserves_backing_without_retaining_plans() {
        let mut module = fixture();
        let backing = module.expressions.as_ptr();
        let arguments = match &module.expressions[3] {
            Expr::Call { arguments, .. } => arguments.as_ptr(),
            _ => unreachable!(),
        };
        let elements = match &module.expressions[5] {
            Expr::Array(elements) => elements.as_ptr(),
            _ => unreachable!(),
        };
        let mut ledger = ledger(1_000_000, 1_000_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
            budget.retain(Retained, 64).unwrap(); // an unrelated owner's sentinel
            for _ in 0..100 {
                module.renumber(&mut budget).unwrap();
                assert_eq!(module.expressions.as_ptr(), backing);
                assert_eq!(module.expressions.len(), 4);
                assert_eq!(budget.retained_bytes(Retained), 64);
                assert_eq!(budget.retained_bytes(Scratch), 0);
            }
        }
        assert_eq!(ledger.retained_bytes(), 0);
        module.verify().unwrap();
        assert!(module.expressions.iter().any(|node| matches!(node, Expr::Call { arguments: found, .. } if found.as_ptr() == arguments)));
        assert!(module
            .expressions
            .iter()
            .any(|node| matches!(node, Expr::Array(found) if found.as_ptr() == elements)));
        let javascript = module.render(PrintPolicy::default()).unwrap();
        let output = std::process::Command::new("node")
            .args([
                "-e",
                &format!("function capture(v){{console.log(JSON.stringify(v))}};{javascript}"),
            ])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            serde_json::json!(["retained payload".repeat(20)]).to_string()
        );
    }

    #[test]
    fn q2_renumber_refusals_are_atomic_and_release_partial_plans() {
        let original = fixture();
        let mut refused = 0;
        let mut completed = 0;
        for work in [0, 5, 30, 60, 120, 1_000_000] {
            for bytes in [64, 100, 160, 256, 1_000_000] {
                let mut module = original.clone();
                let mut ledger = ledger(work, bytes);
                {
                    let mut budget =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                    budget.retain(Retained, 64).unwrap();
                    match module.renumber(&mut budget) {
                        Ok(()) => {
                            module.verify().unwrap();
                            completed += 1;
                        }
                        Err(_) => {
                            assert_eq!(module, original);
                            refused += 1;
                        }
                    }
                    assert_eq!(budget.retained_bytes(Retained), 64);
                    assert_eq!(budget.retained_bytes(Scratch), 0);
                }
                assert_eq!(ledger.retained_bytes(), 0);
            }
        }
        assert!(refused > 0 && completed > 0);
    }
}
