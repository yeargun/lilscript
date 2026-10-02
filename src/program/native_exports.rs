//! Public C names and ownership follow the checked interface table. No source
//! text, C text or parallel export graph is reconstructed here.
use super::*;
use crate::native_symbols::{Component, ExportName};
use crate::primitive::ParameterPassing;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn namespace_macros(&mut self, define: bool) -> Result<(), NativeError> {
        if !self.artifact.is_library() {
            return Ok(());
        }
        for &name in crate::native_symbols::ABI_NAMES {
            self.namespace_macro(name, define)?;
        }
        for &index in &self.plan.struct_order {
            for suffix in [
                "", "_retain", "_release", "_trace", "_copy", "_take", "_clear",
            ] {
                self.namespace_macro(format_args!("ls_t{index}{suffix}"), define)?;
            }
        }
        for index in 0..self.plan.signatures.len() {
            if !self.plan.callable_signature_needed(index) {
                continue;
            }
            for suffix in [
                "", "_retain", "_release", "_call", "_copy", "_take", "_clear",
            ] {
                self.namespace_macro(format_args!("ls_callable{index}{suffix}"), define)?;
            }
        }
        for index in 0..self.plan.arrays.len() {
            self.namespace_macro(format_args!("ls_array{index}"), define)?;
        }
        for binding in self.plan.hosts.bindings {
            if crate::native_providers::bundled(binding.link_name) {
                self.namespace_macro(binding.link_name, define)?;
            }
            let signature = self
                .plan
                .signature_for_type(self.plan.program.cells[binding.cell.index()].ty)
                .unwrap();
            for position in 0..=self.plan.signatures[signature].parameters.len() {
                let (ty, argument) = if position == self.plan.signatures[signature].parameters.len()
                {
                    (self.plan.signatures[signature].result, None)
                } else {
                    (
                        self.plan.signatures[signature].parameters[position],
                        Some(position),
                    )
                };
                for suffix in ["", "_retain", "_release", "_call"] {
                    if suffix == "_call" && !matches!(ty, NativeType::Callable(_))
                        || !suffix.is_empty() && ty == NativeType::Void
                    {
                        continue;
                    }
                    if let Some(i) = argument {
                        self.namespace_macro(
                            format_args!("{}_arg{i}{suffix}", binding.link_name),
                            define,
                        )?;
                    } else {
                        self.namespace_macro(
                            format_args!("{}_result{suffix}", binding.link_name),
                            define,
                        )?;
                    }
                }
            }
        }
        Ok(())
    }
    fn namespace_macro(
        &mut self,
        name: impl fmt::Display + Copy,
        define: bool,
    ) -> Result<(), NativeError> {
        let prefix = self.symbol_prefix;
        if define {
            self.write(format_args!("#define {name} {prefix}_{name}\n"))
        } else {
            self.write(format_args!("#undef {name}\n"))
        }
    }
    fn api_return_zero(&mut self, ty: NativeType) -> Result<(), NativeError> {
        if ty == NativeType::Void {
            self.text("return;\n")
        } else {
            self.write(format_args!("return ({ty}){{0}};\n"))
        }
    }
    fn api_guard(&mut self, ty: NativeType) -> Result<(), NativeError> {
        self.text("if (!ls_api_enter()) { ")?;
        self.api_return_zero(ty)?;
        self.text("}\n")
    }
    fn export_parameters(&mut self, signature: usize, skip: usize) -> Result<(), NativeError> {
        let sig = &self.plan.signatures[signature];
        if sig.parameters.len() == skip && !sig.has_optional() {
            self.text("void")?;
        }
        for (i, ty) in sig.parameters.iter().enumerate().skip(skip) {
            if i != skip {
                self.text(",")?;
            }
            self.write(format_args!(
                "{ty} {}ls_p{i}",
                if sig.source.params[i].passing == ParameterPassing::MutableReference {
                    "*"
                } else {
                    ""
                }
            ))?;
        }
        if sig.has_optional() {
            if sig.parameters.len() > skip {
                self.text(",")?;
            }
            self.text("ls_native_arguments ls_args")?;
        }
        Ok(())
    }
    fn exported_cell_value(&mut self, cell: CellId) -> Result<(), NativeError> {
        match self.plan.cell_storage(cell) {
            ValueStorage::Function(unit) => self.write(format_args!(
                "(ls_callable{}){{ls_adapter{},NULL,UINT64_C({})}}",
                self.plan.signature_for_unit(unit),
                unit.index(),
                unit.index() + 1
            )),
            ValueStorage::Value(_) => self.write(format_args!("*ls_g{}()", cell.index())),
            ValueStorage::Host(index) => self.host_callable(index),
        }
    }
    pub(super) fn export_declarations(&mut self) -> Result<(), NativeError> {
        self.exports(false)
    }
    pub(super) fn export_implementations(&mut self) -> Result<(), NativeError> {
        if self.artifact.is_library() || !self.plan.program.exports().is_empty() {
            self.text("static unsigned ls_api_state;\nstatic size_t ls_api_depth;\nstatic ls_exception_state ls_api_failure;\nstatic bool ls_api_enter(void) {\nif(ls_native_raised) return false;\nif(ls_api_state!=1 && ls_api_state!=2) {ls_native_raise_error(\"TypeError\",\"native library is not initialized or has shut down\");return false;}\n++ls_api_depth;return true;\n}\n")?;
        }
        self.runtime_interface(true)?;
        self.exports(true)
    }
    fn exports(&mut self, implementation: bool) -> Result<(), NativeError> {
        let program = self.plan.program;
        if program.exports().is_empty() && !self.artifact.is_library() {
            return Ok(());
        }
        if !implementation {
            let prefix = self.symbol_prefix;
            self.write(format_args!("/* Initialize once; all calls/owners stay on the originating thread.\n   Arguments borrow. Results/getters transfer ownership. Check exception_pending\n   before using a result. Drain explicitly; shutdown requires no active call. */\nbool {0}_initialize(int argc, const char *const *argv);\nbool {0}_drain(void);\nvoid {0}_shutdown(void);\n",prefix))?;
        }
        if program.exports().is_empty() {
            return Ok(());
        }
        for (entry, definition) in program.entries().iter().enumerate() {
            for export in program.entry_exports(entry) {
                self.budget.work(WorkKind::Render, 1)?;
                let name = ExportName {
                    prefix: self.symbol_prefix,
                    entry: &definition.name,
                    name: &export.name,
                };
                match export.target {
                    InterfaceTarget::Value(cell) => {
                        let storage = self.plan.cell_storage(cell);
                        if let ValueStorage::Function(unit) = storage {
                            if let Some(identity) = program.unit(unit).unwrap().constructor_of {
                                let class = program.class_index(identity).unwrap();
                                self.export_constructor(name, unit, class, implementation)?;
                                continue;
                            }
                        }
                        let ty = self.plan.value_type(storage);
                        if !implementation {
                            self.host_alias(format_args!("{name}_binding"), None, ty)?;
                        }
                        self.write(format_args!("{ty} {name}_get(void)"))?;
                        if implementation {
                            self.text(" {\n")?;
                            self.api_guard(ty)?;
                            self.write(format_args!("{ty} value="))?;
                            self.exported_cell_value(cell)?;
                            self.text(";\n")?;
                            if let Some(retain) = ty.retain("value") {
                                self.text(&retain)?;
                            }
                            self.text("--ls_api_depth;return value;\n}\n")?;
                        } else {
                            self.text(";\n")?;
                        }
                        if let NativeType::Callable(signature) = ty {
                            if !implementation {
                                for (i, &ty) in self.plan.signatures[signature]
                                    .parameters
                                    .iter()
                                    .enumerate()
                                {
                                    self.host_alias(name, Some(i), ty)?;
                                }
                                self.host_alias(
                                    name,
                                    None,
                                    self.plan.signatures[signature].result,
                                )?;
                            }
                            let result = self.plan.signatures[signature].result;
                            self.write(format_args!("{result} {name}("))?;
                            self.export_parameters(signature, 0)?;
                            if !implementation {
                                self.text(");\n")?;
                                continue;
                            }
                            self.text(") {\n")?;
                            self.api_guard(result)?;
                            self.write(format_args!("{ty} callable="))?;
                            self.exported_cell_value(cell)?;
                            self.text(";\nif(ls_native_raised) {--ls_api_depth;")?;
                            self.api_return_zero(result)?;
                            self.text("}\n")?;
                            if result != NativeType::Void {
                                self.write(format_args!("{result} result="))?;
                            }
                            self.write(format_args!("ls_callable{signature}_call(callable"))?;
                            self.signature_arguments(signature, true)?;
                            self.text(");\n--ls_api_depth;\n")?;
                            if result != NativeType::Void {
                                self.text("return result;\n")?;
                            }
                            self.text("}\n")?;
                        }
                    }
                    InterfaceTarget::Type(identity) => {
                        if let Some(class) = program.class_index(identity) {
                            if !implementation {
                                self.write(format_args!(
                                    "typedef ls_native_object *{name}_type;\n"
                                ))?;
                            }
                            if !program.classes[class].shape {
                                self.export_class(name, class, implementation)?;
                            }
                        } else if let Some(index) =
                            program.structs.iter().position(|s| s.identity == identity)
                        {
                            if !implementation {
                                self.write(format_args!("typedef ls_t{index} {name}_type;\n"))?;
                            }
                            self.export_product(name, index, implementation)?;
                        } else if let Some(definition) = program.enum_definition(identity) {
                            let ty = if definition.abi == crate::ast::EnumAbi::String {
                                NativeType::String
                            } else {
                                NativeType::I32
                            };
                            if !implementation {
                                self.write(format_args!("typedef {ty} {name}_type;\n"))?;
                            }
                            for variant in &definition.variants {
                                self.write(format_args!(
                                    "{ty} {name}_variant_{}(void)",
                                    Component(&variant.name)
                                ))?;
                                if !implementation {
                                    self.text(";\n")?;
                                    continue;
                                }
                                self.text(" {return ")?;
                                match variant.value {
                                    Constant::Integer(value) => {
                                        self.write(format_args!("INT32_C({value})"))?
                                    }
                                    Constant::String(id) => {
                                        if program.strings[id.index()].code_units().next().is_none()
                                        {
                                            self.text("(ls_string){0}")?;
                                        } else {
                                            self.write(format_args!("(ls_string){{ls_s{0},sizeof ls_s{0}/sizeof *ls_s{0},NULL}}",id.index()))?;
                                        }
                                    }
                                    _ => unreachable!("checked enum representation"),
                                }
                                self.text(";}\n")?;
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    fn export_constructor(
        &mut self,
        name: ExportName<'_>,
        unit: UnitId,
        class: usize,
        implementation: bool,
    ) -> Result<(), NativeError> {
        let signature = self.plan.signature_for_unit(unit);
        if !implementation {
            for (i, &ty) in self.plan.signatures[signature]
                .parameters
                .iter()
                .enumerate()
                .skip(1)
            {
                self.host_alias(format_args!("{name}_new"), Some(i - 1), ty)?;
            }
            self.host_alias(format_args!("{name}_new"), None, NativeType::Object(class))?;
        }
        self.write(format_args!("ls_native_object *{name}_new("))?;
        self.export_parameters(signature, 1)?;
        if !implementation {
            return self.text(");\n");
        }
        self.text(") {\n")?;
        self.api_guard(NativeType::Object(class))?;
        self.write(format_args!("ls_object{class} *object=ls_native_allocate(sizeof *object,ls_object{class}_destroy,ls_object{class}_trace);\nmemset((char *)object+sizeof(ls_native_object),0,sizeof *object-sizeof(ls_native_object));\n"))?;
        if self.plan.signatures[signature].has_optional() {
            let count = self.plan.signatures[signature].parameters.len();
            self.write(format_args!("bool absent[{count}]={{false}};\nfor(size_t i=1;i<{count};++i) absent[i]=i>ls_args.count || (ls_args.absent && ls_args.absent[i-1]);\nls_native_arguments constructor_args={{ls_args.count<{count}-1 ? ls_args.count+1 : {count},absent}};\n"))?;
        }
        self.write(format_args!(
            "ls_fn{}((ls_native_object *)object",
            unit.index()
        ))?;
        for i in 1..self.plan.signatures[signature].parameters.len() {
            self.write(format_args!(",ls_p{i}"))?;
        }
        if self.plan.signatures[signature].has_optional() {
            self.text(",constructor_args")?;
        }
        self.text(");\n--ls_api_depth;\nif(ls_native_raised) {ls_native_release(object);return NULL;}\nreturn (ls_native_object *)object;\n}\n")
    }
    fn export_product(
        &mut self,
        name: ExportName<'_>,
        index: usize,
        implementation: bool,
    ) -> Result<(), NativeError> {
        self.write(format_args!("ls_value {name}_box(ls_t{index} value)"))?;
        if implementation {
            self.write(format_args!(" {{\nls_native_temporary *temps=NULL;ls_value result=ls_t{index}_box(&temps,value);ls_value_retain(result);ls_native_temporaries_clear(&temps);return result;\n}}\n"))?;
        } else {
            self.text(";\n")?;
        }
        self.write(format_args!("ls_t{index} {name}_unbox(ls_value value)"))?;
        if implementation {
            self.write(format_args!(" {{\nls_t{index} result=ls_value_to_t{index}(value);ls_t{index}_retain(result);return result;\n}}\n"))?;
        } else {
            self.text(";\n")?;
        }
        for field in self.plan.program.structs[index].fields.clone() {
            let definition = &self.plan.program.fields[field];
            let ty = self.plan.field_type(field);
            let field_name = Component(&definition.name);
            let slot = definition.index;
            self.write(format_args!(
                "{ty} {name}_field_{field_name}_get(ls_t{index} object)"
            ))?;
            if implementation {
                self.write(format_args!(" {{\n{ty} value=object.ls_f{slot};\n"))?;
                if let Some(retain) = ty.retain("value") {
                    self.text(&retain)?;
                }
                self.text("return value;\n}\n")?;
            } else {
                self.text(";\n")?;
            }
            self.write(format_args!(
                "void {name}_field_{field_name}_set(ls_t{index} *object,{ty} value)"
            ))?;
            if implementation {
                self.text(" {\n")?;
                if let Some(prefix) = ty.owner_prefix() {
                    self.write(format_args!("{prefix}_copy(&object->ls_f{slot},value);\n"))?;
                } else {
                    self.write(format_args!("object->ls_f{slot}=value;\n"))?;
                }
                self.text("}\n")?;
            } else {
                self.text(";\n")?;
            }
        }
        Ok(())
    }
    fn export_class(
        &mut self,
        name: ExportName<'_>,
        class: usize,
        implementation: bool,
    ) -> Result<(), NativeError> {
        let program = self.plan.program;
        for (slot, &(field, _)) in program.classes[class].fields.iter().enumerate() {
            let field_name = Component(program.strings[field.index()].as_unicode().unwrap());
            let ty = self.plan.class_fields[class][slot];
            let owner = super::super::native_plan::declaring_class(program, class, slot);
            for set in [false, true] {
                let result = if set { NativeType::Void } else { ty };
                self.write(format_args!(
                    "{result} {name}_field_{field_name}_{}(ls_native_object *object",
                    if set { "set" } else { "get" }
                ))?;
                if set {
                    self.write(format_args!(",{ty} value"))?;
                }
                if !implementation {
                    self.text(");\n")?;
                    continue;
                }
                self.text(") {\n")?;
                self.api_guard(result)?;
                self.write(format_args!(
                    "if(!ls_is_class{class}(object)) {{ls_value_mismatch();--ls_api_depth;"
                ))?;
                self.api_return_zero(result)?;
                self.text("}\n")?;
                if set {
                    self.text("if(ls_native_mutable(object)) {\n")?;
                    if let Some(prefix) = ty.owner_prefix() {
                        self.write(format_args!(
                            "{prefix}_copy(&((ls_object{owner} *)object)->ls_m{slot},value);\n"
                        ))?;
                    } else {
                        self.write(format_args!(
                            "((ls_object{owner} *)object)->ls_m{slot}=value;\n"
                        ))?;
                    }
                    self.text("}\n--ls_api_depth;\n}\n")?;
                } else {
                    self.write(format_args!(
                        "{ty} value=((ls_object{owner} *)object)->ls_m{slot};\n"
                    ))?;
                    if let Some(retain) = ty.retain("value") {
                        self.text(&retain)?;
                    }
                    self.text("--ls_api_depth;return value;\n}\n")?;
                }
            }
        }
        for &(method, cell) in &program.classes[class].prototype {
            let method_name = Component(program.strings[method.index()].as_unicode().unwrap());
            let signature = self
                .plan
                .signature_for_type(program.cells[cell.index()].ty)
                .unwrap();
            let result = self.plan.signatures[signature].result;
            if !implementation {
                for (i, &ty) in self.plan.signatures[signature]
                    .parameters
                    .iter()
                    .enumerate()
                {
                    self.host_alias(format_args!("{name}_method_{method_name}"), Some(i), ty)?;
                }
                self.host_alias(format_args!("{name}_method_{method_name}"), None, result)?;
            }
            self.write(format_args!("{result} {name}_method_{method_name}("))?;
            self.export_parameters(signature, 0)?;
            if !implementation {
                self.text(");\n")?;
                continue;
            }
            self.text(") {\n")?;
            self.api_guard(result)?;
            // Destructor identities are the same immutable class witnesses the
            // checked native view uses. Select the actual prototype override.
            for actual in 0..program.classes.len() {
                if program.classes[actual].external
                    || program.classes[actual].shape
                    || !self.plan.class_extends(actual, class)
                {
                    continue;
                }
                let mut current = Some(actual);
                let mut target = cell;
                while let Some(index) = current {
                    self.budget.work(
                        WorkKind::Render,
                        program.classes[index].prototype.len() as u64 + 1,
                    )?;
                    if let Some((_, found)) = program.classes[index]
                        .prototype
                        .iter()
                        .find(|(key, _)| *key == method)
                    {
                        target = *found;
                        break;
                    }
                    current = program.classes[index]
                        .base
                        .and_then(|id| program.class_index(id));
                }
                self.write(format_args!(
                    "if(ls_p0 && ls_p0->destroy==ls_object{actual}_destroy) {{\n"
                ))?;
                let ValueStorage::Function(unit) = self.plan.cell_storage(target) else {
                    unreachable!("class prototype is a declared function");
                };
                let from = self.plan.signature_for_unit(unit);
                if from != signature {
                    self.write(format_args!("ls_native_temporary *ls_temps=NULL;\nls_callable{signature} callee=ls_borrow_adapt{from}_{signature}(&ls_temps,(ls_callable{from}){{ls_adapter{},NULL,UINT64_C({})}});\n",unit.index(),unit.index()+1))?;
                }
                if result != NativeType::Void {
                    self.write(format_args!("{result} result="))?;
                }
                if from != signature {
                    self.write(format_args!("ls_callable{signature}_call(callee,"))?;
                } else {
                    self.write(format_args!("ls_fn{}(", unit.index()))?;
                }
                self.signature_arguments(signature, false)?;
                self.text(");\n")?;
                if from != signature {
                    self.text("ls_native_temporaries_clear(&ls_temps);\n")?;
                }
                self.text("--ls_api_depth;\n")?;
                if result != NativeType::Void {
                    self.text("return result;\n")?;
                } else {
                    self.text("return;\n")?;
                }
                self.text("}\n")?;
            }
            self.text("ls_value_mismatch();--ls_api_depth;\n")?;
            self.api_return_zero(result)?;
            self.text("}\n")?;
        }
        Ok(())
    }
    pub(super) fn library_lifecycle(&mut self) -> Result<(), NativeError> {
        let prefix = self.symbol_prefix;
        self.write(format_args!("bool {prefix}_initialize(int argc,const char *const *argv) {{\n(void)argc;(void)argv;\nif(ls_native_raised) return false;\nif(ls_api_state==2) return true;\nif(ls_api_state==3) {{ls_native_raised=true;ls_native_error_name=ls_api_failure.name;ls_native_error_message=ls_api_failure.message;ls_value_copy(&ls_native_thrown,ls_api_failure.value);return false;}}\nif(ls_api_state) {{ls_native_raise_error(\"TypeError\",\"native library initialization is recursive or follows shutdown\");return false;}}\nls_api_state=1;ls_api_depth=1;\nif(!ls_runtime_init()) {{ls_native_raise_error(\"TypeError\",\"unsupported native floating point mode\");goto ls_init_done;}}\nls_native_identity_counter=UINT64_C({});\n",self.plan.program.units.len()+self.plan.hosts.bindings.len()))?;
        self.provider_initialize()?;
        for unit in self.plan.program.initialization.iter() {
            self.write(format_args!(
                "ls_init{}();\nif(ls_native_raised) goto ls_init_done;\n",
                unit.index()
            ))?;
        }
        self.text("ls_init_done: ls_api_depth=0;\nif(ls_native_raised) {ls_api_state=3;ls_api_failure=(ls_exception_state){true,ls_native_error_name,ls_native_error_message,ls_native_thrown};ls_value_retain(ls_api_failure.value);return false;}\nls_api_state=2;return true;\n}\n")?;
        self.write(format_args!("bool {prefix}_drain(void) {{\nif(!ls_api_enter()) return false;\nif(ls_api_depth!=1) {{--ls_api_depth;ls_native_raise_error(\"TypeError\",\"cannot drain inside an active library call\");return false;}}\n"))?;
        if self.plan.helpers.contains(Helper::Tasks) {
            self.text("ls_task_drain();\n")?;
        }
        self.text("--ls_api_depth;return !ls_native_raised;\n}\n")?;
        self.write(format_args!("void {prefix}_shutdown(void) {{\nif(ls_api_state==4) return;\nif(ls_api_depth) {{ls_native_raise_error(\"TypeError\",\"cannot shut down an active library call\");return;}}\nls_api_state=4;\n"))?;
        if self.plan.helpers.contains(Helper::Tasks) {
            self.text("ls_task_discard_jobs();\n")?;
        }
        self.text("ls_exception_clear(&ls_api_failure);\nls_exception_state pending=ls_exception_save();ls_exception_clear(&pending);\n")?;
        self.release_globals()?;
        self.provider_shutdown()?;
        self.text("ls_native_collect_cycles();\n}\n")
    }
    pub(super) fn release_globals(&mut self) -> Result<(), NativeError> {
        for (index, cell) in self.plan.cells.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if cell.global {
                if let ValueStorage::Value(ty) = cell.storage {
                    if let Some(prefix) = ty.owner_prefix() {
                        self.write(format_args!("{prefix}_clear(&ls_c{index});\n"))?;
                    }
                }
            }
        }
        Ok(())
    }
}
