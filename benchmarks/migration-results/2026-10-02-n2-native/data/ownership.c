/* LilScript native: build as C11 or later with the compiler's default
   floating-point options (`cc -O2 file.c -lm`). No -ffast-math, unsafe-math,
   -fno-signed-zeros or -ffp-contract=fast: each double operation must round
   as in JavaScript. The file disables contraction and checks the rest. */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <inttypes.h>
#include <limits.h>
#include <float.h>
#include <fenv.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#if !defined(__STDC_VERSION__) || __STDC_VERSION__ < 201112L
#error "LilScript native requires C11"
#endif
#if defined(__FAST_MATH__) || (defined(__FINITE_MATH_ONLY__) && __FINITE_MATH_ONLY__ != 0)
#error "LilScript native forbids fast-math and finite-only arithmetic"
#endif
#if defined(__ASSOCIATIVE_MATH__) || defined(__RECIPROCAL_MATH__) || defined(__NO_SIGNED_ZEROS__)
#error "LilScript native forbids reassociation, reciprocal and signed-zero-free arithmetic"
#endif
#if FLT_EVAL_METHOD != 0
#error "LilScript native requires evaluation in the declared floating type"
#endif
#if defined(__clang__)
#pragma clang fp contract(off)
#pragma clang fp reassociate(off)
#elif defined(__GNUC__)
#pragma GCC optimize("fp-contract=off")
#else
#pragma STDC FP_CONTRACT OFF
#endif
_Static_assert((-1 >> 1) == -1, "LilScript native requires arithmetic right shifts of negative values");
_Static_assert(CHAR_BIT == 8, "LilScript native requires 8-bit bytes");
_Static_assert(sizeof(uint16_t) == 2 && sizeof(uint32_t) == 4 && sizeof(uint64_t) == 8,
               "LilScript native requires exact integer widths");
_Static_assert(SIZE_MAX >= UINT32_MAX, "LilScript native requires at least 32-bit object sizes");
_Static_assert(sizeof(double) == 8 && FLT_RADIX == 2 && DBL_MANT_DIG == 53 &&
               DBL_MIN_EXP == -1021 && DBL_MAX_EXP == 1024 && DBL_HAS_SUBNORM == 1,
               "LilScript native requires binary64 with subnormals");

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
    /* (1+2^-27)(1-2^-27)-1 is 0 when the product is rounded and -2^-54 when a
       fused multiply-add contracts it: Clang's -ffp-contract=fast ignores the
       pragmas above. Separate volatile reads keep the two products apart. */
    volatile double left = 1.0 + 0x1p-27, right = 1.0 - 0x1p-27, addend = -1.0;
    double combined = left * right + addend;
    volatile double product = left * right;
    if (combined != product + addend) {
        fputs("LilScript native forbids floating-point contraction (-ffp-contract=fast)\n", stderr);
        return 0;
    }
    return 1;
}
#ifndef LS_NATIVE_UNUSED
#if defined(__GNUC__) || defined(__clang__)
#define LS_NATIVE_UNUSED __attribute__((unused))
#else
#define LS_NATIVE_UNUSED
#endif
#endif
/* UTF-16 string ABI 2. Literals/host-static storage have owner == NULL.
   Parameters borrow; returned strings transfer one owner. Every retained view
   owns its backing allocation, even when data points into its middle. */
