//! One checked patch kernel with retained-fork and consuming publication.
//! Undo owns only changed payloads; indexes stay coherent until final commit.
use super::*;
use crate::output_budget::VectorLayout;
use crate::program::uses::PreparedUseUpdate;

/// A borrowed mutation footprint, created only after this transaction's UnitEdit
/// kernel has changed operations/operands/Places in an already verified snapshot.
/// Validation checks index/stamp agreement; unchanged tables and headers follow
/// from that kernel's actual mutation boundary, not a caller-provided list.
pub(in crate::program) struct FixedUnitEdits<'view, 'src> {
    program: &'view Program<'src>,
    previous_uses: &'view UseIndex,
    changes: &'view [UnitChange],
}
impl<'view, 'src> FixedUnitEdits<'view, 'src> {
    fn new(
        program: &'view Program<'src>,
        previous_uses: &'view UseIndex,
        changes: &'view [UnitChange],
        ledger: &mut BudgetLedger,
        domain: WorkDomain,
    ) -> Result<Self, PublicationError> {
        previous_uses.validate_fixed_changes(program, changes, ledger, domain)?;
        Ok(Self {
            program,
            previous_uses,
            changes,
        })
    }
    pub(in crate::program) fn program(&self) -> &'view Program<'src> {
        self.program
    }
    pub(in crate::program) fn previous_uses(&self) -> &'view UseIndex {
        self.previous_uses
    }
    pub(in crate::program) fn changes(&self) -> &'view [UnitChange] {
        self.changes
    }
}

