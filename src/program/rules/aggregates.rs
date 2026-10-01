//! Consumers of shared allocation/field evidence. Source evaluations stay in
//! place; replacing storage never moves an initializer or an argument effect.
use super::super::aggregates::{self, Escape, Key, ProgramAggregates};
use super::super::uses::{self, Event, ValueUse};
use super::edit::{self, Editor};
use super::*;
use std::collections::HashMap;

pub(super) fn apply(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    request: RuleRequest,
    receipt: &mut RuleReceipt,
) -> Result<bool, &'static str> {
    if (request.fold || request.scalar) && normalize_objects(editor, receipt)? {
        return Ok(true);
    }
    if request.dead_code && collect_stores(editor, request.pristine_builtins, receipt)? {
        return Ok(true);
    }
    let facts = editor.program().aggregates(request.seal);
    if !facts.complete {
        receipt.aggregate_limits += 1;
        return Ok(false);
    }
    receipt.aggregate_analysis_work = receipt
        .aggregate_analysis_work
        .saturating_add(facts.work as u64);
    // Source folds/removals precede a representation edit. Fresh facts on the
    // next round see its complete new use set, never a half-updated allocation.
    if request.fold && namespaces(editor, effects, &facts, receipt)? {
        return Ok(true);
    }
    if fields(editor, effects, &facts, request, receipt) {
        return Ok(true);
    }
    if !request.scalar {
        return Ok(false);
    }
    if record_aliases(editor, effects, &facts, receipt) {
        return Ok(true);
    }
    // Analysis cannot grow quadratically without a ceiling. A refused site
    // retains its storage; subsequent rounds may expose a smaller subject.
    let scan = editor
        .program()
        .units
        .iter()
        .map(|u| u.data().values.len() + u.data().operations.len())
        .sum::<usize>()
        .saturating_add(editor.program().cells.len())
        .max(1);
    let limit = (1usize << 22) / scan;
    let plans: Vec<_> = (0..facts.sites.len())
        .filter(|&site| {
            let allocation = &facts.sites[site];
            allocation.closed() && allocation.escape == Escape::Local && allocation.scalar_kind
        })
        .take(limit)
        .filter_map(|site| scalar_plan(editor.program(), effects, &facts, site))
        .collect();
    let plans: Vec<_> = plans
        .into_iter()
        .filter(|plan| {
            let allocation = &facts.sites[plan.site];
            // Native captures box each mutable cell. Keep the packed reference
            // when splitting would create more owners than its handle + object.
            !request.native
                || !allocation.captured
                || editor.program().unit(allocation.unit).unwrap().kind
                    == UnitKind::ModuleInitialization
                || allocation
                    .fields
                    .iter()
                    .filter(|f| f.read || !request.dead_code)
                    .count()
                    <= 2
        })
        .collect();
    if plans.is_empty() {
        return Ok(false);
    }
    for plan in plans {
        scalarize(editor, &facts, plan, request.dead_code, receipt)?;
    }
    Ok(true)
}

