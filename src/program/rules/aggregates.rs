//! Consumers of shared allocation/field evidence. Source evaluations stay in
//! place; replacing storage never moves an initializer or an argument effect.
use super::super::aggregates::{self, Escape, Key, ProgramAggregates};
use super::super::uses::{self, Event, ValueUse};
use super::edit::{self, Editor};
use super::storage::Map;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::AllocationClass::{Retained, Scratch};
use crate::output_budget::{AllocationBudget, AllocationError};

pub(super) fn apply(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    request: RuleRequest,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    if (request.fold || request.scalar) && normalize_objects(editor, receipt, budget)? {
        return Ok(true);
    }
    if request.dead_code && collect_stores(editor, request.pristine_builtins, receipt, budget)? {
        return Ok(true);
    }
    let facts = editor.program().aggregates_in(request.seal, budget)?;
    if !facts.complete {
        receipt.aggregate_limits += 1;
        return Ok(false);
    }
    receipt.aggregate_analysis_work = receipt
        .aggregate_analysis_work
        .saturating_add(facts.work as u64);
    // Source folds/removals precede a representation edit. Fresh facts on the
    // next round see its complete new use set, never a half-updated allocation.
    if request.fold && namespaces(editor, effects, &facts, receipt, budget)? {
        return Ok(true);
    }
    drop(
        editor
            .program()
            .primitive_classes_in(request.seal, budget)?,
    );
    if fields(editor, effects, &facts, request, receipt, budget)? {
        return Ok(true);
    }
    if !request.scalar {
        return Ok(false);
    }
    if record_aliases(editor, effects, &facts, receipt, budget)? {
        return Ok(true);
    }
    // Analysis cannot grow quadratically without a ceiling. A refused site
    // retains its storage; subsequent rounds may expose a smaller subject.
    let scan = editor
        .program()
        .units
        .iter()
        .fold(0usize, |sum, u| {
            sum.saturating_add(u.data().values.len())
                .saturating_add(u.data().operations.len())
        })
        .saturating_add(editor.program().cells.len())
        .max(1);
    let limit = (1usize << 22) / scan;
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            let program = editor.program();
            budget.retained_phase(|budget| {
                let mut plans = Vec::new();
                if limit == 0 {
                    return Ok(plans);
                }
                let links = ScopeLinks::build(program, budget)?;
                for site in (0..facts.sites.len())
                    .filter(|&site| {
                        let allocation = &facts.sites[site];
                        allocation.closed()
                            && allocation.escape == Escape::Local
                            && allocation.scalar_kind
                    })
                    .take(limit)
                {
                    let allocation = &facts.sites[site];
                    budget.work(WorkKind::Analysis, allocation.fields.len() as u64 + 1)?;
                    if request.native
                        && allocation.captured
                        && program.unit(allocation.unit).unwrap().kind
                            != UnitKind::ModuleInitialization
                        && allocation
                            .fields
                            .iter()
                            .filter(|field| field.read || !request.dead_code)
                            .count()
                            > 2
                    {
                        continue;
                    }
                    if let Some(plan) = scalar_plan(program, effects, &facts, &links, site, budget)?
                    {
                        budget.push(Retained, &mut plans, plan)?;
                    }
                }
                Ok::<_, super::RuleError>(plans)
            })
        },
        |plans, editor, budget| {
            for plan in plans {
                scalarize(editor, &facts, plan, request.dead_code, receipt, budget)?;
            }
            Ok(!plans.is_empty())
        },
    )
}

