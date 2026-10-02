//! Typed bridges and resume points for the owned task protocol. Control flow,
//! parameter transport and frame slot ownership remain with the common writer.
use super::super::native_plan::TaskMethod;
use super::*;

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn task_callbacks(&mut self) -> Result<(), NativeError> {
        for &index in &self.plan.task_callbacks {
            let signature = &self.plan.signatures[index];
            let result = signature.result;
            self.write(format_args!("static ls_value ls_task_invoke{index}(ls_value callback,ls_value input) {{\n(void)input;ls_value output={{0}};\n"))?;
            self.temporary_declaration()?;
            self.write(format_args!("ls_callable{index} fn=ls_value_to_callable{index}(&ls_temps,callback);\nif(ls_native_raised) goto done;\nif(!fn.code) {{ls_value_mismatch();goto done;}}\n"))?;
            if let Some(&ty) = signature.parameters.first() {
                let (prefix, suffix) = Self::conversion(NativeType::Dynamic(Tagged::ANY), ty);
                self.write(format_args!(
                    "{ty} argument={prefix}input{suffix};\nif(ls_native_raised) goto done;\n"
                ))?;
            }
            if result != NativeType::Void {
                self.write(format_args!("{result} value="))?;
            }
            self.write(format_args!("ls_callable{index}_call(fn"))?;
            if !signature.parameters.is_empty() {
                self.text(",argument")?;
            }
            if signature.has_optional() {
                self.write(format_args!(
                    ",(ls_native_arguments){{{},NULL}}",
                    signature.parameters.len()
                ))?;
            }
            self.text(");\n")?;
            if result != NativeType::Void {
                let (prefix, suffix) = Self::conversion(result, NativeType::Dynamic(Tagged::ANY));
                self.write(format_args!("if(!ls_native_raised) {{output={prefix}value{suffix};ls_value_retain(output);}}\n"))?;
                if let Some(drop) = result.release("value") {
                    self.text(&drop)?;
                }
            }
            self.text("done:;\n")?;
            self.clear_temporaries()?;
            self.text("return output;\n}\n")?;
        }
        Ok(())
    }
    pub(super) fn task_call(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: ValueId,
        target: PreparedTarget,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let [CallArgument::Value(value)] =
            data.arguments(data.calls[call.index()].arguments).unwrap()
        else {
            unreachable!("admitted task argument")
        };
        let value = *value;
        self.assignment_start(unit, Destination::Value(result), true)?;
        match target {
            PreparedTarget::TaskBuiltin(BuiltinCall::TaskAll) => {
                self.write(format_args!("ls_task_all(ls_v{})", value.index()))?
            }
            PreparedTarget::TaskBuiltin(builtin) => {
                let function = if builtin == BuiltinCall::TaskResolve {
                    "ls_task_resolved"
                } else {
                    "ls_task_rejected"
                };
                self.write(format_args!("{function}("))?;
                if self
                    .plan
                    .value_type(self.plan.units[unit.index()].values[value.index()])
                    == NativeType::Void
                {
                    self.text("(ls_value){0}")?;
                } else {
                    self.converted(unit, value, NativeType::Dynamic(Tagged::ANY))?;
                }
                self.text(")")?;
            }
            PreparedTarget::TaskMethod {
                receiver,
                kind,
                callback,
            } => {
                let mode = match kind {
                    TaskMethod::Then => 0,
                    TaskMethod::Catch => 1,
                    TaskMethod::Finally => 2,
                };
                self.write(format_args!(
                    "ls_task_chain(ls_v{},{mode},",
                    receiver.index()
                ))?;
                self.converted(unit, value, NativeType::Dynamic(Tagged::ANY))?;
                self.write(format_args!(",ls_task_invoke{callback})"))?;
            }
            _ => unreachable!("task protocol target"),
        }
        self.assignment_end(unit, Destination::Value(result))
    }
    pub(super) fn await_value(
        &mut self,
        unit: UnitId,
        op: OpId,
        value: ValueId,
        result: ValueId,
    ) -> Result<(), NativeError> {
        self.write(format_args!(
            "base->pc={};ls_async_await(base,ls_v{});\n",
            op.index() + 1,
            value.index()
        ))?;
        self.check_exception(unit)?;
        self.clear_temporaries()?;
        self.write(format_args!("return;\nls_resume{}:;\nif(base->rejected) {{ls_native_throw(base->awaited);ls_value_clear(&base->awaited);\n",op.index()))?;
        self.complete(unit, Completion::Throw)?;
        self.text("}\n")?;
        let ty = self
            .plan
            .value_type(self.plan.units[unit.index()].values[result.index()]);
        if ty != NativeType::Void {
            let (prefix, suffix) = Self::conversion(NativeType::Dynamic(Tagged::ANY), ty);
            self.assignment_start(unit, Destination::Value(result), false)?;
            self.write(format_args!("{prefix}base->awaited{suffix}"))?;
            self.assignment_end(unit, Destination::Value(result))?;
        }
        self.text("ls_value_clear(&base->awaited);\n")?;
        self.check_exception(unit)
    }
    pub(super) fn async_exits(&mut self, unit: UnitId) -> Result<(), NativeError> {
        let ty = self.plan.units[unit.index()].return_type;
        // Ordinary fallthrough is possible only for a void async body.
        self.text("ls_task_resolve(base->result,(ls_value){0});ls_async_finish(base);return;\n")?;
        if self.return_exit {
            self.text("ls_return:;ls_task_resolve(base->result,")?;
            if ty == NativeType::Void {
                self.text("(ls_value){0}")?;
            } else {
                let (prefix, suffix) = Self::conversion(ty, NativeType::Dynamic(Tagged::ANY));
                self.write(format_args!("{prefix}ls_pending_return{suffix}"))?;
            }
            self.text(");\n")?;
            self.clear_temporaries()?;
            self.text("ls_async_finish(base);return;\n")?;
        }
        if self.error_exit {
            self.text(
                "ls_error:;ls_task_reject_exception(base->result);ls_async_finish(base);return;\n",
            )?;
        }
        Ok(())
    }
}