fn fields(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    request: RuleRequest,
    receipt: &mut RuleReceipt,
) -> bool {
    let program = editor.program();
    let mut constants = Vec::new();
    let mut forwards = Vec::new();
    let mut removed = Vec::new();
    for unit in &program.units {
        let id = unit.id();
        let data = unit.data();
        let dominance = super::super::activation::StructuredDominance::build(
            data,
            vec![None; data.regions.len()],
            vec![0; data.operations.len()],
            |_| Ok::<(), ()>(()),
        )
        .unwrap();
        let behavior = behaviors(program, effects, id, None);
        for region in &data.regions {
            let mut stores: HashMap<(usize, usize, ValueId), OpId> = HashMap::new();
            for &op in &region.operations {
                let operation = &data.operations[op.index()];
                let place = match operation.kind {
                    OperationKind::Load(p)
                    | OperationKind::Store(p)
                    | OperationKind::CheckPlace(p) => Some(p),
                    _ => None,
                };
                let projection = place
                    .and_then(|p| facts.projection(program, id, p))
                    .filter(|(site, _)| facts.sites[*site].closed());
                if let Some((site, field)) = projection {
                    let allocation = &facts.sites[site];
                    let slot = &allocation.fields[field];
                    match operation.kind {
                        OperationKind::Load(_) => {
                            stores.retain(|&(s, f, _), _| (s, f) != (site, field));
                            if request.fold {
                                if let Some(constant) = &slot.constant {
                                    // Longer literals retain their existing storage/representation
                                    // opportunity; this is the ordinary short-field default.
                                    let short = match constant {
                                        Constant::Boolean(_) | Constant::Null => true,
                                        Constant::Integer(v) => v.to_string().len() <= 3,
                                        _ => false,
                                    };
                                    if short {
                                        constants.push((id, op, constant.clone()));
                                    }
                                } else if !slot.written
                                    && allocation.unit == id
                                    && dominance
                                        .after(data, allocation.operation, op, |_| Ok::<(), ()>(()))
                                        .unwrap_or(false)
                                {
                                    let result = operation.result.unwrap();
                                    if !normalizes_null(
                                        program,
                                        facts,
                                        site,
                                        data.values[result.index()].ty,
                                    ) && data.values[result.index()].ty
                                        == data.values[slot.initial.index()].ty
                                    {
                                        forwards.push((id, op, result, slot.initial));
                                    }
                                }
                            }
                        }
                        OperationKind::Store(_) if request.dead_code => {
                            // The RHS is an earlier operation and remains evaluated.
                            let receiver = aggregates::projection(program, data, place.unwrap())
                                .unwrap()
                                .0;
                            if let Some(previous) = stores.insert((site, field, receiver), op) {
                                removed.push((id, previous));
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                if operation.kind.child_regions().next().is_some()
                    || behavior[op.index()].requires_evaluation()
                {
                    stores.clear();
                }
            }
        }
    }
    if constants.is_empty() && forwards.is_empty() && removed.is_empty() {
        return false;
    }
    for (unit, op, value) in constants {
        edit::make_constant(editor.unit_mut(unit), op, value);
        receipt.folded_fields += 1;
    }
    let mut substitutions = HashMap::new();
    for (unit, op, result, mut value) in forwards {
        while let Some(&next) = substitutions.get(&(unit, value)) {
            value = next;
        }
        let data = editor.unit_mut(unit);
        edit::detach(data, op);
        edit::substitute(data, result, value);
        substitutions.insert((unit, result), value);
        receipt.folded_fields += 1;
    }
    removed.sort_unstable();
    removed.dedup();
    for (unit, op) in removed {
        let data = editor.unit_mut(unit);
        if let Some(result) = data.operations[op.index()].result {
            let value = data.operands(data.operations[op.index()].operands).unwrap()[0];
            edit::substitute(data, result, value);
        }
        edit::detach(data, op);
        receipt.removed_field_stores += 1;
    }
    true
}

struct ScalarPlan {
    site: usize,
    aliases: Vec<CellId>,
    remove: Vec<(UnitId, OpId)>,
    projections: Vec<(UnitId, OpId, usize)>,
    lengths: Vec<(UnitId, OpId)>,
    null_tests: Vec<(UnitId, OpId, bool)>,
    selects: Vec<(UnitId, OpId)>,
    types: Vec<TypeId>,
}
fn scalar_plan(
    program: &Program<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    site: usize,
) -> Option<ScalarPlan> {
    let allocation = &facts.sites[site];
    if !allocation.closed() || allocation.escape != Escape::Local || !allocation.scalar_kind {
        return None;
    }
    let owner = program.unit(allocation.unit)?;
    let mut plan = ScalarPlan {
        site,
        aliases: Vec::new(),
        remove: vec![(allocation.unit, allocation.operation)],
        projections: Vec::new(),
        lengths: Vec::new(),
        null_tests: Vec::new(),
        selects: Vec::new(),
        types: storage_types(program, allocation)?,
    };
    for (index, cell) in program.cells.iter().enumerate() {
        let id = CellId::from_index(index)?;
        if !facts.cell(id).sites.contains(&site) {
            continue;
        }
        let storage = effects.graph().storage(id);
        if facts.cell(id).single() != Some(site)
            || cell.binding != CellBinding::Local
            || storage.stored
            || storage.referenced
            || storage.initializers != 1
        {
            return None;
        }
        plan.aliases.push(id);
    }
    let mut valid = true;
    for unit in &program.units {
        let id = unit.id();
        let data = unit.data();
        for (index, value) in data.values.iter().enumerate() {
            let value_id = ValueId::from_index(index)?;
            let origins = facts.value(id, value_id);
            if !origins.sites.contains(&site) {
                continue;
            }
            if origins.single() != Some(site) {
                return None;
            }
            let definition = &data.operations[value.definition.index()];
            match definition.kind {
                OperationKind::Allocate { .. }
                    if id == allocation.unit && value.definition == allocation.operation => {}
                OperationKind::Load(place) => match data.places[place.index()] {
                    Place::Cell(cell)
                        if plan.aliases.contains(&cell)
                            && effects.initialization().initialized(
                                program,
                                id,
                                value.definition,
                                cell,
                            ) =>
                    {
                        plan.remove.push((id, value.definition))
                    }
                    _ => return None,
                },
                OperationKind::CopyValue => plan.remove.push((id, value.definition)),
                OperationKind::Select { .. } => plan.selects.push((id, value.definition)),
                _ => return None,
            }
        }
        uses::walk(data, |event| {
            let Event::Value(value, usage) = event else { return Ok(()) };
            if !facts.value(id, value).sites.contains(&site) { return Ok(()) }
            match usage {
                ValueUse::Operand { operation, .. } => {
                    let op = &data.operations[operation.index()];
                    match op.kind {
                        OperationKind::Initialize(cell) if plan.aliases.contains(&cell) => plan.remove.push((id, operation)),
                        OperationKind::CopyValue if op.result.is_some_and(|v| facts.value(id, v).single() == Some(site)) => {}
                        OperationKind::Intrinsic(crate::primitive::ResolvedIntrinsic::Property(crate::primitive::Intrinsic::ArrayLength))
                            if allocation.fields.iter().all(|f| matches!(f.key, Key::Index(_))) => plan.lengths.push((id, operation)),
                        OperationKind::Binary(BinaryOp::Eq | BinaryOp::NotEq) => {
                            let args = data.operands(op.operands).unwrap_or(&[]);
                            if args.iter().any(|&v| matches!(aggregates::literal(data, v), Some(Constant::Null | Constant::Undefined))) {
                                plan.null_tests.push((id, operation, matches!(op.kind, OperationKind::Binary(BinaryOp::NotEq))));
                            } else { valid = false; }
                        }
                        _ => valid = false,
                    }
                }
                ValueUse::PlaceReceiver { operation, place } => {
                    match facts.projection(program, id, place) {
                        Some((found, field)) if found == site && matches!(data.operations[operation.index()].kind, OperationKind::Load(_) | OperationKind::Store(_) | OperationKind::CheckPlace(_)) => {
                            if let Some(result) = data.operations[operation.index()].result {
                                if matches!(data.operations[operation.index()].kind, OperationKind::Load(_)) {
                                    let ty = data.values[result.index()].ty;
                                    valid &= !normalizes_null(program, facts, site, ty);
                                }
                            }
                            plan.projections.push((id, operation, field));
                        }
                        _ => valid = false,
                    }
                }
                ValueUse::RegionResult(region) => {
                    valid &= data.regions[region.index()].parent.is_some_and(|parent| data.regions[parent.index()].operations.iter().any(|&op| matches!(data.operations[op.index()].kind, OperationKind::Select { yes, no } if yes == region || no == region)));
                }
                _ => valid = false,
            }
            Ok(())
        }).ok()?;
    }
    if !valid {
        return None;
    }
    for &(unit, op, _) in &plan.projections {
        if !bank_in_scope(
            program,
            allocation.unit,
            owner.operations[allocation.operation.index()].region,
            unit,
            op,
            &mut Vec::new(),
        ) {
            return None;
        }
    }
    // A lexical descendant can capture the field bank. A caller parameter
    // cannot: that requires the existing product-signature family instead.
    for &(unit, _, _) in &plan.projections {
        if unit != allocation.unit
            && !program
                .unit(unit)?
                .captures
                .iter()
                .any(|cell| plan.aliases.contains(cell))
        {
            return None;
        }
    }
    plan.remove.sort_unstable();
    plan.remove.dedup();
    plan.projections.sort_unstable();
    plan.projections.dedup();
    plan.lengths.sort_unstable();
    plan.lengths.dedup();
    plan.null_tests.sort_unstable();
    plan.null_tests.dedup();
    plan.selects.sort_unstable();
    plan.selects.dedup();
    Some(plan)
}

// A cell retains the aggregate's declared storage type, independently of a
// particular narrowed load or the type of its initial value.
fn storage_types(
    program: &Program<'_>,
    allocation: &aggregates::AllocationFacts,
) -> Option<Vec<TypeId>> {
    let data = program.unit(allocation.unit)?;
    let OperationKind::Allocate { ref kind, .. } =
        data.operations[allocation.operation.index()].kind
    else {
        return None;
    };
    match kind {
        AllocationKind::Object(_) => Some(vec![
            data.values[allocation.value.index()].ty;
            allocation.fields.len()
        ]),
        AllocationKind::Instance { class, .. } => allocation
            .fields
            .iter()
            .map(|field| {
                let Key::Named(key) = field.key else {
                    return None;
                };
                program
                    .class(*class)?
                    .fields
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, ty)| *ty)
            })
            .collect(),
        AllocationKind::Array => {
            let Type::Array(element) =
                &program.types[data.values[allocation.value.index()].ty.index()]
            else {
                return None;
            };
            let ty =
                TypeId::from_index(program.types.iter().position(|ty| ty == element.as_ref())?)?;
            Some(vec![ty; allocation.fields.len()])
        }
        _ => None,
    }
}
fn normalizes_null(
    program: &Program<'_>,
    facts: &ProgramAggregates,
    site: usize,
    result: TypeId,
) -> bool {
    let allocation = &facts.sites[site];
    let data = program.unit(allocation.unit).unwrap();
    allocation.record
        || matches!(
            data.operations[allocation.operation.index()].kind,
            OperationKind::Allocate {
                kind: AllocationKind::Array,
                ..
            }
        ) && matches!(
            program.types[result.index()],
            Type::Nullable(_) | Type::Null
        )
}

fn bank_in_scope(
    program: &Program<'_>,
    owner: UnitId,
    scope: RegionId,
    unit: UnitId,
    operation: OpId,
    visiting: &mut Vec<UnitId>,
) -> bool {
    if unit == owner {
        let data = program.unit(unit).unwrap();
        let mut region = Some(data.operations[operation.index()].region);
        while let Some(current) = region {
            if current == scope {
                return true;
            }
            region = data.regions[current.index()].parent;
        }
        return false;
    }
    if visiting.contains(&unit) {
        return false;
    }
    visiting.push(unit);
    let mut found = false;
    for creator in &program.units {
        for (index, op) in creator.data().operations.iter().enumerate() {
            if matches!(op.kind, OperationKind::Closure(child) if child == unit) {
                found = true;
                if !bank_in_scope(
                    program,
                    owner,
                    scope,
                    creator.id(),
                    OpId::from_index(index).unwrap(),
                    visiting,
                ) {
                    visiting.pop();
                    return false;
                }
            }
        }
    }
    visiting.pop();
    found
}

fn scalarize(
    editor: &mut Editor<'_>,
    facts: &ProgramAggregates,
    plan: ScalarPlan,
    dead_code: bool,
    receipt: &mut RuleReceipt,
) -> Result<(), &'static str> {
    let allocation = &facts.sites[plan.site];
    let original = editor.program().unit(allocation.unit).unwrap().operations
        [allocation.operation.index()]
    .clone();
    let mut fields = Vec::new();
    for (index, slot) in allocation.fields.iter().enumerate() {
        // Unread fields retain their initializer/RHS evaluations but own no storage.
        if !slot.read && dead_code {
            fields.push(None);
            continue;
        }
        let cell = editor.add_cell(Cell {
            source_symbol: None,
            name: format!("field{index}"),
            ty: plan.types[index],
            owner: allocation.unit,
            region: original.region,
            declaration: original.span,
            reassigned: slot.written,
            observable_before_initialization: true,
            binding: CellBinding::Local,
            synthetic: true,
            declared_pure: false,
            debug: false,
        })?;
        fields.push(Some(cell));
    }
    let data = editor.unit_mut(allocation.unit);
    let mut initialize = Vec::new();
    for (field, &cell) in allocation.fields.iter().zip(&fields) {
        let Some(cell) = cell else { continue };
        let op = OpId::from_index(data.operations.len()).ok_or("operation capacity")?;
        let start = data.operands.len() as u32;
        data.operands.push(field.initial);
        data.operations.push(Operation {
            authored: original.authored,
            kind: OperationKind::Initialize(cell),
            operands: OperandRange { start, len: 1 },
            result: None,
            region: original.region,
            origin: original.origin,
            span: original.span,
        });
        initialize.push(op);
    }
    let position = data.regions[original.region.index()]
        .operations
        .iter()
        .position(|&op| op == allocation.operation)
        .ok_or("allocation owner")?;
    data.regions[original.region.index()]
        .operations
        .splice(position..position, initialize);
    for &(unit, op, field) in &plan.projections {
        let data = editor.unit_mut(unit);
        if let Some(cell) = fields[field] {
            let place = PlaceId::from_index(data.places.len()).ok_or("place capacity")?;
            data.places.push(Place::Cell(cell));
            data.operations[op.index()].kind = match data.operations[op.index()].kind {
                OperationKind::Load(_) => OperationKind::Load(place),
                OperationKind::Store(_) => OperationKind::Store(place),
                OperationKind::CheckPlace(_) => OperationKind::CheckPlace(place),
                _ => return Err("scalar projection kind"),
            };
        } else {
            // Store results denote the RHS even when the storage disappears.
            if let Some(result) = data.operations[op.index()].result {
                let value = data
                    .operands(data.operations[op.index()].operands)
                    .and_then(|v| v.first())
                    .copied()
                    .ok_or("scalar store value")?;
                edit::substitute(data, result, value);
            }
            edit::detach(data, op);
            receipt.removed_field_stores += 1;
        }
    }
    for &(unit, op) in &plan.lengths {
        edit::make_constant(
            editor.unit_mut(unit),
            op,
            Constant::Integer(fields.len() as i32),
        );
        receipt.folded_fields += 1;
    }
    for &(unit, op, value) in &plan.null_tests {
        edit::make_constant(editor.unit_mut(unit), op, Constant::Boolean(value));
    }
    for &(unit, op) in &plan.selects {
        let data = editor.unit_mut(unit);
        let OperationKind::Select { yes, no } = data.operations[op.index()].kind else {
            return Err("scalar select");
        };
        data.operations[op.index()].kind = OperationKind::If { yes, no: Some(no) };
        data.operations[op.index()].result = None;
        data.regions[yes.index()].result = None;
        data.regions[no.index()].result = None;
    }
    for &(unit, op) in &plan.remove {
        edit::detach(editor.unit_mut(unit), op);
    }
    for index in 0..editor.program().units.len() {
        let unit = UnitId::from_index(index).unwrap();
        if !editor
            .program()
            .unit(unit)
            .unwrap()
            .captures
            .iter()
            .any(|cell| plan.aliases.contains(cell))
        {
            continue;
        }
        let data = editor.unit_mut(unit);
        data.captures.retain(|cell| !plan.aliases.contains(cell));
        if unit != allocation.unit {
            data.captures.extend(fields.iter().flatten().copied());
            data.captures.sort_unstable();
            data.captures.dedup();
        }
    }
    receipt.scalarized_allocations += 1;
    receipt.scalar_fields += fields.iter().flatten().count() as u32;
    Ok(())
}

// Promote compiler-owned object constructors to the shared allocation owner.
// Key/argument evaluations stay at their original operations, including effects.
fn normalize_objects(
    editor: &mut Editor<'_>,
    receipt: &mut RuleReceipt,
) -> Result<bool, &'static str> {
    let mut changes = Vec::new();
    for unit in &editor.program().units {
        let data = unit.data();
        let mut next = data
            .operations
            .iter()
            .filter_map(|op| match op.kind {
                OperationKind::Allocate { identity, .. } => Some(identity.index() + 1),
                _ => None,
            })
            .max()
            .unwrap_or(0);
        for (index, op) in data.operations.iter().enumerate() {
            let OperationKind::Call(call) = op.kind else {
                continue;
            };
            let site = &data.calls[call.index()];
            if !matches!(site.target, CallTarget::Builtin(BuiltinCall::JsObject)) {
                continue;
            }
            let args = data.arguments(site.arguments).unwrap();
            if args.len() % 2 != 0 {
                continue;
            }
            let mut keys = Vec::new();
            let mut values = Vec::new();
            let mut valid = true;
            for pair in args.chunks_exact(2) {
                let [CallArgument::Value(key), CallArgument::Value(value)] = pair else {
                    valid = false;
                    break;
                };
                let Some(Constant::String(key)) = aggregates::literal(data, *key) else {
                    valid = false;
                    break;
                };
                if editor.program().strings[key.index()].as_unicode() == Some("__proto__") {
                    valid = false;
                    break;
                }
                keys.push(*key);
                values.push(*value);
            }
            if !valid {
                continue;
            }
            let identity = AllocationId::from_index(next).ok_or("allocation capacity")?;
            next += 1;
            changes.push((
                unit.id(),
                OpId::from_index(index).unwrap(),
                call,
                identity,
                keys,
                values,
            ));
        }
    }
    let changed = !changes.is_empty();
    for (unit, op, call, identity, keys, values) in changes {
        let data = editor.unit_mut(unit);
        let prepares: Vec<_> = data
            .operations
            .iter()
            .enumerate()
            .filter_map(|(index, op)| {
                matches!(op.kind, OperationKind::PrepareCall(found) if found == call)
                    .then(|| OpId::from_index(index).unwrap())
            })
            .collect();
        for op in prepares {
            edit::detach(data, op);
        }
        let start = data.operands.len() as u32;
        data.operands.extend(&values);
        data.operations[op.index()].operands = OperandRange {
            start,
            len: values.len() as u32,
        };
        data.operations[op.index()].kind = OperationKind::Allocate {
            identity,
            kind: AllocationKind::Object(keys),
        };
        receipt.exposed_allocations += 1;
    }
    Ok(changed)
}

