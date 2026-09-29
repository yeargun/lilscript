//! Calls spelled through the host's call machinery that name a plain call.
//!
//! * `x.m.call(x,…)` is `x.m(…)`: both read `x.m` once and run it with
//!   `x` as its receiver. Under pristine builtins the `call` a function
//!   inherits is `Function.prototype.call`. A port reaching a method of a
//!   `JsValue` through `JS.call(obj["m"], obj, …)` writes the first. The
//!   receiver may be a binding, `this` or a global (read once instead of
//!   twice), or a member chain of them when property reads are pure.
//!
//! A receiver adapter's private callback never reaches the tree as an
//! adapter call: formation emits it as the method itself
//! (`program/javascript_methods.rs`, plan M8.2 A1).
use super::*;
use crate::compilation_policy::WorkKind::Analysis;

impl Module {
    /// `x.m.call(x,…)` becomes `x.m(…)` for a local binding or `this`.
    /// Returns how many calls.
    pub(crate) fn self_method_calls(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError> {
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
            let Expr::Member {
                object: receiver, ..
            } = &self.expressions[method.index()]
            else {
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
            } = self.expression_mut(call)
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
        match (
            &self.expressions[left.index()],
            &self.expressions[right.index()],
        ) {
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
}
