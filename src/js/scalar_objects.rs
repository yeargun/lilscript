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
        self.with_reach(budget, |module, reach, budget| {
            let count = module.bindings.len();
            let mut uses = vec![0usize; count];
            let mut members: Vec<Vec<(ExprId, String)>> = vec![Vec::new(); count];
            let mut excluded = vec![false; count];
            for &(id, _) in &reach.expressions {
                budget.work(Analysis, 1)?;
                match &module.expressions[id.index()] {
                    Expr::Binding(binding) => uses[binding.index()] += 1,
                    Expr::Member {
                        object,
                        property: Property::Named(key),
                    } => {
                        if let Expr::Binding(binding) = module.expressions[object.index()] {
                            members[binding.index()].push((id, key.clone()));
                        }
                    }
                    // `o.m()` passes `o` as `this`; `delete o.k` changes its shape.
                    Expr::Call { callee, .. } => {
                        if let Expr::Member { object, .. } = &module.expressions[callee.index()] {
                            if let Expr::Binding(binding) = module.expressions[object.index()] {
                                excluded[binding.index()] = true;
                            }
                        }
                    }
                    Expr::Unary {
                        op: Unary::Delete,
                        value,
                    } => {
                        if let Expr::Member { object, .. } = &module.expressions[value.index()] {
                            if let Expr::Binding(binding) = module.expressions[object.index()] {
                                excluded[binding.index()] = true;
                            }
                        }
                    }
                    Expr::Assign { target, .. } => {
                        if let Expr::Binding(binding) = module.expressions[target.index()] {
                            excluded[binding.index()] = true;
                        }
                    }
                    _ => {}
                }
            }
            for export in &module.exports {
                excluded[export.binding.index()] = true;
            }
            // Candidates, per region: (index, binding, keys in order).
            let mut candidates: Vec<(RegionId, usize, BindingId, Vec<(String, ExprId)>)> =
                Vec::new();
            let mut frames = None;
            for &region in &reach.regions {
                if region == module.root {
                    continue;
                }
                for (index, statement) in
                    module.regions[region.index()].statements.iter().enumerate()
                {
                    budget.work(Analysis, 1)?;
                    let Statement::Let {
                        binding,
                        value: Some(value),
                    } = *statement
                    else {
                        continue;
                    };
                    let Expr::Object(entries) = &module.expressions[value.index()] else {
                        continue;
                    };
                    if excluded[binding.index()]
                        || module.bindings[binding.index()].pinned
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
                                if name != "__proto__"
                                    && !keys.iter().any(|(seen, _)| seen == name) =>
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
                    // The object binding becomes initialized only after every
                    // field initializer completes. Separate lets would expose
                    // earlier fields to a reentrant capture, or leave them readable
                    // after a later initializer throws. Inert creation and reads
                    // proven initialized cannot expose that intermediate state.
                    // An uncaptured object also needs to keep a direct initializer
                    // self-read in its TDZ.
                    let mut initialization_safe = true;
                    for &(_, value) in &keys {
                        if module.inert_value(value, budget)? {
                            continue;
                        }
                        let safe = if reach.captured[binding.index()] {
                            if frames.is_none() {
                                frames = Some(module.frames(budget)?);
                            }
                            module.scalar_initializer_is_quiet(
                                value,
                                region,
                                index,
                                frames.as_ref().unwrap(),
                                budget,
                            )?
                        } else {
                            !module.mentions_within(value, binding, budget)?
                        };
                        if !safe {
                            initialization_safe = false;
                            break;
                        }
                    }
                    if !initialization_safe {
                        continue;
                    }
                    candidates.push((region, index, binding, keys));
                }
            }
            // Later statements first within a region, so earlier indices hold.
            candidates.sort_unstable_by_key(|(region, index, _, _)| {
                std::cmp::Reverse((region.index(), *index))
            });
            let replaced = candidates.len();
            for (region, index, binding, keys) in candidates {
                budget.work(
                    Analysis,
                    keys.len() as u64 + members[binding.index()].len() as u64,
                )?;
                let scope = module.bindings[binding.index()].scope;
                let spelling = module.bindings[binding.index()].spelling.clone();
                let mut fields: Vec<(String, BindingId)> = Vec::with_capacity(keys.len());
                let mut lets = Vec::with_capacity(keys.len());
                for (key, value) in &keys {
                    let field = BindingId::try_new(module.bindings.len())
                        .ok_or(AllocationError::Capacity)?;
                    budget.reserve_vec(AllocationClass::Retained, &mut module.bindings, 1)?;
                    module.bindings.push(Binding {
                        source_symbol: None,
                        scope,
                        spelling: format!("{spelling}_{key}"),
                        pinned: false,
                        class: None,
                        defined: false,
                    });
                    fields.push((key.clone(), field));
                    lets.push(Statement::Let {
                        binding: field,
                        value: Some(*value),
                    });
                }
                for (member, key) in &members[binding.index()] {
                    let field = fields
                        .iter()
                        .find(|(name, _)| name == key)
                        .map(|(_, field)| *field);
                    if let Some(field) = field {
                        module.set_expression(*member, Expr::Binding(field));
                    }
                }
                module
                    .statements_mut(region.index())
                    .splice(index..=index, lets);
            }
            Ok(replaced)
        })?
    }

    /// Creation and initialized reads cannot call out or throw. In particular,
    /// a parameter read is safe even when the new object is captured later.
    fn scalar_initializer_is_quiet(
        &self,
        value: ExprId,
        region: RegionId,
        index: usize,
        frames: &Frames,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        budget.work(Analysis, 1)?;
        match &self.expressions[value.index()] {
            Expr::Binding(binding) => self.initialized_at(*binding, region, index, frames, budget),
            Expr::Array(items) => {
                for &item in items {
                    if !self.scalar_initializer_is_quiet(item, region, index, frames, budget)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            Expr::Object(entries) => {
                for (key, item) in entries {
                    if matches!(key, Property::Computed(key)
                        if !matches!(self.expressions[key.index()],
                            Expr::Literal(Literal::String(_) | Literal::Number(_))))
                        || !self
                            .scalar_initializer_is_quiet(*item, region, index, frames, budget)?
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => self.inert_value(value, budget),
        }
    }
}