fn namespaces(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    receipt: &mut RuleReceipt,
) -> Result<bool, &'static str> {
    let mut changes = Vec::new();
    for unit in &editor.program().units {
        for (index, op) in unit.data().operations.iter().enumerate() {
            let OperationKind::Call(call) = op.kind else {
                continue;
            };
            let Some((site, field)) = facts.namespace(editor.program(), effects, unit.id(), call)
            else {
                continue;
            };
            if !facts.sites[site].closed() || facts.sites[site].fields[field].written {
                continue;
            }
            changes.push((unit.id(), OpId::from_index(index).unwrap(), call));
        }
    }
    if changes.is_empty() {
        return Ok(false);
    }
    let dynamic = editor.intern_type(Type::Dynamic)?;
    for (unit, invoke, call) in changes {
        let data = editor.unit_mut(unit);
        let args = data
            .arguments(data.calls[call.index()].arguments)
            .unwrap()
            .to_vec();
        let (CallArgument::Value(receiver), CallArgument::Value(key)) = (args[0], args[1]) else {
            return Err("namespace arguments");
        };
        let prepare = data
            .operations
            .iter()
            .enumerate()
            .find_map(|(index, op)| {
                matches!(op.kind, OperationKind::PrepareCall(found) if found == call)
                    .then(|| OpId::from_index(index).unwrap())
            })
            .ok_or("namespace preparation")?;
        let original = data.operations[invoke.index()].clone();
        edit::detach(data, prepare);
        let place = PlaceId::from_index(data.places.len()).ok_or("place capacity")?;
        data.places.push(Place::Index { receiver, key });
        let load = OpId::from_index(data.operations.len()).ok_or("operation capacity")?;
        let callee = ValueId::from_index(data.values.len()).ok_or("value capacity")?;
        data.values.push(Value {
            ty: dynamic,
            definition: load,
        });
        data.operations.push(Operation {
            kind: OperationKind::Load(place),
            operands: OperandRange { start: 0, len: 0 },
            result: Some(callee),
            ..original.clone()
        });
        let region = &mut data.regions[original.region.index()];
        let at = region
            .operations
            .iter()
            .position(|&op| op == invoke)
            .ok_or("namespace invocation")?;
        region.operations.splice(at..at, [load, prepare]);
        let arguments = ArgumentRange {
            start: data.call_arguments.len() as u32,
            len: (args.len() - 2) as u32,
        };
        data.call_arguments.extend_from_slice(&args[2..]);
        let site = &mut data.calls[call.index()];
        site.target = CallTarget::Value {
            callee,
            invocation: Invocation::Value,
        };
        site.arguments = arguments;
        site.contract.signature = Some(dynamic);
        site.contract.supplied = (args.len() - 2) as u32;
        site.contract.defaults = crate::primitive::DefaultConvention::MaterializeAtCaller;
        receipt.flattened_namespace_calls += 1;
    }
    Ok(true)
}

