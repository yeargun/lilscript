//! Structured initialization order shared by semantic family analyses.
//! Buffers are admitted and allocated by the requesting analysis before entry;
//! each traversal charges that analysis through its existing bounded work owner.

use super::{OpId, UnitData};

#[derive(Debug)]
pub(super) struct StructuredDominance {
    parent_operation: Vec<Option<OpId>>,
    position: Vec<usize>,
}

impl StructuredDominance {
    pub(super) fn build<E>(
        unit: &UnitData,
        mut parent_operation: Vec<Option<OpId>>,
        mut position: Vec<usize>,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        assert_eq!(parent_operation.len(), unit.regions.len());
        assert_eq!(position.len(), unit.operations.len());
        for region in &unit.regions {
            for (ordinal, &id) in region.operations.iter().enumerate() {
                work(1)?;
                position[id.index()] = ordinal;
                for child in unit.operations[id.index()].kind.child_regions() {
                    work(1)?;
                    parent_operation[child.index()] = Some(id);
                }
            }
        }
        Ok(Self {
            parent_operation,
            position,
        })
    }

    /// Strictly after initialization in its region or in a later operation's
    /// descendant region. A sibling branch is never an initialization witness.
    pub(super) fn after<E>(
        &self,
        unit: &UnitData,
        initialize: OpId,
        mut operation: OpId,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<bool, E> {
        let region = unit.operations[initialize.index()].region;
        for _ in 0..=unit.regions.len() {
            work(1)?;
            let current_region = unit.operations[operation.index()].region;
            if current_region == region {
                return Ok(self.position[operation.index()] > self.position[initialize.index()]);
            }
            let Some(parent) = self.parent_operation[current_region.index()] else {
                return Ok(false);
            };
            operation = parent;
        }
        // Checked programs cannot cycle; malformed unverified input grants no
        // witness instead of starting an unbounded ownership traversal.
        Ok(false)
    }
}
