//! Static physical storage for checked immutable graphs created at most once.
//! Entries refer to existing SSA definitions; evaluation and immutability are
//! owned by the shared const lowering. Repeated activations retain fresh owners.
use super::*;
use crate::output_budget::AllocationClass::Scratch;

type Key = (UnitId, ValueId);
#[derive(Clone, Copy, Debug)]
enum Literal {
    Scalar(Constant),
    Object(Key),
}
#[derive(Default)]
pub(super) struct StaticStorage {
    values: Vec<Vec<Option<Literal>>>,
    selected: Vec<Vec<bool>>,
    boxes: Vec<Vec<bool>>,
    objects: Vec<Key>,
}
impl StaticStorage {
    pub(super) fn has_objects(&self) -> bool {
        !self.objects.is_empty()
    }
    pub(super) fn build(
        plan: &mut NativePlan<'_, '_>,
        enabled: bool,
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Self, NativeError> {
        if !enabled {
            return Ok(Self::default());
        }
        let program = plan.program;
        let mut result = Self::default();
        let mut states = Vec::new();
        let mut repeated = Vec::new();
        let mut initializers = budget.filled(Scratch, program.cells.len(), None)?;
        for frozen in &program.units {
            let data = frozen.data();
            let values = budget.filled(Scratch, data.values.len(), None)?;
            budget.push(Scratch, &mut result.values, values)?;
            let selected = budget.filled(Scratch, data.values.len(), false)?;
            budget.push(Scratch, &mut result.selected, selected)?;
            let boxes = budget.filled(Scratch, data.values.len(), false)?;
            budget.push(Scratch, &mut result.boxes, boxes)?;
            let state = budget.filled(Scratch, data.values.len(), 0u8)?;
            budget.push(Scratch, &mut states, state)?;
            let mut loops = budget.filled(Scratch, data.regions.len(), false)?;
            for operation in &data.operations {
                budget.work(WorkKind::Analysis, 1)?;
                if matches!(
                    operation.kind,
                    OperationKind::Loop { .. }
                        | OperationKind::ForOf { .. }
                        | OperationKind::ForIn { .. }
                ) {
                    for child in operation.kind.child_regions() {
                        loops[child.index()] = true;
                    }
                }
                if let OperationKind::Initialize(cell) = operation.kind {
                    if program.cells[cell.index()].declared_const {
                        initializers[cell.index()] = data
                            .operands(operation.operands)
                            .unwrap()
                            .first()
                            .map(|&v| (frozen.id(), v));
                    }
                }
            }
            // Region parents are structural, not assumed to precede children.
            for region in 0..loops.len() {
                let mut parent = data.regions[region].parent;
                while let Some(id) = parent {
                    budget.work(WorkKind::Analysis, 1)?;
                    if loops[id.index()] {
                        loops[region] = true;
                        break;
                    }
                    parent = data.regions[id.index()].parent;
                }
            }
            budget.push(Scratch, &mut repeated, loops)?;
        }
        let mut stack = Vec::new();
        for (cell, root) in initializers.iter().enumerate() {
            let Some(root) = *root else {
                continue;
            };
            if program.unit(root.0).unwrap().kind != UnitKind::ModuleInitialization {
                continue;
            }
            if program.cells[cell].reassigned {
                continue;
            }
            budget.push(Scratch, &mut stack, (root, false))?;
            while let Some((key, finish)) = stack.pop() {
                budget.work(WorkKind::Analysis, 1)?;
                let (u, v) = (key.0.index(), key.1.index());
                if states[u][v] == 2 {
                    continue;
                }
                let data = program.unit(key.0).unwrap();
                let op = &data.operations[data.values[v].definition.index()];
                if !finish {
                    if states[u][v] == 1 {
                        continue;
                    }
                    states[u][v] = 1;
                    if data.kind != UnitKind::ModuleInitialization || repeated[u][op.region.index()]
                    {
                        states[u][v] = 2;
                        continue;
                    }
                    budget.push(Scratch, &mut stack, (key, true))?;
                    for &operand in data.operands(op.operands).unwrap() {
                        budget.push(Scratch, &mut stack, ((key.0, operand), false))?;
                    }
                    if let OperationKind::Load(place) = op.kind {
                        let mut at = place;
                        loop {
                            match plan.units[u].places[at.index()].recipe {
                                PlaceRecipe::Cell(cell) => {
                                    if let Some(source) = initializers[cell.index()] {
                                        budget.push(Scratch, &mut stack, (source, false))?;
                                    }
                                    break;
                                }
                                PlaceRecipe::Value(value) => {
                                    budget.push(Scratch, &mut stack, ((key.0, value), false))?;
                                    break;
                                }
                                PlaceRecipe::Field { base, .. } => at = base,
                                PlaceRecipe::Member { receiver, .. } => {
                                    budget.push(Scratch, &mut stack, ((key.0, receiver), false))?;
                                    break;
                                }
                                PlaceRecipe::Element {
                                    receiver, index, ..
                                } => {
                                    budget.push(Scratch, &mut stack, ((key.0, receiver), false))?;
                                    budget.push(Scratch, &mut stack, ((key.0, index), false))?;
                                    break;
                                }
                                PlaceRecipe::Record {
                                    receiver,
                                    key: record_key,
                                    ..
                                } => {
                                    budget.push(Scratch, &mut stack, ((key.0, receiver), false))?;
                                    if let RecordKey::Value(value) = record_key {
                                        budget.push(
                                            Scratch,
                                            &mut stack,
                                            ((key.0, value), false),
                                        )?;
                                    }
                                    break;
                                }
                                _ => break,
                            }
                        }
                    }
                    continue;
                }
                let operands = data.operands(op.operands).unwrap();
                let value = match &op.kind {
                    OperationKind::Constant(c) => Some(Literal::Scalar(*c)),
                    OperationKind::CopyValue => result.get((key.0, operands[0])),
                    OperationKind::Load(place) => {
                        result.place(plan, key.0, *place, &initializers, budget)?
                    }
                    OperationKind::Allocate {
                        kind:
                            AllocationKind::Array
                            | AllocationKind::Struct(_)
                            | AllocationKind::Record(_)
                            | AllocationKind::Object(_)
                            | AllocationKind::Instance { .. },
                        ..
                    } if operands.iter().all(|&v| result.get((key.0, v)).is_some()) => {
                        Some(Literal::Object(key))
                    }
                    _ => None,
                };
                result.values[u][v] = value;
                states[u][v] = 2;
            }
            if let Some(Literal::Object(key)) = result.get(root) {
                budget.push(Scratch, &mut stack, (key, false))?;
                while let Some((key, _)) = stack.pop() {
                    let (u, v) = (key.0.index(), key.1.index());
                    if result.selected[u][v] {
                        continue;
                    }
                    result.selected[u][v] = true;
                    budget.push(Scratch, &mut result.objects, key)?;
                    let data = program.unit(key.0).unwrap();
                    let op = &data.operations[data.values[v].definition.index()];
                    for &value in data.operands(op.operands).unwrap() {
                        budget.work(WorkKind::Analysis, 1)?;
                        if let Some(Literal::Object(child)) = result.get((key.0, value)) {
                            budget.push(Scratch, &mut stack, (child, false))?;
                        }
                    }
                }
            }
        }
        // A generic field can carry an immutable product snapshot. These are
        // separate physical boxes, with no owner chain or startup allocation.
        for &key in &result.objects {
            let data = program.unit(key.0).unwrap();
            let op = &data.operations[data.values[key.1.index()].definition.index()];
            for (position, &v) in data.operands(op.operands).unwrap().iter().enumerate() {
                let target = result.field_type(plan, key, position);
                if matches!(target, NativeType::Dynamic(_)) {
                    if let Some(Literal::Object(child)) = result.get((key.0, v)) {
                        if matches!(
                            plan.value_type(plan.units[child.0.index()].values[child.1.index()]),
                            NativeType::Struct(_)
                        ) {
                            result.boxes[child.0.index()][child.1.index()] = true;
                            plan.helpers.require(Helper::Products);
                        }
                    }
                }
            }
        }
        for row in &states {
            budget.release(Scratch, (row.capacity() * size_of::<u8>()) as u64)?;
        }
        budget.release(Scratch, (states.capacity() * size_of::<Vec<u8>>()) as u64)?;
        for row in &repeated {
            budget.release(Scratch, (row.capacity() * size_of::<bool>()) as u64)?;
        }
        budget.release(
            Scratch,
            (repeated.capacity() * size_of::<Vec<bool>>()) as u64,
        )?;
        budget.release(
            Scratch,
            (initializers.capacity() * size_of::<Option<Key>>()) as u64,
        )?;
        budget.release(
            Scratch,
            (stack.capacity() * size_of::<(Key, bool)>()) as u64,
        )?;
        Ok(result)
    }
    fn get(&self, key: Key) -> Option<Literal> {
        self.values
            .get(key.0.index())?
            .get(key.1.index())
            .copied()
            .flatten()
    }
    fn child(&self, plan: &NativePlan<'_, '_>, parent: Literal, slot: usize) -> Option<Literal> {
        let Literal::Object(key) = parent else {
            return None;
        };
        let data = plan.program.unit(key.0)?;
        let op = &data.operations[data.values[key.1.index()].definition.index()];
        let child = *data.operands(op.operands)?.get(slot)?;
        self.get((key.0, child))
    }
    fn named(&self, plan: &NativePlan<'_, '_>, parent: Literal, name: StringId) -> Option<Literal> {
        let Literal::Object(key) = parent else {
            return None;
        };
        let data = plan.program.unit(key.0)?;
        let op = &data.operations[data.values[key.1.index()].definition.index()];
        let OperationKind::Allocate { kind, .. } = &op.kind else {
            return None;
        };
        let keys = match kind {
            AllocationKind::Record(keys)
            | AllocationKind::Object(keys)
            | AllocationKind::Instance { keys, .. } => keys,
            _ => return None,
        };
        let slot = keys.iter().position(|key| {
            plan.program.strings[key.index()] == plan.program.strings[name.index()]
        })?;
        self.child(plan, parent, slot)
    }
    fn place(
        &self,
        plan: &NativePlan<'_, '_>,
        unit: UnitId,
        place: PlaceId,
        initializers: &[Option<Key>],
        budget: &mut AllocationBudget<'_>,
    ) -> Result<Option<Literal>, NativeError> {
        let mut path = Vec::new();
        let mut at = place;
        let mut result = loop {
            budget.work(WorkKind::Analysis, 1)?;
            match plan.units[unit.index()].places[at.index()].recipe {
                PlaceRecipe::Cell(cell) => {
                    break initializers[cell.index()].and_then(|key| self.get(key))
                }
                PlaceRecipe::Value(value) => break self.get((unit, value)),
                PlaceRecipe::Field { base, slot, .. } => {
                    budget.push(Scratch, &mut path, slot)?;
                    at = base;
                }
                PlaceRecipe::Member { receiver, slot, .. } => {
                    break self
                        .get((unit, receiver))
                        .and_then(|p| self.child(plan, p, slot))
                }
                PlaceRecipe::Element {
                    receiver, index, ..
                } => {
                    break match (self.get((unit, receiver)), self.get((unit, index))) {
                        (Some(parent), Some(Literal::Scalar(Constant::Integer(i)))) if i >= 0 => {
                            self.child(plan, parent, i as usize)
                        }
                        _ => None,
                    }
                }
                PlaceRecipe::Record { receiver, key, .. } => {
                    let key = match key {
                        RecordKey::Literal(key) => Some(key),
                        RecordKey::Value(v) => match self.get((unit, v)) {
                            Some(Literal::Scalar(Constant::String(key))) => Some(key),
                            _ => None,
                        },
                    };
                    break match (self.get((unit, receiver)), key) {
                        (Some(parent), Some(key)) => self.named(plan, parent, key),
                        _ => None,
                    };
                }
                _ => break None,
            }
        };
        for &slot in path.iter().rev() {
            result = result.and_then(|p| self.child(plan, p, slot));
        }
        budget.release(Scratch, (path.capacity() * size_of::<usize>()) as u64)?;
        Ok(result)
    }
    fn field_type(&self, plan: &NativePlan<'_, '_>, key: Key, slot: usize) -> NativeType {
        match plan.value_type(plan.units[key.0.index()].values[key.1.index()]) {
            NativeType::Array(a) => plan.arrays[a],
            NativeType::Struct(s) => plan.field_type(plan.program.structs[s].fields.start + slot),
            NativeType::Object(c) => plan.class_fields[c][slot],
            NativeType::Record | NativeType::Shape => NativeType::Dynamic(Tagged::ANY),
            _ => unreachable!("const allocation has a portable aggregate representation"),
        }
    }
    fn selected(&self, key: Key) -> bool {
        self.selected
            .get(key.0.index())
            .is_some_and(|values| values[key.1.index()])
    }
}

enum Init {
    Value(Literal, NativeType),
    Text(&'static str),
    Field(usize),
    Member(usize),
    Class(usize, Key, usize),
}
impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn static_allocation(
        &mut self,
        unit: UnitId,
        value: ValueId,
    ) -> Result<bool, NativeError> {
        if !self.static_storage.selected((unit, value)) {
            return Ok(false);
        }
        self.assignment_start(unit, Destination::Value(value), false)?;
        let ty = self
            .plan
            .value_type(self.plan.units[unit.index()].values[value.index()]);
        if !matches!(ty, NativeType::Struct(_)) {
            self.text("(")?;
            self.write(format_args!("{ty}"))?;
            self.text(")&")?;
        }
        self.write(format_args!("ls_static_{}_{}", unit.index(), value.index()))?;
        self.assignment_end(unit, Destination::Value(value))?;
        Ok(true)
    }
    fn static_owner(&mut self, destroy: fmt::Arguments<'_>) -> Result<(), NativeError> {
        self.write(format_args!("{{.references=SIZE_MAX,.destroy={destroy}}}"))
    }
    fn static_initializer(&mut self, value: Literal, ty: NativeType) -> Result<(), NativeError> {
        self.static_initializer_task(Init::Value(value, ty))
    }
    fn static_class_initializer(&mut self, key: Key, class: usize) -> Result<(), NativeError> {
        self.static_initializer_task(Init::Class(class, key, class))
    }
    fn static_initializer_task(&mut self, initial: Init) -> Result<(), NativeError> {
        let mut stack = Vec::new();
        self.budget.push(Scratch, &mut stack, initial)?;
        while let Some(task) = stack.pop() {
            self.budget.work(WorkKind::Render, 1)?;
            match task {
                Init::Text(text) => self.text(text)?,
                Init::Field(index) => self.write(format_args!(".ls_f{index}="))?,
                Init::Member(index) => self.write(format_args!(".ls_m{index}="))?,
                Init::Class(class, key, inherited) => {
                    // Class headers use the most-derived destructor witness.
                    let base = self.plan.program.classes[class]
                        .base
                        .and_then(|b| self.plan.program.class_index(b));
                    self.text("{")?;
                    self.budget.push(Scratch, &mut stack, Init::Text("}"))?;
                    for slot in (base.map_or(0, |b| self.plan.class_fields[b].len())
                        ..self.plan.class_fields[class].len())
                        .rev()
                    {
                        let data = self.plan.program.unit(key.0).unwrap();
                        let op = &data.operations[data.values[key.1.index()].definition.index()];
                        let child = data.operands(op.operands).unwrap()[slot];
                        self.budget.push(Scratch, &mut stack, Init::Text(","))?;
                        self.budget.push(
                            Scratch,
                            &mut stack,
                            Init::Value(
                                self.static_storage.get((key.0, child)).unwrap(),
                                self.plan.class_fields[class][slot],
                            ),
                        )?;
                        self.budget.push(Scratch, &mut stack, Init::Member(slot))?;
                    }
                    if let Some(base) = base {
                        self.text(".base=")?;
                        self.budget.push(Scratch, &mut stack, Init::Text(","))?;
                        self.budget
                            .push(Scratch, &mut stack, Init::Class(base, key, inherited))?;
                    } else {
                        self.text(".owner=")?;
                        self.static_owner(format_args!("ls_object{inherited}_destroy"))?;
                        self.text(",")?;
                    }
                }
                Init::Value(Literal::Scalar(c), to) => {
                    let tag = match c {
                        Constant::Integer(_) => "LS_INT",
                        Constant::Number(_) => "LS_FLOAT",
                        Constant::Boolean(_) => "LS_BOOL",
                        Constant::String(_) => "LS_STRING",
                        _ => "LS_NULL",
                    };
                    let tagged = matches!(to, NativeType::Dynamic(_));
                    if tagged {
                        self.write(format_args!("{{.tag={tag}"))?;
                        self.budget.push(Scratch, &mut stack, Init::Text("}"))?;
                    }
                    match c {
                        Constant::Integer(n) => {
                            if tagged {
                                self.text(",.as.i=")?;
                            }
                            if n == i32::MIN {
                                self.text("(-INT32_C(2147483647)-1)")?;
                            } else {
                                self.write(format_args!("INT32_C({n})"))?;
                            }
                        }
                        Constant::Number(bits) => {
                            if tagged {
                                self.text(",.as.f=")?;
                            }
                            let n = f64::from_bits(bits);
                            if n.is_nan() {
                                self.text("NAN")?;
                            } else if n.is_infinite() {
                                self.text(if n < 0.0 { "-INFINITY" } else { "INFINITY" })?;
                            } else if bits == 1u64 << 63 {
                                self.text("-0x0p0")?;
                            } else {
                                self.write(format_args!("{n:e}"))?;
                            }
                        }
                        Constant::Boolean(b) => {
                            if tagged {
                                self.text(",.as.b=")?;
                            }
                            self.text(if b { "true" } else { "false" })?;
                        }
                        Constant::String(id) => {
                            if tagged {
                                self.text(",.as.s=")?;
                            }
                            let len = self.plan.program.strings[id.index()].code_units().count();
                            if len == 0 {
                                self.text("{0}")?;
                            } else {
                                self.write(format_args!("{{ls_s{},{len},NULL}}", id.index()))?;
                            }
                        }
                        Constant::Null | Constant::Undefined => {
                            if !tagged {
                                self.text("{0}")?;
                            }
                        }
                    }
                }
                Init::Value(Literal::Object(key), to) => {
                    let actual = self
                        .plan
                        .value_type(self.plan.units[key.0.index()].values[key.1.index()]);
                    match (actual, to) {
                        (NativeType::Struct(s), NativeType::Struct(_)) => {
                            self.text("{")?;
                            self.budget.push(Scratch, &mut stack, Init::Text("}"))?;
                            let data = self.plan.program.unit(key.0).unwrap();
                            let op =
                                &data.operations[data.values[key.1.index()].definition.index()];
                            for (slot, &child) in
                                data.operands(op.operands).unwrap().iter().enumerate().rev()
                            {
                                self.budget.push(Scratch, &mut stack, Init::Text(","))?;
                                self.budget.push(
                                    Scratch,
                                    &mut stack,
                                    Init::Value(
                                        self.static_storage.get((key.0, child)).unwrap(),
                                        self.plan.field_type(
                                            self.plan.program.structs[s].fields.start + slot,
                                        ),
                                    ),
                                )?;
                                self.budget.push(Scratch, &mut stack, Init::Field(slot))?;
                            }
                        }
                        (NativeType::Struct(_), NativeType::Dynamic(_)) => self
                            .write(format_args!(
                            "{{.tag=LS_PRODUCT,.as.o=(ls_native_object *)&ls_static_box_{}_{}}}",
                            key.0.index(),
                            key.1.index()
                        ))?,
                        (NativeType::Array(_), NativeType::Dynamic(_)) => {
                            self.write(format_args!(
                                "{{.tag=LS_ARRAY,.as.o=(ls_native_object *)&ls_static_{}_{}}}",
                                key.0.index(),
                                key.1.index()
                            ))?
                        }
                        (_, NativeType::Dynamic(_)) => self.write(format_args!(
                            "{{.tag=LS_OBJECT,.as.o=(ls_native_object *)&ls_static_{}_{}}}",
                            key.0.index(),
                            key.1.index()
                        ))?,
                        _ => self.write(format_args!(
                            "({to})&ls_static_{}_{}",
                            key.0.index(),
                            key.1.index()
                        ))?,
                    }
                }
            }
        }
        self.budget
            .release(Scratch, (stack.capacity() * size_of::<Init>()) as u64)?;
        Ok(())
    }
    pub(super) fn static_graphs(&mut self) -> Result<(), NativeError> {
        for index in 0..self.static_storage.objects.len() {
            let key = self.static_storage.objects[index];
            let ty = self
                .plan
                .value_type(self.plan.units[key.0.index()].values[key.1.index()]);
            match ty {
                NativeType::Array(_) => self.text("static ls_native_array ")?,
                NativeType::Record | NativeType::Shape => self.text("static ls_map ")?,
                NativeType::Object(c) => self.write(format_args!("static ls_object{c} "))?,
                NativeType::Struct(_) => self.write(format_args!("static {ty} "))?,
                _ => unreachable!("selected immutable aggregate"),
            }
            self.write(format_args!(
                "ls_static_{}_{} LS_NATIVE_UNUSED;\n",
                key.0.index(),
                key.1.index()
            ))?;
            if let NativeType::Struct(s) = ty {
                if self.static_storage.boxes[key.0.index()][key.1.index()] {
                    self.write(format_args!(
                        "static ls_product{s} ls_static_box_{}_{} LS_NATIVE_UNUSED;\n",
                        key.0.index(),
                        key.1.index()
                    ))?;
                }
            }
        }
        for index in 0..self.static_storage.objects.len() {
            let key = self.static_storage.objects[index];
            let (u, v) = (key.0.index(), key.1.index());
            let data = self.plan.program.unit(key.0).unwrap();
            let op = &data.operations[data.values[v].definition.index()];
            let operands = data.operands(op.operands).unwrap();
            let len = operands.len();
            let ty = self.plan.value_type(self.plan.units[u].values[v]);
            match ty {
                NativeType::Array(a) => {
                    let element = self.plan.arrays[a];
                    if len > 0 {
                        self.write(format_args!(
                            "static {element} ls_static_items_{u}_{v}[{len}]={{"
                        ))?;
                        for &child in operands {
                            self.static_initializer(
                                self.static_storage.get((key.0, child)).unwrap(),
                                element,
                            )?;
                            self.text(",")?;
                        }
                        self.text("};\n")?;
                    }
                    self.write(format_args!(
                        "static ls_native_array ls_static_{u}_{v}={{.owner="
                    ))?;
                    self.static_owner(format_args!("ls_array_destroy"))?;
                    self.write(format_args!(
                        ",.ops=&ls_array{a}_ops,.length={len},.capacity={len},.items="
                    ))?;
                    if len == 0 {
                        self.text("NULL")?;
                    } else {
                        self.write(format_args!("ls_static_items_{u}_{v}"))?;
                    }
                    self.text("};\n")?;
                }
                NativeType::Record | NativeType::Shape => {
                    let OperationKind::Allocate { kind, .. } = &op.kind else {
                        unreachable!()
                    };
                    let keys = match kind {
                        AllocationKind::Record(k)
                        | AllocationKind::Object(k)
                        | AllocationKind::Instance { keys: k, .. } => k,
                        _ => unreachable!(),
                    };
                    let slots = len
                        .checked_mul(2)
                        .ok_or(AllocationError::Capacity)?
                        .max(8)
                        .checked_next_power_of_two()
                        .ok_or(AllocationError::Capacity)?;
                    let mut table = self.budget.filled(Scratch, slots, 0usize)?;
                    if len > 0 {
                        self.write(format_args!(
                            "static ls_map_entry ls_static_items_{u}_{v}[{len}]={{"
                        ))?;
                        for (position, (&key_id, &child)) in keys.iter().zip(operands).enumerate() {
                            self.text("{")?;
                            self.static_initializer(
                                Literal::Scalar(Constant::String(key_id)),
                                NativeType::Dynamic(Tagged::ANY),
                            )?;
                            self.text(",")?;
                            self.static_initializer(
                                self.static_storage.get((key.0, child)).unwrap(),
                                NativeType::Dynamic(Tagged::ANY),
                            )?;
                            self.text(",true},")?;
                            let hash = self.plan.program.strings[key_id.index()]
                                .code_units()
                                .fold(0xcbf29ce484222325u64, |h, c| {
                                    (h ^ u64::from(c)).wrapping_mul(0x100000001b3)
                                });
                            let mut slot = hash as usize & (slots - 1);
                            while table[slot] != 0 {
                                self.budget.work(WorkKind::Analysis, 1)?;
                                slot = (slot + 1) & (slots - 1);
                            }
                            table[slot] = position + 1;
                        }
                        self.text("};\n")?;
                    }
                    self.write(format_args!(
                        "static size_t ls_static_index_{u}_{v}[{slots}]={{"
                    ))?;
                    for entry in &table {
                        self.write(format_args!("{entry},"))?;
                    }
                    self.text("};\n")?;
                    self.budget
                        .release(Scratch, (table.capacity() * size_of::<usize>()) as u64)?;
                    self.write(format_args!("static ls_map ls_static_{u}_{v}={{.owner="))?;
                    self.static_owner(format_args!("ls_record_destroy"))?;
                    self.write(format_args!(",.used={len},.size={len},.capacity={len},.slots={slots},.index=ls_static_index_{u}_{v},.entries="))?;
                    if len == 0 {
                        self.text("NULL")?;
                    } else {
                        self.write(format_args!("ls_static_items_{u}_{v}"))?;
                    }
                    self.text("};\n")?;
                }
                NativeType::Struct(s) => {
                    self.write(format_args!("static ls_t{s} ls_static_{u}_{v}="))?;
                    self.static_initializer(Literal::Object(key), ty)?;
                    self.text(";\n")?;
                    if self.static_storage.boxes[u][v] {
                        self.write(format_args!(
                            "static ls_product{s} ls_static_box_{u}_{v}={{.owner="
                        ))?;
                        self.static_owner(format_args!("ls_product{s}_destroy"))?;
                        self.text(",.value=")?;
                        self.static_initializer(Literal::Object(key), ty)?;
                        self.text("};\n")?;
                    }
                }
                NativeType::Object(c) => {
                    self.write(format_args!("static ls_object{c} ls_static_{u}_{v}="))?;
                    self.static_class_initializer(key, c)?;
                    self.text(";\n")?;
                }
                _ => unreachable!(),
            }
        }
        Ok(())
    }
}