fn fields(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    request: RuleRequest,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            Ok::<_, super::RuleError>(field_plan(
                editor.program(),
                effects,
                facts,
                request,
                budget,
            )?)
        },
        |plan, editor, budget| apply_fields(editor, plan, receipt, budget),
    )
}
#[derive(Default)]
struct FieldPlan {
    constants: Vec<(UnitId, OpId, Constant)>,
    forwards: Vec<(UnitId, OpId, ValueId, ValueId)>,
    removed: Vec<(UnitId, OpId)>,
}
fn field_plan(
    program: &Program<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    request: RuleRequest,
    budget: &mut AllocationBudget<'_>,
) -> Result<FieldPlan, AllocationError> {
    budget.retained_phase(|budget| {
        let mut plan = FieldPlan::default();
        for unit in &program.units {
            let id = unit.id();
            let data = unit.data();
            budget.with_temporary(
                |budget| {
                    let parent = budget.filled(Retained, data.regions.len(), None)?;
                    let position = budget.filled(Retained, data.operations.len(), 0)?;
                    super::super::activation::StructuredDominance::build(
                        data,
                        parent,
                        position,
                        |work| budget.work(WorkKind::Analysis, work as u64),
                    )
                },
                |dominance, budget| {
                    let behavior = behaviors_in(program, effects, id, None, budget)?;
                    for region in &data.regions {
                        let mut stores: Map<(usize, usize, ValueId), OpId> = Map::new(Scratch);
                        for &op in &region.operations {
                            budget.work(WorkKind::Analysis, 1)?;
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
                                        stores.retain(budget, |&(s, f, _), _| {
                                            (s, f) != (site, field)
                                        })?;
                                        if request.fold {
                                            if let Some(constant) = &slot.constant {
                                                // Longer literals retain their existing storage/representation
                                                // opportunity; this is the ordinary short-field default.
                                                let short = match constant {
                                                    Constant::Boolean(_) | Constant::Null => true,
                                                    Constant::Integer(v) => (-99..=999).contains(v),
                                                    _ => false,
                                                };
                                                if short {
                                                    budget.push(
                                                        Retained,
                                                        &mut plan.constants,
                                                        (id, op, constant.clone()),
                                                    )?;
                                                }
                                            } else if !slot.written
                                                && allocation.unit == id
                                                && dominance.after(
                                                    data,
                                                    allocation.operation,
                                                    op,
                                                    |work| {
                                                        budget.work(WorkKind::Analysis, work as u64)
                                                    },
                                                )?
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
                                                    budget.push(
                                                        Retained,
                                                        &mut plan.forwards,
                                                        (id, op, result, slot.initial),
                                                    )?;
                                                }
                                            }
                                        }
                                    }
                                    OperationKind::Store(_) if request.dead_code => {
                                        // The RHS is an earlier operation and remains evaluated.
                                        let receiver =
                                            aggregates::projection(program, data, place.unwrap())
                                                .unwrap()
                                                .0;
                                        if let Some(previous) =
                                            stores.insert((site, field, receiver), op, budget)?
                                        {
                                            budget.push(
                                                Retained,
                                                &mut plan.removed,
                                                (id, previous),
                                            )?;
                                        }
                                    }
                                    _ => {}
                                }
                                continue;
                            }
                            if operation.kind.child_regions().next().is_some()
                                || behavior[op.index()].requires_evaluation()
                            {
                                stores.clear(budget)?;
                            }
                        }
                        stores.release(budget)?;
                    }
                    storage::release_vec(behavior, Retained, budget)?;
                    Ok::<_, AllocationError>(())
                },
            )?;
        }
        budget.work(
            WorkKind::Analysis,
            (plan.removed.len() as u64).saturating_mul(
                u64::from(usize::BITS - plan.removed.len().max(1).leading_zeros()) + 1,
            ),
        )?;
        plan.removed.sort_unstable();
        plan.removed.dedup();
        Ok(plan)
    })
}
fn apply_fields(
    editor: &mut Editor<'_>,
    plan: &FieldPlan,
    receipt: &mut RuleReceipt,
    budget: &mut AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    let changed =
        !plan.constants.is_empty() || !plan.forwards.is_empty() || !plan.removed.is_empty();
    for (unit, op, value) in &plan.constants {
        edit::make_constant_in(
            editor.unit_mut_in(*unit, budget)?,
            *op,
            value.clone(),
            budget,
        )?;
        receipt.folded_fields += 1;
    }
    let mut substitutions = Map::new(Scratch);
    for &(unit, op, result, mut value) in &plan.forwards {
        while let Some(&next) = substitutions.get(&(unit, value)) {
            budget.work(WorkKind::Analysis, 1)?;
            value = next;
        }
        let data = editor.unit_mut_in(unit, budget)?;
        edit::detach(data, op);
        edit::substitute(data, result, value);
        substitutions.insert((unit, result), value, budget)?;
        receipt.folded_fields += 1;
    }
    substitutions.release(budget)?;
    for &(unit, op) in &plan.removed {
        let data = editor.unit_mut_in(unit, budget)?;
        if let Some(result) = data.operations[op.index()].result {
            let value = data.operands(data.operations[op.index()].operands).unwrap()[0];
            edit::substitute(data, result, value);
        }
        edit::detach(data, op);
        receipt.removed_field_stores += 1;
    }
    Ok(changed)
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
    links: &ScopeLinks,
    site: usize,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<ScalarPlan>, AllocationError> {
    storage::optional(budget, |attempt| {
        let allocation = &facts.sites[site];
        if !allocation.closed() || allocation.escape != Escape::Local || !allocation.scalar_kind {
            return None;
        }
        let owner = program.unit(allocation.unit)?;
        let mut plan = ScalarPlan {
            site,
            aliases: Vec::new(),
            remove: attempt.collect(
                Retained,
                std::iter::once((allocation.unit, allocation.operation)),
            )?,
            projections: Vec::new(),
            lengths: Vec::new(),
            null_tests: Vec::new(),
            selects: Vec::new(),
            types: storage_types(program, allocation, attempt)?,
        };
        for (index, cell) in program.cells.iter().enumerate() {
            attempt.work(1)?;
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
            attempt.push(Retained, &mut plan.aliases, id)?;
        }
        let mut valid = true;
        for unit in &program.units {
            let id = unit.id();
            let data = unit.data();
            for (index, value) in data.values.iter().enumerate() {
                attempt.work(plan.aliases.len() as u64 + 1)?;
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
                            attempt.push(Retained, &mut plan.remove, (id, value.definition))?
                        }
                        _ => return None,
                    },
                    OperationKind::CopyValue => {
                        attempt.push(Retained, &mut plan.remove, (id, value.definition))?
                    }
                    OperationKind::Select { .. } => {
                        attempt.push(Retained, &mut plan.selects, (id, value.definition))?
                    }
                    _ => return None,
                }
            }
            uses::walk(data, |event| { (|| {
            attempt.work(1)?;
            let Event::Value(value, usage) = event else { return Some(()) };
            if !facts.value(id, value).sites.contains(&site) { return Some(()) }
            match usage {
                ValueUse::Operand { operation, .. } => {
                    let op = &data.operations[operation.index()];
                    match op.kind {
                        OperationKind::Initialize(cell) if plan.aliases.contains(&cell) => attempt.push(Retained, &mut plan.remove, (id, operation))?,
                        OperationKind::CopyValue if op.result.is_some_and(|v| facts.value(id, v).single() == Some(site)) => {}
                        OperationKind::Intrinsic(crate::primitive::ResolvedIntrinsic::Property(crate::primitive::Intrinsic::ArrayLength))
                            if allocation.fields.iter().all(|f| matches!(f.key, Key::Index(_))) => attempt.push(Retained, &mut plan.lengths, (id, operation))?,
                        OperationKind::Binary(BinaryOp::Eq | BinaryOp::NotEq) => {
                            let args = data.operands(op.operands).unwrap_or(&[]);
                            if args.iter().any(|&v| matches!(aggregates::literal(data, v), Some(Constant::Null | Constant::Undefined))) {
                                attempt.push(Retained, &mut plan.null_tests, (id, operation, matches!(op.kind, OperationKind::Binary(BinaryOp::NotEq))))?;
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
                            attempt.push(Retained, &mut plan.projections, (id, operation, field))?;
                        }
                        _ => valid = false,
                    }
                }
                ValueUse::RegionResult(region) => {
                    valid &= data.regions[region.index()].parent.is_some_and(|parent| data.regions[parent.index()].operations.iter().any(|&op| matches!(data.operations[op.index()].kind, OperationKind::Select { yes, no } if yes == region || no == region)));
                }
                _ => valid = false,
            }
            Some(())
        })().ok_or(uses::UseError::Capacity) }).ok()?;
        }
        if !valid {
            return None;
        }
        for &(unit, op, _) in &plan.projections {
            if !attempt.admit(|budget| {
                links.contains(
                    program,
                    allocation.unit,
                    owner.operations[allocation.operation.index()].region,
                    unit,
                    op,
                    budget,
                )
            })? {
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
        for count in [
            plan.remove.len(),
            plan.projections.len(),
            plan.lengths.len(),
            plan.null_tests.len(),
            plan.selects.len(),
        ] {
            attempt.work(
                (count as u64)
                    .saturating_mul(u64::from(usize::BITS - count.max(1).leading_zeros()) + 1),
            )?;
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
    })
}

// A cell retains the aggregate's declared storage type, independently of a
// particular narrowed load or the type of its initial value.
fn storage_types(
    program: &Program<'_>,
    allocation: &aggregates::AllocationFacts,
    attempt: &mut super::super::analysis_storage::Attempt<'_, '_>,
) -> Option<Vec<TypeId>> {
    let data = program.unit(allocation.unit)?;
    let OperationKind::Allocate { ref kind, .. } =
        data.operations[allocation.operation.index()].kind
    else {
        return None;
    };
    match kind {
        AllocationKind::Object(_) => attempt.filled(
            Retained,
            allocation.fields.len(),
            data.values[allocation.value.index()].ty,
        ),
        AllocationKind::Instance { class, .. } => {
            let class = program.class(*class)?;
            let mut types = attempt.vector(Retained, allocation.fields.len())?;
            for field in &allocation.fields {
                attempt.work(class.fields.len() as u64 + 1)?;
                let Key::Named(key) = field.key else {
                    return None;
                };
                let ty = class
                    .fields
                    .iter()
                    .find(|(name, _)| *name == key)
                    .map(|(_, ty)| *ty)?;
                types.push(ty);
            }
            Some(types)
        }
        AllocationKind::Array => {
            let Type::Array(element) =
                &program.types[data.values[allocation.value.index()].ty.index()]
            else {
                return None;
            };
            attempt.work(program.types.len() as u64)?;
            let ty =
                TypeId::from_index(program.types.iter().position(|ty| ty == element.as_ref())?)?;
            attempt.filled(Retained, allocation.fields.len(), ty)
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

/// Creator edges are stable during a planning phase. Scope queries walk these
/// edges with an explicit stack; an outside creator, missing path or cycle
/// declines the proof. Shared creators retain the original all-paths meaning.
struct ScopeLinks {
    creators: Vec<Vec<(UnitId, OpId)>>,
}
impl ScopeLinks {
    fn build(
        program: &Program<'_>,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        let mut creators = storage::collect(
            (0..program.units.len()).map(|_| Vec::new()),
            Scratch,
            budget,
        )?;
        for unit in &program.units {
            for (index, op) in unit.data().operations.iter().enumerate() {
                budget.work(WorkKind::Analysis, 1)?;
                if let OperationKind::Closure(child) = op.kind {
                    budget.push(
                        Scratch,
                        &mut creators[child.index()],
                        (unit.id(), OpId::from_index(index).unwrap()),
                    )?;
                }
            }
        }
        Ok(Self { creators })
    }
    fn contains(
        &self,
        program: &Program<'_>,
        owner: UnitId,
        scope: RegionId,
        unit: UnitId,
        operation: OpId,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<bool, AllocationError> {
        fn inside(
            program: &Program<'_>,
            owner: UnitId,
            scope: RegionId,
            op: OpId,
            budget: &mut AllocationBudget<'_>,
        ) -> Result<bool, AllocationError> {
            let data = program.unit(owner).expect("scope owner");
            let mut region = Some(data.operations[op.index()].region);
            while let Some(current) = region {
                budget.work(WorkKind::Analysis, 1)?;
                if current == scope {
                    return Ok(true);
                }
                region = data.regions[current.index()].parent;
            }
            Ok(false)
        }
        if unit == owner {
            return inside(program, owner, scope, operation, budget);
        }
        let mut budget = budget.scope();
        let mut frames = budget.vector(Scratch, 1)?;
        frames.push((unit, 0usize));
        while let Some(&(unit, cursor)) = frames.last() {
            budget.work(WorkKind::Analysis, 1)?;
            let creators = &self.creators[unit.index()];
            if creators.is_empty() {
                return Ok(false);
            }
            if cursor == creators.len() {
                frames.pop();
                continue;
            }
            let (parent, op) = creators[cursor];
            frames.last_mut().unwrap().1 += 1;
            if parent == owner {
                if !inside(program, owner, scope, op, &mut budget)? {
                    return Ok(false);
                }
            } else {
                budget.work(WorkKind::Analysis, frames.len() as u64)?;
                if frames.iter().any(|&(unit, _)| unit == parent) {
                    return Ok(false);
                }
                budget.push(Scratch, &mut frames, (parent, 0))?;
            }
        }
        Ok(true)
    }
}

fn scalarize(
    editor: &mut Editor<'_>,
    facts: &ProgramAggregates,
    plan: &ScalarPlan,
    dead_code: bool,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<(), super::RuleError> {
    let allocation = &facts.sites[plan.site];
    let original =
        &editor.program().unit(allocation.unit).unwrap().operations[allocation.operation.index()];
    let (authored, region, origin, span) = (
        original.authored,
        original.region,
        original.origin,
        original.span,
    );
    let mut fields = budget.vector(Scratch, allocation.fields.len())?;
    for (index, slot) in allocation.fields.iter().enumerate() {
        // Unread fields retain their initializer/RHS evaluations but own no storage.
        if !slot.read && dead_code {
            fields.push(None);
            continue;
        }
        let cell = editor.add_cell_in(
            Cell {
                    declared_define: false,
                    declared_const: false,
                source_symbol: None,
                name: budget.format(Retained, format_args!("field{index}"))?,
                ty: plan.types[index],
                owner: allocation.unit,
                region: region,
                declaration: span,
                reassigned: slot.written,
                observable_before_initialization: true,
                binding: CellBinding::Local,
                synthetic: true,
                declared_pure: false,
                debug: false,
            },
            budget,
        )?;
        fields.push(Some(cell));
    }
    let data = editor.unit_mut_in(allocation.unit, budget)?;
    let mut initialize = Vec::new();
    for (field, &cell) in allocation.fields.iter().zip(&fields) {
        let Some(cell) = cell else { continue };
        let op = OpId::from_index(data.operations.len()).ok_or("operation capacity")?;
        let start = data.operands.len() as u32;
        budget.push(Retained, &mut data.operands, field.initial)?;
        budget.push(
            Retained,
            &mut data.operations,
            Operation {
                authored,
                kind: OperationKind::Initialize(cell),
                operands: OperandRange { start, len: 1 },
                result: None,
                region: region,
                origin: origin,
                span,
            },
        )?;
        budget.push(Scratch, &mut initialize, op)?;
    }
    let position = data.regions[region.index()]
        .operations
        .iter()
        .position(|&op| op == allocation.operation)
        .ok_or("allocation owner")?;
    let list = &mut data.regions[region.index()].operations;
    budget.reserve_vec(Retained, list, initialize.len())?;
    list.splice(position..position, initialize.iter().copied());
    storage::release_vec(initialize, Scratch, budget)?;
    for &(unit, op, field) in &plan.projections {
        let data = editor.unit_mut_in(unit, budget)?;
        if let Some(cell) = fields[field] {
            let place = PlaceId::from_index(data.places.len()).ok_or("place capacity")?;
            budget.push(Retained, &mut data.places, Place::Cell(cell))?;
            data.operations[op.index()].kind = match data.operations[op.index()].kind {
                OperationKind::Load(_) => OperationKind::Load(place),
                OperationKind::Store(_) => OperationKind::Store(place),
                OperationKind::CheckPlace(_) => OperationKind::CheckPlace(place),
                _ => return Err("scalar projection kind".into()),
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
        edit::make_constant_in(
            editor.unit_mut_in(unit, budget)?,
            op,
            Constant::Integer(fields.len() as i32),
            budget,
        )?;
        receipt.folded_fields += 1;
    }
    for &(unit, op, value) in &plan.null_tests {
        edit::make_constant_in(
            editor.unit_mut_in(unit, budget)?,
            op,
            Constant::Boolean(value),
            budget,
        )?;
    }
    for &(unit, op) in &plan.selects {
        let data = editor.unit_mut_in(unit, budget)?;
        let OperationKind::Select { yes, no } = data.operations[op.index()].kind else {
            return Err("scalar select".into());
        };
        data.operations[op.index()].kind = OperationKind::If { yes, no: Some(no) };
        data.operations[op.index()].result = None;
        data.regions[yes.index()].result = None;
        data.regions[no.index()].result = None;
    }
    for &(unit, op) in &plan.remove {
        edit::detach(editor.unit_mut_in(unit, budget)?, op);
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
        let data = editor.unit_mut_in(unit, budget)?;
        data.captures.retain(|cell| !plan.aliases.contains(cell));
        if unit != allocation.unit {
            storage::extend(
                &mut data.captures,
                fields.iter().flatten().copied(),
                Retained,
                budget,
            )?;
            data.captures.sort_unstable();
            data.captures.dedup();
        }
    }
    receipt.scalarized_allocations += 1;
    receipt.scalar_fields += fields.iter().flatten().count() as u32;
    storage::release_vec(fields, Scratch, budget)?;
    Ok(())
}

// Promote compiler-owned object constructors to the shared allocation owner.
// Key/argument evaluations stay at their original operations, including effects.
fn normalize_objects(
    editor: &mut Editor<'_>,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(editor, |editor, budget| {
    let mut changes = Vec::new();
    for unit in &editor.program().units {
        budget.work(WorkKind::Analysis, unit.data().operations.len() as u64)?;
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
            budget.work(WorkKind::Analysis, 1)?;
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
                budget.work(WorkKind::Analysis, 1)?;
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
                budget.push(Retained, &mut keys, *key)?;
                budget.push(Retained, &mut values, *value)?;
            }
            if !valid {
                storage::release_vec(keys, Retained, budget)?;
                storage::release_vec(values, Retained, budget)?;
                continue;
            }
            let identity = AllocationId::from_index(next).ok_or("allocation capacity")?;
            next += 1;
            budget.push(Retained, &mut changes, (
                unit.id(),
                OpId::from_index(index).unwrap(),
                call,
                identity,
                keys,
                values,
            ))?;
        }
    }
    Ok::<_, super::RuleError>(changes)
    }, |changes, editor, budget| {
    let changed = !changes.is_empty();
    for &(unit, op, call, identity, ref keys, ref values) in changes {
        let data = editor.unit_mut_in(unit, budget)?;
        for index in 0..data.operations.len() {
            budget.work(WorkKind::Edit, 1)?;
            if matches!(data.operations[index].kind, OperationKind::PrepareCall(found) if found == call) {
                edit::detach(data, OpId::from_index(index).unwrap());
            }
        }
        let start = data.operands.len() as u32;
        budget.extend_copy(Retained, &mut data.operands, &values)?;
        data.operations[op.index()].operands = OperandRange {
            start,
            len: values.len() as u32,
        };
        let keys = budget.copy_slice(Retained, &keys)?;
        edit::replace_kind(
            data,
            op,
            OperationKind::Allocate {
                identity,
                kind: AllocationKind::Object(keys),
            },
            budget,
        )?;
        receipt.exposed_allocations += 1;
    }
    Ok(changed)
    })
}

fn namespaces(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            let mut changes = Vec::new();
            for unit in &editor.program().units {
                for (index, op) in unit.data().operations.iter().enumerate() {
                    budget.work(WorkKind::Analysis, 1)?;
                    let OperationKind::Call(call) = op.kind else {
                        continue;
                    };
                    let Some((site, field)) =
                        facts.namespace(editor.program(), effects, unit.id(), call)
                    else {
                        continue;
                    };
                    if !facts.sites[site].closed() || facts.sites[site].fields[field].written {
                        continue;
                    }
                    budget.push(
                        Retained,
                        &mut changes,
                        (unit.id(), OpId::from_index(index).unwrap(), call),
                    )?;
                }
            }
            Ok::<_, super::RuleError>(changes)
        },
        |changes, editor, budget| {
            if changes.is_empty() {
                return Ok(false);
            }
            let dynamic = editor.intern_type_in(&(Type::Dynamic), budget)?;
            for &(unit, invoke, call) in changes {
                let data = editor.unit_mut_in(unit, budget)?;
                let args = budget.copy_slice(
                    Scratch,
                    data.arguments(data.calls[call.index()].arguments).unwrap(),
                )?;
                let (CallArgument::Value(receiver), CallArgument::Value(key)) = (args[0], args[1])
                else {
                    return Err("namespace arguments".into());
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
                budget.push(Retained, &mut data.places, Place::Index { receiver, key })?;
                let load = OpId::from_index(data.operations.len()).ok_or("operation capacity")?;
                let callee = ValueId::from_index(data.values.len()).ok_or("value capacity")?;
                budget.push(
                    Retained,
                    &mut data.values,
                    Value {
                        ty: dynamic,
                        definition: load,
                    },
                )?;
                budget.push(
                    Retained,
                    &mut data.operations,
                    Operation {
                        kind: OperationKind::Load(place),
                        operands: OperandRange { start: 0, len: 0 },
                        result: Some(callee),
                        ..original.clone()
                    },
                )?;
                let region = &mut data.regions[original.region.index()];
                let at = region
                    .operations
                    .iter()
                    .position(|&op| op == invoke)
                    .ok_or("namespace invocation")?;
                budget.reserve_vec(Retained, &mut region.operations, 2)?;
                region.operations.splice(at..at, [load, prepare]);
                let arguments = ArgumentRange {
                    start: data.call_arguments.len() as u32,
                    len: (args.len() - 2) as u32,
                };
                budget.extend_copy(Retained, &mut data.call_arguments, &args[2..])?;
                let site = &mut data.calls[call.index()];
                site.target = CallTarget::Value {
                    callee,
                    invocation: Invocation::Value,
                };
                site.arguments = arguments;
                site.contract.signature = Some(dynamic);
                site.contract.supplied = (args.len() - 2) as u32;
                // An owned function applies its own declaration defaults. Flattening
                // the namespace preserves the supplied argument count.
                site.contract.defaults = crate::primitive::DefaultConvention::ApplyAtCallee;
                storage::release_vec(args, Scratch, budget)?;
                receipt.flattened_namespace_calls += 1;
            }
            Ok(true)
        },
    )
}

// Feed the existing codec-judged record family one canonical reference cell.
// The family still owns absence normalization and payload representation.
fn record_aliases(
    editor: &mut Editor<'_>,
    effects: &ProgramEffects,
    facts: &ProgramAggregates,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            budget.retained_phase(|budget| {
                let program = editor.program();
                let links = ScopeLinks::build(program, budget)?;
                let mut replacements = Vec::new();
                let mut limits = 0u32;
                let mut work = 0usize;
                for (site, allocation) in facts.sites.iter().enumerate() {
                    if !allocation.record {
                        continue;
                    }
                    let data = program.unit(allocation.unit).unwrap();
                    budget.work(WorkKind::Analysis, data.operations.len() as u64 + 1)?;
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
                        budget.work(WorkKind::Analysis, 1)?;
                        work += 1;
                        if work > (1 << 22) {
                            limits += 1;
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
                                budget.work(WorkKind::Analysis, 1)?;
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
                                        valid &= effects.initialization().initialized(
                                            program,
                                            unit.id(),
                                            at,
                                            alias,
                                        ) && links.contains(
                                            program,
                                            allocation.unit,
                                            program.cells[canonical.index()].region,
                                            unit.id(),
                                            at,
                                            budget,
                                        )?;
                                        budget.push(Retained, &mut loads, (unit.id(), at))?;
                                    }
                                }
                            }
                        }
                        if valid && !loads.is_empty() {
                            budget.push(
                                Retained,
                                &mut replacements,
                                (alias, canonical, initialize, loads),
                            )?;
                        } else {
                            storage::release_vec(loads, Retained, budget)?;
                        }
                    }
                }
                Ok::<_, super::RuleError>((replacements, work, limits))
            })
        },
        |(replacements, work, limits), editor, budget| {
            receipt.aggregate_analysis_work =
                receipt.aggregate_analysis_work.saturating_add(*work as u64);
            receipt.aggregate_limits += *limits;
            let changed = !replacements.is_empty();
            for &(alias, canonical, initialize, ref loads) in replacements {
                for &(unit, at) in loads {
                    let data = editor.unit_mut_in(unit, budget)?;
                    let place = PlaceId::from_index(data.places.len()).unwrap();
                    budget.push(Retained, &mut data.places, Place::Cell(canonical))?;
                    data.operations[at.index()].kind = OperationKind::Load(place);
                }
                if let Some((unit, at)) = initialize {
                    edit::detach(editor.unit_mut_in(unit, budget)?, at);
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
                        let data = editor.unit_mut_in(unit, budget)?;
                        data.captures.retain(|&cell| cell != alias);
                        budget.push(Retained, &mut data.captures, canonical)?;
                        data.captures.sort_unstable();
                        data.captures.dedup();
                    }
                }
                receipt.elided_record_aliases += 1;
            }
            Ok(changed)
        },
    )
}

