//! Exact primitive strings as bounded physical choices over unchanged source.
//!
//! One existing local-facts query supplies contents and ordinary-effect proof.
//! The preparation reserves output before borrowing/copying its payload; no
//! evaluator, cache, target binding, or cache-owned Arc enters this evidence.

use super::facts::{
    self, ExactString, ExactValue, FactRequest, Legality, ObservationDemand, UnitFacts,
};
use super::uses::UseIndex;
use super::*;
use crate::compilation_policy::{
    AnalysisAttempt, AnalysisCompletion, AnalysisWorkReceipt, BudgetError, BudgetLedger, WorkDomain,
};
use std::mem::size_of;

pub(super) const STRING_FAMILY_PLAN: u32 = 4;
pub(super) const STRING_FAMILY_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ValueRef {
    pub unit: UnitId,
    pub value: ValueId,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringChoice {
    LiteralAtDefinition,
    /// Entry of this same semantic activation. Every physical inline occurrence
    /// receives its own binding; this is not permission to hoist into a caller.
    SharedLiteral {
        activation: UnitId,
    },
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FamilyDependencies {
    local: facts::Dependencies,
}
impl FamilyDependencies {
    pub(super) fn valid_for(&self, program: &Program<'_>, _uses: &UseIndex) -> bool {
        self.local.valid_for(program)
    }
    pub(super) fn valid_for_published(&self, program: &Program<'_>, _uses: &UseIndex) -> bool {
        self.local.valid_for(program)
    }
    pub(super) fn validation_work(&self) -> Option<u64> {
        Some(2) // one table stamp and one unit stamp; no cell-derived facts
    }
}

#[derive(Debug)]
enum Payload {
    Pending,
    Source(StringId),
    Owned(StringValue),
}

#[must_use = "retain in Compilation or discard through the original ledger"]
#[derive(Debug)]
pub(super) struct StringFamily {
    unit: UnitId,
    definitions: Vec<ValueRef>,
    operations: Vec<OpId>,
    choice: StringChoice,
    payload: Payload,
    dependencies: FamilyDependencies,
    charge: (WorkDomain, u64),
}
impl StringFamily {
    pub(super) fn unit(&self) -> UnitId {
        self.unit
    }
    pub(super) fn definitions(&self) -> &[ValueRef] {
        &self.definitions
    }
    /// Corresponds to definitions(), in stable ValueRef order.
    pub(super) fn operations(&self) -> &[OpId] {
        &self.operations
    }
    pub(super) fn choice(&self) -> StringChoice {
        self.choice
    }
    pub(super) fn payload<'a>(&'a self, program: &'a Program<'_>) -> &'a StringValue {
        assert!(
            self.dependencies.local.valid_for(program),
            "stale string evidence"
        );
        match &self.payload {
            Payload::Source(id) => &program.strings[id.index()],
            Payload::Owned(value) => value,
            Payload::Pending => unreachable!("only complete evidence leaves preparation"),
        }
    }
    pub(super) fn dependencies(&self) -> &FamilyDependencies {
        &self.dependencies
    }
    pub(super) fn retained_bytes(&self) -> u64 {
        self.charge.1
    }
    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), BudgetError> {
        let (domain, bytes) = self.charge;
        drop(self);
        ledger.release(domain, bytes)
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct FamilyRequest {
    pub attempt: AnalysisAttempt,
    pub scratch_bytes: u64,
    pub output_bytes: u64,
    pub local_facts: FactRequest,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnknownReason {
    EmptyDefinitions,
    CrossUnitDefinitions,
    InvalidDefinition,
    DuplicateDefinitions,
    UnsupportedActivation,
    UnsupportedProducer,
    UnqualifiedFacts,
    UnknownValue,
    NotString,
    UnequalValues,
    ObservableEvaluation,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceLimit {
    Work,
    Scratch,
    Output,
    LocalFacts,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FamilyError {
    InvalidAttempt,
    InvalidProgram(&'static str),
    Capacity,
    AllocationFailed,
    AlreadyChecked,
    Budget(BudgetError),
}
impl From<BudgetError> for FamilyError {
    fn from(error: BudgetError) -> Self {
        Self::Budget(error)
    }
}
#[derive(Debug)]
pub(super) enum FamilyOutcome {
    Complete(StringFamily),
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
#[derive(Debug)]
pub(super) struct FamilyAnalysis {
    pub outcome: FamilyOutcome,
    pub receipt: AnalysisWorkReceipt,
}
#[derive(Debug)]
pub(super) enum PreparationOutcome {
    Ready(PreparedString),
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
#[derive(Debug)]
pub(super) struct Preparation {
    pub outcome: PreparationOutcome,
    /// Ready: preview, billed by finish/discard. Terminal: already billed.
    pub receipt: AnalysisWorkReceipt,
}
#[derive(Debug, Clone, Copy)]
enum Status {
    Unchecked,
    Complete,
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
}
enum Stop {
    Unknown(UnknownReason),
    Truncated(ResourceLimit),
    Error(FamilyError),
}
impl From<FamilyError> for Stop {
    fn from(error: FamilyError) -> Self {
        Self::Error(error)
    }
}

#[must_use = "finish/discard after the one planned facts query, without unrelated ledger spending"]
#[derive(Debug)]
pub(super) struct PreparedString {
    family: StringFamily,
    request: FamilyRequest,
    used: u64,
    output_used: u64,
    reservation: u64,
    status: Status,
}
impl PreparedString {
    pub(super) fn unit(&self) -> UnitId {
        self.family.unit()
    }
    fn work(&mut self, amount: u64) -> Result<(), Stop> {
        self.used = self
            .used
            .checked_add(amount)
            .filter(|work| *work <= self.request.attempt.work_quota)
            .ok_or(Stop::Truncated(ResourceLimit::Work))?;
        Ok(())
    }
    fn receipt(&self) -> AnalysisWorkReceipt {
        AnalysisWorkReceipt {
            attempt: self.request.attempt,
            completion: if matches!(self.status, Status::Truncated(_)) {
                AnalysisCompletion::Truncated
            } else {
                AnalysisCompletion::Complete
            },
            logical_work: self.used,
        }
    }
    fn prepare_inner(
        &mut self,
        program: &Program<'_>,
        definitions: &[ValueRef],
        ledger: &mut BudgetLedger,
    ) -> Result<(), Stop> {
        self.work(1)?;
        let first = definitions
            .first()
            .ok_or(Stop::Unknown(UnknownReason::EmptyDefinitions))?;
        let frozen = program
            .units
            .get(first.unit.index())
            .filter(|unit| unit.id() == first.unit)
            .ok_or(Stop::Unknown(UnknownReason::InvalidDefinition))?;
        self.family.unit = first.unit;
        self.family.dependencies.local = facts::Dependencies {
            unit: first.unit,
            unit_revision: frozen.revision(),
            tables_revision: program.tables_revision,
        };
        if matches!(self.family.choice, StringChoice::SharedLiteral{activation} if activation != first.unit)
        {
            return Err(Stop::Unknown(UnknownReason::UnsupportedActivation));
        }
        let scratch = (size_of::<Self>() - size_of::<StringFamily>()) as u64;
        if scratch > self.request.scratch_bytes {
            return Err(Stop::Truncated(ResourceLimit::Scratch));
        }
        let count = u64::try_from(definitions.len()).map_err(|_| FamilyError::Capacity)?;
        let metadata = count
            .checked_mul((size_of::<ValueRef>() + size_of::<OpId>()) as u64)
            .and_then(|bytes| bytes.checked_add(size_of::<StringFamily>() as u64))
            .ok_or(FamilyError::Capacity)?;
        if metadata > self.request.output_bytes {
            return Err(Stop::Truncated(ResourceLimit::Output));
        }
        self.reservation = self
            .request
            .output_bytes
            .checked_add(scratch)
            .ok_or(FamilyError::Capacity)?;
        if let Err(error) = ledger.retain(self.family.charge.0, self.reservation) {
            self.reservation = 0;
            return Err(FamilyError::Budget(error).into());
        }
        self.work(count)?;
        self.family
            .definitions
            .try_reserve_exact(definitions.len())
            .map_err(|_| FamilyError::AllocationFailed)?;
        self.family
            .operations
            .try_reserve_exact(definitions.len())
            .map_err(|_| FamilyError::AllocationFailed)?;
        self.family.definitions.extend_from_slice(definitions);
        let sort = count
            .checked_mul((usize::BITS - definitions.len().max(1).leading_zeros()) as u64)
            .ok_or(FamilyError::Capacity)?;
        self.work(sort)?;
        self.family.definitions.sort_unstable();
        for index in 0..self.family.definitions.len() {
            self.work(1)?;
            let definition = self.family.definitions[index];
            if definition.unit != first.unit {
                return Err(Stop::Unknown(UnknownReason::CrossUnitDefinitions));
            }
            if index > 0 && definition == self.family.definitions[index - 1] {
                return Err(Stop::Unknown(UnknownReason::DuplicateDefinitions));
            }
            let value = frozen
                .data()
                .values
                .get(definition.value.index())
                .ok_or(Stop::Unknown(UnknownReason::InvalidDefinition))?;
            let operation = frozen
                .data()
                .operations
                .get(value.definition.index())
                .ok_or(FamilyError::InvalidProgram("string value producer"))?;
            if operation.result != Some(definition.value) {
                return Err(FamilyError::InvalidProgram("string producer identity").into());
            }
            if !matches!(
                operation.kind,
                OperationKind::Constant(Constant::String(_))
                    | OperationKind::Binary(BinaryOp::Add)
                    | OperationKind::CopyValue
            ) {
                return Err(Stop::Unknown(UnknownReason::UnsupportedProducer));
            }
            self.family.operations.push(value.definition);
        }
        self.output_used = (self.family.definitions.capacity() as u64)
            .checked_mul(size_of::<ValueRef>() as u64)
            .and_then(|bytes| {
                (self.family.operations.capacity() as u64)
                    .checked_mul(size_of::<OpId>() as u64)
                    .and_then(|ops| bytes.checked_add(ops))
            })
            .and_then(|bytes| bytes.checked_add(size_of::<StringFamily>() as u64))
            .ok_or(FamilyError::Capacity)?;
        if self.output_used > self.request.output_bytes {
            return Err(Stop::Truncated(ResourceLimit::Output));
        }
        Ok(())
    }

    /// The compilation owner calls this once with the planned scoped query.
    /// Output capacity and joint work feasibility were admitted before entry.
    /// Bounds are charged before equality scans and before the sole owned copy.
    pub(super) fn check_facts(
        &mut self,
        program: &Program<'_>,
        facts: &UnitFacts,
        receipt: AnalysisWorkReceipt,
    ) -> Result<(), FamilyError> {
        if !matches!(self.status, Status::Unchecked) {
            return Err(FamilyError::AlreadyChecked);
        }
        self.status = Status::Unknown(UnknownReason::UnqualifiedFacts);
        match self.check_inner(program, facts, receipt) {
            Ok(()) => self.status = Status::Complete,
            Err(Stop::Unknown(reason)) => self.status = Status::Unknown(reason),
            Err(Stop::Truncated(limit)) => self.status = Status::Truncated(limit),
            Err(Stop::Error(error)) => return Err(error),
        }
        Ok(())
    }
    fn check_inner(
        &mut self,
        program: &Program<'_>,
        facts: &UnitFacts,
        receipt: AnalysisWorkReceipt,
    ) -> Result<(), Stop> {
        self.work(1)?;
        if facts.dependencies() != self.family.dependencies.local
            || !facts.dependencies().valid_for(program)
            || receipt.attempt != self.request.local_facts.attempt
            || receipt.logical_work > receipt.attempt.work_quota
        {
            return Err(Stop::Unknown(UnknownReason::UnqualifiedFacts));
        }
        let mut reference: Option<&StringValue> = None;
        let mut source = None;
        for index in 0..self.family.definitions.len() {
            self.work(1)?;
            let definition = self.family.definitions[index];
            let value = match facts.value(definition.value) {
                facts::ValueKnowledge::Exact(ExactValue::String(value)) => value,
                facts::ValueKnowledge::Exact(_) => {
                    return Err(Stop::Unknown(UnknownReason::NotString))
                }
                facts::ValueKnowledge::Unknown(reason) => {
                    return Err(
                        if matches!(
                            reason,
                            facts::UnknownReason::WorkLimit
                                | facts::UnknownReason::MemoryLimit
                                | facts::UnknownReason::Unvisited
                        ) {
                            Stop::Truncated(ResourceLimit::LocalFacts)
                        } else {
                            Stop::Unknown(UnknownReason::UnknownValue)
                        },
                    );
                }
            };
            let text = facts
                .string(program, definition.value)
                .ok_or(FamilyError::InvalidProgram("exact string payload"))?;
            if facts.can_drop(self.family.operations[index], ObservationDemand::Discarded)
                != Legality::PermittedUnderContext
            {
                return Err(Stop::Unknown(UnknownReason::ObservableEvaluation));
            }
            if let Some(first) = reference {
                let work = (first.storage_bytes() as u64)
                    .checked_add(text.storage_bytes() as u64)
                    .ok_or(FamilyError::Capacity)?;
                self.work(work)?;
                if !first.code_units().eq(text.code_units()) {
                    return Err(Stop::Unknown(UnknownReason::UnequalValues));
                }
            } else {
                reference = Some(text);
            }
            if let ExactString::Source(id) = value {
                source.get_or_insert(id);
            }
        }
        if let Some(id) = source {
            self.family.payload = Payload::Source(id);
        } else {
            let text =
                reference.ok_or(FamilyError::InvalidProgram("empty prepared string family"))?;
            let bytes = u64::try_from(text.storage_bytes()).map_err(|_| FamilyError::Capacity)?;
            let output = self
                .output_used
                .checked_add(bytes)
                .ok_or(FamilyError::Capacity)?;
            if output > self.request.output_bytes {
                return Err(Stop::Truncated(ResourceLimit::Output));
            }
            self.work(bytes.checked_add(1).ok_or(FamilyError::Capacity)?)?;
            let copy = text
                .try_clone()
                .map_err(|_| FamilyError::AllocationFailed)?;
            self.output_used = self
                .output_used
                .checked_add(copy.capacity_bytes() as u64)
                .ok_or(FamilyError::Capacity)?;
            self.family.payload = Payload::Owned(copy);
            if self.output_used > self.request.output_bytes {
                return Err(Stop::Truncated(ResourceLimit::Output));
            }
        }
        Ok(())
    }

    pub(super) fn finish(self, ledger: &mut BudgetLedger) -> Result<FamilyAnalysis, FamilyError> {
        let receipt = self.receipt();
        let charged = ledger.charge_analysis(self.family.charge.0, self.request.attempt, receipt);
        if let Err(error) = charged {
            let (domain, bytes) = (self.family.charge.0, self.reservation);
            drop(self);
            ledger.release(domain, bytes)?;
            return Err(error.into());
        }
        let Self {
            mut family,
            reservation,
            output_used,
            status,
            ..
        } = self;
        let outcome = match status {
            Status::Complete => {
                let Some(unused) = reservation.checked_sub(output_used) else {
                    let domain = family.charge.0;
                    drop(family);
                    ledger.release(domain, reservation)?;
                    return Err(FamilyError::Capacity);
                };
                ledger.release(family.charge.0, unused)?;
                family.charge.1 = output_used;
                FamilyOutcome::Complete(family)
            }
            other => {
                let domain = family.charge.0;
                drop(family);
                ledger.release(domain, reservation)?;
                match other {
                    Status::Truncated(limit) => FamilyOutcome::Truncated(limit),
                    Status::Unknown(reason) => FamilyOutcome::Unknown(reason),
                    Status::Unchecked => FamilyOutcome::Unknown(UnknownReason::UnqualifiedFacts),
                    Status::Complete => unreachable!(),
                }
            }
        };
        Ok(FamilyAnalysis { outcome, receipt })
    }
    /// Failure/discard also bills performed checks and releases every payload.
    pub(super) fn discard(self, ledger: &mut BudgetLedger) -> Result<(), FamilyError> {
        let receipt = self.receipt();
        let (domain, bytes) = (self.family.charge.0, self.reservation);
        let charged = ledger.charge_analysis(domain, self.request.attempt, receipt);
        drop(self);
        let released = ledger.release(domain, bytes);
        charged?;
        released?;
        Ok(())
    }
}

pub(super) fn prepare(
    program: &Program<'_>,
    definitions: &[ValueRef],
    choice: StringChoice,
    request: FamilyRequest,
    ledger: &mut BudgetLedger,
    domain: WorkDomain,
) -> Result<Preparation, FamilyError> {
    if request.attempt.plan != STRING_FAMILY_PLAN
        || request.attempt.algorithm_version != STRING_FAMILY_VERSION
        || request.local_facts.attempt.plan != facts::LOCAL_FACTS_PLAN
        || request.local_facts.attempt.algorithm_version != facts::LOCAL_FACTS_VERSION
    {
        return Err(FamilyError::InvalidAttempt);
    }
    // Feasibility only, not a second spending ledger. Compilation executes the
    // one planned facts query then finish/discard with no intervening work.
    // Session memory admission happens while this preparation stays reserved.
    let mut admission = ledger.clone();
    for attempt in [request.attempt, request.local_facts.attempt] {
        admission.charge_analysis(
            domain,
            attempt,
            AnalysisWorkReceipt {
                attempt,
                completion: AnalysisCompletion::Truncated,
                logical_work: attempt.work_quota,
            },
        )?;
    }
    let unit = definitions
        .first()
        .map_or(UnitId::from_index(0).unwrap(), |value| value.unit);
    let mut prepared = PreparedString {
        family: StringFamily {
            unit,
            definitions: Vec::new(),
            operations: Vec::new(),
            choice,
            payload: Payload::Pending,
            dependencies: FamilyDependencies {
                local: facts::Dependencies {
                    unit,
                    unit_revision: program.tables_revision,
                    tables_revision: program.tables_revision,
                },
            },
            charge: (domain, 0),
        },
        request,
        used: 0,
        output_used: 0,
        reservation: 0,
        status: Status::Unchecked,
    };
    let error = match prepared.prepare_inner(program, definitions, ledger) {
        Ok(()) => {
            return Ok(Preparation {
                receipt: prepared.receipt(),
                outcome: PreparationOutcome::Ready(prepared),
            })
        }
        Err(Stop::Unknown(reason)) => {
            prepared.status = Status::Unknown(reason);
            None
        }
        Err(Stop::Truncated(limit)) => {
            prepared.status = Status::Truncated(limit);
            None
        }
        Err(Stop::Error(error)) => Some(error),
    };
    if let Some(error) = error {
        prepared.discard(ledger)?;
        return Err(error);
    }
    let result = prepared.finish(ledger)?;
    let outcome = match result.outcome {
        FamilyOutcome::Unknown(reason) => PreparationOutcome::Unknown(reason),
        FamilyOutcome::Truncated(limit) => PreparationOutcome::Truncated(limit),
        FamilyOutcome::Complete(_) => unreachable!("preparation only publishes checked evidence"),
    };
    Ok(Preparation {
        outcome,
        receipt: result.receipt,
    })
}