impl<'src> Compilation<'src> {
    pub(super) fn edit_transaction(
        &mut self,
        base: SemanticId,
        patches: &[UnitPatch<'_>],
        domain: WorkDomain,
        advance: bool,
    ) -> Result<SemanticId, PublicationError> {
        let base_slot = self.lookup(base)?;
        let target_slot = if advance {
            if self.slots[base_slot]
                .checkpoint
                .as_ref()
                .unwrap()
                .implementations
                .is_some()
            {
                return Err(PublicationError::InvalidPatch);
            }
            base_slot as u32
        } else {
            self.free.ok_or(PublicationError::StoreFull)?
        };
        if patches.is_empty() {
            return Err(PublicationError::InvalidPatch);
        }
        let mut transaction = Transaction::new(self, base_slot, target_slot, domain, advance);
        transaction.prepare(patches)?;
        transaction.apply();
        transaction.finish_checks()?;
        Ok(transaction.commit())
    }

    /// Publish one checked rewrite: its site becomes the rule's replacement
    /// constant with no operands, in a fresh checkpoint whose lineage records
    /// the rule. The original snapshot and every artifact survive.
    pub(super) fn edit_checked_rewrite_transaction(
        &mut self,
        proof: rewrites::CheckedRewrite,
        domain: WorkDomain,
        started: u64,
    ) -> Result<SemanticId, PublicationError> {
        let base_slot = self.lookup(proof.base())?;
        let target_slot = self.free.ok_or(PublicationError::StoreFull)?;
        let rule = proof.rule();
        let replacement = proof.replacement();
        let operations = [OperationPatch {
            operation: rule.operation(),
            kind: &replacement,
            operands: &[],
        }];
        let patches = [UnitPatch {
            unit: rule.unit(),
            expected_revision: proof.unit_revision(),
            operations: &operations,
            places: &[],
        }];
        let mut transaction = Transaction::new(self, base_slot, target_slot, domain, false);
        transaction.start = started;
        transaction.rewrite = Some(proof);
        transaction.prepare(&patches)?;
        transaction.apply();
        transaction.finish_checks()?;
        Ok(transaction.commit())
    }
}

struct UnitEdit {
    unit: UnitId,
    previous: RevisionId,
    current: RevisionId,
    /// Present only when the old allocation really must remain retained.
    copied: Option<(FrozenUnit, Charge)>,
    kinds: Vec<(OpId, OperationKind)>,
    places: Vec<(PlaceId, Place)>,
    /// Before apply these are new values/ranges; afterwards they are undo.
    operands: Option<(Vec<ValueId>, Vec<OperandRange>)>,
    new_payload: u64,
    old_payload: u64,
    applied: bool,
}

impl UnitEdit {
    fn swap(&mut self, unit: &mut FrozenUnit) {
        let (data, revision) = unit.unique_edit().expect("private edit allocation");
        for (operation, kind) in &mut self.kinds {
            std::mem::swap(&mut data.operations[operation.index()].kind, kind);
        }
        for (place, replacement) in &mut self.places {
            std::mem::swap(&mut data.places[place.index()], replacement);
        }
        if let Some((values, ranges)) = &mut self.operands {
            std::mem::swap(&mut data.operands, values);
            for (operation, range) in data.operations.iter_mut().zip(ranges) {
                std::mem::swap(&mut operation.operands, range);
            }
        }
        *revision = if self.applied {
            self.previous
        } else {
            self.current
        };
        self.applied = !self.applied;
    }
}

struct Transaction<'a, 'src> {
    compiler: &'a mut Compilation<'src>,
    base_slot: usize,
    target_slot: u32,
    advance: bool,
    domain: WorkDomain,
    start: u64,
    original: Option<Box<Checkpoint<'src>>>,
    fork: Option<Pending<'src>>,
    edits: Vec<UnitEdit>,
    plans: Vec<PatchPlan>,
    changes: Vec<UnitChange>,
    cell_changes: Vec<CellUseChange>,
    prepared: Option<PreparedUseUpdate>,
    new_index: Option<UseIndex>,
    /// Newly admitted payload owned here, excluding UseIndex's own charges.
    reserved: u64,
    copied_units: usize,
    reused_units: usize,
    copied_bytes: u64,
    verified: super::super::verify::VerificationReceipt,
    committed: bool,
    rewrite: Option<rewrites::CheckedRewrite>,
    origin: Option<SnapshotOrigin>,
    rewrite_bytes: u64,
}

impl<'a, 'src> Transaction<'a, 'src> {
    fn new(
        compiler: &'a mut Compilation<'src>,
        base_slot: usize,
        target_slot: u32,
        domain: WorkDomain,
        advance: bool,
    ) -> Self {
        let start = compiler.ledger.work_used(domain);
        let original = compiler.slots[base_slot].checkpoint.take();
        Self {
            compiler,
            base_slot,
            target_slot,
            advance,
            domain,
            start,
            original,
            fork: None,
            edits: Vec::new(),
            plans: Vec::new(),
            changes: Vec::new(),
            cell_changes: Vec::new(),
            prepared: None,
            new_index: None,
            reserved: 0,
            copied_units: 0,
            reused_units: 0,
            copied_bytes: 0,
            verified: Default::default(),
            committed: false,
            rewrite: None,
            origin: None,
            rewrite_bytes: 0,
        }
    }

    fn work(&mut self, amount: usize) -> Result<(), PublicationError> {
        work(&mut self.compiler.ledger, self.domain, amount)
    }
    fn reserve(&mut self, amount: u64) -> Result<(), PublicationError> {
        let next = self
            .reserved
            .checked_add(amount)
            .ok_or(PublicationError::Capacity)?;
        self.compiler.ledger.retain(self.domain, amount)?;
        self.reserved = next;
        Ok(())
    }
    fn release(&mut self, amount: u64) {
        self.compiler
            .ledger
            .release(self.domain, amount)
            .expect("edit reservation");
        self.reserved -= amount;
    }
    fn vector<T>(&mut self, count: usize) -> Result<Vec<T>, PublicationError> {
        transaction_vector(
            &mut self.compiler.ledger,
            self.domain,
            &mut self.reserved,
            count,
        )
    }
    fn program(&self) -> &Program<'src> {
        self.fork.as_ref().map_or_else(
            || &self.original.as_ref().unwrap().semantic.program,
            |fork| &fork.program,
        )
    }
    fn parts(&mut self) -> (&mut Program<'src>, &mut SemanticCharges) {
        if let Some(fork) = &mut self.fork {
            (&mut fork.program, &mut fork.charges)
        } else {
            let semantic = Arc::get_mut(&mut self.original.as_mut().unwrap().semantic)
                .expect("exclusive source snapshot");
            (&mut semantic.program, &mut semantic.charges)
        }
    }

