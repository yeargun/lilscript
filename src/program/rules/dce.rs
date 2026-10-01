//! Dead code (plan M5.1; architecture §8.3): liveness applied as an edit, so
//! both targets get it.
//!
//! - An operation nothing reads is removed when its evaluation is not
//!   required: it cannot throw, write, diverge, reenter or suspend
//!   (`EvaluationBehavior::requires_evaluation`, from the effect summaries).
//!   Its operands may then die in turn.
//! - A store into a local cell nothing reads is removed.
//! - A named function whose cell nothing reads is retired: its
//!   instantiation pair leaves the module prefix, its cell becomes an unused
//!   local, and nothing creates the unit any more.
//! - A body nothing creates is emptied, captures included, so it uses no
//!   cell: nothing observes it, and no target plans storage for it.
//!
//! Reads are counted over the units something creates; a cell visible
//! beyond the program (exported, a class's constructor or published method,
//! an `import()` namespace member, a script's root binding) counts as read.
//!
//! Prior art: Closure `RemoveUnusedCode` (`closure-compiler@0da58e1
//! src/com/google/javascript/jscomp/RemoveUnusedCode.java`), Terser
//! `drop_unused` (`terser@8fa44c8 lib/compress/drop-unused.js:112`); here
//! liveness comes from typed effect summaries, never from a name test.

use super::edit::{self, Editor};
use super::*;

/// Cells some created unit reads, and cells visible beyond the program.
struct CellUsage {
    read: Vec<bool>,
    visible: Vec<bool>,
}

type Retirement = (OpId, OpId, CellId);

pub(super) fn apply(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    seal: Seal,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    let program = editor.program();
    let created = created_units(program);
    let cells = cell_usage(program, &created, seal);
    let mut removals: Vec<(UnitId, Vec<OpId>, u32)> = Vec::new();
    let mut retirements: Vec<(UnitId, Vec<Retirement>)> = Vec::new();
    let mut emptied: Vec<UnitId> = Vec::new();
    for frozen in &program.units {
        let unit = frozen.id();
        if !created[unit.index()] {
            let data = frozen.data();
            if !data.regions[data.entry.index()].operations.is_empty() || !data.captures.is_empty()
            {
                emptied.push(unit);
            }
            continue;
        }
        let (dead, stores) = dead_operations(program, effects, unit, &cells);
        if !dead.is_empty() {
            removals.push((unit, dead, stores));
        }
        let data = frozen.data();
        if data.kind == UnitKind::ModuleInitialization {
            let retired = retired_functions(program, data, &cells);
            if !retired.is_empty() {
                retirements.push((unit, retired));
            }
        }
    }
    if removals.is_empty() && retirements.is_empty() && emptied.is_empty() {
        return Ok(false);
    }
    for unit in emptied {
        let data = editor.unit_mut_in(unit, budget)?;
        let entry = data.entry;
        data.regions[entry.index()].operations.clear();
        data.captures.clear();
        receipt.emptied_units += 1;
    }
    for (unit, dead, stores) in removals {
        let data = editor.unit_mut_in(unit, budget)?;
        for &op in &dead {
            edit::detach(data, op);
        }
        receipt.removed_operations += dead.len() as u32 - stores;
        receipt.removed_stores += stores;
    }
    for (unit, retired) in retirements {
        let (data, cells) = editor.unit_and_cells_in(unit, budget)?;
        for (creation, initialization, cell) in retired {
            edit::detach(data, creation);
            edit::detach(data, initialization);
            data.instantiation_prefix -= 2;
            cells[cell.index()].binding = CellBinding::Local;
            receipt.retired_functions += 1;
        }
    }
    Ok(true)
}

/// The cell a place reads or writes the storage of, if it is a cell's.
fn root_cell(data: &UnitData, mut place: PlaceId) -> Option<CellId> {
    loop {
        match data.places.get(place.index())? {
            Place::Cell(cell) => return Some(*cell),
            Place::Field { base, .. } => place = *base,
            _ => return None,
        }
    }
}

