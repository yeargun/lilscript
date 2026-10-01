//! Closed typed transport of abstract values. Unlike returned-value provenance,
//! this fact permits control flow, private storage and recursive helper calls.
//! It proves no value, identity, effect, termination or inlining property.
//!
//! The caller has sealed the original generic body's complete input interface.
//! Every visited helper is sealed independently; captured bodies are examined
//! with their creator. All backing belongs to the caller's admission scope.
//! The result is valid only for this immutable Program/UseIndex pair.
use super::super::callable_inputs::{self, CallObservations, CallableInputs, InputOutcome};
use super::super::uses::UseIndex;
use super::super::*;
use super::domains::Admission;
use crate::compilation_contract::JavaScriptExecution;
use crate::primitive::Intrinsic;

pub(in crate::program) fn closed_erased_transport<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    body: UnitId,
    execution: JavaScriptExecution,
    admission: &mut A,
) -> Result<Option<Span>, A::Error> {
    let mut pending = admission.vector(0)?;
    let mut abstract_types = admission.vector(program.types.len())?;
    for ty in program.types.iter() {
        let abstract_value = super::contains_type_parameter(ty, &mut pending, admission)?;
        admission.push(&mut abstract_types, abstract_value)?;
    }
    admission.release(pending)?;
    let mut bodies = admission.vector(1)?;
    admission.push(&mut bodies, body)?;
    let mut callbacks = admission.vector(0)?;
    let result = (|| {
        let mut cursor = 0;
        while cursor < bodies.len() {
            let unit = bodies[cursor];
            cursor += 1;
            let data = program
                .unit(unit)
                .ok_or_else(|| admission.invalid("erased transport body"))?;
            if uses
                .unit(unit)
                .is_none_or(|index| index.revision() != program.units[unit.index()].revision())
                || uses.tables_revision() != program.tables_revision
            {
                return Err(admission.invalid("erased transport stale uses"));
            }
            for operation in &data.operations {
                admission.work(1)?;
                let values = data.operands(operation.operands).unwrap();
                let carries =
                    |value: ValueId| abstract_types[data.values[value.index()].ty.index()];
                let mut relevant = operation.result.is_some_and(carries);
                for &value in values {
                    admission.work(1)?;
                    relevant |= carries(value);
                }
                if let OperationKind::Closure(child) = operation.kind {
                    for &capture in &program.unit(child).unwrap().captures {
                        admission.work(1)?;
                        relevant |= abstract_types[program.cells[capture.index()].ty.index()];
                    }
                    if relevant {
                        enqueue(&mut bodies, child, admission)?;
                    }
                }
                if let OperationKind::Call(call) = operation.kind {
                    let call = &data.calls[call.index()];
                    for argument in data.arguments(call.arguments).unwrap() {
                        admission.work(1)?;
                        relevant |= match *argument {
                            CallArgument::Value(value) | CallArgument::Spread(value) => {
                                carries(value)
                            }
                            CallArgument::Reference(_) => true,
                        };
                    }
                    if let CallTarget::Intrinsic {
                        receiver: Some(value),
                        ..
                    }
                    | CallTarget::Value { callee: value, .. } = call.target
                    {
                        relevant |= carries(value);
                    }
                }
                if !relevant {
                    continue;
                }
                // Conversions to an opaque view must not hide backing, and a
                // host-origin value cannot invent the erased private recipe.
                for ty in values
                    .iter()
                    .map(|v| data.values[v.index()].ty)
                    .chain(operation.result.map(|v| data.values[v.index()].ty))
                {
                    if !closed_type(program, &program.types[ty.index()], admission)? {
                        return Ok(Some(operation.span));
                    }
                }
                let safe = match &operation.kind {
                    OperationKind::Initialize(cell) | OperationKind::Declare(cell) => {
                        closed_cell(program, *cell, &bodies, admission)?
                    }
                    OperationKind::Load(place)
                    | OperationKind::Store(place)
                    | OperationKind::CheckPlace(place) => {
                        closed_place(program, data, *place, &bodies, admission)?
                    }
                    OperationKind::Return | OperationKind::Yield { .. } => {
                        let signature = data
                            .callable_type
                            .and_then(|ty| program.types[ty.index()].callable_signature());
                        match signature {
                            Some(signature) => {
                                closed_type(program, &signature.return_type, admission)?
                            }
                            None => false,
                        }
                    }
                    OperationKind::Allocate { kind, .. } => match kind {
                        AllocationKind::Array
                        | AllocationKind::SpreadArray(_)
                        | AllocationKind::Record(_) => true,
                        AllocationKind::Struct(nominal) => {
                            let definition = &program.structs[nominal.index()];
                            let mut safe = true;
                            for (field, &value) in
                                program.fields[definition.fields.clone()].iter().zip(values)
                            {
                                admission.work(1)?;
                                if carries(value) {
                                    safe &= closed_type(
                                        program,
                                        &program.types[field.ty.index()],
                                        admission,
                                    )?;
                                }
                            }
                            safe
                        }
                        // Public storage requires a concrete codec; a private
                        // nominal is a reference containing private values.
                        AllocationKind::Instance { class, .. } => {
                            let definition = program.class(*class).unwrap();
                            let mut safe = !definition.reflected && !definition.external;
                            for ((_, ty), &value) in definition.fields.iter().zip(values) {
                                admission.work(1)?;
                                if carries(value) {
                                    safe &= closed_type(
                                        program,
                                        &program.types[ty.index()],
                                        admission,
                                    )?;
                                }
                            }
                            safe
                        }
                        AllocationKind::Object(_) => false,
                    },
                    OperationKind::Call(call) => {
                        let site = &data.calls[call.index()];
                        let mut safe = true;
                        for argument in data.arguments(site.arguments).unwrap() {
                            let ty = match *argument {
                                CallArgument::Value(value) | CallArgument::Spread(value) => {
                                    data.values[value.index()].ty
                                }
                                CallArgument::Reference(_) => {
                                    safe = false;
                                    continue;
                                }
                            };
                            safe &= closed_type(program, &program.types[ty.index()], admission)?;
                        }
                        if let Some(signature) = data.call_signature(site) {
                            safe &=
                                closed_type(program, &program.types[signature.index()], admission)?;
                        }
                        if !safe {
                            false
                        } else if let CallTarget::Intrinsic {
                            operation,
                            receiver,
                        } = site.target
                        {
                            collection_transport(program, data, operation, receiver)
                        } else if matches!(
                            site.target,
                            CallTarget::Builtin(BuiltinCall::ObjectKeys)
                        ) {
                            matches!(data.arguments(site.arguments).unwrap(), [CallArgument::Value(value)]
                                if matches!(program.types[data.values[value.index()].ty.index()], Type::Record(_)))
                        } else if let Some(child) =
                            callable_inputs::body_for_call(program, uses, unit, *call, admission)?
                        {
                            // A worklist closes recursive strongly connected
                            // groups without recursive Rust calls or assuming
                            // that an unvisited callee has already passed.
                            admission.work(bodies.len())?;
                            if !bodies.contains(&child) {
                                let input = CallableInputs::for_body_published(
                                    program,
                                    uses,
                                    child,
                                    CallObservations::from_execution(execution),
                                    admission,
                                )?;
                                let InputOutcome::Complete(input) = input else {
                                    return Ok(Some(operation.span));
                                };
                                let sealed = input.runtime_inputs_sealed();
                                input.discard(admission)?;
                                if !sealed {
                                    return Ok(Some(operation.span));
                                }
                                admission.push(&mut bodies, child)?;
                            }
                            true
                        } else {
                            closed_callback(
                                program,
                                uses,
                                unit,
                                *call,
                                execution,
                                &abstract_types,
                                &mut callbacks,
                                admission,
                            )?
                        }
                    }
                    OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                        Intrinsic::ArrayLength | Intrinsic::MapSize,
                    )) => true,
                    OperationKind::Binary(_) => values.iter().any(|v| {
                        matches!(program.types[data.values[v.index()].ty.index()], Type::Null)
                    }),
                    // These operations transport values or inspect presence,
                    // without reading a product's fields or backing identity.
                    OperationKind::Constant(_)
                    | OperationKind::CopyValue
                    | OperationKind::Closure(_)
                    | OperationKind::PrepareCall(_)
                    | OperationKind::ShortCircuit { .. }
                    | OperationKind::Select { .. }
                    | OperationKind::IsUndefined { .. }
                    | OperationKind::Await => true,
                    _ => false,
                };
                if !safe {
                    return Ok(Some(operation.span));
                }
            }
        }
        Ok(None)
    })();
    admission.release(bodies)?;
    admission.release(callbacks)?;
    admission.release(abstract_types)?;
    result
}

