//! The one effects owner (architecture §7 "Effects"; plan M6.2 and M6.3).
//!
//! **Per operation.** `operation_effects` is the one transfer from a checked
//! operation to what evaluating it may do: reads and writes by region,
//! throwing, divergence, running code the program cannot see, suspension and
//! fresh identity. `facts::operation_evaluation_behavior` is its projection
//! for liveness, and a call's answer comes from the callee's summary.
//!
//! **Per unit.** A summary joins a body's operations, bottom-up over the call
//! graph's components (callees first; a recursive component iterates to a
//! fixed point). Operations on the unit's own cells and on objects it
//! allocated are invisible to its callers and are masked out. An exception
//! a `try` catches does not escape. Seeds: a declared `pure` body is summarized
//! from its body like any other, and the contract (M6.3) is checked against
//! that summary; a `pure extern` is trusted to have no observable effect.
//!
//! **Two questions, one analysis.** The language's types are what a body is
//! judged by for its contract: an `int` is an int, a class instance is the
//! program's data object. At run time a typed value that came from outside
//! the program may be raw (D2): an `int` parameter may hold an object whose
//! conversion runs user code, a reference parameter may be a host object with
//! accessors. A summary therefore also records its *obligations*: the
//! parameters it assumes well-formed (primitive, int32, a program object) and
//! whether it relied on typed data it cannot trace to a parameter
//! (`untrusted`). A call discharges each obligation from its argument; one it
//! cannot discharge leaves the call with the conservative behavior of a
//! conversion hook. The contract check ignores obligations; removing a call
//! requires them discharged.
//!
//! **Termination (D3.6, as settled).** A call is removable only when its body
//! provably terminates: every loop has a counted bound and no call reaches a
//! recursive component. That holds for declared and inferred purity alike; a
//! `pure extern`'s host code has no proof, so a discarded `pure extern` call
//! stays. The pending amendment (a declared `pure` asserts termination) is
//! `DECLARED_PURE_ASSERTS_TERMINATION`, off until the owner rules.
use super::call_graph::{callback_intrinsic, CallGraph, Callee, Seal};
use super::facts::{self, EvaluationBehavior, MemoryAccess};
use super::initialization::ProgramInitialization;
use super::views::{Deps, Fact, Limit, Reason};
use super::*;
use crate::check::BuiltinCall;
use crate::primitive::{Intrinsic, ResolvedIntrinsic};
use crate::typed_array::TypedArrayKind;
use ahash::AHashMap;
use std::sync::Arc;

/// D3.6 amendment, awaiting an owner ruling (architecture §7): whether a
/// declared `pure` body or `pure extern` asserts that it terminates.
const DECLARED_PURE_ASSERTS_TERMINATION: bool = false;

/// Bound on the iterations of one recursive component's fixed point.
const COMPONENT_ITERATIONS: usize = 32;
/// Bound on a unit's value-fact fixed point over its own cells.
const VALUE_ITERATIONS: usize = 8;

/// Where a read or write lands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct Regions(u8);

impl Regions {
    pub const NONE: Self = Self(0);
    /// The invocation's own cells: its locals and value parameters.
    pub const OWN_CELLS: Self = Self(1);
    /// Objects the invocation allocated.
    pub const OWN_OBJECTS: Self = Self(2);
    /// Cells outside the invocation: captured, module and caller storage.
    pub const CELLS: Self = Self(4);
    /// Objects the invocation did not allocate.
    pub const FIELDS: Self = Self(8);
    /// Host state: globals, host objects, output.
    pub const HOST: Self = Self(16);
    pub const OWN: Self = Self(1 | 2);
    pub const OBSERVABLE: Self = Self(4 | 8 | 16);
    pub const ANY: Self = Self(31);

    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
    /// What the invocation's caller can observe.
    pub const fn observable(self) -> Self {
        self.intersect(Self::OBSERVABLE)
    }
}

/// A set of parameter positions of the unit under analysis. Positions past
/// 62 share one bit that no argument can discharge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
pub struct ParameterSet(u64);

impl ParameterSet {
    pub const EMPTY: Self = Self(0);
    const OVERFLOW: u64 = 1 << 63;

    pub fn single(position: usize) -> Self {
        if position < 63 {
            Self(1 << position)
        } else {
            Self(Self::OVERFLOW)
        }
    }
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
    pub const fn overflows(self) -> bool {
        self.0 & Self::OVERFLOW != 0
    }
    pub fn contains(self, position: usize) -> bool {
        !Self::single(position).intersect(self).is_empty()
    }
    const fn intersect(self, other: Self) -> Self {
        Self(self.0 & other.0)
    }
    /// Discharge-able positions in increasing order.
    pub fn positions(self) -> impl Iterator<Item = usize> {
        (0..63).filter(move |position| self.0 & (1 << position) != 0)
    }
}

/// What a reference value is, as far as the unit under analysis can tell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Root {
    /// Allocated by this invocation.
    Fresh,
    /// A parameter, or data reachable from it.
    Parameter(u32),
    Unknown,
}

impl Root {
    fn join(self, other: Self) -> Self {
        if self == other {
            self
        } else {
            Self::Unknown
        }
    }
}

/// The effects of evaluating one operation, or of invoking one unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Effects {
    pub reads: Regions,
    pub writes: Regions,
    /// The one cell an access operation touches, for liveness.
    pub cell: Option<CellId>,
    pub may_throw: bool,
    /// Termination is not proven.
    pub may_diverge: bool,
    /// Code outside the program's knowledge may run: host functions,
    /// conversion hooks, accessors.
    pub runs_user_code: bool,
    /// That code may re-enter the program.
    pub reenters: bool,
    pub suspends: bool,
    pub creates_identity: bool,
    pub exhausts_resources: bool,
    /// Leaves the current region (return, break, continue, throw).
    pub transfers_control: bool,
    /// Parameters whose objects are written.
    pub mutated: ParameterSet,
    /// Obligations: parameters assumed primitive at run time.
    pub assumed_primitive: ParameterSet,
    /// Obligations: parameters assumed to be int32 numbers.
    pub assumed_int32: ParameterSet,
    /// Obligations: parameters assumed to be well-formed program objects.
    pub assumed_objects: ParameterSet,
    /// Relied on typed data from outside the program that no parameter
    /// obligation covers.
    pub untrusted: bool,
}

impl Effects {
    pub const NONE: Self = Self {
        reads: Regions::NONE,
        writes: Regions::NONE,
        cell: None,
        may_throw: false,
        may_diverge: false,
        runs_user_code: false,
        reenters: false,
        suspends: false,
        creates_identity: false,
        exhausts_resources: false,
        transfers_control: false,
        mutated: ParameterSet::EMPTY,
        assumed_primitive: ParameterSet::EMPTY,
        assumed_int32: ParameterSet::EMPTY,
        assumed_objects: ParameterSet::EMPTY,
        untrusted: false,
    };
    /// Anything at all: an unknown callee or host operation.
    pub const UNKNOWN: Self = Self {
        reads: Regions::ANY,
        writes: Regions::ANY,
        may_throw: true,
        may_diverge: true,
        runs_user_code: true,
        reenters: true,
        suspends: true,
        creates_identity: true,
        exhausts_resources: true,
        transfers_control: true,
        ..Self::NONE
    };

    pub fn join(&mut self, other: Self) {
        self.reads = self.reads.union(other.reads);
        self.writes = self.writes.union(other.writes);
        self.cell = if self.cell == other.cell {
            self.cell
        } else {
            None
        };
        self.may_throw |= other.may_throw;
        self.may_diverge |= other.may_diverge;
        self.runs_user_code |= other.runs_user_code;
        self.reenters |= other.reenters;
        self.suspends |= other.suspends;
        self.creates_identity |= other.creates_identity;
        self.exhausts_resources |= other.exhausts_resources;
        self.transfers_control |= other.transfers_control;
        self.mutated = self.mutated.union(other.mutated);
        self.assumed_primitive = self.assumed_primitive.union(other.assumed_primitive);
        self.assumed_int32 = self.assumed_int32.union(other.assumed_int32);
        self.assumed_objects = self.assumed_objects.union(other.assumed_objects);
        self.untrusted |= other.untrusted;
    }
    fn joined(mut self, other: Self) -> Self {
        self.join(other);
        self
    }

    /// Whether the effects rest on assumptions this evaluation cannot
    /// discharge by itself.
    pub fn obligated(&self) -> bool {
        self.untrusted
            || !self.assumed_primitive.is_empty()
            || !self.assumed_int32.is_empty()
            || !self.assumed_objects.is_empty()
    }

    /// The liveness projection. An undischarged obligation means a raw value
    /// could reach a conversion hook or an accessor here.
    pub fn behavior(&self) -> EvaluationBehavior {
        let access = |regions: Regions, mutated: bool| {
            if regions.is_empty() && !mutated {
                MemoryAccess::None
            } else if let (Some(cell), false) = (self.cell, mutated) {
                if Regions::OWN_CELLS.union(Regions::CELLS).contains(regions) {
                    MemoryAccess::Cell(cell)
                } else {
                    MemoryAccess::Unknown
                }
            } else {
                MemoryAccess::Unknown
            }
        };
        let behavior = EvaluationBehavior {
            reads: access(self.reads, false),
            writes: access(self.writes, !self.mutated.is_empty()),
            may_throw: self.may_throw,
            may_exhaust_resources: self.exhausts_resources,
            may_diverge: self.may_diverge,
            may_reenter: self.runs_user_code || self.reenters,
            may_suspend: self.suspends,
            creates_identity: self.creates_identity,
            transfers_control: self.transfers_control,
        };
        if self.obligated() {
            behavior.join(EvaluationBehavior::COERCION)
        } else {
            behavior
        }
    }

