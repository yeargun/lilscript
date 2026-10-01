//! Closed identities do not read a later source declaration through its TDZ.
use super::*;

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    pub(super) fn prepare_class_witnesses(&mut self) -> Result<(), FormationError> {
        let program = self.program;
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        for unit in program.units() {
            for operation in &unit.data().operations {
                self.work(1)?;
                let OperationKind::ClosedClassTest(class) = operation.kind else {
                    continue;
                };
                self.work(self.class_witnesses.len())?;
                if self
                    .class_witnesses
                    .iter()
                    .any(|&(identity, _)| identity == class)
                {
                    continue;
                }
                let binding = self.fresh_binding(scope, "class_witness")?;
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut self.class_witnesses,
                    (class, binding),
                )?;
                // This declaration precedes every module's initialization. It
                // is an internal identity slot, not the user's class binding.
                self.helper_statement(
                    root,
                    js::Statement::Let {
                        binding,
                        value: None,
                    },
                )?;
            }
        }
        Ok(())
    }

    pub(super) fn class_witness_registration(
        &mut self,
        class: NominalId,
        expression: js::Expr,
    ) -> Result<js::Expr, FormationError> {
        self.work(self.class_witnesses.len())?;
        let Some((_, binding)) = self
            .class_witnesses
            .iter()
            .find(|&&(id, _)| id == class)
            .copied()
        else {
            return Ok(expression);
        };
        let value = self.expression(expression)?;
        let target = self.reference(binding)?;
        Ok(js::Expr::Assign { target, value })
    }

    pub(super) fn closed_class_test(
        &mut self,
        class: NominalId,
        value: js::ExprId,
    ) -> Result<js::Expr, FormationError> {
        self.work(self.class_witnesses.len())?;
        let binding = self
            .class_witnesses
            .iter()
            .find(|&&(id, _)| id == class)
            .map(|&(_, binding)| binding)
            .ok_or_else(|| {
                self.error(Span::default(), "closed class without an identity witness")
            })?;
        let left = self.reference(binding)?;
        let right = self.literal(js::Literal::Undefined)?;
        let ready = self.expression(js::Expr::Binary {
            op: js::Binary::StrictNotEqual,
            left,
            right,
        })?;
        let right = self.reference(binding)?;
        let test = self.expression(js::Expr::Binary {
            op: js::Binary::InstanceOf,
            left: value,
            right,
        })?;
        Ok(js::Expr::Binary {
            op: js::Binary::And,
            left: ready,
            right: test,
        })
    }
}
