//! Exclusive working units and immutable retained checkpoints.
//!
//! Mutation is a private building operation. The edit layer validates a builder
//! before publishing its frozen result; retaining the old checkpoint keeps a
//! failed or rejected edit from changing the visible program. These wrappers do
//! not introduce a second edit protocol or silently validate semantic structure.

use super::ids::{RevisionId, UnitId};
use super::UnitData;
use std::sync::Arc;

/// An exclusively owned unit under construction or checked batched editing.
#[derive(Debug)]
pub struct WorkingUnit {
    id: UnitId,
    data: UnitData,
    inherited_revision: Option<RevisionId>,
}

impl WorkingUnit {
    pub fn new(id: UnitId, data: UnitData) -> Self {
        Self {
            id,
            data,
            inherited_revision: None,
        }
    }

    pub fn id(&self) -> UnitId {
        self.id
    }

    pub fn data(&self) -> &UnitData {
        &self.data
    }

    /// Invalidates any inherited revision before granting mutation.
    ///
    /// Repeated local rewrites can share this borrow. No new revision or shared
    /// allocation is issued for each rewrite; publication stamps the batch once.
    pub fn get_mut(&mut self) -> &mut UnitData {
        self.inherited_revision = None;
        &mut self.data
    }

    /// Seals storage; callers must validate edited semantics before publication.
    pub fn freeze(self) -> FrozenUnit {
        FrozenUnit {
            id: self.id,
            revision: self.inherited_revision.unwrap_or_else(RevisionId::fresh),
            data: Arc::new(self.data),
        }
    }
}

/// An immutable unit revision retained by a program or candidate checkpoint.
#[derive(Clone, Debug)]
pub struct FrozenUnit {
    id: UnitId,
    revision: RevisionId,
    data: Arc<UnitData>,
}

impl FrozenUnit {
    pub fn id(&self) -> UnitId {
        self.id
    }

    pub fn revision(&self) -> RevisionId {
        self.revision
    }

    pub fn data(&self) -> &UnitData {
        &self.data
    }

    /// Publication adopts a baseline by move; retained external owners would
    /// otherwise outlive the compilation's accounting authority.
    pub(super) fn allocation_is_unique(&self) -> bool {
        Arc::strong_count(&self.data) == 1
    }

    /// Only the admitted publication transaction may borrow a frozen payload
    /// mutably. It keeps the allocation exclusive and restores the previous
    /// revision and payload on refusal before making the checkpoint visible.
    pub(super) fn unique_edit(&mut self) -> Option<(&mut UnitData, &mut RevisionId)> {
        Some((Arc::get_mut(&mut self.data)?, &mut self.revision))
    }

    /// Consumes this handle and drops the payload before its owner releases the
    /// original reservation. Revision equality is not allocation identity.
    pub(super) fn release_allocation(self) -> bool {
        match Arc::try_unwrap(self.data) {
            Ok(data) => {
                drop(data);
                true
            }
            Err(_) => false,
        }
    }

    pub(super) fn allocation_bytes(&self) -> Option<u64> {
        unit_allocation_bytes(&self.data)
    }

    /// Moves unshared data into a builder, or copies a still-shared unit once.
    pub fn into_working(self) -> WorkingUnit {
        let data = match Arc::try_unwrap(self.data) {
            Ok(data) => data,
            Err(shared) => shared.as_ref().clone(),
        };
        WorkingUnit {
            id: self.id,
            data,
            inherited_revision: Some(self.revision),
        }
    }
}

