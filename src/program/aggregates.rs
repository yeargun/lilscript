//! Allocation identity and field observations on the shared program.
//!
//! An origin set is a may-alias answer, never a uniqueness certificate. The
//! bounded monotone solve retains every origin or conservatively closes the
//! overflowing alias component to optimization. Unrelated sites remain usable.
//! Consumers separately prove activation, initialization and complete uses.
use super::analysis_storage as storage;
use super::call_graph::{Callee, EdgeKind, Seal};
use super::effects::ProgramEffects;
use super::views::Deps;
use super::*;
use crate::compilation_policy::WorkKind;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};

const MAX_ORIGINS: usize = 8;
const MAX_ROUNDS: usize = 64;
const MAX_WORK: usize = 1 << 24;
const MAX_VALUES: usize = 1 << 19;
const MAX_FIELDS: usize = 256;

/// A bounded origin set needs no per-value heap allocation. Site ordinals
/// fit u32 because the entire solve rejects more than MAX_VALUES values.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Sites {
    values: [u32; MAX_ORIGINS],
    len: u8,
}
impl Sites {
    fn len(&self) -> usize {
        self.len as usize
    }
    pub(super) fn contains(&self, site: &usize) -> bool {
        self.iter().any(|value| value == *site)
    }
    fn iter(self) -> impl Iterator<Item = usize> {
        self.values
            .into_iter()
            .take(self.len as usize)
            .map(|site| site as usize)
    }
    fn push(&mut self, site: usize) {
        assert!(self.len() < MAX_ORIGINS, "bounded origin set");
        self.values[self.len()] = u32::try_from(site).expect("bounded allocation ordinal");
        self.len += 1;
    }
    fn sort(&mut self) {
        let len = self.len();
        self.values[..len].sort_unstable();
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Origins {
    pub sites: Sites,
    pub unknown: bool,
    saturated: bool,
}
impl Origins {
    fn unknown() -> Self {
        Self {
            sites: Sites::default(),
            unknown: true,
            saturated: false,
        }
    }
    fn join(&mut self, other: &Self, allocations: &mut [AllocationFacts]) {
        self.unknown |= other.unknown;
        if self.saturated || other.saturated {
            self.saturate(other, allocations);
            return;
        }
        for site in other.sites.iter() {
            if !self.sites.contains(&site) {
                if self.sites.len() == MAX_ORIGINS {
                    self.saturate(other, allocations);
                    return;
                }
                self.sites.push(site);
            }
        }
        self.sites.sort();
    }
    fn saturate(&mut self, other: &Self, allocations: &mut [AllocationFacts]) {
        // Dropping a may-alias origin is legal only after permanently denying
        // rewrites of that allocation. Future inputs to this component are
        // denied too, including origins discovered on a later backedge.
        for site in self.sites.iter().chain(other.sites.iter()) {
            allocations[site].escape = Escape::Host;
            allocations[site].origin_limit = true;
        }
        self.sites = Sites::default();
        self.unknown = true;
        self.saturated = true;
    }
    pub fn single(&self) -> Option<usize> {
        (!self.unknown && self.sites.len() == 1).then(|| self.sites.values[0] as usize)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Escape {
    Local,
    Typed,
    Host,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum Key {
    Named(StringId),
    Index(u32),
}
#[derive(Clone, Debug)]
pub(super) struct FieldFacts {
    pub key: Key,
    pub initial: ValueId,
    pub read: bool,
    pub written: bool,
    pub constant: Option<Constant>,
}
#[derive(Clone, Debug)]
pub(super) struct AllocationFacts {
    pub unit: UnitId,
    pub operation: OpId,
    pub value: ValueId,
    pub identity: AllocationId,
    pub fields: Vec<FieldFacts>,
    pub escape: Escape,
    pub identity_observed: bool,
    pub dynamic: bool,
    pub captured: bool,
    pub record: bool,
    pub scalar_kind: bool,
    pub origin_limit: bool,
}
impl AllocationFacts {
    pub fn field(&self, key: Key) -> Option<usize> {
        self.fields.iter().position(|field| field.key == key)
    }
    pub fn closed(&self) -> bool {
        self.escape != Escape::Host && !self.identity_observed && !self.dynamic
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Limit {
    InputSize,
    ActivationObservation,
    PropagationWork,
    PropagationRounds,
}

#[derive(Debug)]
pub(crate) struct ProgramAggregates {
    deps: Deps,
    pub(super) sites: Vec<AllocationFacts>,
    values: Vec<Vec<Origins>>,
    cells: Vec<Origins>,
    pub(super) complete: bool,
    pub(super) limit: Option<Limit>,
    pub(super) work: usize,
}
impl ProgramAggregates {
    pub fn deps(&self) -> &Deps {
        &self.deps
    }
    pub(super) fn value(&self, unit: UnitId, value: ValueId) -> &Origins {
        &self.values[unit.index()][value.index()]
    }
    pub(super) fn cell(&self, cell: CellId) -> &Origins {
        &self.cells[cell.index()]
    }
    pub(super) fn namespace(
        &self,
        program: &Program<'_>,
        effects: &ProgramEffects,
        unit: UnitId,
        call: CallId,
    ) -> Option<(usize, usize)> {
        let data = program.unit(unit)?;
        let call = &data.calls[call.index()];
        if !matches!(call.target, CallTarget::Builtin(BuiltinCall::JsInvoke)) {
            return None;
        }
        let args = data.arguments(call.arguments)?;
        let [CallArgument::Value(receiver), CallArgument::Value(key), ..] = args else {
            return None;
        };
        let Constant::String(key) = literal(data, *key)? else {
            return None;
        };
        let site = self.value(unit, *receiver).single()?;
        let allocation = &self.sites[site];
        let field = allocation.field(Key::Named(*key))?;
        let owner = program.unit(allocation.unit)?;
        let Callee::Unit(body) =
            effects
                .graph()
                .callee_of_value(program, owner, allocation.fields[field].initial)
        else {
            return None;
        };
        // Language closures have lexical receiver semantics. Method adapters,
        // host functions and unknown callables keep their original receiver.
        (program.unit(body)?.kind == UnitKind::Closure).then_some((site, field))
    }
    pub(super) fn projection(
        &self,
        program: &Program<'_>,
        unit: UnitId,
        place: PlaceId,
    ) -> Option<(usize, usize)> {
        let data = program.unit(unit)?;
        let (receiver, key) = projection(program, data, place)?;
        let site = self.value(unit, receiver).single()?;
        Some((site, self.sites[site].field(key)?))
    }
    pub(super) fn build(program: &Program<'_>, effects: &ProgramEffects) -> Self {
        Self::build_in(program, effects, &mut AllocationBudget::new(None))
            .expect("inspection aggregates")
    }
    pub(super) fn build_in(
        program: &Program<'_>,
        effects: &ProgramEffects,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, AllocationError> {
        budget.retained_phase(|budget| {
            if program
                .units
                .iter()
                .fold(0usize, |total, u| {
                    total.saturating_add(u.data().values.len())
                })
                .saturating_add(program.cells.len())
                > MAX_VALUES
            {
                return Ok(Self {
                    deps: Deps::of_program_in(program, budget)?,
                    sites: Vec::new(),
                    values: Vec::new(),
                    cells: Vec::new(),
                    complete: false,
                    limit: Some(Limit::InputSize),
                    work: 0,
                });
            }
            let mut values = budget.vector(Retained, program.units.len())?;
            for unit in &program.units {
                values.push(budget.filled(
                    Retained,
                    unit.data().values.len(),
                    Origins::default(),
                )?);
            }
            let mut result = Self {
                deps: Deps::of_program_in(program, budget)?,
                sites: Vec::new(),
                values,
                cells: budget.filled(Retained, program.cells.len(), Origins::default())?,
                complete: false,
                limit: Some(Limit::PropagationRounds),
                work: 0,
            };
            let graph = effects.graph();
            let free = super::defaults::arguments_free_all_in(program, budget)?;
            let blocked = free.iter().any(|free| !free);
            storage::release(free, Retained, budget)?;
            if blocked {
                result.limit = Some(Limit::ActivationObservation);
                return Ok(result);
            }
            for unit in &program.units {
                let data = unit.data();
                for (index, operation) in data.operations.iter().enumerate() {
                    budget.work(WorkKind::Analysis, 1)?;
                    let OperationKind::Allocate { identity, kind } = &operation.kind else {
                        continue;
                    };
                    let Some(value) = operation.result else {
                        continue;
                    };
                    let args = data.operands(operation.operands).unwrap_or(&[]);
                    let (keys, count, record, scalar_kind, dynamic): (
                        Option<&[StringId]>,
                        _,
                        _,
                        _,
                        _,
                    ) = match kind {
                        AllocationKind::Array => (None, args.len(), false, true, false),
                        AllocationKind::Record(keys) => {
                            (Some(keys), keys.len(), true, false, false)
                        }
                        AllocationKind::Object(keys) => {
                            (Some(keys), keys.len(), false, true, false)
                        }
                        AllocationKind::Instance { class, keys } => {
                            let blocked = program.class(*class).is_none_or(|c| {
                                c.external
                                    || c.observed
                                    || c.reflected
                                    || c.published
                                    || !c.type_params.is_empty()
                            });
                            (Some(keys), keys.len(), false, !blocked, blocked)
                        }
                        _ => (None, 0, false, false, true),
                    };
                    let mut fields =
                        budget.vector(Retained, count.min(args.len()).min(MAX_FIELDS))?;
                    let mut invalid = dynamic || count != args.len() || count > MAX_FIELDS;
                    for (position, &initial) in args.iter().take(count.min(MAX_FIELDS)).enumerate()
                    {
                        budget.work(WorkKind::Analysis, fields.len() as u64 + 1)?;
                        let key = keys.map_or(Key::Index(position as u32), |keys| {
                            Key::Named(keys[position])
                        });
                        invalid |= fields.iter().any(|f: &FieldFacts| f.key == key);
                        if let Key::Named(key) = key {
                            invalid |= program.strings[key.index()].as_unicode()
                                == Some("__proto__")
                                && !record;
                        }
                        let constant = literal(data, initial).cloned();
                        fields.push(FieldFacts {
                            key,
                            initial,
                            read: false,
                            written: false,
                            constant,
                        });
                    }
                    let site = result.sites.len();
                    result.values[unit.id().index()][value.index()]
                        .sites
                        .push(site);
                    budget.push(
                        Retained,
                        &mut result.sites,
                        AllocationFacts {
                            unit: unit.id(),
                            operation: OpId::from_index(index).unwrap(),
                            value,
                            identity: *identity,
                            fields,
                            escape: Escape::Local,
                            identity_observed: false,
                            dynamic: invalid,
                            captured: false,
                            record,
                            scalar_kind,
                            origin_limit: false,
                        },
                    )?;
                }
            }
            let mut returns = budget.filled(Scratch, program.units.len(), Origins::unknown())?;
            for (index, cell) in program.cells.iter().enumerate() {
                budget.work(WorkKind::Analysis, 1)?;
                let id = CellId::from_index(index).unwrap();
                let external =
                    !matches!(cell.binding, CellBinding::Local | CellBinding::Parameter(_))
                        || graph.storage(id).referenced
                        || program.is_reference_parameter(id)
                        || (graph.seal() != Seal::Module
                            && program.unit(cell.owner).unwrap().kind
                                == UnitKind::ModuleInitialization);
                result.cells[index].unknown = external;
            }
            let mut settled = false;
            'rounds: for _ in 0..MAX_ROUNDS {
                let mut changed = false;
                for unit in &program.units {
                    let id = unit.id();
                    let data = unit.data();
                    for (position, &cell) in data.parameters.iter().enumerate() {
                        let mut next = Origins::default();
                        match graph.complete_callers(id) {
                            Some(edges) if !edges.is_empty() => {
                                for edge in edges {
                                    budget.work(WorkKind::Analysis, MAX_ORIGINS as u64)?;
                                    let caller = program.unit(edge.caller).unwrap();
                                    let site = &caller.calls[edge.call.index()];
                                    let origin = match caller
                                        .arguments(site.arguments)
                                        .and_then(|args| args.get(position))
                                    {
                                        Some(CallArgument::Value(value))
                                            if edge.kind == EdgeKind::Call
                                                && site.contract.instantiation.is_none() =>
                                        {
                                            result.value(edge.caller, *value).clone()
                                        }
                                        _ => Origins::unknown(),
                                    };
                                    next.join(&origin, &mut result.sites);
                                }
                            }
                            _ => next.unknown = true,
                        }
                        let old = result.cells[cell.index()].clone();
                        result.cells[cell.index()].join(&next, &mut result.sites);
                        changed |= old != result.cells[cell.index()];
                    }
                    for operation in &data.operations {
                        budget.work(WorkKind::Analysis, MAX_ORIGINS as u64)?;
                        result.work += 1;
                        if result.work > MAX_WORK {
                            result.limit = Some(Limit::PropagationWork);
                            break 'rounds;
                        }
                        let args = data.operands(operation.operands).unwrap_or(&[]);
                        if let OperationKind::Declare(cell) = operation.kind {
                            changed |= !result.cells[cell.index()].unknown;
                            result.cells[cell.index()].unknown = true;
                        }
                        if let Some(&input) = args.first() {
                            let cell = match operation.kind {
                                OperationKind::Initialize(cell) => Some(cell),
                                OperationKind::Store(place) => match data.places[place.index()] {
                                    Place::Cell(cell) => Some(cell),
                                    _ => None,
                                },
                                _ => None,
                            };
                            if let Some(cell) = cell {
                                let next = result.value(id, input).clone();
                                let old = result.cells[cell.index()].clone();
                                result.cells[cell.index()].join(&next, &mut result.sites);
                                changed |= old != result.cells[cell.index()];
                            }
                            if matches!(operation.kind, OperationKind::Return) {
                                let next = result.value(id, input).clone();
                                let old = returns[id.index()].clone();
                                returns[id.index()].join(&next, &mut result.sites);
                                changed |= old != returns[id.index()];
                            }
                        }
                        let Some(value) = operation.result else {
                            continue;
                        };
                        let next = match operation.kind {
                            OperationKind::Allocate { .. } => continue,
                            OperationKind::Constant(_) | OperationKind::Closure(_) => {
                                Origins::unknown()
                            }
                            OperationKind::CopyValue => args
                                .first()
                                .map_or_else(Origins::unknown, |v| result.value(id, *v).clone()),
                            OperationKind::Load(place) => match data.places[place.index()] {
                                Place::Cell(cell) => result.cells[cell.index()].clone(),
                                Place::Value(value) => result.value(id, value).clone(),
                                _ => Origins::unknown(),
                            },
                            OperationKind::Call(call) => match graph.callee(id, call) {
                                Callee::Unit(callee) => returns[callee.index()].clone(),
                                _ => Origins::unknown(),
                            },
                            OperationKind::Select { yes, no } => {
                                let mut next = Origins::default();
                                for region in [yes, no] {
                                    let origin = data.regions[region.index()]
                                        .result
                                        .map_or_else(Origins::unknown, |v| {
                                            result.value(id, v).clone()
                                        });
                                    next.join(&origin, &mut result.sites);
                                }
                                next
                            }
                            OperationKind::ShortCircuit { right, .. } => {
                                let mut next = args.first().map_or_else(Origins::unknown, |v| {
                                    result.value(id, *v).clone()
                                });
                                let origin = data.regions[right.index()]
                                    .result
                                    .map_or_else(Origins::unknown, |v| result.value(id, v).clone());
                                next.join(&origin, &mut result.sites);
                                next
                            }
                            _ => Origins::unknown(),
                        };
                        let slot = &mut result.values[id.index()][value.index()];
                        let old = slot.clone();
                        slot.join(&next, &mut result.sites);
                        changed |= *slot != old;
                    }
                }
                if !changed {
                    settled = true;
                    break;
                }
            }
            if !settled {
                return Ok(result);
            }
            result.complete = true;
            result.limit = None;
            result.observe(program, effects, budget)?;
            Ok(result)
        })
    }

    fn escape(&mut self, origins: &Origins, escape: Escape) {
        for site in origins.sites.iter() {
            self.sites[site].escape = self.sites[site].escape.max(escape);
        }
    }
    fn observe(
        &mut self,
        program: &Program<'_>,
        effects: &ProgramEffects,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<(), AllocationError> {
        let graph = effects.graph();
        for export in program.exports.iter() {
            budget.work(WorkKind::Analysis, MAX_ORIGINS as u64)?;
            if let InterfaceTarget::Value(cell) = export.target {
                self.escape(&self.cell(cell).clone(), Escape::Host);
            }
        }
        for module in program.modules.iter() {
            for &(_, cell) in &module.namespace {
                budget.work(WorkKind::Analysis, MAX_ORIGINS as u64)?;
                self.escape(&self.cell(cell).clone(), Escape::Host);
            }
        }
        for (index, cell) in program.cells.iter().enumerate() {
            budget.work(WorkKind::Analysis, MAX_ORIGINS as u64)?;
            let origins = self.cells[index].clone();
            if self.cells[index].unknown {
                self.escape(&origins, Escape::Host);
            }
            if graph.storage(CellId::from_index(index).unwrap()).shared {
                for site in origins.sites.iter() {
                    self.sites[site].captured = true;
                }
            }
            if cell.binding == CellBinding::Foreign {
                self.escape(&origins, Escape::Host);
            }
        }
        for unit in &program.units {
            let id = unit.id();
            let data = unit.data();
            for operation in &data.operations {
                let arguments = match operation.kind {
                    OperationKind::Call(call) => data
                        .arguments(data.calls[call.index()].arguments)
                        .map_or(0, |args| args.len()),
                    _ => 0,
                };
                let operands = data
                    .operands(operation.operands)
                    .map_or(0, |args| args.len());
                budget.work(
                    WorkKind::Analysis,
                    (arguments as u64 + operands as u64 + 1) * MAX_ORIGINS as u64,
                )?;
                let args = data.operands(operation.operands).unwrap_or(&[]);
                let mut transport = false;
                match operation.kind {
                    OperationKind::Initialize(_)
                    | OperationKind::CopyValue
                    | OperationKind::Select { .. }
                    | OperationKind::ShortCircuit { .. } => transport = true,
                    OperationKind::Store(place)
                        if matches!(data.places[place.index()], Place::Cell(_)) =>
                    {
                        transport = true
                    }
                    OperationKind::Return => {
                        let level = if graph.complete_callers(id).is_some() {
                            Escape::Typed
                        } else {
                            Escape::Host
                        };
                        for &value in args {
                            self.escape(&self.value(id, value).clone(), level);
                        }
                        transport = true;
                    }
                    OperationKind::Call(call) => {
                        let site = &data.calls[call.index()];
                        let namespace = self.namespace(program, effects, id, call);
                        if let Some((allocation, field)) = namespace {
                            self.sites[allocation].fields[field].read = true;
                        }
                        let level = match graph.callee(id, call) {
                            Callee::Unit(_) => Escape::Typed,
                            _ => Escape::Host,
                        };
                        for (position, argument) in data
                            .arguments(site.arguments)
                            .unwrap_or(&[])
                            .iter()
                            .enumerate()
                        {
                            if namespace.is_some() && position == 0 {
                                continue;
                            }
                            if let CallArgument::Value(value) | CallArgument::Spread(value) =
                                *argument
                            {
                                self.escape(&self.value(id, value).clone(), level);
                            }
                        }
                        match site.target {
                            CallTarget::Intrinsic {
                                receiver: Some(value),
                                ..
                            } => {
                                self.escape(&self.value(id, value).clone(), Escape::Host);
                            }
                            CallTarget::Reference { place } => {
                                self.observe_place(program, id, place, None, true)
                            }
                            CallTarget::Value { callee, .. } => {
                                self.escape(&self.value(id, callee).clone(), Escape::Host)
                            }
                            _ => {}
                        }
                    }
                    OperationKind::Load(place) => {
                        self.observe_place(program, id, place, None, false)
                    }
                    OperationKind::CheckPlace(_) => {}
                    OperationKind::Store(place) => {
                        self.observe_place(program, id, place, args.first().copied(), false)
                    }
                    OperationKind::Binary(BinaryOp::Eq | BinaryOp::NotEq) => {
                        let null = args.iter().any(|&v| {
                            matches!(literal(data, v), Some(Constant::Null | Constant::Undefined))
                        });
                        for &value in args {
                            for site in self.value(id, value).sites.iter() {
                                self.sites[site].identity_observed |= !null;
                            }
                        }
                        transport = true;
                    }
                    OperationKind::Intrinsic(crate::primitive::ResolvedIntrinsic::Property(
                        crate::primitive::Intrinsic::ArrayLength,
                    )) => {
                        transport = true;
                    }
                    _ => {}
                }
                if !transport {
                    for &value in args {
                        self.escape(&self.value(id, value).clone(), Escape::Host);
                    }
                }
            }
        }
        Ok(())
    }
    fn observe_place(
        &mut self,
        program: &Program<'_>,
        unit: UnitId,
        place: PlaceId,
        stored: Option<ValueId>,
        call: bool,
    ) {
        let data = program.unit(unit).unwrap();
        let receiver = match data.places[place.index()] {
            Place::Member { receiver, .. }
            | Place::ClassField { receiver, .. }
            | Place::Index { receiver, .. } => receiver,
            _ => return,
        };
        let origins = self.value(unit, receiver).clone();
        let key = projection(program, data, place).map(|(_, key)| key);
        for site in origins.sites.iter() {
            let allocation = &mut self.sites[site];
            let Some(field) = key.and_then(|key| allocation.field(key)) else {
                allocation.dynamic = true;
                continue;
            };
            let field = &mut allocation.fields[field];
            if let Some(stored) = stored {
                field.written = true;
                if field.constant.as_ref() != literal(data, stored) {
                    field.constant = None;
                }
            } else {
                field.read = true;
            }
            allocation.identity_observed |= call;
        }
    }
}

pub(super) fn literal(data: &UnitData, value: ValueId) -> Option<&Constant> {
    match &data.operations[data.values[value.index()].definition.index()].kind {
        OperationKind::Constant(value) => Some(value),
        _ => None,
    }
}
pub(super) fn projection(
    program: &Program<'_>,
    data: &UnitData,
    place: PlaceId,
) -> Option<(ValueId, Key)> {
    Some(match data.places[place.index()] {
        Place::Member { receiver, key } => (receiver, Key::Named(key)),
        Place::ClassField { receiver, field } => {
            (receiver, Key::Named(program.class_field(field)?.0))
        }
        Place::Index { receiver, key } => (
            receiver,
            match literal(data, key)? {
                Constant::String(key) => Key::Named(*key),
                Constant::Integer(index) if *index >= 0 => Key::Index(*index as u32),
                _ => return None,
            },
        ),
        _ => return None,
    })
}