    fn from_behavior(behavior: EvaluationBehavior) -> Self {
        let region = |access: MemoryAccess| match access {
            MemoryAccess::None => Regions::NONE,
            _ => Regions::OBSERVABLE,
        };
        Self {
            reads: region(behavior.reads),
            writes: region(behavior.writes),
            may_throw: behavior.may_throw,
            may_diverge: behavior.may_diverge,
            runs_user_code: behavior.may_reenter,
            reenters: behavior.may_reenter,
            suspends: behavior.may_suspend,
            creates_identity: behavior.creates_identity,
            exhausts_resources: behavior.may_exhaust_resources,
            transfers_control: behavior.transfers_control,
            ..Self::NONE
        }
    }
}

/// Value evidence an effect transfer reads. The summary builder proves it
/// relative to the unit's parameters; liveness supplies its own domains.
pub(super) trait ValueFacts {
    /// Primitive at run time when the parameters in the set are; `None`
    /// when not proven.
    fn primitive(&self, value: ValueId) -> Option<ParameterSet>;
    /// An int32 number when the parameters in the set are.
    fn int32(&self, value: ValueId) -> Option<ParameterSet>;
    fn root(&self, value: ValueId) -> Root;
}

/// Where the transfer runs: the unit and what it can resolve.
pub(super) struct Context<'a, 'src> {
    pub program: &'a Program<'src>,
    pub unit: UnitId,
    pub data: &'a UnitData,
    pub graph: Option<&'a CallGraph>,
    pub summaries: Option<&'a [Fact<UnitEffects>]>,
}

impl Context<'_, '_> {
    fn ty(&self, value: ValueId) -> &Type<'_> {
        &self.program.types[self.data.values[value.index()].ty.index()]
    }
    fn summary(&self, unit: UnitId) -> Option<&UnitEffects> {
        self.summaries?.get(unit.index())?.known()
    }
}

/// A primitive at the language level: no conversion of it runs user code.
fn primitive_type(ty: &Type<'_>) -> bool {
    match ty {
        Type::Int
        | Type::Float
        | Type::Enum(_)
        | Type::String
        | Type::Bool
        | Type::Null
        | Type::Void
        | Type::Symbol => true,
        Type::Nullable(inner) => primitive_type(inner),
        Type::Union(members) => members.iter().all(primitive_type),
        _ => false,
    }
}

/// A host object: a dynamic value or an extern class instance.
fn host_type(program: &Program<'_>, ty: &Type<'_>) -> bool {
    match ty {
        Type::TypeParameter("$js") => true,
        Type::Class(name) | Type::ClassInstance { name, .. } => {
            program.class(name).is_some_and(|class| class.external)
        }
        Type::Nullable(inner) => host_type(program, inner),
        Type::Union(members) => members.iter().any(|member| host_type(program, member)),
        _ => false,
    }
}

fn region_operation(kind: &OperationKind) -> bool {
    matches!(
        kind,
        OperationKind::If { .. }
            | OperationKind::Loop { .. }
            | OperationKind::Try { .. }
            | OperationKind::Block(_)
            | OperationKind::Select { .. }
            | OperationKind::ShortCircuit { .. }
    )
}

/// The one per-operation transfer (see the module comment). A structured
/// operation's own evaluation has no effect; its child regions are answered
/// by the unit summary, which proves loop termination.
pub(super) fn operation_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    operation: &Operation,
) -> Effects {
    use OperationKind as Op;
    let operands = ctx.data.operands(operation.operands).unwrap_or(&[]);
    match &operation.kind {
        Op::IntBinary(_) | Op::Unary { .. } | Op::Binary(_) => {
            primitive_effects(ctx, values, operation, operands)
        }
        Op::Template => {
            let proven = proven(values, operands);
            let typed = operands.iter().all(|&value| primitive_type(ctx.ty(value)));
            let safe = Effects {
                exhausts_resources: true,
                ..Effects::NONE
            };
            match proven {
                Some(bits) => Effects {
                    assumed_primitive: bits,
                    ..safe
                },
                None if typed => Effects {
                    untrusted: true,
                    ..safe
                },
                None => Effects::from_behavior(EvaluationBehavior::COERCION),
            }
        }
        Op::Constant(_) | Op::IsUndefined => Effects::NONE,
        // `typeof` never throws; `Array.isArray` throws on a revoked proxy.
        Op::TypeTest(target) => {
            match crate::primitive::runtime_type_test(&ctx.program.types[target.index()]) {
                Some(crate::primitive::RuntimeTypeTest::TypeOf(_)) => Effects::NONE,
                _ => Effects {
                    may_throw: true,
                    ..Effects::NONE
                },
            }
        }
        // A struct copy may build new backing; identity is not observable.
        Op::CopyValue => Effects {
            exhausts_resources: operation.result.is_none_or(|value| {
                matches!(
                    ctx.ty(value),
                    Type::Struct(_)
                        | Type::StructInstance { .. }
                        | Type::Nullable(_)
                        | Type::Union(_)
                        | Type::TypeParameter(_)
                )
            }),
            ..Effects::NONE
        },
        Op::Load(place) => {
            let mut effects = place_effects(ctx, values, *place, Access::Read);
            // An `int` or other primitive read out of a field, member or
            // element may hold a raw value that the read normalizes (D2: a
            // body keeps its own normalization), which can run a hook.
            if !matches!(
                ctx.data.places[place.index()],
                Place::Cell(_) | Place::Value(_)
            ) && operation
                .result
                .is_some_and(|result| primitive_type(ctx.ty(result)))
            {
                effects.untrusted = true;
            }
            effects
        }
        Op::CheckPlace(place) => place_effects(ctx, values, *place, Access::Check),
        Op::Store(place) => place_effects(ctx, values, *place, Access::Write),
        Op::Initialize(cell) => Effects {
            writes: if ctx.program.cells[cell.index()].owner == ctx.unit {
                Regions::OWN_CELLS
            } else {
                Regions::CELLS
            },
            cell: Some(*cell),
            ..Effects::NONE
        },
        Op::PrepareCall(call) => match &ctx.data.calls[call.index()].target {
            // Preparing fixes the callee; the call carries its effects.
            CallTarget::Value { .. } | CallTarget::Builtin(_) | CallTarget::Intrinsic { .. } => {
                Effects::NONE
            }
            CallTarget::Reference { place } => place_effects(ctx, values, *place, Access::Read),
        },
        Op::PrepareReference { call, position } => {
            let argument = ctx
                .data
                .arguments(ctx.data.calls[call.index()].arguments)
                .and_then(|arguments| arguments.get(*position as usize).copied());
            match argument {
                Some(CallArgument::Reference(place)) => {
                    place_effects(ctx, values, place, Access::Check)
                }
                _ => Effects::UNKNOWN,
            }
        }
        Op::Call(call) => call_effects(ctx, values, *call),
        Op::Closure(_)
        | Op::Allocate {
            kind: AllocationKind::Array | AllocationKind::Record(_) | AllocationKind::Object(_),
            ..
        } => Effects {
            creates_identity: true,
            exhausts_resources: true,
            ..Effects::NONE
        },
        // `[...a]` runs the iterator protocol of each spread operand: the
        // pristine array iterator for an array, anything for other values.
        Op::Allocate {
            kind: AllocationKind::SpreadArray(spreads),
            ..
        } => {
            let mut effects = Effects {
                creates_identity: true,
                exhausts_resources: true,
                ..Effects::NONE
            };
            for (&value, &spread) in operands.iter().zip(spreads) {
                if !spread {
                    continue;
                }
                if matches!(ctx.ty(value), Type::Array(_)) {
                    effects.join(object_effects(ctx, values, value, None, Access::Read));
                } else {
                    return Effects::UNKNOWN;
                }
            }
            effects
        }
        // A logical struct value has neither a constructor hook nor identity.
        Op::Allocate {
            kind: AllocationKind::Struct(_),
            ..
        } => Effects {
            exhausts_resources: true,
            ..Effects::NONE
        },
        Op::Return | Op::Break | Op::Continue => Effects {
            transfers_control: true,
            ..Effects::NONE
        },
        Op::Throw => Effects {
            may_throw: true,
            transfers_control: true,
            ..Effects::NONE
        },
        Op::Intrinsic(operation) => {
            let (receiver, arguments) = match operands.split_first() {
                Some((receiver, rest)) => (Some(*receiver), rest),
                None => (None, operands),
            };
            intrinsic_effects(
                ctx,
                values,
                *operation,
                receiver,
                arguments.iter().copied().map(CallArgument::Value),
            )
        }
        kind if region_operation(kind) => Effects::NONE,
        // Enumerating a record's keys reads it; a dynamic object's
        // enumeration can run proxy traps. Termination is not counted.
        Op::ForIn { .. } => match operands.first() {
            Some(&object) if !host_type(ctx.program, ctx.ty(object)) => {
                object_effects(ctx, values, object, None, Access::Read).joined(Effects {
                    may_diverge: true,
                    ..Effects::NONE
                })
            }
            _ => Effects::UNKNOWN,
        },
        // A generator's steps run its body; suspension, modules and host
        // constructors run code this analysis does not see.
        Op::ForOf { .. }
        | Op::Await
        | Op::Yield { .. }
        | Op::LoadModule { .. }
        | Op::ConstructClass
        | Op::SuperConstruct => Effects::UNKNOWN,
        _ => Effects::UNKNOWN,
    }
}

fn proven(values: &impl ValueFacts, operands: &[ValueId]) -> Option<ParameterSet> {
    operands
        .iter()
        .try_fold(ParameterSet::EMPTY, |bits, &value| {
            values.primitive(value).map(|more| bits.union(more))
        })
}

