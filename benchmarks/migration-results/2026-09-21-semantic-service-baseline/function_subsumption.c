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
static inline int32_t ls_shr(int32_t value, int32_t count) {
    uint32_t shift = (uint32_t)count & UINT32_C(31);
    if (shift == 0) return value;
    uint32_t bits = (uint32_t)value >> shift;
    if (value < 0) bits |= (uint32_t)(UINT32_MAX << (32 - shift));
    return ls_from_u32(bits);
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
int32_t (*code)(void *,int32_t);
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
static inline int32_t ls_callable0_call(ls_callable0 value,int32_t ls_p0) {
ls_native_retain(value.environment);
int32_t result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static void ls_init0(void);
static int32_t ls_fn1(int32_t ls_c7);
static int32_t ls_fn2(int32_t ls_c9,int32_t ls_c10);
static int32_t ls_fn3(int32_t ls_c12);
static int32_t ls_fn4(int32_t ls_c13);
static int32_t ls_fn5(int32_t ls_c14);
static int32_t ls_fn6(int32_t ls_c15,ls_callable0 ls_c16);
static int32_t ls_adapter3(void *environment,int32_t ls_p0) {
(void)environment;
return ls_fn3(ls_p0);
}
static int32_t ls_adapter4(void *environment,int32_t ls_p0) {
(void)environment;
return ls_fn4(ls_p0);
}
static void ls_init0(void) {
int32_t ls_c6;
int32_t ls_c17;
int32_t ls_v6;
int32_t ls_v7;
int32_t ls_v8;
int32_t ls_v9;
bool ls_v10;
int32_t ls_v11;
int32_t ls_v12;
int32_t ls_v13;
int32_t ls_v14;
bool ls_v15;
int32_t ls_v16;
int32_t ls_v18;
int32_t ls_v19;
int32_t ls_v20;
int32_t ls_v21;
int32_t ls_v22;
int32_t ls_v23;
int32_t ls_v24;
bool ls_v25;
int32_t ls_v26;
int32_t ls_v28;
int32_t ls_v29;
int32_t ls_v30;
int32_t ls_v31;
int32_t ls_v32;
int32_t ls_v34;
int32_t ls_v35;
int32_t ls_v36;
int32_t ls_v37;
int32_t ls_v38;
int32_t ls_v39;
int32_t ls_v40;
int32_t ls_v41;
bool ls_v42;
int32_t ls_v43;
int32_t ls_v45;
int32_t ls_v46;
int32_t ls_v47;
int32_t ls_v48;
int32_t ls_v49;
int32_t ls_v50;
int32_t ls_v51;
bool ls_v52;
int32_t ls_v53;
int32_t ls_v55;
int32_t ls_v57;
int32_t ls_v58;
int32_t ls_v59;
int32_t ls_v61;
int32_t ls_v63;
int32_t ls_v64;
int32_t ls_v65;
int32_t ls_v66;
int32_t ls_v67;
int32_t ls_v68;
ls_v6 = ls_from_u32(UINT32_C(0));
ls_c6 = ls_v6;
ls_v7 = ls_from_u32(UINT32_C(0));
ls_c17 = ls_v7;
ls_test93: ;
ls_v8 = ls_c17;
ls_v9 = ls_from_u32(UINT32_C(400));
ls_v10 = ls_v8 < ls_v9;
if (!ls_v10) goto ls_end93;
ls_v11 = ls_c17;
ls_v12 = ls_from_u32(UINT32_C(3));
ls_v13 = ls_rem(ls_v11,ls_v12);
ls_v14 = ls_from_u32(UINT32_C(0));
ls_v15 = ls_v13 == ls_v14;
if (ls_v15) {
ls_v16 = ls_c6;
ls_v18 = ls_c17;
ls_v19 = ls_fn1(ls_v18);
ls_v20 = ls_from_u32((uint32_t)((uint32_t)ls_v16 + (uint32_t)ls_v19));
ls_c6 = ls_v20;
} else {
ls_v21 = ls_c17;
ls_v22 = ls_from_u32(UINT32_C(3));
ls_v23 = ls_rem(ls_v21,ls_v22);
ls_v24 = ls_from_u32(UINT32_C(1));
ls_v25 = ls_v23 == ls_v24;
if (ls_v25) {
ls_v26 = ls_c6;
ls_v28 = ls_c17;
ls_v29 = ls_from_u32(UINT32_C(2));
ls_v30 = ls_fn2(ls_v28,ls_v29);
ls_v31 = ls_from_u32((uint32_t)((uint32_t)ls_v26 + (uint32_t)ls_v30));
ls_c6 = ls_v31;
} else {
ls_v32 = ls_c6;
ls_v34 = ls_c17;
ls_v35 = ls_from_u32(UINT32_C(3));
ls_v36 = ls_fn2(ls_v34,ls_v35);
ls_v37 = ls_from_u32((uint32_t)((uint32_t)ls_v32 + (uint32_t)ls_v36));
ls_c6 = ls_v37;
}
}
ls_v38 = ls_c17;
ls_v39 = ls_from_u32(UINT32_C(2));
ls_v40 = ls_rem(ls_v38,ls_v39);
ls_v41 = ls_from_u32(UINT32_C(0));
ls_v42 = ls_v40 == ls_v41;
if (ls_v42) {
ls_v43 = ls_c6;
ls_v45 = ls_c17;
ls_v46 = ls_fn5(ls_v45);
ls_v47 = ls_from_u32((uint32_t)((uint32_t)ls_v43 + (uint32_t)ls_v46));
ls_c6 = ls_v47;
} else {
ls_v48 = ls_c17;
ls_v49 = ls_from_u32(UINT32_C(4));
ls_v50 = ls_rem(ls_v48,ls_v49);
ls_v51 = ls_from_u32(UINT32_C(1));
ls_v52 = ls_v50 == ls_v51;
if (ls_v52) {
ls_v53 = ls_c6;
ls_v55 = ls_c17;
ls_v57 = ls_fn6(ls_v55,(ls_callable0){ls_adapter3,NULL,UINT64_C(4)});
ls_v58 = ls_from_u32((uint32_t)((uint32_t)ls_v53 + (uint32_t)ls_v57));
ls_c6 = ls_v58;
} else {
ls_v59 = ls_c6;
ls_v61 = ls_c17;
ls_v63 = ls_fn6(ls_v61,(ls_callable0){ls_adapter4,NULL,UINT64_C(5)});
ls_v64 = ls_from_u32((uint32_t)((uint32_t)ls_v59 + (uint32_t)ls_v63));
ls_c6 = ls_v64;
}
}
ls_update93: ;
ls_v65 = ls_c17;
ls_v66 = ls_from_u32(UINT32_C(1));
ls_v67 = ls_from_u32((uint32_t)((uint32_t)ls_v65 + (uint32_t)ls_v66));
ls_c17 = ls_v67;
goto ls_test93;
ls_end93: ;
ls_v68 = ls_c6;
printf("%ld\n",(long)ls_v68);
}
static int32_t ls_fn1(int32_t ls_c7) {
int32_t ls_c8;
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
int32_t ls_v5;
int32_t ls_v6;
int32_t ls_v7;
int32_t ls_v8;
int32_t ls_v9;
int32_t ls_v10;
int32_t ls_v11;
int32_t ls_v12;
int32_t ls_v13;
int32_t ls_v14;
ls_v0 = ls_c7;
ls_v1 = ls_from_u32(UINT32_C(1));
ls_v2 = ls_from_u32((uint32_t)((uint32_t)ls_v0 + (uint32_t)ls_v1));
ls_v3 = ls_from_u32(UINT32_C(17));
ls_v4 = ls_mul(ls_v2,ls_v3);
ls_v5 = ls_from_u32(UINT32_C(31));
ls_v6 = ls_from_u32((uint32_t)((uint32_t)ls_v4 + (uint32_t)ls_v5));
ls_c8 = ls_v6;
ls_v7 = ls_c8;
ls_v8 = ls_c8;
ls_v9 = ls_from_u32(UINT32_C(3));
ls_v10 = ls_shr(ls_v8,ls_v9);
ls_v11 = ls_from_u32((uint32_t)ls_v7 ^ (uint32_t)ls_v10);
ls_c8 = ls_v11;
ls_v12 = ls_c8;
ls_v13 = ls_from_u32(UINT32_C(997));
ls_v14 = ls_rem(ls_v12,ls_v13);
{
int32_t ls_return = ls_v14;
return ls_return;
}
}
static int32_t ls_fn2(int32_t ls_c9,int32_t ls_c10) {
int32_t ls_c11;
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
int32_t ls_v5;
int32_t ls_v6;
int32_t ls_v7;
int32_t ls_v8;
int32_t ls_v9;
int32_t ls_v10;
int32_t ls_v11;
int32_t ls_v12;
int32_t ls_v13;
int32_t ls_v14;
ls_v0 = ls_c9;
ls_v1 = ls_c10;
ls_v2 = ls_from_u32((uint32_t)((uint32_t)ls_v0 + (uint32_t)ls_v1));
ls_v3 = ls_from_u32(UINT32_C(17));
ls_v4 = ls_mul(ls_v2,ls_v3);
ls_v5 = ls_from_u32(UINT32_C(31));
ls_v6 = ls_from_u32((uint32_t)((uint32_t)ls_v4 + (uint32_t)ls_v5));
ls_c11 = ls_v6;
ls_v7 = ls_c11;
ls_v8 = ls_c11;
ls_v9 = ls_from_u32(UINT32_C(3));
ls_v10 = ls_shr(ls_v8,ls_v9);
ls_v11 = ls_from_u32((uint32_t)ls_v7 ^ (uint32_t)ls_v10);
ls_c11 = ls_v11;
ls_v12 = ls_c11;
ls_v13 = ls_from_u32(UINT32_C(997));
ls_v14 = ls_rem(ls_v12,ls_v13);
{
int32_t ls_return = ls_v14;
return ls_return;
}
}
static int32_t ls_fn3(int32_t ls_c12) {
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
ls_v0 = ls_c12;
ls_v1 = ls_from_u32(UINT32_C(3));
ls_v2 = ls_mul(ls_v0,ls_v1);
{
int32_t ls_return = ls_v2;
return ls_return;
}
}
static int32_t ls_fn4(int32_t ls_c13) {
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
ls_v0 = ls_c13;
ls_v1 = ls_from_u32(UINT32_C(2));
ls_v2 = ls_mul(ls_v0,ls_v1);
{
int32_t ls_return = ls_v2;
return ls_return;
}
}
static int32_t ls_fn5(int32_t ls_c14) {
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
int32_t ls_v5;
int32_t ls_v6;
ls_v1 = ls_c14;
ls_v2 = ls_fn3(ls_v1);
ls_v3 = ls_from_u32(UINT32_C(17));
ls_v4 = ls_mul(ls_v2,ls_v3);
ls_v5 = ls_from_u32(UINT32_C(31));
ls_v6 = ls_from_u32((uint32_t)((uint32_t)ls_v4 + (uint32_t)ls_v5));
{
int32_t ls_return = ls_v6;
return ls_return;
}
}
static int32_t ls_fn6(int32_t ls_c15,ls_callable0 ls_c16) {
ls_native_retain(ls_c16.environment);
ls_callable0 ls_v0 = {0};
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
int32_t ls_v5;
int32_t ls_v6;
ls_callable0_copy(&(ls_v0),ls_c16);
ls_v1 = ls_c15;
ls_v2 = ls_v0.code(ls_v0.environment,ls_v1);
ls_v3 = ls_from_u32(UINT32_C(17));
ls_v4 = ls_mul(ls_v2,ls_v3);
ls_v5 = ls_from_u32(UINT32_C(31));
ls_v6 = ls_from_u32((uint32_t)((uint32_t)ls_v4 + (uint32_t)ls_v5));
{
int32_t ls_return = ls_v6;
ls_callable0_clear(&ls_v0);
ls_callable0_clear(&ls_c16);
return ls_return;
}
ls_callable0_clear(&ls_v0);
ls_callable0_clear(&ls_c16);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(7);
ls_init0();
return 0;
}
