//! Dependencies of the current physical tree. Declarations whose evaluation is
//! inert are deferred behind their binding; executable statements, exports and
//! pinned bindings are roots. A single admitted closure solves chains and
//! recursive components without treating an unused function's body as a root.
//! The index belongs to one immutable tree and never survives an edit.
use super::*;
use crate::compilation_policy::WorkKind::{Analysis, Edit};

struct Declaration {
    region: usize,
    position: usize,
    binding: BindingId,
}

struct Vertices {
    expressions: usize,
    regions: usize,
    functions: usize,
    count: usize,
}
impl Vertices {
    fn new(module: &Module) -> Result<Self, AllocationError> {
        let expressions = module.bindings.len();
        let regions = expressions
            .checked_add(module.expressions.len())
            .ok_or(AllocationError::Capacity)?;
        let functions = regions
            .checked_add(module.regions.len())
            .ok_or(AllocationError::Capacity)?;
        let count = functions
            .checked_add(module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        Ok(Self {
            expressions,
            regions,
            functions,
            count,
        })
    }
    fn binding(&self, id: BindingId) -> usize {
        id.index()
    }
    fn expression(&self, id: ExprId) -> usize {
        self.expressions + id.index()
    }
    fn region(&self, id: RegionId) -> usize {
        self.regions + id.index()
    }
    fn function(&self, id: FunctionId) -> usize {
        self.functions + id.index()
    }
}

/// The same dependency graph supplies liveness for every declaration in a
/// physical module. All temporary storage belongs to the caller's scope.
struct UseIndex {
    live: Vec<bool>,
    declarations: Vec<Declaration>,
    region_base: usize,
}
impl UseIndex {
    fn build(module: &Module, budget: &mut AllocationBudget<'_>) -> Result<Self, AllocationError> {
        use AllocationClass::Scratch;
        let nodes = Vertices::new(module)?;
        let mut edges = budget.vector(Scratch, 0)?;
        let mut declarations = budget.vector(Scratch, 0)?;
        for (index, region) in module.regions.iter().enumerate() {
            let owner = nodes.regions + index;
            for (position, statement) in region.statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let deferred = match *statement {
                    Statement::Function { binding, .. }
                    | Statement::Let {
                        binding,
                        value: None,
                    } => Some(binding),
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } if module.inert_value(value, budget)?
                        || module.pure_property_reads && module.discardable_read(value) => Some(binding),
                    _ => None,
                };
                let owner = if let Some(binding) = deferred {
                    budget.push(
                        Scratch,
                        &mut declarations,
                        Declaration {
                            region: index,
                            position,
                            binding,
                        },
                    )?;
                    nodes.binding(binding)
                } else {
                    owner
                };
                let mut count = 0;
                statement.visit_expressions(|_| count += 1);
                statement.visit_regions(|_| count += 1);
                budget.work(Analysis, count as u64)?;
                budget.reserve_vec(Scratch, &mut edges, count)?;
                statement.visit_expressions(|expression| {
                    edges.push((owner, nodes.expression(expression)))
                });
                statement.visit_regions(|region| edges.push((owner, nodes.region(region))));
                match *statement {
                    Statement::Function { function, .. } => {
                        budget.push(Scratch, &mut edges, (owner, nodes.function(function)))?
                    }
                    Statement::ForIn { binding, .. }
                    | Statement::ForOf { binding, .. }
                    | Statement::Try {
                        catch:
                            Some(Catch {
                                binding: Some(binding),
                                ..
                            }),
                        ..
                    } => {
                        budget.push(Scratch, &mut edges, (owner, nodes.binding(binding)))?;
                    }
                    _ => {}
                }
            }
        }
        for (index, expression) in module.expressions.iter().enumerate() {
            budget.work(Analysis, 1)?;
            let owner = nodes.expressions + index;
            if let Expr::Binding(binding) = *expression {
                budget.push(Scratch, &mut edges, (owner, nodes.binding(binding)))?;
            }
            expression.visit_children(|child| {
                budget.push(Scratch, &mut edges, (owner, nodes.expression(child)))
            })?;
            for function in expression.created_functions() {
                budget.work(Analysis, 1)?;
                budget.push(Scratch, &mut edges, (owner, nodes.function(function)))?;
            }
        }
        for (index, function) in module.functions.iter().enumerate() {
            budget.work(Analysis, function.parameters.len() as u64 + 1)?;
            let owner = nodes.functions + index;
            budget.push(Scratch, &mut edges, (owner, nodes.region(function.body)))?;
            for &parameter in &function.parameters {
                budget.push(Scratch, &mut edges, (owner, nodes.binding(parameter)))?;
            }
        }
        let search_work = u64::from(usize::BITS - edges.len().leading_zeros()) + 1;
        budget.work(Analysis, (edges.len() as u64).saturating_mul(search_work))?;
        edges.sort_unstable();
        let mut live = budget.filled(Scratch, nodes.count, false)?;
        let mut pending = budget.vector(Scratch, nodes.count)?;
        let mut enqueue =
            |node: usize, budget: &mut AllocationBudget<'_>| -> Result<(), AllocationError> {
                budget.work(Analysis, 1)?;
                if !std::mem::replace(&mut live[node], true) {
                    pending.push(node);
                }
                Ok(())
            };
        enqueue(nodes.region(module.root), budget)?;
        for (index, binding) in module.bindings.iter().enumerate() {
            budget.work(Analysis, 1)?;
            if binding.pinned {
                enqueue(index, budget)?;
            }
        }
        for export in &module.exports {
            enqueue(nodes.binding(export.binding), budget)?;
        }
        let mut cursor = 0;
        while cursor < pending.len() {
            let owner = pending[cursor];
            cursor += 1;
            budget.work(Analysis, search_work)?;
            let start = edges.partition_point(|&(from, _)| from < owner);
            for &(from, to) in &edges[start..] {
                budget.work(Analysis, 1)?;
                if from != owner {
                    break;
                }
                if !std::mem::replace(&mut live[to], true) {
                    pending.push(to);
                }
            }
        }
        Ok(Self {
            live,
            declarations,
            region_base: nodes.regions,
        })
    }
}

