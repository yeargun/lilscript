//! The edit kernel of the program rules (plan M5.1, architecture §8.1).
//!
//! Rules run before publication, on a program the build exclusively owns, so
//! an edit is an in-place change to a unit's arenas. An edit that removes an
//! operation or a region only detaches it; `compact` then drops everything
//! the unit no longer owns and renumbers what it keeps, which is what the
//! verifier requires ("unowned semantic storage"). A rule commits its edits:
//! every touched unit is compacted and stamped with a fresh revision, and the
//! derived views, which describe the old program, are dropped.

use super::super::ids::RevisionId;
use super::super::views::ProgramViews;
use super::super::*;
use std::collections::HashMap;
use std::sync::Arc;

use super::{storage, RuleError};
use crate::output_budget::AllocationClass::{Retained, Scratch};
use crate::output_budget::{AllocationBudget, AllocationError};

pub(super) struct Editor<'src> {
    program: Program<'src>,
    touched: Vec<bool>,
    // A rule may retain a body while grafting it. Keep its old reservation
    // until those readers have dropped, then retire it at commit.
    retired: Vec<storage::Retired<'src>>,
    tables_changed: bool,
}

impl<'src> Editor<'src> {
    pub(super) fn new_in(
        program: Program<'src>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, RuleError> {
        let touched = budget.filled(Scratch, program.units.len(), false)?;
        Ok(Self {
            program,
            touched,
            retired: Vec::new(),
            tables_changed: false,
        })
    }
    #[cfg(test)]
    pub(super) fn new(program: Program<'src>) -> Self {
        Self::new_in(program, &mut AllocationBudget::new(None)).expect("inspection editor")
    }
    pub(super) fn program(&self) -> &Program<'src> {
        &self.program
    }

    fn unique_unit(
        &mut self,
        unit: UnitId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), RuleError> {
        let index = unit.index();
        self.touched[index] = true;
        if !self.program.units[index].allocation_is_unique() {
            budget.reserve_vec(Scratch, &mut self.retired, 1)?;
            let copied = budget.retained_phase(|budget| {
                let data = storage::unit(self.program.units[index].data(), budget)?;
                storage::freeze(unit, data, budget)
            })?;
            let old = std::mem::replace(&mut self.program.units[index], copied);
            self.retired.push(storage::Retired::Unit(old));
        }
        Ok(())
    }
    pub(super) fn unit_mut_in(
        &mut self,
        unit: UnitId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<&mut UnitData, RuleError> {
        self.unique_unit(unit, budget)?;
        Ok(self.program.units[unit.index()]
            .unique_edit()
            .ok_or("an unshared unit payload is not editable")?
            .0)
    }
    #[cfg(test)]
    pub(super) fn unit_mut(&mut self, unit: UnitId) -> &mut UnitData {
        self.unit_mut_in(unit, &mut AllocationBudget::new(None))
            .expect("inspection edit")
    }
    fn unique_cells(&mut self, budget: &mut AllocationBudget<'_>) -> Result<(), RuleError> {
        if Arc::strong_count(&self.program.cells) == 1 {
            return Ok(());
        }
        budget.reserve_vec(Scratch, &mut self.retired, 1)?;
        let copy = budget.retained_phase(|budget| {
            let mut rows = budget.vector(Retained, self.program.cells.len())?;
            for cell in self.program.cells.iter() {
                rows.push(storage::cell(cell, budget)?);
            }
            storage::table(rows, budget)
        })?;
        let old = std::mem::replace(&mut self.program.cells, copy);
        self.retired.push(storage::Retired::Cells(old));
        Ok(())
    }
    fn unique_types(&mut self, budget: &mut AllocationBudget<'_>) -> Result<(), RuleError> {
        if Arc::strong_count(&self.program.types) == 1 {
            return Ok(());
        }
        budget.reserve_vec(Scratch, &mut self.retired, 1)?;
        let copy = budget.retained_phase(|budget| {
            let mut rows = budget.vector(Retained, self.program.types.len())?;
            for ty in self.program.types.iter() {
                rows.push(storage::ty(ty, budget)?);
            }
            storage::table(rows, budget)
        })?;
        let old = std::mem::replace(&mut self.program.types, copy);
        self.retired.push(storage::Retired::Types(old));
        Ok(())
    }
    fn unique_strings(&mut self, budget: &mut AllocationBudget<'_>) -> Result<(), RuleError> {
        if Arc::strong_count(&self.program.strings) == 1 {
            return Ok(());
        }
        budget.reserve_vec(Scratch, &mut self.retired, 1)?;
        let copy = budget.retained_phase(|budget| {
            let mut rows = budget.vector(Retained, self.program.strings.len())?;
            for text in self.program.strings.iter() {
                rows.push(budget.string_value(Retained, text)?);
            }
            storage::table(rows, budget)
        })?;
        let old = std::mem::replace(&mut self.program.strings, copy);
        self.retired.push(storage::Retired::Strings(old));
        Ok(())
    }
    pub(super) fn unit_and_cells_in(
        &mut self,
        unit: UnitId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(&mut UnitData, &mut [Cell]), RuleError> {
        self.unique_unit(unit, budget)?;
        self.unique_cells(budget)?;
        self.tables_changed = true;
        self.program.tables_revision = RevisionId::fresh();
        let cells =
            Arc::get_mut(&mut self.program.cells).ok_or("an edited cells table is shared")?;
        let data = self.program.units[unit.index()]
            .unique_edit()
            .ok_or("an edited unit became shared")?
            .0;
        Ok((data, cells.as_mut_slice()))
    }
    #[cfg(test)]
    pub(super) fn unit_and_cells(&mut self, unit: UnitId) -> (&mut UnitData, &mut [Cell]) {
        self.unit_and_cells_in(unit, &mut AllocationBudget::new(None))
            .expect("inspection table edit")
    }
    fn retire(&mut self, budget: &mut AllocationBudget<'_>) -> Result<(), RuleError> {
        let mut index = 0;
        while index < self.retired.len() {
            if !self.retired[index].unique() {
                index += 1;
                continue;
            }
            let bytes = self.retired[index].bytes(budget)?;
            drop(self.retired.swap_remove(index));
            budget.release(Retained, bytes)?;
        }
        Ok(())
    }
    pub(super) fn commit_in(&mut self, budget: &mut AllocationBudget<'_>) -> Result<(), RuleError> {
        let changed = self.tables_changed || self.touched.iter().any(|touched| *touched);
        let mut remaps: Vec<Option<RegionRemap>> = Vec::new();
        for index in 0..self.touched.len() {
            if !std::mem::take(&mut self.touched[index]) {
                continue;
            }
            let (data, revision) = self.program.units[index]
                .unique_edit()
                .ok_or("an edited unit payload became shared")?;
            *revision = RevisionId::fresh();
            let remap = compact_in(data, budget)?;
            if remap.moved {
                if remaps.is_empty() {
                    remaps = budget.vector(Scratch, self.program.units.len())?;
                    remaps.resize_with(self.program.units.len(), || None);
                }
                remaps[index] = Some(remap);
            } else {
                storage::release_vec(remap.map, Retained, budget)?;
            }
        }
        if !remaps.is_empty()
            && self.program.cells.iter().any(|cell| {
                remaps
                    .get(cell.owner.index())
                    .and_then(Option::as_ref)
                    .is_some_and(|remap| remap.region(cell.region) != cell.region)
            })
        {
            self.unique_cells(budget)?;
            self.program.tables_revision = RevisionId::fresh();
            for cell in Arc::get_mut(&mut self.program.cells)
                .ok_or("an edited cells table is shared")?
                .iter_mut()
            {
                if let Some(Some(remap)) = remaps.get(cell.owner.index()) {
                    cell.region = remap.region(cell.region);
                }
            }
        }
        for remap in &mut remaps {
            if let Some(remap) = remap.take() {
                storage::release_vec(remap.map, Retained, budget)?;
            }
        }
        storage::release_vec(remaps, Scratch, budget)?;
        if changed {
            self.program.views.invalidate();
        }
        self.tables_changed = false;
        #[cfg(test)]
        if super::COLD_RULE_VIEWS.with(std::cell::Cell::get) {
            self.program.views = ProgramViews::default();
        }
        self.retire(budget)
    }
    #[cfg(test)]
    pub(super) fn commit(&mut self) -> Result<(), RuleError> {
        self.commit_in(&mut AllocationBudget::new(None))
    }
    pub(super) fn handle(&self, unit: UnitId) -> FrozenUnit {
        self.program.units[unit.index()].clone()
    }

    /// The supplied unit and its nested payload already belong to this budget.
    pub(super) fn add_unit_in(
        &mut self,
        data: UnitData,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<UnitId, RuleError> {
        let id = UnitId::from_index(self.program.units.len()).ok_or("unit capacity")?;
        let frozen = storage::freeze(id, data, budget)?;
        budget.push(Retained, &mut self.program.units, frozen)?;
        budget.push(Scratch, &mut self.touched, true)?;
        self.tables_changed = true;
        self.program.tables_revision = RevisionId::fresh();
        Ok(id)
    }
    pub(super) fn intern_type_in(
        &mut self,
        ty: &crate::check::Type<'src>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<TypeId, RuleError> {
        if let Some(index) = self.program.types.iter().position(|known| known == ty) {
            return TypeId::from_index(index).ok_or("type capacity".into());
        }
        self.unique_types(budget)?;
        let id = TypeId::from_index(self.program.types.len()).ok_or("type capacity")?;
        let ty = storage::ty(ty, budget)?;
        budget.push(
            Retained,
            Arc::get_mut(&mut self.program.types).ok_or("an edited types table is shared")?,
            ty,
        )?;
        self.tables_changed = true;
        self.program.tables_revision = RevisionId::fresh();
        Ok(id)
    }
    #[cfg(test)]
    pub(super) fn intern_type(
        &mut self,
        ty: crate::check::Type<'src>,
    ) -> Result<TypeId, RuleError> {
        self.intern_type_in(&ty, &mut AllocationBudget::new(None))
    }
    pub(super) fn intern_string_in(
        &mut self,
        value: &StringValue,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<StringId, RuleError> {
        if let Some(index) = self.program.strings.iter().position(|known| known == value) {
            return StringId::from_index(index).ok_or("string capacity".into());
        }
        self.unique_strings(budget)?;
        let id = StringId::from_index(self.program.strings.len()).ok_or("string capacity")?;
        let value = budget.string_value(Retained, value)?;
        budget.push(
            Retained,
            Arc::get_mut(&mut self.program.strings).ok_or("an edited strings table is shared")?,
            value,
        )?;
        self.tables_changed = true;
        self.program.tables_revision = RevisionId::fresh();
        Ok(id)
    }
    #[cfg(test)]
    pub(super) fn intern_string(&mut self, value: &StringValue) -> Result<StringId, RuleError> {
        self.intern_string_in(value, &mut AllocationBudget::new(None))
    }
    /// Cell.name is an admitted payload; scalarization and inlining construct
    /// or copy it through the same storage owner before transferring it here.
    pub(super) fn add_cell_in(
        &mut self,
        cell: Cell,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<CellId, RuleError> {
        self.unique_cells(budget)?;
        let id = CellId::from_index(self.program.cells.len()).ok_or("cell capacity")?;
        budget.push(
            Retained,
            Arc::get_mut(&mut self.program.cells).ok_or("an edited cells table is shared")?,
            cell,
        )?;
        self.tables_changed = true;
        self.program.tables_revision = RevisionId::fresh();
        Ok(id)
    }
    #[cfg(test)]
    pub(super) fn add_cell(&mut self, cell: Cell) -> Result<CellId, RuleError> {
        self.add_cell_in(cell, &mut AllocationBudget::new(None))
    }
    pub(super) fn finish_in(
        mut self,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Program<'src>, RuleError> {
        self.commit_in(budget)?;
        self.program.views = ProgramViews::default();
        self.retire(budget)?;
        if budget.is_accounted() && !self.retired.is_empty() {
            return Err("a source edit retained a reader beyond its owner".into());
        }
        drop(self.retired.drain(..));
        storage::release_vec(self.retired, Scratch, budget)?;
        storage::release_vec(self.touched, Scratch, budget)?;
        Ok(self.program)
    }
    #[cfg(test)]
    pub(super) fn finish(self) -> Result<Program<'src>, RuleError> {
        self.finish_in(&mut AllocationBudget::new(None))
    }
}

/// Removes `operation` from its region. It and every region it owns leave
/// at compaction.
pub(super) fn detach(data: &mut UnitData, operation: OpId) {
    let region = data.operations[operation.index()].region;
    let list = &mut data.regions[region.index()].operations;
    if let Some(position) = list.iter().position(|op| *op == operation) {
        list.remove(position);
    }
}

/// Makes `operation` a constant; its operands are released.
pub(super) fn replace_kind(
    data: &mut UnitData,
    operation: OpId,
    kind: OperationKind,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), RuleError> {
    let bytes = storage::allocation_bytes(&data.operations[operation.index()].kind)?;
    drop(std::mem::replace(
        &mut data.operations[operation.index()].kind,
        kind,
    ));
    budget.release(Retained, bytes)?;
    Ok(())
}
pub(super) fn make_constant_in(
    data: &mut UnitData,
    operation: OpId,
    constant: Constant,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), RuleError> {
    replace_kind(data, operation, OperationKind::Constant(constant), budget)?;
    data.operations[operation.index()].operands = OperandRange { start: 0, len: 0 };
    Ok(())
}

/// Keep a proved value with the replaced operation's result type. A nullable
/// value may be known present, or a selected arm may have a narrower type than
/// its join. Replacing its ValueId would lose that checked type; an evaluated
/// value place is the existing typed view used by source narrowing. Its input
/// has already run and is never reloaded from mutable storage.
/// The caller proves the runtime value is admissible at the result type.
pub(super) fn make_value_view_in(
    data: &mut UnitData,
    operation: OpId,
    value: ValueId,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), RuleError> {
    let place = PlaceId::from_index(data.places.len()).ok_or("value view place capacity")?;
    budget.push(Retained, &mut data.places, Place::Value(value))?;
    replace_kind(data, operation, OperationKind::Load(place), budget)?;
    let operation = &mut data.operations[operation.index()];
    operation.operands = OperandRange { start: 0, len: 0 };
    Ok(())
}

/// Makes `operation` a block of `region`, one of its child regions; the
/// other child regions and the operands are released. A block keeps the
/// region's lexical scope, as the branch it replaces did.
pub(super) fn make_block_in(
    data: &mut UnitData,
    operation: OpId,
    region: RegionId,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), RuleError> {
    replace_kind(data, operation, OperationKind::Block(region), budget)?;
    data.operations[operation.index()].operands = OperandRange { start: 0, len: 0 };
    Ok(())
}

/// Every use of `from` becomes a use of `to`. The caller guarantees that
/// `to` is available at each use of `from`.
pub(super) fn substitute(data: &mut UnitData, from: ValueId, to: ValueId) {
    let swap = |value: &mut ValueId| {
        if *value == from {
            *value = to;
        }
    };
    data.operands.iter_mut().for_each(swap);
    for argument in &mut data.call_arguments {
        if let CallArgument::Value(value) | CallArgument::Spread(value) = argument {
            swap(value);
        }
    }
    for place in &mut data.places {
        match place {
            Place::Value(value)
            | Place::Member {
                receiver: value, ..
            }
            | Place::ClassField {
                receiver: value, ..
            } => swap(value),
            Place::Index { receiver, key } => {
                swap(receiver);
                swap(key);
            }
            Place::Cell(_) | Place::Field { .. } => {}
        }
    }
    for call in &mut data.calls {
        match &mut call.target {
            CallTarget::Value { callee, .. } => swap(callee),
            CallTarget::Intrinsic {
                receiver: Some(receiver),
                ..
            } => swap(receiver),
            _ => {}
        }
    }
    for region in &mut data.regions {
        if let Some(result) = &mut region.result {
            swap(result);
        }
    }
}

/// Moves the operations of `region` into the region that holds `at`, in
/// `at`'s place; `at` and its other child regions are released. The cells
/// the moved region declared move with its operations. The caller
/// substitutes `at`'s result.
pub(super) fn splice_in(
    data: &mut UnitData,
    cells: &mut [Cell],
    unit: UnitId,
    at: OpId,
    region: RegionId,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), RuleError> {
    let parent = data.operations[at.index()].region;
    let additional = data.regions[region.index()]
        .operations
        .len()
        .saturating_sub(1);
    budget.reserve_vec(
        Retained,
        &mut data.regions[parent.index()].operations,
        additional,
    )?;
    let moved = std::mem::take(&mut data.regions[region.index()].operations);
    for op in &moved {
        data.operations[op.index()].region = parent;
        let operations = &data.operations;
        let regions = &mut data.regions;
        for child in operations[op.index()].kind.child_regions() {
            regions[child.index()].parent = Some(parent);
        }
    }
    let list = &mut data.regions[parent.index()].operations;
    if let Some(position) = list.iter().position(|op| *op == at) {
        list.splice(position..=position, moved.iter().copied());
    }
    for cell in cells
        .iter_mut()
        .filter(|cell| cell.owner == unit && cell.region == region)
    {
        cell.region = parent;
    }
    storage::release_vec(moved, Retained, budget)?;
    Ok(())
}

/// Where each region of a compacted unit went. A removed region maps to its
/// nearest surviving ancestor.
pub(super) struct RegionRemap {
    map: Vec<RegionId>,
    moved: bool,
}

impl RegionRemap {
    pub(super) fn region(&self, old: RegionId) -> RegionId {
        self.map.get(old.index()).copied().unwrap_or(old)
    }
}

struct Remaps<'a> {
    operations: Vec<Option<OpId>>,
    values: Vec<Option<ValueId>>,
    regions: Vec<Option<RegionId>>,
    places: Vec<Option<PlaceId>>,
    calls: Vec<Option<CallId>>,
    instantiations: Vec<Option<CallInstantiationId>>,
    /// A graft's cells: what the copied body declared, cloned for the copy.
    cells: Option<&'a HashMap<CellId, CellId>>,
    /// A graft's parameters that read their argument: a place naming one
    /// reads the argument value.
    forwards: Option<&'a HashMap<CellId, ValueId>>,
    /// A graft's allocation sites, renumbered past the receiver's own.
    allocations: Vec<(AllocationId, AllocationId)>,
}

impl Remaps<'_> {
    fn op(&self, id: OpId) -> Result<OpId, &'static str> {
        self.operations[id.index()].ok_or("a kept region lists a removed operation")
    }
    fn value(&self, id: ValueId) -> Result<ValueId, &'static str> {
        self.values
            .get(id.index())
            .copied()
            .flatten()
            .ok_or("a kept operation reads a removed value")
    }
    fn region(&self, id: RegionId) -> Result<RegionId, &'static str> {
        self.regions[id.index()].ok_or("a kept operation owns a removed region")
    }
    fn place(&self, id: PlaceId) -> Result<PlaceId, &'static str> {
        self.places[id.index()].ok_or("a kept operation uses a removed place")
    }
    fn call(&self, id: CallId) -> Result<CallId, &'static str> {
        self.calls[id.index()].ok_or("a kept operation prepares a removed call")
    }
    fn cell(&self, id: CellId) -> CellId {
        self.cells
            .and_then(|cells| cells.get(&id))
            .copied()
            .unwrap_or(id)
    }

    fn kind(&self, kind: &OperationKind) -> Result<OperationKind, &'static str> {
        use OperationKind as Op;
        Ok(match kind {
            Op::Load(place) => Op::Load(self.place(*place)?),
            Op::CheckPlace(place) => Op::CheckPlace(self.place(*place)?),
            Op::Store(place) => Op::Store(self.place(*place)?),
            Op::PrepareCall(call) => Op::PrepareCall(self.call(*call)?),
            Op::Call(call) => Op::Call(self.call(*call)?),
            Op::PrepareReference { call, position } => Op::PrepareReference {
                call: self.call(*call)?,
                position: *position,
            },
            Op::ShortCircuit { kind, right } => Op::ShortCircuit {
                kind: *kind,
                right: self.region(*right)?,
            },
            Op::Select { yes, no } => Op::Select {
                yes: self.region(*yes)?,
                no: self.region(*no)?,
            },
            Op::If { yes, no } => Op::If {
                yes: self.region(*yes)?,
                no: no.map(|no| self.region(no)).transpose()?,
            },
            Op::Loop { test, body, update } => Op::Loop {
                test: self.region(*test)?,
                body: self.region(*body)?,
                update: self.region(*update)?,
            },
            Op::Try {
                body,
                catch,
                finally,
            } => Op::Try {
                body: self.region(*body)?,
                catch: catch
                    .map(|(cell, region)| {
                        self.region(region)
                            .map(|region| (cell.map(|cell| self.cell(cell)), region))
                    })
                    .transpose()?,
                finally: finally.map(|region| self.region(region)).transpose()?,
            },
            Op::Block(region) => Op::Block(self.region(*region)?),
            Op::ForIn { key, body } => Op::ForIn {
                key: self.cell(*key),
                body: self.region(*body)?,
            },
            Op::ForOf { item, body } => Op::ForOf {
                item: self.cell(*item),
                body: self.region(*body)?,
            },
            Op::Initialize(cell) => Op::Initialize(self.cell(*cell)),
            Op::Declare(cell) => Op::Declare(self.cell(*cell)),
            Op::Allocate { .. } => return Err("allocation payload requires admitted copying"),
            other => other.clone(),
        })
    }

    fn place_payload(&self, place: &Place) -> Result<Place, &'static str> {
        Ok(match place {
            Place::Cell(cell) => match self.forwards.and_then(|forwards| forwards.get(cell)) {
                Some(value) => Place::Value(*value),
                None => Place::Cell(self.cell(*cell)),
            },
            Place::Value(value) => Place::Value(self.value(*value)?),
            Place::Field { base, field } => Place::Field {
                base: self.place(*base)?,
                field: *field,
            },
            Place::Member { receiver, key } => Place::Member {
                receiver: self.value(*receiver)?,
                key: *key,
            },
            Place::ClassField { receiver, field } => Place::ClassField {
                receiver: self.value(*receiver)?,
                field: *field,
            },
            Place::Index { receiver, key } => Place::Index {
                receiver: self.value(*receiver)?,
                key: self.value(*key)?,
            },
        })
    }

    fn argument(&self, argument: &CallArgument) -> Result<CallArgument, &'static str> {
        Ok(match argument {
            CallArgument::Value(value) => CallArgument::Value(self.value(*value)?),
            CallArgument::Reference(place) => CallArgument::Reference(self.place(*place)?),
            CallArgument::Spread(value) => CallArgument::Spread(self.value(*value)?),
        })
    }

    fn target(&self, target: &CallTarget) -> Result<CallTarget, &'static str> {
        Ok(match target {
            CallTarget::Value { callee, invocation } => CallTarget::Value {
                callee: self.value(*callee)?,
                invocation: *invocation,
            },
            CallTarget::Reference { place } => CallTarget::Reference {
                place: self.place(*place)?,
            },
            CallTarget::Builtin(builtin) => CallTarget::Builtin(*builtin),
            CallTarget::Intrinsic {
                operation,
                receiver,
            } => CallTarget::Intrinsic {
                operation: *operation,
                receiver: receiver.map(|value| self.value(value)).transpose()?,
            },
        })
    }
}

