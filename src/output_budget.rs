//! One lexical owner for admitted compiler-output backing storage.
//!
//! Production borrows its compilation's existing ledger. Inspection uses the
//! same allocation methods with no ledger or scope accounting, so inspection
//! builders may grow buffers across temporary wrappers. Counts cover declared buffer/layout
//! capacities, not allocator bookkeeping or whole-process RSS. No operation
//! creates a second ledger, and no allocation needs a journal entry.
//!
//! Buffers must be dropped before their owning scope releases its charges. A
//! buffer grown here must belong to this exact scope and allocation class;
//! child scopes construct fresh objects and transfer them only on success.
//! Publication's borrowed output facade enforces the external lifetime boundary.
use crate::compilation_policy::{BudgetError, BudgetLedger, WorkDomain, WorkKind};
use crate::literal::StringValue;
use std::alloc::{alloc, Layout};
use std::fmt::{self, Write};
use std::mem::{self, size_of};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AllocationClass {
    Retained,
    Scratch,
}
impl AllocationClass {
    fn index(self) -> usize {
        match self {
            Self::Retained => 0,
            Self::Scratch => 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AllocationError {
    Budget(BudgetError),
    Capacity,
    AllocationFailed,
    Unaccounted,
    WrongOwner,
}
impl From<BudgetError> for AllocationError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
impl fmt::Display for AllocationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Budget(error) => write!(formatter, "output budget: {error:?}"),
            Self::Capacity => formatter.write_str("output allocation capacity overflow"),
            Self::AllocationFailed => formatter.write_str("output allocation failed"),
            Self::Unaccounted => {
                formatter.write_str("inspection has no retained allocation admission")
            }
            Self::WrongOwner => {
                formatter.write_str("retained allocation belongs to another compilation")
            }
        }
    }
}
impl std::error::Error for AllocationError {}

pub(crate) type BudgetSession<'a> = AllocationBudget<'a>;

/// Retained = data that may outlive this phase; Scratch = phase-only data.
/// Scope Drop rolls back both. Successful phases drop actual scratch objects,
/// then call finish_retained to move live retained charges into their parent.
#[must_use = "keep the allocation owner alive until its actual buffers are dropped or transferred"]
pub(crate) struct AllocationBudget<'a> {
    ledger: Option<&'a mut BudgetLedger>,
    domain: WorkDomain,
    live: [u64; 2],
    parent_retained: Option<&'a mut u64>,
}

impl<'a> AllocationBudget<'a> {
    pub(crate) fn new(ledger: Option<(&'a mut BudgetLedger, WorkDomain)>) -> Self {
        let (ledger, domain) = match ledger {
            Some((ledger, domain)) => (Some(ledger), domain),
            None => (None, WorkDomain::Baseline),
        };
        Self {
            ledger,
            domain,
            live: [0; 2],
            parent_retained: None,
        }
    }

    pub(crate) fn is_accounted(&self) -> bool {
        self.ledger.is_some()
    }

    /// Coordinate another existing compilation owner (such as Demand or an
    /// artifact store) without adopting or double-counting its reservations.
    /// This internal callback cannot return a borrow of the underlying ledger.
    /// Callers must not replace the ledger or release this scope's allocations.
    pub(crate) fn with_ledger<R>(
        &mut self,
        inspect: impl FnOnce(Option<(&mut BudgetLedger, WorkDomain)>) -> R,
    ) -> R {
        let domain = self.domain;
        inspect(self.ledger.as_deref_mut().map(|ledger| (ledger, domain)))
    }

    pub(crate) fn scope(&mut self) -> AllocationBudget<'_> {
        AllocationBudget {
            ledger: self.ledger.as_deref_mut(),
            domain: self.domain,
            live: [0; 2],
            parent_retained: Some(&mut self.live[AllocationClass::Retained.index()]),
        }
    }

