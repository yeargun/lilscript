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

    /// `s.codeUnitAt(i)` under development checks: the receiver
    /// `index_checked(s, i)` and the argument 0.
    pub(super) fn code_unit_check(
        &mut self,
        receiver: js::ExprId,
        arguments: Vec<js::ExprId>,
    ) -> Result<(js::ExprId, Vec<js::ExprId>), FormationError> {
        let [index] = arguments[..] else {
            return Err(self.error(Span::default(), "codeUnitAt takes one index"));
        };
        let helper = self.index_helper()?;
        let callee = self.reference(helper)?;
        let mut checked = self.budget.vector(AllocationClass::Retained, 2)?;
        self.append(&mut checked, receiver)?;
        self.append(&mut checked, index)?;
        let unit = self.expression(js::Expr::Call {
            callee,
            arguments: checked,
            invocation: Invocation::Value,
        })?;
        let zero = self.literal(js::Literal::Number(0.0))?;
        let mut first = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut first, zero)?;
        Ok((unit, first))
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

/// A crossing's checkable shape (R1): what `typeof` or `Array.isArray` tells.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum Crossing {
    Int,
    Float,
    String,
    Bool,
    Array,
    Function,
}

impl Crossing {
    /// The shape of a value of `ty`, and whether absence is one, or none
    /// where no cheap test tells (a class, a struct, a map, a `JsValue`).
    fn of(ty: &Type<'_>) -> Option<(Self, bool)> {
        Some(match ty {
            Type::Int => (Self::Int, false),
            Type::Float => (Self::Float, false),
            Type::String => (Self::String, false),
            Type::Bool => (Self::Bool, false),
            Type::Array(_) => (Self::Array, false),
            Type::Function(_) => (Self::Function, false),
            Type::Nullable(inner) => (Self::of(inner)?.0, true),
            _ => return None,
        })
    }

