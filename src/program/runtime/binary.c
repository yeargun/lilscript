typedef struct { ls_native_object owner; size_t length; uint8_t *bytes; } ls_buffer;
typedef struct { ls_native_object owner; ls_native_object *buffer; size_t offset; size_t length; } ls_typed;
static LS_NATIVE_UNUSED inline void ls_binary_failure(const char *message) {
    fputs(message, stderr);
    fputc('\n', stderr);
    abort();
}
static LS_NATIVE_UNUSED inline void ls_buffer_destroy(ls_native_object *owner) { free(((ls_buffer *)owner)->bytes); }
static LS_NATIVE_UNUSED inline ls_native_object *ls_buffer_new(int32_t length) {
    if (length < 0) ls_binary_failure("LilScript native buffer length is negative");
    ls_buffer *buffer = ls_native_allocate(sizeof *buffer, ls_buffer_destroy, NULL);
    buffer->length = (size_t)length;
    buffer->bytes = calloc(length ? (size_t)length : 1, 1);
    if (!buffer->bytes) ls_native_resource_failure();
    return &buffer->owner;
}
static LS_NATIVE_UNUSED inline int32_t ls_buffer_byte_length(ls_native_object *owner) { return (int32_t)((ls_buffer *)owner)->length; }
static LS_NATIVE_UNUSED inline size_t ls_binary_relative(int32_t index, size_t length) {
    if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
    return (size_t)index < length ? (size_t)index : length;
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_buffer_slice(ls_native_object *owner, int32_t start, int32_t end) {
    ls_buffer *buffer = (ls_buffer *)owner;
    size_t from = ls_binary_relative(start, buffer->length), to = ls_binary_relative(end, buffer->length);
    size_t length = to > from ? to - from : 0;
    ls_native_object *result = ls_buffer_new((int32_t)length);
    if (length) memcpy(((ls_buffer *)result)->bytes, buffer->bytes + from, length);
    return result;
}
static LS_NATIVE_UNUSED inline void ls_typed_destroy(ls_native_object *owner) { ls_native_release(((ls_typed *)owner)->buffer); }
static LS_NATIVE_UNUSED inline void ls_typed_trace(ls_native_object *owner, ls_native_visit visit, void *context) { visit(((ls_typed *)owner)->buffer, context); }
static LS_NATIVE_UNUSED inline ls_native_object *ls_typed_view(ls_native_object *buffer, size_t offset, size_t length) {
    ls_typed *view = ls_native_allocate(sizeof *view, ls_typed_destroy, ls_typed_trace);
    ls_native_retain(buffer);
    view->buffer = buffer;
    view->offset = offset;
    view->length = length;
    return &view->owner;
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_typed_new(int32_t length, size_t size) {
    if (length < 0) ls_binary_failure("LilScript native typed array length is negative");
    if ((size_t)length > (size_t)INT32_MAX / size) ls_native_resource_failure();
    ls_native_object *buffer = ls_buffer_new((int32_t)((size_t)length * size));
    ls_native_object *view = ls_typed_view(buffer, 0, (size_t)length);
    ls_native_release(buffer);
    return view;
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_typed_over(ls_native_object *buffer, size_t size) {
    size_t length = ((ls_buffer *)buffer)->length;
    if (length % size) ls_binary_failure("LilScript native buffer length is not a multiple of the element size");
    return ls_typed_view(buffer, 0, length / size);
}
static LS_NATIVE_UNUSED inline int32_t ls_typed_length(ls_native_object *owner) { return (int32_t)((ls_typed *)owner)->length; }
static LS_NATIVE_UNUSED inline int32_t ls_typed_byte_offset(ls_native_object *owner) { return (int32_t)((ls_typed *)owner)->offset; }
static LS_NATIVE_UNUSED inline ls_native_object *ls_typed_buffer(ls_native_object *owner) { return ((ls_typed *)owner)->buffer; }
static LS_NATIVE_UNUSED inline uint8_t *ls_typed_at(ls_native_object *owner, int32_t index, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    if (index < 0 || (size_t)index >= view->length) return NULL;
    return ((ls_buffer *)view->buffer)->bytes + view->offset + (size_t)index * size;
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_typed_subarray(ls_native_object *owner, int32_t start, int32_t end, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    size_t from = ls_binary_relative(start, view->length), to = ls_binary_relative(end, view->length);
    return ls_typed_view(view->buffer, view->offset + from * size, to > from ? to - from : 0);
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_typed_slice(ls_native_object *owner, int32_t start, int32_t end, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    size_t from = ls_binary_relative(start, view->length), to = ls_binary_relative(end, view->length);
    size_t length = to > from ? to - from : 0;
    ls_native_object *result = ls_typed_new((int32_t)length, size);
    if (length) memcpy(((ls_buffer *)((ls_typed *)result)->buffer)->bytes, ((ls_buffer *)view->buffer)->bytes + view->offset + from * size, length * size);
    return result;
}
static LS_NATIVE_UNUSED inline void ls_typed_undefined(void) { ls_binary_failure("LilScript native typed array index out of range"); }
#define LS_TYPED_INT(name, type, read, write) \
static LS_NATIVE_UNUSED inline int32_t ls_typed_get_##name(ls_native_object *owner, int32_t index) { \
    uint8_t *at = ls_typed_at(owner, index, sizeof(type)); \
    if (!at) ls_typed_undefined(); \
    type value; memcpy(&value, at, sizeof value); return read; \
} \
static LS_NATIVE_UNUSED inline void ls_typed_set_##name(ls_native_object *owner, int32_t index, int32_t input) { \
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
static LS_NATIVE_UNUSED inline double ls_typed_get_float32(ls_native_object *owner, int32_t index) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(float));
    if (!at) ls_typed_undefined();
    float value; memcpy(&value, at, sizeof value); return (double)value;
}
static LS_NATIVE_UNUSED inline void ls_typed_set_float32(ls_native_object *owner, int32_t index, double input) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(float));
    if (!at) return;
    float value = (float)input; memcpy(at, &value, sizeof value);
}
static LS_NATIVE_UNUSED inline double ls_typed_get_float64(ls_native_object *owner, int32_t index) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(double));
    if (!at) ls_typed_undefined();
    double value; memcpy(&value, at, sizeof value); return value;
}
static LS_NATIVE_UNUSED inline void ls_typed_set_float64(ls_native_object *owner, int32_t index, double input) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(double));
    if (!at) return;
    memcpy(at, &input, sizeof input);
}
