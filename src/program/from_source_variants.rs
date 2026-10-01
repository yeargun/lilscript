//! Closed dispatch and payload matches become ordinary shared operations.
use super::*;

impl<'budget, 'ledger, 'sem, 'ast, 'src> Lower<'budget, 'ledger, 'sem, 'ast, 'src> {
    pub(super) fn identity_test_value(
        &mut self,
        unit: UnitId,
        region: RegionId,
        value: ValueId,
        target: TypeId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let checked = self.program.types[target.index()].clone();
        if let Type::Class(declaration) = &checked {
            if !self.class_info(declaration.identity, span)?.shape
                && !self.class_info(declaration.identity, span)?.external
            {
                let boolean = self.ty(&Type::Bool)?;
                return self.value(
                    unit,
                    region,
                    OperationKind::ClosedClassTest(declaration.identity),
                    &[value],
                    boolean,
                    None,
                    span,
                );
            }
        }
        let constructor = self.identity_test_constructor(unit, region, &checked, span)?;
        let boolean = self.ty(&Type::Bool)?;
        let pair;
        let operands = if let Some(constructor) = constructor {
            pair = [value, constructor];
            &pair[..]
        } else {
            std::slice::from_ref(&value)
        };
        self.value(
            unit,
            region,
            OperationKind::TypeTest(target),
            operands,
            boolean,
            None,
            span,
        )
    }

    pub(super) fn bind_match_payload(
        &mut self,
        unit: UnitId,
        region: RegionId,
        value: ValueId,
        pattern: ast::MatchPattern<'src>,
    ) -> Result<(), ConversionError> {
        let ast::MatchPattern::Payload { binding, span, .. } = pattern else {
            return Ok(());
        };
        let cell = self.declare(unit, region, binding)?;
        let target = self.program.cells[cell.index()].ty;
        let value = self.dynamic_call(
            unit,
            region,
            BuiltinCall::JsAssume,
            &[DynamicOperand::Value(value)],
            target,
            None,
            span,
        )?;
        self.effect(
            unit,
            region,
            OperationKind::Initialize(cell),
            &[value],
            span,
        )
    }

    pub(super) fn virtual_call(
        &mut self,
        unit: UnitId,
        region: RegionId,
        receiver: ValueId,
        name: &str,
        fallback: ClassMethod<'src>,
        arguments: &'ast [ast::Argument<'ast, 'src>],
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        let receiver_ty = self.units[unit.index()].values[receiver.index()].ty;
        let root = base_class(&self.program.types[receiver_ty.index()]).ok_or(Unsupported {
            span,
            feature: "virtual receiver lost its class identity",
        })?;
        let semantics = self.semantics;
        let mut targets = self.budget.vector(Scratch, 0)?;
        for candidate in semantics.classes() {
            self.work(1)?;
            let identity = candidate.declaration.identity;
            if identity == root {
                continue;
            }
            let Some(method) = candidate.methods.get(name) else {
                continue;
            };
            // Only classes that introduce an override need a branch. An
            // inherited implementation uses its nearest ancestor's branch.
            if method.owner != identity || method.member == fallback.member.unwrap() {
                continue;
            }
            let mut current = candidate.base.as_ref().and_then(base_class);
            let mut depth = 1usize;
            while current.is_some_and(|id| id != root) {
                self.work(1)?;
                current = self
                    .class_info(current.unwrap(), span)?
                    .base
                    .as_ref()
                    .and_then(base_class);
                depth += 1;
            }
            if current != Some(root) {
                continue;
            }
            let implementation =
                self.class_method(identity, Some(method.member))?
                    .ok_or(Unsupported {
                        span,
                        feature: "virtual implementation has no checked body",
                    })?;
            let target = self.ty(&Type::Class(candidate.declaration))?;
            self.budget.push(
                Scratch,
                &mut targets,
                (depth, identity, target, implementation),
            )?;
        }
        // Most-derived implementations win, independently of source/module order.
        targets.sort_unstable_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        let signature = self.program.cells[fallback.cell.index()].ty;
        let mut pending = self.budget.vector(Scratch, targets.len())?;
        let mut current = region;
        for &(_, _, target, implementation) in &targets {
            let test = self.identity_test_value(unit, current, receiver, target, span)?;
            let yes = self.region(unit, current, span)?;
            let callee = self.virtual_method_value(unit, yes, implementation, signature, span)?;
            self.units[unit.index()].regions[yes.index()].result = Some(callee);
            let no = self.region(unit, current, span)?;
            self.budget
                .push(Scratch, &mut pending, (current, test, yes, no))?;
            current = no;
        }
        let mut callee = self.virtual_method_value(unit, current, fallback, signature, span)?;
        for (parent, test, yes, no) in pending.iter().copied().rev() {
            self.units[unit.index()].regions[no.index()].result = Some(callee);
            callee = self.value(
                unit,
                parent,
                OperationKind::Select { yes, no },
                &[test],
                signature,
                None,
                span,
            )?;
        }
        drop_vector(pending, Scratch, self.budget)?;
        drop_vector(targets, Scratch, self.budget)?;
        // Select the body before evaluating arguments. They are lowered once:
        // lambdas retain one ownership tree and ref preparations keep their
        // exact position among argument effects. Defaults run in that body.
        let result_type = self.class_call_result(unit, signature, None, span)?;
        let contract = CallContract {
            signature: Some(signature),
            instantiation: None,
            supplied: u32::try_from(arguments.len() + 1).map_err(|_| AllocationError::Capacity)?,
            defaults: DefaultConvention::ApplyAtCallee,
        };
        let (kind, operands, _) = self.prepare_call_with_receiver(
            unit,
            region,
            CallTarget::Value {
                callee,
                invocation: Invocation::Value,
            },
            contract,
            Some(CallReceiver::Value(receiver)),
            arguments,
            span,
        )?;
        let result = self.value(unit, region, kind, &operands, result_type, None, span)?;
        drop_vector(operands, Scratch, self.budget)?;
        Ok(result)
    }

    fn virtual_method_value(
        &mut self,
        unit: UnitId,
        region: RegionId,
        method: ClassMethod<'src>,
        signature: TypeId,
        span: Span,
    ) -> Result<ValueId, ConversionError> {
        self.reference(unit, method.cell)?;
        let callee = self.load_cell(unit, region, method.cell, span)?;
        if self.program.cells[method.cell.index()].ty == signature {
            return Ok(callee);
        }
        // The checked override has the same call contract. Only its receiver
        // is narrower, justified by the immediately enclosing identity test.
        self.dynamic_call(
            unit,
            region,
            BuiltinCall::JsAssume,
            &[DynamicOperand::Value(callee)],
            signature,
            None,
            span,
        )
    }
}