    fn name(self) -> &'static str {
        match self {
            Self::Int => "int",
            Self::Float => "float",
            Self::String => "string",
            Self::Bool => "bool",
            Self::Array => "array",
            Self::Function => "function",
        }
    }
}

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    /// Whether this call's result crosses into the program (R1): a call of an
    /// `extern` function, a method call on a host object, or a trusted view
    /// (`v as T`).
    pub(super) fn crossing_call(&self, unit: ContextId, call: CallId) -> bool {
        if self.contract.checks != PreconditionChecks::Development {
            return false;
        }
        let data = self.data(unit);
        match data.calls[call.index()].target {
            CallTarget::Value { callee, .. } => {
                let OperationKind::Load(place) =
                    data.operations[data.values[callee.index()].definition.index()].kind
                else {
                    return false;
                };
                matches!(data.places[place.index()], Place::Cell(cell)
                    if self.program.cells[cell.index()].binding == CellBinding::Foreign)
            }
            CallTarget::Reference { .. } | CallTarget::Builtin(BuiltinCall::JsAssume) => true,
            _ => false,
        }
    }

    /// `value` checked against `ty` where a value crosses into the program
    /// under development checks, else `value`.
    pub(super) fn crossing_check(
        &mut self,
        ty: TypeId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if self.contract.checks != PreconditionChecks::Development {
            return Ok(value);
        }
        let Some((kind, absent)) = Crossing::of(&self.program.types[ty.index()]) else {
            return Ok(value);
        };
        let helper = self.crossing_helper(kind, absent)?;
        let callee = self.reference(helper)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut arguments, value)?;
        Ok(self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })?)
    }

    /// `function string_checked(v){if(!(typeof v==="string"))throw new
    /// TypeError(…);return v}`, formed once per shape, absence allowed or not.
    fn crossing_helper(
        &mut self,
        kind: Crossing,
        absent: bool,
    ) -> Result<js::BindingId, FormationError> {
        if let Some(&(_, binding)) = self
            .crossing_checks
            .iter()
            .find(|(key, _)| *key == (kind, absent))
        {
            return Ok(binding);
        }
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let body = self.module.region_in(scope, self.budget)?;
        let body_scope = self.module.regions[body.index()].scope;
        let value = self.fresh_binding(body_scope, "value")?;
        let typeof_is = |this: &mut Self, name: &str| -> Result<js::ExprId, FormationError> {
            let read = this.reference(value)?;
            let kind = this.expression(js::Expr::Unary {
                op: js::Unary::TypeOf,
                value: read,
            })?;
            let name = this.string(&crate::literal::StringValue::from(name))?;
            let name = this.literal(js::Literal::String(name))?;
            Ok(this.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left: kind,
                right: name,
            })?)
        };
        let mut test = match kind {
            Crossing::Int => {
                let number = typeof_is(self, "number")?;
                let read = self.reference(value)?;
                let zero = self.literal(js::Literal::Number(0.0))?;
                let truncated = self.expression(js::Expr::Binary {
                    op: js::Binary::BitOr,
                    left: read,
                    right: zero,
                })?;
                let read = self.reference(value)?;
                let int32 = self.expression(js::Expr::Binary {
                    op: js::Binary::StrictEqual,
                    left: truncated,
                    right: read,
                })?;
                self.expression(js::Expr::Binary {
                    op: js::Binary::And,
                    left: number,
                    right: int32,
                })?
            }
            Crossing::Float => typeof_is(self, "number")?,
            Crossing::String => typeof_is(self, "string")?,
            Crossing::Bool => typeof_is(self, "boolean")?,
            Crossing::Function => typeof_is(self, "function")?,
            Crossing::Array => {
                let array = self.text("Array")?;
                let array = self.expression(js::Expr::Host(js::Host::new(array)))?;
                let is_array = self.text("isArray")?;
                let callee = self.expression(js::Expr::Member {
                    object: array,
                    property: js::Property::Named(is_array),
                })?;
                let read = self.reference(value)?;
                let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
                self.append(&mut arguments, read)?;
                self.expression(js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Reference,
                })?
            }
        };
        if absent {
            let read = self.reference(value)?;
            let null = self.literal(js::Literal::Null)?;
            let missing = self.expression(js::Expr::Binary {
                op: js::Binary::Equal,
                left: read,
                right: null,
            })?;
            test = self.expression(js::Expr::Binary {
                op: js::Binary::Or,
                left: missing,
                right: test,
            })?;
        }
        let wrong = self.expression(js::Expr::Unary {
            op: js::Unary::Not,
            value: test,
        })?;
        let fail = self.module.region_in(body_scope, self.budget)?;
        let error = self.text("TypeError")?;
        let error = self.expression(js::Expr::Host(js::Host::new(error)))?;
        let message = format!(
            "a value crosses into the program as {}{} and is not one",
            kind.name(),
            if absent { "?" } else { "" }
        );
        let message = self.string(&crate::literal::StringValue::from(message.as_str()))?;
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
                condition: wrong,
                yes: fail,
                no: None,
            },
        )?;
        let returned = self.reference(value)?;
        self.statement(body, js::Statement::Return(Some(returned)))?;
        let mut parameters = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut parameters, value)?;
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
        let binding = self.fresh_binding(scope, &format!("{}_checked", kind.name()))?;
        self.helper_statement(root, js::Statement::Function { binding, function })?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.crossing_checks,
            ((kind, absent), binding),
        )?;
        Ok(binding)
    }
}

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    /// At an exported function's entry, under development checks, each
    /// parameter a caller must pass (no default: an omitted one is
    /// `undefined` until its default applies), by value, in plain storage,
    /// and of a checkable shape is checked as a crossing (R1).
    pub(super) fn export_parameter_checks(
        &mut self,
        unit: ContextId,
        result: ValueId,
        child: ContextId,
        body: js::RegionId,
    ) -> Result<(), FormationError> {
        if self.contract.checks != PreconditionChecks::Development
            || !self.flows_into_exported_cell(unit, result)?
        {
            return Ok(());
        }
        let program = self.program;
        let Some(Type::Function(signature)) = self
            .data(child)
            .callable_type
            .map(|ty| &program.types[ty.index()])
        else {
            return Ok(());
        };
        let parameters = self.data(child).parameters.clone();
        for (index, &cell) in parameters.iter().enumerate() {
            self.work(1)?;
            let Some(parameter) = signature.params.get(index) else {
                continue;
            };
            if parameter.default.is_some()
                || parameter.passing != crate::primitive::ParameterPassing::Value
                || self.unbound_cells.contains(&cell)
                || self.activation_cells.contains(&cell)
                || references::is_reference(program, cell)
            {
                continue;
            }
            let ty = program.cells[cell.index()].ty;
            if Crossing::of(&program.types[ty.index()]).is_none() {
                continue;
            }
            let binding = self.cell_binding(child, cell)?;
            let read = self.reference(binding)?;
            let checked = self.crossing_check(ty, read)?;
            self.statement(body, js::Statement::Evaluate(checked))?;
        }
        Ok(())
    }
}
