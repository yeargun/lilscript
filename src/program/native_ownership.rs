//! Lifetime emission follows the original region tree and source evaluation
//! order. The admitted target rows classify boxes and callable slots; this
//! module adds no escape graph, call graph, deferred copy-out or cleanup CFG.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn value(&mut self, unit: UnitId, value: ValueId) -> Result<(), NativeError> {
        match self.plan.units[unit.index()].values[value.index()] {
            ValueStorage::Function(body) => {
                let signature = self.plan.signature_for_unit(body);
                self.write(format_args!(
                    "(ls_callable{signature}){{ls_adapter{0},NULL,UINT64_C({1})}}",
                    body.index(),
                    body.index() + 1
                ))
            }
            ValueStorage::Value(_) => self.write(format_args!("ls_v{}", value.index())),
            ValueStorage::Host(_) => unreachable!("native plan forbids escaping a provider symbol"),
        }
    }
    fn destination(&mut self, unit: UnitId, destination: Destination) -> Result<(), NativeError> {
        match destination {
            Destination::Value(value) => self.write(format_args!("ls_v{}", value.index())),
            Destination::Cell(cell) => self.cell_place(unit, cell),
            Destination::Place(place) => self.place(unit, place),
        }
    }
    fn destination_type(&self, unit: UnitId, destination: Destination) -> NativeType {
        match destination {
            Destination::Value(value) => self
                .plan
                .value_type(self.plan.units[unit.index()].values[value.index()]),
            Destination::Cell(cell) => self.plan.value_type(self.plan.cell_storage(cell)),
            Destination::Place(place) => self.plan.place_type(unit, place),
        }
    }
    pub(super) fn assignment_start(
        &mut self,
        unit: UnitId,
        destination: Destination,
        owned: bool,
    ) -> Result<(), NativeError> {
        if let Some(prefix) = self.destination_type(unit, destination).owner_prefix() {
            self.write(format_args!(
                "{prefix}_{}(&(",
                if owned { "take" } else { "copy" }
            ))?;
            self.destination(unit, destination)?;
            self.text("),")
        } else {
            self.destination(unit, destination)?;
            self.text(" = ")
        }
    }
    pub(super) fn assignment_end(
        &mut self,
        unit: UnitId,
        destination: Destination,
    ) -> Result<(), NativeError> {
        if self.destination_type(unit, destination).managed() {
            self.text(")")?;
        }
        self.text(";\n")
    }
    pub(super) fn copy_value(
        &mut self,
        unit: UnitId,
        destination: Destination,
        source: ValueId,
    ) -> Result<(), NativeError> {
        let to = self.destination_type(unit, destination);
        self.assignment_start(unit, destination, false)?;
        self.converted(unit, source, to)?;
        self.assignment_end(unit, destination)
    }
    pub(super) fn cell_place(&mut self, unit: UnitId, cell: CellId) -> Result<(), NativeError> {
        if self.plan.global_cell(cell) && self.plan.program.cells[cell.index()].owner != unit {
            self.write(format_args!("(*ls_g{}())", cell.index()))
        } else if self.plan.boxed_cell(cell) {
            self.box_pointer(unit, cell)?;
            self.text("->value")
        } else if self.plan.reference_parameter(cell) {
            self.write(format_args!("(*ls_c{})", cell.index()))
        } else {
            self.write(format_args!("ls_c{}", cell.index()))
        }
    }
    fn box_pointer(&mut self, unit: UnitId, cell: CellId) -> Result<(), NativeError> {
        if self.plan.program.cells[cell.index()].owner == unit {
            self.write(format_args!("ls_c{}", cell.index()))
        } else {
            let captures = &self.plan.program.unit(unit).unwrap().captures;
            self.budget.work(WorkKind::Render, captures.len() as u64)?;
            let slot = self
                .plan
                .capture_slot(unit, cell)
                .expect("native plan proves inherited boxed capture");
            self.write(format_args!(
                "((ls_env{} *)ls_env)->ls_e{slot}",
                unit.index()
            ))
        }
    }

    pub(super) fn capture_types(&mut self) -> Result<(), NativeError> {
        for (index, cell) in self.plan.cells.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !cell.captured {
                continue;
            }
            let ty = self.plan.value_type(cell.storage);
            self.write(format_args!(
                "typedef struct {{ ls_native_object owner; {ty} value; }} ls_box{index};\n"
            ))?;
            // A box owns a managed payload: it retains on creation and
            // releases when the last closure sharing it goes.
            let destroy = if let Some(prefix) = ty.owner_prefix() {
                self.write(format_args!("static void ls_box_destroy{index}(ls_native_object *owner) {{ {prefix}_clear(&((ls_box{index} *)owner)->value); }}\n"))?;
                format!("ls_box_destroy{index}")
            } else {
                "NULL".to_owned()
            };
            let retain = ty.retain("value").unwrap_or_default();
            self.write(format_args!("static ls_box{index} *ls_box_new{index}({ty} value) {{\nls_box{index} *box = ls_native_allocate(sizeof *box,{destroy});\n{retain}box->value = value;\nreturn box;\n}}\n"))?;
        }
        for frozen in &self.plan.program.units {
            let unit = frozen.id();
            self.budget.work(WorkKind::Render, 1)?;
            if !self.plan.units[unit.index()].has_environment || !self.has_boxes(unit)? {
                continue;
            }
            self.write(format_args!("typedef struct {{\nls_native_object owner;\n"))?;
            for (slot, cell) in frozen.data().captures.iter().copied().enumerate() {
                self.budget.work(WorkKind::Render, 1)?;
                if self.plan.boxed_cell(cell) {
                    self.write(format_args!("ls_box{} *ls_e{slot};\n", cell.index()))?;
                }
            }
            self.write(format_args!("}} ls_env{0};\nstatic void ls_env_destroy{0}(ls_native_object *owner) {{\nls_env{0} *environment = (ls_env{0} *)owner;\n",unit.index()))?;
            for (slot, cell) in frozen.data().captures.iter().copied().enumerate() {
                self.budget.work(WorkKind::Render, 1)?;
                if self.plan.boxed_cell(cell) {
                    self.write(format_args!(
                        "ls_native_release(environment->ls_e{slot});\n"
                    ))?;
                }
            }
            self.text("}\n")?;
        }
        Ok(())
    }
    fn has_boxes(&mut self, unit: UnitId) -> Result<bool, NativeError> {
        let captures = &self.plan.program.unit(unit).unwrap().captures;
        self.budget.work(WorkKind::Render, captures.len() as u64)?;
        Ok(captures.iter().any(|&cell| self.plan.boxed_cell(cell)))
    }
    pub(super) fn closure_recipes(&mut self) -> Result<(), NativeError> {
        for frozen in &self.plan.program.units {
            let unit = frozen.id();
            self.budget.work(WorkKind::Render, 1)?;
            if self.plan.named_adapter_needed(unit) {
                let signature = self.plan.signature_for_unit(unit);
                let result = self.plan.signatures[signature].result;
                self.write(format_args!(
                    "static {result} ls_adapter{}(void *environment",
                    unit.index()
                ))?;
                self.signature_parameters(signature, true, true)?;
                self.text(") {\n(void)environment;\n")?;
                if result != NativeType::Void {
                    self.text("return ")?;
                }
                self.write(format_args!("ls_fn{}(", unit.index()))?;
                self.signature_arguments(signature, false)?;
                self.text(");\n}\n")?;
            }
            if !self.plan.units[unit.index()].has_environment {
                continue;
            }
            let signature = self.plan.signature_for_unit(unit);
            let boxed = self.has_boxes(unit)?;
            self.write(format_args!(
                "static ls_callable{signature} ls_closure{}(",
                unit.index()
            ))?;
            let mut emitted = false;
            for (slot, cell) in frozen.data().captures.iter().copied().enumerate() {
                self.budget.work(WorkKind::Render, 1)?;
                if self.plan.boxed_cell(cell) {
                    if emitted {
                        self.text(",")?;
                    }
                    self.write(format_args!("ls_box{} *ls_e{slot}", cell.index()))?;
                    emitted = true;
                }
            }
            if !emitted {
                self.text("void")?;
            }
            self.text(") {\n")?;
            if boxed {
                self.write(format_args!("ls_env{0} *environment = ls_native_allocate(sizeof *environment,ls_env_destroy{0});\n",unit.index()))?;
                for (slot, cell) in frozen.data().captures.iter().copied().enumerate() {
                    self.budget.work(WorkKind::Render, 1)?;
                    if self.plan.boxed_cell(cell) {
                        self.write(format_args!("ls_native_retain(ls_e{slot});\nenvironment->ls_e{slot} = ls_e{slot};\n"))?;
                    }
                }
            }
            self.write(format_args!(
                "return (ls_callable{signature}){{ls_fn{},{},ls_native_fresh_identity()}};\n}}\n",
                unit.index(),
                if boxed { "environment" } else { "NULL" }
            ))?;
        }
        Ok(())
    }
    pub(super) fn create_closure(
        &mut self,
        unit: UnitId,
        body: UnitId,
        result: ValueId,
    ) -> Result<(), NativeError> {
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        if self.plan.units[body.index()].has_environment {
            self.write(format_args!("ls_closure{}(", body.index()))?;
            let mut emitted = false;
            for &cell in &self.plan.program.unit(body).unwrap().captures {
                self.budget.work(WorkKind::Render, 1)?;
                if self.plan.boxed_cell(cell) {
                    if emitted {
                        self.text(",")?;
                    }
                    self.box_pointer(unit, cell)?;
                    emitted = true;
                }
            }
            self.text(")")?;
        } else {
            // A noncanonical creation of a named body is still a fresh source
            // function object. A named declaration load has stable identity.
            let signature = self.plan.signature_for_unit(body);
            self.write(format_args!(
                "(ls_callable{signature}){{ls_adapter{},NULL,ls_native_fresh_identity()}}",
                body.index()
            ))?;
        }
        self.assignment_end(unit, destination)
    }
    pub(super) fn parameter_owners(&mut self, unit: UnitId) -> Result<(), NativeError> {
        for &cell in &self.plan.program.unit(unit).unwrap().parameters {
            self.budget.work(WorkKind::Render, 1)?;
            if self.plan.boxed_cell(cell) {
                self.write(format_args!(
                    "ls_box{0} *ls_c{0} = ls_box_new{0}(ls_p{0});\n",
                    cell.index()
                ))?;
            } else if let ValueStorage::Value(ty) = self.plan.cell_storage(cell) {
                // A parameter owns what it holds for the call's duration.
                if let Some(retain) = ty.retain(&format!("ls_c{}", cell.index())) {
                    self.text(&retain)?;
                }
            }
        }
        Ok(())
    }
    fn cleanup_cell(&mut self, cell: CellId) -> Result<(), NativeError> {
        if self.plan.global_cell(cell) {
            // Cleared when `main` ends: functions may still use it.
            return Ok(());
        }
        if self.plan.boxed_cell(cell) {
            self.write(format_args!(
                "ls_native_release(ls_c{0});\nls_c{0} = NULL;\n",
                cell.index()
            ))?;
        } else if let ValueStorage::Value(ty) = self.plan.cell_storage(cell) {
            if let Some(prefix) = ty.owner_prefix() {
                self.write(format_args!("{prefix}_clear(&ls_c{});\n", cell.index()))?;
            }
        }
        Ok(())
    }
    pub(super) fn cleanup_region(
        &mut self,
        unit: UnitId,
        region: RegionId,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        // Slots are initially empty, and normal scope exits clear them. A
        // static return can mention later slots safely without a second graph
        // of path-dependent ownership facts or a runtime cleanup registry.
        for &op in data.regions[region.index()].operations.iter().rev() {
            self.budget.work(WorkKind::Render, 1)?;
            let operation = &data.operations[op.index()];
            if let Some(value) = operation.result {
                if let ValueStorage::Value(ty) = self.plan.units[unit.index()].values[value.index()]
                {
                    if let Some(prefix) = ty.owner_prefix() {
                        self.write(format_args!("{prefix}_clear(&ls_v{});\n", value.index()))?;
                    }
                }
            }
            if let OperationKind::Initialize(cell) = operation.kind {
                self.cleanup_cell(cell)?;
            }
        }
        if region == data.entry {
            for &cell in data.parameters.iter().rev() {
                self.budget.work(WorkKind::Render, 1)?;
                self.cleanup_cell(cell)?;
            }
        }
        Ok(())
    }
    pub(super) fn cleanup_path(
        &mut self,
        unit: UnitId,
        stop_before: Option<RegionId>,
    ) -> Result<(), NativeError> {
        for index in (0..self.active_regions.len()).rev() {
            let region = self.active_regions[index];
            if Some(region) == stop_before {
                break;
            }
            self.cleanup_region(unit, region)?;
        }
        Ok(())
    }
    pub(super) fn return_value(
        &mut self,
        unit: UnitId,
        value: Option<ValueId>,
    ) -> Result<(), NativeError> {
        let ty = self.plan.units[unit.index()].return_type;
        self.text("{\n")?;
        if ty != NativeType::Void {
            self.write(format_args!("{ty} ls_return = "))?;
            self.converted(unit, value.expect("native nonvoid return"), ty)?;
            self.text(";\n")?;
            if let Some(retain) = ty.retain("ls_return") {
                self.text(&retain)?;
            }
        }
        self.cleanup_path(unit, None)?;
        self.text(if ty == NativeType::Void {
            "return;\n}\n"
        } else {
            "return ls_return;\n}\n"
        })
    }
}
