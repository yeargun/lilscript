//! Declarations and stores regrouped where the printer lists them together.
//!
//! * `let a=1;S;let b;` is `let a=1,b;S`: an uninitialized declaration joins
//!   the one before it when nothing between names it. No read could meet
//!   its TDZ, and it initializes to `undefined` either way (Terser's
//!   `join_vars`, measured −46 Brotli on katexlil).
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
                                    || self.root_modules.get(at..=index).is_some_and(|modules| {
                                        modules.iter().all(|&module| module == modules[0])
                                    }))
                        });
                        if let Some(at) = target {
                            budget.work(Analysis, (index - at) as u64)?;
                            let named = self.regions[region].statements[at + 1..index]
                                .iter()
                                .any(|statement| self.statement_mentions(statement, binding));
                            if !named {
                                let moved = self.regions[region].statements.remove(index);
                                self.regions[region].statements.insert(at + 1, moved);
                                if root && index < self.root_modules.len() {
                                    let module = self.root_modules.remove(index);
                                    self.root_modules.insert(at + 1, module);
                                }
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

    /// Runs of method stores into one prototype become one `Object.assign`.
    /// Returns how many stores were grouped.
    pub(crate) fn group_prototype_stores(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        // One read of `X.prototype` replaces one per store: the contract must
        // assume member reads run no code.
        if !self.pristine_builtins || !self.pure_property_reads {
            return Ok(0);
        }
        let mut written = budget.filled(AllocationClass::Scratch, self.bindings.len(), false)?;
        budget.work(Analysis, self.expressions.len() as u64)?;
        for expression in &self.expressions {
            if let Expr::Assign { target, .. } = expression {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    written[binding.index()] = true;
                }
            }
        }
        // Adapters: bindings holding a function whose body only returns a
        // function it creates. Calling one runs nothing else and cannot throw.
        let mut adapter = budget.filled(AllocationClass::Scratch, self.bindings.len(), false)?;
        for region in &self.regions {
            for statement in &region.statements {
                budget.work(Analysis, 1)?;
                let (binding, function) = match *statement {
                    Statement::Function { binding, function } => (binding, function),
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => match self.expressions[value.index()] {
                        Expr::Function(function) => (binding, function),
                        _ => continue,
                    },
                    _ => continue,
                };
                let body = self.functions[function.index()].body;
                adapter[binding.index()] = !written[binding.index()]
                    && matches!(
                        self.regions[body.index()].statements[..],
                        [Statement::Return(Some(value))]
                            if matches!(self.expressions[value.index()], Expr::Function(_))
                    );
            }
        }
        let creates = |module: &Self, value: ExprId| match &module.expressions[value.index()] {
            Expr::Literal(_) | Expr::Function(_) | Expr::Regex(_) => true,
            Expr::Call {
                callee, arguments, ..
            } => {
                matches!(module.expressions[callee.index()], Expr::Binding(binding) if adapter[binding.index()])
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
        for region in 0..self.regions.len() {
            let root = region == self.root.index();
            let mut index = 0;
            while index < self.regions[region].statements.len() {
                budget.work(Analysis, 1)?;
                let Some((prototype, class, _, _)) =
                    store(self, &self.regions[region].statements[index])
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
                while let Some(statement) = self.regions[region].statements.get(end) {
                    budget.work(Analysis, 1)?;
                    let same_module = !root
                        || self.root_modules.get(end) == self.root_modules.get(index);
                    if !same_module {
                        break;
                    }
                    if matches!(statement, Statement::Function { .. }) {
                        declarations.push(statement.clone());
                        end += 1;
                        continue;
                    }
                    match store(self, statement) {
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
                    - self.regions[region].statements[last_store..end]
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
                let object = self.expression_in(Expr::Host("Object".into()), None, budget)?;
                let assign = self.expression_in(
                    Expr::Member {
                        object,
                        property: Property::Named("assign".into()),
                    },
                    None,
                    budget,
                )?;
                let literal = self.expression_in(Expr::Object(entries), None, budget)?;
                let call = self.expression_in(
                    Expr::Call {
                        callee: assign,
                        arguments: vec![prototype, literal],
                        invocation: Invocation::Reference,
                    },
                    None,
                    budget,
                )?;
                let replaced = declarations.len() + 1;
                declarations.push(Statement::Evaluate(call));
                self.regions[region].statements.splice(index..end, declarations);
                if root && end <= self.root_modules.len() {
                    self.root_modules.drain(index + replaced..end);
                }
                grouped += count;
                index += replaced;
            }
        }
        Ok(grouped)
    }
}
