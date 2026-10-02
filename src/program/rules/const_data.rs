//! Mandatory, bounded execution of declared compile-time data. The interpreter
//! reads the same typed operations and exact scalar evaluator as optimization.
//! It owns a private heap; only that heap may be mutated during construction.
//! Host calls, suspension, implementation-dependent math and incomplete results
//! are diagnosed, never replaced with guessed values or runtime initialization.
use super::super::call_graph::Callee;
use super::super::facts::{
    self, StoredExact as Exact, StoredKnowledge as Knowledge, StoredString, Work,
};
use super::edit::{self, Editor};
use super::*;
use crate::output_budget::{
    AllocationBudget,
    AllocationClass::{Retained, Scratch},
    AllocationError,
};
use crate::primitive::Intrinsic;
use crate::span::Span;

#[derive(Debug)]
pub(crate) struct Error {
    pub module: ModuleId,
    pub span: Span,
    pub message: String,
    pub allocation: Option<AllocationError>,
}
impl From<AllocationError> for Error {
    fn from(error: AllocationError) -> Self {
        Self {
            module: ModuleId::from_index(0).unwrap(),
            span: Span::default(),
            message: "const evaluation resource limit".into(),
            allocation: Some(error),
        }
    }
}

#[derive(Clone, Debug)]
enum Key {
    Index(usize),
    Named(StringId),
    Text(std::sync::Arc<crate::literal::StringValue>),
    Field(NominalMemberId),
    Class(FieldRef),
}
#[derive(Clone, Debug)]
enum Data {
    Scalar(Exact),
    Heap(usize),
    /// Preserve identity when another const supplies an aggregate or subtree.
    Static(CellId, std::sync::Arc<Vec<Key>>),
    Function(UnitId),
}
#[derive(Clone, Debug)]
struct Value {
    ty: TypeId,
    data: Data,
}
#[derive(Clone)]
struct Heap {
    kind: AllocationKind,
    keys: Option<Vec<crate::literal::StringValue>>,
    values: Vec<Value>,
}
struct Frame {
    unit: UnitId,
    values: Vec<Option<Value>>,
    primitives: Vec<Knowledge>,
    cells: Vec<(CellId, Value)>,
    written: Vec<ValueId>,
}
impl Frame {
    fn reset(&mut self) {
        for id in self.written.drain(..) {
            self.values[id.index()] = None;
            self.primitives[id.index()] = Knowledge::Unknown(facts::UnknownReason::Unvisited);
        }
        self.cells.clear();
    }
}
enum Flow {
    Next,
    Return(Option<Value>),
    Break,
    Continue,
}
struct Interpreter<'p, 'src, 'b, 'ledger> {
    program: &'p Program<'src>,
    effects: &'p ProgramEffects,
    budget: &'b mut AllocationBudget<'ledger>,
    limits: crate::config::ConstEvaluation,
    steps: u32,
    bytes: u64,
    heap: Vec<Heap>,
    constants: Vec<Option<Value>>,
    initializers: Vec<Option<(UnitId, OpId)>>,
    root: (UnitId, OpId),
    span: Span,
    module: ModuleId,
}
impl Interpreter<'_, '_, '_, '_> {
    fn fail(&self, message: impl Into<String>) -> Error {
        Error {
            module: self.module,
            span: self.span,
            message: message.into(),
            allocation: None,
        }
    }
    fn resource(&self, allocation: AllocationError) -> Error {
        Error {
            module: self.module,
            span: self.span,
            message: "const evaluation resource limit".into(),
            allocation: Some(allocation),
        }
    }
    fn tick(&mut self, count: usize) -> Result<(), Error> {
        self.steps = self
            .steps
            .checked_sub(u32::try_from(count).unwrap_or(u32::MAX))
            .ok_or_else(|| {
                self.fail("const evaluation exhausted language.const_evaluation.steps")
            })?;
        self.budget
            .work(crate::compilation_policy::WorkKind::Analysis, count as u64)
            .map_err(|e| self.resource(e))
    }
    fn memory(&mut self, bytes: usize) -> Result<(), Error> {
        self.bytes = self
            .bytes
            .checked_add(bytes as u64)
            .ok_or_else(|| self.fail("const evaluation storage overflow"))?;
        if self.bytes > self.limits.bytes {
            return Err(self.fail("const evaluation exhausted language.const_evaluation.bytes"));
        }
        self.budget
            .retain(Retained, bytes as u64)
            .map_err(|e| self.resource(e))?;
        Ok(())
    }
    fn frame(&mut self, unit: UnitId) -> Result<Frame, Error> {
        let n = self.program.unit(unit).unwrap().values.len();
        self.memory(
            n.checked_mul(
                std::mem::size_of::<Option<Value>>()
                    + std::mem::size_of::<Knowledge>()
                    + 2 * std::mem::size_of::<ValueId>(),
            )
            .ok_or_else(|| self.fail("const frame capacity"))?,
        )?;
        Ok(Frame {
            unit,
            values: vec![None; n],
            primitives: vec![Knowledge::Unknown(facts::UnknownReason::Unvisited); n],
            cells: Vec::new(),
            written: Vec::new(),
        })
    }
    fn value(&self, frame: &Frame, id: ValueId) -> Result<Value, Error> {
        frame.values[id.index()]
            .clone()
            .ok_or_else(|| self.fail("const expression depends on a runtime value"))
    }
    fn scalar(&self, value: &Value) -> Result<Exact, Error> {
        match &value.data {
            Data::Scalar(value) => Ok(value.clone()),
            _ => Err(self.fail("const operation requires an exact primitive")),
        }
    }
    fn truth(&self, value: &Value) -> bool {
        match &value.data {
            Data::Scalar(value) => facts::truthy(self.program, value),
            _ => true,
        }
    }
    fn cell(&mut self, frame: &Frame, cell: CellId) -> Result<Value, Error> {
        self.tick(frame.cells.len() + 1)?;
        if let Some((_, value)) = frame.cells.iter().rev().find(|(known, _)| *known == cell) {
            return Ok(value.clone());
        }
        let declaration = &self.program.cells[cell.index()];
        if let CellBinding::Function(body) = declaration.binding {
            return Ok(Value {
                ty: declaration.ty,
                data: Data::Function(body),
            });
        }
        if !declaration.declared_const {
            return Err(self.fail("const evaluation cannot read mutable or host storage"));
        }
        let lexical_before = self.initializers[cell.index()].is_some_and(|(owner, initialized)| {
            if owner == self.root.0 && initialized.index() < self.root.1.index() {
                let mut current =
                    self.program.unit(self.root.0).unwrap().operations[self.root.1.index()].region;
                loop {
                    if current == declaration.region {
                        return true;
                    }
                    match self.program.unit(owner).unwrap().regions[current.index()].parent {
                        Some(parent) => current = parent,
                        None => break,
                    }
                }
            }
            let order = &self.program.initialization;
            declaration.region == self.program.unit(owner).unwrap().entry
                && order
                    .iter()
                    .position(|unit| *unit == owner)
                    .zip(order.iter().position(|unit| *unit == self.root.0))
                    .is_some_and(|(before, after)| before < after)
        });
        if !lexical_before
            && !self.effects.initialization().initialized(
                self.program,
                self.root.0,
                self.root.1,
                cell,
            )
        {
            return Err(
                self.fail("const data must be initialized before another declaration reads it")
            );
        }
        let value = self.constants[cell.index()]
            .clone()
            .ok_or_else(|| self.fail("const data dependency has not completed exact evaluation"))?;
        Ok(match value.data {
            Data::Scalar(_) => value,
            _ => Value {
                ty: value.ty,
                data: Data::Static(cell, std::sync::Arc::new(Vec::new())),
            },
        })
    }
    fn put(&mut self, frame: &mut Frame, cell: CellId, value: Value) -> Result<(), Error> {
        self.tick(frame.cells.len() + 1)?;
        if let Some((_, into)) = frame.cells.iter_mut().find(|(known, _)| *known == cell) {
            *into = value;
        } else {
            self.memory(2 * std::mem::size_of::<(CellId, Value)>())?;
            frame.cells.push((cell, value));
        }
        Ok(())
    }
    fn resolved(&self, value: &Value) -> Result<Value, Error> {
        self.resolved_at(value, 0)
    }
    fn resolved_at(&self, value: &Value, depth: u16) -> Result<Value, Error> {
        let Data::Static(cell, keys) = &value.data else {
            return Ok(value.clone());
        };
        if depth >= self.limits.depth {
            return Err(self.fail("const data dependency exceeds language.const_evaluation.depth"));
        }
        let stored = self.constants[cell.index()]
            .as_ref()
            .ok_or_else(|| self.fail("missing const dependency"))?;
        let mut result = self.resolved_at(stored, depth + 1)?;
        for key in keys.iter() {
            let Data::Heap(id) = result.data else {
                return Err(self.fail("const projection requires data"));
            };
            let index = self.position(&self.heap[id], key)?;
            result = self.resolved_at(
                self.heap[id]
                    .values
                    .get(index)
                    .ok_or_else(|| self.fail("const data index is outside its bounds"))?,
                depth + 1,
            )?;
        }
        Ok(result)
    }
    fn key_text(&self, key: &Key) -> Option<crate::literal::StringValue> {
        match key {
            Key::Named(key) => Some(self.program.strings[key.index()].clone()),
            Key::Text(text) => Some(text.as_ref().clone()),
            Key::Index(index) => Some(crate::literal::StringValue::from_utf16(
                index.to_string().encode_utf16().collect(),
            )),
            _ => None,
        }
    }
    fn position(&self, heap: &Heap, key: &Key) -> Result<usize, Error> {
        match (&heap.kind, key) {
            (AllocationKind::Array, Key::Index(index)) => Ok(*index),
            (AllocationKind::Struct(_), Key::Field(field)) => self
                .program
                .fields
                .iter()
                .find(|known| known.identity == *field)
                .map(|field| field.index)
                .ok_or_else(|| self.fail("const struct has no such field")),
            (AllocationKind::Instance { .. }, Key::Class(field)) => Ok(field.slot as usize),
            _ if heap.keys.is_some() => {
                let text = self
                    .key_text(key)
                    .ok_or_else(|| self.fail("const key is not a string"))?;
                heap.keys
                    .as_ref()
                    .unwrap()
                    .iter()
                    .position(|known| known == &text)
                    .ok_or_else(|| self.fail("const object has no such key"))
            }
            _ => Err(self.fail("unsupported const data projection")),
        }
    }
    fn project(&mut self, value: &Value, key: &Key, ty: TypeId) -> Result<Value, Error> {
        let resolved = self.resolved(value)?;
        let Data::Heap(id) = resolved.data else {
            return Err(self.fail("const projection requires data"));
        };
        let heap = &self.heap[id];
        let index = self.position(heap, key)?;
        let result = heap
            .values
            .get(index)
            .cloned()
            .ok_or_else(|| self.fail("const data index is outside its bounds"))?;
        if let Data::Static(cell, path) = &value.data {
            if !matches!(result.data, Data::Scalar(_)) {
                self.memory((path.len() + 1) * 2 * std::mem::size_of::<Key>() + 32)?;
                let mut path = path.as_ref().clone();
                path.push(key.clone());
                return Ok(Value {
                    ty,
                    data: Data::Static(*cell, std::sync::Arc::new(path)),
                });
            }
        }
        Ok(result)
    }
    fn key(&self, value: &Value) -> Result<Key, Error> {
        match self.scalar(value)? {
            Exact::Integer(index) if index >= 0 => Ok(Key::Index(index as usize)),
            Exact::String(StoredString::Source(id)) => Ok(Key::Named(id)),
            Exact::String(StoredString::Computed(value)) => Ok(Key::Text(value)),
            _ => Err(self.fail("const data key must be a string or a nonnegative integer")),
        }
    }
    fn place(&mut self, frame: &Frame, place: PlaceId, ty: TypeId) -> Result<Value, Error> {
        let data = self.program.unit(frame.unit).unwrap();
        match data.places[place.index()] {
            Place::Cell(cell) => self.cell(frame, cell),
            Place::Value(value) => self.value(frame, value),
            Place::Field { base, field } => {
                let value = self.place(frame, base, ty)?;
                self.project(&value, &Key::Field(field), ty)
            }
            Place::ClassField { receiver, field } => {
                let value = self.value(frame, receiver)?;
                self.project(&value, &Key::Class(field), ty)
            }
            Place::Member { receiver, key } => {
                let value = self.value(frame, receiver)?;
                self.project(&value, &Key::Named(key), ty)
            }
            Place::Index { receiver, key } => {
                let value = self.value(frame, receiver)?;
                let key = self.key(&self.value(frame, key)?)?;
                self.project(&value, &key, ty)
            }
        }
    }
    fn store(&mut self, frame: &mut Frame, place: PlaceId, value: Value) -> Result<(), Error> {
        let data = self.program.unit(frame.unit).unwrap();
        let (target, key) = match data.places[place.index()] {
            Place::Cell(cell)
                if self.program.cells[cell.index()].owner == frame.unit
                    && !self.program.cells[cell.index()].declared_const =>
            {
                return self.put(frame, cell, value)
            }
            Place::Field { base, field } => (self.place(frame, base, value.ty)?, Key::Field(field)),
            Place::ClassField { receiver, field } => {
                (self.value(frame, receiver)?, Key::Class(field))
            }
            Place::Member { receiver, key } => (self.value(frame, receiver)?, Key::Named(key)),
            Place::Index { receiver, key } => (
                self.value(frame, receiver)?,
                self.key(&self.value(frame, key)?)?,
            ),
            _ => {
                return Err(self.fail("const evaluation cannot write nonlocal or immutable storage"))
            }
        };
        let Data::Heap(id) = target.data else {
            return Err(self.fail("const evaluation cannot mutate a previously declared const"));
        };
        let index = match self.position(&self.heap[id], &key) {
            Ok(index) => index,
            Err(_) if matches!(self.heap[id].kind, AllocationKind::Record(_)) => {
                let text = self
                    .key_text(&key)
                    .ok_or_else(|| self.fail("const record key"))?;
                self.memory(2 * std::mem::size_of::<Value>() + text.code_units().count() * 2 + 64)?;
                self.heap[id].keys.as_mut().unwrap().push(text);
                self.heap[id].values.push(value);
                return Ok(());
            }
            Err(error) => return Err(error),
        };
        if index >= self.heap[id].values.len() {
            return Err(self.fail("const store is outside the constructed data"));
        }
        self.heap[id].values[index] = value;
        Ok(())
    }
    fn primitive(&mut self, frame: &Frame, operation: &Operation) -> Result<Value, Error> {
        let mut work = Work::admitted(
            u64::from(self.steps),
            self.limits.bytes.saturating_sub(self.bytes),
            self.budget.scope(),
        );
        let answer = facts::exact(
            self.program,
            self.program.unit(frame.unit).unwrap(),
            operation,
            &frame.primitives,
            &mut work,
        );
        let exhausted = work.truncated();
        let used = work.used();
        work.finish().map_err(|e| self.resource(e))?;
        self.steps = self
            .steps
            .saturating_sub(u32::try_from(used).unwrap_or(u32::MAX));
        let Knowledge::Exact(value) = answer else {
            return Err(self.fail(if exhausted {
                "const exact scalar evaluation exceeded its bound"
            } else {
                "const operation is not exact or supported"
            }));
        };
        if let Exact::String(StoredString::Computed(value)) = &value {
            self.memory(value.code_units().count().saturating_mul(2))?;
        }
        Ok(Value {
            ty: self.program.unit(frame.unit).unwrap().values[operation.result.unwrap().index()].ty,
            data: Data::Scalar(value),
        })
    }
    fn intrinsic(
        &mut self,
        operation: ResolvedIntrinsic,
        receiver: Option<Value>,
        args: &[Value],
        ty: TypeId,
    ) -> Result<Value, Error> {
        let (ResolvedIntrinsic::Method(method)
        | ResolvedIntrinsic::Property(method)
        | ResolvedIntrinsic::Constructor(method)) = operation;
        if method == Intrinsic::StringLength {
            let value = self.scalar(
                receiver
                    .as_ref()
                    .ok_or_else(|| self.fail("const string receiver"))?,
            )?;
            let length = match value {
                Exact::String(StoredString::Source(id)) => {
                    self.program.strings[id.index()].code_units().count()
                }
                Exact::String(StoredString::Computed(value)) => value.code_units().count(),
                _ => return Err(self.fail("const string length requires a string")),
            };
            self.tick(length)?;
            return Ok(Value {
                ty,
                data: Data::Scalar(Exact::Integer(
                    i32::try_from(length)
                        .map_err(|_| self.fail("const string length exceeds int32"))?,
                )),
            });
        }
        if method == Intrinsic::ArrayLength {
            let value = self.resolved(
                receiver
                    .as_ref()
                    .ok_or_else(|| self.fail("const array receiver"))?,
            )?;
            let Data::Heap(id) = value.data else {
                return Err(self.fail("const array length requires an array"));
            };
            if !matches!(self.heap[id].kind, AllocationKind::Array) {
                return Err(self.fail("const array length receiver"));
            }
            return Ok(Value {
                ty,
                data: Data::Scalar(Exact::Integer(self.heap[id].values.len() as i32)),
            });
        }
        if matches!(method, Intrinsic::ArrayPush | Intrinsic::ArrayPop) {
            let Data::Heap(id) = receiver
                .ok_or_else(|| self.fail("const array receiver"))?
                .data
            else {
                return Err(self.fail("const construction cannot mutate declared const data"));
            };
            if !matches!(self.heap[id].kind, AllocationKind::Array) {
                return Err(self.fail("const array mutation receiver"));
            }
            if method == Intrinsic::ArrayPush {
                self.memory(args.len().saturating_mul(2 * std::mem::size_of::<Value>()))?;
                self.heap[id].values.extend_from_slice(args);
                return Ok(Value {
                    ty,
                    data: Data::Scalar(Exact::Integer(self.heap[id].values.len() as i32)),
                });
            }
            return self.heap[id]
                .values
                .pop()
                .ok_or_else(|| self.fail("const pop of an empty array is unsupported"));
        }
        if let Some(reason) = super::evaluate::intrinsic_refusal(operation) {
            return Err(self.fail(format!("const intrinsic is not exact: {reason:?}")));
        }
        let receiver = receiver
            .as_ref()
            .map(|value| self.scalar(value))
            .transpose()?;
        let args: Vec<_> = args
            .iter()
            .map(|value| self.scalar(value))
            .collect::<Result<_, _>>()?;
        let mut work = Work::admitted(
            u64::from(self.steps),
            self.limits.bytes.saturating_sub(self.bytes),
            self.budget.scope(),
        );
        let answer = super::evaluate::intrinsic(
            self.program,
            operation,
            receiver.as_ref(),
            &args,
            &mut work,
        );
        let used = work.used();
        work.finish().map_err(|e| self.resource(e))?;
        self.steps = self
            .steps
            .saturating_sub(u32::try_from(used).unwrap_or(u32::MAX));
        let answer = answer.ok_or_else(|| {
            self.fail("const intrinsic cannot be evaluated exactly within the configured bound")
        })?;
        if let Exact::String(StoredString::Computed(value)) = &answer {
            self.memory(value.code_units().count().saturating_mul(2))?;
        }
        Ok(Value {
            ty,
            data: Data::Scalar(answer),
        })
    }
    fn copy_value(&mut self, value: Value, depth: u16) -> Result<Value, Error> {
        if !matches!(
            self.program.types[value.ty.index()],
            Type::Struct(_) | Type::StructInstance { .. }
        ) {
            return Ok(value);
        }
        if depth >= self.limits.depth {
            return Err(self.fail("const value copy exceeds language.const_evaluation.depth"));
        }
        let resolved = self.resolved(&value)?;
        let Data::Heap(id) = resolved.data else {
            return Err(self.fail("const value product has no exact representation"));
        };
        self.tick(self.heap[id].values.len() + 1)?;
        self.memory(
            self.heap[id].values.len() * 2 * std::mem::size_of::<Value>()
                + 2 * std::mem::size_of::<Heap>(),
        )?;
        let mut copy = self.heap[id].clone();
        for field in &mut copy.values {
            *field = self.copy_value(field.clone(), depth + 1)?;
        }
        let id = self.heap.len();
        self.heap.push(copy);
        Ok(Value {
            ty: value.ty,
            data: Data::Heap(id),
        })
    }
    fn call(
        &mut self,
        frame: &Frame,
        call: CallId,
        ty: TypeId,
        depth: u16,
    ) -> Result<Value, Error> {
        let data = self.program.unit(frame.unit).unwrap();
        let site = &data.calls[call.index()];
        self.memory(site.arguments.len as usize * 2 * std::mem::size_of::<Value>())?;
        let args =
            data.arguments(site.arguments)
                .unwrap()
                .iter()
                .map(|arg| match arg {
                    CallArgument::Value(id) => self.value(frame, *id),
                    _ => Err(self
                        .fail("const calls do not accept mutable-reference or spread arguments")),
                })
                .collect::<Result<Vec<_>, _>>()?;
        match self.effects.graph().callee(frame.unit, call) {
            Callee::Unit(unit) => {
                if depth >= self.limits.depth {
                    return Err(
                        self.fail("const evaluation exhausted language.const_evaluation.depth")
                    );
                }
                let mut target = self.frame(unit)?;
                let body = self.program.unit(unit).unwrap();
                if body.suspension != Suspension::None
                    || body.constructor_of.is_some()
                    || args.len() > body.parameters.len()
                {
                    return Err(self.fail("unsupported const callable convention"));
                }
                for (i, &cell) in body.parameters.iter().enumerate() {
                    let value = args.get(i).cloned().unwrap_or(Value {
                        ty: self.program.cells[cell.index()].ty,
                        data: Data::Scalar(Exact::Undefined),
                    });
                    self.put(&mut target, cell, value)?;
                }
                match self.region(&mut target, body.entry, depth + 1)? {
                    Flow::Return(Some(value)) => Ok(value),
                    Flow::Return(None) | Flow::Next => Ok(Value {
                        ty,
                        data: Data::Scalar(Exact::Undefined),
                    }),
                    _ => Err(self.fail("invalid const function control flow")),
                }
            }
            Callee::Intrinsic(operation) => {
                let receiver = match site.target {
                    CallTarget::Intrinsic { receiver, .. } => {
                        receiver.map(|id| self.value(frame, id)).transpose()?
                    }
                    _ => None,
                };
                self.intrinsic(operation, receiver, &args, ty)
            }
            Callee::Builtin(operation) if super::evaluate::builtin_primitive(operation) => {
                let args: Vec<_> = args
                    .iter()
                    .map(|value| self.scalar(value))
                    .collect::<Result<_, _>>()?;
                let mut work = Work::admitted(
                    u64::from(self.steps),
                    self.limits.bytes.saturating_sub(self.bytes),
                    self.budget.scope(),
                );
                let answer = super::evaluate::builtin(self.program, operation, &args, &mut work);
                let used = work.used();
                work.finish().map_err(|e| self.resource(e))?;
                self.steps = self
                    .steps
                    .saturating_sub(u32::try_from(used).unwrap_or(u32::MAX));
                Ok(Value {
                    ty,
                    data: Data::Scalar(
                        answer.ok_or_else(|| self.fail("const builtin is not exact"))?,
                    ),
                })
            }
            _ => Err(self.fail("const evaluation cannot invoke host or unresolved code")),
        }
    }
    fn region(&mut self, frame: &mut Frame, region: RegionId, depth: u16) -> Result<Flow, Error> {
        if depth >= self.limits.depth {
            return Err(self.fail("const evaluation exhausted language.const_evaluation.depth"));
        }
        let data = self.program.unit(frame.unit).unwrap();
        for &id in &data.regions[region.index()].operations {
            let op = &data.operations[id.index()];
            self.span = op.span;
            self.module = data.module;
            self.tick(1)?;
            let operands = data.operands(op.operands).unwrap();
            let ty = op.result.map(|v| data.values[v.index()].ty).unwrap_or(
                self.program
                    .cells
                    .iter()
                    .find(|c| c.owner == frame.unit)
                    .map_or(TypeId::from_index(0).unwrap(), |c| c.ty),
            );
            let first = || self.value(frame, operands[0]);
            let mut result = None;
            match &op.kind {
                OperationKind::Return => {
                    return Ok(Flow::Return(
                        operands
                            .first()
                            .map(|v| self.value(frame, *v))
                            .transpose()?,
                    ))
                }
                OperationKind::Break => return Ok(Flow::Break),
                OperationKind::Continue => return Ok(Flow::Continue),
                OperationKind::Initialize(cell) => {
                    let value = first()?;
                    self.put(frame, *cell, value)?;
                }
                OperationKind::Declare(_) | OperationKind::PrepareCall(_) => {}
                OperationKind::Load(place) => result = Some(self.place(frame, *place, ty)?),
                OperationKind::CheckPlace(_) => {} // Store validates the private location before committing.
                OperationKind::Store(place) => {
                    let value = first()?;
                    self.store(frame, *place, value.clone())?;
                    result = Some(value);
                }
                OperationKind::Closure(unit) => {
                    result = Some(Value {
                        ty,
                        data: Data::Function(*unit),
                    })
                }
                OperationKind::CopyValue => result = Some(self.copy_value(first()?, 0)?),
                OperationKind::Allocate { kind, .. } => {
                    if matches!(kind, AllocationKind::SpreadArray(_)) {
                        return Err(self.fail("const spread construction is unsupported"));
                    }
                    self.memory(
                        std::mem::size_of::<Heap>() * 2
                            + operands.len() * 2 * std::mem::size_of::<Value>(),
                    )?;
                    let values = operands
                        .iter()
                        .map(|v| self.value(frame, *v))
                        .collect::<Result<_, _>>()?;
                    let keys = match kind {
                        AllocationKind::Record(keys)
                        | AllocationKind::Object(keys)
                        | AllocationKind::Instance { keys, .. } => Some(
                            keys.iter()
                                .map(|key| self.program.strings[key.index()].clone())
                                .collect(),
                        ),
                        _ => None,
                    };
                    let index = self.heap.len();
                    self.heap.push(Heap {
                        kind: kind.clone(),
                        keys,
                        values,
                    });
                    result = Some(Value {
                        ty,
                        data: Data::Heap(index),
                    });
                }
                OperationKind::Call(call) => result = Some(self.call(frame, *call, ty, depth)?),
                OperationKind::Intrinsic(operation) => {
                    result = Some(self.intrinsic(*operation, Some(first()?), &[], ty)?)
                }
                OperationKind::Block(child) => {
                    let flow = self.region(frame, *child, depth + 1)?;
                    if !matches!(flow, Flow::Next) {
                        return Ok(flow);
                    }
                }
                OperationKind::If { yes, no } => {
                    let child = if self.truth(&first()?) {
                        Some(*yes)
                    } else {
                        *no
                    };
                    if let Some(child) = child {
                        let flow = self.region(frame, child, depth + 1)?;
                        if !matches!(flow, Flow::Next) {
                            return Ok(flow);
                        }
                    }
                }
                OperationKind::Select { yes, no } => {
                    let child = if self.truth(&first()?) { *yes } else { *no };
                    match self.region(frame, child, depth + 1)? {
                        Flow::Next => {}
                        other => return Ok(other),
                    }
                    result = Some(self.value(frame, data.regions[child.index()].result.unwrap())?);
                }
                OperationKind::ShortCircuit { kind, right } => {
                    let left = first()?;
                    let take = match kind {
                        ShortCircuit::Nullish => {
                            matches!(left.data, Data::Scalar(Exact::Null | Exact::Undefined))
                        }
                        ShortCircuit::BooleanAnd | ShortCircuit::JavaScriptAnd => self.truth(&left),
                        _ => !self.truth(&left),
                    };
                    if take {
                        match self.region(frame, *right, depth + 1)? {
                            Flow::Next => {}
                            other => return Ok(other),
                        }
                        result =
                            Some(self.value(frame, data.regions[right.index()].result.unwrap())?);
                    } else {
                        result = Some(left);
                    }
                }
                OperationKind::Loop { test, body, update } => loop {
                    self.tick(1)?;
                    match self.region(frame, *test, depth + 1)? {
                        Flow::Next => {}
                        other => return Ok(other),
                    }
                    if let Some(condition) = data.regions[test.index()].result {
                        if !self.truth(&self.value(frame, condition)?) {
                            break;
                        }
                    }
                    match self.region(frame, *body, depth + 1)? {
                        Flow::Break => break,
                        Flow::Return(value) => return Ok(Flow::Return(value)),
                        _ => {}
                    }
                    match self.region(frame, *update, depth + 1)? {
                        Flow::Next | Flow::Continue => {}
                        Flow::Break => break,
                        other => return Ok(other),
                    }
                },
                OperationKind::Constant(_)
                | OperationKind::IntBinary(_)
                | OperationKind::Binary(_)
                | OperationKind::Unary { .. }
                | OperationKind::IsUndefined { .. }
                | OperationKind::Enum { .. } => result = Some(self.primitive(frame, op)?),
                _ => {
                    return Err(self.fail(
                        "const evaluation encountered a host, effectful or unsupported operation",
                    ))
                }
            }
            if let (Some(id), Some(value)) = (op.result, result) {
                if frame.values[id.index()].is_none() {
                    frame.written.push(id);
                }
                frame.primitives[id.index()] = match &value.data {
                    Data::Scalar(value) => Knowledge::Exact(value.clone()),
                    _ => Knowledge::Unknown(facts::UnknownReason::UnknownOperand),
                };
                frame.values[id.index()] = Some(value);
            }
        }
        Ok(Flow::Next)
    }
}