// Feed the existing codec-judged record family one canonical reference cell.
// The family still owns absence normalization and payload representation.
fn record_aliases(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    receipt: &mut RuleReceipt,
) -> bool {
    let program = editor.program();
    let mut replacements = Vec::new();
    let mut work = 0usize;
    for (site, allocation) in facts.sites.iter().enumerate() {
        if !allocation.record {
            continue;
        }
        let data = program.unit(allocation.unit).unwrap();
        let canonical = data.operations.iter().find_map(|op| match op.kind {
            OperationKind::Initialize(cell)
                if data.operands(op.operands) == Some(&[allocation.value][..]) =>
            {
                Some(cell)
            }
            _ => None,
        });
        let Some(canonical) = canonical else { continue };
        let storage = effects.graph().storage(canonical);
        if storage.stored || storage.referenced || storage.initializers != 1 {
            continue;
        }
        for (index, cell) in program.cells.iter().enumerate() {
            work += 1;
            if work > (1 << 22) {
                receipt.aggregate_limits += 1;
                break;
            }
            let alias = CellId::from_index(index).unwrap();
            if alias == canonical
                || cell.ty != program.cells[canonical.index()].ty
                || facts.cell(alias).single() != Some(site)
            {
                continue;
            }
            let storage = effects.graph().storage(alias);
            if cell.binding != CellBinding::Local
                || storage.stored
                || storage.referenced
                || storage.initializers != 1
            {
                continue;
            }
            let mut loads = Vec::new();
            let mut initialize = None;
            let mut valid = true;
            for unit in &program.units {
                let data = unit.data();
                for (index, op) in data.operations.iter().enumerate() {
                    work += 1;
                    if work > (1 << 22) {
                        valid = false;
                        break;
                    }
                    let at = OpId::from_index(index).unwrap();
                    if matches!(op.kind, OperationKind::Initialize(c) if c == alias) {
                        initialize = Some((unit.id(), at));
                    }
                    if let OperationKind::Load(place) = op.kind {
                        if data.places[place.index()] == Place::Cell(alias) {
                            valid &=
                                effects
                                    .initialization()
                                    .initialized(program, unit.id(), at, alias)
                                    && bank_in_scope(
                                        program,
                                        allocation.unit,
                                        program.cells[canonical.index()].region,
                                        unit.id(),
                                        at,
                                        &mut Vec::new(),
                                    );
                            loads.push((unit.id(), at));
                        }
                    }
                }
            }
            if valid && !loads.is_empty() {
                replacements.push((alias, canonical, initialize, loads));
            }
        }
    }
    receipt.aggregate_analysis_work = receipt.aggregate_analysis_work.saturating_add(work as u64);
    let changed = !replacements.is_empty();
    for (alias, canonical, initialize, loads) in replacements {
        for (unit, at) in loads {
            let data = editor.unit_mut(unit);
            let place = PlaceId::from_index(data.places.len()).unwrap();
            data.places.push(Place::Cell(canonical));
            data.operations[at.index()].kind = OperationKind::Load(place);
        }
        if let Some((unit, at)) = initialize {
            edit::detach(editor.unit_mut(unit), at);
        }
        for index in 0..editor.program().units.len() {
            let unit = UnitId::from_index(index).unwrap();
            if editor
                .program()
                .unit(unit)
                .unwrap()
                .captures
                .contains(&alias)
            {
                let data = editor.unit_mut(unit);
                data.captures.retain(|&cell| cell != alias);
                data.captures.push(canonical);
                data.captures.sort_unstable();
                data.captures.dedup();
            }
        }
        receipt.elided_record_aliases += 1;
    }
    changed
}