fn enqueue<A: Admission>(
    bodies: &mut Vec<UnitId>,
    unit: UnitId,
    admission: &mut A,
) -> Result<(), A::Error> {
    admission.work(bodies.len())?;
    if !bodies.contains(&unit) {
        admission.push(bodies, unit)?;
    }
    Ok(())
}

/// An indirect callback keeps the private callable ABI only when every
/// original caller supplies an owned function with a concrete checked schema.
/// A host callable, reassigned parameter or unresolved producer supplies no
/// permission. The common locator and complete input owner do all tracing.
fn closed_callback<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    unit: UnitId,
    call: CallId,
    execution: JavaScriptExecution,
    abstract_types: &[bool],
    callbacks: &mut Vec<CellId>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    let data = program.unit(unit).unwrap();
    let CallTarget::Value {
        callee,
        invocation: Invocation::Value,
    } = data.calls[call.index()].target
    else {
        return Ok(false);
    };
    let OperationKind::Load(place) =
        data.operations[data.values[callee.index()].definition.index()].kind
    else {
        return Ok(false);
    };
    let Place::Cell(cell) = data.places[place.index()] else {
        return Ok(false);
    };
    let storage = &program.cells[cell.index()];
    let CellBinding::Parameter(position) = storage.binding else {
        return Ok(false);
    };
    if storage.owner != unit
        || storage.reassigned
        || data.parameters.get(position as usize) != Some(&cell)
    {
        return Ok(false);
    }
    admission.work(callbacks.len())?;
    if callbacks.contains(&cell) {
        return Ok(true);
    }
    let inputs = CallableInputs::for_body_published(
        program,
        uses,
        unit,
        CallObservations::from_execution(execution),
        admission,
    )?;
    let InputOutcome::Complete(inputs) = inputs else {
        return Ok(false);
    };
    let result = (|| {
        if !inputs.runtime_inputs_sealed() {
            return Ok(false);
        }
        for input in inputs.calls() {
            admission.work(1)?;
            let caller = program.unit(input.caller).unwrap();
            let arguments = caller
                .arguments(caller.calls[input.target.index()].arguments)
                .unwrap();
            let Some(&CallArgument::Value(value)) = arguments.get(position as usize) else {
                return Ok(false);
            };
            let Some(body) =
                callable_inputs::body_for_value(program, uses, input.caller, value, admission)?
            else {
                return Ok(false);
            };
            if program
                .unit(body)
                .unwrap()
                .callable_type
                .is_none_or(|ty| abstract_types[ty.index()])
            {
                return Ok(false);
            }
        }
        Ok(true)
    })();
    inputs.discard(admission)?;
    let complete = result?;
    if complete {
        admission.push(callbacks, cell)?;
    }
    Ok(complete)
}

