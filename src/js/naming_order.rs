//! Printed first-occurrence order, computed only when that alternative is used.
//! This is a naming prior; it never authorizes movement or rewrites.
use super::*;

pub(super) struct Compact<'a> {
    pub(super) hosts_self: Vec<&'a str>,
    pub(super) printed: Vec<BindingId>,
    pub(super) scoped: Vec<BindingId>,
    pub(super) by_reads: Vec<BindingId>,
}

#[derive(Clone, Copy)]
enum Event {
    Region(RegionId),
    Statement(RegionId, usize),
    Expression(ExprId),
    Function(FunctionId),
    Binding(BindingId),
}

impl<'a> Basis<'a> {
    pub(super) fn compact_in(
        &self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<&Compact<'a>, OutputError> {
        if let Some(order) = self.compact.get() {
            return Ok(order);
        }
        use AllocationClass::{Retained, Scratch};
        let mut phase = budget.scope();
        let module = self.module;
        let mut rank = phase.filled(Scratch, module.bindings.len(), usize::MAX)?;
        let mut functions = phase.filled(Scratch, module.functions.len(), false)?;
        let mut printed = phase.vector(Retained, module.bindings.len())?;
        let mut pending = Vec::new();
        phase.push(Scratch, &mut pending, Event::Region(module.root))?;
        for import in module.imports.iter().rev() {
            phase.push(Scratch, &mut pending, Event::Binding(import.binding))?;
        }
        while let Some(event) = pending.pop() {
            phase.work(WorkKind::Analysis, 1)?;
            let start = pending.len();
            let mut push = |event| phase.push(Scratch, &mut pending, event);
            match event {
                Event::Binding(binding) => {
                    if self.live[binding.index()] && rank[binding.index()] == usize::MAX {
                        rank[binding.index()] = printed.len();
                        printed.push(binding);
                    }
                }
                Event::Region(region) => {
                    for index in 0..module.regions[region.index()].statements.len() {
                        push(Event::Statement(region, index))?;
                    }
                }
                Event::Function(id) => {
                    functions[id.index()] = true;
                    let function = &module.functions[id.index()];
                    for &parameter in &function.parameters {
                        push(Event::Binding(parameter))?;
                    }
                    push(Event::Region(function.body))?;
                }
                Event::Expression(id) => match &module.expressions[id.index()] {
                    Expr::Binding(binding) => push(Event::Binding(*binding))?,
                    Expr::Function(function) => push(Event::Function(*function))?,
                    Expr::Class { base, constructor, methods, members, .. } => {
                        if let Some(base) = base { push(Event::Expression(*base))?; }
                        if let Some(constructor) = constructor { push(Event::Function(*constructor))?; }
                        for (_, method) in methods { push(Event::Function(*method))?; }
                        for member in members { push(Event::Function(member.function))?; }
                    }
                    expression => expression.visit_children(|id| push(Event::Expression(id)))?,
                },
                Event::Statement(region, index) => {
                    match &module.regions[region.index()].statements[index] {
                        Statement::Let { binding, value } => {
                            push(Event::Binding(*binding))?;
                            if let Some(value) = value {
                                push(Event::Expression(*value))?;
                            }
                        }
                        Statement::Function { binding, function } => {
                            push(Event::Binding(*binding))?;
                            push(Event::Function(*function))?;
                        }
                        Statement::ForIn {
                            binding,
                            object,
                            body,
                        }
                        | Statement::ForOf {
                            binding,
                            iterable: object,
                            body,
                        } => {
                            push(Event::Binding(*binding))?;
                            push(Event::Expression(*object))?;
                            push(Event::Region(*body))?;
                        }
                        Statement::Try {
                            body,
                            catch,
                            finally,
                        } => {
                            push(Event::Region(*body))?;
                            if let Some(catch) = catch {
                                if let Some(binding) = catch.binding {
                                    push(Event::Binding(binding))?;
                                }
                                push(Event::Region(catch.body))?;
                            }
                            if let Some(finally) = finally {
                                push(Event::Region(*finally))?;
                            }
                        }
                        statement => {
                            let mut result = Ok(());
                            statement.visit_expressions(|id| {
                                if result.is_ok() {
                                    result = push(Event::Expression(id));
                                }
                            });
                            result?;
                            statement.visit_regions(|id| {
                                if result.is_ok() {
                                    result = push(Event::Region(id));
                                }
                            });
                            result?;
                        }
                    }
                }
            }
            // Children were visited in print order; a stack pops in reverse.
            phase.work(WorkKind::Analysis, (pending.len() - start) as u64)?;
            pending[start..].reverse();
        }
        // Every live declaration must have an owner in the verified tree.
        // Missing one is an implementation error, never a silently blank name.
        for (id, &live) in self.live.iter().enumerate() {
            phase.work(WorkKind::Analysis, 1)?;
            if live && rank[id] == usize::MAX {
                return Err("live binding has no printed naming owner".into());
            }
        }
        let mut scoped = phase.copy_slice(Retained, &printed)?;
        phase.work(WorkKind::Analysis, sort_work(scoped.len(), 1)?)?;
        scoped.sort_unstable_by_key(|id| (module.bindings[id.index()].scope, rank[id.index()]));
        let mut reads = phase.filled(Scratch, module.bindings.len(), 0u32)?;
        for &(id, _) in &self.references {
            phase.work(WorkKind::Analysis, 1)?;
            if let Expr::Binding(binding) = module.expressions[id.index()] {
                reads[binding.index()] = reads[binding.index()].saturating_add(1);
            }
        }
        let mut by_reads = phase.copy_slice(Retained, &scoped)?;
        phase.work(WorkKind::Analysis, sort_work(by_reads.len(), 1)?)?;
        by_reads.sort_unstable_by_key(|id| {
            (
                module.bindings[id.index()].scope,
                std::cmp::Reverse(reads[id.index()]),
                rank[id.index()],
            )
        });
        let mut hosts_self = phase.copy_slice(Retained, &self.hosts)?;
        for (id, live) in functions.into_iter().enumerate() {
            phase.work(WorkKind::Analysis, 1)?;
            if live && self.self_named[id] {
                let name = module.functions[id]
                    .name
                    .exact()
                    .and_then(StringValue::as_unicode)
                    .expect("validated self-named function");
                phase.push(Retained, &mut hosts_self, name)?;
            }
        }
        phase.work(WorkKind::Analysis, hosts_self.len() as u64)?;
        let longest = hosts_self.iter().map(|s| s.len()).max().unwrap_or(0);
        phase.work(WorkKind::Analysis, sort_work(hosts_self.len(), longest)?)?;
        hosts_self.sort_unstable();
        // No dedup is needed: the reservation owner admits and deduplicates
        // this list with explicit required bindings at rendering.
        self.compact
            .set(Compact {
                printed,
                scoped,
                by_reads,
                hosts_self,
            })
            .map_err(|_| OutputError::Invalid("printed naming cache raced"))?;
        phase.finish_retained()?;
        Ok(self.compact.get().unwrap())
    }
}