// A contiguous construction prefix contains only literals, the fresh handle's
// aliases and own-field stores. Delay that fresh allocation to the last store,
// retaining every original operand evaluation and handle definition in order.
fn collect_stores(
    editor: &mut Editor<'_>,
    pristine: bool,
    receipt: &mut RuleReceipt,
) -> Result<bool, &'static str> {
    let mut plans = Vec::new();
    for unit in &editor.program().units {
        let data = unit.data();
        for region in &data.regions {
            for (position, &allocated) in region.operations.iter().enumerate() {
                let original = &data.operations[allocated.index()];
                let OperationKind::Allocate { kind, .. } = &original.kind else {
                    continue;
                };
                let (mut keys, record, array) = match kind {
                    AllocationKind::Object(keys) => (
                        keys.iter().copied().map(Key::Named).collect::<Vec<_>>(),
                        false,
                        false,
                    ),
                    AllocationKind::Record(keys) => (
                        keys.iter().copied().map(Key::Named).collect::<Vec<_>>(),
                        true,
                        false,
                    ),
                    AllocationKind::Array => (
                        (0..data.operands(original.operands).unwrap().len())
                            .map(|n| Key::Index(n as u32))
                            .collect(),
                        false,
                        true,
                    ),
                    _ => continue,
                };
                if keys.len() > 256 {
                    continue;
                }
                if keys.iter().any(|key| matches!(key, Key::Named(k) if editor.program().strings[k.index()].as_unicode() == Some("__proto__"))) { continue; }
                if keys.iter().enumerate().any(|(n, k)| keys[..n].contains(k)) {
                    continue;
                }
                let mut values = data.operands(original.operands).unwrap().to_vec();
                let mut handles = vec![original.result.unwrap()];
                let mut cells = Vec::new();
                let mut transport = vec![allocated];
                let mut removed = Vec::new();
                let mut last = None;
                for &at in region.operations[position + 1..].iter().take(256) {
                    let op = &data.operations[at.index()];
                    let args = data.operands(op.operands).unwrap();
                    match op.kind {
                        OperationKind::Constant(_) => {}
                        OperationKind::Initialize(cell)
                            if args.first().is_some_and(|v| handles.contains(v)) =>
                        {
                            cells.push(cell);
                            transport.push(at);
                        }
                        OperationKind::CopyValue
                            if args.first().is_some_and(|v| handles.contains(v)) =>
                        {
                            handles.extend(op.result);
                            transport.push(at);
                        }
                        OperationKind::Load(place) if matches!(data.places[place.index()], Place::Cell(c) if cells.contains(&c)) =>
                        {
                            handles.extend(op.result);
                            transport.push(at);
                        }
                        OperationKind::Store(place) | OperationKind::CheckPlace(place) => {
                            let Some((receiver, key)) =
                                aggregates::projection(editor.program(), data, place)
                            else {
                                break;
                            };
                            if !handles.contains(&receiver) {
                                break;
                            }
                            if matches!(key, Key::Named(k) if editor.program().strings[k.index()].as_unicode() == Some("__proto__"))
                            {
                                break;
                            }
                            let field = keys.iter().position(|&known| known == key);
                            // Missing own properties can invoke prototype setters.
                            // Records have a null prototype; ordinary objects need
                            // the explicitly requested pristine-host contract.
                            if field.is_none() && !(record || pristine && !array) {
                                break;
                            }
                            if matches!(op.kind, OperationKind::Store(_)) {
                                if args.iter().any(|v| handles.contains(v)) {
                                    break;
                                }
                                if let Some(field) = field {
                                    values[field] = args[0];
                                } else {
                                    keys.push(key);
                                    values.push(args[0]);
                                }
                                last = Some(at);
                            }
                            removed.push(at);
                        }
                        _ => break,
                    }
                }
                let Some(last) = last else { continue };
                // Only move/remove through the final collected store: a later
                // alias/check remains in place and retains its original order.
                let end = region.operations.iter().position(|&op| op == last).unwrap();
                transport.retain(|op| region.operations[..=end].contains(op));
                removed.retain(|op| region.operations[..=end].contains(op));
                plans.push((
                    unit.id(),
                    original.region,
                    allocated,
                    last,
                    keys,
                    values,
                    transport,
                    removed,
                ));
            }
        }
    }
    let changed = !plans.is_empty();
    for (unit, region, allocated, last, keys, values, transport, removed) in plans {
        let data = editor.unit_mut(unit);
        let start = data.operands.len() as u32;
        data.operands.extend(&values);
        data.operations[allocated.index()].operands = OperandRange {
            start,
            len: values.len() as u32,
        };
        match &mut data.operations[allocated.index()].kind {
            OperationKind::Allocate {
                kind: AllocationKind::Object(names) | AllocationKind::Record(names),
                ..
            } => {
                *names = keys
                    .iter()
                    .map(|key| match key {
                        Key::Named(name) => *name,
                        _ => unreachable!(),
                    })
                    .collect();
            }
            _ => {}
        }
        let order = &mut data.regions[region.index()].operations;
        let at = order
            .iter()
            .position(|&op| op == last)
            .ok_or("collected store owner")?;
        let mut replacement = Vec::with_capacity(order.len());
        for (index, &op) in order.iter().enumerate() {
            if index == at {
                replacement.extend_from_slice(&transport);
            }
            if !transport.contains(&op) && !removed.contains(&op) {
                replacement.push(op);
            }
        }
        *order = replacement;
        receipt.collected_field_stores += removed
            .iter()
            .filter(|op| matches!(data.operations[op.index()].kind, OperationKind::Store(_)))
            .count() as u32;
    }
    Ok(changed)
}
