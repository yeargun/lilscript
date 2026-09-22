#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <inttypes.h>
#include <limits.h>
#include <float.h>
#include <fenv.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#if !defined(__STDC_VERSION__) || __STDC_VERSION__ < 201112L
#error "LilScript native requires C11"
#endif
#if defined(__FAST_MATH__) || (defined(__FINITE_MATH_ONLY__) && __FINITE_MATH_ONLY__ != 0)
#error "LilScript native forbids fast-math and finite-only arithmetic"
#endif
#if FLT_EVAL_METHOD != 0
#error "LilScript native requires evaluation in the declared floating type"
#endif
#pragma STDC FP_CONTRACT OFF
_Static_assert(CHAR_BIT == 8, "LilScript native requires 8-bit bytes");
_Static_assert(sizeof(uint16_t) == 2 && sizeof(uint32_t) == 4 && sizeof(uint64_t) == 8,
               "LilScript native requires exact integer widths");
_Static_assert(SIZE_MAX >= UINT32_MAX, "LilScript native requires at least 32-bit object sizes");
_Static_assert(sizeof(double) == 8 && FLT_RADIX == 2 && DBL_MANT_DIG == 53 &&
               DBL_MIN_EXP == -1021 && DBL_MAX_EXP == 1024 && DBL_HAS_SUBNORM == 1,
               "LilScript native requires binary64 with subnormals");
typedef struct { const uint16_t *data; size_t length; } ls_string;
static int ls_runtime_init(void) {
    double one = 1.0, negative_zero = -0.0;
    uint64_t one_bits, zero_bits;
    memcpy(&one_bits, &one, sizeof one_bits);
    memcpy(&zero_bits, &negative_zero, sizeof zero_bits);
    if (fegetround() != FE_TONEAREST || one_bits != UINT64_C(0x3ff0000000000000) ||
        zero_bits != UINT64_C(0x8000000000000000)) {
        fputs("unsupported LilScript native floating-point environment\n", stderr);
        return 0;
    }
    /* FE_TONEAREST does not exclude flush-to-zero or denormals-are-zero. */
    uint64_t tiny_bits = UINT64_C(1), doubled_bits;
    double tiny;
    memcpy(&tiny, &tiny_bits, sizeof tiny);
    volatile double input = tiny, factor = 2.0;
    volatile double doubled = input * factor;
    double observed = doubled;
    memcpy(&doubled_bits, &observed, sizeof doubled_bits);
    if (doubled_bits != UINT64_C(2)) {
        fputs("LilScript native requires gradual binary64 underflow\n", stderr);
        return 0;
    }
    return 1;
}
static inline int32_t ls_from_u32(uint32_t value) {
    return value <= INT32_MAX ? (int32_t)value : (int32_t)((int64_t)value - INT64_C(4294967296));
}
#include <stdlib.h>
typedef struct ls_native_object {
    size_t references;
    void (*destroy)(struct ls_native_object *);
} ls_native_object;

static uint64_t ls_native_identity_counter;
#ifdef LS_NATIVE_QUALIFICATION
static size_t ls_native_live_objects;
#endif

static void ls_native_resource_failure(void) {
    fputs("LilScript native runtime resource exhaustion\n", stderr);
    abort();
}

static void *ls_native_allocate(size_t bytes,
                               void (*destroy)(ls_native_object *)) {
    if (bytes < sizeof(ls_native_object)) ls_native_resource_failure();
    ls_native_object *object = malloc(bytes);
    if (!object) ls_native_resource_failure();
    object->references = 1;
    object->destroy = destroy;
#ifdef LS_NATIVE_QUALIFICATION
    if (ls_native_live_objects == SIZE_MAX) ls_native_resource_failure();
    ++ls_native_live_objects;
#endif
    return object;
}

void ls_native_retain(void *environment) {
    ls_native_object *object = environment;
    if (!object) return;
    if (object->references == SIZE_MAX) ls_native_resource_failure();
    ++object->references;
}

void ls_native_release(void *environment) {
    ls_native_object *object = environment;
    if (!object) return;
    if (--object->references != 0) return;
    if (object->destroy) object->destroy(object);
#ifdef LS_NATIVE_QUALIFICATION
    --ls_native_live_objects;
#endif
    free(object);
}

