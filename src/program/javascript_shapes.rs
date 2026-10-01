//! Shape ABI operations. Construction owns data keys; nullable writes omit
//! keys, and declared accessors keep their observable receiver operation.
use super::*;
use crate::check::binary_types::TypeConstructionAdmission;

#[derive(Clone, PartialEq, Eq)]
pub(super) enum Helper<'src> {
    Clean(TypeId),
    Store(FieldRef),
    Check(Type<'src>),
    Test(TypeId),
    Update(Type<'src>, Vec<usize>),
}

impl<'demand, 'program, 'src> Formation<'demand, 'program, 'src, '_, '_> {
    fn shape_tag_literal(&mut self, tag: Constant) -> Result<js::ExprId, FormationError> {
        let literal = match tag {
            Constant::Integer(value) => js::Literal::Number(value as f64),
            Constant::Boolean(value) => js::Literal::Bool(value),
            Constant::String(id) => {
                js::Literal::String(self.string(&self.program.strings[id.index()])?)
            }
            _ => return Err(self.error(Span::default(), "invalid checked shape tag")),
        };
        self.literal(literal)
    }

    pub(super) fn shape_test(
        &mut self,
        ty: TypeId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let key = Helper::Test(ty);
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let Type::Class(declaration) = self.program.types[ty.index()] else {
                return Err(self.error(Span::default(), "shape test lacks nominal identity"));
            };
            let (slot, tag) = self
                .program
                .class(declaration.identity)
                .and_then(|class| class.discriminant)
                .ok_or_else(|| self.error(Span::default(), "shape test lacks a declared tag"))?;
            let (helper, body, parameters) = self.shape_helper_start(key, 1)?;
            let object = self.reference(parameters[0])?;
            let absent = self.nullish(object)?;
            let present = self.expression(js::Expr::Unary {
                op: js::Unary::Not,
                value: absent,
            })?;
            let object_type = self.expression(js::Expr::Unary {
                op: js::Unary::TypeOf,
                value: object,
            })?;
            let word = self.string(&"object".into())?;
            let word = self.literal(js::Literal::String(word))?;
            let object_type = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left: object_type,
                right: word,
            })?;
            let present = self.expression(js::Expr::Binary {
                op: js::Binary::And,
                left: present,
                right: object_type,
            })?;
            let left = self.shape_member(
                object,
                FieldRef {
                    nominal: declaration.identity,
                    slot,
                },
            )?;
            let right = self.shape_tag_literal(tag)?;
            let matches = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left,
                right,
            })?;
            let result = self.expression(js::Expr::Binary {
                op: js::Binary::And,
                left: present,
                right: matches,
            })?;
            self.statement(body, js::Statement::Return(Some(result)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        self.shape_call(helper, &[value])
    }

    fn checked_data_type(&self, ty: &Type<'src>) -> bool {
        let ty = match ty {
            Type::Nullable(inner) => inner.as_ref(),
            ty => ty,
        };
        super::super::schema::is_shape(self.program, ty)
            || super::super::schema::struct_definition(self.program, ty).is_some()
    }

    pub(super) fn require_storage_adapter(
        &mut self,
        ty: &Type<'src>,
        span: Span,
    ) -> Result<(), FormationError> {
        if !public_structs::adaptable(self.program, ty, 0, self.budget)? {
            return Err(self.error(span, "public field requires a value storage adapter"));
        }
        Ok(())
    }

    pub(super) fn public_storage_type(
        &mut self,
        unit: ContextId,
        place: PlaceId,
    ) -> Result<Option<std::borrow::Cow<'program, Type<'src>>>, FormationError> {
        let Place::ClassField { field, .. } = self.data(unit).places[place.index()] else {
            return Ok(None);
        };
        let definition = self.program.class(field.nominal).unwrap();
        if !definition.reflected && !definition.external {
            return Ok(None);
        }
        let ty =
            super::super::schema::place_type(self.program, self.data(unit), place, self.budget)?
                .ok_or_else(|| self.error(Span::default(), "missing public storage field"))?;
        if !public_structs::carries_product(&ty, self.budget)? {
            return Ok(None);
        }
        self.require_storage_adapter(&ty, Span::default())?;
        Ok(Some(ty))
    }

    pub(super) fn storage_decode(
        &mut self,
        unit: ContextId,
        place: PlaceId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if let Some(ty) = self.public_storage_type(unit, place)? {
            let value = if self.contract.checks
                == crate::compilation_contract::PreconditionChecks::Development
                && self.checked_data_type(&ty)
            {
                self.shape_crossing_type(&ty, value, None)?
            } else {
                value
            };
            self.public_value(&ty, value, true)
        } else {
            Ok(value)
        }
    }

    pub(super) fn storage_read(
        &mut self,
        unit: ContextId,
        place: PlaceId,
    ) -> Result<js::ExprId, FormationError> {
        let value = self.place(unit, place)?;
        self.storage_decode(unit, place, value)
    }

    /// Decode the current root once, after the scheduled RHS, update its private
    /// snapshot, then encode once. A host accessor is never duplicated while
    /// preserving sibling changes caused by reentry during the RHS.
    pub(super) fn public_product_update(
        &mut self,
        ty: &Type<'src>,
        path: &[structs::FieldRecipe],
        root: js::ExprId,
        replacement: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let mut slots = self.budget.vector(AllocationClass::Scratch, path.len())?;
        for recipe in path.iter().rev() {
            self.budget
                .push(AllocationClass::Scratch, &mut slots, recipe.slot)?;
        }
        let ty =
            crate::check::type_admission::TypeQueryAdmission::new(self.budget).clone_type(ty)?;
        let key = Helper::Update(ty, slots);
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let Helper::Update(ty, slots) = &key else {
                unreachable!()
            };
            let ty = crate::check::type_admission::TypeQueryAdmission::new(self.budget)
                .clone_type(ty)?;
            let slots = self.budget.copy_slice(AllocationClass::Scratch, slots)?;
            let (helper, body, parameters) = self.shape_helper_start(key, 2)?;
            let scope = self.module.regions[body.index()].scope;
            let object = self.fresh_binding(scope, "snapshot")?;
            let root = self.reference(parameters[0])?;
            let decoded = self.public_value(&ty, root, true)?;
            self.statement(
                body,
                js::Statement::Let {
                    binding: object,
                    value: Some(decoded),
                },
            )?;
            let mut target = self.reference(object)?;
            for slot in slots.iter() {
                target = self.slot(target, *slot)?;
            }
            self.drop_scratch(slots)?;
            let value = self.reference(parameters[1])?;
            let store = self.expression(js::Expr::Assign { target, value })?;
            self.statement(body, js::Statement::Evaluate(store))?;
            let object = self.reference(object)?;
            let encoded = self.public_value(&ty, object, false)?;
            self.statement(body, js::Statement::Return(Some(encoded)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        self.shape_call(helper, &[root, replacement])
    }

    /// Helpers reuse inert references, never executable occurrences. The target
    /// verifier deliberately rejects DAGs, even for repeated lexical reads.
    fn shape_read(&mut self, value: js::ExprId) -> Result<js::ExprId, FormationError> {
        self.work(1)?;
        match &self.module.expressions[value.index()] {
            js::Expr::Binding(binding) => self.reference(*binding),
            js::Expr::Member { object, property } => {
                let (object, property) = (*object, property.clone());
                let object = self.shape_read(object)?;
                let property = match property {
                    js::Property::Named(name) => js::Property::Named(self.text(&name)?),
                    js::Property::Computed(key) => js::Property::Computed(self.shape_read(key)?),
                };
                self.expression(js::Expr::Member { object, property })
            }
            // Literals may be shared. Other expressions are fresh call operands.
            _ => Ok(value),
        }
    }
    fn shape_named(
        &mut self,
        object: js::ExprId,
        name: &str,
    ) -> Result<js::ExprId, FormationError> {
        let object = self.shape_read(object)?;
        let property = js::Property::Named(self.text(name)?);
        self.expression(js::Expr::Member { object, property })
    }
    fn shape_invoke(
        &mut self,
        callee: js::ExprId,
        args: &[js::ExprId],
    ) -> Result<js::ExprId, FormationError> {
        let mut arguments = self.budget.vector(AllocationClass::Retained, args.len())?;
        for &arg in args {
            let value = self.shape_read(arg)?;
            self.append(&mut arguments, value)?;
        }
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Reference,
        })
    }
    fn shape_require(
        &mut self,
        body: js::RegionId,
        valid: js::ExprId,
    ) -> Result<(), FormationError> {
        let condition = self.expression(js::Expr::Unary {
            op: js::Unary::Not,
            value: valid,
        })?;
        let fail = self
            .module
            .region_in(self.module.regions[body.index()].scope, self.budget)?;
        let callee = self.host_path(&["TypeError"])?;
        let message = self.string(&"shape crossing violates its declared fields".into())?;
        let message = self.literal(js::Literal::String(message))?;
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, &[message])?;
        let thrown = self.expression(js::Expr::Construct { callee, arguments })?;
        self.statement(fail, js::Statement::Throw(thrown))?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes: fail,
                no: None,
            },
        )
    }
    pub(super) fn shape_crossing(
        &mut self,
        ty: TypeId,
        value: js::ExprId,
        seen: Option<js::ExprId>,
    ) -> Result<js::ExprId, FormationError> {
        let ty = &self.program.types[ty.index()];
        self.shape_crossing_type(ty, value, seen)
    }
    fn shape_crossing_type(
        &mut self,
        ty: &Type<'src>,
        value: js::ExprId,
        seen: Option<js::ExprId>,
    ) -> Result<js::ExprId, FormationError> {
        let key = Helper::Check(
            crate::check::type_admission::TypeQueryAdmission::new(self.budget).clone_type(ty)?,
        );
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let (helper, body, parameters) = self.shape_helper_start(key, 2)?;
            let scope = self.module.regions[body.index()].scope;
            let object = self.reference(parameters[0])?;
            let state = self.reference(parameters[1])?;
            let absent = matches!(ty, Type::Nullable(_));
            let schema = match ty {
                Type::Nullable(inner) => inner.as_ref(),
                ty => ty,
            };
            if absent {
                let condition = self.nullish(object)?;
                let yes = self.module.region_in(scope, self.budget)?;
                let returned = self.shape_read(object)?;
                self.statement(yes, js::Statement::Return(Some(returned)))?;
                self.statement(
                    body,
                    js::Statement::If {
                        condition,
                        yes,
                        no: None,
                    },
                )?;
            }
            let nullish = self.nullish(object)?;
            let present = self.expression(js::Expr::Unary {
                op: js::Unary::Not,
                value: nullish,
            })?;
            let typeof_value = self.expression(js::Expr::Unary {
                op: js::Unary::TypeOf,
                value: object,
            })?;
            let word = self.string(&"object".into())?;
            let word = self.literal(js::Literal::String(word))?;
            let object_type = self.expression(js::Expr::Binary {
                op: js::Binary::StrictEqual,
                left: typeof_value,
                right: word,
            })?;
            let valid = self.expression(js::Expr::Binary {
                op: js::Binary::And,
                left: present,
                right: object_type,
            })?;
            self.shape_require(body, valid)?;
            // Each schema has its own weak set in this crossing's state. A
            // recursive graph terminates; the same object under another view
            // is still checked, and a subsequent crossing starts fresh.
            let index = self.literal(js::Literal::Number(helper.index() as f64))?;
            let slot = self.expression(js::Expr::Member {
                object: state,
                property: js::Property::Computed(index),
            })?;
            let visited = self.fresh_binding(scope, "visited")?;
            self.statement(
                body,
                js::Statement::Let {
                    binding: visited,
                    value: Some(slot),
                },
            )?;
            let visited_value = self.reference(visited)?;
            let missing = self.nullish(visited_value)?;
            let callee = self.host_path(&["WeakSet"])?;
            let fresh = self.expression(js::Expr::Construct {
                callee,
                arguments: Vec::new(),
            })?;
            let slot = self.shape_read(slot)?;
            let assign_slot = self.expression(js::Expr::Assign {
                target: slot,
                value: fresh,
            })?;
            let assign_visited = self.assign(visited, assign_slot, None)?;
            self.shape_if(body, missing, assign_visited)?;
            let has = self.shape_named(visited_value, "has")?;
            let condition = self.shape_invoke(has, &[object])?;
            let yes = self.module.region_in(scope, self.budget)?;
            let returned = self.shape_read(object)?;
            self.statement(yes, js::Statement::Return(Some(returned)))?;
            self.statement(
                body,
                js::Statement::If {
                    condition,
                    yes,
                    no: None,
                },
            )?;
            let add = self.shape_named(visited_value, "add")?;
            let add = self.shape_invoke(add, &[object])?;
            self.statement(body, js::Statement::Evaluate(add))?;
            let mut fields = Vec::new();
            if let Some(definition) = super::super::schema::struct_definition(self.program, schema)
            {
                for field in &self.program.fields[definition.fields.clone()] {
                    let ty = super::super::schema::field_type(
                        self.program,
                        schema,
                        field,
                        &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget),
                    )?
                    .ok_or_else(|| self.error(Span::default(), "missing boundary struct field"))?;
                    let name = self.string(&field.name.as_str().into())?;
                    let key = self.literal(js::Literal::String(name))?;
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut fields,
                        (key, ty, false, None),
                    )?;
                }
            } else {
                let shape_fields = super::super::schema::shape_fields(
                    self.program,
                    schema,
                    &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget),
                )?
                .ok_or_else(|| self.error(Span::default(), "invalid shape crossing schema"))?;
                for (field, ty) in shape_fields {
                    let key = self.class_field_key(field)?;
                    let accessor =
                        self.program.class(field.nominal).unwrap().accessors[field.slot as usize];
                    let tag = self
                        .program
                        .class(field.nominal)
                        .unwrap()
                        .discriminant
                        .filter(|(slot, _)| *slot == field.slot)
                        .map(|(_, tag)| tag);
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut fields,
                        (key, std::borrow::Cow::Owned(ty), accessor, tag),
                    )?;
                }
            }
            for (key, field_ty, accessor, tag) in fields {
                self.work(1)?;
                let callee = self.host_path(&["Object", "getOwnPropertyDescriptor"])?;
                let read = self.shape_invoke(callee, &[object, key])?;
                let descriptor = self.fresh_binding(scope, "descriptor")?;
                self.statement(
                    body,
                    js::Statement::Let {
                        binding: descriptor,
                        value: Some(read),
                    },
                )?;
                let descriptor = self.reference(descriptor)?;
                let missing = self.nullish(descriptor)?;
                let present = self.expression(js::Expr::Unary {
                    op: js::Unary::Not,
                    value: missing,
                })?;
                let checked_body = if matches!(
                    field_ty.as_ref(),
                    Type::Nullable(_) | Type::Null | Type::TypeParameter(_)
                ) {
                    let yes = self.module.region_in(scope, self.budget)?;
                    self.statement(
                        body,
                        js::Statement::If {
                            condition: present,
                            yes,
                            no: None,
                        },
                    )?;
                    yes
                } else {
                    self.shape_require(body, present)?;
                    body
                };
                if accessor {
                    continue;
                }
                let key = self.string(&"value".into())?;
                let key = self.literal(js::Literal::String(key))?;
                let data = self.expression(js::Expr::Binary {
                    op: js::Binary::In,
                    left: key,
                    right: descriptor,
                })?;
                self.shape_require(checked_body, data)?;
                if let Some(tag) = tag {
                    let left = self.shape_named(descriptor, "value")?;
                    let right = self.shape_tag_literal(tag)?;
                    let equal = self.expression(js::Expr::Binary {
                        op: js::Binary::StrictEqual,
                        left,
                        right,
                    })?;
                    self.shape_require(checked_body, equal)?;
                }
                let value = self.shape_named(descriptor, "value")?;
                let checked = if self.checked_data_type(&field_ty) {
                    Some(self.shape_crossing_type(&field_ty, value, Some(state))?)
                } else if let Some((kind, absent)) = checks::Crossing::of(&field_ty) {
                    let helper = self.crossing_helper(kind, absent)?;
                    Some(self.shape_call(helper, &[value])?)
                } else {
                    None
                };
                if let Some(checked) = checked {
                    self.statement(checked_body, js::Statement::Evaluate(checked))?;
                }
            }
            let returned = self.shape_read(object)?;
            self.statement(body, js::Statement::Return(Some(returned)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        let state = match seen {
            Some(seen) => self.shape_read(seen)?,
            None => self.expression(js::Expr::Array(Vec::new()))?,
        };
        self.shape_call(helper, &[value, state])
    }
    fn shape_fields(&mut self, ty: TypeId) -> Result<Vec<(FieldRef, Type<'src>)>, FormationError> {
        let ty = match &self.program.types[ty.index()] {
            Type::Nullable(inner) => inner.as_ref(),
            ty => ty,
        };
        super::super::schema::shape_fields(
            self.program,
            ty,
            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget),
        )?
        .ok_or_else(|| self.error(Span::default(), "invalid checked shape schema"))
    }
    fn shape_helper(
        &mut self,
        key: &Helper<'src>,
    ) -> Result<Option<js::BindingId>, FormationError> {
        let mut query = crate::check::type_admission::TypeQueryAdmission::new(self.budget);
        for (cached, binding) in &self.shape_helpers {
            query.work(1)?;
            let equal = match (cached, key) {
                (Helper::Check(a), Helper::Check(b)) => {
                    crate::check::type_relation::type_equal_with(a, b, &mut query)?
                }
                (Helper::Update(a, x), Helper::Update(b, y)) => {
                    query.work(x.len() + y.len())?;
                    x == y && crate::check::type_relation::type_equal_with(a, b, &mut query)?
                }
                (Helper::Clean(a), Helper::Clean(b)) => a == b,
                (Helper::Store(a), Helper::Store(b)) => a == b,
                (Helper::Test(a), Helper::Test(b)) => a == b,
                _ => false,
            };
            if equal {
                return Ok(Some(*binding));
            }
        }
        Ok(None)
    }
    fn shape_helper_start(
        &mut self,
        key: Helper<'src>,
        arity: usize,
    ) -> Result<(js::BindingId, js::RegionId, Vec<js::BindingId>), FormationError> {
        self.work(self.shape_helpers.len() + 1)?;
        let scope = self.module.regions[self.module.root.index()].scope;
        let binding = self.fresh_binding(scope, "shape")?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.shape_helpers,
            (key, binding),
        )?;
        let body = self.module.region_in(scope, self.budget)?;
        let inner = self.module.regions[body.index()].scope;
        let mut parameters = self.budget.vector(AllocationClass::Retained, arity)?;
        for _ in 0..arity {
            let parameter = self.fresh_binding(inner, "value")?;
            self.append(&mut parameters, parameter)?;
        }
        Ok((binding, body, parameters))
    }
    fn shape_helper_finish(
        &mut self,
        binding: js::BindingId,
        body: js::RegionId,
        parameters: Vec<js::BindingId>,
    ) -> Result<(), FormationError> {
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
            self.module.root,
            js::Statement::Function { binding, function },
        )
    }
    fn shape_call(
        &mut self,
        binding: js::BindingId,
        args: &[js::ExprId],
    ) -> Result<js::ExprId, FormationError> {
        let callee = self.reference(binding)?;
        let arguments = self.budget.copy_slice(AllocationClass::Retained, args)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }
    fn shape_member(
        &mut self,
        object: js::ExprId,
        field: FieldRef,
    ) -> Result<js::ExprId, FormationError> {
        let object = self.shape_read(object)?;
        let key = self.class_field_key(field)?;
        self.expression(js::Expr::Member {
            object,
            property: js::Property::Computed(key),
        })
    }
    fn nullish(&mut self, value: js::ExprId) -> Result<js::ExprId, FormationError> {
        let value = self.shape_read(value)?;
        let right = self.literal(js::Literal::Null)?;
        self.expression(js::Expr::Binary {
            op: js::Binary::Equal,
            left: value,
            right,
        })
    }
    fn shape_if(
        &mut self,
        body: js::RegionId,
        condition: js::ExprId,
        action: js::ExprId,
    ) -> Result<(), FormationError> {
        let scope = self.module.regions[body.index()].scope;
        let yes = self.module.region_in(scope, self.budget)?;
        self.statement(yes, js::Statement::Evaluate(action))?;
        self.statement(
            body,
            js::Statement::If {
                condition,
                yes,
                no: None,
            },
        )
    }
    pub(super) fn shape_cleanup(
        &mut self,
        ty: TypeId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if !super::super::schema::is_shape(self.program, &self.program.types[ty.index()]) {
            return Ok(value);
        }
        let fields = self.shape_fields(ty)?;
        if !fields
            .iter()
            .any(|(_, ty)| matches!(ty, Type::Nullable(_) | Type::Null))
        {
            return Ok(value);
        }
        let key = Helper::Clean(ty);
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let (helper, body, parameters) = self.shape_helper_start(key, 1)?;
            for (field, ty) in fields {
                self.work(1)?;
                if !matches!(ty, Type::Nullable(_) | Type::Null) {
                    continue;
                }
                let object = self.reference(parameters[0])?;
                let member = self.shape_member(object, field)?;
                let condition = self.nullish(member)?;
                let deleted = self.expression(js::Expr::Unary {
                    op: js::Unary::Delete,
                    value: member,
                })?;
                self.shape_if(body, condition, deleted)?;
            }
            let value = self.reference(parameters[0])?;
            self.statement(body, js::Statement::Return(Some(value)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        self.shape_call(helper, &[value])
    }
    pub(super) fn shape_store(
        &mut self,
        unit: ContextId,
        place: PlaceId,
        value: js::ExprId,
    ) -> Result<Option<js::ExprId>, FormationError> {
        let Place::ClassField { receiver, field } = self.data(unit).places[place.index()] else {
            return Ok(None);
        };
        let class = self.program.class(field.nominal).unwrap();
        if !class.shape {
            return Ok(None);
        }
        let accessor = class.accessors[field.slot as usize];
        let declared =
            super::super::schema::place_type(self.program, self.data(unit), place, self.budget)?
                .ok_or_else(|| {
                    self.error(Span::default(), "shape write has no checked field type")
                })?;
        if matches!(declared.as_ref(), Type::TypeParameter(_)) && !class.reflected {
            // Opaque private slots retain their value, including a JsValue's
            // null. Public erased slots require a checked presence contract.
            return Ok(None);
        }
        if !matches!(
            declared.as_ref(),
            Type::Nullable(_) | Type::Null | Type::TypeParameter(_)
        ) {
            return Ok(None);
        }
        let key = Helper::Store(field);
        let helper = if let Some(helper) = self.shape_helper(&key)? {
            helper
        } else {
            let (helper, body, parameters) = self.shape_helper_start(key, 2)?;
            let object = self.reference(parameters[0])?;
            let value = self.reference(parameters[1])?;
            let member = self.shape_member(object, field)?;
            let deleted = self.expression(js::Expr::Unary {
                op: js::Unary::Delete,
                value: member,
            })?;
            let written = if accessor {
                let target = self.shape_read(member)?;
                self.expression(js::Expr::Assign { target, value })?
            } else {
                let mut entries = self.budget.vector(AllocationClass::Retained, 4)?;
                for name in ["writable", "enumerable", "configurable"] {
                    let property = js::Property::Named(self.text(name)?);
                    let yes = self.literal(js::Literal::Bool(true))?;
                    self.append(&mut entries, (property, yes))?;
                }
                let property = js::Property::Named(self.text("value")?);
                self.append(&mut entries, (property, value))?;
                let descriptor = self.expression(js::Expr::Object(entries))?;
                let callee = self.host_path(&["Object", "defineProperty"])?;
                let key = self.class_field_key(field)?;
                let object = self.shape_read(object)?;
                let arguments = self
                    .budget
                    .copy_slice(AllocationClass::Retained, &[object, key, descriptor])?;
                self.expression(js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Reference,
                })?
            };
            let condition = self.nullish(value)?;
            let action = self.expression(js::Expr::Conditional {
                condition,
                yes: deleted,
                no: written,
            })?;
            self.statement(body, js::Statement::Evaluate(action))?;
            let value = self.reference(parameters[1])?;
            self.statement(body, js::Statement::Return(Some(value)))?;
            self.shape_helper_finish(helper, body, parameters)?;
            helper
        };
        let object = self.value(unit, receiver)?;
        Ok(Some(self.shape_call(helper, &[object, value])?))
    }
}