fn closed_cell<A: Admission>(
    program: &Program<'_>,
    cell: CellId,
    bodies: &[UnitId],
    admission: &mut A,
) -> Result<bool, A::Error> {
    admission.work(bodies.len())?;
    let cell = &program.cells[cell.index()];
    Ok(cell.binding != CellBinding::Foreign
        && (bodies.contains(&cell.owner) || matches!(cell.binding, CellBinding::Function(_)))
        && closed_type(program, &program.types[cell.ty.index()], admission)?)
}

fn closed_place<A: Admission>(
    program: &Program<'_>,
    data: &UnitData,
    mut place: PlaceId,
    bodies: &[UnitId],
    admission: &mut A,
) -> Result<bool, A::Error> {
    loop {
        admission.work(1)?;
        match data.places[place.index()] {
            Place::Cell(cell) => return closed_cell(program, cell, bodies, admission),
            Place::Field { base, field } => {
                if !closed_type(
                    program,
                    &program.types[program.field(field).unwrap().ty.index()],
                    admission,
                )? {
                    return Ok(false);
                }
                place = base;
            }
            Place::ClassField { receiver, field } => {
                let class = program.class(field.nominal).unwrap();
                return Ok(!class.reflected
                    && !class.external
                    && closed_type(
                        program,
                        &program.types[data.values[receiver.index()].ty.index()],
                        admission,
                    )?
                    && closed_type(
                        program,
                        &program.types[program.class_field(field).unwrap().1.index()],
                        admission,
                    )?);
            }
            Place::Index { receiver, .. } => {
                return closed_type(
                    program,
                    &program.types[data.values[receiver.index()].ty.index()],
                    admission,
                )
            }
            Place::Value(value) => {
                return closed_type(
                    program,
                    &program.types[data.values[value.index()].ty.index()],
                    admission,
                )
            }
            Place::Member { .. } => return Ok(false),
        }
    }
}

