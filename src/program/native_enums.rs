//! ABI enum operations use the checked domain and existing native ownership.
use super::*;
use crate::primitive::EnumOperation;

impl Emitter<'_, '_, '_, '_, '_> {
    fn enum_comparison(
        &mut self,
        unit: UnitId,
        value: ValueId,
        literal: Constant,
    ) -> Result<(), NativeError> {
        match literal {
            Constant::Integer(expected) => {
                self.write(format_args!(
                    "(ls_v{} == INT32_C({expected}))",
                    value.index()
                ))?;
            }
            Constant::String(id) => {
                self.text("ls_string_equal(")?;
                self.value(unit, value)?;
                if self.plan.program.strings[id.index()]
                    .code_units()
                    .next()
                    .is_none()
                {
                    self.text(",(ls_string){0})")?;
                } else {
                    self.write(format_args!(
                        ",(ls_string){{ls_s{0},sizeof ls_s{0}/sizeof *ls_s{0},NULL}})",
                        id.index()
                    ))?;
                }
            }
            _ => unreachable!("checked enum ABI literal"),
        }
        Ok(())
    }

    fn enum_membership(
        &mut self,
        unit: UnitId,
        value: ValueId,
        definition: &EnumDefinition,
    ) -> Result<(), NativeError> {
        if definition.abi == crate::ast::EnumAbi::Flags {
            return self.write(format_args!(
                "(((uint32_t)ls_v{} & UINT32_C({})) == 0)",
                value.index(),
                !definition.flag_mask
            ));
        }
        self.text("(")?;
        for (index, variant) in definition.variants.iter().enumerate() {
            if index > 0 {
                self.text(" || ")?;
            }
            self.enum_comparison(unit, value, variant.value)?;
        }
        self.text(")")
    }

    pub(super) fn enum_operation(
        &mut self,
        unit: UnitId,
        result: ValueId,
        declaration: NominalId,
        operation: EnumOperation,
        args: &[ValueId],
    ) -> Result<(), NativeError> {
        let definition = self.plan.program.enum_definition(declaration).unwrap();
        if operation == EnumOperation::Abi
            || operation == EnumOperation::Ordinal && definition.abi == crate::ast::EnumAbi::Ordinal
        {
            return self.copy_value(unit, Destination::Value(result), args[0]);
        }
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, false)?;
        match operation {
            EnumOperation::From => {
                self.enum_membership(unit, args[0], definition)?;
                self.text(" ? ")?;
                let to = self.destination_type(unit, destination);
                self.converted(unit, args[0], to)?;
                self.text(" : (ls_value){0}")?;
            }
            EnumOperation::Ordinal => {
                for (ordinal, variant) in definition
                    .variants
                    .iter()
                    .enumerate()
                    .take(definition.variants.len() - 1)
                {
                    self.enum_comparison(unit, args[0], variant.value)?;
                    self.write(format_args!(" ? INT32_C({ordinal}) : "))?;
                }
                self.write(format_args!("INT32_C({})", definition.variants.len() - 1))?;
            }
            EnumOperation::Has => {
                self.write(format_args!(
                    "((ls_v{0} & ls_v{1}) == ls_v{1})",
                    args[0].index(),
                    args[1].index()
                ))?;
            }
            EnumOperation::Abi => unreachable!(),
        }
        self.assignment_end(unit, destination)
    }
}
