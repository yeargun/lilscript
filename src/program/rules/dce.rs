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
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

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
    budget.with_temporary_context(
        editor,
        |editor, budget| plan(editor.program(), effects, seal, budget),
        |plan, editor, budget| execute(editor, plan, receipt, budget),
    )
}

#[derive(Default)]
struct Plan {
    removals: Vec<(UnitId, Vec<OpId>, u32)>,
    retirements: Vec<(UnitId, Vec<Retirement>)>,
    emptied: Vec<UnitId>,
}

fn plan(
    program: &Program<'_>,
    effects: &ProgramEffects,
    seal: Seal,
    budget: &mut AllocationBudget<'_>,
) -> Result<Plan, super::RuleError> {
    budget.retained_phase(|budget| {
        let created = created_units_in(program, budget)?;
        let cells = cell_usage(program, &created, seal, budget)?;
        let mut plan = Plan::default();
        for frozen in &program.units {
            budget.work(WorkKind::Analysis, 1)?;
            let unit = frozen.id();
            if !created[unit.index()] {
                let data = frozen.data();
                if !data.regions[data.entry.index()].operations.is_empty()
                    || !data.captures.is_empty()
                {
                    budget.push(Retained, &mut plan.emptied, unit)?;
                }
                continue;
            }
            let (dead, stores) = dead_operations(program, effects, unit, &cells, budget)?;
            if dead.is_empty() {
                storage::release_vec(dead, Retained, budget)?;
            } else {
                budget.push(Retained, &mut plan.removals, (unit, dead, stores))?;
            }
            if frozen.data().kind == UnitKind::ModuleInitialization {
                let retired = retired_functions(program, frozen.data(), &cells, budget)?;
                if retired.is_empty() {
                    storage::release_vec(retired, Retained, budget)?;
                } else {
                    budget.push(Retained, &mut plan.retirements, (unit, retired))?;
                }
            }
        }
        storage::release_vec(created, Retained, budget)?;
        Ok(plan)
    })
}

fn execute(
    editor: &mut Editor<'_>,
    plan: &Plan,
    receipt: &mut RuleReceipt,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    let changed =
        !plan.removals.is_empty() || !plan.retirements.is_empty() || !plan.emptied.is_empty();
    for &unit in &plan.emptied {
        let data = editor.unit_mut_in(unit, budget)?;
        let entry = data.entry;
        data.regions[entry.index()].operations.clear();
        data.captures.clear();
        receipt.emptied_units += 1;
    }
    for (unit, dead, stores) in &plan.removals {
        let data = editor.unit_mut_in(*unit, budget)?;
        for &op in dead {
            edit::detach(data, op);
        }
        receipt.removed_operations += dead.len() as u32 - stores;
        receipt.removed_stores += stores;
    }
    for (unit, retired) in &plan.retirements {
        let (data, cells) = editor.unit_and_cells_in(*unit, budget)?;
        for &(creation, initialization, cell) in retired {
            edit::detach(data, creation);
            edit::detach(data, initialization);
            data.instantiation_prefix -= 2;
            cells[cell.index()].binding = CellBinding::Local;
            receipt.retired_functions += 1;
        }
    }
    Ok(changed)
}

