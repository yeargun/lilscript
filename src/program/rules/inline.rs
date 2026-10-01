//! Removal-only inlining (plan M7.5a; architecture §8.4): a body whose calls
//! are all known is copied into each of them and then retired, when that
//! does not grow the program. The copy runs what the call ran, in the same
//! order: the arguments are already evaluated where the call stands, and
//! the body follows them there.
//!
//! Legality, from the program's facts (v1):
//! - every call is known and direct (`CallGraph::complete_callers`), from
//!   compatible lexical environments, with value arguments for every parameter and no
//!   generic instantiation;
//! - the body is not recursive, does not suspend, is not a constructor or a
//!   `debug` function (R15: `strip_debug` drops its calls), reads no ambient
//!   `this` or `arguments`, and has
//!   one exit: its only `return` ends its entry region, or it has none;
//! - a body with statements goes only into a statement region where no
//!   value computed before the call is still waiting to be used after it:
//!   a statement between them would make the target hold that value.
//!
//! A parameter the program never writes reads its argument directly; the
//! others become local cells of the caller, initialized with the argument.
//! Nested callable bodies and their captured cells are cloned per occurrence;
//! a lexical block renews each dynamic activation and releases native owners.
//!
//! "No growth": a body with one call moves, and its copy may not outnumber
//! the body, its creation and the call operations it replaces. A body with
//! several calls is removed here only when its body is an identity transport.
//! Even a two-operation body can contain a long literal; operation count does
//! not price its duplication. Other duplicates are objective-judged target
//! alternatives, with the retained-call representation still available.
//! Each round inlines an independent set of bodies, callees first: a body
//! copied this round receives no copy, so every copy reads the body as the
//! round found it.
//!
//! Prior art: Closure's `InlineFunctions`/`FunctionInjector` (direct and
//! block inlining at statement-level call sites, `closure-compiler@0da58e1`),
//! Terser's `reduce_vars` single-use inlining (`terser@8fa44c8
//! lib/compress/inline.js`), esbuild's inlining of single-use functions is
//! absent (`f6058f8`); formation's tree inliners (`src/js/inline.rs`,
//! `src/js/blocks.rs`) apply the same conditions to the target tree.

use super::super::ambient;
use super::super::call_graph::{CallEdge, EdgeKind};
use super::edit::{self, Editor, GraftPlan};
use super::*;
use std::collections::{HashMap, HashSet};

/// One call a round inlines.
struct Site {
    caller: UnitId,
    call: CallId,
    operation: OpId,
}

struct Candidate {
    body: UnitId,
    sites: Vec<Site>,
    /// The body's final `return`.
    exit: Option<OpId>,
    /// Per parameter: the program never writes it, so it reads its argument.
    forwarded: Vec<bool>,
    /// The copy declares cells of owned types at its top level: it keeps
    /// its own scope, a block, so their storage ends where the call's did
    /// (native releases owned storage at scope end). A used result crosses the
    /// block through one private result cell, preserving the returned owner.
    /// A cell of a scalar type owns nothing, so its scope is unobservable.
    scoped: bool,
    children: Vec<UnitId>,
}

pub(super) fn apply(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    receipt: &mut RuleReceipt,
    native: bool,
) -> Result<bool, &'static str> {
    let program = editor.program();
    let graph = effects.graph();
    let created = created_units(program);
    // Units whose entry region declares a local that owns storage.
    let mut declares = vec![false; program.units.len()];
    for cell in program.cells.iter() {
        if matches!(cell.binding, CellBinding::Local)
            && !scalar(program, cell.ty)
            && program
                .unit(cell.owner)
                .is_some_and(|owner| owner.entry == cell.region)
        {
            declares[cell.owner.index()] = true;
        }
    }
    // A `debug` function's calls are `strip_debug`'s to drop (R15): its body
    // stays a body.
    let mut debug = vec![false; program.units.len()];
    for cell in program.cells.iter() {
        if let (true, CellBinding::Function(unit)) = (cell.debug, cell.binding) {
            debug[unit.index()] = true;
        }
    }
    let mut chosen = Vec::new();
    let mut bodies = HashSet::new();
    let mut receivers = HashSet::new();
    for component in graph.components() {
        for &body in component {
            if debug[body.index()] {
                continue;
            }
            let Some(candidate) = candidate(program, effects, &created, &declares, body, native, receipt) else {
                continue;
            };
            if receivers.contains(&body)
                || candidate
                    .sites
                    .iter()
                    .any(|site| bodies.contains(&site.caller))
            {
                continue;
            }
            bodies.insert(body);
            receivers.extend(candidate.sites.iter().map(|site| site.caller));
            chosen.push(candidate);
        }
    }
    if chosen.is_empty() {
        return Ok(false);
    }
    let creators = creators(program, &created);
    for candidate in &chosen {
        let source = editor.handle(candidate.body);
        for site in &candidate.sites {
            inline(editor, source.data(), candidate, site, &creators)?;
            receipt.inlined_calls += 1;
            receipt.cloned_closure_units += candidate.children.len() as u32;
        }
        receipt.inlined_bodies += 1;
    }
    Ok(true)
}

