//! Admitted ordering of scratch rows. Standard sort pays its conservative
//! comparison allowance first; lookups admit actual compared text prefixes.
use crate::compilation_policy::WorkKind;
use crate::output_budget::{AllocationBudget, AllocationError};
use std::cmp::Ordering;

pub(super) fn work(
    budget: &mut AllocationBudget<'_>,
    amount: usize,
) -> Result<(), AllocationError> {
    budget.work(
        WorkKind::Analysis,
        u64::try_from(amount).map_err(|_| AllocationError::Capacity)?,
    )
}

pub(super) fn scalar<T: Ord>(
    left: &T,
    right: &T,
    budget: &mut AllocationBudget<'_>,
) -> Result<Ordering, AllocationError> {
    work(budget, 1)?;
    Ok(left.cmp(right))
}

pub(super) fn text(
    left: &str,
    right: &str,
    budget: &mut AllocationBudget<'_>,
) -> Result<Ordering, AllocationError> {
    work(
        budget,
        left.len()
            .min(right.len())
            .checked_add(1)
            .ok_or(AllocationError::Capacity)?,
    )?;
    Ok(left.cmp(right))
}

pub(super) fn sort<T>(
    values: &mut [T],
    budget: &mut AllocationBudget<'_>,
    comparison_work: usize,
    compare: impl FnMut(&T, &T) -> Ordering,
) -> Result<(), AllocationError> {
    if values.len() < 2 {
        return work(budget, values.len());
    }
    let levels = (usize::BITS - values.len().max(1).leading_zeros()) as usize + 1;
    let allowance = values
        .len()
        .checked_mul(levels)
        .and_then(|n| n.checked_mul(comparison_work))
        .ok_or(AllocationError::Capacity)?;
    work(budget, allowance)?;
    values.sort_unstable_by(compare);
    Ok(())
}

pub(super) fn unique<T>(
    values: &mut [T],
    budget: &mut AllocationBudget<'_>,
    comparison_work: usize,
    mut compare: impl FnMut(&T, &T) -> Ordering,
) -> Result<bool, AllocationError> {
    sort(values, budget, comparison_work, &mut compare)?;
    work(
        budget,
        values
            .len()
            .saturating_sub(1)
            .checked_mul(comparison_work)
            .ok_or(AllocationError::Capacity)?,
    )?;
    for pair in values.windows(2) {
        if compare(&pair[0], &pair[1]) == Ordering::Equal {
            return Ok(false);
        }
    }
    Ok(true)
}

pub(super) fn find<'a, T>(
    values: &'a [T],
    budget: &mut AllocationBudget<'_>,
    mut compare: impl FnMut(&T, &mut AllocationBudget<'_>) -> Result<Ordering, AllocationError>,
) -> Result<Option<&'a T>, AllocationError> {
    let (mut left, mut right) = (0, values.len());
    while left < right {
        work(budget, 1)?;
        let middle = left + (right - left) / 2;
        match compare(&values[middle], budget)? {
            Ordering::Less => left = middle + 1,
            Ordering::Greater => right = middle,
            Ordering::Equal => return Ok(Some(&values[middle])),
        }
    }
    Ok(None)
}
