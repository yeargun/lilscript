//! String conversion, concatenation, comparison and the admitted string,
//! `int` and `float` methods. `native_string_runtime` holds their C recipes.
use super::*;
use crate::primitive::Intrinsic;

impl Emitter<'_, '_, '_, '_, '_> {
    /// ECMA-262 ToString of an admitted primitive value.
    fn string_of(&mut self, unit: UnitId, value: ValueId) -> Result<(), NativeError> {
        let index = value.index();
        match self.plan.units[unit.index()].values[index] {
            ValueStorage::Value(NativeType::String) => self.write(format_args!("ls_v{index}")),
            ValueStorage::Value(NativeType::I32) => {
                self.write(format_args!("ls_int_to_string(ls_v{index})"))
            }
            ValueStorage::Value(NativeType::F64) => {
                self.write(format_args!("ls_number_to_string(ls_v{index})"))
            }
            ValueStorage::Value(NativeType::Bool) => {
                self.write(format_args!("ls_bool_to_string(ls_v{index})"))
            }
            _ => unreachable!("native plan admits only primitive string conversion"),
        }
    }

    /// Left to right: every operand is already evaluated, and converting a
    /// primitive has no observable effect.
    pub(super) fn concatenation(
        &mut self,
        unit: UnitId,
        values: &[ValueId],
    ) -> Result<(), NativeError> {
        match values {
            [] => self.text("(ls_string){NULL,0}"),
            [only] => self.string_of(unit, *only),
            [rest @ .., last] => {
                self.text("ls_string_concat(")?;
                self.concatenation(unit, rest)?;
                self.text(",")?;
                self.string_of(unit, *last)?;
                self.text(")")
            }
        }
    }

    /// String `+`, equality and ordering; false when this is not one.
    pub(super) fn string_binary(
        &mut self,
        unit: UnitId,
        kind: BinaryOp,
        result: ValueId,
        left: ValueId,
        right: ValueId,
    ) -> Result<bool, NativeError> {
        let values = &self.plan.units[unit.index()].values;
        let text = ValueStorage::Value(NativeType::String);
        if kind == BinaryOp::Add && values[result.index()] == text {
            self.write(format_args!("ls_v{} = ", result.index()))?;
            self.concatenation(unit, &[left, right])?;
            self.text(";\n")?;
            return Ok(true);
        }
        if values[left.index()] != text || values[right.index()] != text {
            return Ok(false);
        }
        let (r, l, rt) = (result.index(), left.index(), right.index());
        match kind {
            BinaryOp::Eq => self.write(format_args!(
                "ls_v{r} = ls_string_equal(ls_v{l},ls_v{rt});\n"
            ))?,
            BinaryOp::NotEq => self.write(format_args!(
                "ls_v{r} = !ls_string_equal(ls_v{l},ls_v{rt});\n"
            ))?,
            BinaryOp::Less | BinaryOp::LessEq | BinaryOp::Greater | BinaryOp::GreaterEq => {
                let token = match kind {
                    BinaryOp::Less => "<",
                    BinaryOp::LessEq => "<=",
                    BinaryOp::Greater => ">",
                    _ => ">=",
                };
                self.write(format_args!(
                    "ls_v{r} = ls_string_compare(ls_v{l},ls_v{rt}) {token} 0;\n"
                ))?
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(super) fn scalar_method(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: Option<ValueId>,
        receiver: ValueId,
        method: Intrinsic,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let arguments = data.arguments(data.calls[call.index()].arguments).unwrap();
        let argument = |position: usize| match arguments.get(position) {
            Some(CallArgument::Value(value)) => Some(format!("ls_v{}", value.index())),
            _ => None,
        };
        let r = format!("ls_v{}", receiver.index());
        let first = argument(0).unwrap_or_default();
        let expression = match method {
            Intrinsic::StringIncludes => format!("ls_string_index_of({r},{first},0) >= 0"),
            Intrinsic::StringStartsWith => format!("ls_string_starts_with({r},{first})"),
            Intrinsic::StringEndsWith => format!("ls_string_ends_with({r},{first})"),
            Intrinsic::StringIndexOf => format!(
                "ls_string_index_of({r},{first},{})",
                argument(1).unwrap_or_else(|| "0".into())
            ),
            Intrinsic::StringLastIndexOf => format!(
                "ls_string_last_index_of({r},{first},{})",
                argument(1).unwrap_or_else(|| "INT32_MAX".into())
            ),
            Intrinsic::StringRepeat => format!("ls_string_repeat({r},{first})"),
            Intrinsic::StringToUpperCase => format!("ls_string_case({r},true)"),
            Intrinsic::StringToLowerCase => format!("ls_string_case({r},false)"),
            Intrinsic::StringTrim => format!("ls_string_trim({r},true,true)"),
            Intrinsic::StringTrimStart => format!("ls_string_trim({r},true,false)"),
            Intrinsic::StringTrimEnd => format!("ls_string_trim({r},false,true)"),
            Intrinsic::StringSlice => format!(
                "ls_string_slice({r},{first},{},{})",
                argument(1).is_some(),
                argument(1).unwrap_or_else(|| "0".into())
            ),
            Intrinsic::StringSplit => format!("ls_string_split({r},{first})"),
            Intrinsic::StringCodePointLength => format!("ls_string_code_points({r})"),
            Intrinsic::IntToString => format!(
                "ls_int_to_radix({r},{})",
                argument(0).unwrap_or_else(|| "10".into())
            ),
            Intrinsic::IntToUnsignedString => format!(
                "ls_uint_to_radix((uint32_t){r},false,{})",
                argument(0).unwrap_or_else(|| "10".into())
            ),
            Intrinsic::FloatAbs => format!("ls_f64(fabs({r}))"),
            Intrinsic::FloatFloor => format!("ls_f64(floor({r}))"),
            Intrinsic::FloatCeil => format!("ls_f64(ceil({r}))"),
            Intrinsic::FloatRound => format!("ls_f64(ls_round({r}))"),
            Intrinsic::FloatSqrt => format!("ls_f64(sqrt({r}))"),
            Intrinsic::FloatSin => format!("ls_f64(sin({r}))"),
            Intrinsic::FloatCos => format!("ls_f64(cos({r}))"),
            Intrinsic::FloatAcos => format!("ls_f64(acos({r}))"),
            Intrinsic::FloatExp => format!("ls_f64(exp({r}))"),
            Intrinsic::FloatLog => format!("ls_f64(log({r}))"),
            Intrinsic::FloatTan => format!("ls_f64(tan({r}))"),
            Intrinsic::FloatAtan2 => format!("ls_f64(atan2({r},(double){first}))"),
            Intrinsic::FloatHypot => format!("ls_f64(hypot({r},(double){first}))"),
            Intrinsic::FloatMin => format!("ls_f64(ls_min({r},(double){first}))"),
            Intrinsic::FloatMax => format!("ls_f64(ls_max({r},(double){first}))"),
            Intrinsic::FloatToInt => format!("ls_to_i32({r})"),
            _ => unreachable!("native plan admits only these scalar methods"),
        };
        let result = result.unwrap();
        let destination = Destination::Value(result);
        // `split` makes a new array: the result slot takes that owner.
        self.assignment_start(unit, destination, true)?;
        self.text(&expression)?;
        self.assignment_end(unit, destination)
    }

    /// `split` with a string separator, as String.prototype.split: an empty
    /// separator splits code units, and an empty receiver yields `[""]`
    /// unless the separator is empty too.
    pub(super) fn string_split_runtime(&mut self, array: usize) -> Result<(), NativeError> {
        self.write(format_args!(
            "static ls_array{array} *ls_string_split(ls_string text, ls_string separator) {{\n\
ls_array{array} *parts = ls_array{array}_new(0);\n\
if (!separator.length) {{\n\
for (size_t index = 0; index < text.length; index++) ls_array{array}_push(parts, ls_string_view(text, index, index + 1));\n\
return parts;\n}}\n\
if (!text.length) {{ ls_array{array}_push(parts, text); return parts; }}\n\
size_t start = 0;\n\
for (size_t at = 0; at + separator.length <= text.length;) {{\n\
if (ls_string_matches(text, at, separator)) {{ ls_array{array}_push(parts, ls_string_view(text, start, at)); at += separator.length; start = at; }}\n\
else at++;\n}}\n\
ls_array{array}_push(parts, ls_string_view(text, start, text.length));\n\
return parts;\n}}\n"
        ))
    }
}
