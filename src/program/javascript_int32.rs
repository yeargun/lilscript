//! Int32 facts established from producers, not declarations. An `int` cell
//! holds an int32 Number when every value written to it is one: an int
//! constant, an int operation (formation normalizes it or proves its range),
//! a normalized load, intrinsic or host result, or a load of another such
//! cell. A parameter can receive a host value, and so can a classic script's
//! top-level cell, which is a global lexical binding; neither proves anything.
//!
//! A counting loop then bounds its counter. With `c<B` for an int32 `B` and
//! a single `c=c+1` in the loop, the increment reads `c<=2^31-2`, so `c+1`
//! stays in range and its `|0` is redundant; `c>B` with `c=c-1` likewise.
use super::*;

/// What a written value proves about the cell it is written to.
#[derive(Clone, Copy)]
enum Written {
    Int32,
    Cell(CellId),
    Unknown,
}

const UNPROVEN: u8 = 1;
const INT32: u8 = 2;

impl Formation<'_, '_, '_, '_, '_> {
    /// Classify a value's producer, looking through identity copies.
    fn written(&mut self, unit: UnitId, mut value: ValueId) -> Result<Written, FormationError> {
        let program = self.program;
        let data = program.units[unit.index()].data();
        loop {
            self.work(1)?;
            let definition = &data.values[value.index()];
            if !matches!(program.types[definition.ty.index()], Type::Int) {
                return Ok(Written::Unknown);
            }
            let operation = &data.operations[definition.definition.index()];
            return Ok(match operation.kind {
                OperationKind::Constant(Constant::Integer(_))
                | OperationKind::IntBinary(_)
                | OperationKind::Unary { integer: true, .. } => Written::Int32,
                OperationKind::CopyValue => {
                    value = data.operands(operation.operands).unwrap()[0];
                    continue;
                }
                OperationKind::Load(place) => match data.places[place.index()] {
                    Place::Cell(cell) => Written::Cell(cell),
                    _ if matches!(
                        load_result_recipe(program, data, place, definition.ty),
                        LoadResultRecipe::NormalizeInteger
                    ) =>
                    {
                        Written::Int32
                    }
                    _ => Written::Unknown,
                },
                OperationKind::Intrinsic(ResolvedIntrinsic::Property(intrinsic))
                    if js::integer_intrinsic(intrinsic) =>
                {
                    Written::Int32
                }
                OperationKind::Call(_)
                    if matches!(
                        call_result_recipe(program, data, operation),
                        CallResultRecipe::NormalizeInteger | CallResultRecipe::IntrinsicInteger
                    ) =>
                {
                    Written::Int32
                }
                _ => Written::Unknown,
            });
        }
    }

    /// One whole-program pass, on first need: a cell is int32 unless a write
    /// can put another value in it. Unproven cells spread backwards along
    /// cell-to-cell copies until nothing changes.
    fn prove_int32_cells(&mut self) -> Result<(), FormationError> {
        if !self.int32_cells.is_empty() {
            return Ok(());
        }
        let program = self.program;
        let mut status =
            self.budget
                .filled(AllocationClass::Scratch, program.cells.len(), UNPROVEN)?;
        let Some(uses) = self.uses else {
            self.int32_cells = status;
            return Ok(());
        };
        let script =
            self.contract.execution == crate::compilation_contract::JavaScriptExecution::Script;
        // Each function unit's own cell: its parameters are proven through
        // the direct calls that are its only uses.
        let mut function_cells: Vec<Option<CellId>> =
            self.budget
                .filled(AllocationClass::Scratch, program.units.len(), None)?;
        for (index, record) in program.cells.iter().enumerate() {
            self.work(1)?;
            if let CellBinding::Function(unit) = record.binding {
                function_cells[unit.index()] = CellId::from_index(index);
            }
        }
        // (source, dependent): the dependent is int32 only if the source is.
        let mut copies: Vec<(CellId, CellId)> = Vec::new();
        for (index, record) in program.cells.iter().enumerate() {
            self.work(1)?;
            let cell = CellId::from_index(index).unwrap();
            let parameter = match record.binding {
                CellBinding::Local => None,
                CellBinding::Parameter(position) => Some(position),
                _ => continue,
            };
            if !matches!(program.types[record.ty.index()], Type::Int)
                || references::is_reference(program, cell)
                || script
                    && program.unit(record.owner).unwrap().kind == UnitKind::ModuleInitialization
            {
                continue;
            }
            let Some(users) = uses.cell(cell) else {
                continue;
            };
            if users.reference_exposed() {
                continue;
            }
            let mut proven = match parameter {
                Some(position) => self.int32_arguments(
                    record.owner,
                    position,
                    cell,
                    &function_cells,
                    &mut copies,
                )?,
                None => true,
            };
            for site in users.sites() {
                if !proven {
                    break;
                }
                self.work(1)?;
                let (unit, operation) = match *site {
                    CellUseSite::Unit {
                        unit,
                        usage: CellUse::Initialize(operation) | CellUse::Write { operation, .. },
                    } => (unit, operation),
                    CellUseSite::Unit {
                        usage: CellUse::Read { .. } | CellUse::Capture,
                        ..
                    } => continue,
                    // The parameter's arguments were checked above.
                    CellUseSite::Unit {
                        usage: CellUse::Parameter(_),
                        ..
                    } if parameter.is_some() => continue,
                    _ => {
                        proven = false;
                        break;
                    }
                };
                let data = program.units[unit.index()].data();
                let Some(&value) = data
                    .operands(data.operations[operation.index()].operands)
                    .and_then(|values| values.first())
                else {
                    proven = false;
                    break;
                };
                match self.written(unit, value)? {
                    Written::Int32 => {}
                    Written::Cell(source) => {
                        self.budget
                            .push(AllocationClass::Scratch, &mut copies, (source, cell))?;
                    }
                    Written::Unknown => {
                        proven = false;
                        break;
                    }
                }
            }
            if proven {
                status[index] = INT32;
            }
        }
        // A copy from an unproven cell unproves its destination.
        self.work(copies.len())?;
        copies.sort_unstable();
        let mut pending: Vec<CellId> = Vec::new();
        for (index, &state) in status.iter().enumerate() {
            if state != INT32 {
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut pending,
                    CellId::from_index(index).unwrap(),
                )?;
            }
        }
        while let Some(source) = pending.pop() {
            let start = copies.partition_point(|&(from, _)| from < source);
            for &(from, dependent) in &copies[start..] {
                self.work(1)?;
                if from != source {
                    break;
                }
                if status[dependent.index()] == INT32 {
                    status[dependent.index()] = UNPROVEN;
                    self.budget
                        .push(AllocationClass::Scratch, &mut pending, dependent)?;
                }
            }
        }
        self.int32_cells = status;
        Ok(())
    }

    /// Whether every call of a directly-called-only function passes an int32
    /// at `position`; argument cells become copies into the parameter.
    fn int32_arguments(
        &mut self,
        owner: UnitId,
        position: u32,
        parameter: CellId,
        function_cells: &[Option<CellId>],
        copies: &mut Vec<(CellId, CellId)>,
    ) -> Result<bool, FormationError> {
        let program = self.program;
        let Some(uses) = self.uses else {
            return Ok(false);
        };
        let Some(function) = function_cells[owner.index()] else {
            return Ok(false);
        };
        if !self.callee_only_cell(function)? {
            return Ok(false);
        }
        let Some(users) = uses.cell(function) else {
            return Ok(false);
        };
        for site in users.sites() {
            self.work(1)?;
            let CellUseSite::Unit {
                unit: caller,
                usage: CellUse::Read { operation, .. },
            } = *site
            else {
                continue;
            };
            let data = program.units[caller.index()].data();
            let Some(loaded) = data.operations[operation.index()].result else {
                return Ok(false);
            };
            let Some(readers) = uses.unit(caller).and_then(|uses| uses.value_uses(loaded)) else {
                return Ok(false);
            };
            for reader in readers {
                self.work(1)?;
                let ValueUse::CallCallee { call, .. } = *reader else {
                    return Ok(false);
                };
                let Some(CallArgument::Value(argument)) = data
                    .arguments(data.calls[call.index()].arguments)
                    .and_then(|arguments| arguments.get(position as usize))
                    .copied()
                else {
                    return Ok(false);
                };
                match self.written(caller, argument)? {
                    Written::Int32 => {}
                    Written::Cell(source) => {
                        self.budget
                            .push(AllocationClass::Scratch, copies, (source, parameter))?;
                    }
                    Written::Unknown => return Ok(false),
                }
            }
        }
        Ok(true)
    }

    /// Whether a value's JavaScript result is always an int32 Number.
    fn int32_value(&mut self, unit: UnitId, value: ValueId) -> Result<bool, FormationError> {
        Ok(match self.written(unit, value)? {
            Written::Int32 => true,
            Written::Cell(cell) => {
                self.prove_int32_cells()?;
                self.int32_cells[cell.index()] == INT32
            }
            Written::Unknown => false,
        })
    }

    /// The largest and smallest values a proven-int32 bound can take. Under
    /// pristine builtins a string or array length is at most 2^30, the V8 and
    /// SpiderMonkey limits; a host `length` qualifies under numeric lengths.
    fn bound_range(
        &mut self,
        unit: UnitId,
        value: ValueId,
    ) -> Result<Option<(i64, i64)>, FormationError> {
        const LENGTH: i64 = 1 << 30;
        let program = self.program;
        let data = program.units[unit.index()].data();
        let operation = &data.operations[data.values[value.index()].definition.index()];
        let length = match operation.kind {
            OperationKind::Constant(Constant::Integer(constant)) => {
                return Ok(Some((i64::from(constant), i64::from(constant))))
            }
            OperationKind::Intrinsic(ResolvedIntrinsic::Property(
                Intrinsic::StringLength | Intrinsic::ArrayLength,
            )) => self.contract.assumptions.pristine_builtins,
            OperationKind::Call(call) => {
                self.contract.assumptions.numeric_lengths && self.host_length(data, call)
            }
            _ => false,
        };
        if length {
            return Ok(Some((0, LENGTH)));
        }
        Ok(self
            .int32_value(unit, value)?
            .then_some((i32::MIN as i64, i32::MAX as i64)))
    }

    /// `JS.number(value.length)` converted to an int: a host length.
    fn host_length(&self, data: &UnitData, call: CallId) -> bool {
        let CallTarget::Intrinsic {
            operation: ResolvedIntrinsic::Method(Intrinsic::FloatToInt),
            receiver: Some(number),
        } = data.calls[call.index()].target
        else {
            return false;
        };
        let OperationKind::Call(inner) =
            data.operations[data.values[number.index()].definition.index()].kind
        else {
            return false;
        };
        let (CallTarget::Builtin(BuiltinCall::JsNumber), Some(&[CallArgument::Value(argument)])) = (
            &data.calls[inner.index()].target,
            data.arguments(data.calls[inner.index()].arguments),
        ) else {
            return false;
        };
        let OperationKind::Load(place) =
            data.operations[data.values[argument.index()].definition.index()].kind
        else {
            return false;
        };
        matches!(data.places[place.index()], Place::Member { key, .. }
            if self.program.strings[key.index()].as_unicode() == Some("length"))
    }

    /// Record the bound a counting loop puts on each counter it steps.
    pub(super) fn counter_facts(
        &mut self,
        context: ContextId,
        numbers: &mut [NumberFacts],
    ) -> Result<(), FormationError> {
        if self.uses.is_none() {
            return Ok(());
        }
        let unit = self.semantic(context);
        let program = self.program;
        let data = program.units[unit.index()].data();
        let cell_of = |value: ValueId| match data.operations
            [data.values[value.index()].definition.index()]
        .kind
        {
            OperationKind::Load(place) => match data.places[place.index()] {
                Place::Cell(cell) => Some(cell),
                _ => None,
            },
            _ => None,
        };
        let mut steps = Vec::new();
        for operation in &data.operations {
            self.work(1)?;
            let OperationKind::Loop { test, body, update } = operation.kind else {
                continue;
            };
            let Some(condition) = data.regions[test.index()].result else {
                continue;
            };
            let compare = &data.operations[data.values[condition.index()].definition.index()];
            let (&OperationKind::Binary(op), Some(&[left, right])) =
                (&compare.kind, data.operands(compare.operands))
            else {
                continue;
            };
            // `counter < bound` or `counter <= bound` bounds it above;
            // `counter > bound` or `counter >= bound` bounds it below.
            let (upper, strict) = match op {
                BinaryOp::Less => (true, true),
                BinaryOp::LessEq => (true, false),
                BinaryOp::Greater => (false, true),
                BinaryOp::GreaterEq => (false, false),
                _ => continue,
            };
            let mut candidates = [None, None];
            if let Some(cell) = cell_of(left) {
                candidates[0] = Some((cell, right, upper));
            }
            if let Some(cell) = cell_of(right) {
                candidates[1] = Some((cell, left, !upper));
            }
            for (counter, bound, upper) in candidates.into_iter().flatten() {
                steps.clear();
                let Some(facts) = self.counter_steps(
                    unit, counter, bound, upper, strict, test, body, update, &mut steps,
                )?
                else {
                    continue;
                };
                for &old in &steps {
                    numbers[old.index()] = facts;
                }
                break;
            }
        }
        Ok(())
    }

    /// The counter reads feeding each step of a counting loop, with the range
    /// they read: every write to the counter in the loop is one step toward
    /// the bound, in its body or update, and the bound leaves room for the
    /// most steps any one iteration can take.
    #[allow(clippy::too_many_arguments)]
    fn counter_steps(
        &mut self,
        unit: UnitId,
        counter: CellId,
        bound: ValueId,
        upper: bool,
        strict: bool,
        test: RegionId,
        body: RegionId,
        update: RegionId,
        steps: &mut Vec<ValueId>,
    ) -> Result<Option<NumberFacts>, FormationError> {
        let program = self.program;
        let data = program.units[unit.index()].data();
        self.prove_int32_cells()?;
        if self.int32_cells[counter.index()] != INT32 {
            return Ok(None);
        }
        let Some((minimum, maximum)) = self.bound_range(unit, bound)? else {
            return Ok(None);
        };
        let Some(users) = self.uses.and_then(|uses| uses.cell(counter)) else {
            return Ok(None);
        };
        let expected = if upper {
            IntBinary::Add
        } else {
            IntBinary::Subtract
        };
        let mut writes = Vec::new();
        for site in users.sites() {
            self.work(1)?;
            let CellUseSite::Unit {
                unit: writer,
                usage: CellUse::Write { operation, .. },
            } = *site
            else {
                continue;
            };
            if writer != unit {
                return Ok(None);
            }
            // Writes outside the loop set the counter before it runs.
            let mut region = Some(data.operations[operation.index()].region);
            let mut inside = false;
            while let Some(at) = region {
                self.work(1)?;
                if at == test {
                    return Ok(None);
                }
                if at == body || at == update {
                    inside = true;
                    break;
                }
                region = data.regions[at.index()].parent;
            }
            if !inside {
                continue;
            }
            let Some(&[value]) = data.operands(data.operations[operation.index()].operands) else {
                return Ok(None);
            };
            let increment = &data.operations[data.values[value.index()].definition.index()];
            let (&OperationKind::IntBinary(found), Some(&[old, one])) =
                (&increment.kind, data.operands(increment.operands))
            else {
                return Ok(None);
            };
            let reads_counter = matches!(
                data.operations[data.values[old.index()].definition.index()].kind,
                OperationKind::Load(place) if data.places[place.index()] == Place::Cell(counter)
            );
            if found != expected
                || !reads_counter
                || !matches!(
                    data.operations[data.values[one.index()].definition.index()].kind,
                    OperationKind::Constant(Constant::Integer(1))
                )
            {
                return Ok(None);
            }
            self.budget
                .push(AllocationClass::Scratch, &mut writes, operation)?;
            self.budget.push(AllocationClass::Scratch, steps, old)?;
        }
        if steps.is_empty() {
            return Ok(None);
        }
        // The most steps one iteration can take: exclusive branches count
        // once, and a step repeated by a nested loop is unbounded.
        let Some(body_steps) = self.max_steps(data, body, &writes)? else {
            return Ok(None);
        };
        let Some(update_steps) = self.max_steps(data, update, &writes)? else {
            return Ok(None);
        };
        let taken = (body_steps + update_steps) as i64;
        // Before its j-th step (from 0) an iteration's counter is within
        // j of the bound, one inside it when the test is strict.
        let slack = taken - 1 + i64::from(!strict);
        let facts = if upper {
            let highest = maximum - i64::from(strict) + slack;
            if highest + 1 > i32::MAX as i64 {
                return Ok(None);
            }
            NumberFacts::integer_range(i32::MIN as i64, highest, false)
        } else {
            let lowest = minimum + i64::from(strict) - slack;
            if lowest - 1 < i32::MIN as i64 {
                return Ok(None);
            }
            NumberFacts::integer_range(lowest, i32::MAX as i64, false)
        };
        Ok(facts)
    }

    /// The most counter steps a single pass through `region` can execute,
    /// or `None` when a step sits inside a nested loop.
    fn max_steps(
        &mut self,
        data: &UnitData,
        region: RegionId,
        writes: &[OpId],
    ) -> Result<Option<u32>, FormationError> {
        let mut total = 0u32;
        for &operation in &data.regions[region.index()].operations {
            self.work(1)?;
            if writes.contains(&operation) {
                total += 1;
                continue;
            }
            let kind = &data.operations[operation.index()].kind;
            let mut children = kind.child_regions().peekable();
            if children.peek().is_none() {
                continue;
            }
            let steps = match *kind {
                OperationKind::If { yes, no } => {
                    let Some(yes) = self.max_steps(data, yes, writes)? else {
                        return Ok(None);
                    };
                    let no = match no {
                        Some(no) => match self.max_steps(data, no, writes)? {
                            Some(no) => no,
                            None => return Ok(None),
                        },
                        None => 0,
                    };
                    yes.max(no)
                }
                OperationKind::Select { yes, no } => {
                    let (Some(yes), Some(no)) = (
                        self.max_steps(data, yes, writes)?,
                        self.max_steps(data, no, writes)?,
                    ) else {
                        return Ok(None);
                    };
                    yes.max(no)
                }
                OperationKind::Loop { .. }
                | OperationKind::ForIn { .. }
                | OperationKind::ForOf { .. } => {
                    for child in kind.child_regions() {
                        if self.max_steps(data, child, writes)? != Some(0) {
                            return Ok(None);
                        }
                    }
                    0
                }
                _ => {
                    let mut sum = 0;
                    for child in kind.child_regions() {
                        match self.max_steps(data, child, writes)? {
                            Some(steps) => sum += steps,
                            None => return Ok(None),
                        }
                    }
                    sum
                }
            };
            total += steps;
        }
        Ok(Some(total))
    }
}