fn primitive_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    operation: &Operation,
    operands: &[ValueId],
) -> Effects {
    let safe = facts::primitive_evaluation_behavior(ctx.program, ctx.data, operation, true)
        .unwrap_or(EvaluationBehavior::UNKNOWN);
    let raw = facts::primitive_evaluation_behavior(ctx.program, ctx.data, operation, false)
        .unwrap_or(EvaluationBehavior::UNKNOWN);
    let effects = Effects::from_behavior(safe);
    if safe == raw {
        return effects;
    }
    match proven(values, operands) {
        Some(bits) => Effects {
            assumed_primitive: bits,
            ..effects
        },
        // Typed operands convert without hooks; a raw one may not.
        None if operands.iter().all(|&value| primitive_type(ctx.ty(value))) => Effects {
            untrusted: true,
            ..effects
        },
        None => Effects::from_behavior(raw),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Access {
    Read,
    Check,
    Write,
}

fn place_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    place: PlaceId,
    access: Access,
) -> Effects {
    let mut root = place;
    // Field projections of a value struct are part of their root's storage.
    for _ in 0..=ctx.data.places.len() {
        match ctx.data.places[root.index()] {
            Place::Field { base, .. } => root = base,
            Place::Cell(cell) => return cell_effects(ctx, cell, access),
            Place::Value(_) => {
                return if access == Access::Write {
                    Effects::UNKNOWN
                } else {
                    Effects::NONE
                }
            }
            Place::Member { receiver, .. } => {
                return object_effects(ctx, values, receiver, None, access)
            }
            Place::Index { receiver, key } => {
                return object_effects(ctx, values, receiver, Some(key), access)
            }
        }
    }
    Effects::UNKNOWN
}

fn cell_effects(ctx: &Context<'_, '_>, cell: CellId, access: Access) -> Effects {
    let storage = &ctx.program.cells[cell.index()];
    if storage.binding == CellBinding::Foreign {
        return match access {
            // A `pure extern` binding is trusted as declared.
            Access::Read | Access::Check if storage.declared_pure => Effects::NONE,
            // A host global read may throw (a missing binding, an import
            // before its module evaluated) or run a global accessor.
            Access::Read | Access::Check => Effects {
                reads: Regions::HOST,
                may_throw: true,
                untrusted: true,
                ..Effects::NONE
            },
            Access::Write => Effects {
                reads: Regions::HOST,
                writes: Regions::HOST,
                may_throw: true,
                runs_user_code: true,
                reenters: true,
                ..Effects::NONE
            },
        };
    }
    if ctx.program.is_reference_parameter(cell) {
        // Caller storage: a reference may designate any lexical place.
        return Effects {
            reads: Regions::CELLS,
            writes: if access == Access::Write {
                Regions::CELLS
            } else {
                Regions::NONE
            },
            may_throw: true,
            untrusted: true,
            ..Effects::NONE
        };
    }
    let own = storage.owner == ctx.unit;
    // A local read or write before its initialization throws. Function
    // bindings are initialized by their module's instantiation prefix,
    // before any code of that module (and so any of its closures) runs.
    let temporal = match storage.binding {
        CellBinding::Parameter(_) => false,
        CellBinding::Function(_) => ctx
            .program
            .unit(storage.owner)
            .is_none_or(|owner| owner.module != ctx.data.module),
        CellBinding::Local | CellBinding::Foreign => true,
    };
    let region = if own {
        Regions::OWN_CELLS
    } else {
        Regions::CELLS
    };
    let written = access == Access::Write;
    Effects {
        reads: if written { Regions::NONE } else { region },
        writes: if written { region } else { Regions::NONE },
        cell: Some(cell),
        may_throw: temporal,
        ..Effects::NONE
    }
}

fn object_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    receiver: ValueId,
    key: Option<ValueId>,
    access: Access,
) -> Effects {
    if host_type(ctx.program, ctx.ty(receiver)) {
        // Host properties may be accessors; a missing receiver throws.
        return Effects {
            reads: Regions::HOST,
            writes: if access == Access::Write {
                Regions::HOST
            } else {
                Regions::NONE
            },
            may_throw: true,
            runs_user_code: true,
            reenters: true,
            ..Effects::NONE
        };
    }
    let mut effects = Effects::NONE;
    // A write through a parameter is recorded as that parameter's mutation;
    // each caller maps it to where its argument points.
    let (read, written) = match values.root(receiver) {
        Root::Fresh => (Regions::OWN_OBJECTS, Regions::OWN_OBJECTS),
        Root::Parameter(position) => {
            let position = position as usize;
            effects.assumed_objects = ParameterSet::single(position);
            if access == Access::Write {
                effects.mutated = ParameterSet::single(position);
            }
            (Regions::FIELDS, Regions::NONE)
        }
        Root::Unknown => {
            effects.untrusted = true;
            (Regions::FIELDS, Regions::FIELDS)
        }
    };
    if access != Access::Check {
        effects.reads = read;
    }
    if access == Access::Write {
        effects.writes = written;
        effects.exhausts_resources = true;
    }
    if let Some(key) = key {
        match values.primitive(key) {
            Some(bits) => effects.assumed_primitive = effects.assumed_primitive.union(bits),
            None if primitive_type(ctx.ty(key)) => effects.untrusted = true,
            // A dynamic key's property-key conversion may run user code.
            None => effects.join(Effects::from_behavior(EvaluationBehavior::COERCION)),
        }
    }
    effects
}

fn call_effects(ctx: &Context<'_, '_>, values: &impl ValueFacts, call: CallId) -> Effects {
    let site = &ctx.data.calls[call.index()];
    let arguments = ctx.data.arguments(site.arguments).unwrap_or(&[]);
    match site.target {
        CallTarget::Value {
            callee,
            invocation: Invocation::Value,
        } => {
            let callee = ctx.graph.map_or(Callee::Unknown, |graph| {
                graph.callee_of_value(ctx.program, ctx.data, callee)
            });
            match callee {
                Callee::Unit(body) => unit_call_effects(ctx, values, body, arguments),
                Callee::Extern { pure: true, .. } => pure_extern_call(),
                _ => Effects::UNKNOWN,
            }
        }
        CallTarget::Value { .. } | CallTarget::Reference { .. } => Effects::UNKNOWN,
        CallTarget::Builtin(BuiltinCall::JsObject | BuiltinCall::JsArray) => Effects {
            creates_identity: true,
            exhausts_resources: true,
            ..Effects::NONE
        },
        CallTarget::Builtin(BuiltinCall::JsUndefined) => Effects::NONE,
        CallTarget::Builtin(BuiltinCall::Print) => Effects {
            writes: Regions::HOST,
            exhausts_resources: true,
            ..Effects::NONE
        },
        CallTarget::Builtin(_) => Effects::UNKNOWN,
        CallTarget::Intrinsic {
            operation,
            receiver,
        } => intrinsic_effects(ctx, values, operation, receiver, arguments.iter().copied()),
    }
}

/// A trusted `pure extern`: no observable effect, and a result that may be
/// fresh. Its host code has no termination proof (D3.6) and may throw.
fn pure_extern_call() -> Effects {
    Effects {
        may_throw: true,
        may_diverge: !DECLARED_PURE_ASSERTS_TERMINATION,
        creates_identity: true,
        exhausts_resources: true,
        ..Effects::NONE
    }
}

/// A call of a program body: its summary, seen from this call site. The
/// callee's parameter obligations are discharged from the arguments, and
/// its writes through parameters land where the arguments point.
fn unit_call_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    body: UnitId,
    arguments: &[CallArgument],
) -> Effects {
    let (Some(summary), Some(callee)) = (ctx.summary(body), ctx.program.unit(body)) else {
        return Effects::UNKNOWN;
    };
    if arguments.len() != callee.parameters.len() {
        return Effects::UNKNOWN;
    }
    let inner = summary.effects;
    let mut effects = Effects {
        reads: inner.reads,
        writes: inner.writes,
        cell: None,
        may_throw: inner.may_throw,
        may_diverge: inner.may_diverge,
        runs_user_code: inner.runs_user_code,
        reenters: inner.reenters,
        suspends: inner.suspends,
        creates_identity: inner.creates_identity,
        exhausts_resources: inner.exhausts_resources,
        transfers_control: false,
        untrusted: inner.untrusted,
        ..Effects::NONE
    };
    let value = |position: usize| match arguments.get(position) {
        Some(CallArgument::Value(value)) => Some(*value),
        _ => None,
    };
    if inner.mutated.overflows() {
        effects.writes = effects.writes.union(Regions::FIELDS);
    }
    for position in inner.mutated.positions() {
        match value(position).map(|argument| values.root(argument)) {
            Some(Root::Fresh) => effects.writes = effects.writes.union(Regions::OWN_OBJECTS),
            Some(Root::Parameter(own)) => {
                effects.mutated = effects.mutated.union(ParameterSet::single(own as usize))
            }
            Some(Root::Unknown) => effects.writes = effects.writes.union(Regions::FIELDS),
            None => effects.writes = effects.writes.union(Regions::CELLS),
        }
    }
    let overflow = inner.assumed_primitive.overflows()
        || inner.assumed_int32.overflows()
        || inner.assumed_objects.overflows();
    effects.untrusted |= overflow;
    for position in inner.assumed_primitive.positions() {
        match value(position).and_then(|argument| values.primitive(argument)) {
            Some(bits) => effects.assumed_primitive = effects.assumed_primitive.union(bits),
            None => effects.untrusted = true,
        }
    }
    for position in inner.assumed_int32.positions() {
        match value(position).and_then(|argument| values.int32(argument)) {
            Some(bits) => effects.assumed_int32 = effects.assumed_int32.union(bits),
            None => effects.untrusted = true,
        }
    }
    for position in inner.assumed_objects.positions() {
        match value(position).map(|argument| values.root(argument)) {
            Some(Root::Fresh) => {}
            Some(Root::Parameter(own)) => {
                effects.assumed_objects = effects
                    .assumed_objects
                    .union(ParameterSet::single(own as usize))
            }
            _ => effects.untrusted = true,
        }
    }
    effects
}

