use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::stable_hash::{StableHashMap as AHashMap, StableHashSet as AHashSet};

use crate::ir::{
    BlockId, ConstValue, ControlFlowBlock, ControlFlowFunction, ControlFlowInstruction,
    ControlFlowModule, ControlFlowOp, ExportBinding, FunctionId, FunctionKind, FunctionOrigin,
    Intrinsic, IrBinaryOp, IrParameter, IrUnaryOp, LocalId, Terminator, ValueId,
};
use crate::optimizer::{
    analyze_escapes, instruction_has_dynamic_observable_evaluation, OptimizationReport,
};
use crate::semantic::{EscapeState, SymbolId, Type};
use crate::span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressPassOptions {
    pub pipeline_fusion: bool,
    pub partial_escape_sinking: bool,
    pub region_outlining: bool,
    pub expression_superopt: bool,
    pub path_sensitive_propagation: bool,
}

impl Default for CompressPassOptions {
    fn default() -> Self {
        Self {
            pipeline_fusion: true,
            partial_escape_sinking: true,
            region_outlining: true,
            expression_superopt: true,
            path_sensitive_propagation: true,
        }
    }
}

pub fn run_compress_passes(
    module: &mut ControlFlowModule<'_>,
    options: &CompressPassOptions,
) -> Vec<OptimizationReport> {
    run_compress_passes_tracking_outlined_helpers(module, options).0
}

/// Run the compression pipeline and return the helper functions synthesized by
/// repeated-region outlining. The optimizer uses this exact set to keep a
/// profitable outlined boundary from being immediately undone by its late
/// inlining fixed point; other callees remain eligible for normal inlining.
pub(crate) fn run_compress_passes_tracking_outlined_helpers(
    module: &mut ControlFlowModule<'_>,
    options: &CompressPassOptions,
) -> (Vec<OptimizationReport>, Vec<FunctionId>) {
    let mut reports = Vec::new();
    let mut outlined_helpers = Vec::new();
    if options.path_sensitive_propagation {
        reports.push(propagate_path_sensitive_constants(module));
    }
    if options.expression_superopt {
        reports.push(superoptimize_pure_expressions(module));
    }
    if options.partial_escape_sinking {
        reports.push(sink_partial_escape_allocations(module));
    }
    if options.pipeline_fusion {
        reports.push(fuse_array_pipelines(module));
    }
    if options.region_outlining {
        let first_outlined_function = module.functions.len();
        reports.push(outline_repeated_regions(module));
        outlined_helpers.extend(
            module.functions[first_outlined_function..]
                .iter()
                .map(|function| function.id),
        );
        if !outlined_helpers.is_empty() {
            // Outlining synthesizes parameters, return values, and replacement
            // call results. Rebuild their transitive boundary states before a
            // public pass caller can emit or further transform this IR.
            reports.push(analyze_escapes(module));
        }
    }
    (reports, outlined_helpers)
}

pub fn fuse_array_pipelines(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    let mut pending_functions = Vec::new();

    for function_index in 0..module.functions.len() {
        if has_exception_region(&module.functions[function_index]) {
            continue;
        }
        let uses = control_flow_use_counts(&module.functions[function_index]);
        let definitions = instruction_definitions(&module.functions[function_index]);

        let mut fusions = Vec::new();
        for (block_index, block) in module.functions[function_index].blocks.iter().enumerate() {
            for (inst_index, instruction) in block.instructions.iter().enumerate() {
                let Some(producer_out) = instruction.out else {
                    continue;
                };
                let ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayMap,
                    receiver: Some(base),
                    args: producer_args,
                } = &instruction.op
                else {
                    continue;
                };
                if uses.get(&producer_out).copied().unwrap_or(0) != 1 || producer_args.len() != 1 {
                    continue;
                }
                let Some(producer_callback) = producer_args.first().copied() else {
                    continue;
                };
                let Some(consumer_loc) = find_single_array_pipeline_consumer(
                    &module.functions[function_index],
                    producer_out,
                ) else {
                    continue;
                };
                if consumer_loc.0 != block_index || consumer_loc.1 <= inst_index {
                    continue;
                }
                let consumer = &block.instructions[consumer_loc.1];
                let ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayMap,
                    receiver: Some(consumer_receiver),
                    args: consumer_args,
                } = &consumer.op
                else {
                    continue;
                };
                if *consumer_receiver != producer_out || consumer_args.len() != 1 {
                    continue;
                }
                let Some(consumer_callback) = consumer_args.first().copied() else {
                    continue;
                };
                let Some((first_fn, first_captures)) =
                    closure_target(&definitions, producer_callback)
                else {
                    continue;
                };
                let Some((second_fn, second_captures)) =
                    closure_target(&definitions, consumer_callback)
                else {
                    continue;
                };
                if !compatible_map_callbacks(module, first_fn, first_captures.len())
                    || !compatible_map_callbacks(module, second_fn, second_captures.len())
                {
                    continue;
                }
                fusions.push(PipelineFusion {
                    block_index,
                    producer_index: inst_index,
                    consumer_index: consumer_loc.1,
                    base: *base,
                    first_fn,
                    first_captures: first_captures.to_vec(),
                    second_fn,
                    second_captures: second_captures.to_vec(),
                });
            }
        }

        for fusion in fusions.into_iter().rev() {
            let fused_id =
                FunctionId(module.functions.len() as u32 + pending_functions.len() as u32);
            let first = &module.functions[fusion.first_fn.0 as usize];
            let second = &module.functions[fusion.second_fn.0 as usize];
            let Some(fused) = build_fused_map_callback(
                fused_id,
                first,
                second,
                fusion.first_captures.len(),
                fusion.second_captures.len(),
            ) else {
                continue;
            };
            pending_functions.push(fused);

            let function = &mut module.functions[function_index];
            let span = function.blocks[fusion.block_index].instructions[fusion.consumer_index].span;
            let closure_out = ValueId(function.value_count);
            function.value_count += 1;
            function.value_escapes.push(EscapeState::LocalOnly);
            function.value_local_hints.push(None);

            let mut captures = fusion.first_captures.clone();
            captures.extend(fusion.second_captures.iter().copied());

            let producer =
                &mut function.blocks[fusion.block_index].instructions[fusion.producer_index];
            producer.out = Some(closure_out);
            producer.ty = None;
            producer.op = ControlFlowOp::Closure {
                function: fused_id,
                captures,
            };
            producer.span = span;

            let consumer =
                &mut function.blocks[fusion.block_index].instructions[fusion.consumer_index];
            consumer.op = ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::ArrayMap,
                receiver: Some(fusion.base),
                args: vec![closure_out],
            };
            changed = true;
        }
    }

    module.functions.extend(pending_functions);

    OptimizationReport {
        pass_name: "array-pipeline-fusion",
        changed,
    }
}

struct PipelineFusion {
    block_index: usize,
    producer_index: usize,
    consumer_index: usize,
    base: ValueId,
    first_fn: FunctionId,
    first_captures: Vec<ValueId>,
    second_fn: FunctionId,
    second_captures: Vec<ValueId>,
}

fn find_single_array_pipeline_consumer(
    function: &ControlFlowFunction<'_>,
    producer: ValueId,
) -> Option<(usize, usize)> {
    let mut found = None;
    for (block_index, block) in function.blocks.iter().enumerate() {
        for (inst_index, instruction) in block.instructions.iter().enumerate() {
            let ControlFlowOp::Intrinsic {
                intrinsic,
                receiver: Some(receiver),
                ..
            } = &instruction.op
            else {
                continue;
            };
            if *receiver != producer {
                continue;
            }
            if !matches!(
                intrinsic,
                Intrinsic::ArrayMap | Intrinsic::ArrayFilter | Intrinsic::ArrayReduce
            ) {
                return None;
            }
            if found.is_some() {
                return None;
            }
            found = Some((block_index, inst_index));
        }
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_used_values)
        {
            if value == producer {
                return None;
            }
        }
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                if *value == producer {
                    return None;
                }
            }
        }
    }
    found
}

fn compatible_map_callbacks(
    module: &ControlFlowModule<'_>,
    function: FunctionId,
    capture_len: usize,
) -> bool {
    let Some(target) = module.functions.get(function.0 as usize) else {
        return false;
    };
    target.live
        && target.kind == FunctionKind::Closure
        && target.mutable_capture_locals.is_empty()
        && !target.is_async
        && !target.is_generator
        && !has_exception_region(target)
        && target.capture_count == capture_len
        && target.params.len() == capture_len + 1
}