/// The cell a place reads or writes the storage of, if it is a cell's.
fn root_cell(
    data: &UnitData,
    mut place: PlaceId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<CellId>, AllocationError> {
    loop {
        budget.work(WorkKind::Analysis, 1)?;
        match data.places.get(place.index()) {
            Some(Place::Cell(cell)) => return Ok(Some(*cell)),
            Some(Place::Field { base, .. }) => place = *base,
            _ => return Ok(None),
        }
    }
}

fn cell_usage(
    program: &Program<'_>,
    created: &[bool],
    seal: Seal,
    budget: &mut AllocationBudget<'_>,
) -> Result<CellUsage, AllocationError> {
    let mut read = budget.filled(Scratch, program.cells.len(), false)?;
    let mut visible = budget.filled(Scratch, program.cells.len(), false)?;
    let mut show = |cell: CellId| visible[cell.index()] = true;
    for export in program.exports.iter() {
        budget.work(WorkKind::Analysis, 1)?;
        if let InterfaceTarget::Value(cell) = export.target {
            show(cell);
        }
    }
    for module in program.modules.iter() {
        budget.work(WorkKind::Analysis, module.namespace.len() as u64 + 1)?;
        for (_, cell) in &module.namespace {
            show(*cell);
        }
    }
    for class in program.classes.iter() {
        budget.work(WorkKind::Analysis, class.prototype.len() as u64 + 1)?;
        if let Some(cell) = class.value {
            show(cell);
        }
        for (_, cell) in &class.prototype {
            show(*cell);
        }
    }
    if seal != Seal::Module {
        for (index, cell) in program.cells.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
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
        budget.work(
            WorkKind::Analysis,
            data.captures.len() as u64 + data.operations.len() as u64,
        )?;
        // A capture holds the cell for the body's closures, read or not.
        for cell in &data.captures {
            read[cell.index()] = true;
        }
        let mut note = |place: PlaceId, budget: &mut AllocationBudget<'_>| {
            if let Some(cell) = root_cell(data, place, budget)? {
                read[cell.index()] = true;
            }
            Ok::<_, AllocationError>(())
        };
        for operation in &data.operations {
            match operation.kind {
                OperationKind::Load(place) | OperationKind::CheckPlace(place) => {
                    note(place, budget)?
                }
                // A store into a projection keeps the rest of the storage.
                OperationKind::Store(place)
                    if !matches!(data.places.get(place.index()), Some(Place::Cell(_))) =>
                {
                    note(place, budget)?
                }
                _ => {}
            }
        }
        for call in &data.calls {
            budget.work(
                WorkKind::Analysis,
                data.arguments(call.arguments).map_or(0, |args| args.len()) as u64 + 1,
            )?;
            if let CallTarget::Reference { place } = call.target {
                note(place, budget)?;
            }
            for argument in data.arguments(call.arguments).unwrap_or(&[]) {
                if let CallArgument::Reference(place) = argument {
                    note(*place, budget)?;
                }
            }
        }
    }
    Ok(CellUsage { read, visible })
}