fn closed_type<A: Admission>(
    program: &Program<'_>,
    root: &Type<'_>,
    admission: &mut A,
) -> Result<bool, A::Error> {
    let mut pending = admission.vector(0)?;
    let mut next = Some(root);
    let result = (|| {
        while let Some(ty) = next.take().or_else(|| pending.pop()) {
            admission.work(1)?;
            match ty {
                Type::Dynamic | Type::Unknown => return Ok(false),
                Type::Class(declaration) | Type::ClassInstance { declaration, .. }
                    if program
                        .class(declaration.identity)
                        .is_none_or(|c| c.reflected || c.external) =>
                {
                    return Ok(false)
                }
                Type::Array(inner)
                | Type::Record(inner)
                | Type::Task(inner)
                | Type::Generator(inner) => next = Some(inner.as_ref()),
                Type::Nullable(inner) => next = Some(inner.as_ref()),
                Type::Set(_) => return Ok(false),
                Type::Map(key, value) => {
                    // Abstract products have no key identity contract.
                    let mut work = admission.vector(0)?;
                    let abstract_key = super::contains_type_parameter(key, &mut work, admission)?;
                    admission.release(work)?;
                    if abstract_key {
                        return Ok(false);
                    }
                    admission.push(&mut pending, key.as_ref())?;
                    next = Some(value.as_ref());
                }
                Type::Union(members)
                | Type::Intersection(members)
                | Type::StructInstance { args: members, .. }
                | Type::ClassInstance { args: members, .. } => {
                    for member in members {
                        admission.push(&mut pending, member)?;
                    }
                }
                _ => {
                    if let Some(signature) = ty.callable_signature() {
                        next = Some(signature.return_type.as_ref());
                        for parameter in &signature.params {
                            if parameter.passing != crate::primitive::ParameterPassing::Value {
                                return Ok(false);
                            }
                            admission.push(&mut pending, &parameter.ty)?;
                        }
                    }
                }
            }
        }
        Ok(true)
    })();
    admission.release(pending)?;
    result
}

fn collection_transport(
    program: &Program<'_>,
    data: &UnitData,
    operation: ResolvedIntrinsic,
    receiver: Option<ValueId>,
) -> bool {
    let method = match operation {
        ResolvedIntrinsic::Method(method) | ResolvedIntrinsic::Constructor(method) => method,
        _ => return false,
    };
    match method {
        Intrinsic::MapNew => true,
        Intrinsic::MapGet
        | Intrinsic::MapSet
        | Intrinsic::MapHas
        | Intrinsic::MapDelete
        | Intrinsic::MapClear => receiver.is_some_and(|value| {
            matches!(
                program.types[data.values[value.index()].ty.index()],
                Type::Map(_, _)
            )
        }),
        Intrinsic::ArrayPush
        | Intrinsic::ArrayPop
        | Intrinsic::ArraySlice
        | Intrinsic::ArraySplice
        | Intrinsic::ArrayConcat
        | Intrinsic::ArrayReverse
        | Intrinsic::ArrayFill
        | Intrinsic::ArrayCopyWithin => receiver.is_some_and(|value| {
            matches!(
                program.types[data.values[value.index()].ty.index()],
                Type::Array(_)
            )
        }),
        _ => false,
    }
}
