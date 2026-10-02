//! One typed interface writer over the admitted native plan. C and the host
//! header borrow the same signatures; no declaration is parsed back from text.
use super::*;
use crate::primitive::ParameterPassing;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn header(&mut self) -> Result<(), NativeError> {
        self.write(format_args!("#ifndef LILSCRIPT_NATIVE_{}_ABI_V3_H\n#define LILSCRIPT_NATIVE_{}_ABI_V3_H\n",crate::native_symbols::Component(self.symbol_prefix),crate::native_symbols::Component(self.symbol_prefix)))?;
        self.namespace_macros(true)?;
        self.text("#define LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION 3\n#include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\n")?;
        self.text(include_str!("runtime/call.h"))?;
        self.text(include_str!("runtime/string.h"))?;
        self.text(native_memory::INTERFACE)?;
        self.text("typedef struct ls_native_object ls_native_object;\n")?;
        if self.plan.helpers.contains(Helper::Dynamic) {
            self.text(dynamic::INTERFACE)?;
        }
        if self.plan.helpers.contains(Helper::Exceptions) {
            self.text("void ls_native_exception_raise(ls_value value);\nls_value ls_native_exception_take(void);\n")?;
        }
        self.type_declarations()?;
        self.value_types()?;
        self.text(native_memory::QUALIFICATION_INTERFACE)?;
        self.callable_types()?;
        self.host_interface()?;
        self.export_declarations()?;
        self.runtime_interface(false)?;
        self.namespace_macros(false)?;
        self.text("#endif\n")
    }

    pub(super) fn type_declarations(&mut self) -> Result<(), NativeError> {
        for &index in &self.plan.struct_order {
            self.budget.work(WorkKind::Render, 1)?;
            self.write(format_args!("typedef struct ls_t{index} ls_t{index};\n"))?;
        }
        self.array_declarations()?;
        for (index, signature) in self.plan.signatures.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !self.plan.callable_signature_needed(index) { continue; }
            self.write(format_args!(
                "typedef struct {{\n{} (*code)(void *",
                signature.result
            ))?;
            self.signature_parameters(index, false, true)?;
            self.write(format_args!(
                ");\nvoid *environment;\nuint64_t identity;\n}} ls_callable{index};\n"
            ))?;
        }
        Ok(())
    }

    // Signatures are in admitted dependency order. Child handles are complete
    // before a containing function takes or returns them by value.
    pub(super) fn callable_types(&mut self) -> Result<(), NativeError> {
        for (index, signature) in self.plan.signatures.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !self.plan.callable_signature_needed(index) {
                continue;
            }
            self.write(format_args!("static LS_NATIVE_UNUSED inline ls_callable{index} ls_callable{index}_retain(ls_callable{index} value) {{\nls_native_retain(value.environment);\nreturn value;\n}}\nstatic LS_NATIVE_UNUSED inline void ls_callable{index}_release(ls_callable{index} value) {{ ls_native_release(value.environment); }}\n"))?;
            // Assignment primitives receive already evaluated, inert values.
            // Retain-before-release is required for self assignment and aliases.
            self.write(format_args!("static LS_NATIVE_UNUSED inline void ls_callable{index}_copy(ls_callable{index} *destination, ls_callable{index} value) {{\nls_native_retain(value.environment);\nls_native_release(destination->environment);\n*destination = value;\n}}\nstatic LS_NATIVE_UNUSED inline void ls_callable{index}_take(ls_callable{index} *destination, ls_callable{index} value) {{\nls_native_release(destination->environment);\n*destination = value;\n}}\nstatic LS_NATIVE_UNUSED inline void ls_callable{index}_clear(ls_callable{index} *destination) {{\nls_native_release(destination->environment);\n*destination = (ls_callable{index}){{0}};\n}}\n"))?;
            self.write(format_args!(
                "static LS_NATIVE_UNUSED inline {} ls_callable{index}_call(ls_callable{index} value",
                signature.result
            ))?;
            self.signature_parameters(index, true, true)?;
            self.text(") {\nls_native_retain(value.environment);\n")?;
            if signature.result != NativeType::Void {
                self.write(format_args!("{} result = ", signature.result))?;
            }
            self.text("value.code(value.environment")?;
            self.signature_arguments(index, true)?;
            self.text(");\nls_native_release(value.environment);\n")?;
            if signature.result != NativeType::Void {
                self.text("return result;\n")?;
            }
            self.text("}\n")?;
        }
        Ok(())
    }

    pub(super) fn signature_parameters(
        &mut self,
        index: usize,
        names: bool,
        leading: bool,
    ) -> Result<(), NativeError> {
        let signature = &self.plan.signatures[index];
        if !leading && signature.parameters.is_empty() {
            self.text("void")?;
        }
        for (position, ty) in signature.parameters.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if position != 0 || leading {
                self.text(",")?;
            }
            self.write(format_args!(
                "{}",
                if signature.source.params[position].passing == ParameterPassing::MutableReference {
                    format!("{ty} *")
                } else {
                    ty.to_string()
                }
            ))?;
            if names {
                self.write(format_args!(" ls_p{position}"))?;
            }
        }
        if signature.has_optional() {
            self.text(if names { ",ls_native_arguments ls_args" } else { ",ls_native_arguments" })?;
        }
        Ok(())
    }
    pub(super) fn signature_arguments(
        &mut self,
        index: usize,
        leading: bool,
    ) -> Result<(), NativeError> {
        for position in 0..self.plan.signatures[index].parameters.len() {
            self.budget.work(WorkKind::Render, 1)?;
            if position != 0 || leading {
                self.text(",")?;
            }
            self.write(format_args!("ls_p{position}"))?;
        }
        if self.plan.signatures[index].has_optional() {
            self.text(",ls_args")?;
        }
        Ok(())
    }

    pub(super) fn host_interface(&mut self) -> Result<(), NativeError> {
        for binding in self.plan.hosts.bindings {
            self.budget.work(WorkKind::Render, 1)?;
            let signature_id = self
                .plan
                .signature_for_type(self.plan.program.cells[binding.cell.index()].ty)
                .unwrap();
            let signature = &self.plan.signatures[signature_id];
            for (position, ty) in signature.parameters.iter().copied().enumerate() {
                self.host_alias(binding.link_name, Some(position), ty)?;
            }
            self.host_alias(binding.link_name, None, signature.result)?;
            self.write(format_args!("{} {}(", signature.result, binding.link_name))?;
            self.signature_parameters(signature_id, false, false)?;
            self.text(");\n")?;
        }
        Ok(())
    }
    fn alias_name(&mut self, link: impl fmt::Display, position: Option<usize>) -> Result<(), NativeError> {
        match position {
            Some(position) => self.write(format_args!("{link}_arg{position}")),
            None => self.write(format_args!("{link}_result")),
        }
    }
    pub(super) fn host_alias(
        &mut self,
        link: impl fmt::Display + Copy,
        position: Option<usize>,
        ty: NativeType,
    ) -> Result<(), NativeError> {
        self.budget.work(WorkKind::Render, 1)?;
        self.write(format_args!("typedef {ty} "))?;
        self.alias_name(link, position)?;
        self.text(";\n")?;
        if ty==NativeType::Void {return Ok(());}
        self.write(format_args!("static LS_NATIVE_UNUSED inline {ty} "))?;
        self.alias_name(link, position)?;
        self.write(format_args!("_retain({ty} value) {{\n"))?;
        if let Some(retain)=ty.retain("value") {self.text(&retain)?;}
        self.text("return value;\n}\nstatic LS_NATIVE_UNUSED inline void ")?;
        self.alias_name(link, position)?;
        self.write(format_args!("_release({ty} value) {{\n(void)value;\n"))?;
        if let Some(release)=ty.release("value") {self.text(&release)?;}
        self.text("}\n")?;
        let NativeType::Callable(index)=ty else {return Ok(());};
        self.write(format_args!(
            "static LS_NATIVE_UNUSED inline {} ",
            self.plan.signatures[index].result
        ))?;
        self.alias_name(link, position)?;
        self.write(format_args!("_call({ty} value"))?;
        self.signature_parameters(index, true, true)?;
        self.text(") { ")?;
        if self.plan.signatures[index].result != NativeType::Void {
            self.text("return ")?;
        }
        self.write(format_args!("ls_callable{index}_call(value"))?;
        self.signature_arguments(index, true)?;
        self.text("); }\n")
    }
    pub(super) fn value_types(&mut self) -> Result<(), NativeError> {
        let program = self.plan.program;
        for &index in &self.plan.struct_order {
            self.budget.work(WorkKind::Render, 1)?;
            self.write(format_args!("struct ls_t{index} {{\n"))?;
            let definition = &program.structs[index];
            if definition.fields.is_empty() {
                // C11 has no empty struct. This byte has no source projection,
                // identity, serialization or ABI observation in this client.
                self.text("unsigned char ls_empty;\n")?;
            }
            for field in definition.fields.clone() {
                self.budget.work(WorkKind::Render, 1)?;
                self.write(format_args!(
                    "{} ls_f{};\n",
                    self.plan.field_type(field),
                    program.fields[field].index
                ))?;
            }
            self.text("};\n")?;
            for action in ["retain", "release"] {
                self.write(format_args!("static LS_NATIVE_UNUSED inline void ls_t{index}_{action}(ls_t{index} value) {{ (void)value;\n"))?;
                for field in definition.fields.clone() {
                    let ty = self.plan.field_type(field);
                    let value = format!("value.ls_f{}", program.fields[field].index);
                    let operation = if action == "retain" { ty.retain(&value) } else { ty.release(&value) };
                    if let Some(operation) = operation { self.text(&operation)?; }
                }
                self.text("}\n")?;
            }
            self.write(format_args!("static LS_NATIVE_UNUSED inline void ls_t{index}_trace(ls_t{index} value, ls_native_visit visit, void *context) {{ (void)value; (void)visit; (void)context;\n"))?;
            for field in definition.fields.clone() {
                if let Some(trace) = self.plan.field_type(field).trace(&format!("value.ls_f{}", program.fields[field].index)) { self.text(&trace)?; }
            }
            self.write(format_args!("}}\nstatic LS_NATIVE_UNUSED inline void ls_t{index}_copy(ls_t{index} *slot, ls_t{index} value) {{ ls_t{index}_retain(value); ls_t{index}_release(*slot); *slot = value; }}\nstatic LS_NATIVE_UNUSED inline void ls_t{index}_take(ls_t{index} *slot, ls_t{index} value) {{ ls_t{index}_release(*slot); *slot = value; }}\nstatic LS_NATIVE_UNUSED inline void ls_t{index}_clear(ls_t{index} *slot) {{ ls_t{index}_release(*slot); *slot = (ls_t{index}){{0}}; }}\n"))?;
        }
        Ok(())
    }
}
