//! D2's public value-struct adapter. Inside the artifact a value struct uses
//! the private immutable positional recipe; its public ABI is a plain object
//! whose own data properties are the struct's fields in declaration order.
//! An exported function whose signature carries structs is published as one
//! wrapper that keeps the source name, arity and callable kind: it reads each
//! incoming field once, in declaration order, into a fresh positional product,
//! calls the private function, and returns a fresh object. Components transfer
//! raw, like every other adapter; the private body keeps its own
//! normalization. Concrete generic schemas and nullable values use the same
//! adapter. Rest arrays are fresh and may be decoded; other array parameters
//! require the read-only proof below. Aliased mutable collections and opaque
//! unions remain refused when their sharing cannot survive conversion.
use super::*;
use crate::check::{
    binary_types::TypeConstructionAdmission, type_admission::TypeQueryAdmission,
    type_relation::type_equal_with,
};
use crate::primitive::ParameterPassing;

/// Nested value structs are acyclic by construction; the bound only keeps
/// the check itself bounded.
const MAX_PUBLIC_DEPTH: usize = 32;

/// A union codec needs disjoint tests in both its public and private layouts.
/// Products are objects publicly and positional arrays privately; primitive
/// `typeof` categories distinguish them without inspecting a field/getter.
fn union_category(ty: &Type<'_>) -> Option<&'static str> {
    Some(match ty {
        Type::Struct(_) | Type::StructInstance { .. } => "object",
        Type::Int | Type::Float => "number",
        Type::String => "string",
        Type::Bool => "boolean",
        _ => return None,
    })
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

/// Nullable ABI metadata is distinct from the product-layout predicate. Shared
/// validators must not mistake an optional primitive for a value struct.
pub(super) fn carries_absence(
    program: &Program<'_>, ty: &Type<'_>, budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    use crate::check::type_payload::{measure_payload, Payload, PayloadError};
    if !program.absence_abi { return Ok(false); }
    let mut found = false;
    let measured = measure_payload(Payload::Type(ty), budget, |node| {
        if let Payload::Type(Type::Nullable(inner)) = node {
            found |= program.source_contract.unified_absence() || inner.boundary != crate::check::AbsencePin::Auto;
        }
        if let Payload::Signature(signature) = node {
            found |= program.source_contract.unified_absence() && signature.params.iter().any(|p| p.optional);
        }
        Ok::<_, std::convert::Infallible>(())
    });
    match measured {
        Ok(_) => Ok(found),
        Err(PayloadError::Allocation(error)) => Err(error.into()),
        Err(PayloadError::Visitor(never)) => match never {},
    }
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
    if depth >= MAX_PUBLIC_DEPTH {
        return Ok(false);
    }
    if let Type::Nullable(inner) = ty {
        return adaptable(program, inner, depth + 1, budget);
    }
    if let Type::Union(members) = ty {
        for (index, member) in members.iter().enumerate() {
            budget.work(WorkKind::Analysis, index as u64 + 1)?;
            let Some(category) = union_category(member) else { return Ok(false); };
            if members[..index].iter().any(|prior| union_category(prior) == Some(category))
                || !adaptable(program, member, depth + 1, budget)? { return Ok(false); }
        }
        return Ok(true);
    }
    let Some(definition) = super::super::schema::struct_definition(program, ty) else {
        return Ok(false);
    };
    for field in &program.fields[definition.fields.clone()] {
        budget.work(WorkKind::Analysis, 1)?;
        let field_ty = super::super::schema::field_type(
            program,
            ty,
            field,
            &mut TypeQueryAdmission::new(budget),
        )?
        .expect("field of its verified schema");
        if !adaptable(program, &field_ty, depth + 1, budget)? {
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
        let ty = if parameter.rest {
            match &parameter.ty {
                Type::Array(element) => element.as_ref(),
                _ => return Ok(false),
            }
        } else {
            &parameter.ty
        };
        if parameter.passing != crate::primitive::ParameterPassing::Value
            || !adaptable(program, ty, 0, budget)?
        {
            return Ok(false);
        }
    }
    adaptable(program, &signature.return_type, 0, budget)
}

/// A declared, never-reassigned function whose value parameters and result
/// are adaptable. Optional parameters preserve absence for the callee's default;
/// rest parameters own their fresh array, so decoding cannot break host aliases.
pub(super) fn adaptable_export(
    program: &Program<'_>,
    cell: CellId,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, FormationError> {
    let declared = &program.cells[cell.index()];
    if !matches!(declared.binding, CellBinding::Function(_)) || declared.reassigned {
        return Ok(false);
    }
    let Some(signature) = program.types[declared.ty.index()].callable_signature() else {
        return Ok(false);
    };
    let CellBinding::Function(unit) = declared.binding else {
        return Ok(false);
    };
    for (position, parameter) in signature.params.iter().enumerate() {
        budget.work(WorkKind::Analysis, 1)?;
        let element_array = match &parameter.ty {
            Type::Array(element) if carries_product(element, budget)? => {
                adaptable(program, element, 0, budget)?
                    && (parameter.rest || read_only_array(program, unit, position, budget)?)
            }
            _ => false,
        };
        let callable = if let Some(signature) = parameter.ty.callable_signature() {
            adaptable_callable(program, signature, budget)?
                && call_only_parameter(program, unit, position, budget)?
        } else { false };
        if parameter.passing != ParameterPassing::Value
            || !(element_array || callable || adaptable(program, &parameter.ty, 0, budget)?)
        {
            return Ok(false);
        }
    }
    adaptable(program, &signature.return_type, 0, budget)
}

/// A host callback can be adapted when its source value is only invoked. Its
/// wrapper identity, properties and constructibility must remain unobservable.
/// Captures and aliases may cross nested bodies, including returned closures,
/// as long as the callback itself never escapes. Reassignment/defaults supply
/// the same checked private calling convention; this proves no purity property.
fn call_only_parameter(program: &Program<'_>, unit: UnitId, position: usize,
    budget: &mut AllocationBudget<'_>) -> Result<bool, FormationError> {
    let data = program.units[unit.index()].data();
    let Some(&parameter) = data.parameters.get(position) else { return Ok(false); };
    let mut scope = budget.scope();
    let mut aliases = scope.vector(AllocationClass::Scratch, 0)?;
    let loaded_cell = |data: &UnitData, value: ValueId| {
        match data.operations[data.values[value.index()].definition.index()].kind {
            OperationKind::Load(place) => match data.places[place.index()] {
                Place::Cell(cell) => Some(cell),
                _ => None,
            },
            _ => None,
        }
    };
    // Build direct initializer edges once. A child's alias may precede its
    // parent's initializer in the unit table, so source-order scanning cannot
    // close this graph. Each cell is enqueued at most once, including cycles.
    for unit in &program.units {
        let data = unit.data();
        for operation in &data.operations {
            scope.work(WorkKind::Analysis, 1)?;
            if let OperationKind::Initialize(to) = operation.kind {
                if let Some(from) = data.operands(operation.operands).unwrap().first()
                    .and_then(|&value| loaded_cell(data, value)) {
                    scope.push(AllocationClass::Scratch, &mut aliases, (from, to))?;
                }
            }
        }
    }
    let search_work = u64::from(usize::BITS - aliases.len().leading_zeros()) + 1;
    scope.work(WorkKind::Analysis, (aliases.len() as u64).saturating_mul(search_work))?;
    aliases.sort_unstable();
    let mut tracked = scope.vector(AllocationClass::Scratch, program.cells.len())?;
    scope.work(WorkKind::Analysis, program.cells.len() as u64)?;
    tracked.resize(program.cells.len(), false);
    let mut pending = scope.vector(AllocationClass::Scratch, 1)?;
    scope.push(AllocationClass::Scratch, &mut pending, parameter)?;
    tracked[parameter.index()] = true;
    let mut cursor = 0;
    while cursor < pending.len() {
        let cell = pending[cursor];
        cursor += 1;
        if program.units[program.cells[cell.index()].owner.index()].data().kind
            == UnitKind::ModuleInitialization { return Ok(false); }
        scope.work(WorkKind::Analysis, search_work)?;
        let start = aliases.partition_point(|&(from, _)| from < cell);
        for &(from, to) in &aliases[start..] {
            scope.work(WorkKind::Analysis, 1)?;
            if from != cell { break; }
            if !tracked[to.index()] {
                tracked[to.index()] = true;
                scope.push(AllocationClass::Scratch, &mut pending, to)?;
            }
        }
    }
    for unit in &program.units {
        let data = unit.data();
        let loads = |value: ValueId| loaded_cell(data, value).is_some_and(|cell| tracked[cell.index()]);
        for operation in &data.operations {
            let operands = data.operands(operation.operands).unwrap();
            scope.work(WorkKind::Analysis, operands.len() as u64 + 1)?;
            if !matches!(operation.kind, OperationKind::Initialize(cell) if tracked[cell.index()])
                && !matches!(operation.kind, OperationKind::IsUndefined { parameter: Some(_), .. })
                && operands.iter().any(|&value| loads(value)) { return Ok(false); }
            if let OperationKind::Call(call) = operation.kind {
                let call = &data.calls[call.index()];
                // Only ordinary callable invocation may consume the adapter.
                if matches!(call.target, CallTarget::Intrinsic { receiver: Some(value), .. }
                    if loads(value)) { return Ok(false); }
                for argument in data.arguments(call.arguments).unwrap() {
                    scope.work(WorkKind::Analysis, 1)?;
                    match *argument {
                        CallArgument::Value(value) | CallArgument::Spread(value) if loads(value) => return Ok(false),
                        CallArgument::Reference(place) if matches!(data.places[place.index()], Place::Cell(cell) if tracked[cell.index()]) => return Ok(false),
                        _ => {}
                    }
                }
            }
        }
        for place in &data.places {
            scope.work(WorkKind::Analysis, 1)?;
            match *place {
                Place::Member { receiver, .. } | Place::ClassField { receiver, .. }
                    if loads(receiver) => return Ok(false),
                Place::Index { receiver, key } if loads(receiver) || loads(key) => return Ok(false),
                Place::Value(value) if loads(value) => return Ok(false),
                _ => {}
            }
        }
        for region in &data.regions {
            scope.work(WorkKind::Analysis, 1)?;
            if region.result.is_some_and(loads) { return Ok(false); }
        }
    }
    Ok(true)
}

/// Whether the function only reads the array its parameter holds: the cell
/// is never reassigned or captured, and each load of it only indexes an
/// element to read or reads `length`. A decoded copy of a host array is then
/// indistinguishable from the array itself only while the body cannot reenter
/// host code, suspend, or write through another alias. Shared operation effects
/// establish that isolation; it is not implied by local read-only uses.
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
    let mut scope = budget.scope();
    let budget = &mut scope;
    let mut cells = budget.vector(AllocationClass::Scratch, 1)?;
    budget.push(AllocationClass::Scratch, &mut cells, parameter)?;
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
                budget.push(AllocationClass::Scratch, &mut cells, copy)?;
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
            program.cells[cell.index()].reassigned
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
    let effect_context = effects::Context {
        program, unit, data, graph: None, summaries: None,
    };
    let effect_values = effects::DomainFacts { data, domains: &[], roots: &[] };
    for operation in &data.operations {
        let operands = data.operands(operation.operands).unwrap_or(&[]);
        budget.work(WorkKind::Analysis, operands.len() as u64 + 1)?;
        // A local read-only use does not isolate the host array. Unknown calls,
        // suspension or an observable write can change it through another
        // alias while this body keeps reading its decoded snapshot. Ask the
        // shared effect owner; no private syntactic callee/purity whitelist.
        let effect = effects::operation_effects(&effect_context, &effect_values, operation);
        if effect.runs_user_code || effect.reenters || effect.suspends || effect.obligated()
            || effect.writes.intersects(effects::Regions::OBSERVABLE) {
            return Ok(false);
        }
        // A trusted dynamic view can introduce a public codec whose getters
        // run at this boundary, although the source view itself is inert.
        // Such values must not reenter a snapshot-array body either.
        if operation.result.is_some_and(|value| matches!(program.types[data.values[value.index()].ty.index()], Type::Dynamic | Type::Unknown)) {
            return Ok(false);
        }
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
            Place::Member { receiver, .. } | Place::ClassField { receiver, .. }
                if loads(receiver) =>
            {
                return Ok(false)
            }
            Place::Value(value) if loads(value) => return Ok(false),
            _ => {}
        }
    }
    Ok(true)
}

impl<'src> Formation<'_, '_, 'src, '_, '_> {
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
                class: None,
                defined: false,
            },
            self.budget,
        )?)
    }

    pub(super) fn public_key(&mut self, name: &str) -> Result<js::Property, FormationError> {
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
        ty: &Type<'src>,
        value: js::ExprId,
        incoming: bool,
    ) -> Result<js::ExprId, FormationError> {
        let value = if incoming { self.enum_crossing(ty, value)? } else { value };
        if ty.callable_signature().is_some()
            && (carries_product(ty, self.budget)? || carries_absence(self.program, ty, self.budget)?) {
            return self.public_callable_type(ty, value, incoming);
        }
        // A read-only array parameter of structs decodes each element once,
        // in order: `values.map(decode)`.
        if let Type::Array(element) = ty {
            if carries_product(element, self.budget)? {
                let codec = self.public_codec(element, incoming)?;
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
        if let Type::Nullable(_) = ty {
            if carries_product(ty, self.budget)? || carries_absence(self.program, ty, self.budget)? {
                if !incoming && !carries_product(ty, self.budget)?
                    && self.contract.ecmascript.allows(JsSyntaxFeature::NullishCoalescing) {
                    let Type::Nullable(inner) = ty else { unreachable!() };
                    let right = self.literal(if inner.boundary == crate::check::AbsencePin::Undefined {
                        js::Literal::Undefined
                    } else { js::Literal::Null })?;
                    return self.expression(js::Expr::Binary { op: js::Binary::Nullish, left: value, right });
                }
                if incoming && !carries_product(ty, self.budget)?
                    && self.contract.checks != crate::compilation_contract::PreconditionChecks::Development {
                    return Ok(value);
                }
                let codec = self.public_codec(ty, incoming)?;
                let callee = self.reference(codec)?;
                let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
                self.append(&mut arguments, value)?;
                return self.expression(js::Expr::Call {
                    callee,
                    arguments,
                    invocation: Invocation::Value,
                });
            }
            return Ok(value);
        }
        if matches!(ty, Type::Union(_)) && carries_product(ty, self.budget)? {
            let codec = self.public_codec(ty, incoming)?;
            let callee = self.reference(codec)?;
            let arguments = self.budget.copy_slice(AllocationClass::Retained, &[value])?;
            return self.expression(js::Expr::Call { callee, arguments, invocation: Invocation::Value });
        }
        let Some(definition) = super::super::schema::struct_definition(self.program, ty) else {
            return Ok(value);
        };
        let schema = definition.identity.index();
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
                let field_ty = super::super::schema::field_type(
                    program,
                    ty,
                    field,
                    &mut TypeQueryAdmission::new(self.budget),
                )?
                .expect("field of its verified schema");
                let value = self.public_value(&field_ty, element, false)?;
                let key = self.public_key(&field.name)?;
                self.append(&mut entries, (key, value))?;
            }
            self.drop_scratch(remaining)?;
            let object = self.expression(js::Expr::Object(entries))?;
            return self.public_product_cleanup(ty, object);
        }
        let codec = self.public_codec(ty, incoming)?;
        let callee = self.reference(codec)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut arguments, value)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }

    /// Decode a boundary parameter while preserving an omitted default. Each
    /// read is a distinct target occurrence of the same inert local binding.
    pub(super) fn public_parameter(
        &mut self,
        parameter: &crate::check::FunctionParameter<'src>,
        binding: js::BindingId,
    ) -> Result<js::ExprId, FormationError> {
        let value = self.reference(binding)?;
        if parameter.optional && self.program.source_contract.unified_absence()
            && self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
            let inner = TypeQueryAdmission::new(self.budget).clone_type(&parameter.ty)?;
            let inner = TypeQueryAdmission::new(self.budget).box_type(inner)?;
            let ty = crate::check::Type::pinned_nullable(inner, crate::check::AbsencePin::Undefined);
            return self.public_value(&ty, value, true);
        }
        let converted = self.public_value(&parameter.ty, value, true)?;
        if !parameter.optional || !carries_product(&parameter.ty, self.budget)? {
            return Ok(converted);
        }
        let value = self.reference(binding)?;
        let undefined = self.literal(js::Literal::Undefined)?;
        let condition = self.expression(js::Expr::Binary {
            op: js::Binary::StrictEqual,
            left: value,
            right: undefined,
        })?;
        let absent = self.literal(js::Literal::Undefined)?;
        self.expression(js::Expr::Conditional {
            condition,
            yes: absent,
            no: converted,
        })
    }

    /// A class keeps its constructor/prototype identity. Convert its public
    /// value parameters once, before any body/default evaluation. Derived
    /// constructors need no access to `this` for this entry prefix.
    pub(super) fn public_constructor_parameters(
        &mut self,
        context: ContextId,
        body: js::RegionId,
    ) -> Result<(), FormationError> {
        let data = self.data(context);
        if data.constructor_of.is_none() {
            return Ok(());
        }
        let Some(ty) = data.callable_type else {
            return Ok(());
        };
        if self.struct_plan.abi_types.is_empty()
            || !self.struct_plan.abi_types[ty.index()]
        {
            return Ok(());
        }
        let Some(signature) = self.program.types[ty.index()].callable_signature() else {
            return Err(self.error(Span::default(), "constructor parameter signature"));
        };
        for (parameter, &cell) in signature.params.iter().zip(&data.parameters).skip(1) {
            if !carries_product(&parameter.ty, self.budget)?
                && !carries_absence(self.program, &parameter.ty, self.budget)?
                && !(parameter.optional && self.program.source_contract.unified_absence()) {
                continue;
            }
            let binding = self.cell_binding(context, cell)?;
            let value = self.public_parameter(parameter, binding)?;
            let target = self.reference(binding)?;
            let assign = self.expression(js::Expr::Assign { target, value })?;
            self.statement(body, js::Statement::Evaluate(assign))?;
        }
        Ok(())
    }

    pub(super) fn public_constructor_argument(
        &mut self,
        context: ContextId,
        argument: ValueId,
        parameter: Option<&crate::check::FunctionParameter<'src>>,
    ) -> Result<js::ExprId, FormationError> {
        let ty = &self.program.types[self.data(context).values[argument.index()].ty.index()];
        let value = self.value(context, argument)?;
        // Kept constructors have a public entry, even when invoked internally.
        // Use the caller's concrete payload for erased generic parameters, but
        // retain the declared absence spelling (defaults require undefined).
        if let Some(parameter) = parameter {
            let pin = if parameter.optional && self.program.source_contract.unified_absence() {
                Some(crate::check::AbsencePin::Undefined)
            } else if let Type::Nullable(inner) = &parameter.ty {
                Some(inner.boundary)
            } else { None };
            if let Some(pin) = pin {
                let ty = if matches!(ty, Type::Null) { Type::Dynamic }
                    else { TypeQueryAdmission::new(self.budget).clone_type(ty)? };
                let ty = TypeQueryAdmission::new(self.budget).box_type(ty)?;
                return self.public_value(&Type::pinned_nullable(ty, pin), value, false);
            }
        }
        self.public_value(ty, value, false)
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
        self.public_callable_type(&self.program.types[ty.index()], value, false)
    }

    pub(super) fn public_callable_type(&mut self, ty: &Type<'src>, value: js::ExprId, incoming: bool)
        -> Result<js::ExprId, FormationError> {
        let value = if incoming && self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
            let check = self.crossing_helper(super::checks::Crossing::Function, false)?;
            let callee = self.reference(check)?;
            let arguments = self.budget.copy_slice(AllocationClass::Retained, &[value])?;
            self.expression(js::Expr::Call { callee, arguments, invocation: Invocation::Value })?
        } else { value };
        let factory = self.public_callable_factory(ty, incoming)?;
        let callee = self.reference(factory)?;
        let mut arguments = self.budget.vector(AllocationClass::Retained, 1)?;
        self.append(&mut arguments, value)?;
        self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation: Invocation::Value,
        })
    }

    fn public_callable_factory(&mut self, ty: &Type<'src>, incoming: bool) -> Result<js::BindingId, FormationError> {
        for index in 0..self.struct_plan.public_callables.len() {
            self.work(1)?;
            let (cached, direction, binding) = &self.struct_plan.public_callables[index];
            if *direction == incoming && type_equal_with(cached, ty, &mut TypeQueryAdmission::new(self.budget))? {
                return Ok(*binding);
            }
        }
        let program = self.program;
        let Some(signature) = ty.callable_signature() else {
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
            let value = if parameter.receiver {
                let receiver = self.expression(js::Expr::This)?;
                self.public_value(&parameter.ty, receiver, !incoming)?
            } else {
                let binding = self.adapter_binding(inner_scope, "value")?;
                self.append(&mut parameters, binding)?;
                if incoming {
                    let value = self.reference(binding)?;
                    self.outgoing_parameter(parameter, value)?
                } else { self.public_parameter(parameter, binding)? }
            };
            let value = if parameter.rest {
                self.expression(js::Expr::Spread(value))?
            } else {
                value
            };
            self.append(&mut arguments, value)?;
        }
        let callback = self.reference(target)?;
        let (callee, invocation) = if signature.has_receiver() {
            let property = js::Property::Named(self.text("call")?);
            (
                self.expression(js::Expr::Member {
                    object: callback,
                    property,
                })?,
                Invocation::Reference,
            )
        } else {
            (callback, Invocation::Value)
        };
        let call = self.expression(js::Expr::Call {
            callee,
            arguments,
            invocation,
        })?;
        let returned = self.public_value(&signature.return_type, call, incoming)?;
        self.statement(inner, js::Statement::Return(Some(returned)))?;
        let length = signature
            .params
            .iter()
            .filter(|p| !p.receiver)
            .take_while(|p| !p.optional && !p.rest)
            .count();
        let length =
            (length < parameters.len() - usize::from(signature.has_rest())).then_some(length);
        let adapted = js::FunctionId::try_new(self.module.functions.len())
            .ok_or(AllocationError::Capacity)?;
        self.budget.push(
            AllocationClass::Retained,
            &mut self.module.functions,
            js::Function {
                rest: signature.params.last().is_some_and(|p| p.rest),
                parameters,
                body: inner,
                arrow: false,
                name: js::FunctionName::Unobserved,
                strict: false,
                length,
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
                rest: false,
                parameters: outer_parameters,
                body: outer,
                arrow: false,
                name: js::FunctionName::Unobserved,
                // A strict lexical parent also makes a rest wrapper strict,
                // without an illegal directive in its parameterized body.
                // In particular, primitive receivers must not be boxed.
                strict: signature.has_receiver(),
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
        let ty = TypeQueryAdmission::new(self.budget).clone_type(ty)?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.public_callables,
            (ty, incoming, binding),
        )?;
        Ok(binding)
    }

    /// One private codec per schema and direction, shared by every export.
    pub(super) fn public_codec(
        &mut self,
        ty: &Type<'src>,
        incoming: bool,
    ) -> Result<js::BindingId, FormationError> {
        for index in 0..self.struct_plan.public_codecs.len() {
            self.work(1)?;
            let (cached, direction, binding) = &self.struct_plan.public_codecs[index];
            if *direction == incoming
                && type_equal_with(cached, ty, &mut TypeQueryAdmission::new(self.budget))?
            {
                return Ok(*binding);
            }
        }
        let program = self.program;
        let root = self.module.root;
        let scope = self.module.regions[root.index()].scope;
        let body = self.module.region_in(scope, self.budget)?;
        let body_scope = self.module.regions[body.index()].scope;
        let parameter =
            self.adapter_binding(body_scope, if incoming { "boundary" } else { "product" })?;
        let result = if let Type::Union(members) = ty {
            let mut result = None;
            for member in members.iter().rev() {
                let value = self.reference(parameter)?;
                let value = if incoming && self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
                    if let Some((kind, absent)) = super::checks::Crossing::of(member) {
                        let helper = self.crossing_helper(kind, absent)?;
                        let callee = self.reference(helper)?;
                        let arguments = self.budget.copy_slice(AllocationClass::Retained, &[value])?;
                        self.expression(js::Expr::Call { callee, arguments, invocation: Invocation::Value })?
                    } else { value }
                } else { value };
                let converted = self.public_value(member, value, incoming)?;
                result = Some(if let Some(no) = result {
                    let value = self.reference(parameter)?;
                    let left = self.expression(js::Expr::Unary { op: js::Unary::TypeOf, value })?;
                    let category = union_category(member).ok_or_else(|| self.error(Span::default(), "ambiguous public product union"))?;
                    let text = self.string(&category.into())?;
                    let right = self.literal(js::Literal::String(text))?;
                    let condition = self.expression(js::Expr::Binary { op: js::Binary::StrictEqual, left, right })?;
                    self.expression(js::Expr::Conditional { condition, yes: converted, no })?
                } else { converted });
            }
            result.ok_or_else(|| self.error(Span::default(), "empty public product union"))?
        } else if let Type::Nullable(inner) = ty {
            if incoming && inner.boundary != crate::check::AbsencePin::Auto
                && self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
                let left = self.reference(parameter)?;
                let right = self.literal(if inner.boundary == crate::check::AbsencePin::Null {
                    js::Literal::Undefined
                } else { js::Literal::Null })?;
                let wrong = self.expression(js::Expr::Binary { op: js::Binary::StrictEqual, left, right })?;
                self.absence_reject(body, wrong)?;
            }
            let source = self.reference(parameter)?;
            let null = self.literal(js::Literal::Null)?;
            let absent = self.expression(js::Expr::Binary {
                op: js::Binary::Equal,
                left: source,
                right: null,
            })?;
            let source = self.reference(parameter)?;
            let mut present = self.public_value(inner, source, incoming)?;
            if incoming && self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
                if let Some((kind, _)) = super::checks::Crossing::of(inner) {
                    let check = self.crossing_helper(kind, false)?;
                    let callee = self.reference(check)?;
                    let arguments = self.budget.copy_slice(AllocationClass::Retained, &[present])?;
                    present = self.expression(js::Expr::Call { callee, arguments, invocation: Invocation::Value })?;
                }
            }
            let absent_value = if incoming || !carries_absence(program, ty, self.budget)? {
                self.reference(parameter)?
            } else { self.literal(if inner.boundary == crate::check::AbsencePin::Undefined {
                js::Literal::Undefined
            } else { js::Literal::Null })? };
            self.expression(js::Expr::Conditional {
                condition: absent,
                yes: absent_value,
                no: present,
            })?
        } else {
            let definition = super::super::schema::struct_definition(program, ty)
                .ok_or_else(|| self.error(Span::default(), "missing instantiated public schema"))?;
            if incoming && self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
                let object = self.reference(parameter)?;
                self.require_data_object(body, object)?;
            }
            let fields = &program.fields[definition.fields.clone()];
            let mut elements = self
                .budget
                .vector(AllocationClass::Retained, fields.len())?;
            let mut entries = self
                .budget
                .vector(AllocationClass::Retained, fields.len())?;
            for (position, field) in fields.iter().enumerate() {
                self.work(1)?;
                let source = self.reference(parameter)?;
                let field_ty = super::super::schema::field_type(
                    program,
                    ty,
                    field,
                    &mut TypeQueryAdmission::new(self.budget),
                )?
                .expect("field of its verified schema");
                if incoming {
                    let property = self.public_key(&field.name)?;
                    let value = self.expression(js::Expr::Member {
                        object: source,
                        property,
                    })?;
                    let value = if self.contract.checks == crate::compilation_contract::PreconditionChecks::Development {
                        if let Some((kind, absent)) = super::checks::Crossing::of(&field_ty) {
                            let check = self.crossing_helper(kind, absent)?;
                            let callee = self.reference(check)?;
                            let arguments = self.budget.copy_slice(AllocationClass::Retained, &[value])?;
                            self.expression(js::Expr::Call { callee, arguments, invocation: Invocation::Value })?
                        } else { value }
                    } else { value };
                    let value = self.public_value(&field_ty, value, true)?;
                    self.append(&mut elements, value)?;
                } else {
                    let value = self.slot(source, position)?;
                    let value = self.public_value(&field_ty, value, false)?;
                    let key = self.public_key(&field.name)?;
                    self.append(&mut entries, (key, value))?;
                }
            }
            if incoming {
                self.product(elements)?
            } else {
                let object = self.expression(js::Expr::Object(entries))?;
                self.public_product_cleanup(ty, object)?
            }
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
                rest: false,
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
        self.helper_statement(root, js::Statement::Function { binding, function })?;
        let owned_ty = TypeQueryAdmission::new(self.budget).clone_type(ty)?;
        self.budget.push(
            AllocationClass::Scratch,
            &mut self.struct_plan.public_codecs,
            (owned_ty, incoming, binding),
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
        if program
            .unit(unit)
            .is_some_and(|data| data.constructor_of.is_some())
        {
            return Ok(inner);
        }
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
        let Some(signature) = program.types[declared.ty.index()].callable_signature() else {
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
            let value = self.public_parameter(parameter, binding)?;
            let value = if parameter.rest {
                self.expression(js::Expr::Spread(value))?
            } else {
                value
            };
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
                rest: signature.params.last().is_some_and(|p| p.rest),
                parameters,
                body,
                // Only declared functions reach here; their public callable
                // kind is an ordinary constructible function.
                arrow: false,
                name: js::FunctionName::Exact(name),
                strict: false,
                length: program.units[unit.index()]
                    .data()
                    .declared_length
                    .map(|length| length as usize),
                suspension: crate::js::Suspension::None,
            },
        )?;
        let binding = self.adapter_binding(scope, "public_export")?;
        if declared_form {
            // A hoisted declaration is callable from instantiation on, like
            // the source declaration it publishes, and needs no name recipe.
            self.helper_statement(root, js::Statement::Function { binding, function })?;
        } else {
            let value = self.expression(js::Expr::Function(function))?;
            self.helper_statement(
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

#[cfg(test)]
#[test]
fn s4_captured_callback_admission_restores_its_parent_on_every_exit() {
    use crate::compilation_policy::{BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain};
    let arena = bumpalo::Bump::new();
    let source = "struct Point{int x;}export func()->int make(func(Point)->int f){auto alias=f;return ()=>{auto copy=alias;return copy(Point{3});};}";
    let syntax = crate::parse_source(&arena, source).unwrap();
    let semantics = crate::analyze(&syntax).unwrap();
    let program = from_checked_source(&syntax, &semantics).unwrap();
    let cell = program.cells.iter().find(|cell| cell.name == "make").unwrap();
    let CellBinding::Function(unit) = cell.binding else { panic!("function owner"); };
    for (work, memory, succeeds) in [(100_000, 100_000, true), (2, 100_000, false), (100_000, 37, false)] {
        let mut ledger = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
            baseline_work: work, optional_work: 0, baseline_retained_bytes: 0, retained_bytes: memory,
        }).unwrap();
        ledger.retain(WorkDomain::Baseline, 37).unwrap();
        let result = call_only_parameter(&program, unit, 0,
            &mut AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline))));
        if succeeds { assert!(result.unwrap()); } else { assert!(result.is_err()); }
        assert_eq!(ledger.retained_bytes(), 37);
    }
}
