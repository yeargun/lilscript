//! Complete explicit callable uses, separate from representation and body effects.
//! Module execution seals the hidden-frame boundary. Script producer evidence
//! remains conditional until its consumer establishes a separate frame proof.
//! All owned arrays use the caller's existing admission scope and must be
//! discarded through that scope; there is no cache, evaluator or semantic copy.
use super::call_graph::Seal;
use super::raw_domains::Admission;
use super::record_family::OpRef;
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_contract::JavaScriptExecution;

/// The call graph's sealing of root storage, applied to one producer.
#[derive(Debug, Clone, Copy)]
pub(super) struct CallObservations {
    seal: Seal,
}
impl CallObservations {
    pub(super) fn from_execution(execution: JavaScriptExecution) -> Self {
        Self {
            seal: Seal::from_execution(execution),
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum InputScope {
    Producer(OpRef),
    Body(UnitId),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallableProducer {
    pub creation: OpRef,
    pub cell: CellId,
    pub initialize: OpRef,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct CallInput {
    pub producer: OpRef,
    pub caller: UnitId,
    pub load: OpId,
    pub prepare: OpId,
    pub call: OpId,
    pub target: CallId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum UnknownReason {
    ExecutionBoundary,
    NoCreators,
    NoCalls,
    NotClosure,
    NotPrivateCallable,
    Initialization,
    CallableObservation,
    UnsupportedCall,
    Signature,
}
#[derive(Debug)]
pub(super) enum InputOutcome {
    Complete(CallableInputs),
    Unknown(UnknownReason),
}
#[derive(Debug)]
pub(super) struct Dependencies {
    tables: RevisionId,
    units: Vec<(UnitId, RevisionId)>,
    cells: Vec<(CellId, RevisionId)>,
    creators: Vec<(UnitId, RevisionId)>,
}
impl Dependencies {
    pub(super) fn units(&self) -> &[(UnitId, RevisionId)] {
        &self.units
    }
    pub(super) fn cells(&self) -> &[(CellId, RevisionId)] {
        &self.cells
    }
    pub(super) fn creators(&self) -> &[(UnitId, RevisionId)] {
        &self.creators
    }
    /// Independent callers validate coherence once. Published clients already
    /// own the coherent Program/UseIndex pair and use the stamp-only variant.
    pub(super) fn valid_for<A: Admission>(
        &self,
        program: &Program<'_>,
        uses: &UseIndex,
        admission: &mut A,
    ) -> Result<bool, A::Error> {
        admission.work(
            program
                .units
                .len()
                .checked_add(1)
                .ok_or_else(|| admission.invalid("callable input validation capacity"))?,
        )?;
        if !uses.valid_for(program) {
            return Ok(false);
        }
        self.valid_for_published(program, uses, admission)
    }
    pub(super) fn valid_for_published<A: Admission>(
        &self,
        program: &Program<'_>,
        uses: &UseIndex,
        admission: &mut A,
    ) -> Result<bool, A::Error> {
        admission.work(1)?;
        if self.tables != program.tables_revision
            || uses.tables_revision() != program.tables_revision
        {
            return Ok(false);
        }
        for &(unit, revision) in &self.units {
            admission.work(1)?;
            if !program
                .units
                .get(unit.index())
                .is_some_and(|unit| unit.revision() == revision)
                || !uses
                    .unit(unit)
                    .is_some_and(|unit| unit.revision() == revision)
            {
                return Ok(false);
            }
        }
        for &(cell, revision) in &self.cells {
            admission.work(1)?;
            if !uses
                .cell(cell)
                .is_some_and(|users| users.revision() == revision)
            {
                return Ok(false);
            }
        }
        for &(body, revision) in &self.creators {
            admission.work(1)?;
            if !uses
                .creators(body)
                .is_some_and(|users| users.revision() == revision)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
#[derive(Debug)]
#[must_use = "discard callable input evidence through its original admission scope"]
pub(super) struct CallableInputs {
    scope: InputScope,
    body: UnitId,
    seal: Seal,
    producers: Vec<CallableProducer>,
    handles: Vec<OpRef>,
    calls: Vec<CallInput>,
    dependencies: Dependencies,
}
impl CallableInputs {
    pub(super) fn scope(&self) -> InputScope {
        self.scope
    }
    pub(super) fn body(&self) -> UnitId {
        self.body
    }
    pub(super) fn runtime_inputs_sealed(&self) -> bool {
        self.seal == Seal::Module
    }
    pub(super) fn producers(&self) -> &[CallableProducer] {
        &self.producers
    }
    pub(super) fn handle_loads(&self) -> &[OpRef] {
        &self.handles
    }
    pub(super) fn calls(&self) -> &[CallInput] {
        &self.calls
    }
    pub(super) fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }

    pub(super) fn for_cell<A: Admission>(
        program: &Program<'_>,
        uses: &UseIndex,
        cell: CellId,
        observations: CallObservations,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        coherent(program, uses, admission)?;
        Self::for_cell_published(program, uses, cell, observations, admission)
    }
    pub(super) fn for_cell_published<A: Admission>(
        program: &Program<'_>,
        uses: &UseIndex,
        cell: CellId,
        observations: CallObservations,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        tables(program, uses, admission)?;
        let producer = match locate(program, uses, cell, admission) {
            Ok(producer) => producer,
            Err(Stop::Unknown(reason)) => return Ok(InputOutcome::Unknown(reason)),
            Err(Stop::Error(error)) => return Err(error),
        };
        Self::for_producer_published(program, uses, producer, observations, admission)
    }
    pub(super) fn for_producer<A: Admission>(
        program: &Program<'_>,
        uses: &UseIndex,
        producer: OpRef,
        observations: CallObservations,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        coherent(program, uses, admission)?;
        Self::for_producer_published(program, uses, producer, observations, admission)
    }
    pub(super) fn for_producer_published<A: Admission>(
        program: &Program<'_>,
        uses: &UseIndex,
        producer: OpRef,
        observations: CallObservations,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        tables(program, uses, admission)?;
        let data = checked_unit(program, uses, producer.unit, admission)?;
        let op = data
            .operations
            .get(producer.operation.index())
            .ok_or_else(|| admission.invalid("callable producer operation"))?;
        let OperationKind::Closure(body) = op.kind else {
            return Ok(InputOutcome::Unknown(UnknownReason::NotClosure));
        };
        let mut proof = Self::empty(program, InputScope::Producer(producer), body, observations);
        let result = proof.collect_producer(program, uses, producer, admission);
        proof.finish(result, admission)
    }
    pub(super) fn for_body<A: Admission>(
        program: &Program<'_>,
        uses: &UseIndex,
        body: UnitId,
        observations: CallObservations,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        coherent(program, uses, admission)?;
        Self::for_body_published(program, uses, body, observations, admission)
    }
    pub(super) fn for_body_published<A: Admission>(
        program: &Program<'_>,
        uses: &UseIndex,
        body: UnitId,
        observations: CallObservations,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        tables(program, uses, admission)?;
        if observations.seal != Seal::Module {
            return Ok(InputOutcome::Unknown(UnknownReason::ExecutionBoundary));
        }
        checked_unit(program, uses, body, admission)?;
        let creators = uses
            .creators(body)
            .ok_or_else(|| admission.invalid("callable creator set"))?;
        if creators.sites().is_empty() {
            return Ok(InputOutcome::Unknown(UnknownReason::NoCreators));
        }
        let mut proof = Self::empty(program, InputScope::Body(body), body, observations);
        let result = (|| {
            proof.creator_stamp(uses, admission)?;
            for site in creators.sites() {
                admission.work(1)?;
                proof.collect_producer(
                    program,
                    uses,
                    OpRef {
                        unit: site.unit,
                        operation: site.operation,
                    },
                    admission,
                )?;
            }
            Ok(())
        })();
        proof.finish(result, admission)
    }
    fn empty(
        program: &Program<'_>,
        scope: InputScope,
        body: UnitId,
        observations: CallObservations,
    ) -> Self {
        Self {
            scope,
            body,
            seal: observations.seal,
            producers: Vec::new(),
            handles: Vec::new(),
            calls: Vec::new(),
            dependencies: Dependencies {
                tables: program.tables_revision,
                units: Vec::new(),
                cells: Vec::new(),
                creators: Vec::new(),
            },
        }
    }
    fn finish<A: Admission>(
        self,
        result: Result<(), Stop<A::Error>>,
        admission: &mut A,
    ) -> Result<InputOutcome, A::Error> {
        match result {
            Ok(()) if !self.calls.is_empty() => Ok(InputOutcome::Complete(self)),
            Ok(()) => {
                self.discard(admission)?;
                Ok(InputOutcome::Unknown(UnknownReason::NoCalls))
            }
            Err(Stop::Unknown(reason)) => {
                self.discard(admission)?;
                Ok(InputOutcome::Unknown(reason))
            }
            Err(Stop::Error(error)) => {
                let _ = self.discard(admission);
                Err(error)
            }
        }
    }
    pub(super) fn discard<A: Admission>(self, admission: &mut A) -> Result<(), A::Error> {
        let a = admission.release(self.producers);
        let b = admission.release(self.handles);
        let c = admission.release(self.calls);
        let d = admission.release(self.dependencies.units);
        let e = admission.release(self.dependencies.cells);
        let f = admission.release(self.dependencies.creators);
        a.and(b).and(c).and(d).and(e).and(f)
    }
    fn unit<A: Admission>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        unit: UnitId,
        admission: &mut A,
    ) -> Result<(), A::Error> {
        for &(known, _) in &self.dependencies.units {
            admission.work(1)?;
            if known == unit {
                return Ok(());
            }
        }
        checked_unit(program, uses, unit, admission)?;
        admission.push(
            &mut self.dependencies.units,
            (unit, program.units[unit.index()].revision()),
        )
    }
    fn cell<A: Admission>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        cell: CellId,
        admission: &mut A,
    ) -> Result<(), A::Error> {
        for &(known, _) in &self.dependencies.cells {
            admission.work(1)?;
            if known == cell {
                return Ok(());
            }
        }
        let data = program
            .cells
            .get(cell.index())
            .ok_or_else(|| admission.invalid("callable storage cell"))?;
        self.unit(program, uses, data.owner, admission)?;
        let users = uses
            .cell(cell)
            .ok_or_else(|| admission.invalid("callable cell index"))?;
        admission.push(&mut self.dependencies.cells, (cell, users.revision()))
    }
    fn creator_stamp<A: Admission>(
        &mut self,
        uses: &UseIndex,
        admission: &mut A,
    ) -> Result<(), A::Error> {
        if self.dependencies.creators.is_empty() {
            let users = uses
                .creators(self.body)
                .ok_or_else(|| admission.invalid("callable creator index"))?;
            admission.push(
                &mut self.dependencies.creators,
                (self.body, users.revision()),
            )?;
        }
        Ok(())
    }
    fn collect_producer<A: Admission>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        creation: OpRef,
        admission: &mut A,
    ) -> Result<(), Stop<A::Error>> {
        self.unit(program, uses, creation.unit, admission)?;
        self.unit(program, uses, self.body, admission)?;
        let owner = program.unit(creation.unit).unwrap();
        let op = owner
            .operations
            .get(creation.operation.index())
            .ok_or_else(|| admission.invalid("callable closure operation"))?;
        if !matches!(op.kind,OperationKind::Closure(body) if body==self.body) {
            return Err(Stop::Unknown(UnknownReason::NotClosure));
        }
        let value = op
            .result
            .ok_or_else(|| admission.invalid("callable closure result"))?;
        let value_uses = uses
            .unit(creation.unit)
            .unwrap()
            .value_uses(value)
            .ok_or_else(|| admission.invalid("callable closure uses"))?;
        admission.work(value_uses.len())?;
        let [ValueUse::Operand {
            operation: initialize,
            position: 0,
        }] = value_uses
        else {
            return Err(Stop::Unknown(UnknownReason::CallableObservation));
        };
        let initialize = *initialize;
        let init = &owner.operations[initialize.index()];
        let OperationKind::Initialize(cell) = init.kind else {
            return Err(Stop::Unknown(UnknownReason::Initialization));
        };
        let storage = program
            .cells
            .get(cell.index())
            .ok_or_else(|| admission.invalid("callable initialized cell"))?;
        if storage.owner != creation.unit
            || storage.binding == CellBinding::Foreign
            || !matches!(
                program.types[storage.ty.index()],
                Type::Function(_) | Type::GenericFunction(_)
            )
        {
            return Err(Stop::Unknown(UnknownReason::NotPrivateCallable));
        }
        if owner.operands(init.operands) != Some(&[value])
            || init.region != storage.region
            || op.region != storage.region
            || matches!(storage.binding,CellBinding::Function(body) if body!=self.body)
        {
            return Err(Stop::Unknown(UnknownReason::Initialization));
        }
        let body = program.unit(self.body).unwrap();
        if body.callable_type != Some(storage.ty) {
            return Err(Stop::Unknown(UnknownReason::Signature));
        }
        self.cell(program, uses, cell, admission)?;
        let producer = CallableProducer {
            creation,
            cell,
            initialize: OpRef {
                unit: creation.unit,
                operation: initialize,
            },
        };
        admission.push(&mut self.producers, producer)?;
        let users = uses.cell(cell).unwrap();
        let mut found_init = false;
        for site in users.sites() {
            admission.work(1)?;
            let CellUseSite::Unit { unit, usage } = *site else {
                return Err(Stop::Unknown(UnknownReason::CallableObservation));
            };
            self.unit(program, uses, unit, admission)?;
            match usage {
                CellUse::Initialize(operation)
                    if unit == creation.unit && operation == initialize && !found_init =>
                {
                    found_init = true;
                }
                CellUse::Capture => {}
                CellUse::Read { operation, place } => {
                    let caller = program.unit(unit).unwrap();
                    let load = &caller.operations[operation.index()];
                    if caller.places[place.index()] != Place::Cell(cell)
                        || !matches!(load.kind,OperationKind::Load(id) if id==place)
                    {
                        return Err(Stop::Unknown(UnknownReason::UnsupportedCall));
                    }
                    let value = load
                        .result
                        .ok_or_else(|| admission.invalid("callable load result"))?;
                    admission.push(&mut self.handles, OpRef { unit, operation })?;
                    for usage in uses
                        .unit(unit)
                        .unwrap()
                        .value_uses(value)
                        .ok_or_else(|| admission.invalid("callable load uses"))?
                    {
                        admission.work(1)?;
                        let ValueUse::CallCallee {
                            prepare,
                            call: target,
                        } = *usage
                        else {
                            return Err(Stop::Unknown(UnknownReason::CallableObservation));
                        };
                        let site = &caller.calls[target.index()];
                        if !matches!(site.target,CallTarget::Value{callee,invocation:Invocation::Value} if callee==value)
                            || !matches!(caller.operations[prepare.index()].kind,OperationKind::PrepareCall(id) if id==target)
                        {
                            return Err(Stop::Unknown(UnknownReason::UnsupportedCall));
                        }
                        let call = uses
                            .unit(unit)
                            .unwrap()
                            .call_operation(target)
                            .ok_or_else(|| admission.invalid("callable invocation index"))?;
                        let invocation = &caller.operations[call.index()];
                        if !matches!(invocation.kind,OperationKind::Call(id) if id==target) {
                            return Err(Stop::Unknown(UnknownReason::UnsupportedCall));
                        }
                        if site.contract.signature != Some(storage.ty)
                            || site.contract.supplied as usize != body.parameters.len()
                            || site.arguments.len as usize != body.parameters.len()
                            || invocation.operands.len != 0
                        {
                            return Err(Stop::Unknown(UnknownReason::Signature));
                        }
                        admission.push(
                            &mut self.calls,
                            CallInput {
                                producer: creation,
                                caller: unit,
                                load: operation,
                                prepare,
                                call,
                                target,
                            },
                        )?;
                    }
                }
                CellUse::Write { .. } => {
                    return Err(Stop::Unknown(UnknownReason::NotPrivateCallable));
                }
                _ => return Err(Stop::Unknown(UnknownReason::CallableObservation)),
            }
        }
        if !found_init {
            return Err(Stop::Unknown(UnknownReason::Initialization));
        }
        Ok(())
    }
}
#[derive(Debug)]
enum Stop<E> {
    Unknown(UnknownReason),
    Error(E),
}
impl<E> From<E> for Stop<E> {
    fn from(error: E) -> Self {
        Self::Error(error)
    }
}
fn tables<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    admission: &mut A,
) -> Result<(), A::Error> {
    admission.work(1)?;
    if uses.tables_revision() != program.tables_revision {
        return Err(admission.invalid("callable input stale tables"));
    }
    Ok(())
}
fn coherent<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    admission: &mut A,
) -> Result<(), A::Error> {
    admission.work(
        program
            .units
            .len()
            .checked_add(1)
            .ok_or_else(|| admission.invalid("callable input coherence capacity"))?,
    )?;
    if !uses.valid_for(program) {
        return Err(admission.invalid("callable input stale index"));
    }
    Ok(())
}
fn checked_unit<'a, 'src, A: Admission>(
    program: &'a Program<'src>,
    uses: &UseIndex,
    unit: UnitId,
    admission: &mut A,
) -> Result<&'a UnitData, A::Error> {
    admission.work(1)?;
    let frozen = program
        .units
        .get(unit.index())
        .filter(|unit_data| unit_data.id() == unit)
        .ok_or_else(|| admission.invalid("callable input unit"))?;
    if !uses
        .unit(unit)
        .is_some_and(|data| data.revision() == frozen.revision())
    {
        return Err(admission.invalid("callable input stale unit index"));
    }
    Ok(frozen.data())
}
fn locate<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    cell: CellId,
    admission: &mut A,
) -> Result<OpRef, Stop<A::Error>> {
    let storage = program
        .cells
        .get(cell.index())
        .ok_or_else(|| admission.invalid("callable locator cell"))?;
    if storage.binding == CellBinding::Foreign {
        return Err(Stop::Unknown(UnknownReason::NotPrivateCallable));
    }
    let owner = checked_unit(program, uses, storage.owner, admission)?;
    // UnitUses already owns the sorted (CellId, CellUse) occurrences. An
    // Initialize is the first kind in a cell's owner-unit range. Borrow that
    // existing ordering instead of rescanning every call/capture of the cell.
    // The subsequent complete producer proof rejects any non-owner Initialize.
    let references = uses.unit(storage.owner).unwrap().cell_uses();
    admission.work((usize::BITS - references.len().leading_zeros()) as usize + 2)?;
    let start = references.partition_point(|reference| reference.cell < cell);
    let operation = match references.get(start) {
        Some(reference) if reference.cell == cell => match reference.usage {
            CellUse::Initialize(operation) => operation,
            _ => return Err(Stop::Unknown(UnknownReason::Initialization)),
        },
        _ => return Err(Stop::Unknown(UnknownReason::Initialization)),
    };
    if references.get(start + 1).is_some_and(|reference| {
        reference.cell == cell && matches!(reference.usage, CellUse::Initialize(_))
    }) {
        return Err(Stop::Unknown(UnknownReason::Initialization));
    }
    let values = owner
        .operands(owner.operations[operation.index()].operands)
        .ok_or_else(|| admission.invalid("callable locator initializer"))?;
    let &[value] = values else {
        return Err(Stop::Unknown(UnknownReason::Initialization));
    };
    let definition = owner.values[value.index()].definition;
    if !matches!(
        owner.operations[definition.index()].kind,
        OperationKind::Closure(_)
    ) {
        return Err(Stop::Unknown(UnknownReason::NotClosure));
    }
    Ok(OpRef {
        unit: storage.owner,
        operation: definition,
    })
}

/// Resolve a call's original callable producer through the common locator.
/// This is identity evidence only; clients still request the complete scoped
/// input proof before changing a representation or admitting incoming values.
pub(super) fn body_for_call<A: Admission>(
    program: &Program<'_>,
    uses: &UseIndex,
    unit: UnitId,
    call: CallId,
    admission: &mut A,
) -> Result<Option<UnitId>, A::Error> {
    tables(program, uses, admission)?;
    let data = checked_unit(program, uses, unit, admission)?;
    admission.work(1)?;
    let site = data
        .calls
        .get(call.index())
        .ok_or_else(|| admission.invalid("callable locator call"))?;
    let CallTarget::Value {
        callee,
        invocation: Invocation::Value,
    } = site.target
    else {
        return Ok(None);
    };
    let definition = data
        .values
        .get(callee.index())
        .ok_or_else(|| admission.invalid("callable locator callee"))?
        .definition;
    let operation = &data.operations[definition.index()];
    let creation = match operation.kind {
        OperationKind::Closure(_) => OpRef {
            unit,
            operation: definition,
        },
        OperationKind::Load(place) => {
            let Place::Cell(cell) = data.places[place.index()] else {
                return Ok(None);
            };
            match locate(program, uses, cell, admission) {
                Ok(creation) => creation,
                Err(Stop::Unknown(_)) => return Ok(None),
                Err(Stop::Error(error)) => return Err(error),
            }
        }
        _ => return Ok(None),
    };
    let owner = checked_unit(program, uses, creation.unit, admission)?;
    let OperationKind::Closure(body) = owner.operations[creation.operation.index()].kind else {
        return Err(admission.invalid("callable locator creation"));
    };
    Ok(Some(body))
}
