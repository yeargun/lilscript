//! Fresh analysis backing uses the enclosing compiler allocation owner.
//! Container headers live in their containing value; these helpers admit
//! nested vector payloads and shared allocation headers before construction.
use crate::compilation_policy::WorkKind;
use crate::output_budget::{vector_bytes, AllocationBudget, AllocationClass, AllocationError};
use std::sync::Arc;

pub(super) fn collect<T>(
    values: impl IntoIterator<Item = T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<T>, AllocationError> {
    let values = values.into_iter();
    let (lower, upper) = values.size_hint();
    let mut result = budget.vector(class, if upper == Some(lower) { lower } else { 0 })?;
    for value in values {
        budget.push(class, &mut result, value)?;
    }
    Ok(result)
}

pub(super) fn release<T>(
    values: Vec<T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes = vector_bytes(&values)?;
    drop(values);
    budget.release(class, bytes)
}

pub(super) fn shared<T>(
    value: T,
    budget: &mut AllocationBudget<'_>,
) -> Result<Arc<T>, AllocationError> {
    budget.work(WorkKind::Analysis, 1)?;
    budget.retain(
        AllocationClass::Retained,
        (std::mem::size_of::<T>() + 2 * std::mem::size_of::<usize>()) as u64,
    )?;
    Ok(Arc::new(value))
}
