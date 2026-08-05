use ahash::{AHashMap, AHashSet};

use crate::ir::{
    BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction, ControlFlowModule,
    ControlFlowOp, FunctionId, FunctionKind, Instruction, Intrinsic, IrBinaryOp, IrLocal, IrModule,
    IrUnaryOp, LocalId, Phi, TemplateOperand, Terminator, ValueId,
};
use crate::semantic::{EscapeState, Type};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationReport {
    pub pass_name: &'static str,
    pub changed: bool,
}

pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, module: &mut IrModule) -> OptimizationReport;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaError {
    pub message: String,
}

impl std::fmt::Display for SsaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for SsaError {}

pub fn promote_locals_to_ssa(
    module: &mut ControlFlowModule<'_>,
) -> Result<OptimizationReport, SsaError> {
    let mut changed = false;
    for function in &mut module.functions {
        if function.locals_promoted {
            continue;
        }
        promote_function_locals(function)?;
        function.locals_promoted = true;
        changed = true;
    }
    Ok(OptimizationReport {
        pass_name: "mem2reg",
        changed,
    })
}

pub fn optimize_control_flow(
    module: &mut ControlFlowModule<'_>,
) -> Result<Vec<OptimizationReport>, SsaError> {
    let mut reports = vec![
        internalize_entry_globals(module),
        eliminate_unread_globals(module),
        promote_locals_to_ssa(module)?,
    ];

    loop {
        let propagation = fold_and_propagate_control_flow(module);
        let phis = eliminate_redundant_phis(module);
        let algebraic = simplify_algebraic_expressions(module);
        let value_numbering = eliminate_common_subexpressions(module);
        let unreachable = remove_unreachable_control_flow(module);
        let changed = propagation.changed
            || phis.changed
            || algebraic.changed
            || value_numbering.changed
            || unreachable.changed;
        reports.push(propagation);
        reports.push(phis);
        reports.push(algebraic);
        reports.push(value_numbering);
        reports.push(unreachable);
        if !changed {
            break;
        }
    }

    reports.push(propagate_single_assignment_globals(module));
    reports.push(devirtualize_methods(module));
    loop {
        let inlining = inline_small_functions(module);
        let cfg_inlining = inline_single_use_control_flow_function(module);
        let changed = inlining.changed || cfg_inlining.changed;
        reports.push(inlining);
        reports.push(cfg_inlining);
        if !changed {
            break;
        }
    }

    loop {
        let propagation = fold_and_propagate_control_flow(module);
        let phis = eliminate_redundant_phis(module);
        let algebraic = simplify_algebraic_expressions(module);
        let value_numbering = eliminate_common_subexpressions(module);
        let unreachable = remove_unreachable_control_flow(module);
        let changed = propagation.changed
            || phis.changed
            || algebraic.changed
            || value_numbering.changed
            || unreachable.changed;
        reports.push(propagation);
        reports.push(phis);
        reports.push(algebraic);
        reports.push(value_numbering);
        reports.push(unreachable);
        if !changed {
            break;
        }
    }

    reports.push(analyze_escapes(module));
    reports.push(scalar_replace_linear_classes(module));
    reports.push(scalar_replace_control_flow_aggregates(module));
    reports.push(eliminate_overwritten_field_stores(module));

    loop {
        let propagation = fold_and_propagate_control_flow(module);
        let phis = eliminate_redundant_phis(module);
        let algebraic = simplify_algebraic_expressions(module);
        let value_numbering = eliminate_common_subexpressions(module);
        let unreachable = remove_unreachable_control_flow(module);
        let changed = propagation.changed
            || phis.changed
            || algebraic.changed
            || value_numbering.changed
            || unreachable.changed;
        reports.push(propagation);
        reports.push(phis);
        reports.push(algebraic);
        reports.push(value_numbering);
        reports.push(unreachable);
        if !changed {
            break;
        }
    }

    reports.push(eliminate_dead_control_flow_instructions(module));
    reports.push(eliminate_dead_functions(module));
    Ok(reports)
}

fn eliminate_overwritten_field_stores(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            let mut needed = AHashSet::<(ValueId, usize)>::new();
            let mut later_stores = AHashSet::<(ValueId, usize)>::new();
            for instruction in instructions.into_iter().rev() {
                match instruction.op {
                    ControlFlowOp::FieldGet { object, index, .. } => {
                        needed.insert((object, index));
                        retained.push(instruction);
                    }
                    ControlFlowOp::FieldSet { object, index, .. } => {
                        let field = (object, index);
                        if later_stores.contains(&field) && !needed.contains(&field) {
                            changed = true;
                        } else {
                            retained.push(instruction);
                        }
                        needed.remove(&field);
                        later_stores.insert(field);
                    }
                    _ if field_store_barrier(&instruction.op) => {
                        needed.clear();
                        later_stores.clear();
                        retained.push(instruction);
                    }
                    _ => retained.push(instruction),
                }
            }
            retained.reverse();
            block.instructions = retained;
        }
    }
    OptimizationReport {
        pass_name: "dead-field-store-elimination",
        changed,
    }
}

fn field_store_barrier(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::CallDirect { .. }
            | ControlFlowOp::CallValue { .. }
            | ControlFlowOp::CallMethod { .. }
            | ControlFlowOp::Intrinsic { .. }
            | ControlFlowOp::Closure { .. }
    )
}

fn eliminate_unread_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let loaded = module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::LoadGlobal(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let unread = module
        .globals
        .iter()
        .filter(|global| !loaded.contains(&global.symbol))
        .map(|global| global.symbol)
        .collect::<AHashSet<_>>();
    if unread.is_empty() {
        return OptimizationReport {
            pass_name: "unused-global-elimination",
            changed: false,
        };
    }
    for function in &mut module.functions {
        for block in &mut function.blocks {
            block.instructions.retain(|instruction| {
                !matches!(
                    instruction.op,
                    ControlFlowOp::StoreGlobal { global, .. } if unread.contains(&global)
                )
            });
        }
    }
    module
        .globals
        .retain(|global| !unread.contains(&global.symbol));
    OptimizationReport {
        pass_name: "unused-global-elimination",
        changed: true,
    }
}

fn propagate_single_assignment_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut stores = AHashMap::<crate::semantic::SymbolId, Vec<Option<ConstValue>>>::new();
    for function in &module.functions {
        let constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((*out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if let ControlFlowOp::StoreGlobal { global, value } = instruction.op {
                stores
                    .entry(global)
                    .or_default()
                    .push(constants.get(&value).cloned());
            }
        }
    }

    let propagated = stores
        .into_iter()
        .filter_map(|(symbol, values)| match values.as_slice() {
            [Some(value)] => Some((symbol, value.clone())),
            _ => None,
        })
        .collect::<AHashMap<_, _>>();
    if propagated.is_empty() {
        return OptimizationReport {
            pass_name: "global-constant-propagation",
            changed: false,
        };
    }

    for function in &mut module.functions {
        for block in &mut function.blocks {
            block.instructions.retain_mut(|instruction| {
                if let ControlFlowOp::LoadGlobal(symbol) = instruction.op {
                    if let Some(value) = propagated.get(&symbol) {
                        instruction.op = ControlFlowOp::Const(value.clone());
                    }
                    return true;
                }
                !matches!(
                    instruction.op,
                    ControlFlowOp::StoreGlobal { global, .. }
                        if propagated.contains_key(&global)
                )
            });
        }
    }
    module
        .globals
        .retain(|global| !propagated.contains_key(&global.symbol));
    OptimizationReport {
        pass_name: "global-constant-propagation",
        changed: true,
    }
}

fn eliminate_redundant_phis(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let mut aliases = AHashMap::<ValueId, ValueId>::new();
        loop {
            let mut local_change = false;
            for block in &mut function.blocks {
                for phi in &mut block.phis {
                    for (_, value) in &mut phi.incoming {
                        *value = resolve_alias(*value, &aliases);
                    }
                }
                block.phis.retain(|phi| {
                    let Some((_, first)) = phi.incoming.first() else {
                        return true;
                    };
                    if *first != phi.out
                        && phi.incoming.iter().all(|(_, incoming)| incoming == first)
                    {
                        aliases.insert(phi.out, *first);
                        local_change = true;
                        false
                    } else {
                        true
                    }
                });
            }
            changed |= local_change;
            if !local_change {
                break;
            }
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "trivial-phi-elimination",
        changed,
    }
}

