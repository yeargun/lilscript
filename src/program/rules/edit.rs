//! The edit kernel of the program rules (plan M5.1, architecture §8.1).
//!
//! Rules run before publication, on a program the build exclusively owns, so
//! an edit is an in-place change to a unit's arenas. An edit that removes an
//! operation or a region only detaches it; `compact` then drops everything
//! the unit no longer owns and renumbers what it keeps, which is what the
//! verifier requires ("unowned semantic storage"). A round commits once:
//! every touched unit is compacted and stamped with a fresh revision, and the
//! derived views, which describe the old program, are dropped.

use super::super::ids::RevisionId;
use super::super::views::ProgramViews;
use super::super::*;
use std::collections::HashMap;
use std::sync::Arc;

pub(super) struct Editor<'src> {
    program: Program<'src>,
    touched: Vec<bool>,
}

impl<'src> Editor<'src> {
    pub(super) fn new(program: Program<'src>) -> Self {
        let touched = vec![false; program.units.len()];
        Self { program, touched }
    }

    pub(super) fn program(&self) -> &Program<'src> {
        &self.program
    }

    /// One unit's data for editing. A shared payload is copied once.
    pub(super) fn unit_mut(&mut self, unit: UnitId) -> &mut UnitData {
        let index = unit.index();
        self.touched[index] = true;
        let frozen = &mut self.program.units[index];
        if !frozen.allocation_is_unique() {
            *frozen = WorkingUnit::new(unit, frozen.data().clone()).freeze();
        }
        frozen
            .unique_edit()
            .expect("an unshared unit payload is editable")
            .0
    }

    /// One unit's data and the cells table, for an edit that moves a region's
    /// cells or changes a binding.
    pub(super) fn unit_and_cells(&mut self, unit: UnitId) -> (&mut UnitData, &mut [Cell]) {
        let index = unit.index();
        self.touched[index] = true;
        let cells = Arc::make_mut(&mut self.program.cells);
        let frozen = &mut self.program.units[index];
        if !frozen.allocation_is_unique() {
            *frozen = WorkingUnit::new(unit, frozen.data().clone()).freeze();
        }
        let data = frozen
            .unique_edit()
            .expect("an unshared unit payload is editable")
            .0;
        (data, cells.as_mut_slice())
    }

    /// Compacts every touched unit, stamps it with a fresh revision and drops
    /// the derived views. A cell whose region an edit removed was declared in
    /// code that no longer exists; it keeps the nearest region its owner
    /// still has, so the table stays valid.
    pub(super) fn commit(&mut self) -> Result<(), &'static str> {
        let mut remaps: Vec<Option<RegionRemap>> = Vec::new();
        for index in 0..self.touched.len() {
            if !std::mem::take(&mut self.touched[index]) {
                continue;
            }
            let frozen = &mut self.program.units[index];
            let (data, revision) = frozen
                .unique_edit()
                .ok_or("an edited unit payload became shared")?;
            *revision = RevisionId::fresh();
            let remap = compact(data)?;
            if remap.moved {
                if remaps.is_empty() {
                    remaps.resize_with(self.program.units.len(), || None);
                }
                remaps[index] = Some(remap);
            }
        }
        if !remaps.is_empty() {
            let cells = Arc::make_mut(&mut self.program.cells);
            for cell in cells.iter_mut() {
                if let Some(Some(remap)) = remaps.get(cell.owner.index()) {
                    cell.region = remap.region(cell.region);
                }
            }
        }
        self.program.views = ProgramViews::default();
        Ok(())
    }

    /// A unit's current revision, to read while another unit is edited.
    pub(super) fn handle(&self, unit: UnitId) -> FrozenUnit {
        self.program.units[unit.index()].clone()
    }

    /// The type's id in the program's table, added when no equal type is
    /// there yet (conversion interns types the same way).
    pub(super) fn intern_type(
        &mut self,
        ty: crate::check::Type<'src>,
    ) -> Result<TypeId, &'static str> {
        if let Some(index) = self.program.types.iter().position(|known| *known == ty) {
            return TypeId::from_index(index).ok_or("type capacity");
        }
        let types = Arc::make_mut(&mut self.program.types);
        let id = TypeId::from_index(types.len()).ok_or("type capacity")?;
        types.push(ty);
        Ok(id)
    }

    /// Adds a synthetic cell; synthetic cells follow every checked one.
    pub(super) fn add_cell(&mut self, cell: Cell) -> Result<CellId, &'static str> {
        let cells = Arc::make_mut(&mut self.program.cells);
        let id = CellId::from_index(cells.len()).ok_or("cell capacity")?;
        cells.push(cell);
        Ok(id)
    }

    pub(super) fn finish(mut self) -> Result<Program<'src>, &'static str> {
        self.commit()?;
        Ok(self.program)
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
pub(super) fn make_constant(data: &mut UnitData, operation: OpId, constant: Constant) {
    let op = &mut data.operations[operation.index()];
    op.kind = OperationKind::Constant(constant);
    op.operands = OperandRange { start: 0, len: 0 };
}

/// Makes `operation` a block of `region`, one of its child regions; the
/// other child regions and the operands are released. A block keeps the
/// region's lexical scope, as the branch it replaces did.
pub(super) fn make_block(data: &mut UnitData, operation: OpId, region: RegionId) {
    let op = &mut data.operations[operation.index()];
    op.kind = OperationKind::Block(region);
    op.operands = OperandRange { start: 0, len: 0 };
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
pub(super) fn splice(
    data: &mut UnitData,
    cells: &mut [Cell],
    unit: UnitId,
    at: OpId,
    region: RegionId,
) {
    let parent = data.operations[at.index()].region;
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
        list.splice(position..=position, moved);
    }
    for cell in cells
        .iter_mut()
        .filter(|cell| cell.owner == unit && cell.region == region)
    {
        cell.region = parent;
    }
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

fn renumber<T: Copy>(keep: &[bool], id: impl Fn(usize) -> Option<T>) -> Vec<Option<T>> {
    let mut next = 0;
    keep.iter()
        .map(|&kept| {
            kept.then(|| {
                let value = id(next).expect("a compacted arena fits its old capacity");
                next += 1;
                value
            })
        })
        .collect()
}

struct Remaps {
    operations: Vec<Option<OpId>>,
    values: Vec<Option<ValueId>>,
    regions: Vec<Option<RegionId>>,
    places: Vec<Option<PlaceId>>,
    calls: Vec<Option<CallId>>,
    instantiations: Vec<Option<CallInstantiationId>>,
    /// A graft's cells: what the copied body declared, cloned for the copy.
    cells: HashMap<CellId, CellId>,
    /// A graft's parameters that read their argument: a place naming one
    /// reads the argument value.
    forwards: HashMap<CellId, ValueId>,
    /// A graft's allocation sites, renumbered past the receiver's own.
    allocations: HashMap<AllocationId, AllocationId>,
}

impl Remaps {
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
        self.cells.get(&id).copied().unwrap_or(id)
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
            Op::Allocate { identity, kind } => Op::Allocate {
                identity: self.allocations.get(identity).copied().unwrap_or(*identity),
                kind: kind.clone(),
            },
            other => other.clone(),
        })
    }

    fn place_payload(&self, place: &Place) -> Result<Place, &'static str> {
        Ok(match place {
            Place::Cell(cell) => match self.forwards.get(cell) {
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

fn ownership(
    data: &UnitData,
    mut skip: impl FnMut(OpId, &Operation) -> bool,
) -> Result<Keep, &'static str> {
    let mut region_keep = vec![false; data.regions.len()];
    let mut op_keep = vec![false; data.operations.len()];
    let mut stack = vec![data.entry];
    while let Some(region) = stack.pop() {
        let seen = region_keep
            .get_mut(region.index())
            .ok_or("a kept operation owns a missing region")?;
        if std::mem::replace(seen, true) {
            return Err("a region has two owners");
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
            stack.extend(operation.kind.child_regions());
        }
    }
    let mut call_keep = vec![false; data.calls.len()];
    for (index, op) in data.operations.iter().enumerate() {
        if let (true, OperationKind::Call(call)) = (op_keep[index], &op.kind) {
            call_keep[call.index()] = true;
        }
    }
    let mut place_keep = vec![false; data.places.len()];
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
                return Err("a kept preparation has no kept call");
            }
            _ => {}
        }
    }
    let mut instantiation_keep = vec![false; data.call_instantiations.len()];
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
    let mut value_keep = vec![false; data.values.len()];
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
}

