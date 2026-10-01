//! Bounded execution of primitive constant calls. A successful result proves
//! the executed path terminates and has no observable effect; an unsupported
//! operation, capture or exhausted budget supplies no such evidence.
use super::super::call_graph::Callee;
use super::super::facts::{
    self, StoredExact as Exact, StoredKnowledge as Knowledge, StoredString, Work,
};
use super::storage::Map;
use super::*;
use crate::literal::StringValue;
use crate::output_budget::AllocationClass::Scratch;
use crate::primitive::Intrinsic;
use std::sync::Arc;

const MAX_DEPTH: usize = 8;
const MAX_OPERATIONS: usize = 64;
const MAX_STEPS: u32 = 1024;

/// Why bounded execution kept a call. The catalog classifies operations
/// outside primitive execution; depth/size/step/storage bounds are distinct
/// from semantic refusals in the rule receipt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Refusal {
    Host,
    Engine,
    Aggregate,
    Observable,
    Unsupported,
    Limit,
}

pub(super) struct Attempt {
    pub value: Option<Exact>,
    pub refusal: Option<Refusal>,
}

fn intrinsic_refusal(operation: ResolvedIntrinsic) -> Option<Refusal> {
    use crate::catalog::EffectClass as E;
    use Intrinsic as I;
    let (ResolvedIntrinsic::Method(method)
    | ResolvedIntrinsic::Property(method)
    | ResolvedIntrinsic::Constructor(method)) = operation;
    match method {
        I::IntImul
        | I::IntToString
        | I::IntToUnsignedString
        | I::FloatAbs
        | I::FloatFloor
        | I::FloatCeil
        | I::FloatRound
        | I::FloatMin
        | I::FloatMax
        | I::FloatToInt
        | I::StringLength
        | I::StringCharCodeAt
        | I::StringCharCodeAtNumber
        | I::StringCodeUnitAt
        | I::StringCharAt
        | I::StringIncludes
        | I::StringIndexOf
        | I::StringLastIndexOf
        | I::StringStartsWith
        | I::StringEndsWith
        | I::StringTrim
        | I::StringTrimStart
        | I::StringTrimEnd
        | I::StringSlice
        | I::StringCodePointLength
        | I::StringRepeat
        | I::JsTruthy => None,
        // ASCII casing is exact; the evaluator declines the Unicode case.
        I::StringToLowerCase | I::StringToUpperCase => None,
        I::FloatSqrt
        | I::FloatSin
        | I::FloatCos
        | I::FloatAcos
        | I::FloatExp
        | I::FloatLog
        | I::FloatTan
        | I::FloatAtan2
        | I::FloatHypot => Some(Refusal::Engine),
        _ => Some(match crate::catalog::effect_class(operation) {
            E::Write { .. } | E::Callback | E::Print => Refusal::Observable,
            E::Construct { .. } | E::Read { .. } | E::Pure { fresh: true, .. } => {
                Refusal::Aggregate
            }
            // Remaining dynamic/host operations need host or aggregate
            // semantics, beyond the primitive exact-value domain.
            E::Unknown => Refusal::Host,
            E::Pure { .. } | E::Inert { .. } => Refusal::Unsupported,
        }),
    }
}

#[cfg(test)]
#[path = "evaluate_scalar_tests.rs"]
mod scalar_tests;

fn string<'a>(program: &'a Program<'_>, value: &'a Exact) -> Option<&'a StringValue> {
    match value {
        Exact::String(StoredString::Source(id)) => program.strings.get(id.index()),
        Exact::String(StoredString::Computed(value)) => Some(value),
        _ => None,
    }
}

fn integer(value: &Exact) -> Option<i32> {
    match value {
        Exact::Integer(value) => Some(*value),
        _ => None,
    }
}