/// Operations that are statements: a body holding one is not an expression.
fn statement(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::Initialize(_)
            | OperationKind::Declare(_)
            | OperationKind::Store(_)
            | OperationKind::CheckPlace(_)
            | OperationKind::If { .. }
            | OperationKind::Loop { .. }
            | OperationKind::Try { .. }
            | OperationKind::Block(_)
            | OperationKind::ForIn { .. }
            | OperationKind::ForOf { .. }
            | OperationKind::Throw
            | OperationKind::Break
            | OperationKind::Continue
    )
}

fn candidate(
    program: &Program<'_>,
    effects: &ProgramEffects,
    created: &[bool],
    declares: &[bool],
    body: UnitId,
    native: bool,
    receipt: &mut RuleReceipt,
) -> Option<Candidate> {
    let graph = effects.graph();
    let data = program.unit(body)?;
    if !created[body.index()]
        || data.kind == UnitKind::ModuleInitialization
        || data.suspension != Suspension::None
        || data.constructor_of.is_some()
        || graph.recursive(body)
        || data
            .parameters
            .iter()
            .any(|&cell| program.is_reference_parameter(cell))
    {
        return None;
    }
    // Calls from units nothing creates never run; they are emptied.
    let edges: Vec<&CallEdge> = graph
        .complete_callers(body)?
        .iter()
        .filter(|edge| created[edge.caller.index()])
        .collect();
    if edges.is_empty() {
        return None;
    }

    let children = super::inline_clones::children(program, body)?;
    // The body's shape: one exit, nothing it cannot lend to a caller.
    let entry = &data.regions[data.entry.index()].operations;
    let mut exit = None;
    let mut expression = true;
    for (index, operation) in data.operations.iter().enumerate() {
        let op = OpId::from_index(index)?;
        match operation.kind {
            OperationKind::Return => {
                if exit.is_some() || entry.last() != Some(&op) {
                    return None;
                }
                exit = Some(op);
            }
            OperationKind::IsUndefined
            | OperationKind::Yield { .. }
            | OperationKind::Await
            | OperationKind::SuperConstruct
            | OperationKind::LoadModule { .. } => return None,
            ref kind => expression &= !statement(kind),
        }
    }
    // Ambient `this` and `arguments` belong to the activation, and a foreign
    // cell the body declares is not a cell a caller can hold.
    let foreign = |cell: &CellId| {
        let storage = &program.cells[cell.index()];
        ambient::classify(storage).is_some()
            || (storage.owner == body && storage.binding == CellBinding::Foreign)
    };
    if data.captures.iter().any(foreign)
        || data.places.iter().any(|place| match place {
            Place::Cell(cell) => foreign(cell),
            _ => false,
        })
    {
        return None;
    }
    let returned = exit.and_then(|exit| {
        data.operands(data.operations[exit.index()].operands)
            .and_then(|operands| operands.first().copied())
    });
    // A parameter reads its argument when nothing writes it and every load
    // and argument has its exact type, so a use sees the type it saw.
    let mut forwarded: Vec<bool> = data
        .parameters
        .iter()
        .map(|&cell| {
            let storage = graph.storage(cell);
            // A reference-valued parameter written only through a field still
            // denotes the same evaluated handle. Captured formals need a fresh
            // lexical bank; value products keep their private mutable copy.
            !storage.referenced && !storage.shared && (!storage.stored ||
                !matches!(program.types[program.cells[cell.index()].ty.index()],
                    Type::Struct(_) | Type::StructInstance { .. } | Type::Nullable(_))
                && !data.operations.iter().any(|op| matches!(op.kind,
                    OperationKind::Store(place) if matches!(data.places[place.index()], Place::Cell(id) if id == cell))))
        })
        .collect();
    for operation in &data.operations {
        if let (OperationKind::Load(place), Some(result)) = (&operation.kind, operation.result) {
            if let Some(Place::Cell(cell)) = data.places.get(place.index()) {
                if let Some(position) = data.parameters.iter().position(|p| p == cell) {
                    forwarded[position] &=
                        data.values[result.index()].ty == program.cells[cell.index()].ty;
                }
            }
        }
    }

    let mut sites = Vec::with_capacity(edges.len());
    let mut callee_loads = 0usize;
    for edge in edges {
        if edge.kind != EdgeKind::Call {
            return None;
        }
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
        if (caller.module != data.module && data.captures.iter().any(|cell|
            program.unit(program.cells[cell.index()].owner).is_none_or(|owner|
                owner.kind != UnitKind::ModuleInitialization)))
            || site.contract.instantiation.is_some()
            || arguments.len() != data.parameters.len()
            || arguments
                .iter()
                .any(|argument| matches!(argument, CallArgument::Reference(_)))
        {
            return None;
        }
        for (position, argument) in arguments.iter().enumerate() {
            match argument {
                CallArgument::Value(value) => {
                    forwarded[position] &= caller.values[value.index()].ty
                        == program.cells[data.parameters[position].index()].ty;
                }
                // A spread is never a forwarded parameter.
                CallArgument::Spread(_) => forwarded[position] = false,
                CallArgument::Reference(_) => {}
            }
        }
        // A used result is the returned value itself, of the same type.
        let operation = caller.operations.get(edge.operation.index())?;
        if let Some(result) = operation.result.filter(|result| used(caller, *result)) {
            match returned {
                Some(returned)
                    if data.values[returned.index()].ty == caller.values[result.index()].ty => {}
                _ => return None,
            }
        }
        let definition = &caller.operations[caller.values[callee.index()].definition.index()];
        callee_loads += usize::from(matches!(definition.kind, OperationKind::Load(_)));
        sites.push(Site {
            caller: edge.caller,
            call: edge.call,
            operation: edge.operation,
        });
    }

    // An expression stands anywhere when it reads its arguments where the
    // call evaluated them: once each, in order, before anything observable.
    // Otherwise the copy is a statement (a parameter that does not read its
    // argument is a cell initialized where the copy stands), and goes only
    // where statements may.
    // Arguments the target can spell where the copy reads them.
    let movable = sites.iter().all(|site| {
        program.unit(site.caller).is_some_and(|caller| {
            caller
                .arguments(caller.calls[site.call.index()].arguments)
                .is_some_and(|arguments| {
                    arguments.iter().all(|argument| match argument {
                        CallArgument::Value(value) => {
                            let definition =
                                &caller.operations[caller.values[value.index()].definition.index()];
                            match definition.kind {
                                OperationKind::Constant(_) => true,
                                OperationKind::Load(place) => {
                                    matches!(caller.places.get(place.index()), Some(Place::Cell(_)))
                                }
                                _ => false,
                            }
                        }
                        CallArgument::Reference(_) | CallArgument::Spread(_) => false,
                    })
                })
        })
    });
    let expression = expression
        && forwarded.iter().all(|forwarded| *forwarded)
        && reads_in_order(
            program,
            data,
            &behaviors(program, effects, body, None),
            movable,
        );
    // One proved activation needs no renewed JavaScript capture bank. Native
    // still ends its owned locals at the original call's exit. Unknown and
    // repeated activations always keep the lexical block.
    let capture_scope = if children.is_empty() { false } else {
        let (frequency, work) = graph.frequency(program, body);
        receipt.call_frequency_work += work as u64;
        native || frequency != super::super::call_graph::CallFrequency::AtMostOnce
    };
    let scoped = capture_scope || declares[body.index()]
        || forwarded
            .iter()
            .zip(&data.parameters)
            .any(|(forwarded, &cell)| {
                !*forwarded && !scalar(program, program.cells[cell.index()].ty)
            });
    if (!expression || scoped)
        && sites.iter().any(|site| {
            program
                .unit(site.caller)
                .is_none_or(|caller| !statement_site(caller, site.operation))
        })
    {
        return None;
    }

    // No growth: each copy adds the body (less its exit and forwarded
    // loads) and a cell initialization per other parameter; it removes the
    // call, its preparation and, for a function held in a cell, the load of
    // the callee. Retiring the body removes it, its creation and the
    // initialization of the cell that held it.
    let forwarded_loads = data
        .operations
        .iter()
        .filter(|operation| match operation.kind {
            OperationKind::Load(place) => match data.places.get(place.index()) {
                Some(Place::Cell(cell)) => data
                    .parameters
                    .iter()
                    .position(|parameter| parameter == cell)
                    .is_some_and(|position| forwarded[position]),
                _ => false,
            },
            _ => false,
        })
        .count();
    let cells = forwarded.iter().filter(|forwarded| !**forwarded).count();
    let copy = data.operations.len() - usize::from(exit.is_some()) - forwarded_loads + cells;
    let held = callee_loads > 0;
    let growth = if sites.len() == 1 {
        // The body moves: the copy may not outnumber the body, its creation,
        // the cell that held it and the call operations.
        copy <= data.operations.len() + 1 + usize::from(held) + 2 + callee_loads
    } else {
        // Only identity transports add no duplicated representation. Counted
        // operations do not bound literal bytes or predict a codec's repeats.
        // Nonempty bodies compete through the expression-inlining family.
        expression && copy == 0
    };
    growth.then_some(Candidate {
        body,
        sites,
        exit,
        forwarded,
        scoped,
        children,
    })
}

