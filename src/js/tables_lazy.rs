//! Laziness is legal only for checked immutable data used through computed
//! lookups. No wrapper object or Proxy can leak into identity/reflection/ABI.
//! Preserve the original binding's TDZ and receiver-before-key evaluation.
use super::*;

impl Module {
    pub(super) fn lazy_table_eligible(
        &self,
        binding: BindingId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        if !self.immutable_data.contains(&binding)
            || self.exports.iter().any(|export| export.binding == binding)
        {
            return Ok(false);
        }
        let is_read = |id: ExprId| matches!(self.expressions[id.index()], Expr::Binding(found) if found == binding);
        let mut reads = 0;
        for expression in &self.expressions {
            budget.work(Analysis, 1)?;
            let mut invalid = false;
            expression.visit_children(|child| {
                if is_read(child) {
                    if matches!(expression, Expr::Member { object, property: Property::Computed(_) } if *object == child) { reads += 1; }
                    else { invalid = true; }
                }
                Ok::<_, AllocationError>(())
            })?;
            if invalid {
                return Ok(false);
            }
            // Replacing a location with a call would change assignment and
            // invocation semantics. The source const proof also rejects writes.
            if let Expr::Assign { target, .. } = expression {
                if matches!(self.expressions[target.index()], Expr::Member { object, .. } if is_read(object))
                {
                    return Ok(false);
                }
            }
        }
        for region in &self.regions {
            for statement in &region.statements {
                budget.work(Analysis, 1)?;
                let mut escaped = false;
                statement.visit_expressions(|value| escaped |= is_read(value));
                if escaped {
                    return Ok(false);
                }
            }
        }
        Ok(reads > 0)
    }

    /// The cache cell is initialized at the original declaration. Its value
    /// is the first actual argument, so an early access throws before its key
    /// is evaluated, just as `table[key]` did. The input stays live in the body.
    pub(super) fn lazy_table_helper(
        &mut self,
        table: BindingId,
        decoded: ExprId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(BindingId, Statement), AllocationError> {
        self.binding_mut(table).class = None;
        let root_scope = self.regions[self.root.index()].scope;
        let mut emit = Emit {
            module: self,
            budget,
        };
        let helper = emit.binding(root_scope, "lookup")?;
        let (body, scope) = emit.region(root_scope)?;
        let current = emit.binding(scope, "cached")?;
        let key = emit.binding(scope, "key")?;
        let value = emit.read(current)?;
        let target = emit.read(table)?;
        let store = emit.assign(target, decoded)?;
        let value = emit.binary(Binary::Or, value, store)?;
        let key_read = emit.read(key)?;
        let result = emit.node(Expr::Member {
            object: value,
            property: Property::Computed(key_read),
        })?;
        emit.budget.push(
            AllocationClass::Retained,
            &mut emit.module.regions[body.index()].statements,
            Statement::Return(Some(result)),
        )?;
        let parameters = emit
            .budget
            .copy_slice(AllocationClass::Retained, &[current, key])?;
        let function = emit.function(parameters, body)?;
        Ok((
            helper,
            Statement::Function {
                binding: helper,
                function,
            },
        ))
    }

    pub(super) fn lazy_table_reads(
        &mut self,
        table: BindingId,
        helper: BindingId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let count = self.expressions.len();
        for index in 0..count {
            budget.work(Analysis, 1)?;
            let Expr::Member {
                object,
                property: Property::Computed(key),
            } = self.expressions[index]
            else {
                continue;
            };
            if !matches!(self.expressions[object.index()], Expr::Binding(binding) if binding == table)
            {
                continue;
            }
            let callee = Emit {
                module: self,
                budget,
            }
            .read(helper)?;
            let arguments = budget.copy_slice(AllocationClass::Retained, &[object, key])?;
            *self.expression_mut(ExprId::try_new(index).ok_or(AllocationError::Capacity)?) =
                Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Value,
                };
        }
        Ok(())
    }
}