struct Plan {
    cell: CellId,
    unit: UnitId,
    region: RegionId,
    span: Span,
    value: Value,
}
struct Evaluated {
    heap: Vec<Heap>,
    constants: Vec<Option<Value>>,
    plans: Vec<Plan>,
}

fn failure(program: &Program<'_>, cell: CellId, error: RuleError) -> Error {
    let declaration = &program.cells[cell.index()];
    Error {
        module: program.unit(declaration.owner).unwrap().module,
        span: declaration.declaration,
        message: format!("const materialization: {error:?}"),
        allocation: match error {
            RuleError::Allocation(error) => Some(error),
            _ => None,
        },
    }
}

/// Required language lowering runs even with folding/encoding disabled.
pub(crate) fn prepare<'src>(
    program: Program<'src>,
    defines: &crate::config::Defines,
    budget: &mut AllocationBudget<'_>,
) -> Result<Program<'src>, Error> {
    for name in defines.keys() {
        if !program
            .cells
            .iter()
            .any(|cell| cell.declared_define && cell.name == *name)
        {
            return Err(Error {
                module: program.entry_module(),
                span: Span::default(),
                message: format!("configured define `{name}` has no source declaration"),
                allocation: None,
            });
        }
    }
    if !program.cells.iter().any(|cell| cell.declared_const) {
        return Ok(program);
    }
    if let Some(cell) = program
        .cells
        .iter()
        .find(|cell| cell.declared_const && cell.reassigned)
    {
        return Err(Error {
            module: program.unit(cell.owner).unwrap().module,
            span: cell.declaration,
            message: "a const declaration cannot be reassigned or passed by mutable reference"
                .into(),
            allocation: None,
        });
    }
    let fallback = program
        .cells
        .iter()
        .position(|cell| cell.declared_const)
        .and_then(CellId::from_index)
        .unwrap();
    let map_edit = |error: RuleError| Error {
        module: ModuleId::from_index(0).unwrap(),
        span: Span::default(),
        message: format!("const edit: {error:?}"),
        allocation: match error {
            RuleError::Allocation(error) => Some(error),
            _ => None,
        },
    };
    let mut editor = Editor::new_in(program, budget).map_err(map_edit)?;
    budget.with_temporary_context(
        &mut editor,
        |editor, scope| {
            let program = editor.program();
            let fail =
                |error: AllocationError| failure(program, fallback, RuleError::Allocation(error));
            let limits = program.source_contract.const_evaluation;
            // Evaluation payloads stay in this temporary owner. Charge conservatively
            // before grows; never reserve the whole configured ceiling for a tiny const.
            let effects =
                ProgramEffects::build_reusing_in(program, Seal::Module, None, false, scope)
                    .map_err(fail)?;
            let mut initializers = scope
                .filled(Scratch, program.cells.len(), None)
                .map_err(fail)?;
            for unit in &program.units {
                for (index, operation) in unit.data().operations.iter().enumerate() {
                    if let OperationKind::Initialize(cell) = operation.kind {
                        if program.cells[cell.index()].declared_const
                            && !matches!(
                                program.cells[cell.index()].binding,
                                CellBinding::Function(_)
                            )
                        {
                            initializers[cell.index()] =
                                Some((unit.id(), OpId::from_index(index).unwrap()));
                        }
                    }
                }
            }
            let mut constants = scope.vector(Retained, program.cells.len()).map_err(fail)?;
            constants.resize_with(program.cells.len(), || None);
            let root = initializers
                .iter()
                .flatten()
                .next()
                .copied()
                .unwrap_or((program.initialization[0], OpId::from_index(0).unwrap()));
            let mut interpreter = Interpreter {
                program,
                effects: &effects,
                budget: scope,
                limits,
                steps: limits.steps,
                bytes: 0,
                heap: Vec::new(),
                constants,
                initializers,
                root,
                span: program.cells[fallback.index()].declaration,
                module: program.unit(root.0).unwrap().module,
            };
            let mut order: Vec<_> = interpreter
                .initializers
                .iter()
                .enumerate()
                .filter_map(|(cell, init)| {
                    init.map(|(_, operation)| (operation, CellId::from_index(cell).unwrap()))
                })
                .collect();
            // Build a canonical unit/operation order without comparing unrelated source spans.
            let mut units: Vec<UnitId> = program.initialization.as_ref().clone();
            units.extend(
                program
                    .units
                    .iter()
                    .map(|unit| unit.id())
                    .filter(|id| !program.initialization.contains(id)),
            );
            order.sort_unstable_by_key(|(op, cell)| {
                (
                    units
                        .iter()
                        .position(|unit| *unit == program.cells[cell.index()].owner)
                        .unwrap(),
                    op.index(),
                )
            });
            let mut plans = Vec::new();
            let mut root_frame: Option<Frame> = None;
            for (operation, cell) in order {
                let unit = program.cells[cell.index()].owner;
                if program.cells.iter().any(|declaration| {
                    declaration.declared_const && declaration.binding == CellBinding::Function(unit)
                }) {
                    continue;
                }
                interpreter.steps = limits.steps;
                interpreter.root = (unit, operation);
                let region = program.unit(unit).unwrap().operations[operation.index()].region;
                // Reuse the module's frame; reset only written entries. Many
                // independent const declarations must not allocate/clear the
                // entire module value arena for each initializer.
                let mut frame = match root_frame.take() {
                    Some(mut frame) if frame.unit == unit => {
                        frame.reset();
                        frame
                    }
                    _ => interpreter.frame(unit)?,
                };
                let declaration = &program.cells[cell.index()];
                interpreter.span = declaration.declaration;
                interpreter.module = program.unit(unit).unwrap().module;
                let value = if let Some(configured) = defines
                    .get(&declaration.name)
                    .filter(|_| declaration.declared_define)
                {
                    use crate::config::DefineValue as D;
                    let exact = match (&program.types[declaration.ty.index()], configured) {
                        (Type::Bool, D::Boolean(value)) => Exact::Boolean(*value),
                        (Type::Int, D::Integer(value)) => {
                            Exact::Integer(i32::try_from(*value).map_err(|_| {
                                interpreter.fail(format!(
                                    "define `{}` requires an int32 value",
                                    declaration.name
                                ))
                            })?)
                        }
                        (Type::Float, D::Number(bits)) => Exact::Number(*bits),
                        (Type::Float, D::Integer(value))
                            if (*value as f64) as i128 == i128::from(*value) =>
                        {
                            Exact::Number((*value as f64).to_bits())
                        }
                        (Type::String, D::String(value)) => {
                            interpreter.memory(value.len().saturating_mul(4) + 64)?;
                            Exact::String(StoredString::Computed(std::sync::Arc::new(
                                value.as_str().into(),
                            )))
                        }
                        _ => {
                            return Err(interpreter.fail(format!(
                                "define `{}` TOML value does not match its declared scalar type",
                                declaration.name
                            )))
                        }
                    };
                    Value {
                        ty: declaration.ty,
                        data: Data::Scalar(exact),
                    }
                } else {
                    match interpreter.region(&mut frame, region, 0)? {
                        Flow::Next => {}
                        _ => {
                            return Err(
                                interpreter.fail("const initializer cannot transfer control")
                            )
                        }
                    }
                    frame
                        .cells
                        .iter()
                        .find(|(known, _)| *known == cell)
                        .map(|(_, value)| value.clone())
                        .ok_or_else(|| interpreter.fail("const initializer produced no value"))?
                };
                if matches!(value.data, Data::Function(_)) {
                    return Err(interpreter.fail("const data cannot contain a callable identity"));
                }
                interpreter.constants[cell.index()] = Some(value.clone());
                root_frame = Some(frame);
                interpreter.budget.push(
                    Retained,
                    &mut plans,
                    Plan {
                        cell,
                        unit,
                        region,
                        span: program.cells[cell.index()].declaration,
                        value,
                    },
                )?;
            }
            Ok::<_, Error>(Evaluated {
                heap: interpreter.heap,
                constants: interpreter.constants,
                plans,
            })
        },
        |evaluated, editor, budget| {
            for plan in &evaluated.plans {
                materialize(editor, evaluated, plan, budget)
                    .map_err(|error| failure(editor.program(), plan.cell, error))?;
            }
            Ok(())
        },
    )?;
    loop {
        let additions = budget.with_temporary_context(
            &mut editor,
            |editor, scope| {
                let program = editor.program();
                let mut additions = scope.vector(Retained, 0)?;
                for unit in &program.units {
                    for op in &unit.data().operations {
                        scope.work(crate::compilation_policy::WorkKind::Analysis, 1)?;
                        if let OperationKind::Closure(child) = op.kind {
                            for &cell in &program.unit(child).unwrap().captures {
                                if program.cells[cell.index()].owner != unit.id()
                                    && !unit.data().captures.contains(&cell)
                                {
                                    scope.push(Retained, &mut additions, (unit.id(), cell))?;
                                }
                            }
                        }
                    }
                }
                Ok::<_, Error>(additions)
            },
            |additions, editor, budget| {
                for &(unit, cell) in additions {
                    let data = editor.unit_mut_in(unit, budget).map_err(map_edit)?;
                    if !data.captures.contains(&cell) {
                        budget.push(Retained, &mut data.captures, cell)?;
                    }
                }
                Ok(!additions.is_empty())
            },
        )?;
        if !additions {
            break;
        }
    }
    let program = editor.finish_in(budget).map_err(map_edit)?;
    validate(&program, budget)?;
    Ok(program)
}