static uint64_t ls_native_fresh_identity(void) {
    if (ls_native_identity_counter == UINT64_MAX) ls_native_resource_failure();
    return ++ls_native_identity_counter;
}
typedef struct { ls_native_object owner; size_t length; uint8_t *bytes; } ls_buffer;
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
#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void) {
    return ls_native_live_objects;
}
#endif
typedef struct {
ls_native_object * (*code)(void *);
void *environment;
uint64_t identity;
} ls_callable0;
static inline ls_callable0 ls_callable0_retain(ls_callable0 value) {
ls_native_retain(value.environment);
return value;
}
static inline void ls_callable0_release(ls_callable0 value) { ls_native_release(value.environment); }
static inline void ls_callable0_copy(ls_callable0 *destination, ls_callable0 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable0_take(ls_callable0 *destination, ls_callable0 value) {
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable0_clear(ls_callable0 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable0){0};
}
static inline ls_native_object * ls_callable0_call(ls_callable0 value) {
ls_native_retain(value.environment);
ls_native_object * result = value.code(value.environment);
ls_native_release(value.environment);
return result;
}
static void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef struct ls_object0 {
ls_native_object owner;
ls_native_object * ls_m0;
ls_native_object * ls_m1;
} ls_object0;
static void ls_object0_clear_fields(ls_object0 *object) {
(void)object;
ls_object_clear(&object->ls_m0);
ls_object_clear(&object->ls_m1);
}
static void ls_object0_destroy(ls_native_object *owner) { ls_object0_clear_fields((ls_object0 *)owner); }
static void ls_init0(void);
static void ls_init0(void) {
ls_native_object * ls_c1 = NULL;
ls_native_object * ls_c2 = NULL;
ls_native_object * ls_c3 = NULL;
ls_native_object * ls_c4 = NULL;
ls_native_object * ls_c5 = NULL;
ls_native_object * ls_c6 = NULL;
ls_native_object * ls_c7 = NULL;
ls_native_object * ls_c8 = NULL;
ls_native_object * ls_c9 = NULL;
ls_native_object * ls_c10 = NULL;
int32_t ls_v0;
ls_native_object * ls_v1 = NULL;
int32_t ls_v2;
ls_native_object * ls_v3 = NULL;
ls_native_object * ls_v4 = NULL;
ls_native_object * ls_v5 = NULL;
ls_native_object * ls_v6 = NULL;
int32_t ls_v7;
ls_native_object * ls_v9 = NULL;
ls_native_object * ls_v10 = NULL;
int32_t ls_v11;
int32_t ls_v13;
ls_native_object * ls_v14 = NULL;
ls_native_object * ls_v15 = NULL;
ls_native_object * ls_v16 = NULL;
ls_native_object * ls_v17 = NULL;
int32_t ls_v18;
ls_native_object * ls_v20 = NULL;
int32_t ls_v21;
ls_native_object * ls_v23 = NULL;
int32_t ls_v24;
ls_native_object * ls_v26 = NULL;
int32_t ls_v27;
ls_native_object * ls_v29 = NULL;
int32_t ls_v30;
int32_t ls_v31;
ls_native_object * ls_v32 = NULL;
int32_t ls_v33;
int32_t ls_v34;
ls_native_object * ls_v35 = NULL;
int32_t ls_v36;
int32_t ls_v37;
int32_t ls_v38;
ls_native_object * ls_v39 = NULL;
int32_t ls_v40;
int32_t ls_v41;
ls_native_object * ls_v43 = NULL;
int32_t ls_v44;
int32_t ls_v45;
ls_native_object * ls_v47 = NULL;
int32_t ls_v48;
int32_t ls_v49;
ls_native_object * ls_v51 = NULL;
int32_t ls_v52;
int32_t ls_v53;
ls_native_object * ls_v54 = NULL;
int32_t ls_v55;
int32_t ls_v56;
int32_t ls_v57;
int32_t ls_v58;
ls_native_object * ls_v60 = NULL;
int32_t ls_v61;
int32_t ls_v62;
ls_native_object * ls_v64 = NULL;
int32_t ls_v65;
int32_t ls_v66;
int32_t ls_v67;
int32_t ls_v68;
ls_native_object * ls_v70 = NULL;
int32_t ls_v71;
int32_t ls_v72;
ls_native_object * ls_v74 = NULL;
int32_t ls_v75;
int32_t ls_v76;
ls_native_object * ls_v77 = NULL;
int32_t ls_v78;
int32_t ls_v79;
int32_t ls_v80;
int32_t ls_v81;
ls_native_object * ls_v83 = NULL;
int32_t ls_v84;
int32_t ls_v85;
ls_native_object * ls_v87 = NULL;
int32_t ls_v88;
int32_t ls_v89;
ls_native_object * ls_v90 = NULL;
ls_native_object * ls_v91 = NULL;
int32_t ls_v92;
ls_native_object * ls_v94 = NULL;
int32_t ls_v95;
ls_native_object * ls_v97 = NULL;
int32_t ls_v98;
int32_t ls_v99;
ls_native_object * ls_v100 = NULL;
int32_t ls_v101;
int32_t ls_v102;
ls_native_object * ls_v104 = NULL;
int32_t ls_v105;
int32_t ls_v106;
ls_native_object * ls_v107 = NULL;
ls_native_object * ls_v108 = NULL;
int32_t ls_v109;
ls_native_object * ls_v111 = NULL;
int32_t ls_v112;
int32_t ls_v113;
ls_native_object * ls_v114 = NULL;
int32_t ls_v115;
int32_t ls_v116;
ls_native_object * ls_v118 = NULL;
int32_t ls_v119;
int32_t ls_v120;
ls_native_object * ls_v122 = NULL;
int32_t ls_v123;
int32_t ls_v124;
ls_native_object * ls_v125 = NULL;
ls_native_object * ls_v126 = NULL;
int32_t ls_v127;
ls_native_object * ls_v129 = NULL;
ls_native_object * ls_v130 = NULL;
ls_native_object * ls_v131 = NULL;
int32_t ls_v132;
int32_t ls_v133;
int32_t ls_v135;
ls_native_object * ls_v136 = NULL;
ls_native_object * ls_v137 = NULL;
ls_native_object * ls_v138 = NULL;
ls_native_object * ls_v139 = NULL;
int32_t ls_v140;
int32_t ls_v141;
ls_native_object * ls_v142 = NULL;
int32_t ls_v143;
ls_native_object * ls_v145 = NULL;
int32_t ls_v146;
int32_t ls_v147;
ls_native_object * ls_v149 = NULL;
int32_t ls_v150;
int32_t ls_v151;
ls_native_object * ls_v152 = NULL;
ls_native_object * ls_v153 = NULL;
int32_t ls_v154;
ls_v0 = ls_from_u32(UINT32_C(0));
ls_object_take(&(ls_v1),ls_buffer_new(ls_v0));
ls_v2 = ls_from_u32(UINT32_C(0));
ls_object_take(&(ls_v3),ls_typed_new(ls_v2, 1));
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy);
((ls_object0 *)ls_o)->ls_m0 = ls_v1;
ls_native_retain(((ls_object0 *)ls_o)->ls_m0);
((ls_object0 *)ls_o)->ls_m1 = ls_v3;
ls_native_retain(((ls_object0 *)ls_o)->ls_m1);
ls_object_take(&(ls_v4),(ls_native_object *)ls_o);
}
ls_object_copy(&(ls_c1),ls_v4);
ls_object_copy(&(ls_v5),ls_c1);
ls_object_copy(&(ls_v6),((ls_object0 *)ls_v5)->ls_m0);
ls_v7 = ls_buffer_byte_length(ls_v6);
printf("%ld\n",(long)ls_v7);
ls_object_copy(&(ls_v9),ls_c1);
ls_object_copy(&(ls_v10),((ls_object0 *)ls_v9)->ls_m1);
ls_v11 = ls_typed_length(ls_v10);
printf("%ld\n",(long)ls_v11);
ls_v13 = ls_from_u32(UINT32_C(6));
ls_object_take(&(ls_v14),ls_buffer_new(ls_v13));
ls_object_copy(&(ls_c2),ls_v14);
ls_object_copy(&(ls_v15),ls_c2);
ls_object_take(&(ls_v16),ls_typed_over(ls_v15, 1));
ls_object_copy(&(ls_c3),ls_v16);
ls_object_copy(&(ls_v17),ls_c2);
ls_v18 = ls_buffer_byte_length(ls_v17);
printf("%ld\n",(long)ls_v18);
ls_object_copy(&(ls_v20),ls_c3);
ls_v21 = ls_typed_length(ls_v20);
printf("%ld\n",(long)ls_v21);
ls_object_copy(&(ls_v23),ls_c3);
ls_v24 = (int32_t)(ls_typed_length(ls_v23) * 1);
printf("%ld\n",(long)ls_v24);
ls_object_copy(&(ls_v26),ls_c3);
ls_v27 = ls_typed_byte_offset(ls_v26);
printf("%ld\n",(long)ls_v27);
ls_object_copy(&(ls_v29),ls_c3);
ls_v30 = ls_from_u32(UINT32_C(0));
ls_v31 = ls_from_u32(UINT32_C(7));
ls_typed_set_uint8(ls_v29,ls_v30,ls_v31);
ls_object_copy(&(ls_v32),ls_c3);
ls_v33 = ls_from_u32(UINT32_C(1));
ls_v34 = ls_from_u32(UINT32_C(257));
ls_typed_set_uint8(ls_v32,ls_v33,ls_v34);
ls_object_copy(&(ls_v35),ls_c3);
ls_v36 = ls_from_u32(UINT32_C(2));
ls_v37 = ls_from_u32(UINT32_C(1));
ls_v38 = ls_from_u32((uint32_t)(UINT32_C(0) - (uint32_t)ls_v37));
ls_typed_set_uint8(ls_v35,ls_v36,ls_v38);
ls_object_copy(&(ls_v39),ls_c3);
ls_v40 = ls_from_u32(UINT32_C(0));
ls_v41 = ls_typed_get_uint8(ls_v39,ls_v40);
printf("%ld\n",(long)ls_v41);
ls_object_copy(&(ls_v43),ls_c3);
ls_v44 = ls_from_u32(UINT32_C(1));
ls_v45 = ls_typed_get_uint8(ls_v43,ls_v44);
printf("%ld\n",(long)ls_v45);
ls_object_copy(&(ls_v47),ls_c3);
ls_v48 = ls_from_u32(UINT32_C(2));
ls_v49 = ls_typed_get_uint8(ls_v47,ls_v48);
printf("%ld\n",(long)ls_v49);
ls_object_copy(&(ls_v51),ls_c3);
ls_v52 = ls_from_u32(UINT32_C(3));
ls_v53 = ls_from_u32(UINT32_C(255));
ls_typed_set_uint8(ls_v51,ls_v52,ls_v53);
ls_object_copy(&(ls_v54),ls_c3);
ls_v55 = ls_from_u32(UINT32_C(3));
ls_v56 = ls_typed_get_uint8(ls_v54,ls_v55);
ls_v57 = ls_from_u32(UINT32_C(1));
ls_v58 = ls_from_u32((uint32_t)((uint32_t)ls_v56 + (uint32_t)ls_v57));
ls_typed_set_uint8(ls_v54,ls_v55,ls_v58);
printf("%ld\n",(long)ls_v58);
ls_object_copy(&(ls_v60),ls_c3);
ls_v61 = ls_from_u32(UINT32_C(3));
ls_v62 = ls_typed_get_uint8(ls_v60,ls_v61);
printf("%ld\n",(long)ls_v62);
ls_object_copy(&(ls_v64),ls_c3);
ls_v65 = ls_from_u32(UINT32_C(3));
ls_v66 = ls_typed_get_uint8(ls_v64,ls_v65);
ls_v67 = ls_from_u32(UINT32_C(1));
ls_v68 = ls_from_u32((uint32_t)((uint32_t)ls_v66 - (uint32_t)ls_v67));
ls_typed_set_uint8(ls_v64,ls_v65,ls_v68);
printf("%ld\n",(long)ls_v68);
ls_object_copy(&(ls_v70),ls_c3);
ls_v71 = ls_from_u32(UINT32_C(3));
ls_v72 = ls_typed_get_uint8(ls_v70,ls_v71);
printf("%ld\n",(long)ls_v72);
ls_object_copy(&(ls_v74),ls_c3);
ls_v75 = ls_from_u32(UINT32_C(3));
ls_v76 = ls_from_u32(UINT32_C(255));
ls_typed_set_uint8(ls_v74,ls_v75,ls_v76);
ls_object_copy(&(ls_v77),ls_c3);
ls_v78 = ls_from_u32(UINT32_C(3));
ls_v79 = ls_typed_get_uint8(ls_v77,ls_v78);
ls_v80 = ls_from_u32(UINT32_C(2));
ls_v81 = ls_from_u32((uint32_t)((uint32_t)ls_v79 + (uint32_t)ls_v80));
ls_typed_set_uint8(ls_v77,ls_v78,ls_v81);
printf("%ld\n",(long)ls_v81);
ls_object_copy(&(ls_v83),ls_c3);
ls_v84 = ls_from_u32(UINT32_C(3));
ls_v85 = ls_typed_get_uint8(ls_v83,ls_v84);
printf("%ld\n",(long)ls_v85);
ls_object_copy(&(ls_v87),ls_c3);
ls_v88 = ls_from_u32(UINT32_C(1));
ls_v89 = ls_from_u32(UINT32_C(3));
ls_object_take(&(ls_v90),ls_typed_subarray(ls_v87, ls_v88, ls_v89, 1));
ls_object_copy(&(ls_c4),ls_v90);
ls_object_copy(&(ls_v91),ls_c4);
ls_v92 = ls_typed_length(ls_v91);
printf("%ld\n",(long)ls_v92);
ls_object_copy(&(ls_v94),ls_c4);
ls_v95 = ls_typed_byte_offset(ls_v94);
printf("%ld\n",(long)ls_v95);
ls_object_copy(&(ls_v97),ls_c4);
ls_v98 = ls_from_u32(UINT32_C(0));
ls_v99 = ls_from_u32(UINT32_C(9));
ls_typed_set_uint8(ls_v97,ls_v98,ls_v99);
ls_object_copy(&(ls_v100),ls_c3);
ls_v101 = ls_from_u32(UINT32_C(1));
ls_v102 = ls_typed_get_uint8(ls_v100,ls_v101);
printf("%ld\n",(long)ls_v102);
ls_object_copy(&(ls_v104),ls_c3);
ls_v105 = ls_from_u32(UINT32_C(1));
ls_v106 = ls_from_u32(UINT32_C(2147483647));
ls_object_take(&(ls_v107),ls_typed_slice(ls_v104, ls_v105, ls_v106, 1));
ls_object_copy(&(ls_c5),ls_v107);
ls_object_copy(&(ls_v108),ls_c5);
ls_v109 = ls_typed_length(ls_v108);
printf("%ld\n",(long)ls_v109);
ls_object_copy(&(ls_v111),ls_c5);
ls_v112 = ls_from_u32(UINT32_C(0));
ls_v113 = ls_from_u32(UINT32_C(4));
ls_typed_set_uint8(ls_v111,ls_v112,ls_v113);
ls_object_copy(&(ls_v114),ls_c5);
ls_v115 = ls_from_u32(UINT32_C(0));
ls_v116 = ls_typed_get_uint8(ls_v114,ls_v115);
printf("%ld\n",(long)ls_v116);
ls_object_copy(&(ls_v118),ls_c3);
ls_v119 = ls_from_u32(UINT32_C(1));
ls_v120 = ls_typed_get_uint8(ls_v118,ls_v119);
printf("%ld\n",(long)ls_v120);
ls_object_copy(&(ls_v122),ls_c2);
ls_v123 = ls_from_u32(UINT32_C(1));
ls_v124 = ls_from_u32(UINT32_C(4));
ls_object_take(&(ls_v125),ls_buffer_slice(ls_v122, ls_v123, ls_v124));
ls_object_copy(&(ls_c6),ls_v125);
ls_object_copy(&(ls_v126),ls_c6);
ls_v127 = ls_buffer_byte_length(ls_v126);
printf("%ld\n",(long)ls_v127);
ls_object_copy(&(ls_v129),ls_c6);
ls_object_take(&(ls_v130),ls_typed_over(ls_v129, 1));
ls_object_copy(&(ls_c7),ls_v130);
ls_object_copy(&(ls_v131),ls_c7);
ls_v132 = ls_from_u32(UINT32_C(0));
ls_v133 = ls_typed_get_uint8(ls_v131,ls_v132);
printf("%ld\n",(long)ls_v133);
ls_v135 = ls_from_u32(UINT32_C(4));
ls_object_take(&(ls_v136),ls_buffer_new(ls_v135));
ls_object_copy(&(ls_c8),ls_v136);
ls_object_copy(&(ls_v137),ls_c8);
ls_object_take(&(ls_v138),ls_typed_over(ls_v137, 1));
ls_object_copy(&(ls_c9),ls_v138);
ls_object_copy(&(ls_v139),ls_c9);
ls_v140 = ls_from_u32(UINT32_C(2));
ls_v141 = ls_from_u32(UINT32_C(42));
ls_typed_set_uint8(ls_v139,ls_v140,ls_v141);
ls_object_copy(&(ls_v142),ls_c8);
ls_v143 = ls_buffer_byte_length(ls_v142);
printf("%ld\n",(long)ls_v143);
ls_object_copy(&(ls_v145),ls_c9);
ls_v146 = ls_from_u32(UINT32_C(2));
ls_v147 = ls_typed_get_uint8(ls_v145,ls_v146);
printf("%ld\n",(long)ls_v147);
ls_object_copy(&(ls_v149),ls_c8);
ls_v150 = ls_from_u32(UINT32_C(1));
ls_v151 = ls_from_u32(UINT32_C(3));
ls_object_take(&(ls_v152),ls_buffer_slice(ls_v149, ls_v150, ls_v151));
ls_object_copy(&(ls_c10),ls_v152);
ls_object_copy(&(ls_v153),ls_c10);
ls_v154 = ls_buffer_byte_length(ls_v153);
printf("%ld\n",(long)ls_v154);
ls_object_clear(&ls_v153);
ls_object_clear(&ls_c10);
ls_object_clear(&ls_v152);
ls_object_clear(&ls_v149);
ls_object_clear(&ls_v145);
ls_object_clear(&ls_v142);
ls_object_clear(&ls_v139);
ls_object_clear(&ls_c9);
ls_object_clear(&ls_v138);
ls_object_clear(&ls_v137);
ls_object_clear(&ls_c8);
ls_object_clear(&ls_v136);
ls_object_clear(&ls_v131);
ls_object_clear(&ls_c7);
ls_object_clear(&ls_v130);
ls_object_clear(&ls_v129);
ls_object_clear(&ls_v126);
ls_object_clear(&ls_c6);
ls_object_clear(&ls_v125);
ls_object_clear(&ls_v122);
ls_object_clear(&ls_v118);
ls_object_clear(&ls_v114);
ls_object_clear(&ls_v111);
ls_object_clear(&ls_v108);
ls_object_clear(&ls_c5);
ls_object_clear(&ls_v107);
ls_object_clear(&ls_v104);
ls_object_clear(&ls_v100);
ls_object_clear(&ls_v97);
ls_object_clear(&ls_v94);
ls_object_clear(&ls_v91);
ls_object_clear(&ls_c4);
ls_object_clear(&ls_v90);
ls_object_clear(&ls_v87);
ls_object_clear(&ls_v83);
ls_object_clear(&ls_v77);
ls_object_clear(&ls_v74);
ls_object_clear(&ls_v70);
ls_object_clear(&ls_v64);
ls_object_clear(&ls_v60);
ls_object_clear(&ls_v54);
ls_object_clear(&ls_v51);
ls_object_clear(&ls_v47);
ls_object_clear(&ls_v43);
ls_object_clear(&ls_v39);
ls_object_clear(&ls_v35);
ls_object_clear(&ls_v32);
ls_object_clear(&ls_v29);
ls_object_clear(&ls_v26);
ls_object_clear(&ls_v23);
ls_object_clear(&ls_v20);
ls_object_clear(&ls_v17);
ls_object_clear(&ls_c3);
ls_object_clear(&ls_v16);
ls_object_clear(&ls_v15);
ls_object_clear(&ls_c2);
ls_object_clear(&ls_v14);
ls_object_clear(&ls_v10);
ls_object_clear(&ls_v9);
ls_object_clear(&ls_v6);
ls_object_clear(&ls_v5);
ls_object_clear(&ls_c1);
ls_object_clear(&ls_v4);
ls_object_clear(&ls_v3);
ls_object_clear(&ls_v1);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(1);
ls_init0();
return 0;
}
