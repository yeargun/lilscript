//! Calls spelled through the host's call machinery that name a plain call.
//!
//! * `x.m.call(x,…)` is `x.m(…)`: both read `x.m` once and run it with
//!   `x` as its receiver. Under pristine builtins the `call` a function
//!   inherits is `Function.prototype.call`. A port reaching a method of a
//!   `JsValue` through `JS.call(obj["m"], obj, …)` writes the first. The
//!   receiver may be a binding, `this` or a global (read once instead of
//!   twice), or a member chain of them when property reads are pure.
//! * A receiver adapter (`function(f){return function(a,…){return
//!   f(this,a,…)}}`, how `JS.methodN` hands a lambda its `this`) applied
//!   to an arrow that never reads that receiver is the arrow without it.
//!   The adapter passes exactly its own parameters, so the arrow keeps the
//!   same count and the same `length`. Only construction differs: an arrow
//!   cannot be `new`ed, and a lambda that ignores its receiver has nothing
//!   a construction could give it.
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

impl Module {
    /// `x.m.call(x,…)` becomes `x.m(…)` for a local binding or `this`.
    /// Returns how many calls.
    pub(crate) fn self_method_calls(&mut self, budget: &mut AllocationBudget<'_>) -> Result<usize, AllocationError> {
        if !self.pristine_builtins {
            return Ok(0);
        }
        let reach = self.reach(budget)?;
        let pure_reads = self.pure_property_reads;
        let mut rewrites = Vec::new();
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Call {
                callee, arguments, ..
            } = &self.expressions[id.index()]
            else {
                continue;
            };
            let Expr::Member {
                object: method,
                property: Property::Named(call),
            } = &self.expressions[callee.index()]
            else {
                continue;
            };
            if call != "call" {
                continue;
            }
            let Expr::Member { object: receiver, .. } = &self.expressions[method.index()] else {
                continue;
            };
            let Some(&first) = arguments.first() else {
                continue;
            };
            if self.same_reference(*receiver, first, pure_reads) {
                rewrites.push((id, *method));
            }
        }
        for &(call, method) in &rewrites {
            if let Expr::Call {
                callee,
                arguments,
                invocation,
            } = &mut self.expressions[call.index()]
            {
                *callee = method;
                arguments.remove(0);
                *invocation = Invocation::Reference;
            }
        }
        Ok(rewrites.len())
    }

    /// Whether two expressions read the same value without effects: one
    /// binding, `this`, one pristine global, or (when property reads are
    /// pure) one member chain of those with literal or binding keys.
    fn same_reference(&self, left: ExprId, right: ExprId, pure_reads: bool) -> bool {
        match (&self.expressions[left.index()], &self.expressions[right.index()]) {
            (Expr::Binding(left), Expr::Binding(right)) => left == right,
            (Expr::This, Expr::This) => true,
            (Expr::Host(left), Expr::Host(right)) => left == right,
            (Expr::Literal(left), Expr::Literal(right)) => pure_reads && left == right,
            (
                Expr::Member {
                    object: left_object,
                    property: left_property,
                },
                Expr::Member {
                    object: right_object,
                    property: right_property,
                },
            ) if pure_reads => {
                let keys = match (left_property, right_property) {
                    (Property::Named(left), Property::Named(right)) => left == right,
                    (Property::Computed(left), Property::Computed(right)) => {
                        self.same_reference(*left, *right, pure_reads)
                    }
                    _ => false,
                };
                keys && self.same_reference(*left_object, *right_object, pure_reads)
            }
            _ => false,
        }
    }

    /// `adapter(arrow)` becomes the arrow without its first parameter when
    /// the arrow never reads it. Returns how many.
    pub(crate) fn dissolve_receiver_adapters(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
        // Adapters by binding, with the parameter count they forward.
        let mut adapters: Vec<Option<usize>> = vec![None; self.bindings.len()];
        let mut any = false;
        for statement in &self.regions[self.root.index()].statements {
            budget.work(Analysis, 1)?;
            if let Statement::Function { binding, function } = *statement {
                if let Some(count) = self.receiver_adapter(function) {
                    adapters[binding.index()] = Some(count);
                    any = true;
                }
            }
        }
        if !any {
            return Ok(0);
        }
        let reach = self.reach(budget)?;
        let mut references = vec![0usize; self.bindings.len()];
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            if let Expr::Binding(binding) = self.expressions[id.index()] {
                references[binding.index()] += 1;
            }
        }
        let mut rewrites = Vec::new();
        for &(id, _) in &reach.expressions {
            budget.work(Analysis, 1)?;
            let Expr::Call {
                callee, arguments, ..
            } = &self.expressions[id.index()]
            else {
                continue;
            };
            let Expr::Binding(adapter) = self.expressions[callee.index()] else {
                continue;
            };
            let Some(count) = adapters[adapter.index()] else {
                continue;
            };
            let [argument] = arguments[..] else {
                continue;
            };
            let Expr::Function(function) = self.expressions[argument.index()] else {
                continue;
            };
            let lambda = &self.functions[function.index()];
            if !lambda.arrow
                || lambda.length.is_some()
                || lambda.parameters.len() != count + 1
                || references[lambda.parameters[0].index()] != 0
            {
                continue;
            }
            rewrites.push((id, argument, function));
        }
        for &(call, argument, function) in &rewrites {
            self.functions[function.index()].parameters.remove(0);
            // The call is the arrow now; its old node no longer creates it.
            self.expressions[call.index()] = Expr::Function(function);
            self.expressions[argument.index()] = Expr::Literal(Literal::Undefined);
        }
        Ok(rewrites.len())
    }

    /// `function(f){return function(a,…){return f(this,a,…)}}`: the count
    /// of forwarded parameters.
    fn receiver_adapter(&self, outer: FunctionId) -> Option<usize> {
        let outer = &self.functions[outer.index()];
        let [target] = outer.parameters[..] else {
            return None;
        };
        let [Statement::Return(Some(created))] = self.regions[outer.body.index()].statements[..] else {
            return None;
        };
        let Expr::Function(inner) = self.expressions[created.index()] else {
            return None;
        };
        let inner = &self.functions[inner.index()];
        if inner.arrow || inner.length.is_some() || !matches!(inner.suspension, Suspension::None) {
            return None;
        }
        let [Statement::Return(Some(forward))] = self.regions[inner.body.index()].statements[..] else {
            return None;
        };
        let Expr::Call {
            callee, arguments, ..
        } = &self.expressions[forward.index()]
        else {
            return None;
        };
        if !matches!(self.expressions[callee.index()], Expr::Binding(found) if found == target) {
            return None;
        }
        let (first, rest) = arguments.split_first()?;
        if !matches!(self.expressions[first.index()], Expr::This) || rest.len() != inner.parameters.len() {
            return None;
        }
        let forwarded = rest
            .iter()
            .zip(&inner.parameters)
            .all(|(value, parameter)| matches!(self.expressions[value.index()], Expr::Binding(found) if found == *parameter));
        forwarded.then_some(inner.parameters.len())
    }
}