/// What a unit's entry region owns: its regions, the operations they list
/// (less those `skip` leaves out, with the regions they own), and the values,
/// calls, instantiations and places those operations use.
struct Keep {
    regions: Vec<bool>,
    operations: Vec<bool>,
    calls: Vec<bool>,
    places: Vec<bool>,
    instantiations: Vec<bool>,
    values: Vec<bool>,
}

fn ownership_in(
    data: &UnitData,
    mut skip: impl FnMut(OpId, &Operation) -> bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<Keep, RuleError> {
    budget.retained_phase(|budget| {
        let mut region_keep = budget.filled(Retained, data.regions.len(), false)?;
        let mut op_keep = budget.filled(Retained, data.operations.len(), false)?;
        let mut stack = budget.copy_slice(Scratch, &[data.entry])?;
        while let Some(region) = stack.pop() {
            let seen = region_keep
                .get_mut(region.index())
                .ok_or("a kept operation owns a missing region")?;
            if std::mem::replace(seen, true) {
                return Err("a region has two owners".into());
            }
            for &op in &data.regions[region.index()].operations {
                let operation = data
                    .operations
                    .get(op.index())
                    .ok_or("a region lists a missing operation")?;
                if skip(op, operation) {
                    continue;
                }
                op_keep[op.index()] = true;
                storage::extend(&mut stack, operation.kind.child_regions(), Scratch, budget)?;
            }
        }
        let mut call_keep = budget.filled(Retained, data.calls.len(), false)?;
        for (index, op) in data.operations.iter().enumerate() {
            if let (true, OperationKind::Call(call)) = (op_keep[index], &op.kind) {
                call_keep[call.index()] = true;
            }
        }
        let mut place_keep = budget.filled(Retained, data.places.len(), false)?;
        for (index, op) in data.operations.iter().enumerate() {
            if !op_keep[index] {
                continue;
            }
            match op.kind {
                OperationKind::Load(place)
                | OperationKind::Store(place)
                | OperationKind::CheckPlace(place) => place_keep[place.index()] = true,
                OperationKind::PrepareCall(call) | OperationKind::PrepareReference { call, .. }
                    if !call_keep[call.index()] =>
                {
                    return Err("a kept preparation has no kept call".into());
                }
                _ => {}
            }
        }
        let mut instantiation_keep =
            budget.filled(Retained, data.call_instantiations.len(), false)?;
        for (index, call) in data.calls.iter().enumerate() {
            if !call_keep[index] {
                continue;
            }
            if let CallTarget::Reference { place } = call.target {
                place_keep[place.index()] = true;
            }
            for argument in data
                .arguments(call.arguments)
                .ok_or("invalid argument range")?
            {
                if let CallArgument::Reference(place) = argument {
                    place_keep[place.index()] = true;
                }
            }
            if let Some(instantiation) = call.contract.instantiation {
                instantiation_keep[instantiation.index()] = true;
            }
        }
        // A field projection names an earlier place: one descending pass keeps
        // every base a kept projection reaches.
        for index in (0..data.places.len()).rev() {
            if let (true, Place::Field { base, .. }) = (place_keep[index], &data.places[index]) {
                place_keep[base.index()] = true;
            }
        }
        let mut value_keep = budget.filled(Retained, data.values.len(), false)?;
        for (index, op) in data.operations.iter().enumerate() {
            if let (true, Some(result)) = (op_keep[index], op.result) {
                value_keep[result.index()] = true;
            }
        }
        Ok(Keep {
            regions: region_keep,
            operations: op_keep,
            calls: call_keep,
            places: place_keep,
            instantiations: instantiation_keep,
            values: value_keep,
        })
    })
}

/// Drops the storage the entry region no longer owns: operations, their
/// values, regions, calls with their arguments and instantiations, and
/// places. What is kept keeps its order and is renumbered densely.
struct CompactPlan {
    keep: Keep,
    remaps: Remaps<'static>,
    operands: Vec<ValueId>,
    operand_ranges: Vec<OperandRange>,
    arguments: Vec<CallArgument>,
    argument_ranges: Vec<ArgumentRange>,
    regions: Vec<RegionId>,
    released: u64,
}

fn renumber_in<T: Copy>(
    keep: &[bool],
    id: impl Fn(usize) -> Option<T>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<T>>, AllocationError> {
    let mut next = 0;
    storage::collect(
        keep.iter().map(|&kept| {
            kept.then(|| {
                let value = id(next).expect("a compacted arena fits its old capacity");
                next += 1;
                value
            })
        }),
        Retained,
        budget,
    )
}

fn compact_plan(
    data: &UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<CompactPlan, RuleError> {
    let keep = ownership_in(data, |_, _| false, budget)?;
    let everything = |keep: &[bool]| keep.iter().all(|&kept| kept);
    let owned_operands = data.operations.iter().try_fold(0usize, |total, op| {
        total
            .checked_add(op.operands.len as usize)
            .ok_or(AllocationError::Capacity)
    })?;
    let owned_arguments = data.calls.iter().try_fold(0usize, |total, call| {
        total
            .checked_add(call.arguments.len as usize)
            .ok_or(AllocationError::Capacity)
    })?;
    if everything(&keep.operations)
        && everything(&keep.regions)
        && everything(&keep.calls)
        && everything(&keep.places)
        && everything(&keep.instantiations)
        && everything(&keep.values)
        && owned_operands == data.operands.len()
        && owned_arguments == data.call_arguments.len()
    {
        // No backing escapes this temporary input, including the ownership maps.
        // Keep the maps in a plan so their buffers drop before its detached charge.
        return Ok(CompactPlan {
            keep,
            remaps: Remaps {
                operations: Vec::new(),
                values: Vec::new(),
                regions: Vec::new(),
                places: Vec::new(),
                calls: Vec::new(),
                instantiations: Vec::new(),
                cells: None,
                forwards: None,
                allocations: Vec::new(),
            },
            operands: Vec::new(),
            operand_ranges: Vec::new(),
            arguments: Vec::new(),
            argument_ranges: Vec::new(),
            regions: Vec::new(),
            released: 0,
        });
    }
    let remaps = Remaps {
        operations: renumber_in(&keep.operations, OpId::from_index, budget)?,
        values: renumber_in(&keep.values, ValueId::from_index, budget)?,
        regions: renumber_in(&keep.regions, RegionId::from_index, budget)?,
        places: renumber_in(&keep.places, PlaceId::from_index, budget)?,
        calls: renumber_in(&keep.calls, CallId::from_index, budget)?,
        instantiations: renumber_in(
            &keep.instantiations,
            CallInstantiationId::from_index,
            budget,
        )?,
        cells: None,
        forwards: None,
        allocations: Vec::new(),
    };
    // Rewritten operand/argument ranges need not be monotone. Stage their
    // values once rather than overwriting a later operation's original range.
    let mut operands = Vec::new();
    let mut operand_ranges = budget.vector(Retained, data.operations.len())?;
    let mut released = 0u64;
    for (index, op) in data.operations.iter().enumerate() {
        let start = u32::try_from(operands.len()).map_err(|_| AllocationError::Capacity)?;
        operand_ranges.push(OperandRange {
            start,
            len: op.operands.len,
        });
        if keep.operations[index] {
            for &value in data.operands(op.operands).ok_or("invalid operand range")? {
                let value = remaps.value(value)?;
                budget.push(Retained, &mut operands, value)?;
            }
        } else {
            released = released
                .checked_add(storage::allocation_bytes(&op.kind)?)
                .ok_or(AllocationError::Capacity)?;
        }
    }
    let mut arguments = Vec::new();
    let mut argument_ranges = budget.vector(Retained, data.calls.len())?;
    for (index, call) in data.calls.iter().enumerate() {
        let start = u32::try_from(arguments.len()).map_err(|_| AllocationError::Capacity)?;
        argument_ranges.push(ArgumentRange {
            start,
            len: call.arguments.len,
        });
        if keep.calls[index] {
            for arg in data
                .arguments(call.arguments)
                .ok_or("invalid argument range")?
            {
                let arg = remaps.argument(arg)?;
                budget.push(Retained, &mut arguments, arg)?;
            }
        }
    }
    for (index, region) in data.regions.iter().enumerate() {
        if !keep.regions[index] {
            released = released
                .checked_add(crate::output_budget::vector_bytes(&region.operations)?)
                .ok_or(AllocationError::Capacity)?;
        }
    }
    for (index, instance) in data.call_instantiations.iter().enumerate() {
        if !keep.instantiations[index] {
            released = released
                .checked_add(crate::output_budget::vector_bytes(&instance.arguments)?)
                .ok_or(AllocationError::Capacity)?;
        }
    }
    let mut regions = budget.vector(Retained, data.regions.len())?;
    for index in 0..data.regions.len() {
        let mut region = RegionId::from_index(index).unwrap();
        let target = loop {
            if let Some(kept) = remaps.regions[region.index()] {
                break kept;
            }
            region = match data.regions[region.index()].parent {
                Some(parent) => parent,
                None => data.entry,
            };
        };
        regions.push(target);
    }
    Ok(CompactPlan {
        keep,
        remaps,
        operands,
        operand_ranges,
        arguments,
        argument_ranges,
        regions,
        released,
    })
}

fn compact_in(
    data: &mut UnitData,
    budget: &mut AllocationBudget<'_>,
) -> Result<RegionRemap, RuleError> {
    budget.with_temporary_context(data, compact_plan, |plan, data, budget| {
        if plan.remaps.regions.is_empty() {
            return Ok(RegionRemap {
                map: Vec::new(),
                moved: false,
            });
        }
        let remaps = &plan.remaps;
        let map = budget.copy_slice(Retained, &plan.regions)?;
        // Admission precedes every possible growth. Retain/move below neither
        // clones nested payload nor changes the surviving arenas' capacities.
        let more = plan.operands.len().saturating_sub(data.operands.len());
        budget.reserve_vec(Retained, &mut data.operands, more)?;
        let more = plan
            .arguments
            .len()
            .saturating_sub(data.call_arguments.len());
        budget.reserve_vec(Retained, &mut data.call_arguments, more)?;
        budget.work(
            crate::compilation_policy::WorkKind::Edit,
            (data.operations.len()
                + data.values.len()
                + data.regions.len()
                + data.places.len()
                + data.calls.len()
                + data.call_instantiations.len()
                + plan.operands.len()
                + plan.arguments.len()) as u64,
        )?;
        for (index, op) in data.operations.iter_mut().enumerate() {
            if !plan.keep.operations[index] {
                continue;
            }
            // Allocation identity and payload are unchanged by compaction.
            if !matches!(op.kind, OperationKind::Allocate { .. }) {
                op.kind = remaps.kind(&op.kind)?;
            }
            op.operands = plan.operand_ranges[index];
            op.result = op.result.map(|value| remaps.value(value)).transpose()?;
            op.region = remaps.region(op.region)?;
        }
        for (index, value) in data.values.iter_mut().enumerate() {
            if plan.keep.values[index] {
                value.definition = remaps.op(value.definition)?;
            }
        }
        for (index, region) in data.regions.iter_mut().enumerate() {
            if !plan.keep.regions[index] {
                continue;
            }
            region.parent = region
                .parent
                .map(|parent| remaps.region(parent))
                .transpose()?;
            for op in &mut region.operations {
                *op = remaps.op(*op)?;
            }
            region.result = region.result.map(|value| remaps.value(value)).transpose()?;
        }
        for (index, place) in data.places.iter_mut().enumerate() {
            if plan.keep.places[index] {
                *place = remaps.place_payload(place)?;
            }
        }
        for (index, call) in data.calls.iter_mut().enumerate() {
            if !plan.keep.calls[index] {
                continue;
            }
            call.target = remaps.target(&call.target)?;
            call.contract.instantiation = call
                .contract
                .instantiation
                .map(|id| {
                    remaps.instantiations[id.index()].ok_or("a kept call lost its instantiation")
                })
                .transpose()?;
            call.arguments = plan.argument_ranges[index];
        }
        fn keep<T>(values: &mut Vec<T>, kept: &[bool]) {
            let mut index = 0;
            values.retain(|_| {
                let keep = kept[index];
                index += 1;
                keep
            });
        }
        keep(&mut data.operations, &plan.keep.operations);
        keep(&mut data.values, &plan.keep.values);
        keep(&mut data.regions, &plan.keep.regions);
        keep(&mut data.places, &plan.keep.places);
        keep(&mut data.calls, &plan.keep.calls);
        keep(&mut data.call_instantiations, &plan.keep.instantiations);
        data.operands.clear();
        data.operands.extend_from_slice(&plan.operands);
        data.call_arguments.clear();
        data.call_arguments.extend_from_slice(&plan.arguments);
        data.entry = remaps.region(data.entry)?;
        budget.release(Retained, plan.released)?;
        let moved = map
            .iter()
            .enumerate()
            .any(|(index, region)| region.index() != index);
        Ok(RegionRemap { map, moved })
    })
}

/// What a graft copies from a body, and how.
pub(super) struct GraftPlan<'a> {
    /// The body's own cells, cloned for this copy.
    pub(super) cells: &'a HashMap<CellId, CellId>,
    /// Parameters that read their argument: their plain loads are not
    /// copied, and a place naming one reads the argument value.
    pub(super) forwards: &'a HashMap<CellId, ValueId>,
    /// The body's final `return`, which is not copied: its operand is the
    /// copy's result.
    pub(super) exit: Option<OpId>,
}

/// What a graft placed.
pub(super) struct Grafted {
    /// The copied entry operations, in order, for the caller to splice.
    pub(super) operations: Vec<OpId>,
    /// What the exit returned, as a value of the target.
    pub(super) result: Option<ValueId>,
}

/// Where a graft of `source` into a target with `regions` regions puts each
/// source region: the entry becomes `parent`, and every other region the
/// entry owns is appended in source order. A graft skips only operations
/// that own no region, so this is also where `graft` puts them.
pub(super) fn graft_regions_in(
    source: &UnitData,
    regions: usize,
    parent: RegionId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<RegionId>>, RuleError> {
    budget.with_temporary(
        |budget| ownership_in(source, |_, _| false, budget),
        |owned, budget| region_map(source, &owned.regions, regions, parent, budget),
    )
}
fn region_map(
    source: &UnitData,
    keep: &[bool],
    regions: usize,
    parent: RegionId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<RegionId>>, RuleError> {
    let mut next = regions;
    let mut result = budget.vector(Retained, keep.len())?;
    for (index, &kept) in keep.iter().enumerate() {
        result.push(if index == source.entry.index() {
            Some(parent)
        } else if !kept {
            None
        } else {
            let id = RegionId::from_index(next).ok_or("region capacity")?;
            next += 1;
            Some(id)
        });
    }
    Ok(result)
}

fn appended<T>(
    keep: &[bool],
    base: usize,
    id: impl Fn(usize) -> Option<T>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Option<T>>, RuleError> {
    let mut next = base;
    let mut result = budget.vector(Retained, keep.len())?;
    for &kept in keep {
        result.push(if kept {
            let value = id(next).ok_or("graft capacity")?;
            next += 1;
            Some(value)
        } else {
            None
        });
    }
    Ok(result)
}

struct GraftInput<'a> {
    keep: Keep,
    remaps: Remaps<'a>,
}

/// Graft maps borrow cells/forwarded parameters; cloning those hash tables
/// cannot outlive or evade their producer's owner. Arena and nested payload
/// growth belongs to the original target budget.
pub(super) fn graft_in(
    source: &UnitData,
    target: &mut UnitData,
    parent: RegionId,
    plan: &GraftPlan<'_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Grafted, RuleError> {
    budget.with_temporary_context(
        target,
        |target, budget| {
            let keep = ownership_in(
                source,
                |op, operation| {
                    Some(op) == plan.exit
                        || match (&operation.kind, operation.result) {
                            (OperationKind::Load(place), Some(_)) => {
                                match source.places.get(place.index()) {
                                    Some(Place::Cell(cell)) => plan.forwards.contains_key(cell),
                                    _ => false,
                                }
                            }
                            _ => false,
                        }
                },
                budget,
            )?;
            let mut values = appended(
                &keep.values,
                target.values.len(),
                ValueId::from_index,
                budget,
            )?;
            for (index, region) in source.regions.iter().enumerate() {
                if !keep.regions[index] {
                    continue;
                }
                for &op in &region.operations {
                    let operation = &source.operations[op.index()];
                    if let (OperationKind::Load(place), Some(result)) =
                        (&operation.kind, operation.result)
                    {
                        if let Some(Place::Cell(cell)) = source.places.get(place.index()) {
                            if let Some(&value) = plan.forwards.get(cell) {
                                values[result.index()] = Some(value);
                            }
                        }
                    }
                }
            }
            let mut next_site = target
                .operations
                .iter()
                .filter_map(|operation| match operation.kind {
                    OperationKind::Allocate { identity, .. } => Some(identity.index() + 1),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let mut allocations: Vec<(AllocationId, AllocationId)> = Vec::new();
            for (index, operation) in source.operations.iter().enumerate() {
                if let (true, OperationKind::Allocate { identity, .. }) =
                    (keep.operations[index], &operation.kind)
                {
                    let fresh = AllocationId::from_index(next_site).ok_or("allocation capacity")?;
                    match allocations.binary_search_by_key(identity, |&(old, _)| old) {
                        Ok(index) => allocations[index].1 = fresh,
                        Err(index) => {
                            budget.reserve_vec(Retained, &mut allocations, 1)?;
                            allocations.insert(index, (*identity, fresh));
                        }
                    }
                    next_site += 1;
                }
            }
            let remaps = Remaps {
                operations: appended(
                    &keep.operations,
                    target.operations.len(),
                    OpId::from_index,
                    budget,
                )?,
                values,
                regions: region_map(source, &keep.regions, target.regions.len(), parent, budget)?,
                places: appended(
                    &keep.places,
                    target.places.len(),
                    PlaceId::from_index,
                    budget,
                )?,
                calls: appended(&keep.calls, target.calls.len(), CallId::from_index, budget)?,
                instantiations: appended(
                    &keep.instantiations,
                    target.call_instantiations.len(),
                    CallInstantiationId::from_index,
                    budget,
                )?,
                cells: Some(plan.cells),
                forwards: Some(plan.forwards),
                allocations,
            };
            Ok::<_, RuleError>(GraftInput { keep, remaps })
        },
        |input, target, budget| {
            let keep = &input.keep;
            let remaps = &input.remaps;
            for (index, operation) in source.operations.iter().enumerate() {
                if !keep.operations[index] {
                    continue;
                }
                let start = target.operands.len();
                for &value in source
                    .operands(operation.operands)
                    .ok_or("invalid operand range")?
                {
                    let value = remaps.value(value)?;
                    budget.push(Retained, &mut target.operands, value)?;
                }
                let kind = if let OperationKind::Allocate { identity, .. } = &operation.kind {
                    let mut kind = storage::kind(&operation.kind, budget)?;
                    if let OperationKind::Allocate {
                        identity: fresh, ..
                    } = &mut kind
                    {
                        *fresh = remaps
                            .allocations
                            .binary_search_by_key(identity, |&(old, _)| old)
                            .ok()
                            .map_or(*identity, |index| remaps.allocations[index].1);
                    }
                    kind
                } else {
                    remaps.kind(&operation.kind)?
                };
                budget.push(
                    Retained,
                    &mut target.operations,
                    Operation {
                        authored: operation.authored,
                        kind,
                        operands: OperandRange {
                            start: u32::try_from(start).map_err(|_| "operand capacity")?,
                            len: operation.operands.len,
                        },
                        result: operation
                            .result
                            .map(|value| remaps.value(value))
                            .transpose()?,
                        region: remaps.region(operation.region)?,
                        origin: operation.origin,
                        span: operation.span,
                    },
                )?;
            }
            for (index, value) in source.values.iter().enumerate() {
                if keep.values[index] {
                    budget.push(
                        Retained,
                        &mut target.values,
                        Value {
                            ty: value.ty,
                            definition: remaps.op(value.definition)?,
                        },
                    )?;
                }
            }
            let listed = |region: &Region,
                          budget: &mut AllocationBudget<'_>|
             -> Result<Vec<OpId>, RuleError> {
                let mut result = Vec::new();
                for &op in &region.operations {
                    if keep.operations[op.index()] {
                        let op = remaps.op(op)?;
                        budget.push(Retained, &mut result, op)?;
                    }
                }
                Ok(result)
            };
            for (index, region) in source.regions.iter().enumerate() {
                if !keep.regions[index] || index == source.entry.index() {
                    continue;
                }
                let operations = listed(region, budget)?;
                budget.push(
                    Retained,
                    &mut target.regions,
                    Region {
                        authored: region.authored,
                        parent: region
                            .parent
                            .map(|parent| remaps.region(parent))
                            .transpose()?,
                        operations,
                        result: region.result.map(|value| remaps.value(value)).transpose()?,
                        span: region.span,
                    },
                )?;
            }
            for (index, place) in source.places.iter().enumerate() {
                if keep.places[index] {
                    let place = remaps.place_payload(place)?;
                    budget.push(Retained, &mut target.places, place)?;
                }
            }
            for (index, call) in source.calls.iter().enumerate() {
                if !keep.calls[index] {
                    continue;
                }
                let start = target.call_arguments.len();
                for argument in source
                    .arguments(call.arguments)
                    .ok_or("invalid argument range")?
                {
                    let argument = remaps.argument(argument)?;
                    budget.push(Retained, &mut target.call_arguments, argument)?;
                }
                let mut contract = call.contract;
                contract.instantiation = contract
                    .instantiation
                    .map(|id| {
                        remaps.instantiations[id.index()]
                            .ok_or("a kept call lost its instantiation")
                    })
                    .transpose()?;
                budget.push(
                    Retained,
                    &mut target.calls,
                    CallSite {
                        target: remaps.target(&call.target)?,
                        contract,
                        arguments: ArgumentRange {
                            start: u32::try_from(start).map_err(|_| "argument capacity")?,
                            len: call.arguments.len,
                        },
                        omit_trailing: call.omit_trailing,
                        debug: call.debug,
                    },
                )?;
            }
            for (index, instance) in source.call_instantiations.iter().enumerate() {
                if keep.instantiations[index] {
                    let arguments = budget.copy_slice(Retained, &instance.arguments)?;
                    budget.push(
                        Retained,
                        &mut target.call_instantiations,
                        CallInstantiation {
                            declaration: instance.declaration,
                            arguments,
                            signature: instance.signature,
                        },
                    )?;
                }
            }
            let operations = listed(&source.regions[source.entry.index()], budget)?;
            let result = match plan.exit {
                Some(exit) => source
                    .operands(source.operations[exit.index()].operands)
                    .and_then(|operands| operands.first().copied())
                    .map(|value| remaps.value(value))
                    .transpose()?,
                None => None,
            };
            Ok(Grafted { operations, result })
        },
    )
}

/// Appends an operation to `region`'s arena storage (not to its list) and
/// returns it with its result, if it has one.
pub(super) fn push_operation_in(
    data: &mut UnitData,
    kind: OperationKind,
    operands: &[ValueId],
    result: Option<TypeId>,
    region: RegionId,
    span: Span,
    budget: &mut AllocationBudget<'_>,
) -> Result<(OpId, Option<ValueId>), RuleError> {
    let op = OpId::from_index(data.operations.len()).ok_or("operation capacity")?;
    let start = u32::try_from(data.operands.len()).map_err(|_| "operand capacity")?;
    budget.extend_copy(Retained, &mut data.operands, operands)?;
    let value = match result {
        Some(ty) => {
            let value = ValueId::from_index(data.values.len()).ok_or("value capacity")?;
            budget.push(Retained, &mut data.values, Value { ty, definition: op })?;
            Some(value)
        }
        None => None,
    };
    budget.push(
        Retained,
        &mut data.operations,
        Operation {
            authored: data.regions[region.index()].authored,
            kind,
            operands: OperandRange {
                start,
                len: u32::try_from(operands.len()).map_err(|_| "operand capacity")?,
            },
            result: value,
            region,
            origin: None,
            span,
        },
    )?;
    Ok((op, value))
}
