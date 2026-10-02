//! `Map`, `Set` and `Symbol`. A map keeps entries in insertion order and
//! finds them through an open-addressed index, comparing keys with
//! SameValueZero: numbers by value (NaN finds NaN, -0 is +0), strings by
//! code units, everything else by identity. A set is a map whose values
//! are null. Entries own their keys and values.
use super::*;
use crate::primitive::Intrinsic;

pub(in crate::program) const RUNTIME: &str = include_str!("runtime/collections.c");

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn construct(
        &mut self,
        unit: UnitId,
        result: ValueId,
        intrinsic: Intrinsic,
    ) -> Result<(), NativeError> {
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        // A description is observable only through `toString`, which native
        // symbols do not offer.
        self.text(match intrinsic {
            Intrinsic::MapNew | Intrinsic::SetNew => "ls_map_new()",
            _ => "ls_symbol_new()",
        })?;
        self.assignment_end(unit, destination)
    }

    pub(super) fn collection_method(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: ValueId,
        receiver: ValueId,
        method: Intrinsic,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let argument = |position: usize| match arguments[position] {
            CallArgument::Value(value) => value,
            CallArgument::Reference(_) | CallArgument::Spread(_) => {
                unreachable!("native collections take values")
            }
        };
        let any = NativeType::Dynamic(Tagged::ANY);
        let r = receiver.index();
        let destination = Destination::Value(result);
        match method {
            Intrinsic::MapSet | Intrinsic::SetAdd => {
                self.write(format_args!("ls_map_set(ls_v{r},"))?;
                self.converted(unit, argument(0), any)?;
                self.text(",")?;
                if method == Intrinsic::MapSet {
                    self.converted(unit, argument(1), any)?;
                } else {
                    self.text("(ls_value){0}")?;
                }
                self.text(");\n")?;
                // Both return their receiver: one more owner of it.
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("ls_v{r}"))?;
                self.assignment_end(unit, destination)
            }
            Intrinsic::MapGet => {
                // The entry lends its value; the slot takes its own owner.
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("ls_map_get(ls_v{r},"))?;
                self.converted(unit, argument(0), any)?;
                self.text(")")?;
                self.assignment_end(unit, destination)
            }
            Intrinsic::MapHas | Intrinsic::SetHas | Intrinsic::MapDelete | Intrinsic::SetDelete => {
                let operation = if matches!(method, Intrinsic::MapHas | Intrinsic::SetHas) {
                    "has"
                } else {
                    "delete"
                };
                self.write(format_args!(
                    "ls_v{} = ls_map_{operation}(ls_v{r},",
                    result.index()
                ))?;
                self.converted(unit, argument(0), any)?;
                self.text(");\n")
            }
            _ => self.write(format_args!("ls_map_clear(ls_v{r});\n")),
        }
    }
}
