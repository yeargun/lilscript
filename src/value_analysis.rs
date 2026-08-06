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
            | Intrinsic::Uint8ArrayLength
            | Intrinsic::Uint8ArrayByteLength
            | Intrinsic::Uint8ArrayByteOffset
            | Intrinsic::StringLength => (
                Some(I32Range {
                    min: 0,
                    max: i64::from(i32::MAX),
                }),
                false,
            ),
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
        Type::Array(element) | Type::Set(element) | Type::Nullable(element) => {
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
        let Some(initial) = phi.incoming.iter().find_map(|(_, value)| constant(*value)) else {
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
}