    /// Consumes a successful phase, after the caller drops its scratch objects.
    /// A root with live retained storage cannot silently finish it unowned.
    pub(crate) fn finish_retained(mut self) -> Result<(), AllocationError> {
        let retained = self.live[AllocationClass::Retained.index()];
        let next = match self.parent_retained.as_deref() {
            Some(parent) => parent
                .checked_add(retained)
                .ok_or(AllocationError::Capacity)?,
            None if retained == 0 => 0,
            None => return Err(BudgetError::InvalidRelease.into()),
        };
        self.release(
            AllocationClass::Scratch,
            self.live[AllocationClass::Scratch.index()],
        )?;
        if let Some(parent) = self.parent_retained.as_deref_mut() {
            *parent = next;
        }
        self.live[AllocationClass::Retained.index()] = 0;
        Ok(())
    }

    pub(crate) fn work(&mut self, kind: WorkKind, units: u64) -> Result<(), AllocationError> {
        if let Some(ledger) = self.ledger.as_deref_mut() {
            ledger.charge(self.domain, kind, units)?;
        }
        Ok(())
    }

    pub(crate) fn retain(
        &mut self,
        class: AllocationClass,
        bytes: u64,
    ) -> Result<(), AllocationError> {
        if self.ledger.is_none() {
            return Ok(());
        }
        let index = class.index();
        let next = self.live[index]
            .checked_add(bytes)
            .ok_or(AllocationError::Capacity)?;
        // Both admitted allocation classes share this one live-byte owner.
        next.checked_add(self.live[1 - index])
            .ok_or(AllocationError::Capacity)?;
        if let Some(ledger) = self.ledger.as_deref_mut() {
            ledger.retain(self.domain, bytes)?;
        }
        self.live[index] = next;
        Ok(())
    }

    /// Call after deallocation. An invalid release leaves every count unchanged.
    /// The fallible API is also usable inside non-unwinding codec callbacks.
    pub(crate) fn release(
        &mut self,
        class: AllocationClass,
        bytes: u64,
    ) -> Result<(), AllocationError> {
        if self.ledger.is_none() {
            return Ok(());
        }
        let index = class.index();
        let next = self.live[index]
            .checked_sub(bytes)
            .ok_or(BudgetError::InvalidRelease)?;
        if let Some(ledger) = self.ledger.as_deref_mut() {
            ledger.release(self.domain, bytes)?;
        }
        self.live[index] = next;
        Ok(())
    }

    /// A successful scratch result becomes retained data in this same scope.
    pub(crate) fn promote(&mut self, bytes: u64) -> Result<(), AllocationError> {
        if self.ledger.is_none() {
            return Ok(());
        }
        let scratch = self.live[1]
            .checked_sub(bytes)
            .ok_or(BudgetError::InvalidRelease)?;
        let retained = self.live[0]
            .checked_add(bytes)
            .ok_or(AllocationError::Capacity)?;
        self.live = [retained, scratch];
        Ok(())
    }

    pub(crate) fn retained_bytes(&self, class: AllocationClass) -> u64 {
        self.live[class.index()]
    }

    /// Move an already admitted allocation into the same Compilation's private
    /// artifact owner. Owner is that compilation's existing store identity.
    /// Dropping a charge without explicit discard conservatively stays charged.
    pub(crate) fn detach_retained<Owner>(
        &mut self,
        owner: Owner,
        bytes: u64,
    ) -> Result<RetainedCharge<Owner>, AllocationError> {
        if self.ledger.is_none() {
            return Err(AllocationError::Unaccounted);
        }
        let remaining = self.live[0]
            .checked_sub(bytes)
            .ok_or(BudgetError::InvalidRelease)?;
        self.live[0] = remaining;
        Ok(RetainedCharge {
            owner,
            domain: self.domain,
            bytes,
        })
    }

    /// Admit a Box's outer allocation in this existing scope. Any nested
    /// payload in value must already have its own admission. The caller drops
    /// the actual Box before releasing this scope. Standard Box allocation
    /// can abort on allocator OOM; recoverable budget refusal happens first.
    pub(crate) fn boxed<T>(
        &mut self,
        class: AllocationClass,
        value: T,
    ) -> Result<Box<T>, AllocationError> {
        self.work(WorkKind::Render, 1)?;
        self.retain(class, count(size_of::<T>())?)?;
        Ok(Box::new(value))
    }