#define LILSCRIPT_NATIVE_STRING_ABI_VERSION 2
typedef struct ls_native_object ls_native_object;
typedef void (*ls_native_visit)(void *, void *);
void ls_native_retain(void *);
void ls_native_release(void *);
typedef struct { const uint16_t *data; size_t length; ls_native_object *owner; } ls_string;
static LS_NATIVE_UNUSED inline ls_string ls_string_hold(ls_string value) { ls_native_retain(value.owner); return value; }
static LS_NATIVE_UNUSED inline void ls_string_release(ls_string value) { ls_native_release(value.owner); }
static LS_NATIVE_UNUSED inline void ls_string_copy(ls_string *slot, ls_string value) { ls_native_retain(value.owner); ls_native_release(slot->owner); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_string_take(ls_string *slot, ls_string value) { ls_native_release(slot->owner); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_string_clear(ls_string *slot) { ls_native_release(slot->owner); *slot = (ls_string){0}; }
/* Copy host storage into one owned string; input may be released on return. */
ls_string ls_string_from_utf16(const uint16_t *data, size_t length);
#define LS_NATIVE_CYCLE_THRESHOLD UINT32_C(1)
static LS_NATIVE_UNUSED inline int32_t ls_from_u32(uint32_t value) {
    return value <= INT32_MAX ? (int32_t)value : (int32_t)((int64_t)value - INT64_C(4294967296));
}
/* Synchronous trial deletion over the subgraph reachable from retained-cycle
   candidates. Real owner counts stay intact until unreachable objects have all
   been identified. Tracing, marking and destruction use intrusive work lists,
   so graph depth does not consume the C stack. No user code runs in tracing or
   destruction; calls/ownership remain confined to the originating thread. */
#include <stdlib.h>
typedef struct ls_native_object ls_native_object;
typedef void (*ls_native_visit)(void *, void *);
struct ls_native_object {
    size_t references, trial;
    void (*destroy)(ls_native_object *);
    void (*trace)(ls_native_object *, ls_native_visit, void *);
    ls_native_object *candidate_next, *candidate_previous;
    ls_native_object *work_next, *scan_next;
    unsigned char color;
    bool buffered;
};
static uint64_t ls_native_identity_counter;
static size_t ls_native_live_objects, ls_native_since_collection;
static ls_native_object *ls_native_candidates, *ls_native_pending;
static bool ls_native_collecting, ls_native_destroying;
#ifndef LS_NATIVE_CYCLE_THRESHOLD
#define LS_NATIVE_CYCLE_THRESHOLD 4096
#endif

static LS_NATIVE_UNUSED inline void ls_native_resource_failure(void) {
    fputs("LilScript native runtime resource exhaustion\n", stderr);
    abort();
}
static LS_NATIVE_UNUSED inline void ls_native_unbuffer(ls_native_object *object) {
    if (!object->buffered) return;
    if (object->candidate_previous) object->candidate_previous->candidate_next = object->candidate_next;
    else ls_native_candidates = object->candidate_next;
    if (object->candidate_next) object->candidate_next->candidate_previous = object->candidate_previous;
    object->candidate_next = object->candidate_previous = NULL;
    object->buffered = false;
}
static LS_NATIVE_UNUSED inline void ls_native_buffer(ls_native_object *object) {
    if (object->buffered || !object->trace || ls_native_collecting) return;
    object->candidate_next = ls_native_candidates;
    if (ls_native_candidates) ls_native_candidates->candidate_previous = object;
    ls_native_candidates = object;
    object->buffered = true;
}
void ls_native_retain(void *handle) {
    ls_native_object *object = handle;
    if (!object) return;
    if (object->references == SIZE_MAX) ls_native_resource_failure();
    ++object->references;
}
void ls_native_release(void *handle) {
    ls_native_object *object = handle;
    if (!object || (ls_native_collecting && object->color == 3)) return;
    if (!object->references) ls_native_resource_failure();
    if (--object->references) { ls_native_buffer(object); return; }
    ls_native_unbuffer(object);
    object->work_next = ls_native_pending;
    ls_native_pending = object;
    if (ls_native_destroying) return;
    ls_native_destroying = true;
    while (ls_native_pending) {
        object = ls_native_pending;
        ls_native_pending = object->work_next;
        if (object->destroy) object->destroy(object);
        --ls_native_live_objects;
        free(object);
    }
    ls_native_destroying = false;
}
typedef struct { ls_native_object *work, *touched; } ls_native_trial;
static LS_NATIVE_UNUSED inline void ls_native_gray(ls_native_object *object, ls_native_trial *trial) {
    if (object->color) return;
    object->color = 1;
    object->trial = object->references;
    object->scan_next = trial->touched;
    trial->touched = object;
    object->work_next = trial->work;
    trial->work = object;
}
static LS_NATIVE_UNUSED inline void ls_native_subtract(void *handle, void *context) {
    ls_native_object *object = handle;
    if (!object) return;
    ls_native_gray(object, context);
    if (!object->trial) ls_native_resource_failure(); /* invalid trace/owner count */
    --object->trial;
}
static LS_NATIVE_UNUSED inline void ls_native_black(void *handle, void *context) {
    ls_native_object *object = handle;
    ls_native_trial *trial = context;
    if (!object || object->color == 2) return;
    object->color = 2;
    object->work_next = trial->work;
    trial->work = object;
}
void ls_native_collect_cycles(void) {
    if (ls_native_collecting || ls_native_destroying) return;
    ls_native_collecting = true;
    ls_native_trial trial = {0};
    for (ls_native_object *root = ls_native_candidates; root; root = root->candidate_next)
        ls_native_gray(root, &trial);
    while (trial.work) {
        ls_native_object *object = trial.work;
        trial.work = object->work_next;
        if (object->trace) object->trace(object, ls_native_subtract, &trial);
    }
    /* Any surviving external owner roots its entire reachable graph. */
    for (ls_native_object *object = trial.touched; object; object = object->scan_next)
        if (object->trial) ls_native_black(object, &trial);
    while (trial.work) {
        ls_native_object *object = trial.work;
        trial.work = object->work_next;
        if (object->trace) object->trace(object, ls_native_black, &trial);
    }
    while (ls_native_candidates) ls_native_unbuffer(ls_native_candidates);
    ls_native_object *dead = NULL;
    for (ls_native_object *object = trial.touched; object; ) {
        ls_native_object *next = object->scan_next;
        if (object->color == 1) {
            object->color = 3;
            object->scan_next = dead;
            dead = object;
        } else {
            object->color = 0;
            object->scan_next = object->work_next = NULL;
        }
        object = next;
    }
    /* All white objects remain allocated while their outgoing edges are
       dropped. A white-to-white release is a no-op, avoiding dangling reads. */
    for (ls_native_object *object = dead; object; object = object->scan_next)
        if (object->destroy) object->destroy(object);
    while (dead) {
        ls_native_object *next = dead->scan_next;
        --ls_native_live_objects;
        free(dead);
        dead = next;
    }
    ls_native_since_collection = 0;
    ls_native_collecting = false;
}
static LS_NATIVE_UNUSED inline void *ls_native_allocate(size_t bytes,
                               void (*destroy)(ls_native_object *),
                               void (*trace)(ls_native_object *, ls_native_visit, void *)) {
    if (bytes < sizeof(ls_native_object)) ls_native_resource_failure();
    /* Increasing the interval with live size bounds collection pressure when
       applications retain a large graph. Zero selects explicit/shutdown only. */
    size_t interval = LS_NATIVE_CYCLE_THRESHOLD;
    if (interval && interval < ls_native_live_objects / 2) interval = ls_native_live_objects / 2;
    if (interval && ls_native_since_collection >= interval && ls_native_candidates)
        ls_native_collect_cycles();
    ls_native_object *object = calloc(1, bytes);
    if (!object) ls_native_resource_failure();
    object->references = 1;
    object->destroy = destroy;
    object->trace = trace;
    if (ls_native_live_objects == SIZE_MAX) ls_native_resource_failure();
    ++ls_native_live_objects;
    if (ls_native_since_collection != SIZE_MAX) ++ls_native_since_collection;
    return object;
}
static LS_NATIVE_UNUSED inline uint64_t ls_native_fresh_identity(void) {
    if (ls_native_identity_counter == UINT64_MAX) ls_native_resource_failure();
    return ++ls_native_identity_counter;
}
static LS_NATIVE_UNUSED inline int32_t ls_string_length(ls_string value) {
    return ls_from_u32((uint32_t)value.length);
}
static LS_NATIVE_UNUSED inline bool ls_string_equal(ls_string left, ls_string right) {
    return left.length == right.length &&
           (left.length == 0 || memcmp(left.data, right.data, left.length * sizeof *left.data) == 0);
}
#include <stdlib.h>
typedef struct { ls_native_object owner; uint16_t units[]; } ls_string_block;
static LS_NATIVE_UNUSED inline void ls_string_failure(const char *message) {
    fputs(message, stderr); fputc('\n', stderr); abort();
}
static LS_NATIVE_UNUSED inline ls_string ls_string_allocate(size_t length) {
    if (length > (size_t)INT32_MAX || length > (SIZE_MAX - sizeof(ls_string_block)) / sizeof(uint16_t))
        ls_string_failure("LilScript native string length exceeds the runtime limit");
    if (!length) return (ls_string){0};
    ls_string_block *block = ls_native_allocate(sizeof *block + length * sizeof(uint16_t), NULL, NULL);
    return (ls_string){block->units, length, &block->owner};
}
ls_string ls_string_from_utf16(const uint16_t *data, size_t length) {
    ls_string result = ls_string_allocate(length);
    if (length) {
        if (!data) ls_string_failure("LilScript native string input is null");
        memcpy((uint16_t *)result.data, data, length * sizeof *data);
    }
    return result;
}
static LS_NATIVE_UNUSED inline ls_string ls_string_ascii(const char *text, size_t length) {
    if (!length) return (ls_string){0};
    ls_string result = ls_string_allocate(length);
    uint16_t *units = (uint16_t *)result.data;
    for (size_t index = 0; index < length; index++) units[index] = (unsigned char)text[index];
    return result;
}
static LS_NATIVE_UNUSED inline ls_string ls_string_concat(ls_string left, ls_string right) {
    if (!left.length) return ls_string_hold(right);
    if (!right.length) return ls_string_hold(left);
    if (right.length > (size_t)INT32_MAX - left.length)
        ls_string_failure("LilScript native string length exceeds the runtime limit");
    ls_string result = ls_string_allocate(left.length + right.length);
    uint16_t *units = (uint16_t *)result.data;
    memcpy(units, left.data, left.length * sizeof *units);
    memcpy(units + left.length, right.data, right.length * sizeof *units);
    return result;
}
/* Own each part before allocation; release all temporary conversions after
   one checked allocation and copy. This also avoids quadratic template joins. */
static LS_NATIVE_UNUSED inline ls_string ls_string_join_owned(size_t count, ls_string *parts) {
    size_t length = 0;
    for (size_t i = 0; i < count; ++i) {
        if (parts[i].length > (size_t)INT32_MAX - length) ls_string_failure("LilScript native string length exceeds the runtime limit");
        length += parts[i].length;
    }
    ls_string result = ls_string_allocate(length);
    uint16_t *out = (uint16_t *)result.data;
    size_t at = 0;
    for (size_t i = 0; i < count; ++i) {
        if (parts[i].length) memcpy(out + at, parts[i].data, parts[i].length * sizeof *out);
        at += parts[i].length;
        ls_string_release(parts[i]);
    }
    return result;
}
static LS_NATIVE_UNUSED inline int ls_string_compare(ls_string left, ls_string right) {
    size_t shared = left.length < right.length ? left.length : right.length;
    for (size_t index = 0; index < shared; index++) {
        if (left.data[index] != right.data[index]) return left.data[index] < right.data[index] ? -1 : 1;
    }
    return left.length == right.length ? 0 : left.length < right.length ? -1 : 1;
}
static const uint16_t ls_true_units[] = {116, 114, 117, 101};
static const uint16_t ls_false_units[] = {102, 97, 108, 115, 101};
static LS_NATIVE_UNUSED inline ls_string ls_bool_to_string(bool value) {
    return value ? (ls_string){ls_true_units, 4, NULL} : (ls_string){ls_false_units, 5, NULL};
}
static LS_NATIVE_UNUSED inline ls_string ls_uint_to_radix(uint32_t magnitude, bool negative, int32_t radix) {
    if (radix < 2 || radix > 36) ls_string_failure("LilScript native toString radix must be between 2 and 36");
    char digits[40];
    size_t length = 0;
    do {
        digits[length++] = "0123456789abcdefghijklmnopqrstuvwxyz"[magnitude % (uint32_t)radix];
        magnitude /= (uint32_t)radix;
    } while (magnitude);
    if (negative) digits[length++] = '-';
    char text[40];
    for (size_t index = 0; index < length; index++) text[index] = digits[length - 1 - index];
    return ls_string_ascii(text, length);
}
static LS_NATIVE_UNUSED inline ls_string ls_int_to_radix(int32_t value, int32_t radix) {
    uint32_t magnitude = value < 0 ? UINT32_C(0) - (uint32_t)value : (uint32_t)value;
    return ls_uint_to_radix(magnitude, value < 0, radix);
}
static LS_NATIVE_UNUSED inline ls_string ls_int_to_string(int32_t value) {
    return ls_int_to_radix(value, 10);
}
/* The k significant digits (no trailing zeros) and exponent n of the
   shortest decimal 0.d1...dk x 10^n that reads back as `value` (> 0,
   finite). printf rounds correctly, so each precision's candidate is the
   nearest decimal of that length; below a power of two the rounding
   interval is narrower, so the next decimal up may be the one that reads
   back when the nearest does not. */
static LS_NATIVE_UNUSED inline void ls_shortest_decimal(double value, char *digits, int *count, int *exponent) {
    uint64_t bits;
    memcpy(&bits, &value, sizeof bits);
    bool asymmetric = (bits & UINT64_C(0x000fffffffffffff)) == 0 && (bits >> 52) > 1;
    char text[40];
    for (int precision = 0; precision <= 16; precision++) {
        snprintf(text, sizeof text, "%.*e", precision, value);
        bool found = strtod(text, NULL) == value;
        if (!found && asymmetric) {
            char *mark = strchr(text, 'e');
            char *last = mark - 1;
            while (last >= text && (*last == '.' || *last == '9')) {
                if (*last == '9') *last = '0';
                last--;
            }
            if (last >= text) {
                (*last)++;
            } else {
                /* 9.99e+N carried to 10.0e+N: respell it 1.00e+(N+1). */
                int power = atoi(mark + 1) + 1;
                text[0] = '1';
                snprintf(mark, sizeof text - (size_t)(mark - text), "e%+d", power);
            }
            found = strtod(text, NULL) == value;
        }
        if (!found) continue;
        int length = 0;
        char *cursor = text;
        for (; *cursor && *cursor != 'e'; cursor++) {
            if (*cursor >= '0' && *cursor <= '9') digits[length++] = *cursor;
        }
        while (length > 1 && digits[length - 1] == '0') length--;
        *count = length;
        *exponent = atoi(cursor + 1) + 1;
        return;
    }
    ls_string_failure("LilScript native number formatting failed");
}
static LS_NATIVE_UNUSED inline ls_string ls_number_to_string(double value) {
    if (value != value) return ls_string_ascii("NaN", 3);
    if (value == 0) return ls_string_ascii("0", 1);
    char text[64];
    size_t length = 0;
    if (value < 0) {
        text[length++] = '-';
        value = -value;
    }
    if (isinf(value)) {
        memcpy(text + length, "Infinity", 8);
        return ls_string_ascii(text, length + 8);
    }
    char digits[24];
    int k = 0, n = 0;
    ls_shortest_decimal(value, digits, &k, &n);
    if (k <= n && n <= 21) {
        memcpy(text + length, digits, (size_t)k);
        length += (size_t)k;
        for (int index = k; index < n; index++) text[length++] = '0';
    } else if (0 < n && n <= 21) {
        memcpy(text + length, digits, (size_t)n);
        length += (size_t)n;
        text[length++] = '.';
        memcpy(text + length, digits + n, (size_t)(k - n));
        length += (size_t)(k - n);
    } else if (-6 < n && n <= 0) {
        text[length++] = '0';
        text[length++] = '.';
        for (int index = 0; index < -n; index++) text[length++] = '0';
        memcpy(text + length, digits, (size_t)k);
        length += (size_t)k;
    } else {
        text[length++] = digits[0];
        if (k > 1) {
            text[length++] = '.';
            memcpy(text + length, digits + 1, (size_t)(k - 1));
            length += (size_t)(k - 1);
        }
        length += (size_t)snprintf(text + length, sizeof text - length, "e%c%d", n - 1 < 0 ? '-' : '+', n - 1 < 0 ? 1 - n : n - 1);
    }
    return ls_string_ascii(text, length);
}
static LS_NATIVE_UNUSED inline void ls_write_string(ls_string value) {
    for (size_t index = 0; index < value.length; index++) {
        uint32_t point = value.data[index];
        if (point >= 0xD800 && point <= 0xDBFF && index + 1 < value.length &&
            value.data[index + 1] >= 0xDC00 && value.data[index + 1] <= 0xDFFF) {
            point = 0x10000 + ((point - 0xD800) << 10) + (uint32_t)(value.data[index + 1] - 0xDC00);
            index++;
        } else if (point >= 0xD800 && point <= 0xDFFF) {
            point = 0xFFFD;
        }
        if (point < 0x80) {
            putchar((int)point);
        } else if (point < 0x800) {
            putchar((int)(0xC0 | point >> 6));
            putchar((int)(0x80 | (point & 0x3F)));
        } else if (point < 0x10000) {
            putchar((int)(0xE0 | point >> 12));
            putchar((int)(0x80 | ((point >> 6) & 0x3F)));
            putchar((int)(0x80 | (point & 0x3F)));
        } else {
            putchar((int)(0xF0 | point >> 18));
            putchar((int)(0x80 | ((point >> 12) & 0x3F)));
            putchar((int)(0x80 | ((point >> 6) & 0x3F)));
            putchar((int)(0x80 | (point & 0x3F)));
        }
    }
}
static LS_NATIVE_UNUSED inline void ls_print_string(ls_string value) {
    ls_write_string(value);
    putchar('\n');
}
static LS_NATIVE_UNUSED inline void ls_print_number(double value) {
    if (value == 0 && signbit(value)) {
        puts("-0");
        return;
    }
    ls_string text = ls_number_to_string(value);
    ls_print_string(text);
    ls_string_release(text);
}
static LS_NATIVE_UNUSED inline size_t ls_string_clamp(int32_t index, size_t length) {
    return index < 0 ? 0 : (size_t)index > length ? length : (size_t)index;
}
static LS_NATIVE_UNUSED inline size_t ls_string_relative(int32_t index, size_t length) {
    if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
    return (size_t)index < length ? (size_t)index : length;
}
static LS_NATIVE_UNUSED inline bool ls_string_matches(ls_string text, size_t at, ls_string search) {
    return at + search.length <= text.length &&
           (search.length == 0 || memcmp(text.data + at, search.data, search.length * sizeof *search.data) == 0);
}
static LS_NATIVE_UNUSED inline int32_t ls_string_index_of(ls_string text, ls_string search, int32_t position) {
    for (size_t at = ls_string_clamp(position, text.length); at + search.length <= text.length; at++) {
        if (ls_string_matches(text, at, search)) return (int32_t)at;
    }
    return -1;
}
static LS_NATIVE_UNUSED inline int32_t ls_string_last_index_of(ls_string text, ls_string search, int32_t position) {
    if (search.length > text.length) return -1;
    size_t at = ls_string_clamp(position, text.length);
    if (at > text.length - search.length) at = text.length - search.length;
    for (;; at--) {
        if (ls_string_matches(text, at, search)) return (int32_t)at;
        if (at == 0) return -1;
    }
}
static LS_NATIVE_UNUSED inline bool ls_string_starts_with(ls_string text, ls_string search) {
    return ls_string_matches(text, 0, search);
}
static LS_NATIVE_UNUSED inline bool ls_string_ends_with(ls_string text, ls_string search) {
    return search.length <= text.length && ls_string_matches(text, text.length - search.length, search);
}
static LS_NATIVE_UNUSED inline ls_string ls_string_view(ls_string text, size_t start, size_t end) {
    if (end <= start) return (ls_string){0};
    ls_native_retain(text.owner);
    return (ls_string){text.data + start, end - start, text.owner};
}
static LS_NATIVE_UNUSED inline ls_string ls_string_slice(ls_string text, int32_t start, bool bounded, int32_t end) {
    size_t from = ls_string_relative(start, text.length);
    size_t to = bounded ? ls_string_relative(end, text.length) : text.length;
    return ls_string_view(text, from, to);
}
static LS_NATIVE_UNUSED inline bool ls_string_space(uint16_t unit) {
    return (unit >= 9 && unit <= 13) || unit == 32 || unit == 0xA0 || unit == 0x1680 ||
           (unit >= 0x2000 && unit <= 0x200A) || unit == 0x2028 || unit == 0x2029 ||
           unit == 0x202F || unit == 0x205F || unit == 0x3000 || unit == 0xFEFF;
}
static LS_NATIVE_UNUSED inline ls_string ls_string_trim(ls_string text, bool start, bool end) {
    size_t from = 0, to = text.length;
    while (start && from < to && ls_string_space(text.data[from])) from++;
    while (end && to > from && ls_string_space(text.data[to - 1])) to--;
    return ls_string_view(text, from, to);
}
static LS_NATIVE_UNUSED inline ls_string ls_string_repeat(ls_string text, int32_t count) {
    if (count < 0) ls_string_failure("LilScript native repeat count must not be negative");
    if (count == 0 || text.length == 0) return (ls_string){0};
    if (text.length > (size_t)INT32_MAX / (size_t)count)
        ls_string_failure("LilScript native string length exceeds the runtime limit");
    ls_string result = ls_string_allocate(text.length * (size_t)count);
    uint16_t *units = (uint16_t *)result.data;
    for (int32_t index = 0; index < count; index++) memcpy(units + (size_t)index * text.length, text.data, text.length * sizeof *units);
    return result;
}
static LS_NATIVE_UNUSED inline ls_string ls_string_case(ls_string text, bool upper) {
    if (!text.length) return (ls_string){0};
    ls_string result = ls_string_allocate(text.length);
    uint16_t *units = (uint16_t *)result.data;
    for (size_t index = 0; index < text.length; index++) {
        uint16_t unit = text.data[index];
        if (unit >= 0x80) ls_string_failure("LilScript native case mapping supports ASCII text only");
        if (upper && unit >= 'a' && unit <= 'z') unit = (uint16_t)(unit - 32);
        if (!upper && unit >= 'A' && unit <= 'Z') unit = (uint16_t)(unit + 32);
        units[index] = unit;
    }
    return result;
}
static LS_NATIVE_UNUSED inline int32_t ls_string_code_points(ls_string text) {
    int32_t count = 0;
    for (size_t index = 0; index < text.length; index++, count++) {
        if (text.data[index] >= 0xD800 && text.data[index] <= 0xDBFF && index + 1 < text.length &&
            text.data[index + 1] >= 0xDC00 && text.data[index + 1] <= 0xDFFF) index++;
    }
    return count;
}
static LS_NATIVE_UNUSED inline double ls_round(double value) {
    if (!isfinite(value) || value == 0) return value;
    if (value > 0 && value < 0.5) return 0.0;
    if (value < 0 && value >= -0.5) return -0.0;
    double floor_value = floor(value);
    return value - floor_value >= 0.5 ? floor_value + 1.0 : floor_value;
}
static LS_NATIVE_UNUSED inline double ls_min(double left, double right) {
    if (left != left || right != right) return NAN;
    if (left == 0 && right == 0) return signbit(left) ? left : right;
    return left < right ? left : right;
}
static LS_NATIVE_UNUSED inline double ls_max(double left, double right) {
    if (left != left || right != right) return NAN;
    if (left == 0 && right == 0) return signbit(left) ? right : left;
    return left > right ? left : right;
}
/* Shared bounded UTF-16 builder for portable data/text recipes. */
typedef struct { uint16_t *units; size_t length,capacity; } ls_string_builder;
static LS_NATIVE_UNUSED inline void ls_string_builder_unit(ls_string_builder *out, uint16_t unit) {
    if(out->length >= (size_t)INT32_MAX) ls_string_failure("LilScript native builder exceeds the string limit");
    if(out->length==out->capacity) {
        size_t capacity=out->capacity ? out->capacity+out->capacity/2+1 : 64;
        if(capacity>(size_t)INT32_MAX) capacity=INT32_MAX;
        if(capacity>SIZE_MAX/sizeof *out->units) ls_native_resource_failure();
        uint16_t *units=realloc(out->units,capacity*sizeof *units);
        if(!units) ls_native_resource_failure();
        out->units=units; out->capacity=capacity;
    }
    out->units[out->length++]=unit;
}
static LS_NATIVE_UNUSED inline void ls_string_builder_text(ls_string_builder *out, ls_string text) {
    for(size_t i=0;i<text.length;i++) ls_string_builder_unit(out,text.data[i]);
}
static LS_NATIVE_UNUSED inline void ls_string_builder_ascii(ls_string_builder *out, const char *text) {
    while(*text) ls_string_builder_unit(out,(unsigned char)*text++);
}
static LS_NATIVE_UNUSED inline ls_string ls_string_builder_finish(ls_string_builder *out) {
    ls_string result=ls_string_from_utf16(out->units,out->length);
    free(out->units); *out=(ls_string_builder){0}; return result;
}
/* Shared native value layout and ownership operations for C and host headers. */
typedef struct ls_value {
    uint8_t tag;
    /* A callable payload's physical signature. */
    uint32_t signature;
    union {
        int32_t i;
        double f;
        bool b;
        ls_string s;
        ls_native_object *o;
        struct { void (*code)(void); void *environment; uint64_t identity; } c;
    } as;
} ls_value;
enum { LS_NULL, LS_INT, LS_FLOAT, LS_BOOL, LS_STRING, LS_OBJECT, LS_ARRAY, LS_CALLABLE, LS_SYMBOL };
static LS_NATIVE_UNUSED inline void ls_value_retain(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_retain(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_retain(value.as.c.environment);
    else if (value.tag == LS_STRING) ls_native_retain(value.as.s.owner);
}
static LS_NATIVE_UNUSED inline void ls_value_release(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_release(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_release(value.as.c.environment);
    else if (value.tag == LS_STRING) ls_native_release(value.as.s.owner);
}
static LS_NATIVE_UNUSED inline void ls_value_trace(ls_value value, ls_native_visit visit, void *context) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) visit(value.as.o, context);
    else if (value.tag == LS_CALLABLE) visit(value.as.c.environment, context);
    else if (value.tag == LS_STRING) visit(value.as.s.owner, context);
}
static LS_NATIVE_UNUSED inline void ls_value_mismatch(void) {
    fputs("LilScript native value has an unexpected type\n", stderr);
    abort();
}
static LS_NATIVE_UNUSED inline void ls_value_copy(ls_value *slot, ls_value value) { ls_value_retain(value); ls_value_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_value_take(ls_value *slot, ls_value value) { ls_value_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_value_clear(ls_value *slot) { ls_value_release(*slot); *slot = (ls_value){0}; }
static LS_NATIVE_UNUSED inline ls_value ls_value_int(int32_t value) { ls_value result = {.tag = LS_INT}; result.as.i = value; return result; }
static LS_NATIVE_UNUSED inline ls_value ls_value_float(double value) { ls_value result = {.tag = LS_FLOAT}; result.as.f = value; return result; }
static LS_NATIVE_UNUSED inline ls_value ls_value_bool(bool value) { ls_value result = {.tag = LS_BOOL}; result.as.b = value; return result; }
static LS_NATIVE_UNUSED inline ls_value ls_value_string(ls_string value) { ls_value result = {.tag = LS_STRING}; result.as.s = value; return result; }
static LS_NATIVE_UNUSED inline ls_value ls_value_object(ls_native_object *value) { ls_value result = {.tag = LS_OBJECT}; result.as.o = value; return result; }
static LS_NATIVE_UNUSED inline ls_value ls_value_array(ls_native_object *value) { ls_value result = {.tag = LS_ARRAY}; result.as.o = value; return result; }
static LS_NATIVE_UNUSED inline ls_value ls_value_symbol(ls_native_object *value) { ls_value result = {.tag = LS_SYMBOL}; result.as.o = value; return result; }
static LS_NATIVE_UNUSED inline int32_t ls_value_to_int(ls_value value) { if (value.tag != LS_INT) ls_value_mismatch(); return value.as.i; }
static LS_NATIVE_UNUSED inline double ls_value_to_number(ls_value value) {
    if (value.tag == LS_INT) return (double)value.as.i;
    if (value.tag != LS_FLOAT) ls_value_mismatch();
    return value.as.f;
}
static LS_NATIVE_UNUSED inline bool ls_value_to_bool(ls_value value) { if (value.tag != LS_BOOL) ls_value_mismatch(); return value.as.b; }
static LS_NATIVE_UNUSED inline ls_string ls_value_to_string(ls_value value) { if (value.tag != LS_STRING) ls_value_mismatch(); return value.as.s; }
/* A reference slot of a class instance holds null until `init` stores it,
   as in JavaScript: null unboxes to the empty slot there. */
static LS_NATIVE_UNUSED inline ls_native_object *ls_value_to_object(ls_value value) {
    if (value.tag == LS_NULL) return NULL;
    if (value.tag != LS_OBJECT) ls_value_mismatch();
    return value.as.o;
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_value_to_array(ls_value value) {
    if (value.tag == LS_NULL) return NULL;
    if (value.tag != LS_ARRAY) ls_value_mismatch();
    return value.as.o;
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_value_to_symbol(ls_value value) { if (value.tag != LS_SYMBOL) ls_value_mismatch(); return value.as.o; }
static LS_NATIVE_UNUSED inline bool ls_value_number(ls_value value) { return value.tag == LS_INT || value.tag == LS_FLOAT; }
/* JavaScript strict equality: numbers by value, strings by code units,
   everything else by identity. */
static LS_NATIVE_UNUSED inline bool ls_value_equal(ls_value left, ls_value right) {
    if (ls_value_number(left) && ls_value_number(right)) return ls_value_to_number(left) == ls_value_to_number(right);
    if (left.tag != right.tag) return false;
    switch (left.tag) {
    case LS_NULL: return true;
    case LS_BOOL: return left.as.b == right.as.b;
    case LS_STRING: return ls_string_equal(left.as.s, right.as.s);
    case LS_OBJECT: case LS_ARRAY: case LS_SYMBOL: return left.as.o == right.as.o;
    case LS_CALLABLE: return left.as.c.identity == right.as.c.identity;
    default: return false;
    }
}
/* SameValueZero, as `includes` compares: NaN finds NaN. */
static LS_NATIVE_UNUSED inline bool ls_value_same_zero(ls_value left, ls_value right) {
    if (ls_value_number(left) && ls_value_number(right)) {
        double x = ls_value_to_number(left), y = ls_value_to_number(right);
        return x == y || (x != x && y != y);
    }
    return ls_value_equal(left, right);
}
static LS_NATIVE_UNUSED inline void ls_print_value(ls_value value) {
    switch (value.tag) {
    case LS_NULL: puts("null"); break;
    case LS_INT: printf("%ld\n", (long)value.as.i); break;
    case LS_FLOAT: ls_print_number(value.as.f); break;
    case LS_BOOL: puts(value.as.b ? "true" : "false"); break;
    case LS_STRING: ls_print_string(value.as.s); break;
    default:
        fputs("LilScript native cannot print this value\n", stderr);
        abort();
    }
}

/* Array.join accepts checked scalar members. Absence contributes an empty
   element; numeric and boolean conversion is shared with ordinary strings. */
static LS_NATIVE_UNUSED inline void ls_string_builder_value(ls_string_builder *out, ls_value value) {
    switch(value.tag) {
    case LS_NULL: break;
    case LS_STRING: ls_string_builder_text(out,value.as.s); break;
    case LS_BOOL: ls_string_builder_ascii(out,value.as.b ? "true" : "false"); break;
    case LS_INT: case LS_FLOAT: {
        ls_string text=ls_number_to_string(ls_value_to_number(value));
        ls_string_builder_text(out,text); ls_string_release(text); break;
    }
    default: ls_value_mismatch();
    }
}
typedef struct { ls_value key; ls_value value; bool live; } ls_map_entry;
typedef struct ls_map {
    ls_native_object owner;
    ls_map_entry *entries;
    size_t used, capacity, size;
    /* Entry position + 1, or 0 for an empty index slot. */
    size_t *index;
    size_t slots;
} ls_map;
static LS_NATIVE_UNUSED inline void ls_map_destroy(ls_native_object *owner) {
    ls_map *map = (ls_map *)owner;
    for (size_t position = 0; position < map->used; position++) {
        if (map->entries[position].live) {
            ls_value_release(map->entries[position].key);
            ls_value_release(map->entries[position].value);
        }
    }
    free(map->entries);
    free(map->index);
}
static LS_NATIVE_UNUSED inline void ls_map_trace(ls_native_object *owner, ls_native_visit visit, void *context) {
    ls_map *map = (ls_map *)owner;
    for (size_t i = 0; i < map->used; ++i) if (map->entries[i].live) {
        ls_value_trace(map->entries[i].key, visit, context);
        ls_value_trace(map->entries[i].value, visit, context);
    }
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_map_new(void) {
    ls_map *map = ls_native_allocate(sizeof *map, ls_map_destroy, ls_map_trace);
    map->entries = NULL;
    map->used = map->capacity = map->size = 0;
    map->index = NULL;
    map->slots = 0;
    return &map->owner;
}
static LS_NATIVE_UNUSED inline uint64_t ls_map_mix(uint64_t bits) {
    bits ^= bits >> 33;
    bits *= UINT64_C(0xff51afd7ed558ccd);
    bits ^= bits >> 33;
    return bits;
}
static LS_NATIVE_UNUSED inline uint64_t ls_map_hash(ls_value key) {
    switch (key.tag) {
    case LS_INT: case LS_FLOAT: {
        double number = ls_value_to_number(key);
        if (number != number) return UINT64_C(0x7ff8000000000000);
        if (number == 0) number = 0;
        uint64_t bits;
        memcpy(&bits, &number, sizeof bits);
        return ls_map_mix(bits);
    }
    case LS_STRING: {
        uint64_t hash = UINT64_C(0xcbf29ce484222325);
        for (size_t index = 0; index < key.as.s.length; index++) {
            hash ^= key.as.s.data[index];
            hash *= UINT64_C(0x100000001b3);
        }
        return hash;
    }
    case LS_BOOL: return key.as.b ? 2 : 1;
    case LS_CALLABLE: return ls_map_mix(key.as.c.identity);
    case LS_OBJECT: case LS_ARRAY: case LS_SYMBOL: return ls_map_mix((uint64_t)(uintptr_t)key.as.o);
    default: return 0;
    }
}
static LS_NATIVE_UNUSED inline size_t ls_map_find(ls_map *map, ls_value key) {
    if (!map->slots) return SIZE_MAX;
    size_t mask = map->slots - 1;
    for (size_t slot = (size_t)ls_map_hash(key) & mask;; slot = (slot + 1) & mask) {
        size_t entry = map->index[slot];
        if (!entry) return SIZE_MAX;
        if (map->entries[entry - 1].live && ls_value_same_zero(map->entries[entry - 1].key, key)) return entry - 1;
    }
}
/* Compacts removed entries and rebuilds the index with room to grow. */
static LS_NATIVE_UNUSED inline void ls_map_rebuild(ls_map *map, size_t needed) {
    size_t kept = 0;
    for (size_t position = 0; position < map->used; position++) {
        if (map->entries[position].live) map->entries[kept++] = map->entries[position];
    }
    map->used = kept;
    size_t slots = 8;
    while (slots < 2 * needed) {
        if (slots > SIZE_MAX / 4) ls_native_resource_failure();
        slots *= 2;
    }
    free(map->index);
    map->index = calloc(slots, sizeof *map->index);
    if (!map->index) ls_native_resource_failure();
    map->slots = slots;
    for (size_t position = 0; position < map->used; position++) {
        size_t slot = (size_t)ls_map_hash(map->entries[position].key) & (slots - 1);
        while (map->index[slot]) slot = (slot + 1) & (slots - 1);
        map->index[slot] = position + 1;
    }
}
static LS_NATIVE_UNUSED inline int32_t ls_map_size(ls_native_object *owner) {
    return (int32_t)((ls_map *)owner)->size;
}
static LS_NATIVE_UNUSED inline ls_value ls_map_get(ls_native_object *owner, ls_value key) {
    ls_map *map = (ls_map *)owner;
    size_t position = ls_map_find(map, key);
    return position == SIZE_MAX ? (ls_value){0} : map->entries[position].value;
}
static LS_NATIVE_UNUSED inline bool ls_map_has(ls_native_object *owner, ls_value key) {
    return ls_map_find((ls_map *)owner, key) != SIZE_MAX;
}
static LS_NATIVE_UNUSED inline void ls_map_set(ls_native_object *owner, ls_value key, ls_value value) {
    ls_map *map = (ls_map *)owner;
    size_t position = ls_map_find(map, key);
    ls_value_retain(value);
    if (position != SIZE_MAX) {
        ls_value_release(map->entries[position].value);
        map->entries[position].value = value;
        return;
    }
    /* A new key: -0 is stored as +0, as JavaScript normalizes it. */
    if (key.tag == LS_FLOAT && key.as.f == 0) key.as.f = 0;
    if (map->size >= (size_t)INT32_MAX) ls_native_resource_failure();
    if (map->used == map->capacity) {
        if (map->size < map->used / 2) {
            ls_map_rebuild(map, map->size + 1);
        } else {
            size_t capacity = map->capacity ? map->capacity * 2 : 8;
            if (capacity > SIZE_MAX / sizeof *map->entries) ls_native_resource_failure();
            ls_map_entry *entries = realloc(map->entries, capacity * sizeof *entries);
            if (!entries) ls_native_resource_failure();
            map->entries = entries;
            map->capacity = capacity;
        }
    }
    if (2 * (map->used + 1) > map->slots) ls_map_rebuild(map, map->used + 1);
    ls_value_retain(key);
    map->entries[map->used] = (ls_map_entry){key, value, true};
    size_t slot = (size_t)ls_map_hash(key) & (map->slots - 1);
    while (map->index[slot]) slot = (slot + 1) & (map->slots - 1);
    map->index[slot] = ++map->used;
    map->size++;
}
static LS_NATIVE_UNUSED inline bool ls_map_delete(ls_native_object *owner, ls_value key) {
    ls_map *map = (ls_map *)owner;
    size_t position = ls_map_find(map, key);
    if (position == SIZE_MAX) return false;
    map->entries[position].live = false;
    ls_value_release(map->entries[position].key);
    ls_value_release(map->entries[position].value);
    map->size--;
    return true;
}
static LS_NATIVE_UNUSED inline void ls_map_clear(ls_native_object *owner) {
    ls_map *map = (ls_map *)owner;
    for (size_t position = 0; position < map->used; position++) {
        if (map->entries[position].live) {
            map->entries[position].live = false;
            ls_value_release(map->entries[position].key);
            ls_value_release(map->entries[position].value);
        }
    }
    map->size = 0;
    ls_map_rebuild(map, 0);
}
typedef struct { ls_native_object owner; } ls_symbol;
static LS_NATIVE_UNUSED inline ls_native_object *ls_symbol_new(void) {
    ls_symbol *symbol = ls_native_allocate(sizeof *symbol, NULL, NULL);
    return &symbol->owner;
}
static LS_NATIVE_UNUSED inline void ls_record_destroy(ls_native_object *owner) { ls_map_destroy(owner); }
static LS_NATIVE_UNUSED inline ls_native_object *ls_record_new(void) {
    return ls_native_allocate(sizeof(ls_map),ls_record_destroy,ls_map_trace);
}
static LS_NATIVE_UNUSED inline void ls_shape_set(ls_native_object *owner, ls_string key, ls_value value, bool omit_absent) {
    if (omit_absent && value.tag == LS_NULL) ls_map_delete(owner,ls_value_string(key));
    else ls_map_set(owner,ls_value_string(key),value);
}
static LS_NATIVE_UNUSED inline bool ls_shape_tag(ls_value value, ls_string key, ls_value tag) {
    return value.tag == LS_OBJECT && value.as.o && value.as.o->destroy == ls_record_destroy
        && ls_value_equal(ls_map_get(value.as.o,ls_value_string(key)),tag);
}
/* A portable Record owns string keys and tagged values in the shared map.
   Enumeration follows ECMAScript own-key order: canonical array indices first
   in numeric order, then other strings in insertion order. No prototype keys. */
typedef struct { size_t position; uint32_t index; bool numeric; } ls_record_key;
static LS_NATIVE_UNUSED inline bool ls_record_index(ls_string key, uint32_t *result) {
    if (!key.length || key.length > 10 || (key.length > 1 && key.data[0] == '0')) return false;
    uint64_t value=0;
    for (size_t i=0;i<key.length;i++) {
        if (key.data[i] < '0' || key.data[i] > '9') return false;
        value=value*10+(uint32_t)(key.data[i]-'0');
    }
    if (value >= UINT32_MAX) return false;
    *result=(uint32_t)value; return true;
}
static LS_NATIVE_UNUSED int ls_record_key_compare(const void *left, const void *right) {
    const ls_record_key *a=left,*b=right;
    if (a->numeric != b->numeric) return a->numeric ? -1 : 1;
    if (a->numeric) return a->index < b->index ? -1 : a->index > b->index;
    return a->position < b->position ? -1 : a->position > b->position;
}
static LS_NATIVE_UNUSED inline ls_record_key *ls_record_order(ls_native_object *owner) {
    ls_map *record=(ls_map *)owner;
    if (!record->size) return NULL;
    if (record->size > SIZE_MAX/sizeof(ls_record_key)) ls_native_resource_failure();
    ls_record_key *keys=malloc(record->size*sizeof *keys);
    if (!keys) ls_native_resource_failure();
    size_t count=0;
    for (size_t i=0;i<record->used;i++) if(record->entries[i].live) {
        keys[count]=(ls_record_key){.position=i};
        keys[count].numeric=ls_record_index(record->entries[i].key.as.s,&keys[count].index);
        ++count;
    }
    qsort(keys,count,sizeof *keys,ls_record_key_compare);
    return keys;
}
static LS_NATIVE_UNUSED inline ls_value ls_record_get(ls_native_object *record, ls_string key) {
    return ls_map_get(record,ls_value_string(key));
}
static LS_NATIVE_UNUSED inline void ls_record_set(ls_native_object *record, ls_string key, ls_value value) {
    ls_map_set(record,ls_value_string(key),value);
}
static LS_NATIVE_UNUSED inline bool ls_record_has(ls_native_object *record, ls_string key) {
    return ls_map_has(record,ls_value_string(key));
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_record_assign(ls_native_object *target, ls_native_object *source) {
    if (target==source) return target;
    ls_map *record=(ls_map *)source;
    ls_record_key *order=ls_record_order(source);
    for(size_t i=0;i<record->size;i++) {
        ls_map_entry *entry=&record->entries[order[i].position];
        ls_map_set(target,entry->key,entry->value);
    }
    free(order); return target;
}
/* Checked portable JSON values: exact primitive spelling, UTF-16 quoting and
   record key order. Typed generated array recipes share this builder. */
static LS_NATIVE_UNUSED inline void ls_json_quote(ls_string_builder *out, ls_string text) {
    static const char hex[]="0123456789abcdef";
    ls_string_builder_unit(out,'"');
    for(size_t i=0;i<text.length;i++) {
        uint16_t c=text.data[i];
        if(c=='"' || c=='\\') { ls_string_builder_unit(out,'\\'); ls_string_builder_unit(out,c); }
        else if(c==8 || c==9 || c==10 || c==12 || c==13) {
            ls_string_builder_unit(out,'\\'); ls_string_builder_unit(out,c==8?'b':c==9?'t':c==10?'n':c==12?'f':'r');
        } else if(c>=0xD800 && c<=0xDBFF && i+1<text.length && text.data[i+1]>=0xDC00 && text.data[i+1]<=0xDFFF) {
            ls_string_builder_unit(out,c); ls_string_builder_unit(out,text.data[++i]);
        } else if(c<32 || (c>=0xD800 && c<=0xDFFF)) {
            ls_string_builder_ascii(out,"\\u");
            for(int shift=12;shift>=0;shift-=4) ls_string_builder_unit(out,(uint16_t)hex[(c>>shift)&15]);
        } else ls_string_builder_unit(out,c);
    }
    ls_string_builder_unit(out,'"');
}
static LS_NATIVE_UNUSED inline void ls_json_value(ls_string_builder *out, ls_value value) {
    switch(value.tag) {
    case LS_NULL: ls_string_builder_ascii(out,"null"); break;
    case LS_BOOL: ls_string_builder_ascii(out,value.as.b?"true":"false"); break;
    case LS_INT: {
        char text[16]; snprintf(text,sizeof text,"%ld",(long)value.as.i); ls_string_builder_ascii(out,text); break;
    }
    case LS_FLOAT:
        if(!isfinite(value.as.f)) ls_string_builder_ascii(out,"null");
        else { ls_string text=ls_number_to_string(value.as.f); ls_string_builder_text(out,text); ls_string_release(text); }
        break;
    case LS_STRING: ls_json_quote(out,value.as.s); break;
    default: ls_value_mismatch();
    }
}
static LS_NATIVE_UNUSED inline ls_string ls_json_scalar(ls_value value) {
    ls_string_builder out={0}; ls_json_value(&out,value); return ls_string_builder_finish(&out);
}
static LS_NATIVE_UNUSED inline ls_string ls_json_record(ls_native_object *owner) {
    ls_map *record=(ls_map *)owner;
    ls_record_key *order=ls_record_order(owner);
    ls_string_builder out={0}; ls_string_builder_unit(&out,'{');
    for(size_t i=0;i<record->size;i++) {
        ls_map_entry *entry=&record->entries[order[i].position];
        if(i) ls_string_builder_unit(&out,',');
        ls_json_quote(&out,entry->key.as.s); ls_string_builder_unit(&out,':'); ls_json_value(&out,entry->value);
    }
    free(order); ls_string_builder_unit(&out,'}'); return ls_string_builder_finish(&out);
}
typedef struct ls_array0 ls_array0;
typedef struct ls_array1 ls_array1;
typedef struct ls_array2 ls_array2;
typedef struct ls_array3 ls_array3;
typedef struct {
void (*code)(void *,int32_t);
void *environment;
uint64_t identity;
} ls_callable0;
typedef struct {
void (*code)(void *);
void *environment;
uint64_t identity;
} ls_callable1;
#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
void ls_native_collect_cycles(void);
#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void) {
    return ls_native_live_objects;
}
#endif
static LS_NATIVE_UNUSED inline ls_callable0 ls_callable0_retain(ls_callable0 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable0_release(ls_callable0 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable0_copy(ls_callable0 *destination, ls_callable0 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable0_take(ls_callable0 *destination, ls_callable0 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable0_clear(ls_callable0 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable0){0};
}
static LS_NATIVE_UNUSED inline void ls_callable0_call(ls_callable0 value,int32_t ls_p0) {
ls_native_retain(value.environment);
value.code(value.environment,ls_p0);
ls_native_release(value.environment);
}
static LS_NATIVE_UNUSED inline ls_callable1 ls_callable1_retain(ls_callable1 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable1_release(ls_callable1 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable1_copy(ls_callable1 *destination, ls_callable1 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable1_take(ls_callable1 *destination, ls_callable1 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable1_clear(ls_callable1 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable1){0};
}
static LS_NATIVE_UNUSED inline void ls_callable1_call(ls_callable1 value) {
ls_native_retain(value.environment);
value.code(value.environment);
ls_native_release(value.environment);
}
_Static_assert(sizeof(ls_callable0) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable0(ls_callable0 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 0}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable0 ls_value_to_callable0(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable0){0};
if (value.tag != LS_CALLABLE || value.signature != 0) ls_value_mismatch();
ls_callable0 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable1) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable1(ls_callable1 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 1}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable1 ls_value_to_callable1(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable1){0};
if (value.tag != LS_CALLABLE || value.signature != 1) ls_value_mismatch();
ls_callable1 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
static LS_NATIVE_UNUSED void ls_native_undefined_element(void) {
fputs("LilScript native array element is undefined\n", stderr);
abort();
}
static LS_NATIVE_UNUSED size_t ls_array_relative(int32_t index, size_t length) {
if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
return (size_t)index < length ? (size_t)index : length;
}
struct ls_array0 { ls_native_object owner; size_t length; size_t capacity; int32_t *items; };
static LS_NATIVE_UNUSED void ls_array0_acquire(int32_t value) { (void)value; }
static LS_NATIVE_UNUSED void ls_array0_drop(int32_t value) { (void)value; }
static LS_NATIVE_UNUSED void ls_array0_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_array0 *array = (ls_array0 *)owner; (void)visit; (void)context; for (size_t index = 0; index < array->length; ++index) {  } }
static LS_NATIVE_UNUSED void ls_array0_destroy(ls_native_object *owner) {
ls_array0 *array = (ls_array0 *)owner;
for (size_t index = 0; index < array->length; index++) ls_array0_drop(array->items[index]);
free(array->items);
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_new(size_t capacity) {
ls_array0 *array = ls_native_allocate(sizeof *array, ls_array0_destroy, ls_array0_trace);
array->length = 0; array->capacity = 0; array->items = NULL;
if (capacity) {
if (capacity > SIZE_MAX / sizeof *array->items) ls_native_resource_failure();
array->items = malloc(capacity * sizeof *array->items);
if (!array->items) ls_native_resource_failure();
array->capacity = capacity;
}
return array;
}
static LS_NATIVE_UNUSED void ls_array0_reserve(ls_array0 *array, size_t length) {
if (length <= array->capacity) return;
size_t capacity = array->capacity ? array->capacity : 4;
while (capacity < length) { if (capacity > SIZE_MAX / 2 / sizeof *array->items) ls_native_resource_failure(); capacity *= 2; }
int32_t *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static LS_NATIVE_UNUSED int32_t ls_array0_push_owned(ls_array0 *array, int32_t value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array0_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array0_push(ls_array0 *array, int32_t value) {
ls_array0_acquire(value);
return ls_array0_push_owned(array, value);
}
static LS_NATIVE_UNUSED void ls_array0_hole(ls_array0 *array) { ls_array0_push(array, 0); }
static LS_NATIVE_UNUSED int32_t ls_array0_get(ls_array0 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { ls_native_undefined_element(); }
return array->items[index];
}
static LS_NATIVE_UNUSED void ls_array0_set(ls_array0 *array, int32_t index, int32_t value) {
if (index >= 0 && (size_t)index < array->length) { ls_array0_acquire(value); ls_array0_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array0_push(array, value); return; }
ls_native_undefined_element();
}
static LS_NATIVE_UNUSED int32_t ls_array0_pop(ls_array0 *array) {
if (!array->length) { return 0; }
return array->items[--array->length];
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_slice(ls_array0 *array, size_t start, size_t end) {
ls_array0 *result = ls_array0_new(end > start ? end - start : 0);
for (size_t index = start; index < end && index < array->length; index++) ls_array0_push(result, array->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_concat(ls_array0 *left, ls_array0 *right) {
size_t left_length = left->length, right_length = right->length;
ls_array0 *result = ls_array0_new(left_length + right_length);
for (size_t index = 0; index < left_length; index++) ls_array0_push(result, left->items[index]);
for (size_t index = 0; index < right_length; index++) ls_array0_push(result, right->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_reverse(ls_array0 *array) {
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { int32_t value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_fill(ls_array0 *array, int32_t value) {
for (size_t index = 0; index < array->length; index++) { ls_array0_acquire(value); ls_array0_drop(array->items[index]); array->items[index] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_splice(ls_array0 *array, int32_t start, int32_t count) {
size_t from = ls_array_relative(start, array->length);
size_t removed = count <= 0 ? 0 : (size_t)count;
if (removed > array->length - from) removed = array->length - from;
ls_array0 *result = ls_array0_new(removed);
for (size_t index = 0; index < removed; index++) ls_array0_push_owned(result, array->items[from + index]);
memmove(array->items + from, array->items + from + removed, (array->length - from - removed) * sizeof *array->items);
array->length -= removed;
return result;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_copy_within(ls_array0 *array, int32_t target, int32_t start, bool bounded, int32_t end) {
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
static LS_NATIVE_UNUSED void ls_array0_copy(ls_array0 **slot, ls_array0 *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array0_take(ls_array0 **slot, ls_array0 *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array0_clear(ls_array0 **slot) { ls_native_release(*slot); *slot = NULL; }
static LS_NATIVE_UNUSED ls_value ls_array0_optional(ls_array0 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) return (ls_value){0};
return ls_value_int(array->items[index]);
}
static LS_NATIVE_UNUSED int32_t ls_array0_index_of(ls_array0 *array, int32_t right) {
for (size_t index = 0; index < array->length; index++) { int32_t left = array->items[index]; if (left == right) return (int32_t)index; }
return -1;
}
static LS_NATIVE_UNUSED bool ls_array0_includes(ls_array0 *array, int32_t right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { int32_t left = array->items[index]; if (left == right) return true; }
return false;
}
struct ls_array1 { ls_native_object owner; size_t length; size_t capacity; ls_string *items; };
static LS_NATIVE_UNUSED void ls_array1_acquire(ls_string value) { (void)value; ls_native_retain(value.owner);
}
static LS_NATIVE_UNUSED void ls_array1_drop(ls_string value) { (void)value; ls_native_release(value.owner);
}
static LS_NATIVE_UNUSED void ls_array1_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_array1 *array = (ls_array1 *)owner; (void)visit; (void)context; for (size_t index = 0; index < array->length; ++index) { visit(array->items[index].owner, context);
 } }
static LS_NATIVE_UNUSED void ls_array1_destroy(ls_native_object *owner) {
ls_array1 *array = (ls_array1 *)owner;
for (size_t index = 0; index < array->length; index++) ls_array1_drop(array->items[index]);
free(array->items);
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_new(size_t capacity) {
ls_array1 *array = ls_native_allocate(sizeof *array, ls_array1_destroy, ls_array1_trace);
array->length = 0; array->capacity = 0; array->items = NULL;
if (capacity) {
if (capacity > SIZE_MAX / sizeof *array->items) ls_native_resource_failure();
array->items = malloc(capacity * sizeof *array->items);
if (!array->items) ls_native_resource_failure();
array->capacity = capacity;
}
return array;
}
static LS_NATIVE_UNUSED void ls_array1_reserve(ls_array1 *array, size_t length) {
if (length <= array->capacity) return;
size_t capacity = array->capacity ? array->capacity : 4;
while (capacity < length) { if (capacity > SIZE_MAX / 2 / sizeof *array->items) ls_native_resource_failure(); capacity *= 2; }
ls_string *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static LS_NATIVE_UNUSED int32_t ls_array1_push_owned(ls_array1 *array, ls_string value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array1_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array1_push(ls_array1 *array, ls_string value) {
ls_array1_acquire(value);
return ls_array1_push_owned(array, value);
}
static LS_NATIVE_UNUSED void ls_array1_hole(ls_array1 *array) { ls_array1_push(array, (ls_string){0}); }
static LS_NATIVE_UNUSED ls_string ls_array1_get(ls_array1 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { ls_native_undefined_element(); }
return array->items[index];
}
static LS_NATIVE_UNUSED void ls_array1_set(ls_array1 *array, int32_t index, ls_string value) {
if (index >= 0 && (size_t)index < array->length) { ls_array1_acquire(value); ls_array1_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array1_push(array, value); return; }
ls_native_undefined_element();
}
static LS_NATIVE_UNUSED ls_string ls_array1_pop(ls_array1 *array) {
if (!array->length) { return (ls_string){0}; }
return array->items[--array->length];
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_slice(ls_array1 *array, size_t start, size_t end) {
ls_array1 *result = ls_array1_new(end > start ? end - start : 0);
for (size_t index = start; index < end && index < array->length; index++) ls_array1_push(result, array->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_concat(ls_array1 *left, ls_array1 *right) {
size_t left_length = left->length, right_length = right->length;
ls_array1 *result = ls_array1_new(left_length + right_length);
for (size_t index = 0; index < left_length; index++) ls_array1_push(result, left->items[index]);
for (size_t index = 0; index < right_length; index++) ls_array1_push(result, right->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_reverse(ls_array1 *array) {
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { ls_string value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_fill(ls_array1 *array, ls_string value) {
for (size_t index = 0; index < array->length; index++) { ls_array1_acquire(value); ls_array1_drop(array->items[index]); array->items[index] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_splice(ls_array1 *array, int32_t start, int32_t count) {
size_t from = ls_array_relative(start, array->length);
size_t removed = count <= 0 ? 0 : (size_t)count;
if (removed > array->length - from) removed = array->length - from;
ls_array1 *result = ls_array1_new(removed);
for (size_t index = 0; index < removed; index++) ls_array1_push_owned(result, array->items[from + index]);
memmove(array->items + from, array->items + from + removed, (array->length - from - removed) * sizeof *array->items);
array->length -= removed;
return result;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_copy_within(ls_array1 *array, int32_t target, int32_t start, bool bounded, int32_t end) {
size_t length = array->length;
size_t to = ls_array_relative(target, length), from = ls_array_relative(start, length);
size_t final = bounded ? ls_array_relative(end, length) : length;
if (final > from) {
size_t count = final - from;
if (count > length - to) count = length - to;
for (size_t index = 0; index < count; index++) ls_array1_acquire(array->items[from + index]);
for (size_t index = 0; index < count; index++) ls_array1_drop(array->items[to + index]);
memmove(array->items + to, array->items + from, count * sizeof *array->items);
}
return array;
}
static LS_NATIVE_UNUSED void ls_array1_copy(ls_array1 **slot, ls_array1 *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array1_take(ls_array1 **slot, ls_array1 *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array1_clear(ls_array1 **slot) { ls_native_release(*slot); *slot = NULL; }
static LS_NATIVE_UNUSED ls_value ls_array1_optional(ls_array1 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) return (ls_value){0};
return ls_value_string(array->items[index]);
}
static LS_NATIVE_UNUSED int32_t ls_array1_index_of(ls_array1 *array, ls_string right) {
for (size_t index = 0; index < array->length; index++) { ls_string left = array->items[index]; if (ls_string_equal(left, right)) return (int32_t)index; }
return -1;
}
static LS_NATIVE_UNUSED bool ls_array1_includes(ls_array1 *array, ls_string right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { ls_string left = array->items[index]; if (ls_string_equal(left, right)) return true; }
return false;
}
struct ls_array2 { ls_native_object owner; size_t length; size_t capacity; ls_array0 * *items; };
static LS_NATIVE_UNUSED void ls_array2_acquire(ls_array0 * value) { (void)value; ls_native_retain(value);
}
static LS_NATIVE_UNUSED void ls_array2_drop(ls_array0 * value) { (void)value; ls_native_release(value);
}
static LS_NATIVE_UNUSED void ls_array2_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_array2 *array = (ls_array2 *)owner; (void)visit; (void)context; for (size_t index = 0; index < array->length; ++index) { visit(array->items[index], context);
 } }
static LS_NATIVE_UNUSED void ls_array2_destroy(ls_native_object *owner) {
ls_array2 *array = (ls_array2 *)owner;
for (size_t index = 0; index < array->length; index++) ls_array2_drop(array->items[index]);
free(array->items);
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_new(size_t capacity) {
ls_array2 *array = ls_native_allocate(sizeof *array, ls_array2_destroy, ls_array2_trace);
array->length = 0; array->capacity = 0; array->items = NULL;
if (capacity) {
if (capacity > SIZE_MAX / sizeof *array->items) ls_native_resource_failure();
array->items = malloc(capacity * sizeof *array->items);
if (!array->items) ls_native_resource_failure();
array->capacity = capacity;
}
return array;
}
static LS_NATIVE_UNUSED void ls_array2_reserve(ls_array2 *array, size_t length) {
if (length <= array->capacity) return;
size_t capacity = array->capacity ? array->capacity : 4;
while (capacity < length) { if (capacity > SIZE_MAX / 2 / sizeof *array->items) ls_native_resource_failure(); capacity *= 2; }
ls_array0 * *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static LS_NATIVE_UNUSED int32_t ls_array2_push_owned(ls_array2 *array, ls_array0 * value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array2_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array2_push(ls_array2 *array, ls_array0 * value) {
ls_array2_acquire(value);
return ls_array2_push_owned(array, value);
}
static LS_NATIVE_UNUSED void ls_array2_hole(ls_array2 *array) { (void)array; ls_native_undefined_element(); }
static LS_NATIVE_UNUSED ls_array0 * ls_array2_get(ls_array2 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { ls_native_undefined_element(); }
return array->items[index];
}
static LS_NATIVE_UNUSED void ls_array2_set(ls_array2 *array, int32_t index, ls_array0 * value) {
if (index >= 0 && (size_t)index < array->length) { ls_array2_acquire(value); ls_array2_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array2_push(array, value); return; }
ls_native_undefined_element();
}
static LS_NATIVE_UNUSED ls_array0 * ls_array2_pop(ls_array2 *array) {
if (!array->length) { ls_native_undefined_element(); return array->items[0]; }
return array->items[--array->length];
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_slice(ls_array2 *array, size_t start, size_t end) {
ls_array2 *result = ls_array2_new(end > start ? end - start : 0);
for (size_t index = start; index < end && index < array->length; index++) ls_array2_push(result, array->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_concat(ls_array2 *left, ls_array2 *right) {
size_t left_length = left->length, right_length = right->length;
ls_array2 *result = ls_array2_new(left_length + right_length);
for (size_t index = 0; index < left_length; index++) ls_array2_push(result, left->items[index]);
for (size_t index = 0; index < right_length; index++) ls_array2_push(result, right->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_reverse(ls_array2 *array) {
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { ls_array0 * value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_fill(ls_array2 *array, ls_array0 * value) {
for (size_t index = 0; index < array->length; index++) { ls_array2_acquire(value); ls_array2_drop(array->items[index]); array->items[index] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_splice(ls_array2 *array, int32_t start, int32_t count) {
size_t from = ls_array_relative(start, array->length);
size_t removed = count <= 0 ? 0 : (size_t)count;
if (removed > array->length - from) removed = array->length - from;
ls_array2 *result = ls_array2_new(removed);
for (size_t index = 0; index < removed; index++) ls_array2_push_owned(result, array->items[from + index]);
memmove(array->items + from, array->items + from + removed, (array->length - from - removed) * sizeof *array->items);
array->length -= removed;
return result;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_copy_within(ls_array2 *array, int32_t target, int32_t start, bool bounded, int32_t end) {
size_t length = array->length;
size_t to = ls_array_relative(target, length), from = ls_array_relative(start, length);
size_t final = bounded ? ls_array_relative(end, length) : length;
if (final > from) {
size_t count = final - from;
if (count > length - to) count = length - to;
for (size_t index = 0; index < count; index++) ls_array2_acquire(array->items[from + index]);
for (size_t index = 0; index < count; index++) ls_array2_drop(array->items[to + index]);
memmove(array->items + to, array->items + from, count * sizeof *array->items);
}
return array;
}
static LS_NATIVE_UNUSED void ls_array2_copy(ls_array2 **slot, ls_array2 *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array2_take(ls_array2 **slot, ls_array2 *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array2_clear(ls_array2 **slot) { ls_native_release(*slot); *slot = NULL; }
static LS_NATIVE_UNUSED ls_value ls_array2_optional(ls_array2 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) return (ls_value){0};
return ls_value_array((ls_native_object *)array->items[index]);
}
static LS_NATIVE_UNUSED int32_t ls_array2_index_of(ls_array2 *array, ls_array0 * right) {
for (size_t index = 0; index < array->length; index++) { ls_array0 * left = array->items[index]; if (left == right) return (int32_t)index; }
return -1;
}
static LS_NATIVE_UNUSED bool ls_array2_includes(ls_array2 *array, ls_array0 * right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { ls_array0 * left = array->items[index]; if (left == right) return true; }
return false;
}
struct ls_array3 { ls_native_object owner; size_t length; size_t capacity; bool *items; };
static LS_NATIVE_UNUSED void ls_array3_acquire(bool value) { (void)value; }
static LS_NATIVE_UNUSED void ls_array3_drop(bool value) { (void)value; }
static LS_NATIVE_UNUSED void ls_array3_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_array3 *array = (ls_array3 *)owner; (void)visit; (void)context; for (size_t index = 0; index < array->length; ++index) {  } }
static LS_NATIVE_UNUSED void ls_array3_destroy(ls_native_object *owner) {
ls_array3 *array = (ls_array3 *)owner;
for (size_t index = 0; index < array->length; index++) ls_array3_drop(array->items[index]);
free(array->items);
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_new(size_t capacity) {
ls_array3 *array = ls_native_allocate(sizeof *array, ls_array3_destroy, ls_array3_trace);
array->length = 0; array->capacity = 0; array->items = NULL;
if (capacity) {
if (capacity > SIZE_MAX / sizeof *array->items) ls_native_resource_failure();
array->items = malloc(capacity * sizeof *array->items);
if (!array->items) ls_native_resource_failure();
array->capacity = capacity;
}
return array;
}
static LS_NATIVE_UNUSED void ls_array3_reserve(ls_array3 *array, size_t length) {
if (length <= array->capacity) return;
size_t capacity = array->capacity ? array->capacity : 4;
while (capacity < length) { if (capacity > SIZE_MAX / 2 / sizeof *array->items) ls_native_resource_failure(); capacity *= 2; }
bool *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static LS_NATIVE_UNUSED int32_t ls_array3_push_owned(ls_array3 *array, bool value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array3_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array3_push(ls_array3 *array, bool value) {
ls_array3_acquire(value);
return ls_array3_push_owned(array, value);
}
static LS_NATIVE_UNUSED void ls_array3_hole(ls_array3 *array) { (void)array; ls_native_undefined_element(); }
static LS_NATIVE_UNUSED bool ls_array3_get(ls_array3 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { ls_native_undefined_element(); }
return array->items[index];
}
static LS_NATIVE_UNUSED void ls_array3_set(ls_array3 *array, int32_t index, bool value) {
if (index >= 0 && (size_t)index < array->length) { ls_array3_acquire(value); ls_array3_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array3_push(array, value); return; }
ls_native_undefined_element();
}
static LS_NATIVE_UNUSED bool ls_array3_pop(ls_array3 *array) {
if (!array->length) { ls_native_undefined_element(); return array->items[0]; }
return array->items[--array->length];
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_slice(ls_array3 *array, size_t start, size_t end) {
ls_array3 *result = ls_array3_new(end > start ? end - start : 0);
for (size_t index = start; index < end && index < array->length; index++) ls_array3_push(result, array->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_concat(ls_array3 *left, ls_array3 *right) {
size_t left_length = left->length, right_length = right->length;
ls_array3 *result = ls_array3_new(left_length + right_length);
for (size_t index = 0; index < left_length; index++) ls_array3_push(result, left->items[index]);
for (size_t index = 0; index < right_length; index++) ls_array3_push(result, right->items[index]);
return result;
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_reverse(ls_array3 *array) {
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { bool value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_fill(ls_array3 *array, bool value) {
for (size_t index = 0; index < array->length; index++) { ls_array3_acquire(value); ls_array3_drop(array->items[index]); array->items[index] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_splice(ls_array3 *array, int32_t start, int32_t count) {
size_t from = ls_array_relative(start, array->length);
size_t removed = count <= 0 ? 0 : (size_t)count;
if (removed > array->length - from) removed = array->length - from;
ls_array3 *result = ls_array3_new(removed);
for (size_t index = 0; index < removed; index++) ls_array3_push_owned(result, array->items[from + index]);
memmove(array->items + from, array->items + from + removed, (array->length - from - removed) * sizeof *array->items);
array->length -= removed;
return result;
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_copy_within(ls_array3 *array, int32_t target, int32_t start, bool bounded, int32_t end) {
size_t length = array->length;
size_t to = ls_array_relative(target, length), from = ls_array_relative(start, length);
size_t final = bounded ? ls_array_relative(end, length) : length;
if (final > from) {
size_t count = final - from;
if (count > length - to) count = length - to;
for (size_t index = 0; index < count; index++) ls_array3_acquire(array->items[from + index]);
for (size_t index = 0; index < count; index++) ls_array3_drop(array->items[to + index]);
memmove(array->items + to, array->items + from, count * sizeof *array->items);
}
return array;
}
static LS_NATIVE_UNUSED void ls_array3_copy(ls_array3 **slot, ls_array3 *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array3_take(ls_array3 **slot, ls_array3 *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_array3_clear(ls_array3 **slot) { ls_native_release(*slot); *slot = NULL; }
static LS_NATIVE_UNUSED ls_value ls_array3_optional(ls_array3 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) return (ls_value){0};
return ls_value_bool(array->items[index]);
}
static LS_NATIVE_UNUSED int32_t ls_array3_index_of(ls_array3 *array, bool right) {
for (size_t index = 0; index < array->length; index++) { bool left = array->items[index]; if (left == right) return (int32_t)index; }
return -1;
}
static LS_NATIVE_UNUSED bool ls_array3_includes(ls_array3 *array, bool right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { bool left = array->items[index]; if (left == right) return true; }
return false;
}
static LS_NATIVE_UNUSED ls_array1 *ls_string_split(ls_string text, ls_string separator) {
ls_array1 *parts = ls_array1_new(0);
if (!separator.length) {
for (size_t index = 0; index < text.length; index++) ls_array1_push_owned(parts, ls_string_view(text, index, index + 1));
return parts;
}
if (!text.length) { ls_array1_push(parts, text); return parts; }
size_t start = 0;
for (size_t at = 0; at + separator.length <= text.length;) {
if (ls_string_matches(text, at, separator)) { ls_array1_push_owned(parts, ls_string_view(text, start, at)); at += separator.length; start = at; }
else at++;
}
ls_array1_push_owned(parts, ls_string_view(text, start, text.length));
return parts;
}
static LS_NATIVE_UNUSED void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef int32_t host_assertNativeCount_arg0;
typedef void host_assertNativeCount_result;
void host_assertNativeCount(int32_t);
typedef void host_collectNative_result;
void host_collectNative(void);
static LS_NATIVE_UNUSED const uint16_t ls_s2[] = {120,};
static LS_NATIVE_UNUSED const uint16_t ls_s3[] = {108,97,98,101,108,};
static LS_NATIVE_UNUSED const uint16_t ls_s4[] = {118,97,108,117,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s5[] = {110,101,120,116,};
static LS_NATIVE_UNUSED const uint16_t ls_s6[] = {107,105,110,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s7[] = {116,101,120,116,};
static LS_NATIVE_UNUSED const uint16_t ls_s8[] = {101,110,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s9[] = {108,105,110,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s12[] = {49,48,};
static LS_NATIVE_UNUSED const uint16_t ls_s13[] = {116,101,110,};
static LS_NATIVE_UNUSED const uint16_t ls_s14[] = {50,};
static LS_NATIVE_UNUSED const uint16_t ls_s15[] = {116,119,111,};
static LS_NATIVE_UNUSED const uint16_t ls_s16[] = {98,};
static LS_NATIVE_UNUSED const uint16_t ls_s17[] = {98,101,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s18[] = {97,};
static LS_NATIVE_UNUSED const uint16_t ls_s19[] = {97,121,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s20[] = {48,49,};
static LS_NATIVE_UNUSED const uint16_t ls_s21[] = {111,110,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s22[] = {52,50,57,52,57,54,55,50,57,53,};
static LS_NATIVE_UNUSED const uint16_t ls_s23[] = {108,97,115,116,};
static LS_NATIVE_UNUSED const uint16_t ls_s24[] = {52,50,57,52,57,54,55,50,57,52,};
static LS_NATIVE_UNUSED const uint16_t ls_s25[] = {105,110,100,101,120,};
static LS_NATIVE_UNUSED const uint16_t ls_s26[] = {95,95,112,114,111,116,111,95,95,};
static LS_NATIVE_UNUSED const uint16_t ls_s27[] = {111,119,110,};
static LS_NATIVE_UNUSED const uint16_t ls_s29[] = {101,109,112,116,121,};
static LS_NATIVE_UNUSED const uint16_t ls_s30[] = {66,};
static LS_NATIVE_UNUSED const uint16_t ls_s31[] = {51,};
static LS_NATIVE_UNUSED const uint16_t ls_s32[] = {116,104,114,101,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s33[] = {65,};
static LS_NATIVE_UNUSED const uint16_t ls_s34[] = {99,};
static LS_NATIVE_UNUSED const uint16_t ls_s35[] = {67,};
static LS_NATIVE_UNUSED const uint16_t ls_s36[] = {48,};
static LS_NATIVE_UNUSED const uint16_t ls_s37[] = {122,101,114,111,};
static LS_NATIVE_UNUSED const uint16_t ls_s38[] = {124,};
static LS_NATIVE_UNUSED const uint16_t ls_s39[] = {116,111,83,116,114,105,110,103,};
static LS_NATIVE_UNUSED const uint16_t ls_s40[] = {109,105,115,115,105,110,103,};
static LS_NATIVE_UNUSED const uint16_t ls_s41[] = {99,104,105,108,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s42[] = {97,110,115,119,101,114,};
static LS_NATIVE_UNUSED const uint16_t ls_s43[] = {97,10,9,8,12,13,92,34,};
static LS_NATIVE_UNUSED const uint16_t ls_s44[] = {1,55296,88,57343,};
static LS_NATIVE_UNUSED const uint16_t ls_s45[] = {55357,56832,};
static LS_NATIVE_UNUSED const uint16_t ls_s46[] = {8232,8233,};
static LS_NATIVE_UNUSED const uint16_t ls_s47[] = {113,117,111,116,101,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s49[] = {109,105,100,100,108,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s50[] = {97,98,99,};
static LS_NATIVE_UNUSED const uint16_t ls_s51[] = {104,101,108,108,111,};
static LS_NATIVE_UNUSED const uint16_t ls_s52[] = {104,105,};
static LS_NATIVE_UNUSED const uint16_t ls_s53[] = {100,97,116,97,32,100,111,110,101,};
static LS_NATIVE_UNUSED void ls_init0(void);
static LS_NATIVE_UNUSED void ls_fn1(void);
static LS_NATIVE_UNUSED void ls_fn2(void);
static LS_NATIVE_UNUSED int32_t ls_fn3(ls_value ls_c16 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn4(void);
static LS_NATIVE_UNUSED void ls_init0(void) {
int32_t ls_v13 LS_NATIVE_UNUSED;
ls_string ls_v15 LS_NATIVE_UNUSED = {0};
ls_fn1();
ls_fn2();
ls_fn4();
host_collectNative();
ls_v13 = ls_from_u32(UINT32_C(0));
host_assertNativeCount(ls_v13);
ls_string_take(&(ls_v15),(ls_string){ls_s53,sizeof ls_s53/sizeof *ls_s53,NULL});
ls_print_string(ls_v15);
ls_string_clear(&ls_v15);
}
static LS_NATIVE_UNUSED void ls_fn1(void) {
ls_native_object * ls_c7 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c8 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c9 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_c10 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c11 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c12 LS_NATIVE_UNUSED = NULL;
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
ls_string ls_v1 LS_NATIVE_UNUSED = {0};
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_string ls_v4 LS_NATIVE_UNUSED = {0};
ls_string ls_v5 LS_NATIVE_UNUSED = {0};
ls_string ls_v6 LS_NATIVE_UNUSED = {0};
ls_string ls_v7 LS_NATIVE_UNUSED = {0};
ls_string ls_v8 LS_NATIVE_UNUSED = {0};
ls_string ls_v9 LS_NATIVE_UNUSED = {0};
ls_string ls_v10 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v11 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v12 LS_NATIVE_UNUSED = NULL;
ls_string ls_v13 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v15 LS_NATIVE_UNUSED = NULL;
ls_string ls_v16 LS_NATIVE_UNUSED = {0};
int32_t ls_v17 LS_NATIVE_UNUSED;
ls_string ls_v18 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v19 LS_NATIVE_UNUSED = NULL;
ls_string ls_v20 LS_NATIVE_UNUSED = {0};
ls_string ls_v21 LS_NATIVE_UNUSED = {0};
ls_string ls_v22 LS_NATIVE_UNUSED = {0};
ls_string ls_v23 LS_NATIVE_UNUSED = {0};
ls_string ls_v24 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v25 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v26 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v27 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v28 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v29 LS_NATIVE_UNUSED = NULL;
bool ls_v30 LS_NATIVE_UNUSED;
ls_native_object * ls_v32 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v33 LS_NATIVE_UNUSED = NULL;
ls_string ls_v34 LS_NATIVE_UNUSED = {0};
ls_string ls_v35 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v37 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v38 LS_NATIVE_UNUSED = NULL;
ls_string ls_v39 LS_NATIVE_UNUSED = {0};
ls_string ls_v40 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v42 LS_NATIVE_UNUSED = NULL;
ls_string ls_v43 LS_NATIVE_UNUSED = {0};
bool ls_v44 LS_NATIVE_UNUSED;
ls_native_object * ls_v46 LS_NATIVE_UNUSED = NULL;
ls_string ls_v47 LS_NATIVE_UNUSED = {0};
bool ls_v48 LS_NATIVE_UNUSED;
ls_native_object * ls_v50 LS_NATIVE_UNUSED = NULL;
ls_value ls_v51 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v53 LS_NATIVE_UNUSED = NULL;
ls_string ls_v54 LS_NATIVE_UNUSED = {0};
ls_value ls_v55 LS_NATIVE_UNUSED = {0};
ls_string ls_v56 LS_NATIVE_UNUSED = {0};
ls_string ls_v57 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v59 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v60 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v61 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v62 LS_NATIVE_UNUSED = NULL;
bool ls_v63 LS_NATIVE_UNUSED;
int32_t ls_v65 LS_NATIVE_UNUSED;
int32_t ls_v66 LS_NATIVE_UNUSED;
ls_array0 * ls_v67 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v68 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v69 LS_NATIVE_UNUSED = NULL;
ls_value ls_v70 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v71 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v72 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v73 LS_NATIVE_UNUSED = NULL;
int32_t ls_v74 LS_NATIVE_UNUSED;
int32_t ls_v75 LS_NATIVE_UNUSED;
ls_native_object * ls_v76 LS_NATIVE_UNUSED = NULL;
ls_array2 * ls_v77 LS_NATIVE_UNUSED = NULL;
int32_t ls_v78 LS_NATIVE_UNUSED;
ls_array0 * ls_v79 LS_NATIVE_UNUSED = NULL;
int32_t ls_v80 LS_NATIVE_UNUSED;
ls_native_object * ls_v82 LS_NATIVE_UNUSED = NULL;
int32_t ls_v83 LS_NATIVE_UNUSED;
ls_array0 * ls_v84 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v85 LS_NATIVE_UNUSED = NULL;
int32_t ls_v86 LS_NATIVE_UNUSED;
int32_t ls_v88 LS_NATIVE_UNUSED;
ls_native_object * ls_v89 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v90 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v91 LS_NATIVE_UNUSED = NULL;
ls_value ls_v92 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v93 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v94 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v95 LS_NATIVE_UNUSED = NULL;
ls_value ls_v96 LS_NATIVE_UNUSED = {0};
int32_t ls_v97 LS_NATIVE_UNUSED;
int32_t ls_v98 LS_NATIVE_UNUSED;
ls_string_take(&(ls_v0),(ls_string){ls_s13,sizeof ls_s13/sizeof *ls_s13,NULL});
ls_string_take(&(ls_v1),(ls_string){ls_s15,sizeof ls_s15/sizeof *ls_s15,NULL});
ls_string_take(&(ls_v2),(ls_string){ls_s17,sizeof ls_s17/sizeof *ls_s17,NULL});
ls_v3 = ls_from_u32(UINT32_C(2));
ls_string_take(&(ls_v4),ls_string_repeat(ls_v2,ls_v3));
ls_string_take(&(ls_v5),(ls_string){ls_s19,sizeof ls_s19/sizeof *ls_s19,NULL});
ls_string_take(&(ls_v6),(ls_string){ls_s21,sizeof ls_s21/sizeof *ls_s21,NULL});
ls_string_take(&(ls_v7),(ls_string){ls_s23,sizeof ls_s23/sizeof *ls_s23,NULL});
ls_string_take(&(ls_v8),(ls_string){ls_s25,sizeof ls_s25/sizeof *ls_s25,NULL});
ls_string_take(&(ls_v9),(ls_string){ls_s27,sizeof ls_s27/sizeof *ls_s27,NULL});
ls_string_take(&(ls_v10),(ls_string){ls_s29,sizeof ls_s29/sizeof *ls_s29,NULL});
ls_object_take(&(ls_v11),ls_record_new());
ls_record_set(ls_v11,(ls_string){ls_s12,sizeof ls_s12/sizeof *ls_s12,NULL},ls_value_string(ls_v0));
ls_record_set(ls_v11,(ls_string){ls_s14,sizeof ls_s14/sizeof *ls_s14,NULL},ls_value_string(ls_v1));
ls_record_set(ls_v11,(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL},ls_value_string(ls_v4));
ls_record_set(ls_v11,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL},ls_value_string(ls_v5));
ls_record_set(ls_v11,(ls_string){ls_s20,sizeof ls_s20/sizeof *ls_s20,NULL},ls_value_string(ls_v6));
ls_record_set(ls_v11,(ls_string){ls_s22,sizeof ls_s22/sizeof *ls_s22,NULL},ls_value_string(ls_v7));
ls_record_set(ls_v11,(ls_string){ls_s24,sizeof ls_s24/sizeof *ls_s24,NULL},ls_value_string(ls_v8));
ls_record_set(ls_v11,(ls_string){ls_s26,sizeof ls_s26/sizeof *ls_s26,NULL},ls_value_string(ls_v9));
ls_record_set(ls_v11,(ls_string){0},ls_value_string(ls_v10));
ls_object_copy(&(ls_c7),ls_v11);
ls_object_copy(&(ls_v12),ls_c7);
ls_string_take(&(ls_v13),ls_json_record(ls_v12));
ls_print_string(ls_v13);
ls_object_copy(&(ls_v15),ls_c7);
ls_string_take(&(ls_v16),(ls_string){ls_s30,sizeof ls_s30/sizeof *ls_s30,NULL});
ls_v17 = ls_from_u32(UINT32_C(3));
ls_string_take(&(ls_v18),ls_string_repeat(ls_v16,ls_v17));
ls_shape_set(ls_v15,(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL},ls_value_string(ls_v18),false);
ls_object_copy(&(ls_v19),ls_c7);
ls_string_take(&(ls_v20),(ls_string){ls_s31,sizeof ls_s31/sizeof *ls_s31,NULL});
ls_string_take(&(ls_v21),(ls_string){ls_s32,sizeof ls_s32/sizeof *ls_s32,NULL});
ls_shape_set(ls_v19,ls_v20,ls_value_string(ls_v21),false);
ls_string_take(&(ls_v22),(ls_string){ls_s33,sizeof ls_s33/sizeof *ls_s33,NULL});
ls_string_take(&(ls_v23),(ls_string){ls_s35,sizeof ls_s35/sizeof *ls_s35,NULL});
ls_string_take(&(ls_v24),(ls_string){ls_s37,sizeof ls_s37/sizeof *ls_s37,NULL});
ls_object_take(&(ls_v25),ls_record_new());
ls_record_set(ls_v25,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL},ls_value_string(ls_v22));
ls_record_set(ls_v25,(ls_string){ls_s34,sizeof ls_s34/sizeof *ls_s34,NULL},ls_value_string(ls_v23));
ls_record_set(ls_v25,(ls_string){ls_s36,sizeof ls_s36/sizeof *ls_s36,NULL},ls_value_string(ls_v24));
ls_object_copy(&(ls_c8),ls_v25);
ls_object_copy(&(ls_v26),ls_c7);
ls_object_copy(&(ls_v27),ls_c8);
ls_object_copy(&(ls_v28),ls_record_assign(ls_v26,ls_v27));
ls_object_copy(&(ls_v29),ls_c7);
ls_v30 = ls_v28 == ls_v29;
puts(ls_v30 ? "true" : "false");
ls_object_copy(&(ls_v32),ls_c7);
{
ls_map *ls_record = (ls_map *)ls_v32;
ls_record_key *ls_order = ls_record_order(ls_v32);
ls_array1_take(&(ls_v33),ls_array1_new(ls_record->size));
for (size_t ls_i = 0; ls_i < ls_record->size; ++ls_i) {
ls_array1_push(ls_v33,ls_value_to_string(ls_record->entries[ls_order[ls_i].position].key));
}
free(ls_order);
}
ls_string_take(&(ls_v34),(ls_string){ls_s38,sizeof ls_s38/sizeof *ls_s38,NULL});
{
ls_string_builder ls_join = {0};
for (size_t ls_k = 0; ls_k < ls_v33->length; ++ls_k) {
if (ls_k) ls_string_builder_text(&ls_join,ls_v34);
ls_string_builder_value(&ls_join,ls_value_string(ls_v33->items[ls_k]));
}
ls_string_take(&(ls_v35),ls_string_builder_finish(&ls_join));
}
ls_print_string(ls_v35);
ls_object_copy(&(ls_v37),ls_c7);
{
ls_map *ls_record = (ls_map *)ls_v37;
ls_record_key *ls_order = ls_record_order(ls_v37);
ls_array1_take(&(ls_v38),ls_array1_new(ls_record->size));
for (size_t ls_i = 0; ls_i < ls_record->size; ++ls_i) {
ls_array1_push(ls_v38,ls_value_to_string(ls_record->entries[ls_order[ls_i].position].value));
}
free(ls_order);
}
ls_string_take(&(ls_v39),(ls_string){ls_s38,sizeof ls_s38/sizeof *ls_s38,NULL});
{
ls_string_builder ls_join = {0};
for (size_t ls_k = 0; ls_k < ls_v38->length; ++ls_k) {
if (ls_k) ls_string_builder_text(&ls_join,ls_v39);
ls_string_builder_value(&ls_join,ls_value_string(ls_v38->items[ls_k]));
}
ls_string_take(&(ls_v40),ls_string_builder_finish(&ls_join));
}
ls_print_string(ls_v40);
ls_object_copy(&(ls_v42),ls_c7);
ls_string_take(&(ls_v43),(ls_string){ls_s26,sizeof ls_s26/sizeof *ls_s26,NULL});
ls_v44 = ls_record_has(ls_v42,ls_v43);
puts(ls_v44 ? "true" : "false");
ls_object_copy(&(ls_v46),ls_c7);
ls_string_take(&(ls_v47),(ls_string){ls_s39,sizeof ls_s39/sizeof *ls_s39,NULL});
ls_v48 = ls_record_has(ls_v46,ls_v47);
puts(ls_v48 ? "true" : "false");
ls_object_copy(&(ls_v50),ls_c7);
ls_value_copy(&(ls_v51),ls_record_get(ls_v50,(ls_string){ls_s40,sizeof ls_s40/sizeof *ls_s40,NULL}));
ls_print_value(ls_v51);
ls_object_copy(&(ls_v53),ls_c7);
ls_string_take(&(ls_v54),(ls_string){ls_s26,sizeof ls_s26/sizeof *ls_s26,NULL});
ls_value_copy(&(ls_v55),ls_record_get(ls_v53,ls_v54));
if (ls_v55.tag != LS_NULL) {
ls_string_copy(&(ls_v57),ls_value_to_string(ls_v55));
} else {
ls_string_take(&(ls_v56),(ls_string){ls_s40,sizeof ls_s40/sizeof *ls_s40,NULL});
ls_string_copy(&(ls_v57),ls_v56);
ls_string_clear(&ls_v56);
}
ls_print_string(ls_v57);
ls_object_copy(&(ls_v59),ls_c7);
ls_object_copy(&(ls_v60),ls_c7);
ls_object_copy(&(ls_v61),ls_record_assign(ls_v59,ls_v60));
ls_object_copy(&(ls_v62),ls_c7);
ls_v63 = ls_v61 == ls_v62;
puts(ls_v63 ? "true" : "false");
ls_v65 = ls_from_u32(UINT32_C(1));
ls_v66 = ls_from_u32(UINT32_C(2));
ls_array0_take(&(ls_v67),ls_array0_new(2));
ls_array0_push(ls_v67,ls_v65);
ls_array0_push(ls_v67,ls_v66);
ls_object_take(&(ls_v68),ls_record_new());
ls_record_set(ls_v68,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL},ls_value_array((ls_native_object *)ls_v67));
ls_object_copy(&(ls_c9),ls_v68);
ls_object_copy(&(ls_v69),ls_c9);
ls_value_copy(&(ls_v70),ls_record_get(ls_v69,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL}));
if (ls_v70.tag != LS_NULL) {
ls_array0_copy(&(ls_v72),(ls_array0 *)ls_value_to_array(ls_v70));
} else {
ls_array0_take(&(ls_v71),ls_array0_new(0));
ls_array0_copy(&(ls_v72),ls_v71);
ls_array0_clear(&ls_v71);
}
ls_array0_copy(&(ls_c10),ls_v72);
ls_array0_copy(&(ls_v73),ls_c10);
ls_v74 = ls_from_u32(UINT32_C(3));
ls_v75 = ls_array0_push(ls_v73,ls_v74);
ls_object_copy(&(ls_v76),ls_c9);
{
ls_map *ls_record = (ls_map *)ls_v76;
ls_record_key *ls_order = ls_record_order(ls_v76);
ls_array2_take(&(ls_v77),ls_array2_new(ls_record->size));
for (size_t ls_i = 0; ls_i < ls_record->size; ++ls_i) {
ls_array2_push(ls_v77,(ls_array0 *)ls_value_to_array(ls_record->entries[ls_order[ls_i].position].value));
}
free(ls_order);
}
ls_v78 = ls_from_u32(UINT32_C(0));
ls_array0_copy(&(ls_v79),ls_array2_get(ls_v77,ls_v78));
ls_v80 = (int32_t)ls_v79->length;
printf("%ld\n",(long)ls_v80);
ls_object_copy(&(ls_v82),ls_c9);
ls_v83 = ls_from_u32(UINT32_C(7));
ls_array0_take(&(ls_v84),ls_array0_new(1));
ls_array0_push(ls_v84,ls_v83);
ls_shape_set(ls_v82,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL},ls_value_array((ls_native_object *)ls_v84),false);
ls_array0_copy(&(ls_v85),ls_c10);
ls_v86 = (int32_t)ls_v85->length;
printf("%ld\n",(long)ls_v86);
ls_v88 = ls_from_u32(UINT32_C(42));
ls_object_take(&(ls_v89),ls_record_new());
ls_record_set(ls_v89,(ls_string){ls_s42,sizeof ls_s42/sizeof *ls_s42,NULL},ls_value_int(ls_v88));
ls_object_take(&(ls_v90),ls_record_new());
ls_record_set(ls_v90,(ls_string){ls_s41,sizeof ls_s41/sizeof *ls_s41,NULL},ls_value_object(ls_v89));
ls_object_copy(&(ls_c11),ls_v90);
ls_object_copy(&(ls_v91),ls_c11);
ls_value_copy(&(ls_v92),ls_record_get(ls_v91,(ls_string){ls_s41,sizeof ls_s41/sizeof *ls_s41,NULL}));
if (ls_v92.tag != LS_NULL) {
ls_object_copy(&(ls_v94),ls_value_to_object(ls_v92));
} else {
ls_object_take(&(ls_v93),ls_record_new());
ls_object_copy(&(ls_v94),ls_v93);
ls_object_clear(&ls_v93);
}
ls_object_copy(&(ls_c12),ls_v94);
ls_object_copy(&(ls_v95),ls_c12);
ls_v96 = ls_record_get(ls_v95,(ls_string){ls_s42,sizeof ls_s42/sizeof *ls_s42,NULL});
if (ls_v96.tag != LS_NULL) {
ls_v98 = ls_value_to_int(ls_v96);
} else {
ls_v97 = ls_from_u32(UINT32_C(0));
ls_v98 = ls_v97;
}
printf("%ld\n",(long)ls_v98);
ls_object_clear(&ls_v95);
ls_object_clear(&ls_c12);
ls_object_clear(&ls_v94);
ls_value_clear(&ls_v92);
ls_object_clear(&ls_v91);
ls_object_clear(&ls_c11);
ls_object_clear(&ls_v90);
ls_object_clear(&ls_v89);
ls_array0_clear(&ls_v85);
ls_array0_clear(&ls_v84);
ls_object_clear(&ls_v82);
ls_array0_clear(&ls_v79);
ls_array2_clear(&ls_v77);
ls_object_clear(&ls_v76);
ls_array0_clear(&ls_v73);
ls_array0_clear(&ls_c10);
ls_array0_clear(&ls_v72);
ls_value_clear(&ls_v70);
ls_object_clear(&ls_v69);
ls_object_clear(&ls_c9);
ls_object_clear(&ls_v68);
ls_array0_clear(&ls_v67);
ls_object_clear(&ls_v62);
ls_object_clear(&ls_v61);
ls_object_clear(&ls_v60);
ls_object_clear(&ls_v59);
ls_string_clear(&ls_v57);
ls_value_clear(&ls_v55);
ls_string_clear(&ls_v54);
ls_object_clear(&ls_v53);
ls_value_clear(&ls_v51);
ls_object_clear(&ls_v50);
ls_string_clear(&ls_v47);
ls_object_clear(&ls_v46);
ls_string_clear(&ls_v43);
ls_object_clear(&ls_v42);
ls_string_clear(&ls_v40);
ls_string_clear(&ls_v39);
ls_array1_clear(&ls_v38);
ls_object_clear(&ls_v37);
ls_string_clear(&ls_v35);
ls_string_clear(&ls_v34);
ls_array1_clear(&ls_v33);
ls_object_clear(&ls_v32);
ls_object_clear(&ls_v29);
ls_object_clear(&ls_v28);
ls_object_clear(&ls_v27);
ls_object_clear(&ls_v26);
ls_object_clear(&ls_c8);
ls_object_clear(&ls_v25);
ls_string_clear(&ls_v24);
ls_string_clear(&ls_v23);
ls_string_clear(&ls_v22);
ls_string_clear(&ls_v21);
ls_string_clear(&ls_v20);
ls_object_clear(&ls_v19);
ls_string_clear(&ls_v18);
ls_string_clear(&ls_v16);
ls_object_clear(&ls_v15);
ls_string_clear(&ls_v13);
ls_object_clear(&ls_v12);
ls_object_clear(&ls_c7);
ls_object_clear(&ls_v11);
ls_string_clear(&ls_v10);
ls_string_clear(&ls_v9);
ls_string_clear(&ls_v8);
ls_string_clear(&ls_v7);
ls_string_clear(&ls_v6);
ls_string_clear(&ls_v5);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v1);
ls_string_clear(&ls_v0);
}
static LS_NATIVE_UNUSED void ls_fn2(void) {
ls_array1 * ls_c13 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c14 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c15 LS_NATIVE_UNUSED = NULL;
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
ls_string ls_v1 LS_NATIVE_UNUSED = {0};
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
ls_string ls_v3 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v4 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v5 LS_NATIVE_UNUSED = NULL;
ls_string ls_v6 LS_NATIVE_UNUSED = {0};
int32_t ls_v8 LS_NATIVE_UNUSED;
int32_t ls_v9 LS_NATIVE_UNUSED;
int32_t ls_v10 LS_NATIVE_UNUSED;
ls_array0 * ls_v11 LS_NATIVE_UNUSED = NULL;
ls_string ls_v12 LS_NATIVE_UNUSED = {0};
bool ls_v14 LS_NATIVE_UNUSED;
bool ls_v15 LS_NATIVE_UNUSED;
ls_array3 * ls_v16 LS_NATIVE_UNUSED = NULL;
ls_string ls_v17 LS_NATIVE_UNUSED = {0};
ls_string ls_v19 LS_NATIVE_UNUSED = {0};
ls_string ls_v20 LS_NATIVE_UNUSED = {0};
ls_value ls_v22 LS_NATIVE_UNUSED = {0};
ls_string ls_v23 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v25 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v26 LS_NATIVE_UNUSED = NULL;
ls_string ls_v27 LS_NATIVE_UNUSED = {0};
ls_string ls_v29 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v30 LS_NATIVE_UNUSED = NULL;
ls_string ls_v31 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v32 LS_NATIVE_UNUSED = NULL;
ls_string ls_v33 LS_NATIVE_UNUSED = {0};
ls_string_take(&(ls_v0),(ls_string){ls_s43,sizeof ls_s43/sizeof *ls_s43,NULL});
ls_string_take(&(ls_v1),(ls_string){ls_s44,sizeof ls_s44/sizeof *ls_s44,NULL});
ls_string_take(&(ls_v2),(ls_string){ls_s45,sizeof ls_s45/sizeof *ls_s45,NULL});
ls_string_take(&(ls_v3),(ls_string){ls_s46,sizeof ls_s46/sizeof *ls_s46,NULL});
ls_array1_take(&(ls_v4),ls_array1_new(4));
ls_array1_push(ls_v4,ls_v0);
ls_array1_push(ls_v4,ls_v1);
ls_array1_push(ls_v4,ls_v2);
ls_array1_push(ls_v4,ls_v3);
ls_array1_copy(&(ls_c13),ls_v4);
ls_array1_copy(&(ls_v5),ls_c13);
{
ls_string_builder ls_json = {0};
ls_string_builder_unit(&ls_json,'[');
for (size_t ls_i = 0; ls_i < ls_v5->length; ++ls_i) {
if (ls_i) ls_string_builder_unit(&ls_json,',');
ls_json_value(&ls_json,ls_value_string(ls_v5->items[ls_i]));
}
ls_string_builder_unit(&ls_json,']');
ls_string_take(&(ls_v6),ls_string_builder_finish(&ls_json));
}
ls_print_string(ls_v6);
ls_v8 = ls_from_u32(UINT32_C(2147483648));
ls_v9 = ls_from_u32(UINT32_C(0));
ls_v10 = ls_from_u32(UINT32_C(2147483647));
ls_array0_take(&(ls_v11),ls_array0_new(3));
ls_array0_push(ls_v11,ls_v8);
ls_array0_push(ls_v11,ls_v9);
ls_array0_push(ls_v11,ls_v10);
{
ls_string_builder ls_json = {0};
ls_string_builder_unit(&ls_json,'[');
for (size_t ls_i = 0; ls_i < ls_v11->length; ++ls_i) {
if (ls_i) ls_string_builder_unit(&ls_json,',');
ls_json_value(&ls_json,ls_value_int(ls_v11->items[ls_i]));
}
ls_string_builder_unit(&ls_json,']');
ls_string_take(&(ls_v12),ls_string_builder_finish(&ls_json));
}
ls_print_string(ls_v12);
ls_v14 = true;
ls_v15 = false;
ls_array3_take(&(ls_v16),ls_array3_new(2));
ls_array3_push(ls_v16,ls_v14);
ls_array3_push(ls_v16,ls_v15);
{
ls_string_builder ls_json = {0};
ls_string_builder_unit(&ls_json,'[');
for (size_t ls_i = 0; ls_i < ls_v16->length; ++ls_i) {
if (ls_i) ls_string_builder_unit(&ls_json,',');
ls_json_value(&ls_json,ls_value_bool(ls_v16->items[ls_i]));
}
ls_string_builder_unit(&ls_json,']');
ls_string_take(&(ls_v17),ls_string_builder_finish(&ls_json));
}
ls_print_string(ls_v17);
ls_string_take(&(ls_v19),(ls_string){ls_s47,sizeof ls_s47/sizeof *ls_s47,NULL});
ls_string_take(&(ls_v20),ls_json_scalar(ls_value_string(ls_v19)));
ls_print_string(ls_v20);
ls_v22 = (ls_value){0};
ls_string_take(&(ls_v23),ls_json_scalar(ls_v22));
ls_print_string(ls_v23);
ls_object_take(&(ls_v25),ls_record_new());
ls_object_copy(&(ls_c14),ls_v25);
ls_object_copy(&(ls_v26),ls_c14);
ls_string_take(&(ls_v27),ls_json_record(ls_v26));
ls_print_string(ls_v27);
ls_string_take(&(ls_v29),(ls_string){ls_s49,sizeof ls_s49/sizeof *ls_s49,NULL});
ls_string_take(&(ls_v31),(ls_string){ls_s23,sizeof ls_s23/sizeof *ls_s23,NULL});
ls_object_take(&(ls_v30),ls_record_new());
ls_record_set(ls_v30,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL},ls_value_string(ls_v31));
ls_record_set(ls_v30,(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL},ls_value_string(ls_v29));
ls_object_copy(&(ls_c15),ls_v30);
ls_object_copy(&(ls_v32),ls_c15);
ls_string_take(&(ls_v33),ls_json_record(ls_v32));
ls_print_string(ls_v33);
ls_string_clear(&ls_v33);
ls_object_clear(&ls_v32);
ls_object_clear(&ls_c15);
ls_object_clear(&ls_v30);
ls_string_clear(&ls_v31);
ls_string_clear(&ls_v29);
ls_string_clear(&ls_v27);
ls_object_clear(&ls_v26);
ls_object_clear(&ls_c14);
ls_object_clear(&ls_v25);
ls_string_clear(&ls_v23);
ls_string_clear(&ls_v20);
ls_string_clear(&ls_v19);
ls_string_clear(&ls_v17);
ls_array3_clear(&ls_v16);
ls_string_clear(&ls_v12);
ls_array0_clear(&ls_v11);
ls_string_clear(&ls_v6);
ls_array1_clear(&ls_v5);
ls_array1_clear(&ls_c13);
ls_array1_clear(&ls_v4);
ls_string_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v1);
ls_string_clear(&ls_v0);
}
static LS_NATIVE_UNUSED int32_t ls_fn3(ls_value ls_c16 LS_NATIVE_UNUSED) {
ls_value_retain(ls_c16);
ls_value ls_v0 LS_NATIVE_UNUSED = {0};
bool ls_v1 LS_NATIVE_UNUSED;
ls_native_object * ls_v2 LS_NATIVE_UNUSED = NULL;
ls_string ls_v3 LS_NATIVE_UNUSED = {0};
int32_t ls_v4 LS_NATIVE_UNUSED;
ls_native_object * ls_v5 LS_NATIVE_UNUSED = NULL;
int32_t ls_v6 LS_NATIVE_UNUSED;
ls_value_copy(&(ls_v0),ls_c16);
ls_v1 = ls_shape_tag(ls_v0,(ls_string){ls_s6,sizeof ls_s6/sizeof *ls_s6,NULL},ls_value_string((ls_string){ls_s7,sizeof ls_s7/sizeof *ls_s7,NULL}));
if (ls_v1) {
ls_object_copy(&(ls_v2),ls_value_to_object(ls_c16));
ls_string_copy(&(ls_v3),ls_value_to_string(ls_record_get(ls_v2,(ls_string){ls_s7,sizeof ls_s7/sizeof *ls_s7,NULL})));
ls_v4 = ls_string_length(ls_v3);
{
int32_t ls_return = ls_v4;
ls_string_clear(&ls_v3);
ls_object_clear(&ls_v2);
ls_object_clear(&ls_v5);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c16);
return ls_return;
}
ls_string_clear(&ls_v3);
ls_object_clear(&ls_v2);
}
ls_object_copy(&(ls_v5),ls_value_to_object(ls_c16));
ls_v6 = ls_value_to_int(ls_record_get(ls_v5,(ls_string){ls_s9,sizeof ls_s9/sizeof *ls_s9,NULL}));
{
int32_t ls_return = ls_v6;
ls_object_clear(&ls_v5);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c16);
return ls_return;
}
ls_object_clear(&ls_v5);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c16);
}
static LS_NATIVE_UNUSED void ls_fn4(void) {
ls_native_object * ls_c17 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c18 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c19 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c20 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c21 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c22 LS_NATIVE_UNUSED = NULL;
int32_t ls_v0 LS_NATIVE_UNUSED;
ls_native_object * ls_v1 LS_NATIVE_UNUSED = NULL;
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_string ls_v4 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v5 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v6 LS_NATIVE_UNUSED = NULL;
int32_t ls_v7 LS_NATIVE_UNUSED;
ls_native_object * ls_v8 LS_NATIVE_UNUSED = NULL;
ls_string ls_v9 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v10 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v11 LS_NATIVE_UNUSED = NULL;
int32_t ls_v12 LS_NATIVE_UNUSED;
ls_native_object * ls_v13 LS_NATIVE_UNUSED = NULL;
ls_string ls_v14 LS_NATIVE_UNUSED = {0};
int32_t ls_v15 LS_NATIVE_UNUSED;
int32_t ls_v16 LS_NATIVE_UNUSED;
ls_native_object * ls_v18 LS_NATIVE_UNUSED = NULL;
int32_t ls_v19 LS_NATIVE_UNUSED;
ls_native_object * ls_v20 LS_NATIVE_UNUSED = NULL;
int32_t ls_v21 LS_NATIVE_UNUSED;
ls_native_object * ls_v23 LS_NATIVE_UNUSED = NULL;
int32_t ls_v24 LS_NATIVE_UNUSED;
int32_t ls_v26 LS_NATIVE_UNUSED;
ls_value ls_v27 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v28 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v29 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v30 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v31 LS_NATIVE_UNUSED = NULL;
ls_value ls_v32 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v33 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v34 LS_NATIVE_UNUSED = NULL;
int32_t ls_v35 LS_NATIVE_UNUSED;
ls_string ls_v37 LS_NATIVE_UNUSED = {0};
ls_string ls_v38 LS_NATIVE_UNUSED = {0};
ls_value ls_v39 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v40 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v41 LS_NATIVE_UNUSED = NULL;
int32_t ls_v42 LS_NATIVE_UNUSED;
ls_native_object * ls_v43 LS_NATIVE_UNUSED = NULL;
ls_string ls_v44 LS_NATIVE_UNUSED = {0};
ls_value ls_v45 LS_NATIVE_UNUSED = {0};
ls_string ls_v46 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v47 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v48 LS_NATIVE_UNUSED = NULL;
ls_value ls_v49 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v50 LS_NATIVE_UNUSED = NULL;
ls_value ls_v51 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v53 LS_NATIVE_UNUSED = NULL;
ls_value ls_v54 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v57 LS_NATIVE_UNUSED = NULL;
int32_t ls_v58 LS_NATIVE_UNUSED;
int32_t ls_v61 LS_NATIVE_UNUSED;
ls_string ls_v62 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v63 LS_NATIVE_UNUSED = NULL;
int32_t ls_v64 LS_NATIVE_UNUSED;
ls_v0 = ls_from_u32(UINT32_C(3));
ls_object_take(&(ls_v1),ls_record_new());
ls_shape_set(ls_v1,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL},ls_value_int(ls_v0),false);
ls_object_copy(&(ls_c17),ls_v1);
ls_string_take(&(ls_v2),(ls_string){ls_s50,sizeof ls_s50/sizeof *ls_s50,NULL});
ls_v3 = ls_from_u32(UINT32_C(2));
ls_string_take(&(ls_v4),ls_string_repeat(ls_v2,ls_v3));
ls_object_take(&(ls_v5),ls_record_new());
ls_shape_set(ls_v5,(ls_string){ls_s3,sizeof ls_s3/sizeof *ls_s3,NULL},ls_value_string(ls_v4),false);
ls_object_copy(&(ls_c18),ls_v5);
ls_object_copy(&(ls_v6),ls_c17);
ls_v7 = ls_value_to_int(ls_record_get(ls_v6,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL}));
ls_object_copy(&(ls_v8),ls_c18);
ls_string_copy(&(ls_v9),ls_value_to_string(ls_record_get(ls_v8,(ls_string){ls_s3,sizeof ls_s3/sizeof *ls_s3,NULL})));
ls_object_take(&(ls_v10),ls_record_new());
ls_shape_set(ls_v10,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL},ls_value_int(ls_v7),false);
ls_shape_set(ls_v10,(ls_string){ls_s3,sizeof ls_s3/sizeof *ls_s3,NULL},ls_value_string(ls_v9),false);
ls_object_copy(&(ls_c19),ls_v10);
ls_object_copy(&(ls_v11),ls_c19);
ls_v12 = ls_value_to_int(ls_record_get(ls_v11,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL}));
ls_object_copy(&(ls_v13),ls_c19);
ls_string_copy(&(ls_v14),ls_value_to_string(ls_record_get(ls_v13,(ls_string){ls_s3,sizeof ls_s3/sizeof *ls_s3,NULL})));
ls_v15 = ls_string_length(ls_v14);
ls_v16 = ls_from_u32((uint32_t)((uint32_t)ls_v12 + (uint32_t)ls_v15));
printf("%ld\n",(long)ls_v16);
ls_object_copy(&(ls_v18),ls_c19);
ls_v19 = ls_from_u32(UINT32_C(7));
ls_shape_set(ls_v18,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL},ls_value_int(ls_v19),false);
ls_object_copy(&(ls_v20),ls_c17);
ls_v21 = ls_value_to_int(ls_record_get(ls_v20,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL}));
printf("%ld\n",(long)ls_v21);
ls_object_copy(&(ls_v23),ls_c19);
ls_v24 = ls_value_to_int(ls_record_get(ls_v23,(ls_string){ls_s2,sizeof ls_s2/sizeof *ls_s2,NULL}));
printf("%ld\n",(long)ls_v24);
ls_v26 = ls_from_u32(UINT32_C(9));
ls_value_take(&(ls_v27),(ls_value){0});
ls_object_take(&(ls_v28),ls_record_new());
ls_shape_set(ls_v28,(ls_string){ls_s4,sizeof ls_s4/sizeof *ls_s4,NULL},ls_value_int(ls_v26),false);
ls_shape_set(ls_v28,(ls_string){ls_s5,sizeof ls_s5/sizeof *ls_s5,NULL},ls_v27,true);
ls_object_copy(&(ls_c20),ls_v28);
ls_object_copy(&(ls_v29),ls_c20);
ls_object_copy(&(ls_v30),ls_c20);
ls_shape_set(ls_v29,(ls_string){ls_s5,sizeof ls_s5/sizeof *ls_s5,NULL},ls_value_object(ls_v30),true);
ls_object_copy(&(ls_v31),ls_c20);
ls_value_copy(&(ls_v32),ls_record_get(ls_v31,(ls_string){ls_s5,sizeof ls_s5/sizeof *ls_s5,NULL}));
if (ls_v32.tag != LS_NULL) {
ls_object_copy(&(ls_v34),ls_value_to_object(ls_v32));
} else {
ls_object_copy(&(ls_v33),ls_c20);
ls_object_copy(&(ls_v34),ls_v33);
ls_object_clear(&ls_v33);
}
ls_v35 = ls_value_to_int(ls_record_get(ls_v34,(ls_string){ls_s4,sizeof ls_s4/sizeof *ls_s4,NULL}));
printf("%ld\n",(long)ls_v35);
ls_string_take(&(ls_v37),(ls_string){ls_s51,sizeof ls_s51/sizeof *ls_s51,NULL});
ls_string_take(&(ls_v38),(ls_string){ls_s7,sizeof ls_s7/sizeof *ls_s7,NULL});
ls_v39 = (ls_value){0};
ls_object_take(&(ls_v40),ls_record_new());
ls_shape_set(ls_v40,(ls_string){ls_s6,sizeof ls_s6/sizeof *ls_s6,NULL},ls_value_string(ls_v38),false);
ls_shape_set(ls_v40,(ls_string){ls_s7,sizeof ls_s7/sizeof *ls_s7,NULL},ls_value_string(ls_v37),false);
ls_shape_set(ls_v40,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL},ls_v39,true);
ls_object_copy(&(ls_c21),ls_v40);
ls_object_copy(&(ls_v41),ls_c21);
ls_v42 = ls_from_u32(UINT32_C(8));
ls_shape_set(ls_v41,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL},ls_value_int(ls_v42),true);
ls_object_copy(&(ls_v43),ls_c21);
ls_string_copy(&(ls_v44),ls_value_to_string(ls_record_get(ls_v43,(ls_string){ls_s6,sizeof ls_s6/sizeof *ls_s6,NULL})));
ls_v45 = ls_record_get(ls_v43,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL});
ls_string_take(&(ls_v46),(ls_string){ls_s52,sizeof ls_s52/sizeof *ls_s52,NULL});
ls_object_take(&(ls_v47),ls_record_new());
ls_shape_set(ls_v47,(ls_string){ls_s6,sizeof ls_s6/sizeof *ls_s6,NULL},ls_value_string(ls_v44),false);
ls_shape_set(ls_v47,(ls_string){ls_s7,sizeof ls_s7/sizeof *ls_s7,NULL},ls_value_string(ls_v46),false);
ls_shape_set(ls_v47,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL},ls_v45,true);
ls_object_copy(&(ls_c22),ls_v47);
ls_object_copy(&(ls_v48),ls_c21);
ls_v49 = (ls_value){0};
ls_shape_set(ls_v48,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL},ls_v49,true);
ls_object_copy(&(ls_v50),ls_c22);
ls_v51 = ls_record_get(ls_v50,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL});
ls_print_value(ls_v51);
ls_object_copy(&(ls_v53),ls_c21);
ls_v54 = ls_record_get(ls_v53,(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL});
ls_print_value(ls_v54);
ls_object_copy(&(ls_v57),ls_c21);
ls_v58 = ls_fn3(ls_value_object(ls_v57));
printf("%ld\n",(long)ls_v58);
ls_v61 = ls_from_u32(UINT32_C(42));
ls_string_take(&(ls_v62),(ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8,NULL});
ls_object_take(&(ls_v63),ls_record_new());
ls_shape_set(ls_v63,(ls_string){ls_s6,sizeof ls_s6/sizeof *ls_s6,NULL},ls_value_string(ls_v62),false);
ls_shape_set(ls_v63,(ls_string){ls_s9,sizeof ls_s9/sizeof *ls_s9,NULL},ls_value_int(ls_v61),false);
ls_v64 = ls_fn3(ls_value_object(ls_v63));
printf("%ld\n",(long)ls_v64);
ls_object_clear(&ls_v63);
ls_string_clear(&ls_v62);
ls_object_clear(&ls_v57);
ls_object_clear(&ls_v53);
ls_object_clear(&ls_v50);
ls_object_clear(&ls_v48);
ls_object_clear(&ls_c22);
ls_object_clear(&ls_v47);
ls_string_clear(&ls_v46);
ls_string_clear(&ls_v44);
ls_object_clear(&ls_v43);
ls_object_clear(&ls_v41);
ls_object_clear(&ls_c21);
ls_object_clear(&ls_v40);
ls_string_clear(&ls_v38);
ls_string_clear(&ls_v37);
ls_object_clear(&ls_v34);
ls_value_clear(&ls_v32);
ls_object_clear(&ls_v31);
ls_object_clear(&ls_v30);
ls_object_clear(&ls_v29);
ls_object_clear(&ls_c20);
ls_object_clear(&ls_v28);
ls_value_clear(&ls_v27);
ls_object_clear(&ls_v23);
ls_object_clear(&ls_v20);
ls_object_clear(&ls_v18);
ls_string_clear(&ls_v14);
ls_object_clear(&ls_v13);
ls_object_clear(&ls_v11);
ls_object_clear(&ls_c19);
ls_object_clear(&ls_v10);
ls_string_clear(&ls_v9);
ls_object_clear(&ls_v8);
ls_object_clear(&ls_v6);
ls_object_clear(&ls_c18);
ls_object_clear(&ls_v5);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v2);
ls_object_clear(&ls_c17);
ls_object_clear(&ls_v1);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(5);
ls_init0();
fflush(stdout);
ls_native_collect_cycles();
return 0;
}
