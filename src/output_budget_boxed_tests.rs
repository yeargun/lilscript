use super::*;
use crate::compilation_policy::{BudgetPlan, ResourceLimits};
use std::cell::Cell;
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn ledger(memory: u64, work: u64) -> BudgetLedger {
    BudgetLedger::new(
        ResourceLimits::default(),
        BudgetPlan {
            baseline_work: 0,
            optional_work: work,
            baseline_retained_bytes: 0,
            retained_bytes: memory,
        },
    )
    .unwrap()
}

#[repr(align(128))]
struct ObservedDrop(Rc<Cell<usize>>);
impl Drop for ObservedDrop {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn boxed_value_is_admitted_until_actual_drop_and_scope_release() {
    let bytes = size_of::<ObservedDrop>() as u64;
    let mut ledger = ledger(bytes, 100);
    let drops = Rc::new(Cell::new(0));
    {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let value = scope
            .boxed(AllocationClass::Scratch, ObservedDrop(drops.clone()))
            .unwrap();
        assert_eq!(scope.retained_bytes(AllocationClass::Scratch), bytes);
        assert_eq!((&*value as *const ObservedDrop as usize) % 128, 0);
        assert_eq!(drops.get(), 0);
        drop(value);
        assert_eq!(drops.get(), 1);
        assert_eq!(scope.retained_bytes(AllocationClass::Scratch), bytes);
    }
    assert_eq!(ledger.retained_bytes(), 0);
    assert_eq!(ledger.peak_retained_bytes(), bytes);
}

#[test]
fn boxed_memory_or_work_refusal_drops_input_once_and_retains_nothing() {
    let bytes = size_of::<ObservedDrop>() as u64;
    for (memory, work) in [(bytes - 1, 100), (bytes, 0)] {
        let drops = Rc::new(Cell::new(0));
        let mut ledger = ledger(memory, work);
        {
            let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let result = scope.boxed(AllocationClass::Scratch, ObservedDrop(drops.clone()));
            assert!(matches!(result, Err(AllocationError::Budget(_))));
            assert_eq!(drops.get(), 1);
            assert_eq!(scope.retained_bytes(AllocationClass::Scratch), 0);
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.peak_retained_bytes(), 0);
    }
}

#[test]
fn zero_size_box_observes_drop_without_memory_admission() {
    static DROPS: AtomicUsize = AtomicUsize::new(0);
    struct Zero;
    impl Drop for Zero {
        fn drop(&mut self) {
            DROPS.fetch_add(1, Ordering::Relaxed);
        }
    }
    DROPS.store(0, Ordering::Relaxed);
    let mut ledger = ledger(0, 100);
    {
        let mut scope = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
        let value = scope.boxed(AllocationClass::Scratch, Zero).unwrap();
        assert_eq!(size_of::<Zero>(), 0);
        assert_eq!(scope.retained_bytes(AllocationClass::Scratch), 0);
        drop(value);
    }
    assert_eq!(DROPS.load(Ordering::Relaxed), 1);
    assert_eq!(ledger.peak_retained_bytes(), 0);
    assert_eq!(ledger.retained_bytes(), 0);
}