    /// Exact capacity, including for over-aligned T. No initialized T is read
    /// or dropped by allocation, and zero-sized T does not allocate storage.
    pub(crate) fn vector<T>(
        &mut self,
        class: AllocationClass,
        capacity: usize,
    ) -> Result<Vec<T>, AllocationError> {
        self.work(WorkKind::Render, 1)?;
        let layout = VectorLayout::<T>::new(capacity)?;
        let bytes = layout.bytes();
        self.retain(class, bytes)?;
        match layout.allocate() {
            Ok(value) => Ok(value),
            Err(error) => {
                self.release(class, bytes)?;
                Err(error)
            }
        }
    }

    pub(crate) fn filled<T: Copy>(
        &mut self,
        class: AllocationClass,
        length: usize,
        value: T,
    ) -> Result<Vec<T>, AllocationError> {
        self.work(WorkKind::Render, count(length)?)?;
        let mut result = self.vector(class, length)?;
        result.resize(length, value);
        Ok(result)
    }

    pub(crate) fn copy_slice<T: Copy>(
        &mut self,
        class: AllocationClass,
        values: &[T],
    ) -> Result<Vec<T>, AllocationError> {
        self.work(WorkKind::Render, count(values.len())?)?;
        let mut result = self.vector(class, values.len())?;
        result.extend_from_slice(values);
        Ok(result)
    }

    /// Admit the complete destination while the original buffer is still
    /// charged. Move non-Copy elements only after successful allocation; errors
    /// preserve the original Vec and its reservation. No realloc size guessing.
    pub(crate) fn reserve_vec<T>(
        &mut self,
        class: AllocationClass,
        vector: &mut Vec<T>,
        additional: usize,
    ) -> Result<(), AllocationError> {
        let required = vector
            .len()
            .checked_add(additional)
            .ok_or(AllocationError::Capacity)?;
        if required <= vector.capacity() {
            return Ok(());
        }
        let capacity = required
            .max(vector.capacity().checked_mul(2).unwrap_or(required))
            .max(4);
        // Reject an impossible Layout before charging for a move that cannot run.
        VectorLayout::<T>::new(capacity)?;
        let old_bytes = vector_bytes(vector)?;
        if self.ledger.is_some() && old_bytes > self.retained_bytes(class) {
            return Err(BudgetError::InvalidRelease.into());
        }
        self.work(WorkKind::Render, count(vector.len())?)?;
        let mut next = self.vector(class, capacity)?;
        // append cannot allocate: next's admitted capacity >= original length.
        // It moves ownership, including arbitrary non-Copy Drop values.
        next.append(vector);
        let old = mem::replace(vector, next);
        drop(old);
        self.release(class, old_bytes)
    }

    pub(crate) fn push<T>(
        &mut self,
        class: AllocationClass,
        vector: &mut Vec<T>,
        value: T,
    ) -> Result<(), AllocationError> {
        self.work(WorkKind::Render, 1)?;
        self.reserve_vec(class, vector, 1)?;
        vector.push(value);
        Ok(())
    }

    pub(crate) fn extend_copy<T: Copy>(
        &mut self,
        class: AllocationClass,
        vector: &mut Vec<T>,
        values: &[T],
    ) -> Result<(), AllocationError> {
        self.work(WorkKind::Render, count(values.len())?)?;
        self.reserve_vec(class, vector, values.len())?;
        vector.extend_from_slice(values);
        Ok(())
    }

    pub(crate) fn string(
        &mut self,
        class: AllocationClass,
        value: &str,
    ) -> Result<String, AllocationError> {
        let bytes = self.copy_slice(class, value.as_bytes())?;
        // SAFETY: bytes were copied exactly from a valid UTF-8 str.
        Ok(unsafe { String::from_utf8_unchecked(bytes) })
    }

    pub(crate) fn reserve_string(
        &mut self,
        class: AllocationClass,
        value: &mut String,
        additional: usize,
    ) -> Result<(), AllocationError> {
        let mut bytes = mem::take(value).into_bytes();
        let result = self.reserve_vec(class, &mut bytes, additional);
        // SAFETY: reserve_vec never changes the initialized bytes, on either
        // success or failure. The caller's String is restored before returning.
        *value = unsafe { String::from_utf8_unchecked(bytes) };
        result
    }