/// Whether the body reads its parameters as the call evaluated its
/// arguments: each at most once, in parameter order, in its entry region,
/// with nothing before the last read but constants and parameter reads
/// (formation's tree inliner holds the same condition, `src/js/inline.rs`).
/// The target then spells each argument where it is read, and evaluation
/// keeps its order. With `arithmetic` (every argument is a constant or a
/// cell's value, which the target reads where it is used), pure arithmetic
/// on primitives may come between the reads too.
fn reads_in_order(
    program: &Program<'_>,
    data: &UnitData,
    behaviors: &[EvaluationBehavior],
    arithmetic: bool,
) -> bool {
    let entry = &data.regions[data.entry.index()].operations;
    let parameter = |place: PlaceId| {
        let mut place = place;
        loop {
            match data.places.get(place.index()) {
                Some(Place::Cell(cell)) => {
                    return data.parameters.iter().position(|p| p == cell);
                }
                Some(Place::Field { base, .. }) => place = *base,
                _ => return None,
            }
        }
    };
    let read = |operation: &Operation| match operation.kind {
        OperationKind::Load(place) => parameter(place),
        _ => None,
    };
    // Every read stands in the entry region: none in a branch or a loop.
    let in_entry = data
        .operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| read(operation).is_some())
        .all(|(index, _)| entry.iter().any(|op| op.index() == index));
    if !in_entry {
        return false;
    }
    let mut next = 0;
    let mut last = 0;
    for (at, op) in entry.iter().enumerate() {
        if let Some(position) = read(&data.operations[op.index()]) {
            if position < next {
                return false;
            }
            next = position + 1;
            last = at;
        }
    }
    // Every caller is known and typed (`candidate`), so a parameter holds a
    // value of its type, and arithmetic on primitives runs no user code.
    let primitive = |value: &ValueId| {
        matches!(
            program.ty(data.values[value.index()].ty),
            Some(
                crate::check::Type::Int
                    | crate::check::Type::Float
                    | crate::check::Type::Bool
                    | crate::check::Type::String
            )
        )
    };
    entry[..last].iter().all(|op| {
        let operation = &data.operations[op.index()];
        match operation.kind {
            OperationKind::Constant(_) => true,
            OperationKind::Load(_) => read(operation).is_some(),
            OperationKind::IntBinary(_)
            | OperationKind::Binary(_)
            | OperationKind::Unary { .. }
            | OperationKind::CopyValue
            | OperationKind::TypeTest(_)
            | OperationKind::Template => {
                arithmetic
                    && (!behaviors[op.index()].requires_evaluation()
                        || data
                            .operands(operation.operands)
                            .is_some_and(|operands| operands.iter().all(primitive)))
            }
            _ => false,
        }
    })
}