/// Drops the storage the entry region no longer owns: operations, their
/// values, regions, calls with their arguments and instantiations, and
/// places. What is kept keeps its order and is renumbered densely.
pub(super) fn compact(data: &mut UnitData) -> Result<RegionRemap, &'static str> {
    let Keep {
        regions: region_keep,
        operations: op_keep,
        calls: call_keep,
        places: place_keep,
        instantiations: instantiation_keep,
        values: value_keep,
    } = ownership(data, |_, _| false)?;
    let everything = |keep: &[bool]| keep.iter().all(|kept| *kept);
    // An edit that rewrites an operation's operands or a call's arguments
    // leaves the old slots unowned: they are dropped too.
    let owned_operands: usize = data
        .operations
        .iter()
        .map(|op| op.operands.len as usize)
        .sum();
    let owned_arguments: usize = data
        .calls
        .iter()
        .map(|call| call.arguments.len as usize)
        .sum();
    if everything(&op_keep)
        && everything(&region_keep)
        && everything(&call_keep)
        && everything(&place_keep)
        && everything(&instantiation_keep)
        && everything(&value_keep)
        && owned_operands == data.operands.len()
        && owned_arguments == data.call_arguments.len()
    {
        return Ok(RegionRemap {
            map: Vec::new(),
            moved: false,
        });
    }
    let remaps = Remaps {
        operations: renumber(&op_keep, OpId::from_index),
        values: renumber(&value_keep, ValueId::from_index),
        regions: renumber(&region_keep, RegionId::from_index),
        places: renumber(&place_keep, PlaceId::from_index),
        calls: renumber(&call_keep, CallId::from_index),
        instantiations: renumber(&instantiation_keep, CallInstantiationId::from_index),
        cells: HashMap::new(),
        forwards: HashMap::new(),
        allocations: HashMap::new(),
    };

    let mut operands = Vec::with_capacity(data.operands.len());
    let mut operations = Vec::with_capacity(op_keep.iter().filter(|kept| **kept).count());
    for (index, op) in data.operations.iter().enumerate() {
        if !op_keep[index] {
            continue;
        }
        let start = operands.len();
        for value in data.operands(op.operands).ok_or("invalid operand range")? {
            operands.push(remaps.value(*value)?);
        }
        operations.push(Operation {
            kind: remaps.kind(&op.kind)?,
            operands: OperandRange {
                start: u32::try_from(start).map_err(|_| "operand capacity")?,
                len: op.operands.len,
            },
            result: op.result.map(|value| remaps.value(value)).transpose()?,
            region: remaps.region(op.region)?,
            origin: op.origin,
            span: op.span,
        });
    }
    let values = data
        .values
        .iter()
        .enumerate()
        .filter(|(index, _)| value_keep[*index])
        .map(|(_, value)| {
            Ok(Value {
                ty: value.ty,
                definition: remaps.op(value.definition)?,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let regions = data
        .regions
        .iter()
        .enumerate()
        .filter(|(index, _)| region_keep[*index])
        .map(|(_, region)| {
            Ok(Region {
                parent: region
                    .parent
                    .map(|parent| remaps.region(parent))
                    .transpose()?,
                operations: region
                    .operations
                    .iter()
                    .map(|op| remaps.op(*op))
                    .collect::<Result<_, _>>()?,
                result: region.result.map(|value| remaps.value(value)).transpose()?,
                span: region.span,
            })
        })
        .collect::<Result<Vec<_>, &'static str>>()?;
    let places = data
        .places
        .iter()
        .enumerate()
        .filter(|(index, _)| place_keep[*index])
        .map(|(_, place)| remaps.place_payload(place))
        .collect::<Result<Vec<_>, &'static str>>()?;
    let mut call_arguments = Vec::with_capacity(data.call_arguments.len());
    let mut calls = Vec::with_capacity(call_keep.iter().filter(|kept| **kept).count());
    for (index, call) in data.calls.iter().enumerate() {
        if !call_keep[index] {
            continue;
        }
        let start = call_arguments.len();
        for argument in data
            .arguments(call.arguments)
            .ok_or("invalid argument range")?
        {
            call_arguments.push(remaps.argument(argument)?);
        }
        let mut contract = call.contract;
        contract.instantiation = contract
            .instantiation
            .map(|id| remaps.instantiations[id.index()].ok_or("a kept call lost its instantiation"))
            .transpose()?;
        calls.push(CallSite {
            target: remaps.target(&call.target)?,
            contract,
            arguments: ArgumentRange {
                start: u32::try_from(start).map_err(|_| "argument capacity")?,
                len: call.arguments.len,
            },
            debug: call.debug,
        });
    }
    let call_instantiations = data
        .call_instantiations
        .iter()
        .enumerate()
        .filter(|(index, _)| instantiation_keep[*index])
        .map(|(_, instantiation)| instantiation.clone())
        .collect();

    // A removed region's cells map to its nearest surviving ancestor.
    let mut map = Vec::with_capacity(data.regions.len());
    for index in 0..data.regions.len() {
        let mut region = RegionId::from_index(index).unwrap();
        let target = loop {
            if let Some(kept) = remaps.regions[region.index()] {
                break kept;
            }
            match data.regions[region.index()].parent {
                Some(parent) => region = parent,
                None => break remaps.region(data.entry)?,
            }
        };
        map.push(target);
    }

    data.entry = remaps.region(data.entry)?;
    data.operations = operations;
    data.operands = operands;
    data.values = values;
    data.regions = regions;
    data.places = places;
    data.calls = calls;
    data.call_arguments = call_arguments;
    data.call_instantiations = call_instantiations;
    Ok(RegionRemap { map, moved: true })
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
pub(super) fn graft_regions(
    source: &UnitData,
    regions: usize,
    parent: RegionId,
) -> Result<Vec<Option<RegionId>>, &'static str> {
    let owned = ownership(source, |_, _| false)?;
    let mut next = regions;
    owned
        .regions
        .iter()
        .enumerate()
        .map(|(index, &kept)| {
            if index == source.entry.index() {
                return Ok(Some(parent));
            }
            if !kept {
                return Ok(None);
            }
            let id = RegionId::from_index(next).ok_or("region capacity")?;
            next += 1;
            Ok(Some(id))
        })
        .collect()
}

fn appended<T>(
    keep: &[bool],
    base: usize,
    id: impl Fn(usize) -> Option<T>,
) -> Result<Vec<Option<T>>, &'static str> {
    let mut next = base;
    keep.iter()
        .map(|&kept| {
            if !kept {
                return Ok(None);
            }
            let value = id(next).ok_or("graft capacity")?;
            next += 1;
            Ok(Some(value))
        })
        .collect()
}

/// Copies `source`'s body into `target`, its regions hanging from `parent`:
/// every region, operation, value, place, call and instantiation its entry
/// owns, except the exit and the plain loads of forwarded parameters, whose
/// results are the arguments. The copied entry operations are returned, not
/// placed. Allocation sites are renumbered past the target's own, since a
/// site's identity is local to its unit.
pub(super) fn graft(
    source: &UnitData,
    target: &mut UnitData,
    parent: RegionId,
    plan: &GraftPlan<'_>,
) -> Result<Grafted, &'static str> {
    let mut forwarded: Vec<(ValueId, ValueId)> = Vec::new();
    let keep = ownership(source, |op, operation| {
        if Some(op) == plan.exit {
            return true;
        }
        if let (OperationKind::Load(place), Some(result)) = (&operation.kind, operation.result) {
            if let Some(Place::Cell(cell)) = source.places.get(place.index()) {
                if let Some(&value) = plan.forwards.get(cell) {
                    forwarded.push((result, value));
                    return true;
                }
            }
        }
        false
    })?;
    let mut values = appended(&keep.values, target.values.len(), ValueId::from_index)?;
    for (from, to) in forwarded {
        values[from.index()] = Some(to);
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
    let mut allocations = HashMap::new();
    for (index, operation) in source.operations.iter().enumerate() {
        if let (true, OperationKind::Allocate { identity, .. }) =
            (keep.operations[index], &operation.kind)
        {
            let fresh = AllocationId::from_index(next_site).ok_or("allocation capacity")?;
            allocations.insert(*identity, fresh);
            next_site += 1;
        }
    }
    let remaps = Remaps {
        operations: appended(&keep.operations, target.operations.len(), OpId::from_index)?,
        values,
        regions: graft_regions(source, target.regions.len(), parent)?,
        places: appended(&keep.places, target.places.len(), PlaceId::from_index)?,
        calls: appended(&keep.calls, target.calls.len(), CallId::from_index)?,
        instantiations: appended(
            &keep.instantiations,
            target.call_instantiations.len(),
            CallInstantiationId::from_index,
        )?,
        cells: plan.cells.clone(),
        forwards: plan.forwards.clone(),
        allocations,
    };
    for (index, operation) in source.operations.iter().enumerate() {
        if !keep.operations[index] {
            continue;
        }
        let start = target.operands.len();
        for value in source
            .operands(operation.operands)
            .ok_or("invalid operand range")?
        {
            target.operands.push(remaps.value(*value)?);
        }
        target.operations.push(Operation {
            kind: remaps.kind(&operation.kind)?,
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
        });
    }
    for (index, value) in source.values.iter().enumerate() {
        if keep.values[index] {
            target.values.push(Value {
                ty: value.ty,
                definition: remaps.op(value.definition)?,
            });
        }
    }
    let listed = |region: &Region| -> Result<Vec<OpId>, &'static str> {
        region
            .operations
            .iter()
            .filter(|op| keep.operations[op.index()])
            .map(|op| remaps.op(*op))
            .collect()
    };
    for (index, region) in source.regions.iter().enumerate() {
        if !keep.regions[index] || index == source.entry.index() {
            continue;
        }
        target.regions.push(Region {
            parent: region
                .parent
                .map(|parent| remaps.region(parent))
                .transpose()?,
            operations: listed(region)?,
            result: region.result.map(|value| remaps.value(value)).transpose()?,
            span: region.span,
        });
    }
    for (index, place) in source.places.iter().enumerate() {
        if keep.places[index] {
            target.places.push(remaps.place_payload(place)?);
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
            target.call_arguments.push(remaps.argument(argument)?);
        }
        let mut contract = call.contract;
        contract.instantiation = contract
            .instantiation
            .map(|id| remaps.instantiations[id.index()].ok_or("a kept call lost its instantiation"))
            .transpose()?;
        target.calls.push(CallSite {
            target: remaps.target(&call.target)?,
            contract,
            arguments: ArgumentRange {
                start: u32::try_from(start).map_err(|_| "argument capacity")?,
                len: call.arguments.len,
            },
            debug: call.debug,
        });
    }
    for (index, instantiation) in source.call_instantiations.iter().enumerate() {
        if keep.instantiations[index] {
            target.call_instantiations.push(instantiation.clone());
        }
    }
    let operations = listed(&source.regions[source.entry.index()])?;
    let result = match plan.exit {
        Some(exit) => source
            .operands(source.operations[exit.index()].operands)
            .and_then(|operands| operands.first().copied())
            .map(|value| remaps.value(value))
            .transpose()?,
        None => None,
    };
    Ok(Grafted { operations, result })
}

/// Appends an operation to `region`'s arena storage (not to its list) and
/// returns it with its result, if it has one.
pub(super) fn push_operation(
    data: &mut UnitData,
    kind: OperationKind,
    operands: &[ValueId],
    result: Option<TypeId>,
    region: RegionId,
    span: Span,
) -> Result<(OpId, Option<ValueId>), &'static str> {
    let op = OpId::from_index(data.operations.len()).ok_or("operation capacity")?;
    let start = u32::try_from(data.operands.len()).map_err(|_| "operand capacity")?;
    data.operands.extend_from_slice(operands);
    let value = match result {
        Some(ty) => {
            let value = ValueId::from_index(data.values.len()).ok_or("value capacity")?;
            data.values.push(Value { ty, definition: op });
            Some(value)
        }
        None => None,
    };
    data.operations.push(Operation {
        kind,
        operands: OperandRange {
            start,
            len: u32::try_from(operands.len()).map_err(|_| "operand capacity")?,
        },
        result: value,
        region,
        origin: None,
        span,
    });
    Ok((op, value))
}
