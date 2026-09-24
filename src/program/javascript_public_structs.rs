//! D2's public value-struct adapter. Inside the artifact a value struct uses
//! the private immutable positional recipe; its public ABI is a plain object
//! whose own data properties are the struct's fields in declaration order.
//! An exported function whose signature carries structs is published as one
//! wrapper that keeps the source name, arity and callable kind: it reads each
//! incoming field once, in declaration order, into a fresh positional product,
//! calls the private function, and returns a fresh object. Components transfer
//! raw, like every other adapter; the private body keeps its own
//! normalization. A struct inside a collection, callable, union or nullable
//! has identity or aliasing that copying cannot preserve, so it stays refused.
use super::*;
use crate::primitive::ParameterPassing;

/// Nested value structs are acyclic by construction; the bound only keeps
/// the check itself bounded.
const MAX_PUBLIC_DEPTH: usize = 32;

fn schema_of(
    program: &Program<'_>,
    identity: NominalId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<usize>, FormationError> {
    for (schema, definition) in program.structs.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        if definition.identity == identity {
            return Ok(Some(schema));
        }
    }
    Ok(None)
}

pub(super) fn carries_product(
    ty: &Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    let mut pending = budget.vector(AllocationClass::Scratch, 1)?;
    let contains = super::super::facts::contains_nominal_product(
        ty,
        &mut pending,
        &mut references::Meter(budget),
    )?;
    let bytes = pending
        .capacity()
        .checked_mul(std::mem::size_of::<&Type<'_>>())
        .ok_or(AllocationError::Capacity)?;
    drop(pending);
    budget.release(AllocationClass::Scratch, bytes as u64)?;
    Ok(contains)
}

/// Whether `ty` crosses the public boundary exactly under the object ABI.
pub(super) fn adaptable(
    program: &Program<'_>,
    ty: &Type<'_>,
    depth: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    if !carries_product(ty, budget)? {
        return Ok(true);
    }
    let Type::Struct(structure) = ty else {
        return Ok(false);
    };
    if depth >= MAX_PUBLIC_DEPTH {
        return Ok(false);
    }
    let Some(schema) = schema_of(program, structure.identity, budget)? else {
        return Ok(false);
    };
    let definition = &program.structs[schema];
    if !definition.type_parameters.is_empty() {
        return Ok(false);
    }
    for field in &program.fields[definition.fields.clone()] {
        budget.work(WorkKind::Analysis, 1)?;
        if !adaptable(program, &program.types[field.ty.index()], depth + 1, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// A function value host code may call through a D2 callable adapter: each
/// parameter a value of an adaptable type, and an adaptable or nullable
/// struct result.
pub(super) fn adaptable_callable(
    program: &Program<'_>,
    signature: &crate::check::FunctionSignature<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    for parameter in &signature.params {
        budget.work(WorkKind::Analysis, 1)?;
        if parameter.passing != crate::primitive::ParameterPassing::Value
            || !adaptable(program, &parameter.ty, 0, budget)?
        {
            return Ok(false);
        }
    }
    let result = match signature.return_type.as_ref() {
        Type::Nullable(inner) if matches!(inner.as_ref(), Type::Struct(_)) => inner.as_ref(),
        result => result,
    };
    adaptable(program, result, 0, budget)
}

/// A declared, never-reassigned function whose value parameters and result
/// are adaptable. A struct parameter with a default would need the wrapper to
/// forward absence; that is not part of this adapter.
pub(super) fn adaptable_export(
    program: &Program<'_>,
    cell: CellId,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    let declared = &program.cells[cell.index()];
    if !matches!(declared.binding, CellBinding::Function(_)) || declared.assigned {
        return Ok(false);
    }
    let Type::Function(signature) = &program.types[declared.ty.index()] else {
        return Ok(false);
    };
    let CellBinding::Function(unit) = declared.binding else {
        return Ok(false);
    };
    for (position, parameter) in signature.params.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        let element_array = match &parameter.ty {
            Type::Array(element) if matches!(element.as_ref(), Type::Struct(_)) => {
                adaptable(program, element, 0, budget)?
                    && read_only_array(program, unit, position, budget)?
            }
            _ => false,
        };
        if parameter.passing != ParameterPassing::Value
            || !(element_array || adaptable(program, &parameter.ty, 0, budget)?)
            || (parameter.default.is_some() && carries_product(&parameter.ty, budget)?)
        {
            return Ok(false);
        }
    }
    adaptable(program, &signature.return_type, 0, budget)
}

/// Whether the function only reads the array its parameter holds: the cell
/// is never reassigned or captured, and each load of it only indexes an
/// element to read or reads `length`. A decoded copy of a host array is then
/// indistinguishable from the array itself, so an exported function may take
/// an array of structs.
fn read_only_array(
    program: &Program<'_>,
    unit: UnitId,
    position: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    let data = program.units[unit.index()].data();
    let Some(&parameter) = data.parameters.get(position) else {
        return Ok(false);
    };
    // The parameter, and the synthetic cells a `for...of` copies it into.
    let mut cells = vec![parameter];
    budget.work(WorkKind::Analysis, (data.operations.len() * 2) as u64)?;
    let loads_any = |cells: &[CellId], value: ValueId| {
        matches!(
            data.operations[data.values[value.index()].definition.index()].kind,
            OperationKind::Load(place)
                if matches!(data.places[place.index()], Place::Cell(cell) if cells.contains(&cell))
        )
    };
    for operation in &data.operations {
        if let OperationKind::Initialize(copy) = operation.kind {
            let operands = data.operands(operation.operands).unwrap_or(&[]);
            if program.cells[copy.index()].synthetic
                && operands
                    .first()
                    .is_some_and(|&value| loads_any(&cells, value))
            {
                cells.push(copy);
            }
        }
    }
    budget.work(
        WorkKind::Analysis,
        (program.units.len() * cells.len()) as u64,
    )?;
    // Conversion marks every synthetic loop cell assigned; a copy is only
    // ever initialized, which the store scan below confirms.
    let stored = |cell: CellId| {
        data.operations.iter().any(|operation| {
            matches!(operation.kind, OperationKind::Store(place)
                if data.places[place.index()] == Place::Cell(cell))
        })
    };
    for &cell in &cells {
        let reassigned = if cell == parameter {
            program.cells[cell.index()].assigned
        } else {
            stored(cell)
        };
        if reassigned
            || program
                .units
                .iter()
                .any(|unit| unit.data().captures.contains(&cell))
        {
            return Ok(false);
        }
    }
    let loads = |value: ValueId| loads_any(&cells, value);
    for operation in &data.operations {
        let operands = data.operands(operation.operands).unwrap_or(&[]);
        let admitted = match operation.kind {
            // The copy into a tracked synthetic cell is itself tracked.
            OperationKind::Initialize(copy) if cells.contains(&copy) => true,
            // Reading an element or the length leaves the array unobserved.
            OperationKind::Load(place) => match data.places[place.index()] {
                Place::Index { key, .. } => !loads(key),
                _ => true,
            },
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::ArrayLength)) => true,
            OperationKind::Store(place) | OperationKind::CheckPlace(place) => {
                !matches!(data.places[place.index()], Place::Index { receiver, .. } if loads(receiver))
                    && !operands.iter().any(|&value| loads(value))
            }
            _ => {
                !operands.iter().any(|&value| loads(value))
                    && !matches!(operation.kind, OperationKind::Call(call)
                        if data.arguments(data.calls[call.index()].arguments).unwrap_or(&[]).iter().any(
                            |argument| matches!(*argument, CallArgument::Value(value) if loads(value))
                        ) || matches!(data.calls[call.index()].target,
                            CallTarget::Intrinsic { receiver: Some(receiver), .. } if loads(receiver)))
            }
        };
        if !admitted {
            return Ok(false);
        }
    }
    // Places hold the array only as an element receiver of those reads.
    for place in &data.places {
        match *place {
            Place::Member { receiver, .. } if loads(receiver) => return Ok(false),
            Place::Value(value) if loads(value) => return Ok(false),
            _ => {}
        }
    }
    Ok(true)
}

impl Formation<'_, '_, '_, '_, '_> {
    fn adapter_binding(
        &mut self,
        scope: js::ScopeId,
        spelling: &str,
    ) -> Result<js::BindingId, FormationError> {
        let spelling = self.text(spelling)?;
        Ok(self.module.binding_in(
            js::Binding {
                source_symbol: None,
                scope,
                spelling,
                pinned: false,
            },
            self.budget,
        )?)
    }

    fn public_key(&mut self, name: &str) -> Result<js::Property, FormationError> {
        // `__proto__:` in an object literal sets the prototype instead of
        // defining a field, so that one name is always a computed key.
        if name != "__proto__" && js::identifier_name(name) {
            return Ok(js::Property::Named(self.text(name)?));
        }
        let key = self.string(&crate::literal::StringValue::from(name))?;
        let key = self.literal(js::Literal::String(key))?;
        Ok(js::Property::Computed(key))
    }

    /// Convert one value across the boundary in either direction.
    /// Adaptability already holds, so a non-struct type carries no product.
    pub(super) fn public_value(
        &mut self,
        ty: &Type<'_>,
        value: js::ExprId,
        incoming: bool,
    ) -> Result<js::ExprId, FormationError> {
        // A read-only array parameter of structs decodes each element once,
        // in order: `values.map(decode)`.
        if let (true, Type::Array(element)) = (incoming, ty) {
            if let Type::Struct(structure) = element.as_ref() {
                let schema = schema_of(self.program, structure.identity, self.budget)?
                    .ok_or_else(|| self.error(Span::default(), "missing value-struct schema"))?;
                let codec = self.public_codec(schema, true)?;
                let property = js::Property::Named(self.text("map")?);
                let callee = self.expression(js::Expr::Member {
                    object: value,
                    property,
                })?;
                let codec = self.reference(codec)?;
                let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
                self.append(&mut arguments, codec)?;
                return self.expression(js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Reference,
                });
            }
        }
        let Type::Struct(structure) = ty else {
            return Ok(value);
        };
        let schema = schema_of(self.program, structure.identity, self.budget)?
            .ok_or_else(|| self.error(Span::default(), "missing value-struct schema"))?;
        // A fresh product meets its public shape field by field: `{k:e,…}`,
        // not `encode([e,…])`. The fields evaluate in declaration order
        // either way; a nested encoder moves before the later fields, but
        // it only reads an immutable product and allocates.
        let program = self.program;
        let fields = &program.fields[program.structs[schema].fields.clone()];
        let expressions = &self.module.expressions;
        let fresh = !incoming
            && matches!(&expressions[value.index()], js::Expr::Array(elements)
                if elements.len() == fields.len()
                    && elements
                        .iter()
                        .all(|element| !matches!(expressions[element.index()], js::Expr::Spread(_))));
        if fresh {
            self.work(fields.len())?;
            let mut remaining = self.budget.vector(AllocationClass::Scratch, fields.len())?;
            if let js::Expr::Array(elements) = &self.module.expressions[value.index()] {
                self.budget
                    .extend_copy(AllocationClass::Scratch, &mut remaining, elements)?;
            }
            let mut entries = self
                .budget
                .vector(AllocationClass::Retained, fields.len())?;
            for (field, &element) in fields.iter().zip(&remaining) {
                let value = self.public_value(&program.types[field.ty.index()], element, false)?;
                let key = self.public_key(&field.name)?;
                self.append(&mut entries, (key, value))?;
            }
            self.drop_scratch(remaining)?;
            return self.expression(js::Expr::Object(entries));
        }
        let codec = self.public_codec(schema, incoming)?;
        let callee = self.reference(codec)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut arguments, value)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }

    /// A function value that reaches a `JsValue` position: host code calls it
    /// with public shapes and must receive public shapes back. One hoisted
    /// factory per function type wraps the value once, where it is passed:
    /// `function(f){return function(a,…){return encode(f(decode(a),…))}}`.
    pub(super) fn public_callable(
        &mut self,
        ty: TypeId,
        value: js::ExprId,
    ) -> Result<js::ExprId, FormationError> {
        let factory = self.public_callable_factory(ty)?;
        let callee = self.reference(factory)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut arguments, value)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }

    fn public_callable_factory(&mut self, ty: TypeId) -> Result<js::BindingId, FormationError> {
        for index in 0..self.struct_plan.public_callables.len() {
            self.work(1)?;
            let (cached, binding) = self.struct_plan.public_callables[index];
            if cached == ty {
                return Ok(binding);
            }
        }
        let program = self.program;
        let Type::Function(signature) = &program.types[ty.index()] else {
            return Err(self.error(
                Span::default(),
                "public callable adapter over a non-function",
            ));
        };
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let outer = self.module.region_in(scope, self.budget)?;
        let outer_scope = self.module.regions[outer.index()].scope;
        let target = self.adapter_binding(outer_scope, "callback")?;
        let inner = self.module.region_in(outer_scope, self.budget)?;
        let inner_scope = self.module.regions[inner.index()].scope;
        let mut parameters = self
            .budget
            .vector(AllocationClass::Retained, signature.params.len())?;
        let mut arguments = self
            .budget
            .vector(AllocationClass::Retained, signature.params.len())?;
        for parameter in &signature.params {
            self.work(1)?;
            let binding = self.adapter_binding(inner_scope, "value")?;
            self.append(&mut parameters, binding)?;
            let value = self.reference(binding)?;
            let value = self.public_value(&parameter.ty, value, true)?;
            self.append(&mut arguments, value)?;
        }
        let callee = self.reference(target)?;
        let call = self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })?;
        match signature.return_type.as_ref() {
            Type::Nullable(present) if matches!(present.as_ref(), Type::Struct(_)) => {
                // `let r=f(…);return r==null?r:encode(r)`: the call runs once.
                let result = self.adapter_binding(inner_scope, "result")?;
                self.statement(
                    inner,
                    js::Statement::Let {
                        binding: result,
                        value: Some(call),
                    },
                )?;
                let left = self.reference(result)?;
                let right = self.literal(js::Literal::Null)?;
                let condition = self.expression(js::Expr::Binary {
                    op: js::Binary::Equal,
                    left,
                    right,
                })?;
                let yes = self.reference(result)?;
                let no = self.reference(result)?;
                let no = self.public_value(present, no, false)?;
                let returned = self.expression(js::Expr::Conditional { condition, yes, no })?;
                self.statement(inner, js::Statement::Return(Some(returned)))?;
            }
            result => {
                let returned = self.public_value(result, call, false)?;
                self.statement(inner, js::Statement::Return(Some(returned)))?;
            }
        }
        let adapted = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters,
                body: inner,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::js::Suspension::None,
            },
        )?;
        let adapted = self.expression(js::Expr::Function(adapted))?;
        self.statement(outer, js::Statement::Return(Some(adapted)))?;
        let mut outer_parameters = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut outer_parameters, target)?;
        let factory = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters: outer_parameters,
                body: outer,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length: None,
                suspension: crate::js::Suspension::None,
            },
        )?;
        // Hoisted like the codecs it calls.
        let binding = self.adapter_binding(scope, "public_callable")?;
        self.statement(
            root,
            js::Statement::Function {
                binding,
                function: factory,
            },
        )?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.public_callables,
            (ty, binding),
        )?;
        Ok(binding)
    }

    /// One private codec per schema and direction, shared by every export.
    fn public_codec(
        &mut self,
        schema: usize,
        incoming: bool,
    ) -> Result<js::BindingId, FormationError> {
        for index in 0..self.struct_plan.public_codecs.len() {
            self.work(1)?;
            let (cached, direction, binding) = self.struct_plan.public_codecs[index];
            if cached == schema && direction == incoming {
                return Ok(binding);
            }
        }
        let program = self.program;
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let body = self.module.region_in(scope, self.budget)?;
        let body_scope = self.module.regions[body.index()].scope;
        let parameter =
            self.adapter_binding(body_scope, if incoming { "boundary" } else { "product" })?;
        let fields = &program.fields[program.structs[schema].fields.clone()];
        let mut elements = self
            .budget
            .vector(AllocationClass::Retained, fields.len())?;
        let mut entries = self
            .budget
            .vector(AllocationClass::Retained, fields.len())?;
        for (position, field) in fields.iter().enumerate() {
            self.work(1)?;
            let source = self.reference(parameter)?;
            let ty = &program.types[field.ty.index()];
            if incoming {
                let property = self.public_key(&field.name)?;
                let value = self.expression(js::Expr::Member {
                    object: source,
                    property,
                })?;
                let value = self.public_value(ty, value, true)?;
                self.append(&mut elements, value)?;
            } else {
                let value = self.slot(source, position)?;
                let value = self.public_value(ty, value, false)?;
                let key = self.public_key(&field.name)?;
                self.append(&mut entries, (key, value))?;
            }
        }
        let result = if incoming {
            self.product(elements)?
        } else {
            self.expression(js::Expr::Object(entries))?
        };
        self.statement(body, js::Statement::Return(Some(result)))?;
        let mut parameters = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut parameters, parameter)?;
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
        // Hoisted like the wrapper that calls it, so a wrapper reached from a
        // module cycle before this module finishes evaluating still works.
        let binding =
            self.adapter_binding(scope, if incoming { "public_in" } else { "public_out" })?;
        self.statement(root, js::Statement::Function { binding, function })?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.public_codecs,
            (schema, incoming, binding),
        )?;
        Ok(binding)
    }

    /// The published function for an exported cell whose signature carries
    /// value structs. `inner` is the cell's private storage binding.
    pub(super) fn public_struct_export(
        &mut self,
        cell: CellId,
        inner: js::BindingId,
    ) -> Result<js::BindingId, FormationError> {
        // One wrapper per source function, shared by every export name, so
        // two names for one function still observe one identity.
        for index in 0..self.struct_plan.public_exports.len() {
            self.work(1)?;
            let (adapted, binding, _) = self.struct_plan.public_exports[index];
            if adapted == cell {
                return Ok(binding);
            }
        }
        let program = self.program;
        let declared = &program.cells[cell.index()];
        let CellBinding::Function(unit) = declared.binding else {
            return Err(self.error(declared.declaration, "public value-struct ABI adaptation"));
        };
        // The wrapper supplies its own receiver and argument list. A body that
        // observes either would see the wrapper's instead of the caller's.
        for index in 0..self.demand.contexts().len() {
            self.work(1)?;
            if self.demand.contexts()[index].unit == unit
                && self.contexts[index]
                    .as_ref()
                    .is_some_and(|context| context.plan.observes_activation)
            {
                return Err(self.error(
                    declared.declaration,
                    "public value-struct adapter over an observed activation",
                ));
            }
        }
        self.work(self.demand.function_lookup_work())?;
        if self
            .demand
            .function_layout(unit)
            .is_some_and(|layout| !layout.parameters().is_empty())
        {
            return Err(self.error(
                declared.declaration,
                "public value-struct adapter over field transport",
            ));
        }
        let Type::Function(signature) = &program.types[declared.ty.index()] else {
            return Err(self.error(declared.declaration, "public value-struct ABI adaptation"));
        };
        let name = program.units[unit.index()]
            .data()
            .function_name
            .ok_or_else(|| {
                self.error(
                    declared.declaration,
                    "missing semantic function-name contract",
                )
            })?;
        let name = &program.strings[name.index()];
        let declared_form = self.free_declaration_name(name)?;
        let name = self.string(name)?;
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let body = self.module.region_in(scope, self.budget)?;
        let body_scope = self.module.regions[body.index()].scope;
        let mut parameters = self
            .budget
            .vector(AllocationClass::Retained, signature.params.len())?;
        let mut arguments = self
            .budget
            .vector(AllocationClass::Retained, signature.params.len())?;
        for parameter in &signature.params {
            self.work(1)?;
            let binding = self.adapter_binding(body_scope, "value")?;
            self.append(&mut parameters, binding)?;
            let value = self.reference(binding)?;
            let value = self.public_value(&parameter.ty, value, true)?;
            self.append(&mut arguments, value)?;
        }
        let callee = self.reference(inner)?;
        let result = self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })?;
        let result = self.public_value(&signature.return_type, result, false)?;
        self.statement(body, js::Statement::Return(Some(result)))?;
        let function = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                parameters,
                body,
                // Only declared functions reach here; their public callable
                // kind is an ordinary constructible function.
                arrow: false,
                name: js::FunctionName::Exact(name),
                strict: false,
                length: None,
                suspension: crate::js::Suspension::None,
            },
        )?;
        let binding = self.adapter_binding(scope, "public_export")?;
        if declared_form {
            // A hoisted declaration is callable from instantiation on, like
            // the source declaration it publishes, and needs no name recipe.
            self.statement(root, js::Statement::Function { binding, function })?;
        } else {
            let value = self.expression(js::Expr::Function(function))?;
            self.statement(
                root,
                js::Statement::Let {
                    binding,
                    value: Some(value),
                },
            )?;
        }
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.public_exports,
            (cell, binding, declared_form.then_some(function)),
        )?;
        Ok(binding)
    }

    /// A root declaration requires its binding to be spelled as its name. That
    /// is only sound when the name is a strict-mode binding identifier that no
    /// external reference or earlier wrapper declaration already claims.
    fn free_declaration_name(
        &mut self,
        name: &crate::literal::StringValue,
    ) -> Result<bool, FormationError> {
        self.work(name.storage_bytes())?;
        let Some(name) = name.as_unicode() else {
            return Ok(false);
        };
        if !js::identifier(name) || matches!(name, "eval" | "arguments") {
            return Ok(false);
        }
        for index in 0..self.struct_plan.public_exports.len() {
            self.work(1)?;
            if let (_, _, Some(function)) = self.struct_plan.public_exports[index] {
                if let js::FunctionName::Exact(existing) =
                    &self.module.functions[function.index()].name
                {
                    if existing.as_unicode() == Some(name) {
                        return Ok(false);
                    }
                }
            }
        }
        self.work(self.module.expressions.len())?;
        Ok(!self.module.references_host(name))
    }
}
