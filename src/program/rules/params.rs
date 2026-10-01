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
) -> Result<bool, &'static str> {
    let program = editor.program();
    let graph = effects.graph();
    let created = created_units(program);
    let mut creations: Vec<Vec<(UnitId, OpId)>> = vec![Vec::new(); program.units.len()];
    for frozen in &program.units {
        for (index, operation) in frozen.data().operations.iter().enumerate() {
            if let OperationKind::Closure(body) = operation.kind {
                creations[body.index()].push((frozen.id(), OpId::from_index(index).unwrap()));
            }
        }
    }
    let eligible: Vec<Change> = program
        .units
        .iter()
        .filter_map(|frozen| {
            change(
                program,
                graph,
                values,
                constants_permitted,
                &created,
                &creations,
                frozen.id(),
            )
        })
        .collect();
    // Each signature's functions, and how many of them may change.
    let mut members: std::collections::HashMap<TypeId, usize> = std::collections::HashMap::new();
    for frozen in &program.units {
        let data = frozen.data();
        if created[frozen.id().index()] {
            if let Some(ty) = data.callable_type {
                *members.entry(ty).or_default() += 1;
            }
        }
    }
    let mut classes: std::collections::HashMap<TypeId, Vec<Change>> =
        std::collections::HashMap::new();
    for change in eligible {
        classes.entry(change.signature).or_default().push(change);
    }
    let mut changes = Vec::new();
    for (signature, mut class) in classes {
        if members.get(&signature).copied() != Some(class.len()) {
            continue;
        }
        // A position leaves when no member reads it; a constant stands for a
        // parameter only in a class of one; a result leaves when no member's
        // calls use it.
        let arity = class[0].unread.len();
        let single = class.len() == 1;
        let unread: Vec<bool> = (0..arity)
            .map(|position| class.iter().all(|change| change.unread[position]))
            .collect();
        let void = class.iter().all(|change| change.void);
        for change in &mut class {
            if !single {
                change
                    .constants
                    .iter_mut()
                    .for_each(|constant| *constant = None);
            }
            change.void = void;
            change.dropped = (0..arity)
                .map(|position| unread[position] || change.constants[position].is_some())
                .collect();
        }
        if class
            .iter()
            .any(|change| change.void || change.dropped.iter().any(|dropped| *dropped))
        {
            changes.extend(class);
        }
    }
    if changes.is_empty() {
        return Ok(false);
    }
    changes.sort_by_key(|change| change.body);
    for change in changes {
        receipt.dropped_parameters += change.dropped.iter().filter(|d| **d).count() as u32;
        receipt.constant_parameters += change.constants.iter().flatten().count() as u32;
        receipt.unused_results += u32::from(change.void);
        execute(editor, change)?;
    }
    Ok(true)
}

