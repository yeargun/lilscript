//! Borrowed payload traversal shared by retained input admission and queried
//! type operations. This walks storage shape, never evaluates a type relation.

use super::{DefaultValue, FunctionParameter, FunctionSignature, Type};
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
use std::mem::size_of;

#[derive(Clone, Copy, Debug)]
pub(crate) enum Payload<'a, 'src> {
    Type(&'a Type<'src>),
    Default(&'a DefaultValue<'src>),
    Signature(&'a FunctionSignature<'src>),
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PayloadMeasure {
    /// Nested backing capacities, excluding the root Type/DefaultValue slot.
    /// A Signature root includes its shared Arc allocation and owned buffers.
    /// Shared signatures are counted per occurrence, conservatively matching
    /// the existing input payload allowance; this is not unique live/RSS data.
    pub owned_bytes: u64,
    /// Visited Type, DefaultValue, Signature and parameter/name records.
    pub nodes: u64,
    /// Borrowed names/default string bytes potentially compared or rendered.
    pub text_bytes: u64,
}

#[derive(Debug)]
pub(crate) enum PayloadError<E> {
    Allocation(AllocationError),
    Visitor(E),
}

impl<E> From<AllocationError> for PayloadError<E> {
    fn from(error: AllocationError) -> Self {
        Self::Allocation(error)
    }
}

fn count(value: usize) -> Result<u64, AllocationError> {
    u64::try_from(value).map_err(|_| AllocationError::Capacity)
}

fn add(value: &mut u64, amount: u64) -> Result<(), AllocationError> {
    *value = value.checked_add(amount).ok_or(AllocationError::Capacity)?;
    Ok(())
}

fn backing<T>(capacity: usize) -> Result<u64, AllocationError> {
    count(
        capacity
            .checked_mul(size_of::<T>())
            .ok_or(AllocationError::Capacity)?,
    )
}

fn records(
    measured: &mut PayloadMeasure,
    budget: &mut AllocationBudget<'_>,
    amount: usize,
) -> Result<(), AllocationError> {
    let amount = count(amount)?;
    budget.work(WorkKind::Analysis, amount)?;
    add(&mut measured.nodes, amount)
}

fn text(
    measured: &mut PayloadMeasure,
    budget: &mut AllocationBudget<'_>,
    value: &str,
) -> Result<(), AllocationError> {
    let amount = count(value.len())?;
    budget.work(WorkKind::Analysis, amount)?;
    add(&mut measured.text_bytes, amount)
}

fn enqueue<'a, 'src>(
    node: Payload<'a, 'src>,
    next: &mut Option<Payload<'a, 'src>>,
    pending: &mut Vec<Payload<'a, 'src>>,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    if next.is_none() {
        *next = Some(node);
        Ok(())
    } else {
        budget.push(AllocationClass::Scratch, pending, node)
    }
}

/// A single admitted iterative walk. Visitor may validate a retained node but
/// must not independently recurse over its payload or allocate unowned data.
/// Node/name/parameter scan work is admitted before visitor invocation.
/// Primitive leaves and single-child chains allocate no traversal buffer.
/// Any branch stack belongs to an inner scope and is dropped before release.
pub(crate) fn measure_payload<'a, 'src, E>(
    root: Payload<'a, 'src>,
    budget: &mut AllocationBudget<'_>,
    mut visitor: impl FnMut(Payload<'a, 'src>) -> Result<(), E>,
) -> Result<PayloadMeasure, PayloadError<E>> {
    let mut scope = budget.scope();
    let mut pending = Vec::new();
    let mut next = Some(root);
    let mut measured = PayloadMeasure::default();
    let result = (|| {
        while let Some(node) = next.take().or_else(|| pending.pop()) {
            records(&mut measured, &mut scope, 1)?;
            match node {
                Payload::Type(
                    Type::Enum(name)
                    | Type::Class(name)
                    | Type::TypeParameter(name)
                    | Type::ClassInstance { name, .. },
                )
                | Payload::Default(
                    DefaultValue::String(name)
                    | DefaultValue::Struct { name, .. }
                    | DefaultValue::NewClass { name, .. },
                ) => {
                    text(&mut measured, &mut scope, name)?;
                }
                Payload::Type(
                    Type::Struct(declaration) | Type::StructInstance { declaration, .. },
                ) => {
                    text(&mut measured, &mut scope, declaration.name)?;
                }
                Payload::Type(Type::GenericFunction(function)) => {
                    records(&mut measured, &mut scope, function.type_params.len())?;
                    for name in &function.type_params {
                        text(&mut measured, &mut scope, name)?;
                    }
                }
                Payload::Signature(signature) => {
                    // Pays the validation scan before a visitor invokes the
                    // existing shared parameter-contract validator.
                    records(&mut measured, &mut scope, signature.params.len())?;
                }
                _ => {}
            }
            visitor(node).map_err(PayloadError::Visitor)?;

            match node {
                Payload::Type(ty) => match ty {
                    Type::Array(inner)
                    | Type::Record(inner)
                    | Type::Set(inner)
                    | Type::Task(inner)
                    | Type::Generator(inner)
                    | Type::Nullable(inner) => {
                        add(&mut measured.owned_bytes, count(size_of::<Type<'_>>())?)?;
                        enqueue(Payload::Type(inner), &mut next, &mut pending, &mut scope)?;
                    }
                    Type::Map(key, value) => {
                        add(&mut measured.owned_bytes, backing::<Type<'_>>(2)?)?;
                        enqueue(Payload::Type(key), &mut next, &mut pending, &mut scope)?;
                        enqueue(Payload::Type(value), &mut next, &mut pending, &mut scope)?;
                    }
                    Type::Union(members)
                    | Type::StructInstance { args: members, .. }
                    | Type::ClassInstance { args: members, .. } => {
                        add(
                            &mut measured.owned_bytes,
                            backing::<Type<'_>>(members.capacity())?,
                        )?;
                        scope.work(WorkKind::Analysis, count(members.len())?)?;
                        for child in members {
                            enqueue(Payload::Type(child), &mut next, &mut pending, &mut scope)?;
                        }
                    }
                    Type::Function(signature) => {
                        enqueue(
                            Payload::Signature(signature),
                            &mut next,
                            &mut pending,
                            &mut scope,
                        )?;
                    }
                    Type::GenericFunction(function) => {
                        add(
                            &mut measured.owned_bytes,
                            backing::<&str>(function.type_params.capacity())?,
                        )?;
                        enqueue(
                            Payload::Signature(&function.signature),
                            &mut next,
                            &mut pending,
                            &mut scope,
                        )?;
                    }
                    _ => {}
                },
                Payload::Signature(signature) => {
                    add(
                        &mut measured.owned_bytes,
                        count(size_of::<FunctionSignature<'_>>())?,
                    )?;
                    add(&mut measured.owned_bytes, backing::<usize>(2)?)?;
                    add(
                        &mut measured.owned_bytes,
                        backing::<FunctionParameter<'_>>(signature.params.capacity())?,
                    )?;
                    add(&mut measured.owned_bytes, count(size_of::<Type<'_>>())?)?;
                    for parameter in &signature.params {
                        enqueue(
                            Payload::Type(&parameter.ty),
                            &mut next,
                            &mut pending,
                            &mut scope,
                        )?;
                        if let Some(default) = &parameter.default {
                            enqueue(
                                Payload::Default(default),
                                &mut next,
                                &mut pending,
                                &mut scope,
                            )?;
                        }
                    }
                    enqueue(
                        Payload::Type(&signature.return_type),
                        &mut next,
                        &mut pending,
                        &mut scope,
                    )?;
                }
                Payload::Default(value) => {
                    if let DefaultValue::Array(values)
                    | DefaultValue::Struct { values, .. }
                    | DefaultValue::NewClass { args: values, .. } = value
                    {
                        add(
                            &mut measured.owned_bytes,
                            backing::<DefaultValue<'_>>(values.capacity())?,
                        )?;
                        scope.work(WorkKind::Analysis, count(values.len())?)?;
                        for value in values {
                            enqueue(Payload::Default(value), &mut next, &mut pending, &mut scope)?;
                        }
                    }
                }
            }
        }
        Ok(measured)
    })();
    drop(pending);
    result
}

#[cfg(test)]
#[path = "type_payload_tests.rs"]
mod tests;
