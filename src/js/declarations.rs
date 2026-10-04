//! Declarations and stores regrouped where the printer lists them together.
//!
//! * `let a=1;S;let b;` is `let a=1,b;S`: an uninitialized declaration joins
//!   the one before it when nothing between names it. No read could meet
//!   its TDZ, and it initializes to `undefined` either way. Joining removes
//!   a declaration keyword without moving an observable initializer.
//! * `X.prototype.a=f;X.prototype.b=g` is `Object.assign(X.prototype,{a:f,
//!   b:g})` under pristine builtins and pure member reads (`X.prototype` is
//!   read once instead of per store), when each value only creates something
//!   (a literal, a function, or a call of an adapter that only returns a new
//!   function): the values run in the same order and cannot throw, and
//!   `Object.assign` stores each key as an assignment would.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// How far back an uninitialized declaration looks for one to join.
const JOIN_WINDOW: usize = 16;

impl Module {
    /// Move each uninitialized `let` up to join the declaration before it.
    /// Root statements move only within one source module. Returns how many.
    pub(crate) fn join_empty_declarations(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let mut joined = 0;
        for region in 0..self.regions.len() {
            let root = region == self.root.index();
            let mut previous: Option<usize> = None;
            let mut index = 0;
            while index < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                match self.regions[region].statements[index] {
                    Statement::Let {
                        binding,
                        value: None,
                    } => {
                        let target = previous.filter(|&at| {
                            at + 1 < index
                                && index - at <= JOIN_WINDOW
                                && (!root
                                    || self.root_rows.get(at..=index).is_some_and(|rows| {
                                        rows.iter().all(|row| row.module == rows[0].module)
                                    }))
                        });
                        if let Some(at) = target {
                            budget.work(Analysis, (index - at) as u64)?;
                            let mut named = false;
                            for statement in &self.regions[region].statements[at + 1..index] {
                                if self.statement_mentions_in(statement, binding, budget)? {
                                    named = true;
                                    break;
                                }
                            }
                            if !named {
                                self.move_statement(region, index, at + 1);
                                previous = Some(at + 1);
                                joined += 1;
                                index += 1;
                                continue;
                            }
                        }
                        previous = Some(index);
                    }
                    Statement::Let { .. } => previous = Some(index),
                    _ => {}
                }
                index += 1;
            }
        }
        Ok(joined)
    }

    /// `Object.defineProperty(C,"k",{value:f,writable:!0,configurable:!0})`
    /// after a root `let C=class{…}` is `static k(…){…}` in its body, and
    /// `Object.defineProperty(C.prototype,"k",{get:f,configurable:!0})` is
    /// `get k(){…}`: the same property with the same attributes (a class
    /// element is configurable and not enumerable, a static method also
    /// writable), holding the same function, now named `k` / `get k` as
    /// upstream's class element is. A static comes from an arrow that reads
    /// no frame of its own, so `this` cannot tell; the member exists from the
    /// class's creation on, which nothing between can observe, since no
    /// statement in between mentions `C`. Class code is strict already.
    pub(crate) fn fold_class_members(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        if !self.pristine_builtins {
            return Ok(0);
        }
        let root = self.root.index();
        let mut folded = 0;
        let mut index = 0;
        while index < self.regions[root].statements.len() {
            budget.work(Analysis, 1)?;
            let Some((class, member)) = self.class_member_definition(&self.regions[root].statements[index]) else {
                index += 1;
                continue;
            };
            let declared = (0..index).rev().find(|&at| {
                matches!(self.regions[root].statements[at], Statement::Let { binding, value: Some(value) }
                    if binding == class && matches!(self.expressions[value.index()], Expr::Class { .. }))
            });
            let Some(declared) = declared else {
                index += 1;
                continue;
            };
            let mut observed = false;
            for at in declared + 1..index {
                let statement = self.regions[root].statements[at].clone();
                if self.statement_mentions_in(&statement, class, budget)? {
                    observed = true;
                    break;
                }
            }
            if observed {
                index += 1;
                continue;
            }
            let Statement::Let { value: Some(value), .. } = self.regions[root].statements[declared] else {
                unreachable!("found class declaration")
            };
            let function = self.function_mut(member.function);
            function.arrow = false;
            function.strict = false;
            if let Expr::Class { members, .. } = self.expression_mut(value) {
                members.push(member);
            }
            self.remove_statement(root, index);
            folded += 1;
        }
        Ok(folded)
    }

    /// The class binding and member that a `defineProperty` statement of
    /// `fold_class_members`' two shapes defines.
    fn class_member_definition(&self, statement: &Statement) -> Option<(BindingId, ClassMember)> {
        let Statement::Evaluate(call) = *statement else {
            return None;
        };
        let Expr::Call { callee, arguments, .. } = &self.expressions[call.index()] else {
            return None;
        };
        let Expr::Member { object, property: Property::Named(method) } = &self.expressions[callee.index()] else {
            return None;
        };
        let object_global = match &self.expressions[object.index()] {
            Expr::Host(host) => host.name == "Object" && matches!(host.kind, crate::catalog::HostKind::Standard(_)),
            Expr::Binding(binding) => self.standard_global(*binding) && self.bindings[binding.index()].spelling == "Object",
            _ => false,
        };
        if method != "defineProperty" || !object_global {
            return None;
        }
        let [target, key, descriptor] = arguments.as_slice() else {
            return None;
        };
        let Expr::Literal(Literal::String(key)) = &self.expressions[key.index()] else {
            return None;
        };
        let key = key.as_unicode()?;
        if !identifier_name(key) || matches!(key, "constructor" | "prototype" | "__proto__") {
            return None;
        }
        let Expr::Object(entries) = &self.expressions[descriptor.index()] else {
            return None;
        };
        let entry = |name: &str| {
            entries.iter().find(|(entry, _)| matches!(entry, Property::Named(found) if found == name)).map(|(_, value)| *value)
        };
        let named = entries.iter().all(|(entry, _)| matches!(entry, Property::Named(_)));
        let yes = |id: Option<ExprId>| matches!(id.map(|id| &self.expressions[id.index()]), Some(Expr::Literal(Literal::Bool(true))));
        match &self.expressions[target.index()] {
            Expr::Binding(class) => {
                if !named || entries.len() != 3 || !yes(entry("writable")) || !yes(entry("configurable")) {
                    return None;
                }
                let Expr::Function(function) = self.expressions[entry("value")?.index()] else {
                    return None;
                };
                let created = &self.functions[function.index()];
                (created.arrow && created.suspension != Suspension::Generator && self.frame_free(function))
                    .then(|| (*class, ClassMember { kind: MemberKind::Static, name: key.into(), function }))
            }
            Expr::Member { object, property: Property::Named(prototype) } if prototype == "prototype" => {
                let Expr::Binding(class) = self.expressions[object.index()] else {
                    return None;
                };
                if !named || entries.len() != 2 || !yes(entry("configurable")) {
                    return None;
                }
                let Expr::Function(function) = self.expressions[entry("get")?.index()] else {
                    return None;
                };
                let created = &self.functions[function.index()];
                (!created.arrow && created.parameters.is_empty() && created.suspension == Suspension::None)
                    .then(|| (class, ClassMember { kind: MemberKind::Getter, name: key.into(), function }))
            }
            _ => None,
        }
    }

    /// Runs of method stores into one prototype become one `Object.assign`.
    /// Returns how many stores were grouped.
    pub(crate) fn group_prototype_stores(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        let folded = self.fold_class_members(budget)?;
        // One read of `X.prototype` replaces one per store: the contract must
        // assume member reads run no code.
        if !self.pristine_builtins || !self.pure_property_reads {
            return Ok(folded);
        }
        let grouped = self.with_reach_tree(budget, |module, reach, budget| {
        let creates = |module: &Self, value: ExprId| match &module.expressions[value.index()] {
            Expr::Literal(_) | Expr::Function(_) | Expr::Regex(_) => true,
            Expr::Call {
                callee, arguments, ..
            } => {
                matches!(module.expressions[callee.index()], Expr::Binding(binding) if reach.bindings[binding.index()].factory(module))
                    && arguments.iter().all(|argument| {
                        matches!(
                            module.expressions[argument.index()],
                            Expr::Literal(_) | Expr::Function(_)
                        )
                    })
            }
            _ => false,
        };
        // A store `X.prototype.k=v` of a creation: its target, X and k.
        let store = |module: &Self, statement: &Statement| {
            let Statement::Evaluate(root) = *statement else {
                return None;
            };
            let Expr::Assign { target, value } = module.expressions[root.index()] else {
                return None;
            };
            let Expr::Member {
                object: prototype,
                property: Property::Named(key),
            } = &module.expressions[target.index()]
            else {
                return None;
            };
            let Expr::Member {
                object: class,
                property: Property::Named(name),
            } = &module.expressions[prototype.index()]
            else {
                return None;
            };
            let Expr::Binding(class) = module.expressions[class.index()] else {
                return None;
            };
            // `X` may be assigned elsewhere: the values only create, so it
            // holds one value throughout the run, and so does its prototype.
            (name == "prototype" && key != "__proto__" && creates(module, value))
                .then(|| (*prototype, class, key.clone(), value))
        };
        let mut grouped = 0;
        for region in 0..module.regions.len() {
            let root = region == module.root.index();
            let mut index = 0;
            while index < module.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                let Some((prototype, class, _, _)) =
                    store(module, &module.regions[region].statements[index])
                else {
                    index += 1;
                    continue;
                };
                // Function declarations between the stores (adapters the
                // printer places at first use) are hoisted: they move ahead.
                let mut entries = Vec::new();
                let mut declarations = Vec::new();
                let mut end = index;
                let mut last_store = index;
                while let Some(statement) = module.regions[region].statements.get(end) {
                    budget.work(Analysis, 1)?;
                    let same_module = !root || module.root_module(end) == module.root_module(index);
                    if !same_module {
                        break;
                    }
                    if matches!(statement, Statement::Function { .. }) {
                        declarations.push(statement.clone());
                        end += 1;
                        continue;
                    }
                    match store(module, statement) {
                        Some((_, other, key, value)) if other == class => {
                            entries.push((Property::Named(key), value));
                            end += 1;
                            last_store = end;
                        }
                        _ => break,
                    }
                }
                // Declarations after the last store stay where they are.
                let kept = declarations.len()
                    - module.regions[region].statements[last_store..end]
                        .iter()
                        .filter(|statement| matches!(statement, Statement::Function { .. }))
                        .count();
                declarations.truncate(kept);
                let end = last_store;
                if entries.len() < 2 {
                    index += 1;
                    continue;
                }
                let count = entries.len();
                let object = module.expression_in(Expr::Host(Host::new("Object")), None, budget)?;
                let assign = module.expression_in(
                    Expr::Member {
                        object,
                        property: Property::Named("assign".into()),
                    },
                    None,
                    budget,
                )?;
                let literal = module.expression_in(Expr::Object(entries), None, budget)?;
                let call = module.expression_in(
                    Expr::Call {
                        callee: assign,
                        arguments: vec![prototype, literal],
                        invocation: Invocation::Reference,
                    },
                    None,
                    budget,
                )?;
                let replaced = declarations.len() + 1;
                let removed_kinds = module.regions[region].statements[index..end]
                    .iter()
                    .map(|statement| {
                        if matches!(statement, Statement::Function { .. }) {
                            StatementKind::Declaration
                        } else {
                            StatementKind::Store
                        }
                    })
                    .collect::<Vec<_>>();
                declarations.push(Statement::Evaluate(call));
                // The hoisted declarations keep their rows; the one call
                // holds every store.
                module.splice_statements(region, index..end, declarations, |original| {
                    let mut stores = None::<RootRow>;
                    let mut declared = Vec::with_capacity(replaced);
                    for (offset, row) in original.iter().enumerate() {
                        if matches!(removed_kinds[offset], StatementKind::Declaration) {
                            declared.push(*row);
                        } else {
                            stores = Some(stores.map_or(*row, |held| held.fuse(*row)));
                        }
                    }
                    declared.push(stores.unwrap_or(original[0]));
                    declared
                });
                grouped += count;
                index += replaced;
            }
        }
        Ok::<usize, AllocationError>(grouped)
        })??;
        Ok(folded + grouped)
    }
}

/// What a statement a prototype-store group replaces was.
enum StatementKind {
    Declaration,
    Store,
}