/// How an intrinsic touches its receiver and arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntrinsicClass {
    /// A computation over primitives, which convert their inputs.
    Pure {
        throws: bool,
        fresh: bool,
    },
    /// No input is converted and nothing is read or written.
    Inert {
        throws: bool,
    },
    /// Reads the receiver container.
    Read {
        fresh: bool,
    },
    /// Writes the receiver container.
    Write {
        throws: bool,
    },
    /// Creates a new object from primitive inputs.
    Construct {
        throws: bool,
    },
    /// Calls its first argument once per element of the receiver.
    Callback,
    Print,
    Unknown,
}

fn intrinsic_class(operation: ResolvedIntrinsic) -> IntrinsicClass {
    use Intrinsic as I;
    use IntrinsicClass as Class;
    let intrinsic = match operation {
        ResolvedIntrinsic::Property(intrinsic)
        | ResolvedIntrinsic::Method(intrinsic)
        | ResolvedIntrinsic::Constructor(intrinsic) => intrinsic,
    };
    if let ResolvedIntrinsic::Constructor(intrinsic) = operation {
        return match intrinsic {
            I::MapNew | I::SetNew | I::SymbolNew => Class::Construct { throws: false },
            I::ArrayBufferNew | I::SharedArrayBufferNew | I::RegexNew => {
                Class::Construct { throws: true }
            }
            _ if typed_array_constructor(intrinsic) => Class::Construct { throws: true },
            _ => Class::Unknown,
        };
    }
    if callback_intrinsic(operation) {
        return Class::Callback;
    }
    match intrinsic {
        I::IntImul
        | I::IntToString
        | I::IntToUnsignedString
        | I::FloatAbs
        | I::FloatFloor
        | I::FloatCeil
        | I::FloatRound
        | I::FloatSqrt
        | I::FloatSin
        | I::FloatCos
        | I::FloatAcos
        | I::FloatExp
        | I::FloatLog
        | I::FloatTan
        | I::FloatAtan2
        | I::FloatHypot
        | I::FloatMin
        | I::FloatMax
        | I::FloatToInt
        | I::StringLength
        | I::StringCharCodeAt
        | I::StringCharAt
        | I::StringIncludes
        | I::StringIndexOf
        | I::StringLastIndexOf
        | I::StringStartsWith
        | I::StringEndsWith
        | I::StringToUpperCase
        | I::StringToLowerCase
        | I::StringTrim
        | I::StringTrimStart
        | I::StringTrimEnd
        | I::StringSlice
        | I::StringCodePointLength
        | I::JsMathPI => Class::Pure {
            throws: false,
            fresh: false,
        },
        I::StringSplit => Class::Pure {
            throws: false,
            fresh: true,
        },
        I::StringRepeat => Class::Pure {
            throws: true,
            fresh: false,
        },
        I::JsTruthy
        | I::JsTypeOf
        | I::JsIsNullish
        | I::JsIsFalse
        | I::JsIsUndefined
        | I::JsStrictEqual
        | I::JsStrictNotEqual => Class::Inert { throws: false },
        I::JsIsArray => Class::Inert { throws: true },
        I::ArrayLength
        | I::ArrayIndexOf
        | I::ArrayIncludes
        | I::MapSize
        | I::MapGet
        | I::MapHas
        | I::SetSize
        | I::SetHas
        | I::BufferByteLength
        | I::RegexSource
        | I::RegexFlags
        | I::RegexGlobal
        | I::RegexIgnoreCase
        | I::RegexMultiline
        | I::RegexDotAll
        | I::RegexSticky
        | I::RegexUnicode => Class::Read { fresh: false },
        I::ArraySlice | I::BufferSlice => Class::Read { fresh: true },
        I::ArrayPush
        | I::ArrayPop
        | I::ArraySplice
        | I::ArrayFill
        | I::ArrayCopyWithin
        | I::ArrayReverse
        | I::TypedArrayFill
        | I::TypedArrayCopyWithin
        | I::MapSet
        | I::MapDelete
        | I::MapClear
        | I::SetAdd
        | I::SetDelete
        | I::SetClear => Class::Write { throws: false },
        // An offset past the end throws a RangeError.
        I::TypedArraySet => Class::Write { throws: true },
        I::Print => Class::Print,
        _ => match typed_array_member(intrinsic) {
            Some(fresh) => Class::Read { fresh },
            None => Class::Unknown,
        },
    }
}

fn typed_array_constructor(intrinsic: Intrinsic) -> bool {
    TypedArrayKind::ALL
        .iter()
        .any(|kind| kind.new_intrinsic() == intrinsic)
}

/// A typed array property or view method: `Some(creates a view)`.
fn typed_array_member(intrinsic: Intrinsic) -> Option<bool> {
    TypedArrayKind::ALL.iter().find_map(|kind| {
        if [
            kind.length_intrinsic(),
            kind.byte_length_intrinsic(),
            kind.byte_offset_intrinsic(),
            kind.buffer_intrinsic(),
        ]
        .contains(&intrinsic)
        {
            Some(false)
        } else if [kind.slice_intrinsic(), kind.subarray_intrinsic()].contains(&intrinsic) {
            Some(true)
        } else {
            None
        }
    })
}

/// Whether an intrinsic dispatches through a builtin the host can replace: a
/// prototype method or accessor, or a global constructor or namespace. Its
/// language meaning is what the `pure` contract judges; that the running
/// builtin still has it is an assumption about the environment, so it is an
/// undischarged obligation for removal. The primitive string and array
/// `length`, and the language operators, dispatch through nothing.
fn replaceable(operation: ResolvedIntrinsic) -> bool {
    let (ResolvedIntrinsic::Property(intrinsic)
    | ResolvedIntrinsic::Method(intrinsic)
    | ResolvedIntrinsic::Constructor(intrinsic)) = operation;
    !matches!(
        intrinsic,
        Intrinsic::StringLength
            | Intrinsic::ArrayLength
            | Intrinsic::JsTruthy
            | Intrinsic::JsTypeOf
            | Intrinsic::JsIsNullish
            | Intrinsic::JsIsFalse
            | Intrinsic::JsIsUndefined
            | Intrinsic::JsStrictEqual
            | Intrinsic::JsStrictNotEqual
    )
}

fn intrinsic_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    operation: ResolvedIntrinsic,
    receiver: Option<ValueId>,
    arguments: impl Iterator<Item = CallArgument>,
) -> Effects {
    let mut effects = classified_intrinsic_effects(ctx, values, operation, receiver, arguments);
    if replaceable(operation) {
        effects.untrusted = true;
    }
    effects
}

fn classified_intrinsic_effects(
    ctx: &Context<'_, '_>,
    values: &impl ValueFacts,
    operation: ResolvedIntrinsic,
    receiver: Option<ValueId>,
    arguments: impl Iterator<Item = CallArgument>,
) -> Effects {
    use IntrinsicClass as Class;
    let arguments = arguments.collect::<Vec<_>>();
    let argument_values = arguments
        .iter()
        .filter_map(|argument| match argument {
            CallArgument::Value(value) => Some(*value),
            CallArgument::Reference(_) => None,
        })
        .collect::<Vec<_>>();
    if argument_values.len() != arguments.len() {
        return Effects::UNKNOWN;
    }
    // Primitive inputs convert without hooks when proven; a typed raw input
    // is an obligation; a non-primitive one may run user code.
    let converted = |inputs: &mut dyn Iterator<Item = ValueId>| -> Effects {
        let mut effects = Effects::NONE;
        for value in inputs {
            match values.primitive(value) {
                Some(bits) => effects.assumed_primitive = effects.assumed_primitive.union(bits),
                None if primitive_type(ctx.ty(value)) => effects.untrusted = true,
                None => return Effects::from_behavior(EvaluationBehavior::COERCION),
            }
        }
        effects
    };
    let class = intrinsic_class(operation);
    match class {
        Class::Pure { throws, fresh } => {
            let mut effects =
                converted(&mut receiver.into_iter().chain(argument_values.iter().copied()));
            effects.may_throw |= throws;
            effects.creates_identity |= fresh;
            effects.exhausts_resources |= fresh;
            effects
        }
        Class::Inert { throws } => Effects {
            may_throw: throws,
            ..Effects::NONE
        },
        Class::Read { fresh } => {
            let Some(receiver) = receiver else {
                return Effects::UNKNOWN;
            };
            let mut effects = object_effects(ctx, values, receiver, None, Access::Read);
            effects.join(converted(
                &mut argument_values
                    .iter()
                    .copied()
                    .filter(|&value| primitive_type(ctx.ty(value))),
            ));
            effects.creates_identity |= fresh;
            effects.exhausts_resources |= fresh;
            effects
        }
        Class::Write { throws } => {
            let Some(receiver) = receiver else {
                return Effects::UNKNOWN;
            };
            let mut effects = object_effects(ctx, values, receiver, None, Access::Write);
            effects.join(converted(
                &mut argument_values
                    .iter()
                    .copied()
                    .filter(|&value| primitive_type(ctx.ty(value))),
            ));
            effects.may_throw |= throws;
            effects
        }
        Class::Construct { throws } => {
            // A constructor from an iterable runs its iterator; only
            // primitive arguments are known not to.
            if argument_values
                .iter()
                .any(|&value| !primitive_type(ctx.ty(value)))
            {
                return Effects::UNKNOWN;
            }
            let mut effects = converted(&mut argument_values.iter().copied());
            effects.creates_identity = true;
            effects.exhausts_resources = true;
            effects.may_throw |= throws;
            effects
        }
        Class::Callback => {
            let (Some(receiver), Some(&callback)) = (receiver, argument_values.first()) else {
                return Effects::UNKNOWN;
            };
            let body = match ctx
                .graph
                .map(|graph| graph.callee_of_value(ctx.program, ctx.data, callback))
            {
                Some(Callee::Unit(body)) => body,
                _ => return Effects::UNKNOWN,
            };
            let Some(summary) = ctx.summary(body) else {
                return Effects::UNKNOWN;
            };
            let inner = summary.effects;
            // Elements reach the callback raw: none of its obligations can
            // be discharged here.
            let mut effects = Effects {
                reads: inner.reads,
                writes: inner.writes,
                may_throw: inner.may_throw,
                may_diverge: inner.may_diverge,
                runs_user_code: inner.runs_user_code,
                reenters: inner.reenters,
                suspends: inner.suspends,
                creates_identity: true,
                exhausts_resources: true,
                untrusted: inner.obligated(),
                ..Effects::NONE
            };
            if !inner.mutated.is_empty() {
                effects.writes = effects.writes.union(Regions::FIELDS);
            }
            effects.join(object_effects(ctx, values, receiver, None, Access::Read));
            effects
        }
        Class::Print => Effects {
            writes: Regions::HOST,
            ..Effects::NONE
        },
        Class::Unknown => Effects::UNKNOWN,
    }
}