fn build_fused_map_callback<'src>(
    id: FunctionId,
    first: &ControlFlowFunction<'src>,
    second: &ControlFlowFunction<'src>,
    first_captures: usize,
    second_captures: usize,
) -> Option<ControlFlowFunction<'src>> {
    let element = first.params.get(first_captures)?;
    let mut params = Vec::new();
    let mut next_value = 0_u32;
    let empty = Span::empty(0);

    let mut push_param = |source: &IrParameter<'src>| {
        let value = ValueId(next_value);
        next_value += 1;
        let local = LocalId(params.len() as u32);
        params.push(IrParameter {
            symbol: SymbolId(params.len() as u32),
            local,
            value,
            name: source.name,
            ty: source.ty.clone(),
            default: None,
            span: empty,
        });
        value
    };

    let mut first_capture_values = Vec::with_capacity(first_captures);
    for parameter in first.params.iter().take(first_captures) {
        first_capture_values.push(push_param(parameter));
    }
    let mut second_capture_values = Vec::with_capacity(second_captures);
    for parameter in second.params.iter().take(second_captures) {
        second_capture_values.push(push_param(parameter));
    }
    let element_value = push_param(element);

    let mid = ValueId(next_value);
    next_value += 1;
    let result = ValueId(next_value);
    next_value += 1;

    let mut first_args = first_capture_values;
    first_args.push(element_value);
    let mut second_args = second_capture_values;
    second_args.push(mid);

    let instructions = vec![
        ControlFlowInstruction {
            out: Some(mid),
            ty: Some(first.return_type.clone()),
            op: ControlFlowOp::CallDirect {
                function: first.id,
                provided_args: first_args.len(),
                args: first_args,
            },
            span: empty,
        },
        ControlFlowInstruction {
            out: Some(result),
            ty: Some(second.return_type.clone()),
            op: ControlFlowOp::CallDirect {
                function: second.id,
                provided_args: second_args.len(),
                args: second_args,
            },
            span: empty,
        },
    ];

    Some(ControlFlowFunction {
        id,
        name: None,
        kind: FunctionKind::Closure,
        origin: FunctionOrigin::Synthesized,
        declared_pure: first.declared_pure && second.declared_pure,
        is_async: false,
        is_generator: false,
        params,
        capture_count: first_captures + second_captures,
        mutable_capture_locals: Vec::new(),
        return_type: second.return_type.clone(),
        locals: Vec::new(),
        blocks: vec![ControlFlowBlock {
            id: BlockId(0),
            phis: Vec::new(),
            instructions,
            terminator: Some(Terminator::Return(Some(result))),
            span: empty,
        }],
        shapes: Vec::new(),
        entry: BlockId(0),
        value_count: next_value,
        value_local_hints: vec![None; next_value as usize],
        value_escapes: vec![EscapeState::LocalOnly; next_value as usize],
        locals_promoted: true,
        live: true,
        span: empty,
    })
}

pub fn sink_partial_escape_allocations(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;

    for function in &mut module.functions {
        if has_exception_region(function) {
            continue;
        }

        let mut sink_targets = Vec::new();
        for block_index in 0..function.blocks.len() {
            let Some(Terminator::Branch {
                then_block,
                else_block,
                ..
            }) = function.blocks[block_index].terminator.clone()
            else {
                continue;
            };

            let instructions = function.blocks[block_index].instructions.clone();
            for (inst_index, instruction) in instructions.iter().enumerate() {
                let Some(out) = instruction.out else {
                    continue;
                };
                if !is_sinkable_allocation(&instruction.op) {
                    continue;
                }
                if function
                    .value_escapes
                    .get(out.0 as usize)
                    .copied()
                    .unwrap_or(EscapeState::LocalOnly)
                    != EscapeState::LocalOnly
                {
                    continue;
                }

                let mut then_uses = 0usize;
                let mut else_uses = 0usize;
                let mut other_uses = 0usize;
                let mut join_phi = false;

                for (other_index, block) in function.blocks.iter().enumerate() {
                    for phi in &block.phis {
                        for (_, value) in &phi.incoming {
                            if *value == out {
                                join_phi = true;
                            }
                        }
                    }
                    for used in block.instructions.iter().flat_map(|inst| {
                        control_flow_used_values(&inst.op)
                            .into_iter()
                            .chain(inst.out.into_iter().filter(|_| false))
                    }) {
                        if used != out {
                            continue;
                        }
                        if other_index == then_block.0 as usize {
                            then_uses += 1;
                        } else if other_index == else_block.0 as usize {
                            else_uses += 1;
                        } else {
                            other_uses += 1;
                        }
                    }
                    for used in block
                        .terminator
                        .as_ref()
                        .into_iter()
                        .flat_map(terminator_used_values)
                    {
                        if used != out {
                            continue;
                        }
                        if other_index == then_block.0 as usize {
                            then_uses += 1;
                        } else if other_index == else_block.0 as usize {
                            else_uses += 1;
                        } else {
                            other_uses += 1;
                        }
                    }
                }

                if join_phi || other_uses != 0 {
                    continue;
                }
                let target = if then_uses > 0 && else_uses == 0 {
                    then_block
                } else if else_uses > 0 && then_uses == 0 {
                    else_block
                } else {
                    continue;
                };

                if !allocation_uses_are_local_reads(function, out, target) {
                    continue;
                }

                sink_targets.push((block_index, inst_index, target));
            }
        }

        sink_targets.sort_by_key(|target| std::cmp::Reverse(target.1));
        for (block_index, inst_index, target) in sink_targets {
            let instruction = function.blocks[block_index].instructions.remove(inst_index);
            function.blocks[target.0 as usize]
                .instructions
                .insert(0, instruction);
            changed = true;
        }
    }

    OptimizationReport {
        pass_name: "partial-escape-allocation-sinking",
        changed,
    }
}

fn is_sinkable_allocation(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::Array(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::NewClass {
                constructor: None,
                ..
            }
    )
}

fn allocation_uses_are_local_reads(
    function: &ControlFlowFunction<'_>,
    allocation: ValueId,
    block: BlockId,
) -> bool {
    for instruction in &function.blocks[block.0 as usize].instructions {
        for used in control_flow_used_values(&instruction.op) {
            if used != allocation {
                continue;
            }
            if !matches!(
                &instruction.op,
                ControlFlowOp::FieldGet { object, .. }
                    | ControlFlowOp::RecordFieldGet { object, .. }
                    | ControlFlowOp::IndexGet { object, .. }
                    | ControlFlowOp::ArrayGetOptional { object, .. }
                    | ControlFlowOp::CallMethod { receiver: object, .. }
                    | ControlFlowOp::HostCall { receiver: object, .. }
                    | ControlFlowOp::HostFieldGet { object, .. }
                if *object == allocation
            ) {
                return false;
            }
        }
    }
    true
}

