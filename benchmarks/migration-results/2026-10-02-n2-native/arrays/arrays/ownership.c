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
enum { LS_NULL, LS_INT, LS_FLOAT, LS_BOOL, LS_STRING, LS_OBJECT, LS_ARRAY, LS_CALLABLE, LS_SYMBOL, LS_PRODUCT };
static LS_NATIVE_UNUSED inline void ls_value_retain(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL || value.tag == LS_PRODUCT) ls_native_retain(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_retain(value.as.c.environment);
    else if (value.tag == LS_STRING) ls_native_retain(value.as.s.owner);
}
static LS_NATIVE_UNUSED inline void ls_value_release(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL || value.tag == LS_PRODUCT) ls_native_release(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_release(value.as.c.environment);
    else if (value.tag == LS_STRING) ls_native_release(value.as.s.owner);
}
static LS_NATIVE_UNUSED inline void ls_value_trace(ls_value value, ls_native_visit visit, void *context) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL || value.tag == LS_PRODUCT) visit(value.as.o, context);
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
    case LS_OBJECT: case LS_ARRAY: case LS_SYMBOL: case LS_PRODUCT: return left.as.o == right.as.o;
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
/* Expression-temporary ownership. A product box embeds its link, so there is
   no side allocation or process-lifetime arena. The generated activation owns
   this chain until its statement has copied/retained every escaping result. */
typedef struct ls_native_temporary {
    struct ls_native_temporary *next;
    ls_native_object *owner;
} ls_native_temporary;
static LS_NATIVE_UNUSED inline void ls_native_temporary_push(ls_native_temporary **head, ls_native_temporary *entry, ls_native_object *owner) {
    entry->next=*head; entry->owner=owner; *head=entry;
}
static LS_NATIVE_UNUSED inline void ls_native_temporaries_clear(ls_native_temporary **head) {
    while (*head) {
        ls_native_temporary *entry=*head;
        ls_native_object *owner=entry->owner;
        *head=entry->next;
        ls_native_release(owner);
    }
}
/* One array identity and buffer across concrete and generic views. Concrete
   accessors retain their typed fast path; the descriptor converts only when
   the requested element representation differs from the physical buffer. */