fn simplify_algebraic_expressions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let value_types = control_flow_value_types(function);
        let mut constants = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (&instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((*out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        let mut aliases = AHashMap::<ValueId, ValueId>::new();

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
                match instruction.op {
                    ControlFlowOp::Unary {
                        op: IrUnaryOp::Not,
                        value,
                    } => {
                        if let Some(ConstValue::Bool(value)) = constants.get(&value) {
                            replacement = Some(ConstValue::Bool(!value));
                        }
                    }
                    ControlFlowOp::Binary { op, lhs, rhs } => {
                        let lhs_const = constants.get(&lhs);
                        let rhs_const = constants.get(&rhs);
                        let output_type = instruction.ty.as_ref();
                        if output_type == Some(&Type::Int) {
                            alias = match op {
                                IrBinaryOp::Add if is_int(rhs_const, 0) => Some(lhs),
                                IrBinaryOp::Add if is_int(lhs_const, 0) => Some(rhs),
                                IrBinaryOp::Sub if is_int(rhs_const, 0) => Some(lhs),
                                IrBinaryOp::Mul if is_int(rhs_const, 1) => Some(lhs),
                                IrBinaryOp::Mul if is_int(lhs_const, 1) => Some(rhs),
                                IrBinaryOp::Mul if is_int(rhs_const, 0) => Some(rhs),
                                IrBinaryOp::Mul if is_int(lhs_const, 0) => Some(lhs),
                                IrBinaryOp::Div if is_int(rhs_const, 1) => Some(lhs),
                                _ => None,
                            };
                            if replacement.is_none()
                                && lhs == rhs
                                && matches!(op, IrBinaryOp::Sub | IrBinaryOp::Mod)
                            {
                                replacement = Some(ConstValue::Int(0));
                            }
                        } else if output_type == Some(&Type::Float) {
                            alias = match op {
                                IrBinaryOp::Sub if is_float(rhs_const, 0.0) => Some(lhs),
                                IrBinaryOp::Mul if is_float(rhs_const, 1.0) => Some(lhs),
                                IrBinaryOp::Mul if is_float(lhs_const, 1.0) => Some(rhs),
                                IrBinaryOp::Div if is_float(rhs_const, 1.0) => Some(lhs),
                                _ => None,
                            };
                        } else if output_type == Some(&Type::Bool) {
                            alias = match op {
                                IrBinaryOp::Eq if is_bool(rhs_const, true) => Some(lhs),
                                IrBinaryOp::Eq if is_bool(lhs_const, true) => Some(rhs),
                                IrBinaryOp::NotEq if is_bool(rhs_const, false) => Some(lhs),
                                IrBinaryOp::NotEq if is_bool(lhs_const, false) => Some(rhs),
                                _ => None,
                            };
                            if alias.is_none() {
                                let negated = match op {
                                    IrBinaryOp::Eq if is_bool(rhs_const, false) => Some(lhs),
                                    IrBinaryOp::Eq if is_bool(lhs_const, false) => Some(rhs),
                                    IrBinaryOp::NotEq if is_bool(rhs_const, true) => Some(lhs),
                                    IrBinaryOp::NotEq if is_bool(lhs_const, true) => Some(rhs),
                                    _ => None,
                                };
                                if let Some(value) = negated {
                                    instruction.op = ControlFlowOp::Unary {
                                        op: IrUnaryOp::Not,
                                        value,
                                    };
                                    changed = true;
                                } else if lhs == rhs
                                    && matches!(
                                        value_types.get(&lhs),
                                        Some(
                                            Type::Int | Type::Bool | Type::String | Type::Class(_)
                                        )
                                    )
                                {
                                    replacement = match op {
                                        IrBinaryOp::Eq
                                        | IrBinaryOp::LessEq
                                        | IrBinaryOp::GreaterEq => Some(ConstValue::Bool(true)),
                                        IrBinaryOp::NotEq
                                        | IrBinaryOp::Less
                                        | IrBinaryOp::Greater => Some(ConstValue::Bool(false)),
                                        _ => None,
                                    };
                                }
                            }
                        }
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
        pass_name: "algebraic-simplification",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ValueNumberKey {
    Constant(ConstantNumber),
    Unary(IrUnaryOp, ValueId),
    Binary(IrBinaryOp, ValueId, ValueId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum ConstantNumber {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(String),
}

impl From<&ConstValue> for ConstantNumber {
    fn from(value: &ConstValue) -> Self {
        match value {
            ConstValue::Int(value) => Self::Int(*value),
            ConstValue::Float(value) => Self::Float(value.to_bits()),
            ConstValue::Bool(value) => Self::Bool(*value),
            ConstValue::String(value) => Self::String(value.clone()),
        }
    }
}

fn eliminate_common_subexpressions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let mut aliases = AHashMap::<ValueId, ValueId>::new();
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            let mut numbers = AHashMap::<ValueNumberKey, ValueId>::new();
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let key = match &instruction.op {
                    ControlFlowOp::Const(value) => {
                        Some(ValueNumberKey::Constant(ConstantNumber::from(value)))
                    }
                    ControlFlowOp::Unary { op, value } => Some(ValueNumberKey::Unary(*op, *value)),
                    ControlFlowOp::Binary { op, lhs, rhs } => {
                        let (lhs, rhs) = if is_commutative(*op) && rhs.0 < lhs.0 {
                            (*rhs, *lhs)
                        } else {
                            (*lhs, *rhs)
                        };
                        Some(ValueNumberKey::Binary(*op, lhs, rhs))
                    }
                    _ => None,
                };
                if let (Some(out), Some(key)) = (instruction.out, key) {
                    if let Some(previous) = numbers.get(&key) {
                        aliases.insert(out, *previous);
                        changed = true;
                        continue;
                    }
                    numbers.insert(key, out);
                }
                retained.push(instruction);
            }
            block.instructions = retained;
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "local-value-numbering",
        changed,
    }
}

fn is_int(value: Option<&ConstValue>, expected: i64) -> bool {
    matches!(value, Some(ConstValue::Int(value)) if *value == expected)
}

fn is_float(value: Option<&ConstValue>, expected: f64) -> bool {
    matches!(value, Some(ConstValue::Float(value)) if *value == expected)
}

fn is_bool(value: Option<&ConstValue>, expected: bool) -> bool {
    matches!(value, Some(ConstValue::Bool(value)) if *value == expected)
}

fn is_commutative(op: IrBinaryOp) -> bool {
    matches!(
        op,
        IrBinaryOp::Add
            | IrBinaryOp::Mul
            | IrBinaryOp::Eq
            | IrBinaryOp::NotEq
            | IrBinaryOp::And
            | IrBinaryOp::Or
    )
}

fn control_flow_value_types<'src>(
    function: &ControlFlowFunction<'src>,
) -> AHashMap<ValueId, Type<'src>> {
    let mut types = AHashMap::new();
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

fn internalize_entry_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut shared = AHashSet::new();
    for function in module
        .functions
        .iter()
        .filter(|function| function.id != module.entry)
    {
        for block in &function.blocks {
            for instruction in &block.instructions {
                match instruction.op {
                    ControlFlowOp::LoadGlobal(symbol) => {
                        shared.insert(symbol);
                    }
                    ControlFlowOp::StoreGlobal { global, .. } => {
                        shared.insert(global);
                    }
                    _ => {}
                }
            }
        }
    }

    let mut loaded_by_entry = AHashSet::new();
    for block in &module.functions[module.entry.0 as usize].blocks {
        for instruction in &block.instructions {
            if let ControlFlowOp::LoadGlobal(symbol) = instruction.op {
                loaded_by_entry.insert(symbol);
            }
        }
    }

    let internal = module
        .globals
        .iter()
        .filter(|global| {
            loaded_by_entry.contains(&global.symbol) && !shared.contains(&global.symbol)
        })
        .cloned()
        .collect::<Vec<_>>();
    if internal.is_empty() {
        return OptimizationReport {
            pass_name: "global-internalization",
            changed: false,
        };
    }

    let entry = &mut module.functions[module.entry.0 as usize];
    let mut local_by_symbol = AHashMap::new();
    for global in &internal {
        let local = LocalId(entry.locals.len() as u32);
        entry.locals.push(IrLocal {
            id: local,
            symbol: global.symbol,
            name: global.name,
            ty: global.ty.clone(),
            span: global.span,
        });
        local_by_symbol.insert(global.symbol, local);
    }
    for block in &mut entry.blocks {
        for instruction in &mut block.instructions {
            instruction.op = match instruction.op.clone() {
                ControlFlowOp::LoadGlobal(symbol) if local_by_symbol.contains_key(&symbol) => {
                    ControlFlowOp::LoadLocal(local_by_symbol[&symbol])
                }
                ControlFlowOp::StoreGlobal { global, value }
                    if local_by_symbol.contains_key(&global) =>
                {
                    ControlFlowOp::StoreLocal {
                        local: local_by_symbol[&global],
                        value,
                    }
                }
                op => op,
            };
        }
    }
    module
        .globals
        .retain(|global| !local_by_symbol.contains_key(&global.symbol));

    OptimizationReport {
        pass_name: "global-internalization",
        changed: true,
    }
}

fn devirtualize_methods(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                let replacement = match &instruction.op {
                    ControlFlowOp::CallMethod {
                        receiver,
                        function,
                        args,
                        ..
                    } => {
                        let mut direct_args = Vec::with_capacity(args.len() + 1);
                        direct_args.push(*receiver);
                        direct_args.extend(args.iter().copied());
                        Some(ControlFlowOp::CallDirect {
                            function: *function,
                            args: direct_args,
                        })
                    }
                    _ => None,
                };
                if let Some(replacement) = replacement {
                    instruction.op = replacement;
                    changed = true;
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "devirtualization",
        changed,
    }
}

fn fold_and_propagate_control_flow(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let mut constants = AHashMap::<ValueId, ConstValue>::new();
        let mut array_lengths = AHashMap::<ValueId, usize>::new();
        let mut local_change = true;
        while local_change {
            local_change = false;
            for block in &mut function.blocks {
                for phi in &block.phis {
                    let values = phi
                        .incoming
                        .iter()
                        .filter_map(|(_, value)| constants.get(value))
                        .collect::<Vec<_>>();
                    if values.len() == phi.incoming.len()
                        && values
                            .first()
                            .is_some_and(|first| values.iter().all(|value| *value == *first))
                    {
                        let value = values[0].clone();
                        if constants.insert(phi.out, value).is_none() {
                            local_change = true;
                        }
                    }
                }

                for instruction in &mut block.instructions {
                    let Some(out) = instruction.out else {
                        continue;
                    };
                    match &instruction.op {
                        ControlFlowOp::Const(value) => {
                            if constants.insert(out, value.clone()).is_none() {
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Array(values) => {
                            array_lengths.insert(out, values.len());
                        }
                        ControlFlowOp::Unary { op, value } => {
                            if let Some(folded) = constants
                                .get(value)
                                .and_then(|value| fold_unary(*op, value))
                            {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Binary { op, lhs, rhs } => {
                            if let Some(folded) = constants
                                .get(lhs)
                                .zip(constants.get(rhs))
                                .and_then(|(lhs, rhs)| fold_binary(*op, lhs, rhs))
                            {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::ArrayLength,
                            receiver: Some(receiver),
                            ..
                        } => {
                            if let Some(length) = array_lengths.get(receiver).copied() {
                                let folded = ConstValue::Int(length as i64);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic,
                            receiver: Some(receiver),
                            args,
                        } if matches!(
                            intrinsic,
                            Intrinsic::StringLength
                                | Intrinsic::StringIncludes
                                | Intrinsic::StringStartsWith
                                | Intrinsic::StringEndsWith
                                | Intrinsic::StringToUpperCase
                                | Intrinsic::StringToLowerCase
                        ) =>
                        {
                            let folded = constants.get(receiver).and_then(|receiver| {
                                fold_string_intrinsic(*intrinsic, receiver, args, &constants)
                            });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Template(parts) => {
                            let mut folded_parts = Vec::with_capacity(parts.len());
                            let mut changed_parts = false;
                            for part in parts {
                                match part {
                                    TemplateOperand::String(value) => {
                                        push_template_string(&mut folded_parts, value);
                                    }
                                    TemplateOperand::Value(value) => {
                                        if let Some(value) =
                                            constants.get(value).and_then(constant_string_value)
                                        {
                                            push_template_string(&mut folded_parts, &value);
                                            changed_parts = true;
                                        } else {
                                            folded_parts.push(TemplateOperand::Value(*value));
                                        }
                                    }
                                }
                            }
                            if folded_parts
                                .iter()
                                .all(|part| matches!(part, TemplateOperand::String(_)))
                            {
                                let mut value = String::new();
                                for part in &folded_parts {
                                    if let TemplateOperand::String(part) = part {
                                        value.push_str(part);
                                    }
                                }
                                let folded = ConstValue::String(value);
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            } else if changed_parts || folded_parts.len() != parts.len() {
                                instruction.op = ControlFlowOp::Template(folded_parts);
                                changed = true;
                                local_change = true;
                            }
                        }
                        _ => {}
                    }
                }

                if let Some(Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                }) = block.terminator.clone()
                {
                    if let Some(ConstValue::Bool(condition)) = constants.get(&condition) {
                        block.terminator = Some(Terminator::Jump(if *condition {
                            then_block
                        } else {
                            else_block
                        }));
                        changed = true;
                        local_change = true;
                    }
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "constant-propagation",
        changed,
    }
}

fn remove_unreachable_control_flow(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let reachable = reachable_blocks(function);
        if reachable.len() == function.blocks.len() {
            continue;
        }
        let mut mapping = vec![None; function.blocks.len()];
        let mut next = 0u32;
        for (old, mapped) in mapping.iter_mut().enumerate() {
            if reachable.contains(&old) {
                *mapped = Some(BlockId(next));
                next += 1;
            }
        }
        let mut blocks = Vec::with_capacity(reachable.len());
        for (old, mut block) in std::mem::take(&mut function.blocks).into_iter().enumerate() {
            let Some(new_id) = mapping[old] else {
                continue;
            };
            block.id = new_id;
            for phi in &mut block.phis {
                phi.incoming.retain_mut(|(incoming, _)| {
                    if let Some(mapped) = mapping[incoming.0 as usize] {
                        *incoming = mapped;
                        true
                    } else {
                        false
                    }
                });
            }
            if let Some(terminator) = &mut block.terminator {
                remap_terminator_blocks(terminator, &mapping);
            }
            blocks.push(block);
        }
        function.entry = mapping[function.entry.0 as usize]
            .expect("the function entry block is always reachable");
        function.shapes.retain_mut(|shape| match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                merge_block,
            } => {
                let mapped = [*header, *then_block, *else_block, *merge_block]
                    .map(|block| mapping[block.0 as usize]);
                if let [Some(new_header), Some(new_then), Some(new_else), Some(new_merge)] = mapped
                {
                    *header = new_header;
                    *then_block = new_then;
                    *else_block = new_else;
                    *merge_block = new_merge;
                    true
                } else {
                    false
                }
            }
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                exit,
            } => {
                let Some(new_header) = mapping[header.0 as usize] else {
                    return false;
                };
                let Some(new_body) = mapping[body.0 as usize] else {
                    return false;
                };
                let Some(new_exit) = mapping[exit.0 as usize] else {
                    return false;
                };
                let new_update = match *update {
                    Some(block) => match mapping[block.0 as usize] {
                        Some(mapped) => Some(mapped),
                        None => return false,
                    },
                    None => None,
                };
                *header = new_header;
                *body = new_body;
                *update = new_update;
                *exit = new_exit;
                true
            }
        });
        function.blocks = blocks;
        changed = true;
    }
    OptimizationReport {
        pass_name: "unreachable-block-elimination",
        changed,
    }
}

fn remap_terminator_blocks(terminator: &mut Terminator, mapping: &[Option<BlockId>]) {
    match terminator {
        Terminator::Jump(target) => {
            *target = mapping[target.0 as usize].expect("reachable jump target must be mapped")
        }
        Terminator::Branch {
            then_block,
            else_block,
            ..
        } => {
            *then_block =
                mapping[then_block.0 as usize].expect("reachable branch target must be mapped");
            *else_block =
                mapping[else_block.0 as usize].expect("reachable branch target must be mapped");
        }
        _ => {}
    }
}

fn inline_small_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    const INLINE_LIMIT: usize = 12;
    let recursive = recursive_functions(module);
    let candidates = module
        .functions
        .iter()
        .filter(|function| {
            !matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                && !recursive.contains(&function.id)
                && function.blocks.len() == 1
                && function.blocks[0].phis.is_empty()
                && function.blocks[0].instructions.len() <= INLINE_LIMIT
                && matches!(function.blocks[0].terminator, Some(Terminator::Return(_)))
        })
        .map(|function| (function.id, function.clone()))
        .collect::<AHashMap<_, _>>();
    let mut changed = false;

    for caller in &mut module.functions {
        let mut aliases = AHashMap::<ValueId, ValueId>::new();
        for block in &mut caller.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                let constructor_call = match &instruction.op {
                    ControlFlowOp::NewClass {
                        class,
                        constructor: Some(constructor),
                        args,
                    } => Some((*class, *constructor, args.clone())),
                    _ => None,
                };
                if let Some((class, constructor, args)) = constructor_call {
                    let Some(object) = instruction.out else {
                        rewritten.push(instruction);
                        continue;
                    };
                    let Some(callee) = candidates.get(&constructor) else {
                        rewritten.push(instruction);
                        continue;
                    };
                    if callee.params.len() != args.len() + 1
                        || !matches!(callee.blocks[0].terminator, Some(Terminator::Return(None)))
                    {
                        rewritten.push(instruction);
                        continue;
                    }

                    let call_args = std::iter::once(object).chain(args);
                    let mut mapping = callee
                        .params
                        .iter()
                        .zip(call_args)
                        .filter_map(|(param, arg)| {
                            let arg = resolve_alias(arg, &aliases);
                            (param.value != arg).then_some((param.value, arg))
                        })
                        .collect::<AHashMap<_, _>>();
                    instruction.op = ControlFlowOp::NewClass {
                        class,
                        constructor: None,
                        args: Vec::new(),
                    };
                    rewritten.push(instruction);
                    for callee_instruction in &callee.blocks[0].instructions {
                        let mut cloned = callee_instruction.clone();
                        rewrite_control_flow_op_once(&mut cloned.op, &mapping);
                        if let Some(old_out) = cloned.out {
                            let new_out = ValueId(caller.value_count);
                            caller.value_count += 1;
                            let escape = callee
                                .value_escapes
                                .get(old_out.0 as usize)
                                .copied()
                                .unwrap_or(EscapeState::LocalOnly);
                            caller.value_escapes.push(escape);
                            mapping.insert(old_out, new_out);
                            cloned.out = Some(new_out);
                        }
                        rewritten.push(cloned);
                    }
                    changed = true;
                    continue;
                }
                let ControlFlowOp::CallDirect { function, args } = &instruction.op else {
                    rewritten.push(instruction);
                    continue;
                };
                if *function == caller.id {
                    rewritten.push(instruction);
                    continue;
                }
                let Some(callee) = candidates.get(function) else {
                    rewritten.push(instruction);
                    continue;
                };
                if args.len() != callee.params.len() {
                    rewritten.push(instruction);
                    continue;
                }

                let mut mapping = callee
                    .params
                    .iter()
                    .zip(args)
                    .filter_map(|(param, arg)| {
                        let arg = resolve_alias(*arg, &aliases);
                        (param.value != arg).then_some((param.value, arg))
                    })
                    .collect::<AHashMap<_, _>>();
                for callee_instruction in &callee.blocks[0].instructions {
                    let mut cloned = callee_instruction.clone();
                    rewrite_control_flow_op_once(&mut cloned.op, &mapping);
                    if let Some(old_out) = cloned.out {
                        let new_out = ValueId(caller.value_count);
                        caller.value_count += 1;
                        let escape = callee
                            .value_escapes
                            .get(old_out.0 as usize)
                            .copied()
                            .unwrap_or(EscapeState::LocalOnly);
                        caller.value_escapes.push(escape);
                        mapping.insert(old_out, new_out);
                        cloned.out = Some(new_out);
                    }
                    rewritten.push(cloned);
                }

                if let (Some(call_out), Some(Terminator::Return(Some(returned)))) =
                    (instruction.out, &callee.blocks[0].terminator)
                {
                    aliases.insert(
                        call_out,
                        mapping.get(returned).copied().unwrap_or(*returned),
                    );
                }
                changed = true;
            }
            block.instructions = rewritten;
        }
        rewrite_control_flow_function(caller, &aliases);
    }

    OptimizationReport {
        pass_name: "inlining",
        changed,
    }
}

fn inline_single_use_control_flow_function(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    const INLINE_LIMIT: usize = 30;
    let recursive = recursive_functions(module);
    let mut call_counts = AHashMap::<FunctionId, usize>::new();
    let mut address_taken = AHashSet::<FunctionId>::new();
    for instruction in module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
    {
        match instruction.op {
            ControlFlowOp::CallDirect { function, .. } => {
                *call_counts.entry(function).or_insert(0) += 1;
            }
            ControlFlowOp::Closure { function, .. } => {
                address_taken.insert(function);
            }
            _ => {}
        }
    }
    let candidates = module
        .functions
        .iter()
        .filter(|function| {
            matches!(
                function.kind,
                FunctionKind::Function | FunctionKind::Method { .. }
            ) && function.blocks.len() > 1
                && !recursive.contains(&function.id)
                && !address_taken.contains(&function.id)
                && call_counts.get(&function.id) == Some(&1)
                && function
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len())
                    .sum::<usize>()
                    <= INLINE_LIMIT
        })
        .map(|function| (function.id, function.clone()))
        .collect::<AHashMap<_, _>>();

    let mut site = None;
    'functions: for (function_index, caller) in module.functions.iter().enumerate() {
        let shaped_headers = caller
            .shapes
            .iter()
            .map(crate::ir::ControlShape::header)
            .collect::<AHashSet<_>>();
        for (block_index, block) in caller.blocks.iter().enumerate() {
            if shaped_headers.contains(&block.id) {
                continue;
            }
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let ControlFlowOp::CallDirect { function, args } = &instruction.op else {
                    continue;
                };
                if *function == caller.id || !candidates.contains_key(function) {
                    continue;
                }
                site = Some((
                    function_index,
                    block_index,
                    instruction_index,
                    *function,
                    args.clone(),
                    instruction.out,
                    instruction.ty.clone(),
                ));
                break 'functions;
            }
        }
    }

    let Some((caller_index, block_index, instruction_index, callee_id, args, out, ty)) = site
    else {
        return OptimizationReport {
            pass_name: "cfg-inlining",
            changed: false,
        };
    };
    let callee = &candidates[&callee_id];
    inline_control_flow_call(
        &mut module.functions[caller_index],
        block_index,
        instruction_index,
        callee,
        &args,
        out,
        ty,
    );
    OptimizationReport {
        pass_name: "cfg-inlining",
        changed: true,
    }
}

