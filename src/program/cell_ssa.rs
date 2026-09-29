//! Where each load of a unit's own cell reads from (plan M5.5: the cell-SSA
//! view): the definitions that may reach it, solved over the unit's regions
//! (`dataflow`).
//!
//! A cell is **tracked** in its owner unit when only the owner defines it:
//! every initialization, declaration, write and catch binding of the cell
//! is in the owner, and no call receives it by reference. Then no call and
//! no nested function can change it between two of the owner's operations,
//! so the owner's regions alone decide what each load reads.
//!
//! A **definition** is the unit's entry (a parameter's value) or the
//! operation that last gave the cell a value on some path: its
//! initialization, a declaration (R3: a store precedes every read), a store
//! to the cell or to a field of it (a partial definition: the load then
//! reads the value that store changed), a `for…in` or `for…of` binding, or a
//! catch binding. A whole-cell load's answer is every definition reaching
//! it; an answer of one initialization or store is the flow-sensitive fact
//! reaching-definition forwarding (M7.4) and the value ranges (M6.4b) read.
//!
//! Prior art: Binaryen's `LocalGraph` (the sets reaching each get over
//! structured control) and Closure's `MaybeReachingVariableUse`; LLVM's
//! mem2reg promotes allocas whose address does not escape, the same test as
//! "no call receives it by reference".
// Read by reaching-definition forwarding (M7.4) and the value ranges
// (M6.4b), both step 10; until then only its tests read it.
#![allow(dead_code)]

use super::dataflow::{self, Forward, Stop};
use super::uses::{CellUse, CellUseSite, UseIndex};
use super::*;

/// A definition reaching a load.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Definition {
    /// A parameter's value on entry.
    Entry,
    /// The operation that gave the cell its value.
    Operation(OpId),
}

/// The view for one unit.
pub(super) struct CellSsa {
    /// Per operation: for a whole-cell load of a tracked cell, the
    /// definitions reaching it, sorted.
    reaching: Vec<Option<Vec<Definition>>>,
}

impl CellSsa {
    /// The definitions reaching `load`, when it loads a tracked cell whole.
    pub(super) fn reaching(&self, load: OpId) -> Option<&[Definition]> {
        self.reaching.get(load.index())?.as_deref()
    }

    /// Solve the view for `unit` of `program`.
    pub(super) fn build<E>(
        program: &Program<'_>,
        uses: &UseIndex,
        unit: UnitId,
        mut work: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, Stop<E>> {
        let data = program.units[unit.index()].data();
        let mut ordinals: Vec<Option<usize>> = vec![None; program.cells.len()];
        let mut tracked = Vec::new();
        for (index, cell) in program.cells.iter().enumerate() {
            work(1).map_err(Stop::Budget)?;
            let id = CellId::from_index(index).expect("a cell index");
            if cell.owner != unit || program.is_reference_parameter(id) {
                continue;
            }
            let Some(users) = uses.cell(id) else {
                continue;
            };
            let owned = users.sites().iter().all(|site| match *site {
                CellUseSite::Unit { unit: user, usage } => match usage {
                    CellUse::Reference { .. } => false,
                    CellUse::Initialize(_)
                    | CellUse::Declare(_)
                    | CellUse::Write { .. }
                    | CellUse::CatchBinding { .. } => user == unit,
                    _ => true,
                },
                // An export reads the value; a host import defines it
                // outside the program.
                _ => false,
            });
            if owned {
                ordinals[index] = Some(tracked.len());
                tracked.push(id);
            }
        }
        let analysis = Reaching {
            ordinals: &ordinals,
            tracked: tracked.len(),
        };
        let mut entry = analysis.unreachable();
        for &parameter in &data.parameters {
            if let Some(ordinal) = ordinals[parameter.index()] {
                entry[ordinal] = vec![Definition::Entry];
            }
        }
        let solution = dataflow::solve(data, &analysis, entry, &mut work)?;
        let mut reaching = vec![None; data.operations.len()];
        for (index, operation) in data.operations.iter().enumerate() {
            work(1).map_err(Stop::Budget)?;
            let OperationKind::Load(place) = operation.kind else {
                continue;
            };
            let Place::Cell(cell) = data.places[place.index()] else {
                continue;
            };
            if let Some(ordinal) = ordinals[cell.index()] {
                let op = OpId::from_index(index).expect("an operation index");
                reaching[index] = Some(solution.before(op)[ordinal].clone());
            }
        }
        Ok(Self { reaching })
    }
}

/// Reaching definitions of the tracked cells: a sorted set per cell, joined
/// by union.
struct Reaching<'a> {
    ordinals: &'a [Option<usize>],
    tracked: usize,
}

impl Reaching<'_> {
    fn define(&self, cell: CellId, operation: OpId, state: &mut [Vec<Definition>]) {
        if let Some(ordinal) = self.ordinals[cell.index()] {
            state[ordinal] = vec![Definition::Operation(operation)];
        }
    }
}

/// The cell a place is rooted at: the cell itself, or through fields.
fn root_cell(unit: &UnitData, mut place: PlaceId) -> Option<CellId> {
    for _ in 0..=unit.places.len() {
        match unit.places[place.index()] {
            Place::Cell(cell) => return Some(cell),
            Place::Field { base, .. } => place = base,
            _ => return None,
        }
    }
    None
}

impl Forward for Reaching<'_> {
    type State = Vec<Vec<Definition>>;

    fn unreachable(&self) -> Self::State {
        vec![Vec::new(); self.tracked]
    }

    fn join(&self, into: &mut Self::State, from: &Self::State) {
        for (into, from) in into.iter_mut().zip(from) {
            if from.is_empty() || into == from {
                continue;
            }
            let mut merged = Vec::with_capacity(into.len() + from.len());
            let (mut left, mut right) = (0, 0);
            while left < into.len() || right < from.len() {
                match (into.get(left), from.get(right)) {
                    (Some(a), Some(b)) if a == b => {
                        merged.push(*a);
                        left += 1;
                        right += 1;
                    }
                    (Some(a), Some(b)) if a < b => {
                        merged.push(*a);
                        left += 1;
                    }
                    (Some(_), Some(b)) => {
                        merged.push(*b);
                        right += 1;
                    }
                    (Some(a), None) => {
                        merged.push(*a);
                        left += 1;
                    }
                    (None, Some(b)) => {
                        merged.push(*b);
                        right += 1;
                    }
                    (None, None) => unreachable!("the loop condition"),
                }
            }
            *into = merged;
        }
    }

    fn transfer(&self, unit: &UnitData, operation: OpId, state: &mut Self::State) {
        match unit.operations[operation.index()].kind {
            OperationKind::Initialize(cell)
            | OperationKind::Declare(cell)
            | OperationKind::ForIn { key: cell, .. }
            | OperationKind::ForOf { item: cell, .. } => self.define(cell, operation, state),
            OperationKind::Try {
                catch: Some((Some(cell), _)),
                ..
            } => self.define(cell, operation, state),
            OperationKind::Store(place) => {
                if let Some(cell) = root_cell(unit, place) {
                    self.define(cell, operation, state);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
#[path = "cell_ssa_tests.rs"]
mod tests;