/// Stream reads so cascading liveness never materializes another operand list.
/// A visitor can admit its own queue growth through the same allocation owner.
pub(super) fn visit_reads(
    data: &UnitData,
    operation: &Operation,
    budget: &mut AllocationBudget<'_>,
    mut visit: impl FnMut(ValueId, &mut AllocationBudget<'_>) -> Result<(), AllocationError>,
) -> Result<(), AllocationError> {
    fn place_values(
        data: &UnitData,
        mut place: PlaceId,
        budget: &mut AllocationBudget<'_>,
        visit: &mut impl FnMut(ValueId, &mut AllocationBudget<'_>) -> Result<(), AllocationError>,
    ) -> Result<(), AllocationError> {
        loop {
            budget.work(WorkKind::Analysis, 1)?;
            match data.places.get(place.index()) {
                Some(Place::Value(value)) => visit(*value, budget)?,
                Some(Place::Member { receiver, .. } | Place::ClassField { receiver, .. }) => {
                    visit(*receiver, budget)?
                }
                Some(Place::Index { receiver, key }) => {
                    visit(*receiver, budget)?;
                    visit(*key, budget)?;
                }
                Some(Place::Field { base, .. }) => {
                    place = *base;
                    continue;
                }
                Some(Place::Cell(_)) | None => {}
            }
            return Ok(());
        }
    }
    for &value in data.operands(operation.operands).unwrap_or(&[]) {
        budget.work(WorkKind::Analysis, 1)?;
        visit(value, budget)?;
    }
    match operation.kind {
        OperationKind::Load(place)
        | OperationKind::Store(place)
        | OperationKind::CheckPlace(place) => place_values(data, place, budget, &mut visit)?,
        OperationKind::PrepareCall(call) => match data.calls[call.index()].target {
            CallTarget::Value { callee, .. } => visit(callee, budget)?,
            CallTarget::Intrinsic {
                receiver: Some(receiver),
                ..
            } => visit(receiver, budget)?,
            CallTarget::Reference { place } => place_values(data, place, budget, &mut visit)?,
            _ => {}
        },
        OperationKind::Call(call) => {
            let site = &data.calls[call.index()];
            for argument in data.arguments(site.arguments).unwrap_or(&[]) {
                budget.work(WorkKind::Analysis, 1)?;
                match *argument {
                    CallArgument::Value(value) | CallArgument::Spread(value) => {
                        visit(value, budget)?
                    }
                    CallArgument::Reference(place) => {
                        place_values(data, place, budget, &mut visit)?
                    }
                }
            }
        }
        _ => {}
    }
    for child in operation.kind.child_regions() {
        budget.work(WorkKind::Analysis, 1)?;
        if let Some(value) = data.regions[child.index()].result {
            visit(value, budget)?;
        }
    }
    Ok(())
}

/// A read query needs no temporary value list.
pub(super) fn used_in(
    data: &UnitData,
    value: ValueId,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, AllocationError> {
    for region in &data.regions {
        budget.work(WorkKind::Analysis, 1)?;
        if region.result == Some(value) {
            return Ok(true);
        }
    }
    for operation in &data.operations {
        budget.work(WorkKind::Analysis, 1)?;
        let mut found = false;
        visit_reads(data, operation, budget, |read, _| {
            found |= read == value;
            Ok(())
        })?;
        if found {
            return Ok(true);
        }
    }
    Ok(false)
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
    budget: &mut AllocationBudget<'_>,
) -> Result<(Vec<OpId>, u32), AllocationError> {
    budget.retained_phase(|budget| {
        let data = program.unit(unit).expect("a program unit");
        let behaviors = behaviors_in(program, effects, unit, None, budget)?;
        let initialization = effects.initialization();
        let mut in_prefix = budget.filled(Scratch, data.operations.len(), false)?;
        for op in prefix(data) {
            in_prefix[op.index()] = true;
        }
        let mut preparations =
            storage::collect((0..data.calls.len()).map(|_| Vec::new()), Scratch, budget)?;
        let mut uses = budget.filled(Scratch, data.values.len(), 0u32)?;
        for (index, operation) in data.operations.iter().enumerate() {
            budget.work(WorkKind::Analysis, 1)?;
            if let OperationKind::PrepareCall(call) | OperationKind::PrepareReference { call, .. } =
                operation.kind
            {
                budget.push(
                    Scratch,
                    &mut preparations[call.index()],
                    OpId::from_index(index).unwrap(),
                )?;
            }
            visit_reads(data, operation, budget, |value, _| {
                uses[value.index()] = uses[value.index()]
                    .checked_add(1)
                    .ok_or(AllocationError::Capacity)?;
                Ok(())
            })?;
        }
        let unread_local = |cell: CellId| {
            program.cells[cell.index()].binding == CellBinding::Local
                && !cells.read[cell.index()]
                && !cells.visible[cell.index()]
        };
        let mut dead = budget.filled(Scratch, data.operations.len(), false)?;
        let mut removed = Vec::new();
        let mut stores = 0;
        let mut pending = storage::collect(
            (0..data.operations.len())
                .rev()
                .map(|index| OpId::from_index(index).unwrap()),
            Scratch,
            budget,
        )?;
        while let Some(op) = pending.pop() {
            budget.work(WorkKind::Analysis, 1)?;
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
            let preparation = match operation.kind {
                OperationKind::Call(call) => preparations[call.index()].as_slice(),
                _ => &[],
            };
            for op in std::iter::once(op).chain(preparation.iter().copied()) {
                if std::mem::replace(&mut dead[op.index()], true) {
                    continue;
                }
                budget.push(Retained, &mut removed, op)?;
                visit_reads(
                    data,
                    &data.operations[op.index()],
                    budget,
                    |value, budget| {
                        let count = &mut uses[value.index()];
                        *count = count.checked_sub(1).ok_or(AllocationError::Capacity)?;
                        if *count == 0 {
                            budget.push(
                                Scratch,
                                &mut pending,
                                data.values[value.index()].definition,
                            )?;
                        }
                        Ok(())
                    },
                )?;
            }
            if store.is_some() {
                stores += 1;
            }
        }
        storage::release_vec(behaviors, Retained, budget)?;
        Ok((removed, stores))
    })
}

/// The named functions of a module prefix whose cells nothing reads.
fn retired_functions(
    program: &Program<'_>,
    data: &UnitData,
    cells: &CellUsage,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Retirement>, AllocationError> {
    let mut retired = Vec::new();
    for pair in prefix(data).chunks_exact(2) {
        budget.work(WorkKind::Analysis, 1)?;
        let OperationKind::Closure(body) = data.operations[pair[0].index()].kind else {
            continue;
        };
        let OperationKind::Initialize(cell) = data.operations[pair[1].index()].kind else {
            continue;
        };
        if program.cells[cell.index()].binding == CellBinding::Function(body)
            && !cells.read[cell.index()]
            && !cells.visible[cell.index()]
        {
            budget.push(Retained, &mut retired, (pair[0], pair[1], cell))?;
        }
    }
    Ok(retired)
}
