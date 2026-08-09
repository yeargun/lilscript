use ahash::{AHashMap, AHashSet};

use crate::ir::{
    ConstValue, ControlFlowFunction, ControlFlowModule, ControlFlowOp, ControlShape, ExportBinding,
    FunctionId, FunctionKind, Intrinsic, IrBinaryOp, IrUnaryOp, Terminator, ValueId,
};
use crate::semantic::{EscapeState, Type};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct I32Range {
    pub min: i64,
    pub max: i64,
}

impl I32Range {
    pub const FULL: Self = Self {
        min: i32::MIN as i64,
        max: i32::MAX as i64,
    };

    pub fn exact(value: i64) -> Self {
        Self::checked(value, value).unwrap_or(Self::FULL)
    }

    pub fn checked(min: i64, max: i64) -> Option<Self> {
        (min >= i64::from(i32::MIN) && max <= i64::from(i32::MAX) && min <= max)
            .then_some(Self { min, max })
    }

    fn add(self, rhs: Self) -> (Self, bool) {
        Self::checked(self.min + rhs.min, self.max + rhs.max)
            .map_or((Self::FULL, false), |range| (range, true))
    }

    fn sub(self, rhs: Self) -> (Self, bool) {
        Self::checked(self.min - rhs.max, self.max - rhs.min)
            .map_or((Self::FULL, false), |range| (range, true))
    }

    fn mul(self, rhs: Self) -> (Self, bool) {
        let products = [
            self.min * rhs.min,
            self.min * rhs.max,
            self.max * rhs.min,
            self.max * rhs.max,
        ];
        Self::checked(
            *products.iter().min().expect("four products"),
            *products.iter().max().expect("four products"),
        )
        .map_or((Self::FULL, false), |range| (range, true))
    }

    fn neg(self) -> (Self, bool) {
        Self::checked(-self.max, -self.min).map_or((Self::FULL, false), |range| (range, true))
    }

    fn modulo(self, rhs: Self) -> (Self, bool) {
        if rhs.min <= 0 && rhs.max >= 0 {
            return (Self::FULL, false);
        }
        let bound = rhs.min.unsigned_abs().max(rhs.max.unsigned_abs()) as i64 - 1;
        let min = if self.min < 0 {
            self.min.max(-bound)
        } else {
            0
        };
        let max = if self.max > 0 { self.max.min(bound) } else { 0 };
        (Self { min, max }, true)
    }

    fn join(self, rhs: Self) -> Self {
        Self {
            min: self.min.min(rhs.min),
            max: self.max.max(rhs.max),
        }
    }