struct Materializer<'a, 'src, 'b, 'ledger> {
    editor: &'a mut Editor<'src>,
    evaluated: &'a Evaluated,
    plan: &'a Plan,
    budget: &'b mut AllocationBudget<'ledger>,
    memo: Vec<(usize, ValueId)>,
    visiting: Vec<usize>,
    allocation: usize,
}
impl Materializer<'_, '_, '_, '_> {
    fn emit(
        &mut self,
        kind: OperationKind,
        operands: &[ValueId],
        ty: Option<TypeId>,
    ) -> Result<Option<ValueId>, RuleError> {
        let data = self.editor.unit_mut_in(self.plan.unit, self.budget)?;
        let (op, result) = edit::push_operation_in(
            data,
            kind,
            operands,
            ty,
            self.plan.region,
            self.plan.span,
            self.budget,
        )?;
        self.budget.push(
            Retained,
            &mut data.regions[self.plan.region.index()].operations,
            op,
        )?;
        Ok(result)
    }
    fn place(&mut self, place: Place) -> Result<PlaceId, RuleError> {
        let data = self.editor.unit_mut_in(self.plan.unit, self.budget)?;
        let id = PlaceId::from_index(data.places.len()).ok_or("const place capacity")?;
        self.budget.push(Retained, &mut data.places, place)?;
        Ok(id)
    }
    fn resolved(&self, value: &Value, depth: u16) -> Result<Value, RuleError> {
        let Data::Static(cell, path) = &value.data else {
            return Ok(value.clone());
        };
        if depth >= self.editor.program().source_contract.const_evaluation.depth {
            return Err("const dependency depth".into());
        }
        let mut current = self.resolved(
            self.evaluated.constants[cell.index()]
                .as_ref()
                .ok_or("const dependency")?,
            depth + 1,
        )?;
        for key in path.iter() {
            let Data::Heap(id) = current.data else {
                return Err("const reference path".into());
            };
            let heap = &self.evaluated.heap[id];
            let index = match key {
                Key::Index(index) => *index,
                Key::Field(field) => {
                    self.editor
                        .program()
                        .fields
                        .iter()
                        .find(|f| f.identity == *field)
                        .ok_or("const field")?
                        .index
                }
                Key::Class(field) => field.slot as usize,
                Key::Named(key) => heap
                    .keys
                    .as_ref()
                    .ok_or("const keys")?
                    .iter()
                    .position(|k| k == &self.editor.program().strings[key.index()])
                    .ok_or("const key")?,
                Key::Text(key) => heap
                    .keys
                    .as_ref()
                    .ok_or("const keys")?
                    .iter()
                    .position(|k| k == key.as_ref())
                    .ok_or("const key")?,
            };
            current = self.resolved(heap.values.get(index).ok_or("const index")?, depth + 1)?;
        }
        Ok(current)
    }
    fn value(&mut self, value: &Value) -> Result<ValueId, RuleError> {
        self.budget
            .work(crate::compilation_policy::WorkKind::Analysis, 1)?;
        let ty = value.ty;
        match &value.data {
            Data::Scalar(value) => {
                let value = match value {
                    Exact::Integer(value) => Constant::Integer(*value),
                    Exact::Number(value) => Constant::Number(*value),
                    Exact::Boolean(value) => Constant::Boolean(*value),
                    Exact::Null => Constant::Null,
                    Exact::Undefined => Constant::Undefined,
                    Exact::String(StoredString::Source(value)) => Constant::String(*value),
                    Exact::String(StoredString::Computed(value)) => {
                        Constant::String(self.editor.intern_string_in(value, self.budget)?)
                    }
                };
                Ok(self
                    .emit(OperationKind::Constant(value), &[], Some(ty))?
                    .unwrap())
            }
            Data::Heap(id) => {
                if let Some((_, value)) = self.memo.iter().find(|(known, _)| known == id) {
                    return Ok(*value);
                }
                if self.visiting.contains(id) {
                    return Err("const data must be acyclic".into());
                }
                if self.visiting.len()
                    >= usize::from(self.editor.program().source_contract.const_evaluation.depth)
                {
                    return Err("const data exceeds configured depth".into());
                }
                self.budget.push(Scratch, &mut self.visiting, *id)?;
                let heap = &self.evaluated.heap[*id];
                let mut values = self.budget.vector(Scratch, heap.values.len())?;
                for value in &heap.values {
                    values.push(self.value(value)?);
                }
                let mut kind = match &heap.kind {
                    AllocationKind::Record(_) => AllocationKind::Record(Vec::new()),
                    AllocationKind::Object(_) => AllocationKind::Object(Vec::new()),
                    AllocationKind::Instance { class, .. } => AllocationKind::Instance {
                        class: *class,
                        keys: Vec::new(),
                    },
                    other => other.clone(),
                };
                if let Some(keys) = &heap.keys {
                    let mut retained = self.budget.vector(Retained, keys.len())?;
                    for key in keys.iter() {
                        retained.push(self.editor.intern_string_in(key, self.budget)?);
                    }
                    let old = match &mut kind {
                        AllocationKind::Record(keys)
                        | AllocationKind::Object(keys)
                        | AllocationKind::Instance { keys, .. } => {
                            Some(std::mem::replace(keys, retained))
                        }
                        _ => None,
                    };
                    if let Some(old) = old {
                        super::storage::release_vec(old, Retained, self.budget)?;
                    }
                }
                let identity =
                    AllocationId::from_index(self.allocation).ok_or("const allocation capacity")?;
                self.allocation += 1;
                let result = self
                    .emit(
                        OperationKind::Allocate { identity, kind },
                        &values,
                        Some(ty),
                    )?
                    .unwrap();
                super::storage::release_vec(values, Scratch, self.budget)?;
                self.visiting.pop();
                self.budget.push(Scratch, &mut self.memo, (*id, result))?;
                Ok(result)
            }
            Data::Static(cell, path) => {
                let declaration = &self.editor.program().cells[cell.index()];
                let owner = declaration.owner;
                let mut ty = declaration.ty;
                if owner != self.plan.unit {
                    let data = self.editor.unit_mut_in(self.plan.unit, self.budget)?;
                    if !data.captures.contains(cell) {
                        self.budget.push(Retained, &mut data.captures, *cell)?;
                    }
                }
                let place = self.place(Place::Cell(*cell))?;
                let mut result = self
                    .emit(OperationKind::Load(place), &[], Some(ty))?
                    .unwrap();
                let mut current = self.evaluated.constants[cell.index()]
                    .as_ref()
                    .ok_or("missing const reference")?
                    .clone();
                for key in path.iter() {
                    // Static paths are recorded only from successful exact projections.
                    current = self.resolved(&current, 0)?;
                    let Data::Heap(id) = current.data else {
                        return Err("nested const reference path".into());
                    };
                    let heap = &self.evaluated.heap[id];
                    let (place, index) = match key {
                        Key::Index(index) => {
                            let integer = self.editor.intern_type_in(&Type::Int, self.budget)?;
                            let key = self
                                .emit(
                                    OperationKind::Constant(Constant::Integer(*index as i32)),
                                    &[],
                                    Some(integer),
                                )?
                                .unwrap();
                            (
                                Place::Index {
                                    receiver: result,
                                    key,
                                },
                                *index,
                            )
                        }
                        Key::Named(key) => {
                            let text = &self.editor.program().strings[key.index()];
                            let index = heap
                                .keys
                                .as_ref()
                                .ok_or("const named projection")?
                                .iter()
                                .position(|known| known == text)
                                .ok_or("const key")?;
                            (
                                Place::Member {
                                    receiver: result,
                                    key: *key,
                                },
                                index,
                            )
                        }
                        Key::Text(text) => {
                            let index = heap
                                .keys
                                .as_ref()
                                .ok_or("const named projection")?
                                .iter()
                                .position(|known| known == text.as_ref())
                                .ok_or("const key")?;
                            let key = self.editor.intern_string_in(text, self.budget)?;
                            (
                                Place::Member {
                                    receiver: result,
                                    key,
                                },
                                index,
                            )
                        }
                        Key::Field(field) => {
                            let index = self
                                .editor
                                .program()
                                .fields
                                .iter()
                                .find(|f| f.identity == *field)
                                .ok_or("const field")?
                                .index;
                            let base = self.place(Place::Value(result))?;
                            (
                                Place::Field {
                                    base,
                                    field: *field,
                                },
                                index,
                            )
                        }
                        Key::Class(field) => (
                            Place::ClassField {
                                receiver: result,
                                field: *field,
                            },
                            field.slot as usize,
                        ),
                    };
                    current = heap.values[index].clone();
                    ty = current.ty;
                    let place = self.place(place)?;
                    result = self
                        .emit(OperationKind::Load(place), &[], Some(ty))?
                        .unwrap();
                }
                Ok(result)
            }
            Data::Function(_) => Err("const data cannot retain functions".into()),
        }
    }
}
fn materialize(
    editor: &mut Editor<'_>,
    evaluated: &Evaluated,
    plan: &Plan,
    budget: &mut AllocationBudget<'_>,
) -> Result<(), RuleError> {
    let data = editor.unit_mut_in(plan.unit, budget)?;
    let allocation = data
        .operations
        .iter()
        .filter_map(|op| {
            if let OperationKind::Allocate { identity, .. } = op.kind {
                Some(identity.index() + 1)
            } else {
                None
            }
        })
        .max()
        .unwrap_or(0);
    data.regions[plan.region.index()].operations.clear();
    let mut emit = Materializer {
        editor,
        evaluated,
        plan,
        budget,
        memo: Vec::new(),
        visiting: Vec::new(),
        allocation,
    };
    let value = emit.value(&plan.value)?;
    emit.emit(OperationKind::Initialize(plan.cell), &[value], None)?;
    super::storage::release_vec(emit.memo, Scratch, emit.budget)?;
    super::storage::release_vec(emit.visiting, Scratch, emit.budget)?;
    let (data, cells) = emit.editor.unit_and_cells_in(plan.unit, emit.budget)?;
    let owner = data
        .operations
        .iter()
        .enumerate()
        .find_map(|(index, op)| {
            matches!(op.kind, OperationKind::Block(child) if child == plan.region)
                .then(|| OpId::from_index(index).unwrap())
        })
        .ok_or("const initializer region has no declaration owner")?;
    edit::splice_in(data, cells, plan.unit, owner, plan.region, emit.budget)?;
    Ok(())
}