#[allow(clippy::too_many_arguments)]
fn inline_control_flow_call<'src>(
    caller: &mut ControlFlowFunction<'src>,
    block_index: usize,
    instruction_index: usize,
    callee: &ControlFlowFunction<'src>,
    args: &[ValueId],
    call_out: Option<ValueId>,
    call_type: Option<Type<'src>>,
) {
    let base = caller.blocks.len() as u32;
    let block_mapping = callee
        .blocks
        .iter()
        .map(|block| (block.id, BlockId(base + block.id.0)))
        .collect::<AHashMap<_, _>>();
    let continuation = BlockId(base + callee.blocks.len() as u32);
    let mut value_mapping = callee
        .params
        .iter()
        .zip(args)
        .map(|(parameter, argument)| (parameter.value, *argument))
        .collect::<AHashMap<_, _>>();

    for block in &callee.blocks {
        for phi in &block.phis {
            allocate_inlined_value(caller, callee, phi.out, &mut value_mapping);
        }
        for instruction in &block.instructions {
            if let Some(out) = instruction.out {
                allocate_inlined_value(caller, callee, out, &mut value_mapping);
            }
        }
    }

    let (remaining, original_terminator, original_span) = {
        let block = &mut caller.blocks[block_index];
        let remaining = block.instructions.split_off(instruction_index + 1);
        block.instructions.pop();
        let original_terminator = block.terminator.take();
        block.terminator = Some(Terminator::Jump(block_mapping[&callee.entry]));
        (remaining, original_terminator, block.span)
    };

    let mut returns = Vec::<(BlockId, ValueId)>::new();
    for block in &callee.blocks {
        let id = block_mapping[&block.id];
        let phis = block
            .phis
            .iter()
            .map(|phi| Phi {
                out: value_mapping[&phi.out],
                local: LocalId(u32::MAX),
                ty: phi.ty.clone(),
                incoming: phi
                    .incoming
                    .iter()
                    .map(|(incoming, value)| {
                        (
                            block_mapping[incoming],
                            mapped_value(*value, &value_mapping),
                        )
                    })
                    .collect(),
                span: phi.span,
            })
            .collect();
        let instructions = block
            .instructions
            .iter()
            .map(|instruction| {
                let mut instruction = instruction.clone();
                instruction.out = instruction.out.map(|out| value_mapping[&out]);
                rewrite_control_flow_op_once(&mut instruction.op, &value_mapping);
                instruction
            })
            .collect();
        let terminator = match block
            .terminator
            .as_ref()
            .expect("optimized callee blocks have terminators")
        {
            Terminator::Jump(target) => Terminator::Jump(block_mapping[target]),
            Terminator::Branch {
                condition,
                then_block,
                else_block,
            } => Terminator::Branch {
                condition: mapped_value(*condition, &value_mapping),
                then_block: block_mapping[then_block],
                else_block: block_mapping[else_block],
            },
            Terminator::Return(value) => {
                if let Some(value) = value {
                    returns.push((id, mapped_value(*value, &value_mapping)));
                }
                Terminator::Jump(continuation)
            }
            Terminator::Unreachable => Terminator::Unreachable,
        };
        caller.blocks.push(crate::ir::ControlFlowBlock {
            id,
            phis,
            instructions,
            terminator: Some(terminator),
            span: block.span,
        });
    }

    for shape in &callee.shapes {
        caller.shapes.push(match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                merge_block,
            } => crate::ir::ControlShape::If {
                header: block_mapping[header],
                then_block: block_mapping[then_block],
                else_block: block_mapping[else_block],
                merge_block: block_mapping[merge_block],
            },
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                exit,
            } => crate::ir::ControlShape::Loop {
                header: block_mapping[header],
                body: block_mapping[body],
                update: update.map(|block| block_mapping[&block]),
                exit: block_mapping[exit],
            },
        });
    }

    let phis = call_out
        .zip(call_type)
        .map(|(out, ty)| {
            vec![Phi {
                out,
                local: LocalId(u32::MAX),
                ty,
                incoming: returns,
                span: original_span,
            }]
        })
        .unwrap_or_default();
    caller.blocks.push(crate::ir::ControlFlowBlock {
        id: continuation,
        phis,
        instructions: remaining,
        terminator: original_terminator,
        span: original_span,
    });
}