    pub(crate) fn push_str(
        &mut self,
        class: AllocationClass,
        target: &mut String,
        value: &str,
    ) -> Result<(), AllocationError> {
        self.work(WorkKind::Render, count(value.len())?)?;
        self.reserve_string(class, target, value.len())?;
        target.push_str(value);
        Ok(())
    }

    pub(crate) fn push_char(
        &mut self,
        class: AllocationClass,
        target: &mut String,
        value: char,
    ) -> Result<(), AllocationError> {
        let mut bytes = [0; 4];
        self.push_str(class, target, value.encode_utf8(&mut bytes))
    }

    pub(crate) fn string_value(
        &mut self,
        class: AllocationClass,
        value: &StringValue,
    ) -> Result<StringValue, AllocationError> {
        if let Some(value) = value.as_unicode() {
            self.string(class, value).map(Into::into)
        } else {
            let units = self.copy_slice(
                class,
                value
                    .as_unpaired_utf16()
                    .expect("decoded string representation"),
            )?;
            Ok(StringValue::from_preserved_unpaired_utf16(units))
        }
    }

    /// Formatting's sink admits every output chunk before growth/copy. Display
    /// implementations themselves must not hide compiler-owned allocations.
    pub(crate) fn format(
        &mut self,
        class: AllocationClass,
        arguments: fmt::Arguments<'_>,
    ) -> Result<String, AllocationError> {
        let mut text = String::new();
        if let Err(error) = self.write_fmt(class, &mut text, arguments) {
            let bytes = count(text.capacity())?;
            drop(text);
            self.release(class, bytes)?;
            return Err(error);
        }
        Ok(text)
    }

    /// Append formatted chunks to an existing admitted buffer without a
    /// temporary String. On failure the valid partial output stays owned by
    /// the caller's scope, exactly as for push_str.
    pub(crate) fn write_fmt(
        &mut self,
        class: AllocationClass,
        text: &mut String,
        arguments: fmt::Arguments<'_>,
    ) -> Result<(), AllocationError> {
        let mut writer = AdmittedWriter {
            budget: self,
            class,
            text,
            error: None,
        };
        let result = writer.write_fmt(arguments);
        let error = writer.error;
        if result.is_err() {
            return Err(error.unwrap_or(AllocationError::AllocationFailed));
        }
        Ok(())
    }
}

impl Drop for AllocationBudget<'_> {
    fn drop(&mut self) {
        if let Some(ledger) = self.ledger.as_deref_mut() {
            // Retain validates total arithmetic before mutating either class.
            let bytes = self.live[0] + self.live[1];
            let result = ledger.release(self.domain, bytes);
            debug_assert!(
                result.is_ok(),
                "allocation owner releases only its own live charge"
            );
        }
    }
}

struct AdmittedWriter<'budget, 'ledger> {
    budget: &'budget mut AllocationBudget<'ledger>,
    class: AllocationClass,
    text: &'budget mut String,
    error: Option<AllocationError>,
}
impl Write for AdmittedWriter<'_, '_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        match self.budget.push_str(self.class, self.text, value) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.error = Some(error);
                Err(fmt::Error)
            }
        }
    }
}

#[must_use = "the original Compilation must retain or explicitly discard this admitted charge"]
#[derive(Debug)]
pub(crate) struct RetainedCharge<Owner> {
    owner: Owner,
    domain: WorkDomain,
    bytes: u64,
}
impl<Owner> RetainedCharge<Owner> {
    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }
    pub(crate) fn domain(&self) -> WorkDomain {
        self.domain
    }
}
impl<Owner: Eq> RetainedCharge<Owner> {
    pub(crate) fn belongs_to(&self, owner: &Owner) -> bool {
        &self.owner == owner
    }
    /// Transfer the existing reservation into an owner's finer-grained charge
    /// partition. This consumes authority without releasing or charging bytes.
    pub(crate) fn into_parts(
        self,
        owner: &Owner,
    ) -> Result<(WorkDomain, u64), (Self, AllocationError)> {
        if !self.belongs_to(owner) {
            return Err((self, AllocationError::WrongOwner));
        }
        Ok((self.domain, self.bytes))
    }
    /// The artifact owner first drops its actual buffers. Wrong-owner/invalid
    /// release returns the still-live token, so failure cannot lose admission.
    pub(crate) fn discard(
        self,
        owner: &Owner,
        ledger: &mut BudgetLedger,
    ) -> Result<(), (Self, AllocationError)> {
        if !self.belongs_to(owner) {
            return Err((self, AllocationError::WrongOwner));
        }
        match ledger.release(self.domain, self.bytes) {
            Ok(()) => Ok(()),
            Err(error) => Err((self, error.into())),
        }
    }
}

