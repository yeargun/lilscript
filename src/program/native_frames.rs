//! Suspension storage and iterator scheduling reuse the common region writer.
//! Frame members are typed source slots; program counters are emitted labels,
//! never a second CFG or an interpreted instruction stream.
use super::*;

#[derive(Clone, Copy)]
enum SlotName {
    PendingReturn,
    SavedReturn(usize),
    Cell(usize),
    Value(usize),
    Callee(usize),
    Completion(usize),
    Exception(usize),
    Iterator(usize),
    Next(usize),
}
impl fmt::Display for SlotName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (prefix, index) = match *self {
            Self::PendingReturn => return f.write_str("ls_pending_return"),
            Self::SavedReturn(n) => ("ls_saved_return", n),
            Self::Cell(n) => ("ls_c", n),
            Self::Value(n) => ("ls_v", n),
            Self::Callee(n) => ("ls_pc", n),
            Self::Completion(n) => ("ls_completion", n),
            Self::Exception(n) => ("ls_saved", n),
            Self::Iterator(n) => ("ls_iterator", n),
            Self::Next(n) => ("ls_next", n),
        };
        write!(f, "{prefix}{index}")
    }
}
#[derive(Clone, Copy)]
enum SlotType {
    Value(NativeType),
    Box(CellId),
    Exception,
    Iterator,
    Int,
}
#[derive(Clone, Copy)]
struct Slot {
    name: SlotName,
    ty: SlotType,
}
impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn generator_parameters(&self, unit: UnitId) -> Option<RegionId> {
        let data = self.plan.program.unit(unit).unwrap();
        let region = data.parameter_region?;
        data.regions[data.entry.index()].operations.iter().any(|&op|matches!(data.operations[op.index()].kind,OperationKind::Block(child) if child==region)).then_some(region)
    }
    fn frame_slots(&mut self, unit: UnitId) -> Result<Vec<Slot>, NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let mut slots = Vec::new();
        let mut cell = |id: CellId| -> Result<(), NativeError> {
            if self.plan.global_cell(id) {
                return Ok(());
            }
            let ty = if self.plan.boxed_cell(id) {
                SlotType::Box(id)
            } else if let ValueStorage::Value(ty) = self.plan.cell_storage(id) {
                SlotType::Value(ty)
            } else {
                return Ok(());
            };
            self.budget.push(
                AllocationClass::Scratch,
                &mut slots,
                Slot {
                    name: SlotName::Cell(id.index()),
                    ty,
                },
            )?;
            Ok(())
        };
        for &id in &data.parameters {
            cell(id)?;
        }
        for &(_, id) in &self.plan.units[unit.index()].catch_bindings {
            cell(id)?;
        }
        for op in &data.operations {
            if let OperationKind::Initialize(id) | OperationKind::Declare(id) = op.kind {
                cell(id)?;
            }
        }
        for (index, &storage) in self.plan.units[unit.index()].values.iter().enumerate() {
            if let ValueStorage::Value(ty) = storage {
                if ty != NativeType::Void {
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut slots,
                        Slot {
                            name: SlotName::Value(index),
                            ty: SlotType::Value(ty),
                        },
                    )?;
                }
            }
        }
        for (index, target) in self.plan.units[unit.index()].calls.iter().enumerate() {
            if let PreparedTarget::Placed { signature, .. } = *target {
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut slots,
                    Slot {
                        name: SlotName::Callee(index),
                        ty: SlotType::Value(NativeType::Callable(signature)),
                    },
                )?;
            }
        }
        let return_type = self.plan.units[unit.index()].return_type;
        if return_type != NativeType::Void {
            self.budget.push(
                AllocationClass::Scratch,
                &mut slots,
                Slot {
                    name: SlotName::PendingReturn,
                    ty: SlotType::Value(return_type),
                },
            )?;
        }
        for (index, op) in data.operations.iter().enumerate() {
            if matches!(
                op.kind,
                OperationKind::Try {
                    finally: Some(_),
                    ..
                }
            ) {
                if return_type != NativeType::Void {
                    self.budget.push(
                        AllocationClass::Scratch,
                        &mut slots,
                        Slot {
                            name: SlotName::SavedReturn(index),
                            ty: SlotType::Value(return_type),
                        },
                    )?;
                }
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut slots,
                    Slot {
                        name: SlotName::Completion(index),
                        ty: SlotType::Int,
                    },
                )?;
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut slots,
                    Slot {
                        name: SlotName::Exception(index),
                        ty: SlotType::Exception,
                    },
                )?;
            }
            if matches!(
                op.kind,
                OperationKind::ForOf { .. } | OperationKind::Yield { delegate: true }
            ) {
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut slots,
                    Slot {
                        name: SlotName::Iterator(index),
                        ty: SlotType::Iterator,
                    },
                )?;
                // Borrowed until the operation converts/retains the payload.
                self.budget.push(
                    AllocationClass::Scratch,
                    &mut slots,
                    Slot {
                        name: SlotName::Next(index),
                        ty: SlotType::Value(NativeType::Dynamic(Tagged::PLAIN)),
                    },
                )?;
            }
        }
        self.budget.work(
            WorkKind::Analysis,
            (data.operations.len() + data.values.len() + data.parameters.len() + data.calls.len())
                as u64,
        )?;
        Ok(slots)
    }
    fn release_frame_slots(&mut self, slots: Vec<Slot>) -> Result<(), NativeError> {
        let bytes = slots.capacity() * size_of::<Slot>();
        drop(slots);
        self.budget
            .release(AllocationClass::Scratch, bytes as u64)?;
        Ok(())
    }
    pub(super) fn suspension_frames(&mut self) -> Result<(), NativeError> {
        if self.plan.helpers.contains(Helper::Iterators)
            && self.plan.helpers.contains(Helper::Binary)
        {
            for kind in crate::typed_array::TypedArrayKind::ALL {
                let name = binary::kind_name(kind);
                let boxed = if kind.element_is_float() {
                    "float"
                } else {
                    "int"
                };
                self.write(format_args!("static LS_NATIVE_UNUSED bool ls_iteration_{name}(ls_iteration *iterator,ls_value *value,ls_native_temporary **temps) {{\n(void)temps;if(iterator->index>=(size_t)ls_typed_length(iterator->source)) return false;\n*value=ls_value_{boxed}(ls_typed_get_{name}(iterator->source,(int32_t)iterator->index++));return !ls_native_raised;\n}}\n"))?;
            }
        }
        for frozen in &self.plan.program.units {
            if frozen.data().suspension == Suspension::None
                || !self.plan.created[frozen.id().index()]
            {
                continue;
            }
            let unit = frozen.id();
            let index = unit.index();
            let generator = frozen.data().suspension == Suspension::Generator;
            let base = if generator {
                "ls_generator"
            } else {
                "ls_async_frame"
            };
            let close = if generator { ",bool" } else { "" };
            let slots = self.frame_slots(unit)?;
            self.write(format_args!(
                "typedef struct {{{base} base;ls_native_arguments arguments;\n"
            ))?;
            if !frozen.data().parameters.is_empty() {
                self.write(format_args!(
                    "bool absent[{}];\n",
                    frozen.data().parameters.len()
                ))?;
            }
            for slot in &slots {
                match slot.ty {
                    SlotType::Value(ty) => self.write(format_args!("{ty} "))?,
                    SlotType::Box(cell) => self.write(format_args!("ls_box{} *", cell.index()))?,
                    SlotType::Exception => self.text("ls_exception_state ")?,
                    SlotType::Iterator => self.text("ls_iteration ")?,
                    SlotType::Int => self.text("int ")?,
                }
                self.write(format_args!("{};\n", slot.name))?;
            }
            self.write(format_args!("}} ls_frame{index};\nstatic void ls_step{index}({base} *{close});\nstatic void ls_frame_clear{index}({base} *base) {{\nls_frame{index} *frame=(ls_frame{index} *)base;(void)frame;\n"))?;
            for slot in &slots {
                let name = slot.name;
                match slot.ty {
                    SlotType::Value(ty) => {
                        if let Some(prefix) = ty.owner_prefix() {
                            self.write(format_args!("{prefix}_clear(&frame->{name});\n"))?;
                        }
                    }
                    SlotType::Box(_) => self.write(format_args!(
                        "ls_native_release(frame->{name});frame->{name}=NULL;\n"
                    ))?,
                    SlotType::Exception => {
                        self.write(format_args!("ls_exception_clear(&frame->{name});\n"))?
                    }
                    SlotType::Iterator => {
                        self.write(format_args!("ls_iteration_dispose(&frame->{name});\n"))?
                    }
                    _ => {}
                }
            }
            self.write(format_args!("}}\nstatic void ls_frame_trace{index}({base} *base,ls_native_visit visit,void *context) {{\nls_frame{index} *frame=(ls_frame{index} *)base;(void)frame;(void)visit;(void)context;\n"))?;
            for slot in &slots {
                let name = slot.name;
                match slot.ty {
                    SlotType::Value(ty) => {
                        if let Some(trace) = ty.trace(&format!("frame->{name}")) {
                            self.text(&trace)?;
                        }
                    }
                    SlotType::Box(_) => {
                        self.write(format_args!("visit(frame->{name},context);\n"))?
                    }
                    SlotType::Exception => self.write(format_args!(
                        "ls_value_trace(frame->{name}.value,visit,context);\n"
                    ))?,
                    SlotType::Iterator => {
                        self.write(format_args!("visit(frame->{name}.source,context);\n"))?
                    }
                    _ => {}
                }
            }
            self.text("}\n")?;
            self.release_frame_slots(slots)?;
        }
        Ok(())
    }
    pub(super) fn suspension_begin(&mut self, unit: UnitId) -> Result<(), NativeError> {
        let index = unit.index();
        let slots = self.frame_slots(unit)?;
        for slot in &slots {
            self.write(format_args!("#define {0} (ls_frame->{0})\n", slot.name))?;
        }
        self.text(
            "#define ls_env (ls_frame->base.environment)\n#define ls_args (ls_frame->arguments)\n",
        )?;
        let generator = self.plan.program.unit(unit).unwrap().suspension == Suspension::Generator;
        let base = if generator {
            "ls_generator"
        } else {
            "ls_async_frame"
        };
        let close = if generator {
            ",bool ls_closing LS_NATIVE_UNUSED"
        } else {
            ""
        };
        self.write(format_args!("static void ls_step{index}({base} *base{close}) {{\nls_frame{index} *ls_frame=(ls_frame{index} *)base;\n"))?;
        self.temporary_declaration()?;
        self.text("switch(base->pc) {case 0:break;\n")?;
        for (op, operation) in self
            .plan
            .program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .enumerate()
        {
            if matches!(
                operation.kind,
                OperationKind::Yield { .. } | OperationKind::Await
            ) {
                self.write(format_args!("case {}:goto ls_resume{op};\n", op + 1))?;
            }
        }
        if self.generator_parameters(unit).is_some() {
            self.text("case SIZE_MAX:goto ls_body;\n")?;
        }
        self.text("default:ls_native_resource_failure();}\n")?;
        self.release_frame_slots(slots)
    }
    pub(super) fn suspension_factory(&mut self, unit: UnitId) -> Result<(), NativeError> {
        let index = unit.index();
        let slots = self.frame_slots(unit)?;
        for slot in &slots {
            self.write(format_args!("#undef {}\n", slot.name))?;
        }
        self.text("#undef ls_env\n#undef ls_args\n")?;
        self.release_frame_slots(slots)?;
        self.signature(unit)?;
        self.text(" {\n")?;
        let generator = self.plan.program.unit(unit).unwrap().suspension == Suspension::Generator;
        let runtime = if generator {
            "ls_generator"
        } else {
            "ls_async"
        };
        self.write(format_args!("ls_frame{index} *frame=ls_native_allocate(sizeof *frame,{runtime}_destroy,{runtime}_trace);\nframe->base.step=ls_step{index};frame->base.clear=ls_frame_clear{index};frame->base.trace_slots=ls_frame_trace{index};\n"))?;
        if !generator {
            self.text("ls_task *result=ls_task_new();frame->base.result=result;ls_native_retain(result);\n")?;
        }
        let data = self.plan.program.unit(unit).unwrap();
        if self.plan.units[index].has_environment {
            self.text("frame->base.environment=ls_env;ls_native_retain(ls_env);\n")?;
        }
        for &cell in &data.parameters {
            if self.plan.boxed_cell(cell) {
                self.write(format_args!(
                    "frame->ls_c{0}=ls_box_new{0}(ls_p{0});\n",
                    cell.index()
                ))?;
            } else if let ValueStorage::Value(ty) = self.plan.cell_storage(cell) {
                self.write(format_args!("frame->ls_c{0}=ls_c{0};\n", cell.index()))?;
                if let Some(retain) = ty.retain(&format!("frame->ls_c{}", cell.index())) {
                    self.text(&retain)?;
                }
            }
        }
        if self.plan.signatures[self.plan.signature_for_unit(unit)].has_optional() {
            self.text("frame->arguments=ls_args;\n")?;
            if !data.parameters.is_empty() {
                self.write(format_args!("if(ls_args.absent) {{for(size_t i=0;i<ls_args.count && i<{};++i) frame->absent[i]=ls_args.absent[i];frame->arguments.absent=frame->absent;}}\n",data.parameters.len()))?;
            }
        }
        if self.generator_parameters(unit).is_some() {
            self.write(format_args!("ls_step{index}(&frame->base,false);\nif(ls_native_raised) {{ls_native_release(frame);return NULL;}}\n"))?;
        }
        if generator {
            self.text("return &frame->base.owner;\n}\n")
        } else {
            self.write(format_args!("ls_step{index}(&frame->base);ls_native_release(frame);return &result->owner;\n}}\n"))
        }
    }
    pub(super) fn iteration_declarations(&mut self, unit: UnitId) -> Result<(), NativeError> {
        for (index, operation) in self
            .plan
            .program
            .unit(unit)
            .unwrap()
            .operations
            .iter()
            .enumerate()
        {
            if matches!(operation.kind, OperationKind::ForOf { .. }) {
                self.write(format_args!(
                    "ls_iteration ls_iterator{index}={{0}};ls_value ls_next{index}={{0}};\n"
                ))?;
            }
        }
        Ok(())
    }
    fn iterator_recipe(&self, ty: NativeType) -> String {
        match ty {
            NativeType::Generator => "ls_iteration_generator".into(),
            NativeType::Set => "ls_iteration_set".into(),
            NativeType::Array(_) => "ls_iteration_array".into(),
            NativeType::Typed(kind) => format!("ls_iteration_{}", binary::kind_name(kind)),
            _ => unreachable!("admitted iterable representation"),
        }
    }
    pub(super) fn for_of(
        &mut self,
        unit: UnitId,
        op: OpId,
        item: CellId,
        body: RegionId,
        source: ValueId,
    ) -> Result<(), NativeError> {
        let index = op.index();
        let ty = self
            .plan
            .value_type(self.plan.units[unit.index()].values[source.index()]);
        let recipe = self.iterator_recipe(ty);
        self.write(format_args!(
            "ls_iteration_start(&ls_iterator{index},(ls_native_object *)ls_v{},{recipe});\n",
            source.index()
        ))?;
        self.check_exception(unit)?;
        self.write(format_args!("ls_update{index}: LS_NATIVE_UNUSED;\nif(!ls_iteration_next(&ls_iterator{index},&ls_next{index},&ls_temps)) goto ls_end{index};\n"))?;
        self.push(Task::ForOfEnd(op))?;
        self.push(Task::ForOfBody {
            operation: op,
            item,
            body,
        })
    }
    pub(super) fn for_of_body(
        &mut self,
        unit: UnitId,
        op: OpId,
        item: CellId,
        body: RegionId,
    ) -> Result<(), NativeError> {
        self.region_owners(unit, body)?;
        self.budget
            .push(AllocationClass::Scratch, &mut self.active_regions, body)?;
        let ty = self.plan.value_type(self.plan.cell_storage(item));
        let (prefix, suffix) = Self::conversion(NativeType::Dynamic(Tagged::ANY), ty);
        if self.plan.boxed_cell(item) {
            if self.storage.stack_cell(item) {
                self.write(format_args!("ls_sb{0}=(ls_box{0}){{.owner={{.references=SIZE_MAX-1}}}}; ls_c{0}=&ls_sb{0}; ls_box_initialize{0}(ls_c{0},{prefix}ls_next{1}{suffix});\n",item.index(),op.index()))?;
            } else {
                self.write(format_args!("ls_c{0}=ls_box_new{0}({prefix}ls_next{1}{suffix});\n",item.index(),op.index()))?;
            }
        } else {
            self.assignment_start(unit, Destination::Cell(item), false)?;
            self.write(format_args!("{prefix}ls_next{}{suffix}", op.index()))?;
            self.assignment_end(unit, Destination::Cell(item))?;
        }
        self.check_exception_region(body)?;
        self.clear_temporaries()?;
        self.push(Task::Region {
            region: body,
            next: 0,
            enclosing_loop: Some(op),
            destination: None,
        })
    }
    pub(super) fn for_of_end(&mut self, unit: UnitId, op: OpId) -> Result<(), NativeError> {
        let index = op.index();
        self.write(format_args!(
            "goto ls_update{index};\nls_end{index}:;ls_iteration_dispose(&ls_iterator{index});\n"
        ))?;
        self.operation = Some(op);
        self.check_exception(unit)
    }
    pub(super) fn yield_value(
        &mut self,
        unit: UnitId,
        op: OpId,
        value: ValueId,
        delegate: bool,
    ) -> Result<(), NativeError> {
        let index = op.index();
        if delegate {
            let ty = self
                .plan
                .value_type(self.plan.units[unit.index()].values[value.index()]);
            let recipe = self.iterator_recipe(ty);
            self.write(format_args!("ls_iteration_start(&ls_iterator{index},(ls_native_object *)ls_v{},{recipe});\nls_delegate_next{index}:;\nif(!ls_iteration_next(&ls_iterator{index},&ls_next{index},&ls_temps)) goto ls_delegate_done{index};\nls_delegate_yield{index}:;\nls_value_copy(&base->yielded,ls_next{index});\n",value.index()))?;
        } else {
            self.text("ls_value_copy(&base->yielded,")?;
            self.converted(unit, value, NativeType::Dynamic(Tagged::ANY))?;
            self.text(");\n")?;
        }
        self.check_exception(unit)?;
        self.clear_temporaries()?;
        self.write(format_args!(
            "base->pc={};return;\nls_resume{index}:;\nif(ls_closing) {{ls_closing=false;\n",
            index + 1
        ))?;
        if delegate {
            self.write(format_args!("if(ls_iterator{index}.next==ls_iteration_generator && ls_iterator{index}.source) {{\nls_generator_close(ls_iterator{index}.source);\n"))?;
            self.check_exception(unit)?;
            self.write(format_args!("if(!((ls_generator *)ls_iterator{index}.source)->done) {{ls_next{index}=((ls_generator *)ls_iterator{index}.source)->yielded;goto ls_delegate_yield{index};}}\n}}\nls_iteration_dispose(&ls_iterator{index});\n"))?;
        }
        self.complete(unit, Completion::Return)?;
        self.text("}\n")?;
        if delegate {
            self.write(format_args!("goto ls_delegate_next{index};\nls_delegate_done{index}:;ls_iteration_dispose(&ls_iterator{index});\n"))?;
            self.check_exception(unit)?;
        }
        Ok(())
    }
    /// Called only for abrupt lexical exit. Normal iteration cleanup and a
    /// continue targeting this same loop leave the iterator open.
    pub(super) fn close_region_iterator(
        &mut self,
        unit: UnitId,
        region: RegionId,
        action: Completion,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let parent = data.regions[region.index()].parent;
        let Some(parent) = parent else {
            return Ok(());
        };
        for &id in &data.regions[parent.index()].operations {
            self.budget.work(WorkKind::Render, 1)?;
            if matches!(data.operations[id.index()].kind,OperationKind::ForOf {body,..} if body==region)
                && action != Completion::Continue(id)
            {
                self.write(format_args!(
                    "ls_iteration_close(&ls_iterator{});\n",
                    id.index()
                ))?;
                if action != Completion::Throw {
                    self.text("if(ls_native_raised) {\n")?;
                    let depth = self
                        .active_regions
                        .iter()
                        .position(|&r| r == parent)
                        .unwrap();
                    self.complete_at(unit, Completion::Throw, depth)?;
                    self.text("}\n")?;
                }
                break;
            }
        }
        Ok(())
    }
}