typedef struct ls_array_ops {
    size_t width;
    void (*retain)(const void *);
    void (*drop)(void *);
    void (*trace)(const void *, ls_native_visit, void *);
    ls_value (*read)(ls_native_temporary **, const void *);
    void (*write)(void *, ls_value);
} ls_array_ops;
typedef struct ls_native_array {
    ls_native_object owner;
    const ls_array_ops *ops;
    size_t length, capacity;
    void *items;
    /* Null for a dense array. Sparse map results allocate presence lazily. */
    uint8_t *present;
} ls_native_array;
static LS_NATIVE_UNUSED void ls_native_undefined_element(void) {
    fputs("LilScript native array element is undefined\n",stderr); abort();
}
static LS_NATIVE_UNUSED inline void *ls_array_item(ls_native_array *array, size_t index) {
    return (unsigned char *)array->items + index*array->ops->width;
}
static LS_NATIVE_UNUSED inline bool ls_array_has(ls_native_array *array, size_t index) {
    return index < array->length && (!array->present || array->present[index]);
}
static LS_NATIVE_UNUSED void ls_array_destroy(ls_native_object *owner) {
    ls_native_array *array=(ls_native_array *)owner;
    for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) array->ops->drop(ls_array_item(array,i));
    free(array->items); free(array->present);
}
static LS_NATIVE_UNUSED void ls_array_trace(ls_native_object *owner, ls_native_visit visit, void *context) {
    ls_native_array *array=(ls_native_array *)owner;
    for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) array->ops->trace(ls_array_item(array,i),visit,context);
}
static LS_NATIVE_UNUSED inline void ls_array_reserve(ls_native_array *array, size_t needed) {
    if(needed <= array->capacity) return;
    if(needed > (size_t)INT32_MAX) ls_native_resource_failure();
    size_t capacity=array->capacity ? array->capacity : 4;
    while(capacity < needed) {
        if(capacity > (size_t)INT32_MAX/2) { capacity=INT32_MAX; break; }
        capacity*=2;
    }
    if(capacity > SIZE_MAX/array->ops->width) ls_native_resource_failure();
    void *items=realloc(array->items,capacity*array->ops->width);
    if(!items) ls_native_resource_failure();
    array->items=items;
    if(array->present) {
        uint8_t *present=realloc(array->present,capacity);
        if(!present) ls_native_resource_failure();
        memset(present+array->capacity,0,capacity-array->capacity);
        array->present=present;
    }
    array->capacity=capacity;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_new(const ls_array_ops *ops, size_t capacity) {
    ls_native_array *array=ls_native_allocate(sizeof *array,ls_array_destroy,ls_array_trace);
    array->ops=ops; ls_array_reserve(array,capacity); return array;
}
static LS_NATIVE_UNUSED inline void ls_array_presence(ls_native_array *array) {
    if(array->present) return;
    array->present=calloc(array->capacity ? array->capacity : 1,1);
    if(!array->present) ls_native_resource_failure();
    memset(array->present,1,array->length);
}
static LS_NATIVE_UNUSED inline void *ls_array_append_slot(ls_native_array *array) {
    ls_array_reserve(array,array->length+1);
    void *slot=ls_array_item(array,array->length);
    memset(slot,0,array->ops->width);
    if(array->present) array->present[array->length]=1;
    ++array->length; return slot;
}
static LS_NATIVE_UNUSED inline void *ls_array_store_slot(ls_native_array *array, int32_t index) {
    if(index<0 || (size_t)index>array->length) ls_native_undefined_element();
    if((size_t)index==array->length) return ls_array_append_slot(array);
    if(array->present) array->present[index]=1;
    return ls_array_item(array,(size_t)index);
}
static LS_NATIVE_UNUSED inline void ls_array_hole(ls_native_array *array) {
    ls_array_append_slot(array); ls_array_presence(array); array->present[array->length-1]=0;
}
static LS_NATIVE_UNUSED inline ls_value ls_array_read(ls_native_array *array, size_t index, ls_native_temporary **temps) {
    if(!ls_array_has(array,index)) return (ls_value){0};
    return array->ops->read(temps,ls_array_item(array,index));
}
static LS_NATIVE_UNUSED inline void ls_array_write(ls_native_array *array, int32_t index, ls_value value) {
    array->ops->write(ls_array_store_slot(array,index),value);
}
static LS_NATIVE_UNUSED inline void ls_array_drop_last(ls_native_array *array) {
    if(!array->length) return;
    size_t index=array->length-1;
    if(ls_array_has(array,index)) array->ops->drop(ls_array_item(array,index));
    memset(ls_array_item(array,index),0,array->ops->width);
    array->length=index;
}
static LS_NATIVE_UNUSED inline size_t ls_array_relative(int32_t index, size_t length) {
    if(index<0) return (size_t)-(int64_t)index>=length ? 0 : length-(size_t)-(int64_t)index;
    return (size_t)index<length ? (size_t)index : length;
}
/* The caller selects separate destination storage. Equal schemas copy directly;
   different schemas preserve aliases through a short-lived tagged view. */
static LS_NATIVE_UNUSED inline void ls_array_append_from(ls_native_array *out, ls_native_array *source, size_t index) {
    if(!ls_array_has(source,index)) { ls_array_hole(out); return; }
    if(out->ops==source->ops) {
        void *slot=ls_array_append_slot(out);
        const void *input=ls_array_item(source,index);
        source->ops->retain(input); memcpy(slot,input,source->ops->width);
    } else {
        ls_native_temporary *temps=NULL;
        ls_value value=ls_array_read(source,index,&temps);
        out->ops->write(ls_array_append_slot(out),value);
        ls_native_temporaries_clear(&temps);
    }
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_slice(ls_native_array *array, size_t start, size_t end) {
    if(end>array->length) end=array->length;
    if(end<start) end=start;
    ls_native_array *out=ls_array_new(array->ops,end-start);
    for(size_t i=start;i<end;++i) ls_array_append_from(out,array,i);
    return out;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_concat(ls_native_array *left, ls_native_array *right) {
    if(left->length > (size_t)INT32_MAX-right->length) ls_native_resource_failure();
    ls_native_array *out=ls_array_new(left->ops,left->length+right->length);
    for(size_t i=0;i<left->length;++i) ls_array_append_from(out,left,i);
    for(size_t i=0;i<right->length;++i) ls_array_append_from(out,right,i);
    return out;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_splice(ls_native_array *array, int32_t start, int32_t count) {
    size_t from=ls_array_relative(start,array->length),removed=count>0?(size_t)count:0;
    if(removed>array->length-from) removed=array->length-from;
    ls_native_array *out=ls_array_slice(array,from,from+removed);
    for(size_t i=0;i<removed;++i) if(ls_array_has(array,from+i)) array->ops->drop(ls_array_item(array,from+i));
    size_t remaining=array->length-from-removed;
    if(remaining) memmove(ls_array_item(array,from),ls_array_item(array,from+removed),remaining*array->ops->width);
    if(array->present && remaining) memmove(array->present+from,array->present+from+removed,remaining);
    array->length-=removed; return out;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_reverse(ls_native_array *array) {
    if(array->length<2) return array;
    void *temporary=malloc(array->ops->width);
    if(!temporary) ls_native_resource_failure();
    for(size_t low=0,high=array->length-1;low<high;++low,--high) {
        void *a=ls_array_item(array,low),*b=ls_array_item(array,high);
        memcpy(temporary,a,array->ops->width); memcpy(a,b,array->ops->width); memcpy(b,temporary,array->ops->width);
        if(array->present) { uint8_t flag=array->present[low]; array->present[low]=array->present[high]; array->present[high]=flag; }
    }
    free(temporary); return array;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_copy_within(ls_native_array *array, int32_t target, int32_t start, bool bounded, int32_t end) {
    size_t to=ls_array_relative(target,array->length),from=ls_array_relative(start,array->length);
    size_t final=bounded?ls_array_relative(end,array->length):array->length;
    if(final<=from) return array;
    size_t count=final-from;
    if(count>array->length-to) count=array->length-to;
    for(size_t i=0;i<count;++i) if(ls_array_has(array,from+i)) array->ops->retain(ls_array_item(array,from+i));
    for(size_t i=0;i<count;++i) if(ls_array_has(array,to+i)) array->ops->drop(ls_array_item(array,to+i));
    if(count) {
        memmove(ls_array_item(array,to),ls_array_item(array,from),count*array->ops->width);
        if(array->present) memmove(array->present+to,array->present+from,count);
    }
    return array;
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
    case LS_OBJECT: case LS_ARRAY: case LS_SYMBOL: case LS_PRODUCT: return ls_map_mix((uint64_t)(uintptr_t)key.as.o);
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
typedef struct ls_t0 ls_t0;
typedef struct ls_t1 ls_t1;
typedef struct ls_t2 ls_t2;
typedef struct ls_native_array ls_array0;
typedef struct ls_native_array ls_array1;
typedef struct ls_native_array ls_array2;
typedef struct ls_native_array ls_array3;
typedef struct ls_native_array ls_array4;
typedef struct ls_native_array ls_array5;
typedef struct {
void (*code)(void *,int32_t);
void *environment;
uint64_t identity;
} ls_callable1;
typedef struct {
void (*code)(void *);
void *environment;
uint64_t identity;
} ls_callable2;
typedef struct {
ls_value (*code)(void *,ls_value);
void *environment;
uint64_t identity;
} ls_callable15;
typedef struct {
bool (*code)(void *,ls_value);
void *environment;
uint64_t identity;
} ls_callable18;
typedef struct {
int32_t (*code)(void *,int32_t);
void *environment;
uint64_t identity;
} ls_callable27;
typedef struct {
ls_t2 (*code)(void *,ls_t2,ls_t2);
void *environment;
uint64_t identity;
} ls_callable40;
struct ls_t0 {
ls_array0 * ls_f0;
};
static LS_NATIVE_UNUSED inline void ls_t0_retain(ls_t0 value) { (void)value;
ls_native_retain(value.ls_f0);
}
static LS_NATIVE_UNUSED inline void ls_t0_release(ls_t0 value) { (void)value;
ls_native_release(value.ls_f0);
}
static LS_NATIVE_UNUSED inline void ls_t0_trace(ls_t0 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
visit(value.ls_f0, context);
}
static LS_NATIVE_UNUSED inline void ls_t0_copy(ls_t0 *slot, ls_t0 value) { ls_t0_retain(value); ls_t0_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t0_take(ls_t0 *slot, ls_t0 value) { ls_t0_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t0_clear(ls_t0 *slot) { ls_t0_release(*slot); *slot = (ls_t0){0}; }
struct ls_t1 {
int32_t ls_f0;
};
static LS_NATIVE_UNUSED inline void ls_t1_retain(ls_t1 value) { (void)value;
}
static LS_NATIVE_UNUSED inline void ls_t1_release(ls_t1 value) { (void)value;
}
static LS_NATIVE_UNUSED inline void ls_t1_trace(ls_t1 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
}
static LS_NATIVE_UNUSED inline void ls_t1_copy(ls_t1 *slot, ls_t1 value) { ls_t1_retain(value); ls_t1_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t1_take(ls_t1 *slot, ls_t1 value) { ls_t1_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t1_clear(ls_t1 *slot) { ls_t1_release(*slot); *slot = (ls_t1){0}; }
struct ls_t2 {
ls_string ls_f0;
ls_array1 * ls_f1;
};
static LS_NATIVE_UNUSED inline void ls_t2_retain(ls_t2 value) { (void)value;
ls_native_retain(value.ls_f0.owner);
ls_native_retain(value.ls_f1);
}
static LS_NATIVE_UNUSED inline void ls_t2_release(ls_t2 value) { (void)value;
ls_native_release(value.ls_f0.owner);
ls_native_release(value.ls_f1);
}
static LS_NATIVE_UNUSED inline void ls_t2_trace(ls_t2 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
visit(value.ls_f0.owner, context);
visit(value.ls_f1, context);
}
static LS_NATIVE_UNUSED inline void ls_t2_copy(ls_t2 *slot, ls_t2 value) { ls_t2_retain(value); ls_t2_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t2_take(ls_t2 *slot, ls_t2 value) { ls_t2_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t2_clear(ls_t2 *slot) { ls_t2_release(*slot); *slot = (ls_t2){0}; }
typedef struct { ls_native_object owner; ls_native_temporary temporary; ls_t0 value; } ls_product0;
static LS_NATIVE_UNUSED void ls_product0_destroy(ls_native_object *owner) { ls_t0_release(((ls_product0 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_product0_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_t0_trace(((ls_product0 *)owner)->value,visit,context); }
static LS_NATIVE_UNUSED ls_value ls_t0_box(ls_native_temporary **temps, ls_t0 value) {
ls_product0 *box=ls_native_allocate(sizeof *box,ls_product0_destroy,ls_product0_trace);
ls_t0_retain(value); box->value=value;
ls_native_temporary_push(temps,&box->temporary,&box->owner);
ls_value result={.tag=LS_PRODUCT}; result.as.o=&box->owner; return result;
}
static LS_NATIVE_UNUSED ls_t0 ls_value_to_t0(ls_value value) {
if (value.tag!=LS_PRODUCT || !value.as.o || value.as.o->destroy!=ls_product0_destroy) ls_value_mismatch();
return ((ls_product0 *)value.as.o)->value;
}
typedef struct { ls_native_object owner; ls_native_temporary temporary; ls_t1 value; } ls_product1;
static LS_NATIVE_UNUSED void ls_product1_destroy(ls_native_object *owner) { ls_t1_release(((ls_product1 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_product1_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_t1_trace(((ls_product1 *)owner)->value,visit,context); }
static LS_NATIVE_UNUSED ls_value ls_t1_box(ls_native_temporary **temps, ls_t1 value) {
ls_product1 *box=ls_native_allocate(sizeof *box,ls_product1_destroy,ls_product1_trace);
ls_t1_retain(value); box->value=value;
ls_native_temporary_push(temps,&box->temporary,&box->owner);
ls_value result={.tag=LS_PRODUCT}; result.as.o=&box->owner; return result;
}
static LS_NATIVE_UNUSED ls_t1 ls_value_to_t1(ls_value value) {
if (value.tag!=LS_PRODUCT || !value.as.o || value.as.o->destroy!=ls_product1_destroy) ls_value_mismatch();
return ((ls_product1 *)value.as.o)->value;
}
typedef struct { ls_native_object owner; ls_native_temporary temporary; ls_t2 value; } ls_product2;
static LS_NATIVE_UNUSED void ls_product2_destroy(ls_native_object *owner) { ls_t2_release(((ls_product2 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_product2_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_t2_trace(((ls_product2 *)owner)->value,visit,context); }
static LS_NATIVE_UNUSED ls_value ls_t2_box(ls_native_temporary **temps, ls_t2 value) {
ls_product2 *box=ls_native_allocate(sizeof *box,ls_product2_destroy,ls_product2_trace);
ls_t2_retain(value); box->value=value;
ls_native_temporary_push(temps,&box->temporary,&box->owner);
ls_value result={.tag=LS_PRODUCT}; result.as.o=&box->owner; return result;
}
static LS_NATIVE_UNUSED ls_t2 ls_value_to_t2(ls_value value) {
if (value.tag!=LS_PRODUCT || !value.as.o || value.as.o->destroy!=ls_product2_destroy) ls_value_mismatch();
return ((ls_product2 *)value.as.o)->value;
}
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
static LS_NATIVE_UNUSED inline void ls_callable1_call(ls_callable1 value,int32_t ls_p0) {
ls_native_retain(value.environment);
value.code(value.environment,ls_p0);
ls_native_release(value.environment);
}
static LS_NATIVE_UNUSED inline ls_callable2 ls_callable2_retain(ls_callable2 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable2_release(ls_callable2 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable2_copy(ls_callable2 *destination, ls_callable2 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable2_take(ls_callable2 *destination, ls_callable2 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable2_clear(ls_callable2 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable2){0};
}
static LS_NATIVE_UNUSED inline void ls_callable2_call(ls_callable2 value) {
ls_native_retain(value.environment);
value.code(value.environment);
ls_native_release(value.environment);
}
static LS_NATIVE_UNUSED inline ls_callable15 ls_callable15_retain(ls_callable15 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable15_release(ls_callable15 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable15_copy(ls_callable15 *destination, ls_callable15 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable15_take(ls_callable15 *destination, ls_callable15 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable15_clear(ls_callable15 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable15){0};
}
static LS_NATIVE_UNUSED inline ls_value ls_callable15_call(ls_callable15 value,ls_value ls_p0) {
ls_native_retain(value.environment);
ls_value result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable18 ls_callable18_retain(ls_callable18 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable18_release(ls_callable18 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable18_copy(ls_callable18 *destination, ls_callable18 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable18_take(ls_callable18 *destination, ls_callable18 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable18_clear(ls_callable18 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable18){0};
}
static LS_NATIVE_UNUSED inline bool ls_callable18_call(ls_callable18 value,ls_value ls_p0) {
ls_native_retain(value.environment);
bool result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable27 ls_callable27_retain(ls_callable27 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable27_release(ls_callable27 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable27_copy(ls_callable27 *destination, ls_callable27 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable27_take(ls_callable27 *destination, ls_callable27 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable27_clear(ls_callable27 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable27){0};
}
static LS_NATIVE_UNUSED inline int32_t ls_callable27_call(ls_callable27 value,int32_t ls_p0) {
ls_native_retain(value.environment);
int32_t result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable40 ls_callable40_retain(ls_callable40 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable40_release(ls_callable40 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable40_copy(ls_callable40 *destination, ls_callable40 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable40_take(ls_callable40 *destination, ls_callable40 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable40_clear(ls_callable40 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable40){0};
}
static LS_NATIVE_UNUSED inline ls_t2 ls_callable40_call(ls_callable40 value,ls_t2 ls_p0,ls_t2 ls_p1) {
ls_native_retain(value.environment);
ls_t2 result = value.code(value.environment,ls_p0,ls_p1);
ls_native_release(value.environment);
return result;
}
_Static_assert(sizeof(ls_callable1) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable1(ls_callable1 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 1}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable1 ls_value_to_callable1(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable1){0};
if (value.tag != LS_CALLABLE || value.signature != 1) ls_value_mismatch();
ls_callable1 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable2) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable2(ls_callable2 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 2}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable2 ls_value_to_callable2(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable2){0};
if (value.tag != LS_CALLABLE || value.signature != 2) ls_value_mismatch();
ls_callable2 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable15) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable15(ls_callable15 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 15}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable15 ls_value_to_callable15(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable15){0};
if (value.tag != LS_CALLABLE || value.signature != 15) ls_value_mismatch();
ls_callable15 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable18) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable18(ls_callable18 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 18}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable18 ls_value_to_callable18(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable18){0};
if (value.tag != LS_CALLABLE || value.signature != 18) ls_value_mismatch();
ls_callable18 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable27) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable27(ls_callable27 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 27}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable27 ls_value_to_callable27(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable27){0};
if (value.tag != LS_CALLABLE || value.signature != 27) ls_value_mismatch();
ls_callable27 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable40) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable40(ls_callable40 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 40}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable40 ls_value_to_callable40(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable40){0};
if (value.tag != LS_CALLABLE || value.signature != 40) ls_value_mismatch();
ls_callable40 result; memcpy(&result, &value.as.c, sizeof result); return result;
}

static LS_NATIVE_UNUSED void ls_array0_acquire(ls_value value) { (void)value; ls_value_retain(value);
 }
static LS_NATIVE_UNUSED void ls_array0_drop(ls_value value) { (void)value; ls_value_release(value);
 }
static LS_NATIVE_UNUSED void ls_array0_retain_slot(const void *slot) { ls_value value=*(ls_value const *)slot; ls_array0_acquire(value); }
static LS_NATIVE_UNUSED void ls_array0_drop_slot(void *slot) { ls_value value=*(ls_value *)slot; ls_array0_drop(value); }
static LS_NATIVE_UNUSED void ls_array0_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_value value=*(ls_value const *)slot; (void)value; (void)visit; (void)context; ls_value_trace(value, visit, context);

}
static LS_NATIVE_UNUSED ls_value ls_array0_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_value value=*(ls_value const *)slot; return value;
}
static LS_NATIVE_UNUSED void ls_array0_write_slot(void *slot, ls_value input) {
ls_value value=input; ls_array0_acquire(value); ls_array0_drop(*(ls_value *)slot); *(ls_value *)slot=value;
}
static const ls_array_ops ls_array0_ops={sizeof(ls_value),ls_array0_retain_slot,ls_array0_drop_slot,ls_array0_trace_slot,ls_array0_read_slot,ls_array0_write_slot};
static LS_NATIVE_UNUSED ls_array0 *ls_array0_new(size_t capacity) { return ls_array_new(&ls_array0_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array0_hole(ls_array0 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_value ls_array0_absent(void) { return (ls_value){0}; }
static LS_NATIVE_UNUSED ls_value ls_array0_get(ls_array0 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array0_absent();
if(array->ops==&ls_array0_ops) return ((ls_value *)array->items)[index];
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED ls_value ls_array0_optional(ls_array0 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array0_push_owned(ls_array0 *array, ls_value value) {
if(array->ops==&ls_array0_ops) { *(ls_value *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array0_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array0_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array0_push(ls_array0 *array, ls_value value) {
ls_array0_acquire(value); return ls_array0_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array0_set(ls_array0 *array, int32_t index, ls_value value) {
if(array->ops==&ls_array0_ops) {
ls_array0_acquire(value); ls_value *slot=ls_array_store_slot(array,index); ls_array0_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array0_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_value ls_array0_pop(ls_array0 *array) {
if(!array->length) return ls_array0_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array0_absent(); }
if(array->ops==&ls_array0_ops) { ls_value result=((ls_value *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_value result=ls_array0_get(array,(int32_t)index,&ls_temps); ls_array0_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_slice(ls_array0 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_concat(ls_array0 *left, ls_array0 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_reverse(ls_array0 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_splice(ls_array0 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_copy_within(ls_array0 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_fill(ls_array0 *array, ls_value value) {
for(size_t i=0;i<array->length;++i) { ls_array0_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array0_copy(ls_array0 **slot, ls_array0 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array0_take(ls_array0 **slot, ls_array0 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array0_clear(ls_array0 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED int32_t ls_array0_index_of(ls_array0 *array, ls_value right) {
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {
ls_value left=ls_array0_get(array,(int32_t)i,&ls_temps); bool same=(ls_value_equal(left,right));
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}
return -1;
}
static LS_NATIVE_UNUSED bool ls_array0_includes(ls_array0 *array, ls_value right, int32_t start) {
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {
ls_value left=ls_array0_get(array,(int32_t)i,&ls_temps); bool same=(ls_value_same_zero(left,right));
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}
return false;
}

static LS_NATIVE_UNUSED void ls_array1_acquire(int32_t value) { (void)value;  }
static LS_NATIVE_UNUSED void ls_array1_drop(int32_t value) { (void)value;  }
static LS_NATIVE_UNUSED void ls_array1_retain_slot(const void *slot) { int32_t value=*(int32_t const *)slot; ls_array1_acquire(value); }
static LS_NATIVE_UNUSED void ls_array1_drop_slot(void *slot) { int32_t value=*(int32_t *)slot; ls_array1_drop(value); }
static LS_NATIVE_UNUSED void ls_array1_trace_slot(const void *slot, ls_native_visit visit, void *context) {
int32_t value=*(int32_t const *)slot; (void)value; (void)visit; (void)context; 
}
static LS_NATIVE_UNUSED ls_value ls_array1_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; int32_t value=*(int32_t const *)slot; return ls_value_int(value);
}
static LS_NATIVE_UNUSED void ls_array1_write_slot(void *slot, ls_value input) {
int32_t value=ls_value_to_int(input); ls_array1_acquire(value); ls_array1_drop(*(int32_t *)slot); *(int32_t *)slot=value;
}
static const ls_array_ops ls_array1_ops={sizeof(int32_t),ls_array1_retain_slot,ls_array1_drop_slot,ls_array1_trace_slot,ls_array1_read_slot,ls_array1_write_slot};
static LS_NATIVE_UNUSED ls_array1 *ls_array1_new(size_t capacity) { return ls_array_new(&ls_array1_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array1_hole(ls_array1 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED int32_t ls_array1_absent(void) { return 0; }
static LS_NATIVE_UNUSED int32_t ls_array1_get(ls_array1 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array1_absent();
if(array->ops==&ls_array1_ops) return ((int32_t *)array->items)[index];
return ls_value_to_int(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array1_optional(ls_array1 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array1_push_owned(ls_array1 *array, int32_t value) {
if(array->ops==&ls_array1_ops) { *(int32_t *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array1_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array1_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array1_push(ls_array1 *array, int32_t value) {
ls_array1_acquire(value); return ls_array1_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array1_set(ls_array1 *array, int32_t index, int32_t value) {
if(array->ops==&ls_array1_ops) {
ls_array1_acquire(value); int32_t *slot=ls_array_store_slot(array,index); ls_array1_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array1_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED int32_t ls_array1_pop(ls_array1 *array) {
if(!array->length) return ls_array1_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array1_absent(); }
if(array->ops==&ls_array1_ops) { int32_t result=((int32_t *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
int32_t result=ls_array1_get(array,(int32_t)index,&ls_temps); ls_array1_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_slice(ls_array1 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_concat(ls_array1 *left, ls_array1 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_reverse(ls_array1 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_splice(ls_array1 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_copy_within(ls_array1 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_fill(ls_array1 *array, int32_t value) {
for(size_t i=0;i<array->length;++i) { ls_array1_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array1_copy(ls_array1 **slot, ls_array1 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array1_take(ls_array1 **slot, ls_array1 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array1_clear(ls_array1 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED int32_t ls_array1_index_of(ls_array1 *array, int32_t right) {
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {
int32_t left=ls_array1_get(array,(int32_t)i,&ls_temps); bool same=(left==right);
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}
return -1;
}
static LS_NATIVE_UNUSED bool ls_array1_includes(ls_array1 *array, int32_t right, int32_t start) {
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {
int32_t left=ls_array1_get(array,(int32_t)i,&ls_temps); bool same=(left==right);
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}
return false;
}

static LS_NATIVE_UNUSED void ls_array2_acquire(ls_string value) { (void)value; ls_native_retain(value.owner);
 }
static LS_NATIVE_UNUSED void ls_array2_drop(ls_string value) { (void)value; ls_native_release(value.owner);
 }
static LS_NATIVE_UNUSED void ls_array2_retain_slot(const void *slot) { ls_string value=*(ls_string const *)slot; ls_array2_acquire(value); }
static LS_NATIVE_UNUSED void ls_array2_drop_slot(void *slot) { ls_string value=*(ls_string *)slot; ls_array2_drop(value); }
static LS_NATIVE_UNUSED void ls_array2_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_string value=*(ls_string const *)slot; (void)value; (void)visit; (void)context; visit(value.owner, context);

}
static LS_NATIVE_UNUSED ls_value ls_array2_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_string value=*(ls_string const *)slot; return ls_value_string(value);
}
static LS_NATIVE_UNUSED void ls_array2_write_slot(void *slot, ls_value input) {
ls_string value=ls_value_to_string(input); ls_array2_acquire(value); ls_array2_drop(*(ls_string *)slot); *(ls_string *)slot=value;
}
static const ls_array_ops ls_array2_ops={sizeof(ls_string),ls_array2_retain_slot,ls_array2_drop_slot,ls_array2_trace_slot,ls_array2_read_slot,ls_array2_write_slot};
static LS_NATIVE_UNUSED ls_array2 *ls_array2_new(size_t capacity) { return ls_array_new(&ls_array2_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array2_hole(ls_array2 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_string ls_array2_absent(void) { return (ls_string){0}; }
static LS_NATIVE_UNUSED ls_string ls_array2_get(ls_array2 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array2_absent();
if(array->ops==&ls_array2_ops) return ((ls_string *)array->items)[index];
return ls_value_to_string(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array2_optional(ls_array2 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array2_push_owned(ls_array2 *array, ls_string value) {
if(array->ops==&ls_array2_ops) { *(ls_string *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array2_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array2_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array2_push(ls_array2 *array, ls_string value) {
ls_array2_acquire(value); return ls_array2_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array2_set(ls_array2 *array, int32_t index, ls_string value) {
if(array->ops==&ls_array2_ops) {
ls_array2_acquire(value); ls_string *slot=ls_array_store_slot(array,index); ls_array2_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array2_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_string ls_array2_pop(ls_array2 *array) {
if(!array->length) return ls_array2_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array2_absent(); }
if(array->ops==&ls_array2_ops) { ls_string result=((ls_string *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_string result=ls_array2_get(array,(int32_t)index,&ls_temps); ls_array2_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_slice(ls_array2 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_concat(ls_array2 *left, ls_array2 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_reverse(ls_array2 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_splice(ls_array2 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_copy_within(ls_array2 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_fill(ls_array2 *array, ls_string value) {
for(size_t i=0;i<array->length;++i) { ls_array2_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array2_copy(ls_array2 **slot, ls_array2 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array2_take(ls_array2 **slot, ls_array2 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array2_clear(ls_array2 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED int32_t ls_array2_index_of(ls_array2 *array, ls_string right) {
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {
ls_string left=ls_array2_get(array,(int32_t)i,&ls_temps); bool same=(ls_string_equal(left,right));
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}
return -1;
}
static LS_NATIVE_UNUSED bool ls_array2_includes(ls_array2 *array, ls_string right, int32_t start) {
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {
ls_string left=ls_array2_get(array,(int32_t)i,&ls_temps); bool same=(ls_string_equal(left,right));
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}
return false;
}

static LS_NATIVE_UNUSED void ls_array3_acquire(ls_array1 * value) { (void)value; ls_native_retain(value);
 }
static LS_NATIVE_UNUSED void ls_array3_drop(ls_array1 * value) { (void)value; ls_native_release(value);
 }
static LS_NATIVE_UNUSED void ls_array3_retain_slot(const void *slot) { ls_array1 * value=*(ls_array1 * const *)slot; ls_array3_acquire(value); }
static LS_NATIVE_UNUSED void ls_array3_drop_slot(void *slot) { ls_array1 * value=*(ls_array1 * *)slot; ls_array3_drop(value); }
static LS_NATIVE_UNUSED void ls_array3_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_array1 * value=*(ls_array1 * const *)slot; (void)value; (void)visit; (void)context; visit(value, context);

}
static LS_NATIVE_UNUSED ls_value ls_array3_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_array1 * value=*(ls_array1 * const *)slot; return ls_value_array((ls_native_object *)value);
}
static LS_NATIVE_UNUSED void ls_array3_write_slot(void *slot, ls_value input) {
ls_array1 * value=(ls_array1 *)ls_value_to_array(input); ls_array3_acquire(value); ls_array3_drop(*(ls_array1 * *)slot); *(ls_array1 * *)slot=value;
}
static const ls_array_ops ls_array3_ops={sizeof(ls_array1 *),ls_array3_retain_slot,ls_array3_drop_slot,ls_array3_trace_slot,ls_array3_read_slot,ls_array3_write_slot};
static LS_NATIVE_UNUSED ls_array3 *ls_array3_new(size_t capacity) { return ls_array_new(&ls_array3_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array3_hole(ls_array3 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_array1 * ls_array3_absent(void) { ls_native_undefined_element(); return (ls_array1 *){0}; }
static LS_NATIVE_UNUSED ls_array1 * ls_array3_get(ls_array3 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array3_absent();
if(array->ops==&ls_array3_ops) return ((ls_array1 * *)array->items)[index];
return (ls_array1 *)ls_value_to_array(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array3_optional(ls_array3 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array3_push_owned(ls_array3 *array, ls_array1 * value) {
if(array->ops==&ls_array3_ops) { *(ls_array1 * *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array3_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array3_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array3_push(ls_array3 *array, ls_array1 * value) {
ls_array3_acquire(value); return ls_array3_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array3_set(ls_array3 *array, int32_t index, ls_array1 * value) {
if(array->ops==&ls_array3_ops) {
ls_array3_acquire(value); ls_array1 * *slot=ls_array_store_slot(array,index); ls_array3_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array3_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_array1 * ls_array3_pop(ls_array3 *array) {
if(!array->length) return ls_array3_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array3_absent(); }
if(array->ops==&ls_array3_ops) { ls_array1 * result=((ls_array1 * *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_array1 * result=ls_array3_get(array,(int32_t)index,&ls_temps); ls_array3_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array3 *ls_array3_slice(ls_array3 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array3 *ls_array3_concat(ls_array3 *left, ls_array3 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array3 *ls_array3_reverse(ls_array3 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array3 *ls_array3_splice(ls_array3 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array3 *ls_array3_copy_within(ls_array3 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array3 *ls_array3_fill(ls_array3 *array, ls_array1 * value) {
for(size_t i=0;i<array->length;++i) { ls_array3_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array3_copy(ls_array3 **slot, ls_array3 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array3_take(ls_array3 **slot, ls_array3 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array3_clear(ls_array3 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED int32_t ls_array3_index_of(ls_array3 *array, ls_array1 * right) {
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {
ls_array1 * left=ls_array3_get(array,(int32_t)i,&ls_temps); bool same=(left==right);
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}
return -1;
}
static LS_NATIVE_UNUSED bool ls_array3_includes(ls_array3 *array, ls_array1 * right, int32_t start) {
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {
ls_array1 * left=ls_array3_get(array,(int32_t)i,&ls_temps); bool same=(left==right);
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}
return false;
}

static LS_NATIVE_UNUSED void ls_array4_acquire(ls_t2 value) { (void)value; ls_t2_retain(value);
 }
static LS_NATIVE_UNUSED void ls_array4_drop(ls_t2 value) { (void)value; ls_t2_release(value);
 }
static LS_NATIVE_UNUSED void ls_array4_retain_slot(const void *slot) { ls_t2 value=*(ls_t2 const *)slot; ls_array4_acquire(value); }
static LS_NATIVE_UNUSED void ls_array4_drop_slot(void *slot) { ls_t2 value=*(ls_t2 *)slot; ls_array4_drop(value); }
static LS_NATIVE_UNUSED void ls_array4_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_t2 value=*(ls_t2 const *)slot; (void)value; (void)visit; (void)context; ls_t2_trace(value, visit, context);

}
static LS_NATIVE_UNUSED ls_value ls_array4_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_t2 value=*(ls_t2 const *)slot; return ls_t2_box(temps,value);
}
static LS_NATIVE_UNUSED void ls_array4_write_slot(void *slot, ls_value input) {
ls_t2 value=ls_value_to_t2(input); ls_array4_acquire(value); ls_array4_drop(*(ls_t2 *)slot); *(ls_t2 *)slot=value;
}
static const ls_array_ops ls_array4_ops={sizeof(ls_t2),ls_array4_retain_slot,ls_array4_drop_slot,ls_array4_trace_slot,ls_array4_read_slot,ls_array4_write_slot};
static LS_NATIVE_UNUSED ls_array4 *ls_array4_new(size_t capacity) { return ls_array_new(&ls_array4_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array4_hole(ls_array4 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_t2 ls_array4_absent(void) { ls_native_undefined_element(); return (ls_t2){0}; }
static LS_NATIVE_UNUSED ls_t2 ls_array4_get(ls_array4 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array4_absent();
if(array->ops==&ls_array4_ops) return ((ls_t2 *)array->items)[index];
return ls_value_to_t2(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array4_optional(ls_array4 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array4_push_owned(ls_array4 *array, ls_t2 value) {
if(array->ops==&ls_array4_ops) { *(ls_t2 *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array4_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array4_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array4_push(ls_array4 *array, ls_t2 value) {
ls_array4_acquire(value); return ls_array4_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array4_set(ls_array4 *array, int32_t index, ls_t2 value) {
if(array->ops==&ls_array4_ops) {
ls_array4_acquire(value); ls_t2 *slot=ls_array_store_slot(array,index); ls_array4_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array4_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_t2 ls_array4_pop(ls_array4 *array) {
if(!array->length) return ls_array4_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array4_absent(); }
if(array->ops==&ls_array4_ops) { ls_t2 result=((ls_t2 *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_t2 result=ls_array4_get(array,(int32_t)index,&ls_temps); ls_array4_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array4 *ls_array4_slice(ls_array4 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array4 *ls_array4_concat(ls_array4 *left, ls_array4 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array4 *ls_array4_reverse(ls_array4 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array4 *ls_array4_splice(ls_array4 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array4 *ls_array4_copy_within(ls_array4 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array4 *ls_array4_fill(ls_array4 *array, ls_t2 value) {
for(size_t i=0;i<array->length;++i) { ls_array4_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array4_copy(ls_array4 **slot, ls_array4 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array4_take(ls_array4 **slot, ls_array4 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array4_clear(ls_array4 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED void ls_array5_acquire(ls_t1 value) { (void)value; ls_t1_retain(value);
 }
static LS_NATIVE_UNUSED void ls_array5_drop(ls_t1 value) { (void)value; ls_t1_release(value);
 }
static LS_NATIVE_UNUSED void ls_array5_retain_slot(const void *slot) { ls_t1 value=*(ls_t1 const *)slot; ls_array5_acquire(value); }
static LS_NATIVE_UNUSED void ls_array5_drop_slot(void *slot) { ls_t1 value=*(ls_t1 *)slot; ls_array5_drop(value); }
static LS_NATIVE_UNUSED void ls_array5_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_t1 value=*(ls_t1 const *)slot; (void)value; (void)visit; (void)context; ls_t1_trace(value, visit, context);

}
static LS_NATIVE_UNUSED ls_value ls_array5_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_t1 value=*(ls_t1 const *)slot; return ls_t1_box(temps,value);
}
static LS_NATIVE_UNUSED void ls_array5_write_slot(void *slot, ls_value input) {
ls_t1 value=ls_value_to_t1(input); ls_array5_acquire(value); ls_array5_drop(*(ls_t1 *)slot); *(ls_t1 *)slot=value;
}
static const ls_array_ops ls_array5_ops={sizeof(ls_t1),ls_array5_retain_slot,ls_array5_drop_slot,ls_array5_trace_slot,ls_array5_read_slot,ls_array5_write_slot};
static LS_NATIVE_UNUSED ls_array5 *ls_array5_new(size_t capacity) { return ls_array_new(&ls_array5_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array5_hole(ls_array5 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_t1 ls_array5_absent(void) { ls_native_undefined_element(); return (ls_t1){0}; }
static LS_NATIVE_UNUSED ls_t1 ls_array5_get(ls_array5 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array5_absent();
if(array->ops==&ls_array5_ops) return ((ls_t1 *)array->items)[index];
return ls_value_to_t1(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array5_optional(ls_array5 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array5_push_owned(ls_array5 *array, ls_t1 value) {
if(array->ops==&ls_array5_ops) { *(ls_t1 *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array5_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array5_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array5_push(ls_array5 *array, ls_t1 value) {
ls_array5_acquire(value); return ls_array5_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array5_set(ls_array5 *array, int32_t index, ls_t1 value) {
if(array->ops==&ls_array5_ops) {
ls_array5_acquire(value); ls_t1 *slot=ls_array_store_slot(array,index); ls_array5_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array5_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_t1 ls_array5_pop(ls_array5 *array) {
if(!array->length) return ls_array5_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array5_absent(); }
if(array->ops==&ls_array5_ops) { ls_t1 result=((ls_t1 *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_t1 result=ls_array5_get(array,(int32_t)index,&ls_temps); ls_array5_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array5 *ls_array5_slice(ls_array5 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array5 *ls_array5_concat(ls_array5 *left, ls_array5 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array5 *ls_array5_reverse(ls_array5 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array5 *ls_array5_splice(ls_array5 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array5 *ls_array5_copy_within(ls_array5 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array5 *ls_array5_fill(ls_array5 *array, ls_t1 value) {
for(size_t i=0;i<array->length;++i) { ls_array5_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array5_copy(ls_array5 **slot, ls_array5 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array5_take(ls_array5 **slot, ls_array5 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array5_clear(ls_array5 **slot) { ls_native_release(*slot); *slot=NULL; }
static LS_NATIVE_UNUSED ls_array2 *ls_string_split(ls_string text, ls_string separator) {
ls_array2 *parts = ls_array2_new(0);
if (!separator.length) {
for (size_t index = 0; index < text.length; index++) ls_array2_push_owned(parts, ls_string_view(text, index, index + 1));
return parts;
}
if (!text.length) { ls_array2_push(parts, text); return parts; }
size_t start = 0;
for (size_t at = 0; at + separator.length <= text.length;) {
if (ls_string_matches(text, at, separator)) { ls_array2_push_owned(parts, ls_string_view(text, start, at)); at += separator.length; start = at; }
else at++;
}
ls_array2_push_owned(parts, ls_string_view(text, start, text.length));
return parts;
}
static LS_NATIVE_UNUSED void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef struct ls_object0 {
ls_native_object owner;
ls_array0 * ls_m0;
} ls_object0;
static LS_NATIVE_UNUSED void ls_object0_clear_fields(ls_object0 *object) {
(void)object;
ls_array0_clear(&object->ls_m0);
}
static LS_NATIVE_UNUSED void ls_object0_destroy(ls_native_object *owner) { ls_object0_clear_fields((ls_object0 *)owner); }
static LS_NATIVE_UNUSED void ls_object0_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_object0 *object = (ls_object0 *)owner; (void)object; (void)visit; (void)context;
visit(object->ls_m0, context);
}
typedef int32_t host_assertNativeCount_arg0;
typedef void host_assertNativeCount_result;
void host_assertNativeCount(int32_t);
typedef void host_collectNative_result;
void host_collectNative(void);
typedef int32_t host_assertMaxNativeCount_arg0;
typedef void host_assertMaxNativeCount_result;
void host_assertMaxNativeCount(int32_t);
typedef struct { ls_native_object owner; ls_array1 * value; bool ready; } ls_box54;
static LS_NATIVE_UNUSED void ls_box_destroy54(ls_native_object *owner) { ls_array1_clear(&((ls_box54 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_box_trace54(ls_native_object *owner, ls_native_visit visit, void *context) { visit(((ls_box54 *)owner)->value, context);
 }
static LS_NATIVE_UNUSED ls_box54 *ls_box_allocate54(void) { return ls_native_allocate(sizeof(ls_box54),ls_box_destroy54,ls_box_trace54); }
static LS_NATIVE_UNUSED void ls_box_initialize54(ls_box54 *box, ls_array1 * value) {
ls_native_retain(value);
ls_native_release(box->value);
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box54 *ls_box_new54(ls_array1 * value) { ls_box54 *box = ls_box_allocate54(); ls_box_initialize54(box, value); return box; }
static LS_NATIVE_UNUSED ls_array1 * *ls_box_value54(ls_box54 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct { ls_native_object owner; int32_t value; bool ready; } ls_box57;
static LS_NATIVE_UNUSED ls_box57 *ls_box_allocate57(void) { return ls_native_allocate(sizeof(ls_box57),NULL,NULL); }
static LS_NATIVE_UNUSED void ls_box_initialize57(ls_box57 *box, int32_t value) {
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box57 *ls_box_new57(int32_t value) { ls_box57 *box = ls_box_allocate57(); ls_box_initialize57(box, value); return box; }
static LS_NATIVE_UNUSED int32_t *ls_box_value57(ls_box57 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct {
ls_native_object owner;
ls_box54 *ls_e0;
} ls_env18;
static LS_NATIVE_UNUSED void ls_env_destroy18(ls_native_object *owner) {
ls_env18 *environment = (ls_env18 *)owner;
ls_native_release(environment->ls_e0);
}
static LS_NATIVE_UNUSED void ls_env_trace18(ls_native_object *owner, ls_native_visit visit, void *context) {
ls_env18 *environment = (ls_env18 *)owner;
visit(environment->ls_e0, context);
}
typedef struct {
ls_native_object owner;
ls_box57 *ls_e0;
} ls_env19;
static LS_NATIVE_UNUSED void ls_env_destroy19(ls_native_object *owner) {
ls_env19 *environment = (ls_env19 *)owner;
ls_native_release(environment->ls_e0);
}
static LS_NATIVE_UNUSED void ls_env_trace19(ls_native_object *owner, ls_native_visit visit, void *context) {
ls_env19 *environment = (ls_env19 *)owner;
visit(environment->ls_e0, context);
}
static LS_NATIVE_UNUSED const uint16_t ls_s16[] = {124,};
static LS_NATIVE_UNUSED const uint16_t ls_s17[] = {111,110,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s18[] = {116,119,111,};
static LS_NATIVE_UNUSED const uint16_t ls_s19[] = {102,105,108,108,101,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s20[] = {108,101,102,116,};
static LS_NATIVE_UNUSED const uint16_t ls_s21[] = {114,105,103,104,116,};
static LS_NATIVE_UNUSED const uint16_t ls_s22[] = {105,110,115,105,100,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s23[] = {99,111,112,121,};
static LS_NATIVE_UNUSED const uint16_t ls_s24[] = {115,116,97,114,116,};
static LS_NATIVE_UNUSED const uint16_t ls_s25[] = {97,114,114,97,121,115,32,100,111,110,101,};
static LS_NATIVE_UNUSED void ls_init0(void);
static LS_NATIVE_UNUSED ls_value ls_fn1(ls_value ls_c17 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_array0 * ls_fn2(ls_array0 * ls_c18 LS_NATIVE_UNUSED,ls_value ls_c19 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_array0 * ls_fn3(ls_value ls_c20 LS_NATIVE_UNUSED,ls_value ls_c21 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_value ls_fn4(ls_array0 * ls_c22 LS_NATIVE_UNUSED,int32_t ls_c23 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn5(ls_array0 * ls_c24 LS_NATIVE_UNUSED,int32_t ls_c25 LS_NATIVE_UNUSED,ls_value ls_c26 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_array0 * ls_fn6(ls_array0 * ls_c27 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_array0 * ls_fn7(ls_array0 * ls_c29 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn8(ls_native_object * ls_c30 LS_NATIVE_UNUSED,ls_array0 * ls_c31 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_value ls_fn9(ls_native_object * ls_c32 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn10(ls_native_object * ls_c33 LS_NATIVE_UNUSED,ls_value ls_c34 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED bool ls_fn11(ls_value ls_c35 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED bool ls_fn12(ls_array0 * ls_c36 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn13(void);
static LS_NATIVE_UNUSED void ls_fn14(void);
static LS_NATIVE_UNUSED ls_value ls_fn15(void *ls_env LS_NATIVE_UNUSED,ls_value ls_c28 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED bool ls_fn16(void *ls_env LS_NATIVE_UNUSED,ls_value ls_c37 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_t2 ls_fn17(void *ls_env LS_NATIVE_UNUSED,ls_t2 ls_c52 LS_NATIVE_UNUSED,ls_t2 ls_c53 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED int32_t ls_fn18(void *ls_env LS_NATIVE_UNUSED,int32_t ls_c56 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn19(void *ls_env LS_NATIVE_UNUSED,int32_t ls_c58 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_callable15 ls_closure15(void) {
return (ls_callable15){ls_fn15,NULL,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED ls_callable18 ls_closure16(void) {
return (ls_callable18){ls_fn16,NULL,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED ls_callable40 ls_closure17(void) {
return (ls_callable40){ls_fn17,NULL,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED ls_callable27 ls_closure18(ls_box54 *ls_e0) {
ls_env18 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy18,ls_env_trace18);
ls_native_retain(ls_e0);
environment->ls_e0 = ls_e0;
return (ls_callable27){ls_fn18,environment,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED ls_callable1 ls_closure19(ls_box57 *ls_e0) {
ls_env19 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy19,ls_env_trace19);
ls_native_retain(ls_e0);
environment->ls_e0 = ls_e0;
return (ls_callable1){ls_fn19,environment,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED void ls_init0(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_v19 LS_NATIVE_UNUSED;
int32_t ls_v26 LS_NATIVE_UNUSED;
ls_string ls_v28 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn13();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
host_collectNative();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v19 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
host_assertNativeCount(ls_v19);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn14();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
host_collectNative();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v26 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
host_assertNativeCount(ls_v26);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v28),(ls_string){ls_s25,sizeof ls_s25/sizeof *ls_s25,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v28);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v28);
}
static LS_NATIVE_UNUSED ls_value ls_fn1(ls_value ls_c17 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c17);
ls_value ls_v0 LS_NATIVE_UNUSED = {0};
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value_copy(&(ls_v0),ls_c17);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_v0);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_v1;
ls_value_retain(ls_return);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c17);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c17);
}
static LS_NATIVE_UNUSED ls_array0 * ls_fn2(ls_array0 * ls_c18 LS_NATIVE_UNUSED,ls_value ls_c19 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c18);
ls_value_retain(ls_c19);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_array0 * ls_v4 LS_NATIVE_UNUSED = NULL;
ls_array0_copy(&(ls_v0),ls_c18);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_c19);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_array0_push(ls_v0,ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v4),ls_c18);
ls_native_temporaries_clear(&ls_temps);
{
ls_array0 * ls_return = ls_v4;
ls_native_retain(ls_return);
ls_array0_clear(&ls_v4);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_value_clear(&ls_c19);
ls_array0_clear(&ls_c18);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_array0_clear(&ls_v4);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_value_clear(&ls_c19);
ls_array0_clear(&ls_c18);
}
static LS_NATIVE_UNUSED ls_array0 * ls_fn3(ls_value ls_c20 LS_NATIVE_UNUSED,ls_value ls_c21 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c20);
ls_value_retain(ls_c21);
ls_value ls_v0 LS_NATIVE_UNUSED = {0};
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v4 LS_NATIVE_UNUSED = NULL;
ls_value_copy(&(ls_v0),ls_c20);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_v0);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_c21);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v3),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array0_take(&(ls_v4),ls_array0_new(2));
ls_array0_push(ls_v4,ls_v1);
ls_array0_push(ls_v4,ls_v3);
ls_native_temporaries_clear(&ls_temps);
{
ls_array0 * ls_return = ls_v4;
ls_native_retain(ls_return);
ls_array0_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c21);
ls_value_clear(&ls_c20);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_array0_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c21);
ls_value_clear(&ls_c20);
}
static LS_NATIVE_UNUSED ls_value ls_fn4(ls_array0 * ls_c22 LS_NATIVE_UNUSED,int32_t ls_c23 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c22);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_array0_copy(&(ls_v0),ls_c22);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_c23;
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_array0_get(ls_v0,ls_v1,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v3),ls_v2);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_v3;
ls_value_retain(ls_return);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c22);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c22);
}
static LS_NATIVE_UNUSED void ls_fn5(ls_array0 * ls_c24 LS_NATIVE_UNUSED,int32_t ls_c25 LS_NATIVE_UNUSED,ls_value ls_c26 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c24);
ls_value_retain(ls_c26);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_array0_copy(&(ls_v0),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_c25;
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_c26);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v3),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array0_set(ls_v0,ls_v1,ls_v3);
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_array0_clear(&ls_v0);
ls_value_clear(&ls_c26);
ls_array0_clear(&ls_c24);
}
static LS_NATIVE_UNUSED ls_array0 * ls_fn6(ls_array0 * ls_c27 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c27);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_callable15 ls_v1 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v2 LS_NATIVE_UNUSED = NULL;
ls_array0_copy(&(ls_v0),ls_c27);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable15_take(&(ls_v1),ls_closure15());
ls_native_temporaries_clear(&ls_temps);
{ ls_array0 *ls_src=ls_v0; size_t ls_len=ls_src->length;
ls_array0 *ls_out=ls_array0_new(ls_len);
for(size_t ls_k=0;ls_k<ls_len;++ls_k) {
if(!ls_array_has(ls_src,ls_k)) {ls_array0_hole(ls_out);continue;}
ls_value ls_item = ls_array0_get(ls_src,(int32_t)ls_k,&ls_temps);
ls_value_retain(ls_item);
ls_value ls_mapped = ls_v1.code(ls_v1.environment,ls_item);
ls_array0_push_owned(ls_out,ls_mapped);
ls_value_release(ls_item);
ls_native_temporaries_clear(&ls_temps);
}
ls_array0_take(&(ls_v2),ls_out);
}
ls_native_temporaries_clear(&ls_temps);
{
ls_array0 * ls_return = ls_v2;
ls_native_retain(ls_return);
ls_array0_clear(&ls_v2);
ls_callable15_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c27);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_array0_clear(&ls_v2);
ls_callable15_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c27);
}
static LS_NATIVE_UNUSED ls_array0 * ls_fn7(ls_array0 * ls_c29 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c29);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v1 LS_NATIVE_UNUSED = NULL;
ls_array0_copy(&(ls_v0),ls_c29);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v1),ls_array0_reverse(ls_v0));
ls_native_temporaries_clear(&ls_temps);
{
ls_array0 * ls_return = ls_v1;
ls_native_retain(ls_return);
ls_array0_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c29);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_array0_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c29);
}
static LS_NATIVE_UNUSED void ls_fn8(ls_native_object * ls_c30 LS_NATIVE_UNUSED,ls_array0 * ls_c31 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c30);
ls_native_retain(ls_c31);
ls_native_object * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v1 LS_NATIVE_UNUSED = NULL;
ls_object_copy(&(ls_v0),ls_c30);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v1),ls_c31);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(((ls_object0 *)ls_v0)->ls_m0),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_array0_clear(&ls_v1);
ls_object_clear(&ls_v0);
ls_array0_clear(&ls_c31);
ls_object_clear(&ls_c30);
}
static LS_NATIVE_UNUSED ls_value ls_fn9(ls_native_object * ls_c32 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c32);
ls_native_object * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v1 LS_NATIVE_UNUSED = NULL;
int32_t ls_v2 LS_NATIVE_UNUSED;
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_value ls_v4 LS_NATIVE_UNUSED = {0};
ls_object_copy(&(ls_v0),ls_c32);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v1),((ls_object0 *)ls_v0)->ls_m0);
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v3),ls_array0_get(ls_v1,ls_v2,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v4),ls_v3);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_v4;
ls_value_retain(ls_return);
ls_value_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_array0_clear(&ls_v1);
ls_object_clear(&ls_v0);
ls_object_clear(&ls_c32);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_array0_clear(&ls_v1);
ls_object_clear(&ls_v0);
ls_object_clear(&ls_c32);
}
static LS_NATIVE_UNUSED void ls_fn10(ls_native_object * ls_c33 LS_NATIVE_UNUSED,ls_value ls_c34 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c33);
ls_value_retain(ls_c34);
ls_native_object * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v1 LS_NATIVE_UNUSED = NULL;
int32_t ls_v2 LS_NATIVE_UNUSED;
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_value ls_v4 LS_NATIVE_UNUSED = {0};
ls_object_copy(&(ls_v0),ls_c33);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v1),((ls_object0 *)ls_v0)->ls_m0);
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v3),ls_c34);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v4),ls_v3);
ls_native_temporaries_clear(&ls_temps);
ls_array0_set(ls_v1,ls_v2,ls_v4);
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_array0_clear(&ls_v1);
ls_object_clear(&ls_v0);
ls_value_clear(&ls_c34);
ls_object_clear(&ls_c33);
}
static LS_NATIVE_UNUSED bool ls_fn11(ls_value ls_c35 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c35);
int32_t ls_v1 LS_NATIVE_UNUSED;
bool ls_v3 LS_NATIVE_UNUSED;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(64));
ls_native_temporaries_clear(&ls_temps);
host_assertMaxNativeCount(ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_v3 = true;
ls_native_temporaries_clear(&ls_temps);
{
bool ls_return = ls_v3;
ls_value_clear(&ls_c35);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_c35);
}
static LS_NATIVE_UNUSED bool ls_fn12(ls_array0 * ls_c36 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c36);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_callable18 ls_v1 LS_NATIVE_UNUSED = {0};
bool ls_v2 LS_NATIVE_UNUSED;
ls_array0_copy(&(ls_v0),ls_c36);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable18_take(&(ls_v1),ls_closure16());
ls_native_temporaries_clear(&ls_temps);
{ ls_array0 *ls_src=ls_v0; size_t ls_len=ls_src->length;
bool ls_found=true;
for(size_t ls_k=0;ls_k<ls_len;++ls_k) {
if(!ls_array_has(ls_src,ls_k)) continue;
ls_value ls_item = ls_array0_get(ls_src,(int32_t)ls_k,&ls_temps);
ls_value_retain(ls_item);
bool ls_predicate_result = ls_v1.code(ls_v1.environment,ls_item);
bool ls_predicate = ls_predicate_result;
ls_value_release(ls_item);
ls_native_temporaries_clear(&ls_temps);
if(!ls_predicate) { ls_found=false; break; }
}
ls_v2=ls_found;
}
ls_native_temporaries_clear(&ls_temps);
{
bool ls_return = ls_v2;
ls_callable18_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c36);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_callable18_clear(&ls_v1);
ls_array0_clear(&ls_v0);
ls_array0_clear(&ls_c36);
}
static LS_NATIVE_UNUSED void ls_fn13(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_c38 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_c39 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c40 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_c41 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_c42 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_c43 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_c44 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_c45 LS_NATIVE_UNUSED = NULL;
ls_array2 * ls_c46 LS_NATIVE_UNUSED = NULL;
ls_array3 * ls_c47 LS_NATIVE_UNUSED = NULL;
ls_array4 * ls_c48 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_c49 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_c50 LS_NATIVE_UNUSED = NULL;
ls_t2 ls_c51 LS_NATIVE_UNUSED = {0};
ls_box54 *ls_c54 = NULL;
ls_array1 * ls_c55 LS_NATIVE_UNUSED = NULL;
ls_box57 *ls_c57 = NULL;
int32_t ls_v0 LS_NATIVE_UNUSED;
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_array1 * ls_v2 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v4 LS_NATIVE_UNUSED = NULL;
int32_t ls_v5 LS_NATIVE_UNUSED;
ls_array1 * ls_v6 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v7 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v8 LS_NATIVE_UNUSED = NULL;
bool ls_v9 LS_NATIVE_UNUSED;
ls_array1 * ls_v12 LS_NATIVE_UNUSED = NULL;
int32_t ls_v13 LS_NATIVE_UNUSED;
int32_t ls_v14 LS_NATIVE_UNUSED;
ls_array1 * ls_v17 LS_NATIVE_UNUSED = NULL;
int32_t ls_v18 LS_NATIVE_UNUSED;
int32_t ls_v19 LS_NATIVE_UNUSED;
ls_array1 * ls_v21 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v22 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v23 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v26 LS_NATIVE_UNUSED = NULL;
int32_t ls_v28 LS_NATIVE_UNUSED;
ls_array1 * ls_v30 LS_NATIVE_UNUSED = NULL;
ls_string ls_v31 LS_NATIVE_UNUSED = {0};
ls_string ls_v32 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v34 LS_NATIVE_UNUSED = NULL;
int32_t ls_v36 LS_NATIVE_UNUSED;
ls_array1 * ls_v38 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_v39 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v40 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v41 LS_NATIVE_UNUSED = NULL;
int32_t ls_v42 LS_NATIVE_UNUSED;
int32_t ls_v43 LS_NATIVE_UNUSED;
ls_array1 * ls_v44 LS_NATIVE_UNUSED = NULL;
int32_t ls_v45 LS_NATIVE_UNUSED;
int32_t ls_v48 LS_NATIVE_UNUSED;
int32_t ls_v49 LS_NATIVE_UNUSED;
ls_array1 * ls_v50 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v51 LS_NATIVE_UNUSED = NULL;
int32_t ls_v52 LS_NATIVE_UNUSED;
int32_t ls_v53 LS_NATIVE_UNUSED;
ls_array1 * ls_v55 LS_NATIVE_UNUSED = NULL;
int32_t ls_v56 LS_NATIVE_UNUSED;
int32_t ls_v57 LS_NATIVE_UNUSED;
ls_array1 * ls_v59 LS_NATIVE_UNUSED = NULL;
ls_string ls_v60 LS_NATIVE_UNUSED = {0};
ls_string ls_v61 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v64 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v65 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v66 LS_NATIVE_UNUSED = NULL;
ls_string ls_v67 LS_NATIVE_UNUSED = {0};
ls_string ls_v68 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v70 LS_NATIVE_UNUSED = NULL;
int32_t ls_v71 LS_NATIVE_UNUSED;
int32_t ls_v72 LS_NATIVE_UNUSED;
ls_array1 * ls_v73 LS_NATIVE_UNUSED = NULL;
int32_t ls_v74 LS_NATIVE_UNUSED;
int32_t ls_v75 LS_NATIVE_UNUSED;
ls_array1 * ls_v78 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v79 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v80 LS_NATIVE_UNUSED = NULL;
bool ls_v81 LS_NATIVE_UNUSED;
ls_array1 * ls_v83 LS_NATIVE_UNUSED = NULL;
ls_string ls_v84 LS_NATIVE_UNUSED = {0};
ls_string ls_v85 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v87 LS_NATIVE_UNUSED = NULL;
int32_t ls_v88 LS_NATIVE_UNUSED;
bool ls_v89 LS_NATIVE_UNUSED;
ls_array1 * ls_v91 LS_NATIVE_UNUSED = NULL;
int32_t ls_v92 LS_NATIVE_UNUSED;
int32_t ls_v93 LS_NATIVE_UNUSED;
ls_array1 * ls_v95 LS_NATIVE_UNUSED = NULL;
int32_t ls_v96 LS_NATIVE_UNUSED;
int32_t ls_v97 LS_NATIVE_UNUSED;
int32_t ls_v98 LS_NATIVE_UNUSED;
ls_array1 * ls_v99 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v100 LS_NATIVE_UNUSED = NULL;
ls_string ls_v101 LS_NATIVE_UNUSED = {0};
ls_string ls_v102 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v104 LS_NATIVE_UNUSED = NULL;
int32_t ls_v105 LS_NATIVE_UNUSED;
int32_t ls_v106 LS_NATIVE_UNUSED;
ls_array1 * ls_v107 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v108 LS_NATIVE_UNUSED = NULL;
ls_string ls_v109 LS_NATIVE_UNUSED = {0};
ls_string ls_v110 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v112 LS_NATIVE_UNUSED = NULL;
ls_string ls_v113 LS_NATIVE_UNUSED = {0};
ls_string ls_v114 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v116 LS_NATIVE_UNUSED = NULL;
int32_t ls_v117 LS_NATIVE_UNUSED;
ls_array1 * ls_v118 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v119 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v120 LS_NATIVE_UNUSED = NULL;
ls_string ls_v121 LS_NATIVE_UNUSED = {0};
ls_string ls_v122 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v124 LS_NATIVE_UNUSED = NULL;
int32_t ls_v125 LS_NATIVE_UNUSED;
ls_array1 * ls_v126 LS_NATIVE_UNUSED = NULL;
ls_string ls_v127 LS_NATIVE_UNUSED = {0};
ls_string ls_v128 LS_NATIVE_UNUSED = {0};
ls_string ls_v131 LS_NATIVE_UNUSED = {0};
int32_t ls_v132 LS_NATIVE_UNUSED;
ls_string ls_v133 LS_NATIVE_UNUSED = {0};
ls_string ls_v134 LS_NATIVE_UNUSED = {0};
int32_t ls_v135 LS_NATIVE_UNUSED;
ls_string ls_v136 LS_NATIVE_UNUSED = {0};
ls_array2 * ls_v137 LS_NATIVE_UNUSED = NULL;
ls_array2 * ls_v138 LS_NATIVE_UNUSED = NULL;
ls_string ls_v139 LS_NATIVE_UNUSED = {0};
int32_t ls_v140 LS_NATIVE_UNUSED;
ls_string ls_v141 LS_NATIVE_UNUSED = {0};
ls_array2 * ls_v142 LS_NATIVE_UNUSED = NULL;
ls_array2 * ls_v143 LS_NATIVE_UNUSED = NULL;
ls_string ls_v144 LS_NATIVE_UNUSED = {0};
ls_array2 * ls_v146 LS_NATIVE_UNUSED = NULL;
int32_t ls_v147 LS_NATIVE_UNUSED;
ls_string ls_v148 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v151 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v152 LS_NATIVE_UNUSED = NULL;
ls_array3 * ls_v153 LS_NATIVE_UNUSED = NULL;
ls_array3 * ls_v155 LS_NATIVE_UNUSED = NULL;
int32_t ls_v156 LS_NATIVE_UNUSED;
ls_array1 * ls_v157 LS_NATIVE_UNUSED = NULL;
ls_array3 * ls_v159 LS_NATIVE_UNUSED = NULL;
int32_t ls_v160 LS_NATIVE_UNUSED;
ls_array1 * ls_v161 LS_NATIVE_UNUSED = NULL;
int32_t ls_v162 LS_NATIVE_UNUSED;
int32_t ls_v163 LS_NATIVE_UNUSED;
ls_array1 * ls_v164 LS_NATIVE_UNUSED = NULL;
int32_t ls_v165 LS_NATIVE_UNUSED;
ls_array3 * ls_v167 LS_NATIVE_UNUSED = NULL;
int32_t ls_v168 LS_NATIVE_UNUSED;
ls_array1 * ls_v169 LS_NATIVE_UNUSED = NULL;
ls_array3 * ls_v170 LS_NATIVE_UNUSED = NULL;
int32_t ls_v171 LS_NATIVE_UNUSED;
ls_array1 * ls_v172 LS_NATIVE_UNUSED = NULL;
bool ls_v173 LS_NATIVE_UNUSED;
ls_string ls_v175 LS_NATIVE_UNUSED = {0};
int32_t ls_v176 LS_NATIVE_UNUSED;
ls_string ls_v177 LS_NATIVE_UNUSED = {0};
int32_t ls_v178 LS_NATIVE_UNUSED;
ls_array1 * ls_v179 LS_NATIVE_UNUSED = NULL;
ls_t2 ls_v180 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v181 LS_NATIVE_UNUSED = {0};
ls_string ls_v182 LS_NATIVE_UNUSED = {0};
int32_t ls_v183 LS_NATIVE_UNUSED;
ls_string ls_v184 LS_NATIVE_UNUSED = {0};
int32_t ls_v185 LS_NATIVE_UNUSED;
ls_array1 * ls_v186 LS_NATIVE_UNUSED = NULL;
ls_t2 ls_v187 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v188 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v189 LS_NATIVE_UNUSED = NULL;
ls_array4 * ls_v190 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_v191 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v192 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v193 LS_NATIVE_UNUSED = NULL;
int32_t ls_v194 LS_NATIVE_UNUSED;
ls_string ls_v195 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v196 LS_NATIVE_UNUSED = NULL;
int32_t ls_v197 LS_NATIVE_UNUSED;
ls_string ls_v198 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v201 LS_NATIVE_UNUSED = NULL;
ls_array4 * ls_v202 LS_NATIVE_UNUSED = NULL;
ls_array4 * ls_v203 LS_NATIVE_UNUSED = NULL;
int32_t ls_v204 LS_NATIVE_UNUSED;
ls_string ls_v205 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v206 LS_NATIVE_UNUSED = NULL;
int32_t ls_v207 LS_NATIVE_UNUSED;
ls_string ls_v208 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v210 LS_NATIVE_UNUSED = NULL;
int32_t ls_v211 LS_NATIVE_UNUSED;
ls_string ls_v212 LS_NATIVE_UNUSED = {0};
ls_array4 * ls_v214 LS_NATIVE_UNUSED = NULL;
ls_callable40 ls_v215 LS_NATIVE_UNUSED = {0};
ls_string ls_v216 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v217 LS_NATIVE_UNUSED = NULL;
ls_t2 ls_v218 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v219 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v220 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v221 LS_NATIVE_UNUSED = {0};
ls_string ls_v222 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v224 LS_NATIVE_UNUSED = NULL;
int32_t ls_v225 LS_NATIVE_UNUSED;
int32_t ls_v227 LS_NATIVE_UNUSED;
int32_t ls_v228 LS_NATIVE_UNUSED;
int32_t ls_v229 LS_NATIVE_UNUSED;
ls_array1 * ls_v230 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v231 LS_NATIVE_UNUSED = NULL;
ls_callable27 ls_v232 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v233 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v234 LS_NATIVE_UNUSED = NULL;
ls_string ls_v235 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v237 LS_NATIVE_UNUSED = NULL;
int32_t ls_v238 LS_NATIVE_UNUSED;
int32_t ls_v239 LS_NATIVE_UNUSED;
int32_t ls_v240 LS_NATIVE_UNUSED;
bool ls_v241 LS_NATIVE_UNUSED;
bool ls_v242 LS_NATIVE_UNUSED;
bool ls_v243 LS_NATIVE_UNUSED;
ls_value ls_v244 LS_NATIVE_UNUSED = {0};
ls_value ls_v245 LS_NATIVE_UNUSED = {0};
ls_value ls_v246 LS_NATIVE_UNUSED = {0};
ls_value ls_v247 LS_NATIVE_UNUSED = {0};
bool ls_v248 LS_NATIVE_UNUSED;
ls_array1 * ls_v250 LS_NATIVE_UNUSED = NULL;
int32_t ls_v251 LS_NATIVE_UNUSED;
int32_t ls_v252 LS_NATIVE_UNUSED;
ls_array1 * ls_v254 LS_NATIVE_UNUSED = NULL;
int32_t ls_v255 LS_NATIVE_UNUSED;
bool ls_v256 LS_NATIVE_UNUSED;
int32_t ls_v258 LS_NATIVE_UNUSED;
ls_array1 * ls_v259 LS_NATIVE_UNUSED = NULL;
ls_callable1 ls_v260 LS_NATIVE_UNUSED = {0};
int32_t ls_v262 LS_NATIVE_UNUSED;
ls_c54 = ls_box_allocate54();
ls_c57 = ls_box_allocate57();
ls_v0 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v2),ls_array1_new(2));
ls_array1_push(ls_v2,ls_v0);
ls_array1_push(ls_v2,ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c38),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v4),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_v5 = ls_from_u32(UINT32_C(3));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v6),ls_fn2(ls_v4,ls_value_int(ls_v5)));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c39),ls_v6);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v7),ls_c39);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v8),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_v9 = ls_v7 == ls_v8;
ls_native_temporaries_clear(&ls_temps);
puts(ls_v9 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v12),ls_c39);
ls_native_temporaries_clear(&ls_temps);
ls_v13 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v14 = ls_from_u32(UINT32_C(7));
ls_native_temporaries_clear(&ls_temps);
ls_fn5(ls_v12,ls_v13,ls_value_int(ls_v14));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v17),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_v18 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v19 = ls_value_to_int(ls_fn4(ls_v17,ls_v18));
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v19);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v21),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_array0_take(&(ls_v22),ls_array0_new(0));
ls_native_temporaries_clear(&ls_temps);
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy, ls_object0_trace);
((ls_object0 *)ls_o)->ls_m0 = ls_v22;
ls_native_retain(((ls_object0 *)ls_o)->ls_m0);
ls_object_take(&(ls_v23),(ls_native_object *)ls_o);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn8(ls_v23,ls_v21);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_c40),ls_v23);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v26),ls_c40);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v28 = ls_from_u32(UINT32_C(9));
ls_native_temporaries_clear(&ls_temps);
ls_fn10(ls_v26,ls_value_int(ls_v28));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v30),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v31),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v30->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v31);
if(ls_array_has(ls_v30,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v30,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v32),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v32);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v34),ls_c40);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v36 = ls_value_to_int(ls_fn9(ls_v34));
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v36);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v38),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v39),(ls_t0){ls_v38});
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v40),ls_v39);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c41),ls_v40);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v41),ls_c41.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v42 = ls_from_u32(UINT32_C(4));
ls_native_temporaries_clear(&ls_temps);
ls_v43 = ls_array1_push(ls_v41,ls_v42);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v44),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_v45 = (int32_t)ls_v44->length;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v45);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v48 = ls_from_u32(UINT32_C(5));
ls_native_temporaries_clear(&ls_temps);
ls_v49 = ls_from_u32(UINT32_C(6));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v50),ls_fn3(ls_value_int(ls_v48),ls_value_int(ls_v49)));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c42),ls_v50);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v51),ls_c42);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v52 = ls_from_u32(UINT32_C(7));
ls_native_temporaries_clear(&ls_temps);
ls_v53 = ls_array1_push(ls_v51,ls_v52);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v55),ls_c42);
ls_native_temporaries_clear(&ls_temps);
ls_v56 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v57 = ls_from_u32(UINT32_C(8));
ls_native_temporaries_clear(&ls_temps);
ls_fn5(ls_v55,ls_v56,ls_value_int(ls_v57));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v59),ls_c42);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v60),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v59->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v60);
if(ls_array_has(ls_v59,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v59,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v61),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v61);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v64),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v65),ls_fn6(ls_v64));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c43),ls_v65);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v66),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v67),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v66->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v67);
if(ls_array_has(ls_v66,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v66,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v68),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v68);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v70),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_v71 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v72 = ls_from_u32(UINT32_C(11));
ls_native_temporaries_clear(&ls_temps);
ls_array1_set(ls_v70,ls_v71,ls_v72);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v73),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_v74 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v75 = ls_array1_get(ls_v73,ls_v74,&ls_temps);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v75);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v78),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v79),ls_fn7(ls_v78));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v80),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_v81 = ls_v79 == ls_v80;
ls_native_temporaries_clear(&ls_temps);
puts(ls_v81 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v83),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v84),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v83->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v84);
if(ls_array_has(ls_v83,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v83,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v85),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v85);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v87),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v88 = ls_from_u32(UINT32_C(11));
ls_native_temporaries_clear(&ls_temps);
ls_v89=ls_array1_includes(ls_v87,ls_v88,0);
ls_native_temporaries_clear(&ls_temps);
puts(ls_v89 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v91),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v92 = ls_from_u32(UINT32_C(3));
ls_native_temporaries_clear(&ls_temps);
ls_v93=ls_array1_index_of(ls_v91,ls_v92);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v93);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v95),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v96 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v97 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v98 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v99),ls_array1_copy_within(ls_v95,ls_v96,ls_v97,true,ls_v98));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v100),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v101),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v100->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v101);
if(ls_array_has(ls_v100,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v100,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v102),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v102);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v104),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v105 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v106 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v107),ls_array1_splice(ls_v104,ls_v105,ls_v106));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c44),ls_v107);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v108),ls_c44);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v109),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v108->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v109);
if(ls_array_has(ls_v108,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v108,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v110),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v110);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v112),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v113),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v112->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v113);
if(ls_array_has(ls_v112,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v112,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v114),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v114);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v116),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v117 = ls_from_u32(UINT32_C(12));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v118),ls_array1_new(1));
ls_array1_push(ls_v118,ls_v117);
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v119),ls_array1_concat(ls_v116,ls_v118));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c45),ls_v119);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v120),ls_c45);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v121),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v120->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v121);
if(ls_array_has(ls_v120,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v120,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v122),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v122);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v124),ls_c45);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v125 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v126),ls_array1_slice(ls_v124,ls_array_relative(ls_v125,ls_v124->length),ls_v124->length));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v127),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_join={0};
for(size_t ls_k=0;ls_k<ls_v126->length;++ls_k) {
if(ls_k) ls_string_builder_text(&ls_join,ls_v127);
if(ls_array_has(ls_v126,ls_k)) ls_string_builder_value(&ls_join,ls_value_int(ls_array1_get(ls_v126,(int32_t)ls_k,&ls_temps)));
ls_native_temporaries_clear(&ls_temps);
}
ls_string_take(&(ls_v128),ls_string_builder_finish(&ls_join));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v128);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v131),(ls_string){ls_s17,sizeof ls_s17/sizeof *ls_s17,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v132 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v133),ls_string_repeat(ls_v131,ls_v132));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v134),(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v135 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v136),ls_string_repeat(ls_v134,ls_v135));
ls_native_temporaries_clear(&ls_temps);
ls_array2_take(&(ls_v137),ls_fn3(ls_value_string(ls_v133),ls_value_string(ls_v136)));
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_c46),ls_v137);
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_v138),ls_c46);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v139),(ls_string){ls_s19,sizeof ls_s19/sizeof *ls_s19,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v140 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v141),ls_string_repeat(ls_v139,ls_v140));
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_v142),ls_array2_fill(ls_v138,ls_v141));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_v143),ls_c46);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v144),ls_array2_pop(ls_v143));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v144);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_v146),ls_c46);
ls_native_temporaries_clear(&ls_temps);
ls_v147 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v148),ls_array2_get(ls_v146,ls_v147,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v148);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v151),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v152),ls_c42);
ls_native_temporaries_clear(&ls_temps);
ls_array3_take(&(ls_v153),ls_fn3(ls_value_array((ls_native_object *)ls_v151),ls_value_array((ls_native_object *)ls_v152)));
ls_native_temporaries_clear(&ls_temps);
ls_array3_copy(&(ls_c47),ls_v153);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array3_copy(&(ls_v155),ls_c47);
ls_native_temporaries_clear(&ls_temps);
ls_v156 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v157),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_fn5(ls_v155,ls_v156,ls_value_array((ls_native_object *)ls_v157));
ls_native_temporaries_clear(&ls_temps);
ls_array3_copy(&(ls_v159),ls_c47);
ls_native_temporaries_clear(&ls_temps);
ls_v160 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v161),ls_array3_get(ls_v159,ls_v160,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v162 = ls_from_u32(UINT32_C(10));
ls_native_temporaries_clear(&ls_temps);
ls_v163 = ls_array1_push(ls_v161,ls_v162);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v164),ls_c38);
ls_native_temporaries_clear(&ls_temps);
ls_v165 = (int32_t)ls_v164->length;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v165);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array3_copy(&(ls_v167),ls_c47);
ls_native_temporaries_clear(&ls_temps);
ls_v168 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v169),ls_array3_get(ls_v167,ls_v168,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_array3_copy(&(ls_v170),ls_c47);
ls_native_temporaries_clear(&ls_temps);
ls_v171 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v172),ls_array3_get(ls_v170,ls_v171,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_v173 = ls_v169 == ls_v172;
ls_native_temporaries_clear(&ls_temps);
puts(ls_v173 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v175),(ls_string){ls_s20,sizeof ls_s20/sizeof *ls_s20,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v176 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v177),ls_string_repeat(ls_v175,ls_v176));
ls_native_temporaries_clear(&ls_temps);
ls_v178 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v179),ls_array1_new(1));
ls_array1_push(ls_v179,ls_v178);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v180),(ls_t2){ls_v177,ls_v179});
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v181),ls_v180);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v182),(ls_string){ls_s21,sizeof ls_s21/sizeof *ls_s21,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v183 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v184),ls_string_repeat(ls_v182,ls_v183));
ls_native_temporaries_clear(&ls_temps);
ls_v185 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v186),ls_array1_new(1));
ls_array1_push(ls_v186,ls_v185);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v187),(ls_t2){ls_v184,ls_v186});
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v188),ls_v187);
ls_native_temporaries_clear(&ls_temps);
ls_array4_take(&(ls_v189),ls_array4_new(2));
ls_array4_push(ls_v189,ls_v181);
ls_array4_push(ls_v189,ls_v188);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_c48),ls_v189);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v190),ls_c48);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v191),(ls_t0){ls_v190});
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v192),ls_v191);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c49),ls_v192);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v193),ls_c49.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_v194 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
(void)(ls_array4_get(ls_v193,ls_v194,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v195),(ls_string){ls_s22,sizeof ls_s22/sizeof *ls_s22,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_t2 ls_wb0 = ls_array4_get(ls_v193,ls_v194,&ls_temps);
ls_t2_retain(ls_wb0);
ls_string_copy(&ls_wb0.ls_f0,ls_v195);
ls_array4_set(ls_v193,ls_v194,ls_wb0);
ls_t2_release(ls_wb0);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v196),ls_c48);
ls_native_temporaries_clear(&ls_temps);
ls_v197 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v198),ls_array4_get(ls_v196,ls_v197,&ls_temps).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v198);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v201),ls_c48);
ls_native_temporaries_clear(&ls_temps);
ls_array4_take(&(ls_v202),ls_fn6(ls_v201));
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_c50),ls_v202);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v203),ls_c50);
ls_native_temporaries_clear(&ls_temps);
ls_v204 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
(void)(ls_array4_get(ls_v203,ls_v204,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v205),(ls_string){ls_s23,sizeof ls_s23/sizeof *ls_s23,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_t2 ls_wb0 = ls_array4_get(ls_v203,ls_v204,&ls_temps);
ls_t2_retain(ls_wb0);
ls_string_copy(&ls_wb0.ls_f0,ls_v205);
ls_array4_set(ls_v203,ls_v204,ls_wb0);
ls_t2_release(ls_wb0);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v206),ls_c48);
ls_native_temporaries_clear(&ls_temps);
ls_v207 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v208),ls_array4_get(ls_v206,ls_v207,&ls_temps).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v208);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v210),ls_c50);
ls_native_temporaries_clear(&ls_temps);
ls_v211 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v212),ls_array4_get(ls_v210,ls_v211,&ls_temps).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v212);
ls_native_temporaries_clear(&ls_temps);
ls_array4_copy(&(ls_v214),ls_c50);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable40_take(&(ls_v215),ls_closure17());
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v216),(ls_string){ls_s24,sizeof ls_s24/sizeof *ls_s24,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v217),ls_array1_new(0));
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v218),(ls_t2){ls_v216,ls_v217});
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v219),ls_v218);
ls_native_temporaries_clear(&ls_temps);
{ ls_array4 *ls_src=ls_v214; size_t ls_len=ls_src->length;
ls_t2 ls_acc = ls_v219;
ls_t2_retain(ls_acc);
for(size_t ls_k=0;ls_k<ls_len;++ls_k) {
if(!ls_array_has(ls_src,ls_k)) continue;
ls_t2 ls_item = ls_array4_get(ls_src,(int32_t)ls_k,&ls_temps);
ls_t2_retain(ls_item);
ls_t2 ls_next = ls_v215.code(ls_v215.environment,ls_acc,ls_item);
ls_t2_take(&ls_acc,ls_next);
ls_t2_release(ls_item);
ls_native_temporaries_clear(&ls_temps);
}
ls_t2_take(&(ls_v220),ls_acc);
}
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v221),ls_v220);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_c51),ls_v221);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v222),ls_c51.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v222);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v224),ls_c51.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_v225 = (int32_t)ls_v224->length;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v225);
ls_native_temporaries_clear(&ls_temps);
ls_v227 = ls_from_u32(UINT32_C(5));
ls_native_temporaries_clear(&ls_temps);
ls_v228 = ls_from_u32(UINT32_C(6));
ls_native_temporaries_clear(&ls_temps);
ls_v229 = ls_from_u32(UINT32_C(7));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v230),ls_array1_new(3));
ls_array1_push(ls_v230,ls_v227);
ls_array1_push(ls_v230,ls_v228);
ls_array1_push(ls_v230,ls_v229);
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize54(ls_c54,ls_v230);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v231),(*ls_box_value54(ls_c54)));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable27_take(&(ls_v232),ls_closure18(ls_c54));
ls_native_temporaries_clear(&ls_temps);
{ ls_array1 *ls_src=ls_v231; size_t ls_len=ls_src->length;
ls_array1 *ls_out=ls_array1_new(ls_len);
for(size_t ls_k=0;ls_k<ls_len;++ls_k) {
if(!ls_array_has(ls_src,ls_k)) {ls_array1_hole(ls_out);continue;}
int32_t ls_item = ls_array1_get(ls_src,(int32_t)ls_k,&ls_temps);
int32_t ls_mapped = ls_v232.code(ls_v232.environment,ls_item);
ls_array1_push_owned(ls_out,ls_mapped);
ls_native_temporaries_clear(&ls_temps);
}
ls_array1_take(&(ls_v233),ls_out);
}
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c55),ls_v233);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v234),ls_c55);
ls_native_temporaries_clear(&ls_temps);
{
ls_string_builder ls_json = {0};
ls_string_builder_unit(&ls_json,'[');
for (size_t ls_i = 0; ls_i < ls_v234->length; ++ls_i) {
if (ls_i) ls_string_builder_unit(&ls_json,',');
if(!ls_array_has(ls_v234,ls_i)) ls_string_builder_ascii(&ls_json,"null"); else ls_json_value(&ls_json,ls_value_int(ls_array1_get(ls_v234,(int32_t)ls_i,&ls_temps)));
}
ls_string_builder_unit(&ls_json,']');
ls_string_take(&(ls_v235),ls_string_builder_finish(&ls_json));
}
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v235);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v237),ls_c55);
ls_native_temporaries_clear(&ls_temps);
ls_v238 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_v239 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v240 = (int32_t)ls_v237->length;
ls_native_temporaries_clear(&ls_temps);
ls_v241 = ls_v238 >= ls_v239;
ls_native_temporaries_clear(&ls_temps);
ls_v243 = ls_v241;
if (ls_v241) {
ls_native_temporaries_clear(&ls_temps);
ls_v242 = ls_v238 < ls_v240;
ls_native_temporaries_clear(&ls_temps);
ls_v243 = ls_v242;
ls_native_temporaries_clear(&ls_temps);
}
if (ls_v243) {
ls_native_temporaries_clear(&ls_temps);
ls_v244 = ls_array1_optional(ls_v237,ls_v238,&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v246 = ls_v244;
ls_native_temporaries_clear(&ls_temps);
} else {
ls_v245 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_v246 = ls_v245;
ls_native_temporaries_clear(&ls_temps);
}
ls_v247 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_v248 = ls_value_equal(ls_v246,ls_v247);
ls_native_temporaries_clear(&ls_temps);
puts(ls_v248 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v250),ls_c55);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v251 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v252=ls_array1_index_of(ls_v250,ls_v251);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v252);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v254),ls_c55);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v255 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v256=ls_array1_includes(ls_v254,ls_v255,0);
ls_native_temporaries_clear(&ls_temps);
puts(ls_v256 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_v258 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize57(ls_c57,ls_v258);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v259),ls_c55);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable1_take(&(ls_v260),ls_closure19(ls_c57));
ls_native_temporaries_clear(&ls_temps);
{ ls_array1 *ls_src=ls_v259; size_t ls_len=ls_src->length;
for(size_t ls_k=0;ls_k<ls_len;++ls_k) {
if(!ls_array_has(ls_src,ls_k)) continue;
int32_t ls_item = ls_array1_get(ls_src,(int32_t)ls_k,&ls_temps);
ls_v260.code(ls_v260.environment,ls_item);
ls_native_temporaries_clear(&ls_temps);
}
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v262 = (*ls_box_value57(ls_c57));
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v262);
ls_native_temporaries_clear(&ls_temps);
ls_callable1_clear(&ls_v260);
ls_array1_clear(&ls_v259);
ls_native_release(ls_c57);
ls_c57 = NULL;
ls_array1_clear(&ls_v254);
ls_array1_clear(&ls_v250);
ls_array1_clear(&ls_v237);
ls_string_clear(&ls_v235);
ls_array1_clear(&ls_v234);
ls_array1_clear(&ls_c55);
ls_array1_clear(&ls_v233);
ls_callable27_clear(&ls_v232);
ls_array1_clear(&ls_v231);
ls_native_release(ls_c54);
ls_c54 = NULL;
ls_array1_clear(&ls_v230);
ls_array1_clear(&ls_v224);
ls_string_clear(&ls_v222);
ls_t2_clear(&ls_c51);
ls_t2_clear(&ls_v221);
ls_t2_clear(&ls_v220);
ls_t2_clear(&ls_v219);
ls_t2_clear(&ls_v218);
ls_array1_clear(&ls_v217);
ls_string_clear(&ls_v216);
ls_callable40_clear(&ls_v215);
ls_array4_clear(&ls_v214);
ls_string_clear(&ls_v212);
ls_array4_clear(&ls_v210);
ls_string_clear(&ls_v208);
ls_array4_clear(&ls_v206);
ls_string_clear(&ls_v205);
ls_array4_clear(&ls_v203);
ls_array4_clear(&ls_c50);
ls_array4_clear(&ls_v202);
ls_array4_clear(&ls_v201);
ls_string_clear(&ls_v198);
ls_array4_clear(&ls_v196);
ls_string_clear(&ls_v195);
ls_array4_clear(&ls_v193);
ls_t0_clear(&ls_c49);
ls_t0_clear(&ls_v192);
ls_t0_clear(&ls_v191);
ls_array4_clear(&ls_v190);
ls_array4_clear(&ls_c48);
ls_array4_clear(&ls_v189);
ls_t2_clear(&ls_v188);
ls_t2_clear(&ls_v187);
ls_array1_clear(&ls_v186);
ls_string_clear(&ls_v184);
ls_string_clear(&ls_v182);
ls_t2_clear(&ls_v181);
ls_t2_clear(&ls_v180);
ls_array1_clear(&ls_v179);
ls_string_clear(&ls_v177);
ls_string_clear(&ls_v175);
ls_array1_clear(&ls_v172);
ls_array3_clear(&ls_v170);
ls_array1_clear(&ls_v169);
ls_array3_clear(&ls_v167);
ls_array1_clear(&ls_v164);
ls_array1_clear(&ls_v161);
ls_array3_clear(&ls_v159);
ls_array1_clear(&ls_v157);
ls_array3_clear(&ls_v155);
ls_array3_clear(&ls_c47);
ls_array3_clear(&ls_v153);
ls_array1_clear(&ls_v152);
ls_array1_clear(&ls_v151);
ls_string_clear(&ls_v148);
ls_array2_clear(&ls_v146);
ls_string_clear(&ls_v144);
ls_array2_clear(&ls_v143);
ls_array2_clear(&ls_v142);
ls_string_clear(&ls_v141);
ls_string_clear(&ls_v139);
ls_array2_clear(&ls_v138);
ls_array2_clear(&ls_c46);
ls_array2_clear(&ls_v137);
ls_string_clear(&ls_v136);
ls_string_clear(&ls_v134);
ls_string_clear(&ls_v133);
ls_string_clear(&ls_v131);
ls_string_clear(&ls_v128);
ls_string_clear(&ls_v127);
ls_array1_clear(&ls_v126);
ls_array1_clear(&ls_v124);
ls_string_clear(&ls_v122);
ls_string_clear(&ls_v121);
ls_array1_clear(&ls_v120);
ls_array1_clear(&ls_c45);
ls_array1_clear(&ls_v119);
ls_array1_clear(&ls_v118);
ls_array1_clear(&ls_v116);
ls_string_clear(&ls_v114);
ls_string_clear(&ls_v113);
ls_array1_clear(&ls_v112);
ls_string_clear(&ls_v110);
ls_string_clear(&ls_v109);
ls_array1_clear(&ls_v108);
ls_array1_clear(&ls_c44);
ls_array1_clear(&ls_v107);
ls_array1_clear(&ls_v104);
ls_string_clear(&ls_v102);
ls_string_clear(&ls_v101);
ls_array1_clear(&ls_v100);
ls_array1_clear(&ls_v99);
ls_array1_clear(&ls_v95);
ls_array1_clear(&ls_v91);
ls_array1_clear(&ls_v87);
ls_string_clear(&ls_v85);
ls_string_clear(&ls_v84);
ls_array1_clear(&ls_v83);
ls_array1_clear(&ls_v80);
ls_array1_clear(&ls_v79);
ls_array1_clear(&ls_v78);
ls_array1_clear(&ls_v73);
ls_array1_clear(&ls_v70);
ls_string_clear(&ls_v68);
ls_string_clear(&ls_v67);
ls_array1_clear(&ls_v66);
ls_array1_clear(&ls_c43);
ls_array1_clear(&ls_v65);
ls_array1_clear(&ls_v64);
ls_string_clear(&ls_v61);
ls_string_clear(&ls_v60);
ls_array1_clear(&ls_v59);
ls_array1_clear(&ls_v55);
ls_array1_clear(&ls_v51);
ls_array1_clear(&ls_c42);
ls_array1_clear(&ls_v50);
ls_array1_clear(&ls_v44);
ls_array1_clear(&ls_v41);
ls_t0_clear(&ls_c41);
ls_t0_clear(&ls_v40);
ls_t0_clear(&ls_v39);
ls_array1_clear(&ls_v38);
ls_object_clear(&ls_v34);
ls_string_clear(&ls_v32);
ls_string_clear(&ls_v31);
ls_array1_clear(&ls_v30);
ls_object_clear(&ls_v26);
ls_object_clear(&ls_c40);
ls_object_clear(&ls_v23);
ls_array0_clear(&ls_v22);
ls_array1_clear(&ls_v21);
ls_array1_clear(&ls_v17);
ls_array1_clear(&ls_v12);
ls_array1_clear(&ls_v8);
ls_array1_clear(&ls_v7);
ls_array1_clear(&ls_c39);
ls_array1_clear(&ls_v6);
ls_array1_clear(&ls_v4);
ls_array1_clear(&ls_c38);
ls_array1_clear(&ls_v2);
}
static LS_NATIVE_UNUSED void ls_fn14(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_array5 * ls_c59 LS_NATIVE_UNUSED = NULL;
int32_t ls_c60 LS_NATIVE_UNUSED;
ls_array5 * ls_c61 LS_NATIVE_UNUSED = NULL;
ls_array5 * ls_v0 LS_NATIVE_UNUSED = NULL;
int32_t ls_v1 LS_NATIVE_UNUSED;
int32_t ls_v2 LS_NATIVE_UNUSED;
int32_t ls_v3 LS_NATIVE_UNUSED;
bool ls_v4 LS_NATIVE_UNUSED;
ls_array5 * ls_v5 LS_NATIVE_UNUSED = NULL;
int32_t ls_v6 LS_NATIVE_UNUSED;
ls_t1 ls_v7 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v8 LS_NATIVE_UNUSED = {0};
int32_t ls_v9 LS_NATIVE_UNUSED;
int32_t ls_v10 LS_NATIVE_UNUSED;
int32_t ls_v11 LS_NATIVE_UNUSED;
int32_t ls_v12 LS_NATIVE_UNUSED;
ls_array5 * ls_v14 LS_NATIVE_UNUSED = NULL;
bool ls_v15 LS_NATIVE_UNUSED;
ls_array5 * ls_v18 LS_NATIVE_UNUSED = NULL;
ls_array5 * ls_v19 LS_NATIVE_UNUSED = NULL;
ls_array5 * ls_v20 LS_NATIVE_UNUSED = NULL;
int32_t ls_v21 LS_NATIVE_UNUSED;
int32_t ls_v22 LS_NATIVE_UNUSED;
ls_array5_take(&(ls_v0),ls_array5_new(0));
ls_native_temporaries_clear(&ls_temps);
ls_array5_copy(&(ls_c59),ls_v0);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_c60 = ls_v1;
ls_native_temporaries_clear(&ls_temps);
ls_test17: ;
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_c60;
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_from_u32(UINT32_C(1000));
ls_native_temporaries_clear(&ls_temps);
ls_v4 = ls_v2 < ls_v3;
ls_native_temporaries_clear(&ls_temps);
if (!ls_v4) goto ls_end17;
ls_array5_copy(&(ls_v5),ls_c59);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v6 = ls_c60;
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v7),(ls_t1){ls_v6});
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v8),ls_v7);
ls_native_temporaries_clear(&ls_temps);
ls_v9 = ls_array5_push(ls_v5,ls_v8);
ls_native_temporaries_clear(&ls_temps);
ls_t1_clear(&ls_v8);
ls_t1_clear(&ls_v7);
ls_array5_clear(&ls_v5);
ls_update17: LS_NATIVE_UNUSED;
ls_v10 = ls_c60;
ls_native_temporaries_clear(&ls_temps);
ls_v11 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v12 = ls_from_u32((uint32_t)((uint32_t)ls_v10 + (uint32_t)ls_v11));
ls_native_temporaries_clear(&ls_temps);
ls_c60 = ls_v12;
ls_native_temporaries_clear(&ls_temps);
goto ls_test17;
ls_end17: ;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array5_copy(&(ls_v14),ls_c59);
ls_native_temporaries_clear(&ls_temps);
ls_v15 = ls_fn12(ls_v14);
ls_native_temporaries_clear(&ls_temps);
puts(ls_v15 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array5_copy(&(ls_v18),ls_c59);
ls_native_temporaries_clear(&ls_temps);
ls_array5_take(&(ls_v19),ls_fn6(ls_v18));
ls_native_temporaries_clear(&ls_temps);
ls_array5_copy(&(ls_c61),ls_v19);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array5_copy(&(ls_v20),ls_c61);
ls_native_temporaries_clear(&ls_temps);
ls_v21 = ls_from_u32(UINT32_C(999));
ls_native_temporaries_clear(&ls_temps);
ls_v22 = ls_array5_get(ls_v20,ls_v21,&ls_temps).ls_f0;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v22);
ls_native_temporaries_clear(&ls_temps);
ls_array5_clear(&ls_v20);
ls_array5_clear(&ls_c61);
ls_array5_clear(&ls_v19);
ls_array5_clear(&ls_v18);
ls_array5_clear(&ls_v14);
ls_array5_clear(&ls_c59);
ls_array5_clear(&ls_v0);
}
static LS_NATIVE_UNUSED ls_value ls_fn15(void *ls_env LS_NATIVE_UNUSED,ls_value ls_c28 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c28);
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_value ls_v4 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_c28);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_value_take(&(ls_v3),ls_fn1(ls_v2));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v4),ls_v3);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_v4;
ls_value_retain(ls_return);
ls_value_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_c28);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_c28);
}
static LS_NATIVE_UNUSED bool ls_fn16(void *ls_env LS_NATIVE_UNUSED,ls_value ls_c37 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c37);
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
bool ls_v3 LS_NATIVE_UNUSED;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_c37);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_fn11(ls_v2);
ls_native_temporaries_clear(&ls_temps);
{
bool ls_return = ls_v3;
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_c37);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_c37);
}
static LS_NATIVE_UNUSED ls_t2 ls_fn17(void *ls_env LS_NATIVE_UNUSED,ls_t2 ls_c52 LS_NATIVE_UNUSED,ls_t2 ls_c53 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_t2_retain(ls_c52);
ls_t2_retain(ls_c53);
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
ls_string ls_v1 LS_NATIVE_UNUSED = {0};
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v3 LS_NATIVE_UNUSED = NULL;
int32_t ls_v4 LS_NATIVE_UNUSED;
int32_t ls_v5 LS_NATIVE_UNUSED;
ls_t2 ls_v6 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v7 LS_NATIVE_UNUSED = {0};
ls_string_copy(&(ls_v0),ls_c52.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v1),ls_c53.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v2),ls_string_join_owned(2,(ls_string[]){ls_string_hold(ls_v0),ls_string_hold(ls_v1)}));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c52.ls_f0),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v3),ls_c52.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v4 = ls_from_u32(UINT32_C(3));
ls_native_temporaries_clear(&ls_temps);
ls_v5 = ls_array1_push(ls_v3,ls_v4);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v6),ls_c52);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v7),ls_v6);
ls_native_temporaries_clear(&ls_temps);
{
ls_t2 ls_return = ls_v7;
ls_t2_retain(ls_return);
ls_t2_clear(&ls_v7);
ls_t2_clear(&ls_v6);
ls_array1_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v1);
ls_string_clear(&ls_v0);
ls_t2_clear(&ls_c53);
ls_t2_clear(&ls_c52);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t2_clear(&ls_v7);
ls_t2_clear(&ls_v6);
ls_array1_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v1);
ls_string_clear(&ls_v0);
ls_t2_clear(&ls_c53);
ls_t2_clear(&ls_c52);
}
static LS_NATIVE_UNUSED int32_t ls_fn18(void *ls_env LS_NATIVE_UNUSED,int32_t ls_c56 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v0 LS_NATIVE_UNUSED = NULL;
int32_t ls_v1 LS_NATIVE_UNUSED;
int32_t ls_v2 LS_NATIVE_UNUSED;
ls_array1_copy(&(ls_v0),(*ls_box_value54(((ls_env18 *)ls_env)->ls_e0)));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_array1_pop(ls_v0);
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_c56;
ls_native_temporaries_clear(&ls_temps);
{
int32_t ls_return = ls_v2;
ls_array1_clear(&ls_v0);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_array1_clear(&ls_v0);
}
static LS_NATIVE_UNUSED void ls_fn19(void *ls_env LS_NATIVE_UNUSED,int32_t ls_c58 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_v0 LS_NATIVE_UNUSED;
int32_t ls_v1 LS_NATIVE_UNUSED;
int32_t ls_v2 LS_NATIVE_UNUSED;
ls_v0 = (*ls_box_value57(((ls_env19 *)ls_env)->ls_e0));
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_from_u32((uint32_t)((uint32_t)ls_v0 + (uint32_t)ls_v1));
ls_native_temporaries_clear(&ls_temps);
(*ls_box_value57(((ls_env19 *)ls_env)->ls_e0)) = ls_v2;
ls_native_temporaries_clear(&ls_temps);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(20);
ls_init0();
fflush(stdout);
ls_native_collect_cycles();
return 0;
}