/// Whether anything reads `value`.
pub(super) fn used(data: &UnitData, value: ValueId) -> bool {
    let mut scratch = Vec::new();
    data.regions
        .iter()
        .any(|region| region.result == Some(value))
        || data.operations.iter().any(|operation| {
            super::dce::reads(data, operation, &mut scratch);
            scratch.contains(&value)
        })
}

/// A statement region, where the target writes statements (not a loop's
/// test or update, not an expression's operand), in which no value computed
/// before `operation` is read after it, and no call prepared before it is
/// made after it: the site is not inside another call's arguments.
fn statement_site(data: &UnitData, operation: OpId) -> bool {
    let region = data.operations[operation.index()].region;
    if data.regions[region.index()].result.is_some() {
        return false;
    }
    if let Some(parent) = data.regions[region.index()].parent {
        let owner = data.regions[parent.index()]
            .operations
            .iter()
            .map(|op| &data.operations[op.index()].kind)
            .find(|kind| kind.child_regions().any(|child| child == region));
        match owner {
            Some(OperationKind::Loop { body, .. }) if *body != region => return false,
            Some(OperationKind::ShortCircuit { .. } | OperationKind::Select { .. }) => {
                return false
            }
            _ => {}
        }
    }
    let list = &data.regions[region.index()].operations;
    let Some(position) = list.iter().position(|op| *op == operation) else {
        return false;
    };
    let mut prepared = HashSet::new();
    for op in &list[..position] {
        match data.operations[op.index()].kind {
            OperationKind::PrepareCall(call) | OperationKind::PrepareReference { call, .. } => {
                prepared.insert(call);
            }
            OperationKind::Call(call) => {
                prepared.remove(&call);
            }
            _ => {}
        }
    }
    if let OperationKind::Call(call) = data.operations[operation.index()].kind {
        prepared.remove(&call);
    }
    if !prepared.is_empty() {
        return false;
    }
    // A constant is spelled where it is read, so it never waits.
    let pending: HashSet<ValueId> = list[..position]
        .iter()
        .map(|op| &data.operations[op.index()])
        .filter(|operation| !matches!(operation.kind, OperationKind::Constant(_)))
        .filter_map(|operation| operation.result)
        .collect();
    if pending.is_empty() {
        return true;
    }
    let mut scratch = Vec::new();
    let mut stack: Vec<OpId> = list[position + 1..].to_vec();
    while let Some(op) = stack.pop() {
        let operation = &data.operations[op.index()];
        super::dce::reads(data, operation, &mut scratch);
        if scratch.iter().any(|value| pending.contains(value)) {
            return false;
        }
        for child in operation.kind.child_regions() {
            stack.extend(data.regions[child.index()].operations.iter().copied());
        }
    }
    true
}