fn count(value: usize) -> Result<u64, AllocationError> {
    u64::try_from(value).map_err(|_| AllocationError::Capacity)
}
fn vector_bytes<T>(value: &Vec<T>) -> Result<u64, AllocationError> {
    count(
        value
            .capacity()
            .checked_mul(size_of::<T>())
            .ok_or(AllocationError::Capacity)?,
    )
}
/// Checked backing layout for allocation owners with their own admission
/// scope. Compute once, admit `bytes()`, then allocate the exact capacity.
/// This contains no ledger, reservation, initialized values or second buffer.
pub(crate) struct VectorLayout<T> {
    capacity: usize,
    layout: Option<Layout>,
    marker: std::marker::PhantomData<T>,
}
impl<T> VectorLayout<T> {
    pub(crate) fn new(capacity: usize) -> Result<Self, AllocationError> {
        let layout = if size_of::<T>() == 0 || capacity == 0 {
            None
        } else {
            Some(Layout::array::<T>(capacity).map_err(|_| AllocationError::Capacity)?)
        };
        Ok(Self {
            capacity,
            layout,
            marker: std::marker::PhantomData,
        })
    }
    pub(crate) fn bytes(&self) -> u64 {
        self.layout.map_or(0, |layout| layout.size() as u64)
    }
    pub(crate) fn allocate(self) -> Result<Vec<T>, AllocationError> {
        let Some(layout) = self.layout else {
            return Ok(Vec::new());
        };
        // SAFETY: Layout::array checked size/alignment and excludes zero size.
        // Global allocation returns suitably aligned storage or null.
        let pointer = unsafe { alloc(layout) }.cast::<T>();
        if pointer.is_null() {
            return Err(AllocationError::AllocationFailed);
        }
        // SAFETY: This is exactly T's layout and capacity*T's size. Length is
        // zero; Vec takes the global allocation with the same drop layout.
        Ok(unsafe { Vec::from_raw_parts(pointer, 0, self.capacity) })
    }
}

