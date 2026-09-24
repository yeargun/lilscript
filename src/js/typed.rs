//! Edits the source types allow (013-T1): formation records what each
//! binding always holds (`Module::binding_classes`).
//!
//! * A test of a nullable object against `null` is its truthiness: an
//!   object is always truthy, and `null` (or a binding not yet assigned)
//!   never is. `if(x!=null)` is `if(x)`, `x==null&&…` is `!x&&…`, wherever
//!   only the truth of the value is used.
//! * `Array.prototype.m.call(a,…)` is `a.m(…)` when `a` only ever holds
//!   array literals: under pristine builtins its own `m` is the prototype's.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

/// Array methods the host lowering reaches through `Array.prototype`.
const ARRAY_METHODS: [&str; 9] = ["push", "pop", "slice", "indexOf", "sort", "splice", "join", "shift", "unshift"];

impl Module {
    /// Null tests of nullable objects in test positions become truthiness.
    /// Returns how many.
    pub(crate) fn truthy_null_tests(&mut self, budget: &mut AllocationBudget<'_>) -> Result<usize, AllocationError> {
        if self.binding_classes.is_empty() {
            return Ok(0);
        }
        let classes = self.value_classes();
        let reach = self.reach(budget)?;
        let mut tests = Vec::new();
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                match statement {
                    Statement::If { condition, .. } => tests.push(*condition),
                    Statement::Loop {
                        condition: Some(condition),
                        ..
                    } => tests.push(*condition),
                    _ => {}
                }
            }
        }
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            match self.expressions[id.index()] {
                Expr::Conditional { condition, .. } => tests.push(condition),
                Expr::Unary {
                    op: Unary::Not,
                    value,
                } => tests.push(value),
                _ => {}
            }
        }
        let mut rewritten = 0;
        while let Some(test) = tests.pop() {
            budget.work(Analysis, 1)?;
            match self.expressions[test.index()].clone() {
                Expr::Binary {
                    op: Binary::And | Binary::Or,
                    left,
                    right,
                } => {
                    tests.push(left);
                    tests.push(right);
                }
                Expr::Binary {
                    op: op @ (Binary::Equal | Binary::NotEqual),
                    left,
                    right,
                } => {
                    let nothing = |id: ExprId| {
                        matches!(
                            self.expressions[id.index()],
                            Expr::Literal(Literal::Null | Literal::Undefined)
                        )
                    };
                    let object = |id: ExprId| match self.expressions[id.index()] {
                        Expr::Binding(binding) => matches!(
                            classes.get(binding.index()).copied().flatten(),
                            Some(ValueClass::NullableObject | ValueClass::Object)
                        ),
                        _ => false,
                    };
                    let tested = if object(left) && nothing(right) {
                        left
                    } else if object(right) && nothing(left) {
                        right
                    } else {
                        continue;
                    };
                    self.expressions[test.index()] = if op == Binary::NotEqual {
                        self.expressions[tested.index()].clone()
                    } else {
                        Expr::Unary {
                            op: Unary::Not,
                            value: tested,
                        }
                    };
                    rewritten += 1;
                }
                _ => {}
            }
        }
        Ok(rewritten)
    }

    /// `Array.prototype.m.call(a,…)` becomes `a.m(…)` for a binding `a` that
    /// only ever holds array literals. Returns how many calls, and each old
    /// node's new id when the arena was renumbered.
    pub(crate) fn array_receiver_calls(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(usize, Option<Vec<Option<ExprId>>>), AllocationError> {
        if !self.pristine_builtins {
            return Ok((0, None));
        }
        let reach = self.reach(budget)?;
        // Bindings whose every value is an array literal (and parameters,
        // loop and catch bindings, never).
        let mut arrays = vec![Some(false); self.bindings.len()];
        let mut parameters = vec![false; self.bindings.len()];
        for function in &self.functions {
            for parameter in &function.parameters {
                parameters[parameter.index()] = true;
            }
        }
        let mut mark = |binding: BindingId, is_array: bool, arrays: &mut Vec<Option<bool>>| {
            let slot = &mut arrays[binding.index()];
            *slot = match *slot {
                Some(false) if is_array => Some(true),
                Some(true) if is_array => Some(true),
                Some(false) => Some(false),
                _ => None,
            };
            if !is_array {
                *slot = None;
            }
        };
        for &region in &reach.regions {
            for statement in &self.regions[region.index()].statements {
                budget.work(Analysis, 1)?;
                match *statement {
                    Statement::Let {
                        binding,
                        value: Some(value),
                    } => mark(binding, matches!(self.expressions[value.index()], Expr::Array(_)), &mut arrays),
                    Statement::ForIn { binding, .. } | Statement::ForOf { binding, .. } => {
                        arrays[binding.index()] = None
                    }
                    Statement::Try {
                        catch:
                            Some(Catch {
                                binding: Some(binding),
                                ..
                            }),
                        ..
                    } => arrays[binding.index()] = None,
                    Statement::Function { binding, .. } => arrays[binding.index()] = None,
                    _ => {}
                }
            }
        }
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            if let Expr::Assign { target, value } = self.expressions[id.index()] {
                if let Expr::Binding(binding) = self.expressions[target.index()] {
                    mark(binding, matches!(self.expressions[value.index()], Expr::Array(_)), &mut arrays);
                }
            }
        }
        for export in &self.exports {
            arrays[export.binding.index()] = None;
        }
        let host = |module: &Self, id: ExprId, name: &str| matches!(&module.expressions[id.index()], Expr::Host(found) if found == name);
        let named = |module: &Self, id: ExprId| match &module.expressions[id.index()] {
            Expr::Member {
                object,
                property: Property::Named(name),
            } => Some((*object, name.clone())),
            _ => None,
        };
        let mut rewrites = Vec::new();
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Call {
                callee, arguments, ..
            } = &self.expressions[id.index()]
            else {
                continue;
            };
            let Some((method_node, call)) = named(self, *callee) else {
                continue;
            };
            if call != "call" {
                continue;
            }
            let Some((prototype_node, method)) = named(self, method_node) else {
                continue;
            };
            if !ARRAY_METHODS.contains(&method.as_str()) {
                continue;
            }
            let Some((array_node, prototype)) = named(self, prototype_node) else {
                continue;
            };
            if prototype != "prototype" || !host(self, array_node, "Array") {
                continue;
            }
            let Some(&receiver) = arguments.first() else {
                continue;
            };
            let Expr::Binding(binding) = self.expressions[receiver.index()] else {
                continue;
            };
            if parameters[binding.index()] || arrays[binding.index()] != Some(true) {
                continue;
            }
            rewrites.push((id, receiver, method));
        }
        let count = rewrites.len();
        let mut disordered = false;
        for (call, receiver, method) in rewrites {
            let member = self.expression_in(
                Expr::Member {
                    object: receiver,
                    property: Property::Named(method),
                },
                None,
                budget,
            )?;
            disordered = true;
            if let Expr::Call {
                callee,
                arguments,
                invocation,
            } = &mut self.expressions[call.index()]
            {
                *callee = member;
                arguments.remove(0);
                *invocation = Invocation::Reference;
            }
        }
        let map = if disordered { Some(self.renumber(budget)?) } else { None };
        Ok((count, map))
    }
}