/// The operations that evaluate a call's arguments: those between its
/// preparation and the call in the call's region, when none declares a cell
/// and no value they compute is read after the call. Otherwise none.
fn arguments_evaluation(data: &UnitData, region: RegionId, site: &Site) -> Vec<OpId> {
    let list = &data.regions[region.index()].operations;
    let prepared = list.iter().position(|op| {
        matches!(data.operations[op.index()].kind, OperationKind::PrepareCall(call) if call == site.call)
    });
    let called = list.iter().position(|op| *op == site.operation);
    let (Some(prepared), Some(called)) = (prepared, called) else {
        return Vec::new();
    };
    if prepared >= called {
        return Vec::new();
    }
    let evaluation: Vec<OpId> = list[prepared + 1..called].to_vec();
    let mut defined = HashSet::new();
    let mut stack = evaluation.clone();
    while let Some(op) = stack.pop() {
        let operation = &data.operations[op.index()];
        if matches!(
            operation.kind,
            OperationKind::Initialize(_) | OperationKind::Declare(_)
        ) {
            return Vec::new();
        }
        defined.extend(operation.result);
        for child in operation.kind.child_regions() {
            stack.extend(data.regions[child.index()].operations.iter().copied());
        }
    }
    let mut scratch = Vec::new();
    let mut later: Vec<OpId> = list[called + 1..].to_vec();
    while let Some(op) = later.pop() {
        let operation = &data.operations[op.index()];
        super::dce::reads(data, operation, &mut scratch);
        if scratch.iter().any(|value| defined.contains(value)) {
            return Vec::new();
        }
        for child in operation.kind.child_regions() {
            later.extend(data.regions[child.index()].operations.iter().copied());
        }
    }
    if data.regions[region.index()]
        .result
        .is_some_and(|value| defined.contains(&value))
    {
        return Vec::new();
    }
    evaluation
}