// A contiguous construction prefix contains only literals, the fresh handle's
// aliases and own-field stores. Delay that fresh allocation to the last store,
// retaining every original operand evaluation and handle definition in order.
fn collect_stores(
    editor: &mut Editor<'_>,
    pristine: bool,
    receipt: &mut RuleReceipt,
    budget: &mut crate::output_budget::AllocationBudget<'_>,
) -> Result<bool, super::RuleError> {
    budget.with_temporary_context(
        editor,
        |editor, budget| {
            let program = editor.program();
            let mut plans = Vec::new();
            for unit in &program.units {
                for region in &unit.data().regions {
                    for (position, &allocated) in region.operations.iter().enumerate() {
                        budget.work(WorkKind::Analysis, 1)?;
                        if let Some(plan) = collected_plan(
                            program,
                            unit.id(),
                            region,
                            position,
                            allocated,
                            pristine,
                            budget,
                        )? {
                            budget.push(Retained, &mut plans, plan)?;
                        }
                    }
                }
            }
            Ok::<_, super::RuleError>(plans)
        },
        |plans, editor, budget| {
            let changed = !plans.is_empty();
            for &(
                unit,
                region,
                allocated,
                last,
                ref keys,
                ref values,
                ref transport,
                ref removed,
            ) in plans
            {
                let data = editor.unit_mut_in(unit, budget)?;
                let start = data.operands.len() as u32;
                budget.extend_copy(Retained, &mut data.operands, &values)?;
                data.operations[allocated.index()].operands = OperandRange {
                    start,
                    len: values.len() as u32,
                };
                match &mut data.operations[allocated.index()].kind {
                    OperationKind::Allocate {
                        kind: AllocationKind::Object(names) | AllocationKind::Record(names),
                        ..
                    } => {
                        let copied = storage::collect(
                            keys.iter().map(|key| match key {
                                Key::Named(name) => *name,
                                _ => unreachable!(),
                            }),
                            Retained,
                            budget,
                        )?;
                        storage::release_vec(std::mem::replace(names, copied), Retained, budget)?;
                    }
                    _ => {}
                }
                let order = &mut data.regions[region.index()].operations;
                let at = order
                    .iter()
                    .position(|&op| op == last)
                    .ok_or("collected store owner")?;
                let mut replacement = budget.vector(Retained, order.len())?;
                for (index, &op) in order.iter().enumerate() {
                    if index == at {
                        budget.extend_copy(Retained, &mut replacement, &transport)?;
                    }
                    if !transport.contains(&op) && !removed.contains(&op) {
                        budget.push(Retained, &mut replacement, op)?;
                    }
                }
                storage::release_vec(std::mem::replace(order, replacement), Retained, budget)?;
                receipt.collected_field_stores += removed
                    .iter()
                    .filter(|op| {
                        matches!(data.operations[op.index()].kind, OperationKind::Store(_))
                    })
                    .count() as u32;
            }
            Ok(changed)
        },
    )
}