fn allocate_inlined_value(
    caller: &mut ControlFlowFunction<'_>,
    callee: &ControlFlowFunction<'_>,
    old: ValueId,
    mapping: &mut AHashMap<ValueId, ValueId>,
) {
    if mapping.contains_key(&old) {
        return;
    }
    let new = ValueId(caller.value_count);
    caller.value_count += 1;
    caller.value_escapes.push(
        callee
            .value_escapes
            .get(old.0 as usize)
            .copied()
            .unwrap_or(EscapeState::LocalOnly),
    );
    mapping.insert(old, new);
}

fn mapped_value(value: ValueId, mapping: &AHashMap<ValueId, ValueId>) -> ValueId {
    mapping.get(&value).copied().unwrap_or(value)
}

fn recursive_functions(module: &ControlFlowModule<'_>) -> AHashSet<crate::ir::FunctionId> {
    let graph = module
        .functions
        .iter()
        .map(|function| {
            let callees = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match instruction.op {
                    ControlFlowOp::CallDirect { function, .. }
                    | ControlFlowOp::CallMethod { function, .. }
                    | ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    } => Some(function),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (function.id, callees)
        })
        .collect::<AHashMap<_, _>>();
    let mut recursive = AHashSet::new();
    for start in graph.keys().copied() {
        let mut visited = AHashSet::new();
        let mut work = graph.get(&start).cloned().unwrap_or_default();
        while let Some(function) = work.pop() {
            if function == start {
                recursive.insert(start);
                break;
            }
            if visited.insert(function) {
                work.extend(graph.get(&function).into_iter().flatten().copied());
            }
        }
    }
    recursive
}

fn scalar_replace_linear_classes(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let layouts = module
        .classes
        .iter()
        .filter_map(|layout| {
            let fields = layout
                .fields
                .iter()
                .map(|field| {
                    default_scalar_constant(&field.ty).map(|value| (field.ty.clone(), value))
                })
                .collect::<Option<Vec<_>>>()?;
            Some((layout.name, fields))
        })
        .collect::<AHashMap<_, _>>();
    let mut changed = false;

    for function in &mut module.functions {
        let mut candidates = AHashMap::<ValueId, (usize, &str)>::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for instruction in &block.instructions {
                if let (
                    Some(out),
                    ControlFlowOp::NewClass {
                        class,
                        constructor: None,
                        args,
                    },
                ) = (instruction.out, &instruction.op)
                {
                    if args.is_empty()
                        && layouts.contains_key(class)
                        && function.value_escapes[out.0 as usize] == EscapeState::LocalOnly
                    {
                        candidates.insert(out, (block_index, class));
                    }
                }
            }
        }

        let mut invalid = AHashSet::new();
        for (block_index, block) in function.blocks.iter().enumerate() {
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::FieldGet { object, .. }
                    | ControlFlowOp::FieldSet { object, .. }
                        if candidates
                            .get(object)
                            .is_some_and(|(defined_in, _)| *defined_in == block_index) => {}
                    _ => {
                        for value in control_flow_used_values(&instruction.op) {
                            if candidates.contains_key(&value) {
                                invalid.insert(value);
                            }
                        }
                    }
                }
            }
            for value in block
                .terminator
                .as_ref()
                .into_iter()
                .flat_map(terminator_used_values)
            {
                if candidates.contains_key(&value) {
                    invalid.insert(value);
                }
            }
        }

        let mut aliases = AHashMap::<ValueId, ValueId>::new();
        for (block_index, block) in function.blocks.iter_mut().enumerate() {
            let instructions = std::mem::take(&mut block.instructions);
            let mut rewritten = Vec::with_capacity(instructions.len());
            let mut fields_by_object = AHashMap::<ValueId, Vec<ValueId>>::new();
            for mut instruction in instructions {
                rewrite_control_flow_op(&mut instruction.op, &aliases);
                match (&instruction.out, &instruction.op) {
                    (
                        Some(object),
                        ControlFlowOp::NewClass {
                            class,
                            constructor: None,
                            args,
                        },
                    ) if args.is_empty()
                        && candidates
                            .get(object)
                            .is_some_and(|(defined_in, _)| *defined_in == block_index)
                        && !invalid.contains(object) =>
                    {
                        let mut fields = Vec::new();
                        for (ty, value) in &layouts[class] {
                            let out = ValueId(function.value_count);
                            function.value_count += 1;
                            function.value_escapes.push(EscapeState::LocalOnly);
                            rewritten.push(ControlFlowInstruction {
                                out: Some(out),
                                ty: Some(ty.clone()),
                                op: ControlFlowOp::Const(value.clone()),
                                span: instruction.span,
                            });
                            fields.push(out);
                        }
                        fields_by_object.insert(*object, fields);
                        changed = true;
                    }
                    (
                        _,
                        ControlFlowOp::FieldSet {
                            object,
                            index,
                            value,
                            ..
                        },
                    ) if candidates.contains_key(object) && !invalid.contains(object) => {
                        if let Some(field) = fields_by_object
                            .get_mut(object)
                            .and_then(|fields| fields.get_mut(*index))
                        {
                            *field = resolve_alias(*value, &aliases);
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    (Some(out), ControlFlowOp::FieldGet { object, index, .. })
                        if candidates.contains_key(object) && !invalid.contains(object) =>
                    {
                        if let Some(value) = fields_by_object
                            .get(object)
                            .and_then(|fields| fields.get(*index))
                            .copied()
                        {
                            aliases.insert(*out, resolve_alias(value, &aliases));
                            changed = true;
                        } else {
                            rewritten.push(instruction);
                        }
                    }
                    _ => rewritten.push(instruction),
                }
            }
            block.instructions = rewritten;
        }
        rewrite_control_flow_function(function, &aliases);
    }

    OptimizationReport {
        pass_name: "class-scalar-replacement",
        changed,
    }
}