pub(crate) fn outline_repeated_regions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    // Function spans are translated into chunk ownership only after IR
    // optimization. A synthesized helper has no ownership metadata of its
    // own, so outlining in the presence of a lazy module could hoist a private
    // lazy-region helper into the entry chunk and introduce a static cycle.
    // Keep the complete unoutlined IR until outlining becomes owner-aware.
    if !module.lazy_modules.is_empty() {
        return OptimizationReport {
            pass_name: "repeated-region-outlining",
            changed: false,
        };
    }
    let exported = exported_functions(module);
    let pure_functions = module
        .functions
        .iter()
        .filter(|function| function.declared_pure)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();

    let mut candidates = AHashMap::<u64, Vec<RegionOccurrence>>::default();
    for function in module.functions.iter().filter(|function| {
        function.live
            // Closed-script optimization intentionally clears exports before
            // this pass. Repeated pure regions can therefore live directly in
            // the final entry function after ordinary wrapper inlining. The
            // region proof below is identical for Entry and Function bodies;
            // extern/method/constructor boundaries remain excluded.
            && matches!(function.kind, FunctionKind::Entry | FunctionKind::Function)
            && !function.is_async
            && !function.is_generator
            && !exported.contains(&function.id)
            && !has_exception_region(function)
    }) {
        let value_types = control_flow_value_types(function);
        for (block_index, block) in function.blocks.iter().enumerate() {
            let len = block.instructions.len();
            for window in 4..=28 {
                if window > len {
                    break;
                }
                for start in 0..=(len - window) {
                    let slice = &block.instructions[start..start + window];
                    if !region_is_outline_safe(slice, &pure_functions, &value_types) {
                        continue;
                    }
                    let Some((fingerprint, live_ins, live_outs)) =
                        fingerprint_region(slice, function, block_index, start, &value_types)
                    else {
                        continue;
                    };
                    if live_ins.len() > 8 || live_outs.len() > 1 {
                        continue;
                    }
                    candidates
                        .entry(fingerprint)
                        .or_default()
                        .push(RegionOccurrence {
                            function: function.id,
                            block: BlockId(block_index as u32),
                            start,
                            window,
                            live_ins,
                            live_outs,
                        });
                }
            }
        }
    }

    let mut groups = Vec::new();
    for (_, mut occurrences) in candidates {
        occurrences.sort_by_key(|occurrence| {
            (occurrence.function.0, occurrence.block.0, occurrence.start)
        });
        // A hash is only a candidate bucket. Verify the entire canonicalized
        // operation and type shape so collisions or omitted hand-hashed op
        // metadata can never combine semantically different regions.
        let mut equivalent = Vec::<Vec<RegionOccurrence>>::new();
        for occurrence in occurrences {
            if let Some(group) = equivalent
                .iter_mut()
                .find(|group| regions_are_exactly_equivalent(module, &group[0], &occurrence))
            {
                group.push(occurrence);
            } else {
                equivalent.push(vec![occurrence]);
            }
        }
        for mut group in equivalent {
            group.dedup_by(|left, right| {
                left.function == right.function
                    && left.block == right.block
                    && left.start < right.start + right.window
                    && right.start < left.start + left.window
            });
            if group.len() >= 2 {
                groups.push(group);
            }
        }
    }
    groups.sort_by_key(|group| {
        let window = group[0].window;
        let occurrences = group.len();
        std::cmp::Reverse((occurrences.saturating_sub(1)) * window)
    });

    let mut occupied = AHashSet::<(FunctionId, BlockId, usize)>::default();
    let mut selected = Vec::new();
    for group in groups {
        if selected.len() >= 4 {
            break;
        }
        let window = group[0].window;
        let occurrences = group.len();
        let prototype = &group[0];
        let source = &module.functions[prototype.function.0 as usize];
        let block_len = source.blocks[prototype.block.0 as usize].instructions.len();
        if prototype.start + prototype.window > block_len {
            continue;
        }
        let region = &source.blocks[prototype.block.0 as usize].instructions
            [prototype.start..prototype.start + prototype.window];
        let (helper_cost, savings) = if region_has_index_set(region) {
            (
                window.saturating_add(2),
                occurrences.saturating_sub(1).saturating_mul(window),
            )
        } else {
            (
                window,
                occurrences
                    .saturating_mul(window)
                    .saturating_sub(occurrences),
            )
        };
        if region_has_index_set(region) && occurrences < 3 {
            continue;
        }
        if savings <= helper_cost {
            continue;
        }
        if group.iter().any(|occurrence| {
            (0..occurrence.window).any(|offset| {
                occupied.contains(&(
                    occurrence.function,
                    occurrence.block,
                    occurrence.start + offset,
                ))
            })
        }) {
            continue;
        }
        for occurrence in &group {
            for offset in 0..occurrence.window {
                occupied.insert((
                    occurrence.function,
                    occurrence.block,
                    occurrence.start + offset,
                ));
            }
        }
        selected.push(group);
    }

    let mut pending = Vec::new();
    for group in &selected {
        let prototype = &group[0];
        let source = &module.functions[prototype.function.0 as usize];
        let block_len = source.blocks[prototype.block.0 as usize].instructions.len();
        if prototype.start + prototype.window > block_len {
            continue;
        }
        let region = source.blocks[prototype.block.0 as usize].instructions
            [prototype.start..prototype.start + prototype.window]
            .to_vec();
        let helper_id = FunctionId(module.functions.len() as u32);
        let Some(helper) = build_outlined_helper(helper_id, source, &region, prototype) else {
            continue;
        };
        module.functions.push(helper);
        for occurrence in group {
            pending.push((helper_id, occurrence.clone()));
        }
    }

    pending.sort_by_key(|(_, occurrence)| {
        (
            occurrence.function.0,
            occurrence.block.0,
            std::cmp::Reverse(occurrence.start),
        )
    });

    let mut changed = false;
    let mut replacement_aliases = AHashMap::<FunctionId, AHashMap<ValueId, ValueId>>::default();
    for (helper_id, occurrence) in pending {
        let function = &mut module.functions[occurrence.function.0 as usize];
        let block = &mut function.blocks[occurrence.block.0 as usize];
        if occurrence.start + occurrence.window > block.instructions.len() {
            continue;
        }
        let span = block.instructions[occurrence.start].span;
        let result_ty = occurrence
            .live_outs
            .first()
            .and_then(|out| {
                block.instructions[occurrence.start..occurrence.start + occurrence.window]
                    .iter()
                    .find_map(|instruction| {
                        (instruction.out == Some(*out)).then(|| instruction.ty.clone())
                    })
            })
            .flatten();
        let out = occurrence.live_outs.first().copied();
        let aliases = replacement_aliases.entry(occurrence.function).or_default();
        let args = occurrence
            .live_ins
            .iter()
            .map(|value| resolve_alias(*value, aliases))
            .collect::<Vec<_>>();
        if let Some(out) = out {
            let mapped = ValueId(function.value_count);
            function.value_count += 1;
            function.value_escapes.push(EscapeState::LocalOnly);
            function.value_local_hints.push(None);
            let call = ControlFlowInstruction {
                out: Some(mapped),
                ty: result_ty,
                op: ControlFlowOp::CallDirect {
                    function: helper_id,
                    provided_args: args.len(),
                    args,
                },
                span,
            };
            block.instructions.splice(
                occurrence.start..occurrence.start + occurrence.window,
                std::iter::once(call),
            );
            rewrite_control_flow_function(function, &AHashMap::from_iter([(out, mapped)]));
            aliases.insert(out, mapped);
        } else {
            let call = ControlFlowInstruction {
                out: None,
                ty: None,
                op: ControlFlowOp::CallDirect {
                    function: helper_id,
                    provided_args: args.len(),
                    args,
                },
                span,
            };
            block.instructions.splice(
                occurrence.start..occurrence.start + occurrence.window,
                std::iter::once(call),
            );
        }
        changed = true;
    }

    OptimizationReport {
        pass_name: "repeated-region-outlining",
        changed,
    }
}

#[derive(Debug, Clone)]
struct RegionOccurrence {
    function: FunctionId,
    block: BlockId,
    start: usize,
    window: usize,
    live_ins: Vec<ValueId>,
    live_outs: Vec<ValueId>,
}

fn region_is_outline_safe(
    region: &[ControlFlowInstruction<'_>],
    pure_functions: &AHashSet<FunctionId>,
    value_types: &AHashMap<ValueId, Type<'_>>,
) -> bool {
    region.iter().all(|instruction| {
        if instruction_has_dynamic_observable_evaluation(instruction, value_types) {
            return false;
        }
        match &instruction.op {
            ControlFlowOp::Const(_)
            | ControlFlowOp::Unary { .. }
            | ControlFlowOp::Binary { .. }
            | ControlFlowOp::TypeCheck { .. }
            | ControlFlowOp::Array(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::FieldGet { .. }
            | ControlFlowOp::RecordFieldGet { .. }
            | ControlFlowOp::IndexGet { .. }
            | ControlFlowOp::IndexSet { .. }
            | ControlFlowOp::ArrayGetOptional { .. } => true,
            ControlFlowOp::CallDirect { function, .. } => pure_functions.contains(function),
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::IntImul
                    | Intrinsic::IntToString
                    | Intrinsic::IntToUnsignedString
                    | Intrinsic::ArrayLength
                    | Intrinsic::FloatAbs
                    | Intrinsic::FloatFloor
                    | Intrinsic::FloatCeil
                    | Intrinsic::FloatRound
                    | Intrinsic::FloatSqrt
                    | Intrinsic::FloatSin
                    | Intrinsic::FloatCos
                    | Intrinsic::FloatAcos
                    | Intrinsic::FloatExp
                    | Intrinsic::FloatLog
                    | Intrinsic::FloatTan
                    | Intrinsic::FloatAtan2
                    | Intrinsic::FloatHypot
                    | Intrinsic::FloatMin
                    | Intrinsic::FloatMax
                    | Intrinsic::FloatToInt
                    | Intrinsic::StringLength
                    | Intrinsic::StringCharCodeAt
                    | Intrinsic::StringCharAt
                    | Intrinsic::StringIncludes
                    | Intrinsic::StringIndexOf
                    | Intrinsic::StringLastIndexOf
                    | Intrinsic::StringRepeat
                    | Intrinsic::StringStartsWith
                    | Intrinsic::StringEndsWith
                    | Intrinsic::StringToUpperCase
                    | Intrinsic::StringToLowerCase
                    | Intrinsic::JsTruthy
                    | Intrinsic::JsIsArray
                    | Intrinsic::JsIsObject
                    | Intrinsic::JsTypeOf
                    | Intrinsic::JsIsNullish
                    | Intrinsic::JsIsFalse
                    | Intrinsic::JsIsUndefined
                    | Intrinsic::JsStringSlice
                    | Intrinsic::JsStringIndexOf
                    | Intrinsic::JsStringSplit
                    | Intrinsic::JsBox,
                ..
            } => true,
            ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::HostCall { .. }
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::Await { .. }
            | ControlFlowOp::DynamicImport { .. }
            | ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::LoadLocal(_)
            | ControlFlowOp::LoadGlobal(_)
            | ControlFlowOp::FieldSet { .. }
            | ControlFlowOp::RecordFieldSet { .. }
            | ControlFlowOp::CaughtException => false,
            _ => false,
        }
    })
}

fn region_has_index_set(region: &[ControlFlowInstruction<'_>]) -> bool {
    region
        .iter()
        .any(|instruction| matches!(instruction.op, ControlFlowOp::IndexSet { .. }))
}

fn fingerprint_region(
    region: &[ControlFlowInstruction<'_>],
    function: &ControlFlowFunction<'_>,
    block_index: usize,
    start: usize,
    value_types: &AHashMap<ValueId, Type<'_>>,
) -> Option<(u64, Vec<ValueId>, Vec<ValueId>)> {
    let defined = region
        .iter()
        .enumerate()
        .filter_map(|(index, instruction)| instruction.out.map(|out| (out, index)))
        .collect::<AHashMap<_, _>>();

    let mut live_ins = Vec::new();
    let mut live_in_index = AHashMap::<ValueId, usize>::default();
    let mut hasher = DefaultHasher::new();
    region.len().hash(&mut hasher);

    for instruction in region {
        std::mem::discriminant(&instruction.op).hash(&mut hasher);
        instruction.out.is_some().hash(&mut hasher);
        // Type is intentionally part of the cheap hash as well as the exact
        // equivalence proof below. A string operation and a structurally
        // identical JsValue coercion must never share an outlined helper.
        format!("{:?}", instruction.ty).hash(&mut hasher);
        match &instruction.op {
            ControlFlowOp::Const(value) => hash_const(value, &mut hasher),
            ControlFlowOp::Unary { op, .. } => op.hash(&mut hasher),
            ControlFlowOp::Binary { op, .. } => op.hash(&mut hasher),
            ControlFlowOp::TypeCheck { target, .. } => format!("{target:?}").hash(&mut hasher),
            ControlFlowOp::CallDirect { function, .. } => function.0.hash(&mut hasher),
            ControlFlowOp::Intrinsic { intrinsic, .. } => intrinsic.hash(&mut hasher),
            ControlFlowOp::FieldGet {
                owner,
                field,
                index,
                ..
            }
            | ControlFlowOp::FieldSet {
                owner,
                field,
                index,
                ..
            } => {
                owner.hash(&mut hasher);
                field.hash(&mut hasher);
                index.hash(&mut hasher);
            }
            ControlFlowOp::RecordFieldGet { property, .. }
            | ControlFlowOp::RecordFieldSet { property, .. } => property.hash(&mut hasher),
            ControlFlowOp::Struct { name, .. } => name.hash(&mut hasher),
            ControlFlowOp::IndexGet { .. } | ControlFlowOp::IndexSet { .. } => {}
            _ => {}
        }
        for value in control_flow_used_values(&instruction.op) {
            if let Some(def_index) = defined.get(&value) {
                (0u8, *def_index).hash(&mut hasher);
            } else {
                let index = *live_in_index.entry(value).or_insert_with(|| {
                    let index = live_ins.len();
                    live_ins.push(value);
                    index
                });
                (1u8, index).hash(&mut hasher);
            }
        }
    }

    for live_in in &live_ins {
        format!("{:?}", value_types.get(live_in)?).hash(&mut hasher);
    }

    let region_defs = defined.keys().copied().collect::<AHashSet<_>>();
    let end = start + region.len();
    let mut live_outs = Vec::new();
    for (other_block, block) in function.blocks.iter().enumerate() {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                if region_defs.contains(value) {
                    push_unique(&mut live_outs, *value);
                }
            }
        }
        for (inst_index, instruction) in block.instructions.iter().enumerate() {
            if other_block == block_index && inst_index >= start && inst_index < end {
                continue;
            }
            for value in control_flow_used_values(&instruction.op) {
                if region_defs.contains(&value) {
                    push_unique(&mut live_outs, value);
                }
            }
        }
        for value in block
            .terminator
            .as_ref()
            .into_iter()
            .flat_map(terminator_used_values)
        {
            if region_defs.contains(&value) {
                push_unique(&mut live_outs, value);
            }
        }
    }

    for live_out in &live_outs {
        defined.get(live_out)?.hash(&mut hasher);
    }

    Some((hasher.finish(), live_ins, live_outs))
}

type CanonicalRegion<'src> = (
    Vec<Type<'src>>,
    Vec<(Option<ValueId>, Option<Type<'src>>, ControlFlowOp<'src>)>,
    Vec<ValueId>,
);

