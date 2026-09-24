//! Prepared reverse-use replacements. The temporary owner contains only new
//! chunks and their destination IDs. A private destination stamp prevents a
//! prepared update from being installed in another or already advanced index;
//! it is never a semantic dependency or a scheduling identity.
use super::*;
use crate::output_budget::VectorLayout;

#[derive(Debug)]
struct UnitReplacement {
    unit: UnitId,
    uses: Arc<Shared<UnitUses>>,
    // Exact old/new occurrence comparison results; no independent facts.
    cells_changed: bool,
    creators_changed: bool,
}
type CellReplacement = (CellId, Option<RevisionId>, CellUsers);
type CreatorReplacement = (UnitId, CreatorUsers);

#[must_use = "install, fork, or discard prepared use replacements"]
#[derive(Debug)]
pub(in crate::program) struct PreparedUseUpdate {
    owner: RevisionId,
    tables_revision: RevisionId,
    unit_count: usize,
    cell_count: usize,
    fixed: bool,
    units: Vec<UnitReplacement>,
    cells: Vec<CellReplacement>,
    creators: Vec<CreatorReplacement>,
    metadata: Charge,
    receipt: UseIndexReceipt,
}
impl PreparedUseUpdate {
    pub(in crate::program) fn receipt(&self) -> UseIndexReceipt {
        self.receipt
    }
    pub(in crate::program) fn validation_work(&self) -> usize {
        1
    }
    pub(in crate::program) fn valid_for(&self, destination: &UseIndex) -> bool {
        self.owner == destination.update_owner
    }
    pub(in crate::program) fn cell_changes(
        &self,
    ) -> impl ExactSizeIterator<Item = (CellId, RevisionId, RevisionId)> + '_ {
        assert!(self.fixed, "cell changes require fixed tables");
        self.cells.iter().map(|(id, previous, users)| {
            (
                *id,
                previous.expect("existing fixed-table cell"),
                users.revision,
            )
        })
    }
    pub(in crate::program) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let Self {
            units,
            cells,
            creators,
            metadata,
            ..
        } = self;
        let mut outcome = Ok(());
        for replacement in units {
            outcome = outcome.and(release_shared(replacement.uses, ledger));
        }
        for (_, _, cell) in cells {
            if let Some(sites) = cell.sites {
                outcome = outcome.and(release_shared(sites, ledger));
            }
        }
        for (_, creators) in creators {
            if let Some(sites) = creators.sites {
                outcome = outcome.and(release_shared(sites, ledger));
            }
        }
        outcome.and(ledger.release(metadata.domain, metadata.bytes))
    }
}

/// Own partial chunks before any later fallible work, including unwinding.
struct Pending<'a> {
    update: Option<PreparedUseUpdate>,
    destination: Option<UseIndex>,
    ledger: &'a mut BudgetLedger,
}
impl Drop for Pending<'_> {
    fn drop(&mut self) {
        if let Some(index) = self.destination.take() {
            index.discard(self.ledger).expect("owned pending index");
        }
        if let Some(update) = self.update.take() {
            update
                .discard(self.ledger)
                .expect("owned pending use update");
        }
    }
}

fn vector<T>(count: usize) -> Result<Vec<T>, UseError> {
    VectorLayout::new(count)
        .and_then(VectorLayout::allocate)
        .map_err(|_| UseError::Capacity)
}
fn metadata_vector<T>(
    count: usize,
    charge: &mut Charge,
    scratch: &mut Scratch<'_>,
) -> Result<Vec<T>, UseError> {
    let layout = VectorLayout::<T>::new(count).map_err(|_| UseError::Capacity)?;
    let next = charge
        .bytes
        .checked_add(layout.bytes())
        .ok_or(UseError::Capacity)?;
    scratch.ledger.retain(charge.domain, layout.bytes())?;
    charge.bytes = next;
    layout.allocate().map_err(|_| UseError::Capacity)
}
fn scratch_vector<T>(count: usize, scratch: &mut Scratch<'_>) -> Result<Vec<T>, UseError> {
    let layout = VectorLayout::<T>::new(count).map_err(|_| UseError::Capacity)?;
    scratch.reserve(layout.bytes())?;
    layout.allocate().map_err(|_| UseError::Capacity)
}