fn default_scalar_constant(ty: &Type<'_>) -> Option<ConstValue> {
    match ty {
        Type::Int => Some(ConstValue::Int(0)),
        Type::Float => Some(ConstValue::Float(0.0)),
        Type::Bool => Some(ConstValue::Bool(false)),
        Type::String => Some(ConstValue::String(String::new())),
        _ => None,
    }
}

fn analyze_escapes(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    let extern_functions = module
        .functions
        .iter()
        .filter(|function| function.kind == FunctionKind::Extern)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    for function in &mut module.functions {
        let mut aggregates = AHashSet::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let Some(out) = instruction.out {
                    if matches!(
                        instruction.op,
                        ControlFlowOp::Array(_)
                            | ControlFlowOp::Struct { .. }
                            | ControlFlowOp::NewClass { .. }
                            | ControlFlowOp::Closure { .. }
                    ) {
                        aggregates.insert(out);
                    }
                }
            }
        }

        let mut mark = |value: ValueId, state: EscapeState| {
            if !aggregates.contains(&value) {
                return;
            }
            let slot = &mut function.value_escapes[value.0 as usize];
            if escape_rank(state) > escape_rank(*slot) {
                *slot = state;
                changed = true;
            }
        };

        for block in &function.blocks {
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::StoreGlobal { value, .. } => {
                        mark(*value, EscapeState::EscapesToTypedCode)
                    }
                    ControlFlowOp::CallDirect {
                        function: callee,
                        args,
                    } => {
                        let escape = if extern_functions.contains(callee) {
                            EscapeState::EscapesToUntypedBoundary
                        } else {
                            EscapeState::EscapesToTypedCode
                        };
                        for value in args {
                            mark(*value, escape);
                        }
                        if escape == EscapeState::EscapesToUntypedBoundary {
                            if let Some(out) = instruction.out {
                                mark(out, escape);
                            }
                        }
                    }
                    ControlFlowOp::CallMethod { args, .. } => {
                        for value in args {
                            mark(*value, EscapeState::EscapesToTypedCode);
                        }
                    }
                    ControlFlowOp::CallValue { callee, args } => {
                        mark(*callee, EscapeState::EscapesToTypedCode);
                        for value in args {
                            mark(*value, EscapeState::EscapesToTypedCode);
                        }
                    }
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::Print,
                        args,
                        ..
                    } => {
                        for value in args {
                            mark(*value, EscapeState::EscapesToUntypedBoundary);
                        }
                    }
                    ControlFlowOp::Intrinsic { receiver, args, .. } => {
                        if let Some(receiver) = receiver {
                            mark(*receiver, EscapeState::EscapesToTypedCode);
                        }
                        for value in args {
                            mark(*value, EscapeState::EscapesToTypedCode);
                        }
                    }
                    ControlFlowOp::Closure { captures, .. } => {
                        for value in captures {
                            mark(*value, EscapeState::EscapesToTypedCode);
                        }
                    }
                    _ => {}
                }
            }
            if let Some(Terminator::Return(Some(value))) = block.terminator {
                mark(value, EscapeState::EscapesToTypedCode);
            }
        }
    }
    OptimizationReport {
        pass_name: "escape-analysis",
        changed,
    }
}

fn scalar_replace_control_flow_aggregates(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let mut changed = false;
    for function in &mut module.functions {
        let mut structs = AHashMap::<ValueId, Vec<ValueId>>::new();
        let mut invalid = AHashSet::<ValueId>::new();
        for block in &function.blocks {
            for instruction in &block.instructions {
                if let (Some(out), ControlFlowOp::Struct { fields, .. }) =
                    (instruction.out, &instruction.op)
                {
                    if function.value_escapes[out.0 as usize] == EscapeState::LocalOnly {
                        structs.insert(out, fields.clone());
                    }
                }
            }
        }
        for block in &function.blocks {
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::FieldGet { object, .. } if structs.contains_key(object) => {}
                    _ => {
                        for used in control_flow_used_values(&instruction.op) {
                            if structs.contains_key(&used) {
                                invalid.insert(used);
                            }
                        }
                    }
                }
            }
            for used in block
                .terminator
                .as_ref()
                .into_iter()
                .flat_map(terminator_used_values)
            {
                if structs.contains_key(&used) {
                    invalid.insert(used);
                }
            }
        }

        let mut aliases = AHashMap::<ValueId, ValueId>::new();
        for block in &mut function.blocks {
            let instructions = std::mem::take(&mut block.instructions);
            let mut retained = Vec::with_capacity(instructions.len());
            for instruction in instructions {
                match (&instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Struct { .. })
                        if structs.contains_key(out) && !invalid.contains(out) =>
                    {
                        changed = true;
                    }
                    (Some(out), ControlFlowOp::FieldGet { object, index, .. })
                        if structs.contains_key(object) && !invalid.contains(object) =>
                    {
                        if let Some(value) = structs
                            .get(object)
                            .and_then(|fields| fields.get(*index))
                            .copied()
                        {
                            aliases.insert(*out, resolve_alias(value, &aliases));
                            changed = true;
                        } else {
                            retained.push(instruction);
                        }
                    }
                    _ => retained.push(instruction),
                }
            }
            block.instructions = retained;
        }
        rewrite_control_flow_function(function, &aliases);
    }
    OptimizationReport {
        pass_name: "scalar-replacement-cfg",
        changed,
    }
}

fn eliminate_dead_control_flow_instructions(
    module: &mut ControlFlowModule<'_>,
) -> OptimizationReport {
    let mut changed = false;
    let effectful_functions = find_effectful_functions(module);
    for function in &mut module.functions {
        let closure_targets = closure_targets(function);
        loop {
            let uses = control_flow_use_counts(function);
            let mut local_change = false;
            for block in &mut function.blocks {
                let old_len = block.instructions.len();
                block.instructions.retain(|instruction| {
                    instruction.out.is_none_or(|out| {
                        uses.get(&out).copied().unwrap_or(0) != 0
                            || control_flow_op_has_side_effects(
                                &instruction.op,
                                &effectful_functions,
                                &closure_targets,
                            )
                    })
                });
                local_change |= block.instructions.len() != old_len;

                let old_phi_len = block.phis.len();
                block
                    .phis
                    .retain(|phi| uses.get(&phi.out).copied().unwrap_or(0) != 0);
                local_change |= block.phis.len() != old_phi_len;
            }
            changed |= local_change;
            if !local_change {
                break;
            }
        }
    }
    OptimizationReport {
        pass_name: "ssa-dead-code-elimination",
        changed,
    }
}

fn eliminate_dead_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut reachable = AHashSet::new();
    let mut work = vec![module.entry];
    while let Some(function_id) = work.pop() {
        if !reachable.insert(function_id) {
            continue;
        }
        let Some(function) = module.functions.get(function_id.0 as usize) else {
            continue;
        };
        for block in &function.blocks {
            for instruction in &block.instructions {
                match &instruction.op {
                    ControlFlowOp::CallDirect { function, .. }
                    | ControlFlowOp::Closure { function, .. } => work.push(*function),
                    ControlFlowOp::CallMethod { function, .. } => work.push(*function),
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    } => work.push(*function),
                    _ => {}
                }
            }
        }
    }

    let mut changed = false;
    for function in &mut module.functions {
        let live = reachable.contains(&function.id);
        changed |= function.live != live;
        function.live = live;
    }
    OptimizationReport {
        pass_name: "dead-function-elimination",
        changed,
    }
}

fn control_flow_use_counts(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, usize> {
    let mut uses = AHashMap::new();
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

fn control_flow_used_values(op: &ControlFlowOp<'_>) -> Vec<ValueId> {
    match op {
        ControlFlowOp::Const(_) | ControlFlowOp::LoadLocal(_) | ControlFlowOp::LoadGlobal(_) => {
            Vec::new()
        }
        ControlFlowOp::Unary { value, .. } => vec![*value],
        ControlFlowOp::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ControlFlowOp::Array(values) => values.clone(),
        ControlFlowOp::Struct { fields, .. } => fields.clone(),
        ControlFlowOp::NewClass { args, .. } => args.clone(),
        ControlFlowOp::Closure { captures, .. } => captures.clone(),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            vec![*value]
        }
        ControlFlowOp::FieldGet { object, .. } => vec![*object],
        ControlFlowOp::FieldSet { object, value, .. } => vec![*object, *value],
        ControlFlowOp::IndexGet { object, index } => vec![*object, *index],
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
        ControlFlowOp::CallMethod { receiver, args, .. } => {
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
                TemplateOperand::Value(value) => Some(*value),
                TemplateOperand::String(_) => None,
            })
            .collect(),
    }
}

fn terminator_used_values(terminator: &Terminator) -> Vec<ValueId> {
    match terminator {
        Terminator::Branch { condition, .. } => vec![*condition],
        Terminator::Return(Some(value)) => vec![*value],
        _ => Vec::new(),
    }
}

fn find_effectful_functions(module: &ControlFlowModule<'_>) -> AHashSet<crate::ir::FunctionId> {
    let mut effectful = module
        .functions
        .iter()
        .filter(|function| function.kind == FunctionKind::Extern)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    loop {
        let mut changed = false;
        for function in &module.functions {
            if effectful.contains(&function.id) {
                continue;
            }
            let targets = closure_targets(function);
            let has_effect = function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    control_flow_op_has_side_effects(&instruction.op, &effectful, &targets)
                });
            if has_effect {
                effectful.insert(function.id);
                changed = true;
            }
        }
        if !changed {
            return effectful;
        }
    }
}

