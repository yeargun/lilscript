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
ls_native_object * (*code)(void *,int32_t);
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
static inline ls_native_object * ls_callable0_call(ls_callable0 value,int32_t ls_p0) {
ls_native_retain(value.environment);
ls_native_object * result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef struct ls_object0 {
ls_native_object owner;
int32_t ls_m0;
} ls_object0;
static void ls_object0_clear_fields(ls_object0 *object) {
(void)object;
}
static void ls_object0_destroy(ls_native_object *owner) { ls_object0_clear_fields((ls_object0 *)owner); }
static void ls_init0(void);
static void ls_fn1(ls_native_object * ls_c2,int32_t ls_c3);
static int32_t ls_fn2(ls_native_object * ls_c4,int32_t ls_c5);
static void ls_init0(void) {
ls_native_object * ls_c1 = NULL;
int32_t ls_v2;
ls_native_object * ls_v3 = NULL;
int32_t ls_v5;
ls_native_object * ls_v7 = NULL;
int32_t ls_v9;
int32_t ls_v10;
ls_native_object * ls_v12 = NULL;
int32_t ls_v14;
int32_t ls_v15;
int32_t ls_v16;
ls_v2 = ls_from_u32(UINT32_C(0));
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy);
((ls_object0 *)ls_o)->ls_m0 = ls_v2;
ls_object_take(&(ls_v3),(ls_native_object *)ls_o);
}
ls_v5 = ls_from_u32(UINT32_C(10));
ls_fn1(ls_v3,ls_v5);
ls_object_copy(&(ls_c1),ls_v3);
ls_object_copy(&(ls_v7),ls_c1);
ls_v9 = ls_from_u32(UINT32_C(5));
ls_v10 = ls_fn2(ls_v7,ls_v9);
printf("%ld\n",(long)ls_v10);
ls_object_copy(&(ls_v12),ls_c1);
ls_v14 = ls_from_u32(UINT32_C(3));
ls_v15 = ls_from_u32((uint32_t)(UINT32_C(0) - (uint32_t)ls_v14));
ls_v16 = ls_fn2(ls_v12,ls_v15);
printf("%ld\n",(long)ls_v16);
ls_object_clear(&ls_v12);
ls_object_clear(&ls_v7);
ls_object_clear(&ls_c1);
ls_object_clear(&ls_v3);
}
static void ls_fn1(ls_native_object * ls_c2,int32_t ls_c3) {
ls_native_retain(ls_c2);
ls_native_object * ls_v0 = NULL;
int32_t ls_v1;
ls_object_copy(&(ls_v0),ls_c2);
ls_v1 = ls_c3;
((ls_object0 *)ls_v0)->ls_m0 = ls_v1;
ls_object_clear(&ls_v0);
ls_object_clear(&ls_c2);
}
static int32_t ls_fn2(ls_native_object * ls_c4,int32_t ls_c5) {
ls_native_retain(ls_c4);
ls_native_object * ls_v0 = NULL;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
ls_native_object * ls_v4 = NULL;
int32_t ls_v5;
ls_object_copy(&(ls_v0),ls_c4);
ls_v1 = ((ls_object0 *)ls_v0)->ls_m0;
ls_v2 = ls_c5;
ls_v3 = ls_from_u32((uint32_t)((uint32_t)ls_v1 + (uint32_t)ls_v2));
((ls_object0 *)ls_v0)->ls_m0 = ls_v3;
ls_object_copy(&(ls_v4),ls_c4);
ls_v5 = ((ls_object0 *)ls_v4)->ls_m0;
{
int32_t ls_return = ls_v5;
ls_object_clear(&ls_v4);
ls_object_clear(&ls_v0);
ls_object_clear(&ls_c4);
return ls_return;
}
ls_object_clear(&ls_v4);
ls_object_clear(&ls_v0);
ls_object_clear(&ls_c4);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(3);
ls_init0();
return 0;
}