/// The unit whose operation creates each created body.
fn creators(program: &Program<'_>, created: &[bool]) -> Vec<Option<UnitId>> {
    let mut creators = vec![None; program.units.len()];
    for frozen in &program.units {
        if !created[frozen.id().index()] {
            continue;
        }
        for operation in &frozen.data().operations {
            if let OperationKind::Closure(body) = operation.kind {
                creators[body.index()] = Some(frozen.id());
            }
        }
    }
    creators
}

fn inline(
    editor: &mut Editor<'_>,
    body: &UnitData,
    candidate: &Candidate,
    site: &Site,
    creators: &[Option<UnitId>],
) -> Result<(), &'static str> {
    let program = editor.program();
    let caller = program.unit(site.caller).ok_or("a missing caller")?;
    let at = &caller.operations[site.operation.index()];
    let (region, span) = (at.region, at.span);
    let result_type = at.result.filter(|&value| used(caller, value))
        .map(|value| caller.values[value.index()].ty);
    let foreign_module = caller.module != body.module;
    let arguments: Vec<ValueId> = caller
        .arguments(caller.calls[site.call.index()].arguments)
        .ok_or("invalid argument range")?
        .iter()
        .map(|argument| match argument {
            CallArgument::Value(value) => Ok(*value),
            CallArgument::Reference(_) => Err("an inlined call passes a reference"),
            CallArgument::Spread(_) => Err("an inlined call spreads an argument"),
        })
        .collect::<Result<_, _>>()?;
    // A scoped copy stands in a block at the call: the block's region is
    // the next one the caller's arena takes.
    let scope = if candidate.scoped {
        Some(RegionId::from_index(caller.regions.len()).ok_or("region capacity")?)
    } else {
        None
    };
    let home = scope.unwrap_or(region);
    let regions = edit::graft_regions(
        body,
        caller.regions.len() + usize::from(scope.is_some()),
        home,
    )?;

    // The body's own cells: a forwarded parameter reads its argument, every
    // other cell is cloned where the copy stands.
    let mut forwards = HashMap::new();
    let mut clones = Vec::new();
    for (index, cell) in program.cells.iter().enumerate() {
        if cell.owner != candidate.body || cell.binding == CellBinding::Foreign {
            continue;
        }
        let id = CellId::from_index(index).ok_or("cell capacity")?;
        let parameter = body
            .parameters
            .iter()
            .position(|&parameter| parameter == id);
        if let Some(position) = parameter.filter(|&position| candidate.forwarded[position]) {
            forwards.insert(id, arguments[position]);
            continue;
        }
        let mut clone = cell.clone();
        clone.owner = site.caller;
        // A parameter's copy is initialized where the copy stands.
        clone.region = match parameter {
            Some(_) => home,
            None => regions
                .get(cell.region.index())
                .copied()
                .flatten()
                .unwrap_or(home),
        };
        clone.binding = CellBinding::Local;
        clone.synthetic = true;
        clone.observable_before_initialization &= parameter.is_none();
        clones.push((id, clone));
    }
    let mut cells = HashMap::new();
    for (id, clone) in clones {
        cells.insert(id, editor.add_cell(clone)?);
    }
    let units = super::inline_clones::clone(editor, &candidate.children, &mut cells)?;
    let root_bindings: Vec<_> = cells.iter().filter_map(|(&old, &new)| {
        let original = &editor.program().cells[old.index()];
        if original.owner != candidate.body { return None; }
        let CellBinding::Function(unit) = original.binding else { return None };
        units.get(&unit).copied().map(|unit| (new, unit))
    }).collect();
    if !root_bindings.is_empty() {
        let (_, table) = editor.unit_and_cells(site.caller);
        for (cell, unit) in root_bindings { table[cell.index()].binding = CellBinding::Function(unit); }
    }
    let result_cell = if scope.is_some() {
        if let Some(ty) = result_type {
            Some(editor.add_cell(Cell {
                source_symbol: None, name: "inline_result".into(), ty,
                owner: site.caller, region, declaration: span, reassigned: true,
                observable_before_initialization: false, binding: CellBinding::Local,
                synthetic: true, declared_pure: false, debug: false,
            })?)
        } else { None }
    } else { None };
    let captures: Vec<CellId> = body.captures.clone();

    let mut source = body.clone();
    for op in &mut source.operations {
        if let OperationKind::Closure(unit) = &mut op.kind {
            *unit = units.get(unit).copied().unwrap_or(*unit);
        }
        // Node IDs are local to the source module. The copied operation has a
        // new call-site origin; its nested callable keeps its original module.
        if foreign_module { op.origin = None; op.span = span; }
    }

    let data = editor.unit_mut(site.caller);
    if let Some(scope) = scope {
        if scope.index() != data.regions.len() {
            return Err("a scoped copy's region moved");
        }
        data.regions.push(Region {
            parent: Some(region),
            operations: Vec::new(),
            result: None,
            span,
        });
    }
    let grafted = edit::graft(
        &source,
        data,
        home,
        &GraftPlan {
            cells: &cells,
            forwards: &forwards,
            exit: candidate.exit,
        },
    )?;
    let mut inserted = Vec::with_capacity(body.parameters.len() + grafted.operations.len());
    for (position, parameter) in body.parameters.iter().enumerate() {
        if let Some(&clone) = cells.get(parameter) {
            let (op, _) = edit::push_operation(
                data,
                OperationKind::Initialize(clone),
                &[arguments[position]],
                None,
                home,
                span,
            )?;
            inserted.push(op);
        }
    }
    inserted.extend(grafted.operations);
    let mut returned = grafted.result;
    if let Some(scope) = scope {
        let result_place = if let Some(cell) = result_cell {
            let place = PlaceId::from_index(data.places.len()).ok_or("inline result place")?;
            data.places.push(Place::Cell(cell));
            let value = returned.ok_or("inline result has no returned value")?;
            let (store, _) = edit::push_operation(data, OperationKind::Store(place), &[value], None, scope, span)?;
            inserted.push(store);
            Some(place)
        } else { None };
        // The arguments are evaluated between the call's preparation and the
        // call: in the block they run at the same point, and their values
        // stay inside it, where the copy reads them.
        let mut evaluation = arguments_evaluation(data, region, site);
        for &op in &evaluation {
            data.operations[op.index()].region = scope;
            let children: Vec<RegionId> =
                data.operations[op.index()].kind.child_regions().collect();
            for child in children {
                data.regions[child.index()].parent = Some(scope);
            }
        }
        data.regions[region.index()]
            .operations
            .retain(|op| !evaluation.contains(op));
        evaluation.extend(inserted);
        data.regions[scope.index()].operations = evaluation;
        inserted = Vec::new();
        if let Some(cell) = result_cell {
            let (declare, _) = edit::push_operation(data, OperationKind::Declare(cell), &[], None, region, span)?;
            inserted.push(declare);
        }
        let (block, _) =
            edit::push_operation(data, OperationKind::Block(scope), &[], None, region, span)?;
        inserted.push(block);
        returned = if let Some(place) = result_place {
            let (load, value) = edit::push_operation(data, OperationKind::Load(place), &[], result_type, region, span)?;
            inserted.push(load);
            value
        } else { None };
    }
    let list = &mut data.regions[region.index()].operations;
    let position = list
        .iter()
        .position(|op| *op == site.operation)
        .ok_or("an inlined call left its region")?;
    list.splice(position..position, inserted);
    let preparations: Vec<OpId> = data
        .operations
        .iter()
        .enumerate()
        .filter(|(_, operation)| match operation.kind {
            OperationKind::PrepareCall(call) | OperationKind::PrepareReference { call, .. } => {
                call == site.call
            }
            _ => false,
        })
        .filter_map(|(index, _)| OpId::from_index(index))
        .collect();
    for op in preparations {
        edit::detach(data, op);
    }
    // A used result has the returned value's type (`candidate`).
    if let (Some(result), Some(returned)) = (
        data.operations[site.operation.index()].result,
        returned,
    ) {
        edit::substitute(data, result, returned);
    }
    edit::detach(data, site.operation);

    // The copy reads what the body captured: the caller captures it too,
    // and so does each unit between the caller and the cell's owner.
    for cell in captures {
        let owner = editor.program().cells[cell.index()].owner;
        let global = editor.program().unit(owner).is_some_and(|data| data.kind == UnitKind::ModuleInitialization);
        let mut unit = site.caller;
        while unit != owner {
            let data = editor.unit_mut(unit);
            if data.captures.contains(&cell) {
                break;
            }
            data.captures.push(cell);
            if global && data.kind == UnitKind::ModuleInitialization { break; }
            unit = creators[unit.index()].ok_or("a capture has no path to its owner")?;
        }
    }
    Ok(())
}
