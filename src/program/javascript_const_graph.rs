//! Immutable stored public graphs have their own representation. Value products
//! become public objects; arrays/records retain aliases through one weak memo.
//! The memo is owned by a hoisted synthetic function, so module placement and
//! load order cannot put its initialization after a caller. All code is target
//! IR, with ordinary dependencies, naming and exact artifact costs.
use super::*;
use crate::check::binary_types::TypeConstructionAdmission;
use crate::check::type_admission::TypeQueryAdmission;
use crate::check::type_relation::type_equal_with;

pub(super) fn adaptable(
    program: &Program<'_>,
    ty: &Type<'_>,
    depth: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    budget.work(WorkKind::Analysis, 1)?;
    if depth >= 32 {
        return Ok(false);
    }
    if !public_structs::carries_product(ty, budget)? {
        return Ok(true);
    }
    match ty {
        Type::Array(inner) | Type::Record(inner) => adaptable(program, inner, depth + 1, budget),
        Type::Nullable(inner) => adaptable(program, inner, depth + 1, budget),
        Type::Union(members) => {
            for (index, member) in members.iter().enumerate() {
                let Some(category) = category(member) else {
                    return Ok(false);
                };
                if members[..index]
                    .iter()
                    .any(|other| self::category(other) == Some(category))
                    || !adaptable(program, member, depth + 1, budget)?
                {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => {
            let Some(definition) = super::super::schema::struct_definition(program, ty) else {
                return Ok(false);
            };
            for field in &program.fields[definition.fields.clone()] {
                let field_ty = super::super::schema::field_type(
                    program,
                    ty,
                    field,
                    &mut TypeQueryAdmission::new(budget),
                )?
                .expect("verified const schema");
                if !adaptable(program, &field_ty, depth + 1, budget)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
    }
}

fn category(ty: &Type<'_>) -> Option<&'static str> {
    Some(match ty {
        Type::Struct(_) | Type::StructInstance { .. } => "object",
        Type::Int | Type::Float => "number",
        Type::String => "string",
        Type::Bool => "boolean",
        _ => return None,
    })
}

impl<'src> Formation<'_, '_, 'src, '_, '_> {
    pub(super) fn publish_const_graph(
        &mut self,
        unit: ContextId,
        cell: CellId,
    ) -> Result<(), FormationError> {
        let declared = &self.program.cells[cell.index()];
        let ty = &self.program.types[declared.ty.index()];
        if !self.contract.abi.preserve_root_exports
            || !declared.declared_const
            || !public_structs::carries_product(ty, self.budget)?
        {
            return Ok(());
        }
        self.work(self.program.exports().len())?;
        if !self
            .demand
            .value_exports()
            .any(|(_, exported)| exported == cell)
        {
            return Ok(());
        }
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let private = self.cell_binding(unit, cell)?;
        let binding = self.fresh_binding(scope, "const_public")?;
        let value = self.reference(private)?;
        let value = self.const_graph_value(ty, value)?;
        let anchor = if self.module.pristine_builtins {
            self.anchor
        } else {
            js::Anchor::Anchored
        };
        let mut row = js::RootRow::new(self.current_module, anchor).at(self.point);
        row.completes = Some(private);
        self.push_statement(
            root,
            js::Statement::Let {
                binding,
                value: Some(value),
            },
            row,
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.const_exports,
            (cell, binding),
        )?;
        Ok(())
    }

    fn cg_call(
        &mut self,
        callee: js::ExprId,
        arguments: &[js::ExprId],
        invocation: Invocation,
    ) -> Result<js::ExprId, FormationError> {
        let arguments = self
            .budget
            .copy_slice(AllocationClass::Retained, arguments)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation,
        })
    }
    fn cg_member(
        &mut self,
        binding: js::BindingId,
        name: &str,
    ) -> Result<js::ExprId, FormationError> {
        let object = self.reference(binding)?;
        let property = js::Property::Named(self.text(name)?);
        self.expression(js::Expr::Member { object, property })
    }
    fn cg_method(
        &mut self,
        binding: js::BindingId,
        name: &str,
        arguments: &[js::ExprId],
    ) -> Result<js::ExprId, FormationError> {
        let callee = self.cg_member(binding, name)?;
        self.cg_call(callee, arguments, Invocation::Reference)
    }
    fn cg_function(
        &mut self,
        binding: js::BindingId,
        parameters: &[js::BindingId],
        body: js::RegionId,
    ) -> Result<(), FormationError> {
        let parameters = self
            .budget
            .copy_slice(AllocationClass::Retained, parameters)?;
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
    fn const_graph_memo(&mut self) -> Result<js::BindingId, FormationError> {
        if let Some(binding) = self.struct_plan.const_memo {
            return Ok(binding);
        }
        let scope = self.module.regions[self.module.root.index()].scope;
        let binding = self.fresh_binding(scope, "const_views")?;
        self.struct_plan.const_memo = Some(binding);
        let body = self.module.region_in(scope, self.budget)?;
        let cached = self.cg_member(binding, "cache")?;
        let target = self.cg_member(binding, "cache")?;
        let callee = self.host_path(&["WeakMap"])?;
        let value = self.expression(js::Expr::Construct {
            callee,
            arguments: Vec::new(),
        })?;
        let initialized = self.expression(js::Expr::Assign { target, value })?;
        let value = self.expression(js::Expr::Binary {
            op: js::Binary::Or,
            left: cached,
            right: initialized,
        })?;
        self.statement(body, js::Statement::Return(Some(value)))?;
        self.cg_function(binding, &[], body)?;
        Ok(binding)
    }
    fn const_graph_value(
        &mut self,
        ty: &Type<'src>,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        if !public_structs::carries_product(ty, self.budget)? {
            let value = self.public_value(ty, value, false)?;
            return if super::super::rules::const_data::aggregate(ty) {
                self.freeze_const_boundary(value)
            } else {
                Ok(value)
            };
        }
        let codec = self.const_graph_codec(ty)?;
        let callee = self.reference(codec)?;
        self.cg_call(callee, &[value], Invocation::Value)
    }
    fn const_graph_codec(&mut self, ty: &Type<'src>) -> Result<js::BindingId, FormationError> {
        for (known, binding) in &self.struct_plan.const_codecs {
            if type_equal_with(known, ty, &mut TypeQueryAdmission::new(self.budget))? {
                return Ok(*binding);
            }
        }
        let root_scope = self.module.regions[self.module.root.index()].scope;
        let binding = self.fresh_binding(root_scope, "const_encode")?;
        let owned = TypeQueryAdmission::new(self.budget).clone_type(ty)?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.const_codecs,
            (owned, binding),
        )?;
        let body = self.module.region_in(root_scope, self.budget)?;
        let scope = self.module.regions[body.index()].scope;
        let input = self.fresh_binding(scope, "input")?;
        let result = match ty {
            Type::Nullable(inner) => {
                let left = self.reference(input)?;
                let right = self.literal(js::Literal::Null)?;
                let condition = self.expression(js::Expr::Binary {
                    op: js::Binary::Equal,
                    left,
                    right,
                })?;
                let value = self.reference(input)?;
                let no = self.const_graph_value(inner, value)?;
                let yes = if public_structs::carries_absence(self.program, ty, self.budget)? {
                    self.literal(if inner.boundary == crate::check::AbsencePin::Undefined {
                        js::Literal::Undefined
                    } else {
                        js::Literal::Null
                    })?
                } else {
                    self.reference(input)?
                };
                self.expression(js::Expr::Conditional { condition, yes, no })?
            }
            Type::Union(members) => {
                let mut result = None;
                for member in members.iter().rev() {
                    let input_read = self.reference(input)?;
                    let yes = self.const_graph_value(member, input_read)?;
                    result = Some(if let Some(no) = result {
                        let value = self.reference(input)?;
                        let left = self.expression(js::Expr::Unary {
                            op: js::Unary::TypeOf,
                            value,
                        })?;
                        let text = category(member).ok_or_else(|| {
                            self.error(Span::default(), "ambiguous const product union")
                        })?;
                        let payload = self.text(text)?;
                        let right = self.literal(js::Literal::String(payload.into()))?;
                        let condition = self.expression(js::Expr::Binary {
                            op: js::Binary::StrictEqual,
                            left,
                            right,
                        })?;
                        self.expression(js::Expr::Conditional { condition, yes, no })?
                    } else {
                        yes
                    });
                }
                result.ok_or_else(|| self.error(Span::default(), "empty const product union"))?
            }
            Type::Array(element) | Type::Record(element) => {
                let memo = self.const_graph_memo()?;
                let callee = self.reference(memo)?;
                let value = self.cg_call(callee, &[], Invocation::Value)?;
                let map = self.fresh_binding(scope, "views")?;
                self.statement(
                    body,
                    js::Statement::Let {
                        binding: map,
                        value: Some(value),
                    },
                )?;
                let input_read = self.reference(input)?;
                let cached = self.cg_method(map, "get", &[input_read])?;
                let output = self.fresh_binding(scope, "output")?;
                self.statement(
                    body,
                    js::Statement::Let {
                        binding: output,
                        value: Some(cached),
                    },
                )?;
                let found = self.module.region_in(scope, self.budget)?;
                let value = self.reference(output)?;
                self.statement(found, js::Statement::Return(Some(value)))?;
                let condition = self.reference(output)?;
                self.statement(
                    body,
                    js::Statement::If {
                        condition,
                        yes: found,
                        no: None,
                    },
                )?;
                let is_array = matches!(ty, Type::Array(_));
                let value = if is_array {
                    self.expression(js::Expr::Array(Vec::new()))?
                } else {
                    let callee = self.host_path(&["Object", "create"])?;
                    let null = self.literal(js::Literal::Null)?;
                    self.cg_call(callee, &[null], Invocation::Value)?
                };
                let target = self.reference(output)?;
                let assign = self.expression(js::Expr::Assign { target, value })?;
                self.statement(body, js::Statement::Evaluate(assign))?;
                let input_read = self.reference(input)?;
                let output_read = self.reference(output)?;
                let set = self.cg_method(map, "set", &[input_read, output_read])?;
                self.statement(body, js::Statement::Evaluate(set))?;
                let iteration = self.module.region_in(scope, self.budget)?;
                let key =
                    self.fresh_binding(self.module.regions[iteration.index()].scope, "key")?;
                let key_read = self.reference(key)?;
                let input_read = self.reference(input)?;
                let value = self.expression(js::Expr::Member {
                    object: input_read,
                    property: js::Property::Computed(key_read),
                })?;
                let value = self.const_graph_value(element, value)?;
                let output_read = self.reference(output)?;
                let key_read = self.reference(key)?;
                let target = self.expression(js::Expr::Member {
                    object: output_read,
                    property: js::Property::Computed(key_read),
                })?;
                let assign = self.expression(js::Expr::Assign { target, value })?;
                self.statement(iteration, js::Statement::Evaluate(assign))?;
                let callee = self.host_path(&["Object", "keys"])?;
                let input_read = self.reference(input)?;
                let iterable = self.cg_call(callee, &[input_read], Invocation::Value)?;
                self.statement(
                    body,
                    js::Statement::ForOf {
                        binding: key,
                        iterable,
                        body: iteration,
                    },
                )?;
                if is_array {
                    // Preserve even a trailing hole; const evaluation normally
                    // creates dense arrays, but the adapter never assumes it.
                    let target = self.cg_member(output, "length")?;
                    let value = self.cg_member(input, "length")?;
                    let assign = self.expression(js::Expr::Assign { target, value })?;
                    self.statement(body, js::Statement::Evaluate(assign))?;
                }
                let callee = self.host_path(&["Object", "freeze"])?;
                let output_read = self.reference(output)?;
                self.cg_call(callee, &[output_read], Invocation::Value)?
            }
            _ => {
                let program = self.program;
                let definition = super::super::schema::struct_definition(program, ty)
                    .ok_or_else(|| self.error(Span::default(), "const product schema"))?;
                let fields = &program.fields[definition.fields.clone()];
                let mut entries = self
                    .budget
                    .vector(AllocationClass::Retained, fields.len())?;
                for (index, field) in fields.iter().enumerate() {
                    let input_read = self.reference(input)?;
                    let value = self.slot(input_read, index)?;
                    let field_ty = super::super::schema::field_type(
                        program,
                        ty,
                        field,
                        &mut TypeQueryAdmission::new(self.budget),
                    )?
                    .expect("verified const schema");
                    let value = self.const_graph_value(&field_ty, value)?;
                    let key = self.public_key(&field.name)?;
                    self.append(&mut entries, (key, value))?;
                }
                let value = self.expression(js::Expr::Object(entries))?;
                let value = self.public_product_cleanup(ty, value)?;
                let callee = self.host_path(&["Object", "freeze"])?;
                self.cg_call(callee, &[value], Invocation::Value)?
            }
        };
        self.statement(body, js::Statement::Return(Some(result)))?;
        self.cg_function(binding, &[input], body)?;
        Ok(binding)
    }
}
