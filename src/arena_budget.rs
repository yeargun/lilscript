//! Shared, pre-admitted bump growth for parser and stable source storage.
use crate::output_budget::AllocationError;
use bumpalo::Bump;
use std::alloc::Layout;
use std::cell::Cell;

pub(crate) trait ArenaAdmission {
    fn work(&self, units: u64) -> Result<(), AllocationError>;
    fn reserve(&self, additional: usize) -> Result<(), AllocationError>;
    fn settle(&self);
}

pub(crate) fn reserve(
    arena: &Bump,
    charged: &Cell<usize>,
    additional: usize,
    retain: impl FnOnce(u64) -> Result<(), AllocationError>,
) -> Result<(), AllocationError> {
    let next = charged
        .get()
        .checked_add(additional)
        .ok_or(AllocationError::Capacity)?;
    retain(additional as u64)?;
    charged.set(next);
    arena.set_allocation_limit(Some(next));
    Ok(())
}

pub(crate) fn settle(arena: &Bump, charged: &Cell<usize>, release: impl FnOnce(u64)) {
    let actual = arena.allocated_bytes();
    let unused = charged
        .get()
        .checked_sub(actual)
        .expect("bump growth cannot exceed admitted backing capacity");
    // A limit below allocated_bytes disables bumpalo's slow-path limit.
    arena.set_allocation_limit(Some(actual));
    release(unused as u64);
    charged.set(actual);
}

pub(crate) fn attempt<A: ArenaAdmission + ?Sized, T>(
    arena: &Bump,
    admission: &A,
    layout: Layout,
    mut allocate: impl FnMut() -> Result<T, AllocationError>,
) -> Result<T, AllocationError> {
    admission.work(1)?;
    match allocate() {
        Ok(value) => return Ok(value),
        Err(AllocationError::AllocationFailed) => {}
        Err(error) => return Err(error),
    }
    let mut allowance = layout
        .size()
        .checked_add(layout.align())
        .map(|minimum| minimum.max(arena.allocated_bytes()).max(512))
        .and_then(usize::checked_next_power_of_two)
        .ok_or(AllocationError::Capacity)?;
    // No private bumpalo chunk-size formula is duplicated. Every retry has a
    // finite, preadmitted limit, and failed attempts leave prior chunks live.
    for _ in 0..3 {
        admission.reserve(allowance)?;
        let settlement = Settlement(admission);
        let result = allocate();
        drop(settlement);
        match result {
            Ok(value) => return Ok(value),
            Err(AllocationError::AllocationFailed) => {}
            Err(error) => return Err(error),
        }
        allowance = allowance.checked_mul(2).ok_or(AllocationError::Capacity)?;
    }
    Err(AllocationError::AllocationFailed)
}

struct Settlement<'a, A: ArenaAdmission + ?Sized>(&'a A);

impl<A: ArenaAdmission + ?Sized> Drop for Settlement<'_, A> {
    fn drop(&mut self) {
        self.0.settle();
    }
}
