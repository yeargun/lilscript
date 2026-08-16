use std::collections::{BTreeMap, VecDeque};

use crate::stable_hash::StableHashSet as AHashSet;
use serde::{Deserialize, Serialize};

use crate::codegen_ir_js::{ControlFlowSpelling, IrJsOptions};
use crate::ir::{
    BlockId, ControlFlowFunction, ControlFlowModule, ControlFlowOp, ControlShape, FunctionKind,
    Intrinsic,
};
use crate::semantic::Type;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OptimizationProfile {
    pub version: u32,
    pub functions: BTreeMap<String, u64>,
    pub loops: BTreeMap<String, u64>,
}

impl Default for OptimizationProfile {
    fn default() -> Self {
        Self {
            version: 1,
            functions: BTreeMap::new(),
            loops: BTreeMap::new(),
        }
    }
}

impl OptimizationProfile {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err(format!(
                "unsupported optimization profile version {}; expected 1",
                self.version
            ));
        }
        if self
            .functions
            .values()
            .chain(self.loops.values())
            .any(|count| *count == 0)
        {
            return Err("optimization profile counters must be greater than zero".to_string());
        }
        Ok(())
    }

    pub fn merge(&mut self, overlay: &Self) {
        self.functions.extend(overlay.functions.clone());
        self.loops.extend(overlay.loops.clone());
    }

    pub fn function_count(&self, function: &ControlFlowFunction<'_>) -> u64 {
        self.functions
            .get(&function_profile_key(function))
            .copied()
            .unwrap_or(1)
    }

    pub fn loop_count(&self, function: &ControlFlowFunction<'_>, shape_index: usize) -> u64 {
        self.loops
            .get(&loop_profile_key(function, shape_index))
            .copied()
            .unwrap_or_else(|| self.function_count(function))
    }

    pub fn block_count(&self, function: &ControlFlowFunction<'_>, block: BlockId) -> u64 {
        let mut count = self.function_count(function);
        for (shape_index, shape) in function.shapes.iter().enumerate() {
            let ControlShape::Loop { body, exit, .. } = shape else {
                continue;
            };
            if blocks_until_exit(function, *body, *exit).contains(&block) {
                count = count.max(self.loop_count(function, shape_index));
            }
        }
        count
    }
}

pub fn function_profile_key(function: &ControlFlowFunction<'_>) -> String {
    match (function.kind, function.name) {
        (FunctionKind::Entry, _) => "$entry".to_string(),
        (FunctionKind::Method { class }, Some(name)) => format!("{class}.{name}"),
        (FunctionKind::Constructor { class }, _) => format!("{class}.constructor"),
        (FunctionKind::Closure, Some(name)) => format!("{name}#closure@{}", function.span.start),
        (FunctionKind::Closure, None) => format!("$closure@{}", function.span.start),
        (_, Some(name)) => name.to_string(),
        (_, None) => format!("$function@{}", function.span.start),
    }
}

