//! Parameters and returns (plan M7.3; architecture §8.4), on bodies whose
//! calls are all known and direct (`CallGraph::complete_callers`):
//!
//! - a parameter the body never reads leaves the signature and every call;
//!   the call still evaluates its argument, which dead code removes when
//!   nothing requires it;
//! - a parameter every call passes the same exact value becomes that
//!   constant where it is read, when the literal costs no more than the
//!   arguments it removes (L3), and then leaves;
//! - a result no call uses leaves: the body returns nothing, and its calls
//!   yield `void`.
//!
//! Nothing outside the program can call such a body, so its signature is
//! the program's to choose: the body's type, the cell that holds it, its
//! callee values and its calls' contracts all take the new signature.
//!
//! A signature is a coupling class (architecture §9.5): the functions that
//! share it print one shape, and a codec compresses that repetition, so
//! dropping a parameter from some of them but not the others can cost more
//! than it saves (a family of same-signature handlers where only some read
//! their first parameter measured +300 Brotli when dropped one by one). So
//! an edit applies to every function of a
//! signature or to none: a position leaves when no function of that
//! signature reads it, a result when none of their calls uses it, and a
//! constant parameter only where one function has the signature to itself.
//!
//! Exclusions: the ambient `arguments` (it sees the arity), reference
//! parameters, generic instantiations, a default that names another
//! parameter (callers evaluate it by position), a call that omits a default
//! the callee builds, and a callee value that reaches a call through a copy.
//!
//! Prior art: Closure's `OptimizeParameters` (unused and constant actuals,
//! `closure-compiler@0da58e1 .../OptimizeParameters.java:49-51,169,805`) and
//! `OptimizeReturns` (`.../OptimizeReturns.java:197,223`); Terser's
//! `keep_fargs=false` drops only unused trailing parameters.

use super::super::ambient;
use super::super::call_graph::CallGraph;
use super::edit::Editor;
use super::values::ProgramValues;
use super::*;
use crate::check::{FunctionSignature, FunctionType, Type};
use crate::compilation_policy::WorkKind;
use crate::output_budget::AllocationClass::{Retained, Scratch};
use crate::output_budget::{AllocationBudget, AllocationError};

/// One body's new signature and the edits that follow from it.
struct Change {
    body: UnitId,
    /// The signature it shares with its coupling class.
    signature: TypeId,
    /// Per parameter: the body never reads it.
    unread: Vec<bool>,
    /// Per parameter: it leaves (decided per signature).
    dropped: Vec<bool>,
    /// Per parameter: its loads become this constant.
    constants: Vec<Option<Constant>>,
    /// The result leaves.
    void: bool,
    /// The operation that creates the body, and its unit.
    creation: (UnitId, OpId),
    /// The cell that holds the body, if its calls load it from one.
    holder: Option<CellId>,
    /// Each call: its unit, call and the callee value it names.
    calls: Vec<(UnitId, CallId, OpId, ValueId)>,
}

pub(super) fn apply(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    values: &ProgramValues,
    constants_permitted: bool,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            plans(
                editor.program(),
                effects.graph(),
                values,
                constants_permitted,
                budget,
            )
        },
        |changes, editor, budget| {
            for change in changes {
                receipt.dropped_parameters += change.dropped.iter().filter(|d| **d).count() as u32;
                receipt.constant_parameters += change.constants.iter().flatten().count() as u32;
                receipt.unused_results += u32::from(change.void);
                execute(editor, change, budget)?;
            }
            Ok(!changes.is_empty())
        },
    )
}

fn discard(change: Change, budget: &mut AllocationBudget<'_>) -> Result<(), AllocationError> {
    storage::release_vec(change.unread, Retained, budget)?;
    storage::release_vec(change.dropped, Retained, budget)?;
    storage::release_vec(change.constants, Retained, budget)?;
    storage::release_vec(change.calls, Retained, budget)
}

