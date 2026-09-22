//! Proof of ordinary primitive *runtime* values, independent of declared type.
//!
//! Discover only producer dependencies of requested roots. A closed cell joins
//! every initializer and write from the complete use index. Opaque boundaries
//! then propagate through existing reverse uses; each discovered node becomes
//! unknown at most once. Optimistic closed cycles express the induction that
//! initialized primitive storage remains primitive under primitive-only writes.
//! This proves no initialization, effect, motion or allocation obligation.
use super::callable_inputs::{CallObservations, CallableInputs, InputOutcome};
use super::helper_family::HelperFamily;
use super::record_family::{ReadOrWrite, RecordFamily};
use super::uses::{CellUse, CellUseSite, UseIndex, ValueUse};
use super::*;
use crate::compilation_contract::JavaScriptExecution;

/// All backing belongs to the caller's existing scratch scope. Implementations
/// admit capacity and relocation peaks before allocation, and drop a released
/// Vec before releasing its charge. Admission errors unwind that same scope.
pub(super) trait Admission {
    type Error;
    fn work(&mut self, amount: usize) -> Result<(), Self::Error>;
    fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, Self::Error>;
    fn push<T>(&mut self, target: &mut Vec<T>, value: T) -> Result<(), Self::Error>;
    fn release<T>(&mut self, value: Vec<T>) -> Result<(), Self::Error>;
    fn invalid(&self, reason: &'static str) -> Self::Error;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ResultRecipe {
    Source,
    NormalizedI32,
    Undefined,
}

/// The selected target owner must use this same recipe when emitting the
/// operation. A declared Int/String type is not a result conversion.
pub(super) trait Recipes {
    fn result(&self, program: &Program<'_>, unit: UnitId, operation: OpId) -> ResultRecipe;
}

pub(super) struct SourceRecipes;
impl Recipes for SourceRecipes {
    fn result(&self, _: &Program<'_>, _: UnitId, _: OpId) -> ResultRecipe {
        ResultRecipe::Source
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Subject {
    Value { unit: UnitId, value: ValueId },
    Cell(CellId),
    RecordSlot { state: CellId, slot: u32 },
}

/// Only complete private-call/record access families can add producer edges.
/// This is borrowed evidence, not caller-supplied primitive assertions. Helper
/// discovery must have finished its complete call/environment classification
/// before querying body domains; the body effect proof may follow the query.
#[derive(Clone, Copy)]
pub(super) struct DomainInputs<'a> {
    helper: Option<&'a HelperFamily>,
    records: &'a [&'a RecordFamily],
    execution: Option<JavaScriptExecution>,
}
impl<'a> DomainInputs<'a> {
    pub(super) fn empty() -> Self {
        Self {
            helper: None,
            records: &[],
            execution: None,
        }
    }
    pub(super) fn for_helper(helper: &'a HelperFamily) -> Self {
        Self {
            helper: Some(helper),
            records: &[],
            execution: None,
        }
    }
    pub(super) fn with_records(mut self, records: &'a [&'a RecordFamily]) -> Self {
        self.records = records;
        self
    }
    pub(super) fn with_execution(mut self, execution: JavaScriptExecution) -> Self {
        self.execution = Some(execution);
        self
    }
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
    pub(super) fn valid_for<A: Admission>(
        &self,
        program: &Program<'_>,
        uses: &UseIndex,
        budget: &mut A,
    ) -> Result<bool, A::Error> {
        budget.work(1)?;
        if self.tables != program.tables_revision
            || uses.tables_revision() != program.tables_revision
        {
            return Ok(false);
        }
        for &(unit, revision) in &self.units {
            budget.work(1)?;
            if !program
                .units
                .get(unit.index())
                .is_some_and(|value| value.id() == unit && value.revision() == revision)
                || !uses
                    .unit(unit)
                    .is_some_and(|value| value.revision() == revision)
            {
                return Ok(false);
            }
        }
        for &(cell, revision) in &self.cells {
            budget.work(1)?;
            if !uses
                .cell(cell)
                .is_some_and(|value| value.revision() == revision)
            {
                return Ok(false);
            }
        }
        for &(body, revision) in &self.creators {
            budget.work(1)?;
            if !uses
                .creators(body)
                .is_some_and(|value| value.revision() == revision)
            {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Key {
    Subject(Subject),
    Region { unit: UnitId, region: RegionId },
    InputCall { unit: UnitId, operation: OpId },
}
impl Key {
    fn hash(self) -> usize {
        let (tag, a, b) = match self {
            Self::Subject(Subject::Value { unit, value }) => (1u64, unit.index(), value.index()),
            Self::Subject(Subject::Cell(cell)) => (2, cell.index(), 0),
            Self::Subject(Subject::RecordSlot { state, slot }) => (3, state.index(), slot as usize),
            Self::Region { unit, region } => (4, unit.index(), region.index()),
            Self::InputCall { unit, operation } => (5, unit.index(), operation.index()),
        };
        let mut word = ((a as u64) << 32) | b as u64;
        word ^= tag.wrapping_mul(0x9e3779b97f4a7c15);
        word ^= word >> 30;
        word = word.wrapping_mul(0xbf58476d1ce4e5b9);
        word ^= word >> 27;
        word = word.wrapping_mul(0x94d049bb133111eb);
        (word ^ (word >> 31)) as usize
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Locator {
    RegionOwner(usize),
    CallBody(UnitId),
}
#[derive(Debug)]
struct Node {
    key: Key,
    unknown: bool,
    // Sparse checked locators share the existing key table. Regions identify
    // the result owner; complete call evidence identifies the parameter body.
    // Argument/result edges stay in Program/UseIndex and are never copied here.
    locator: Option<Locator>,
}

#[derive(Debug)]
pub(super) struct DomainProof {
    nodes: Vec<Node>,
    table: Vec<Option<usize>>,
    pending: Vec<usize>,
    // One lazily requested call proof per body. These retain original use
    // witnesses through taint propagation, not a second dependency graph.
    incoming: Vec<(UnitId, Option<InputOutcome>)>,
    dependencies: Dependencies,
}
impl DomainProof {
    pub(super) fn build<A: Admission, R: Recipes>(
        program: &Program<'_>,
        uses: &UseIndex,
        roots: &[Subject],
        inputs: DomainInputs<'_>,
        recipes: &R,
        budget: &mut A,
    ) -> Result<Self, A::Error> {
        budget.work(1)?;
        if uses.tables_revision() != program.tables_revision {
            return Err(budget.invalid("raw domain stale table index"));
        }
        let mut proof = Self {
            nodes: Vec::new(),
            table: Vec::new(),
            pending: Vec::new(),
            incoming: Vec::new(),
            dependencies: Dependencies {
                tables: program.tables_revision,
                units: Vec::new(),
                cells: Vec::new(),
                creators: Vec::new(),
            },
        };
        let result = (|| {
            if let Some(helper) = inputs.helper {
                let work = helper
                    .dependencies()
                    .units()
                    .len()
                    .checked_add(helper.dependencies().cells().len())
                    .and_then(|n| n.checked_add(helper.dependencies().creators().len()))
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| budget.invalid("raw domain helper validation work"))?;
                budget.work(work)?;
                if !helper.dependencies().valid_for_published(program, uses) {
                    return Err(budget.invalid("raw domain stale helper family"));
                }
                for &(unit, revision) in helper.dependencies().units() {
                    budget.work(1)?;
                    if !program
                        .units
                        .get(unit.index())
                        .is_some_and(|data| data.revision() == revision)
                    {
                        return Err(budget.invalid("raw domain stale helper unit"));
                    }
                    proof.unit(program, uses, unit, budget)?;
                }
                for &(cell, revision) in helper.dependencies().cells() {
                    budget.work(1)?;
                    if !uses
                        .cell(cell)
                        .is_some_and(|data| data.revision() == revision)
                    {
                        return Err(budget.invalid("raw domain stale helper cell"));
                    }
                    proof.cell_stamp(program, uses, cell, budget)?;
                }
                for &(body, _) in helper.dependencies().creators() {
                    proof.creator_stamp(uses, body, budget)?;
                }
                for call in helper.calls() {
                    budget.work(1)?;
                    proof.input_call(call.caller, call.call, helper.root().body, budget)?;
                }
            }
            for &root in roots {
                proof.insert(Key::Subject(root), budget)?;
            }
            let mut cursor = 0;
            while cursor < proof.nodes.len() {
                budget.work(1)?;
                proof.expand(program, uses, inputs, recipes, cursor, budget)?;
                cursor += 1;
            }
            cursor = 0;
            while cursor < proof.pending.len() {
                budget.work(1)?;
                let node = proof.pending[cursor];
                proof.propagate(program, uses, inputs, recipes, node, budget)?;
                cursor += 1;
            }
            Ok(())
        })();
        match result {
            Ok(()) => Ok(proof),
            Err(error) => {
                let _ = proof.discard(budget);
                Err(error)
            }
        }
    }
    pub(super) fn dependencies(&self) -> &Dependencies {
        &self.dependencies
    }
    pub(super) fn primitive<A: Admission>(
        &self,
        subject: Subject,
        budget: &mut A,
    ) -> Result<bool, A::Error> {
        Ok(self
            .find(Key::Subject(subject), budget)?
            .is_some_and(|index| !self.nodes[index].unknown))
    }
    pub(super) fn discard<A: Admission>(self, budget: &mut A) -> Result<(), A::Error> {
        let Self {
            nodes,
            table,
            pending,
            mut incoming,
            dependencies,
        } = self;
        // Attempt every release even when one reports an accounting error.
        let a = budget.release(nodes);
        let b = budget.release(table);
        let c = budget.release(pending);
        let d = budget.release(dependencies.units);
        let e = budget.release(dependencies.cells);
        let f = budget.release(dependencies.creators);
        let mut g = Ok(());
        for (_, entry) in incoming.drain(..) {
            if let Some(InputOutcome::Complete(proof)) = entry {
                let released = proof.discard(budget);
                g = g.and(released);
            }
        }
        let h = budget.release(incoming);
        a.and(b).and(c).and(d).and(e).and(f).and(g).and(h)
    }
    fn find<A: Admission>(&self, key: Key, budget: &mut A) -> Result<Option<usize>, A::Error> {
        if self.table.is_empty() {
            return Ok(None);
        }
        let mut slot = key.hash() & (self.table.len() - 1);
        loop {
            budget.work(1)?;
            let Some(index) = self.table[slot] else {
                return Ok(None);
            };
            if self.nodes[index].key == key {
                return Ok(Some(index));
            }
            slot = (slot + 1) & (self.table.len() - 1);
        }
    }
    fn insert<A: Admission>(&mut self, key: Key, budget: &mut A) -> Result<usize, A::Error> {
        if let Some(index) = self.find(key, budget)? {
            return Ok(index);
        }
        if self.nodes.len() >= self.table.len() / 2 {
            let capacity = self
                .table
                .len()
                .max(8)
                .checked_mul(2)
                .ok_or_else(|| budget.invalid("raw domain table capacity"))?;
            let mut next = budget.vector(capacity)?;
            next.resize(capacity, None);
            let rehash = (|| {
                for (index, node) in self.nodes.iter().enumerate() {
                    let mut slot = node.key.hash() & (capacity - 1);
                    loop {
                        budget.work(1)?;
                        if next[slot].is_none() {
                            next[slot] = Some(index);
                            break;
                        }
                        slot = (slot + 1) & (capacity - 1);
                    }
                }
                Ok(())
            })();
            if let Err(error) = rehash {
                let _ = budget.release(next);
                return Err(error);
            }
            budget.release(std::mem::replace(&mut self.table, next))?;
        }
        let index = self.nodes.len();
        budget.push(
            &mut self.nodes,
            Node {
                key,
                unknown: false,
                locator: None,
            },
        )?;
        let mut slot = key.hash() & (self.table.len() - 1);
        loop {
            budget.work(1)?;
            if self.table[slot].is_none() {
                self.table[slot] = Some(index);
                break;
            }
            slot = (slot + 1) & (self.table.len() - 1);
        }
        Ok(index)
    }
    fn unknown<A: Admission>(&mut self, index: usize, budget: &mut A) -> Result<(), A::Error> {
        if !self.nodes[index].unknown {
            budget.push(&mut self.pending, index)?;
            self.nodes[index].unknown = true;
        }
        Ok(())
    }
    fn poison<A: Admission>(&mut self, key: Key, budget: &mut A) -> Result<(), A::Error> {
        if let Some(index) = self.find(key, budget)? {
            self.unknown(index, budget)?;
        }
        Ok(())
    }
    fn unit<A: Admission>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        unit: UnitId,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        for &(known, _) in &self.dependencies.units {
            budget.work(1)?;
            if known == unit {
                return Ok(());
            }
        }
        let data = program
            .units
            .get(unit.index())
            .filter(|data| data.id() == unit)
            .ok_or_else(|| budget.invalid("raw domain unit"))?;
        if !uses
            .unit(unit)
            .is_some_and(|index| index.revision() == data.revision())
        {
            return Err(budget.invalid("raw domain stale unit index"));
        }
        budget.push(&mut self.dependencies.units, (unit, data.revision()))
    }
    fn cell_stamp<A: Admission>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        cell: CellId,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        for &(known, _) in &self.dependencies.cells {
            budget.work(1)?;
            if known == cell {
                return Ok(());
            }
        }
        let owner = program
            .cells
            .get(cell.index())
            .ok_or_else(|| budget.invalid("raw domain cell"))?
            .owner;
        self.unit(program, uses, owner, budget)?;
        let users = uses
            .cell(cell)
            .ok_or_else(|| budget.invalid("raw domain cell index"))?;
        budget.push(&mut self.dependencies.cells, (cell, users.revision()))
    }
    fn creator_stamp<A: Admission>(
        &mut self,
        uses: &UseIndex,
        body: UnitId,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        for &(known, _) in &self.dependencies.creators {
            budget.work(1)?;
            if known == body {
                return Ok(());
            }
        }
        let creators = uses
            .creators(body)
            .ok_or_else(|| budget.invalid("raw domain creator index"))?;
        budget.push(&mut self.dependencies.creators, (body, creators.revision()))
    }
    fn incoming<A: Admission>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        body: UnitId,
        execution: JavaScriptExecution,
        budget: &mut A,
    ) -> Result<usize, A::Error> {
        for (index, &(known, _)) in self.incoming.iter().enumerate() {
            budget.work(1)?;
            if known == body {
                return Ok(index);
            }
        }
        let index = self.incoming.len();
        // Admit the owner slot before producing owned evidence. A failed Vec
        // growth must never drop evidence while leaving its scratch charged.
        budget.push(&mut self.incoming, (body, None))?;
        let proof = CallableInputs::for_body_published(
            program,
            uses,
            body,
            CallObservations::from_execution(execution),
            budget,
        )?;
        self.incoming[index].1 = Some(proof);
        let Some(InputOutcome::Complete(proof)) = &self.incoming[index].1 else {
            return Ok(index);
        };
        let (unit_count, cell_count, creator_count) = (
            proof.dependencies().units().len(),
            proof.dependencies().cells().len(),
            proof.dependencies().creators().len(),
        );
        for offset in 0..unit_count {
            let unit = self
                .complete_incoming(index)
                .unwrap()
                .dependencies()
                .units()[offset]
                .0;
            self.unit(program, uses, unit, budget)?;
        }
        for offset in 0..cell_count {
            let cell = self
                .complete_incoming(index)
                .unwrap()
                .dependencies()
                .cells()[offset]
                .0;
            self.cell_stamp(program, uses, cell, budget)?;
        }
        for offset in 0..creator_count {
            let body = self
                .complete_incoming(index)
                .unwrap()
                .dependencies()
                .creators()[offset]
                .0;
            self.creator_stamp(uses, body, budget)?;
        }
        let calls = self.complete_incoming(index).unwrap().calls().len();
        for offset in 0..calls {
            budget.work(1)?;
            let call = self.complete_incoming(index).unwrap().calls()[offset];
            self.input_call(call.caller, call.call, body, budget)?;
        }
        Ok(index)
    }
    fn complete_incoming(&self, index: usize) -> Option<&CallableInputs> {
        match &self.incoming[index].1 {
            Some(InputOutcome::Complete(proof)) => Some(proof),
            _ => None,
        }
    }
    fn value<A: Admission>(
        &mut self,
        unit: UnitId,
        value: ValueId,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        self.insert(Key::Subject(Subject::Value { unit, value }), budget)?;
        Ok(())
    }
    fn region<A: Admission>(
        &mut self,
        unit: UnitId,
        region: RegionId,
        owner: usize,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        let index = self.insert(Key::Region { unit, region }, budget)?;
        if self.nodes[index]
            .locator
            .is_some_and(|found| found != Locator::RegionOwner(owner))
        {
            return Err(budget.invalid("raw domain region owner"));
        }
        self.nodes[index].locator = Some(Locator::RegionOwner(owner));
        Ok(())
    }
    fn input_call<A: Admission>(
        &mut self,
        unit: UnitId,
        operation: OpId,
        body: UnitId,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        let index = self.insert(Key::InputCall { unit, operation }, budget)?;
        if self.nodes[index]
            .locator
            .is_some_and(|found| found != Locator::CallBody(body))
        {
            return Err(budget.invalid("raw domain conflicting call body"));
        }
        self.nodes[index].locator = Some(Locator::CallBody(body));
        Ok(())
    }
    fn record<'a, A: Admission>(
        inputs: DomainInputs<'a>,
        state: CellId,
        budget: &mut A,
    ) -> Result<Option<&'a RecordFamily>, A::Error> {
        for &record in inputs.records {
            budget.work(1)?;
            if record.root().state == state {
                return Ok(Some(record));
            }
        }
        Ok(None)
    }
    fn projection<A: Admission>(
        inputs: DomainInputs<'_>,
        unit: UnitId,
        operation: OpId,
        budget: &mut A,
    ) -> Result<Option<Subject>, A::Error> {
        for record in inputs.records {
            for projection in record.projections() {
                budget.work(1)?;
                if projection.operation.unit == unit && projection.operation.operation == operation
                {
                    return Ok(Some(Subject::RecordSlot {
                        state: record.root().state,
                        slot: projection.slot,
                    }));
                }
            }
        }
        Ok(None)
    }
    fn expand<A: Admission, R: Recipes>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        inputs: DomainInputs<'_>,
        recipes: &R,
        index: usize,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        match self.nodes[index].key {
            Key::InputCall { .. } => {}
            Key::Subject(Subject::Value { unit, value }) => {
                self.unit(program, uses, unit, budget)?;
                let data = program.unit(unit).unwrap();
                let definition = data
                    .values
                    .get(value.index())
                    .ok_or_else(|| budget.invalid("raw domain value"))?
                    .definition;
                let operation = &data.operations[definition.index()];
                if recipes.result(program, unit, definition) != ResultRecipe::Source {
                    return Ok(());
                }
                let operands = data.operands(operation.operands).unwrap();
                match operation.kind {
                    OperationKind::Constant(_)
                    | OperationKind::IntBinary(_)
                    | OperationKind::Unary {
                        op: UnaryOp::Not, ..
                    }
                    | OperationKind::Unary { integer: true, .. } => {}
                    OperationKind::CopyValue
                    | OperationKind::Binary(_)
                    | OperationKind::Unary { .. } => {
                        for &value in operands {
                            self.value(unit, value, budget)?;
                        }
                    }
                    OperationKind::Load(place) => match data.places[place.index()] {
                        Place::Cell(cell) => {
                            self.insert(Key::Subject(Subject::Cell(cell)), budget)?;
                        }
                        _ => {
                            if let Some(slot) = Self::projection(inputs, unit, definition, budget)?
                            {
                                self.insert(Key::Subject(slot), budget)?;
                            } else {
                                self.unknown(index, budget)?;
                            }
                        }
                    },
                    OperationKind::Select { yes, no } => {
                        self.region(unit, yes, index, budget)?;
                        self.region(unit, no, index, budget)?;
                    }
                    OperationKind::ShortCircuit { right, .. } => {
                        for &value in operands {
                            self.value(unit, value, budget)?;
                        }
                        self.region(unit, right, index, budget)?;
                    }
                    _ => self.unknown(index, budget)?,
                }
            }
            Key::Subject(Subject::Cell(cell)) => {
                self.cell_stamp(program, uses, cell, budget)?;
                let data = &program.cells[cell.index()];
                if data.binding == CellBinding::Foreign || program.is_reference_parameter(cell) {
                    self.unknown(index, budget)?;
                }
                let mut initialized = false;
                for site in uses.cell(cell).unwrap().sites() {
                    budget.work(1)?;
                    let CellUseSite::Unit { unit, usage } = *site else {
                        self.unknown(index, budget)?;
                        continue;
                    };
                    self.unit(program, uses, unit, budget)?;
                    match usage {
                        CellUse::Initialize(operation) | CellUse::Write { operation, .. } => {
                            initialized |= matches!(usage, CellUse::Initialize(_));
                            let body = program.unit(unit).unwrap();
                            let operands = body
                                .operands(body.operations[operation.index()].operands)
                                .unwrap();
                            if let Some(&value) = operands.first() {
                                self.value(unit, value, budget)?;
                            } else {
                                self.unknown(index, budget)?;
                            }
                        }
                        CellUse::Parameter(position) => {
                            initialized = true;
                            if program.is_reference_parameter(cell) {
                                self.unknown(index, budget)?;
                                continue;
                            }
                            if let Some(helper) =
                                inputs.helper.filter(|helper| helper.root().body == unit)
                            {
                                if helper.calls().is_empty() {
                                    self.unknown(index, budget)?;
                                }
                                for call in helper.calls() {
                                    budget.work(1)?;
                                    let body = program.unit(call.caller).ok_or_else(|| {
                                        budget.invalid("raw domain helper caller")
                                    })?;
                                    let operands = body
                                        .arguments(body.calls[call.target.index()].arguments)
                                        .unwrap();
                                    let value =
                                        *operands.get(position as usize).ok_or_else(|| {
                                            budget.invalid("raw domain helper parameter")
                                        })?;
                                    if let CallArgument::Value(value) = value {
                                        self.value(call.caller, value, budget)?;
                                    } else {
                                        self.unknown(index, budget)?;
                                    }
                                }
                            } else if let Some(execution) = inputs.execution {
                                let incoming =
                                    self.incoming(program, uses, unit, execution, budget)?;
                                if let Some(proof) = self.complete_incoming(incoming) {
                                    let count = proof.calls().len();
                                    for offset in 0..count {
                                        budget.work(1)?;
                                        let call =
                                            self.complete_incoming(incoming).unwrap().calls()
                                                [offset];
                                        let caller = program.unit(call.caller).unwrap();
                                        let operands = caller
                                            .arguments(caller.calls[call.target.index()].arguments)
                                            .unwrap();
                                        let value =
                                            *operands.get(position as usize).ok_or_else(|| {
                                                budget.invalid("raw domain body parameter")
                                            })?;
                                        if let CallArgument::Value(value) = value {
                                            self.value(call.caller, value, budget)?;
                                        } else {
                                            self.unknown(index, budget)?;
                                        }
                                    }
                                } else {
                                    self.unknown(index, budget)?;
                                }
                            } else {
                                self.unknown(index, budget)?;
                            }
                        }
                        CellUse::Reference { .. } => {
                            self.unknown(index, budget)?;
                        }
                        CellUse::CatchBinding { .. } => {
                            initialized = true;
                            self.unknown(index, budget)?;
                        }
                        CellUse::Read { .. } | CellUse::Capture => {}
                    }
                }
                if !initialized {
                    self.unknown(index, budget)?;
                }
            }
            Key::Subject(Subject::RecordSlot { state, slot }) => {
                let Some(record) = Self::record(inputs, state, budget)? else {
                    return self.unknown(index, budget);
                };
                budget.work(
                    record
                        .dependencies()
                        .units()
                        .len()
                        .checked_add(2)
                        .ok_or_else(|| budget.invalid("raw domain dependency work"))?,
                )?;
                if !record.dependencies().valid_for_published(program, uses) {
                    return Err(budget.invalid("raw domain stale record family"));
                }
                self.cell_stamp(program, uses, state, budget)?;
                for &(unit, _) in record.dependencies().units() {
                    self.unit(program, uses, unit, budget)?;
                }
                let field = record
                    .slots()
                    .get(slot as usize)
                    .ok_or_else(|| budget.invalid("raw domain record slot"))?;
                if let Some(value) = field.initial_value {
                    self.value(record.allocation().unit, value, budget)?;
                }
                for projection in record.projections() {
                    budget.work(1)?;
                    if projection.slot == slot && projection.kind == ReadOrWrite::Write {
                        let unit = projection.operation.unit;
                        let data = program.unit(unit).unwrap();
                        let operation = &data.operations[projection.operation.operation.index()];
                        for &value in data.operands(operation.operands).unwrap() {
                            self.value(unit, value, budget)?;
                        }
                    }
                }
            }
            Key::Region { unit, region } => {
                self.unit(program, uses, unit, budget)?;
                if let Some(value) = program.unit(unit).unwrap().regions[region.index()].result {
                    self.value(unit, value, budget)?;
                }
            }
        }
        Ok(())
    }
    fn propagate<A: Admission, R: Recipes>(
        &mut self,
        program: &Program<'_>,
        uses: &UseIndex,
        inputs: DomainInputs<'_>,
        recipes: &R,
        index: usize,
        budget: &mut A,
    ) -> Result<(), A::Error> {
        match self.nodes[index].key {
            Key::InputCall { .. } => {}
            Key::Subject(Subject::Value { unit, value }) => {
                let data = program.unit(unit).unwrap();
                for usage in uses.unit(unit).unwrap().value_uses(value).unwrap() {
                    budget.work(1)?;
                    match *usage {
                        ValueUse::RegionResult(region) => {
                            self.poison(Key::Region { unit, region }, budget)?
                        }
                        ValueUse::Operand {
                            operation,
                            position,
                        }
                        | ValueUse::CallArgument {
                            operation,
                            position,
                            ..
                        } => {
                            let op = &data.operations[operation.index()];
                            if let Some(result) = op.result {
                                if recipes.result(program, unit, operation) == ResultRecipe::Source
                                    && matches!(
                                        op.kind,
                                        OperationKind::CopyValue
                                            | OperationKind::Binary(_)
                                            | OperationKind::Unary {
                                                integer: false,
                                                op: UnaryOp::Neg
                                            }
                                            | OperationKind::ShortCircuit { .. }
                                    )
                                {
                                    self.poison(
                                        Key::Subject(Subject::Value {
                                            unit,
                                            value: result,
                                        }),
                                        budget,
                                    )?;
                                }
                            }
                            match op.kind {
                                OperationKind::Initialize(cell) => {
                                    self.poison(Key::Subject(Subject::Cell(cell)), budget)?
                                }
                                OperationKind::Store(place) => {
                                    if let Place::Cell(cell) = data.places[place.index()] {
                                        self.poison(Key::Subject(Subject::Cell(cell)), budget)?;
                                    } else if let Some(slot) =
                                        Self::projection(inputs, unit, operation, budget)?
                                    {
                                        self.poison(Key::Subject(slot), budget)?;
                                    }
                                }
                                OperationKind::Call(_) => {
                                    if let Some(locator) =
                                        self.find(Key::InputCall { unit, operation }, budget)?
                                    {
                                        let Some(Locator::CallBody(body)) =
                                            self.nodes[locator].locator
                                        else {
                                            return Err(
                                                budget.invalid("raw domain missing call body")
                                            );
                                        };
                                        let cell = *program
                                            .unit(body)
                                            .unwrap()
                                            .parameters
                                            .get(position as usize)
                                            .ok_or_else(|| {
                                                budget.invalid("raw domain call argument position")
                                            })?;
                                        self.poison(Key::Subject(Subject::Cell(cell)), budget)?;
                                    }
                                }
                                OperationKind::Allocate { .. } => {
                                    for record in inputs.records {
                                        budget.work(1)?;
                                        if record.allocation().unit == unit
                                            && record.allocation().operation == operation
                                        {
                                            for (slot, field) in record.slots().iter().enumerate() {
                                                budget.work(1)?;
                                                if field.initial_value == Some(value) {
                                                    self.poison(
                                                        Key::Subject(Subject::RecordSlot {
                                                            state: record.root().state,
                                                            slot: slot as u32,
                                                        }),
                                                        budget,
                                                    )?;
                                                }
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        ValueUse::PlaceReceiver { .. }
                        | ValueUse::PlaceKey { .. }
                        | ValueUse::CallCallee { .. }
                        | ValueUse::CallReceiver { .. } => {}
                    }
                }
            }
            Key::Subject(Subject::Cell(cell)) => {
                for site in uses.cell(cell).unwrap().sites() {
                    budget.work(1)?;
                    if let CellUseSite::Unit {
                        unit,
                        usage: CellUse::Read { operation, .. },
                    } = *site
                    {
                        let data = program.unit(unit).unwrap();
                        if recipes.result(program, unit, operation) == ResultRecipe::Source {
                            if let Some(value) = data.operations[operation.index()].result {
                                self.poison(Key::Subject(Subject::Value { unit, value }), budget)?;
                            }
                        }
                    }
                }
            }
            Key::Subject(Subject::RecordSlot { state, slot }) => {
                if let Some(record) = Self::record(inputs, state, budget)? {
                    for projection in record.projections() {
                        budget.work(1)?;
                        if projection.slot == slot && projection.kind == ReadOrWrite::Read {
                            let unit = projection.operation.unit;
                            let op = projection.operation.operation;
                            if recipes.result(program, unit, op) == ResultRecipe::Source {
                                if let Some(value) =
                                    program.unit(unit).unwrap().operations[op.index()].result
                                {
                                    self.poison(
                                        Key::Subject(Subject::Value { unit, value }),
                                        budget,
                                    )?;
                                }
                            }
                        }
                    }
                }
            }
            Key::Region { .. } => {
                if let Some(Locator::RegionOwner(owner)) = self.nodes[index].locator {
                    self.unknown(owner, budget)?;
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compilation_policy::{
        AnalysisAttempt, BudgetLedger, BudgetPlan, ResourceLimits, WorkDomain, WorkKind,
    };
    use crate::output_budget::{AllocationBudget, AllocationClass, AllocationError};
    use std::mem::size_of;

    struct Meter<'a, 'b>(&'a mut AllocationBudget<'b>);
    impl Admission for Meter<'_, '_> {
        type Error = AllocationError;
        fn work(&mut self, amount: usize) -> Result<(), Self::Error> {
            self.0.work(WorkKind::Analysis, amount as u64)
        }
        fn vector<T>(&mut self, capacity: usize) -> Result<Vec<T>, Self::Error> {
            self.work(capacity)?;
            self.0.vector(AllocationClass::Scratch, capacity)
        }
        fn push<T>(&mut self, target: &mut Vec<T>, value: T) -> Result<(), Self::Error> {
            self.work(1)?;
            self.0.push(AllocationClass::Scratch, target, value)
        }
        fn release<T>(&mut self, value: Vec<T>) -> Result<(), Self::Error> {
            let bytes = value
                .capacity()
                .checked_mul(size_of::<T>())
                .ok_or(AllocationError::Capacity)?;
            drop(value);
            self.0.release(AllocationClass::Scratch, bytes as u64)
        }
        fn invalid(&self, _: &'static str) -> Self::Error {
            AllocationError::Capacity
        }
    }
    fn ledger(optional_work: u64) -> BudgetLedger {
        BudgetLedger::new(
            ResourceLimits::default(),
            BudgetPlan {
                baseline_work: 10_000_000,
                optional_work,
                baseline_retained_bytes: 0,
                retained_bytes: 10_000_000,
            },
        )
        .unwrap()
    }
    fn checked(source: &str, inspect: impl FnOnce(&Program<'_>)) {
        let arena = bumpalo::Bump::new();
        let syntax = crate::parse_source(&arena, source).unwrap();
        let semantics = crate::analyze(&syntax).unwrap();
        let program = from_checked_source(&syntax, &semantics).unwrap();
        program.verify().unwrap();
        inspect(&program);
    }
    fn cell(program: &Program<'_>, name: &str) -> CellId {
        let found: Vec<_> = program
            .cells
            .iter()
            .enumerate()
            .filter(|(_, cell)| cell.name == name)
            .collect();
        assert_eq!(found.len(), 1, "unique fixture cell {name}");
        CellId::from_index(found[0].0).unwrap()
    }

    #[test]
    fn private_cell_cycles_have_an_inductive_domain_but_opaque_boundaries_do_not() {
        checked(
            r#"
            extern string opaque();
            string text="";int index=0;
            while(index<3){text=text+"x";index+=1;}
            string foreign=opaque();
            string read(string parameter){return parameter;}
            print(text);print(foreign);
        "#,
            |program| {
                let mut ledger = ledger(1_000_000);
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                {
                    let mut allocation =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let mut meter = Meter(&mut allocation);
                    let roots = [
                        Subject::Cell(cell(program, "text")),
                        Subject::Cell(cell(program, "foreign")),
                        Subject::Cell(cell(program, "parameter")),
                    ];
                    let proof = DomainProof::build(
                        program,
                        &uses,
                        &roots,
                        DomainInputs::empty(),
                        &SourceRecipes,
                        &mut meter,
                    )
                    .unwrap();
                    assert!(proof.primitive(roots[0], &mut meter).unwrap());
                    assert!(!proof.primitive(roots[1], &mut meter).unwrap());
                    assert!(!proof.primitive(roots[2], &mut meter).unwrap());
                    proof.discard(&mut meter).unwrap();
                    assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
                }
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            },
        );
    }

    #[test]
    fn actual_target_normalization_proves_a_result_without_proving_its_inputs() {
        checked("extern int[] host();extern int opaque();int normalized=host()[0];int raw=opaque();print(normalized);print(raw);", |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
            {
                let mut allocation = AllocationBudget::new(Some((&mut ledger,WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let roots = [Subject::Cell(cell(program,"normalized")),Subject::Cell(cell(program,"raw"))];
                let semantic = DomainProof::build(program,&uses,&roots,DomainInputs::empty(),&SourceRecipes,&mut meter).unwrap();
                assert!(!semantic.primitive(roots[0],&mut meter).unwrap());
                semantic.discard(&mut meter).unwrap();
                let target = DomainProof::build(program,&uses,&roots,DomainInputs::empty(),&super::super::javascript::JavaScriptRecipes,&mut meter).unwrap();
                assert!(target.primitive(roots[0],&mut meter).unwrap());
                assert!(!target.primitive(roots[1],&mut meter).unwrap());
                target.discard(&mut meter).unwrap();
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch),0);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(),0);
        });
    }

    #[test]
    fn opaque_lazy_region_result_taints_the_enclosing_value_without_tainting_not() {
        checked(
            r#"extern string opaque();extern string? nullable();extern bool truth();bool choose=true;string selected=if(choose){opaque()}else{"ok"};string lazy=nullable()??"ok";bool negated=!truth();print(selected);print(lazy);print(negated);"#,
            |program| {
                assert!(program.units.iter().any(|unit| unit
                    .data()
                    .operations
                    .iter()
                    .any(|operation| matches!(operation.kind, OperationKind::Select { .. }))));
                assert!(program
                    .units
                    .iter()
                    .any(
                        |unit| unit.data().operations.iter().any(|operation| matches!(
                            operation.kind,
                            OperationKind::ShortCircuit { .. }
                        ))
                    ));
                let mut ledger = ledger(1_000_000);
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                {
                    let mut allocation =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let mut meter = Meter(&mut allocation);
                    let roots = [
                        Subject::Cell(cell(program, "selected")),
                        Subject::Cell(cell(program, "lazy")),
                        Subject::Cell(cell(program, "negated")),
                    ];
                    let proof = DomainProof::build(
                        program,
                        &uses,
                        &roots,
                        DomainInputs::empty(),
                        &SourceRecipes,
                        &mut meter,
                    )
                    .unwrap();
                    assert!(!proof.primitive(roots[0], &mut meter).unwrap());
                    assert!(!proof.primitive(roots[1], &mut meter).unwrap());
                    assert!(proof.primitive(roots[2], &mut meter).unwrap());
                    proof.discard(&mut meter).unwrap();
                }
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            },
        );
    }

    #[test]
    fn a_writer_rhs_edit_invalidates_even_when_cell_use_locations_are_unchanged() {
        checked(
            r#"extern string opaque();string state="safe";void change(){string ignored=opaque();state="next";}string read(){return state;}int unrelated(){return 7;}print(read());"#,
            |program| {
                let mut ledger = ledger(1_000_000);
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                let root = Subject::Cell(cell(program, "state"));
                let mut revised = program.clone();
                let writer = program
                    .units
                    .iter()
                    .find(|unit| {
                        unit.data()
                            .operations
                            .iter()
                            .any(|op| matches!(op.kind, OperationKind::Store(_)))
                    })
                    .unwrap()
                    .id();
                let mut working = revised.units[writer.index()].clone().into_working();
                let body = working.get_mut();
                let opaque = body
                    .operations
                    .iter()
                    .find(|op| matches!(op.kind, OperationKind::Call(_)))
                    .unwrap()
                    .result
                    .unwrap();
                let write = body
                    .operations
                    .iter()
                    .find(|op| matches!(op.kind, OperationKind::Store(_)))
                    .unwrap()
                    .operands
                    .start as usize;
                body.operands[write] = opaque;
                revised.units[writer.index()] = working.freeze();
                revised.verify().unwrap();
                let changed = UseIndex::build(&revised, &mut ledger, WorkDomain::Baseline).unwrap();
                let state = cell(program, "state");
                assert_eq!(
                    uses.cell(state).unwrap().sites(),
                    changed.cell(state).unwrap().sites()
                );
                {
                    let mut allocation =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let mut meter = Meter(&mut allocation);
                    let old = DomainProof::build(
                        program,
                        &uses,
                        &[root],
                        DomainInputs::empty(),
                        &SourceRecipes,
                        &mut meter,
                    )
                    .unwrap();
                    assert!(old.primitive(root, &mut meter).unwrap());
                    assert!(old
                        .dependencies()
                        .units()
                        .iter()
                        .any(|(unit, _)| *unit == writer));
                    // Keeping the old cell-set stamp unchanged cannot hide an
                    // edited producer body: its own unit revision is required.
                    assert!(!old
                        .dependencies()
                        .valid_for(&revised, &uses, &mut meter)
                        .unwrap());
                    assert!(!old
                        .dependencies()
                        .valid_for(&revised, &changed, &mut meter)
                        .unwrap());
                    let unrelated = program
                        .cells
                        .iter()
                        .find(|cell| cell.name == "unrelated")
                        .unwrap();
                    let CellBinding::Function(unrelated) = unrelated.binding else {
                        unreachable!()
                    };
                    assert!(!old
                        .dependencies()
                        .units()
                        .iter()
                        .any(|(unit, _)| *unit == unrelated));
                    let new = DomainProof::build(
                        &revised,
                        &changed,
                        &[root],
                        DomainInputs::empty(),
                        &SourceRecipes,
                        &mut meter,
                    )
                    .unwrap();
                    assert!(!new.primitive(root, &mut meter).unwrap());
                    new.discard(&mut meter).unwrap();
                    old.discard(&mut meter).unwrap();
                    assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
                }
                changed.discard(&mut ledger).unwrap();
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            },
        );
    }

    #[test]
    fn record_layout_evidence_does_not_certify_opaque_payloads() {
        use super::super::record_family::{
            self, FamilyOutcome, FamilyRequest, RECORD_FAMILY_PLAN, RECORD_FAMILY_VERSION,
        };
        checked("extern int opaque();Record<int> state=record{x:opaque(),y:1};auto read=()=>state.x??0;print(read());", |program| {
            let mut ledger=ledger(1_000_000);
            let uses=UseIndex::build(program,&mut ledger,WorkDomain::Baseline).unwrap();
            let state=cell(program,"state");
            let outcome=record_family::analyze(program,&uses,state,FamilyRequest {
                attempt:AnalysisAttempt{plan:RECORD_FAMILY_PLAN,algorithm_version:RECORD_FAMILY_VERSION,work_quota:100_000},scratch_bytes:100_000,output_bytes:100_000,
            },&mut ledger,WorkDomain::Optional).unwrap().outcome;
            let FamilyOutcome::Complete(record)=outcome else {panic!("complete record access family: {outcome:?}")};
            {
                let mut allocation=AllocationBudget::new(Some((&mut ledger,WorkDomain::Optional)));
                let mut meter=Meter(&mut allocation);
                let slot=|key:&str| record.slots().iter().position(|slot|program.strings[slot.key.index()].as_unicode()==Some(key)).unwrap() as u32;
                let roots=[Subject::RecordSlot{state,slot:slot("x")},Subject::RecordSlot{state,slot:slot("y")}];
                let records=[&record];
                let proof=DomainProof::build(program,&uses,&roots,DomainInputs::empty().with_records(&records),&SourceRecipes,&mut meter).unwrap();
                assert!(!proof.primitive(roots[0],&mut meter).unwrap());
                assert!(proof.primitive(roots[1],&mut meter).unwrap());
                proof.discard(&mut meter).unwrap();
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch),0);
            }
            record.discard(&mut ledger).unwrap();uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(),0);
        });
    }

    #[test]
    fn a_table_only_edit_rejects_an_old_index_before_allocating() {
        checked("string value=\"safe\";print(value);", |program| {
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            let mut revised = program.clone();
            Arc::make_mut(&mut revised.types).push(Type::Array(Box::new(Type::Bool)));
            revised.tables_revision = RevisionId::fresh();
            revised.verify().unwrap();
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let result = DomainProof::build(
                    &revised,
                    &uses,
                    &[Subject::Cell(cell(program, "value"))],
                    DomainInputs::empty(),
                    &SourceRecipes,
                    &mut meter,
                );
                assert!(result.is_err());
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }

    #[test]
    fn sparse_growth_and_work_or_memory_denial_release_every_owned_buffer() {
        let mut source = String::from("string value=\"\";");
        for _ in 0..100 {
            source.push_str("value=value+\"x\";");
        }
        source.push_str("print(value);");
        checked(&source, |program| {
            for limit in [0, 20, 100, 500, 1_000_000] {
                let mut ledger = ledger(limit);
                let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
                {
                    let mut allocation =
                        AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                    let mut meter = Meter(&mut allocation);
                    let root = Subject::Cell(cell(program, "value"));
                    let result = DomainProof::build(
                        program,
                        &uses,
                        &[root],
                        DomainInputs::empty(),
                        &SourceRecipes,
                        &mut meter,
                    );
                    if limit == 1_000_000 {
                        let proof = result.unwrap();
                        assert!(proof.primitive(root, &mut meter).unwrap());
                        assert!(proof.nodes.len() > 100);
                        proof.discard(&mut meter).unwrap();
                    } else {
                        assert!(result.is_err(), "work limit {limit}");
                    }
                    assert_eq!(
                        allocation.retained_bytes(AllocationClass::Scratch),
                        0,
                        "work limit {limit}"
                    );
                }
                uses.discard(&mut ledger).unwrap();
                assert_eq!(ledger.retained_bytes(), 0);
            }
            let mut ledger = ledger(1_000_000);
            let uses = UseIndex::build(program, &mut ledger, WorkDomain::Baseline).unwrap();
            // Model other live compiler owners occupying the shared cap. The
            // query must admit its genuine old-plus-new growth with 1 KiB left.
            let held = 10_000_000 - ledger.retained_bytes() - 1024;
            ledger.retain(WorkDomain::Optional, held).unwrap();
            {
                let mut allocation =
                    AllocationBudget::new(Some((&mut ledger, WorkDomain::Optional)));
                let mut meter = Meter(&mut allocation);
                let result = DomainProof::build(
                    program,
                    &uses,
                    &[Subject::Cell(cell(program, "value"))],
                    DomainInputs::empty(),
                    &SourceRecipes,
                    &mut meter,
                );
                assert!(matches!(result, Err(AllocationError::Budget(_))));
                assert_eq!(allocation.retained_bytes(AllocationClass::Scratch), 0);
            }
            ledger.release(WorkDomain::Optional, held).unwrap();
            uses.discard(&mut ledger).unwrap();
            assert_eq!(ledger.retained_bytes(), 0);
        });
    }
}
