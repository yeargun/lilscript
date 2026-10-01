//! Qualification of the private immutable-product recipe at semantic transfers.
//! Each check uses the actual destination/interface. This is not a second
//! provenance graph: unsupported erasure is rejected where source knowledge
//! would otherwise disappear, before a later JsValue use can expose backing.
use super::super::facts::domains::Admission;
use super::*;

/// Products at a value position have no backing-array identity. Collections
/// containing products remain reference values and stop this particular walk.
fn product_variant(ty: &Type<'_>, budget: &mut AllocationBudget<'_>) -> Result<bool, FormationError> {
    let mut scope = budget.scope();
    let mut pending = scope.vector(AllocationClass::Scratch, 1)?;
    scope.push(AllocationClass::Scratch, &mut pending, ty)?;
    while let Some(ty) = pending.pop() {
        scope.work(WorkKind::Analysis, 1)?;
        match ty {
            Type::Struct(_) | Type::StructInstance { .. } => return Ok(true),
            Type::Nullable(inner) => scope.push(AllocationClass::Scratch, &mut pending, inner.as_ref())?,
            Type::Union(members) => for member in members { scope.push(AllocationClass::Scratch, &mut pending, member)?; },
            _ => {}
        }
    }
    Ok(false)
}

impl<'program, 'src> Formation<'_, 'program, 'src, '_, '_> {
    fn product_record_keys(&self, context: ContextId, call: CallId) -> bool {
        let data = self.data(context);
        let call = &data.calls[call.index()];
        matches!(call.target, CallTarget::Builtin(BuiltinCall::ObjectKeys))
            && matches!(data.arguments(call.arguments).unwrap(), [CallArgument::Value(value)]
                if matches!(self.program.types[data.values[value.index()].ty.index()], Type::Record(_)))
    }

    fn struct_boundary_value(&self, context: ContextId, value: ValueId) -> bool {
        self.struct_plan.boundary_types[self.data(context).values[value.index()].ty.index()]
    }

    /// The checker already established language assignability. This narrower
    /// target check admits a representation-preserving shape only. In
    /// particular, assignability to JsValue is not a product ABI adapter.
    fn struct_transfer_shape(
        &mut self,
        expected: &Type<'_>,
        actual: &Type<'_>,
        span: Span,
    ) -> Result<(), FormationError> {
        self.work(1)?;
        // Canonical, uninstantiated nominal identity already has one private
        // recipe. Ordinary logical copies need no traversal or scratch buffer.
        if std::ptr::eq(expected, actual) && matches!(actual, Type::Struct(_)) {
            return Ok(());
        }
        let mut pending = self.budget.vector(AllocationClass::Scratch, 1)?;
        self.budget
            .push(AllocationClass::Scratch, &mut pending, (expected, actual))?;
        while let Some((expected, actual)) = pending.pop() {
            self.work(1)?;
            let pair = match (expected, actual) {
                (Type::Nullable(expected), Type::Nullable(actual)) => {
                    Some((expected.as_ref(), actual.as_ref()))
                }
                (Type::Nullable(_), Type::Null) => None,
                (Type::Nullable(expected), actual) => Some((expected.as_ref(), actual)),
                // A checked narrowing/widening selects the same nominal
                // private recipe. Dynamic erasure still requires a codec;
                // an opaque union member is never that codec's permission.
                (Type::Union(expected), actual) if !matches!(actual, Type::Dynamic | Type::Unknown) => {
                    let actual = if let Type::Union(members) = actual { members.as_slice() }
                        else { std::slice::from_ref(actual) };
                    for actual in actual {
                        let mut found = false;
                        for expected in expected {
                            self.work(1)?;
                            found |= crate::check::type_relation::type_equal_with(expected, actual,
                                &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?;
                        }
                        if !found { return Err(self.error(span, "value-struct union transfer requires an ABI adapter")); }
                    }
                    None
                }
                (expected, Type::Union(actual)) if !matches!(expected, Type::Dynamic | Type::Unknown) => {
                    let mut found = false;
                    for actual in actual {
                        self.work(1)?;
                        found |= crate::check::type_relation::type_equal_with(expected, actual,
                            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?;
                    }
                    if !found { return Err(self.error(span, "value-struct union narrowing requires an ABI adapter")); }
                    None
                }
                (Type::Array(expected), Type::Array(actual))
                | (Type::Record(expected), Type::Record(actual))
                | (Type::Set(expected), Type::Set(actual))
                | (Type::Task(expected), Type::Task(actual))
                | (Type::Generator(expected), Type::Generator(actual)) => {
                    Some((expected.as_ref(), actual.as_ref()))
                }
                (Type::Map(ek, ev), Type::Map(ak, av)) => {
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut pending,
                        (ek.as_ref(), ak.as_ref()),
                    )?;
                    Some((ev.as_ref(), av.as_ref()))
                }
                (Type::Function(expected), Type::Function(actual))
                    if expected.params.len() == actual.params.len() =>
                {
                    for (expected, actual) in expected.params.iter().zip(&actual.params) {
                        self.work(1)?;
                        if expected.passing != actual.passing {
                            return Err(self.error(span, "value-struct callable passing adapter"));
                        }
                        self.budget.push(
                            AllocationClass::Scratch,
                            &mut pending,
                            (&expected.ty, &actual.ty),
                        )?;
                    }
                    Some((expected.return_type.as_ref(), actual.return_type.as_ref()))
                }
                (Type::Struct(expected), Type::Struct(actual)) => {
                    if expected.identity != actual.identity {
                        return Err(
                            self.error(span, "value-struct transfer requires an ABI adapter")
                        );
                    }
                    None
                }
                (Type::StructInstance { .. }, Type::StructInstance { .. })
                | (Type::GenericFunction(_), Type::GenericFunction(_)) => {
                    if !crate::check::type_relation::type_equal_with(
                        expected,
                        actual,
                        &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget),
                    )? {
                        return Err(self.error(
                            span,
                            "instantiated value-struct transfer disagrees with its schema",
                        ));
                    }
                    None
                }
                (Type::Class(expected), Type::Class(actual)) => {
                    self.work(1)?;
                    if expected.identity != actual.identity {
                        return Err(
                            self.error(span, "value-struct transfer requires an ABI adapter")
                        );
                    }
                    None
                }
                (Type::Enum(expected), Type::Enum(actual)) if expected == actual => None,
                (Type::Dynamic, Type::Dynamic) => None,
                // These shapes need a qualified adapter/instantiated schema;
                // a matching-looking type parameter is not enough evidence.
                (
                    _,
                    Type::Union(_)
                    | Type::StructInstance { .. }
                    | Type::ClassInstance { .. }
                    | Type::GenericFunction(_)
                    | Type::TypeParameter(_),
                )
                | (
                    Type::Union(_)
                    | Type::StructInstance { .. }
                    | Type::ClassInstance { .. }
                    | Type::GenericFunction(_)
                    | Type::TypeParameter(_),
                    _,
                ) => {
                    return Err(self.error(span, "value-struct transfer requires an ABI adapter"));
                }
                _ if std::mem::discriminant(expected) == std::mem::discriminant(actual) => None,
                _ => return Err(self.error(span, "value-struct transfer requires an ABI adapter")),
            };
            if let Some(pair) = pair {
                self.budget
                    .push(AllocationClass::Scratch, &mut pending, pair)?;
            }
        }
        self.drop_scratch(pending)?;
        Ok(())
    }

    fn struct_transfer(
        &mut self,
        context: ContextId,
        value: ValueId,
        expected: &Type<'_>,
        span: Span,
    ) -> Result<(), FormationError> {
        self.work(1)?;
        if self.struct_boundary_value(context, value) {
            let actual = &self.program.types[self.data(context).values[value.index()].ty.index()];
            if matches!(expected, Type::Dynamic) && self.public_encode(context, value, actual)? {
                return Ok(());
            }
            self.struct_transfer_shape(expected, actual, span)?;
        }
        Ok(())
    }

    /// Whether this value is formed through its D2 public encoder.
    pub(super) fn encoded(
        &mut self,
        context: ContextId,
        value: ValueId,
    ) -> Result<bool, FormationError> {
        if self.struct_plan.public_encodes.is_empty() {
            return Ok(false);
        }
        self.work(self.struct_plan.public_encodes.len())?;
        Ok(self.struct_plan.public_encodes.contains(&(context, value)))
    }

    /// A struct value that reaches a `JsValue` destination is seen as its D2
    /// public shape: a fresh plain object of its fields in declaration order,
    /// exactly as an exported result. A value has no identity, so the copy is
    /// the value. Only a single-use value is encoded, so no other use of it
    /// can observe the change of representation.
    fn public_encode(
        &mut self,
        context: ContextId,
        value: ValueId,
        actual: &Type<'_>,
    ) -> Result<bool, FormationError> {
        let admitted = match actual {
            Type::Struct(_) | Type::StructInstance { .. } | Type::Nullable(_) | Type::Union(_) => {
                super::public_structs::adaptable(self.program, actual, 0, self.budget)?
            }
            // A function value is wrapped by a D2 callable adapter.
            Type::Function(signature) => {
                super::public_structs::adaptable_callable(self.program, signature, self.budget)?
            }
            Type::GenericFunction(function) => super::public_structs::adaptable_callable(
                self.program,
                &function.signature,
                self.budget,
            )?,
            _ => false,
        };
        if !admitted {
            return Ok(false);
        }
        // Kept constructors already decode at their own entry. Wrapping one
        // as an ordinary callable would lose identity and constructibility.
        let data = self.data(context);
        if let OperationKind::Load(place) =
            data.operations[data.values[value.index()].definition.index()].kind
        {
            if let Place::Cell(cell) = data.places[place.index()] {
                if let CellBinding::Function(unit) = self.program.cells[cell.index()].binding {
                    if self
                        .program
                        .unit(unit)
                        .is_some_and(|unit| unit.constructor_of.is_some())
                    {
                        return Ok(true);
                    }
                }
            }
        }
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        let unit = self.semantic(context);
        let single = uses
            .unit(unit)
            .and_then(|uses| uses.value_uses(value))
            .is_some_and(|uses| uses.len() == 1);
        if !single {
            return Ok(false);
        }
        self.work(self.struct_plan.public_encodes.len() + 1)?;
        if !self.struct_plan.public_encodes.contains(&(context, value)) {
            self.budget.push(
                AllocationClass::Scratch,
                &mut self.struct_plan.public_encodes,
                (context, value),
            )?;
        }
        Ok(true)
    }

    fn struct_transfer_destination(
        &mut self,
        context: ContextId,
        place: PlaceId,
        value: ValueId,
        span: Span,
    ) -> Result<(), FormationError> {
        if !self.struct_boundary_value(context, value) {
            return Ok(());
        }
        let projected = if matches!(
            self.data(context).places[place.index()],
            Place::Field { .. }
        ) {
            super::super::schema::place_type(self.program, self.data(context), place, self.budget)?
        } else {
            None
        };
        let member = self.member_declared_type(context, place)?;
        let expected = match self.data(context).places[place.index()] {
            Place::Cell(cell) => &self.program.types[self.program.cells[cell.index()].ty.index()],
            Place::Field { .. } => projected
                .as_deref()
                .ok_or_else(|| self.error(span, "missing instantiated field type"))?,
            Place::Member { .. } | Place::ClassField { .. } | Place::Index { .. } => {
                match member.as_deref() {
                    Some(declared) => declared,
                    None => {
                        return Err(
                            self.error(span, "value-struct storage requires an ABI adapter")
                        );
                    }
                }
            }
            Place::Value(_) => return Err(self.error(span, "read-only value destination")),
        };
        self.struct_transfer(context, value, expected, span)
    }

    /// Physical product layout is determined by the instantiated field schema.
    /// Reflected/extern storage is adapted by the shared storage recipe.
    fn member_declared_type(
        &mut self,
        context: ContextId,
        place: PlaceId,
    ) -> Result<Option<std::borrow::Cow<'program, Type<'src>>>, FormationError> {
        self.work(1)?;
        let program = self.program;
        let data = self.data(context);
        let receiver = match data.places[place.index()] {
            Place::ClassField { .. } => {
                self.public_storage_type(context, place)?;
                return Ok(super::super::schema::place_type(program, data, place, self.budget)?);
            }
            Place::Member { receiver, .. } | Place::Index { receiver, .. } => receiver,
            _ => return Ok(None),
        };
        Ok(match &program.types[data.values[receiver.index()].ty.index()] {
            Type::Array(inner) | Type::Record(inner) => Some(std::borrow::Cow::Borrowed(inner.as_ref())),
            dynamic @ Type::Dynamic => Some(std::borrow::Cow::Borrowed(dynamic)),
            _ => None,
        })
    }

    fn struct_load_interface(
        &mut self,
        context: ContextId,
        place: PlaceId,
        result: ValueId,
        span: Span,
    ) -> Result<(), FormationError> {
        if !self.struct_boundary_value(context, result) {
            return Ok(());
        }
        self.work(1)?;
        let data = self.data(context);
        let projected = if matches!(data.places[place.index()], Place::Field { .. }) {
            super::super::schema::place_type(self.program, data, place, self.budget)?
        } else {
            None
        };
        let member = self.member_declared_type(context, place)?;
        let declared = match data.places[place.index()] {
            Place::Cell(cell) => &self.program.types[self.program.cells[cell.index()].ty.index()],
            Place::Value(value) => &self.program.types[data.values[value.index()].ty.index()],
            Place::Field { .. } => projected
                .as_deref()
                .ok_or_else(|| self.error(span, "missing instantiated field type"))?,
            Place::Member { .. } | Place::ClassField { .. } | Place::Index { .. } => {
                match member.as_deref() {
                    Some(declared) => declared,
                    // Dynamic host receivers lack a product interface.
                    None => {
                        return Err(self.error(span, "value-struct load requires an ABI adapter"));
                    }
                }
            }
        };
        let result = &self.program.types[data.values[result.index()].ty.index()];
        // A nullable read can carry the checker's guarded present type. This
        // establishes a representation shape only, not a no-throw or valid
        // non-null proof; ordinary access checks remain in the target recipe.
        let declared = match (declared, result) {
            (Type::Nullable(inner), result) if !matches!(result, Type::Nullable(_)) => {
                inner.as_ref()
            }
            _ => declared,
        };
        self.struct_transfer_shape(result, declared, span)
    }

    fn validate_struct_transfers(
        &mut self,
        context: ContextId,
        operation: &Operation,
    ) -> Result<(), FormationError> {
        let data = self.data(context);
        let operands = data.operands(operation.operands).unwrap();
        self.work(operands.len())?;
        let has_struct_operand = operands
            .iter()
            .any(|value| self.struct_boundary_value(context, *value));
        let span = operation.span;
        let result_type = operation
            .result
            .map(|value| &self.program.types[data.values[value.index()].ty.index()]);
        match &operation.kind {
            OperationKind::Initialize(cell) => {
                let expected = &self.program.types[self.program.cells[cell.index()].ty.index()];
                self.struct_transfer(context, operands[0], expected, span)?;
            }
            OperationKind::CopyValue => {
                self.struct_transfer(context, operands[0], result_type.unwrap(), span)?
            }
            OperationKind::Store(place) => {
                self.struct_transfer_destination(context, *place, operands[0], span)?
            }
            OperationKind::Return if has_struct_operand => {
                let expected = match data.callable_type.map(|ty| &self.program.types[ty.index()]) {
                    Some(Type::Function(function)) => function.return_type.as_ref(),
                    Some(Type::GenericFunction(function)) => {
                        function.signature.return_type.as_ref()
                    }
                    _ => return Err(self.error(span, "value-struct return interface")),
                };
                self.struct_transfer(context, operands[0], expected, span)?;
            }
            OperationKind::Throw if has_struct_operand => {
                return Err(self.error(span, "thrown value-struct ABI adaptation"));
            }
            OperationKind::Select { yes, no } => {
                for region in [*yes, *no] {
                    if let Some(value) = data.regions[region.index()].result {
                        self.struct_transfer(context, value, result_type.unwrap(), span)?;
                    }
                }
            }
            OperationKind::ShortCircuit { kind, right } => {
                let expected = result_type.unwrap();
                let left = operands[0];
                if *kind == ShortCircuit::Nullish && self.struct_boundary_value(context, left) {
                    // Only the non-null completion of this operand reaches the result.
                    let actual = &self.program.types[data.values[left.index()].ty.index()];
                    let actual = match actual {
                        Type::Nullable(inner) => inner.as_ref(),
                        _ => actual,
                    };
                    self.struct_transfer_shape(expected, actual, span)?;
                } else {
                    self.struct_transfer(context, left, expected, span)?;
                }
                if let Some(value) = data.regions[right.index()].result {
                    self.struct_transfer(context, value, expected, span)?;
                }
            }
            OperationKind::Allocate { kind, .. } if has_struct_operand => match kind {
                AllocationKind::SpreadArray(spread) => {
                    let array = result_type.unwrap();
                    let Type::Array(element) = array else {
                        return Err(self.error(span, "value-struct collection interface"));
                    };
                    for (&value, &spread) in operands.iter().zip(spread) {
                        let expected = if spread { array } else { element.as_ref() };
                        self.struct_transfer(context, value, expected, span)?;
                    }
                }
                AllocationKind::Array | AllocationKind::Record(_) => {
                    let expected = match result_type.unwrap() {
                        Type::Array(inner) | Type::Record(inner) => inner.as_ref(),
                        _ => return Err(self.error(span, "value-struct collection interface")),
                    };
                    for &value in operands {
                        self.struct_transfer(context, value, expected, span)?;
                    }
                }
                AllocationKind::Instance { class, .. } => {
                    let program = self.program;
                    let definition = program.class(*class)
                        .ok_or_else(|| self.error(span, "missing class schema"))?;
                    for (slot, &value) in operands.iter().enumerate() {
                        self.work(1)?;
                        if !self.struct_boundary_value(context, value) { continue; }
                        let ty = super::super::schema::class_field_type(program, result_type.unwrap(),
                            FieldRef { nominal: *class, slot: slot as u32 },
                            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?
                            .ok_or_else(|| self.error(span, "missing instantiated construction field"))?;
                        if definition.reflected || definition.external {
                            self.require_storage_adapter(&ty, span)?;
                        }
                        self.struct_transfer(context, value, &ty, span)?;
                    }
                }
                AllocationKind::Object(_) => {
                    let fields = if matches!(result_type, Some(Type::Intersection(_))) {
                        super::super::schema::shape_fields(self.program, result_type.unwrap(),
                            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?
                    } else { None };
                    for (index, &value) in operands.iter().enumerate() {
                        self.work(1)?;
                        if !self.struct_boundary_value(context, value) { continue; }
                        if let Some(fields) = &fields {
                            let ty = &fields[index].1;
                            self.require_storage_adapter(ty, span)?;
                            self.struct_transfer(context, value, ty, span)?;
                        } else if matches!(result_type, Some(Type::Dynamic)) {
                            self.struct_transfer(context, value, &Type::Dynamic, span)?;
                        } else {
                            return Err(self.error(span, "value-struct object entry ABI adaptation"));
                        }
                    }
                }
                AllocationKind::Struct(identity) => {
                    let mut fields = None;
                    for definition in self.program.structs.iter() {
                        self.work(1)?;
                        if definition.identity == *identity {
                            fields = Some(definition.fields.clone());
                            break;
                        }
                    }
                    let fields =
                        fields.ok_or_else(|| self.error(span, "missing value-struct schema"))?;
                    for (field, &value) in self.program.fields[fields].iter().zip(operands) {
                        let field_ty = super::super::schema::field_type(
                            self.program,
                            result_type.unwrap(),
                            field,
                            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget),
                        )?
                        .ok_or_else(|| {
                            self.error(span, "missing instantiated construction field")
                        })?;
                        self.struct_transfer(context, value, &field_ty, span)?;
                    }
                }
            },
            OperationKind::Call(call) => {
                let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
                if let Some(signature) = data.call_signature(&data.calls[call.index()]) {
                    let dynamic = matches!(self.program.types[signature.index()], Type::Dynamic);
                    let params = match &self.program.types[signature.index()] {
                        Type::Function(function) => Some(&function.params),
                        Type::GenericFunction(function) => Some(&function.signature.params),
                        _ => None,
                    };
                    for (index, &argument) in arguments.iter().enumerate() {
                        self.work(1)?;
                        let CallArgument::Value(value) = argument else {
                            continue;
                        };
                        // A retained JsValue signature is an ordinary dynamic
                        // call contract. It needs a product adapter only when
                        // an actual argument carries private product backing.
                        if !self.struct_boundary_value(context, value) {
                            continue;
                        }
                        // A dynamic callee takes `JsValue` arguments.
                        if dynamic {
                            self.struct_transfer(context, value, &Type::Dynamic, span)?;
                            continue;
                        }
                        let params = params.ok_or_else(|| {
                            self.error(span, "value-struct call requires an ABI adapter")
                        })?;
                        if let Some(expected) = params.get(index) {
                            self.struct_transfer(context, value, &expected.ty, span)?;
                        } else {
                            return Err(
                                self.error(span, "value-struct variadic argument ABI adaptation")
                            );
                        }
                    }
                } else if self.product_record_keys(context, *call) {
                    // Typed record keys describe the record's public storage,
                    // independently of the representation of its payloads.
                    // No product is passed to or returned from this intrinsic.
                } else if self.product_array_method(
                    context,
                    operation,
                    &data.calls[call.index()].target,
                )? {
                    // Every product-bearing operand was checked closed there.
                } else if matches!(
                    data.calls[call.index()].target,
                    CallTarget::Builtin(BuiltinCall::JsAssume)
                ) {
                    // `JS.assume` to a struct type is the identity; to a host
                    // type (`JsValue`, an extern class) it views the value's
                    // public shape, like any `JsValue` position.
                    if let (Some(result), [CallArgument::Value(value)]) = (result_type, arguments) {
                        let host = Type::Dynamic;
                        let expected =
                            if super::public_structs::carries_product(result, self.budget)? {
                                result
                            } else {
                                &host
                            };
                        self.struct_transfer(context, *value, expected, span)?;
                    }
                } else {
                    // A host builtin's operands are `JsValue` positions.
                    let host = matches!(
                        data.calls[call.index()].target,
                        CallTarget::Builtin(builtin) if crate::catalog::host_builtin(builtin)
                    );
                    for &argument in arguments {
                        self.work(1)?;
                        if let CallArgument::Value(value) | CallArgument::Spread(value) = argument {
                            if self.struct_boundary_value(context, value) {
                                let actual =
                                    &self.program.types[data.values[value.index()].ty.index()];
                                if host && self.public_encode(context, value, actual)? {
                                    continue;
                                }
                                return Err(self.error(
                                    span,
                                    "value-struct primitive argument ABI adaptation",
                                ));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Whether this call is an array method that moves whole product
    /// elements between program code without reading their representation.
    ///
    /// Host model: the standard `Array.prototype` methods are intact and no
    /// built-in prototype defines accessors for array index keys. This is the
    /// model of every JavaScript compiler, including the old route; a host
    /// that breaks it can observe private product backing through an absent
    /// index, which a program-owned array never exposes otherwise. Public
    /// boundaries stay exact through their D2 adapters.
    ///
    /// Equality (`indexOf`, `includes`) would compare backing identity and
    /// `join` would print it, so both stay refused. Every product-bearing
    /// operand and result must have a closed shape: one recipe per nominal
    /// struct, with no adapter between instantiations or union members.
    fn product_array_method(
        &mut self,
        context: ContextId,
        operation: &Operation,
        target: &CallTarget,
    ) -> Result<bool, FormationError> {
        let data = self.data(context);
        let (method, receiver) = match *target {
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Method(method),
                receiver: Some(receiver),
            } => (method, Some(receiver)),
            // A new program-owned Map; its values may be products.
            CallTarget::Intrinsic {
                operation: ResolvedIntrinsic::Constructor(Intrinsic::MapNew),
                receiver: None,
            } => (Intrinsic::MapNew, None),
            _ => return Ok(false),
        };
        // A Map compares keys by identity, so only its values may carry
        // products; a Set of products would compare backing identity.
        let collection = receiver
            .map(|receiver| data.values[receiver.index()].ty)
            .or_else(|| {
                operation
                    .result
                    .map(|result| data.values[result.index()].ty)
            });
        let map_key = match collection.map(|ty| &self.program.types[ty.index()]) {
            Some(Type::Map(key, _)) => Some(key.as_ref()),
            _ => None,
        };
        if let Some(key) = map_key {
            if !matches!(
                method,
                Intrinsic::MapNew
                    | Intrinsic::MapGet
                    | Intrinsic::MapSet
                    | Intrinsic::MapHas
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
            ) || super::public_structs::carries_product(key, self.budget)?
            {
                return Ok(false);
            }
            return self.closed_product_operands(context, operation, receiver);
        }
        let Some(receiver) = receiver else {
            return Ok(false);
        };
        if !matches!(
            method,
            Intrinsic::ArrayPush
                | Intrinsic::ArrayPop
                | Intrinsic::ArraySlice
                | Intrinsic::ArraySplice
                | Intrinsic::ArrayConcat
                | Intrinsic::ArrayReverse
                | Intrinsic::ArrayFill
                | Intrinsic::ArrayCopyWithin
                | Intrinsic::ArrayMap
                | Intrinsic::ArrayFilter
                | Intrinsic::ArrayReduce
                | Intrinsic::ArrayForEach
                | Intrinsic::ArraySome
                | Intrinsic::ArrayEvery
                | Intrinsic::ArrayFindIndex
        ) {
            return Ok(false);
        }
        if !matches!(
            self.program.types[data.values[receiver.index()].ty.index()],
            Type::Array(_)
        ) {
            return Ok(false);
        }
        self.closed_product_operands(context, operation, Some(receiver))
    }

    /// Every product-bearing operand and result of an admitted collection
    /// call has a closed shape.
    fn closed_product_operands(
        &mut self,
        context: ContextId,
        operation: &Operation,
        receiver: Option<ValueId>,
    ) -> Result<bool, FormationError> {
        let data = self.data(context);
        let arguments = match operation.kind {
            OperationKind::Call(call) => {
                data.arguments(data.calls[call.index()].arguments).unwrap()
            }
            _ => &[],
        };
        let values =
            operation
                .result
                .into_iter()
                .chain(receiver)
                .chain(arguments.iter().filter_map(|argument| match *argument {
                    CallArgument::Value(value) => Some(value),
                    _ => None,
                }));
        for value in values {
            self.work(1)?;
            if self.struct_boundary_value(context, value) {
                let ty = &self.program.types[data.values[value.index()].ty.index()];
                self.struct_transfer_shape(ty, ty, operation.span)?;
            }
        }
        Ok(true)
    }

    /// Demand has already qualified each requested original generic body's
    /// complete private inputs and closed typed transport under this contract.
    /// Every physical context consumes that result without another body scan.
    /// A false answer is not a primitive-domain fact and does not relax the
    /// ordinary per-transfer checks below.
    fn validate_generic_product_context(
        &mut self,
        context: ContextId,
    ) -> Result<(), FormationError> {
        self.work(1)?;
        if self.demand.generic_product_transport(context)
            && !self.data(context).callable_type.is_some_and(|ty| {
                matches!(self.program.types[ty.index()], Type::GenericFunction(_))
            })
        {
            return Err(self.error(
                Span::default(),
                "generic product callable declaration mismatch",
            ));
        }
        Ok(())
    }

    /// A closed product representation cannot cross an unadapted opaque ABI.
    /// This qualification belongs to the target, while common verification owns
    /// language typing. In particular JS.assume is not a representation proof.
    /// Returns whether this context's frame must be strict: in a classic
    /// script a sloppy host callback could otherwise read `caller.arguments`
    /// of a struct-bearing frame and see the private product backing.
    pub(super) fn validate_struct_context(
        &mut self,
        context: ContextId,
    ) -> Result<bool, FormationError> {
        if self.struct_plan.boundary_types.is_empty() {
            return Ok(false);
        }
        self.validate_generic_product_context(context)?;
        let data = self.data(context);
        let strict_frame = self.contract.execution
            == crate::compilation_contract::JavaScriptExecution::Script
            && data
                .callable_type
                .is_some_and(|ty| self.struct_plan.boundary_types[ty.index()]);
        for (index, operation) in data.operations.iter().enumerate() {
            self.work(1)?;
            if !self
                .demand
                .needs_operation(context, OpId::from_index(index).unwrap())
            {
                continue;
            }
            self.validate_struct_transfers(context, operation)?;
            let operands = data.operands(operation.operands).unwrap();
            let mut boundary = operation
                .result
                .is_some_and(|value| self.struct_boundary_value(context, value));
            for value in operands {
                self.work(1)?;
                boundary |= self.struct_boundary_value(context, *value);
            }
            let mut product_value = false;
            if boundary && matches!(operation.kind, OperationKind::Binary(_) | OperationKind::TypeTest(_)) {
                for &value in operands {
                    product_value |= product_variant(&self.program.types[data.values[value.index()].ty.index()], self.budget)?;
                }
            }
            match operation.kind {
                OperationKind::Load(mut place)
                | OperationKind::Store(mut place)
                | OperationKind::CheckPlace(mut place) => {
                    let accessed_place = place;
                    while let Place::Field { base, .. } = data.places[place.index()] {
                        self.work(1)?;
                        place = base;
                    }
                    if let Place::Cell(cell) = data.places[place.index()] {
                        let cell = &self.program.cells[cell.index()];
                        if cell.binding == CellBinding::Foreign
                            && self.struct_plan.boundary_types[cell.ty.index()]
                        {
                            return Err(self
                                .error(operation.span, "foreign value-struct storage adaptation"));
                        }
                    }
                    if matches!(operation.kind, OperationKind::Load(_)) {
                        self.struct_load_interface(
                            context,
                            accessed_place,
                            operation.result.unwrap(),
                            operation.span,
                        )?;
                    }
                }
                OperationKind::Call(call) => {
                    let site = &data.calls[call.index()];
                    for &argument in data.arguments(site.arguments).unwrap() {
                        self.work(1)?;
                        // An encoded argument reaches the callee as a plain object.
                        if let CallArgument::Value(value) | CallArgument::Spread(value) = argument {
                            if !self.encoded(context, value)? {
                                boundary |= self.struct_boundary_value(context, value);
                            }
                        }
                    }
                    boundary |= site
                        .contract
                        .signature
                        .is_some_and(|ty| self.struct_plan.boundary_types[ty.index()]);
                    if let CallTarget::Reference { place } = site.target {
                        if matches!(data.places[place.index()], Place::Field { .. }) {
                            return Err(self
                                .error(operation.span, "value-struct method receiver adaptation"));
                        }
                        if let Place::Member { receiver, .. }
                        | Place::ClassField { receiver, .. }
                        | Place::Index { receiver, .. } = data.places[place.index()]
                        {
                            boundary |= self.struct_boundary_value(context, receiver);
                        }
                    }
                    if let CallTarget::Intrinsic {
                        receiver: Some(receiver),
                        ..
                    } = site.target
                    {
                        boundary |= self.struct_boundary_value(context, receiver);
                    }
                    if boundary && self.product_array_method(context, operation, &site.target)? {
                        boundary = false;
                    }
                    // `JS.assume`'s transfer was checked. Record key queries
                    // expose no payload backing; their input shape was checked.
                    if matches!(site.target, CallTarget::Builtin(BuiltinCall::JsAssume))
                        || self.product_record_keys(context, call)
                    {
                        boundary = false;
                    }
                    if boundary {
                        if site.contract.signature.is_some_and(|ty| {
                            matches!(self.program.types[ty.index()], Type::Union(_))
                        }) {
                            return Err(self
                                .error(operation.span, "value-struct union call ABI adaptation"));
                        }
                        if site.contract.instantiation.is_some() {
                            let uses = self.uses.ok_or_else(|| {
                                self.error(
                                    operation.span,
                                    "generic product call requires complete use index",
                                )
                            })?;
                            let semantic = self.semantic(context);
                            let mut meter = references::Meter(self.budget);
                            if super::super::callable_inputs::body_for_call(
                                self.program,
                                uses,
                                semantic,
                                call,
                                &mut meter,
                            )?
                            .is_none()
                            {
                                return Err(meter.invalid(
                                    "generic product call requires an original owned body",
                                ));
                            }
                            // The demanded physical callee context validates
                            // its complete input scope and shared origin rule.
                        }
                        let mut owned = if let CallTarget::Value { callee, .. } = site.target {
                            match data.operations[data.values[callee.index()].definition.index()]
                                .kind
                            {
                                OperationKind::Closure(_) => true,
                                OperationKind::Load(place) => match data.places[place.index()] {
                                    Place::Cell(cell) => {
                                        matches!(
                                            self.program.cells[cell.index()].binding,
                                            CellBinding::Function(_)
                                        ) && !self.program.cells[cell.index()].reassigned
                                    }
                                    _ => false,
                                },
                                _ => false,
                            }
                        } else {
                            false
                        };
                        if !owned {
                            if let Some(uses) = self.uses {
                                // Reuse the common original-producer locator
                                // for an immutable local closure handle. This
                                // establishes internal identity; expanded ABI
                                // permission remains the FunctionLayout proof.
                                let semantic = self.semantic(context);
                                let mut meter = references::Meter(self.budget);
                                owned = super::super::callable_inputs::body_for_call(
                                    self.program,
                                    uses,
                                    semantic,
                                    call,
                                    &mut meter,
                                )?
                                .is_some();
                            }
                        }
                        if !owned {
                            // A value of a checked function type is program
                            // code: a host function can only reach one through
                            // a foreign cell, host result, extern class field
                            // or `JS.assume`, and each of those is refused for
                            // a product-bearing type. An escaping function
                            // has no expanded layout, so it takes products
                            // packed, exactly as this call passes them.
                            if let CallTarget::Value { callee, .. } = site.target {
                                owned = matches!(
                                    self.program.types[data.values[callee.index()].ty.index()],
                                    Type::Function(_)
                                );
                            }
                        }
                        if !owned {
                            return Err(
                                self.error(operation.span, "value-struct call boundary adaptation")
                            );
                        }
                    }
                }
                // `length` or `size` of a product collection reads no element.
                OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                    Intrinsic::ArrayLength | Intrinsic::MapSize,
                )) => {}
                OperationKind::Intrinsic(_) if boundary => {
                    return Err(
                        self.error(operation.span, "value-struct intrinsic boundary adaptation")
                    );
                }
                // Source equality must not inherit the identity of this recipe's
                // backing arrays. Null tests remain ordinary nullable tests.
                OperationKind::Binary(_)
                    if product_value && !operands.iter().any(|value| {
                        matches!(
                            self.program.types[data.values[value.index()].ty.index()],
                            Type::Null
                        )
                    }) =>
                {
                    return Err(self.error(operation.span, "value-struct comparison contract"));
                }
                OperationKind::TypeTest(target) if product_value
                    && !matches!(crate::primitive::runtime_type_test(&self.program.types[target.index()]),
                        Some(crate::primitive::RuntimeTypeTest::TypeOf(_))) => {
                    return Err(self.error(operation.span, "a value-struct union needs a representation-independent primitive type test"));
                }
                _ => {}
            }
        }
        Ok(strict_frame)
    }
}