fn control_flow_op_has_side_effects(
    op: &ControlFlowOp<'_>,
    effectful_functions: &AHashSet<crate::ir::FunctionId>,
    closure_targets: &AHashMap<ValueId, crate::ir::FunctionId>,
) -> bool {
    match op {
        ControlFlowOp::StoreLocal { .. }
        | ControlFlowOp::StoreGlobal { .. }
        | ControlFlowOp::FieldSet { .. }
        | ControlFlowOp::IndexSet { .. }
        | ControlFlowOp::CallMethod { .. } => true,
        ControlFlowOp::CallValue { callee, .. } => closure_targets
            .get(callee)
            .is_none_or(|function| effectful_functions.contains(function)),
        ControlFlowOp::CallDirect { function, .. } => effectful_functions.contains(function),
        ControlFlowOp::NewClass {
            constructor: Some(function),
            ..
        } => effectful_functions.contains(function),
        ControlFlowOp::NewClass {
            constructor: None, ..
        } => false,
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::ArrayMap
                | Intrinsic::ArrayFilter
                | Intrinsic::ArrayReduce
                | Intrinsic::ArrayForEach,
            args,
            ..
        } => args.first().is_none_or(|callback| {
            closure_targets
                .get(callback)
                .is_none_or(|function| effectful_functions.contains(function))
        }),
        ControlFlowOp::Intrinsic { intrinsic, .. } => matches!(
            intrinsic,
            Intrinsic::Print | Intrinsic::ArrayPush | Intrinsic::ArrayPop
        ),
        _ => false,
    }
}

fn closure_targets(function: &ControlFlowFunction<'_>) -> AHashMap<ValueId, crate::ir::FunctionId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Closure { function, .. }) => Some((out, *function)),
            _ => None,
        })
        .collect()
}

fn escape_rank(state: EscapeState) -> u8 {
    match state {
        EscapeState::LocalOnly => 0,
        EscapeState::EscapesToTypedCode => 1,
        EscapeState::EscapesToUntypedBoundary => 2,
    }
}

#[derive(Debug, Default)]
pub struct Pipeline {
    reports: Vec<OptimizationReport>,
}

impl Pipeline {
    pub fn run(&mut self, module: &mut IrModule, passes: &mut [&mut dyn OptimizationPass]) {
        self.reports.clear();
        for pass in passes {
            self.reports.push(pass.run(module));
        }
    }

    pub fn reports(&self) -> &[OptimizationReport] {
        &self.reports
    }
}

#[derive(Debug, Default)]
pub struct ConstantFolding;

impl OptimizationPass for ConstantFolding {
    fn name(&self) -> &'static str {
        "constant-folding"
    }

    fn run(&mut self, module: &mut IrModule) -> OptimizationReport {
        let mut changed = false;
        for function in &mut module.functions {
            let mut constants = AHashMap::new();
            for block in &mut function.blocks {
                for instruction in &mut block.instructions {
                    match instruction {
                        Instruction::Const { out, value, .. } => {
                            constants.insert(*out, value.clone());
                        }
                        Instruction::Binary {
                            out,
                            op,
                            lhs,
                            rhs,
                            span,
                        } => {
                            let folded = constants
                                .get(lhs)
                                .zip(constants.get(rhs))
                                .and_then(|(lhs, rhs)| fold_binary(*op, lhs, rhs));
                            if let Some(value) = folded {
                                let out = *out;
                                let span = *span;
                                constants.insert(out, value.clone());
                                *instruction = Instruction::Const { out, value, span };
                                changed = true;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        OptimizationReport {
            pass_name: self.name(),
            changed,
        }
    }
}

#[derive(Debug, Default)]
pub struct ScalarReplacement;

impl OptimizationPass for ScalarReplacement {
    fn name(&self) -> &'static str {
        "scalar-replacement"
    }

    fn run(&mut self, module: &mut IrModule) -> OptimizationReport {
        let mut changed = false;
        for function in &mut module.functions {
            let mut aggregates = AHashMap::<ValueId, Vec<ValueId>>::new();
            let mut escaping = AHashSet::<ValueId>::new();

            for block in &function.blocks {
                for instruction in &block.instructions {
                    if let Instruction::Struct { out, fields, .. } = instruction {
                        aggregates.insert(*out, fields.clone());
                    }
                }
            }

            for block in &function.blocks {
                for instruction in &block.instructions {
                    match instruction {
                        Instruction::FieldGet { aggregate, .. } => {
                            if !aggregates.contains_key(aggregate) {
                                escaping.insert(*aggregate);
                            }
                        }
                        _ => {
                            for used in used_values(instruction) {
                                if aggregates.contains_key(&used) {
                                    escaping.insert(used);
                                }
                            }
                        }
                    }
                }
            }

            let mut aliases = AHashMap::<ValueId, ValueId>::new();
            for block in &mut function.blocks {
                let mut rewritten = Vec::with_capacity(block.instructions.len());
                for mut instruction in block.instructions.drain(..) {
                    match &instruction {
                        Instruction::Struct { out, .. } if !escaping.contains(out) => {
                            changed = true;
                            continue;
                        }
                        Instruction::FieldGet {
                            out,
                            aggregate,
                            index,
                            ..
                        } if !escaping.contains(aggregate) => {
                            if let Some(field) = aggregates
                                .get(aggregate)
                                .and_then(|fields| fields.get(*index))
                            {
                                aliases.insert(*out, resolve_alias(*field, &aliases));
                                changed = true;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    rewrite_values(&mut instruction, &aliases);
                    rewritten.push(instruction);
                }
                block.instructions = rewritten;
            }
        }

        OptimizationReport {
            pass_name: self.name(),
            changed,
        }
    }
}

#[derive(Debug, Default)]
pub struct DeadCodeElimination;

impl OptimizationPass for DeadCodeElimination {
    fn name(&self) -> &'static str {
        "dead-code-elimination"
    }

    fn run(&mut self, module: &mut IrModule) -> OptimizationReport {
        let mut changed = false;
        for function in &mut module.functions {
            let mut live = AHashSet::<ValueId>::new();

            for block in function.blocks.iter_mut().rev() {
                let old_len = block.instructions.len();
                let mut retained = Vec::with_capacity(old_len);
                for instruction in block.instructions.drain(..).rev() {
                    match instruction {
                        Instruction::Return { value, .. } => {
                            if let Some(value) = value {
                                live.insert(value);
                            }
                            retained.push(instruction);
                        }
                        Instruction::Call { out, ref args, .. } => {
                            if let Some(out) = out {
                                live.remove(&out);
                            }
                            live.extend(args.iter().copied());
                            retained.push(instruction);
                        }
                        pure => {
                            let Some(out) = result_value(&pure) else {
                                retained.push(pure);
                                continue;
                            };
                            if live.remove(&out) {
                                live.extend(used_values(&pure));
                                retained.push(pure);
                            }
                        }
                    }
                }
                retained.reverse();
                changed |= retained.len() != old_len;
                block.instructions = retained;
            }
        }

        OptimizationReport {
            pass_name: self.name(),
            changed,
        }
    }
}

fn fold_string_intrinsic(
    intrinsic: Intrinsic,
    receiver: &ConstValue,
    args: &[ValueId],
    constants: &AHashMap<ValueId, ConstValue>,
) -> Option<ConstValue> {
    let ConstValue::String(receiver) = receiver else {
        return None;
    };
    let argument = || {
        args.first()
            .and_then(|value| constants.get(value))
            .and_then(|value| match value {
                ConstValue::String(value) => Some(value.as_str()),
                _ => None,
            })
    };
    match intrinsic {
        Intrinsic::StringLength => Some(ConstValue::Int(receiver.chars().count() as i64)),
        Intrinsic::StringIncludes => Some(ConstValue::Bool(receiver.contains(argument()?))),
        Intrinsic::StringStartsWith => Some(ConstValue::Bool(receiver.starts_with(argument()?))),
        Intrinsic::StringEndsWith => Some(ConstValue::Bool(receiver.ends_with(argument()?))),
        Intrinsic::StringToUpperCase if receiver.is_ascii() => {
            Some(ConstValue::String(receiver.to_ascii_uppercase()))
        }
        Intrinsic::StringToLowerCase if receiver.is_ascii() => {
            Some(ConstValue::String(receiver.to_ascii_lowercase()))
        }
        _ => None,
    }
}

fn constant_string_value(value: &ConstValue) -> Option<String> {
    Some(match value {
        ConstValue::Int(value) => value.to_string(),
        ConstValue::Float(value) => value.to_string(),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => value.clone(),
    })
}

fn push_template_string(parts: &mut Vec<TemplateOperand>, value: &str) {
    if let Some(TemplateOperand::String(previous)) = parts.last_mut() {
        previous.push_str(value);
    } else if !value.is_empty() {
        parts.push(TemplateOperand::String(value.to_string()));
    }
}

fn fold_binary(op: IrBinaryOp, lhs: &ConstValue, rhs: &ConstValue) -> Option<ConstValue> {
    use ConstValue::{Bool, Float, Int, String};
    use IrBinaryOp::{Add, Div, Eq, Greater, GreaterEq, Less, LessEq, Mod, Mul, NotEq, Sub};

    match (op, lhs, rhs) {
        (Add, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_add(*rhs as i32)))),
        (Sub, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_sub(*rhs as i32)))),
        (Mul, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_mul(*rhs as i32)))),
        (Div, Int(_), Int(0)) | (Mod, Int(_), Int(0)) => None,
        (Div, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_div(*rhs as i32)))),
        (Mod, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_rem(*rhs as i32)))),
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
        (Less, Float(lhs), Float(rhs)) => Some(Bool(lhs < rhs)),
        (LessEq, Float(lhs), Float(rhs)) => Some(Bool(lhs <= rhs)),
        (Greater, Float(lhs), Float(rhs)) => Some(Bool(lhs > rhs)),
        (GreaterEq, Float(lhs), Float(rhs)) => Some(Bool(lhs >= rhs)),
        (IrBinaryOp::And, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs && *rhs)),
        (IrBinaryOp::Or, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs || *rhs)),
        _ => None,
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

fn result_value(instruction: &Instruction) -> Option<ValueId> {
    match instruction {
        Instruction::Const { out, .. }
        | Instruction::Binary { out, .. }
        | Instruction::Struct { out, .. }
        | Instruction::FieldGet { out, .. } => Some(*out),
        Instruction::Call { out, .. } => *out,
        Instruction::Return { .. } => None,
    }
}

fn used_values(instruction: &Instruction) -> Vec<ValueId> {
    match instruction {
        Instruction::Const { .. } => Vec::new(),
        Instruction::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        Instruction::Struct { fields, .. } => fields.clone(),
        Instruction::FieldGet { aggregate, .. } => vec![*aggregate],
        Instruction::Call { args, .. } => args.clone(),
        Instruction::Return { value, .. } => value.iter().copied().collect(),
    }
}