fn change(
    program: &Program<'_>,
    graph: &CallGraph,
    values: &ProgramValues,
    constants_permitted: bool,
    created: &[bool],
    creations: &[Vec<(UnitId, OpId)>],
    body: UnitId,
) -> Option<Change> {
    let data = program.unit(body)?;
    if !created[body.index()]
        || data.kind == UnitKind::ModuleInitialization
        || data.suspension != Suspension::None
        || data.constructor_of.is_some()
    {
        return None;
    }
    let Some(Type::Function(signature)) = data.callable_type.and_then(|ty| program.ty(ty)) else {
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
    let mut calls = Vec::with_capacity(edges.len());
    for edge in edges {
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
        let loads = program
            .units
            .iter()
            .flat_map(|frozen| {
                let data = frozen.data();
                data.operations.iter().filter(move |operation| {
                    matches!(operation.kind, OperationKind::Load(place)
                        if data.places.get(place.index()) == Some(&Place::Cell(cell)))
                })
            })
            .count();
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
    let mut unread = vec![false; data.parameters.len()];
    let mut constants = vec![None; data.parameters.len()];
    for (position, &cell) in data.parameters.iter().enumerate() {
        let storage = graph.storage(cell);
        let places: Vec<&Place> = data
            .places
            .iter()
            .filter(|place| root_cell(data, place) == Some(cell))
            .collect();
        if places.is_empty() && !storage.shared {
            unread[position] = true;
            continue;
        }
        if storage.stored
            || storage.referenced
            || storage.shared
            || places.iter().any(|place| !matches!(place, Place::Cell(_)))
        {
            continue;
        }
        if !constants_permitted {
            continue;
        }
        let mut known: Option<facts::StoredExact> = None;
        let mut agreed = true;
        for &(caller, call, _, _) in &calls {
            let unit = program.unit(caller)?;
            let CallArgument::Value(value) =
                unit.arguments(unit.calls[call.index()].arguments)?[position]
            else {
                return None;
            };
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
        let Some(constant) = fold::constant(program, program.cells[cell.index()].ty, &known) else {
            continue;
        };
        // Each read spells the literal where a name stood; each call loses
        // at least a one-character argument and its separator, and the
        // declaration its parameter.
        let reads = data
            .operations
            .iter()
            .filter(|operation| {
                matches!(operation.kind, OperationKind::Load(place)
                    if data.places.get(place.index()) == Some(&Place::Cell(cell)))
            })
            .count();
        let Some(text) = fold::constant_text(program, &constant) else {
            continue;
        };
        if reads * text.saturating_sub(1) <= 2 * calls.len() + 2 {
            constants[position] = Some(constant);
        }
    }

    // A result no call uses.
    let void = !signature.return_type.is_void()
        && calls.iter().all(|&(caller, _, operation, _)| {
            program.unit(caller).is_some_and(|unit| {
                unit.operations[operation.index()]
                    .result
                    .is_none_or(|result| !super::inline::used(unit, result))
            })
        });
    Some(Change {
        body,
        signature: data.callable_type?,
        dropped: vec![false; unread.len()],
        unread,
        constants,
        void,
        creation,
        holder,
        calls,
    })
}

/// The cell a place's storage belongs to, through field projections.
fn root_cell(data: &UnitData, place: &Place) -> Option<CellId> {
    let mut place = place;
    loop {
        match place {
            Place::Cell(cell) => return Some(*cell),
            Place::Field { base, .. } => place = data.places.get(base.index())?,
            _ => return None,
        }
    }
}

fn execute(editor: &mut Editor<'_>, change: Change) -> Result<(), &'static str> {
    let program = editor.program();
    let data = program.unit(change.body).ok_or("a missing body")?;
    let Some(Type::Function(old)) = data.callable_type.and_then(|ty| program.ty(ty)) else {
        return Err("a changed body lost its signature");
    };
    let signature = FunctionSignature {
        params: old
            .params
            .iter()
            .zip(&change.dropped)
            .filter(|(_, dropped)| !**dropped)
            .map(|(parameter, _)| parameter.clone())
            .collect(),
        return_type: if change.void {
            Box::new(Type::Void)
        } else {
            old.return_type.clone()
        },
    };
    let ty = editor.intern_type(Type::Function(FunctionType::new(signature)))?;
    // A call always has a result; a void call's is of type `void`.
    let void = if change.void {
        Some(editor.intern_type(Type::Void)?)
    } else {
        None
    };

    // The body: constants for its constant parameters, no result, and the
    // parameters that stay, renumbered.
    let (data, cells) = editor.unit_and_cells(change.body);
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
                super::edit::make_constant(data, op, constant);
            }
        }
    }
    let parameters = std::mem::take(&mut data.parameters);
    for (position, cell) in parameters.into_iter().enumerate() {
        if change.dropped[position] {
            // An unread cell of the body: nothing initializes or reads it.
            cells[cell.index()].binding = CellBinding::Local;
        } else {
            let at = u32::try_from(data.parameters.len()).map_err(|_| "parameter capacity")?;
            cells[cell.index()].binding = CellBinding::Parameter(at);
            data.parameters.push(cell);
        }
    }
    data.callable_type = Some(ty);
    if let Some(holder) = change.holder {
        cells[holder.index()].ty = ty;
    }

    // The creation's value takes the new type.
    let (unit, op) = change.creation;
    let data = editor.unit_mut(unit);
    if let Some(value) = data.operations[op.index()].result {
        data.values[value.index()].ty = ty;
    }

    // Each call: the callee value's type, the contract, the arguments that
    // stay and, for a result that leaves, no result.
    for (caller, call, operation, callee) in change.calls {
        let data = editor.unit_mut(caller);
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
            .ok_or("invalid argument range")?
            .to_vec();
        let supplied = site.contract.supplied as usize;
        let dropped_supplied = change.dropped[..supplied.min(change.dropped.len())]
            .iter()
            .filter(|dropped| **dropped)
            .count();
        let kept: Vec<CallArgument> = arguments
            .into_iter()
            .zip(&change.dropped)
            .filter(|(_, dropped)| !**dropped)
            .map(|(argument, _)| argument)
            .collect();
        let start = u32::try_from(data.call_arguments.len()).map_err(|_| "argument capacity")?;
        let len = u32::try_from(kept.len()).map_err(|_| "argument capacity")?;
        data.call_arguments.extend(kept);
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
