//! Binary memory: reference-counted buffers and typed array views of them.
//! A view owns its buffer. Element writes convert as ECMA-262 does: integer
//! kinds wrap modulo their width, `Uint8Clamped` clamps, `Float32` rounds to
//! binary32. Writes past the end are ignored, as in JavaScript. Every read
//! requires an in-range index (R11) and traps if it is invalid. A guarded
//! `.get(i)` checks before reading storage. Elements use the host's byte order,
//! as JavaScript's typed arrays do.
use super::*;
use crate::primitive::Intrinsic;
use crate::typed_array::{TypedArrayIntrinsic, TypedArrayKind};

pub(in crate::program) const RUNTIME: &str = include_str!("runtime/binary.c");

/// The C name part of a typed array kind's element accessors.
pub(super) fn kind_name(kind: TypedArrayKind) -> &'static str {
    match kind {
        TypedArrayKind::Int8 => "int8",
        TypedArrayKind::Uint8 => "uint8",
        TypedArrayKind::Uint8Clamped => "uint8c",
        TypedArrayKind::Int16 => "int16",
        TypedArrayKind::Uint16 => "uint16",
        TypedArrayKind::Int32 => "int32",
        TypedArrayKind::Uint32 => "uint32",
        TypedArrayKind::Float32 => "float32",
        TypedArrayKind::Float64 => "float64",
    }
}

impl Emitter<'_, '_, '_, '_, '_> {
    pub(super) fn construct_binary(
        &mut self,
        unit: UnitId,
        call: CallId,
        result: ValueId,
        intrinsic: Intrinsic,
    ) -> Result<(), NativeError> {
        let data = self.plan.program.unit(unit).unwrap();
        let CallArgument::Value(argument) =
            data.arguments(data.calls[call.index()].arguments).unwrap()[0]
        else {
            unreachable!("native binary constructors take a value")
        };
        let from_buffer = self.plan.units[unit.index()].values[argument.index()]
            == ValueStorage::Value(NativeType::Buffer);
        let expression = match intrinsic {
            Intrinsic::ArrayBufferNew | Intrinsic::SharedArrayBufferNew => {
                format!("ls_buffer_new(ls_v{})", argument.index())
            }
            _ => {
                let (kind, _) =
                    crate::typed_array::classify_typed_array_intrinsic(intrinsic).unwrap();
                let size = kind.bytes_per_element();
                if from_buffer {
                    format!("ls_typed_over(ls_v{}, {size})", argument.index())
                } else {
                    format!("ls_typed_new(ls_v{}, {size})", argument.index())
                }
            }
        };
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        self.text(&expression)?;
        self.assignment_end(unit, destination)
    }

    pub(super) fn binary_method(
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
            CallArgument::Value(value) => value.index(),
            CallArgument::Reference(_) | CallArgument::Spread(_) => {
                unreachable!("native binary ranges take values")
            }
        };
        let (start, end, r) = (argument(0), argument(1), receiver.index());
        let expression = match crate::typed_array::classify_typed_array_intrinsic(method) {
            Some((kind, TypedArrayIntrinsic::Subarray)) => format!(
                "ls_typed_subarray(ls_v{r}, ls_v{start}, ls_v{end}, {})",
                kind.bytes_per_element()
            ),
            Some((kind, _)) => format!(
                "ls_typed_slice(ls_v{r}, ls_v{start}, ls_v{end}, {})",
                kind.bytes_per_element()
            ),
            None => format!("ls_buffer_slice(ls_v{r}, ls_v{start}, ls_v{end})"),
        };
        let destination = Destination::Value(result);
        self.assignment_start(unit, destination, true)?;
        self.text(&expression)?;
        self.assignment_end(unit, destination)
    }

    /// A typed array or buffer property; `buffer` lends the view's buffer.
    pub(super) fn binary_property(
        &mut self,
        unit: UnitId,
        result: ValueId,
        receiver: ValueId,
        intrinsic: Intrinsic,
    ) -> Result<(), NativeError> {
        let r = receiver.index();
        let destination = Destination::Value(result);
        let expression = match crate::typed_array::classify_typed_array_intrinsic(intrinsic) {
            Some((_, TypedArrayIntrinsic::Length)) => format!("ls_typed_length(ls_v{r})"),
            Some((kind, TypedArrayIntrinsic::ByteLength)) => format!(
                "(int32_t)(ls_typed_length(ls_v{r}) * {})",
                kind.bytes_per_element()
            ),
            Some((_, TypedArrayIntrinsic::ByteOffset)) => format!("ls_typed_byte_offset(ls_v{r})"),
            Some((_, TypedArrayIntrinsic::Buffer)) => {
                let to = self
                    .plan
                    .value_type(self.plan.units[unit.index()].values[result.index()]);
                let (prefix, suffix) = Self::conversion(NativeType::Buffer, to);
                self.assignment_start(unit, destination, false)?;
                self.write(format_args!("{prefix}ls_typed_buffer(ls_v{r}){suffix}"))?;
                return self.assignment_end(unit, destination);
            }
            _ => format!("ls_buffer_byte_length(ls_v{r})"),
        };
        self.write(format_args!("ls_v{} = {expression};\n", result.index()))
    }
}