fn cell_usage(program: &Program<'_>, created: &[bool], seal: Seal) -> CellUsage {
    let mut read = vec![false; program.cells.len()];
    let mut visible = vec![false; program.cells.len()];
    let mut show = |cell: CellId| visible[cell.index()] = true;
    for export in program.exports.iter() {
        if let InterfaceTarget::Value(cell) = export.target {
            show(cell);
        }
    }
    for module in program.modules.iter() {
        for (_, cell) in &module.namespace {
            show(*cell);
        }
    }
    for class in program.classes.iter() {
        if let Some(cell) = class.value {
            show(cell);
        }
        for (_, cell) in &class.prototype {
            show(*cell);
        }
    }
    if seal != Seal::Module {
        for (index, cell) in program.cells.iter().enumerate() {
            if program
                .unit(cell.owner)
                .is_some_and(|owner| owner.kind == UnitKind::ModuleInitialization)
            {
                visible[index] = true;
            }
        }
    }
    for frozen in &program.units {
        if !created[frozen.id().index()] {
            continue;
        }
        let data = frozen.data();
        // A capture holds the cell for the body's closures, read or not.
        for cell in &data.captures {
            read[cell.index()] = true;
        }
        let mut note = |place: PlaceId| {
            if let Some(cell) = root_cell(data, place) {
                read[cell.index()] = true;
            }
        };
        for operation in &data.operations {
            match operation.kind {
                OperationKind::Load(place) | OperationKind::CheckPlace(place) => note(place),
                // A store into a projection keeps the rest of the storage.
                OperationKind::Store(place)
                    if !matches!(data.places.get(place.index()), Some(Place::Cell(_))) =>
                {
                    note(place)
                }
                _ => {}
            }
        }
        for call in &data.calls {
            if let CallTarget::Reference { place } = call.target {
                note(place);
            }
            for argument in data.arguments(call.arguments).unwrap_or(&[]) {
                if let CallArgument::Reference(place) = argument {
                    note(*place);
                }
            }
        }
    }
    CellUsage { read, visible }
}

/// The values an operation reads: its operands, the values its places and
/// call name, and the results of the regions it owns.
pub(super) fn reads(data: &UnitData, operation: &Operation, out: &mut Vec<ValueId>) {
    out.clear();
    out.extend_from_slice(data.operands(operation.operands).unwrap_or(&[]));
    let place_values = |place: PlaceId, out: &mut Vec<ValueId>| {
        let mut place = place;
        loop {
            match data.places.get(place.index()) {
                Some(Place::Value(value)) => out.push(*value),
                Some(Place::Member { receiver, .. } | Place::ClassField { receiver, .. }) => {
                    out.push(*receiver)
                }
                Some(Place::Index { receiver, key }) => out.extend([*receiver, *key]),
                Some(Place::Field { base, .. }) => {
                    place = *base;
                    continue;
                }
                Some(Place::Cell(_)) | None => {}
            }
            break;
        }
    };
    match operation.kind {
        OperationKind::Load(place)
        | OperationKind::Store(place)
        | OperationKind::CheckPlace(place) => place_values(place, out),
        OperationKind::PrepareCall(call) => match data.calls[call.index()].target {
            CallTarget::Value { callee, .. } => out.push(callee),
            CallTarget::Intrinsic {
                receiver: Some(receiver),
                ..
            } => out.push(receiver),
            CallTarget::Reference { place } => place_values(place, out),
            _ => {}
        },
        OperationKind::Call(call) => {
            let site = &data.calls[call.index()];
            for argument in data.arguments(site.arguments).unwrap_or(&[]) {
                match *argument {
                    CallArgument::Value(value) | CallArgument::Spread(value) => out.push(value),
                    CallArgument::Reference(place) => place_values(place, out),
                }
            }
        }
        _ => {}
    }
    for child in operation.kind.child_regions() {
        out.extend(data.regions[child.index()].result);
    }
}

/// Operation kinds that may go once nothing reads their result, when their
/// behavior does not require evaluation.
fn value_kind(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Constant(_)
            | OperationKind::CopyValue
            | OperationKind::IntBinary(_)
            | OperationKind::Binary(_)
            | OperationKind::Unary { .. }
            | OperationKind::Load(_)
            | OperationKind::Intrinsic(_)
            | OperationKind::Closure(_)
            | OperationKind::Allocate { .. }
            | OperationKind::IsUndefined { .. }
            | OperationKind::TypeTest(_)
            | OperationKind::ClosedClassTest(_)
            | OperationKind::Template
            | OperationKind::Call(_)
    )
}