/// The summary of invoking one unit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitEffects {
    /// Observable effects and obligations; own cells and allocations are
    /// masked, and control transfers stay inside the body.
    pub effects: Effects,
    /// The result is primitive when these parameters are; `None` if not
    /// proven.
    pub result_primitive: Option<ParameterSet>,
    pub declared_pure: bool,
}

impl UnitEffects {
    /// A call whose obligations are discharged has no observable effect,
    /// cannot throw and provably terminates: dropping it when its result is
    /// unused is exact.
    pub fn discardable(&self) -> bool {
        let effects = &self.effects;
        effects.writes.observable().is_empty()
            && effects.mutated.is_empty()
            && !effects.may_throw
            && !effects.may_diverge
            && !effects.runs_user_code
            && !effects.reenters
            && !effects.suspends
    }
    /// The `pure` contract: no write outside the body's own cells and
    /// allocations, no host interaction and no unknown code. Reading program
    /// state, throwing, allocating and divergence are allowed; obligations
    /// about raw values are the caller's concern.
    pub fn observable_effect(&self) -> bool {
        let effects = &self.effects;
        !effects.writes.observable().is_empty()
            || !effects.mutated.is_empty()
            || effects.reads.intersects(Regions::HOST)
            || effects.runs_user_code
            || effects.suspends
    }
}

/// Every unit's summary, the call graph they were computed over and the
/// initialization facts they read: the program table effects publish (keyed
/// by `UnitId`).
#[derive(Debug)]
pub struct ProgramEffects {
    deps: Deps,
    graph: CallGraph,
    initialization: Arc<ProgramInitialization>,
    units: Vec<Fact<UnitEffects>>,
    roots: Vec<Vec<Root>>,
}

impl ProgramEffects {
    /// Effects and initialization order under `seal`, built together: each
    /// needs the other. Summaries are first computed with the
    /// initialization owner's structural answer; the root statements'
    /// effects in them decide when each body may first run; the summaries
    /// are then computed again with that complete answer, which only ever
    /// removes a temporal-dead-zone throw. Both rounds are sound.
    pub fn build(program: &Program<'_>, seal: Seal) -> Self {
        let graph = CallGraph::build(program, seal);
        let mut initialization = ProgramInitialization::structural(program, &graph);
        // Module initializers are never called, so each one's operations
        // are complete when it is summarized: callees come first.
        let mut statements = vec![Vec::new(); program.units.len()];
        Self::summarize_units(program, &graph, &initialization, Some(&mut statements));
        initialization.schedule(program, &graph, &statements);
        let initialization = Arc::new(initialization);
        let (units, roots) = Self::summarize_units(program, &graph, &initialization, None);
        Self {
            deps: Deps::of_program(program),
            graph,
            initialization,
            units,
            roots,
        }
    }

    /// Every unit's summary over `graph`, bottom-up over its components.
    /// `record` receives, per module initializer, each operation's effects.
    fn summarize_units(
        program: &Program<'_>,
        graph: &CallGraph,
        access: &ProgramInitialization,
        mut record: Option<&mut Vec<Vec<Effects>>>,
    ) -> (Vec<Fact<UnitEffects>>, Vec<Vec<Root>>) {
        let count = program.units.len();
        let mut declared = vec![false; count];
        for cell in program.cells.iter() {
            if let CellBinding::Function(unit) = cell.binding {
                if let Some(slot) = declared.get_mut(unit.index()) {
                    *slot |= cell.declared_pure;
                }
            }
        }
        let mut units: Vec<Fact<UnitEffects>> = (0..count)
            .map(|_| Fact::Truncated(Limit::Iterations))
            .collect();
        let mut roots = vec![Vec::new(); count];
        for component in graph.components() {
            let recursive = component.first().is_some_and(|&unit| graph.recursive(unit));
            // A recursive component starts from no effects and grows to its
            // least fixed point; recursion alone makes it diverge.
            for &unit in component {
                units[unit.index()] = Fact::Known(
                    UnitEffects {
                        effects: Effects {
                            may_diverge: recursive,
                            ..Effects::NONE
                        },
                        result_primitive: Some(ParameterSet::EMPTY),
                        declared_pure: declared[unit.index()],
                    },
                    Deps {
                        tables: program.tables_revision,
                        units: Vec::new(),
                    },
                );
            }
            let mut settled = false;
            for _ in 0..if recursive { COMPONENT_ITERATIONS } else { 1 } {
                let mut changed = false;
                for &unit in component {
                    // Module initializers are never called: their operations
                    // are the root statements the initialization owner reads.
                    let operations = match record.as_deref_mut() {
                        Some(record)
                            if program.unit(unit).is_some_and(|data| {
                                data.kind == UnitKind::ModuleInitialization
                            }) =>
                        {
                            let slot = &mut record[unit.index()];
                            slot.clear();
                            Some(slot)
                        }
                        _ => None,
                    };
                    let (summary, unit_roots) = summarize(
                        program,
                        graph,
                        &units,
                        unit,
                        declared[unit.index()],
                        access,
                        operations,
                    );
                    let summary = match summary {
                        Fact::Known(mut summary, deps) => {
                            summary.effects.may_diverge |= recursive;
                            Fact::Known(summary, deps)
                        }
                        other => other,
                    };
                    changed |= units[unit.index()] != summary;
                    units[unit.index()] = summary;
                    roots[unit.index()] = unit_roots;
                }
                if !changed || !recursive {
                    settled = true;
                    break;
                }
            }
            if !settled {
                for &unit in component {
                    units[unit.index()] = Fact::Truncated(Limit::Iterations);
                }
            }
        }
        (units, roots)
    }

    pub fn deps(&self) -> &Deps {
        &self.deps
    }
    pub fn graph(&self) -> &CallGraph {
        &self.graph
    }
    /// The initialization owner these summaries were computed with.
    pub fn initialization(&self) -> &Arc<ProgramInitialization> {
        &self.initialization
    }
    pub fn unit(&self, unit: UnitId) -> Option<&Fact<UnitEffects>> {
        self.units.get(unit.index())
    }
    pub fn summary(&self, unit: UnitId) -> Option<&UnitEffects> {
        self.unit(unit)?.known()
    }
    pub(super) fn summaries(&self) -> &[Fact<UnitEffects>] {
        &self.units
    }
    pub(super) fn roots(&self, unit: UnitId) -> &[Root] {
        self.roots.get(unit.index()).map_or(&[], Vec::as_slice)
    }

    /// Declared `pure` units whose summary shows an observable effect, in
    /// unit order (M6.3).
    pub fn pure_violations(&self) -> Vec<UnitId> {
        self.units
            .iter()
            .enumerate()
            .filter_map(|(index, fact)| match fact {
                Fact::Known(summary, _) if summary.declared_pure && summary.observable_effect() => {
                    UnitId::from_index(index)
                }
                _ => None,
            })
            .collect()
    }
}

/// Value evidence for one operation outside a summary: liveness's proven
/// primitive domains, the summary's roots, and int32 from the producer.
pub(super) struct DomainFacts<'a> {
    pub data: &'a UnitData,
    pub domains: &'a [bool],
    pub roots: &'a [Root],
}

impl ValueFacts for DomainFacts<'_> {
    fn primitive(&self, value: ValueId) -> Option<ParameterSet> {
        (self.domains.get(value.index()) == Some(&true)).then_some(ParameterSet::EMPTY)
    }
    fn int32(&self, value: ValueId) -> Option<ParameterSet> {
        int32_producer(self.data, value).then_some(ParameterSet::EMPTY)
    }
    fn root(&self, value: ValueId) -> Root {
        self.roots.get(value.index()).copied().unwrap_or_else(|| {
            match self.data.operations[self.data.values[value.index()].definition.index()].kind {
                OperationKind::Allocate {
                    kind: AllocationKind::Array | AllocationKind::Record(_) | AllocationKind::Object(_),
                    ..
                } => Root::Fresh,
                _ => Root::Unknown,
            }
        })
    }
}