type CollectedPlan = (
    UnitId,
    RegionId,
    OpId,
    OpId,
    Vec<Key>,
    Vec<ValueId>,
    Vec<OpId>,
    Vec<OpId>,
);
fn collected_plan(
    program: &Program<'_>,
    unit: UnitId,
    region: &Region,
    position: usize,
    allocated: OpId,
    pristine: bool,
    budget: &mut AllocationBudget<'_>,
) -> Result<Option<CollectedPlan>, AllocationError> {
    storage::optional(budget, |attempt| {
        let data = program.unit(unit)?;
        let original = &data.operations[allocated.index()];
        let OperationKind::Allocate { kind, .. } = &original.kind else {
            return None;
        };
        let (mut keys, record, array) = match kind {
            AllocationKind::Object(keys) if keys.len() <= 256 => (
                attempt.collect(Retained, keys.iter().copied().map(Key::Named))?,
                false,
                false,
            ),
            AllocationKind::Record(keys) if keys.len() <= 256 => (
                attempt.collect(Retained, keys.iter().copied().map(Key::Named))?,
                true,
                false,
            ),
            AllocationKind::Array if data.operands(original.operands)?.len() <= 256 => (
                attempt.collect(
                    Retained,
                    (0..data.operands(original.operands)?.len()).map(|n| Key::Index(n as u32)),
                )?,
                false,
                true,
            ),
            _ => return None,
        };
        attempt.work((keys.len() as u64).saturating_mul(keys.len() as u64 + 1))?;
        if keys.len() > 256 {
            return None;
        }
        if keys.iter().any(|key| matches!(key, Key::Named(k) if program.strings[k.index()].as_unicode() == Some("__proto__"))) { return None; }
        if keys.iter().enumerate().any(|(n, k)| keys[..n].contains(k)) {
            return None;
        }
        let mut values = attempt.admit(|budget| {
            budget.copy_slice(Retained, data.operands(original.operands).unwrap())
        })?;
        let mut handles = attempt.collect(Scratch, std::iter::once(original.result.unwrap()))?;
        let mut cells = Vec::new();
        let mut transport = attempt.collect(Retained, std::iter::once(allocated))?;
        let mut removed = Vec::new();
        let mut last = None;
        for &at in region.operations[position + 1..].iter().take(256) {
            let op = &data.operations[at.index()];
            let args = data.operands(op.operands).unwrap();
            attempt.work(
                (args.len() as u64 + 1).saturating_mul(handles.len() as u64 + 1)
                    + cells.len() as u64
                    + keys.len() as u64
                    + 1,
            )?;
            match op.kind {
                OperationKind::Constant(_) => {}
                OperationKind::Initialize(cell)
                    if args.first().is_some_and(|v| handles.contains(v)) =>
                {
                    attempt.push(Scratch, &mut cells, cell)?;
                    attempt.push(Retained, &mut transport, at)?;
                }
                OperationKind::CopyValue if args.first().is_some_and(|v| handles.contains(v)) => {
                    if let Some(result) = op.result {
                        attempt.push(Scratch, &mut handles, result)?;
                    }
                    attempt.push(Retained, &mut transport, at)?;
                }
                OperationKind::Load(place) if matches!(data.places[place.index()], Place::Cell(c) if cells.contains(&c)) =>
                {
                    if let Some(result) = op.result {
                        attempt.push(Scratch, &mut handles, result)?;
                    }
                    attempt.push(Retained, &mut transport, at)?;
                }
                OperationKind::Store(place) | OperationKind::CheckPlace(place) => {
                    let Some((receiver, key)) = aggregates::projection(program, data, place) else {
                        break;
                    };
                    if !handles.contains(&receiver) {
                        break;
                    }
                    if matches!(key, Key::Named(k) if program.strings[k.index()].as_unicode() == Some("__proto__"))
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
                            attempt.push(Retained, &mut keys, key)?;
                            attempt.push(Retained, &mut values, args[0])?;
                        }
                        last = Some(at);
                    }
                    attempt.push(Retained, &mut removed, at)?;
                }
                _ => break,
            }
        }
        let last = last?;
        // Only move/remove through the final collected store: a later
        // alias/check remains in place and retains its original order.
        attempt.work(
            (region.operations.len() as u64)
                .saturating_mul(transport.len() as u64 + removed.len() as u64 + 1),
        )?;
        let end = region.operations.iter().position(|&op| op == last).unwrap();
        transport.retain(|op| region.operations[..=end].contains(op));
        removed.retain(|op| region.operations[..=end].contains(op));
        Some((
            unit,
            original.region,
            allocated,
            last,
            keys,
            values,
            transport,
            removed,
        ))
    })
}