/// Existing arena capacities, including nested owning buffers. This does not
/// include the FrozenUnit handle (owned by Program's vector) or allocator/RSS
/// overhead. The caller charges traversal work before using this size hook.
pub(super) fn unit_allocation_bytes(data: &UnitData) -> Option<u64> {
    use std::mem::size_of;
    fn capacity<T>(values: &Vec<T>) -> Option<u64> {
        (values.capacity() as u64).checked_mul(size_of::<T>() as u64)
    }
    let mut bytes = (size_of::<UnitData>() + 2 * size_of::<usize>()) as u64;
    for size in [
        capacity(&data.parameters)?,
        capacity(&data.captures)?,
        capacity(&data.operations)?,
        capacity(&data.operands)?,
        capacity(&data.values)?,
        capacity(&data.regions)?,
        capacity(&data.places)?,
        capacity(&data.calls)?,
        capacity(&data.call_instantiations)?,
        capacity(&data.call_arguments)?,
    ] {
        bytes = bytes.checked_add(size)?;
    }
    for instance in &data.call_instantiations {
        bytes = bytes.checked_add(capacity(&instance.arguments)?)?;
    }
    for region in &data.regions {
        bytes = bytes.checked_add(capacity(&region.operations)?)?;
    }
    for operation in &data.operations {
        if let super::OperationKind::Allocate {
            kind: super::AllocationKind::Record(keys) | super::AllocationKind::Object(keys),
            ..
        } = &operation.kind
        {
            bytes = bytes.checked_add(capacity(keys)?)?;
        }
        if let super::OperationKind::Allocate {
            kind: super::AllocationKind::SpreadArray(spread),
            ..
        } = &operation.kind
        {
            bytes = bytes.checked_add(capacity(spread)?)?;
        }
    }
    Some(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::semantic_program::ids::{CellId, ValueId};
    use crate::semantic_program::UnitKind;

    fn checkpoint() -> FrozenUnit {
        let mut data = UnitData::empty(UnitKind::Function);
        data.parameters.push(CellId::from_index(0).unwrap());
        WorkingUnit::new(UnitId::from_index(0).unwrap(), data).freeze()
    }

    #[test]
    fn unshared_thaw_moves_existing_arena_allocations() {
        let original = checkpoint();
        let parameter_storage = original.data().parameters.as_ptr();
        let revision = original.revision();
        let working = original.into_working();
        assert_eq!(working.data().parameters.as_ptr(), parameter_storage);
        let unchanged = working.freeze();
        assert_eq!(unchanged.revision(), revision);
        assert_eq!(unchanged.data().parameters.as_ptr(), parameter_storage);
    }

    #[test]
    fn sibling_edits_preserve_identity_without_aliasing_revisions_or_data() {
        let original = checkpoint();
        let mut left = original.clone().into_working();
        let mut right = original.clone().into_working();
        left.get_mut().captures.push(CellId::from_index(1).unwrap());
        right
            .get_mut()
            .captures
            .push(CellId::from_index(2).unwrap());
        let left = left.freeze();
        let right = right.freeze();

        assert_eq!(left.id(), original.id());
        assert_eq!(right.id(), original.id());
        assert_ne!(left.revision(), original.revision());
        assert_ne!(right.revision(), original.revision());
        assert_ne!(left.revision(), right.revision());
        assert!(original.data().captures.is_empty());
        assert_eq!(left.data().captures, [CellId::from_index(1).unwrap()]);
        assert_eq!(right.data().captures, [CellId::from_index(2).unwrap()]);
    }

    #[test]
    fn abandoned_private_edit_cannot_change_retained_checkpoint() {
        let original = checkpoint();
        let revision = original.revision();
        {
            let mut rejected = original.clone().into_working();
            rejected.get_mut().parameters.clear();
            rejected
                .get_mut()
                .operands
                .push(ValueId::from_index(7).unwrap());
        }
        assert_eq!(original.revision(), revision);
        assert_eq!(original.data().parameters, [CellId::from_index(0).unwrap()]);
        assert!(original.data().operands.is_empty());
    }

    #[test]
    fn requesting_mutation_conservatively_changes_revision_even_without_a_write() {
        let original = checkpoint();
        let revision = original.revision();
        let mut working = original.into_working();
        let _ = working.get_mut();
        assert_ne!(working.freeze().revision(), revision);
    }

    #[test]
    fn shared_checkpoints_retain_storage_and_revision_without_payload_copy() {
        let original = checkpoint();
        let sibling = original.clone();
        assert!(Arc::ptr_eq(&original.data, &sibling.data));
        assert_eq!(original.revision(), sibling.revision());
        let working = sibling.into_working();
        assert_ne!(
            working.data().parameters.as_ptr(),
            original.data().parameters.as_ptr()
        );
        assert_eq!(working.freeze().revision(), original.revision());
    }
}
