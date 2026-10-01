//! Public const data has exact keys and immutable identity. Freeze only this
//! explicitly published boundary; private data uses the shared const proof.
//! The helper is typed target IR with an owning module, ordinary dependencies,
//! demand pruning, naming and full-artifact scoring.
use super::*;
impl Formation<'_, '_, '_, '_, '_> {
    pub(super) fn freeze_const_boundary(
        &mut self,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let helper = match self
            .module
            .const_freezers
            .iter()
            .find(|(owner, _)| *owner == self.current_module)
            .map(|(_, helper)| *helper)
        {
            Some(helper) => helper,
            None => {
                let root = self.module.root;
                let scope = self.module.regions[root.index()].scope;
                let helper = self.fresh_binding(scope, "const_data")?;
                self.budget.push(
                    AllocationClass::Retained,
                    &mut self.module.const_freezers,
                    (self.current_module, helper),
                )?;
                let body = self.module.region_in(scope, self.budget)?;
                let body_scope = self.module.regions[body.index()].scope;
                let input = self.fresh_binding(body_scope, "value")?;
                let input_read = self.reference(input)?;
                let typeof_input = self.expression(js::Expr::Unary {
                    op: js::Unary::TypeOf,
                    value: input_read,
                })?;
                let object_text = self.text("object")?;
                let object = self.literal(js::Literal::String(object_text.into()))?;
                let object_test = self.expression(js::Expr::Binary {
                    op: js::Binary::StrictEqual,
                    left: typeof_input,
                    right: object,
                })?;
                let input_read = self.reference(input)?;
                let condition = self.expression(js::Expr::Binary {
                    op: js::Binary::And,
                    left: input_read,
                    right: object_test,
                })?;
                let objects = self.module.region_in(body_scope, self.budget)?;
                let object_scope = self.module.regions[objects.index()].scope;
                let iteration = self.module.region_in(object_scope, self.budget)?;
                let key =
                    self.fresh_binding(self.module.regions[iteration.index()].scope, "key")?;
                let keys = self.host_path(&["Object", "keys"])?;
                let input_read = self.reference(input)?;
                let arguments = self
                    .budget
                    .copy_slice(AllocationClass::Retained, &[input_read])?;
                let keys = self.expression(js::Expr::Call {
                    callee: keys,
                    arguments,
                    invocation: Invocation::Value,
                })?;
                let input_read = self.reference(input)?;
                let key_read = self.reference(key)?;
                let child = self.expression(js::Expr::Member {
                    object: input_read,
                    property: js::Property::Computed(key_read),
                })?;
                let callee = self.reference(helper)?;
                let arguments = self
                    .budget
                    .copy_slice(AllocationClass::Retained, &[child])?;
                let recursive = self.expression(js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Value,
                })?;
                self.statement(iteration, js::Statement::Evaluate(recursive))?;
                self.statement(
                    objects,
                    js::Statement::ForOf {
                        binding: key,
                        iterable: keys,
                        body: iteration,
                    },
                )?;
                let freeze = self.host_path(&["Object", "freeze"])?;
                let input_read = self.reference(input)?;
                let arguments = self
                    .budget
                    .copy_slice(AllocationClass::Retained, &[input_read])?;
                let freeze = self.expression(js::Expr::Call {
                    callee: freeze,
                    arguments,
                    invocation: Invocation::Value,
                })?;
                self.statement(objects, js::Statement::Evaluate(freeze))?;
                self.statement(
                    body,
                    js::Statement::If {
                        condition,
                        yes: objects,
                        no: None,
                    },
                )?;
                let input_read = self.reference(input)?;
                self.statement(body, js::Statement::Return(Some(input_read)))?;
                let parameters = self
                    .budget
                    .copy_slice(AllocationClass::Retained, &[input])?;
                let function = js::FunctionId::try_new(self.module.functions.len())
                    .ok_or(AllocationError::Capacity)?;
                self.budget.push(
                    AllocationClass::Retained,
                    &mut self.module.functions,
                    js::Function {
                        rest: false,
                        parameters,
                        body,
                        arrow: false,
                        name: js::FunctionName::Unobserved,
                        strict: false,
                        length: None,
                        suspension: js::Suspension::None,
                    },
                )?;
                self.helper_statement(
                    root,
                    js::Statement::Function {
                        binding: helper,
                        function,
                    },
                )?;
                helper
            }
        };
        let callee = self.reference(helper)?;
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, &[value])?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }
}
