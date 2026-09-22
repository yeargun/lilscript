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
static void ls_init0(void);
static void ls_init0(void) {
int32_t ls_c0;
int32_t ls_c1;
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
bool ls_v4;
int32_t ls_v5;
int32_t ls_v6;
bool ls_v7;
int32_t ls_v8;
int32_t ls_v9;
bool ls_v10;
int32_t ls_v11;
int32_t ls_v12;
int32_t ls_v13;
int32_t ls_v14;
int32_t ls_v15;
int32_t ls_v16;
int32_t ls_v17;
ls_v0 = ls_from_u32(UINT32_C(0));
ls_c0 = ls_v0;
ls_v1 = ls_from_u32(UINT32_C(0));
ls_c1 = ls_v1;
ls_test25: ;
ls_v2 = ls_c1;
ls_v3 = ls_from_u32(UINT32_C(10));
ls_v4 = ls_v2 < ls_v3;
if (!ls_v4) goto ls_end25;
ls_v5 = ls_c1;
ls_v6 = ls_from_u32(UINT32_C(2));
ls_v7 = ls_v5 == ls_v6;
if (ls_v7) {
goto ls_update25;
}
ls_v8 = ls_c1;
ls_v9 = ls_from_u32(UINT32_C(7));
ls_v10 = ls_v8 == ls_v9;
if (ls_v10) {
goto ls_end25;
}
ls_v11 = ls_c0;
ls_v12 = ls_c1;
ls_v13 = ls_from_u32((uint32_t)((uint32_t)ls_v11 + (uint32_t)ls_v12));
ls_c0 = ls_v13;
ls_update25: ;
ls_v14 = ls_c1;
ls_v15 = ls_from_u32(UINT32_C(1));
ls_v16 = ls_from_u32((uint32_t)((uint32_t)ls_v14 + (uint32_t)ls_v15));
ls_c1 = ls_v16;
goto ls_test25;
ls_end25: ;
ls_v17 = ls_c0;
printf("%ld\n",(long)ls_v17);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_init0();
return 0;
}