fn canonical_region<'src>(
    function: &ControlFlowFunction<'src>,
    occurrence: &RegionOccurrence,
) -> Option<CanonicalRegion<'src>> {
    let block = function.blocks.get(occurrence.block.0 as usize)?;
    let region = block
        .instructions
        .get(occurrence.start..occurrence.start + occurrence.window)?;
    let value_types = control_flow_value_types(function);
    let live_in_types = occurrence
        .live_ins
        .iter()
        .map(|value| value_types.get(value).cloned())
        .collect::<Option<Vec<_>>>()?;

    let mut remap = AHashMap::<ValueId, ValueId>::default();
    let mut next_value = 0_u32;
    for live_in in &occurrence.live_ins {
        remap.insert(*live_in, ValueId(next_value));
        next_value += 1;
    }

    let mut instructions = Vec::with_capacity(region.len());
    for instruction in region {
        let mut op = instruction.op.clone();
        rewrite_control_flow_op_once(&mut op, &remap);
        let out = instruction.out.map(|original| {
            let canonical = ValueId(next_value);
            next_value += 1;
            remap.insert(original, canonical);
            canonical
        });
        instructions.push((out, instruction.ty.clone(), op));
    }
    let live_outs = occurrence
        .live_outs
        .iter()
        .map(|value| remap.get(value).copied())
        .collect::<Option<Vec<_>>>()?;
    Some((live_in_types, instructions, live_outs))
}

fn regions_are_exactly_equivalent(
    module: &ControlFlowModule<'_>,
    left: &RegionOccurrence,
    right: &RegionOccurrence,
) -> bool {
    let Some(left_function) = module.functions.get(left.function.0 as usize) else {
        return false;
    };
    let Some(right_function) = module.functions.get(right.function.0 as usize) else {
        return false;
    };
    canonical_region(left_function, left)
        .zip(canonical_region(right_function, right))
        .is_some_and(|(left, right)| left == right)
}

fn build_outlined_helper<'src>(
    id: FunctionId,
    source: &ControlFlowFunction<'src>,
    region: &[ControlFlowInstruction<'src>],
    occurrence: &RegionOccurrence,
) -> Option<ControlFlowFunction<'src>> {
    let empty = Span::empty(0);
    let mut next_value = 0_u32;
    let mut params = Vec::new();
    let mut remap = AHashMap::<ValueId, ValueId>::default();
    let value_types = control_flow_value_types(source);

    for (index, live_in) in occurrence.live_ins.iter().enumerate() {
        let value = ValueId(next_value);
        next_value += 1;
        remap.insert(*live_in, value);
        let ty = value_types.get(live_in)?.clone();
        params.push(IrParameter {
            symbol: SymbolId(index as u32),
            local: LocalId(index as u32),
            value,
            name: "",
            ty,
            default: None,
            span: empty,
        });
    }

    let mut instructions = region.to_vec();
    for instruction in &mut instructions {
        if let Some(out) = instruction.out {
            let mapped = ValueId(next_value);
            next_value += 1;
            remap.insert(out, mapped);
            instruction.out = Some(mapped);
        }
        rewrite_control_flow_op_once(&mut instruction.op, &remap);
    }

    let return_value = occurrence
        .live_outs
        .first()
        .and_then(|out| remap.get(out).copied());
    let return_type = return_value
        .and_then(|value| {
            instructions.iter().find_map(|instruction| {
                (instruction.out == Some(value)).then(|| instruction.ty.clone())
            })
        })
        .flatten()
        .unwrap_or(Type::Void);

    Some(ControlFlowFunction {
        id,
        name: None,
        kind: FunctionKind::Function,
        origin: FunctionOrigin::RepeatedRegionOutline,
        declared_pure: !region_has_index_set(region),
        is_async: false,
        is_generator: false,
        params,
        capture_count: 0,
        mutable_capture_locals: Vec::new(),
        return_type,
        locals: Vec::new(),
        blocks: vec![ControlFlowBlock {
            id: BlockId(0),
            phis: Vec::new(),
            instructions,
            terminator: Some(Terminator::Return(return_value)),
            span: empty,
        }],
        shapes: Vec::new(),
        entry: BlockId(0),
        value_count: next_value,
        value_local_hints: vec![None; next_value as usize],
        value_escapes: vec![EscapeState::LocalOnly; next_value as usize],
        locals_promoted: true,
        live: true,
        span: empty,
    })
}

