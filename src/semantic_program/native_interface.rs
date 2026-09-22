//! One typed interface writer over the admitted native plan. C and the host
//! header borrow the same signatures; no declaration is parsed back from text.
use super::*;
use crate::primitive::ParameterPassing;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn header(&mut self) -> Result<(), NativeError> {
        self.text("#ifndef LILSCRIPT_NATIVE_CALLBACK_ABI_V1_H\n#define LILSCRIPT_NATIVE_CALLBACK_ABI_V1_H\n#define LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION 1\n#include <stdbool.h>\n#include <stddef.h>\n#include <stdint.h>\ntypedef struct { const uint16_t *data; size_t length; } ls_string;\n")?;
        self.value_types()?;
        self.text(native_memory::INTERFACE)?;
        self.text(native_memory::QUALIFICATION_INTERFACE)?;
        self.callable_types()?;
        self.host_interface()?;
        self.text("#endif\n")
    }

    // Signatures are in admitted dependency order. Child handles are complete
    // before a containing function takes or returns them by value.
    pub(super) fn callable_types(&mut self) -> Result<(), NativeError> {
        for (index, signature) in self.plan.signatures.iter().enumerate() {
            self.budget.work(WorkKind::Render, 1)?;
            if !self.plan.callable_signature_needed(index) {
                continue;
            }
            self.write(format_args!(
                "typedef struct {{\n{} (*code)(void *",
                signature.result
            ))?;
            self.signature_parameters(index, false, true)?;
            self.write(format_args!(
                ");\nvoid *environment;\nuint64_t identity;\n}} ls_callable{index};\n"
            ))?;
            self.write(format_args!("static inline ls_callable{index} ls_callable{index}_retain(ls_callable{index} value) {{\nls_native_retain(value.environment);\nreturn value;\n}}\nstatic inline void ls_callable{index}_release(ls_callable{index} value) {{ ls_native_release(value.environment); }}\n"))?;
            // Assignment primitives receive already evaluated, inert values.
            // Retain-before-release is required for self assignment and aliases.
            self.write(format_args!("static inline void ls_callable{index}_copy(ls_callable{index} *destination, ls_callable{index} value) {{\nls_native_retain(value.environment);\nls_native_release(destination->environment);\n*destination = value;\n}}\nstatic inline void ls_callable{index}_take(ls_callable{index} *destination, ls_callable{index} value) {{\nls_native_release(destination->environment);\n*destination = value;\n}}\nstatic inline void ls_callable{index}_clear(ls_callable{index} *destination) {{\nls_native_release(destination->environment);\n*destination = (ls_callable{index}){{0}};\n}}\n"))?;
            self.write(format_args!(
                "static inline {} ls_callable{index}_call(ls_callable{index} value",
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
                "{ty}{}",
                if signature.source.params[position].passing == ParameterPassing::MutableReference {
                    " *"
                } else {
                    ""
                }
            ))?;
            if names {
                self.write(format_args!(" ls_p{position}"))?;
            }
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
    fn alias_name(&mut self, link: &str, position: Option<usize>) -> Result<(), NativeError> {
        match position {
            Some(position) => self.write(format_args!("{link}_arg{position}")),
            None => self.write(format_args!("{link}_result")),
        }
    }
    fn host_alias(
        &mut self,
        link: &str,
        position: Option<usize>,
        ty: NativeType,
    ) -> Result<(), NativeError> {
        self.budget.work(WorkKind::Render, 1)?;
        self.write(format_args!("typedef {ty} "))?;
        self.alias_name(link, position)?;
        self.text(";\n")?;
        let NativeType::Callable(index) = ty else {
            return Ok(());
        };
        self.write(format_args!("static inline {ty} "))?;
        self.alias_name(link, position)?;
        self.write(format_args!("_retain({ty} value) {{ return ls_callable{index}_retain(value); }}\nstatic inline void "))?;
        self.alias_name(link, position)?;
        self.write(format_args!(
            "_release({ty} value) {{ ls_callable{index}_release(value); }}\nstatic inline {} ",
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
            self.write(format_args!("typedef struct {{\n"))?;
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
            self.write(format_args!("}} ls_t{index};\n"))?;
        }
        Ok(())
    }
}