fn number(value: &Exact) -> Option<f64> {
    match value {
        Exact::Integer(value) => Some(f64::from(*value)),
        Exact::Number(bits) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

fn computed(units: Vec<u16>) -> Exact {
    Exact::String(StoredString::Computed(Arc::new(StringValue::from_utf16(
        units,
    ))))
}

/// Exact primitive methods only. Transcendentals and Unicode case conversion
/// deliberately remain unknown: host implementations need not agree bitwise.
fn intrinsic(
    program: &Program<'_>,
    operation: ResolvedIntrinsic,
    receiver: Option<&Exact>,
    args: &[Exact],
    work: &mut Work,
) -> Option<Exact> {
    let ResolvedIntrinsic::Method(method) = operation else {
        return None;
    };
    let arguments = match method {
        Intrinsic::FloatAbs
        | Intrinsic::FloatFloor
        | Intrinsic::FloatCeil
        | Intrinsic::FloatRound
        | Intrinsic::FloatToInt
        | Intrinsic::StringCodePointLength => 0..=0,
        Intrinsic::FloatMin | Intrinsic::FloatMax | Intrinsic::IntImul => 1..=1,
        Intrinsic::JsTruthy => 0..=0,
        _ => crate::catalog::intrinsic_recipe(method)?.arguments,
    };
    if !arguments.contains(&args.len()) {
        return None;
    }
    let receiver = receiver?;
    use Intrinsic as I;
    if method == I::JsTruthy {
        return Some(Exact::Boolean(facts::truthy(program, receiver)));
    }
    if method == I::IntImul {
        return Some(Exact::Integer(
            integer(receiver)?.wrapping_mul(integer(args.first()?)?),
        ));
    }
    if matches!(method, I::IntToString | I::IntToUnsignedString) {
        let value = integer(receiver)?;
        let radix = args.first().map_or(Some(10), integer)?;
        if !(2..=36).contains(&radix) || !work.reserve_evaluation(256) {
            return None;
        }
        let unsigned = method == I::IntToUnsignedString;
        let mut magnitude = if unsigned {
            u64::from(value as u32)
        } else {
            i64::from(value).unsigned_abs()
        };
        let digits = b"0123456789abcdefghijklmnopqrstuvwxyz";
        let mut units = Vec::with_capacity(33);
        loop {
            units.push(u16::from(digits[(magnitude % radix as u64) as usize]));
            magnitude /= radix as u64;
            if magnitude == 0 {
                break;
            }
        }
        if value < 0 && !unsigned {
            units.push(u16::from(b'-'));
        }
        units.reverse();
        return Some(computed(units));
    }
    if let Some(value) = number(receiver) {
        if method == I::FloatToInt {
            // Truncate before reducing modulo 2^32. A Rust float-to-int cast
            // alone saturates; the source operation wraps and maps every
            // non-finite value and either zero sign to positive zero.
            let value = if value.is_finite() {
                value.trunc().rem_euclid(4_294_967_296.0) as u32 as i32
            } else {
                0
            };
            return Some(Exact::Integer(value));
        }
        let result = match method {
            I::FloatAbs => value.abs(),
            I::FloatFloor => value.floor(),
            I::FloatCeil => value.ceil(),
            I::FloatRound => {
                // JS rounds ties toward +infinity and preserves negative zero.
                let floor = value.floor();
                let rounded = if value - floor < 0.5 {
                    floor
                } else {
                    floor + 1.0
                };
                if rounded == 0.0 {
                    0.0f64.copysign(value)
                } else {
                    rounded
                }
            }
            I::FloatMin | I::FloatMax => {
                let other = number(args.first()?)?;
                if value.is_nan() || other.is_nan() {
                    f64::NAN
                } else if value == 0.0 && other == 0.0 {
                    let negative = if method == I::FloatMin {
                        value.is_sign_negative() || other.is_sign_negative()
                    } else {
                        value.is_sign_negative() && other.is_sign_negative()
                    };
                    if negative {
                        -0.0
                    } else {
                        0.0
                    }
                } else if method == I::FloatMin {
                    value.min(other)
                } else {
                    value.max(other)
                }
            }
            _ => return None,
        };
        return Some(Exact::Number(result.to_bits()));
    }
    let value = string(program, receiver)?;
    let argument_bytes = args
        .iter()
        .filter_map(|arg| string(program, arg))
        .try_fold(0u64, |sum, text| {
            sum.checked_add(text.storage_bytes() as u64)
        })?;
    let bytes = (value.storage_bytes() as u64).checked_add(argument_bytes)?;
    if !work.reserve_evaluation(bytes.checked_mul(12)?.checked_add(256)?) {
        return None;
    }
    let units: Vec<u16> = value.code_units().collect();
    let len = units.len();
    let index = |position: i32| (position.max(0) as usize).min(len);
    let argument =
        |at: usize, default: i32| -> Option<i32> { args.get(at).map_or(Some(default), integer) };
    match method {
        I::StringCodePointLength => {
            if !work.charge(len as u64) {
                return None;
            }
            let mut count = 0usize;
            let mut at = 0;
            while at < len {
                let pair = matches!(units[at], 0xd800..=0xdbff)
                    && units
                        .get(at + 1)
                        .is_some_and(|unit| matches!(unit, 0xdc00..=0xdfff));
                at += if pair { 2 } else { 1 };
                count += 1;
            }
            Some(Exact::Integer(count as i32))
        }
        I::StringCharCodeAt | I::StringCharCodeAtNumber | I::StringCodeUnitAt | I::StringCharAt => {
            let position = integer(args.first()?)?;
            let unit = usize::try_from(position)
                .ok()
                .and_then(|at| units.get(at))
                .copied();
            Some(match method {
                I::StringCharAt => computed(unit.into_iter().collect()),
                I::StringCharCodeAtNumber => {
                    Exact::Number(unit.map_or(f64::NAN, f64::from).to_bits())
                }
                I::StringCodeUnitAt => Exact::Integer(i32::from(unit?)),
                _ => Exact::Integer(unit.map_or(0, i32::from)),
            })
        }
        I::StringIndexOf
        | I::StringLastIndexOf
        | I::StringIncludes
        | I::StringStartsWith
        | I::StringEndsWith => {
            let needle = string(program, args.first()?)?;
            let needle_units: Vec<u16> = needle.code_units().collect();
            // The search helper is bounded even for repeated prefixes.
            if !work.charge((len as u64 + 1).checked_mul(needle_units.len() as u64 + 1)?) {
                return None;
            }
            let default = if matches!(method, I::StringLastIndexOf | I::StringEndsWith) {
                i32::try_from(len).ok()?
            } else {
                0
            };
            let position = argument(1, default)?;
            Some(match method {
                I::StringIndexOf | I::StringLastIndexOf => {
                    Exact::Integer(crate::literal::index_of_units(
                        &units,
                        &needle_units,
                        position,
                        method == I::StringLastIndexOf,
                    ))
                }
                I::StringIncludes => Exact::Boolean(
                    crate::literal::index_of_units(&units, &needle_units, position, false) >= 0,
                ),
                I::StringStartsWith => {
                    Exact::Boolean(units[index(position)..].starts_with(&needle_units))
                }
                I::StringEndsWith => {
                    Exact::Boolean(units[..index(position)].ends_with(&needle_units))
                }
                _ => unreachable!(),
            })
        }
        I::StringSlice => {
            let relative = |at: i32| {
                if at < 0 {
                    (len as i64 + i64::from(at)).max(0) as usize
                } else {
                    index(at)
                }
            };
            let start = relative(integer(args.first()?)?);
            let end = relative(argument(1, i32::try_from(len).ok()?)?).max(start);
            Some(computed(units[start..end].to_vec()))
        }
        I::StringTrim | I::StringTrimStart | I::StringTrimEnd => {
            let whitespace = |unit: &u16| {
                matches!(*unit,
                0x0009..=0x000d | 0x0020 | 0x00a0 | 0x1680 | 0x2000..=0x200a
                | 0x2028 | 0x2029 | 0x202f | 0x205f | 0x3000 | 0xfeff)
            };
            let start = if method == I::StringTrimEnd {
                0
            } else {
                units.iter().take_while(|unit| whitespace(unit)).count()
            };
            let end = if method == I::StringTrimStart {
                len
            } else {
                len - units[start..]
                    .iter()
                    .rev()
                    .take_while(|unit| whitespace(unit))
                    .count()
            };
            Some(computed(units[start..end].to_vec()))
        }
        I::StringToUpperCase | I::StringToLowerCase if units.iter().all(|unit| *unit <= 0x7f) => {
            Some(computed(
                units
                    .into_iter()
                    .map(|unit| {
                        let byte = unit as u8;
                        u16::from(if method == I::StringToUpperCase {
                            byte.to_ascii_uppercase()
                        } else {
                            byte.to_ascii_lowercase()
                        })
                    })
                    .collect(),
            ))
        }
        I::StringRepeat => {
            let count = usize::try_from(integer(args.first()?)?).ok()?;
            let size = len.checked_mul(count)?;
            if !work.reserve_evaluation((size as u64).checked_mul(8)?.checked_add(64)?) {
                return None;
            }
            Some(computed(units.repeat(count)))
        }
        _ => None,
    }
}

fn known(values: &[Knowledge], value: ValueId) -> Option<&Exact> {
    match values.get(value.index())? {
        Knowledge::Exact(value) => Some(value),
        _ => None,
    }
}

fn builtin_primitive(builtin: crate::check::BuiltinCall) -> bool {
    use crate::check::BuiltinCall as B;
    matches!(
        builtin,
        B::JsUndefined
            | B::JsTypeOf
            | B::JsIsNullish
            | B::JsIsFalse
            | B::JsIsUndefined
            | B::JsStrictEqual
            | B::JsStrictNotEqual
    )
}

fn builtin(
    program: &Program<'_>,
    builtin: crate::check::BuiltinCall,
    args: &[Exact],
    work: &mut Work,
) -> Option<Exact> {
    use crate::check::BuiltinCall as B;
    match (builtin, args) {
        (B::JsUndefined, []) => Some(Exact::Undefined),
        (B::JsIsNullish, [value]) => Some(Exact::Boolean(matches!(
            value,
            Exact::Null | Exact::Undefined
        ))),
        (B::JsIsFalse, [value]) => Some(Exact::Boolean(matches!(value, Exact::Boolean(false)))),
        (B::JsIsUndefined, [value]) => Some(Exact::Boolean(matches!(value, Exact::Undefined))),
        (B::JsTypeOf, [value]) => {
            let name = match value {
                Exact::Integer(_) | Exact::Number(_) => "number",
                Exact::Boolean(_) => "boolean",
                Exact::String(_) => "string",
                Exact::Null => "object",
                Exact::Undefined => "undefined",
            };
            work.reserve_evaluation(64)
                .then(|| computed(name.encode_utf16().collect()))
        }
        (B::JsStrictEqual | B::JsStrictNotEqual, [left, right]) => {
            let equal = if let (Some(left), Some(right)) = (number(left), number(right)) {
                // Strict numeric equality identifies both zero signs and
                // rejects NaN, independent of the stored binary64 payload.
                left == right
            } else if let (Some(left), Some(right)) =
                (string(program, left), string(program, right))
            {
                if !work.charge(
                    (left.storage_bytes() as u64).checked_add(right.storage_bytes() as u64)?,
                ) {
                    return None;
                }
                left.code_units().eq(right.code_units())
            } else {
                left == right
            };
            Some(Exact::Boolean(equal == (builtin == B::JsStrictEqual)))
        }
        _ => None,
    }
}

pub(super) fn call(
    program: &Program<'_>,
    effects: &ProgramEffects,
    unit: UnitId,
    call: CallId,
    values: &[Knowledge],
    pristine: bool,
    work: &mut Work,
) -> Attempt {
    let mut refusal = Refusal::Unsupported;
    let value = work.temporary(|work| {
        let mut evaluator = Evaluator {
            program,
            effects,
            pristine,
            work,
            steps: MAX_STEPS,
            refusal,
        };
        let value = evaluator.call(unit, call, values, 0);
        refusal = if evaluator.work.truncated() {
            Refusal::Limit
        } else {
            evaluator.refusal
        };
        value
    });
    Attempt {
        refusal: value.is_none().then_some(refusal),
        value,
    }
}

struct Evaluator<'a, 'src, 'ledger> {
    program: &'a Program<'src>,
    effects: &'a ProgramEffects,
    pristine: bool,
    work: &'a mut Work<'ledger>,
    steps: u32,
    refusal: Refusal,
}

enum Flow {
    Next,
    Return(Exact),
    Break,
    Continue,
}

impl Evaluator<'_, '_, '_> {
    fn step(&mut self) -> Option<()> {
        let Some(steps) = self.steps.checked_sub(1) else {
            self.refusal = Refusal::Limit;
            return None;
        };
        self.steps = steps;
        self.work.charge(1).then_some(())
    }

    fn call(
        &mut self,
        unit: UnitId,
        call: CallId,
        values: &[Knowledge],
        depth: usize,
    ) -> Option<Exact> {
        self.step()?;
        if depth >= MAX_DEPTH {
            self.refusal = Refusal::Limit;
            return None;
        }
        let caller = self.program.unit(unit)?;
        let callee = self.effects.graph().callee(unit, call);
        match callee {
            // This checked numeric primitive lowers to ToInt32 itself. It
            // does not look up a mutable Number/Math method. Other supported
            // methods still need the host assumption before evaluation.
            Callee::Intrinsic(operation)
                if self.pristine
                    || !crate::catalog::host_replaceable(operation)
                    || matches!(operation, ResolvedIntrinsic::Method(Intrinsic::FloatToInt)) =>
            {
                if let Some(refusal) = intrinsic_refusal(operation) {
                    self.refusal = refusal;
                    return None;
                }
            }
            Callee::Builtin(crate::check::BuiltinCall::MathImul) if self.pristine => {}
            Callee::Builtin(operation) if builtin_primitive(operation) => {}
            Callee::Unit(body) if self.program.unit(body)?.operations.len() <= MAX_OPERATIONS => {}
            Callee::Unit(_) => {
                self.refusal = Refusal::Limit;
                return None;
            }
            _ => {
                self.refusal = Refusal::Host;
                return None;
            }
        }
        let site = &caller.calls[call.index()];
        let arguments = caller.arguments(site.arguments)?;
        if arguments.len() > MAX_OPERATIONS {
            self.refusal = Refusal::Limit;
            return None;
        }
        let mut args = self
            .work
            .admit(|budget| budget.vector(Scratch, arguments.len()))?;
        for argument in arguments {
            let CallArgument::Value(value) = argument else {
                return None;
            };
            args.push(known(values, *value)?.clone());
        }
        let result = (|| match callee {
            Callee::Builtin(crate::check::BuiltinCall::MathImul) if self.pristine => {
                let [left, right] = args.as_slice() else {
                    return None;
                };
                Some(Exact::Integer(integer(left)?.wrapping_mul(integer(right)?)))
            }
            Callee::Builtin(operation) if builtin_primitive(operation) => {
                builtin(self.program, operation, &args, self.work)
            }
            Callee::Intrinsic(operation) => {
                let CallTarget::Intrinsic { receiver, .. } = site.target else {
                    return None;
                };
                let receiver = match receiver {
                    Some(value) => Some(known(values, value)?),
                    None => None,
                };
                let result = intrinsic(self.program, operation, receiver, &args, self.work);
                if result.is_none()
                    && matches!(
                        operation,
                        ResolvedIntrinsic::Method(
                            Intrinsic::StringToLowerCase | Intrinsic::StringToUpperCase
                        )
                    )
                {
                    self.refusal = Refusal::Engine;
                }
                result
            }
            Callee::Unit(callee) => self.unit(callee, &args, depth + 1),
            _ => None,
        })();
        self.work.release(Scratch, args)?;
        result
    }

    fn unit(&mut self, unit: UnitId, args: &[Exact], depth: usize) -> Option<Exact> {
        let data = self.program.unit(unit)?;
        if data.suspension != Suspension::None
            || data.constructor_of.is_some()
            || data.operations.len() > MAX_OPERATIONS
            || data.parameters.len() != args.len()
        {
            return None;
        }
        let bytes = data
            .values
            .len()
            .checked_mul(std::mem::size_of::<Knowledge>())?
            .checked_add(data.operations.len().checked_mul(128)?)?;
        if !self.work.reserve_policy(bytes as u64) {
            return None;
        }
        let mut values = self.work.collect(
            Scratch,
            std::iter::repeat_n(
                Knowledge::Unknown(facts::UnknownReason::Unvisited),
                data.values.len(),
            ),
        )?;
        let mut cells = Map::new(Scratch);
        for (&cell, value) in data.parameters.iter().zip(args) {
            self.work
                .admit(|budget| cells.insert(cell, value.clone(), budget))?;
        }
        let result = match self.region(unit, data.entry, &mut values, &mut cells, depth) {
            Some(Flow::Return(value)) => Some(value),
            Some(Flow::Next) => Some(Exact::Undefined),
            Some(Flow::Break | Flow::Continue) | None => None,
        };
        self.work.release(Scratch, values)?;
        self.work.admit(|budget| cells.release(budget))?;
        result
    }

    fn region(
        &mut self,
        unit: UnitId,
        region: RegionId,
        values: &mut [Knowledge],
        cells: &mut Map<CellId, Exact>,
        depth: usize,
    ) -> Option<Flow> {
        let data = self.program.unit(unit)?;
        for &op in &data.regions[region.index()].operations {
            self.step()?;
            let operation = &data.operations[op.index()];
            let operands = data.operands(operation.operands)?;
            let first = || known(values, *operands.first()?);
            let mut result = None;
            match operation.kind {
                OperationKind::Return => {
                    return Some(Flow::Return(if operands.is_empty() {
                        Exact::Undefined
                    } else {
                        first()?.clone()
                    }))
                }
                OperationKind::Break => return Some(Flow::Break),
                OperationKind::Continue => return Some(Flow::Continue),
                OperationKind::Declare(cell) if self.program.cells[cell.index()].owner == unit => {
                    self.work.admit(|budget| cells.remove(&cell, budget))?;
                }
                OperationKind::Initialize(cell)
                    if self.program.cells[cell.index()].owner == unit =>
                {
                    let value = first()?.clone();
                    self.work
                        .admit(|budget| cells.insert(cell, value, budget))?;
                }
                OperationKind::Load(place) => {
                    let Place::Cell(cell) = data.places[place.index()] else {
                        return None;
                    };
                    result = cells.get(&cell).cloned();
                    if result.is_none() {
                        // Only hoisted function bindings may lack a primitive
                        // value; their calls are resolved by the call graph.
                        if !matches!(
                            self.program.cells[cell.index()].binding,
                            CellBinding::Function(_)
                        ) || !self.effects.initialization().initialized(
                            self.program,
                            unit,
                            op,
                            cell,
                        ) {
                            return None;
                        }
                    }
                }
                OperationKind::CheckPlace(place) | OperationKind::Store(place) => {
                    let Place::Cell(cell) = data.places[place.index()] else {
                        return None;
                    };
                    if self.program.cells[cell.index()].owner != unit || cells.get(&cell).is_none()
                    {
                        return None;
                    }
                    if matches!(operation.kind, OperationKind::Store(_)) {
                        result = Some(first()?.clone());
                        let value = result.clone()?;
                        self.work
                            .admit(|budget| cells.insert(cell, value, budget))?;
                    }
                }
                OperationKind::PrepareCall(_) => {}
                OperationKind::Call(call) => result = Some(self.call(unit, call, values, depth)?),
                OperationKind::IsUndefined { nullish, .. } => {
                    result = Some(Exact::Boolean(
                        matches!(first()?, Exact::Undefined)
                            || nullish && matches!(first()?, Exact::Null),
                    ))
                }
                OperationKind::Block(child) => {
                    let flow = self.region(unit, child, values, cells, depth)?;
                    if !matches!(flow, Flow::Next) {
                        return Some(flow);
                    }
                }
                OperationKind::If { yes, no } => {
                    let Exact::Boolean(condition) = first()? else {
                        return None;
                    };
                    let child = if *condition { Some(yes) } else { no };
                    if let Some(child) = child {
                        let flow = self.region(unit, child, values, cells, depth)?;
                        if !matches!(flow, Flow::Next) {
                            return Some(flow);
                        }
                        result = data.regions[child.index()]
                            .result
                            .and_then(|value| known(values, value))
                            .cloned();
                    }
                }
                OperationKind::Select { yes, no } => {
                    let Exact::Boolean(condition) = first()? else {
                        return None;
                    };
                    let child = if *condition { yes } else { no };
                    let flow = self.region(unit, child, values, cells, depth)?;
                    if !matches!(flow, Flow::Next) {
                        return Some(flow);
                    }
                    result = Some(known(values, data.regions[child.index()].result?)?.clone());
                }
                OperationKind::ShortCircuit { kind, right } => {
                    let left = first()?.clone();
                    let take = match kind {
                        ShortCircuit::Nullish => matches!(left, Exact::Null | Exact::Undefined),
                        ShortCircuit::BooleanAnd | ShortCircuit::JavaScriptAnd => {
                            facts::truthy(self.program, &left)
                        }
                        ShortCircuit::BooleanOr | ShortCircuit::JavaScriptOr => {
                            !facts::truthy(self.program, &left)
                        }
                    };
                    result = Some(if take {
                        let flow = self.region(unit, right, values, cells, depth)?;
                        if !matches!(flow, Flow::Next) {
                            return Some(flow);
                        }
                        known(values, data.regions[right.index()].result?)?.clone()
                    } else {
                        left
                    });
                }
                OperationKind::Loop { test, body, update } => loop {
                    // Empty `for(;;){}` still spends steps. Iterations and
                    // nested calls share one budget; completion is the proof.
                    self.step()?;
                    if !matches!(self.region(unit, test, values, cells, depth)?, Flow::Next) {
                        return None;
                    }
                    if let Some(condition) = data.regions[test.index()].result {
                        let Exact::Boolean(condition) = known(values, condition)? else {
                            return None;
                        };
                        if !*condition {
                            break;
                        }
                    }
                    match self.region(unit, body, values, cells, depth)? {
                        Flow::Break => break,
                        flow @ Flow::Return(_) => return Some(flow),
                        Flow::Next | Flow::Continue => {}
                    }
                    // Continue runs the update; break and return do not.
                    if !matches!(self.region(unit, update, values, cells, depth)?, Flow::Next) {
                        return None;
                    }
                },
                OperationKind::Constant(_)
                | OperationKind::CopyValue
                | OperationKind::IntBinary(_)
                | OperationKind::Binary(_)
                | OperationKind::Unary { .. }
                | OperationKind::Intrinsic(ResolvedIntrinsic::Property(Intrinsic::StringLength)) => {
                    let Knowledge::Exact(value) =
                        facts::exact(self.program, data, operation, values, self.work)
                    else {
                        return None;
                    };
                    result = Some(value);
                }
                _ => return None,
            }
            if let Some(id) = operation.result {
                if let Some(value) = result {
                    values[id.index()] = Knowledge::Exact(value);
                }
            }
        }
        Some(Flow::Next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn text(value: &str) -> Exact {
        Exact::String(StoredString::Computed(Arc::new(StringValue::from(value))))
    }

    fn spelling(value: &Exact) -> String {
        match value {
            Exact::String(StoredString::Computed(value)) => crate::js_string::literal(value, '"'),
            Exact::Integer(value) => value.to_string(),
            Exact::Number(bits) => {
                let value = f64::from_bits(*bits);
                if value.is_nan() {
                    "NaN".into()
                } else if value == f64::INFINITY {
                    "Infinity".into()
                } else if value == f64::NEG_INFINITY {
                    "-Infinity".into()
                } else if value == 0.0 && value.is_sign_negative() {
                    "-0".into()
                } else {
                    value.to_string()
                }
            }
            _ => panic!("oracle input"),
        }
    }

    fn observation(value: &Exact) -> serde_json::Value {
        match value {
            Exact::String(StoredString::Computed(value)) => {
                serde_json::json!({"units":value.code_units().collect::<Vec<_>>()})
            }
            Exact::Integer(_) | Exact::Number(_) => serde_json::json!({"number":spelling(value)}),
            Exact::Boolean(value) => serde_json::json!({"boolean":value}),
            _ => panic!("oracle output"),
        }
    }

    #[test]
    fn exact_methods_match_node_on_utf16_indices_whitespace_and_signed_zero() {
        use Intrinsic as I;
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, "").unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        let mut cases = vec![
            (I::IntToString, Exact::Integer(i32::MIN), vec![], "toString"),
            (
                I::IntToString,
                Exact::Integer(-31),
                vec![Exact::Integer(16)],
                "toString",
            ),
            (
                I::IntToString,
                Exact::Integer(35),
                vec![Exact::Integer(36)],
                "toString",
            ),
            (
                I::IntToUnsignedString,
                Exact::Integer(-1),
                vec![Exact::Integer(16)],
                "toString",
            ),
            (
                I::StringSlice,
                text("a😀b"),
                vec![Exact::Integer(2), Exact::Integer(3)],
                "slice",
            ),
            (
                I::StringSlice,
                text("a😀b"),
                vec![Exact::Integer(-3), Exact::Integer(-1)],
                "slice",
            ),
            (
                I::StringCharAt,
                text("😀"),
                vec![Exact::Integer(0)],
                "charAt",
            ),
            (
                I::StringCharAt,
                text("😀"),
                vec![Exact::Integer(-1)],
                "charAt",
            ),
            (
                I::StringCharCodeAt,
                text("😀"),
                vec![Exact::Integer(1)],
                "charCodeAt",
            ),
            (
                I::StringCharCodeAt,
                text("😀"),
                vec![Exact::Integer(9)],
                "charCodeAt",
            ),
            (
                I::StringCharCodeAtNumber,
                text("😀"),
                vec![Exact::Integer(1)],
                "charCodeAt",
            ),
            (
                I::StringCharCodeAtNumber,
                text("😀"),
                vec![Exact::Integer(-1)],
                "charCodeAt",
            ),
            (
                I::StringCharCodeAtNumber,
                text(""),
                vec![Exact::Integer(0)],
                "charCodeAt",
            ),
            (
                I::StringIndexOf,
                text("😀xy😀"),
                vec![text("😀"), Exact::Integer(1)],
                "indexOf",
            ),
            (
                I::StringLastIndexOf,
                text("xyxy"),
                vec![text("xy"), Exact::Integer(-1)],
                "lastIndexOf",
            ),
            (
                I::StringIndexOf,
                text("abc"),
                vec![text(""), Exact::Integer(99)],
                "indexOf",
            ),
            (
                I::StringStartsWith,
                text("abc"),
                vec![text("bc"), Exact::Integer(1)],
                "startsWith",
            ),
            (
                I::StringEndsWith,
                text("abc"),
                vec![text("b"), Exact::Integer(2)],
                "endsWith",
            ),
            (
                I::StringTrim,
                text("\u{feff}\u{85}X\u{180e}\u{a0}"),
                vec![],
                "trim",
            ),
            (
                I::StringTrimStart,
                text("\u{2028}x \n"),
                vec![],
                "trimStart",
            ),
            (I::StringTrimEnd, text("\n x\u{2029}"), vec![], "trimEnd"),
            (
                I::StringRepeat,
                text("😀"),
                vec![Exact::Integer(2)],
                "repeat",
            ),
            (I::StringToUpperCase, text("azAZ09"), vec![], "toUpperCase"),
            (I::StringToLowerCase, text("azAZ09"), vec![], "toLowerCase"),
        ];
        for value in [
            -0.0,
            -0.1,
            -0.5,
            -1.5,
            0.5,
            4503599627370496.0,
            f64::INFINITY,
            f64::NAN,
        ] {
            for (method, name) in [
                (I::FloatRound, "round"),
                (I::FloatFloor, "floor"),
                (I::FloatCeil, "ceil"),
                (I::FloatAbs, "abs"),
            ] {
                cases.push((method, Exact::Number(value.to_bits()), vec![], name));
            }
            for (method, name) in [(I::FloatMin, "min"), (I::FloatMax, "max")] {
                cases.push((
                    method,
                    Exact::Number(value.to_bits()),
                    vec![Exact::Number((-0.0f64).to_bits())],
                    name,
                ));
            }
        }
        let mut expected = Vec::new();
        let mut expressions = Vec::new();
        for (method, receiver, args, name) in cases {
            let result = intrinsic(
                &program,
                ResolvedIntrinsic::Method(method),
                Some(&receiver),
                &args,
                &mut Work::bounded(1_000_000, 1_000_000),
            )
            .expect(name);
            expected.push(observation(&result));
            let mut expression = if matches!(receiver, Exact::Number(_)) {
                let mut operands = vec![spelling(&receiver)];
                operands.extend(args.iter().map(spelling));
                format!("Math.{name}({})", operands.join(","))
            } else {
                let receiver = if method == I::IntToUnsignedString {
                    format!("({}>>>0)", spelling(&receiver))
                } else {
                    spelling(&receiver)
                };
                format!(
                    "({}).{name}({})",
                    receiver,
                    args.iter().map(spelling).collect::<Vec<_>>().join(",")
                )
            };
            if method == I::StringCharCodeAt {
                expression = format!("({expression}|0)");
            }
            expressions.push(expression);
        }
        let script = format!(
            r#"const obs=v=>typeof v==='string'?{{units:Array.from({{length:v.length}},(_,i)=>v.charCodeAt(i))}}:typeof v==='boolean'?{{boolean:v}}:{{number:Object.is(v,-0)?'-0':String(v)}};console.log(JSON.stringify([{}].map(obs)))"#,
            expressions.join(",")
        );
        let output = Command::new("node").args(["-e", &script]).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let actual: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn constant_method_limits_refuse_expansion_and_unknown_unicode_case() {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, "").unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        for (method, receiver, args) in [
            (
                Intrinsic::StringRepeat,
                text("x"),
                vec![Exact::Integer(i32::MAX)],
            ),
            (Intrinsic::StringRepeat, text("x"), vec![Exact::Integer(-1)]),
            (Intrinsic::StringToUpperCase, text("ß"), vec![]),
        ] {
            assert!(intrinsic(
                &program,
                ResolvedIntrinsic::Method(method),
                Some(&receiver),
                &args,
                &mut Work::bounded(4096, 4096)
            )
            .is_none());
        }
    }
}