impl UseIndex {
    /// Validate only the footprint produced by the fixed edit kernel. This
    /// does not create a capability and cannot authorize arbitrary callers to
    /// skip checking the rest of a Program.
    pub(in crate::program) fn validate_fixed_changes(
        &self,
        program: &Program<'_>,
        changes: &[publication::UnitChange],
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<(), UseError> {
        ledger.charge(
            domain,
            WorkKind::Analysis,
            changes
                .len()
                .checked_mul(4)
                .and_then(|n| n.checked_add(1))
                .ok_or(UseError::Capacity)? as u64,
        )?;
        if program.tables_revision != self.tables_revision
            || program.units.len() != self.units.len()
            || program.cells.len() != self.cells.len()
        {
            return Err(UseError::UnstampedTables);
        }
        let mut previous = None;
        for change in changes {
            if previous.is_some_and(|id| id >= change.unit) || change.previous == change.current {
                return Err(UseError::InvalidChangeSet);
            }
            let old = self
                .units
                .get(change.unit.index())
                .ok_or(UseError::InvalidChangeSet)?;
            let current = program
                .units
                .get(change.unit.index())
                .ok_or(UseError::InvalidChangeSet)?;
            if current.id() != change.unit
                || old.data.revision != change.previous
                || current.revision() != change.current
            {
                return Err(UseError::InvalidChangeSet);
            }
            previous = Some(change.unit);
        }
        Ok(())
    }

    /// The transaction owns both the complete change list and unchanged
    /// metadata. Never replace this borrowed capability with a caller flag.
    pub(in crate::program) fn prepare_fixed_replacements(
        &self,
        edits: &publication::FixedUnitEdits<'_, '_>,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<PreparedUseUpdate, UseError> {
        let start = ledger.work_used(domain);
        ledger.charge(domain, WorkKind::Analysis, 1)?;
        if !std::ptr::eq(self, edits.previous_uses()) {
            return Err(UseError::InvalidChangeSet);
        }
        let mut prepared = self.prepare_sparse(
            edits.program(),
            edits.changes().iter().map(|change| change.unit),
            false,
            ledger,
            domain,
        )?;
        prepared.receipt.logical_work = ledger.work_used(domain) - start;
        Ok(prepared)
    }

    /// General validating wrapper retained for unrestricted internal clients.
    pub(in crate::program) fn prepare_replacements(
        &self,
        program: &Program<'_>,
        changed: &[UnitId],
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<PreparedUseUpdate, UseError> {
        ledger.charge(domain, WorkKind::Analysis, 1)?;
        if program.tables_revision != self.tables_revision
            || program.units.len() != self.units.len()
            || program.cells.len() != self.cells.len()
        {
            return Err(UseError::UnstampedTables);
        }
        let mut prepared = self.prepare_update(program, changed, ledger, domain)?;
        prepared.receipt.logical_work += 1;
        Ok(prepared)
    }

    pub(super) fn prepare_update(
        &self,
        program: &Program<'_>,
        changed: &[UnitId],
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<PreparedUseUpdate, UseError> {
        let start = ledger.work_used(domain);
        if program.units.len() < self.units.len() || program.cells.len() < self.cells.len() {
            return Err(UseError::ShrinkingTables);
        }
        let mut scratch = Scratch::new(ledger, domain);
        let mut declared = scratch_vector(changed.len(), &mut scratch)?;
        scratch.work(changed.len())?;
        declared.extend_from_slice(changed);
        scratch.work(sort_work(declared.len())?)?;
        declared.sort_unstable();
        scratch.work(declared.len())?;
        for (index, id) in declared.iter().enumerate() {
            if id.index() >= program.units.len() || index != 0 && declared[index - 1] == *id {
                return Err(UseError::InvalidChangeSet);
            }
        }
        // Public callers can omit a change, so this one header scan remains.
        // Compact actual IDs in the same admitted vector; do not allocate a
        // second changed-unit view or a Program-sized membership mask.
        scratch.work(program.units.len())?;
        let mut cursor = 0usize;
        let mut actual = 0usize;
        for (index, unit) in program.units.iter().enumerate() {
            if unit.id().index() != index {
                return Err(UseError::InvalidProgram("unit identity"));
            }
            let was_declared = declared.get(cursor) == Some(&unit.id());
            if was_declared {
                cursor += 1;
            }
            let changed = self
                .units
                .get(index)
                .is_none_or(|old| old.data.revision != unit.revision());
            if changed {
                if !was_declared {
                    return Err(UseError::UndeclaredUnitChange(unit.id()));
                }
                declared[actual] = unit.id();
                actual += 1;
            }
        }
        debug_assert_eq!(cursor, declared.len());
        declared.truncate(actual);
        scratch.work(
            program
                .exports()
                .len()
                .checked_add(self.exports.len())
                .ok_or(UseError::Capacity)?,
        )?;
        if program.exports().len() > u32::MAX as usize {
            return Err(UseError::Capacity);
        }
        let exports = || {
            program
                .exports()
                .iter()
                .enumerate()
                .filter_map(|(index, export)| match export.target {
                    InterfaceTarget::Value(cell) => Some((index as u32, cell)),
                    InterfaceTarget::Struct(_) => None,
                })
        };
        if exports().any(|(_, cell)| cell.index() >= program.cells.len()) {
            return Err(UseError::InvalidProgram("export cell"));
        }
        // Both the bounds pass and sequence comparison visit export metadata.
        scratch.work(program.exports().len())?;
        let exports_changed = !exports().eq(self.exports.iter().copied());
        if program.tables_revision == self.tables_revision
            && (program.cells.len() != self.cells.len() || exports_changed)
        {
            return Err(UseError::UnstampedTables);
        }
        let mut prepared = self.prepare_sparse(
            program,
            declared.iter().copied(),
            exports_changed,
            scratch.ledger,
            domain,
        )?;
        prepared.receipt.logical_work = scratch.ledger.work_used(domain) - start;
        Ok(prepared)
    }

    /// Both validation boundaries enter this exact chunk-building owner.
    /// `changed` is sorted, unique, and contains precisely changed revisions.
    fn prepare_sparse(
        &self,
        program: &Program<'_>,
        changed: impl Clone + ExactSizeIterator<Item = UnitId>,
        exports_changed: bool,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<PreparedUseUpdate, UseError> {
        let start = ledger.work_used(domain);
        let header = size_of::<PreparedUseUpdate>() as u64;
        ledger.retain(domain, header)?;
        let mut pending = Pending {
            update: Some(PreparedUseUpdate {
                owner: self.update_owner,
                tables_revision: program.tables_revision,
                unit_count: program.units.len(),
                cell_count: program.cells.len(),
                fixed: program.tables_revision == self.tables_revision
                    && program.units.len() == self.units.len()
                    && program.cells.len() == self.cells.len(),
                units: Vec::new(),
                cells: Vec::new(),
                creators: Vec::new(),
                metadata: Charge {
                    domain,
                    bytes: header,
                },
                receipt: UseIndexReceipt::default(),
            }),
            destination: None,
            ledger,
        };
        {
            let update = pending.update.as_mut().unwrap();
            let mut scratch = Scratch::new(pending.ledger, domain);
            update.units = metadata_vector(changed.len(), &mut update.metadata, &mut scratch)?;
            for id in changed {
                scratch.work(1)?;
                let frozen = program
                    .units
                    .get(id.index())
                    .ok_or(UseError::InvalidChangeSet)?;
                let unit = build_unit(frozen, program.cells.len(), scratch.ledger, domain)?;
                update.receipt.allocated_bytes += unit.charge.bytes;
                update.receipt.rebuilt_units += 1;
                // Own the chunk before comparison admission can fail.
                update.units.push(UnitReplacement {
                    unit: id,
                    uses: unit,
                    cells_changed: false,
                    creators_changed: false,
                });
                let row = update.units.last_mut().unwrap();
                let old = self.units.get(id.index()).map(|old| &old.data);
                let old_cells = old.map_or(&[][..], |old| old.cells.as_slice());
                let old_creators = old.map_or(&[][..], |old| old.closures.as_slice());
                scratch.work(
                    old_cells
                        .len()
                        .min(row.uses.data.cells.len())
                        .checked_add(old_creators.len().min(row.uses.data.closures.len()))
                        .and_then(|n| n.checked_add(2))
                        .ok_or(UseError::Capacity)?,
                )?;
                row.cells_changed = old_cells != row.uses.data.cells.as_slice();
                row.creators_changed = old_creators != row.uses.data.closures.as_slice();
            }
            prepare_cells(
                self,
                program,
                &update.units,
                exports_changed,
                &mut update.cells,
                &mut update.metadata,
                &mut update.receipt,
                &mut scratch,
            )?;
            // The filtered iterator visits all prepared rows on both passes.
            scratch.work(
                update
                    .units
                    .len()
                    .checked_mul(2)
                    .ok_or(UseError::Capacity)?,
            )?;
            prepare_creators(
                Some(self),
                update
                    .units
                    .iter()
                    .filter(|row| row.creators_changed)
                    .map(|row| (row.unit, &row.uses.data)),
                &update.units,
                program.units.len(),
                &mut update.creators,
                &mut update.metadata,
                &mut update.receipt,
                &mut scratch,
            )?;
            // Installation/release cannot seek work after the commit point.
            scratch.work(
                update
                    .units
                    .len()
                    .checked_add(update.cells.len())
                    .and_then(|n| n.checked_add(update.creators.len()))
                    .and_then(|n| n.checked_add(1))
                    .ok_or(UseError::Capacity)?,
            )?;
            update.receipt.logical_work = scratch.ledger.work_used(domain) - start;
        }
        Ok(pending.update.take().unwrap())
    }

    /// No admission or allocation. The caller must have checked valid_for and
    /// completed every fallible transaction action before transferring ownership.
    pub(in crate::program) fn install_replacements(
        &mut self,
        update: PreparedUseUpdate,
        ledger: &mut BudgetLedger,
    ) {
        assert!(
            update.fixed && update.valid_for(self),
            "prepared index destination"
        );
        self.install_prepared(update, ledger, 0, 0);
    }

    pub(in crate::program) fn fork_fixed_replacements(
        &self,
        edits: &publication::FixedUnitEdits<'_, '_>,
        update: PreparedUseUpdate,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, UseError> {
        self.fork_checked(
            edits.program(),
            update,
            ledger,
            domain,
            |prepared, ledger| {
                ledger.charge(domain, WorkKind::Analysis, 1)?;
                if !std::ptr::eq(self, edits.previous_uses()) {
                    return Err(UseError::InvalidChangeSet);
                }
                self.validate_fixed_prepared(edits.changes(), prepared, ledger, domain)
            },
        )
    }

    // The capability has already qualified these changes against its Program.
    // The destination stamp alone would also accept a different edit prepared
    // against the same old index; compare the exact replacement revisions.
    fn validate_fixed_prepared(
        &self,
        changes: &[publication::UnitChange],
        prepared: &PreparedUseUpdate,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<(), UseError> {
        ledger.charge(
            domain,
            WorkKind::Analysis,
            changes.len().checked_add(1).ok_or(UseError::Capacity)? as u64,
        )?;
        if !prepared.fixed || changes.len() != prepared.units.len() {
            return Err(UseError::InvalidChangeSet);
        }
        for (change, row) in changes.iter().zip(&prepared.units) {
            if change.unit != row.unit || change.current != row.uses.data.revision {
                return Err(UseError::InvalidChangeSet);
            }
        }
        Ok(())
    }

    pub(in crate::program) fn fork_replacements(
        &self,
        program: &Program<'_>,
        update: PreparedUseUpdate,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, UseError> {
        self.fork_checked(program, update, ledger, domain, |prepared, ledger| {
            ledger.charge(domain, WorkKind::Analysis, program.units.len() as u64)?;
            let mut next = prepared.units.iter().peekable();
            for (index, unit) in program.units.iter().enumerate() {
                let uses = if next.peek().is_some_and(|row| row.unit.index() == index) {
                    &next.next().unwrap().uses.data
                } else {
                    &self
                        .units
                        .get(index)
                        .ok_or(UseError::InvalidChangeSet)?
                        .data
                };
                if uses.revision != unit.revision() {
                    return Err(UseError::InvalidChangeSet);
                }
            }
            if prepared.fixed {
                ledger.charge(domain, WorkKind::Analysis, program.exports().len() as u64)?;
                let exports = program
                    .exports()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, export)| match export.target {
                        InterfaceTarget::Value(cell) => Some((index as u32, cell)),
                        InterfaceTarget::Struct(_) => None,
                    });
                if !exports.eq(self.exports.iter().copied()) {
                    return Err(UseError::UnstampedTables);
                }
            }
            Ok(())
        })
    }

    /// One owning fork kernel; a rejected qualifier still consumes and cleans
    /// every prepared chunk. Fixed and unrestricted entries only differ in the
    /// evidence allowing them to qualify the destination Program.
    fn fork_checked(
        &self,
        program: &Program<'_>,
        update: PreparedUseUpdate,
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
        validate: impl FnOnce(&PreparedUseUpdate, &mut BudgetLedger) -> Result<(), UseError>,
    ) -> Result<Self, UseError> {
        let start = ledger.work_used(domain);
        let mut pending = Pending {
            update: Some(update),
            destination: None,
            ledger,
        };
        let prepared = pending.update.as_ref().unwrap();
        pending.ledger.charge(domain, WorkKind::Analysis, 1)?;
        if !prepared.valid_for(self)
            || prepared.tables_revision != program.tables_revision
            || prepared.unit_count != program.units.len()
            || prepared.cell_count != program.cells.len()
        {
            return Err(UseError::InvalidChangeSet);
        }
        validate(prepared, pending.ledger)?;
        // Even a fixed retained fork must own its new full header shell. The
        // fixed entry skips redundant validation, not this actual copy cost.
        pending.ledger.charge(
            domain,
            WorkKind::Analysis,
            program
                .units
                .len()
                .checked_add(program.cells.len())
                .ok_or(UseError::Capacity)? as u64,
        )?;
        let destination = UseIndex::shell(program, pending.ledger, domain)?;
        pending.destination = Some(destination);
        let prepared = pending.update.as_ref().unwrap();
        let destination = pending.destination.as_mut().unwrap();
        let mut next = prepared.units.iter().peekable();
        for index in 0..program.units.len() {
            destination
                .units
                .push(if let Some(old) = self.units.get(index) {
                    old.clone()
                } else {
                    while next.peek().is_some_and(|row| row.unit.index() < index) {
                        next.next();
                    }
                    next.next().expect("prepared appended unit").uses.clone()
                });
            destination.creators.push(
                self.creators
                    .get(index)
                    .map_or_else(CreatorUsers::empty, CreatorUsers::share),
            );
        }
        for index in 0..program.cells.len() {
            destination.cells.push(
                self.cells
                    .get(index)
                    .map_or_else(CellUsers::empty, CellUsers::share),
            );
        }
        let shell_bytes = destination.charge.bytes;
        let work = pending.ledger.work_used(domain) - start;
        let update = pending.update.take().unwrap();
        pending.destination.as_mut().unwrap().install_prepared(
            update,
            pending.ledger,
            shell_bytes,
            work,
        );
        Ok(pending.destination.take().unwrap())
    }

    fn install_prepared(
        &mut self,
        update: PreparedUseUpdate,
        ledger: &mut BudgetLedger,
        shell_bytes: u64,
        work: u64,
    ) {
        let PreparedUseUpdate {
            units,
            cells,
            creators,
            metadata,
            mut receipt,
            tables_revision,
            ..
        } = update;
        for replacement in units {
            let old =
                std::mem::replace(&mut self.units[replacement.unit.index()], replacement.uses);
            release_shared(old, ledger).expect("owned replaced unit-use chunk");
        }
        for (id, _, cell) in cells {
            let old = std::mem::replace(&mut self.cells[id.index()], cell);
            if let Some(sites) = old.sites {
                release_shared(sites, ledger).expect("owned replaced cell-use chunk");
            }
        }
        for (id, creators) in creators {
            let old = std::mem::replace(&mut self.creators[id.index()], creators);
            if let Some(sites) = old.sites {
                release_shared(sites, ledger).expect("owned replaced creator-use chunk");
            }
        }
        ledger
            .release(metadata.domain, metadata.bytes)
            .expect("owned prepared-index metadata");
        receipt.allocated_bytes += shell_bytes;
        receipt.logical_work += work;
        self.receipt = receipt;
        self.tables_revision = tables_revision;
        self.update_owner = RevisionId::fresh();
    }
}

/// A scalar cursor over one sorted predecessor stream. It avoids repeating the
/// same admitted binary lookup for adjacent occurrences in one source unit.
struct Removal<'a> {
    units: &'a [UnitReplacement],
    creators: bool,
    last: Option<(UnitId, bool)>,
}
impl Removal<'_> {
    fn removes(&mut self, unit: UnitId, scratch: &mut Scratch<'_>) -> Result<bool, UseError> {
        if let Some((previous, changed)) = self.last {
            if previous == unit {
                return Ok(changed);
            }
        }
        scratch.work((usize::BITS - self.units.len().leading_zeros()) as usize + 1)?;
        let changed = self
            .units
            .binary_search_by_key(&unit, |row| row.unit)
            .ok()
            .is_some_and(|index| {
                let row = &self.units[index];
                if self.creators {
                    row.creators_changed
                } else {
                    row.cells_changed
                }
            });
        self.last = Some((unit, changed));
        Ok(changed)
    }
}

#[allow(clippy::too_many_arguments)]
fn prepare_cells(
    previous: &UseIndex,
    program: &Program<'_>,
    units: &[UnitReplacement],
    exports_changed: bool,
    output: &mut Vec<CellReplacement>,
    metadata: &mut Charge,
    receipt: &mut UseIndexReceipt,
    scratch: &mut Scratch<'_>,
) -> Result<(), UseError> {
    let exports = || {
        program
            .exports()
            .iter()
            .enumerate()
            .filter_map(|(index, export)| match export.target {
                InterfaceTarget::Value(cell) => Some((index as u32, cell)),
                InterfaceTarget::Struct(_) => None,
            })
    };
    let mut endpoints = 0usize;
    let mut additions = 0usize;
    scratch.work(units.len())?;
    for row in units.iter().filter(|row| row.cells_changed) {
        let old = previous
            .units
            .get(row.unit.index())
            .map_or(0, |unit| unit.data.cells.len());
        endpoints = endpoints
            .checked_add(old)
            .and_then(|n| n.checked_add(row.uses.data.cells.len()))
            .ok_or(UseError::Capacity)?;
        additions = additions
            .checked_add(row.uses.data.cells.len())
            .ok_or(UseError::Capacity)?;
    }
    if exports_changed {
        scratch.work(program.exports().len())?;
        let count = exports().count();
        endpoints = endpoints
            .checked_add(previous.exports.len())
            .and_then(|n| n.checked_add(count))
            .ok_or(UseError::Capacity)?;
        additions = additions.checked_add(count).ok_or(UseError::Capacity)?;
    }
    let mut affected = scratch_vector(endpoints, scratch)?;
    let mut added = scratch_vector(additions, scratch)?;
    scratch.work(units.len())?;
    for row in units.iter().filter(|row| row.cells_changed) {
        let old = previous
            .units
            .get(row.unit.index())
            .map_or(&[][..], |unit| unit.data.cells.as_slice());
        scratch.work(
            old.len()
                .checked_add(row.uses.data.cells.len())
                .ok_or(UseError::Capacity)?,
        )?;
        affected.extend(old.iter().map(|reference| reference.cell));
        for reference in &row.uses.data.cells {
            affected.push(reference.cell);
            added.push((
                reference.cell,
                CellUseSite::Unit {
                    unit: row.unit,
                    usage: reference.usage,
                },
            ));
        }
    }
    if exports_changed {
        scratch.work(
            previous
                .exports
                .len()
                .checked_add(program.exports().len())
                .ok_or(UseError::Capacity)?,
        )?;
        affected.extend(previous.exports.iter().map(|&(_, cell)| cell));
        for (index, cell) in exports() {
            affected.push(cell);
            added.push((cell, CellUseSite::Export { index }));
        }
    }
    scratch.work(
        sort_work(affected.len())?
            .checked_add(sort_work(added.len())?)
            .ok_or(UseError::Capacity)?,
    )?;
    affected.sort_unstable();
    added.sort_unstable();
    scratch.work(affected.len())?;
    affected.dedup();
    *output = metadata_vector(affected.len(), metadata, scratch)?;
    scratch.work(
        affected
            .len()
            .checked_add(added.len())
            .ok_or(UseError::Capacity)?,
    )?;
    let mut cursor = 0usize;
    for cell in affected {
        let begin = cursor;
        while cursor < added.len() && added[cursor].0 == cell {
            cursor += 1;
        }
        let old = previous.cells.get(cell.index());
        if let Some(users) = merge_cell(
            old.map_or(&[][..], CellUsers::sites),
            &added[begin..cursor],
            units,
            exports_changed,
            scratch,
        )? {
            receipt.allocated_bytes += users.sites.as_ref().map_or(0, |sites| sites.charge.bytes);
            receipt.rebuilt_cell_sets += 1;
            output.push((cell, old.map(|old| old.revision), users));
        }
    }
    debug_assert_eq!(cursor, added.len());
    Ok(())
}

fn merge_cell(
    old: &[CellUseSite],
    added: &[(CellId, CellUseSite)],
    changed: &[UnitReplacement],
    exports_changed: bool,
    scratch: &mut Scratch<'_>,
) -> Result<Option<CellUsers>, UseError> {
    let count = old
        .len()
        .checked_add(added.len())
        .ok_or(UseError::Capacity)?;
    let charge = cell_bytes(count)?;
    scratch.ledger.retain(scratch.domain, charge)?;
    let outcome = (|| {
        let mut sites = vector(count)?;
        scratch.work(
            count
                .checked_mul(3)
                .and_then(|n| n.checked_add(1))
                .ok_or(UseError::Capacity)?,
        )?;
        let mut removal = Removal {
            units: changed,
            creators: false,
            last: None,
        };
        let mut before = 0usize;
        let mut after = 0usize;
        loop {
            while let Some(site) = old.get(before) {
                let remove = match *site {
                    CellUseSite::Unit { unit, .. } => removal.removes(unit, scratch)?,
                    CellUseSite::Export { .. } => exports_changed,
                };
                if !remove {
                    break;
                }
                before += 1;
            }
            match (old.get(before), added.get(after)) {
                (Some(old), Some((_, new))) if old <= new => {
                    sites.push(*old);
                    before += 1;
                }
                (_, Some((_, new))) => {
                    sites.push(*new);
                    after += 1;
                }
                (Some(old), None) => {
                    sites.push(*old);
                    before += 1;
                }
                (None, None) => break,
            }
        }
        scratch.work(sites.len().min(old.len()))?;
        if sites == old {
            return Ok(None);
        }
        if sites.is_empty() {
            return Ok(Some(CellUsers::empty()));
        }
        scratch.work(sites.len())?;
        let reference_exposed = sites.iter().any(|site| {
            matches!(
                site,
                CellUseSite::Unit {
                    usage: CellUse::Reference { .. },
                    ..
                }
            )
        });
        Ok(Some(CellUsers {
            revision: RevisionId::fresh(),
            reference_exposed,
            sites: Some(Arc::new(Shared {
                data: sites,
                charge: Charge {
                    domain: scratch.domain,
                    bytes: charge,
                },
            })),
        }))
    })();
    if !matches!(&outcome, Ok(Some(CellUsers { sites: Some(_), .. }))) {
        scratch.ledger.release(scratch.domain, charge)?;
    }
    outcome
}

/// The existing closure occurrence owner feeds initial and incremental merges.
#[allow(clippy::too_many_arguments)]
fn prepare_creators<'a>(
    previous: Option<&UseIndex>,
    units: impl Clone + Iterator<Item = (UnitId, &'a UnitUses)>,
    changed: &[UnitReplacement],
    unit_count: usize,
    output: &mut Vec<CreatorReplacement>,
    metadata: &mut Charge,
    receipt: &mut UseIndexReceipt,
    scratch: &mut Scratch<'_>,
) -> Result<(), UseError> {
    let mut endpoints = 0usize;
    let mut additions = 0usize;
    for (id, unit) in units.clone() {
        scratch.work(1)?;
        let old = previous
            .and_then(|old| old.units.get(id.index()))
            .map_or(0, |old| old.data.closures.len());
        endpoints = endpoints
            .checked_add(old)
            .and_then(|n| n.checked_add(unit.closures.len()))
            .ok_or(UseError::Capacity)?;
        additions = additions
            .checked_add(unit.closures.len())
            .ok_or(UseError::Capacity)?;
    }
    let mut affected = scratch_vector(endpoints, scratch)?;
    let mut added = scratch_vector(additions, scratch)?;
    for (id, unit) in units {
        let old = previous
            .and_then(|old| old.units.get(id.index()))
            .map_or(&[][..], |old| old.data.closures.as_slice());
        scratch.work(
            old.len()
                .checked_add(unit.closures.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(UseError::Capacity)?,
        )?;
        for &(_, body) in old {
            if body.index() >= unit_count {
                return Err(UseError::InvalidProgram("closure body"));
            }
            affected.push(body);
        }
        for &(operation, body) in &unit.closures {
            if body.index() >= unit_count {
                return Err(UseError::InvalidProgram("closure body"));
            }
            affected.push(body);
            added.push((
                body,
                ClosureCreation {
                    unit: id,
                    operation,
                },
            ));
        }
    }
    scratch.work(
        sort_work(affected.len())?
            .checked_add(sort_work(added.len())?)
            .ok_or(UseError::Capacity)?,
    )?;
    affected.sort_unstable();
    added.sort_unstable();
    scratch.work(affected.len())?;
    affected.dedup();
    *output = metadata_vector(affected.len(), metadata, scratch)?;
    scratch.work(
        affected
            .len()
            .checked_add(added.len())
            .ok_or(UseError::Capacity)?,
    )?;
    let mut cursor = 0usize;
    for body in affected {
        let begin = cursor;
        while cursor < added.len() && added[cursor].0 == body {
            cursor += 1;
        }
        let old = previous
            .and_then(|old| old.creators.get(body.index()))
            .map_or(&[][..], CreatorUsers::sites);
        if let Some(creators) = merge_creators(old, &added[begin..cursor], changed, scratch)? {
            receipt.rebuilt_creator_sets += 1;
            receipt.allocated_bytes += creators
                .sites
                .as_ref()
                .map_or(0, |sites| sites.charge.bytes);
            output.push((body, creators));
        }
    }
    debug_assert_eq!(cursor, added.len());
    Ok(())
}

fn merge_creators(
    old: &[ClosureCreation],
    added: &[(UnitId, ClosureCreation)],
    changed: &[UnitReplacement],
    scratch: &mut Scratch<'_>,
) -> Result<Option<CreatorUsers>, UseError> {
    let count = old
        .len()
        .checked_add(added.len())
        .ok_or(UseError::Capacity)?;
    let charge = creator_bytes(count)?;
    scratch.ledger.retain(scratch.domain, charge)?;
    let outcome = (|| {
        let mut sites = vector(count)?;
        scratch.work(
            count
                .checked_mul(3)
                .and_then(|n| n.checked_add(1))
                .ok_or(UseError::Capacity)?,
        )?;
        let mut removal = Removal {
            units: changed,
            creators: true,
            last: None,
        };
        let mut before = 0usize;
        let mut after = 0usize;
        loop {
            while let Some(site) = old.get(before) {
                if !removal.removes(site.unit, scratch)? {
                    break;
                }
                before += 1;
            }
            match (old.get(before), added.get(after)) {
                (Some(old), Some((_, new))) if old <= new => {
                    sites.push(*old);
                    before += 1;
                }
                (_, Some((_, new))) => {
                    sites.push(*new);
                    after += 1;
                }
                (Some(old), None) => {
                    sites.push(*old);
                    before += 1;
                }
                (None, None) => break,
            }
        }
        scratch.work(sites.len().min(old.len()))?;
        if sites == old {
            return Ok(None);
        }
        if sites.is_empty() {
            return Ok(Some(CreatorUsers::empty()));
        }
        Ok(Some(CreatorUsers {
            revision: RevisionId::fresh(),
            sites: Some(Arc::new(Shared {
                data: sites,
                charge: Charge {
                    domain: scratch.domain,
                    bytes: charge,
                },
            })),
        }))
    })();
    if !matches!(&outcome, Ok(Some(CreatorUsers { sites: Some(_), .. }))) {
        scratch.ledger.release(scratch.domain, charge)?;
    }
    outcome
}

pub(super) fn build_creators(
    index: &mut UseIndex,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<(), UseError> {
    let header = size_of::<PreparedUseUpdate>() as u64;
    ledger.retain(domain, header)?;
    let mut pending = Pending {
        update: Some(PreparedUseUpdate {
            owner: index.update_owner,
            tables_revision: index.tables_revision,
            unit_count: index.units.len(),
            cell_count: index.cells.len(),
            fixed: true,
            units: Vec::new(),
            cells: Vec::new(),
            creators: Vec::new(),
            metadata: Charge {
                domain,
                bytes: header,
            },
            receipt: UseIndexReceipt::default(),
        }),
        destination: None,
        ledger,
    };
    {
        let update = pending.update.as_mut().unwrap();
        let mut scratch = Scratch::new(pending.ledger, domain);
        scratch.work(index.units.len())?;
        for _ in 0..index.units.len() {
            index.creators.push(CreatorUsers::empty());
        }
        prepare_creators(
            None,
            index.units.iter().enumerate().map(|(id, unit)| {
                (
                    UnitId::from_index(id).expect("checked unit identity"),
                    &unit.data,
                )
            }),
            &[],
            index.units.len(),
            &mut update.creators,
            &mut update.metadata,
            &mut update.receipt,
            &mut scratch,
        )?;
        scratch.work(update.creators.len())?;
    }
    let update = pending.update.take().unwrap();
    let receipt = update.receipt;
    let old_receipt = index.receipt;
    index.install_prepared(update, pending.ledger, 0, 0);
    index.receipt = UseIndexReceipt {
        rebuilt_creator_sets: old_receipt.rebuilt_creator_sets + receipt.rebuilt_creator_sets,
        allocated_bytes: old_receipt.allocated_bytes + receipt.allocated_bytes,
        ..old_receipt
    };
    Ok(())
}

#[cfg(test)]
#[path = "uses_update_tests.rs"]
mod tests;