fn plans(
    program: &Program<'_>,
    graph: &CallGraph,
    values: &ProgramValues,
    constants_permitted: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<Change>, super::RuleError> {
    budget.retained_phase(|budget| {
        let created = created_units_in(program, budget)?;
        let mut creations = storage::collect(
            (0..program.units.len()).map(|_| Vec::new()),
            Scratch,
            budget,
        )?;
        let mut members = budget.filled(Scratch, program.types.len(), 0usize)?;
        let mut loads = budget.filled(Scratch, program.cells.len(), (0usize, 0usize))?;
        for frozen in &program.units {
            if created[frozen.id().index()] {
                if let Some(ty) = frozen.data().callable_type {
                    members[ty.index()] += 1;
                }
            }
            for (index, operation) in frozen.data().operations.iter().enumerate() {
                budget.work(WorkKind::Analysis, 1)?;
                match operation.kind {
                    OperationKind::Closure(body) => budget.push(
                        Scratch,
                        &mut creations[body.index()],
                        (frozen.id(), OpId::from_index(index).unwrap()),
                    )?,
                    OperationKind::Load(place) => {
                        if let Some(Place::Cell(cell)) = frozen.data().places.get(place.index()) {
                            loads[cell.index()].0 += 1;
                            if program.cells[cell.index()].owner == frozen.id() {
                                loads[cell.index()].1 += 1;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut eligible = Vec::new();
        for frozen in &program.units {
            if let Some(change) = change(
                program,
                graph,
                values,
                constants_permitted,
                &created,
                &creations,
                &loads,
                frozen.id(),
                budget,
            )? {
                budget.push(Retained, &mut eligible, change)?;
            }
        }
        // Sorted adjacent groups replace allocator-owned hash buckets. Their
        // final application order remains the original body order.
        budget.work(
            WorkKind::Analysis,
            (eligible.len() as u64)
                .saturating_mul(u64::from(usize::BITS - eligible.len().max(1).leading_zeros()) + 1),
        )?;
        eligible.sort_unstable_by_key(|change| (change.signature, change.body));
        let mut start = 0;
        while start < eligible.len() {
            let signature = eligible[start].signature;
            let end =
                start + eligible[start..].partition_point(|change| change.signature == signature);
            let class = &mut eligible[start..end];
            if members[signature.index()] != class.len() {
                for change in class {
                    change.void = false;
                    change.dropped.fill(false);
                }
                start = end;
                continue;
            }
            let arity = class[0].unread.len();
            let single = class.len() == 1;
            budget.work(
                WorkKind::Analysis,
                (arity as u64).saturating_mul(class.len() as u64),
            )?;
            let unread = storage::collect(
                (0..arity).map(|position| class.iter().all(|change| change.unread[position])),
                Scratch,
                budget,
            )?;
            let void = class.iter().all(|change| change.void);
            for change in class {
                if !single {
                    change.constants.fill(None);
                }
                change.void = void;
                for (position, dropped) in change.dropped.iter_mut().enumerate() {
                    *dropped = unread[position] || change.constants[position].is_some();
                }
            }
            storage::release_vec(unread, Scratch, budget)?;
            start = end;
        }
        let mut changes = Vec::new();
        for change in eligible.drain(..) {
            if change.void || change.dropped.iter().any(|dropped| *dropped) {
                budget.push(Retained, &mut changes, change)?;
            } else {
                discard(change, budget)?;
            }
        }
        storage::release_vec(eligible, Retained, budget)?;
        storage::release_vec(created, Retained, budget)?;
        budget.work(
            WorkKind::Analysis,
            (changes.len() as u64)
                .saturating_mul(u64::from(usize::BITS - changes.len().max(1).leading_zeros()) + 1),
        )?;
        changes.sort_unstable_by_key(|change| change.body);
        Ok(changes)
    })
}

fn change(
    program: &Program<'_>,
    graph: &CallGraph,
    values: &ProgramValues,
    constants_permitted: bool,
    created: &[bool],
    creations: &[Vec<(UnitId, OpId)>],
    load_counts: &[(usize, usize)],
    body: UnitId,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<Change>, AllocationError> {
    storage::optional(budget, |attempt| {
        let data = program.unit(body)?;
        attempt.work(
            data.parameters.len() as u64
                + data.places.len() as u64
                + data.captures.len() as u64
                + 1,
        )?;
        if !created[body.index()]
            || data.kind == UnitKind::ModuleInitialization
            || data.suspension != Suspension::None
            || data.constructor_of.is_some()
        {
            return None;
        }
        let Some(Type::Function(signature)) = data.callable_type.and_then(|ty| program.ty(ty))
        else {
            return None;
        };
        if signature.params.len() != data.parameters.len()
            || signature.params.iter().any(|parameter| {
                parameter.passing != crate::primitive::ParameterPassing::Value || parameter.optional
            })
            || data.places.iter().any(|place| {
                matches!(place, Place::Cell(cell)
                if ambient::classify(&program.cells[cell.index()]).is_some())
            })
            || data
                .captures
                .iter()
                .any(|cell| ambient::classify(&program.cells[cell.index()]).is_some())
        {
            return None;
        }
        let edges = graph.complete_callers(body)?;
        if edges.is_empty() || edges.iter().any(|edge| !created[edge.caller.index()]) {
            return None;
        }
        let &[creation] = creations[body.index()].as_slice() else {
            return None;
        };
        let creator = program.unit(creation.0)?;
        let created_value = creator.operations[creation.1.index()].result?;

        // Each call names the body through the value its creation made, or a
        // load of the one cell that holds it.
        let mut holder = None;
        let mut calls = attempt.vector(Retained, edges.len())?;
        for edge in edges {
            attempt.work(1)?;
            let caller = program.unit(edge.caller)?;
            let site = caller.calls.get(edge.call.index())?;
            let CallTarget::Value {
                callee,
                invocation: Invocation::Value,
            } = site.target
            else {
                return None;
            };
            let arguments = caller.arguments(site.arguments)?;
            attempt.work(arguments.len() as u64)?;
            if site.contract.instantiation.is_some()
                || site.contract.signature != data.callable_type
                || arguments.len() != data.parameters.len()
                || arguments
                    .iter()
                    .any(|argument| matches!(argument, CallArgument::Reference(_)))
            {
                return None;
            }
            let definition = &caller.operations[caller.values[callee.index()].definition.index()];
            match definition.kind {
                OperationKind::Closure(unit)
                    if unit == body && (edge.caller, callee) == (creation.0, created_value) => {}
                OperationKind::Load(place) => {
                    let Some(&Place::Cell(cell)) = caller.places.get(place.index()) else {
                        return None;
                    };
                    if *holder.get_or_insert(cell) != cell {
                        return None;
                    }
                }
                _ => return None,
            }
            calls.push((edge.caller, edge.call, edge.operation, callee));
        }
        if let Some(cell) = holder {
            // The holder is initialized with the creation, and every load of it
            // is one of these calls' callee values.
            let loads = load_counts[cell.index()].0;
            let storage = graph.storage(cell);
            if loads != calls.len()
                || storage.stored
                || storage.referenced
                || storage.initializers != 1
                || program.cells[cell.index()].ty != data.callable_type?
            {
                return None;
            }
        }

        // Per parameter: whether the body reads it, and whether every call
        // passes the same exact value.
        let mut unread = attempt.filled(Retained, data.parameters.len(), false)?;
        let mut constants = attempt.collect(Retained, (0..data.parameters.len()).map(|_| None))?;
        let mut roots = attempt.vector(Scratch, data.places.len())?;
        for place in &data.places {
            roots.push(attempt.admit(|budget| root_cell(data, place, budget))?);
        }
        for (position, &cell) in data.parameters.iter().enumerate() {
            let storage = graph.storage(cell);
            attempt.work(data.places.len() as u64 + 1)?;
            let mut reads = 0;
            let mut projected = false;
            for (place, root) in data.places.iter().zip(&roots) {
                if *root == Some(cell) {
                    reads += 1;
                    projected |= !matches!(place, Place::Cell(_));
                }
            }
            if reads == 0 && !storage.shared {
                unread[position] = true;
                continue;
            }
            if storage.stored || storage.referenced || storage.shared || projected {
                continue;
            }
            if !constants_permitted {
                continue;
            }
            let mut known: Option<facts::StoredExact> = None;
            let mut agreed = true;
            for &(caller, call, _, _) in &calls {
                attempt.work(1)?;
                let unit = program.unit(caller)?;
                let CallArgument::Value(value) =
                    unit.arguments(unit.calls[call.index()].arguments)?[position]
                else {
                    return None;
                };
                if unit.operations[unit.values[value.index()].definition.index()]
                    .authored
                    .get(crate::representation::ChoiceFamily::QuoteDelimiter)
                    .is_some()
                {
                    agreed = false;
                    break;
                }
                match values.exact(caller, value) {
                    Some(exact) if known.as_ref().is_none_or(|first| first == exact) => {
                        known = Some(exact.clone());
                    }
                    _ => {
                        agreed = false;
                        break;
                    }
                }
            }
            let (true, Some(known)) = (agreed, known) else {
                continue;
            };
            let Some(constant) = fold::constant(program, program.cells[cell.index()].ty, &known)
            else {
                continue;
            };
            // Each read spells the literal where a name stood; each call loses
            // at least a one-character argument and its separator, and the
            // declaration its parameter.
            let reads = load_counts[cell.index()].1;
            let Some(text) = fold::constant_text(program, &constant) else {
                continue;
            };
            if reads * text.saturating_sub(1) <= 2 * calls.len() + 2 {
                constants[position] = Some(constant);
            }
        }

        // A result no call uses.
        let mut void = !signature.return_type.is_void();
        if void {
            for &(caller, _, operation, _) in &calls {
                let caller = program.unit(caller)?;
                if let Some(result) = caller.operations[operation.index()].result {
                    if attempt.admit(|budget| super::dce::used_in(caller, result, budget))? {
                        void = false;
                        break;
                    }
                }
            }
        }
        Some(Change {
            body,
            signature: data.callable_type?,
            dropped: attempt.filled(Retained, unread.len(), false)?,
            unread,
            constants,
            void,
            creation,
            holder,
            calls,
        })
    })
}

/// The cell a place's storage belongs to, through field projections.
fn root_cell(
    data: &UnitData,
    place: &Place,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<CellId>, AllocationError> {
    let mut place = place;
    loop {
        budget.work(WorkKind::Analysis, 1)?;
        match place {
            Place::Cell(cell) => return Ok(Some(*cell)),
            Place::Field { base, .. } => {
                let Some(base) = data.places.get(base.index()) else {
                    return Ok(None);
                };
                place = base;
            }
            _ => return Ok(None),
        }
    }
}

fn execute(
    editor: &mut Editor<'_>,
    change: &Change,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<(), super::RuleError> {
    let ty = budget.with_temporary_context(
        editor,
        |editor, budget| {
            let program = editor.program();
            let data = program.unit(change.body).ok_or("a missing body")?;
            let Some(Type::Function(old)) = data.callable_type.and_then(|ty| program.ty(ty)) else {
                return Err(super::RuleError::from("a changed body lost its signature"));
            };
            let count = change.dropped.iter().filter(|dropped| !**dropped).count();
            let mut params = budget.vector(Retained, count)?;
            for (parameter, dropped) in old.params.iter().zip(&change.dropped) {
                if *dropped {
                    continue;
                }
                params.push(crate::check::FunctionParameter {
                    ty: storage::ty(&parameter.ty, budget)?,
                    receiver: parameter.receiver,
                    passing: parameter.passing,
                    optional: parameter.optional,
                    rest: parameter.rest,
                });
            }
            let result = if change.void {
                Type::Void
            } else {
                storage::ty(&old.return_type, budget)?
            };
            let return_type = budget.boxed(Retained, result)?;
            budget.retain(
                Retained,
                (std::mem::size_of::<FunctionSignature<'_>>() + 2 * std::mem::size_of::<usize>())
                    as u64,
            )?;
            Ok(Type::Function(FunctionType::new(FunctionSignature {
                params,
                return_type,
            })))
        },
        |ty, editor, budget| editor.intern_type_in(ty, budget),
    )?;
    // A call always has a result; a void call's is of type `void`.
    let void = if change.void {
        Some(editor.intern_type_in(&(Type::Void), budget)?)
    } else {
        None
    };

    // The body: constants for its constant parameters, no result, and the
    // parameters that stay, renumbered.
    let (data, cells) = editor.unit_and_cells_in(change.body, budget)?;
    for index in 0..data.operations.len() {
        let OperationKind::Load(place) = data.operations[index].kind else {
            if change.void && matches!(data.operations[index].kind, OperationKind::Return) {
                data.operations[index].operands = OperandRange { start: 0, len: 0 };
            }
            continue;
        };
        let Some(&Place::Cell(cell)) = data.places.get(place.index()) else {
            continue;
        };
        if let Some(position) = data.parameters.iter().position(|p| *p == cell) {
            if let Some(constant) = change.constants[position].clone() {
                let op = OpId::from_index(index).ok_or("operation capacity")?;
                super::edit::make_constant_in(data, op, constant, budget)?;
            }
        }
    }
    let mut at = 0u32;
    let mut position = 0;
    data.parameters.retain(|&cell| {
        let dropped = change.dropped[position];
        position += 1;
        if dropped {
            // An unread cell of the body: nothing initializes or reads it.
            cells[cell.index()].binding = CellBinding::Local;
        } else {
            cells[cell.index()].binding = CellBinding::Parameter(at);
            at += 1;
        }
        !dropped
    });
    data.callable_type = Some(ty);
    if let Some(holder) = change.holder {
        cells[holder.index()].ty = ty;
    }

    // The creation's value takes the new type.
    let (unit, op) = change.creation;
    let data = editor.unit_mut_in(unit, budget)?;
    if let Some(value) = data.operations[op.index()].result {
        data.values[value.index()].ty = ty;
    }

    // Each call: the callee value's type, the contract, the arguments that
    // stay and, for a result that leaves, no result.
    for &(caller, call, operation, callee) in &change.calls {
        let data = editor.unit_mut_in(caller, budget)?;
        data.values[callee.index()].ty = ty;
        data.calls[call.index()].contract.signature = Some(ty);
        if let (Some(void), Some(result)) = (void, data.operations[operation.index()].result) {
            data.values[result.index()].ty = void;
        }
        if !change.dropped.iter().any(|dropped| *dropped) {
            continue;
        }
        let site = &data.calls[call.index()];
        let arguments = data
            .arguments(site.arguments)
            .ok_or("invalid argument range")?;
        let supplied = site.contract.supplied as usize;
        let dropped_supplied = change.dropped[..supplied.min(change.dropped.len())]
            .iter()
            .filter(|dropped| **dropped)
            .count();
        let kept: Vec<CallArgument> = storage::collect(
            arguments
                .iter()
                .copied()
                .zip(&change.dropped)
                .filter(|(_, dropped)| !**dropped)
                .map(|(argument, _)| argument),
            Scratch,
            budget,
        )?;
        let start = u32::try_from(data.call_arguments.len()).map_err(|_| "argument capacity")?;
        let len = u32::try_from(kept.len()).map_err(|_| "argument capacity")?;
        budget.extend_copy(Retained, &mut data.call_arguments, &kept)?;
        storage::release_vec(kept, Scratch, budget)?;
        let site = &mut data.calls[call.index()];
        site.arguments = ArgumentRange { start, len };
        // Parameter positions changed. The defaults rule recomputes target
        // omission from the new signature in the next fixed-point round.
        site.omit_trailing = 0;
        site.contract.supplied =
            u32::try_from(supplied - dropped_supplied).map_err(|_| "argument capacity")?;
    }
    Ok(())
}