#[cfg(test)]
#[path = "output_budget_boxed_tests.rs"]
mod boxed_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{BudgetPlan, ResourceLimits};
    use std::cell::Cell;
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    fn new_ledger(memory: u64, work: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 100_000,
                optional_work: work,
                baseline_retained_bytes: 32,
                retained_bytes: memory,
            },
        )
        .unwrap()
    }
    #[repr(align(256))]
    struct AlignedDrop(Rc<Cell<usize>>, usize);
    impl Drop for AlignedDrop {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn exact_layout_growth_moves_overaligned_noncopy_values_once_and_counts_overlap() {
        let mut ledger = new_ledger(100_000, 100_000);
        let drops = Rc::new(Cell::new(0));
        let old_capacity;
        let new_capacity;
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut values = budget.vector(AllocationClass::Retained, 2).unwrap();
            old_capacity = values.capacity();
            assert_eq!(old_capacity, 2);
            for value in 0..3 {
                budget
                    .push(
                        AllocationClass::Retained,
                        &mut values,
                        AlignedDrop(drops.clone(), value),
                    )
                    .unwrap();
            }
            new_capacity = values.capacity();
            assert_eq!(new_capacity, 4);
            assert_eq!(values.as_ptr() as usize % 256, 0);
            assert_eq!(
                values.iter().map(|value| value.1).collect::<Vec<_>>(),
                [0, 1, 2]
            );
            assert_eq!(drops.get(), 0, "moving does not drop elements");
            assert_eq!(
                budget.retained_bytes(AllocationClass::Retained),
                (new_capacity * size_of::<AlignedDrop>()) as u64
            );
            drop(values);
            assert_eq!(drops.get(), 3);
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.work_used(WorkDomain::Baseline), 0);
        assert_eq!(
            ledger.peak_retained_bytes(),
            ((old_capacity + new_capacity) * size_of::<AlignedDrop>()) as u64
        );
    }

    #[test]
    fn denied_growth_keeps_original_noncopy_storage_and_does_not_steal_baseline_reserve() {
        let mut ledger = new_ledger(32 + 5 * size_of::<AlignedDrop>() as u64, 100_000);
        let drops = Rc::new(Cell::new(0));
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut values = budget.vector(AllocationClass::Retained, 2).unwrap();
            for value in 0..2 {
                budget
                    .push(
                        AllocationClass::Retained,
                        &mut values,
                        AlignedDrop(drops.clone(), value),
                    )
                    .unwrap();
            }
            let pointer = values.as_ptr();
            let before = budget.retained_bytes(AllocationClass::Retained);
            assert_eq!(
                budget.reserve_vec(AllocationClass::Retained, &mut values, 1),
                Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                    WorkDomain::Optional
                )))
            );
            assert_eq!(values.as_ptr(), pointer);
            assert_eq!(values.capacity(), 2);
            assert_eq!(values.len(), 2);
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), before);
            assert_eq!(drops.get(), 0);
            drop(values);
        }
        assert_eq!(drops.get(), 2);
        assert_eq!(ledger.retained_bytes(), 0);
        assert_eq!(ledger.work_used(WorkDomain::Baseline), 0);
        ledger.retain(WorkDomain::Baseline, 32).unwrap();
        ledger.release(WorkDomain::Baseline, 32).unwrap();
    }

    #[test]
    fn nested_phase_promotes_only_live_result_and_drops_scratch_before_transfer() {
        let mut ledger = new_ledger(100_000, 100_000);
        {
            let mut session = BudgetSession::new(Some((&mut ledger, WorkDomain::Optional)));
            let retained;
            {
                let mut phase = session.scope();
                let scratch = phase.filled(AllocationClass::Scratch, 11, 0u32).unwrap();
                retained = phase
                    .string(AllocationClass::Scratch, "retained result")
                    .unwrap();
                let bytes = retained.capacity() as u64;
                phase.promote(bytes).unwrap();
                assert_eq!(phase.retained_bytes(AllocationClass::Retained), bytes);
                assert_eq!(phase.retained_bytes(AllocationClass::Scratch), 44);
                drop(scratch);
                phase.finish_retained().unwrap();
            }
            assert_eq!(session.retained_bytes(AllocationClass::Scratch), 0);
            assert_eq!(
                session.retained_bytes(AllocationClass::Retained),
                retained.capacity() as u64
            );
            drop(retained);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn callback_error_and_unwind_release_only_their_scope_and_drop_values_once() {
        let mut ledger = new_ledger(100_000, 100_000);
        let drops = Rc::new(Cell::new(0));
        {
            let mut session = BudgetSession::new(Some((&mut ledger, WorkDomain::Optional)));
            let survivor = session
                .string(AllocationClass::Retained, "already retained")
                .unwrap();
            let before = session.retained_bytes(AllocationClass::Retained);
            let failed = (|| -> Result<(), &'static str> {
                let mut phase = session.scope();
                let _scratch = phase.filled(AllocationClass::Scratch, 19, 0u64).unwrap();
                let mut target = phase.vector(AllocationClass::Retained, 1).unwrap();
                phase
                    .push(
                        AllocationClass::Retained,
                        &mut target,
                        AlignedDrop(drops.clone(), 0),
                    )
                    .unwrap();
                Err("callback error")
            })();
            assert_eq!(failed, Err("callback error"));
            assert_eq!(drops.get(), 1);
            assert_eq!(session.retained_bytes(AllocationClass::Retained), before);
            let panic = catch_unwind(AssertUnwindSafe(|| {
                let mut phase = session.scope();
                let mut target = phase.vector(AllocationClass::Retained, 1).unwrap();
                phase
                    .push(
                        AllocationClass::Retained,
                        &mut target,
                        AlignedDrop(drops.clone(), 0),
                    )
                    .unwrap();
                let _scratch = phase.string(AllocationClass::Scratch, "scratch").unwrap();
                panic!("callback panic");
            }));
            assert!(panic.is_err());
            assert_eq!(drops.get(), 2);
            assert_eq!(session.retained_bytes(AllocationClass::Retained), before);
            drop(survivor);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn strings_preserve_unicode_and_lone_surrogates_with_exact_backing_charges() {
        let mut ledger = new_ledger(100_000, 100_000);
        let unicode = StringValue::from("é🙂");
        let unpaired = StringValue::from_utf16(vec![0xd800, 0x0061, 0xdc00]);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let first = budget
                .string_value(AllocationClass::Retained, &unicode)
                .unwrap();
            let second = budget
                .string_value(AllocationClass::Retained, &unpaired)
                .unwrap();
            assert_eq!(first, unicode);
            assert_eq!(second, unpaired);
            assert_eq!(second.as_unpaired_utf16(), unpaired.as_unpaired_utf16());
            assert_eq!(first.capacity_bytes(), unicode.storage_bytes());
            assert_eq!(second.capacity_bytes(), unpaired.storage_bytes());
            assert_eq!(
                budget.retained_bytes(AllocationClass::Retained),
                (first.capacity_bytes() + second.capacity_bytes()) as u64
            );
            let spelling = budget
                .format(
                    AllocationClass::Scratch,
                    format_args!("value_{}_{}", 123, 456),
                )
                .unwrap();
            assert_eq!(spelling, "value_123_456");
            drop((first, second, spelling));
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn text_growth_and_copy_work_fail_before_mutating_existing_text() {
        let mut ledger = new_ledger(32 + 11, 100_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut text = budget.string(AllocationClass::Retained, "four").unwrap();
            let pointer = text.as_ptr();
            assert_eq!(
                budget.push_str(AllocationClass::Retained, &mut text, "!"),
                Err(AllocationError::Budget(BudgetError::MemoryExhausted(
                    WorkDomain::Optional
                )))
            );
            assert_eq!(text, "four");
            assert_eq!(text.as_ptr(), pointer);
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), 4);
            drop(text);
        }
        assert_eq!(ledger.retained_bytes(), 0);
        let mut ledger = new_ledger(100_000, 5);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let mut text = budget.string(AllocationClass::Retained, "four").unwrap();
            assert_eq!(
                budget.push_str(AllocationClass::Retained, &mut text, "!"),
                Err(AllocationError::Budget(BudgetError::WorkExhausted(
                    WorkDomain::Optional
                )))
            );
            assert_eq!(text, "four");
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), 4);
            drop(text);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn exact_capacity_checks_zst_and_temporary_inspection_wrappers_use_same_allocator() {
        let mut ledger = new_ledger(100_000, 100_000);
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            assert_eq!(
                budget
                    .vector::<u64>(AllocationClass::Retained, usize::MAX)
                    .unwrap_err(),
                AllocationError::Capacity
            );
            let values = budget.filled(AllocationClass::Retained, 200, ()).unwrap();
            assert_eq!(values.len(), 200);
            assert_eq!(budget.retained_bytes(AllocationClass::Retained), 0);
            drop(values);
        }
        assert_eq!(ledger.retained_bytes(), 0);
        let drops = Rc::new(Cell::new(0));
        let mut values = Vec::new();
        let mut text = String::new();
        for index in 0..9 {
            let mut inspection = AllocationBudget::new(None);
            inspection
                .push(
                    AllocationClass::Retained,
                    &mut values,
                    AlignedDrop(drops.clone(), index),
                )
                .unwrap();
            inspection
                .push_str(AllocationClass::Retained, &mut text, "x")
                .unwrap();
            assert_eq!(inspection.retained_bytes(AllocationClass::Retained), 0);
            assert_eq!(
                inspection.detach_retained(1u64, 0).unwrap_err(),
                AllocationError::Unaccounted
            );
        }
        assert_eq!(values.len(), 9);
        assert_eq!(text, "xxxxxxxxx");
        drop(values);
        assert_eq!(drops.get(), 9);
    }

    #[test]
    fn retained_artifact_transfer_is_nonduplicating_owner_checked_and_move_safe() {
        let mut first = new_ledger(100_000, 100_000);
        let mut second = new_ledger(100_000, 100_000);
        let artifact;
        let charge;
        {
            let mut session = BudgetSession::new(Some((&mut first, WorkDomain::Optional)));
            artifact = session
                .string(AllocationClass::Retained, "final artifact")
                .unwrap();
            let bytes = artifact.capacity() as u64;
            assert_eq!(
                session.detach_retained(11u64, bytes + 1).unwrap_err(),
                AllocationError::Budget(BudgetError::InvalidRelease)
            );
            assert_eq!(session.retained_bytes(AllocationClass::Retained), bytes);
            charge = session.detach_retained(11u64, bytes).unwrap();
            assert_eq!(session.retained_bytes(AllocationClass::Retained), 0);
            assert_eq!(charge.domain(), WorkDomain::Optional);
        }
        assert_eq!(first.retained_bytes(), artifact.capacity() as u64);
        let bytes = charge.bytes();
        let (charge, error) = charge.discard(&22u64, &mut second).unwrap_err();
        assert_eq!(error, AllocationError::WrongOwner);
        assert_eq!(first.retained_bytes(), bytes);
        assert_eq!(second.retained_bytes(), 0);
        // Moving the owner/ledger does not invalidate its existing store key.
        let mut moved = first;
        drop(artifact);
        charge.discard(&11u64, &mut moved).unwrap();
        assert_eq!(moved.retained_bytes(), 0);
    }
    #[test]
    fn partition_transfer_preserves_reservation_and_refused_token() {
        let mut ledger = new_ledger(100_000, 100_000);
        let (value, charge) = {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            let value = budget
                .string(AllocationClass::Retained, "owned payload")
                .unwrap();
            let charge = budget.detach_retained(7, value.capacity() as u64).unwrap();
            (value, charge)
        };
        let retained = ledger.retained_bytes();
        let (charge, error) = charge.into_parts(&8).unwrap_err();
        assert_eq!(error, AllocationError::WrongOwner);
        assert_eq!(ledger.retained_bytes(), retained);
        let (domain, bytes) = charge.into_parts(&7).unwrap();
        assert_eq!(domain, WorkDomain::Optional);
        assert_eq!(bytes, retained);
        assert_eq!(ledger.retained_bytes(), retained);
        drop(value);
        ledger.release(domain, bytes).unwrap();
        assert_eq!(ledger.retained_bytes(), 0);
    }

    #[test]
    fn shared_ledger_callback_keeps_other_owners_separate_from_target_classes() {
        let mut ledger = new_ledger(100_000, 100_000);
        {
            let mut session = BudgetSession::new(Some((&mut ledger, WorkDomain::Optional)));
            let target = session.string(AllocationClass::Retained, "target").unwrap();
            let target_bytes = target.capacity() as u64;
            let separately_owned = session.with_ledger(|ledger| {
                let (ledger, domain) = ledger.unwrap();
                assert_eq!(domain, WorkDomain::Optional);
                ledger.retain(WorkDomain::Baseline, 7).unwrap();
                vec![0u8; 7]
            });
            assert_eq!(
                session.retained_bytes(AllocationClass::Retained),
                target_bytes
            );
            assert_eq!(
                session.with_ledger(|ledger| ledger.unwrap().0.retained_bytes()),
                target_bytes + 7
            );
            drop(separately_owned);
            session
                .with_ledger(|ledger| ledger.unwrap().0.release(WorkDomain::Baseline, 7))
                .unwrap();
            assert_eq!(
                session.retained_bytes(AllocationClass::Retained),
                target_bytes
            );
            drop(target);
        }
        assert_eq!(ledger.retained_bytes(), 0);
    }
}