pub(crate) fn aggregate(ty: &Type<'_>) -> bool {
    match ty {
        Type::Nullable(inner) => return aggregate(inner.as_ref()),
        Type::Union(types) => return types.iter().any(aggregate),
        _ => {}
    }
    !matches!(
        ty,
        Type::Int
            | Type::Float
            | Type::Bool
            | Type::String
            | Type::Null
            | Type::Void
            | Type::Enum(_)
            | Type::Function(_)
            | Type::GenericFunction(_)
    )
}
fn add(into: &mut u8, bits: u8) -> bool {
    let before = *into;
    *into |= bits;
    before != *into
}
/// Bit 1 means the reference itself is read-only. Bit 2 means an ordinary
/// mutable container can reach const data. This avoids treating storing a const
/// subtree into a new container as a write into the subtree.
fn place_bits(data: &UnitData, place: PlaceId, cells: &[u8], values: &[u8]) -> u8 {
    match data.places[place.index()] {
        Place::Cell(cell) => cells[cell.index()],
        Place::Value(value) => values[value.index()],
        Place::Field { base, .. } => u8::from(place_bits(data, base, cells, values) != 0),
        Place::Member { receiver, .. }
        | Place::ClassField { receiver, .. }
        | Place::Index { receiver, .. } => u8::from(values[receiver.index()] != 0),
    }
}
fn receiver_bits(data: &UnitData, place: PlaceId, cells: &[u8], values: &[u8]) -> u8 {
    match data.places[place.index()] {
        Place::Cell(cell) => cells[cell.index()],
        Place::Value(value) => values[value.index()],
        Place::Field { base, .. } => place_bits(data, base, cells, values),
        Place::Member { receiver, .. }
        | Place::ClassField { receiver, .. }
        | Place::Index { receiver, .. } => values[receiver.index()],
    }
}
fn mark_place(
    data: &UnitData,
    place: PlaceId,
    bits: u8,
    cells: &mut [u8],
    values: &mut [u8],
) -> bool {
    match data.places[place.index()] {
        Place::Cell(cell) => add(&mut cells[cell.index()], bits),
        Place::Value(value) => add(&mut values[value.index()], bits),
        Place::Field { base, .. } => mark_place(data, base, 2, cells, values),
        Place::Member { receiver, .. }
        | Place::ClassField { receiver, .. }
        | Place::Index { receiver, .. } => add(&mut values[receiver.index()], 2),
    }
}
fn tracked(ty: &Type<'_>) -> bool {
    aggregate(ty) || matches!(ty, Type::Function(_) | Type::GenericFunction(_))
}
/// The ordinary effect catalog stays conservative about host dispatch. Const
/// checking additionally knows these source-typed operations cannot call user
/// coercions or mutate their inputs. This does not grant removal permissions.
fn readonly_intrinsic(program:&Program<'_>,data:&UnitData,site:&CallSite,intrinsic:crate::primitive::ResolvedIntrinsic)->bool {
    use crate::catalog::EffectClass as E;
    use crate::primitive::ResolvedIntrinsic as R;
    if matches!(crate::catalog::effect_class(intrinsic),E::Read {..}|E::Pure {fresh:false,..}|E::Inert {..}) {return true;}
    match intrinsic {
        R::Method(Intrinsic::ArrayConcat)=>true,
        R::Method(Intrinsic::ArrayJoin)=>{
            let CallTarget::Intrinsic {receiver:Some(receiver),..}=site.target else {return false;};
            matches!(&program.types[data.values[receiver.index()].ty.index()],Type::Array(element) if matches!(element.as_ref(),Type::Int|Type::Float|Type::Bool|Type::String|Type::Enum(_)))
        },
        _=>false,
    }
}
/// Fresh identity alone is insufficient (typed subarray views still alias).
/// These operations copy their outer storage, retaining any const children.
fn copies_outer(intrinsic:crate::primitive::ResolvedIntrinsic)->bool {
    use crate::primitive::ResolvedIntrinsic as R;
    matches!(intrinsic,R::Method(Intrinsic::ArraySlice|Intrinsic::ArrayConcat|Intrinsic::ArraySplice|Intrinsic::BufferSlice))
}
fn validate(program: &Program<'_>, budget: &mut AllocationBudget<'_>) -> Result<(), Error> {
    let fail = |unit: UnitId, span: Span, message: &str| Error {
        module: program.unit(unit).unwrap().module,
        span,
        message: message.into(),
        allocation: None,
    };
    let allocation = |error: AllocationError| Error {
        module: program.entry_module(),
        span: Span::default(),
        message: "const checking resource limit".into(),
        allocation: Some(error),
    };
    let mut scope = budget.scope();
    let effects = ProgramEffects::build_reusing_in(program, Seal::Module, None, false, &mut scope)
        .map_err(allocation)?;
    let mut cells = scope
        .filled(Scratch, program.cells.len(), 0u8)
        .map_err(allocation)?;
    let mut values = scope
        .vector(Scratch, program.units.len())
        .map_err(allocation)?;
    for unit in &program.units {
        values.push(
            scope
                .filled(Scratch, unit.data().values.len(), 0u8)
                .map_err(allocation)?,
        );
    }
    let mut returns = scope
        .filled(Scratch, program.units.len(), 0u8)
        .map_err(allocation)?;
    for (index, cell) in program.cells.iter().enumerate() {
        if cell.declared_const && aggregate(&program.types[cell.ty.index()]) {
            cells[index] = 1;
        }
    }
    let mut changed = true;
    while changed {
        changed = false;
        for (index, cell) in program.cells.iter().enumerate() {
            if let CellBinding::Function(unit) = cell.binding {
                changed |= add(&mut cells[index], returns[unit.index()]);
            }
        }
        for unit in &program.units {
            let data = unit.data();
            let own = unit.id().index();
            for operation in &data.operations {
                scope
                    .work(crate::compilation_policy::WorkKind::Analysis, 1)
                    .map_err(allocation)?;
                let operands = data.operands(operation.operands).unwrap();
                let first = operands
                    .first()
                    .map_or(0, |value| values[own][value.index()]);
                let mut bits = 0;
                match operation.kind {
                    OperationKind::Initialize(cell) => {
                        if let Some(value) = operands
                            .first()
                            .filter(|v| tracked(&program.types[data.values[v.index()].ty.index()]))
                        {
                            changed |= add(&mut cells[cell.index()], first);
                            if program.cells[cell.index()].declared_const {
                                changed |= add(&mut values[own][value.index()], 1);
                            }
                        }
                    }
                    OperationKind::Store(place) => {
                        bits = first;
                        if first != 0 {
                            changed |= mark_place(data, place, first, &mut cells, &mut values[own]);
                        }
                    }
                    OperationKind::Load(place) => {
                        bits = place_bits(data, place, &cells, &values[own])
                    }
                    OperationKind::CopyValue => bits = first,
                    OperationKind::Closure(target) => bits = returns[target.index()],
                    OperationKind::Allocate { .. } => {
                        if operands.iter().any(|v| values[own][v.index()] != 0) {
                            bits = 2;
                        }
                        if operation
                            .result
                            .is_some_and(|v| values[own][v.index()] & 1 != 0)
                        {
                            for value in operands {
                                if aggregate(&program.types[data.values[value.index()].ty.index()])
                                {
                                    changed |= add(&mut values[own][value.index()], 1);
                                }
                            }
                        }
                    }
                    OperationKind::Select { yes, no } => {
                        for region in [yes, no] {
                            if let Some(value) = data.regions[region.index()].result {
                                bits |= values[own][value.index()];
                            }
                        }
                    }
                    OperationKind::ShortCircuit { right, .. } => {
                        bits = first
                            | data.regions[right.index()]
                                .result
                                .map_or(0, |v| values[own][v.index()])
                    }
                    OperationKind::Return => {
                        changed |= add(&mut returns[own], first);
                    }
                    OperationKind::Call(call) => {
                        if let Callee::Unit(target) = effects.graph().callee(unit.id(), call) {
                            let params = &program.unit(target).unwrap().parameters;
                            let args = data.arguments(data.calls[call.index()].arguments).unwrap();
                            for (parameter, arg) in params.iter().zip(args) {
                                if let CallArgument::Value(value) = arg {
                                    changed |= add(
                                        &mut cells[parameter.index()],
                                        values[own][value.index()],
                                    );
                                }
                            }
                            bits = returns[target.index()];
                        } else if let Callee::Intrinsic(intrinsic) = effects.graph().callee(unit.id(), call)
                        {
                            let site = &data.calls[call.index()];
                            let receiver = match site.target {
                                CallTarget::Intrinsic { receiver, .. } => {
                                    receiver.map_or(0, |v| values[own][v.index()])
                                }
                                _ => 0,
                            };
                            if receiver != 0 || data.arguments(site.arguments).unwrap().iter().any(|arg| matches!(arg, CallArgument::Value(value) if values[own][value.index()] != 0)) { bits = if copies_outer(intrinsic) {2}else{1}; }
                        } else if matches!(effects.graph().callee(unit.id(),call),Callee::Builtin(crate::check::BuiltinCall::ObjectValues)) {
                            let site=&data.calls[call.index()];
                            if data.arguments(site.arguments).unwrap().iter().any(|arg|matches!(arg,CallArgument::Value(value) if values[own][value.index()]!=0)) {bits=2;}
                        }
                    }
                    _ => {}
                }
                if let Some(value) = operation
                    .result
                    .filter(|v| tracked(&program.types[data.values[v.index()].ty.index()]))
                {
                    changed |= add(&mut values[own][value.index()], bits);
                }
            }
        }
    }
    for unit in &program.units {
        let data = unit.data();
        let row = &values[unit.id().index()];
        let const_body = program
            .cells
            .iter()
            .any(|cell| cell.declared_const && cell.binding == CellBinding::Function(unit.id()));
        for operation in &data.operations {
            scope
                .work(crate::compilation_policy::WorkKind::Analysis, 1)
                .map_err(allocation)?;
            match operation.kind {
                OperationKind::Load(place)
                    if !const_body
                        && matches!(data.places[place.index()], Place::Cell(cell) if program.cells[cell.index()].declared_const && matches!(program.cells[cell.index()].binding, CellBinding::Function(_))) =>
                {
                    return Err(fail(
                        unit.id(),
                        operation.span,
                        "a const function cannot escape to runtime; call it in a const initializer",
                    ));
                }
                OperationKind::Store(place) => {
                    let stored = data
                        .operands(operation.operands)
                        .unwrap()
                        .first()
                        .is_some_and(|value| row[value.index()] != 0);
                    let safe_storage = matches!(data.places[place.index()], Place::Cell(cell) if program.cells[cell.index()].binding != CellBinding::Foreign);
                    if stored && !safe_storage {
                        return Err(fail(unit.id(), operation.span, "const data cannot escape through host or unproved container storage; construct a private container with an initializer"));
                    }
                    let immutable = match data.places[place.index()] {
                        Place::Cell(cell) => program.cells[cell.index()].declared_const,
                        _ => receiver_bits(data, place, &cells, row) & 1 != 0,
                    };
                    if immutable {
                        return Err(fail(
                            unit.id(),
                            operation.span,
                            "cannot mutate const data through a field, index or alias",
                        ));
                    }
                }
                OperationKind::Call(call) => {
                    let site = &data.calls[call.index()];
                    let args = data.arguments(site.arguments).unwrap();
                    if args.iter().any(|arg| matches!(arg, CallArgument::Reference(place) if receiver_bits(data, *place, &cells, row) != 0)) {
                        return Err(fail(unit.id(), operation.span, "const data cannot be passed through a mutable reference"));
                    }
                    let tainted = args.iter().any(|arg| match arg {
                        CallArgument::Value(value) | CallArgument::Spread(value) => {
                            row[value.index()] != 0
                        }
                        CallArgument::Reference(place) => {
                            receiver_bits(data, *place, &cells, row) != 0
                        }
                    });
                    let receiver = match site.target {
                        CallTarget::Intrinsic { receiver, .. } => {
                            receiver.map_or(0, |v| row[v.index()])
                        }
                        CallTarget::Reference { place } => receiver_bits(data, place, &cells, row),
                        _ => 0,
                    };
                    let callable = match site.target {
                        CallTarget::Value { callee, .. } => row[callee.index()],
                        _ => 0,
                    };
                    match effects.graph().callee(unit.id(), call) {
                        Callee::Unit(target) => {
                            let body = program.unit(target).unwrap();
                            let rest = body.callable_type.is_some_and(|ty| {
                                match &program.types[ty.index()] {
                                    Type::Function(signature) => {
                                        signature.params.iter().any(|parameter| parameter.rest)
                                    }
                                    _ => true,
                                }
                            });
                            if tainted
                                && (rest
                                    || args.len() > body.parameters.len()
                                    || args
                                        .iter()
                                        .any(|arg| matches!(arg, CallArgument::Spread(_))))
                            {
                                return Err(fail(unit.id(), operation.span, "const data requires explicit value arguments; mutable rest/spread transport is not proved"));
                            }
                            if receiver != 0 {
                                return Err(fail(
                                    unit.id(),
                                    operation.span,
                                    "const data cannot be passed as an unproved mutable receiver",
                                ));
                            }
                            if !const_body
                                && program.cells.iter().any(|cell| {
                                    cell.declared_const
                                        && cell.binding == CellBinding::Function(target)
                                })
                            {
                                return Err(fail(unit.id(), operation.span, "call a const function in a const initializer; runtime calls require an ordinary function"));
                            }
                        }
                        Callee::Intrinsic(intrinsic) if tainted || receiver != 0 => {
                            use crate::catalog::EffectClass as E;
                            let writes_mutable_outer=receiver&1==0 && !tainted && matches!(crate::catalog::effect_class(intrinsic),E::Write {..});
                            if !readonly_intrinsic(program,data,site,intrinsic) && !writes_mutable_outer {
                                return Err(fail(unit.id(), operation.span, "const data cannot reach a mutating, callback or unknown intrinsic"));
                            }
                        }
                        Callee::Builtin(crate::check::BuiltinCall::ObjectKeys|crate::check::BuiltinCall::ObjectValues|crate::check::BuiltinCall::ObjectHasOwn|crate::check::BuiltinCall::JsonStringify)=>{},
                        _ if tainted || receiver != 0 || callable != 0 => {
                            return Err(fail(
                                unit.id(),
                                operation.span,
                                "const data cannot escape to a mutable host or unresolved call",
                            ))
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    // Explicit const exports are frozen by their boundary owner. Publishing
    // a mutable alias would conceal the immutable contract from the consumer.
    for export in &program.exports[program.public.clone()] {
        let InterfaceTarget::Value(cell) = export.target else {
            continue;
        };
        let declaration = &program.cells[cell.index()];
        if declaration.declared_const && matches!(declaration.binding, CellBinding::Function(_)) {
            return Err(fail(
                declaration.owner,
                declaration.declaration,
                "a const function has no public runtime value",
            ));
        }
        let escaping = match declaration.binding {
            CellBinding::Function(unit) => returns[unit.index()] != 0,
            _ => cells[cell.index()] != 0,
        };
        if escaping && !declaration.declared_const {
            return Err(fail(
                declaration.owner,
                declaration.declaration,
                "publish immutable data through an explicit const export",
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "const_data_tests.rs"]
mod tests;