impl Module {
    /// Retire every unobserved inert declaration in one solve. The mutation
    /// consumes a completed index, preserves statement order and updates root
    /// rows with the same retention mask. No graph is consulted after an edit.
    pub(crate) fn prune_declarations(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut scope = budget.scope();
        let index = UseIndex::build(self, &mut scope)?;
        let work = self
            .regions
            .iter()
            .try_fold(self.root_rows.len(), |n, region| {
                n.checked_add(region.statements.len())
            })
            .ok_or(AllocationError::Capacity)?;
        scope.work(Edit, work as u64)?;
        let mut cursor = 0;
        let mut removed = 0;
        while cursor < index.declarations.len() {
            let start = cursor;
            let region = index.declarations[start].region;
            while cursor < index.declarations.len() && index.declarations[cursor].region == region {
                cursor += 1;
            }
            let declarations = &index.declarations[start..cursor];
            // Unreachable arena remnants are not syntax. Their eventual
            // compaction must not masquerade as a semantic progress edit.
            if !index.live[index.region_base + region]
                || declarations
                    .iter()
                    .all(|decl| index.live[decl.binding.index()])
            {
                continue;
            }
            self.journal_region(region);
            // Each scan advances monotonically through both lists. Removing
            // interleaved declarations must not shift the whole suffix N times.
            let live_bindings = &index.live;
            let keep = || {
                let mut next = 0;
                let mut position = 0;
                move || {
                    let retained = if declarations
                        .get(next)
                        .is_some_and(|decl| decl.position == position)
                    {
                        let live = live_bindings[declarations[next].binding.index()];
                        next += 1;
                        live
                    } else {
                        true
                    };
                    position += 1;
                    retained
                }
            };
            if self.rows_of(region) {
                let mut keep_row = keep();
                self.root_rows.retain(|_| keep_row());
            }
            let mut keep_statement = keep();
            self.regions[region].statements.retain(|_| {
                let retained = keep_statement();
                removed += usize::from(!retained);
                retained
            });
        }
        Ok(removed)
    }
}
