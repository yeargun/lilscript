//! Closed identities do not read a later source declaration through its TDZ.
use super::*;

pub(super) struct PayloadFactory {
    pub(super) body: js::RegionId,
    parameters: Vec<js::BindingId>,
    arguments: Vec<js::ExprId>,
    captures: Vec<(CellId, js::BindingId)>,
}

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    pub(super) fn payload_cell(&mut self, cell: CellId) -> Result<bool, FormationError> {
        self.work((usize::BITS - self.payload_cells.len().leading_zeros()) as usize)?;
        Ok(self.payload_cells.binary_search(&cell).is_ok())
    }

    /// An expression region cannot declare a block-local binding. A captured
    /// binding therefore gets a fresh carrier at its original initialization;
    /// closures snapshot that carrier, leaving suspension in the source frame.
    pub(super) fn prepare_expression_bindings(
        &mut self,
        unit: UnitId,
        region: RegionId,
    ) -> Result<(), FormationError> {
        let program = self.program;
        for &id in &program.units[unit.index()].data().regions[region.index()].operations {
            self.work(1)?;
            let (OperationKind::Initialize(cell) | OperationKind::Declare(cell)) =
                program.units[unit.index()].data().operations[id.index()].kind
            else {
                continue;
            };
            if program.cells[cell.index()].source_symbol.is_none() {
                continue;
            }
            let mut captured = false;
            if let Some(uses) = self.uses.and_then(|uses| uses.cell(cell)) {
                for usage in uses.sites() {
                    self.work(1)?;
                    if matches!(
                        usage,
                        CellUseSite::Unit {
                            usage: CellUse::Capture,
                            ..
                        }
                    ) {
                        captured = true;
                        break;
                    }
                }
            } else {
                for body in program.units() {
                    self.work(body.data().captures.len() + 1)?;
                    if body.data().captures.contains(&cell) {
                        captured = true;
                        break;
                    }
                }
            }
            if captured {
                self.work((usize::BITS - self.payload_cells.len().leading_zeros()) as usize)?;
                if let Err(position) = self.payload_cells.binary_search(&cell) {
                    self.work(self.payload_cells.len() - position)?;
                    self.budget.reserve_vec(
                        AllocationClass::Scratch,
                        &mut self.payload_cells,
                        1,
                    )?;
                    self.payload_cells.insert(position, cell);
                }
            }
        }
        Ok(())
    }

    pub(super) fn payload_captures(&mut self, unit: UnitId) -> Result<bool, FormationError> {
        let program = self.program;
        for &cell in &program.units[unit.index()].data().captures {
            self.work(1)?;
            if self.payload_cell(cell)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(super) fn payload_factory(
        &mut self,
        context: ContextId,
        created: UnitId,
        parent: js::ScopeId,
    ) -> Result<Option<PayloadFactory>, FormationError> {
        if !self.payload_captures(created)? {
            return Ok(None);
        }
        let body = self.module.region_in(parent, self.budget)?;
        let scope = self.module.regions[body.index()].scope;
        let mut factory = PayloadFactory {
            body,
            parameters: Vec::new(),
            arguments: Vec::new(),
            captures: Vec::new(),
        };
        let program = self.program;
        for &cell in &program.units[created.index()].data().captures {
            self.work(1)?;
            if !self.payload_cell(cell)? {
                continue;
            }
            let binding = self.fresh_binding(scope, "payload")?;
            let carrier = self.cell_binding(context, cell)?;
            let argument = self.reference(carrier)?;
            self.append(&mut factory.parameters, binding)?;
            self.append(&mut factory.arguments, argument)?;
            self.budget.push(
                AllocationClass::Scratch,
                &mut factory.captures,
                (cell, binding),
            )?;
        }
        Ok(Some(factory))
    }

    pub(super) fn payload_capture_bindings(
        &mut self,
        context: ContextId,
        factory: &PayloadFactory,
    ) -> Result<(), FormationError> {
        for &(cell, binding) in &factory.captures {
            let captures = &mut self.contexts[context.index()].as_mut().unwrap().captures;
            self.budget.work(
                WorkKind::Render,
                (usize::BITS - captures.len().leading_zeros()) as u64,
            )?;
            match captures.binary_search_by_key(&cell, |&(cell, _)| cell) {
                Ok(position) => captures[position].1 = binding,
                Err(position) => {
                    self.budget
                        .work(WorkKind::Render, (captures.len() - position) as u64)?;
                    self.budget
                        .reserve_vec(AllocationClass::Scratch, captures, 1)?;
                    captures.insert(position, (cell, binding));
                }
            }
        }
        Ok(())
    }

    /// The factory creates a closure only. It never runs the arm or moves an
    /// await, host call, default argument, or lexical activation observation.
    pub(super) fn finish_payload_factory(
        &mut self,
        factory: Option<PayloadFactory>,
        node: js::Expr,
    ) -> Result<js::Expr, FormationError> {
        let Some(factory) = factory else {
            return Ok(node);
        };
        let value = self.expression(node)?;
        self.statement(factory.body, js::Statement::Return(Some(value)))?;
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters: factory.parameters,
                rest: false,
                body: factory.body,
                arrow: true,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: js::Suspension::None,
            },
        )?;
        self.drop_scratch(factory.captures)?;
        let callee = self.expression(js::Expr::Function(function))?;
        Ok(js::Expr::Call {
            callee,
            arguments: factory.arguments,
            invocation: Invocation::Value,
        })
    }
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
