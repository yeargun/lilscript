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

/// A legality probe can decline normally while allocation refusal must abort
/// its caller. Keep those outcomes distinct without retaining the allocations
/// of a rejected probe. This uses the caller's existing ledger and scope.
pub(super) fn optional<T>(
    budget: &mut AllocationBudget<'_>,
    build: impl FnOnce(&mut Attempt<'_, '_>) -> Option<T>,
) -> Result<Option<T>, AllocationError> {
    let mut phase = budget.scope();
    let mut attempt = Attempt {
        budget: &mut phase,
        failure: None,
    };
    let value = build(&mut attempt);
    let failure = attempt.failure.take();
    drop(attempt);
    if let Some(error) = failure {
        return Err(error);
    }
    if value.is_some() {
        phase.finish_retained()?;
    }
    Ok(value)
}

pub(super) struct Attempt<'a, 'ledger> {
    budget: &'a mut AllocationBudget<'ledger>,
    failure: Option<AllocationError>,
}
impl Attempt<'_, '_> {
    pub(super) fn admit<T>(
        &mut self,
        build: impl FnOnce(&mut AllocationBudget<'_>) -> Result<T, AllocationError>,
    ) -> Option<T> {
        if self.failure.is_some() {
            return None;
        }
        match build(self.budget) {
            Ok(value) => Some(value),
            Err(error) => {
                self.failure = Some(error);
                None
            }
        }
    }
    pub(super) fn work(&mut self, amount: u64) -> Option<()> {
        self.admit(|budget| budget.work(WorkKind::Analysis, amount))
    }
    pub(super) fn vector<T>(&mut self, class: AllocationClass, capacity: usize) -> Option<Vec<T>> {
        self.admit(|budget| budget.vector(class, capacity))
    }
    pub(super) fn filled<T: Copy>(
        &mut self,
        class: AllocationClass,
        count: usize,
        value: T,
    ) -> Option<Vec<T>> {
        self.admit(|budget| budget.filled(class, count, value))
    }
    pub(super) fn push<T>(
        &mut self,
        class: AllocationClass,
        values: &mut Vec<T>,
        value: T,
    ) -> Option<()> {
        self.admit(|budget| budget.push(class, values, value))
    }
    pub(super) fn collect<T>(
        &mut self,
        class: AllocationClass,
        values: impl IntoIterator<Item = T>,
    ) -> Option<Vec<T>> {
        self.admit(|budget| collect(values, class, budget))
    }
}

/// Small sparse ID remaps keep sorted inline entries, with no opaque hash
/// allocation or random iteration order. Keys/values have no owned payload;
/// callers separately admit any nested storage before inserting it.
pub(super) struct Map<K, V> {
    entries: Vec<(K, V)>,
    class: AllocationClass,
}
impl<K: Ord, V> Map<K, V> {
    pub(super) fn new(class: AllocationClass) -> Self {
        Self {
            entries: Vec::new(),
            class,
        }
    }
    pub(super) fn get(&self, key: &K) -> Option<&V> {
        self.entries
            .binary_search_by(|(found, _)| found.cmp(key))
            .ok()
            .map(|index| &self.entries[index].1)
    }
    pub(super) fn iter(&self) -> impl Iterator<Item = (&K, &V)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }
    pub(super) fn insert(
        &mut self,
        key: K,
        value: V,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<V>, AllocationError> {
        budget.work(
            WorkKind::Analysis,
            u64::from(usize::BITS - self.entries.len().max(1).leading_zeros()) + 1,
        )?;
        match self.entries.binary_search_by(|(found, _)| found.cmp(&key)) {
            Ok(index) => Ok(Some(std::mem::replace(&mut self.entries[index].1, value))),
            Err(index) => {
                budget.work(WorkKind::Analysis, (self.entries.len() - index) as u64)?;
                budget.reserve_vec(self.class, &mut self.entries, 1)?;
                self.entries.insert(index, (key, value));
                Ok(None)
            }
        }
    }
    pub(super) fn remove(
        &mut self,
        key: &K,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<V>, AllocationError> {
        budget.work(
            WorkKind::Analysis,
            u64::from(usize::BITS - self.entries.len().max(1).leading_zeros()) + 1,
        )?;
        if let Ok(index) = self.entries.binary_search_by(|(found, _)| found.cmp(key)) {
            budget.work(WorkKind::Analysis, (self.entries.len() - index) as u64)?;
            Ok(Some(self.entries.remove(index).1))
        } else {
            Ok(None)
        }
    }
    pub(super) fn clear(
        &mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        budget.work(WorkKind::Analysis, self.entries.len() as u64)?;
        self.entries.clear();
        Ok(())
    }
    pub(super) fn retain(
        &mut self,
        budget: &mut AllocationBudget<'_>,
        mut keep: impl FnMut(&K, &mut V) -> bool,
    ) -> Result<(), AllocationError> {
        budget.work(WorkKind::Analysis, self.entries.len() as u64)?;
        self.entries.retain_mut(|(key, value)| keep(key, value));
        Ok(())
    }
    pub(super) fn release(self, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
        release(self.entries, self.class, budget)
    }
}