    fn prepare(&mut self, patches: &[UnitPatch<'_>]) -> Result<(), PublicationError> {
        let identity = RevisionId::fresh();
        let semantic = &self.original.as_ref().unwrap().semantic;
        let mut budget = AllocationBudget::new(Some((&mut self.compiler.ledger, self.domain)));
        let (meaning, lineage) = if let Some(proof) = self.rewrite {
            (
                semantic.meaning,
                semantic
                    .lineage
                    .append(self.compiler.store, proof.step(identity), &mut budget)?,
            )
        } else {
            (RevisionId::fresh(), semantic.lineage.share(&mut budget)?)
        };
        self.rewrite_bytes = lineage.retained_bytes() - semantic.lineage.retained_bytes();
        self.origin = Some(SnapshotOrigin {
            identity,
            meaning,
            lineage,
        });
        drop(budget);
        self.plans = self.vector(patches.len())?;
        let mut previous = None;
        for patch in patches {
            self.work(1)?;
            if previous.is_some_and(|id| id >= patch.unit) {
                return Err(PublicationError::InvalidPatch);
            }
            previous = Some(patch.unit);
            let frozen = self
                .original
                .as_ref()
                .unwrap()
                .semantic
                .program
                .units
                .get(patch.unit.index())
                .ok_or(PublicationError::InvalidPatch)?;
            if frozen.revision() != patch.expected_revision {
                return Err(PublicationError::StaleRevision(patch.unit));
            }
            let mut workspace = Workspace::new(&mut self.compiler.ledger, self.domain);
            self.plans
                .push(plan_patch(frozen.data(), patch, &mut workspace)?);
        }
        let checkpoint = self.original.as_ref().unwrap();
        let reuse_shell = self.advance
            && checkpoint.charge.domain == self.domain
            && checkpoint.semantic.charges.shell.domain == self.domain
            && Arc::strong_count(&checkpoint.semantic) == 1;
        if !reuse_shell {
            let count = checkpoint.semantic.program.units.len();
            self.work(
                count
                    .checked_mul(2)
                    .and_then(|n| n.checked_add(18))
                    .ok_or(PublicationError::Capacity)?,
            )?;
            let bytes = shell_bytes(count, count, 0)?;
            self.reserve(bytes)?;
            let base = &self.original.as_ref().unwrap().semantic;
            // Complete shell admission precedes its two owning vectors.
            let mut program = share_program_without_units(&base.program);
            program.units = copy_vector(&base.program.units)?;
            self.fork = Some(Pending {
                program,
                charges: SemanticCharges {
                    shell: Charge {
                        domain: self.domain,
                        bytes,
                    },
                    units: copy_vector(&base.charges.units)?,
                    tables: base.charges.tables,
                    diagnostic_text_bytes: base.charges.diagnostic_text_bytes,
                },
                changes: Vec::new(),
                cell_changes: Vec::new(),
                uses: None,
            });
        }
        self.edits = self.vector(patches.len())?;
        self.changes = self.vector(patches.len())?;
        for (index, patch) in patches.iter().enumerate() {
            self.prepare_unit(index, patch)?;
        }
        // Pay the bounded move/restore path before mutation. No admission is
        // attempted by rollback even after deadline/work exhaustion.
        let undo_work = self.edits.iter().try_fold(0usize, |n, edit| {
            n.checked_add(edit.kinds.len())
                .and_then(|n| n.checked_add(edit.places.len()))
                .and_then(|n| n.checked_add(edit.operands.as_ref().map_or(0, |(_, r)| r.len())))
                .and_then(|n| n.checked_add(2))
                .ok_or(PublicationError::Capacity)
        })?;
        self.work(undo_work.checked_mul(2).ok_or(PublicationError::Capacity)?)?;
        Ok(())
    }

    fn prepare_unit(
        &mut self,
        index: usize,
        patch: &UnitPatch<'_>,
    ) -> Result<(), PublicationError> {
        let domain = self.domain;
        let can_reuse = {
            let (program, charges) = self.parts();
            charges.units[patch.unit.index()].domain == domain
                && program.units[patch.unit.index()].allocation_is_unique()
        };
        let mut kinds = self.vector(patch.operations.len())?;
        let mut places = self.vector(patch.places.len())?;
        let mut new_payload = 0;
        let mut old_payload = 0;
        let mut rebuild = false;
        for edit in patch.operations {
            self.work(
                edit.operands
                    .len()
                    .checked_add(1)
                    .ok_or(PublicationError::Capacity)?,
            )?;
            let old =
                &self.program().units[patch.unit.index()].data().operations[edit.operation.index()];
            rebuild |= self
                .program()
                .unit(patch.unit)
                .unwrap()
                .operands(old.operands)
                .ok_or(PublicationError::InvalidPatch)?
                != edit.operands;
            old_payload = sum(&[old_payload, kind_bytes(&old.kind, can_reuse)?])?;
            let bytes = kind_bytes(edit.kind, false)?;
            if let OperationKind::Allocate {
                kind: AllocationKind::Object(keys) | AllocationKind::Record(keys),
                ..
            } = edit.kind
            {
                self.work(keys.len())?;
            }
            if let OperationKind::Allocate {
                kind: AllocationKind::SpreadArray(spread),
                ..
            } = edit.kind
            {
                self.work(spread.len())?;
            }
            self.reserve(bytes)?;
            new_payload = sum(&[new_payload, bytes])?;
            kinds.push((edit.operation, copy_kind(edit.kind)?));
        }
        for edit in patch.places {
            self.work(1)?;
            places.push((edit.place, edit.replacement.clone()));
        }
        let operands = if rebuild {
            let mut values = self.vector(self.plans[index].operands)?;
            let length = self.program().unit(patch.unit).unwrap().operations.len();
            let mut ranges = self.vector(length)?;
            self.work(
                length
                    .checked_add(self.plans[index].operands)
                    .ok_or(PublicationError::Capacity)?,
            )?;
            let data = self.program().unit(patch.unit).unwrap();
            let mut replacements = patch.operations.iter().peekable();
            for (position, operation) in data.operations.iter().enumerate() {
                let input = if replacements
                    .peek()
                    .is_some_and(|p| p.operation.index() == position)
                {
                    replacements.next().unwrap().operands
                } else {
                    data.operands(operation.operands)
                        .ok_or(PublicationError::InvalidPatch)?
                };
                ranges.push(OperandRange {
                    start: values.len() as u32,
                    len: input.len() as u32,
                });
                values.extend_from_slice(input);
            }
            new_payload = sum(&[new_payload, capacity(&values)?])?;
            old_payload = sum(&[
                old_payload,
                if can_reuse {
                    capacity(&data.operands)?
                } else {
                    bytes::<ValueId>(data.operands.len())?
                },
            ])?;
            Some((values, ranges))
        } else {
            None
        };
        let copied = if can_reuse {
            let (_, charges) = self.parts();
            charges.units[patch.unit.index()]
                .bytes
                .checked_add(new_payload)
                .and_then(|n| n.checked_sub(old_payload))
                .ok_or(PublicationError::Capacity)?;
            self.reused_units += 1;
            None
        } else {
            let data = self.program().unit(patch.unit).unwrap();
            self.work(
                data.operations
                    .len()
                    .checked_add(data.regions.len())
                    .and_then(|count| count.checked_add(data.call_instantiations.len()))
                    .and_then(|n| n.checked_add(12))
                    .ok_or(PublicationError::Capacity)?,
            )?;
            let original_bytes = self.program().units[patch.unit.index()]
                .allocation_bytes()
                .ok_or(PublicationError::Capacity)?;
            self.work(usize::try_from(original_bytes).map_err(|_| PublicationError::Capacity)?)?;
            self.reserve(original_bytes)?;
            let data = copy_unit(self.program().unit(patch.unit).unwrap())?;
            let replacement = WorkingUnit::new(patch.unit, data).freeze();
            let actual = replacement
                .allocation_bytes()
                .ok_or(PublicationError::Capacity)?;
            assert!(actual <= original_bytes);
            actual
                .checked_add(new_payload)
                .and_then(|n| n.checked_sub(old_payload))
                .ok_or(PublicationError::Capacity)?;
            self.release(original_bytes - actual);
            self.copied_units += 1;
            self.copied_bytes = sum(&[self.copied_bytes, actual])?;
            let (program, charges) = self.parts();
            let old = std::mem::replace(&mut program.units[patch.unit.index()], replacement);
            let charge = std::mem::replace(
                &mut charges.units[patch.unit.index()],
                Charge {
                    domain,
                    bytes: actual,
                },
            );
            Some((old, charge))
        };
        self.edits.push(UnitEdit {
            unit: patch.unit,
            previous: patch.expected_revision,
            current: RevisionId::fresh(),
            copied,
            kinds,
            places,
            operands,
            new_payload,
            old_payload,
            applied: false,
        });
        Ok(())
    }

    fn apply(&mut self) {
        let (program, edits) = if let Some(fork) = &mut self.fork {
            (&mut fork.program, &mut self.edits)
        } else {
            (
                &mut Arc::get_mut(&mut self.original.as_mut().unwrap().semantic)
                    .unwrap()
                    .program,
                &mut self.edits,
            )
        };
        for edit in edits {
            edit.swap(&mut program.units[edit.unit.index()]);
            self.changes.push(UnitChange {
                unit: edit.unit,
                previous: edit.previous,
                current: edit.current,
            });
        }
        failpoint(EditPhase::Applied, &mut self.compiler.ledger);
    }

    fn finish_checks(&mut self) -> Result<(), PublicationError> {
        let original = self.original.as_ref().unwrap();
        let program = self
            .fork
            .as_ref()
            .map_or(&original.semantic.program, |fork| &fork.program);
        let edits = FixedUnitEdits::new(
            program,
            &original.semantic.uses,
            &self.changes,
            &mut self.compiler.ledger,
            self.domain,
        )?;
        {
            let mut budget = AllocationBudget::new(Some((&mut self.compiler.ledger, self.domain)));
            self.verified = super::super::verify::verify_fixed_replacements(&edits, &mut budget)
                .map_err(|error| match error {
                    super::super::verify::VerificationError::Allocation(error) => {
                        PublicationError::from(error)
                    }
                    _ => PublicationError::InvalidReplacement,
                })?;
        }
        failpoint(EditPhase::Verified, &mut self.compiler.ledger);
        self.prepared = Some(original.semantic.uses.prepare_fixed_replacements(
            &edits,
            &mut self.compiler.ledger,
            self.domain,
        )?);
        let count = self.prepared.as_ref().unwrap().cell_changes().len();
        self.cell_changes = transaction_vector(
            &mut self.compiler.ledger,
            self.domain,
            &mut self.reserved,
            count,
        )?;
        work(&mut self.compiler.ledger, self.domain, count)?;
        self.cell_changes
            .extend(self.prepared.as_ref().unwrap().cell_changes().map(
                |(cell, previous, current)| CellUseChange {
                    cell,
                    previous,
                    current,
                },
            ));
        work(
            &mut self.compiler.ledger,
            self.domain,
            self.prepared.as_ref().unwrap().validation_work(),
        )?;
        if !self
            .prepared
            .as_ref()
            .unwrap()
            .valid_for(&original.semantic.uses)
        {
            return Err(PublicationError::InvalidReplacement);
        }
        if self.fork.is_some() {
            self.new_index = Some(original.semantic.uses.fork_fixed_replacements(
                &edits,
                self.prepared.take().unwrap(),
                &mut self.compiler.ledger,
                self.domain,
            )?);
        }
        failpoint(EditPhase::Prepared, &mut self.compiler.ledger);
        self.work(0)?;
        Ok(())
    }

    fn commit(mut self) -> SemanticId {
        let index_receipt = self.new_index.as_ref().map_or_else(
            || self.prepared.as_ref().unwrap().receipt(),
            UseIndex::receipt,
        );
        let mut newly_retained_units = 0;
        // All recoverable allocation/admission/semantic checks have finished.
        // Fork publication still creates its fully admitted fixed Arc/Box
        // headers; allocator abort is outside the rollback guarantee. Index
        // installation itself allocates nothing. No user callback runs here.
        while let Some(mut edit) = self.edits.pop() {
            let copied = edit.copied.take();
            let (_, charges) = self.parts();
            let charge = &mut charges.units[edit.unit.index()];
            charge.bytes = charge.bytes + edit.new_payload - edit.old_payload;
            let transfer = if copied.is_some() {
                charge.bytes
            } else {
                edit.new_payload
            };
            newly_retained_units += transfer;
            // Drop old nested buffers before releasing their original charge.
            drop(edit.kinds);
            drop(edit.places);
            drop(edit.operands);
            if let Some((old, old_charge)) = copied {
                self.release(edit.old_payload);
                if old.release_allocation() {
                    old_charge
                        .release(&mut self.compiler.ledger)
                        .expect("replaced old unit charge");
                }
            } else {
                self.compiler
                    .ledger
                    .release(self.domain, edit.old_payload)
                    .expect("replaced old payload charge");
            }
            self.reserved -= transfer;
        }
        let metadata = capacity(&self.changes).unwrap() + capacity(&self.cell_changes).unwrap();
        let shell_new = self
            .fork
            .as_ref()
            .map_or(0, |fork| fork.charges.shell.bytes);
        let receipt = PublicationReceipt {
            logical_work: self.compiler.ledger.work_used(self.domain) - self.start,
            allocated_bytes: shell_new
                + metadata
                + newly_retained_units
                + index_receipt.allocated_bytes
                + self.rewrite_bytes,
            copied_units: self.copied_units,
            reused_units: self.reused_units,
            copied_payload_bytes: self.copied_bytes,
            verified_units: self.verified.units,
            verified_operations: self.verified.operations,
            index: index_receipt,
            adopted_after_frontend: false,
        };
        let origin = self.origin.take().unwrap();
        let result = if let Some(mut fork) = self.fork.take() {
            fork.uses = self.new_index.take();
            fork.charges.shell.bytes += metadata;
            fork.changes = std::mem::take(&mut self.changes);
            fork.cell_changes = std::mem::take(&mut self.cell_changes);
            self.reserved -= shell_new + metadata;
            if self.advance {
                discard_checkpoint(*self.original.take().unwrap(), &mut self.compiler.ledger)
                    .expect("consumed checkpoint charges");
                // publish expects a free-list slot. Replacing an occupied slot
                // leaves that list and the live count unchanged.
                self.compiler.slots[self.base_slot].next_free = self.compiler.free;
                self.compiler.free = Some(self.target_slot);
                self.compiler.live -= 1;
            } else {
                self.compiler.slots[self.base_slot].checkpoint = self.original.take();
            }
            self.compiler
                .publish(self.target_slot, fork, receipt, origin)
        } else {
            let checkpoint = self.original.as_mut().unwrap();
            let semantic = Arc::get_mut(&mut checkpoint.semantic).unwrap();
            // The transaction is now committing; rollback paths retain the old identity.
            semantic.identity = origin.identity;
            semantic.meaning = origin.meaning;
            let old_lineage = std::mem::replace(&mut semantic.lineage, origin.lineage);
            old_lineage.discard(&mut self.compiler.ledger);
            semantic
                .uses
                .install_replacements(self.prepared.take().unwrap(), &mut self.compiler.ledger);
            let old_changes =
                std::mem::replace(&mut checkpoint.changes, std::mem::take(&mut self.changes));
            let old_cells = std::mem::replace(
                &mut checkpoint.cell_changes,
                std::mem::take(&mut self.cell_changes),
            );
            let old_bytes = capacity(&old_changes).unwrap() + capacity(&old_cells).unwrap();
            drop(old_changes);
            drop(old_cells);
            self.compiler
                .ledger
                .release(checkpoint.charge.domain, old_bytes)
                .expect("old checkpoint metadata");
            checkpoint.charge.bytes = checkpoint.charge.bytes + metadata - old_bytes;
            checkpoint.receipt = receipt;
            self.reserved -= metadata;
            let generation = RevisionId::fresh();
            self.compiler.slots[self.base_slot].generation = Some(generation);
            self.compiler.slots[self.base_slot].checkpoint = self.original.take();
            SemanticId {
                store: self.compiler.store,
                slot: self.target_slot,
                generation,
            }
        };
        self.committed = true;
        result
    }
}

impl Drop for Transaction<'_, '_> {
    fn drop(&mut self) {
        if let Some(origin) = self.origin.take() {
            origin.lineage.discard(&mut self.compiler.ledger);
        }
        if let Some(prepared) = self.prepared.take() {
            prepared
                .discard(&mut self.compiler.ledger)
                .expect("prepared use update");
        }
        if let Some(index) = self.new_index.take() {
            index
                .discard(&mut self.compiler.ledger)
                .expect("private index");
        }
        if !self.committed {
            while let Some(mut edit) = self.edits.pop() {
                let (program, charges) = self.parts();
                if edit.applied {
                    edit.swap(&mut program.units[edit.unit.index()]);
                }
                if let Some((old, charge)) = edit.copied.take() {
                    let discarded = std::mem::replace(&mut program.units[edit.unit.index()], old);
                    charges.units[edit.unit.index()] = charge;
                    drop(discarded);
                }
                // New replacement payload is back in undo after restoration.
                drop(edit);
            }
            // Restored fork payload only shares the still-owned original.
            drop(self.fork.take());
            self.compiler.slots[self.base_slot].checkpoint = self.original.take();
        }
        drop(std::mem::take(&mut self.edits));
        drop(std::mem::take(&mut self.plans));
        drop(std::mem::take(&mut self.changes));
        drop(std::mem::take(&mut self.cell_changes));
        self.compiler
            .ledger
            .release(self.domain, self.reserved)
            .expect("edit temporary reservations");
    }
}

fn kind_bytes(kind: &OperationKind, retained: bool) -> Result<u64, PublicationError> {
    match kind {
        OperationKind::Allocate {
            kind: AllocationKind::Record(keys) | AllocationKind::Object(keys),
            ..
        } => {
            if retained {
                capacity(keys)
            } else {
                bytes::<StringId>(keys.len())
            }
        }
        OperationKind::Allocate {
            kind: AllocationKind::SpreadArray(spread),
            ..
        } => {
            if retained {
                capacity(spread)
            } else {
                bytes::<bool>(spread.len())
            }
        }
        _ => Ok(0),
    }
}

/// Caller owns admission for these exact-capacity copies, including nested
/// kind/region payload. Unlike Vec::clone the allocation failure is returned.
fn copy_vector<T: Clone>(values: &[T]) -> Result<Vec<T>, PublicationError> {
    let mut copy = VectorLayout::new(values.len())?.allocate()?;
    copy.extend_from_slice(values);
    Ok(copy)
}
fn copy_kind(kind: &OperationKind) -> Result<OperationKind, PublicationError> {
    Ok(match kind {
        OperationKind::Allocate {
            identity,
            kind: AllocationKind::Record(keys),
        } => OperationKind::Allocate {
            identity: *identity,
            kind: AllocationKind::Record(copy_vector(keys)?),
        },
        OperationKind::Allocate {
            identity,
            kind: AllocationKind::Object(keys),
        } => OperationKind::Allocate {
            identity: *identity,
            kind: AllocationKind::Object(copy_vector(keys)?),
        },
        OperationKind::Allocate {
            identity,
            kind: AllocationKind::SpreadArray(spread),
        } => OperationKind::Allocate {
            identity: *identity,
            kind: AllocationKind::SpreadArray(copy_vector(spread)?),
        },
        _ => kind.clone(),
    })
}
fn copy_unit(unit: &UnitData) -> Result<UnitData, PublicationError> {
    let mut operations = VectorLayout::new(unit.operations.len())?.allocate()?;
    for operation in &unit.operations {
        operations.push(Operation {
            kind: copy_kind(&operation.kind)?,
            operands: operation.operands,
            result: operation.result,
            region: operation.region,
            origin: operation.origin,
            span: operation.span,
        });
    }
    let mut regions = VectorLayout::new(unit.regions.len())?.allocate()?;
    for region in &unit.regions {
        regions.push(Region {
            parent: region.parent,
            operations: copy_vector(&region.operations)?,
            result: region.result,
            span: region.span,
        });
    }
    let mut call_instantiations = VectorLayout::new(unit.call_instantiations.len())?.allocate()?;
    for instance in &unit.call_instantiations {
        call_instantiations.push(CallInstantiation {
            declaration: instance.declaration,
            arguments: copy_vector(&instance.arguments)?,
            signature: instance.signature,
        });
    }
    Ok(UnitData {
        kind: unit.kind,
        suspension: unit.suspension,
        host_class: unit.host_class,
        module: unit.module,
        instantiation_prefix: unit.instantiation_prefix,
        function_name: unit.function_name,
        callable_type: unit.callable_type,
        parameters: copy_vector(&unit.parameters)?,
        captures: copy_vector(&unit.captures)?,
        entry: unit.entry,
        operations,
        operands: copy_vector(&unit.operands)?,
        values: copy_vector(&unit.values)?,
        regions,
        places: copy_vector(&unit.places)?,
        calls: copy_vector(&unit.calls)?,
        call_instantiations,
        call_arguments: copy_vector(&unit.call_arguments)?,
    })
}
fn share_program_without_units<'src>(program: &Program<'src>) -> Program<'src> {
    Program {
        tables_revision: program.tables_revision,
        units: Vec::new(),
        cells: program.cells.clone(),
        types: program.types.clone(),
        strings: program.strings.clone(),
        structs: program.structs.clone(),
        enums: program.enums.clone(),
        fields: program.fields.clone(),
        classes: program.classes.clone(),
        exports: program.exports.clone(),
        initialization: program.initialization.clone(),
        modules: program.modules.clone(),
        entry: program.entry,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditPhase {
    Applied,
    Verified,
    Prepared,
}
#[cfg(test)]
thread_local! { static PANIC_AT: std::cell::Cell<Option<EditPhase>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
thread_local! { static DEADLINE_AT: std::cell::Cell<Option<EditPhase>> = const { std::cell::Cell::new(None) }; }
#[cfg(test)]
pub(super) fn inject_rewrite_failure(phase: u8, panic: bool) {
    let phase = match phase {
        0 => EditPhase::Applied,
        1 => EditPhase::Verified,
        2 => EditPhase::Prepared,
        _ => panic!("unknown edit failure phase"),
    };
    if panic {
        PANIC_AT.with(|selected| selected.set(Some(phase)));
    } else {
        DEADLINE_AT.with(|selected| selected.set(Some(phase)));
    }
}
fn failpoint(phase: EditPhase, ledger: &mut BudgetLedger) {
    #[cfg(test)]
    PANIC_AT.with(|selected| {
        if selected.get() == Some(phase) {
            selected.set(None);
            panic!("injected checked edit unwind");
        }
    });
    #[cfg(test)]
    DEADLINE_AT.with(|selected| {
        if selected.get() == Some(phase) {
            selected.set(None);
            ledger.set_deadline_elapsed_for_test(std::time::Duration::from_millis(
                ledger.wall_time_ms().unwrap(),
            ));
        }
    });
    #[cfg(not(test))]
    let _ = (phase, ledger);
}

#[cfg(test)]
#[path = "exclusive_publication_tests.rs"]
mod tests;

// Keep the transaction's existing reservation owner while allowing disjoint
// field borrows during checked publication. This creates no separate allocator.
fn transaction_vector<T>(
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
    reserved: &mut u64,
    count: usize,
) -> Result<Vec<T>, PublicationError> {
    work(ledger, domain, 1)?;
    let layout = VectorLayout::<T>::new(count)?;
    let next = reserved
        .checked_add(layout.bytes())
        .ok_or(PublicationError::Capacity)?;
    ledger.retain(domain, layout.bytes())?;
    *reserved = next;
    Ok(layout.allocate()?)
}