pub fn superoptimize_pure_expressions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;

    for function in &mut module.functions {
        if has_exception_region(function) {
            continue;
        }
        let value_types = control_flow_value_types(function);
        let mut aliases = AHashMap::<ValueId, ValueId>::default();
        let mut unary_sources = AHashMap::<ValueId, (IrUnaryOp, ValueId)>::default();
        let mut binary_sources = AHashMap::<ValueId, (IrBinaryOp, ValueId, ValueId)>::default();
        let mut constants = AHashMap::<ValueId, ConstValue>::default();

        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let Some(out) = instruction.out else {
                    retained.push(instruction);
                    continue;
                };

                let mut alias = None;
                let mut replacement = None;
                match &instruction.op {
                    ControlFlowOp::Const(value) => {
                        constants.insert(out, value.clone());
                    }
                    ControlFlowOp::Unary {
                        op: IrUnaryOp::Not,
                        value,
                    } => {
                        if let Some((IrUnaryOp::Not, inner)) = unary_sources.get(value).copied() {
                            if matches!(value_types.get(&inner), Some(Type::Bool))
                                || matches!(value_types.get(value), Some(Type::Bool))
                            {
                                alias = Some(inner);
                            }
                        }
                        unary_sources.insert(out, (IrUnaryOp::Not, *value));
                    }
                    ControlFlowOp::Binary { op, lhs, rhs } => {
                        let lhs = resolve_alias(*lhs, &aliases);
                        let rhs = resolve_alias(*rhs, &aliases);
                        let output_type = instruction.ty.as_ref();
                        if output_type == Some(&Type::Int) {
                            match op {
                                IrBinaryOp::BitOr | IrBinaryOp::BitAnd if lhs == rhs => {
                                    alias = Some(lhs);
                                }
                                IrBinaryOp::Xor if lhs == rhs => {
                                    replacement = Some(ConstValue::Int(0));
                                }
                                IrBinaryOp::Sub => {
                                    if let Some((IrBinaryOp::Add, a, b)) =
                                        binary_sources.get(&lhs).copied()
                                    {
                                        if b == rhs {
                                            alias = Some(a);
                                        } else if a == rhs {
                                            alias = Some(b);
                                        }
                                    }
                                }
                                IrBinaryOp::Add => {
                                    if let Some((IrBinaryOp::Sub, a, b)) =
                                        binary_sources.get(&lhs).copied()
                                    {
                                        if b == rhs {
                                            alias = Some(a);
                                        }
                                    }
                                    if alias.is_none() {
                                        if let Some((IrBinaryOp::Add, a, b)) =
                                            binary_sources.get(&lhs).copied()
                                        {
                                            if constants.contains_key(&b)
                                                && constants.contains_key(&rhs)
                                            {
                                                let right = ValueId(function.value_count);
                                                function.value_count += 1;
                                                function.value_escapes.push(EscapeState::LocalOnly);
                                                function.value_local_hints.push(None);
                                                retained.push(ControlFlowInstruction {
                                                    out: Some(right),
                                                    ty: Some(Type::Int),
                                                    op: ControlFlowOp::Binary {
                                                        op: IrBinaryOp::Add,
                                                        lhs: b,
                                                        rhs,
                                                    },
                                                    span: instruction.span,
                                                });
                                                instruction.op = ControlFlowOp::Binary {
                                                    op: IrBinaryOp::Add,
                                                    lhs: a,
                                                    rhs: right,
                                                };
                                                binary_sources
                                                    .insert(out, (IrBinaryOp::Add, a, right));
                                                changed = true;
                                                retained.push(instruction);
                                                continue;
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        binary_sources.insert(out, (*op, lhs, rhs));
                    }
                    _ => {}
                }

                if let Some(alias) = alias {
                    aliases.insert(out, resolve_alias(alias, &aliases));
                    changed = true;
                    continue;
                }
                if let Some(value) = replacement {
                    instruction.op = ControlFlowOp::Const(value.clone());
                    constants.insert(out, value);
                    changed = true;
                }
                retained.push(instruction);
            }
            block.instructions = retained;
        }
        rewrite_control_flow_function(function, &aliases);
    }

    OptimizationReport {
        pass_name: "expression-superoptimization",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Lattice {
    Bottom,
    Constant(ConstValue),
    Top,
}

impl Lattice {
    fn meet(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value,
            (Self::Top, _) | (_, Self::Top) => Self::Top,
            (Self::Constant(left), Self::Constant(right)) if left == right => Self::Constant(left),
            _ => Self::Top,
        }
    }
}

pub fn propagate_path_sensitive_constants(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let mut changed = false;

    for function in &mut module.functions {
        if has_exception_region(function) || function.blocks.is_empty() {
            continue;
        }

        let block_count = function.blocks.len();
        let mut executable = vec![false; block_count];
        let mut values = vec![Lattice::Bottom; function.value_count as usize];
        executable[function.entry.0 as usize] = true;

        let mut progressed = true;
        while progressed {
            progressed = false;
            for block_index in 0..block_count {
                if !executable[block_index] {
                    continue;
                }
                let block = &function.blocks[block_index];
                for phi in &block.phis {
                    let mut lattice = Lattice::Bottom;
                    for (pred, value) in &phi.incoming {
                        if !executable.get(pred.0 as usize).copied().unwrap_or(false) {
                            continue;
                        }
                        lattice = lattice.meet(
                            values
                                .get(value.0 as usize)
                                .cloned()
                                .unwrap_or(Lattice::Top),
                        );
                    }
                    progressed |= set_lattice(&mut values, phi.out, lattice);
                }

                for instruction in &block.instructions {
                    let Some(out) = instruction.out else {
                        continue;
                    };
                    let lattice = match &instruction.op {
                        ControlFlowOp::Const(value) => Lattice::Constant(value.clone()),
                        ControlFlowOp::Unary { op, value } => match values.get(value.0 as usize) {
                            Some(Lattice::Constant(constant)) => fold_unary(*op, constant)
                                .map(Lattice::Constant)
                                .unwrap_or(Lattice::Top),
                            Some(Lattice::Bottom) => Lattice::Bottom,
                            _ => Lattice::Top,
                        },
                        ControlFlowOp::Binary { op, lhs, rhs } => {
                            match (values.get(lhs.0 as usize), values.get(rhs.0 as usize)) {
                                (Some(Lattice::Constant(left)), Some(Lattice::Constant(right))) => {
                                    fold_binary(*op, left, right)
                                        .map(Lattice::Constant)
                                        .unwrap_or(Lattice::Top)
                                }
                                (Some(Lattice::Bottom), _) | (_, Some(Lattice::Bottom)) => {
                                    Lattice::Bottom
                                }
                                _ => Lattice::Top,
                            }
                        }
                        _ => Lattice::Top,
                    };
                    progressed |= set_lattice(&mut values, out, lattice);
                }

                match block.terminator.clone() {
                    Some(Terminator::Jump(target)) => {
                        progressed |= mark_executable(&mut executable, target.0 as usize);
                    }
                    Some(Terminator::Branch {
                        condition,
                        then_block,
                        else_block,
                    }) => match values.get(condition.0 as usize) {
                        Some(Lattice::Constant(ConstValue::Bool(true))) => {
                            progressed |= mark_executable(&mut executable, then_block.0 as usize);
                        }
                        Some(Lattice::Constant(ConstValue::Bool(false))) => {
                            progressed |= mark_executable(&mut executable, else_block.0 as usize);
                        }
                        Some(Lattice::Bottom) => {}
                        _ => {
                            progressed |= mark_executable(&mut executable, then_block.0 as usize);
                            progressed |= mark_executable(&mut executable, else_block.0 as usize);
                        }
                    },
                    Some(Terminator::Try { body, catch_block }) => {
                        progressed |= mark_executable(&mut executable, body.0 as usize);
                        if let Some(catch_block) = catch_block {
                            progressed |= mark_executable(&mut executable, catch_block.0 as usize);
                        }
                    }
                    _ => {}
                }
            }
        }

        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                let Some(out) = instruction.out else {
                    continue;
                };
                let Some(Lattice::Constant(value)) = values.get(out.0 as usize).cloned() else {
                    continue;
                };
                if !matches!(instruction.op, ControlFlowOp::Const(_)) {
                    instruction.op = ControlFlowOp::Const(value);
                    changed = true;
                }
            }
        }
    }

    OptimizationReport {
        pass_name: "path-sensitive-constant-propagation",
        changed,
    }
}

fn set_lattice(values: &mut [Lattice], id: ValueId, lattice: Lattice) -> bool {
    let Some(slot) = values.get_mut(id.0 as usize) else {
        return false;
    };
    let next = slot.clone().meet(lattice);
    if *slot != next {
        *slot = next;
        true
    } else {
        false
    }
}

fn mark_executable(executable: &mut [bool], block: usize) -> bool {
    let Some(slot) = executable.get_mut(block) else {
        return false;
    };
    if *slot {
        false
    } else {
        *slot = true;
        true
    }
}

fn has_exception_region(function: &ControlFlowFunction<'_>) -> bool {
    function
        .shapes
        .iter()
        .any(|shape| matches!(shape, crate::ir::ControlShape::Try { .. }))
}

fn exported_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    module
        .exports
        .iter()
        .chain(
            module
                .lazy_modules
                .iter()
                .flat_map(|module| module.exports.iter()),
        )
        .filter_map(|export| match export.binding {
            ExportBinding::Function(function) => Some(function),
            _ => None,
        })
        .collect()
}

fn instruction_definitions<'a, 'src>(
    function: &'a ControlFlowFunction<'src>,
) -> AHashMap<ValueId, &'a ControlFlowOp<'src>> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect()
}

fn closure_target<'a, 'src>(
    definitions: &AHashMap<ValueId, &'a ControlFlowOp<'src>>,
    value: ValueId,
) -> Option<(FunctionId, &'a [ValueId])> {
    match definitions.get(&value) {
        Some(ControlFlowOp::Closure { function, captures }) => {
            Some((*function, captures.as_slice()))
        }
        _ => None,
    }
}

fn control_flow_use_counts(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, usize> {
    let mut uses = AHashMap::default();
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                *uses.entry(*value).or_insert(0) += 1;
            }
        }
        for instruction in &block.instructions {
            for value in control_flow_used_values(&instruction.op) {
                *uses.entry(value).or_insert(0) += 1;
            }
        }
        if let Some(terminator) = &block.terminator {
            for value in terminator_used_values(terminator) {
                *uses.entry(value).or_insert(0) += 1;
            }
        }
    }
    uses
}

