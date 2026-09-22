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
typedef struct { ls_native_object owner; int32_t value; } ls_box15;
static ls_box15 *ls_box_new15(int32_t value) {
ls_box15 *box = ls_native_allocate(sizeof *box,NULL);
box->value = value;
return box;
}
typedef struct {
ls_native_object owner;
ls_box15 *ls_e1;
} ls_env3;
static void ls_env_destroy3(ls_native_object *owner) {
ls_env3 *environment = (ls_env3 *)owner;
ls_native_release(environment->ls_e1);
}
static void ls_init0(void);
static int32_t ls_fn1(int32_t ls_c14);
static ls_callable0 ls_fn2(int32_t ls_p15);
static int32_t ls_fn3(void *ls_env,int32_t ls_c16);
static ls_callable0 ls_closure3(ls_box15 *ls_e1) {
ls_env3 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy3);
ls_native_retain(ls_e1);
environment->ls_e1 = ls_e1;
return (ls_callable0){ls_fn3,environment,ls_native_fresh_identity()};
}
static void ls_init0(void) {
ls_callable0 ls_c2 = {0};
ls_callable0 ls_c3 = {0};
ls_callable0 ls_c4 = {0};
ls_callable0 ls_c5 = {0};
ls_callable0 ls_c6 = {0};
ls_callable0 ls_c7 = {0};
ls_callable0 ls_c8 = {0};
ls_callable0 ls_c9 = {0};
ls_callable0 ls_c10 = {0};
ls_callable0 ls_c11 = {0};
ls_callable0 ls_c12 = {0};
ls_callable0 ls_c13 = {0};
int32_t ls_v3;
ls_callable0 ls_v4 = {0};
int32_t ls_v6;
ls_callable0 ls_v7 = {0};
int32_t ls_v9;
ls_callable0 ls_v10 = {0};
int32_t ls_v12;
ls_callable0 ls_v13 = {0};
int32_t ls_v15;
ls_callable0 ls_v16 = {0};
int32_t ls_v18;
ls_callable0 ls_v19 = {0};
int32_t ls_v21;
ls_callable0 ls_v22 = {0};
int32_t ls_v24;
ls_callable0 ls_v25 = {0};
int32_t ls_v27;
ls_callable0 ls_v28 = {0};
int32_t ls_v30;
ls_callable0 ls_v31 = {0};
int32_t ls_v33;
ls_callable0 ls_v34 = {0};
int32_t ls_v36;
ls_callable0 ls_v37 = {0};
ls_callable0 ls_v38 = {0};
int32_t ls_v39;
int32_t ls_v40;
ls_callable0 ls_v42 = {0};
int32_t ls_v43;
int32_t ls_v44;
ls_callable0 ls_v46 = {0};
int32_t ls_v47;
int32_t ls_v48;
ls_callable0 ls_v50 = {0};
int32_t ls_v51;
int32_t ls_v52;
ls_callable0 ls_v54 = {0};
int32_t ls_v55;
int32_t ls_v56;
ls_callable0 ls_v58 = {0};
int32_t ls_v59;
int32_t ls_v60;
ls_callable0 ls_v62 = {0};
int32_t ls_v63;
int32_t ls_v64;
ls_callable0 ls_v66 = {0};
int32_t ls_v67;
int32_t ls_v68;
ls_callable0 ls_v70 = {0};
int32_t ls_v71;
int32_t ls_v72;
ls_callable0 ls_v74 = {0};
int32_t ls_v75;
int32_t ls_v76;
ls_callable0 ls_v78 = {0};
int32_t ls_v79;
int32_t ls_v80;
ls_callable0 ls_v82 = {0};
int32_t ls_v83;
int32_t ls_v84;
ls_callable0 ls_v86 = {0};
int32_t ls_v87;
int32_t ls_v88;
ls_callable0 ls_v90 = {0};
int32_t ls_v91;
int32_t ls_v92;
ls_callable0 ls_v94 = {0};
int32_t ls_v95;
int32_t ls_v96;
ls_callable0 ls_v98 = {0};
int32_t ls_v99;
int32_t ls_v100;
ls_callable0 ls_v102 = {0};
int32_t ls_v103;
int32_t ls_v104;
ls_callable0 ls_v106 = {0};
int32_t ls_v107;
int32_t ls_v108;
ls_callable0 ls_v110 = {0};
int32_t ls_v111;
int32_t ls_v112;
ls_callable0 ls_v114 = {0};
int32_t ls_v115;
int32_t ls_v116;
ls_callable0 ls_v118 = {0};
int32_t ls_v119;
int32_t ls_v120;
ls_callable0 ls_v122 = {0};
int32_t ls_v123;
int32_t ls_v124;
ls_callable0 ls_v126 = {0};
int32_t ls_v127;
int32_t ls_v128;
ls_callable0 ls_v130 = {0};
int32_t ls_v131;
int32_t ls_v132;
ls_v3 = ls_from_u32(UINT32_C(1));
ls_callable0_take(&(ls_v4),ls_fn2(ls_v3));
ls_callable0_copy(&(ls_c2),ls_v4);
ls_v6 = ls_from_u32(UINT32_C(2));
ls_callable0_take(&(ls_v7),ls_fn2(ls_v6));
ls_callable0_copy(&(ls_c3),ls_v7);
ls_v9 = ls_from_u32(UINT32_C(3));
ls_callable0_take(&(ls_v10),ls_fn2(ls_v9));
ls_callable0_copy(&(ls_c4),ls_v10);
ls_v12 = ls_from_u32(UINT32_C(4));
ls_callable0_take(&(ls_v13),ls_fn2(ls_v12));
ls_callable0_copy(&(ls_c5),ls_v13);
ls_v15 = ls_from_u32(UINT32_C(5));
ls_callable0_take(&(ls_v16),ls_fn2(ls_v15));
ls_callable0_copy(&(ls_c6),ls_v16);
ls_v18 = ls_from_u32(UINT32_C(6));
ls_callable0_take(&(ls_v19),ls_fn2(ls_v18));
ls_callable0_copy(&(ls_c7),ls_v19);
ls_v21 = ls_from_u32(UINT32_C(7));
ls_callable0_take(&(ls_v22),ls_fn2(ls_v21));
ls_callable0_copy(&(ls_c8),ls_v22);
ls_v24 = ls_from_u32(UINT32_C(8));
ls_callable0_take(&(ls_v25),ls_fn2(ls_v24));
ls_callable0_copy(&(ls_c9),ls_v25);
ls_v27 = ls_from_u32(UINT32_C(9));
ls_callable0_take(&(ls_v28),ls_fn2(ls_v27));
ls_callable0_copy(&(ls_c10),ls_v28);
ls_v30 = ls_from_u32(UINT32_C(10));
ls_callable0_take(&(ls_v31),ls_fn2(ls_v30));
ls_callable0_copy(&(ls_c11),ls_v31);
ls_v33 = ls_from_u32(UINT32_C(11));
ls_callable0_take(&(ls_v34),ls_fn2(ls_v33));
ls_callable0_copy(&(ls_c12),ls_v34);
ls_v36 = ls_from_u32(UINT32_C(12));
ls_callable0_take(&(ls_v37),ls_fn2(ls_v36));
ls_callable0_copy(&(ls_c13),ls_v37);
ls_callable0_copy(&(ls_v38),ls_c2);
ls_v39 = ls_from_u32(UINT32_C(10));
ls_v40 = ls_v38.code(ls_v38.environment,ls_v39);
printf("%ld\n",(long)ls_v40);
ls_callable0_copy(&(ls_v42),ls_c3);
ls_v43 = ls_from_u32(UINT32_C(11));
ls_v44 = ls_v42.code(ls_v42.environment,ls_v43);
printf("%ld\n",(long)ls_v44);
ls_callable0_copy(&(ls_v46),ls_c4);
ls_v47 = ls_from_u32(UINT32_C(12));
ls_v48 = ls_v46.code(ls_v46.environment,ls_v47);
printf("%ld\n",(long)ls_v48);
ls_callable0_copy(&(ls_v50),ls_c5);
ls_v51 = ls_from_u32(UINT32_C(13));
ls_v52 = ls_v50.code(ls_v50.environment,ls_v51);
printf("%ld\n",(long)ls_v52);
ls_callable0_copy(&(ls_v54),ls_c6);
ls_v55 = ls_from_u32(UINT32_C(14));
ls_v56 = ls_v54.code(ls_v54.environment,ls_v55);
printf("%ld\n",(long)ls_v56);
ls_callable0_copy(&(ls_v58),ls_c7);
ls_v59 = ls_from_u32(UINT32_C(15));
ls_v60 = ls_v58.code(ls_v58.environment,ls_v59);
printf("%ld\n",(long)ls_v60);
ls_callable0_copy(&(ls_v62),ls_c8);
ls_v63 = ls_from_u32(UINT32_C(16));
ls_v64 = ls_v62.code(ls_v62.environment,ls_v63);
printf("%ld\n",(long)ls_v64);
ls_callable0_copy(&(ls_v66),ls_c9);
ls_v67 = ls_from_u32(UINT32_C(17));
ls_v68 = ls_v66.code(ls_v66.environment,ls_v67);
printf("%ld\n",(long)ls_v68);
ls_callable0_copy(&(ls_v70),ls_c10);
ls_v71 = ls_from_u32(UINT32_C(18));
ls_v72 = ls_v70.code(ls_v70.environment,ls_v71);
printf("%ld\n",(long)ls_v72);
ls_callable0_copy(&(ls_v74),ls_c11);
ls_v75 = ls_from_u32(UINT32_C(19));
ls_v76 = ls_v74.code(ls_v74.environment,ls_v75);
printf("%ld\n",(long)ls_v76);
ls_callable0_copy(&(ls_v78),ls_c12);
ls_v79 = ls_from_u32(UINT32_C(20));
ls_v80 = ls_v78.code(ls_v78.environment,ls_v79);
printf("%ld\n",(long)ls_v80);
ls_callable0_copy(&(ls_v82),ls_c13);
ls_v83 = ls_from_u32(UINT32_C(21));
ls_v84 = ls_v82.code(ls_v82.environment,ls_v83);
printf("%ld\n",(long)ls_v84);
ls_callable0_copy(&(ls_v86),ls_c2);
ls_v87 = ls_from_u32(UINT32_C(20));
ls_v88 = ls_v86.code(ls_v86.environment,ls_v87);
printf("%ld\n",(long)ls_v88);
ls_callable0_copy(&(ls_v90),ls_c3);
ls_v91 = ls_from_u32(UINT32_C(21));
ls_v92 = ls_v90.code(ls_v90.environment,ls_v91);
printf("%ld\n",(long)ls_v92);
ls_callable0_copy(&(ls_v94),ls_c4);
ls_v95 = ls_from_u32(UINT32_C(22));
ls_v96 = ls_v94.code(ls_v94.environment,ls_v95);
printf("%ld\n",(long)ls_v96);
ls_callable0_copy(&(ls_v98),ls_c5);
ls_v99 = ls_from_u32(UINT32_C(23));
ls_v100 = ls_v98.code(ls_v98.environment,ls_v99);
printf("%ld\n",(long)ls_v100);
ls_callable0_copy(&(ls_v102),ls_c6);
ls_v103 = ls_from_u32(UINT32_C(24));
ls_v104 = ls_v102.code(ls_v102.environment,ls_v103);
printf("%ld\n",(long)ls_v104);
ls_callable0_copy(&(ls_v106),ls_c7);
ls_v107 = ls_from_u32(UINT32_C(25));
ls_v108 = ls_v106.code(ls_v106.environment,ls_v107);
printf("%ld\n",(long)ls_v108);
ls_callable0_copy(&(ls_v110),ls_c8);
ls_v111 = ls_from_u32(UINT32_C(26));
ls_v112 = ls_v110.code(ls_v110.environment,ls_v111);
printf("%ld\n",(long)ls_v112);
ls_callable0_copy(&(ls_v114),ls_c9);
ls_v115 = ls_from_u32(UINT32_C(27));
ls_v116 = ls_v114.code(ls_v114.environment,ls_v115);
printf("%ld\n",(long)ls_v116);
ls_callable0_copy(&(ls_v118),ls_c10);
ls_v119 = ls_from_u32(UINT32_C(28));
ls_v120 = ls_v118.code(ls_v118.environment,ls_v119);
printf("%ld\n",(long)ls_v120);
ls_callable0_copy(&(ls_v122),ls_c11);
ls_v123 = ls_from_u32(UINT32_C(29));
ls_v124 = ls_v122.code(ls_v122.environment,ls_v123);
printf("%ld\n",(long)ls_v124);
ls_callable0_copy(&(ls_v126),ls_c12);
ls_v127 = ls_from_u32(UINT32_C(30));
ls_v128 = ls_v126.code(ls_v126.environment,ls_v127);
printf("%ld\n",(long)ls_v128);
ls_callable0_copy(&(ls_v130),ls_c13);
ls_v131 = ls_from_u32(UINT32_C(31));
ls_v132 = ls_v130.code(ls_v130.environment,ls_v131);
printf("%ld\n",(long)ls_v132);
ls_callable0_clear(&ls_v130);
ls_callable0_clear(&ls_v126);
ls_callable0_clear(&ls_v122);
ls_callable0_clear(&ls_v118);
ls_callable0_clear(&ls_v114);
ls_callable0_clear(&ls_v110);
ls_callable0_clear(&ls_v106);
ls_callable0_clear(&ls_v102);
ls_callable0_clear(&ls_v98);
ls_callable0_clear(&ls_v94);
ls_callable0_clear(&ls_v90);
ls_callable0_clear(&ls_v86);
ls_callable0_clear(&ls_v82);
ls_callable0_clear(&ls_v78);
ls_callable0_clear(&ls_v74);
ls_callable0_clear(&ls_v70);
ls_callable0_clear(&ls_v66);
ls_callable0_clear(&ls_v62);
ls_callable0_clear(&ls_v58);
ls_callable0_clear(&ls_v54);
ls_callable0_clear(&ls_v50);
ls_callable0_clear(&ls_v46);
ls_callable0_clear(&ls_v42);
ls_callable0_clear(&ls_v38);
ls_callable0_clear(&ls_c13);
ls_callable0_clear(&ls_v37);
ls_callable0_clear(&ls_c12);
ls_callable0_clear(&ls_v34);
ls_callable0_clear(&ls_c11);
ls_callable0_clear(&ls_v31);
ls_callable0_clear(&ls_c10);
ls_callable0_clear(&ls_v28);
ls_callable0_clear(&ls_c9);
ls_callable0_clear(&ls_v25);
ls_callable0_clear(&ls_c8);
ls_callable0_clear(&ls_v22);
ls_callable0_clear(&ls_c7);
ls_callable0_clear(&ls_v19);
ls_callable0_clear(&ls_c6);
ls_callable0_clear(&ls_v16);
ls_callable0_clear(&ls_c5);
ls_callable0_clear(&ls_v13);
ls_callable0_clear(&ls_c4);
ls_callable0_clear(&ls_v10);
ls_callable0_clear(&ls_c3);
ls_callable0_clear(&ls_v7);
ls_callable0_clear(&ls_c2);
ls_callable0_clear(&ls_v4);
}
static int32_t ls_fn1(int32_t ls_c14) {
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
ls_v0 = ls_c14;
ls_v1 = ls_from_u32(UINT32_C(7));
ls_v2 = ls_mul(ls_v0,ls_v1);
ls_v3 = ls_from_u32(UINT32_C(11));
ls_v4 = ls_from_u32((uint32_t)((uint32_t)ls_v2 + (uint32_t)ls_v3));
{
int32_t ls_return = ls_v4;
return ls_return;
}
}
static ls_callable0 ls_fn2(int32_t ls_p15) {
ls_box15 *ls_c15 = ls_box_new15(ls_p15);
ls_callable0 ls_v0 = {0};
ls_callable0_take(&(ls_v0),ls_closure3(ls_c15));
{
ls_callable0 ls_return = ls_v0;
ls_native_retain(ls_return.environment);
ls_callable0_clear(&ls_v0);
ls_native_release(ls_c15);
ls_c15 = NULL;
return ls_return;
}
ls_callable0_clear(&ls_v0);
ls_native_release(ls_c15);
ls_c15 = NULL;
}
static int32_t ls_fn3(void *ls_env,int32_t ls_c16) {
int32_t ls_c17;
int32_t ls_v0;
int32_t ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
bool ls_v5;
int32_t ls_v6;
int32_t ls_v7;
int32_t ls_v8;
int32_t ls_v10;
int32_t ls_v11;
ls_v0 = ls_c16;
ls_v1 = ((ls_env3 *)ls_env)->ls_e1->value;
ls_v2 = ls_from_u32((uint32_t)((uint32_t)ls_v0 + (uint32_t)ls_v1));
ls_c17 = ls_v2;
ls_v3 = ls_c17;
ls_v4 = ls_from_u32(UINT32_C(100));
ls_v5 = ls_v3 > ls_v4;
if (ls_v5) {
ls_v6 = ls_c17;
ls_v7 = ls_from_u32(UINT32_C(3));
ls_v8 = ls_from_u32((uint32_t)((uint32_t)ls_v6 - (uint32_t)ls_v7));
{
int32_t ls_return = ls_v8;
return ls_return;
}
}
ls_v10 = ls_c17;
ls_v11 = ls_fn1(ls_v10);
{
int32_t ls_return = ls_v11;
return ls_return;
}
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(4);
ls_init0();
return 0;
}