    fn widen(self, next: Self) -> Self {
        Self {
            min: if next.min < self.min {
                i64::from(i32::MIN)
            } else {
                self.min
            },
            max: if next.max > self.max {
                i64::from(i32::MAX)
            } else {
                self.max
            },
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FunctionIntegerFacts {
    ranges: AHashMap<ValueId, I32Range>,
    elidable_coercions: AHashSet<ValueId>,
    return_range: Option<I32Range>,
}

impl FunctionIntegerFacts {
    pub fn range(&self, value: ValueId) -> Option<I32Range> {
        self.ranges.get(&value).copied()
    }

    pub fn ranges(&self) -> &AHashMap<ValueId, I32Range> {
        &self.ranges
    }

    pub fn can_elide_coercion(&self, value: ValueId) -> bool {
        self.elidable_coercions.contains(&value)
    }

    pub fn return_range(&self) -> Option<I32Range> {
        self.return_range
    }
}

#[derive(Debug, Clone, Default)]
pub struct IntegerValueAnalysis {
    functions: Vec<FunctionIntegerFacts>,
    field_ranges: AHashMap<String, AHashMap<usize, I32Range>>,
}

const FINITE_VALUE_LIMIT: usize = 4;

#[derive(Debug, Clone, PartialEq)]
pub struct FiniteValueSet {
    values: Vec<ConstValue>,
}

impl FiniteValueSet {
    fn singleton(value: ConstValue) -> Option<Self> {
        is_finite_constant(&value).then_some(Self {
            values: vec![value],
        })
    }

    fn union(&self, other: &Self) -> Option<Self> {
        let mut values = self.values.clone();
        for value in &other.values {
            if !values.contains(value) {
                values.push(value.clone());
                if values.len() > FINITE_VALUE_LIMIT {
                    return None;
                }
            }
        }
        Some(Self { values })
    }

    pub fn values(&self) -> &[ConstValue] {
        &self.values
    }

    pub fn constant(&self) -> Option<&ConstValue> {
        (self.values.len() == 1).then(|| &self.values[0])
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
enum FiniteSummary {
    #[default]
    Bottom,
    Values(FiniteValueSet),
    Unknown,
}

impl FiniteSummary {
    fn from_constant(value: ConstValue) -> Self {
        FiniteValueSet::singleton(value).map_or(Self::Unknown, Self::Values)
    }

    fn join(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::Unknown, _) | (_, Self::Unknown) => Self::Unknown,
            (Self::Bottom, value) | (value, Self::Bottom) => value.clone(),
            (Self::Values(lhs), Self::Values(rhs)) => {
                lhs.union(rhs).map_or(Self::Unknown, Self::Values)
            }
        }
    }

    fn values(&self) -> Option<&FiniteValueSet> {
        match self {
            Self::Values(values) => Some(values),
            Self::Bottom | Self::Unknown => None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FunctionFiniteFacts {
    values: AHashMap<ValueId, FiniteValueSet>,
    return_values: Option<FiniteValueSet>,
}

impl FunctionFiniteFacts {
    pub fn values(&self, value: ValueId) -> Option<&FiniteValueSet> {
        self.values.get(&value)
    }

    pub fn constant(&self, value: ValueId) -> Option<&ConstValue> {
        self.values(value).and_then(FiniteValueSet::constant)
    }

    pub fn constants(&self) -> impl Iterator<Item = (ValueId, &ConstValue)> {
        self.values
            .iter()
            .filter_map(|(value, values)| values.constant().map(|constant| (*value, constant)))
    }

    pub fn return_values(&self) -> Option<&FiniteValueSet> {
        self.return_values.as_ref()
    }

    pub fn return_constant(&self) -> Option<&ConstValue> {
        self.return_values().and_then(FiniteValueSet::constant)
    }
}

#[derive(Debug, Clone, Default)]
pub struct FiniteValueAnalysis {
    functions: Vec<FunctionFiniteFacts>,
    field_values: AHashMap<String, AHashMap<usize, FiniteValueSet>>,
}

impl FiniteValueAnalysis {
    pub fn function(&self, function: FunctionId) -> &FunctionFiniteFacts {
        &self.functions[function.0 as usize]
    }

    pub fn field_values(&self, owner: &str, index: usize) -> Option<&FiniteValueSet> {
        self.field_values
            .get(owner)
            .and_then(|fields| fields.get(&index))
    }

    pub fn field_constant(&self, owner: &str, index: usize) -> Option<&ConstValue> {
        self.field_values(owner, index)
            .and_then(FiniteValueSet::constant)
    }
}

impl IntegerValueAnalysis {
    pub fn function(&self, function: FunctionId) -> &FunctionIntegerFacts {
        &self.functions[function.0 as usize]
    }

    pub fn field_range(&self, owner: &str, index: usize) -> Option<I32Range> {
        self.field_ranges
            .get(owner)
            .and_then(|fields| fields.get(&index))
            .copied()
    }
}

pub fn analyze_integer_values(module: &ControlFlowModule<'_>) -> IntegerValueAnalysis {
    let exported = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Function(function) => Some(function),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let indirect = indirectly_called_functions(module);
    let unsafe_fields = aggregate_owners_exposed_to_untyped_code(module);
    let mut parameter_ranges = module
        .functions
        .iter()
        .map(|function| {
            function
                .params
                .iter()
                .map(|parameter| {
                    (parameter.ty == Type::Int
                        && (function.kind == FunctionKind::Extern
                            || exported.contains(&function.id)
                            || indirect.contains(&function.id)))
                    .then_some(I32Range::FULL)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut return_ranges = vec![None; module.functions.len()];
    let mut field_ranges = default_class_field_ranges(module, &unsafe_fields);
    loop {
        let next_facts = module
            .functions
            .iter()
            .map(|function| {
                analyze_function(
                    function,
                    &parameter_ranges[function.id.0 as usize],
                    &return_ranges,
                    &field_ranges,
                    module,
                )
            })
            .collect::<Vec<_>>();
        let mut proposed_parameters = module
            .functions
            .iter()
            .map(|function| {
                function
                    .params
                    .iter()
                    .map(|parameter| {
                        (parameter.ty == Type::Int
                            && (function.kind == FunctionKind::Extern
                                || exported.contains(&function.id)
                                || indirect.contains(&function.id)))
                        .then_some(I32Range::FULL)
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut proposed_fields = default_class_field_ranges(module, &unsafe_fields);

        for function in &module.functions {
            let local = &next_facts[function.id.0 as usize];
            collect_call_arguments(module, function, local, &mut proposed_parameters);
            collect_field_writes(
                module,
                function,
                local,
                &unsafe_fields,
                &mut proposed_fields,
            );
        }

        let mut changed = false;
        for (current, proposed) in parameter_ranges.iter_mut().zip(proposed_parameters) {
            for (current, proposed) in current.iter_mut().zip(proposed) {
                changed |= widen_summary(current, proposed);
            }
        }
        for function in &module.functions {
            if function.return_type != Type::Int {
                continue;
            }
            changed |= widen_summary(
                &mut return_ranges[function.id.0 as usize],
                next_facts[function.id.0 as usize].return_range,
            );
        }
        changed |= widen_field_summaries(&mut field_ranges, proposed_fields);
        if !changed {
            break;
        }
    }

    let facts = module
        .functions
        .iter()
        .map(|function| {
            analyze_function(
                function,
                &parameter_ranges[function.id.0 as usize],
                &return_ranges,
                &field_ranges,
                module,
            )
        })
        .collect();

    IntegerValueAnalysis {
        functions: facts,
        field_ranges,
    }
}

pub fn analyze_finite_values(module: &ControlFlowModule<'_>) -> FiniteValueAnalysis {
    let exported = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Function(function) => Some(function),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let indirect = indirectly_called_functions(module);
    let unsafe_fields = aggregate_owners_exposed_to_untyped_code(module);
    let boundary_parameter = |function: &ControlFlowFunction<'_>| {
        function.kind == FunctionKind::Extern
            || exported.contains(&function.id)
            || indirect.contains(&function.id)
    };
    let mut parameter_values = module
        .functions
        .iter()
        .map(|function| {
            function
                .params
                .iter()
                .map(|parameter| {
                    if !finite_type(&parameter.ty) || boundary_parameter(function) {
                        FiniteSummary::Unknown
                    } else {
                        FiniteSummary::Bottom
                    }
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let mut return_values = module
        .functions
        .iter()
        .map(|function| {
            if function.kind == FunctionKind::Extern && finite_type(&function.return_type) {
                FiniteSummary::Unknown
            } else {
                FiniteSummary::Bottom
            }
        })
        .collect::<Vec<_>>();
    let mut field_values = default_class_field_values(module, &unsafe_fields);

    loop {
        let next_facts = module
            .functions
            .iter()
            .map(|function| {
                analyze_finite_function(
                    function,
                    &parameter_values[function.id.0 as usize],
                    &return_values,
                    &field_values,
                )
            })
            .collect::<Vec<_>>();
        let mut proposed_parameters = module
            .functions
            .iter()
            .map(|function| {
                function
                    .params
                    .iter()
                    .map(|parameter| {
                        if !finite_type(&parameter.ty) || boundary_parameter(function) {
                            FiniteSummary::Unknown
                        } else {
                            FiniteSummary::Bottom
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let mut proposed_fields = default_class_field_values(module, &unsafe_fields);

        for function in &module.functions {
            let local = &next_facts[function.id.0 as usize];
            collect_finite_call_arguments(module, function, local, &mut proposed_parameters);
            collect_finite_field_writes(
                module,
                function,
                local,
                &unsafe_fields,
                &mut proposed_fields,
            );
        }

        let mut changed = false;
        for (current, proposed) in parameter_values.iter_mut().zip(proposed_parameters) {
            for (current, proposed) in current.iter_mut().zip(proposed) {
                changed |= join_finite_summary(current, &proposed);
            }
        }
        for function in &module.functions {
            if finite_type(&function.return_type) {
                changed |= join_finite_summary(
                    &mut return_values[function.id.0 as usize],
                    &next_facts[function.id.0 as usize].return_values,
                );
            }
        }
        changed |= join_finite_field_summaries(&mut field_values, proposed_fields);
        if !changed {
            break;
        }
    }

    let functions = module
        .functions
        .iter()
        .map(|function| {
            let facts = analyze_finite_function(
                function,
                &parameter_values[function.id.0 as usize],
                &return_values,
                &field_values,
            );
            FunctionFiniteFacts {
                values: facts
                    .values
                    .into_iter()
                    .filter_map(|(value, summary)| match summary {
                        FiniteSummary::Values(values) => Some((value, values)),
                        FiniteSummary::Bottom | FiniteSummary::Unknown => None,
                    })
                    .collect(),
                return_values: facts.return_values.values().cloned(),
            }
        })
        .collect();
    let field_values = field_values
        .into_iter()
        .filter_map(|(owner, fields)| {
            let fields = fields
                .into_iter()
                .filter_map(|(index, summary)| match summary {
                    FiniteSummary::Values(values) => Some((index, values)),
                    FiniteSummary::Bottom | FiniteSummary::Unknown => None,
                })
                .collect::<AHashMap<_, _>>();
            (!fields.is_empty()).then_some((owner, fields))
        })
        .collect();

    FiniteValueAnalysis {
        functions,
        field_values,
    }
}

#[derive(Debug, Clone, Default)]
struct LocalFiniteFacts {
    values: AHashMap<ValueId, FiniteSummary>,
    return_values: FiniteSummary,
}

fn analyze_finite_function(
    function: &ControlFlowFunction<'_>,
    parameter_values: &[FiniteSummary],
    return_values: &[FiniteSummary],
    field_values: &AHashMap<String, AHashMap<usize, FiniteSummary>>,
) -> LocalFiniteFacts {
    let mut values = AHashMap::new();
    for (parameter, summary) in function.params.iter().zip(parameter_values) {
        if !matches!(summary, FiniteSummary::Bottom) {
            values.insert(parameter.value, summary.clone());
        }
    }

    loop {
        let mut changed = false;
        for phi in function.blocks.iter().flat_map(|block| &block.phis) {
            if !finite_type(&phi.ty) {
                continue;
            }
            let candidate = join_complete_finite_values(
                phi.incoming
                    .iter()
                    .map(|(_, value)| finite_value_summary(&values, *value)),
            );
            changed |= update_finite_value(&mut values, phi.out, candidate);
        }
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            let (Some(out), Some(ty)) = (instruction.out, instruction.ty.as_ref()) else {
                continue;
            };
            if !finite_type(ty) {
                continue;
            }
            let candidate =
                evaluate_finite_instruction(&instruction.op, &values, return_values, field_values);
            changed |= update_finite_value(&mut values, out, candidate);
        }
        if !changed {
            break;
        }
    }

    let return_values = if finite_type(&function.return_type) {
        function
            .blocks
            .iter()
            .filter_map(|block| match block.terminator {
                Some(Terminator::Return(value)) => Some(value),
                _ => None,
            })
            .fold(FiniteSummary::Bottom, |summary, value| {
                summary.join(&value.map_or(FiniteSummary::Unknown, |value| {
                    observed_finite_summary(&values, value)
                }))
            })
    } else {
        FiniteSummary::Unknown
    };
    LocalFiniteFacts {
        values,
        return_values,
    }
}

fn evaluate_finite_instruction(
    op: &ControlFlowOp<'_>,
    values: &AHashMap<ValueId, FiniteSummary>,
    return_values: &[FiniteSummary],
    field_values: &AHashMap<String, AHashMap<usize, FiniteSummary>>,
) -> FiniteSummary {
    match op {
        ControlFlowOp::Const(value) => FiniteSummary::from_constant(value.clone()),
        ControlFlowOp::Unary {
            op: IrUnaryOp::Not,
            value,
        } => map_finite_values(finite_value_summary(values, *value), |value| match value {
            ConstValue::Bool(value) => Some(ConstValue::Bool(!value)),
            _ => None,
        }),
        ControlFlowOp::Binary { op, lhs, rhs } => combine_finite_values(
            finite_value_summary(values, *lhs),
            finite_value_summary(values, *rhs),
            |lhs, rhs| fold_finite_binary(*op, lhs, rhs),
        ),
        ControlFlowOp::FieldGet { owner, index, .. } => field_values
            .get(*owner)
            .and_then(|fields| fields.get(index))
            .cloned()
            .unwrap_or(FiniteSummary::Bottom),
        ControlFlowOp::CallDirect { function, .. } | ControlFlowOp::CallMethod { function, .. } => {
            return_values[function.0 as usize].clone()
        }
        _ => FiniteSummary::Unknown,
    }
}

fn finite_value_summary(
    values: &AHashMap<ValueId, FiniteSummary>,
    value: ValueId,
) -> FiniteSummary {
    values.get(&value).cloned().unwrap_or(FiniteSummary::Bottom)
}

fn observed_finite_summary(
    values: &AHashMap<ValueId, FiniteSummary>,
    value: ValueId,
) -> FiniteSummary {
    match finite_value_summary(values, value) {
        FiniteSummary::Bottom => FiniteSummary::Unknown,
        summary => summary,
    }
}

fn update_finite_value(
    values: &mut AHashMap<ValueId, FiniteSummary>,
    value: ValueId,
    candidate: FiniteSummary,
) -> bool {
    let current = values.get(&value).cloned().unwrap_or(FiniteSummary::Bottom);
    let next = current.join(&candidate);
    if current == next {
        false
    } else {
        values.insert(value, next);
        true
    }
}

fn join_complete_finite_values(values: impl IntoIterator<Item = FiniteSummary>) -> FiniteSummary {
    let mut result = FiniteSummary::Bottom;
    for value in values {
        if matches!(value, FiniteSummary::Bottom) {
            return FiniteSummary::Bottom;
        }
        result = result.join(&value);
    }
    result
}

fn map_finite_values(
    summary: FiniteSummary,
    map: impl Fn(&ConstValue) -> Option<ConstValue>,
) -> FiniteSummary {
    let FiniteSummary::Values(values) = summary else {
        return summary;
    };
    let mut result: Option<FiniteValueSet> = None;
    for value in &values.values {
        let Some(mapped) = map(value).and_then(FiniteValueSet::singleton) else {
            return FiniteSummary::Unknown;
        };
        result = match result {
            Some(current) => current.union(&mapped),
            None => Some(mapped),
        };
        if result.is_none() {
            return FiniteSummary::Unknown;
        }
    }
    result.map_or(FiniteSummary::Bottom, FiniteSummary::Values)
}

fn combine_finite_values(
    lhs: FiniteSummary,
    rhs: FiniteSummary,
    combine: impl Fn(&ConstValue, &ConstValue) -> Option<ConstValue>,
) -> FiniteSummary {
    match (lhs, rhs) {
        (FiniteSummary::Unknown, _) | (_, FiniteSummary::Unknown) => FiniteSummary::Unknown,
        (FiniteSummary::Bottom, _) | (_, FiniteSummary::Bottom) => FiniteSummary::Bottom,
        (FiniteSummary::Values(lhs), FiniteSummary::Values(rhs)) => {
            let mut result: Option<FiniteValueSet> = None;
            for lhs in &lhs.values {
                for rhs in &rhs.values {
                    let Some(value) = combine(lhs, rhs).and_then(FiniteValueSet::singleton) else {
                        return FiniteSummary::Unknown;
                    };
                    result = match result {
                        Some(current) => current.union(&value),
                        None => Some(value),
                    };
                    if result.is_none() {
                        return FiniteSummary::Unknown;
                    }
                }
            }
            result.map_or(FiniteSummary::Bottom, FiniteSummary::Values)
        }
    }
}

fn fold_finite_binary(op: IrBinaryOp, lhs: &ConstValue, rhs: &ConstValue) -> Option<ConstValue> {
    match (op, lhs, rhs) {
        (IrBinaryOp::Add, ConstValue::String(lhs), ConstValue::String(rhs)) => {
            Some(ConstValue::String(format!("{lhs}{rhs}")))
        }
        (IrBinaryOp::Eq, lhs, rhs) => Some(ConstValue::Bool(lhs == rhs)),
        (IrBinaryOp::NotEq, lhs, rhs) => Some(ConstValue::Bool(lhs != rhs)),
        (IrBinaryOp::And, ConstValue::Bool(lhs), ConstValue::Bool(rhs)) => {
            Some(ConstValue::Bool(*lhs && *rhs))
        }
        (IrBinaryOp::Or, ConstValue::Bool(lhs), ConstValue::Bool(rhs)) => {
            Some(ConstValue::Bool(*lhs || *rhs))
        }
        _ => None,
    }
}

fn collect_finite_call_arguments(
    module: &ControlFlowModule<'_>,
    caller: &ControlFlowFunction<'_>,
    facts: &LocalFiniteFacts,
    proposed: &mut [Vec<FiniteSummary>],
) {
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
            if !finite_type(&parameter.ty) {
                continue;
            }
            let argument = observed_finite_summary(&facts.values, *argument);
            let slot = &mut proposed[callee.0 as usize][index];
            *slot = slot.join(&argument);
        }
    }
}

fn collect_finite_field_writes(
    module: &ControlFlowModule<'_>,
    function: &ControlFlowFunction<'_>,
    facts: &LocalFiniteFacts,
    unsafe_fields: &AHashSet<String>,
    proposed: &mut AHashMap<String, AHashMap<usize, FiniteSummary>>,
) {
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        match &instruction.op {
            ControlFlowOp::Struct { name, fields } if !unsafe_fields.contains(*name) => {
                let Some(layout) = module.structs.iter().find(|layout| layout.name == *name) else {
                    continue;
                };
                for (field, value) in layout.fields.iter().zip(fields) {
                    if finite_type(&field.ty) {
                        join_finite_field(
                            proposed,
                            name,
                            field.index,
                            observed_finite_summary(&facts.values, *value),
                        );
                    }
                }
            }
            ControlFlowOp::FieldSet {
                owner,
                index,
                value,
                ..
            } if !unsafe_fields.contains(*owner)
                && aggregate_field_has_finite_type(module, owner, *index) =>
            {
                join_finite_field(
                    proposed,
                    owner,
                    *index,
                    observed_finite_summary(&facts.values, *value),
                );
            }
            _ => {}
        }
    }
}

fn default_class_field_values(
    module: &ControlFlowModule<'_>,
    unsafe_fields: &AHashSet<String>,
) -> AHashMap<String, AHashMap<usize, FiniteSummary>> {
    let instantiated = module
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::NewClass { class, .. } => Some(class),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let mut fields = AHashMap::new();
    for layout in module.structs.iter().chain(&module.classes) {
        for field in &layout.fields {
            if finite_type(&field.ty) && unsafe_fields.contains(layout.name) {
                join_finite_field(
                    &mut fields,
                    layout.name,
                    field.index,
                    FiniteSummary::Unknown,
                );
            }
        }
    }
    for layout in &module.classes {
        if !instantiated.contains(layout.name) || unsafe_fields.contains(layout.name) {
            continue;
        }
        for field in &layout.fields {
            let value = match &field.ty {
                Type::Bool => Some(ConstValue::Bool(false)),
                Type::String => Some(ConstValue::String(String::new())),
                Type::Null | Type::Nullable(_) => Some(ConstValue::Null),
                _ => None,
            };
            if let Some(value) = value {
                join_finite_field(
                    &mut fields,
                    layout.name,
                    field.index,
                    FiniteSummary::from_constant(value),
                );
            }
        }
    }
    fields
}

fn join_finite_summary(current: &mut FiniteSummary, proposed: &FiniteSummary) -> bool {
    let next = current.join(proposed);
    if *current == next {
        false
    } else {
        *current = next;
        true
    }
}

fn join_finite_field(
    fields: &mut AHashMap<String, AHashMap<usize, FiniteSummary>>,
    owner: &str,
    index: usize,
    summary: FiniteSummary,
) {
    let slot = fields
        .entry(owner.to_string())
        .or_default()
        .entry(index)
        .or_default();
    *slot = slot.join(&summary);
}

fn join_finite_field_summaries(
    current: &mut AHashMap<String, AHashMap<usize, FiniteSummary>>,
    proposed: AHashMap<String, AHashMap<usize, FiniteSummary>>,
) -> bool {
    let mut changed = false;
    for (owner, fields) in proposed {
        for (index, summary) in fields {
            let slot = current
                .entry(owner.clone())
                .or_default()
                .entry(index)
                .or_default();
            changed |= join_finite_summary(slot, &summary);
        }
    }
    changed
}

fn finite_type(ty: &Type<'_>) -> bool {
    matches!(
        ty,
        Type::Bool | Type::String | Type::Null | Type::Nullable(_)
    )
}

fn is_finite_constant(value: &ConstValue) -> bool {
    matches!(
        value,
        ConstValue::Bool(_) | ConstValue::String(_) | ConstValue::Null
    )
}

fn analyze_function(
    function: &ControlFlowFunction<'_>,
    parameter_ranges: &[Option<I32Range>],
    return_ranges: &[Option<I32Range>],
    field_ranges: &AHashMap<String, AHashMap<usize, I32Range>>,
    module: &ControlFlowModule<'_>,
) -> FunctionIntegerFacts {
    let mut ranges = AHashMap::new();
    for (parameter, range) in function.params.iter().zip(parameter_ranges) {
        if let Some(range) = range {
            ranges.insert(parameter.value, *range);
        }
    }
    seed_induction_ranges(function, &mut ranges);
    let mut elidable_coercions = AHashSet::new();

    loop {
        let mut changed = false;
        for phi in function.blocks.iter().flat_map(|block| &block.phis) {
            if phi.ty != Type::Int {
                continue;
            }
            let Some(candidate) = join_known(
                phi.incoming
                    .iter()
                    .map(|(_, value)| ranges.get(value).copied()),
            ) else {
                continue;
            };
            changed |= update_local_range(&mut ranges, phi.out, candidate);
        }
        for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
            if instruction.ty.as_ref() != Some(&Type::Int) {
                continue;
            }
            let Some(out) = instruction.out else {
                continue;
            };
            let (candidate, coercion_is_elidable) = evaluate_integer_instruction(
                &instruction.op,
                &ranges,
                return_ranges,
                field_ranges,
                module,
            );
            let Some(candidate) = candidate else {
                continue;
            };
            changed |= update_local_range(&mut ranges, out, candidate);
            if coercion_is_elidable {
                elidable_coercions.insert(out);
            }
        }
        if !changed {
            break;
        }
    }

    let return_range = if function.return_type == Type::Int {
        join_known(
            function
                .blocks
                .iter()
                .filter_map(|block| match block.terminator {
                    Some(Terminator::Return(Some(value))) => Some(ranges.get(&value).copied()),
                    _ => None,
                }),
        )
    } else {
        None
    };
    FunctionIntegerFacts {
        ranges,
        elidable_coercions,
        return_range,
    }
}

fn evaluate_integer_instruction(
    op: &ControlFlowOp<'_>,
    ranges: &AHashMap<ValueId, I32Range>,
    return_ranges: &[Option<I32Range>],
    field_ranges: &AHashMap<String, AHashMap<usize, I32Range>>,
    module: &ControlFlowModule<'_>,
) -> (Option<I32Range>, bool) {
    let range = |value: &ValueId| ranges.get(value).copied();
    match op {
        ControlFlowOp::Const(ConstValue::Int(value)) => (Some(I32Range::exact(*value)), false),
        ControlFlowOp::Unary {
            op: IrUnaryOp::Neg,
            value,
        } => range(value)
            .map(I32Range::neg)
            .map_or((None, false), |(range, safe)| (Some(range), safe)),
        ControlFlowOp::Binary { op, lhs, rhs } => {
            let Some((lhs, rhs)) = range(lhs).zip(range(rhs)) else {
                return (None, false);
            };
            let (range, safe) = match op {
                IrBinaryOp::Add => lhs.add(rhs),
                IrBinaryOp::Sub => lhs.sub(rhs),
                IrBinaryOp::Mul => lhs.mul(rhs),
                IrBinaryOp::Mod => lhs.modulo(rhs),
                IrBinaryOp::BitAnd if rhs.min >= 0 => (
                    I32Range {
                        min: 0,
                        max: rhs.max,
                    },
                    true,
                ),
                IrBinaryOp::BitAnd
                | IrBinaryOp::BitOr
                | IrBinaryOp::Xor
                | IrBinaryOp::ShiftLeft
                | IrBinaryOp::ShiftRight
                | IrBinaryOp::UnsignedShiftRight
                | IrBinaryOp::Div => (I32Range::FULL, false),
                _ => return (None, false),
            };
            (Some(range), safe)
        }
        ControlFlowOp::FieldGet { owner, index, .. } => (
            field_ranges
                .get(*owner)
                .and_then(|fields| fields.get(index))
                .copied(),
            false,
        ),
        ControlFlowOp::CallDirect { function, .. } | ControlFlowOp::CallMethod { function, .. } => {
            let callee = &module.functions[function.0 as usize];
            if callee.kind == FunctionKind::Extern {
                (Some(I32Range::FULL), false)
            } else {
                (return_ranges[function.0 as usize], false)
            }
        }
        ControlFlowOp::Intrinsic { intrinsic, .. } => match intrinsic {
            Intrinsic::ArrayLength
            | Intrinsic::MapSize
            | Intrinsic::SetSize
            | Intrinsic::BufferByteLength
            | Intrinsic::StringLength => (
                Some(I32Range {
                    min: 0,
                    max: i64::from(i32::MAX),
                }),
                false,
            ),
            other
                if matches!(
                    crate::typed_array::classify_typed_array_intrinsic(*other),
                    Some((
                        _,
                        crate::typed_array::TypedArrayIntrinsic::Length
                            | crate::typed_array::TypedArrayIntrinsic::ByteLength
                            | crate::typed_array::TypedArrayIntrinsic::ByteOffset
                    ))
                ) =>
            {
                (
                    Some(I32Range {
                        min: 0,
                        max: i64::from(i32::MAX),
                    }),
                    false,
                )
            }
            Intrinsic::StringCharCodeAt => (
                Some(I32Range {
                    min: 0,
                    max: 65_535,
                }),
                false,
            ),
            Intrinsic::IntImul => (Some(I32Range::FULL), false),
            _ => (Some(I32Range::FULL), false),
        },
        _ => (Some(I32Range::FULL), false),
    }
}

fn collect_call_arguments(
    module: &ControlFlowModule<'_>,
    caller: &ControlFlowFunction<'_>,
    facts: &FunctionIntegerFacts,
    proposed: &mut [Vec<Option<I32Range>>],
) {
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
            if parameter.ty != Type::Int {
                continue;
            }
            if let Some(range) = facts.range(*argument) {
                join_summary(&mut proposed[callee.0 as usize][index], range);
            }
        }
    }
}

fn collect_field_writes(
    module: &ControlFlowModule<'_>,
    function: &ControlFlowFunction<'_>,
    facts: &FunctionIntegerFacts,
    unsafe_fields: &AHashSet<String>,
    proposed: &mut AHashMap<String, AHashMap<usize, I32Range>>,
) {
    for instruction in function.blocks.iter().flat_map(|block| &block.instructions) {
        match &instruction.op {
            ControlFlowOp::Struct { name, fields } if !unsafe_fields.contains(*name) => {
                let Some(layout) = module.structs.iter().find(|layout| layout.name == *name) else {
                    continue;
                };
                for (field, value) in layout.fields.iter().zip(fields) {
                    if field.ty == Type::Int {
                        if let Some(range) = facts.range(*value) {
                            join_field(proposed, name, field.index, range);
                        }
                    }
                }
            }
            ControlFlowOp::FieldSet {
                owner,
                index,
                value,
                ..
            } if !unsafe_fields.contains(*owner)
                && aggregate_field_is_int(module, owner, *index) =>
            {
                if let Some(range) = facts.range(*value) {
                    join_field(proposed, owner, *index, range);
                }
            }
            _ => {}
        }
    }
}

fn default_class_field_ranges(
    module: &ControlFlowModule<'_>,
    unsafe_fields: &AHashSet<String>,
) -> AHashMap<String, AHashMap<usize, I32Range>> {
    let instantiated = module
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::NewClass { class, .. } => Some(class),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let mut fields = AHashMap::new();
    for layout in &module.classes {
        if !instantiated.contains(layout.name) || unsafe_fields.contains(layout.name) {
            continue;
        }
        for field in &layout.fields {
            if field.ty == Type::Int {
                join_field(&mut fields, layout.name, field.index, I32Range::exact(0));
            }
        }
    }
    fields
}

fn aggregate_owners_exposed_to_untyped_code(module: &ControlFlowModule<'_>) -> AHashSet<String> {
    let exported_globals = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Global(symbol) => Some(symbol),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let exported_functions = module
        .exports
        .iter()
        .filter_map(|export| match export.binding {
            ExportBinding::Function(function) => Some(function),
            _ => None,
        })
        .collect::<AHashSet<_>>();
    let mut unsafe_owners = AHashSet::new();
    for global in module
        .globals
        .iter()
        .filter(|global| global.external || exported_globals.contains(&global.symbol))
    {
        collect_aggregate_owners(&global.ty, &mut unsafe_owners);
    }
    for function in &module.functions {
        if function.kind == FunctionKind::Extern || exported_functions.contains(&function.id) {
            for parameter in &function.params {
                collect_aggregate_owners(&parameter.ty, &mut unsafe_owners);
            }
            collect_aggregate_owners(&function.return_type, &mut unsafe_owners);
        }
        let types = value_types(function);
        for (index, escape) in function.value_escapes.iter().enumerate() {
            if *escape != EscapeState::EscapesToUntypedBoundary {
                continue;
            }
            if let Some(ty) = types.get(&ValueId(index as u32)) {
                collect_aggregate_owners(ty, &mut unsafe_owners);
            }
        }
    }
    unsafe_owners
}

fn collect_aggregate_owners(ty: &Type<'_>, owners: &mut AHashSet<String>) {
    match ty {
        Type::Struct(name) | Type::Class(name) => {
            owners.insert((*name).to_string());
        }
        Type::StructInstance { name, args } | Type::ClassInstance { name, args } => {
            owners.insert((*name).to_string());
            for argument in args {
                collect_aggregate_owners(argument, owners);
            }
        }
        Type::Array(element)
        | Type::Set(element)
        | Type::Nullable(element)
        | Type::Task(element) => {
            collect_aggregate_owners(element, owners);
        }
        Type::Map(key, value) => {
            collect_aggregate_owners(key, owners);
            collect_aggregate_owners(value, owners);
        }
        Type::Union(members) => {
            for member in members {
                collect_aggregate_owners(member, owners);
            }
        }
        Type::Function(signature) => {
            for parameter in &signature.params {
                collect_aggregate_owners(parameter, owners);
            }
            collect_aggregate_owners(&signature.return_type, owners);
        }
        Type::GenericFunction(function) => {
            for parameter in &function.signature.params {
                collect_aggregate_owners(parameter, owners);
            }
            collect_aggregate_owners(&function.signature.return_type, owners);
        }
        Type::Int
        | Type::Float
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::ArrayBuffer
        | Type::SharedArrayBuffer
        | Type::Uint8Array
        | Type::Int8Array
        | Type::Uint8ClampedArray
        | Type::Int16Array
        | Type::Uint16Array
        | Type::Int32Array
        | Type::Uint32Array
        | Type::Float32Array
        | Type::Float64Array
        | Type::Symbol
        | Type::ModuleNamespace(_)
        | Type::ModuleLoadError
        | Type::TypeParameter(_) => {}
    }
}

fn aggregate_field_is_int(module: &ControlFlowModule<'_>, owner: &str, index: usize) -> bool {
    module
        .structs
        .iter()
        .chain(&module.classes)
        .find(|layout| layout.name == owner)
        .and_then(|layout| layout.fields.get(index))
        .is_some_and(|field| field.ty == Type::Int)
}

fn aggregate_field_has_finite_type(
    module: &ControlFlowModule<'_>,
    owner: &str,
    index: usize,
) -> bool {
    module
        .structs
        .iter()
        .chain(&module.classes)
        .find(|layout| layout.name == owner)
        .and_then(|layout| layout.fields.get(index))
        .is_some_and(|field| finite_type(&field.ty))
}

fn value_types<'src>(function: &ControlFlowFunction<'src>) -> AHashMap<ValueId, Type<'src>> {
    let mut types = AHashMap::new();
    for parameter in &function.params {
        types.insert(parameter.value, parameter.ty.clone());
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

fn indirectly_called_functions(module: &ControlFlowModule<'_>) -> AHashSet<FunctionId> {
    module
        .functions
        .iter()
        .flat_map(|function| function.blocks.iter())
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| match instruction.op {
            ControlFlowOp::Closure { function, .. } => Some(function),
            _ => None,
        })
        .collect()
}

fn seed_induction_ranges(
    function: &ControlFlowFunction<'_>,
    ranges: &mut AHashMap<ValueId, I32Range>,
) {
    let definitions = function
        .blocks
        .iter()
        .flat_map(|block| &block.instructions)
        .filter_map(|instruction| instruction.out.map(|out| (out, &instruction.op)))
        .collect::<AHashMap<_, _>>();
    let constant = |value: ValueId| match definitions.get(&value) {
        Some(ControlFlowOp::Const(ConstValue::Int(value))) => Some(*value),
        _ => None,
    };
    for shape in &function.shapes {
        let ControlShape::Loop { header, .. } = shape else {
            continue;
        };
        let block = &function.blocks[header.0 as usize];
        let Some(Terminator::Branch { condition, .. }) = block.terminator else {
            continue;
        };
        let Some(ControlFlowOp::Binary { op, lhs, rhs }) = definitions.get(&condition) else {
            continue;
        };
        let (phi_value, bound, ascending, inclusive) =
            if block.phis.iter().any(|phi| phi.out == *lhs) {
                let Some(bound) = constant(*rhs) else {
                    continue;
                };
                match op {
                    IrBinaryOp::Less => (*lhs, bound, true, false),
                    IrBinaryOp::LessEq => (*lhs, bound, true, true),
                    IrBinaryOp::Greater => (*lhs, bound, false, false),
                    IrBinaryOp::GreaterEq => (*lhs, bound, false, true),
                    _ => continue,
                }
            } else if block.phis.iter().any(|phi| phi.out == *rhs) {
                let Some(bound) = constant(*lhs) else {
                    continue;
                };
                match op {
                    IrBinaryOp::Greater => (*rhs, bound, true, false),
                    IrBinaryOp::GreaterEq => (*rhs, bound, true, true),
                    IrBinaryOp::Less => (*rhs, bound, false, false),
                    IrBinaryOp::LessEq => (*rhs, bound, false, true),
                    _ => continue,
                }
            } else {
                continue;
            };
        let Some(phi) = block.phis.iter().find(|phi| phi.out == phi_value) else {
            continue;
        };
        let initial = phi
            .incoming
            .iter()
            .find_map(|(_, value)| constant(*value))
            .or_else(|| {
                (!ascending && bound >= i64::from(i32::MIN) && bound <= i64::from(i32::MAX))
                    .then_some(i64::from(i32::MAX))
            });
        let Some(initial) = initial else {
            continue;
        };
        let candidate = if ascending && initial <= bound {
            I32Range::checked(initial, bound - i64::from(!inclusive))
        } else if !ascending && initial >= bound {
            I32Range::checked(bound + i64::from(!inclusive), initial)
        } else {
            None
        };
        if let Some(candidate) = candidate {
            ranges.insert(phi_value, candidate);
        }
    }
}

fn join_known(values: impl IntoIterator<Item = Option<I32Range>>) -> Option<I32Range> {
    let mut values = values.into_iter();
    let first = values.next()??;
    values.try_fold(first, |range, value| Some(range.join(value?)))
}

fn update_local_range(
    ranges: &mut AHashMap<ValueId, I32Range>,
    value: ValueId,
    candidate: I32Range,
) -> bool {
    match ranges.get(&value).copied() {
        None => {
            ranges.insert(value, candidate);
            true
        }
        Some(current) => {
            let next = current.widen(current.join(candidate));
            if next == current {
                false
            } else {
                ranges.insert(value, next);
                true
            }
        }
    }
}

fn join_summary(slot: &mut Option<I32Range>, range: I32Range) {
    *slot = Some(slot.map_or(range, |current| current.join(range)));
}

fn widen_summary(slot: &mut Option<I32Range>, proposed: Option<I32Range>) -> bool {
    let Some(proposed) = proposed else {
        return false;
    };
    let next = slot.map_or(proposed, |current| current.widen(current.join(proposed)));
    if *slot == Some(next) {
        false
    } else {
        *slot = Some(next);
        true
    }
}

fn join_field(
    fields: &mut AHashMap<String, AHashMap<usize, I32Range>>,
    owner: &str,
    index: usize,
    range: I32Range,
) {
    let slot = fields
        .entry(owner.to_string())
        .or_default()
        .entry(index)
        .or_insert(range);
    *slot = slot.join(range);
}

fn widen_field_summaries(
    current: &mut AHashMap<String, AHashMap<usize, I32Range>>,
    proposed: AHashMap<String, AHashMap<usize, I32Range>>,
) -> bool {
    let mut changed = false;
    for (owner, fields) in proposed {
        for (index, range) in fields {
            let owner_fields = current.entry(owner.clone()).or_default();
            if let Some(slot) = owner_fields.get_mut(&index) {
                let next = slot.widen(slot.join(range));
                if next != *slot {
                    *slot = next;
                    changed = true;
                }
            } else {
                owner_fields.insert(index, range);
                changed = true;
            }
        }
    }
    changed
}

#[cfg(test)]
mod tests {
    use bumpalo::Bump;

    use super::*;
    use crate::{
        analyze, lower_to_control_flow,
        optimizer::{optimize_control_flow_with_options, OptimizationOptions},
        parse_source,
    };

    #[test]
    fn propagates_direct_call_arguments_and_returns() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "extern int read();int digit(int value){return value%10;}int offset(int value){return value+5;}print(offset(digit(read())));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut ir, &options, false).unwrap();
        let analysis = analyze_integer_values(&ir);
        let digit = ir
            .functions
            .iter()
            .find(|function| function.name == Some("digit"))
            .unwrap();
        let offset = ir
            .functions
            .iter()
            .find(|function| function.name == Some("offset"))
            .unwrap();

        assert_eq!(
            analysis.function(digit.id).return_range(),
            Some(I32Range { min: -9, max: 9 })
        );
        assert_eq!(
            analysis.function(offset.id).range(offset.params[0].value),
            Some(I32Range { min: -9, max: 9 })
        );
        assert_eq!(
            analysis.function(offset.id).return_range(),
            Some(I32Range { min: -4, max: 14 })
        );
    }

    #[test]
    fn invalidates_fields_that_cross_an_untyped_boundary() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Box{int value;}extern void mutate(Box box);Box box=Box{7};mutate(box);print(box.value+1);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            scalar_replacement: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut ir, &options, false).unwrap();
        let analysis = analyze_integer_values(&ir);

        assert_eq!(analysis.field_range("Box", 0), None);
    }

    #[test]
    fn invalidates_fields_in_exported_function_type_graphs() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "export struct Box{int value;}export int first(Box[] boxes){return boxes[0].value+1;}",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            scalar_replacement: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut ir, &options, true).unwrap();
        let analysis = analyze_integer_values(&ir);

        assert_eq!(analysis.field_range("Box", 0), None);
    }

    #[test]
    fn widens_recursive_argument_growth_instead_of_iterating_by_value() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "int climb(int value){if(value>=10){return value;}return climb(value+1);}print(climb(0));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        let options = OptimizationOptions {
            inlining: false,
            ..OptimizationOptions::default()
        };
        optimize_control_flow_with_options(&mut ir, &options, false).unwrap();
        let analysis = analyze_integer_values(&ir);
        let climb = ir
            .functions
            .iter()
            .find(|function| function.name == Some("climb"))
            .unwrap();

        assert_eq!(
            analysis.function(climb.id).range(climb.params[0].value),
            Some(I32Range::FULL)
        );
    }

    #[test]
    fn propagates_finite_arguments_and_returns_across_direct_calls() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string label(bool enabled){if(enabled){return \"on\";}return \"off\";}print(label(true));print(label(false));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let analysis = analyze_finite_values(&ir);
        let label = ir
            .functions
            .iter()
            .find(|function| function.name == Some("label"))
            .unwrap();
        let parameter_values = analysis
            .function(label.id)
            .values(label.params[0].value)
            .unwrap()
            .values();
        let return_values = analysis
            .function(label.id)
            .return_values()
            .unwrap()
            .values();

        assert_eq!(parameter_values.len(), 2);
        assert!(parameter_values.contains(&ConstValue::Bool(true)));
        assert!(parameter_values.contains(&ConstValue::Bool(false)));
        assert_eq!(return_values.len(), 2);
        assert!(return_values.contains(&ConstValue::String("on".to_string())));
        assert!(return_values.contains(&ConstValue::String("off".to_string())));
    }

    #[test]
    fn summarizes_exact_and_finite_nominal_fields() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Badge{string label;bool active;}string text(Badge badge){return badge.label;}Badge first=Badge{\"new\",true};Badge second=Badge{\"new\",false};print(text(first));print(text(second));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let analysis = analyze_finite_values(&ir);

        assert_eq!(
            analysis.field_constant("Badge", 0),
            Some(&ConstValue::String("new".to_string()))
        );
        let active = analysis.field_values("Badge", 1).unwrap().values();
        assert_eq!(active.len(), 2);
        assert!(active.contains(&ConstValue::Bool(true)));
        assert!(active.contains(&ConstValue::Bool(false)));
    }

    #[test]
    fn invalidates_finite_fields_at_untyped_boundaries() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "struct Badge{string label;}extern void mutate(Badge badge);Badge badge=Badge{\"new\"};mutate(badge);print(badge.label);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let analysis = analyze_finite_values(&ir);

        assert_eq!(analysis.field_values("Badge", 0), None);
    }

    #[test]
    fn does_not_treat_unmodeled_generic_values_as_absent_nullable_values() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "T? maybe<T>(bool present,T value){if(present){return value;}return null;}print(maybe(true,7)!=null);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let analysis = analyze_finite_values(&ir);
        let maybe = ir
            .functions
            .iter()
            .find(|function| function.name == Some("maybe"))
            .unwrap();

        assert_eq!(analysis.function(maybe.id).return_values(), None);
    }

    #[test]
    fn does_not_treat_class_values_as_absent_nullable_returns() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "class Handle{int id;init(int id){this.id=id;}}Handle? maybe(bool present){if(present){return new Handle(1);}return null;}print(maybe(true)!=null);",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let analysis = analyze_finite_values(&ir);
        let maybe = ir
            .functions
            .iter()
            .find(|function| function.name == Some("maybe"))
            .unwrap();

        assert_eq!(analysis.function(maybe.id).return_values(), None);
    }

    #[test]
    fn widens_large_value_sets_to_unknown() {
        let arena = Bump::new();
        let program = parse_source(
            &arena,
            "string echo(string value){return value;}print(echo(\"a\"));print(echo(\"b\"));print(echo(\"c\"));print(echo(\"d\"));print(echo(\"e\"));",
        )
        .unwrap();
        let semantics = analyze(&program).unwrap();
        let mut ir = lower_to_control_flow(&program, &semantics).unwrap();
        crate::optimizer::promote_locals_to_ssa(&mut ir).unwrap();
        let analysis = analyze_finite_values(&ir);
        let echo = ir
            .functions
            .iter()
            .find(|function| function.name == Some("echo"))
            .unwrap();

        assert_eq!(
            analysis.function(echo.id).values(echo.params[0].value),
            None
        );
        assert_eq!(analysis.function(echo.id).return_values(), None);
    }
}
