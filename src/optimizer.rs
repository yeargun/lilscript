use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use ahash::{AHashMap, AHashSet};

use crate::ir::{
    BlockId, ConstValue, ControlFlowFunction, ControlFlowInstruction, ControlFlowModule,
    ControlFlowOp, ExportBinding, FunctionId, FunctionKind, Instruction, Intrinsic, IrBinaryOp,
    IrLocal, IrModule, IrUnaryOp, LocalId, Phi, TemplateOperand, Terminator, ValueId,
};
use crate::profile::OptimizationProfile;
use crate::semantic::{EscapeState, SymbolId, Type};
use crate::span::Span;
use crate::value_analysis::analyze_finite_values;

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct OptimizationReport {
    pub pass_name: &'static str,
    pub changed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OptimizationOptions {
    pub constant_folding: bool,
    pub algebraic_simplification: bool,
    pub common_subexpression_elimination: bool,
    pub finite_value_propagation: bool,
    pub global_optimization: bool,
    pub inlining: bool,
    pub inline_closure_factories: bool,
    pub scalar_replacement: bool,
    pub dead_store_elimination: bool,
    pub dead_code_elimination: bool,
    pub specialize_tagged_constants: bool,
    pub call_site_specialization: bool,
    pub capture_signature_cloning: bool,
    pub identical_function_folding: bool,
    pub inline_instruction_limit: usize,
    pub inline_control_flow_limit: usize,
    pub inline_growth_limit: Option<usize>,
}

impl Default for OptimizationOptions {
    fn default() -> Self {
        Self {
            constant_folding: true,
            algebraic_simplification: true,
            common_subexpression_elimination: true,
            finite_value_propagation: true,
            global_optimization: true,
            inlining: true,
            inline_closure_factories: true,
            scalar_replacement: true,
            dead_store_elimination: true,
            dead_code_elimination: true,
            specialize_tagged_constants: false,
            call_site_specialization: true,
            capture_signature_cloning: true,
            identical_function_folding: true,
            inline_instruction_limit: 12,
            inline_control_flow_limit: 30,
            inline_growth_limit: None,
        }
    }
}

impl OptimizationOptions {
    pub const fn disabled() -> Self {
        Self {
            constant_folding: false,
            algebraic_simplification: false,
            common_subexpression_elimination: false,
            finite_value_propagation: false,
            global_optimization: false,
            inlining: false,
            inline_closure_factories: false,
            scalar_replacement: false,
            dead_store_elimination: false,
            dead_code_elimination: false,
            specialize_tagged_constants: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            identical_function_folding: false,
            inline_instruction_limit: 0,
            inline_control_flow_limit: 0,
            inline_growth_limit: Some(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptimizationGuidance {
    pub profile: OptimizationProfile,
    pub specialization_min_count: u64,
    pub max_specializations_per_function: usize,
    pub max_clone_instructions: usize,
}

impl Default for OptimizationGuidance {
    fn default() -> Self {
        Self {
            profile: OptimizationProfile::default(),
            specialization_min_count: 100,
            max_specializations_per_function: 8,
            max_clone_instructions: 64,
        }
    }
}

pub trait OptimizationPass {
    fn name(&self) -> &'static str;
    fn run(&mut self, module: &mut IrModule) -> OptimizationReport;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SsaError {
    pub span: Span,
    pub message: String,
}

impl std::fmt::Display for SsaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at byte range {}..{}",
            self.message, self.span.start, self.span.end
        )
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
    module.exports.clear();
    optimize_control_flow_inner(
        module,
        &OptimizationOptions::default(),
        &OptimizationGuidance::default(),
    )
}

pub fn optimize_control_flow_for_module(
    module: &mut ControlFlowModule<'_>,
) -> Result<Vec<OptimizationReport>, SsaError> {
    optimize_control_flow_inner(
        module,
        &OptimizationOptions::default(),
        &OptimizationGuidance::default(),
    )
}

pub fn optimize_control_flow_with_options(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    preserve_exports: bool,
) -> Result<Vec<OptimizationReport>, SsaError> {
    optimize_control_flow_with_guidance(
        module,
        options,
        preserve_exports,
        &OptimizationGuidance::default(),
    )
}

pub fn optimize_control_flow_with_guidance(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    preserve_exports: bool,
    guidance: &OptimizationGuidance,
) -> Result<Vec<OptimizationReport>, SsaError> {
    if !preserve_exports {
        module.exports.clear();
    }
    optimize_control_flow_inner(module, options, guidance)
}

fn optimize_control_flow_inner(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    guidance: &OptimizationGuidance,
) -> Result<Vec<OptimizationReport>, SsaError> {
    let mut reports = Vec::new();
    if options.global_optimization {
        reports.push(internalize_entry_globals(module));
    }
    if options.dead_code_elimination {
        reports.push(eliminate_unread_globals(module));
    }
    reports.push(promote_locals_to_ssa(module)?);
    optimize_scalar_fixed_point(module, options, &mut reports);

    if options.global_optimization {
        reports.push(propagate_single_assignment_globals(module));
    }
    reports.push(devirtualize_methods(module));
    reports.push(devirtualize_known_closure_calls(module));
    reports.push(specialize_constant_parameters(
        module,
        options.specialize_tagged_constants,
        options.finite_value_propagation,
    ));
    if options.call_site_specialization {
        reports.push(specialize_profiled_call_sites(
            module,
            options.specialize_tagged_constants,
            guidance,
        ));
    }
    if options.capture_signature_cloning {
        reports.push(clone_constant_capture_signatures(module, guidance));
    }
    reports.push(devirtualize_known_closure_calls(module));
    reports.push(optimize_unused_parameters(module));
    reports.push(optimize_unused_returns(module));
    reports.push(validate_declared_purity(module)?);
    if options.inlining {
        loop {
            let inlining = inline_small_functions(module, options);
            let cfg_inlining =
                inline_single_use_control_flow_function(module, options.inline_control_flow_limit);
            let changed = inlining.changed || cfg_inlining.changed;
            reports.push(inlining);
            reports.push(cfg_inlining);
            if !changed {
                break;
            }
        }
        reports.push(specialize_constant_parameters(
            module,
            options.specialize_tagged_constants,
            options.finite_value_propagation,
        ));
        if options.capture_signature_cloning {
            reports.push(clone_constant_capture_signatures(module, guidance));
            reports.push(devirtualize_known_closure_calls(module));
        }
        reports.push(optimize_unused_parameters(module));
        reports.push(optimize_unused_returns(module));
    }

    optimize_scalar_fixed_point(module, options, &mut reports);

    reports.push(analyze_escapes(module));
    if options.scalar_replacement {
        reports.push(scalar_replace_linear_classes(module));
        reports.push(scalar_replace_control_flow_aggregates(module));
    }
    if options.dead_store_elimination {
        reports.push(eliminate_overwritten_field_stores(module));
    }

    optimize_scalar_fixed_point(module, options, &mut reports);
    if options.dead_code_elimination {
        reports.push(eliminate_dead_control_flow_instructions(module));
    }
    reports.push(optimize_unused_parameters(module));
    reports.push(optimize_unused_returns(module));
    optimize_scalar_fixed_point(module, options, &mut reports);

    if options.identical_function_folding {
        reports.push(fold_identical_private_functions(module));
    }

    if options.dead_code_elimination {
        reports.push(eliminate_dead_control_flow_instructions(module));
        reports.push(eliminate_dead_functions(module));
    }
    Ok(reports)
}

fn optimize_scalar_fixed_point(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
    reports: &mut Vec<OptimizationReport>,
) {
    loop {
        let propagation = options
            .constant_folding
            .then(|| fold_and_propagate_control_flow(module, options.finite_value_propagation));
        let phis = eliminate_redundant_phis(module);
        let algebraic = options
            .algebraic_simplification
            .then(|| simplify_algebraic_expressions(module));
        let value_numbering = options
            .common_subexpression_elimination
            .then(|| eliminate_common_subexpressions(module));
        let unreachable = remove_unreachable_control_flow(module);
        let changed = propagation.as_ref().is_some_and(|report| report.changed)
            || phis.changed
            || algebraic.as_ref().is_some_and(|report| report.changed)
            || value_numbering
                .as_ref()
                .is_some_and(|report| report.changed)
            || unreachable.changed;
        reports.extend(propagation);
        reports.push(phis);
        reports.extend(algebraic);
        reports.extend(value_numbering);
        reports.push(unreachable);
        if !changed {
            break;
        }
    }
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
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::HostCall { .. }
            | ControlFlowOp::Intrinsic { .. }
            | ControlFlowOp::Closure { .. }
    )
}

fn eliminate_unread_globals(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut loaded = module
        .functions
        .iter()
        .flat_map(|function| &function.blocks)
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::LoadGlobal(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    loaded.extend(
        module
            .exports
            .iter()
            .filter_map(|export| match export.binding {
                ExportBinding::Global(symbol) => Some(symbol),
                _ => None,
            }),
    );
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
    let exported = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Global(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
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
        .filter_map(|(symbol, values)| {
            if exported.contains(&symbol) {
                None
            } else {
                match values.as_slice() {
                    [Some(value)] => Some((symbol, value.clone())),
                    _ => None,
                }
            }
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
                    ControlFlowOp::TypeCheck { value, ref target } => {
                        if let Some(source) = value_types.get(&value) {
                            if !matches!(source, Type::Union(_) | Type::Nullable(_)) {
                                replacement = Some(ConstValue::Bool(source == target));
                            }
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
                                IrBinaryOp::Xor if is_int(rhs_const, 0) => Some(lhs),
                                IrBinaryOp::Xor if is_int(lhs_const, 0) => Some(rhs),
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
                                            Type::Int
                                                | Type::Bool
                                                | Type::String
                                                | Type::Class(_)
                                                | Type::ClassInstance { .. }
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
    Null,
}

impl From<&ConstValue> for ConstantNumber {
    fn from(value: &ConstValue) -> Self {
        match value {
            ConstValue::Int(value) => Self::Int(*value),
            ConstValue::Float(value) => Self::Float(value.to_bits()),
            ConstValue::Bool(value) => Self::Bool(*value),
            ConstValue::String(value) => Self::String(value.clone()),
            ConstValue::Null => Self::Null,
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
            | IrBinaryOp::BitAnd
            | IrBinaryOp::BitOr
            | IrBinaryOp::Eq
            | IrBinaryOp::NotEq
            | IrBinaryOp::Xor
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
    let exported = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Global(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
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
            !global.external
                && loaded_by_entry.contains(&global.symbol)
                && !shared.contains(&global.symbol)
                && !exported.contains(&global.symbol)
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

fn devirtualize_known_closure_calls(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut changed = false;
    let direct_targets = module
        .functions
        .iter()
        .filter(|function| function.kind != FunctionKind::Closure)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    for function in &mut module.functions {
        let closures = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Closure { function, captures })
                    if direct_targets.contains(function) =>
                {
                    Some((out, (*function, captures.clone())))
                }
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in function
            .blocks
            .iter_mut()
            .flat_map(|block| &mut block.instructions)
        {
            let replacement = match &instruction.op {
                ControlFlowOp::CallValue { callee, args } => {
                    closures.get(callee).map(|(target, captures)| {
                        let mut direct_args = captures.clone();
                        direct_args.extend(args);
                        ControlFlowOp::CallDirect {
                            function: *target,
                            args: direct_args,
                        }
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
    OptimizationReport {
        pass_name: "closure-call-devirtualization",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum SpecializationValue {
    Constant(ConstantKey),
    Function(FunctionId),
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
enum ConstantKey {
    Int(i64),
    Float(u64),
    Bool(bool),
    String(String),
    Null,
}

impl ConstantKey {
    fn from_value(value: &ConstValue) -> Self {
        match value {
            ConstValue::Int(value) => Self::Int(*value),
            ConstValue::Float(value) => Self::Float(value.to_bits()),
            ConstValue::Bool(value) => Self::Bool(*value),
            ConstValue::String(value) => Self::String(value.clone()),
            ConstValue::Null => Self::Null,
        }
    }

    fn to_value(&self) -> ConstValue {
        match self {
            Self::Int(value) => ConstValue::Int(*value),
            Self::Float(value) => ConstValue::Float(f64::from_bits(*value)),
            Self::Bool(value) => ConstValue::Bool(*value),
            Self::String(value) => ConstValue::String(value.clone()),
            Self::Null => ConstValue::Null,
        }
    }
}

#[derive(Debug, Clone)]
struct ProfiledCallSite {
    caller: usize,
    block: usize,
    instruction: usize,
    frequency: u64,
}

#[derive(Debug, Clone)]
struct ProfiledSpecialization {
    callee: FunctionId,
    signature: Vec<(usize, SpecializationValue)>,
    sites: Vec<ProfiledCallSite>,
}

fn specialize_profiled_call_sites(
    module: &mut ControlFlowModule<'_>,
    specialize_tagged_constants: bool,
    guidance: &OptimizationGuidance,
) -> OptimizationReport {
    let mut groups =
        AHashMap::<(FunctionId, Vec<(usize, SpecializationValue)>), Vec<ProfiledCallSite>>::new();
    for (caller_index, caller) in module.functions.iter().enumerate() {
        let definitions = specialization_definitions(caller);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let ControlFlowOp::CallDirect { function, args } = &instruction.op else {
                    continue;
                };
                if *function == caller.id {
                    continue;
                }
                let Some(callee) = module.functions.get(function.0 as usize) else {
                    continue;
                };
                if !callee.live
                    || !matches!(
                        callee.kind,
                        FunctionKind::Function | FunctionKind::Method { .. }
                    )
                {
                    continue;
                }
                let signature = callee
                    .params
                    .iter()
                    .zip(args)
                    .enumerate()
                    .filter_map(|(index, (parameter, argument))| {
                        let value = definitions.get(argument)?;
                        match value {
                            SpecializationValue::Function(_)
                                if matches!(parameter.ty, Type::Function(_)) =>
                            {
                                Some((index, value.clone()))
                            }
                            SpecializationValue::Constant(key)
                                if specialize_tagged_constants
                                    || constant_has_direct_native_representation(
                                        &key.to_value(),
                                        &parameter.ty,
                                    ) =>
                            {
                                Some((index, value.clone()))
                            }
                            _ => None,
                        }
                    })
                    .collect::<Vec<_>>();
                if signature.is_empty() {
                    continue;
                }
                groups
                    .entry((*function, signature))
                    .or_default()
                    .push(ProfiledCallSite {
                        caller: caller_index,
                        block: block_index,
                        instruction: instruction_index,
                        frequency: guidance.profile.block_count(caller, block.id),
                    });
            }
        }
    }

    let mut candidates = groups
        .into_iter()
        .filter_map(|((callee, signature), sites)| {
            let function = &module.functions[callee.0 as usize];
            let instructions = function_instruction_count(function);
            let frequency = sites
                .iter()
                .map(|site| site.frequency)
                .fold(0u64, u64::saturating_add);
            let call_savings = sites
                .len()
                .saturating_mul(signature.len())
                .saturating_mul(3);
            let clone_cost = instructions
                .saturating_mul(4)
                .saturating_add(function.blocks.len().saturating_mul(3));
            (instructions <= guidance.max_clone_instructions
                && (frequency >= guidance.specialization_min_count || call_savings > clone_cost))
                .then_some((
                    frequency,
                    ProfiledSpecialization {
                        callee,
                        signature,
                        sites,
                    },
                ))
        })
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| left.1.callee.0.cmp(&right.1.callee.0))
            .then_with(|| left.1.signature.cmp(&right.1.signature))
    });

    let mut per_function = AHashMap::<FunctionId, usize>::new();
    let mut changed = false;
    for (_, candidate) in candidates {
        let count = per_function.entry(candidate.callee).or_default();
        if *count >= guidance.max_specializations_per_function {
            continue;
        }
        *count += 1;
        let new_id = FunctionId(module.functions.len() as u32);
        let clone = clone_function_with_specialization(
            &module.functions[candidate.callee.0 as usize],
            new_id,
            &candidate.signature,
        );
        module.functions.push(clone);
        for site in candidate.sites {
            let instruction = &mut module.functions[site.caller].blocks[site.block].instructions
                [site.instruction];
            let ControlFlowOp::CallDirect { function, args } = &mut instruction.op else {
                continue;
            };
            *function = new_id;
            remove_specialized_arguments(args, &candidate.signature);
        }
        changed = true;
    }
    OptimizationReport {
        pass_name: "profiled-call-site-specialization",
        changed,
    }
}

fn clone_constant_capture_signatures(
    module: &mut ControlFlowModule<'_>,
    guidance: &OptimizationGuidance,
) -> OptimizationReport {
    let mut groups =
        AHashMap::<(FunctionId, Vec<(usize, SpecializationValue)>), Vec<ProfiledCallSite>>::new();
    for (caller_index, caller) in module.functions.iter().enumerate() {
        let definitions = specialization_definitions(caller);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            for (instruction_index, instruction) in block.instructions.iter().enumerate() {
                let ControlFlowOp::Closure { function, captures } = &instruction.op else {
                    continue;
                };
                if captures.is_empty() {
                    continue;
                }
                let Some(target) = module.functions.get(function.0 as usize) else {
                    continue;
                };
                if target.kind != FunctionKind::Closure || target.capture_count != captures.len() {
                    continue;
                }
                let signature = captures
                    .iter()
                    .enumerate()
                    .map(|(index, capture)| match definitions.get(capture) {
                        Some(SpecializationValue::Constant(value)) => {
                            Some((index, SpecializationValue::Constant(value.clone())))
                        }
                        _ => None,
                    })
                    .collect::<Option<Vec<_>>>();
                let Some(signature) = signature else {
                    continue;
                };
                groups
                    .entry((*function, signature))
                    .or_default()
                    .push(ProfiledCallSite {
                        caller: caller_index,
                        block: block_index,
                        instruction: instruction_index,
                        frequency: guidance.profile.block_count(caller, block.id),
                    });
            }
        }
    }

    let mut candidates = groups.into_iter().collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        let left_frequency = left.1.iter().map(|site| site.frequency).sum::<u64>();
        let right_frequency = right.1.iter().map(|site| site.frequency).sum::<u64>();
        right_frequency
            .cmp(&left_frequency)
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut per_function = AHashMap::<FunctionId, usize>::new();
    let mut changed = false;
    for ((target, signature), sites) in candidates {
        let frequency = sites.iter().map(|site| site.frequency).sum::<u64>();
        let function = &module.functions[target.0 as usize];
        let instructions = function_instruction_count(function);
        let allocation_savings = sites
            .len()
            .saturating_mul(signature.len())
            .saturating_mul(4);
        let clone_cost = instructions.saturating_mul(4);
        if instructions > guidance.max_clone_instructions
            || (frequency < guidance.specialization_min_count && allocation_savings <= clone_cost)
        {
            continue;
        }
        let count = per_function.entry(target).or_default();
        if *count >= guidance.max_specializations_per_function {
            continue;
        }
        *count += 1;
        let new_id = FunctionId(module.functions.len() as u32);
        let mut clone = clone_function_with_specialization(
            &module.functions[target.0 as usize],
            new_id,
            &signature,
        );
        clone.capture_count = clone.capture_count.saturating_sub(signature.len());
        module.functions.push(clone);
        for site in sites {
            let instruction = &mut module.functions[site.caller].blocks[site.block].instructions
                [site.instruction];
            let ControlFlowOp::Closure { function, captures } = &mut instruction.op else {
                continue;
            };
            *function = new_id;
            remove_specialized_arguments(captures, &signature);
        }
        changed = true;
    }
    OptimizationReport {
        pass_name: "capture-signature-cloning",
        changed,
    }
}

fn specialization_definitions(
    function: &ControlFlowFunction<'_>,
) -> AHashMap<ValueId, SpecializationValue> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Const(value)) => Some((
                out,
                SpecializationValue::Constant(ConstantKey::from_value(value)),
            )),
            (Some(out), ControlFlowOp::Closure { function, captures }) if captures.is_empty() => {
                Some((out, SpecializationValue::Function(*function)))
            }
            _ => None,
        })
        .collect()
}

fn clone_function_with_specialization<'src>(
    original: &ControlFlowFunction<'src>,
    id: FunctionId,
    signature: &[(usize, SpecializationValue)],
) -> ControlFlowFunction<'src> {
    let mut clone = original.clone();
    clone.id = id;
    let mut replacements = AHashMap::new();
    let mut constants = Vec::new();
    for (index, value) in signature {
        let parameter = &clone.params[*index];
        let out = ValueId(clone.value_count);
        clone.value_count += 1;
        clone.value_escapes.push(EscapeState::LocalOnly);
        replacements.insert(parameter.value, out);
        let operation = match value {
            SpecializationValue::Constant(value) => ControlFlowOp::Const(value.to_value()),
            SpecializationValue::Function(function) => ControlFlowOp::Closure {
                function: *function,
                captures: Vec::new(),
            },
        };
        constants.push(ControlFlowInstruction {
            out: Some(out),
            ty: Some(parameter.ty.clone()),
            op: operation,
            span: parameter.span,
        });
    }
    rewrite_control_flow_function(&mut clone, &replacements);
    clone.blocks[clone.entry.0 as usize]
        .instructions
        .splice(0..0, constants);
    clone.params = clone
        .params
        .drain(..)
        .enumerate()
        .filter_map(|(index, parameter)| {
            (!signature.iter().any(|(removed, _)| *removed == index)).then_some(parameter)
        })
        .collect();
    clone
}

fn remove_specialized_arguments(
    arguments: &mut Vec<ValueId>,
    signature: &[(usize, SpecializationValue)],
) {
    *arguments = arguments
        .drain(..)
        .enumerate()
        .filter_map(|(index, argument)| {
            (!signature.iter().any(|(removed, _)| *removed == index)).then_some(argument)
        })
        .collect();
}

fn function_instruction_count(function: &ControlFlowFunction<'_>) -> usize {
    function
        .blocks
        .iter()
        .map(|block| block.instructions.len() + block.phis.len())
        .sum()
}

fn optimize_unused_parameters(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_functions(module);
    let indirect = indirectly_referenced_functions(module);
    let removals = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && !exported.contains(&function.id)
                && !indirect.contains(&function.id)
        })
        .filter_map(|function| {
            let uses = control_flow_use_counts(function);
            let unused = function
                .params
                .iter()
                .enumerate()
                .filter_map(|(index, parameter)| {
                    (uses.get(&parameter.value).copied().unwrap_or(0) == 0).then_some(index)
                })
                .collect::<Vec<_>>();
            (!unused.is_empty()).then_some((function.id, unused))
        })
        .collect::<AHashMap<_, _>>();
    if removals.is_empty() {
        return OptimizationReport {
            pass_name: "unused-parameter-optimization",
            changed: false,
        };
    }
    for function in &mut module.functions {
        if let Some(indices) = removals.get(&function.id) {
            function.params = function
                .params
                .drain(..)
                .enumerate()
                .filter_map(|(index, parameter)| (!indices.contains(&index)).then_some(parameter))
                .collect();
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if let ControlFlowOp::CallDirect {
                    function: callee,
                    args,
                } = &mut instruction.op
                {
                    if let Some(indices) = removals.get(callee) {
                        *args = args
                            .drain(..)
                            .enumerate()
                            .filter_map(|(index, argument)| {
                                (!indices.contains(&index)).then_some(argument)
                            })
                            .collect();
                    }
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "unused-parameter-optimization",
        changed: true,
    }
}

fn specialize_constant_parameters(
    module: &mut ControlFlowModule<'_>,
    specialize_tagged_constants: bool,
    finite_value_propagation: bool,
) -> OptimizationReport {
    let exported = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Function(function) => Some(function),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let indirect = indirectly_referenced_functions(module);
    let finite_values = finite_value_propagation.then(|| analyze_finite_values(module));
    let mut calls = AHashMap::<FunctionId, Vec<Vec<Option<ConstValue>>>>::new();
    for caller in &module.functions {
        let constants = caller
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Const(value)) => Some((out, value.clone())),
                _ => None,
            })
            .collect::<AHashMap<_, _>>();
        for instruction in caller.blocks.iter().flat_map(|block| &block.instructions) {
            if let ControlFlowOp::CallDirect { function, args } = &instruction.op {
                calls.entry(*function).or_default().push(
                    args.iter()
                        .map(|argument| {
                            constants.get(argument).cloned().or_else(|| {
                                finite_values
                                    .as_ref()
                                    .and_then(|analysis| {
                                        analysis.function(caller.id).constant(*argument)
                                    })
                                    .cloned()
                            })
                        })
                        .collect(),
                );
            }
        }
    }
    let specializations = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && !exported.contains(&function.id)
                && !indirect.contains(&function.id)
        })
        .filter_map(|function| {
            let sites = calls.get(&function.id)?;
            let constants = function
                .params
                .iter()
                .enumerate()
                .filter_map(|(index, parameter)| {
                    let first = sites.first()?.get(index)?.clone()?;
                    ((specialize_tagged_constants
                        || constant_has_direct_native_representation(&first, &parameter.ty))
                        && sites
                            .iter()
                            .all(|args| args.get(index) == Some(&Some(first.clone()))))
                    .then_some((index, first))
                })
                .collect::<Vec<_>>();
            (!constants.is_empty()).then_some((function.id, constants))
        })
        .collect::<AHashMap<_, _>>();
    if specializations.is_empty() {
        return OptimizationReport {
            pass_name: "constant-parameter-specialization",
            changed: false,
        };
    }
    for function in &mut module.functions {
        if let Some(parameters) = specializations.get(&function.id) {
            let mut replacements = AHashMap::new();
            let mut constants = Vec::new();
            for (index, value) in parameters {
                let parameter = &function.params[*index];
                let out = ValueId(function.value_count);
                function.value_count += 1;
                function.value_escapes.push(EscapeState::LocalOnly);
                replacements.insert(parameter.value, out);
                constants.push(ControlFlowInstruction {
                    out: Some(out),
                    ty: Some(parameter.ty.clone()),
                    op: ControlFlowOp::Const(value.clone()),
                    span: parameter.span,
                });
            }
            rewrite_control_flow_function(function, &replacements);
            function.blocks[function.entry.0 as usize]
                .instructions
                .splice(0..0, constants);
            function.params = function
                .params
                .drain(..)
                .enumerate()
                .filter_map(|(index, parameter)| {
                    (!parameters.iter().any(|(removed, _)| *removed == index)).then_some(parameter)
                })
                .collect();
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if let ControlFlowOp::CallDirect {
                    function: callee,
                    args,
                } = &mut instruction.op
                {
                    if let Some(parameters) = specializations.get(callee) {
                        *args = args
                            .drain(..)
                            .enumerate()
                            .filter_map(|(index, argument)| {
                                (!parameters.iter().any(|(removed, _)| *removed == index))
                                    .then_some(argument)
                            })
                            .collect();
                    }
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "constant-parameter-specialization",
        changed: true,
    }
}

fn constant_has_direct_native_representation(value: &ConstValue, ty: &Type<'_>) -> bool {
    matches!(
        (value, ty),
        (ConstValue::Int(_), Type::Int)
            | (ConstValue::Float(_), Type::Float)
            | (ConstValue::Bool(_), Type::Bool)
            | (ConstValue::String(_), Type::String)
    )
}

fn optimize_unused_returns(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_functions(module);
    let indirect = indirectly_referenced_functions(module);
    let mut observed = AHashSet::<FunctionId>::new();
    for function in &module.functions {
        let uses = control_flow_use_counts(function);
        for block in &function.blocks {
            for instruction in &block.instructions {
                let ControlFlowOp::CallDirect {
                    function: callee, ..
                } = instruction.op
                else {
                    continue;
                };
                if instruction
                    .out
                    .is_some_and(|out| uses.get(&out).copied().unwrap_or(0) != 0)
                {
                    observed.insert(callee);
                }
            }
        }
    }
    let candidates = module
        .functions
        .iter()
        .filter(|function| {
            function.live
                && !function.return_type.is_void()
                && matches!(
                    function.kind,
                    FunctionKind::Function | FunctionKind::Method { .. }
                )
                && !exported.contains(&function.id)
                && !indirect.contains(&function.id)
                && !observed.contains(&function.id)
        })
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    if candidates.is_empty() {
        return OptimizationReport {
            pass_name: "unused-return-optimization",
            changed: false,
        };
    }
    for function in &mut module.functions {
        if candidates.contains(&function.id) {
            function.return_type = Type::Void;
            for block in &mut function.blocks {
                if matches!(block.terminator, Some(Terminator::Return(Some(_)))) {
                    block.terminator = Some(Terminator::Return(None));
                }
            }
        }
        for block in &mut function.blocks {
            for instruction in &mut block.instructions {
                if matches!(
                    instruction.op,
                    ControlFlowOp::CallDirect { function, .. } if candidates.contains(&function)
                ) {
                    instruction.out = None;
                    instruction.ty = None;
                }
            }
        }
    }
    OptimizationReport {
        pass_name: "unused-return-optimization",
        changed: true,
    }
}

fn indirectly_referenced_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    let mut indirect = AHashSet::new();
    for function in &module.functions {
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            match &instruction.op {
                ControlFlowOp::Closure { function, .. }
                | ControlFlowOp::NewClass {
                    constructor: Some(function),
                    ..
                } => {
                    indirect.insert(*function);
                }
                _ => {}
            }
        }
    }
    indirect
}

fn fold_and_propagate_control_flow(
    module: &mut ControlFlowModule<'_>,
    finite_value_propagation: bool,
) -> OptimizationReport {
    let mut changed = false;
    let finite_values = finite_value_propagation.then(|| analyze_finite_values(module));
    let effect_summaries = analyze_function_effects(module);
    let array_argument_retention_barriers = array_argument_retention_barriers(module);
    let parameter_array_lengths = analyze_array_parameter_lengths(module, &effect_summaries);
    for function in &mut module.functions {
        let mut constants = finite_values
            .as_ref()
            .into_iter()
            .flat_map(|analysis| analysis.function(function.id).constants())
            .map(|(value, constant)| (value, constant.clone()))
            .collect::<AHashMap<ValueId, ConstValue>>();
        let array_lengths = stable_array_lengths(
            function,
            &parameter_array_lengths[function.id.0 as usize],
            &effect_summaries,
            &array_argument_retention_barriers,
        );
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
                            let _ = values;
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
                            intrinsic: Intrinsic::IntImul,
                            receiver: None,
                            args,
                        } => {
                            let folded =
                                args.as_slice()
                                    .first()
                                    .zip(args.get(1))
                                    .and_then(|(lhs, rhs)| {
                                        match (constants.get(lhs), constants.get(rhs)) {
                                            (
                                                Some(ConstValue::Int(lhs)),
                                                Some(ConstValue::Int(rhs)),
                                            ) => Some(ConstValue::Int(i64::from(
                                                (*lhs as i32).wrapping_mul(*rhs as i32),
                                            ))),
                                            _ => None,
                                        }
                                    });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic:
                                intrinsic @ (Intrinsic::IntToString | Intrinsic::IntToUnsignedString),
                            receiver: Some(receiver),
                            args,
                        } => {
                            let folded = constants.get(receiver).and_then(|receiver| {
                                let ConstValue::Int(value) = receiver else {
                                    return None;
                                };
                                let radix = args
                                    .first()
                                    .map(|argument| constants.get(argument))
                                    .unwrap_or(Some(&ConstValue::Int(10)));
                                let Some(ConstValue::Int(radix @ 2..=36)) = radix else {
                                    return None;
                                };
                                Some(ConstValue::String(format_i32_radix(
                                    *value as i32,
                                    *radix as u32,
                                    matches!(intrinsic, Intrinsic::IntToUnsignedString),
                                )))
                            });
                            if let Some(folded) = folded {
                                instruction.op = ControlFlowOp::Const(folded.clone());
                                constants.insert(out, folded);
                                changed = true;
                                local_change = true;
                            }
                        }
                        ControlFlowOp::Intrinsic {
                            intrinsic:
                                intrinsic @ (Intrinsic::FloatAbs
                                | Intrinsic::FloatFloor
                                | Intrinsic::FloatCeil
                                | Intrinsic::FloatMin
                                | Intrinsic::FloatMax),
                            receiver: Some(receiver),
                            args,
                        } => {
                            let folded = constants.get(receiver).and_then(|receiver| {
                                let ConstValue::Float(value) = receiver else {
                                    return None;
                                };
                                let argument = || {
                                    args.first()
                                        .and_then(|argument| constants.get(argument))
                                        .and_then(|argument| match argument {
                                            ConstValue::Float(value) => Some(*value),
                                            _ => None,
                                        })
                                };
                                Some(ConstValue::Float(match intrinsic {
                                    Intrinsic::FloatAbs => value.abs(),
                                    Intrinsic::FloatFloor => value.floor(),
                                    Intrinsic::FloatCeil => value.ceil(),
                                    Intrinsic::FloatMin => js_min(*value, argument()?),
                                    Intrinsic::FloatMax => js_max(*value, argument()?),
                                    _ => unreachable!(),
                                }))
                            });
                            if let Some(folded) = folded {
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
                                | Intrinsic::StringCharCodeAt
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum ArrayLengthFact {
    #[default]
    Bottom,
    Exact(usize),
    Unknown,
}

impl ArrayLengthFact {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, value) | (value, Self::Bottom) => value,
            (Self::Exact(left), Self::Exact(right)) if left == right => Self::Exact(left),
            _ => Self::Unknown,
        }
    }
}

fn analyze_array_parameter_lengths(
    module: &ControlFlowModule<'_>,
    effect_summaries: &[FunctionEffectSummary],
) -> Vec<Vec<ArrayLengthFact>> {
    let retention_barriers = array_argument_retention_barriers(module);
    let exported = exported_functions(module);
    let indirect = module
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::Closure { function, .. } => Some(function),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let boundary_facts = || {
        module
            .functions
            .iter()
            .map(|function| {
                function
                    .params
                    .iter()
                    .map(|parameter| {
                        if matches!(parameter.ty, Type::Array(_))
                            && (function.kind == FunctionKind::Extern
                                || exported.contains(&function.id)
                                || indirect.contains(&function.id))
                        {
                            ArrayLengthFact::Unknown
                        } else {
                            ArrayLengthFact::Bottom
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    let mut parameter_facts = boundary_facts();
    loop {
        let function_lengths = module
            .functions
            .iter()
            .map(|function| {
                stable_array_lengths(
                    function,
                    &parameter_facts[function.id.0 as usize],
                    effect_summaries,
                    &retention_barriers,
                )
            })
            .collect::<Vec<_>>();
        let mut proposed = boundary_facts();
        for caller in &module.functions {
            let caller_lengths = &function_lengths[caller.id.0 as usize];
            for instruction in caller.blocks.iter().flat_map(|block| &block.instructions) {
                let (callee, arguments) = match &instruction.op {
                    ControlFlowOp::CallDirect { function, args } => (*function, args.clone()),
                    ControlFlowOp::CallMethod {
                        receiver,
                        function,
                        args,
                        ..
                    } => {
                        let mut values = vec![*receiver];
                        values.extend(args);
                        (*function, values)
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        args,
                        ..
                    } => {
                        let mut values = instruction.out.into_iter().collect::<Vec<_>>();
                        values.extend(args);
                        (*function, values)
                    }
                    _ => continue,
                };
                let Some(callee_function) = module.functions.get(callee.0 as usize) else {
                    continue;
                };
                for (index, (argument, parameter)) in
                    arguments.iter().zip(&callee_function.params).enumerate()
                {
                    if !matches!(parameter.ty, Type::Array(_)) {
                        continue;
                    }
                    let incoming = caller_lengths
                        .get(argument)
                        .copied()
                        .map_or(ArrayLengthFact::Unknown, ArrayLengthFact::Exact);
                    proposed[callee.0 as usize][index] =
                        proposed[callee.0 as usize][index].join(incoming);
                }
            }
        }
        if proposed == parameter_facts {
            return parameter_facts;
        }
        parameter_facts = proposed;
    }
}

fn stable_array_lengths(
    function: &ControlFlowFunction<'_>,
    parameter_lengths: &[ArrayLengthFact],
    effect_summaries: &[FunctionEffectSummary],
    retention_barriers: &[bool],
) -> AHashMap<ValueId, usize> {
    let mut candidates = function
        .params
        .iter()
        .zip(parameter_lengths)
        .filter_map(|(parameter, length)| match length {
            ArrayLengthFact::Exact(length) => Some((parameter.value, *length)),
            ArrayLengthFact::Bottom | ArrayLengthFact::Unknown => None,
        })
        .chain(
            function
                .blocks
                .iter()
                .flat_map(|block| &block.instructions)
                .filter_map(|instruction| match (instruction.out, &instruction.op) {
                    (Some(out), ControlFlowOp::Array(values)) => Some((out, values.len())),
                    _ => None,
                }),
        )
        .collect::<AHashMap<_, _>>();
    if candidates.is_empty() {
        return candidates;
    }
    let mut dependencies = AHashMap::<ValueId, ValueId>::new();
    loop {
        let mut changed = false;
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let (
                Some(out),
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayMap,
                    receiver: Some(receiver),
                    ..
                },
            ) = (instruction.out, &instruction.op)
            else {
                continue;
            };
            if let Some(length) = candidates.get(receiver).copied() {
                if candidates.insert(out, length).is_none() {
                    dependencies.insert(out, *receiver);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut invalid = AHashSet::new();
    let invalidate = |value: ValueId, invalid: &mut AHashSet<ValueId>| {
        if candidates.contains_key(&value) {
            invalid.insert(value);
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            for (_, value) in &phi.incoming {
                invalidate(*value, &mut invalid);
            }
        }
        for instruction in &block.instructions {
            match &instruction.op {
                ControlFlowOp::Array(values) | ControlFlowOp::Struct { fields: values, .. } => {
                    for value in values {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::NewClass { args, .. } => {
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::CallDirect { function, args } => {
                    invalidate_direct_call_array_arguments(
                        *function,
                        args,
                        effect_summaries,
                        retention_barriers,
                        &mut invalid,
                        &invalidate,
                    );
                }
                ControlFlowOp::Closure { captures, .. } => {
                    for value in captures {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::StoreGlobal { value, .. } => invalidate(*value, &mut invalid),
                ControlFlowOp::FieldSet { object, value, .. }
                | ControlFlowOp::HostFieldSet { object, value, .. } => {
                    invalidate(*object, &mut invalid);
                    invalidate(*value, &mut invalid);
                }
                ControlFlowOp::HostFieldGet { object, .. } => invalidate(*object, &mut invalid),
                ControlFlowOp::IndexSet {
                    object,
                    index,
                    value,
                } => {
                    invalidate(*object, &mut invalid);
                    invalidate(*index, &mut invalid);
                    invalidate(*value, &mut invalid);
                }
                ControlFlowOp::CallValue { callee, args } => {
                    invalidate(*callee, &mut invalid);
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::CallMethod {
                    receiver,
                    function,
                    args,
                    ..
                } => {
                    let mut arguments = vec![*receiver];
                    arguments.extend(args);
                    invalidate_direct_call_array_arguments(
                        *function,
                        &arguments,
                        effect_summaries,
                        retention_barriers,
                        &mut invalid,
                        &invalidate,
                    );
                }
                ControlFlowOp::HostCall { receiver, args, .. } => {
                    invalidate(*receiver, &mut invalid);
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                ControlFlowOp::Intrinsic {
                    intrinsic,
                    receiver,
                    args,
                } => {
                    if matches!(
                        intrinsic,
                        Intrinsic::Print | Intrinsic::ArrayPush | Intrinsic::ArrayPop
                    ) {
                        if let Some(receiver) = receiver {
                            invalidate(*receiver, &mut invalid);
                        }
                    }
                    for value in args {
                        invalidate(*value, &mut invalid);
                    }
                }
                _ => {}
            }
        }
        if let Some(Terminator::Return(Some(value))) = block.terminator {
            invalidate(value, &mut invalid);
        }
    }
    loop {
        let old_len = invalid.len();
        invalid.extend(
            dependencies
                .iter()
                .filter_map(|(value, source)| invalid.contains(source).then_some(*value))
                .collect::<Vec<_>>(),
        );
        if invalid.len() == old_len {
            break;
        }
    }
    candidates
        .into_iter()
        .filter(|(value, _)| !invalid.contains(value))
        .collect()
}

fn invalidate_direct_call_array_arguments(
    function: FunctionId,
    args: &[ValueId],
    effect_summaries: &[FunctionEffectSummary],
    retention_barriers: &[bool],
    invalid: &mut AHashSet<ValueId>,
    invalidate: &impl Fn(ValueId, &mut AHashSet<ValueId>),
) {
    let summary = effect_summaries.get(function.0 as usize);
    let may_retain_arguments = retention_barriers
        .get(function.0 as usize)
        .copied()
        .unwrap_or(true);
    for (index, argument) in args.iter().enumerate() {
        if may_retain_arguments
            || summary.is_none_or(|summary| {
                summary.inherent || summary.mutated_parameters.contains(&index)
            })
        {
            invalidate(*argument, invalid);
        }
    }
}

fn array_argument_retention_barriers(module: &ControlFlowModule<'_>) -> Vec<bool> {
    module
        .functions
        .iter()
        .map(|function| {
            function.kind == FunctionKind::Extern || type_can_carry_reference(&function.return_type)
        })
        .collect()
}

fn type_can_carry_reference(ty: &Type<'_>) -> bool {
    match ty {
        Type::Int | Type::Float | Type::String | Type::Bool | Type::Null | Type::Void => false,
        Type::Nullable(inner) => type_can_carry_reference(inner),
        Type::Union(members) => members.iter().any(type_can_carry_reference),
        Type::Array(_)
        | Type::Map(_, _)
        | Type::Set(_)
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Uint8Array
        | Type::Task(_)
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::Struct(_)
        | Type::Class(_)
        | Type::StructInstance { .. }
        | Type::ClassInstance { .. }
        | Type::TypeParameter(_)
        | Type::Function(_)
        | Type::GenericFunction(_) => true,
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

fn inline_small_functions(
    module: &mut ControlFlowModule<'_>,
    options: &OptimizationOptions,
) -> OptimizationReport {
    let recursive = recursive_functions(module);
    let exported = exported_functions(module);
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
            ControlFlowOp::NewClass {
                constructor: Some(function),
                ..
            } => {
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
            !matches!(function.kind, FunctionKind::Entry | FunctionKind::Extern)
                && !recursive.contains(&function.id)
                && !exported.contains(&function.id)
                && !function_has_type_parameters(function)
                && function.blocks.len() == 1
                && function.blocks[0].phis.is_empty()
                && (options.inline_closure_factories
                    || !function.blocks[0]
                        .instructions
                        .iter()
                        .any(|instruction| matches!(instruction.op, ControlFlowOp::Closure { .. })))
                && function.blocks[0].instructions.len() <= options.inline_instruction_limit
                && options.inline_growth_limit.is_none_or(|limit| {
                    let instructions = function.blocks[0].instructions.len();
                    let calls = call_counts.get(&function.id).copied().unwrap_or(0);
                    let retained_instructions = if address_taken.contains(&function.id) {
                        instructions
                    } else {
                        0
                    };
                    let before = instructions + calls;
                    let after = retained_instructions + instructions.saturating_mul(calls);
                    after.saturating_sub(before) <= limit
                })
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
    inline_limit: usize,
) -> OptimizationReport {
    let recursive = recursive_functions(module);
    let exported = exported_functions(module);
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
                && !function_has_type_parameters(function)
                && !recursive.contains(&function.id)
                && !exported.contains(&function.id)
                && !address_taken.contains(&function.id)
                && call_counts.get(&function.id) == Some(&1)
                && function
                    .blocks
                    .iter()
                    .map(|block| block.instructions.len())
                    .sum::<usize>()
                    <= inline_limit
        })
        .map(|function| (function.id, function.clone()))
        .collect::<AHashMap<_, _>>();

    let mut site = None;
    'functions: for (function_index, caller) in module.functions.iter().enumerate() {
        let structured_interiors = structured_interior_blocks(caller);
        for (block_index, block) in caller.blocks.iter().enumerate() {
            if structured_interiors.contains(&block.id) {
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

fn function_has_type_parameters(function: &ControlFlowFunction<'_>) -> bool {
    function
        .params
        .iter()
        .any(|parameter| type_has_type_parameter(&parameter.ty))
        || type_has_type_parameter(&function.return_type)
}

fn type_has_type_parameter(ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter(_) => true,
        Type::Array(element) => type_has_type_parameter(element),
        Type::Map(key, value) => type_has_type_parameter(key) || type_has_type_parameter(value),
        Type::Set(element) => type_has_type_parameter(element),
        Type::Nullable(inner) => type_has_type_parameter(inner),
        Type::Union(members) => members.iter().any(type_has_type_parameter),
        Type::StructInstance { args, .. } | Type::ClassInstance { args, .. } => {
            args.iter().any(type_has_type_parameter)
        }
        Type::Function(signature) => {
            signature.params.iter().any(type_has_type_parameter)
                || type_has_type_parameter(&signature.return_type)
        }
        Type::GenericFunction(_) => true,
        _ => false,
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
    let insertion_block = caller.blocks[block_index].id;
    let shapes_compose_safely =
        callee.shapes.is_empty() || !structured_interior_blocks(caller).contains(&insertion_block);
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

    // Nested region metadata needs a full region-tree rewrite; use the CFG state machine meanwhile.
    if !shapes_compose_safely {
        caller.shapes.clear();
    }
    for shape in callee.shapes.iter().filter(|_| shapes_compose_safely) {
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

fn structured_interior_blocks(function: &ControlFlowFunction<'_>) -> AHashSet<BlockId> {
    let mut blocks = AHashSet::new();
    for shape in &function.shapes {
        match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                ..
            } => {
                blocks.extend([*header, *then_block, *else_block]);
            }
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                ..
            } => {
                blocks.extend([*header, *body]);
                blocks.extend(update);
            }
        }
    }
    blocks
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EscapeNode {
    Value(FunctionId, ValueId),
    Global(SymbolId),
}

fn analyze_escapes(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let extern_functions = module
        .functions
        .iter()
        .filter(|function| function.kind == FunctionKind::Extern)
        .map(|function| function.id)
        .collect::<AHashSet<_>>();
    let exported_functions = exported_functions(module);
    let exported_globals = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Global(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let returns = module
        .functions
        .iter()
        .map(|function| {
            let values = function
                .blocks
                .iter()
                .filter_map(|block| match block.terminator {
                    Some(Terminator::Return(Some(value))) => Some(value),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (function.id, values)
        })
        .collect::<AHashMap<_, _>>();
    let mut states = AHashMap::<EscapeNode, EscapeState>::new();
    let mut edges = AHashMap::<EscapeNode, AHashSet<EscapeNode>>::new();

    for global in &module.globals {
        let node = EscapeNode::Global(global.symbol);
        mark_escape_node(&mut states, node, EscapeState::EscapesToTypedCode);
        if exported_globals.contains(&global.symbol) {
            mark_escape_node(&mut states, node, EscapeState::EscapesToUntypedBoundary);
        }
    }

    for function in &module.functions {
        let exported = exported_functions.contains(&function.id);
        for (index, state) in function.value_escapes.iter().copied().enumerate() {
            mark_escape_node(
                &mut states,
                EscapeNode::Value(function.id, ValueId(index as u32)),
                state,
            );
        }
        if exported {
            for parameter in &function.params {
                mark_escape_node(
                    &mut states,
                    EscapeNode::Value(function.id, parameter.value),
                    EscapeState::EscapesToUntypedBoundary,
                );
            }
            for value in &returns[&function.id] {
                mark_escape_node(
                    &mut states,
                    EscapeNode::Value(function.id, *value),
                    EscapeState::EscapesToUntypedBoundary,
                );
            }
        }
        for block in &function.blocks {
            for phi in &block.phis {
                for (_, incoming) in &phi.incoming {
                    add_escape_edge(
                        &mut edges,
                        EscapeNode::Value(function.id, phi.out),
                        EscapeNode::Value(function.id, *incoming),
                    );
                }
            }
            for instruction in &block.instructions {
                let value_node = |value| EscapeNode::Value(function.id, value);
                match &instruction.op {
                    ControlFlowOp::StoreGlobal { global, value } => {
                        add_escape_edge(
                            &mut edges,
                            value_node(*value),
                            EscapeNode::Global(*global),
                        );
                    }
                    ControlFlowOp::LoadGlobal(global) => {
                        if let Some(out) = instruction.out {
                            add_escape_edge(
                                &mut edges,
                                value_node(out),
                                EscapeNode::Global(*global),
                            );
                        }
                    }
                    ControlFlowOp::HostFieldGet { object, .. } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*object),
                            EscapeState::EscapesToUntypedBoundary,
                        );
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::HostFieldSet { object, value, .. } => {
                        for value in [*object, *value] {
                            mark_escape_node(
                                &mut states,
                                value_node(value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::HostCall { receiver, args, .. } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*receiver),
                            EscapeState::EscapesToUntypedBoundary,
                        );
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                        if let Some(out) = instruction.out {
                            mark_escape_node(
                                &mut states,
                                value_node(out),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::CallDirect {
                        function: callee,
                        args,
                    } => {
                        if extern_functions.contains(callee) {
                            for value in args {
                                mark_escape_node(
                                    &mut states,
                                    value_node(*value),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                            if let Some(out) = instruction.out {
                                mark_escape_node(
                                    &mut states,
                                    value_node(out),
                                    EscapeState::EscapesToUntypedBoundary,
                                );
                            }
                        } else if let Some(callee_function) =
                            module.functions.get(callee.0 as usize)
                        {
                            for (argument, parameter) in args.iter().zip(&callee_function.params) {
                                add_escape_edge(
                                    &mut edges,
                                    value_node(*argument),
                                    EscapeNode::Value(*callee, parameter.value),
                                );
                            }
                            if let Some(out) = instruction.out {
                                for returned in &returns[callee] {
                                    add_escape_edge(
                                        &mut edges,
                                        value_node(out),
                                        EscapeNode::Value(*callee, *returned),
                                    );
                                }
                            }
                        }
                    }
                    ControlFlowOp::CallMethod { receiver, args, .. } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*receiver),
                            EscapeState::EscapesToTypedCode,
                        );
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::CallValue { callee, args } => {
                        mark_escape_node(
                            &mut states,
                            value_node(*callee),
                            EscapeState::EscapesToTypedCode,
                        );
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::Print,
                        args,
                        ..
                    } => {
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToUntypedBoundary,
                            );
                        }
                    }
                    ControlFlowOp::Intrinsic { receiver, args, .. } => {
                        if let Some(receiver) = receiver {
                            mark_escape_node(
                                &mut states,
                                value_node(*receiver),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                        for value in args {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::Closure { captures, .. } => {
                        for value in captures {
                            mark_escape_node(
                                &mut states,
                                value_node(*value),
                                EscapeState::EscapesToTypedCode,
                            );
                        }
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(constructor),
                        args,
                        ..
                    } => {
                        if let Some(callee) = module.functions.get(constructor.0 as usize) {
                            let values = instruction.out.into_iter().chain(args.iter().copied());
                            for (argument, parameter) in values.zip(&callee.params) {
                                add_escape_edge(
                                    &mut edges,
                                    value_node(argument),
                                    EscapeNode::Value(*constructor, parameter.value),
                                );
                            }
                        }
                    }
                    _ => {}
                }
            }
            if let Some(Terminator::Return(Some(value))) = block.terminator {
                mark_escape_node(
                    &mut states,
                    EscapeNode::Value(function.id, value),
                    if exported {
                        EscapeState::EscapesToUntypedBoundary
                    } else {
                        EscapeState::EscapesToTypedCode
                    },
                );
            }
        }
    }

    loop {
        let mut updates = Vec::new();
        for (node, neighbors) in &edges {
            let state = states.get(node).copied().unwrap_or(EscapeState::LocalOnly);
            for neighbor in neighbors {
                let current = states
                    .get(neighbor)
                    .copied()
                    .unwrap_or(EscapeState::LocalOnly);
                if escape_rank(state) > escape_rank(current) {
                    updates.push((*neighbor, state));
                }
            }
        }
        if updates.is_empty() {
            break;
        }
        for (node, state) in updates {
            mark_escape_node(&mut states, node, state);
        }
    }

    let mut changed = false;
    for function in &mut module.functions {
        for (index, slot) in function.value_escapes.iter_mut().enumerate() {
            let state = states
                .get(&EscapeNode::Value(function.id, ValueId(index as u32)))
                .copied()
                .unwrap_or(EscapeState::LocalOnly);
            if *slot != state {
                *slot = state;
                changed = true;
            }
        }
    }
    OptimizationReport {
        pass_name: "escape-analysis",
        changed,
    }
}

fn add_escape_edge(
    edges: &mut AHashMap<EscapeNode, AHashSet<EscapeNode>>,
    left: EscapeNode,
    right: EscapeNode,
) {
    edges.entry(left).or_default().insert(right);
    edges.entry(right).or_default().insert(left);
}

fn mark_escape_node(
    states: &mut AHashMap<EscapeNode, EscapeState>,
    node: EscapeNode,
    state: EscapeState,
) {
    let slot = states.entry(node).or_insert(EscapeState::LocalOnly);
    if escape_rank(state) > escape_rank(*slot) {
        *slot = state;
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
    let effect_summaries = analyze_function_effects(module);
    let effectful_functions = effectful_functions(&effect_summaries);
    for function in &mut module.functions {
        let closure_targets = closure_targets(function);
        loop {
            let uses = control_flow_use_counts(function);
            let (mutable_roots, unobserved_mutables) =
                unobserved_local_mutables(function, &uses, &effect_summaries);
            let mut local_change = false;
            for block in &mut function.blocks {
                let old_len = block.instructions.len();
                block.instructions.retain(|instruction| {
                    let result_is_used = instruction
                        .out
                        .is_some_and(|out| uses.get(&out).copied().unwrap_or(0) != 0);
                    let mutates_only_unobserved_state = mutation_receiver(&instruction.op)
                        .and_then(|receiver| mutable_roots.get(&receiver))
                        .is_some_and(|root| unobserved_mutables.contains(root))
                        || direct_call_mutates_only_unobserved_state(
                            &instruction.op,
                            &effect_summaries,
                            &mutable_roots,
                            &unobserved_mutables,
                        );
                    result_is_used
                        || (!mutates_only_unobserved_state
                            && control_flow_op_has_side_effects(
                                &instruction.op,
                                &effectful_functions,
                                &closure_targets,
                            ))
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

fn unobserved_local_mutables(
    function: &ControlFlowFunction<'_>,
    uses: &AHashMap<ValueId, usize>,
    effect_summaries: &[FunctionEffectSummary],
) -> (AHashMap<ValueId, ValueId>, AHashSet<ValueId>) {
    let mut roots = local_mutable_roots(function, is_local_mutable_allocation);
    if roots.is_empty() {
        return (roots, AHashSet::new());
    }

    extend_mutable_alias_roots(function, &mut roots);

    let mut observed = AHashSet::new();
    let mut mutation_groups = Vec::new();
    let mut observe = |value: ValueId| {
        if let Some(root) = roots.get(&value) {
            observed.insert(*root);
        }
    };
    for block in &function.blocks {
        for phi in &block.phis {
            if !roots.contains_key(&phi.out) {
                for (_, value) in &phi.incoming {
                    observe(*value);
                }
            }
        }
        for instruction in &block.instructions {
            match &instruction.op {
                ControlFlowOp::CallDirect { function, args }
                    if instruction
                        .out
                        .is_none_or(|out| uses.get(&out).copied().unwrap_or(0) == 0) =>
                {
                    let summary = effect_summaries.get(function.0 as usize);
                    let mutation_roots = summary
                        .filter(|summary| !summary.inherent)
                        .and_then(|summary| local_mutation_roots(summary, args, &roots));
                    let call_is_locally_discardable = summary
                        .is_some_and(|summary| !summary.inherent && mutation_roots.is_some());
                    if let Some(group) = mutation_roots.filter(|group| !group.is_empty()) {
                        mutation_groups.push(group);
                    } else if !call_is_locally_discardable {
                        for value in args {
                            observe(*value);
                        }
                    }
                }
                ControlFlowOp::IndexSet {
                    object,
                    index,
                    value,
                } if roots.contains_key(object) => {
                    observe(*index);
                    observe(*value);
                }
                ControlFlowOp::Intrinsic {
                    intrinsic:
                        intrinsic @ (Intrinsic::ArrayPush
                        | Intrinsic::ArrayPop
                        | Intrinsic::MapSet
                        | Intrinsic::MapDelete
                        | Intrinsic::MapClear
                        | Intrinsic::SetAdd
                        | Intrinsic::SetDelete
                        | Intrinsic::SetClear),
                    receiver: Some(receiver),
                    args,
                } if roots.contains_key(receiver) => {
                    for value in args {
                        observe(*value);
                    }
                    let fluent_alias = matches!(intrinsic, Intrinsic::MapSet | Intrinsic::SetAdd);
                    if !fluent_alias
                        && instruction
                            .out
                            .is_some_and(|out| uses.get(&out).copied().unwrap_or(0) != 0)
                    {
                        observe(*receiver);
                    }
                }
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
                    receiver: Some(receiver),
                    ..
                } if roots.contains_key(receiver)
                    && instruction.out.is_some_and(|out| roots.contains_key(&out)) => {}
                op => {
                    for value in control_flow_used_values(op) {
                        observe(value);
                    }
                }
            }
        }
        if let Some(terminator) = &block.terminator {
            for value in terminator_used_values(terminator) {
                observe(value);
            }
        }
    }
    loop {
        let mut changed = false;
        for group in &mutation_groups {
            if group.iter().any(|root| observed.contains(root)) {
                for root in group {
                    changed |= observed.insert(*root);
                }
            }
        }
        if !changed {
            break;
        }
    }
    let all_roots = roots.values().copied().collect::<AHashSet<_>>();
    let unobserved = all_roots.difference(&observed).copied().collect();
    (roots, unobserved)
}

fn local_mutable_roots(
    function: &ControlFlowFunction<'_>,
    is_seed: fn(&ControlFlowOp<'_>) -> bool,
) -> AHashMap<ValueId, ValueId> {
    function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| {
            let out = instruction.out?;
            is_seed(&instruction.op).then_some((out, out))
        })
        .collect()
}

fn extend_mutable_alias_roots<Root: Copy + Eq>(
    function: &ControlFlowFunction<'_>,
    roots: &mut AHashMap<ValueId, Root>,
) {
    loop {
        let mut changed = false;
        for block in &function.blocks {
            for phi in &block.phis {
                let mut incoming = phi
                    .incoming
                    .iter()
                    .filter_map(|(_, value)| roots.get(value).copied());
                let Some(root) = incoming.next() else {
                    continue;
                };
                if incoming.all(|candidate| candidate == root)
                    && phi
                        .incoming
                        .iter()
                        .all(|(_, value)| roots.get(value) == Some(&root))
                    && roots.insert(phi.out, root).is_none()
                {
                    changed = true;
                }
            }
            for instruction in &block.instructions {
                let Some(out) = instruction.out else {
                    continue;
                };
                let receiver = match instruction.op {
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::MapSet | Intrinsic::SetAdd,
                        receiver: Some(receiver),
                        ..
                    } => Some(receiver),
                    ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::UnwrapNullable | Intrinsic::UnwrapUnion,
                        receiver: Some(receiver),
                        ..
                    } => Some(receiver),
                    _ => None,
                };
                if let Some(root) = receiver.and_then(|receiver| roots.get(&receiver).copied()) {
                    if roots.insert(out, root).is_none() {
                        changed = true;
                    }
                }
            }
        }
        if !changed {
            break;
        }
    }
}

fn is_local_mutable_allocation(op: &ControlFlowOp<'_>) -> bool {
    matches!(
        op,
        ControlFlowOp::Array(_)
            | ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::MapNew
                    | Intrinsic::SetNew
                    | Intrinsic::ArrayBufferNew
                    | Intrinsic::SharedArrayBufferNew
                    | Intrinsic::Uint8ArrayNew,
                ..
            }
    )
}

fn is_owned_mutable_allocation(op: &ControlFlowOp<'_>) -> bool {
    is_local_mutable_allocation(op)
        || matches!(
            op,
            ControlFlowOp::Struct { .. } | ControlFlowOp::NewClass { .. }
        )
}

fn mutation_receiver(op: &ControlFlowOp<'_>) -> Option<ValueId> {
    match op {
        ControlFlowOp::IndexSet { object, .. } | ControlFlowOp::FieldSet { object, .. } => {
            Some(*object)
        }
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::ArrayPush
                | Intrinsic::ArrayPop
                | Intrinsic::MapSet
                | Intrinsic::MapDelete
                | Intrinsic::MapClear
                | Intrinsic::SetAdd
                | Intrinsic::SetDelete
                | Intrinsic::SetClear,
            receiver,
            ..
        } => *receiver,
        _ => None,
    }
}

fn local_mutation_roots(
    summary: &FunctionEffectSummary,
    args: &[ValueId],
    roots: &AHashMap<ValueId, ValueId>,
) -> Option<AHashSet<ValueId>> {
    let mut mutation_roots = AHashSet::new();
    for parameter in &summary.mutated_parameters {
        let root = args
            .get(*parameter)
            .and_then(|value| roots.get(value))
            .copied()?;
        mutation_roots.insert(root);
    }
    Some(mutation_roots)
}

fn direct_call_mutates_only_unobserved_state(
    op: &ControlFlowOp<'_>,
    summaries: &[FunctionEffectSummary],
    roots: &AHashMap<ValueId, ValueId>,
    unobserved: &AHashSet<ValueId>,
) -> bool {
    let ControlFlowOp::CallDirect { function, args } = op else {
        return false;
    };
    let Some(summary) = summaries.get(function.0 as usize) else {
        return false;
    };
    !summary.inherent
        && !summary.mutated_parameters.is_empty()
        && local_mutation_roots(summary, args, roots).is_some_and(|mutation_roots| {
            mutation_roots.iter().all(|root| unobserved.contains(root))
        })
}

fn fold_identical_private_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let exported = exported_functions(module);
    let mut changed = false;

    loop {
        let mut direct_only = module
            .functions
            .iter()
            .filter(|function| {
                function.live
                    && function.kind == FunctionKind::Function
                    && !exported.contains(&function.id)
            })
            .map(|function| function.id)
            .collect::<AHashSet<_>>();

        for function in module.functions.iter().filter(|function| function.live) {
            for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
                match instruction.op {
                    ControlFlowOp::Closure { function, .. }
                    | ControlFlowOp::CallMethod { function, .. } => {
                        direct_only.remove(&function);
                    }
                    ControlFlowOp::NewClass {
                        constructor: Some(function),
                        ..
                    } => {
                        direct_only.remove(&function);
                    }
                    _ => {}
                }
            }
        }

        if direct_only.len() < 2 {
            break;
        }
        let mut groups = AHashMap::<PrivateFunctionShape, Vec<FunctionId>>::new();
        for function in module
            .functions
            .iter()
            .filter(|function| direct_only.contains(&function.id))
        {
            groups
                .entry(private_function_shape(function))
                .or_default()
                .push(function.id);
        }
        let mut redirects = AHashMap::<FunctionId, FunctionId>::new();
        for group in groups.values().filter(|group| group.len() > 1) {
            let normalized = group
                .iter()
                .map(|function| {
                    (
                        *function,
                        normalize_private_function(&module.functions[function.0 as usize]),
                    )
                })
                .collect::<Vec<_>>();
            let mut representatives = Vec::<usize>::new();
            for (index, (function, body)) in normalized.iter().enumerate() {
                if let Some(representative) = representatives
                    .iter()
                    .copied()
                    .find(|representative| normalized[*representative].1 == *body)
                {
                    redirects.insert(*function, normalized[representative].0);
                } else {
                    representatives.push(index);
                }
            }
        }
        if redirects.is_empty() {
            break;
        }

        for function in &mut module.functions {
            for instruction in function
                .blocks
                .iter_mut()
                .flat_map(|block| &mut block.instructions)
            {
                rewrite_function_reference(&mut instruction.op, &redirects);
            }
        }
        for duplicate in redirects.keys() {
            module.functions[duplicate.0 as usize].live = false;
        }
        changed = true;
    }

    OptimizationReport {
        pass_name: "identical-private-function-folding",
        changed,
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PrivateFunctionShape {
    declared_pure: bool,
    parameter_count: usize,
    local_count: usize,
    capture_count: usize,
    shape_count: usize,
    blocks: Vec<(usize, usize, usize, u64)>,
}

fn private_function_shape(function: &ControlFlowFunction<'_>) -> PrivateFunctionShape {
    let blocks = function
        .blocks
        .iter()
        .map(|block| {
            let mut hasher = DefaultHasher::new();
            for phi in &block.phis {
                phi.incoming.len().hash(&mut hasher);
            }
            for instruction in &block.instructions {
                std::mem::discriminant(&instruction.op).hash(&mut hasher);
                instruction.out.is_some().hash(&mut hasher);
            }
            block
                .terminator
                .as_ref()
                .map(std::mem::discriminant)
                .hash(&mut hasher);
            (
                block.phis.len(),
                block.instructions.len(),
                block.phis.iter().map(|phi| phi.incoming.len()).sum(),
                hasher.finish(),
            )
        })
        .collect();
    PrivateFunctionShape {
        declared_pure: function.declared_pure,
        parameter_count: function.params.len(),
        local_count: function.locals.len(),
        capture_count: function.capture_count,
        shape_count: function.shapes.len(),
        blocks,
    }
}

fn rewrite_function_reference(
    op: &mut ControlFlowOp<'_>,
    redirects: &AHashMap<FunctionId, FunctionId>,
) {
    let reference = match op {
        ControlFlowOp::NewClass {
            constructor: Some(function),
            ..
        }
        | ControlFlowOp::Closure { function, .. }
        | ControlFlowOp::CallDirect { function, .. }
        | ControlFlowOp::CallMethod { function, .. } => function,
        _ => return,
    };
    while let Some(representative) = redirects.get(reference) {
        *reference = *representative;
    }
}

fn normalize_private_function<'src>(
    function: &ControlFlowFunction<'src>,
) -> ControlFlowFunction<'src> {
    const SELF_REFERENCE: FunctionId = FunctionId(u32::MAX);
    let mut normalized = function.clone();
    let empty = Span::empty(0);
    let original_id = normalized.id;
    let original_value_escapes = normalized.value_escapes.clone();
    normalized.id = SELF_REFERENCE;
    normalized.name = None;
    normalized.span = empty;
    normalized.live = true;

    let mut local_ids = AHashMap::<LocalId, LocalId>::new();
    let mut next_local = 0_u32;
    for parameter in &normalized.params {
        local_ids.entry(parameter.local).or_insert_with(|| {
            let id = LocalId(next_local);
            next_local += 1;
            id
        });
    }
    for local in &normalized.locals {
        local_ids.entry(local.id).or_insert_with(|| {
            let id = LocalId(next_local);
            next_local += 1;
            id
        });
    }
    for block in &normalized.blocks {
        for phi in &block.phis {
            local_ids.entry(phi.local).or_insert_with(|| {
                let id = LocalId(next_local);
                next_local += 1;
                id
            });
        }
        for instruction in &block.instructions {
            let local = match instruction.op {
                ControlFlowOp::LoadLocal(local) | ControlFlowOp::StoreLocal { local, .. } => {
                    Some(local)
                }
                _ => None,
            };
            if let Some(local) = local {
                local_ids.entry(local).or_insert_with(|| {
                    let id = LocalId(next_local);
                    next_local += 1;
                    id
                });
            }
        }
    }

    let mut value_ids = AHashMap::<ValueId, ValueId>::new();
    let mut next_value = 0_u32;
    for parameter in &normalized.params {
        value_ids.entry(parameter.value).or_insert_with(|| {
            let id = ValueId(next_value);
            next_value += 1;
            id
        });
    }
    for block in &normalized.blocks {
        for phi in &block.phis {
            value_ids.entry(phi.out).or_insert_with(|| {
                let id = ValueId(next_value);
                next_value += 1;
                id
            });
        }
        for instruction in &block.instructions {
            if let Some(out) = instruction.out {
                value_ids.entry(out).or_insert_with(|| {
                    let id = ValueId(next_value);
                    next_value += 1;
                    id
                });
            }
        }
    }
    normalized.value_escapes = vec![EscapeState::LocalOnly; next_value as usize];
    for (original, canonical) in &value_ids {
        if let Some(state) = original_value_escapes.get(original.0 as usize) {
            normalized.value_escapes[canonical.0 as usize] = *state;
        }
    }

    let block_ids = normalized
        .blocks
        .iter()
        .enumerate()
        .map(|(index, block)| (block.id, BlockId(index as u32)))
        .collect::<AHashMap<_, _>>();
    normalized.entry = block_ids[&normalized.entry];
    for (index, parameter) in normalized.params.iter_mut().enumerate() {
        parameter.symbol = SymbolId(index as u32);
        parameter.local = local_ids[&parameter.local];
        parameter.value = value_ids[&parameter.value];
        parameter.name = "";
        parameter.span = empty;
    }
    for (index, local) in normalized.locals.iter_mut().enumerate() {
        local.id = local_ids[&local.id];
        local.symbol = SymbolId((normalized.params.len() + index) as u32);
        local.name = "";
        local.span = empty;
    }
    for block in &mut normalized.blocks {
        block.id = block_ids[&block.id];
        block.span = empty;
        for phi in &mut block.phis {
            phi.out = value_ids[&phi.out];
            phi.local = local_ids[&phi.local];
            for (predecessor, value) in &mut phi.incoming {
                *predecessor = block_ids[predecessor];
                *value = value_ids[value];
            }
            phi.span = empty;
        }
        for instruction in &mut block.instructions {
            if let Some(out) = &mut instruction.out {
                *out = value_ids[out];
            }
            rewrite_control_flow_values(&mut instruction.op, |value| {
                *value = value_ids[value];
            });
            match &mut instruction.op {
                ControlFlowOp::LoadLocal(local) => *local = local_ids[local],
                ControlFlowOp::StoreLocal { local, .. } => *local = local_ids[local],
                ControlFlowOp::NewClass {
                    constructor: Some(reference),
                    ..
                }
                | ControlFlowOp::Closure {
                    function: reference,
                    ..
                }
                | ControlFlowOp::CallDirect {
                    function: reference,
                    ..
                }
                | ControlFlowOp::CallMethod {
                    function: reference,
                    ..
                } if *reference == original_id => *reference = SELF_REFERENCE,
                _ => {}
            }
            instruction.span = empty;
        }
        if let Some(terminator) = &mut block.terminator {
            match terminator {
                Terminator::Jump(target) => *target = block_ids[target],
                Terminator::Branch {
                    condition,
                    then_block,
                    else_block,
                } => {
                    *condition = value_ids[condition];
                    *then_block = block_ids[then_block];
                    *else_block = block_ids[else_block];
                }
                Terminator::Return(Some(value)) => *value = value_ids[value],
                Terminator::Return(None) | Terminator::Unreachable => {}
            }
        }
    }
    for shape in &mut normalized.shapes {
        match shape {
            crate::ir::ControlShape::If {
                header,
                then_block,
                else_block,
                merge_block,
            } => {
                *header = block_ids[header];
                *then_block = block_ids[then_block];
                *else_block = block_ids[else_block];
                *merge_block = block_ids[merge_block];
            }
            crate::ir::ControlShape::Loop {
                header,
                body,
                update,
                exit,
            } => {
                *header = block_ids[header];
                *body = block_ids[body];
                if let Some(update) = update {
                    *update = block_ids[update];
                }
                *exit = block_ids[exit];
            }
        }
    }
    normalized.value_count = next_value;
    normalized
}

fn eliminate_dead_functions(module: &mut ControlFlowModule<'_>) -> OptimizationReport {
    let mut reachable = AHashSet::new();
    let mut work = vec![module.entry];
    work.extend(exported_functions(module));
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
        ControlFlowOp::Const(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => Vec::new(),
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => vec![*value],
        ControlFlowOp::Binary { lhs, rhs, .. } => vec![*lhs, *rhs],
        ControlFlowOp::Array(values) => values.clone(),
        ControlFlowOp::Struct { fields, .. } => fields.clone(),
        ControlFlowOp::NewClass { args, .. } => args.clone(),
        ControlFlowOp::Closure { captures, .. } => captures.clone(),
        ControlFlowOp::StoreLocal { value, .. } | ControlFlowOp::StoreGlobal { value, .. } => {
            vec![*value]
        }
        ControlFlowOp::FieldGet { object, .. } | ControlFlowOp::HostFieldGet { object, .. } => {
            vec![*object]
        }
        ControlFlowOp::FieldSet { object, value, .. }
        | ControlFlowOp::HostFieldSet { object, value, .. } => vec![*object, *value],
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
        ControlFlowOp::HostCall { receiver, args, .. } => {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum EffectRoot {
    Parameter(usize),
    Local(ValueId),
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct FunctionEffectSummary {
    inherent: bool,
    mutated_parameters: AHashSet<usize>,
}

fn analyze_function_effects(module: &ControlFlowModule<'_>) -> Vec<FunctionEffectSummary> {
    let mut summaries = vec![FunctionEffectSummary::default(); module.functions.len()];
    for function in &module.functions {
        if function.kind == FunctionKind::Extern && !function.declared_pure {
            if let Some(summary) = summaries.get_mut(function.id.0 as usize) {
                summary.inherent = true;
            }
        }
    }

    loop {
        let mut changed = false;
        for function in &module.functions {
            if function.kind == FunctionKind::Extern {
                continue;
            }
            let candidate = summarize_function_effects(function, &summaries);
            let summary = &mut summaries[function.id.0 as usize];
            if candidate.inherent && !summary.inherent {
                summary.inherent = true;
                changed = true;
            }
            for parameter in candidate.mutated_parameters {
                changed |= summary.mutated_parameters.insert(parameter);
            }
        }
        if !changed {
            return summaries;
        }
    }
}

fn summarize_function_effects(
    function: &ControlFlowFunction<'_>,
    summaries: &[FunctionEffectSummary],
) -> FunctionEffectSummary {
    let mut roots = function
        .params
        .iter()
        .enumerate()
        .map(|(index, parameter)| (parameter.value, EffectRoot::Parameter(index)))
        .collect::<AHashMap<_, _>>();
    roots.extend(
        local_mutable_roots(function, is_owned_mutable_allocation)
            .into_keys()
            .map(|value| (value, EffectRoot::Local(value))),
    );
    extend_mutable_alias_roots(function, &mut roots);

    let closures = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match (instruction.out, &instruction.op) {
            (Some(out), ControlFlowOp::Closure { function, captures }) => {
                Some((out, (*function, captures.clone())))
            }
            _ => None,
        })
        .collect::<AHashMap<_, _>>();

    let mut result = FunctionEffectSummary::default();
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        match &instruction.op {
            ControlFlowOp::StoreGlobal { .. }
            | ControlFlowOp::HostFieldGet { .. }
            | ControlFlowOp::HostFieldSet { .. }
            | ControlFlowOp::DynamicImport { .. } => result.inherent = true,
            ControlFlowOp::FieldSet { object, .. } | ControlFlowOp::IndexSet { object, .. } => {
                record_mutation(*object, &roots, &mut result);
            }
            ControlFlowOp::CallDirect { function, args } => {
                apply_callee_summary(
                    summaries.get(function.0 as usize),
                    args,
                    &roots,
                    &mut result,
                );
            }
            ControlFlowOp::CallValue { callee, args } => {
                let Some((function, captures)) = closures.get(callee) else {
                    result.inherent = true;
                    continue;
                };
                let actuals = captures.iter().chain(args).copied().collect::<Vec<_>>();
                apply_callee_summary(
                    summaries.get(function.0 as usize),
                    &actuals,
                    &roots,
                    &mut result,
                );
            }
            ControlFlowOp::CallMethod {
                receiver,
                function,
                args,
                ..
            } => {
                let actuals = std::iter::once(*receiver)
                    .chain(args.iter().copied())
                    .collect::<Vec<_>>();
                apply_callee_summary(
                    summaries.get(function.0 as usize),
                    &actuals,
                    &roots,
                    &mut result,
                );
            }
            ControlFlowOp::NewClass {
                constructor: Some(function),
                args,
                ..
            } => apply_constructor_summary(
                summaries.get(function.0 as usize),
                args,
                &roots,
                &mut result,
            ),
            ControlFlowOp::HostCall { pure, .. } => result.inherent |= !pure,
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::ArrayPush
                    | Intrinsic::ArrayPop
                    | Intrinsic::MapSet
                    | Intrinsic::MapDelete
                    | Intrinsic::MapClear
                    | Intrinsic::SetAdd
                    | Intrinsic::SetDelete
                    | Intrinsic::SetClear,
                receiver,
                ..
            } => {
                if let Some(receiver) = receiver {
                    record_mutation(*receiver, &roots, &mut result);
                } else {
                    result.inherent = true;
                }
            }
            ControlFlowOp::Intrinsic {
                intrinsic: Intrinsic::Print,
                ..
            } => result.inherent = true,
            ControlFlowOp::Intrinsic {
                intrinsic:
                    Intrinsic::ArrayMap
                    | Intrinsic::ArrayFilter
                    | Intrinsic::ArrayReduce
                    | Intrinsic::ArrayForEach,
                args,
                ..
            } => summarize_callback_effects(args, &closures, summaries, &roots, &mut result),
            ControlFlowOp::Const(_)
            | ControlFlowOp::Unary { .. }
            | ControlFlowOp::Binary { .. }
            | ControlFlowOp::TypeCheck { .. }
            | ControlFlowOp::Array(_)
            | ControlFlowOp::Struct { .. }
            | ControlFlowOp::NewClass {
                constructor: None, ..
            }
            | ControlFlowOp::Closure { .. }
            | ControlFlowOp::LoadLocal(_)
            | ControlFlowOp::StoreLocal { .. }
            | ControlFlowOp::LoadGlobal(_)
            | ControlFlowOp::FieldGet { .. }
            | ControlFlowOp::IndexGet { .. }
            | ControlFlowOp::Intrinsic { .. }
            | ControlFlowOp::Template(_) => {}
        }
    }
    result
}

fn record_mutation(
    value: ValueId,
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    match roots.get(&value) {
        Some(EffectRoot::Parameter(parameter)) => {
            result.mutated_parameters.insert(*parameter);
        }
        Some(EffectRoot::Local(_)) => {}
        None => result.inherent = true,
    }
}

fn apply_callee_summary(
    callee: Option<&FunctionEffectSummary>,
    args: &[ValueId],
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    let Some(callee) = callee else {
        result.inherent = true;
        return;
    };
    result.inherent |= callee.inherent;
    for parameter in &callee.mutated_parameters {
        if let Some(argument) = args.get(*parameter) {
            record_mutation(*argument, roots, result);
        } else {
            result.inherent = true;
        }
    }
}

fn apply_constructor_summary(
    constructor: Option<&FunctionEffectSummary>,
    args: &[ValueId],
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    let Some(constructor) = constructor else {
        result.inherent = true;
        return;
    };
    result.inherent |= constructor.inherent;
    for parameter in &constructor.mutated_parameters {
        if *parameter == 0 {
            continue;
        }
        if let Some(argument) = args.get(parameter - 1) {
            record_mutation(*argument, roots, result);
        } else {
            result.inherent = true;
        }
    }
}

fn summarize_callback_effects(
    args: &[ValueId],
    closures: &AHashMap<ValueId, (FunctionId, Vec<ValueId>)>,
    summaries: &[FunctionEffectSummary],
    roots: &AHashMap<ValueId, EffectRoot>,
    result: &mut FunctionEffectSummary,
) {
    let Some((function, captures)) = args.first().and_then(|callback| closures.get(callback))
    else {
        result.inherent = true;
        return;
    };
    let Some(summary) = summaries.get(function.0 as usize) else {
        result.inherent = true;
        return;
    };
    result.inherent |= summary.inherent;
    for parameter in &summary.mutated_parameters {
        if let Some(capture) = captures.get(*parameter) {
            record_mutation(*capture, roots, result);
        } else {
            result.inherent = true;
        }
    }
}

fn effectful_functions(summaries: &[FunctionEffectSummary]) -> AHashSet<FunctionId> {
    summaries
        .iter()
        .enumerate()
        .filter(|(_, summary)| summary.inherent || !summary.mutated_parameters.is_empty())
        .map(|(index, _)| FunctionId(index as u32))
        .collect()
}

fn find_effectful_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    effectful_functions(&analyze_function_effects(module))
}

fn validate_declared_purity(
    module: &ControlFlowModule<'_>,
) -> Result<OptimizationReport, SsaError> {
    let effectful = find_effectful_functions(module);
    if let Some(function) = module
        .functions
        .iter()
        .find(|function| function.declared_pure && effectful.contains(&function.id))
    {
        return Err(SsaError {
            span: function.span,
            message: format!(
                "function `{}` is declared `pure` but may perform an observable side effect",
                function.name.unwrap_or("<closure>")
            ),
        });
    }
    Ok(OptimizationReport {
        pass_name: "pure-contract-validation",
        changed: false,
    })
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
        | ControlFlowOp::HostFieldGet { .. }
        | ControlFlowOp::HostFieldSet { .. }
        | ControlFlowOp::IndexSet { .. }
        | ControlFlowOp::CallMethod { .. } => true,
        ControlFlowOp::HostCall { pure, .. } => !pure,
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
            Intrinsic::Print
                | Intrinsic::ArrayPush
                | Intrinsic::ArrayPop
                | Intrinsic::MapSet
                | Intrinsic::MapDelete
                | Intrinsic::MapClear
                | Intrinsic::SetAdd
                | Intrinsic::SetDelete
                | Intrinsic::SetClear
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
    let string_argument = || {
        args.first()
            .and_then(|value| constants.get(value))
            .and_then(|value| match value {
                ConstValue::String(value) => Some(value.as_str()),
                _ => None,
            })
    };
    match intrinsic {
        Intrinsic::StringLength => Some(ConstValue::Int(receiver.encode_utf16().count() as i64)),
        Intrinsic::StringCharCodeAt => {
            let index = args
                .first()
                .and_then(|value| constants.get(value))
                .and_then(|value| match value {
                    ConstValue::Int(value) => usize::try_from(*value).ok(),
                    _ => None,
                })?;
            Some(ConstValue::Int(i64::from(
                receiver.encode_utf16().nth(index).unwrap_or(0),
            )))
        }
        Intrinsic::StringIncludes => Some(ConstValue::Bool(receiver.contains(string_argument()?))),
        Intrinsic::StringStartsWith => {
            Some(ConstValue::Bool(receiver.starts_with(string_argument()?)))
        }
        Intrinsic::StringEndsWith => Some(ConstValue::Bool(receiver.ends_with(string_argument()?))),
        Intrinsic::StringToUpperCase if receiver.is_ascii() => {
            Some(ConstValue::String(receiver.to_ascii_uppercase()))
        }
        Intrinsic::StringToLowerCase if receiver.is_ascii() => {
            Some(ConstValue::String(receiver.to_ascii_lowercase()))
        }
        _ => None,
    }
}

fn js_min(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        return f64::NAN;
    }
    if lhs == 0.0 && rhs == 0.0 {
        return if lhs.is_sign_negative() || rhs.is_sign_negative() {
            -0.0
        } else {
            0.0
        };
    }
    if lhs < rhs {
        lhs
    } else {
        rhs
    }
}

fn js_max(lhs: f64, rhs: f64) -> f64 {
    if lhs.is_nan() || rhs.is_nan() {
        return f64::NAN;
    }
    if lhs == 0.0 && rhs == 0.0 {
        return if lhs.is_sign_positive() || rhs.is_sign_positive() {
            0.0
        } else {
            -0.0
        };
    }
    if lhs > rhs {
        lhs
    } else {
        rhs
    }
}

fn constant_string_value(value: &ConstValue) -> Option<String> {
    Some(match value {
        ConstValue::Int(value) => value.to_string(),
        ConstValue::Float(value) => value.to_string(),
        ConstValue::Bool(value) => value.to_string(),
        ConstValue::String(value) => value.clone(),
        ConstValue::Null => "null".to_string(),
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
    use IrBinaryOp::{
        Add, BitAnd, BitOr, Div, Eq, Greater, GreaterEq, Less, LessEq, Mod, Mul, NotEq, ShiftLeft,
        ShiftRight, Sub, UnsignedShiftRight, Xor,
    };

    if let Some((lhs, rhs)) = mixed_numeric_constants(lhs, rhs) {
        return match op {
            Add => Some(Float(lhs + rhs)),
            Sub => Some(Float(lhs - rhs)),
            Mul => Some(Float(lhs * rhs)),
            Div if rhs != 0.0 => Some(Float(lhs / rhs)),
            Eq => Some(Bool(lhs == rhs)),
            NotEq => Some(Bool(lhs != rhs)),
            Less => Some(Bool(lhs < rhs)),
            LessEq => Some(Bool(lhs <= rhs)),
            Greater => Some(Bool(lhs > rhs)),
            GreaterEq => Some(Bool(lhs >= rhs)),
            _ => None,
        };
    }

    match (op, lhs, rhs) {
        (Add, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_add(*rhs as i32)))),
        (Sub, Int(lhs), Int(rhs)) => Some(Int(i64::from((*lhs as i32).wrapping_sub(*rhs as i32)))),
        (Mul, Int(lhs), Int(rhs)) => {
            Some(Int(i64::from(js_i32_multiply(*lhs as i32, *rhs as i32))))
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
        (Less, Float(lhs), Float(rhs)) => Some(Bool(lhs < rhs)),
        (LessEq, Float(lhs), Float(rhs)) => Some(Bool(lhs <= rhs)),
        (Greater, Float(lhs), Float(rhs)) => Some(Bool(lhs > rhs)),
        (GreaterEq, Float(lhs), Float(rhs)) => Some(Bool(lhs >= rhs)),
        (IrBinaryOp::And, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs && *rhs)),
        (IrBinaryOp::Or, Bool(lhs), Bool(rhs)) => Some(Bool(*lhs || *rhs)),
        _ => None,
    }
}

fn js_i32_multiply(lhs: i32, rhs: i32) -> i32 {
    let product = f64::from(lhs) * f64::from(rhs);
    (product as i64 as u32) as i32
}

fn format_i32_radix(value: i32, radix: u32, unsigned: bool) -> String {
    const DIGITS: &[u8; 36] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    let negative = !unsigned && value < 0;
    let mut magnitude = if unsigned {
        value as u32
    } else {
        value.unsigned_abs()
    };
    let mut reversed = Vec::new();
    loop {
        reversed.push(DIGITS[(magnitude % radix) as usize]);
        magnitude /= radix;
        if magnitude == 0 {
            break;
        }
    }
    if negative {
        reversed.push(b'-');
    }
    reversed.reverse();
    String::from_utf8(reversed).expect("radix digits are ASCII")
}

fn mixed_numeric_constants(lhs: &ConstValue, rhs: &ConstValue) -> Option<(f64, f64)> {
    match (lhs, rhs) {
        (ConstValue::Int(lhs), ConstValue::Float(rhs)) => Some((*lhs as f64, *rhs)),
        (ConstValue::Float(lhs), ConstValue::Int(rhs)) => Some((*lhs, *rhs as f64)),
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
            span: function.span,
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
                    span: instruction.span,
                    message: "local load has no result value".to_string(),
                })?;
                let value = stacks[local.0 as usize]
                    .last()
                    .copied()
                    .ok_or_else(|| SsaError {
                        span: instruction.span,
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

    let block_span = function.blocks[block_index].span;
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
                    span: block_span,
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
        ControlFlowOp::Const(_)
        | ControlFlowOp::LoadLocal(_)
        | ControlFlowOp::LoadGlobal(_)
        | ControlFlowOp::DynamicImport { .. } => {}
        ControlFlowOp::Unary { value, .. } | ControlFlowOp::TypeCheck { value, .. } => {
            rewrite(value)
        }
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
        ControlFlowOp::FieldGet { object, .. } | ControlFlowOp::HostFieldGet { object, .. } => {
            rewrite(object)
        }
        ControlFlowOp::FieldSet { object, value, .. }
        | ControlFlowOp::HostFieldSet { object, value, .. } => {
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
        ControlFlowOp::HostCall { receiver, args, .. } => {
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
    fn specializes_constant_arguments_and_removes_unused_call_results() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int work(int mode,int value){if(mode==3){print(value);return 1;}return 2;}work(3,4);work(3,8);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "constant-parameter-specialization" && report.changed
        }));
        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "unused-return-optimization" && report.changed }));
        let parameters = output
            .split_once('(')
            .and_then(|(_, rest)| rest.split_once(')'))
            .map(|(parameters, _)| parameters)
            .expect("specialized function must be emitted");
        assert!(
            !parameters.is_empty() && !parameters.contains(','),
            "{output}"
        );
        assert!(!output.contains("return"), "{output}");
        assert!(!output.contains("a(3,"), "{output}");
    }

    #[test]
    fn folds_identical_private_functions_after_inlining_decisions() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int left(int value){return value*3+1;}int right(int item){return item*3+1;}print(left(read())+right(read()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let right = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("right"))
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "identical-private-function-folding" && report.changed
        }));
        assert!(!control_flow.functions[right.0 as usize].live);
        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            1
        );
    }

    #[test]
    fn preserves_exported_function_identity_during_identical_folding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int left(int value){return value*3+1;}int right(int item){return item*3+1;}print(left(read())+right(read()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let right = control_flow
            .functions
            .iter()
            .find(|function| function.name == Some("right"))
            .map(|function| function.id)
            .unwrap();
        control_flow.exports.push(crate::ir::IrExport {
            name: "right",
            binding: ExportBinding::Function(right),
            span: S,
        });
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, true).unwrap();

        assert!(control_flow.functions[right.0 as usize].live);
        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            2
        );
    }

    #[test]
    fn preserves_address_taken_function_identity_during_identical_folding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();extern void retain(func(int)->int callback);int left(int value){return value*3+1;}int right(int item){return item*3+1;}retain(right);print(left(read()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            2
        );
    }

    #[test]
    fn preserves_distinct_escape_contracts_during_identical_folding() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Box{int value;}extern void retain(Box box);int left(Box box){return box.value;}int right(Box item){return item.value;}Box local=Box{1};Box escaped=Box{2};retain(escaped);print(left(local)+right(escaped));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            call_site_specialization: false,
            capture_signature_cloning: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert_eq!(
            control_flow
                .functions
                .iter()
                .filter(|function| function.live && function.kind == FunctionKind::Function)
                .count(),
            2
        );
    }

    #[test]
    fn normalizes_phi_locals_removed_from_function_metadata() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            include_str!("../tests/cases/nested_short_circuit.lil"),
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();

        let reports = optimize_control_flow(&mut control_flow).unwrap();

        assert!(reports
            .iter()
            .any(|report| report.pass_name == "identical-private-function-folding"));
    }

    #[test]
    fn does_not_specialize_tagged_generic_parameters_as_raw_constants() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "T apply<T>(T value,func(T)->T transform){return transform(value);}int triple(int value){return value*3;}print(apply(3,triple));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let generic = control_flow
            .functions
            .iter()
            .find(|function| {
                function
                    .params
                    .iter()
                    .any(|parameter| matches!(parameter.ty, Type::TypeParameter(_)))
            })
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(control_flow.functions[generic.0 as usize]
            .params
            .iter()
            .any(|parameter| matches!(parameter.ty, Type::TypeParameter(_))));
    }

    #[test]
    fn specializes_tagged_generic_constants_for_javascript_only() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "T apply<T>(T value,func(T)->T transform){return transform(value);}int triple(int value){return value*3;}print(apply(3,triple));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let generic = control_flow
            .functions
            .iter()
            .find(|function| {
                function
                    .params
                    .iter()
                    .any(|parameter| matches!(parameter.ty, Type::TypeParameter(_)))
            })
            .map(|function| function.id)
            .unwrap();
        let options = OptimizationOptions {
            inlining: false,
            specialize_tagged_constants: true,
            ..OptimizationOptions::default()
        };

        let reports =
            optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "constant-parameter-specialization" && report.changed
        }));
        assert!(control_flow.functions[generic.0 as usize]
            .params
            .iter()
            .all(|parameter| !matches!(parameter.ty, Type::TypeParameter(_))));
    }

    #[test]
    fn folds_interprocedural_finite_values_without_dropping_effectful_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string mode(){print(\"effect\");return \"active\";}string render(string value){if(value==\"active\"){return \"A\";}return \"B\";}print(render(mode()));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(output.contains("effect"), "{output}");
        assert!(output.contains("\"A\""), "{output}");
        assert!(!output.contains("===\"active\""), "{output}");
    }

    #[test]
    fn folds_exact_nominal_field_values_across_typed_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Theme{string name;}string render(Theme theme){if(theme.name==\"dark\"){return \"D\";}return \"L\";}Theme theme=Theme{\"dark\"};print(render(theme));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut control_flow = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            scalar_replacement: false,
            ..OptimizationOptions::default()
        };

        optimize_control_flow_with_options(&mut control_flow, &options, false).unwrap();
        let output = crate::codegen_ir_js::emit_optimized_ir_js(&control_flow).unwrap();

        assert!(output.contains("\"D\""), "{output}");
        assert!(!output.contains("===\"dark\""), "{output}");
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
    fn folds_mixed_numeric_constants_by_value() {
        assert_eq!(
            fold_binary(IrBinaryOp::Eq, &ConstValue::Int(7), &ConstValue::Float(7.0),),
            Some(ConstValue::Bool(true))
        );
        assert_eq!(
            fold_binary(
                IrBinaryOp::Add,
                &ConstValue::Float(0.5),
                &ConstValue::Int(2),
            ),
            Some(ConstValue::Float(2.5))
        );
    }

    #[test]
    fn folds_integer_multiplication_with_javascript_operator_semantics() {
        assert_eq!(js_i32_multiply(2_147_483_647, 2_147_483_647), 0);
        assert_eq!(
            js_i32_multiply(2_147_483_647, 2_147_483_646),
            -2_147_483_648
        );
        assert_eq!(js_i32_multiply(123_456_789, 987_654_321), -67_153_024);
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
            .filter(|function| function.live)
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
    fn independently_controls_closure_factory_inlining() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "func(int)->int make(int offset){return (int value)=>value+offset;}func(int)->int add=make(2);print(add(3));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let original = lower_to_control_flow(&program, &semantics).unwrap();
        let factory = original
            .functions
            .iter()
            .find(|function| function.name == Some("make"))
            .unwrap()
            .id;

        let mut inlined = original.clone();
        optimize_control_flow(&mut inlined).unwrap();
        assert!(!has_direct_call(&inlined, factory));

        let mut outlined = original;
        optimize_control_flow_with_options(
            &mut outlined,
            &OptimizationOptions {
                inline_closure_factories: false,
                ..OptimizationOptions::default()
            },
            false,
        )
        .unwrap();
        assert!(has_direct_call(&outlined, factory));
    }

    fn has_direct_call(module: &ControlFlowModule<'_>, target: FunctionId) -> bool {
        module
            .functions
            .iter()
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .any(|instruction| {
                matches!(instruction.op, ControlFlowOp::CallDirect { function, .. } if function == target)
            })
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

    #[test]
    fn folds_literal_array_lengths_across_unrelated_effects_only() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void tick();int[] values=[1,2,3];tick();print(values.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::Array(_)
                    | ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayLength,
                        ..
                    }
            )));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern void mutate(int[] values);int[] values=[1,2,3];mutate(values);print(values.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayLength,
                    ..
                }
            )));

        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1,2,3];int[] mapped=values.map((int value)=>value*2);mapped.push(8);print(mapped.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayLength,
                    ..
                }
            )));
    }

    #[test]
    fn folds_interprocedural_array_lengths_only_for_closed_stable_calls() {
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        let has_array_length = |source: &str, preserve_exports: bool| {
            let arena = Bump::new();
            let program = parse_source(&arena, source).unwrap();
            let semantics = analyze(&program).unwrap();
            let mut module = lower_to_control_flow(&program, &semantics).unwrap();
            optimize_control_flow_with_options(&mut module, &options, preserve_exports).unwrap();
            module
                .functions
                .iter()
                .flat_map(|function| &function.blocks)
                .flat_map(|block| &block.instructions)
                .any(|instruction| {
                    matches!(
                        instruction.op,
                        ControlFlowOp::Intrinsic {
                            intrinsic: Intrinsic::ArrayLength,
                            ..
                        }
                    )
                })
        };

        assert!(!has_array_length(
            "int count(int[] values){return values.length;}print(count([1,2,3]));print(count([4,5,6]));",
            false,
        ));
        assert!(has_array_length(
            "int count(int[] values){return values.length;}print(count([1]));print(count([1,2]));",
            false,
        ));
        assert!(has_array_length(
            "int count(int[] values){values.push(1);return values.length;}print(count([1,2,3]));",
            false,
        ));
        assert!(has_array_length(
            "export int count(int[] values){return values.length;}print(count([1,2,3]));",
            true,
        ));
    }

    #[test]
    fn removes_unobserved_local_collection_mutation_graphs() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1];values.push(2);values[0]=3;Map<string,int> map=new Map<string,int>();map.set(\"a\",1).set(\"b\",2);Set<int> set=new Set<int>();set.add(1).add(2);print(7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(
                instruction.op,
                ControlFlowOp::Array(_)
                    | ControlFlowOp::IndexSet { .. }
                    | ControlFlowOp::Intrinsic {
                        intrinsic: Intrinsic::ArrayPush
                            | Intrinsic::MapNew
                            | Intrinsic::MapSet
                            | Intrinsic::SetNew
                            | Intrinsic::SetAdd,
                        ..
                    }
            )));
    }

    #[test]
    fn infers_fluent_local_collection_helpers_are_effect_free() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void scratch(){Map<string,int> map=new Map<string,int>();map.set(\"a\",1).set(\"b\",2);Set<int> set=new Set<int>();set.add(1).add(2);}scratch();print(7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallDirect { .. })));
        assert!(module
            .functions
            .iter()
            .any(|function| { function.name == Some("scratch") && !function.live }));
    }

    #[test]
    fn removes_parameter_mutation_calls_only_for_unobserved_roots() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void fill(int[] left,int[] right){left.push(1);right.push(2);}void forward(int[] left,int[] right){fill(left,right);}int[] deadLeft=[];int[] deadRight=[];forward(deadLeft,deadRight);int[] live=[2];int[] linked=[];forward(live,linked);print(live.length);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let entry = &module.functions[module.entry.0 as usize];
        let calls = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. }))
            .count();
        let arrays = entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter(|instruction| matches!(instruction.op, ControlFlowOp::Array(_)))
            .count();

        assert_eq!(calls, 1);
        assert_eq!(arrays, 2);
    }

    #[test]
    fn preserves_parameter_mutation_calls_with_inherent_effects() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "void noisy(int[] values){values.push(1);print(9);}int[] values=[];noisy(values);print(7);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut module, &options, false).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::CallDirect { .. })));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(instruction.op, ControlFlowOp::Array(_))));
    }

    #[test]
    fn preserves_collection_mutations_with_observed_state_or_results() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int[] values=[1];int length=values.push(2);print(length);Map<string,int> map=new Map<string,int>();map.set(\"a\",1);print(map.get(\"a\"));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut module).unwrap();
        let entry = &module.functions[module.entry.0 as usize];

        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::ArrayPush,
                    ..
                }
            )));
        assert!(entry
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .any(|instruction| matches!(
                instruction.op,
                ControlFlowOp::Intrinsic {
                    intrinsic: Intrinsic::MapSet,
                    ..
                }
            )));
    }

    #[test]
    fn profile_guidance_specializes_higher_order_calls_and_devirtualizes_them() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int apply(func(int)->int operation,int value){return operation(value);}int increment(int value){return value+1;}print(apply(increment,41));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            specialize_tagged_constants: true,
            ..OptimizationOptions::default()
        };
        let mut profile = OptimizationProfile::default();
        profile.functions.insert("$entry".to_string(), 10_000);
        let reports = optimize_control_flow_with_guidance(
            &mut module,
            &options,
            false,
            &OptimizationGuidance {
                profile,
                specialization_min_count: 10,
                max_specializations_per_function: 4,
                max_clone_instructions: 64,
            },
        )
        .unwrap();

        assert!(reports.iter().any(|report| {
            report.pass_name == "profiled-call-site-specialization" && report.changed
        }));
        assert!(module
            .functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .all(|instruction| !matches!(instruction.op, ControlFlowOp::CallValue { .. })));
    }

    #[test]
    fn profile_guidance_clones_constant_capture_signatures() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "func(int)->int make(int offset){return (int value)=>value+offset;}func(int)->int add=make(2);print(add(40));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut module = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        let mut profile = OptimizationProfile::default();
        profile.functions.insert("$entry".to_string(), 10_000);
        profile.functions.insert("make".to_string(), 10_000);
        let reports = optimize_control_flow_with_guidance(
            &mut module,
            &options,
            false,
            &OptimizationGuidance {
                profile,
                specialization_min_count: 10,
                max_specializations_per_function: 4,
                max_clone_instructions: 64,
            },
        )
        .unwrap();

        assert!(reports
            .iter()
            .any(|report| { report.pass_name == "capture-signature-cloning" && report.changed }));
        assert!(module
            .functions
            .iter()
            .filter(|function| function.live)
            .flat_map(|function| &function.blocks)
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match &instruction.op {
                ControlFlowOp::Closure { captures, .. } => Some(captures),
                _ => None,
            })
            .all(Vec::is_empty));
    }
}