fn control_flow_value_types<'src>(
    function: &ControlFlowFunction<'src>,
) -> AHashMap<ValueId, Type<'src>> {
    let mut types = AHashMap::default();
    for param in &function.params {
        types.insert(param.value, param.ty.clone());
    }
    for block in &function.blocks {
        for phi in &block.phis {
            types.insert(phi.out, phi.ty.clone());
        }
        for instruction in &block.instructions {
            if let (Some(out), Some(ty)) = (instruction.out, &instruction.ty) {
                types.insert(out, ty.clone());
            }
        }
    }
    types
}

fn control_flow_used_values(op: &ControlFlowOp<'_>) -> Vec<ValueId> {
    match op {
        ControlFlowOp::Const(_)
        | ControlFlowOp::CaughtException
        | ControlFlowOp::CaptureLocal(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => Vec::new(),
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => vec![*value],
        ControlFlowOp::Await { task } => vec![*task],
        ControlFlowOp::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
            values.clone()
        }
        ControlFlowOp::ArraySpread(operands) => operands
            .iter()
            .map(|operand| match operand {
                crate::ir::ArrayOperand::Value(value) | crate::ir::ArrayOperand::Spread(value) => {
                    *value
                }
            })
            .collect(),
        ControlFlowOp::Record(entries) => entries.iter().map(|(_, value)| *value).collect(),
        ControlFlowOp::RecordSpread(operands) => operands
            .iter()
            .map(|operand| match operand {
                crate::ir::RecordOperand::Entry(_, value)
                | crate::ir::RecordOperand::Spread(value) => *value,
            })
            .collect(),
        ControlFlowOp::NewClass { args, .. } => args.clone(),
        ControlFlowOp::Closure { captures, .. } => captures.clone(),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            vec![*value]
        }
        ControlFlowOp::FieldGet { object, .. }
        | ControlFlowOp::RecordFieldGet { object, .. }
        | ControlFlowOp::RecordRest { object, .. }
        | ControlFlowOp::HostFieldGet { object, .. } => vec![*object],
        ControlFlowOp::FieldSet { object, value, .. }
        | ControlFlowOp::RecordFieldSet { object, value, .. }
        | ControlFlowOp::HostFieldSet { object, value, .. } => vec![*object, *value],
        ControlFlowOp::IndexGet { object, index } => vec![*object, *index],
        ControlFlowOp::ArrayGetOptional { object, .. } => vec![*object],
        ControlFlowOp::IndexSet {
            object,
            index,
            value,
        } => vec![*object, *index, *value],
        ControlFlowOp::CallDirect { args, .. } => args.clone(),
        ControlFlowOp::CallValue { callee, args } => {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*callee);
            values.extend(args);
            values
        }
        ControlFlowOp::CallMethod { receiver, args, .. }
        | ControlFlowOp::HostCall { receiver, args, .. } => {
            let mut values = Vec::with_capacity(args.len() + 1);
            values.push(*receiver);
            values.extend(args);
            values
        }
        ControlFlowOp::Intrinsic { receiver, args, .. } => {
            let mut values = Vec::with_capacity(args.len() + usize::from(receiver.is_some()));
            values.extend(receiver);
            values.extend(args);
            values
        }
        ControlFlowOp::Template(parts) => parts
            .iter()
            .filter_map(|part| match part {
                crate::ir::TemplateOperand::Value(value) => Some(*value),
                crate::ir::TemplateOperand::String(_) => None,
            })
            .collect(),
    }
}

fn terminator_used_values(terminator: &Terminator) -> Vec<ValueId> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![*condition],
        Terminator::Return(Some(value)) | Terminator::Throw(value) => vec![*value],
        _ => Vec::new(),
    }
}

fn resolve_alias(mut value: ValueId, aliases: &AHashMap<ValueId, ValueId>) -> ValueId {
    while let Some(next) = aliases.get(&value) {
        if *next == value {
            break;
        }
        value = *next;
    }
    value
}

fn rewrite_control_flow_function(
    function: &mut ControlFlowFunction<'_>,
    aliases: &AHashMap<ValueId, ValueId>,
) {
    for block in &mut function.blocks {
        for phi in &mut block.phis {
            for (_, value) in &mut phi.incoming {
                *value = resolve_alias(*value, aliases);
            }
            match &mut phi.origin {
                crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::ShortCircuit {
                    lhs,
                    ..
                })
                | crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Nullish { lhs }) => {
                    *lhs = resolve_alias(*lhs, aliases);
                }
                crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::OptionalAccess {
                    object,
                }) => {
                    *object = resolve_alias(*object, aliases);
                }
                crate::ir::PhiOrigin::Local(_)
                | crate::ir::PhiOrigin::Expression(crate::ir::ExpressionPhi::Conditional)
                | crate::ir::PhiOrigin::Synthetic => {}
            }
        }
        for instruction in &mut block.instructions {
            rewrite_control_flow_op(&mut instruction.op, aliases);
        }
        if let Some(terminator) = &mut block.terminator {
            rewrite_terminator(terminator, aliases);
        }
    }
}

fn rewrite_control_flow_op(op: &mut ControlFlowOp<'_>, aliases: &AHashMap<ValueId, ValueId>) {
    rewrite_control_flow_values(op, |value| *value = resolve_alias(*value, aliases));
}

fn rewrite_control_flow_op_once(op: &mut ControlFlowOp<'_>, mapping: &AHashMap<ValueId, ValueId>) {
    rewrite_control_flow_values(op, |value| {
        if let Some(mapped) = mapping.get(value) {
            *value = *mapped;
        }
    });
}

