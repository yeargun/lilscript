//! Choose storage while semantic values and physical evaluation sites are still
//! explicit. Pending roots are owned expression occurrences, not a copied IR.
//! Fusion consumes an ordered suffix; it never duplicates or speculates work.
use super::demand::{ContextId, DemandPlan, EffectiveUseRole, EffectiveUseSite};
use super::*;
use crate::compilation_policy::WorkKind;
use crate::js;
use crate::output_budget::{AllocationBudget, AllocationClass::Scratch, AllocationError};

#[derive(Debug, Clone, Copy)]
pub(super) enum ValueStorage {
    Absent,
    Candidate,
    Required,
    Deferred(Option<js::ExprId>),
    Captured(js::BindingId),
    /// A total read of a cell that nothing writes after initialization.
    /// It is not an evaluation event: each use reads the binding again.
    Rematerialized,
}
impl ValueStorage {
    pub(super) fn binding(self) -> Option<js::BindingId> {
        match self {
            Self::Captured(binding) => Some(binding),
            _ => None,
        }
    }
    fn deferred(self) -> bool {
        matches!(self, Self::Deferred(_))
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PlacementDepth {
    /// Certified upper bound on target ancestry at scalar-expression insertion
    /// sites in this physical context, including its expression-region parents.
    pub enclosing: usize,
    pub limit: usize,
}

#[derive(Clone, Copy)]
enum Uses {
    None,
    One(EffectiveUseSite),
    Many,
    Capture,
}
#[derive(Clone, Copy)]
struct Root {
    value: ValueId,
    height: usize,
}
#[derive(Clone, Copy)]
struct CallFrame {
    call: CallId,
    floor: usize,
    callee_height: usize,
    /// Bound every original producer as a possible captured prefix; later
    /// retroactive capture cannot add an unseen subtree to this envelope.
    prefix_height: usize,
}

// A call can add Call + ToInt32/Void + its argument Sequence. One more Assign
// covers a retained result. These are bounds on existing typed target recipes,
// not a global count of operations that happen to have been fused.
const CALL_LAYERS: usize = 3;
const CONSUMER_LAYERS: usize = CALL_LAYERS + 1;
const ENCLOSING_CALL_LAYERS: usize = CALL_LAYERS + 1;
/// A deferred closure's whole consumer tree stays at most this tall. Its body
/// is planned before that tree exists, so the bound is fixed in advance.
const CLOSURE_TREE_HEIGHT: usize = 16;
/// Target layers between a closure's creation site and its body statements:
/// Assign→Function→function→region→statement when captured, and the consumer
/// shells plus the bounded tree when deferred.
pub(super) const CAPTURED_CLOSURE_ENTRY: usize = 4;
pub(super) const DEFERRED_CLOSURE_ENTRY: usize = CONSUMER_LAYERS + CLOSURE_TREE_HEIGHT + 4;

fn work(budget: &mut AllocationBudget<'_>, amount: usize) -> Result<(), AllocationError> {
    budget.work(
        WorkKind::Analysis,
        u64::try_from(amount).map_err(|_| AllocationError::Capacity)?,
    )
}
fn bounded_add(a: usize, b: usize) -> usize {
    a.saturating_add(b)
}

/// One bounded scratch summary of the verified earlier-base place DAG. Paths
/// are shared by all load/store recipes in this physical activation; no client
/// repeatedly walks ancestors to rediscover their target-height contribution.
fn projection_depths(
    data: &UnitData,
    budget: &mut AllocationBudget<'_>,
    mut root_depth: impl FnMut(
        PlaceId,
        CellId,
        &mut AllocationBudget<'_>,
    ) -> Result<usize, AllocationError>,
) -> Result<Vec<usize>, AllocationError> {
    // Most units have only lexical/host places. Admit the projection table on
    // its first actual client; those units still use this same planning route.
    let mut depths = Vec::new();
    for (index, place) in data.places.iter().enumerate() {
        work(budget, 1)?;
        let depth = match *place {
            Place::Field { base, .. } => {
                if base.index() >= index {
                    return Err(AllocationError::Capacity);
                }
                depths
                    .get(base.index())
                    .copied()
                    .unwrap_or(0usize)
                    .checked_add(1)
                    .ok_or(AllocationError::Capacity)?
            }
            Place::Cell(cell) => root_depth(
                PlaceId::from_index(index).ok_or(AllocationError::Capacity)?,
                cell,
                budget,
            )?,
            _ => 0,
        };
        if depth != 0 && depths.is_empty() {
            depths = budget.filled(Scratch, data.places.len(), 0usize)?;
        }
        if !depths.is_empty() {
            depths[index] = depth;
        }
    }
    Ok(depths)
}

/// One optional bit per place for the actual reference ABI. Value-only units
/// allocate no table. The earlier-base DAG gives every projection its root bit.
fn reference_places(
    data: &UnitData,
    demand: &DemandPlan<'_, '_>,
    budget: &mut AllocationBudget<'_>,
) -> Result<Vec<bool>, AllocationError> {
    let mut references = Vec::new();
    for (index, place) in data.places.iter().enumerate() {
        work(budget, 1)?;
        let is_reference = match *place {
            Place::Cell(cell) => demand.is_reference_parameter(cell),
            Place::Field { base, .. } => references.get(base.index()).copied().unwrap_or(false),
            _ => false,
        };
        if is_reference && references.is_empty() {
            references = budget.filled(Scratch, data.places.len(), false)?;
        }
        if !references.is_empty() {
            references[index] = is_reference;
        }
    }
    Ok(references)
}

/// All Candidates are resolved before Formation allocates any SSA binding.
/// On failure the caller discards its un-emitted formation state. Scratch is
/// lexical and releases through the same borrowed output allocation owner.
pub(super) fn plan(
    data: &UnitData,
    demand: &DemandPlan<'_, '_>,
    context: ContextId,
    storage: &mut [ValueStorage],
    depth: PlacementDepth,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    if storage.len() != data.values.len() {
        return Err(AllocationError::Capacity);
    }
    let mut phase = budget.scope();
    let place_depths = projection_depths(data, &mut phase, |place, cell, budget| {
        if !demand.context(context).kind.is_inline() || !demand.is_reference_parameter(cell) {
            return Ok(0);
        }
        // The same original-place resolver used by demand supplies the actual
        // prefix omitted from this inline unit's local Place DAG. Local Field
        // edges then extend it once through the ordinary earlier-base scan.
        let location = demand
            .resolve_location_visited(super::demand::PlaceLocation { context, place }, |n| {
                work(budget, n)
            })?;
        Ok(location.map_or(0, |location| location.field_depth))
    })?;
    let reference_places = reference_places(data, demand, &mut phase)?;
    let mut counts = phase.filled(Scratch, data.values.len(), Uses::None)?;
    let mut last_site = None;
    let mut site_uses = 0usize;
    let mut maximum_inputs = 0usize;
    demand.visit_effective_value_uses(
        context,
        |n| work(&mut phase, n),
        |input| {
            counts[input.value.index()] =
                if matches!(input.role, EffectiveUseRole::CapturedCallCallee(_)) {
                    Uses::Capture
                } else {
                    match counts[input.value.index()] {
                        Uses::None => Uses::One(input.site),
                        Uses::Capture => Uses::Capture,
                        _ => Uses::Many,
                    }
                };
            if last_site == Some(input.site) {
                site_uses = site_uses.checked_add(1).ok_or(AllocationError::Capacity)?;
            } else {
                last_site = Some(input.site);
                site_uses = 1;
            }
            maximum_inputs = maximum_inputs.max(site_uses);
            Ok(())
        },
    )?;
    // A field update rooted at an array element, record entry or class
    // field re-reads that root while rebuilding it, so the root's receiver
    // and key are captured and read repeatedly, never deferred into one
    // occurrence.
    let mut aggregate_roots = Vec::new();
    for operation in &data.operations {
        work(&mut phase, 1)?;
        let (OperationKind::Store(mut root) | OperationKind::CheckPlace(mut root)) = operation.kind
        else {
            continue;
        };
        let mut projected = false;
        while let Place::Field { base, .. } = data.places[root.index()] {
            work(&mut phase, 1)?;
            root = base;
            projected = true;
        }
        let (receiver, key) = match data.places[root.index()] {
            Place::Member { receiver, .. } if projected => (receiver, None),
            Place::Index { receiver, key } if projected => (receiver, Some(key)),
            _ => continue,
        };
        if aggregate_roots.is_empty() {
            aggregate_roots = phase.filled(Scratch, data.values.len(), false)?;
        }
        aggregate_roots[receiver.index()] = true;
        if let Some(key) = key {
            aggregate_roots[key.index()] = true;
        }
    }
    // Regions formed inside a parent expression: arms, operands, loop tests
    // and updates. Their results leave the tree that planned them.
    let mut expression_regions = phase.filled(Scratch, data.regions.len(), false)?;
    for operation in &data.operations {
        work(&mut phase, 1)?;
        match operation.kind {
            OperationKind::Select { yes, no } => {
                expression_regions[yes.index()] = true;
                expression_regions[no.index()] = true;
            }
            OperationKind::ShortCircuit { right, .. } => expression_regions[right.index()] = true,
            OperationKind::Loop { test, update, .. } => {
                expression_regions[test.index()] = true;
                expression_regions[update.index()] = true;
            }
            _ => {}
        }
    }
    for (index, slot) in storage.iter_mut().enumerate() {
        work(&mut phase, 1)?;
        if !matches!(slot, ValueStorage::Candidate) {
            continue;
        }
        let aggregate_root = aggregate_roots.get(index).copied().unwrap_or(false);
        let operation = data.values[index].definition;
        let same_region = match counts[index] {
            Uses::One(EffectiveUseSite::Operation(consumer)) => {
                data.operations[consumer.index()].region
                    == data.operations[operation.index()].region
            }
            Uses::One(EffectiveUseSite::RegionResult(region)) => {
                region == data.operations[operation.index()].region
            }
            _ => false,
        };
        // An inline body has no height proof. A closure's body is planned for
        // the fixed deferred-closure bound, which a consumer tree in a
        // statement region keeps; a region result leaves that tree.
        let child_body = demand
            .child(context, operation)
            .is_some_and(|child| demand.context(child).kind.is_inline())
            || matches!(
                data.operations[operation.index()].kind,
                OperationKind::Closure(_)
            ) && (expression_regions[data.operations[operation.index()].region.index()]
                || !matches!(counts[index], Uses::One(EffectiveUseSite::Operation(_))));
        // Reconstructing a value's ancestors reads its current sibling fields.
        // The RHS must already have completed, including calls that mutate
        // those siblings. Capturing here preserves the semantic schedule; a
        // deferred call inside the reconstruction would read stale siblings.
        // Constants and selected inert aliases have Absent storage and never
        // enter this candidate-fusion decision.
        let field_store_rhs = match counts[index] {
            Uses::One(EffectiveUseSite::Operation(consumer)) => {
                let consumer = &data.operations[consumer.index()];
                matches!(consumer.kind, OperationKind::Store(place)
                    if matches!(data.places[place.index()], Place::Field { .. }) || reference_places.get(place.index()).copied().unwrap_or(false))
                    && data
                        .operands(consumer.operands)
                        .and_then(|values| values.first())
                        == Some(&ValueId::from_index(index).unwrap())
            }
            _ => false,
        };
        *slot = if same_region && !child_body && !field_store_rhs && !aggregate_root {
            ValueStorage::Deferred(None)
        } else {
            ValueStorage::Required
        };
    }
    let mut inputs = phase.vector(Scratch, maximum_inputs)?;
    let mut pending = phase.vector(Scratch, data.values.len())?;
    let mut frames = phase.vector(Scratch, data.calls.len())?;
    let mut heights = phase.filled(Scratch, data.values.len(), 1usize)?;
    // Pending roots whose tree contains a deferred closure.
    let mut closures = phase.filled(Scratch, data.values.len(), false)?;
    let mut region_heights = phase.filled(Scratch, data.regions.len(), 1usize)?;
    let mut order = phase.vector(Scratch, data.regions.len())?;
    let stack_capacity = data
        .regions
        .len()
        .checked_mul(2)
        .ok_or(AllocationError::Capacity)?;
    let mut regions = phase.vector(Scratch, stack_capacity)?;
    regions.push((data.entry, false));
    while let Some((region, finish)) = regions.pop() {
        work(&mut phase, 1)?;
        if finish {
            order.push(region);
            continue;
        }
        regions.push((region, true));
        for &operation in &data.regions[region.index()].operations {
            work(&mut phase, 1)?;
            for child in data.operations[operation.index()].kind.child_regions() {
                work(&mut phase, 1)?;
                regions.push((child, false));
            }
        }
    }
    for &region in &order {
        pending.clear();
        frames.clear();
        let mut maximum_height = 1usize;
        for &operation in &data.regions[region.index()].operations {
            work(&mut phase, 1)?;
            let op = &data.operations[operation.index()];
            let prepare = matches!(op.kind, OperationKind::PrepareCall(_));
            let tail_return = demand.context(context).kind.is_inline()
                && demand.needs_return(context)
                && matches!(op.kind, OperationKind::Return);
            let needed = demand.needs_operation(context, operation) || tail_return;
            // A removed invocation still closes its prepared argument envelope.
            // Formation emits the retained argument work at this exact boundary.
            if !prepare && !matches!(op.kind, OperationKind::Call(_)) && !needed {
                continue;
            }
            if matches!(op.kind, OperationKind::Constant(_)) {
                continue;
            }
            // Each use reads a rematerialized load again; its definition is
            // not an evaluation event.
            if op
                .result
                .is_some_and(|value| matches!(storage[value.index()], ValueStorage::Rematerialized))
            {
                continue;
            }
            work(&mut phase, demand.product_lookup_work())?;
            let product_recipe = demand
                .product_operation(demand.context(context).unit, operation)
                .is_some_and(|recipe| {
                    !matches!(
                        recipe,
                        super::product_family::ProductOperationKind::Access(_)
                    )
                });
            // Absent storage with production-only demand can be implemented
            // by already-selected shared data. There is no evaluation event
            // at this definition to obstruct adjacent expression placement.
            if op
                .result
                .is_some_and(|value| matches!(storage[value.index()], ValueStorage::Absent))
                && !demand.needs_execution(context, operation)
                && !product_recipe
                && op.kind.child_regions().next().is_none()
                && !matches!(op.kind, OperationKind::Call(_))
            {
                continue;
            }
            inputs.clear();
            let mut input_extra = 0;
            demand.visit_effective_site_uses(
                context,
                EffectiveUseSite::Operation(operation),
                |n| work(&mut phase, n),
                |input| {
                    // Capacity was admitted from this exact occurrence inventory.
                    debug_assert!(inputs.len() < inputs.capacity());
                    input_extra = input_extra.max(demand.effective_input_depth(context, input));
                    inputs.push(input.value);
                    Ok(())
                },
            )?;
            let floor = frames.last().map_or(0, |frame: &CallFrame| frame.floor);
            let mut input_height = bounded_add(
                claim(&inputs, &mut pending, floor, storage, &heights, &mut phase)?,
                input_extra,
            );
            if let OperationKind::PrepareCall(call) = op.kind {
                frames.push(CallFrame {
                    call,
                    floor: pending.len(),
                    callee_height: input_height,
                    prefix_height: 0,
                });
                continue;
            }
            let mut frame_prefix = 0;
            let mut enclosing = bounded_add(
                depth.enclosing,
                frames.len().saturating_mul(ENCLOSING_CALL_LAYERS),
            );
            if let OperationKind::Call(call) = op.kind {
                let frame = frames.pop().expect("checked balanced call schedule");
                assert_eq!(frame.call, call, "checked prepared-call identity");
                frame_prefix = frame.prefix_height;
                // Remaining argument work executes inside the call's prefix.
                frame_prefix =
                    frame_prefix.max(flush(&mut pending, frame.floor, storage, &mut phase)?);
                input_height = input_height
                    .max(frame.callee_height)
                    .max(bounded_add(frame_prefix, 1));
                enclosing = bounded_add(
                    depth.enclosing,
                    frames.len().saturating_mul(ENCLOSING_CALL_LAYERS),
                );
                if !needed && frame_prefix == 0 {
                    // No invocation, callee lookup, or argument work survives.
                    // This envelope contributes no evaluation barrier at all.
                    continue;
                }
            }
            work(&mut phase, 1)?;
            let opaque_child = demand
                .child(context, operation)
                .is_some_and(|child| demand.context(child).kind.is_inline());
            // A closure-bearing tree has the fixed closure height bound; any
            // taller consumer captures its operands instead.
            let mut carries_closure = matches!(op.kind, OperationKind::Closure(_))
                || inputs
                    .iter()
                    .any(|value| storage[value.index()].deferred() && closures[value.index()]);
            let mut limit = depth.limit;
            if carries_closure {
                limit = limit.min(bounded_add(
                    bounded_add(enclosing, CLOSURE_TREE_HEIGHT),
                    CONSUMER_LAYERS,
                ));
            }
            let mut height = if needed && product_recipe {
                bounded_add(input_height, 2)
            } else if needed {
                recipe_height(
                    op,
                    data,
                    &reference_places,
                    input_height,
                    &region_heights,
                    &place_depths,
                    depth.limit,
                    opaque_child,
                )
            } else {
                // The discarded invocation leaves only its argument Sequence.
                bounded_add(frame_prefix, 1)
            };
            // Inserting an expression into a parent must fit its real recipe
            // path. Capturing operands severs only those extra nested paths.
            if bounded_add(bounded_add(enclosing, height), CONSUMER_LAYERS) > limit {
                for &value in &inputs {
                    work(&mut phase, 1)?;
                    capture_value(value, &mut pending, storage, &mut phase)?;
                }
                carries_closure = matches!(op.kind, OperationKind::Closure(_));
                limit = depth.limit;
                input_height = bounded_add(1, input_extra).max(bounded_add(frame_prefix, 1));
                height = if needed && product_recipe {
                    bounded_add(input_height, 2)
                } else if needed {
                    recipe_height(
                        op,
                        data,
                        &reference_places,
                        input_height,
                        &region_heights,
                        &place_depths,
                        depth.limit,
                        opaque_child,
                    )
                } else {
                    bounded_add(frame_prefix, 1)
                };
                if bounded_add(bounded_add(enclosing, height), CONSUMER_LAYERS) > limit {
                    if matches!(op.kind, OperationKind::PrepareReference { .. })
                        || matches!(op.kind, OperationKind::Load(place) | OperationKind::Store(place) | OperationKind::CheckPlace(place)
                        if place_depths.get(place.index()).copied().unwrap_or(0) > 0)
                    {
                        // Required result storage cannot shorten the intrinsic
                        // projection/reconstruction path. Refuse it before
                        // building an over-deep typed target expression.
                        return Err(AllocationError::Capacity);
                    }
                    if let Some(result) = op.result {
                        if storage[result.index()].deferred() {
                            storage[result.index()] = ValueStorage::Required;
                        }
                    }
                }
            }
            let deferred = op
                .result
                .is_some_and(|value| storage[value.index()].deferred());
            if deferred {
                let value = op.result.unwrap();
                heights[value.index()] = height;
                closures[value.index()] = carries_closure;
                pending.push(Root { value, height });
            } else {
                let floor = frames.last().map_or(0, |frame| frame.floor);
                let captured = flush(&mut pending, floor, storage, &mut phase)?;
                maximum_height = maximum_height.max(captured).max(bounded_add(height, 1));
                if let Some(frame) = frames.last_mut() {
                    frame.prefix_height = frame.prefix_height.max(captured);
                }
            }
            if let Some(frame) = frames.last_mut() {
                frame.prefix_height = frame.prefix_height.max(bounded_add(height, 1));
            }
        }
        assert!(
            frames.is_empty(),
            "checked region has no unfinished prepared calls"
        );
        inputs.clear();
        let mut result_extra = 0;
        demand.visit_effective_site_uses(
            context,
            EffectiveUseSite::RegionResult(region),
            |n| work(&mut phase, n),
            |input| {
                result_extra = result_extra.max(demand.effective_input_depth(context, input));
                inputs.push(input.value);
                Ok(())
            },
        )?;
        let result_height = bounded_add(
            claim(&inputs, &mut pending, 0, storage, &heights, &mut phase)?,
            result_extra,
        );
        maximum_height =
            maximum_height
                .max(result_height)
                .max(flush(&mut pending, 0, storage, &mut phase)?);
        // Expression-region formation joins required work and its result with
        // at most one Sequence node. Statement regions only overestimate here.
        region_heights[region.index()] = bounded_add(maximum_height, 1);
    }
    work(&mut phase, storage.len())?;
    debug_assert!(storage
        .iter()
        .all(|value| !matches!(value, ValueStorage::Candidate)));
    drop((
        counts,
        expression_regions,
        inputs,
        pending,
        frames,
        heights,
        closures,
        region_heights,
        order,
        regions,
    ));
    Ok(())
}

fn capture_value(
    value: ValueId,
    pending: &mut [Root],
    storage: &mut [ValueStorage],
    budget: &mut AllocationBudget<'_>,
) -> Result<(), AllocationError> {
    if !storage[value.index()].deferred() {
        return Ok(());
    }
    // Retroactively fixing a root at its definition also fixes earlier roots:
    // they cannot be postponed past its now-observable evaluation statement.
    for root in pending {
        work(budget, 1)?;
        if storage[root.value.index()].deferred() {
            storage[root.value.index()] = ValueStorage::Required;
        }
        if root.value == value {
            break;
        }
    }
    storage[value.index()] = ValueStorage::Required;
    Ok(())
}
fn flush(
    pending: &mut Vec<Root>,
    floor: usize,
    storage: &mut [ValueStorage],
    budget: &mut AllocationBudget<'_>,
) -> Result<usize, AllocationError> {
    let mut height = 0;
    for root in &pending[floor..] {
        work(budget, 1)?;
        if storage[root.value.index()].deferred() {
            storage[root.value.index()] = ValueStorage::Required;
            height = height.max(bounded_add(root.height, 1));
        }
    }
    pending.truncate(floor);
    Ok(height)
}
fn claim(
    inputs: &[ValueId],
    pending: &mut Vec<Root>,
    floor: usize,
    storage: &mut [ValueStorage],
    heights: &[usize],
    budget: &mut AllocationBudget<'_>,
) -> Result<usize, AllocationError> {
    let mut position = pending.len();
    let mut height = 1;
    let mut matches = true;
    for &value in inputs.iter().rev() {
        work(budget, 1)?;
        if !storage[value.index()].deferred() {
            continue;
        }
        while position > floor && !storage[pending[position - 1].value.index()].deferred() {
            work(budget, 1)?;
            position -= 1;
        }
        if position == floor || pending[position - 1].value != value {
            matches = false;
            break;
        }
        position -= 1;
        height = height.max(heights[value.index()]);
    }
    if matches {
        pending.truncate(position);
        return Ok(height);
    }
    flush(pending, floor, storage, budget)?;
    for &value in inputs {
        work(budget, 1)?;
        capture_value(value, pending, storage, budget)?;
    }
    Ok(1)
}

fn recipe_height(
    operation: &Operation,
    data: &UnitData,
    references: &[bool],
    input: usize,
    regions: &[usize],
    places: &[usize],
    limit: usize,
    opaque_child: bool,
) -> usize {
    // Independently planned bodies have no scalar height proof. Propagate this
    // through lazy regions/call envelopes so embedding cannot deepen a child
    // context behind Formation's certified static insertion-depth bound.
    if opaque_child {
        return limit.saturating_add(1);
    }
    use OperationKind as Op;
    match operation.kind {
        Op::Constant(_) => 1,
        Op::CopyValue => input,
        // Address preflight has no SSA result to capture. Its actual checked
        // access still traverses the place path and a bounded void/check shell.
        Op::PrepareReference { call, position } => {
            let CallArgument::Reference(place) =
                data.arguments(data.calls[call.index()].arguments).unwrap()[position as usize]
            else {
                unreachable!("verified reference argument")
            };
            bounded_add(
                input,
                bounded_add(places.get(place.index()).copied().unwrap_or(0), 4),
            )
            .max(4)
        }
        Op::Store(place) if references.get(place.index()).copied().unwrap_or(false) => {
            let depth = places.get(place.index()).copied().unwrap_or(0);
            bounded_add(
                input,
                if depth == 0 {
                    5
                } else {
                    bounded_add(depth.saturating_mul(2), 8)
                },
            )
        }
        Op::CheckPlace(place) => bounded_add(
            bounded_add(input, places.get(place.index()).copied().unwrap_or(0)),
            3,
        ),
        Op::Load(place) => bounded_add(
            bounded_add(input, places.get(place.index()).copied().unwrap_or(0)),
            3,
        )
        .max(5),
        Op::Store(place) if places.get(place.index()).copied().unwrap_or(0) > 0 => bounded_add(
            bounded_add(input, places[place.index()].saturating_mul(2)),
            4,
        ),
        Op::Store(_) => bounded_add(input, 2),
        Op::IntBinary(_) | Op::Unary { .. } | Op::Intrinsic(_) => bounded_add(input, 1),
        Op::Binary(BinaryOp::Nullish) => bounded_add(input, 3).max(5),
        Op::Binary(_) | Op::Allocate { .. } => bounded_add(input, 1),
        Op::Call(_) => bounded_add(input, CALL_LAYERS).max(4),
        Op::ShortCircuit { right, kind } => {
            let inner = input.max(regions[right.index()]);
            if kind == ShortCircuit::Nullish {
                bounded_add(inner, 3).max(5)
            } else {
                bounded_add(inner, 1)
            }
        }
        Op::Select { yes, no } => {
            bounded_add(input.max(regions[yes.index()]).max(regions[no.index()]), 1)
        }
        // A closure's body is another activation, planned for the fixed
        // deferred-closure bound; to its consumer it is one leaf.
        Op::Closure(_) => 1,
        _ => bounded_add(input, 1),
    }
}

#[cfg(test)]
mod tests {
    use super::super::demand::DemandMode;
    use super::*;
    use crate::compilation_policy::{
        BudgetError, BudgetLedger, BudgetPlan, CompilationRequest, ResourceLimits, WorkDomain,
    };

    fn ledger() -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 10_000_000,
                optional_work: 10_000_000,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000_000,
            },
        )
        .unwrap()
    }
    fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        inspect(&from_checked_source(&syntax, &semantics).unwrap());
    }
    fn with_placement(
        program: &Program<'_>,
        limit: usize,
        inspect: impl FnOnce(&UnitData, &[ValueStorage]),
    ) {
        with_placement_contract(program, limit, false, inspect);
    }
    fn with_placement_contract(
        program: &Program<'_>,
        limit: usize,
        strip_console: bool,
        inspect: impl FnOnce(&UnitData, &[ValueStorage]),
    ) {
        let mut config = crate::config::ProjectConfig::default();
        config.javascript.strip_console = strip_console;
        let policy = config
            .resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            })
            .unwrap();
        let demand = DemandPlan::build(
            program,
            None,
            None,
            policy.javascript_contract().unwrap(),
            DemandMode::Prune,
            None,
        )
        .unwrap();
        let context = demand.root();
        let data = program.unit(demand.context(context).unit).unwrap();
        let mut storage: Vec<_> = data
            .values
            .iter()
            .enumerate()
            .map(|(index, value)| {
                if demand.needs_value(context, ValueId::from_index(index).unwrap())
                    && !matches!(
                        data.operations[value.definition.index()].kind,
                        OperationKind::Constant(_)
                    )
                {
                    ValueStorage::Candidate
                } else {
                    ValueStorage::Absent
                }
            })
            .collect();
        let mut ledger = ledger();
        {
            let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
            plan(
                data,
                &demand,
                context,
                &mut storage,
                PlacementDepth {
                    enclosing: 0,
                    limit,
                },
                &mut budget,
            )
            .unwrap();
            assert_eq!(
                budget.retained_bytes(Scratch),
                0,
                "planner has no retained scratch"
            );
        }
        assert_eq!(ledger.retained_bytes(), 0);
        assert!(ledger.peak_retained_bytes() > 0);
        assert!(storage
            .iter()
            .all(|value| !matches!(value, ValueStorage::Candidate)));
        inspect(data, &storage);
        demand.discard(None).unwrap();
    }
    #[test]
    fn lexical_only_places_need_no_projection_workspace() {
        checked("int value=1;print(value);", |program| {
            let data = program.unit(program.initialization[0]).unwrap();
            assert!(!data.places.is_empty());
            let mut ledger = BudgetLedger::new(
                ResourceLimits::default(),
                BudgetPlan {
                    baseline_work: 10_000,
                    optional_work: 10_000,
                    baseline_retained_bytes: 0,
                    retained_bytes: 0,
                },
            )
            .unwrap();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Baseline)));
                let depths = projection_depths(data, &mut budget, |_, _, _| Ok(0)).unwrap();
                assert!(depths.is_empty());
            }
            assert_eq!(ledger.peak_retained_bytes(), 0);
        });
    }

    #[test]
    fn struct_reconstruction_captures_rhs_before_reading_current_siblings() {
        checked("struct Pair{int x;int y;}Pair target=Pair{1,2};int rhs(){target.y=9;return 3;}target.x=rhs();print(target.y);", |program| {
            with_placement(program, js::MAX_NESTING, |data, storage| {
                let store = data.operations.iter().find(|operation|
                    matches!(operation.kind, OperationKind::Store(place)
                        if matches!(data.places[place.index()], Place::Field {..}))).unwrap();
                let rhs = data.operands(store.operands).unwrap()[0];
                assert!(matches!(data.operations[data.values[rhs.index()].definition.index()].kind,
                    OperationKind::Call(_)), "fixture uses one effectful RHS with no earlier materialization");
                assert!(matches!(storage[rhs.index()], ValueStorage::Required),
                    "reconstruction cannot defer the call below current sibling reads");
            });
        });
    }

    #[test]
    fn nested_place_depth_scratch_is_admitted_once_and_released_on_failure() {
        checked("struct P{int x;}struct Box{P point;}Box value=Box{P{1}};value.point.x=2;print(value.point.x);", |program| {
            let data = program.unit(program.initialization[0]).unwrap();
            let mut ledger = ledger();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut phase = budget.scope();
                let depths = projection_depths(data, &mut phase, |_, _, _| Ok(0)).unwrap();
                assert_eq!(depths.iter().copied().max(), Some(2));
                for operation in &data.operations {
                    if let OperationKind::Load(place) | OperationKind::Store(place) = operation.kind {
                        if depths[place.index()] == 2 {
                            let height = recipe_height(operation, data, &[], 1, &[], &depths, js::MAX_NESTING, false);
                            assert!(height >= 6, "nested projections retain both path levels");
                            if matches!(operation.kind, OperationKind::Store(_)) {
                                assert_eq!(height, 9, "two-level reconstruction includes current siblings");
                            }
                        }
                    }
                }
                drop(depths);
                drop(phase);
                assert_eq!(budget.retained_bytes(Scratch), 0);
            }
            assert_eq!(ledger.retained_bytes(), 0);
            let mut tiny = BudgetLedger::new(ResourceLimits::default(), BudgetPlan {
                baseline_work: 10_000, optional_work: 10_000, baseline_retained_bytes: 0,
                retained_bytes: 1,
            }).unwrap();
            {
                let mut budget = AllocationBudget::new(Some((&mut tiny, WorkDomain::Optional)));
                assert!(matches!(projection_depths(data, &mut budget, |_, _, _| Ok(0)),
                    Err(AllocationError::Budget(BudgetError::MemoryExhausted(WorkDomain::Optional)))));
            }
            assert_eq!(tiny.retained_bytes(), 0);
            let mut config = crate::config::ProjectConfig::default();
            config.javascript.strip_console = false;
            let policy = config.resolve_policy(CompilationRequest::JavaScript {
                preserve_root_exports: true,
            }).unwrap();
            let demand = DemandPlan::build(program, None, None,
                policy.javascript_contract().unwrap(), DemandMode::Prune, None).unwrap();
            let context = demand.root();
            let mut storage = data.values.iter().enumerate().map(|(index, value)| {
                if demand.needs_value(context, ValueId::from_index(index).unwrap())
                    && !matches!(data.operations[value.definition.index()].kind, OperationKind::Constant(_)) {
                    ValueStorage::Candidate
                } else { ValueStorage::Absent }
            }).collect::<Vec<_>>();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                assert!(matches!(plan(data, &demand, context, &mut storage,
                    PlacementDepth {enclosing: 0, limit: 8}, &mut budget),
                    Err(AllocationError::Capacity)),
                    "capturing a result cannot shorten a reconstruction's intrinsic path");
                assert_eq!(budget.retained_bytes(Scratch), 0,
                    "depth refusal releases the planner's partial scratch state");
            }
            assert_eq!(ledger.retained_bytes(), 0);
            demand.discard(None).unwrap();
        });
    }

    fn value(index: usize) -> ValueId {
        ValueId::from_index(index).unwrap()
    }
    #[test]
    fn ordered_suffix_is_consumed_once_and_reverse_order_requires_storage() {
        let mut budget = AllocationBudget::new(None);
        let mut storage = [ValueStorage::Deferred(None); 2];
        let heights = [3, 4];
        let mut pending = vec![
            Root {
                value: value(0),
                height: 3,
            },
            Root {
                value: value(1),
                height: 4,
            },
        ];
        assert_eq!(
            claim(
                &[value(0), value(1)],
                &mut pending,
                0,
                &mut storage,
                &heights,
                &mut budget
            )
            .unwrap(),
            4
        );
        assert!(pending.is_empty());
        assert!(storage.iter().all(|value| value.deferred()));
        let mut pending = vec![
            Root {
                value: value(0),
                height: 3,
            },
            Root {
                value: value(1),
                height: 4,
            },
        ];
        assert_eq!(
            claim(
                &[value(1), value(0)],
                &mut pending,
                0,
                &mut storage,
                &heights,
                &mut budget
            )
            .unwrap(),
            1
        );
        assert!(storage
            .iter()
            .all(|value| matches!(value, ValueStorage::Required)));
    }
    #[test]
    fn cross_call_frame_capture_also_fixes_earlier_roots() {
        let mut budget = AllocationBudget::new(None);
        let mut storage = [ValueStorage::Deferred(None); 2];
        let heights = [3, 4];
        let mut pending = vec![
            Root {
                value: value(0),
                height: 3,
            },
            Root {
                value: value(1),
                height: 4,
            },
        ];
        assert_eq!(
            claim(
                &[value(1)],
                &mut pending,
                2,
                &mut storage,
                &heights,
                &mut budget
            )
            .unwrap(),
            1
        );
        assert_eq!(pending.len(), 2, "suspended frame positions remain stable");
        assert!(storage
            .iter()
            .all(|value| matches!(value, ValueStorage::Required)));
    }
    #[test]
    fn required_work_inside_arguments_does_not_flush_suspended_parent_roots() {
        let mut budget = AllocationBudget::new(None);
        let mut storage = [ValueStorage::Deferred(None); 2];
        let mut pending = vec![
            Root {
                value: value(0),
                height: 3,
            },
            Root {
                value: value(1),
                height: 4,
            },
        ];
        assert_eq!(
            flush(&mut pending, 1, &mut storage, &mut budget).unwrap(),
            5
        );
        assert_eq!(pending.len(), 1);
        assert!(storage[0].deferred());
        assert!(matches!(storage[1], ValueStorage::Required));
    }
    #[test]
    fn nested_argument_calls_remain_expressions_in_original_order() {
        checked("extern int first();extern int second();extern void sink(int left,int right);sink(first(),second());",|program|with_placement(program,512,|data,storage|{
            let mut inner=0;
            for operation in &data.operations {
                if let OperationKind::Call(call)=operation.kind {
                    let arguments=data.arguments(data.calls[call.index()].arguments).unwrap();
                    if arguments.is_empty()&&matches!(data.calls[call.index()].target,CallTarget::Value{..}) {
                        assert!(storage[operation.result.unwrap().index()].deferred());inner+=1;
                    }
                }
            }
            assert_eq!(inner,2);
        }));
    }
    #[test]
    fn removed_call_envelopes_close_with_retained_argument_work() {
        checked(
            "extern int first();extern int second();print(first()+second());print(1);",
            |program| {
                with_placement_contract(program, 512, true, |data, storage| {
                    let mut hosts = 0;
                    for operation in &data.operations {
                        if let OperationKind::Call(call) = operation.kind {
                            if matches!(data.calls[call.index()].target, CallTarget::Value { .. }) {
                                // A foreign result is not proved primitive: the
                                // addition still owes its observable coercion and
                                // possible exception. Each call remains its single
                                // input expression, despite the removed logger.
                                assert!(storage[operation.result.unwrap().index()].deferred());
                                hosts += 1;
                            }
                        }
                    }
                    assert_eq!(hosts, 2);
                    let addition = data
                        .operations
                        .iter()
                        .find(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
                        .unwrap();
                    assert!(
                        matches!(
                            storage[addition.result.unwrap().index()],
                            ValueStorage::Absent
                        ),
                        "the coercion executes without storing its unobserved result"
                    );
                });
            },
        );
    }
    #[test]
    fn empty_removed_envelope_does_not_capture_an_adjacent_expression() {
        checked(
            "extern int first();extern void sink(int value);sink(first()+2);print(1);",
            |original| {
                let mut program = original.clone();
                let unit = program.initialization[0];
                let mut changed = program.units[unit.index()].clone().into_working();
                let data = changed.get_mut();
                let print = data
                    .calls
                    .iter()
                    .position(|target| {
                        matches!(target.target, CallTarget::Builtin(BuiltinCall::Print))
                    })
                    .unwrap();
                let arithmetic = data
                    .operations
                    .iter()
                    .position(|operation| matches!(operation.kind, OperationKind::IntBinary(_)))
                    .unwrap();
                let schedule = &mut data.regions[data.entry.index()].operations;
                let start = schedule.iter().position(|id| {
                    matches!(data.operations[id.index()].kind, OperationKind::PrepareCall(call) if call.index() == print)
                }).unwrap();
                let removed = schedule.drain(start..).collect::<Vec<_>>();
                let insert = schedule
                    .iter()
                    .position(|id| id.index() == arithmetic)
                    .unwrap();
                schedule.splice(insert..insert, removed);
                program.units[unit.index()] = changed.freeze();
                program.verify().unwrap();
                with_placement_contract(&program, 512, true, |data, storage| {
                    let first = data
                        .operations
                        .iter()
                        .find(|operation| {
                            matches!(operation.kind, OperationKind::Call(call) if data.calls[call.index()].arguments.len == 0)
                        })
                        .unwrap();
                    assert!(storage[first.result.unwrap().index()].deferred());
                });
            },
        );
    }
    #[test]
    fn reordered_value_uses_preserve_original_effectful_definitions() {
        checked(
            "extern int first();extern int second();print(first()+second());",
            |original| {
                let mut program = original.clone();
                let unit = program.initialization[0];
                let mut changed = program.units[unit.index()].clone().into_working();
                let data = changed.get_mut();
                let operation = data
                    .operations
                    .iter()
                    .find(|op| matches!(op.kind, OperationKind::IntBinary(_)))
                    .unwrap();
                let start = operation.operands.start as usize;
                let left = data.operands[start];
                let right = data.operands[start + 1];
                data.operands.swap(start, start + 1);
                program.units[unit.index()] = changed.freeze();
                program.verify().unwrap();
                with_placement(&program, 512, |_, storage| {
                    assert!(matches!(storage[left.index()], ValueStorage::Required));
                    assert!(matches!(storage[right.index()], ValueStorage::Required));
                });
            },
        );
    }
    #[test]
    fn depth_admission_segments_long_arithmetic_without_disabling_all_fusion() {
        let source = format!(
            "print({});",
            std::iter::repeat_n("1", 90).collect::<Vec<_>>().join("+")
        );
        checked(&source, |program| {
            with_placement(program, 32, |data, storage| {
                let mut depths = vec![1usize; data.values.len()];
                let mut deferred = 0;
                let mut captured = 0;
                let mut maximum = 0;
                for operation in &data.operations {
                    let Some(result) = operation.result else {
                        continue;
                    };
                    if matches!(operation.kind, OperationKind::IntBinary(_)) {
                        // Actual structured IntBinary contributes one node. A
                        // captured result contributes Assign and becomes a leaf at
                        // each later use; this oracle does not use recipe_height.
                        let height = 1 + data
                            .operands(operation.operands)
                            .unwrap()
                            .iter()
                            .map(|value| depths[value.index()])
                            .max()
                            .unwrap();
                        if storage[result.index()].deferred() {
                            deferred += 1;
                            depths[result.index()] = height;
                            maximum = maximum.max(height);
                        } else {
                            captured += 1;
                            depths[result.index()] = 1;
                            maximum = maximum.max(height + 1);
                        }
                    }
                }
                assert!(
                    deferred > 40 && captured > 0,
                    "bounded fusion remains useful"
                );
                assert!(
                    maximum + 3 <= 32,
                    "complete print-call wrapper also fits: {maximum}"
                );
            })
        });
    }
    #[test]
    fn exhausted_work_releases_planner_scratch_and_leaves_source_valid() {
        checked("print(1+2);", |program| {
            let mut config = crate::config::ProjectConfig::default();
            config.javascript.strip_console = false;
            let policy = config
                .resolve_policy(CompilationRequest::JavaScript {
                    preserve_root_exports: true,
                })
                .unwrap();
            let demand = DemandPlan::build(
                program,
                None,
                None,
                policy.javascript_contract().unwrap(),
                DemandMode::Prune,
                None,
            )
            .unwrap();
            let context = demand.root();
            let data = program.unit(demand.context(context).unit).unwrap();
            let mut storage = vec![ValueStorage::Candidate; data.values.len()];
            let mut ledger = ledger();
            ledger
                .charge(WorkDomain::Optional, WorkKind::Analysis, 10_000_000)
                .unwrap();
            {
                let mut budget = AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                assert!(matches!(
                    plan(
                        data,
                        &demand,
                        context,
                        &mut storage,
                        PlacementDepth {
                            enclosing: 0,
                            limit: 512
                        },
                        &mut budget
                    ),
                    Err(AllocationError::Budget(BudgetError::WorkExhausted(
                        WorkDomain::Optional
                    )))
                ));
            }
            assert_eq!(ledger.retained_bytes(), 0);
            program.verify().unwrap();
            demand.discard(None).unwrap();
        });
    }
}
