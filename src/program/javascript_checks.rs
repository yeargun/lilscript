//! Development checks: the `checks` contract axis (language.md R11,
//! future-architecture's preconditions). Under `checks = "development"` an
//! index read of an array, a typed array or a string calls one hoisted
//! helper, which reads the element when the index is in range and throws a
//! `RangeError` where production reads past the end, an unspecified result.
//! A production build emits none of this.
use super::*;
use crate::compilation_contract::PreconditionChecks;

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    /// Whether a load of `place` is an index read the contract checks.
    pub(super) fn checked_index_read(&self, unit: ContextId, place: PlaceId) -> bool {
        if self.contract.checks != PreconditionChecks::Development {
            return false;
        }
        let Place::Index { receiver, .. } = self.data(unit).places[place.index()] else {
            return false;
        };
        let ty = &self.program.types[self.data(unit).values[receiver.index()].ty.index()];
        matches!(ty, Type::Array(_) | Type::String)
            || crate::typed_array::TypedArrayKind::from_type(ty).is_some()
    }

    /// `index_checked(a, i)` for the read `a[i]`: the receiver, then the
    /// index, evaluated once each, as the read evaluates them.
    pub(super) fn index_read_check(
        &mut self,
        unit: ContextId,
        place: PlaceId,
    ) -> Result<js::ExprId, FormationError> {
        let Place::Index { receiver, key } = self.data(unit).places[place.index()] else {
            return Err(self.error(Span::default(), "checked index read of another place"));
        };
        // A rest list spelled as formals has no array to check.
        if let Some(formal) = self.formal_read(unit, receiver, place)? {
            return Ok(formal);
        }
        let helper = self.index_helper()?;
        let callee = self.reference(helper)?;
        let object = self.value(unit, receiver)?;
        let key = self.value(unit, key)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, 2)?;
        self.append(&mut arguments, object)?;
        self.append(&mut arguments, key)?;
        Ok(self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })?)
    }

    /// The hoisted helper, formed once:
    /// `function index_checked(a,i){if(!(i>=0&&i<a.length))throw new RangeError("index out of range");return a[i]}`.
    fn index_helper(&mut self) -> Result<js::BindingId, FormationError> {
        if let Some(binding) = self.index_check {
            return Ok(binding);
        }
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let body = self.module.region_in(scope, self.budget)?;
        let body_scope = self.module.regions[body.index()].scope;
        let array = self.fresh_binding(body_scope, "array")?;
        let index = self.fresh_binding(body_scope, "index")?;
        let position = self.reference(index)?;
        let zero = self.literal(js::Literal::Number(0.0))?;
        let at_least_zero = self.expression(js::Expr::Binary {
            op: js::Binary::GreaterEqual,
            left: position,
            right: zero,
        })?;
        let position = self.reference(index)?;
        let receiver = self.reference(array)?;
        let length = self.text("length")?;
        let length = self.expression(js::Expr::Member {
            object: receiver,
            property: js::Property::Named(length),
        })?;
        let below_length = self.expression(js::Expr::Binary {
            op: js::Binary::Less,
            left: position,
            right: length,
        })?;
        let in_range = self.expression(js::Expr::Binary {
            op: js::Binary::And,
            left: at_least_zero,
            right: below_length,
        })?;
        let out_of_range = self.expression(js::Expr::Unary {
            op: js::Unary::Not,
            value: in_range,
        })?;
        let fail = self.module.region_in(body_scope, self.budget)?;
        let error = self.text("RangeError")?;
        let error = self.expression(js::Expr::Host(js::Host::new(error)))?;
        let message = self.string(&crate::literal::StringValue::from("index out of range"))?;
        let message = self.literal(js::Literal::String(message))?;
        let mut error_arguments = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut error_arguments, message)?;
        let thrown = self.expression(js::Expr::Construct {
            callee: error,
            arguments: error_arguments,
        })?;
        self.statement(fail, js::Statement::Throw(thrown))?;
        self.statement(
            body,
            js::Statement::If {
                condition: out_of_range,
                yes: fail,
                no: None,
            },
        )?;
        let receiver = self.reference(array)?;
        let position = self.reference(index)?;
        let element = self.expression(js::Expr::Member {
            object: receiver,
            property: js::Property::Computed(position),
        })?;
        self.statement(body, js::Statement::Return(Some(element)))?;
        let mut parameters = self.budget.vector(AllocationClass::Retained, 2)?;
        self.append(&mut parameters, array)?;
        self.append(&mut parameters, index)?;
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters,
                body,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::js::Suspension::None,
            },
        )?;
        let binding = self.fresh_binding(scope, "index_checked")?;
        // Hoisted, so a read during initialization finds it.
        self.helper_statement(root, js::Statement::Function { binding, function })?;
        self.index_check = Some(binding);
        Ok(binding)
    }
}