fn rewrite_control_flow_values(op: &mut ControlFlowOp<'_>, mut rewrite: impl FnMut(&mut ValueId)) {
    match op {
        ControlFlowOp::Const(_)
        | ControlFlowOp::CaughtException
        | ControlFlowOp::CaptureLocal(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => {}
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => {
            rewrite(value)
        }
        ControlFlowOp::Await { task } => rewrite(task),
        ControlFlowOp::Binary { lhs, rhs, .. } => {
            rewrite(lhs);
            rewrite(rhs);
        }
        ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
            values.iter_mut().for_each(&mut rewrite)
        }
        ControlFlowOp::ArraySpread(operands) => {
            operands.iter_mut().for_each(|operand| match operand {
                crate::ir::ArrayOperand::Value(value) | crate::ir::ArrayOperand::Spread(value) => {
                    rewrite(value)
                }
            })
        }
        ControlFlowOp::Record(entries) => entries.iter_mut().for_each(|(_, value)| rewrite(value)),
        ControlFlowOp::RecordSpread(operands) => {
            operands.iter_mut().for_each(|operand| match operand {
                crate::ir::RecordOperand::Entry(_, value)
                | crate::ir::RecordOperand::Spread(value) => rewrite(value),
            })
        }
        ControlFlowOp::NewClass { args, .. } => args.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::Closure { captures, .. } => captures.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            rewrite(value)
        }
        ControlFlowOp::FieldGet { object, .. }
        | ControlFlowOp::RecordFieldGet { object, .. }
        | ControlFlowOp::RecordRest { object, .. }
        | ControlFlowOp::HostFieldGet { object, .. } => rewrite(object),
        ControlFlowOp::FieldSet { object, value, .. }
        | ControlFlowOp::RecordFieldSet { object, value, .. }
        | ControlFlowOp::HostFieldSet { object, value, .. } => {
            rewrite(object);
            rewrite(value);
        }
        ControlFlowOp::IndexGet { object, index } => {
            rewrite(object);
            rewrite(index);
        }
        ControlFlowOp::ArrayGetOptional { object, .. } => rewrite(object),
        ControlFlowOp::IndexSet {
            object,
            index,
            value,
        } => {
            rewrite(object);
            rewrite(index);
            rewrite(value);
        }
        ControlFlowOp::CallDirect { args, .. } => args.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::CallMethod { receiver, args, .. }
        | ControlFlowOp::HostCall { receiver, args, .. } => {
            rewrite(receiver);
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::CallValue { callee, args } => {
            rewrite(callee);
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::Intrinsic { receiver, args, .. } => {
            if let Some(receiver) = receiver {
                rewrite(receiver);
            }
            args.iter_mut().for_each(&mut rewrite);
        }
        ControlFlowOp::Template(parts) => {
            for part in parts {
                if let crate::ir::TemplateOperand::Value(value) = part {
                    rewrite(value);
                }
            }
        }
    }
}

fn rewrite_terminator(terminator: &mut Terminator, aliases: &AHashMap<ValueId, ValueId>) {
    match terminator {
        Terminator::Branch { condition, .. } => {
            *condition = resolve_alias(*condition, aliases);
        }
        Terminator::Return(Some(value)) | Terminator::Throw(value) => {
            *value = resolve_alias(*value, aliases);
        }
        _ => {}
    }
}

fn hash_const(value: &ConstValue, hasher: &mut DefaultHasher) {
    match value {
        ConstValue::Int(value) => {
            0u8.hash(hasher);
            value.hash(hasher);
        }
        ConstValue::Float(value) => {
            1u8.hash(hasher);
            value.to_bits().hash(hasher);
        }
        ConstValue::Bool(value) => {
            2u8.hash(hasher);
            value.hash(hasher);
        }
        ConstValue::String(value) => {
            3u8.hash(hasher);
            value.hash(hasher);
        }
        ConstValue::Null => 4u8.hash(hasher),
    }
}

fn push_unique(values: &mut Vec<ValueId>, value: ValueId) {
    if !values.contains(&value) {
        values.push(value);
    }
}

fn fold_unary(op: IrUnaryOp, value: &ConstValue) -> Option<ConstValue> {
    match (op, value) {
        (IrUnaryOp::Neg, ConstValue::Int(value)) => {
            Some(ConstValue::Int(i64::from((*value as i32).wrapping_neg())))
        }
        (IrUnaryOp::Neg, ConstValue::Float(value)) => Some(ConstValue::Float(-value)),
        (IrUnaryOp::Not, ConstValue::Bool(value)) => Some(ConstValue::Bool(!value)),
        _ => None,
    }
}

fn fold_binary(op: IrBinaryOp, lhs: &ConstValue, rhs: &ConstValue) -> Option<ConstValue> {
    use ConstValue::{Bool, Float, Int, String};
    use IrBinaryOp::{
        Add, BitAnd, BitOr, Div, Eq, Greater, GreaterEq, Less, LessEq, Mod, Mul, NotEq, ShiftLeft,
        ShiftRight, Sub, UnsignedShiftRight, Xor,
    };

    match (op, lhs, rhs) {
        (Add, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_add(*rhs as i32)))),
        (Sub, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_sub(*rhs as i32)))),
        (Mul, Int(lhs), Int(rhs)) => {
            let product = f64::from(*lhs as i32) * f64::from(*rhs as i32);
            Some(Int(i64::from((product as i64 as u32) as i32)))
        }
        (Div, Int(_), Int(0)) | (Mod, Int(_), Int(0)) => None,
        (Div, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_div(*rhs as i32)))),
        (Mod, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_rem(*rhs as i32)))),
        (BitAnd, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32) & (*rhs as i32)))),
        (BitOr, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32) | (*rhs as i32)))),
        (Xor, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32) ^ (*rhs as i32)))),
        (ShiftLeft, Int(lhs), Int(rhs)) => Some(Int(i64::from(
            (*lhs as i32).wrapping_shl((*rhs as u32) & 31),
        ))),
        (ShiftRight, Int(lhs), Int(rhs)) => {
            Some(Int(i64::from((*lhs as i32) >> ((*rhs as u32) & 31))))
        }
        (UnsignedShiftRight, Int(lhs), Int(rhs)) => Some(Int(i64::from(
            (((*lhs as i32) as u32) >> ((*rhs as u32) & 31)) as i32,
        ))),
        (Add, Float(lhs), Float(rhs)) => Some(Float(lhs + rhs)),
        (Sub, Float(lhs), Float(rhs)) => Some(Float(lhs - rhs)),
        (Mul, Float(lhs), Float(rhs)) => Some(Float(lhs * rhs)),
        (Div, Float(_), Float(rhs)) if *rhs == 0.0 => None,
        (Div, Float(lhs), Float(rhs)) => Some(Float(lhs / rhs)),
        (Add, String(lhs), String(rhs)) => Some(String(format!("{lhs}{rhs}"))),
        (Eq, lhs, rhs) => Some(Bool(lhs == rhs)),
        (NotEq, lhs, rhs) => Some(Bool(lhs != rhs)),
        (Less, Int(lhs), Int(rhs)) => Some(Bool(lhs < rhs)),
        (LessEq, Int(lhs), Int(rhs)) => Some(Bool(lhs <= rhs)),
        (Greater, Int(lhs), Int(rhs)) => Some(Bool(lhs > rhs)),
        (GreaterEq, Int(lhs), Int(rhs)) => Some(Bool(lhs >= rhs)),
        (IrBinaryOp::And, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs && *rhs)),
        (IrBinaryOp::Or, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs || *rhs)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::optimizer::promote_locals_to_ssa;
    use crate::{analyze, lower_to_control_flow, parse_source};

    #[test]
    fn fuses_map_map_pipelines_into_one_callback() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
int[] values = [1, 2, 3];
auto mapped = values.map((int value) => value + 1).map((int value) => value * 2);
print(mapped[0]);
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let before = module
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .flat_map(|block| block.instructions.iter())
            .filter(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayMap,
                        ..
                    }
                )
            })
            .count();
        let report = fuse_array_pipelines(&mut module);
        assert!(report.changed);
        assert_eq!(report.pass_name, "array-pipeline-fusion");
        let after = module
            .functions
            .iter()
            .flat_map(|function| function.blocks.iter())
            .flat_map(|block| block.instructions.iter())
            .filter(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayMap,
                        ..
                    }
                )
            })
            .count();
        assert!(after < before, "expected fewer ArrayMap intrinsics");
        assert!(module.functions.iter().any(|function| {
            function.kind == FunctionKind::Closure
                && function.blocks.iter().any(|block| {
                    block
                        .instructions
                        .iter()
                        .filter(|instruction| {
                            matches!(instruction.op, ControlFlowOp::CallDirect { .. })
                        })
                        .count()
                        >= 2
                })
        }));
    }

    #[test]
    fn sinks_local_array_into_single_branch() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
extern bool flag();
int use() {
  int[] xs = [1, 2, 3];
  if (flag()) {
    return xs[0];
  }
  return 0;
}
print(use());
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let report = sink_partial_escape_allocations(&mut module);
        assert!(report.changed);
        assert_eq!(report.pass_name, "partial-escape-allocation-sinking");
        let use_fn = module
            .functions
            .iter()
            .find(|function| function.name == Some("use"))
            .unwrap();
        let header = &use_fn.blocks[0];
        assert!(header
            .instructions
            .iter()
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::Array(_))));
        let then_block = &use_fn.blocks[1];
        assert!(matches!(
            then_block
                .instructions
                .first()
                .map(|instruction| &instruction.op),
            Some(ControlFlowOp::Array(_))
        ));
    }

    #[test]
    fn superoptimizes_int_identities_and_double_not() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
int work(int x, int y) {
  int a = (x + y) - y;
  int b = x | x;
  int c = x ^ x;
  bool flag = !(!true);
  int extra = 0;
  if (flag) {
    extra = 1;
  }
  return a + b + c + extra;
}
print(work(3, 4));
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let report = superoptimize_pure_expressions(&mut module);
        assert_eq!(report.pass_name, "expression-superoptimization");
        let work = module
            .functions
            .iter()
            .find(|function| function.name == Some("work"))
            .unwrap();
        let has_xor_zero = work
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Const(ConstValue::Int(0))));
        assert!(has_xor_zero || report.changed);
    }

    #[test]
    fn propagates_constants_along_known_branch() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
int work() {
  int value = 2;
  if (true) {
    return value + 3;
  }
  return 0;
}
print(work());
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let report = propagate_path_sensitive_constants(&mut module);
        assert_eq!(report.pass_name, "path-sensitive-constant-propagation");
        assert!(
            report.changed
                || module.functions.iter().any(|function| {
                    function.blocks.iter().any(|block| {
                        block.instructions.iter().any(|instruction| {
                            matches!(instruction.op, ControlFlowOp::Const(ConstValue::Int(5)))
                        })
                    })
                })
        );
    }

    #[test]
    fn run_compress_passes_respects_options_order() {
        let arena = Bump::new();
        let program = parse_source(&arena, "print(1 + 2);").unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let reports = run_compress_passes(
            &mut module,
            &CompressPassOptions {
                pipeline_fusion: false,
                partial_escape_sinking: false,
                region_outlining: false,
                expression_superopt: true,
                path_sensitive_propagation: true,
            },
        );
        assert_eq!(
            reports
                .iter()
                .map(|report| report.pass_name)
                .collect::<Vec<_>>(),
            vec![
                "path-sensitive-constant-propagation",
                "expression-superoptimization"
            ]
        );
    }

    #[test]
    fn outlining_tracks_repeated_regions_from_the_closed_script_entry() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
extern int input(int index);
print(((input(0) + 1) * 3 - 2) ^ 7);
print(((input(1) + 1) * 3 - 2) ^ 7);
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();
        let original_functions = module.functions.len();

        let (reports, outlined) = run_compress_passes_tracking_outlined_helpers(
            &mut module,
            &CompressPassOptions {
                pipeline_fusion: false,
                partial_escape_sinking: false,
                region_outlining: true,
                expression_superopt: false,
                path_sensitive_propagation: false,
            },
        );

        assert!(
            reports.iter().any(|report| {
                report.pass_name == "repeated-region-outlining" && report.changed
            }),
            "{:#?}",
            module.functions[module.entry.0 as usize]
        );
        assert!(!outlined.is_empty());
        assert!(outlined
            .iter()
            .all(|function| function.0 as usize >= original_functions));
        assert!(outlined.iter().all(|function| {
            module.functions[function.0 as usize].origin == FunctionOrigin::RepeatedRegionOutline
        }));
        let entry = &module.functions[module.entry.0 as usize];
        let outlined_calls = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::CallDirect { function, .. }
                        if outlined.contains(&function)
                )
            })
            .count();
        assert!(
            outlined_calls >= 2,
            "both repeated entry regions must call the tracked helper"
        );
    }

    #[test]
    fn outlining_declines_modules_with_lazy_chunk_ownership() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
