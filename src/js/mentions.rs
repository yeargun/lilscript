//! Exact per-statement mention facts for single-use forwarding. The region
//! owner invalidates the receiver after each move and removes the old row.
//! Proofs about evaluation order, writes and scope remain with the rule.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;
use std::mem::size_of;

#[derive(Default)]
pub(super) struct StatementMentions {
    // Allocated only when a forwarding candidate reaches the later-statement
    // scan. Each child vector is separately admitted by the collector below.
    rows: Vec<Option<Vec<BindingId>>>,
}

impl StatementMentions {
    pub(super) fn contains(
        &mut self,
        module: &Module,
        region: usize,
        at: usize,
        binding: BindingId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if self.rows.is_empty() {
            let count = module.regions[region].statements.len();
            self.rows = budget.vector(AllocationClass::Scratch, count)?;
            budget.work(Analysis, count as u64)?;
            self.rows.resize_with(count, || None);
        }
        let statement = &module.regions[region].statements[at];
        if self.rows[at].is_none() {
            self.rows[at] = Some(module.statement_bindings(statement, budget)?);
        }
        let bindings = self.rows[at].as_ref().unwrap();
        budget.work(Analysis, bindings.len().max(1).ilog2() as u64 + 1)?;
        let found = bindings.binary_search(&binding).is_ok();
        #[cfg(test)]
        assert_eq!(
            found,
            module.statement_mentions(statement, binding),
            "indexed statement mentions differ from the independent scan"
        );
        Ok(found)
    }

    pub(super) fn moved(
        &mut self,
        removed: usize,
        receiver: usize,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        if self.rows.is_empty() {
            return Ok(());
        }
        // The receiver includes any nested branch into which the value moved.
        // Every other surviving statement still owns the same occurrences.
        discard(self.rows[receiver].take(), budget)?;
        budget.work(Analysis, (self.rows.len() - removed) as u64)?;
        discard(self.rows.remove(removed), budget)
    }

    pub(super) fn discard(self, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        let bytes = self.rows.capacity() as u64 * size_of::<Option<Vec<BindingId>>>() as u64;
        for row in self.rows {
            discard(row, budget)?;
        }
        budget.release(AllocationClass::Scratch, bytes)
    }
}

fn discard(
    bindings: Option<Vec<BindingId>>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes = bindings.as_ref().map_or(0, |values| {
        values.capacity() as u64 * size_of::<BindingId>() as u64
    });
    drop(bindings);
    budget.release(AllocationClass::Retained, bytes)
}

#[derive(Clone, Copy)]
enum Pending {
    Expression(ExprId),
    Region(RegionId),
}

impl Module {
    fn statement_bindings(
        &self,
        statement: &Statement,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Vec<BindingId>, AllocationError> {
        let mut scope = budget.scope();
        let budget = &mut scope;
        let mut pending = Vec::new();
        let mut bindings = Vec::new();
        self.push_statement_mentions(statement, &mut pending, budget)?;
        while let Some(node) = pending.pop() {
            budget.work(Analysis, 1)?;
            match node {
                Pending::Expression(id) => {
                    let expression = &self.expressions[id.index()];
                    if let Expr::Binding(binding) = expression {
                        budget.push(AllocationClass::Retained, &mut bindings, *binding)?;
                    }
                    for function in expression.created_functions() {
                        budget.push(
                            AllocationClass::Scratch,
                            &mut pending,
                            Pending::Region(self.functions[function.index()].body),
                        )?;
                    }
                    expression.visit_children(|child| {
                        budget.push(
                            AllocationClass::Scratch,
                            &mut pending,
                            Pending::Expression(child),
                        )
                    })?;
                }
                Pending::Region(region) => {
                    for statement in &self.regions[region.index()].statements {
                        self.push_statement_mentions(statement, &mut pending, budget)?;
                    }
                }
            }
        }
        // A fixed sorting tariff, independent of the values and cache hits.
        let work = (bindings.len() as u64)
            .checked_mul(bindings.len().max(1).ilog2() as u64 + 2)
            .ok_or(AllocationError::Capacity)?;
        budget.work(Analysis, work)?;
        bindings.sort_unstable();
        bindings.dedup();
        drop(pending);
        // Only the completed set outlives this collector. Its region owner
        // drops the actual allocation before releasing its retained charge.
        scope.finish_retained()?;
        Ok(bindings)
    }

    fn push_statement_mentions(
        &self,
        statement: &Statement,
        pending: &mut Vec<Pending>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        budget.work(Analysis, 1)?;
        let mut count = 0;
        statement.visit_expressions(|_| count += 1);
        statement.visit_regions(|_| count += 1);
        let function = match statement {
            Statement::Function { function, .. } => Some(*function),
            _ => None,
        };
        budget.reserve_vec(
            AllocationClass::Scratch,
            pending,
            count + usize::from(function.is_some()),
        )?;
        statement.visit_expressions(|id| pending.push(Pending::Expression(id)));
        statement.visit_regions(|id| pending.push(Pending::Region(id)));
        if let Some(function) = function {
            pending.push(Pending::Region(self.functions[function.index()].body));
        }
        Ok(())
    }
}
