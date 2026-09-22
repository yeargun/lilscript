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
static inline int32_t ls_to_i32(double value) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof bits);
    unsigned biased = (unsigned)((bits >> 52) & UINT64_C(2047));
    if (biased < 1023 || biased >= 1107) return 0;
    unsigned exponent = biased - 1023;
    uint64_t significand = (bits & UINT64_C(0x000fffffffffffff)) | UINT64_C(0x0010000000000000);
    uint32_t low = exponent < 52 ? (uint32_t)(significand >> (52 - exponent))
                                : (uint32_t)((uint32_t)significand << (exponent - 52));
    if (bits >> 63) low = (uint32_t)(UINT32_C(0) - low);
    return ls_from_u32(low);
}
static inline double ls_f64(double value) {
    volatile double rounded = value;
    return rounded;
}
static inline int32_t ls_mul(int32_t left, int32_t right) {
    return ls_to_i32(ls_f64((double)left * (double)right));
}
static inline int32_t ls_rem(int32_t left, int32_t right) {
    if (right == 0 || (left == INT32_MIN && right == -1)) return 0;
    return left % right;
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
#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void) {
    return ls_native_live_objects;
}
#endif
typedef struct ls_array0 ls_array0;
static void ls_native_undefined_element(void) {
fputs("LilScript native array element is undefined\n", stderr);
abort();
}
static size_t ls_array_relative(int32_t index, size_t length) {
if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
return (size_t)index < length ? (size_t)index : length;
}
struct ls_array0 { ls_native_object owner; size_t length; size_t capacity; int32_t *items; };
static void ls_array0_acquire(int32_t value) { (void)value; }
static void ls_array0_drop(int32_t value) { (void)value; }
static void ls_array0_destroy(ls_native_object *owner) {
ls_array0 *array = (ls_array0 *)owner;
for (size_t index = 0; index < array->length; index++) ls_array0_drop(array->items[index]);
free(array->items);
}
static ls_array0 *ls_array0_new(size_t capacity) {
ls_array0 *array = ls_native_allocate(sizeof *array, ls_array0_destroy);
array->length = 0; array->capacity = 0; array->items = NULL;
if (capacity) {
if (capacity > SIZE_MAX / sizeof *array->items) ls_native_resource_failure();
array->items = malloc(capacity * sizeof *array->items);
if (!array->items) ls_native_resource_failure();
array->capacity = capacity;
}
return array;
}
static void ls_array0_reserve(ls_array0 *array, size_t length) {
if (length <= array->capacity) return;
size_t capacity = array->capacity ? array->capacity : 4;
while (capacity < length) { if (capacity > SIZE_MAX / 2 / sizeof *array->items) ls_native_resource_failure(); capacity *= 2; }
int32_t *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static int32_t ls_array0_push_owned(ls_array0 *array, int32_t value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array0_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static int32_t ls_array0_push(ls_array0 *array, int32_t value) {
ls_array0_acquire(value);
return ls_array0_push_owned(array, value);
}
static void ls_array0_hole(ls_array0 *array) { ls_array0_push(array, 0); }
static int32_t ls_array0_get(ls_array0 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { return 0; }
return array->items[index];
}
static void ls_array0_set(ls_array0 *array, int32_t index, int32_t value) {
if (index >= 0 && (size_t)index < array->length) { ls_array0_acquire(value); ls_array0_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array0_push(array, value); return; }
ls_native_undefined_element();
}
static int32_t ls_array0_pop(ls_array0 *array) {
if (!array->length) { return 0; }
return array->items[--array->length];
}
static ls_array0 *ls_array0_slice(ls_array0 *array, size_t start, size_t end) {
ls_array0 *result = ls_array0_new(end > start ? end - start : 0);
for (size_t index = start; index < end && index < array->length; index++) ls_array0_push(result, array->items[index]);
return result;
}
static ls_array0 *ls_array0_concat(ls_array0 *left, ls_array0 *right) {
size_t left_length = left->length, right_length = right->length;
ls_array0 *result = ls_array0_new(left_length + right_length);
for (size_t index = 0; index < left_length; index++) ls_array0_push(result, left->items[index]);
for (size_t index = 0; index < right_length; index++) ls_array0_push(result, right->items[index]);
return result;
}
static ls_array0 *ls_array0_reverse(ls_array0 *array) {
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { int32_t value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static ls_array0 *ls_array0_fill(ls_array0 *array, int32_t value) {
for (size_t index = 0; index < array->length; index++) { ls_array0_acquire(value); ls_array0_drop(array->items[index]); array->items[index] = value; }
return array;
}
static ls_array0 *ls_array0_splice(ls_array0 *array, int32_t start, int32_t count) {
size_t from = ls_array_relative(start, array->length);
size_t removed = count <= 0 ? 0 : (size_t)count;
if (removed > array->length - from) removed = array->length - from;
ls_array0 *result = ls_array0_new(removed);
for (size_t index = 0; index < removed; index++) ls_array0_push_owned(result, array->items[from + index]);
memmove(array->items + from, array->items + from + removed, (array->length - from - removed) * sizeof *array->items);
array->length -= removed;
return result;
}
static ls_array0 *ls_array0_copy_within(ls_array0 *array, int32_t target, int32_t start, bool bounded, int32_t end) {
size_t length = array->length;
size_t to = ls_array_relative(target, length), from = ls_array_relative(start, length);
size_t final = bounded ? ls_array_relative(end, length) : length;
if (final > from) {
size_t count = final - from;
if (count > length - to) count = length - to;
for (size_t index = 0; index < count; index++) ls_array0_acquire(array->items[from + index]);
for (size_t index = 0; index < count; index++) ls_array0_drop(array->items[to + index]);
memmove(array->items + to, array->items + from, count * sizeof *array->items);
}
return array;
}
static void ls_array0_copy(ls_array0 **slot, ls_array0 *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static void ls_array0_take(ls_array0 **slot, ls_array0 *value) { ls_native_release(*slot); *slot = value; }
static void ls_array0_clear(ls_array0 **slot) { ls_native_release(*slot); *slot = NULL; }
static int32_t ls_array0_index_of(ls_array0 *array, int32_t right) {
for (size_t index = 0; index < array->length; index++) { int32_t left = array->items[index]; if (left == right) return (int32_t)index; }
return -1;
}
static bool ls_array0_includes(ls_array0 *array, int32_t right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { int32_t left = array->items[index]; if (left == right) return true; }
return false;
}
static void ls_init0(void);
static void ls_init0(void) {
ls_array0 * ls_c0 = NULL;
int32_t ls_c1;
int32_t ls_c2;
int32_t ls_c3;
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
ls_array0 * ls_v4 = NULL;
ls_array0 * ls_v5 = NULL;
int32_t ls_v6;
int32_t ls_v7;
int32_t ls_v8;
int32_t ls_v9;
int32_t ls_v10;
int32_t ls_v11;
ls_array0 * ls_v12 = NULL;
int32_t ls_v13;
int32_t ls_v14;
int32_t ls_v15;
int32_t ls_v16;
ls_array0 * ls_v17 = NULL;
int32_t ls_v18;
int32_t ls_v19;
int32_t ls_v20;
int32_t ls_v21;
int32_t ls_v22;
ls_array0 * ls_v23 = NULL;
int32_t ls_v24;
int32_t ls_v25;
int32_t ls_v26;
int32_t ls_v27;
int32_t ls_v28;
int32_t ls_v30;
int32_t ls_v32;
ls_v0 = ls_from_u32(UINT32_C(19));
ls_v1 = ls_from_u32((uint32_t)(UINT32_C(0) - (uint32_t)ls_v0));
ls_v2 = ls_from_u32(UINT32_C(7));
ls_v3 = ls_from_u32(UINT32_C(2147483647));
ls_array0_take(&(ls_v4),ls_array0_new(3));
ls_array0_push(ls_v4,ls_v1);
ls_array0_push(ls_v4,ls_v2);
ls_array0_push(ls_v4,ls_v3);
ls_array0_copy(&(ls_c0),ls_v4);
ls_array0_copy(&(ls_v5),ls_c0);
ls_v6 = ls_from_u32(UINT32_C(0));
ls_v7 = ls_array0_get(ls_v5,ls_v6);
ls_v8 = ls_from_u32(UINT32_C(10));
ls_v9 = ls_rem(ls_v7,ls_v8);
ls_v10 = ls_from_u32(UINT32_C(5));
ls_v11 = ls_from_u32((uint32_t)((uint32_t)ls_v9 + (uint32_t)ls_v10));
ls_c1 = ls_v11;
ls_array0_copy(&(ls_v12),ls_c0);
ls_v13 = ls_from_u32(UINT32_C(1));
ls_v14 = ls_array0_get(ls_v12,ls_v13);
ls_v15 = ls_from_u32(UINT32_C(10));
ls_v16 = ls_rem(ls_v14,ls_v15);
ls_array0_copy(&(ls_v17),ls_c0);
ls_v18 = ls_from_u32(UINT32_C(1));
ls_v19 = ls_array0_get(ls_v17,ls_v18);
ls_v20 = ls_from_u32(UINT32_C(10));
ls_v21 = ls_rem(ls_v19,ls_v20);
ls_v22 = ls_mul(ls_v16,ls_v21);
ls_c2 = ls_v22;
ls_array0_copy(&(ls_v23),ls_c0);
ls_v24 = ls_from_u32(UINT32_C(2));
ls_v25 = ls_array0_get(ls_v23,ls_v24);
ls_v26 = ls_from_u32(UINT32_C(1));
ls_v27 = ls_from_u32((uint32_t)((uint32_t)ls_v25 + (uint32_t)ls_v26));
ls_c3 = ls_v27;
ls_v28 = ls_c1;
printf("%ld\n",(long)ls_v28);
ls_v30 = ls_c2;
printf("%ld\n",(long)ls_v30);
ls_v32 = ls_c3;
printf("%ld\n",(long)ls_v32);
ls_array0_clear(&ls_v23);
ls_array0_clear(&ls_v17);
ls_array0_clear(&ls_v12);
ls_array0_clear(&ls_v5);
ls_array0_clear(&ls_c0);
ls_array0_clear(&ls_v4);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(1);
ls_init0();
return 0;
}
