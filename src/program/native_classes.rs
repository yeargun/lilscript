//! Class instances: one reference-counted C struct per class. A derived
//! class embeds its base as its first member, so the flattened field list
//! (base fields first) is laid out once and an upcast needs no conversion.
//! Every class slot has type `ls_native_object *`; a member access casts to
//! the class that declares the field. Reference counting does not collect
//! cycles, so an instance reachable from itself is never freed.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    fn base_class(&self, class: usize) -> Option<usize> {
        let program = self.plan.program;
        program.classes[class]
            .base
            .and_then(|base| program.class_index(base))
    }

    pub(super) fn class_types(&mut self) -> Result<(), NativeError> {
        if !self.plan.uses_objects() {
            return Ok(());
        }
        self.text(
            "static void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }\n\
static void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }\n\
static void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }\n",
        )?;
        let program = self.plan.program;
        let mut emitted =
            self.budget
                .filled(AllocationClass::Scratch, program.classes.len(), false)?;
        let mut chain = self
            .budget
            .vector(AllocationClass::Scratch, program.classes.len())?;
        for root in 0..program.classes.len() {
            // Bases first: a derived struct embeds its base by value.
            chain.clear();
            let mut class = Some(root);
            while let Some(current) = class {
                self.budget.work(WorkKind::Render, 1)?;
                if emitted[current] || program.classes[current].external {
                    break;
                }
                chain.push(current);
                class = self.base_class(current);
            }
            for &class in chain.iter().rev() {
                self.budget.work(WorkKind::Render, 1)?;
                emitted[class] = true;
                let base = self.base_class(class);
                let inherited = base.map_or(0, |base| program.classes[base].fields.len());
                self.write(format_args!("typedef struct ls_object{class} {{\n"))?;
                match base {
                    Some(base) => self.write(format_args!("ls_object{base} base;\n"))?,
                    None => self.text("ls_native_object owner;\n")?,
                }
                for slot in inherited..self.plan.class_fields[class].len() {
                    let ty = self.plan.class_fields[class][slot];
                    self.write(format_args!("{ty} ls_m{slot};\n"))?;
                }
                self.write(format_args!(
                    "}} ls_object{class};\nstatic void ls_object{class}_clear_fields(ls_object{class} *object) {{\n(void)object;\n"
                ))?;
                for slot in inherited..self.plan.class_fields[class].len() {
                    let ty = self.plan.class_fields[class][slot];
                    if let Some(prefix) = ty.owner_prefix() {
                        self.write(format_args!("{prefix}_clear(&object->ls_m{slot});\n"))?;
                    }
                }
                if let Some(base) = base {
                    self.write(format_args!(
                        "ls_object{base}_clear_fields(&object->base);\n"
                    ))?;
                }
                self.write(format_args!(
                    "}}\nstatic void ls_object{class}_destroy(ls_native_object *owner) {{ ls_object{class}_clear_fields((ls_object{class} *)owner); }}\n"
                ))?;
            }
        }
        self.budget.release(
            AllocationClass::Scratch,
            (emitted.capacity() * size_of::<bool>()) as u64,
        )?;
        drop(emitted);
        self.budget.release(
            AllocationClass::Scratch,
            (chain.capacity() * size_of::<usize>()) as u64,
        )?;
        drop(chain);
        // The allocator's immutable destructor is already a per-class witness.
        // Compare function identities; no extra object header or per-instance tag.
        for target in 0..program.classes.len() {
            self.budget.work(WorkKind::Render, 1)?;
            if !self.plan.class_tests[target] {
                continue;
            }
            self.write(format_args!("static bool ls_is_class{target}(ls_native_object *value) {{ return value && (false"))?;
            for actual in 0..program.classes.len() {
                if program.classes[actual].external {
                    continue;
                }
                let mut ancestor = Some(actual);
                while let Some(class) = ancestor {
                    self.budget.work(WorkKind::Render, 1)?;
                    if class == target {
                        self.write(format_args!(
                            " || value->destroy == ls_object{actual}_destroy"
                        ))?;
                        break;
                    }
                    ancestor = self.base_class(class);
                }
            }
            self.text("); }\n")?;
        }
        Ok(())
    }

    pub(super) fn class_type_test(
        &mut self,
        unit: UnitId,
        result: ValueId,
        value: ValueId,
        class: usize,
    ) -> Result<(), NativeError> {
        let ty = self
            .plan
            .value_type(self.plan.units[unit.index()].values[value.index()]);
        let (result, value) = (result.index(), value.index());
        match ty {
            NativeType::Dynamic(_) => self.write(format_args!("ls_v{result} = ls_v{value}.tag == LS_OBJECT && ls_is_class{class}(ls_v{value}.as.o);\n")),
            NativeType::Object(_) => self.write(format_args!("ls_v{result} = ls_is_class{class}(ls_v{value});\n")),
            _ => self.write(format_args!("ls_v{result} = false;\n")),
        }
    }

    /// A fresh instance for a kept class's constructor, which stores every
    /// field before any read: each field starts zeroed (no owner).
    pub(super) fn allocate_empty_object(
        &mut self,
        unit: UnitId,
        result: ValueId,
        class: usize,
    ) -> Result<(), NativeError> {
        self.write(format_args!(
            "{{\nls_object{class} *ls_o = ls_native_allocate(sizeof *ls_o, ls_object{class}_destroy);\nmemset((char *)ls_o + sizeof(ls_native_object), 0, sizeof *ls_o - sizeof(ls_native_object));\n"
        ))?;
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        self.text("(ls_native_object *)ls_o")?;
        self.assignment_end(unit, destination)?;
        self.text("}\n")
    }

    /// A fresh instance: every field receives its operand, and a managed
    /// field acquires its own owner.
    pub(super) fn allocate_object(
        &mut self,
        unit: UnitId,
        result: ValueId,
        class: usize,
        values: &[ValueId],
    ) -> Result<(), NativeError> {
        self.write(format_args!(
            "{{\nls_object{class} *ls_o = ls_native_allocate(sizeof *ls_o, ls_object{class}_destroy);\n"
        ))?;
        for (slot, &value) in values.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            let declaring = super::native_plan::declaring_class(self.plan.program, class, slot);
            let ty = self.plan.class_fields[declaring][slot];
            let target = format!("((ls_object{declaring} *)ls_o)->ls_m{slot}");
            self.write(format_args!("{target} = "))?;
            self.converted(unit, value, ty)?;
            self.text(";\n")?;
            if let Some(retain) = ty.retain(&target) {
                self.text(&retain)?;
            }
        }
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        self.text("(ls_native_object *)ls_o")?;
        self.assignment_end(unit, destination)?;
        self.text("}\n")
    }
}
