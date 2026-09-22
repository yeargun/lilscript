//! Compact handles for the semantic program's contiguous tables.
//!
//! Handles are positions, not addresses or source spellings. Values, operations,
//! regions, places and allocation sites are local to a unit. Cross-unit clients
//! must carry the owning unit and consult a matching published revision. Cells,
//! types and strings belong to the program's tables.

use std::num::{NonZeroU32, NonZeroU64};
use std::sync::atomic::{AtomicU64, Ordering};

macro_rules! semantic_handles {
    ($($name:ident),+ $(,)?) => {$(
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(NonZeroU32);

        impl $name {
            /// Returns `None` when the arena position exceeds handle capacity.
            pub const fn from_index(index: usize) -> Option<Self> {
                if index >= u32::MAX as usize {
                    None
                } else {
                    match NonZeroU32::new(index as u32 + 1) {
                        Some(encoded) => Some(Self(encoded)),
                        None => None,
                    }
                }
            }

            pub const fn index(self) -> usize {
                self.0.get() as usize - 1
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.debug_tuple(stringify!($name)).field(&self.index()).finish()
            }
        }
    )+};
}

semantic_handles!(
    ModuleId,
    UnitId,
    OpId,
    ValueId,
    CellId,
    RegionId,
    PlaceId,
    CallId,
    CallInstantiationId,
    AllocationId,
    TypeId,
    StringId,
);

/// Identity of an immutable semantic unit state within this process.
///
/// Sibling branches do not copy a revision counter and accidentally issue the
/// same revision for different bodies. This identity is for equality checks;
/// it is not a deterministic scheduling key or a persistent content digest.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RevisionId(NonZeroU64);

impl RevisionId {
    pub(super) fn fresh() -> Self {
        static NEXT_REVISION: AtomicU64 = AtomicU64::new(1);
        let next = NEXT_REVISION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .expect("semantic unit revision capacity exceeded");
        Self(NonZeroU64::new(next).expect("semantic revisions start at one"))
    }
}

impl std::fmt::Debug for RevisionId {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_tuple("RevisionId")
            .field(&self.0.get())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::mem::size_of;

    #[test]
    fn local_handles_round_trip_without_truncating_large_positions() {
        for index in [0, 1, 127, u32::MAX as usize - 1] {
            assert_eq!(OpId::from_index(index).unwrap().index(), index);
        }
        assert_eq!(OpId::from_index(u32::MAX as usize), None);
        assert_eq!(OpId::from_index(usize::MAX), None);
    }

    #[test]
    fn optional_handles_do_not_double_per_operation_storage() {
        assert_eq!(size_of::<OpId>(), size_of::<u32>());
        assert_eq!(size_of::<Option<OpId>>(), size_of::<u32>());
        assert_eq!(size_of::<Option<ValueId>>(), size_of::<u32>());
        assert_eq!(size_of::<Option<CellId>>(), size_of::<u32>());
        assert_eq!(size_of::<Option<RevisionId>>(), size_of::<u64>());
    }

    #[test]
    fn independently_issued_revisions_never_alias() {
        let mut revisions = std::collections::HashSet::new();
        for _ in 0..64 {
            assert!(revisions.insert(RevisionId::fresh()));
        }
    }
}