pure int publicScore(int first, int second) {
  int a = first + 1;
  int b = a * 3;
  int c = b - 2;
  int d = c ^ 7;
  int e = second + 1;
  int f = e * 3;
  int g = f - 2;
  int h = g ^ 7;
  return d + h;
}
print(publicScore(1, 2));
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();
        let public_score = module
            .functions
            .iter()
            .find(|function| function.name == Some("publicScore"))
            .expect("public helper")
            .id;
        module.lazy_modules.push(crate::ir::IrLazyModule {
            id: 0,
            source: "lazy.lil",
            exports: vec![crate::ir::IrExport {
                name: "publicScore",
                binding: ExportBinding::Function(public_score),
                span: Span::empty(0),
            }],
            span: Span::empty(0),
        });
        let original_functions = module.functions.len();

        let report = outline_repeated_regions(&mut module);

        assert!(!report.changed);
        assert_eq!(module.functions.len(), original_functions);
    }

    #[test]
    fn outlining_separates_typed_strings_from_dynamic_coercion_regions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
pure string typed_a(string left, string right) {
  string a = "" + left;
  string b = "" + right;
  return a + b;
}
pure string typed_b(string left, string right) {
  string a = "" + left;
  string b = "" + right;
  return a + b;
}
string dynamic_a(JsValue left, JsValue right) {
  string a = "" + left;
  string b = "" + right;
  return a + b;
}
string dynamic_b(JsValue left, JsValue right) {
  string a = "" + left;
  string b = "" + right;
  return a + b;
}
extern string text();
extern JsValue any();
print(typed_a(text(), text()));
print(typed_b(text(), text()));
print(dynamic_a(any(), any()));
print(dynamic_b(any(), any()));
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let typed = module
            .functions
            .iter()
            .find(|function| function.name == Some("typed_a"))
            .unwrap();
        let dynamic = module
            .functions
            .iter()
            .find(|function| function.name == Some("dynamic_a"))
            .unwrap();
        let typed_region = &typed.blocks[0].instructions;
        let dynamic_region = &dynamic.blocks[0].instructions;
        assert!(typed_region.len() >= 4);
        assert_eq!(typed_region.len(), dynamic_region.len());
        let typed_types = control_flow_value_types(typed);
        let dynamic_types = control_flow_value_types(dynamic);
        let (typed_hash, typed_ins, typed_outs) =
            fingerprint_region(typed_region, typed, 0, 0, &typed_types).unwrap();
        let (dynamic_hash, dynamic_ins, dynamic_outs) =
            fingerprint_region(dynamic_region, dynamic, 0, 0, &dynamic_types).unwrap();
        assert_ne!(
            typed_hash, dynamic_hash,
            "live-in types must affect the hash"
        );
        let typed_occurrence = RegionOccurrence {
            function: typed.id,
            block: BlockId(0),
            start: 0,
            window: typed_region.len(),
            live_ins: typed_ins,
            live_outs: typed_outs,
        };
        let dynamic_occurrence = RegionOccurrence {
            function: dynamic.id,
            block: BlockId(0),
            start: 0,
            window: dynamic_region.len(),
            live_ins: dynamic_ins,
            live_outs: dynamic_outs,
        };
        assert!(!regions_are_exactly_equivalent(
            &module,
            &typed_occurrence,
            &dynamic_occurrence
        ));

        let report = outline_repeated_regions(&mut module);
        assert!(report.changed, "the two typed regions should still outline");
        for name in ["dynamic_a", "dynamic_b"] {
            let function = module
                .functions
                .iter()
                .find(|function| function.name == Some(name))
                .unwrap();
            assert!(
                function
                    .blocks
                    .iter()
                    .flat_map(|block| &block.instructions)
                    .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallDirect { .. })),
                "dynamic coercion region `{name}` must remain in place"
            );
            assert!(function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| matches!(
                    instruction.op,
                    ControlFlowOp::Binary {
                        op: IrBinaryOp::Add,
                        ..
                    }
                )));
        }
    }

    #[test]
    fn outlining_preserves_non_int_phi_live_in_types() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
pure string phi_a(bool first, string left, string right) {
  string selected = left;
  if (!first) { selected = right; }
  string a = selected + "a";
  string b = a + "b";
  string c = b + "c";
  return c + "d";
}
pure string phi_b(bool first, string left, string right) {
  string selected = left;
  if (!first) { selected = right; }
  string a = selected + "a";
  string b = a + "b";
  string c = b + "c";
  return c + "d";
}
extern bool choose();
extern string text();
print(phi_a(choose(), text(), text()));
print(phi_b(choose(), text(), text()));
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();
        let original_functions = module.functions.len();

        let report = outline_repeated_regions(&mut module);
        assert!(report.changed);
        let helper_ids = module
            .functions
            .iter()
            .filter(|function| matches!(function.name, Some("phi_a") | Some("phi_b")))
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match instruction.op {
                ControlFlowOp::CallDirect { function, .. }
                    if function.0 as usize >= original_functions =>
                {
                    Some(function)
                }
                _ => None,
            })
            .collect::<AHashSet<_>>();
        assert!(
            !helper_ids.is_empty(),
            "expected the merge-tail region to outline"
        );
        assert!(helper_ids.iter().any(|helper| {
            let helper = &module.functions[helper.0 as usize];
            !helper.params.is_empty()
                && helper
                    .params
                    .iter()
                    .all(|parameter| parameter.ty == Type::String)
        }));
    }

    #[test]
    fn outlining_resolves_pending_cross_block_dependencies() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
pure string route_a(bool take, string input) {
  string a = input + "a";
  string b = a + "b";
  string c = b + "c";
  string produced = c + "d";
  if (take) {
    string e = produced + "e";
    string f = e + "f";
    string g = f + "g";
    return g + "h";
  }
  return produced;
}
pure string route_b(bool take, string input) {
  string a = input + "a";
  string b = a + "b";
  string c = b + "c";
  string produced = c + "d";
  if (take) {
    string e = produced + "e";
    string f = e + "f";
    string g = f + "g";
    return g + "h";
  }
  return produced;
}
extern bool choose();
extern string text();
print(route_a(choose(), text()));
print(route_b(choose(), text()));
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let report = outline_repeated_regions(&mut module);
        assert!(report.changed);
        for name in ["route_a", "route_b"] {
            let function = module
                .functions
                .iter()
                .find(|function| function.name == Some(name))
                .unwrap();
            let outlined_calls = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. }))
                .count();
            assert!(
                outlined_calls >= 2,
                "expected both dependent regions in `{name}`"
            );

            let definitions = function
                .params
                .iter()
                .map(|parameter| parameter.value)
                .chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.phis)
                        .map(|phi| phi.out),
                )
                .chain(
                    function
                        .blocks
                        .iter()
                        .flat_map(|block| &block.instructions)
                        .filter_map(|instruction| instruction.out),
                )
                .collect::<AHashSet<_>>();
            for value in function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .flat_map(|instruction| control_flow_used_values(&instruction.op))
            {
                assert!(
                    definitions.contains(&value),
                    "outlined `{name}` reintroduced dangling value {value:?}"
                );
            }
        }
    }

    #[test]
    fn outlines_repeated_index_set_store_regions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            r#"
void store_a(Float64Array buf, float x, float y) {
  buf[0] = x;
  buf[1] = y;
  buf[2] = x;
  buf[3] = y;
}
void store_b(Float64Array buf, float x, float y) {
  buf[0] = x;
  buf[1] = y;
  buf[2] = x;
  buf[3] = y;
}
void store_c(Float64Array buf, float x, float y) {
  buf[0] = x;
  buf[1] = y;
  buf[2] = x;
  buf[3] = y;
}
Float64Array values = new Float64Array(4);
store_a(values, 1.0, 2.0);
store_b(values, 3.0, 4.0);
store_c(values, 5.0, 6.0);
print(values[0]);
"#,
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        promote_locals_to_ssa(&mut module).unwrap();

        let before_helpers = module.functions.len();
        let report = outline_repeated_regions(&mut module);
        assert!(report.changed, "expected IndexSet store regions to outline");
        assert_eq!(report.pass_name, "repeated-region-outlining");
        assert!(
            module.functions.len() > before_helpers,
            "expected an outlined helper function"
        );
        let store_calls = module
            .functions
            .iter()
            .filter(|function| {
                matches!(
                    function.name,
                    Some("store_a") | Some("store_b") | Some("store_c")
                )
            })
            .flat_map(|function| function.blocks.iter())
            .flat_map(|block| block.instructions.iter())
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. }))
            .count();
        assert!(
            store_calls >= 3,
            "expected each store site to call the outlined helper, got {store_calls}"
        );
        assert!(module.functions.iter().any(|function| {
            function.name.is_none()
                && !function.declared_pure
                && function.blocks.iter().any(|block| {
                    block
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction.op, ControlFlowOp::IndexSet { .. }))
                })
        }));
    }
}
