//! An object only ever read and written through its named fields is its
//! fields: `let c={on:!1,n:a};…c.n=c.n+1…()=>c.on` becomes `let d=!1,e=a;
//! …e=e+1…()=>d`. Nothing else holds the object, so nothing can tell a
//! field from a local: every reference is `c.k` for a key the literal
//! defines, never a method call (its `this`), a `delete`, or the object
//! itself. Closures that read or write a field capture its local instead,
//! which they share exactly as they shared the object. The literal's values
//! evaluate in the same order, each now into its own `let`.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

impl Module {
    /// Replace member-only objects by their fields. Returns how many objects.
    pub(crate) fn scalarize_member_objects(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let reach = self.reach(budget)?;
        let count = self.bindings.len();
        let mut uses = vec![0usize; count];
        let mut members: Vec<Vec<(ExprId, String)>> = vec![Vec::new(); count];
        let mut excluded = vec![false; count];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match &self.expressions[id.index()] {
                Expr::Binding(binding) => uses[binding.index()] += 1,
                Expr::Member {
                    object,
                    property: Property::Named(key),
                } => {
                    if let Expr::Binding(binding) = self.expressions[object.index()] {
                        members[binding.index()].push((id, key.clone()));
                    }
                }
                // `o.m()` passes `o` as `this`; `delete o.k` changes its shape.
                Expr::Call { callee, .. } => {
                    if let Expr::Member { object, .. } = &self.expressions[callee.index()] {
                        if let Expr::Binding(binding) = self.expressions[object.index()] {
                            excluded[binding.index()] = true;
                        }
                    }
                }
                Expr::Unary {
                    op: Unary::Delete,
                    value,
                } => {
                    if let Expr::Member { object, .. } = &self.expressions[value.index()] {
                        if let Expr::Binding(binding) = self.expressions[object.index()] {
                            excluded[binding.index()] = true;
                        }
                    }
                }
                Expr::Assign { target, .. } => {
                    if let Expr::Binding(binding) = self.expressions[target.index()] {
                        excluded[binding.index()] = true;
                    }
                }
                _ => {}
            }
        }
        for export in &self.exports {
            excluded[export.binding.index()] = true;
        }
        // Candidates, per region: (index, binding, keys in order).
        let mut candidates: Vec<(RegionId, usize, BindingId, Vec<(String, ExprId)>)> = Vec::new();
        for &region in &reach.regions {
            if region == self.root {
                continue;
            }
            for (index, statement) in self.regions[region.index()].statements.iter().enumerate() {
                budget.work(Analysis, 1)?;
                let Statement::Let {
                    binding,
                    value: Some(value),
                } = *statement
                else {
                    continue;
                };
                let Expr::Object(entries) = &self.expressions[value.index()] else {
                    continue;
                };
                if excluded[binding.index()]
                    || self.bindings[binding.index()].pinned
                    || uses[binding.index()] == 0
                    || uses[binding.index()] != members[binding.index()].len()
                    || entries.len() < 2
                {
                    continue;
                }
                let mut keys: Vec<(String, ExprId)> = Vec::with_capacity(entries.len());
                let mut plain = true;
                for (key, item) in entries {
                    match key {
                        Property::Named(name)
                            if name != "__proto__" && !keys.iter().any(|(seen, _)| seen == name) =>
                        {
                            keys.push((name.clone(), *item));
                        }
                        _ => {
                            plain = false;
                            break;
                        }
                    }
                }
                if !plain
                    || !members[binding.index()]
                        .iter()
                        .all(|(_, key)| keys.iter().any(|(name, _)| name == key))
                {
                    continue;
                }
                candidates.push((region, index, binding, keys));
            }
        }
        // Later statements first within a region, so earlier indices hold.
        candidates.sort_unstable_by_key(|(region, index, _, _)| std::cmp::Reverse((region.index(), *index)));
        let replaced = candidates.len();
        for (region, index, binding, keys) in candidates {
            budget.work(Analysis, keys.len() as u64 + members[binding.index()].len() as u64)?;
            let scope = self.bindings[binding.index()].scope;
            let spelling = self.bindings[binding.index()].spelling.clone();
            let mut fields: Vec<(String, BindingId)> = Vec::with_capacity(keys.len());
            let mut lets = Vec::with_capacity(keys.len());
            for (key, value) in &keys {
                let field = BindingId::try_new(self.bindings.len()).ok_or(AllocationError::Capacity)?;
                budget.reserve_vec(AllocationClass::Retained, &mut self.bindings, 1)?;
                self.bindings.push(Binding {
                    source_symbol: None,
                    scope,
                    spelling: format!("{spelling}_{key}"),
                    pinned: false,
                });
                fields.push((key.clone(), field));
                lets.push(Statement::Let {
                    binding: field,
                    value: Some(*value),
                });
            }
            for (member, key) in &members[binding.index()] {
                let field = fields.iter().find(|(name, _)| name == key).map(|(_, field)| *field);
                if let Some(field) = field {
                    self.expressions[member.index()] = Expr::Binding(field);
                }
            }
            self.regions[region.index()].statements.splice(index..=index, lets);
        }
        Ok(replaced)
    }
}