pub fn loop_profile_key(function: &ControlFlowFunction<'_>, shape_index: usize) -> String {
    format!("{}#{shape_index}", function_profile_key(function))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct JavaScriptPerformanceMetrics {
    pub deoptimization_risk: u64,
    pub allocation_pressure: u64,
    pub indirect_call_pressure: u64,
    pub monomorphic_call_sites: u64,
    pub hot_code_pressure: u64,
    pub score: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JavaScriptPerformanceWeights {
    pub deoptimization: u32,
    pub allocation: u32,
    pub indirect_call: u32,
    pub hot_code: u32,
}

impl JavaScriptPerformanceMetrics {
    pub fn with_score(mut self, weights: JavaScriptPerformanceWeights) -> Self {
        self.score = self
            .deoptimization_risk
            .saturating_mul(u64::from(weights.deoptimization))
            .saturating_add(
                self.allocation_pressure
                    .saturating_mul(u64::from(weights.allocation)),
            )
            .saturating_add(
                self.indirect_call_pressure
                    .saturating_mul(u64::from(weights.indirect_call)),
            )
            .saturating_add(
                self.hot_code_pressure
                    .saturating_mul(u64::from(weights.hot_code)),
            );
        self
    }
}

pub fn analyze_javascript_performance(
    module: &ControlFlowModule<'_>,
    options: &IrJsOptions,
    profile: &OptimizationProfile,
    weights: JavaScriptPerformanceWeights,
) -> JavaScriptPerformanceMetrics {
    let mut metrics = JavaScriptPerformanceMetrics::default();
    for function in &module.functions {
        if !function.live || function.kind == FunctionKind::Extern {
            continue;
        }
        let function_weight = execution_weight(profile.function_count(function));
        let instruction_count = function
            .blocks
            .iter()
            .map(|block| block.instructions.len() + block.phis.len())
            .sum::<usize>() as u64;
        metrics.hot_code_pressure = metrics
            .hot_code_pressure
            .saturating_add(instruction_count.saturating_mul(function_weight));

        if options.control_flow_spelling == ControlFlowSpelling::StateMachine {
            metrics.deoptimization_risk = metrics.deoptimization_risk.saturating_add(
                (function.blocks.len().saturating_sub(1) as u64)
                    .saturating_mul(function_weight)
                    .saturating_mul(2),
            );
        }

        let known_closures = function
            .blocks
            .iter()
            .flat_map(|block| &block.instructions)
            .filter_map(|instruction| match (instruction.out, &instruction.op) {
                (Some(out), ControlFlowOp::Closure { .. }) => Some(out),
                _ => None,
            })
            .collect::<AHashSet<_>>();
        for block in &function.blocks {
            let weight = execution_weight(profile.block_count(function, block.id));
            for instruction in &block.instructions {
                if instruction
                    .ty
                    .as_ref()
                    .is_some_and(type_has_dynamic_runtime_shape)
                {
                    metrics.deoptimization_risk =
                        metrics.deoptimization_risk.saturating_add(weight);
                }
                match &instruction.op {
                    ControlFlowOp::CallDirect { .. } | ControlFlowOp::CallMethod { .. } => {
                        metrics.monomorphic_call_sites =
                            metrics.monomorphic_call_sites.saturating_add(weight);
                    }
                    ControlFlowOp::CallValue { callee, .. } => {
                        if known_closures.contains(callee) {
                            metrics.monomorphic_call_sites =
                                metrics.monomorphic_call_sites.saturating_add(weight);
                        } else {
                            metrics.indirect_call_pressure =
                                metrics.indirect_call_pressure.saturating_add(weight);
                            metrics.deoptimization_risk = metrics
                                .deoptimization_risk
                                .saturating_add(weight.saturating_mul(2));
                        }
                    }
                    ControlFlowOp::HostCall { .. }
                    | ControlFlowOp::HostFieldGet { .. }
                    | ControlFlowOp::HostFieldSet { .. } => {
                        metrics.deoptimization_risk =
                            metrics.deoptimization_risk.saturating_add(weight);
                    }
                    operation if allocation_units(operation).is_some() => {
                        metrics.allocation_pressure = metrics.allocation_pressure.saturating_add(
                            weight.saturating_mul(allocation_units(operation).unwrap_or(0)),
                        );
                    }
                    _ => {}
                }
            }
        }
    }
    metrics.with_score(weights)
}

fn execution_weight(count: u64) -> u64 {
    1 + count.ilog2() as u64
}

fn type_has_dynamic_runtime_shape(ty: &Type<'_>) -> bool {
    match ty {
        Type::Union(_) | Type::TypeParameter(_) | Type::GenericFunction(_) => true,
        Type::Array(element) | Type::Set(element) => type_has_dynamic_runtime_shape(element),
        Type::Map(key, value) => {
            type_has_dynamic_runtime_shape(key) || type_has_dynamic_runtime_shape(value)
        }
        Type::Function(signature) => {
            signature.params.iter().any(type_has_dynamic_runtime_shape)
                || type_has_dynamic_runtime_shape(&signature.return_type)
        }
        _ => false,
    }
}

fn allocation_units(operation: &ControlFlowOp<'_>) -> Option<u64> {
    match operation {
        ControlFlowOp::Array(_) | ControlFlowOp::Struct { .. } => Some(2),
        ControlFlowOp::NewClass { .. } => Some(3),
        ControlFlowOp::Closure { captures, .. } if !captures.is_empty() => Some(2),
        ControlFlowOp::Intrinsic {
            intrinsic:
                Intrinsic::ArrayMap
                | Intrinsic::ArrayFilter
                | Intrinsic::MapNew
                | Intrinsic::SetNew
                | Intrinsic::ArrayBufferNew
                | Intrinsic::SharedArrayBufferNew
                | Intrinsic::BufferSlice
                | Intrinsic::StringToUpperCase
                | Intrinsic::StringToLowerCase,
            ..
        } => Some(2),
        ControlFlowOp::Intrinsic { intrinsic, .. }
            if matches!(
                crate::typed_array::classify_typed_array_intrinsic(*intrinsic),
                Some((
                    _,
                    crate::typed_array::TypedArrayIntrinsic::New
                        | crate::typed_array::TypedArrayIntrinsic::Slice
                        | crate::typed_array::TypedArrayIntrinsic::Subarray
                ))
            ) =>
        {
            Some(2)
        }
        ControlFlowOp::Template(_) => Some(1),
        _ => None,
    }
}

fn blocks_until_exit(
    function: &ControlFlowFunction<'_>,
    start: BlockId,
    exit: BlockId,
) -> AHashSet<BlockId> {
    let mut blocks = AHashSet::default();
    let mut queue = VecDeque::from([start]);
    while let Some(block) = queue.pop_front() {
        if block == exit || !blocks.insert(block) {
            continue;
        }
        let Some(data) = function.blocks.get(block.0 as usize) else {
            continue;
        };
        match data.terminator.as_ref() {
            Some(crate::ir::Terminator::Jump(target)) => queue.push_back(*target),
            Some(crate::ir::Terminator::Branch {
                then_block,
                else_block,
                ..
            }) => {
                queue.push_back(*then_block);
                queue.push_back(*else_block);
            }
            _ => {}
        }
    }
    blocks
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::lower::lower_to_control_flow;
    use crate::optimizer::optimize_control_flow;
    use crate::parser::parse_source;
    use crate::semantic::analyze;

    #[test]
    fn profile_keys_and_hot_loops_weight_performance_pressure() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int apply(func(int)->int operation,int value){return operation(value);} int identity(int value){return value;} for(int i=0;i<3;i++){print(apply(identity,i));}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        optimize_control_flow(&mut ir).unwrap();

        let entry = &ir.functions[ir.entry.0 as usize];
        let mut cold = OptimizationProfile::default();
        let cold_metrics = analyze_javascript_performance(
            &ir,
            &IrJsOptions::default(),
            &cold,
            JavaScriptPerformanceWeights {
                deoptimization: 1,
                allocation: 1,
                indirect_call: 1,
                hot_code: 1,
            },
        );
        cold.loops.insert(loop_profile_key(entry, 0), 1_000_000);
        let hot_metrics = analyze_javascript_performance(
            &ir,
            &IrJsOptions::default(),
            &cold,
            JavaScriptPerformanceWeights {
                deoptimization: 1,
                allocation: 1,
                indirect_call: 1,
                hot_code: 1,
            },
        );
        assert!(hot_metrics.hot_code_pressure >= cold_metrics.hot_code_pressure);
        assert_eq!(function_profile_key(entry), "$entry");
    }

    #[test]
    fn rejects_unknown_profile_versions_and_zero_counts() {
        let mut profile = OptimizationProfile {
            version: 2,
            ..OptimizationProfile::default()
        };
        assert!(profile.validate().is_err());
        profile.version = 1;
        profile.functions.insert("hot".to_string(), 0);
        assert!(profile.validate().is_err());
    }
}
