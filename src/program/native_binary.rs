//! Binary memory: reference-counted buffers and typed array views of them.
//! A view owns its buffer. Element writes convert as ECMA-262 does: integer
//! kinds wrap modulo their width, `Uint8Clamped` clamps, `Float32` rounds to
//! binary32. Writes past the end are ignored, as in JavaScript; an integer
//! read past the end is 0 (`a[i]|0`) and a float read stops the program,
//! since `undefined` is no native value. Elements use the host's byte order,
//! as JavaScript's typed arrays do.
use super::*;
use crate::primitive::Intrinsic;
use crate::typed_array::{TypedArrayIntrinsic, TypedArrayKind};

pub(in crate::program) const RUNTIME: &str = r#"typedef struct { ls_native_object owner; size_t length; uint8_t *bytes; } ls_buffer;
typedef struct { ls_native_object owner; ls_native_object *buffer; size_t offset; size_t length; } ls_typed;
static void ls_binary_failure(const char *message) {
    fputs(message, stderr);
    fputc('\n', stderr);
    abort();
}
static void ls_buffer_destroy(ls_native_object *owner) { free(((ls_buffer *)owner)->bytes); }
static ls_native_object *ls_buffer_new(int32_t length) {
    if (length < 0) ls_binary_failure("LilScript native buffer length is negative");
    ls_buffer *buffer = ls_native_allocate(sizeof *buffer, ls_buffer_destroy);
    buffer->length = (size_t)length;
    buffer->bytes = calloc(length ? (size_t)length : 1, 1);
    if (!buffer->bytes) ls_native_resource_failure();
    return &buffer->owner;
}
static int32_t ls_buffer_byte_length(ls_native_object *owner) { return (int32_t)((ls_buffer *)owner)->length; }
static size_t ls_binary_relative(int32_t index, size_t length) {
    if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
    return (size_t)index < length ? (size_t)index : length;
}
static ls_native_object *ls_buffer_slice(ls_native_object *owner, int32_t start, int32_t end) {
    ls_buffer *buffer = (ls_buffer *)owner;
    size_t from = ls_binary_relative(start, buffer->length), to = ls_binary_relative(end, buffer->length);
    size_t length = to > from ? to - from : 0;
    ls_native_object *result = ls_buffer_new((int32_t)length);
    if (length) memcpy(((ls_buffer *)result)->bytes, buffer->bytes + from, length);
    return result;
}
static void ls_typed_destroy(ls_native_object *owner) { ls_native_release(((ls_typed *)owner)->buffer); }
static ls_native_object *ls_typed_view(ls_native_object *buffer, size_t offset, size_t length) {
    ls_typed *view = ls_native_allocate(sizeof *view, ls_typed_destroy);
    ls_native_retain(buffer);
    view->buffer = buffer;
    view->offset = offset;
    view->length = length;
    return &view->owner;
}
static ls_native_object *ls_typed_new(int32_t length, size_t size) {
    if (length < 0) ls_binary_failure("LilScript native typed array length is negative");
    if ((size_t)length > (size_t)INT32_MAX / size) ls_native_resource_failure();
    ls_native_object *buffer = ls_buffer_new((int32_t)((size_t)length * size));
    ls_native_object *view = ls_typed_view(buffer, 0, (size_t)length);
    ls_native_release(buffer);
    return view;
}
static ls_native_object *ls_typed_over(ls_native_object *buffer, size_t size) {
    size_t length = ((ls_buffer *)buffer)->length;
    if (length % size) ls_binary_failure("LilScript native buffer length is not a multiple of the element size");
    return ls_typed_view(buffer, 0, length / size);
}
static int32_t ls_typed_length(ls_native_object *owner) { return (int32_t)((ls_typed *)owner)->length; }
static int32_t ls_typed_byte_offset(ls_native_object *owner) { return (int32_t)((ls_typed *)owner)->offset; }
static ls_native_object *ls_typed_buffer(ls_native_object *owner) { return ((ls_typed *)owner)->buffer; }
static uint8_t *ls_typed_at(ls_native_object *owner, int32_t index, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    if (index < 0 || (size_t)index >= view->length) return NULL;
    return ((ls_buffer *)view->buffer)->bytes + view->offset + (size_t)index * size;
}
static ls_native_object *ls_typed_subarray(ls_native_object *owner, int32_t start, int32_t end, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    size_t from = ls_binary_relative(start, view->length), to = ls_binary_relative(end, view->length);
    return ls_typed_view(view->buffer, view->offset + from * size, to > from ? to - from : 0);
}
static ls_native_object *ls_typed_slice(ls_native_object *owner, int32_t start, int32_t end, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    size_t from = ls_binary_relative(start, view->length), to = ls_binary_relative(end, view->length);
    size_t length = to > from ? to - from : 0;
    ls_native_object *result = ls_typed_new((int32_t)length, size);
    if (length) memcpy(((ls_buffer *)((ls_typed *)result)->buffer)->bytes, ((ls_buffer *)view->buffer)->bytes + view->offset + from * size, length * size);
    return result;
}
static void ls_typed_undefined(void) { ls_binary_failure("LilScript native typed array element is undefined"); }
#define LS_TYPED_INT(name, type, read, write) \
static int32_t ls_typed_get_##name(ls_native_object *owner, int32_t index) { \
    uint8_t *at = ls_typed_at(owner, index, sizeof(type)); \
    if (!at) return 0; \
    type value; memcpy(&value, at, sizeof value); return read; \
} \
static void ls_typed_set_##name(ls_native_object *owner, int32_t index, int32_t input) { \
    uint8_t *at = ls_typed_at(owner, index, sizeof(type)); \
    if (!at) return; \
    type value = write; memcpy(at, &value, sizeof value); \
}
LS_TYPED_INT(int8, int8_t, (int32_t)value, (int8_t)(uint8_t)(uint32_t)input)
LS_TYPED_INT(uint8, uint8_t, (int32_t)value, (uint8_t)(uint32_t)input)
LS_TYPED_INT(uint8c, uint8_t, (int32_t)value, (uint8_t)(input < 0 ? 0 : input > 255 ? 255 : input))
LS_TYPED_INT(int16, int16_t, (int32_t)value, (int16_t)(uint16_t)(uint32_t)input)
LS_TYPED_INT(uint16, uint16_t, (int32_t)value, (uint16_t)(uint32_t)input)
LS_TYPED_INT(int32, int32_t, value, input)
LS_TYPED_INT(uint32, uint32_t, ls_from_u32(value), (uint32_t)input)
static double ls_typed_get_float32(ls_native_object *owner, int32_t index) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(float));
    if (!at) ls_typed_undefined();
    float value; memcpy(&value, at, sizeof value); return (double)value;
}
static void ls_typed_set_float32(ls_native_object *owner, int32_t index, double input) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(float));
    if (!at) return;
    float value = (float)input; memcpy(at, &value, sizeof value);
}
static double ls_typed_get_float64(ls_native_object *owner, int32_t index) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(double));
    if (!at) ls_typed_undefined();
    double value; memcpy(&value, at, sizeof value); return value;
}
static void ls_typed_set_float64(ls_native_object *owner, int32_t index, double input) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(double));
    if (!at) return;
    memcpy(at, &input, sizeof input);
}
"#;

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
                let (kind, _) = crate::typed_array::classify_typed_array_intrinsic(intrinsic).unwrap();
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
            CallArgument::Reference(_) => unreachable!("native binary ranges take values"),
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