/// An int32 by its producer: integer constants and arithmetic, and lengths,
/// which are `int` by the language's contract.
fn int32_producer(data: &UnitData, value: ValueId) -> bool {
    let mut value = value;
    for _ in 0..=data.values.len() {
        let Some(entry) = data.values.get(value.index()) else {
            return false;
        };
        let operation = &data.operations[entry.definition.index()];
        match operation.kind {
            OperationKind::Constant(Constant::Integer(_))
            | OperationKind::IntBinary(_)
            | OperationKind::Unary { integer: true, .. } => return true,
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(intrinsic)) => {
                return length_intrinsic(intrinsic)
            }
            OperationKind::CopyValue => {
                match data
                    .operands(operation.operands)
                    .and_then(|operands| operands.first())
                {
                    Some(&operand) => value = operand,
                    None => return false,
                }
            }
            _ => return false,
        }
    }
    false
}

/// The lengths that are `int` by the language's contract and dispatch
/// through no replaceable builtin (a typed array's `length` is a prototype
/// accessor).
fn length_intrinsic(intrinsic: Intrinsic) -> bool {
    intrinsic == Intrinsic::StringLength || intrinsic == Intrinsic::ArrayLength
}

/// A value-fact lattice element over the unit's parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proof {
    Bottom,
    Known(ParameterSet),
    Top,
}

