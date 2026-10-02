//! Portable record and JSON recipes. Values keep the checked program's
//! representations; only record storage crosses the shared tagged-value ABI.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn record_key(&mut self, key: RecordKey) -> Result<(), NativeError> {
        match key {
            RecordKey::Value(value) => self.write(format_args!("ls_v{}", value.index())),
            RecordKey::Literal(key) => {
                if self.plan.program.strings[key.index()].code_units().next().is_none() {
                    self.text("(ls_string){0}")
                } else {
                    self.write(format_args!("(ls_string){{ls_s{0},sizeof ls_s{0}/sizeof *ls_s{0},NULL}}", key.index()))
                }
            }
        }
    }

    pub(super) fn record_builtin(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: ValueId,
        builtin: BuiltinCall,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let argument = |index: usize| match arguments[index] {
            CallArgument::Value(value) => value,
            _ => unreachable!("admitted record argument"),
        };
        let receiver = argument(0);
        let r = receiver.index();
        let destination = Destination::Value(result);
        match builtin {
            BuiltinCall::ObjectHasOwn => {
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("ls_record_has(ls_v{r},ls_v{})", argument(1).index()))?;
                self.assignment_end(unit, destination)
            }
            BuiltinCall::ObjectAssign => {
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("ls_record_assign(ls_v{r},ls_v{})", argument(1).index()))?;
                self.assignment_end(unit, destination)
            }
            BuiltinCall::ObjectKeys | BuiltinCall::ObjectValues => {
                let NativeType::Array(array) = self.plan.value_type(self.plan.units[unit.index()].values[result.index()]) else {
                    unreachable!("record enumeration has array storage")
                };
                let element = self.plan.arrays[array];
                let (prefix, suffix) = Self::conversion(NativeType::Dynamic(Tagged::ANY), element);
                self.text("{\n")?;
                self.write(format_args!("ls_map *ls_record = (ls_map *)ls_v{r};\nls_record_key *ls_order = ls_record_order(ls_v{r});\n"))?;
                self.assignment_start(unit, destination, true)?;
                self.write(format_args!("ls_array{array}_new(ls_record->size)"))?;
                self.assignment_end(unit, destination)?;
                self.write(format_args!("for (size_t ls_i = 0; ls_i < ls_record->size; ++ls_i) {{\nls_array{array}_push(ls_v{},", result.index()))?;
                let field = if builtin == BuiltinCall::ObjectKeys { "key" } else { "value" };
                self.write(format_args!("{prefix}ls_record->entries[ls_order[ls_i].position].{field}{suffix});\n}}\nfree(ls_order);\n}}\n"))
            }
            BuiltinCall::JsonStringify => {
                let ty = self.plan.value_type(self.plan.units[unit.index()].values[receiver.index()]);
                if let NativeType::Array(array) = ty {
                    let (prefix, suffix) = Self::conversion(self.plan.arrays[array], NativeType::Dynamic(Tagged::ANY));
                    self.write(format_args!("{{\nls_string_builder ls_json = {{0}};\nls_string_builder_unit(&ls_json,'[');\nfor (size_t ls_i = 0; ls_i < ls_v{r}->length; ++ls_i) {{\nif (ls_i) ls_string_builder_unit(&ls_json,',');\nif(!ls_array_has(ls_v{r},ls_i)) ls_string_builder_ascii(&ls_json,\"null\"); else ls_json_value(&ls_json,{prefix}ls_array{array}_get(ls_v{r},(int32_t)ls_i,&ls_temps){suffix});\n}}\nls_string_builder_unit(&ls_json,']');\n"))?;
                    self.assignment_start(unit, destination, true)?;
                    self.text("ls_string_builder_finish(&ls_json)")?;
                    self.assignment_end(unit, destination)?;
                    self.text("}\n")
                } else {
                    self.assignment_start(unit, destination, true)?;
                    if ty == NativeType::Record {
                        self.write(format_args!("ls_json_record(ls_v{r})"))?;
                    } else {
                        self.text("ls_json_scalar(")?;
                        self.converted(unit, receiver, NativeType::Dynamic(Tagged::ANY))?;
                        self.text(")")?;
                    }
                    self.assignment_end(unit, destination)
                }
            }
            _ => unreachable!("admitted portable data builtin"),
        }
    }
}

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn allocate_shape(&mut self, unit: UnitId, result: ValueId, keys: &[StringId], args: &[ValueId]) -> Result<(), NativeError> {
        let program = self.plan.program;
        let data = program.unit(unit).unwrap();
        let ty = &program.types[data.values[result.index()].ty.index()];
        let fields = super::super::schema::shape_fields(program, ty,
            &mut crate::check::type_admission::TypeQueryAdmission::new(self.budget))?.expect("admitted shape schema");
        self.assignment_start(unit, Destination::Value(result), true)?;
        self.text("ls_record_new()")?;
        self.assignment_end(unit, Destination::Value(result))?;
        for ((&key, &value), (_, declared)) in keys.iter().zip(args).zip(&fields) {
            self.write(format_args!("ls_shape_set(ls_v{},", result.index()))?;
            self.record_key(RecordKey::Literal(key))?;
            self.text(",")?;
            self.converted(unit, value, NativeType::Dynamic(Tagged::ANY))?;
            self.write(format_args!(",{});\n", super::super::native_plan::native_optional_key(declared)))?;
        }
        Ok(())
    }

    pub(super) fn shape_test(&mut self, unit: UnitId, result: ValueId, value: ValueId, target: TypeId) -> Result<(), NativeError> {
        let program = self.plan.program;
        let (Type::Class(declaration) | Type::ClassInstance { declaration, .. }) = &program.types[target.index()] else {
            unreachable!("admitted tagged shape test")
        };
        let definition = program.class(declaration.identity).unwrap();
        let (slot, tag) = definition.discriminant.unwrap();
        let key = definition.fields[slot as usize].0;
        self.write(format_args!("ls_v{} = ls_shape_tag(", result.index()))?;
        self.converted(unit, value, NativeType::Dynamic(Tagged::ANY))?;
        self.text(",")?;
        self.record_key(RecordKey::Literal(key))?;
        self.text(",")?;
        match tag {
            Constant::String(id) => { self.text("ls_value_string(")?; self.record_key(RecordKey::Literal(id))?; self.text(")")?; }
            Constant::Integer(value) => self.write(format_args!("ls_value_int(ls_from_u32(UINT32_C({})))", value as u32))?,
            Constant::Boolean(value) => self.write(format_args!("ls_value_bool({value})"))?,
            _ => unreachable!("checked shape tag"),
        }
        self.text(");\n")
    }
}
