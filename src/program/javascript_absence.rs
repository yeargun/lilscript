//! R2 boundary transport. Internal absence is unconstrained; a public slot or
//! callable parameter owns its spelling. Adapters evaluate each operand once.
use super::*;
use crate::check::{
    AbsencePin, FunctionParameter, binary_types::TypeConstructionAdmission,
    type_admission::TypeQueryAdmission,
};

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    /// A collection owns shared mutable storage. Normalize its optional element
    /// when it is written, preserving the collection's identity across aliases.
    /// This only changes absence; a private product keeps its private layout.
    pub(super) fn stored_absence(
        &mut self,
        ty: &Type<'src>,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let Type::Nullable(inner) = ty else {
            return Ok(value);
        };
        if !self.program.source_contract.unified_absence() && inner.boundary == AbsencePin::Auto {
            return Ok(value);
        }
        // The nullable codec over a dynamic present value has no product or
        // callable adaptation. Only the absence pin is relevant to storage.
        let pinned = Type::pinned_nullable(
            TypeQueryAdmission::new(self.budget).box_type(Type::Dynamic)?,
            inner.boundary,
        );
        self.public_value(&pinned, value, false)
    }

    pub(super) fn array_element(
        &mut self,
        unit: ContextId,
        result: ValueId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let Type::Array(element) =
            &self.program.types[self.data(unit).values[result.index()].ty.index()]
        else {
            return Ok(value);
        };
        self.stored_absence(element, value)
    }

    pub(super) fn array_spread(
        &mut self,
        unit: ContextId,
        result: ValueId,
        source: ValueId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let Type::Array(expected) =
            &self.program.types[self.data(unit).values[result.index()].ty.index()]
        else {
            return Ok(value);
        };
        let Type::Array(actual) =
            &self.program.types[self.data(unit).values[source.index()].ty.index()]
        else {
            return Ok(value);
        };
        let Type::Nullable(pin) = expected.as_ref() else {
            return Ok(value);
        };
        if crate::check::absence::same_storage_pin(expected, actual) {
            return Ok(value);
        }
        let present = TypeQueryAdmission::new(self.budget).box_type(Type::Dynamic)?;
        let codec = self.public_codec(&Type::pinned_nullable(present, pin.boundary), false)?;
        let mapper = self.reference(codec)?;
        let callee = self.host_path(&["Array", "from"])?;
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, &[value, mapper])?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Reference,
        })
    }

    pub(super) fn host_call_boundary(&self, unit: ContextId, call: CallId) -> bool {
        let data = self.data(unit);
        match data.calls[call.index()].target {
            CallTarget::Value { callee, .. } => {
                match data.operations[data.values[callee.index()].definition.index()].kind {
                    OperationKind::Load(place) => {
                        matches!(data.places[place.index()], Place::Cell(cell)
                    if self.program.cells[cell.index()].binding == CellBinding::Foreign)
                    }
                    _ => matches!(
                        self.program.types[data.values[callee.index()].ty.index()],
                        Type::Dynamic
                    ),
                }
            }
            CallTarget::Reference { .. } => true,
            CallTarget::Builtin(builtin) => crate::catalog::host_builtin(builtin),
            _ => false,
        }
    }

    pub(super) fn outgoing_parameter(
        &mut self,
        parameter: &FunctionParameter<'src>,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if parameter.optional && self.program.source_contract.unified_absence() {
            let inner = TypeQueryAdmission::new(self.budget).clone_type(&parameter.ty)?;
            let inner = TypeQueryAdmission::new(self.budget).box_type(inner)?;
            let ty = Type::pinned_nullable(inner, AbsencePin::Undefined);
            self.public_value(&ty, value, false)
        } else {
            self.public_value(&parameter.ty, value, false)
        }
    }

    pub(super) fn boundary_argument(
        &mut self,
        unit: ContextId,
        call: CallId,
        position: u32,
        source: ValueId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let data = self.data(unit);
        let actual = &self.program.types[data.values[source.index()].ty.index()];
        if !self.program.absence_abi && !public_structs::carries_product(actual, self.budget)? {
            return Ok(value);
        }
        let site = &data.calls[call.index()];
        let signature = site
            .contract
            .instantiation
            .map(|id| data.call_instantiations[id.index()].signature)
            .or(site.contract.signature)
            .and_then(|id| self.program.types[id.index()].callable_signature());
        let parameter = signature.and_then(|signature| signature.params.get(position as usize));
        if let CallTarget::Intrinsic {
            operation: ResolvedIntrinsic::Method(operation),
            receiver: Some(receiver),
        } = site.target
        {
            let receiver = &self.program.types[data.values[receiver.index()].ty.index()];
            let slot = match (receiver, operation, position) {
                (Type::Array(element), Intrinsic::ArrayPush | Intrinsic::ArrayFill, 0) => {
                    Some(element.as_ref())
                }
                (Type::Map(_, value), Intrinsic::MapSet, 1) => Some(value.as_ref()),
                _ => None,
            };
            if let Some(slot) = slot {
                return self.stored_absence(slot, value);
            }
            if operation == Intrinsic::ArrayMap
                && position == 0
                && public_structs::carries_absence(self.program, actual, self.budget)?
            {
                if let Some(signature) = actual.callable_signature() {
                    if matches!(signature.return_type.as_ref(), Type::Nullable(_))
                        && !public_structs::carries_product(actual, self.budget)?
                    {
                        return self.public_callable_type(actual, value, false);
                    }
                }
            }
        }
        if self.host_call_boundary(unit, call) {
            if let Some(parameter) = parameter {
                if !matches!(parameter.ty, Type::Dynamic | Type::Unknown) {
                    return self.outgoing_parameter(parameter, value);
                }
            }
            // A JsValue destination pins the legacy null spelling. Product
            // values already encoded by their single-use transport stay so.
            if public_structs::carries_absence(self.program, actual, self.budget)? {
                return self.public_value(actual, value, false);
            }
        } else if parameter.is_some_and(|parameter| {
            matches!(
                parameter.ty,
                Type::Dynamic | Type::Unknown | Type::TypeParameter(_)
            )
        }) {
            return self.absence_erasure(actual, value);
        }
        Ok(value)
    }

    pub(super) fn absence_erasure(
        &mut self,
        actual: &Type<'src>,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if public_structs::carries_absence(self.program, actual, self.budget)? {
            self.public_value(actual, value, false)
        } else {
            Ok(value)
        }
    }

    pub(super) fn boundary_cell(
        &mut self,
        cell: CellId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if self.contract.abi.preserve_root_exports {
            for (_, exported) in self.program.value_exports() {
                self.work(1)?;
                if exported == cell
                    && !matches!(
                        self.program.cells[cell.index()].binding,
                        CellBinding::Function(_)
                    )
                {
                    let ty = &self.program.types[self.program.cells[cell.index()].ty.index()];
                    // Stored products keep their private layout. A separate
                    // public graph is initialized beside this cell below.
                    if self.program.cells[cell.index()].declared_const
                        && super::public_structs::carries_product(ty, self.budget)? {
                        return Ok(value);
                    }
                    let value = self.public_value(ty, value, false)?;
                    return if self.program.cells[cell.index()].declared_const && super::super::rules::const_data::aggregate(ty) {
                        if !self.module.pristine_builtins { self.anchor = js::Anchor::Anchored; }
                        self.freeze_const_boundary(value)
                    } else { Ok(value) };
                }
            }
        }
        Ok(value)
    }

    pub(super) fn checked_absence_pin(
        &mut self,
        ty: &Type<'src>,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let Type::Nullable(inner) = ty else {
            return Ok(value);
        };
        if inner.boundary == AbsencePin::Auto
            || self.contract.checks != crate::compilation_contract::PreconditionChecks::Development
        {
            return Ok(value);
        }
        let present = TypeQueryAdmission::new(self.budget).box_type(Type::Dynamic)?;
        self.public_value(&Type::pinned_nullable(present, inner.boundary), value, true)
    }

    pub(super) fn absence_reject(
        &mut self,
        body: js::RegionId,
        condition: js::ExprId,
    ) -> Result<(), FormationError> {
        self.boundary_reject(body, condition, "wrong absence spelling at a declared boundary (R2)")
    }

    pub(super) fn boundary_reject(&mut self, body: js::RegionId, condition: js::ExprId, message: &str)
        -> Result<(), FormationError> {
        let scope = self.module.regions[body.index()].scope;
        let fail = self.module.region_in(scope, self.budget)?;
        let callee = self.host_path(&["TypeError"])?;
        let message = self.string(&message.into())?;
        let message = self.literal(js::Literal::String(message))?;
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, &[message])?;
        let error = self.expression(js::Expr::Construct { callee, arguments })?;
        self.statement(fail, js::Statement::Throw(error))?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: fail,
                no: None,
            },
        )
    }
}