fn rewrite_values(instruction: &mut Instruction, aliases: &AHashMap<ValueId, ValueId>) {
    match instruction {
        Instruction::Const { .. } => {}
        Instruction::Binary { lhs, rhs, .. } => {
            *lhs = resolve_alias(*lhs, aliases);
            *rhs = resolve_alias(*rhs, aliases);
        }
        Instruction::Struct { fields, .. } => {
            for field in fields {
                *field = resolve_alias(*field, aliases);
            }
        }
        Instruction::FieldGet { aggregate, .. } => {
            *aggregate = resolve_alias(*aggregate, aliases);
        }
        Instruction::Call { args, .. } => {
            for arg in args {
                *arg = resolve_alias(*arg, aliases);
            }
        }
        Instruction::Return { value, .. } => {
            if let Some(value) = value {
                *value = resolve_alias(*value, aliases);
            }
        }
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

fn promote_function_locals(function: &mut ControlFlowFunction<'_>) -> Result<(), SsaError> {
    let block_count = function.blocks.len();
    if block_count == 0 || function.locals.is_empty() {
        return Ok(());
    }
    let entry = function.entry.0 as usize;
    let predecessors = cfg_predecessors(function);
    let reachable = reachable_blocks(function);
    let dominators = compute_dominators(entry, &predecessors, &reachable);
    let immediate_dominators = compute_immediate_dominators(entry, &dominators, &reachable);
    let dominance_frontiers =
        compute_dominance_frontiers(&predecessors, &immediate_dominators, &reachable);

    let local_count = function.locals.len();
    let live_in = local_live_in(function, local_count);
    let mut def_blocks = vec![AHashSet::<usize>::new(); local_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for instruction in &block.instructions {
            if let ControlFlowOp::StoreLocal { local, .. } = instruction.op {
                def_blocks[local.0 as usize].insert(block_index);
            }
        }
    }

    let mut has_phi = vec![AHashSet::<usize>::new(); local_count];
    for local_index in 0..local_count {
        let mut work = def_blocks[local_index].iter().copied().collect::<Vec<_>>();
        work.sort_unstable();
        while let Some(block_index) = work.pop() {
            let mut frontier = dominance_frontiers[block_index]
                .iter()
                .copied()
                .collect::<Vec<_>>();
            frontier.sort_unstable();
            for target in frontier {
                if !live_in[target].contains(&local_index) {
                    continue;
                }
                if !has_phi[local_index].insert(target) {
                    continue;
                }
                let out = ValueId(function.value_count);
                function.value_count += 1;
                function.value_escapes.push(EscapeState::LocalOnly);
                let local = &function.locals[local_index];
                let span = function.blocks[target].span;
                function.blocks[target].phis.push(Phi {
                    out,
                    local: LocalId(local_index as u32),
                    ty: local.ty.clone(),
                    incoming: Vec::new(),
                    span,
                });
                if !def_blocks[local_index].contains(&target) {
                    work.push(target);
                }
            }
        }
    }

    for block in &mut function.blocks {
        block.phis.sort_by_key(|phi| phi.local.0);
    }

    let mut dominator_children = vec![Vec::new(); block_count];
    for (block, idom) in immediate_dominators.iter().enumerate() {
        if let Some(idom) = idom {
            dominator_children[*idom].push(block);
        }
    }
    for children in &mut dominator_children {
        children.sort_unstable();
    }

    let mut stacks = vec![Vec::<ValueId>::new(); local_count];
    let mut aliases = AHashMap::<ValueId, ValueId>::new();
    rename_block(
        entry,
        function,
        &dominator_children,
        &mut stacks,
        &mut aliases,
    )?;

    eliminate_trivial_phis(function, &mut aliases);
    rewrite_control_flow_function(function, &aliases);

    if function.blocks.iter().any(|block| {
        block.instructions.iter().any(|instruction| {
            matches!(
                instruction.op,
                ControlFlowOp::LoadLocal(_) | ControlFlowOp::StoreLocal { .. }
            )
        })
    }) {
        return Err(SsaError {
            message: format!(
                "local promotion left memory operations in function {:?}",
                function.id
            ),
        });
    }
    Ok(())
}

fn local_live_in(function: &ControlFlowFunction<'_>, local_count: usize) -> Vec<AHashSet<usize>> {
    let block_count = function.blocks.len();
    let mut uses = vec![AHashSet::<usize>::new(); block_count];
    let mut definitions = vec![AHashSet::<usize>::new(); block_count];
    for (block_index, block) in function.blocks.iter().enumerate() {
        for instruction in &block.instructions {
            match instruction.op {
                ControlFlowOp::LoadLocal(local) => {
                    let local = local.0 as usize;
                    if !definitions[block_index].contains(&local) {
                        uses[block_index].insert(local);
                    }
                }
                ControlFlowOp::StoreLocal { local, .. } => {
                    definitions[block_index].insert(local.0 as usize);
                }
                _ => {}
            }
        }
    }

    let mut live_in = vec![AHashSet::<usize>::new(); block_count];
    let mut live_out = vec![AHashSet::<usize>::new(); block_count];
    loop {
        let mut changed = false;
        for (block_index, block) in function.blocks.iter().enumerate().rev() {
            let mut out = AHashSet::with_capacity(local_count);
            for successor in terminator_successors(block.terminator.as_ref()) {
                out.extend(live_in[successor].iter().copied());
            }
            let mut input = uses[block_index].clone();
            input.extend(out.difference(&definitions[block_index]).copied());
            if out != live_out[block_index] {
                live_out[block_index] = out;
                changed = true;
            }
            if input != live_in[block_index] {
                live_in[block_index] = input;
                changed = true;
            }
        }
        if !changed {
            return live_in;
        }
    }
}

fn cfg_predecessors(function: &ControlFlowFunction<'_>) -> Vec<Vec<usize>> {
    let mut predecessors = vec![Vec::new(); function.blocks.len()];
    for (index, block) in function.blocks.iter().enumerate() {
        for successor in terminator_successors(block.terminator.as_ref()) {
            predecessors[successor].push(index);
        }
    }
    for incoming in &mut predecessors {
        incoming.sort_unstable();
        incoming.dedup();
    }
    predecessors
}

fn reachable_blocks(function: &ControlFlowFunction<'_>) -> AHashSet<usize> {
    let mut reachable = AHashSet::new();
    let mut work = vec![function.entry.0 as usize];
    while let Some(block) = work.pop() {
        if !reachable.insert(block) {
            continue;
        }
        work.extend(terminator_successors(
            function.blocks[block].terminator.as_ref(),
        ));
    }
    reachable
}

fn terminator_successors(terminator: Option<&Terminator>) -> Vec<usize> {
    match terminator {
        Some(Terminator::Jump(block)) => vec![block.0 as usize],
        Some(Terminator::Branch {
            then_block,
            else_block,
            ..
        }) => vec![then_block.0 as usize, else_block.0 as usize],
        _ => Vec::new(),
    }
}

fn compute_dominators(
    entry: usize,
    predecessors: &[Vec<usize>],
    reachable: &AHashSet<usize>,
) -> Vec<AHashSet<usize>> {
    let all = reachable.iter().copied().collect::<AHashSet<_>>();
    let mut dominators = (0..predecessors.len())
        .map(|block| {
            if block == entry || !reachable.contains(&block) {
                AHashSet::from_iter([block])
            } else {
                all.clone()
            }
        })
        .collect::<Vec<_>>();

    let mut changed = true;
    while changed {
        changed = false;
        for block in 0..predecessors.len() {
            if block == entry || !reachable.contains(&block) {
                continue;
            }
            let mut incoming = predecessors[block]
                .iter()
                .filter(|pred| reachable.contains(pred));
            let mut next = incoming
                .next()
                .map(|pred| dominators[*pred].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next.retain(|dominator| dominators[*predecessor].contains(dominator));
            }
            next.insert(block);
            if next != dominators[block] {
                dominators[block] = next;
                changed = true;
            }
        }
    }
    dominators
}

fn compute_immediate_dominators(
    entry: usize,
    dominators: &[AHashSet<usize>],
    reachable: &AHashSet<usize>,
) -> Vec<Option<usize>> {
    let mut result = vec![None; dominators.len()];
    for block in 0..dominators.len() {
        if block == entry || !reachable.contains(&block) {
            continue;
        }
        result[block] = dominators[block]
            .iter()
            .copied()
            .filter(|candidate| *candidate != block)
            .max_by_key(|candidate| dominators[*candidate].len());
    }
    result
}

fn compute_dominance_frontiers(
    predecessors: &[Vec<usize>],
    immediate_dominators: &[Option<usize>],
    reachable: &AHashSet<usize>,
) -> Vec<AHashSet<usize>> {
    let mut frontiers = vec![AHashSet::new(); predecessors.len()];
    for block in 0..predecessors.len() {
        let incoming = predecessors[block]
            .iter()
            .filter(|pred| reachable.contains(pred))
            .copied()
            .collect::<Vec<_>>();
        if incoming.len() < 2 {
            continue;
        }
        let Some(stop) = immediate_dominators[block] else {
            continue;
        };
        for predecessor in incoming {
            let mut runner = predecessor;
            while runner != stop {
                frontiers[runner].insert(block);
                let Some(next) = immediate_dominators[runner] else {
                    break;
                };
                runner = next;
            }
        }
    }
    frontiers
}

fn rename_block(
    block_index: usize,
    function: &mut ControlFlowFunction<'_>,
    dominator_children: &[Vec<usize>],
    stacks: &mut [Vec<ValueId>],
    aliases: &mut AHashMap<ValueId, ValueId>,
) -> Result<(), SsaError> {
    let mut pushes = vec![0usize; stacks.len()];

    let phi_defs = function.blocks[block_index]
        .phis
        .iter()
        .filter(|phi| phi.local != LocalId(u32::MAX))
        .map(|phi| (phi.local, phi.out))
        .collect::<Vec<_>>();
    for (local, out) in phi_defs {
        stacks[local.0 as usize].push(out);
        pushes[local.0 as usize] += 1;
    }

    let instructions = std::mem::take(&mut function.blocks[block_index].instructions);
    let mut retained = Vec::with_capacity(instructions.len());
    for mut instruction in instructions {
        match instruction.op {
            ControlFlowOp::LoadLocal(local) => {
                let out = instruction.out.ok_or_else(|| SsaError {
                    message: "local load has no result value".to_string(),
                })?;
                let value = stacks[local.0 as usize]
                    .last()
                    .copied()
                    .ok_or_else(|| SsaError {
                        message: format!("local {:?} is read before definition", local),
                    })?;
                aliases.insert(out, resolve_alias(value, aliases));
            }
            ControlFlowOp::StoreLocal { local, value } => {
                let value = resolve_alias(value, aliases);
                stacks[local.0 as usize].push(value);
                pushes[local.0 as usize] += 1;
            }
            _ => {
                rewrite_control_flow_op(&mut instruction.op, aliases);
                retained.push(instruction);
            }
        }
    }
    function.blocks[block_index].instructions = retained;
    if let Some(terminator) = &mut function.blocks[block_index].terminator {
        rewrite_terminator(terminator, aliases);
    }

    let successors = terminator_successors(function.blocks[block_index].terminator.as_ref());
    for successor in successors {
        for phi in &mut function.blocks[successor].phis {
            if phi.local == LocalId(u32::MAX) {
                continue;
            }
            let value = stacks[phi.local.0 as usize]
                .last()
                .copied()
                .ok_or_else(|| SsaError {
                    message: format!(
                        "local {:?} has no reaching definition for block {:?}",
                        phi.local, phi.out
                    ),
                })?;
            phi.incoming
                .push((BlockId(block_index as u32), resolve_alias(value, aliases)));
        }
    }

    for child in &dominator_children[block_index] {
        rename_block(*child, function, dominator_children, stacks, aliases)?;
    }

    for (local, count) in pushes.into_iter().enumerate() {
        let len = stacks[local].len();
        stacks[local].truncate(len - count);
    }
    Ok(())
}

fn eliminate_trivial_phis(
    function: &mut ControlFlowFunction<'_>,
    aliases: &mut AHashMap<ValueId, ValueId>,
) {
    let mut changed = true;
    while changed {
        changed = false;
        for block in &mut function.blocks {
            block.phis.retain(|phi| {
                let mut values = phi
                    .incoming
                    .iter()
                    .map(|(_, value)| resolve_alias(*value, aliases))
                    .filter(|value| *value != phi.out);
                let Some(first) = values.next() else {
                    return true;
                };
                if values.all(|value| value == first) {
                    aliases.insert(phi.out, first);
                    changed = true;
                    false
                } else {
                    true
                }
            });
        }
    }
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
        ControlFlowOp::Const(_) | ControlFlowOp::LoadLocal(_) | ControlFlowOp::LoadGlobal(_) => {}
        ControlFlowOp::Unary { value, .. } => rewrite(value),
        ControlFlowOp::Binary { lhs, rhs, .. } => {
            rewrite(lhs);
            rewrite(rhs);
        }
        ControlFlowOp::Array(values) => values.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::Struct { fields, .. } => fields.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::NewClass { args, .. } => args.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::Closure { captures, .. } => captures.iter_mut().for_each(&mut rewrite),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            rewrite(value)
        }
        ControlFlowOp::FieldGet { object, .. } => rewrite(object),
        ControlFlowOp::FieldSet { object, value, .. } => {
            rewrite(object);
            rewrite(value);
        }
        ControlFlowOp::IndexGet { object, index } => {
            rewrite(object);
            rewrite(index);
        }
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
        ControlFlowOp::CallMethod { receiver, args, .. } => {
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
        Terminator::Return(Some(value)) => {
            *value = resolve_alias(*value, aliases);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::ir::{BasicBlock, IrFunction};
    use crate::span::Span;
    use crate::{analyze, lower_to_control_flow, parse_source};

    const S: Span = Span::new(0, 0);

    fn module(instructions: Vec<Instruction>) -> IrModule {
        IrModule {
            functions: vec![IrFunction {
                id: None,
                blocks: vec![BasicBlock {
                    id: None,
                    instructions,
                }],
            }],
        }
    }

    #[test]
    fn folds_constants() {
        let mut module = module(vec![
            Instruction::Const {
                out: ValueId(0),
                value: ConstValue::Int(1),
                span: S,
            },
            Instruction::Const {
                out: ValueId(1),
                value: ConstValue::Int(6),
                span: S,
            },
            Instruction::Binary {
                out: ValueId(2),
                op: IrBinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
                span: S,
            },
            Instruction::Return {
                value: Some(ValueId(2)),
                span: S,
            },
        ]);

        let report = ConstantFolding.run(&mut module);
        assert!(report.changed);
        assert!(matches!(
            &module.functions[0].blocks[0].instructions[2],
            Instruction::Const {
                value: ConstValue::Int(7),
                ..
            }
        ));
    }

    #[test]
    fn removes_dead_value_chains() {
        let mut module = module(vec![
            Instruction::Const {
                out: ValueId(0),
                value: ConstValue::Int(1),
                span: S,
            },
            Instruction::Const {
                out: ValueId(1),
                value: ConstValue::Int(2),
                span: S,
            },
            Instruction::Binary {
                out: ValueId(2),
                op: IrBinaryOp::Add,
                lhs: ValueId(0),
                rhs: ValueId(1),
                span: S,
            },
            Instruction::Return {
                value: None,
                span: S,
            },
        ]);

        let report = DeadCodeElimination.run(&mut module);
        assert!(report.changed);
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 1);
    }

    #[test]
    fn scalar_replaces_non_escaping_structs() {
        let mut module = module(vec![
            Instruction::Const {
                out: ValueId(0),
                value: ConstValue::Int(10),
                span: S,
            },
            Instruction::Const {
                out: ValueId(1),
                value: ConstValue::Int(20),
                span: S,
            },
            Instruction::Struct {
                out: ValueId(2),
                fields: vec![ValueId(0), ValueId(1)],
                span: S,
            },
            Instruction::FieldGet {
                out: ValueId(3),
                aggregate: ValueId(2),
                index: 1,
                span: S,
            },
            Instruction::Return {
                value: Some(ValueId(3)),
                span: S,
            },
        ]);

        let report = ScalarReplacement.run(&mut module);
        assert!(report.changed);
        let instructions = &module.functions[0].blocks[0].instructions;
        assert_eq!(instructions.len(), 3);
        assert!(matches!(
            instructions.last(),
            Some(Instruction::Return {
                value: Some(ValueId(1)),
                ..
            })
        ));
    }

    #[test]
    fn preserves_escaping_structs() {
        let mut module = module(vec![
            Instruction::Struct {
                out: ValueId(0),
                fields: Vec::new(),
                span: S,
            },
            Instruction::Return {
                value: Some(ValueId(0)),
                span: S,
            },
        ]);

        let report = ScalarReplacement.run(&mut module);
        assert!(!report.changed);
        assert_eq!(module.functions[0].blocks[0].instructions.len(), 2);
    }

    #[test]
    fn promotes_cfg_locals_and_inserts_loop_phis() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int total=0;int run(int limit){int sum=0;for(int i=0;i<limit;i++){sum+=i;}return sum;}total=run(4);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let report = promote_locals_to_ssa(&mut module).unwrap();
        assert!(report.changed);
        assert!(module
            .functions
            .iter()
            .all(|function| function.locals_promoted));
        assert!(module.functions.iter().all(|function| function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadLocal(_) | ControlFlowOp::StoreLocal { .. }
            ))));
        assert!(module.functions[1]
            .blocks
            .iter()
            .any(|block| !block.phis.is_empty()));
    }

    #[test]
    fn runs_the_whole_cfg_optimization_pipeline() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int x;int y;}int sum(){Point p=Point{1,2};return p.x+p.y;}int dead(){return 99;}if(true){print(sum());}else{print(0);}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();

        assert!(reports
            .iter()
            .any(|report| report.pass_name == "inlining" && report.changed));
        assert!(reports.iter().any(|report| {
            report.pass_name == "unreachable-block-elimination" && report.changed
        }));
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "scalar-replacement-cfg" && report.changed }));
        assert!(!module.functions[1].live);
        assert!(!module.functions[2].live);
    }

    #[test]
    fn devirtualizes_class_method_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Value{int x;init(int x){this.x=x;}int get(){return this.x;}}Value v=new Value(7);print(v.get());",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        assert!(module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .all(|block| block
                .instructions
                .iter()
                .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallMethod { .. }))));
    }

    #[test]
    fn marks_extern_arguments_as_untyped_escapes() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Point{int x;int y;}extern void consume(Point p);Point p=Point{1,2};consume(p);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];
        let aggregate = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .find_map(|instruction| match instruction.op {
                ControlFlowOp::Struct { .. } => instruction.out,
                _ => None,
            })
            .unwrap();
        assert_eq!(
            entry.value_escapes[aggregate.0 as usize],
            EscapeState::EscapesToUntypedBoundary
        );
    }

    #[test]
    fn pruned_ssa_handles_locals_scoped_inside_nested_loops() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int total=0;for(int outer=0;outer<3;outer++){int inner=0;while(inner<3){total+=outer*inner;inner++;}}print(total);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        assert!(module.functions.iter().all(|function| function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadLocal(_) | ControlFlowOp::StoreLocal { .. }
            ))));
    }

    #[test]
    fn eliminates_repeated_ssa_expressions_with_value_numbering() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int value=read();print(value*7+value*7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();
        let multiplications = module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| {
                matches!(
                    instruction.op,
                    ControlFlowOp::Binary {
                        op: IrBinaryOp::Mul,
                        ..
                    }
                )
            })
            .count();
        assert_eq!(multiplications, 1);
        assert!(reports
            .iter()
            .any(|report| report.pass_name == "local-value-numbering" && report.changed));
    }

    #[test]
    fn propagates_single_assignment_globals_into_functions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int factor=6;int scale(int value){return value*factor;}print(scale(7));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();
        assert!(module.globals.is_empty());
        assert!(module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::LoadGlobal(_) | ControlFlowOp::StoreGlobal { .. }
            )));
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "global-constant-propagation" && report.changed }));
    }

    #[test]
    fn does_not_inline_recursive_functions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int factorial(int value){if(value<=1){return 1;}return value*factorial(value-1);}print(factorial(5));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        assert!(module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. })));
    }

    #[test]
    fn removes_field_stores_overwritten_before_observation() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Box{int value;}extern void consume(Box box);Box box=new Box();box.value=1;box.value=2;consume(box);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let reports = optimize_control_flow(&mut module).unwrap();
        let stores = module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::FieldSet { .. }))
            .count();
        assert_eq!(stores, 1);
        assert!(reports.iter().any(|report| {
            report.pass_name == "dead-field-store-elimination" && report.changed
        }));
    }
}