impl Proof {
    fn join(self, other: Self) -> Self {
        match (self, other) {
            (Self::Bottom, other) | (other, Self::Bottom) => other,
            (Self::Known(a), Self::Known(b)) => Self::Known(a.union(b)),
            _ => Self::Top,
        }
    }
    fn from(value: Option<ParameterSet>) -> Self {
        value.map_or(Self::Top, Self::Known)
    }
    fn get(self) -> Option<ParameterSet> {
        match self {
            Self::Known(bits) => Some(bits),
            // Bottom only survives for storage nothing writes: no value.
            Self::Bottom => Some(ParameterSet::EMPTY),
            Self::Top => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellFacts {
    primitive: Proof,
    int32: Proof,
    root: Option<Root>,
}

/// The unit's value facts, relative to its parameters.
struct UnitValues {
    primitive: Vec<Option<ParameterSet>>,
    int32: Vec<Option<ParameterSet>>,
    roots: Vec<Root>,
}

impl ValueFacts for UnitValues {
    fn primitive(&self, value: ValueId) -> Option<ParameterSet> {
        self.primitive.get(value.index()).copied().flatten()
    }
    fn int32(&self, value: ValueId) -> Option<ParameterSet> {
        self.int32.get(value.index()).copied().flatten()
    }
    fn root(&self, value: ValueId) -> Root {
        self.roots
            .get(value.index())
            .copied()
            .unwrap_or(Root::Unknown)
    }
}

/// Cells whose every write this unit's own operations make, so their
/// contents follow from those writes.
fn tracked_cells(
    program: &Program<'_>,
    graph: &CallGraph,
    unit: UnitId,
) -> AHashMap<CellId, CellFacts> {
    let data = program.unit(unit).unwrap();
    let mut cells = AHashMap::default();
    let mut track = |cell: CellId, initial: CellFacts| {
        let storage = graph.storage(cell);
        let entry = &program.cells[cell.index()];
        if entry.owner == unit
            && !storage.referenced
            && !(storage.shared && storage.stored)
            && !program.is_reference_parameter(cell)
        {
            cells.insert(cell, initial);
        }
    };
    for (position, &cell) in data.parameters.iter().enumerate() {
        let ty = &program.types[program.cells[cell.index()].ty.index()];
        let bits = ParameterSet::single(position);
        track(
            cell,
            CellFacts {
                primitive: if primitive_type(ty) {
                    Proof::Known(bits)
                } else {
                    Proof::Top
                },
                int32: if matches!(ty, Type::Int) {
                    Proof::Known(bits)
                } else {
                    Proof::Top
                },
                root: Some(Root::Parameter(position as u32)),
            },
        );
    }
    for operation in &data.operations {
        if let OperationKind::Initialize(cell) = operation.kind {
            if program.cells[cell.index()].binding == CellBinding::Local {
                track(
                    cell,
                    CellFacts {
                        primitive: Proof::Bottom,
                        int32: Proof::Bottom,
                        root: None,
                    },
                );
            }
        }
    }
    cells
}

fn unit_values(
    program: &Program<'_>,
    graph: &CallGraph,
    summaries: &[Fact<UnitEffects>],
    unit: UnitId,
) -> UnitValues {
    let data = program.unit(unit).unwrap();
    let mut cells = tracked_cells(program, graph, unit);
    let mut values = UnitValues {
        primitive: vec![None; data.values.len()],
        int32: vec![None; data.values.len()],
        roots: vec![Root::Unknown; data.values.len()],
    };
    let ctx = Context {
        program,
        unit,
        data,
        graph: Some(graph),
        summaries: Some(summaries),
    };
    let mut converged = false;
    for _ in 0..VALUE_ITERATIONS {
        let mut changed = false;
        for operation in &data.operations {
            if let Some(result) = operation.result {
                let (primitive, int32, root) = value_transfer(&ctx, &values, &cells, operation);
                values.primitive[result.index()] = primitive;
                values.int32[result.index()] = int32;
                values.roots[result.index()] = root;
            }
            let written = match operation.kind {
                OperationKind::Initialize(cell) => Some(cell),
                OperationKind::Store(place) => match data.places[place.index()] {
                    Place::Cell(cell) => Some(cell),
                    _ => None,
                },
                _ => None,
            };
            let Some(cell) = written else {
                // A projected store changes part of a tracked struct cell.
                if let OperationKind::Store(place) = operation.kind {
                    if let Some(cell) = place_cell(data, place) {
                        if let Some(state) = cells.get_mut(&cell) {
                            let next = CellFacts {
                                primitive: Proof::Top,
                                int32: Proof::Top,
                                root: Some(Root::Unknown),
                            };
                            changed |= *state != next;
                            *state = next;
                        }
                    }
                }
                continue;
            };
            let Some(state) = cells.get_mut(&cell) else {
                continue;
            };
            let operand = data
                .operands(operation.operands)
                .and_then(|operands| operands.first().copied());
            let next = match operand {
                Some(operand) => CellFacts {
                    primitive: state.primitive.join(Proof::from(values.primitive(operand))),
                    int32: state.int32.join(Proof::from(values.int32(operand))),
                    root: Some(match state.root {
                        Some(root) => root.join(values.root(operand)),
                        None => values.root(operand),
                    }),
                },
                None => CellFacts {
                    primitive: Proof::Top,
                    int32: Proof::Top,
                    root: Some(Root::Unknown),
                },
            };
            changed |= *state != next;
            *state = next;
        }
        if !changed {
            converged = true;
            break;
        }
    }
    if !converged {
        // Give up on storage: every load of a tracked cell is unproven.
        for state in cells.values_mut() {
            *state = CellFacts {
                primitive: Proof::Top,
                int32: Proof::Top,
                root: Some(Root::Unknown),
            };
        }
        for operation in &data.operations {
            if let Some(result) = operation.result {
                let (primitive, int32, root) = value_transfer(&ctx, &values, &cells, operation);
                values.primitive[result.index()] = primitive;
                values.int32[result.index()] = int32;
                values.roots[result.index()] = root;
            }
        }
    }
    values
}

/// The cell a place's storage is rooted in, through struct projections.
fn place_cell(data: &UnitData, place: PlaceId) -> Option<CellId> {
    let mut root = place;
    for _ in 0..=data.places.len() {
        match data.places[root.index()] {
            Place::Field { base, .. } => root = base,
            Place::Cell(cell) => return Some(cell),
            _ => return None,
        }
    }
    None
}

fn value_transfer(
    ctx: &Context<'_, '_>,
    values: &UnitValues,
    cells: &AHashMap<CellId, CellFacts>,
    operation: &Operation,
) -> (Option<ParameterSet>, Option<ParameterSet>, Root) {
    use OperationKind as Op;
    let data = ctx.data;
    let operands = data.operands(operation.operands).unwrap_or(&[]);
    let join_regions = |regions: &[RegionId], first: Option<ValueId>| {
        let mut primitive =
            first.map_or(Proof::Bottom, |value| Proof::from(values.primitive(value)));
        let mut int32 = first.map_or(Proof::Bottom, |value| Proof::from(values.int32(value)));
        let mut root: Option<Root> = first.map(|value| values.root(value));
        for region in regions {
            match data.regions[region.index()].result {
                Some(result) => {
                    primitive = primitive.join(Proof::from(values.primitive(result)));
                    int32 = int32.join(Proof::from(values.int32(result)));
                    root = Some(
                        root.map_or(values.root(result), |root| root.join(values.root(result))),
                    );
                }
                None => {
                    primitive = Proof::Top;
                    int32 = Proof::Top;
                    root = Some(Root::Unknown);
                }
            }
        }
        (
            match primitive {
                Proof::Known(bits) => Some(bits),
                _ => None,
            },
            match int32 {
                Proof::Known(bits) => Some(bits),
                _ => None,
            },
            root.unwrap_or(Root::Unknown),
        )
    };
    let none = (None, None, Root::Unknown);
    let primitive = (Some(ParameterSet::EMPTY), None, Root::Unknown);
    let integer = (
        Some(ParameterSet::EMPTY),
        Some(ParameterSet::EMPTY),
        Root::Unknown,
    );
    match &operation.kind {
        Op::Constant(Constant::Integer(_)) => integer,
        Op::Constant(_) | Op::IsUndefined | Op::TypeTest(_) | Op::Template => primitive,
        Op::IntBinary(_) | Op::Unary { integer: true, .. } => integer,
        Op::Unary { .. } => primitive,
        Op::Binary(BinaryOp::And | BinaryOp::Or | BinaryOp::Nullish) => {
            let proof = proven(values, operands);
            (proof, None, Root::Unknown)
        }
        Op::Binary(_) => primitive,
        Op::CopyValue => match operands.first() {
            Some(&value) => (
                values.primitive(value),
                values.int32(value),
                values.root(value),
            ),
            None => none,
        },
        Op::Load(place) => match data.places[place.index()] {
            Place::Value(value) => (
                values.primitive(value),
                values.int32(value),
                values.root(value),
            ),
            Place::Cell(cell) => match cells.get(&cell) {
                Some(state) => (
                    state.primitive.get(),
                    state.int32.get(),
                    state.root.unwrap_or(Root::Unknown),
                ),
                None => none,
            },
            // Data reachable from a parameter stays that parameter's.
            Place::Member { receiver, .. } | Place::Index { receiver, .. } => {
                match values.root(receiver) {
                    Root::Parameter(position) => (None, None, Root::Parameter(position)),
                    _ => none,
                }
            }
            Place::Field { .. } => match place_cell(data, *place)
                .and_then(|cell| cells.get(&cell))
                .and_then(|state| state.root)
            {
                Some(Root::Parameter(position)) => (None, None, Root::Parameter(position)),
                _ => none,
            },
        },
        Op::Allocate {
            kind:
                AllocationKind::Array
                | AllocationKind::SpreadArray(_)
                | AllocationKind::Record(_)
                | AllocationKind::Object(_),
            ..
        }
        | Op::Closure(_) => (None, None, Root::Fresh),
        Op::Allocate { .. } => none,
        // A replaceable builtin can return anything; only the language's
        // lengths are known.
        Op::Intrinsic(ResolvedIntrinsic::Property(intrinsic)) if length_intrinsic(*intrinsic) => {
            integer
        }
        Op::Intrinsic(_) => none,
        Op::Select { yes, no } => join_regions(&[*yes, *no], None),
        Op::ShortCircuit { right, .. } => join_regions(&[*right], operands.first().copied()),
        Op::Call(call) => {
            let site = &data.calls[call.index()];
            match site.target {
                CallTarget::Builtin(BuiltinCall::JsObject | BuiltinCall::JsArray) => {
                    (None, None, Root::Fresh)
                }
                CallTarget::Intrinsic { operation, .. } if !replaceable(operation) => primitive,
                CallTarget::Value {
                    callee,
                    invocation: Invocation::Value,
                } => match ctx
                    .graph
                    .map(|graph| graph.callee_of_value(ctx.program, data, callee))
                {
                    Some(Callee::Unit(body)) => {
                        let arguments = data.arguments(site.arguments).unwrap_or(&[]);
                        let result = ctx
                            .summary(body)
                            .and_then(|summary| summary.result_primitive);
                        let primitive = result.and_then(|bits| {
                            if bits.overflows() {
                                return None;
                            }
                            bits.positions()
                                .try_fold(ParameterSet::EMPTY, |acc, position| {
                                    match arguments.get(position) {
                                        Some(CallArgument::Value(value)) => {
                                            values.primitive(*value).map(|more| acc.union(more))
                                        }
                                        _ => None,
                                    }
                                })
                        });
                        (primitive, None, Root::Unknown)
                    }
                    _ => none,
                },
                _ => none,
            }
        }
        _ => none,
    }
}

/// Parent operation of each region, and whether each region lies in the
/// body of a `try` that catches.
struct Structure {
    parent: Vec<Option<OpId>>,
}

impl Structure {
    fn new(data: &UnitData) -> Self {
        let mut parent = vec![None; data.regions.len()];
        for (index, operation) in data.operations.iter().enumerate() {
            for child in operation.kind.child_regions() {
                if let Some(slot) = parent.get_mut(child.index()) {
                    *slot = OpId::from_index(index);
                }
            }
        }
        Self { parent }
    }
    /// Whether an exception thrown in `region` is caught inside the unit.
    fn caught(&self, data: &UnitData, mut region: RegionId) -> bool {
        for _ in 0..=data.regions.len() {
            let Some(owner) = self.parent[region.index()] else {
                return false;
            };
            let operation = &data.operations[owner.index()];
            if let OperationKind::Try {
                body,
                catch: Some(_),
                ..
            } = operation.kind
            {
                if body == region {
                    return true;
                }
            }
            region = operation.region;
        }
        false
    }
    /// Whether `region` is `ancestor` or nested inside it.
    fn within(&self, data: &UnitData, mut region: RegionId, ancestor: RegionId) -> bool {
        for _ in 0..=data.regions.len() {
            if region == ancestor {
                return true;
            }
            let Some(owner) = self.parent[region.index()] else {
                return false;
            };
            region = data.operations[owner.index()].region;
        }
        false
    }
}

/// A unit's summary. `access` answers whether an access of a cell is past
/// that cell's initialization (the initialization owner, at the tier the
/// caller has); `record`, when given, receives every operation's effects as
/// the summary joins them, before the projection to what callers observe.
fn summarize(
    program: &Program<'_>,
    graph: &CallGraph,
    summaries: &[Fact<UnitEffects>],
    unit: UnitId,
    declared_pure: bool,
    access: &ProgramInitialization,
    mut record: Option<&mut Vec<Effects>>,
) -> (Fact<UnitEffects>, Vec<Root>) {
    let data = program.unit(unit).unwrap();
    if data.suspension != Suspension::None {
        return (Fact::Unknown(Reason::Suspending), Vec::new());
    }
    if data.host_class.is_some() {
        return (Fact::Unknown(Reason::HostConstructor), Vec::new());
    }
    let values = unit_values(program, graph, summaries, unit);
    let ctx = Context {
        program,
        unit,
        data,
        graph: Some(graph),
        summaries: Some(summaries),
    };
    let structure = Structure::new(data);
    let mut effects = Effects::NONE;
    let mut result_primitive = Some(ParameterSet::EMPTY);
    for (index, operation) in data.operations.iter().enumerate() {
        let id = OpId::from_index(index).unwrap();
        let mut operation_effects = operation_effects(&ctx, &values, operation);
        // An access past its cell's initialization cannot observe the
        // temporal dead zone, its only failure.
        if operation_effects.may_throw {
            if let Some(cell) = access_cell(data, operation) {
                if access.initialized(program, unit, id, cell) {
                    operation_effects.may_throw = false;
                }
            }
        }
        if operation_effects.may_throw && structure.caught(data, operation.region) {
            operation_effects.may_throw = false;
        }
        match operation.kind {
            OperationKind::Loop { test, body, update } => {
                match loop_bound(&ctx, &values, &structure, id, test, body, update) {
                    Some(bits) => {
                        operation_effects.assumed_int32 =
                            operation_effects.assumed_int32.union(bits)
                    }
                    None => operation_effects.may_diverge = true,
                }
            }
            OperationKind::Return => {
                let returned = data
                    .operands(operation.operands)
                    .and_then(|operands| operands.first().copied());
                if let Some(value) = returned {
                    result_primitive = result_primitive
                        .zip(values.primitive(value))
                        .map(|(a, b)| a.union(b));
                }
            }
            _ => {}
        }
        if let Some(record) = record.as_deref_mut() {
            record.push(operation_effects);
        }
        operation_effects.transfers_control = false;
        operation_effects.cell = None;
        operation_effects.reads = operation_effects.reads.observable();
        operation_effects.writes = operation_effects.writes.observable();
        effects.join(operation_effects);
    }
    let deps = Deps {
        tables: program.tables_revision,
        units: std::iter::once(unit)
            .chain(graph.calls_from(unit).iter().map(|edge| edge.callee))
            .map(|unit| (unit, program.units[unit.index()].revision()))
            .collect(),
    };
    (
        Fact::Known(
            UnitEffects {
                effects,
                result_primitive,
                declared_pure,
            },
            deps,
        ),
        values.roots,
    )
}

/// The cell a load, store or place check accesses directly.
fn access_cell(data: &UnitData, operation: &Operation) -> Option<CellId> {
    match operation.kind {
        OperationKind::Load(place)
        | OperationKind::Store(place)
        | OperationKind::CheckPlace(place) => place_cell(data, place),
        _ => None,
    }
}

/// A counted bound for a loop (D3.6): its test compares an int counter with
/// an invariant int32 bound, the counter moves toward the bound by a constant
/// at least once per iteration and never elsewhere in the loop, and the
/// per-iteration step cannot wrap past the bound. Returns the parameters the
/// bound's int32 proof rests on.
fn loop_bound(
    ctx: &Context<'_, '_>,
    values: &UnitValues,
    structure: &Structure,
    loop_operation: OpId,
    test: RegionId,
    body: RegionId,
    update: RegionId,
) -> Option<ParameterSet> {
    let data = ctx.data;
    let program = ctx.program;
    let result = data.regions[test.index()].result?;
    let comparison = &data.operations[data.values[result.index()].definition.index()];
    if comparison.region != test {
        return None;
    }
    let OperationKind::Binary(operator) = comparison.kind else {
        return None;
    };
    let &[left, right] = data.operands(comparison.operands)? else {
        return None;
    };
    let int = |value: ValueId| matches!(ctx.ty(value), Type::Int);
    if !int(left) || !int(right) {
        return None;
    }
    let inside = |operation: OpId| {
        let region = data.operations[operation.index()].region;
        structure.within(data, region, test)
            || structure.within(data, region, body)
            || structure.within(data, region, update)
    };
    let counter_of = |value: ValueId| -> Option<CellId> {
        let definition = data.values[value.index()].definition;
        let operation = &data.operations[definition.index()];
        let OperationKind::Load(place) = operation.kind else {
            return None;
        };
        let Place::Cell(cell) = data.places[place.index()] else {
            return None;
        };
        let storage = &program.cells[cell.index()];
        let facts = ctx.graph?.storage(cell);
        (storage.owner == ctx.unit
            && storage.binding == CellBinding::Local
            && matches!(program.types[storage.ty.index()], Type::Int)
            && !facts.shared
            && !facts.referenced)
            .then_some(cell)
    };
    // Normalize to `counter <op> bound`.
    let (counter, bound, increasing, strict) = match (counter_of(left), counter_of(right)) {
        (Some(cell), _) => match operator {
            BinaryOp::Less => (cell, right, true, true),
            BinaryOp::LessEq => (cell, right, true, false),
            BinaryOp::Greater => (cell, right, false, true),
            BinaryOp::GreaterEq => (cell, right, false, false),
            _ => return None,
        },
        (None, Some(cell)) => match operator {
            BinaryOp::Greater => (cell, left, true, true),
            BinaryOp::GreaterEq => (cell, left, true, false),
            BinaryOp::Less => (cell, left, false, true),
            BinaryOp::LessEq => (cell, left, false, false),
            _ => return None,
        },
        _ => return None,
    };
    // Every write of the counter inside the loop steps it toward the bound
    // by a constant, outside any nested loop; at least one runs on every
    // iteration.
    let mut step_total: i64 = 0;
    let (mut in_update, mut in_body, mut continues) = (false, false, false);
    for (index, operation) in data.operations.iter().enumerate() {
        let id = OpId::from_index(index).unwrap();
        if id == loop_operation || !inside(id) {
            continue;
        }
        if matches!(operation.kind, OperationKind::Continue)
            && innermost_loop(data, structure, operation.region) == Some(loop_operation)
            && structure.within(data, operation.region, body)
        {
            continues = true;
        }
        let writes_counter = match operation.kind {
            OperationKind::Store(place) => place_cell(data, place) == Some(counter),
            OperationKind::Initialize(cell) => cell == counter,
            _ => false,
        };
        if !writes_counter {
            continue;
        }
        let OperationKind::Store(_) = operation.kind else {
            return None;
        };
        if innermost_loop(data, structure, operation.region) != Some(loop_operation) {
            return None;
        }
        let &[stored] = data.operands(operation.operands)? else {
            return None;
        };
        let step = counter_step(data, stored, counter, increasing)?;
        step_total = step_total.checked_add(step)?;
        if unconditional_in(data, structure, operation.region, update) {
            in_update = true;
        } else if unconditional_in(data, structure, operation.region, body) {
            in_body = true;
        }
    }
    // The update runs after every iteration, `continue` included; a step at
    // the top of the body runs on every iteration only if no `continue`
    // can skip it.
    if !(in_update || (in_body && !continues)) {
        return None;
    }
    // The bound is invariant in the loop.
    let loop_writes_objects = || {
        data.operations
            .iter()
            .enumerate()
            .any(|(index, operation)| {
                let id = OpId::from_index(index).unwrap();
                if !inside(id) {
                    return false;
                }
                let effects = operation_effects(ctx, values, operation);
                effects.writes.intersects(
                    Regions::OWN_OBJECTS
                        .union(Regions::FIELDS)
                        .union(Regions::HOST),
                ) || !effects.mutated.is_empty()
                    || effects.runs_user_code
            })
    };
    if !invariant(ctx, structure, bound, &inside, &loop_writes_objects, 0) {
        return None;
    }
    let assumptions = values.int32(bound)?;
    // The last step cannot wrap past the bound.
    let constant = match data.operations[data.values[bound.index()].definition.index()].kind {
        OperationKind::Constant(Constant::Integer(value)) => Some(i64::from(value)),
        _ => None,
    };
    let (max, min) = (i64::from(i32::MAX), i64::from(i32::MIN));
    let safe = match (increasing, strict) {
        (true, true) => {
            step_total == 1 || constant.is_some_and(|bound| bound <= max - step_total + 1)
        }
        (true, false) => constant.is_some_and(|bound| bound <= max - step_total),
        (false, true) => {
            step_total == 1 || constant.is_some_and(|bound| bound >= min + step_total - 1)
        }
        (false, false) => constant.is_some_and(|bound| bound >= min + step_total),
    };
    safe.then_some(assumptions)
}

/// How far a stored value moves the counter: `counter + k` or `k + counter`
/// with `k > 0` when increasing; `counter - k` when decreasing.
fn counter_step(
    data: &UnitData,
    stored: ValueId,
    counter: CellId,
    increasing: bool,
) -> Option<i64> {
    let operation = &data.operations[data.values[stored.index()].definition.index()];
    let OperationKind::IntBinary(operator) = operation.kind else {
        return None;
    };
    let &[left, right] = data.operands(operation.operands)? else {
        return None;
    };
    let loads_counter = |value: ValueId| {
        matches!(
            data.operations[data.values[value.index()].definition.index()].kind,
            OperationKind::Load(place) if data.places[place.index()] == Place::Cell(counter)
        )
    };
    let constant = |value: ValueId| match data.operations
        [data.values[value.index()].definition.index()]
    .kind
    {
        OperationKind::Constant(Constant::Integer(value)) if value > 0 => Some(i64::from(value)),
        _ => None,
    };
    match (operator, increasing) {
        (crate::primitive::IntBinary::Add, true) => {
            if loads_counter(left) {
                constant(right)
            } else if loads_counter(right) {
                constant(left)
            } else {
                None
            }
        }
        (crate::primitive::IntBinary::Subtract, false) if loads_counter(left) => constant(right),
        _ => None,
    }
}

/// The loop operation whose body, test or update most closely encloses
/// `region`.
fn innermost_loop(data: &UnitData, structure: &Structure, mut region: RegionId) -> Option<OpId> {
    for _ in 0..=data.regions.len() {
        let owner = structure.parent[region.index()]?;
        if matches!(
            data.operations[owner.index()].kind,
            OperationKind::Loop { .. } | OperationKind::ForIn { .. } | OperationKind::ForOf { .. }
        ) {
            return Some(owner);
        }
        region = data.operations[owner.index()].region;
    }
    None
}

/// Whether `region` is `target` or reached from it only through blocks, so
/// an operation there runs whenever `target` completes normally.
fn unconditional_in(
    data: &UnitData,
    structure: &Structure,
    mut region: RegionId,
    target: RegionId,
) -> bool {
    for _ in 0..=data.regions.len() {
        if region == target {
            return true;
        }
        let Some(owner) = structure.parent[region.index()] else {
            return false;
        };
        let operation = &data.operations[owner.index()];
        if !matches!(operation.kind, OperationKind::Block(_)) {
            return false;
        }
        region = operation.region;
    }
    false
}

/// Whether a bound value is the same on every iteration.
fn invariant(
    ctx: &Context<'_, '_>,
    structure: &Structure,
    value: ValueId,
    inside: &impl Fn(OpId) -> bool,
    loop_writes_objects: &impl Fn() -> bool,
    depth: usize,
) -> bool {
    if depth > 8 {
        return false;
    }
    let data = ctx.data;
    let definition = data.values[value.index()].definition;
    // A value computed before the loop never changes.
    if !inside(definition) {
        return true;
    }
    let operation = &data.operations[definition.index()];
    let operand = || {
        data.operands(operation.operands)
            .and_then(|operands| operands.first().copied())
    };
    match operation.kind {
        OperationKind::Constant(_) => true,
        OperationKind::CopyValue => operand().is_some_and(|value| {
            invariant(
                ctx,
                structure,
                value,
                inside,
                loop_writes_objects,
                depth + 1,
            )
        }),
        OperationKind::Load(place) => match data.places[place.index()] {
            Place::Cell(cell) => {
                let storage = &ctx.program.cells[cell.index()];
                let Some(facts) = ctx.graph.map(|graph| graph.storage(cell)) else {
                    return false;
                };
                storage.binding != CellBinding::Foreign
                    && !ctx.program.is_reference_parameter(cell)
                    && !facts.stored
                    && !facts.referenced
                    && data.operations.iter().enumerate().all(|(index, operation)| {
                        !(matches!(operation.kind, OperationKind::Initialize(initialized) if initialized == cell)
                            && inside(OpId::from_index(index).unwrap()))
                    })
            }
            Place::Value(value) => invariant(
                ctx,
                structure,
                value,
                inside,
                loop_writes_objects,
                depth + 1,
            ),
            _ => false,
        },
        OperationKind::Intrinsic(ResolvedIntrinsic::Property(intrinsic))
            if length_intrinsic(intrinsic) =>
        {
            let Some(receiver) = operand() else {
                return false;
            };
            // A string never changes; an array's length holds while the loop
            // writes no object.
            (intrinsic == Intrinsic::StringLength || !loop_writes_objects())
                && invariant(
                    ctx,
                    structure,
                    receiver,
                    inside,
                    loop_writes_objects,
                    depth + 1,
                )
        }
        _ => false,
    }
}

/// The pure contract (M6.3): each declared `pure` unit whose summary shows an
/// observable effect, with its declaration span and name.
pub(super) fn pure_contract_violations(program: &Program<'_>) -> Vec<(ModuleId, Span, String)> {
    let effects = program.effects(Seal::Module);
    effects
        .pure_violations()
        .into_iter()
        .filter_map(|unit| {
            let data = program.unit(unit)?;
            let cell = program
                .cells
                .iter()
                .find(|cell| cell.binding == CellBinding::Function(unit))?;
            let name = data
                .function_name
                .and_then(|name| program.strings.get(name.index()))
                .and_then(|name| name.as_unicode().map(str::to_string))
                .unwrap_or_else(|| cell.name.clone());
            Some((data.module, cell.declaration, name))
        })
        .collect()
}
