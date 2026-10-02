//! Source rule storage operations. Copies admit complete nested backing before
//! construction, through the existing conversion owner; inspection delegates
//! to the same operations with an unmetered budget.
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{vector_bytes, AllocationBudget, AllocationClass, AllocationError};
use std::mem::size_of;
use std::sync::Arc;
use AllocationClass::Retained;

pub(super) fn release_vec<T>(
    values: Vec<T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    let bytes = vector_bytes(&values)?;
    drop(values);
    budget.release(class, bytes)
}

pub(super) fn collect<T>(
    values: impl IntoIterator<Item = T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<T>, AllocationError> {
    let values = values.into_iter();
    let (min, max) = values.size_hint();
    let mut result = budget.vector(class, if max == Some(min) { min } else { 0 })?;
    for value in values {
        budget.push(class, &mut result, value)?;
    }
    Ok(result)
}

pub(super) fn extend<T>(
    target: &mut Vec<T>,
    values: impl IntoIterator<Item = T>,
    class: AllocationClass,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    for value in values {
        budget.push(class, target, value)?;
    }
    Ok(())
}

/// Only inline records without owning fields use this helper.
fn shallow<T: Clone>(
    values: &[T],
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<T>, AllocationError> {
    budget.work(WorkKind::Edit, values.len() as u64)?;
    let mut result = budget.vector(Retained, values.len())?;
    result.extend_from_slice(values);
    Ok(result)
}

pub(super) fn allocation_bytes(kind: &OperationKind) -> Result<u64, AllocationError> {
    match kind {
        OperationKind::Allocate {
            kind:
                AllocationKind::Record(keys)
                | AllocationKind::Object(keys)
                | AllocationKind::Instance { keys, .. },
            ..
        } => vector_bytes(keys),
        OperationKind::Allocate {
            kind: AllocationKind::SpreadArray(spread),
            ..
        } => vector_bytes(spread),
        _ => Ok(0),
    }
}

pub(super) fn kind(
    kind: &OperationKind,
    budget: &mut AllocationBudget<'_>,
) -> Result<OperationKind, AllocationError> {
    Ok(match kind {
        OperationKind::Allocate { identity, kind } => OperationKind::Allocate {
            identity: *identity,
            kind: match kind {
                AllocationKind::Record(keys) => {
                    AllocationKind::Record(budget.copy_slice(Retained, keys)?)
                }
                AllocationKind::Object(keys) => {
                    AllocationKind::Object(budget.copy_slice(Retained, keys)?)
                }
                AllocationKind::Instance { class, keys } => AllocationKind::Instance {
                    class: *class,
                    keys: budget.copy_slice(Retained, keys)?,
                },
                AllocationKind::SpreadArray(spread) => {
                    AllocationKind::SpreadArray(budget.copy_slice(Retained, spread)?)
                }
                other => other.clone(),
            },
        },
        other => other.clone(),
    })
}

pub(super) fn operation(
    source: &Operation,
    budget: &mut AllocationBudget<'_>,
) -> Result<Operation, AllocationError> {
    Ok(Operation {
        authored: source.authored,
        kind: kind(&source.kind, budget)?,
        operands: source.operands,
        result: source.result,
        region: source.region,
        origin: source.origin,
        span: source.span,
    })
}

pub(super) fn unit(
    source: &UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<UnitData, AllocationError> {
    budget.retained_phase(|budget| {
        let mut operations = budget.vector(Retained, source.operations.len())?;
        for row in &source.operations {
            operations.push(operation(row, budget)?);
        }
        let mut regions = budget.vector(Retained, source.regions.len())?;
        for row in &source.regions {
            regions.push(Region {
                authored: row.authored,
                parent: row.parent,
                operations: budget.copy_slice(Retained, &row.operations)?,
                result: row.result,
                span: row.span,
            });
        }
        let mut call_instantiations = budget.vector(Retained, source.call_instantiations.len())?;
        for row in &source.call_instantiations {
            call_instantiations.push(CallInstantiation {
                declaration: row.declaration,
                arguments: budget.copy_slice(Retained, &row.arguments)?,
                signature: row.signature,
            });
        }
        Ok(UnitData {
            kind: source.kind,
            suspension: source.suspension,
            parameter_region: source.parameter_region,
            constructor_of: source.constructor_of,
            module: source.module,
            instantiation_prefix: source.instantiation_prefix,
            function_name: source.function_name,
            callable_type: source.callable_type,
            native_default_length: source.native_default_length,
            declared_length: source.declared_length,
            entry: source.entry,
            parameters: budget.copy_slice(Retained, &source.parameters)?,
            captures: budget.copy_slice(Retained, &source.captures)?,
            operands: budget.copy_slice(Retained, &source.operands)?,
            values: shallow(&source.values, budget)?,
            places: shallow(&source.places, budget)?,
            calls: shallow(&source.calls, budget)?,
            call_arguments: shallow(&source.call_arguments, budget)?,
            operations,
            regions,
            call_instantiations,
        })
    })
}

pub(super) fn freeze(
    id: UnitId,
    data: UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<FrozenUnit, AllocationError> {
    budget.work(WorkKind::Edit, 1)?;
    budget.retain(
        Retained,
        (size_of::<UnitData>() + 2 * size_of::<usize>()) as u64,
    )?;
    Ok(WorkingUnit::new(id, data).freeze())
}

pub(super) fn table<T>(
    values: Vec<T>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Arc<Vec<T>>, AllocationError> {
    budget.work(WorkKind::Edit, 1)?;
    budget.retain(
        Retained,
        (size_of::<Vec<T>>() + 2 * size_of::<usize>()) as u64,
    )?;
    Ok(Arc::new(values))
}

pub(super) fn cell(
    source: &Cell,
    budget: &mut AllocationBudget<'_>,
) -> Result<Cell, AllocationError> {
    Ok(Cell {
        declared_define: source.declared_define,
        declared_const: source.declared_const,
        source_symbol: source.source_symbol,
        name: budget.string(Retained, &source.name)?,
        ty: source.ty,
        owner: source.owner,
        region: source.region,
        declaration: source.declaration,
        reassigned: source.reassigned,
        observable_before_initialization: source.observable_before_initialization,
        binding: source.binding,
        synthetic: source.synthetic,
        declared_pure: source.declared_pure,
        debug: source.debug,
    })
}

pub(super) fn type_bytes(
    ty: &crate::check::Type<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<u64, AllocationError> {
    use crate::check::type_payload::{measure_payload, Payload, PayloadError};
    match measure_payload(Payload::Type(ty), budget, |_| {
        Ok::<_, std::convert::Infallible>(())
    }) {
        Ok(measure) => {
            budget.work(WorkKind::Edit, measure.nodes)?;
            Ok(measure.owned_bytes)
        }
        Err(PayloadError::Allocation(error)) => Err(error),
        Err(PayloadError::Visitor(never)) => match never {},
    }
}

/// Uses the same conservative shared-signature partition as conversion and
/// publication, then releases proven unused capacity after the admitted clone.
pub(super) fn ty<'src>(
    ty: &crate::check::Type<'src>,
    budget: &mut AllocationBudget<'_>,
) -> Result<crate::check::Type<'src>, AllocationError> {
    if !budget.is_accounted() {
        return Ok(ty.clone());
    }
    let allowance = type_bytes(ty, budget)?;
    budget.retain(Retained, allowance)?;
    let result = ty.clone();
    let actual = type_bytes(&result, budget)?;
    budget.release(
        Retained,
        allowance
            .checked_sub(actual)
            .ok_or(AllocationError::Unaccounted)?,
    )?;
    Ok(result)
}

pub(super) enum Retired<'src> {
    Unit(FrozenUnit),
    Cells(Arc<Vec<Cell>>),
    Types(Arc<Vec<crate::check::Type<'src>>>),
    Strings(Arc<Vec<StringValue>>),
}
impl Retired<'_> {
    pub(super) fn unique(&self) -> bool {
        match self {
            Self::Unit(unit) => unit.allocation_is_unique(),
            Self::Cells(rows) => Arc::strong_count(rows) == 1,
            Self::Types(rows) => Arc::strong_count(rows) == 1,
            Self::Strings(rows) => Arc::strong_count(rows) == 1,
        }
    }
    pub(super) fn bytes(&self, budget: &mut AllocationBudget<'_>) -> Result<u64, AllocationError> {
        fn header<T>(rows: &Vec<T>) -> Result<u64, AllocationError> {
            Ok(vector_bytes(rows)? + (size_of::<Vec<T>>() + 2 * size_of::<usize>()) as u64)
        }
        Ok(match self {
            Self::Unit(unit) => unit.allocation_bytes().ok_or(AllocationError::Capacity)?,
            Self::Cells(rows) => rows.iter().try_fold(header(rows)?, |sum, row| {
                sum.checked_add(row.name.capacity() as u64)
                    .ok_or(AllocationError::Capacity)
            })?,
            Self::Types(rows) => {
                let mut sum = header(rows)?;
                for row in rows.iter() {
                    sum = sum
                        .checked_add(type_bytes(row, budget)?)
                        .ok_or(AllocationError::Capacity)?;
                }
                sum
            }
            Self::Strings(rows) => rows.iter().try_fold(header(rows)?, |sum, row| {
                sum.checked_add(row.capacity_bytes() as u64)
                    .ok_or(AllocationError::Capacity)
            })?,
        })
    }
}

/// Drop a temporary admitted body (without a FrozenUnit Arc header).
pub(super) fn release_unit(
    data: UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    budget.work(
        WorkKind::Edit,
        (data.operations.len() + data.regions.len() + data.call_instantiations.len()) as u64,
    )?;
    let bytes = crate::program::storage::unit_allocation_bytes(&data)
        .ok_or(AllocationError::Capacity)?
        - (size_of::<UnitData>() + 2 * size_of::<usize>()) as u64;
    drop(data);
    budget.release(Retained, bytes)
}

pub(super) use super::super::analysis_storage::{optional, Map};