/// The operations of `unit` that go, and how many of them are stores. A
/// removal releases what it read, so the search cascades to a fixed point.
fn dead_operations(
    program: &Program<'_>,
    effects: &ProgramEffects,
    unit: UnitId,
    cells: &CellUsage,
) -> (Vec<OpId>, u32) {
    let data = program.unit(unit).expect("a program unit");
    let behaviors = behaviors(program, effects, unit, None);
    let initialization = effects.initialization();
    let mut in_prefix = vec![false; data.operations.len()];
    for op in prefix(data) {
        in_prefix[op.index()] = true;
    }
    let mut preparations: Vec<Vec<OpId>> = vec![Vec::new(); data.calls.len()];
    let mut uses = vec![0u32; data.values.len()];
    let mut scratch = Vec::new();
    for (index, operation) in data.operations.iter().enumerate() {
        if let OperationKind::PrepareCall(call) | OperationKind::PrepareReference { call, .. } =
            operation.kind
        {
            preparations[call.index()].push(OpId::from_index(index).unwrap());
        }
        reads(data, operation, &mut scratch);
        for value in &scratch {
            uses[value.index()] += 1;
        }
    }
    let unread_local = |cell: CellId| {
        program.cells[cell.index()].binding == CellBinding::Local
            && !cells.read[cell.index()]
            && !cells.visible[cell.index()]
    };
    let mut dead = vec![false; data.operations.len()];
    let mut removed = Vec::new();
    let mut stores = 0;
    let mut pending: Vec<OpId> = (0..data.operations.len())
        .rev()
        .map(|index| OpId::from_index(index).unwrap())
        .collect();
    while let Some(op) = pending.pop() {
        if dead[op.index()] || in_prefix[op.index()] {
            continue;
        }
        let operation = &data.operations[op.index()];
        let store = match operation.kind {
            OperationKind::Initialize(cell) | OperationKind::Declare(cell) => Some(cell),
            OperationKind::Store(place) => match data.places.get(place.index()) {
                Some(&Place::Cell(cell)) => Some(cell),
                _ => None,
            },
            _ => None,
        };
        let removable = match store {
            // A declaration cannot fail; an assignment before it would throw
            // (the temporal dead zone), so it goes only past the declaration.
            Some(cell) => {
                unread_local(cell)
                    && (matches!(
                        operation.kind,
                        OperationKind::Initialize(_) | OperationKind::Declare(_)
                    ) || initialization.initialized(program, unit, op, cell))
            }
            None => {
                value_kind(&operation.kind)
                    && operation
                        .result
                        .is_some_and(|result| uses[result.index()] == 0)
                    && !behaviors[op.index()].requires_evaluation()
            }
        };
        if !removable {
            continue;
        }
        let mut released = vec![op];
        if let OperationKind::Call(call) = operation.kind {
            released.extend(preparations[call.index()].iter().copied());
        }
        for op in released {
            if std::mem::replace(&mut dead[op.index()], true) {
                continue;
            }
            removed.push(op);
            reads(data, &data.operations[op.index()], &mut scratch);
            for value in &scratch {
                let count = &mut uses[value.index()];
                *count -= 1;
                if *count == 0 {
                    pending.push(data.values[value.index()].definition);
                }
            }
        }
        if store.is_some() {
            stores += 1;
        }
    }
    (removed, stores)
}

/// The named functions of a module prefix whose cells nothing reads.
fn retired_functions(program: &Program<'_>, data: &UnitData, cells: &CellUsage) -> Vec<Retirement> {
    prefix(data)
        .chunks_exact(2)
        .filter_map(|pair| {
            let OperationKind::Closure(body) = data.operations[pair[0].index()].kind else {
                return None;
            };
            let OperationKind::Initialize(cell) = data.operations[pair[1].index()].kind else {
                return None;
            };
            (program.cells[cell.index()].binding == CellBinding::Function(body)
                && !cells.read[cell.index()]
                && !cells.visible[cell.index()])
            .then_some((pair[0], pair[1], cell))
        })
        .collect()
}
