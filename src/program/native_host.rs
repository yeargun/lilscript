//! C providers and managed host operations share the runtime's allocation
//! witnesses and ownership rules. All adapters call compiled typed functions.
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn host_callable(&mut self, index: usize) -> Result<(), NativeError> {
        let cell = self.plan.hosts.bindings[index].cell;
        let signature = self
            .plan
            .signature_for_type(self.plan.program.cells[cell.index()].ty)
            .unwrap();
        self.write(format_args!(
            "(ls_callable{signature}){{ls_host_adapter{index},NULL,UINT64_C({})}}",
            self.plan.program.units.len() + index + 1
        ))
    }
    pub(super) fn provider_implementations(&mut self) -> Result<(), NativeError> {
        let bundled = self
            .plan
            .hosts
            .bindings
            .iter()
            .any(|h| crate::native_providers::bundled(h.link_name));
        if bundled {
            for binding in self.plan.hosts.bindings {
                if let Some(index) = crate::native_providers::NAMES
                    .iter()
                    .position(|name| *name == binding.link_name)
                {
                    self.write(format_args!("#define LS_HOST_PROVIDER_{index} 1\n"))?;
                }
            }
            self.text(include_str!("runtime/providers.c"))?;
        }
        for (index, binding) in self.plan.hosts.bindings.iter().enumerate() {
            let signature = self
                .plan
                .signature_for_type(self.plan.program.cells[binding.cell.index()].ty)
                .unwrap();
            let result = self.plan.signatures[signature].result;
            self.write(format_args!(
                "static LS_NATIVE_UNUSED {result} ls_host_adapter{index}(void *environment"
            ))?;
            self.signature_parameters(signature, true, true)?;
            self.text(") {\n(void)environment;\n")?;
            if result != NativeType::Void {
                self.text("return ")?;
            }
            self.write(format_args!("{}(", binding.link_name))?;
            self.signature_arguments(signature, false)?;
            self.text(");\n}\n")?;
        }
        Ok(())
    }
    pub(super) fn provider_initialize(&mut self) -> Result<(), NativeError> {
        if self
            .plan
            .hosts
            .bindings
            .iter()
            .any(|h| crate::native_providers::arguments(h.link_name))
        {
            let label = if self.artifact.is_library() || !self.plan.program.exports().is_empty() {
                "ls_init_done"
            } else {
                "ls_shutdown"
            };
            self.write(format_args!(
                "if(!ls_host_arguments_init(argc,(const char *const *)argv)) goto {label};\n"
            ))?;
        }
        Ok(())
    }
    pub(super) fn provider_shutdown(&mut self) -> Result<(), NativeError> {
        if self
            .plan
            .hosts
            .bindings
            .iter()
            .any(|h| crate::native_providers::arguments(h.link_name))
        {
            self.text("ls_host_arguments_clear();\n")?;
        }
        Ok(())
    }
    pub(super) fn runtime_interface(&mut self, implementation: bool) -> Result<(), NativeError> {
        if !self.plan.helpers.contains(Helper::Exceptions) {
            return Ok(());
        }
        for (helper, macro_name) in [
            (Helper::Arrays, "ARRAYS"),
            (Helper::Tasks, "TASKS"),
            (Helper::Iterators, "ITERATORS"),
        ] {
            if self.plan.helpers.contains(helper) {
                self.write(format_args!("#define LS_NATIVE_API_{macro_name} 1\n"))?;
            }
        }
        self.text(if implementation {
            include_str!("runtime/interface.c")
        } else {
            include_str!("runtime/interface.h")
        })?;
        self.text("#undef LS_NATIVE_API_ARRAYS\n#undef LS_NATIVE_API_TASKS\n#undef LS_NATIVE_API_ITERATORS\n")
    }
}
