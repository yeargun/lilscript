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
typedef struct ls_t0 ls_t0;
typedef struct ls_t1 ls_t1;
typedef struct ls_native_array ls_array0;
typedef struct ls_native_array ls_array1;
typedef struct ls_native_array ls_array2;
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
ls_string (*code)(void *);
void *environment;
uint64_t identity;
} ls_callable3;
typedef struct {
int32_t (*code)(void *);
void *environment;
uint64_t identity;
} ls_callable5;
typedef struct {
int32_t (*code)(void *,int32_t);
void *environment;
uint64_t identity;
} ls_callable8;
struct ls_t0 {
ls_string ls_f0;
ls_array2 * ls_f1;
};
static LS_NATIVE_UNUSED inline void ls_t0_retain(ls_t0 value) { (void)value;
ls_native_retain(value.ls_f0.owner);
ls_native_retain(value.ls_f1);
}
static LS_NATIVE_UNUSED inline void ls_t0_release(ls_t0 value) { (void)value;
ls_native_release(value.ls_f0.owner);
ls_native_release(value.ls_f1);
}
static LS_NATIVE_UNUSED inline void ls_t0_trace(ls_t0 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
visit(value.ls_f0.owner, context);
visit(value.ls_f1, context);
}
static LS_NATIVE_UNUSED inline void ls_t0_copy(ls_t0 *slot, ls_t0 value) { ls_t0_retain(value); ls_t0_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t0_take(ls_t0 *slot, ls_t0 value) { ls_t0_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t0_clear(ls_t0 *slot) { ls_t0_release(*slot); *slot = (ls_t0){0}; }
struct ls_t1 {
ls_callable5 ls_f0;
ls_string ls_f1;
};
static LS_NATIVE_UNUSED inline void ls_t1_retain(ls_t1 value) { (void)value;
ls_native_retain(value.ls_f0.environment);
ls_native_retain(value.ls_f1.owner);
}
static LS_NATIVE_UNUSED inline void ls_t1_release(ls_t1 value) { (void)value;
ls_native_release(value.ls_f0.environment);
ls_native_release(value.ls_f1.owner);
}
static LS_NATIVE_UNUSED inline void ls_t1_trace(ls_t1 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
visit(value.ls_f0.environment, context);
visit(value.ls_f1.owner, context);
}
static LS_NATIVE_UNUSED inline void ls_t1_copy(ls_t1 *slot, ls_t1 value) { ls_t1_retain(value); ls_t1_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t1_take(ls_t1 *slot, ls_t1 value) { ls_t1_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t1_clear(ls_t1 *slot) { ls_t1_release(*slot); *slot = (ls_t1){0}; }
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
static LS_NATIVE_UNUSED inline ls_callable3 ls_callable3_retain(ls_callable3 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable3_release(ls_callable3 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable3_copy(ls_callable3 *destination, ls_callable3 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable3_take(ls_callable3 *destination, ls_callable3 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable3_clear(ls_callable3 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable3){0};
}
static LS_NATIVE_UNUSED inline ls_string ls_callable3_call(ls_callable3 value) {
ls_native_retain(value.environment);
ls_string result = value.code(value.environment);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable5 ls_callable5_retain(ls_callable5 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable5_release(ls_callable5 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable5_copy(ls_callable5 *destination, ls_callable5 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable5_take(ls_callable5 *destination, ls_callable5 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable5_clear(ls_callable5 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable5){0};
}
static LS_NATIVE_UNUSED inline int32_t ls_callable5_call(ls_callable5 value) {
ls_native_retain(value.environment);
int32_t result = value.code(value.environment);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable8 ls_callable8_retain(ls_callable8 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable8_release(ls_callable8 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable8_copy(ls_callable8 *destination, ls_callable8 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable8_take(ls_callable8 *destination, ls_callable8 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable8_clear(ls_callable8 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable8){0};
}
static LS_NATIVE_UNUSED inline int32_t ls_callable8_call(ls_callable8 value,int32_t ls_p0) {
ls_native_retain(value.environment);
int32_t result = value.code(value.environment,ls_p0);
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
_Static_assert(sizeof(ls_callable3) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable3(ls_callable3 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 3}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable3 ls_value_to_callable3(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable3){0};
if (value.tag != LS_CALLABLE || value.signature != 3) ls_value_mismatch();
ls_callable3 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable5) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable5(ls_callable5 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 5}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable5 ls_value_to_callable5(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable5){0};
if (value.tag != LS_CALLABLE || value.signature != 5) ls_value_mismatch();
ls_callable5 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable8) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable8(ls_callable8 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 8}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable8 ls_value_to_callable8(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable8){0};
if (value.tag != LS_CALLABLE || value.signature != 8) ls_value_mismatch();
ls_callable8 result; memcpy(&result, &value.as.c, sizeof result); return result;
}

static LS_NATIVE_UNUSED void ls_array0_acquire(ls_string value) { (void)value; ls_native_retain(value.owner);
 }
static LS_NATIVE_UNUSED void ls_array0_drop(ls_string value) { (void)value; ls_native_release(value.owner);
 }
static LS_NATIVE_UNUSED void ls_array0_retain_slot(const void *slot) { ls_string value=*(ls_string const *)slot; ls_array0_acquire(value); }
static LS_NATIVE_UNUSED void ls_array0_drop_slot(void *slot) { ls_string value=*(ls_string *)slot; ls_array0_drop(value); }
static LS_NATIVE_UNUSED void ls_array0_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_string value=*(ls_string const *)slot; (void)value; (void)visit; (void)context; visit(value.owner, context);

}
static LS_NATIVE_UNUSED ls_value ls_array0_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_string value=*(ls_string const *)slot; return ls_value_string(value);
}
static LS_NATIVE_UNUSED void ls_array0_write_slot(void *slot, ls_value input) {
ls_string value=ls_value_to_string(input); ls_array0_acquire(value); ls_array0_drop(*(ls_string *)slot); *(ls_string *)slot=value;
}
static const ls_array_ops ls_array0_ops={sizeof(ls_string),ls_array0_retain_slot,ls_array0_drop_slot,ls_array0_trace_slot,ls_array0_read_slot,ls_array0_write_slot};
static LS_NATIVE_UNUSED ls_array0 *ls_array0_new(size_t capacity) { return ls_array_new(&ls_array0_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array0_hole(ls_array0 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_string ls_array0_absent(void) { return (ls_string){0}; }
static LS_NATIVE_UNUSED ls_string ls_array0_get(ls_array0 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array0_absent();
if(array->ops==&ls_array0_ops) return ((ls_string *)array->items)[index];
return ls_value_to_string(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array0_optional(ls_array0 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array0_push_owned(ls_array0 *array, ls_string value) {
if(array->ops==&ls_array0_ops) { *(ls_string *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array0_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array0_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array0_push(ls_array0 *array, ls_string value) {
ls_array0_acquire(value); return ls_array0_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array0_set(ls_array0 *array, int32_t index, ls_string value) {
if(array->ops==&ls_array0_ops) {
ls_array0_acquire(value); ls_string *slot=ls_array_store_slot(array,index); ls_array0_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array0_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_string ls_array0_pop(ls_array0 *array) {
if(!array->length) return ls_array0_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array0_absent(); }
if(array->ops==&ls_array0_ops) { ls_string result=((ls_string *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_string result=ls_array0_get(array,(int32_t)index,&ls_temps); ls_array0_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_slice(ls_array0 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_concat(ls_array0 *left, ls_array0 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_reverse(ls_array0 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_splice(ls_array0 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_copy_within(ls_array0 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array0 *ls_array0_fill(ls_array0 *array, ls_string value) {
for(size_t i=0;i<array->length;++i) { ls_array0_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array0_copy(ls_array0 **slot, ls_array0 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array0_take(ls_array0 **slot, ls_array0 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array0_clear(ls_array0 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED int32_t ls_array0_index_of(ls_array0 *array, ls_string right) {
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {
ls_string left=ls_array0_get(array,(int32_t)i,&ls_temps); bool same=(ls_string_equal(left,right));
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}
return -1;
}
static LS_NATIVE_UNUSED bool ls_array0_includes(ls_array0 *array, ls_string right, int32_t start) {
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {
ls_string left=ls_array0_get(array,(int32_t)i,&ls_temps); bool same=(ls_string_equal(left,right));
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}
return false;
}

static LS_NATIVE_UNUSED void ls_array1_acquire(ls_t0 value) { (void)value; ls_t0_retain(value);
 }
static LS_NATIVE_UNUSED void ls_array1_drop(ls_t0 value) { (void)value; ls_t0_release(value);
 }
static LS_NATIVE_UNUSED void ls_array1_retain_slot(const void *slot) { ls_t0 value=*(ls_t0 const *)slot; ls_array1_acquire(value); }
static LS_NATIVE_UNUSED void ls_array1_drop_slot(void *slot) { ls_t0 value=*(ls_t0 *)slot; ls_array1_drop(value); }
static LS_NATIVE_UNUSED void ls_array1_trace_slot(const void *slot, ls_native_visit visit, void *context) {
ls_t0 value=*(ls_t0 const *)slot; (void)value; (void)visit; (void)context; ls_t0_trace(value, visit, context);

}
static LS_NATIVE_UNUSED ls_value ls_array1_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; ls_t0 value=*(ls_t0 const *)slot; return ls_t0_box(temps,value);
}
static LS_NATIVE_UNUSED void ls_array1_write_slot(void *slot, ls_value input) {
ls_t0 value=ls_value_to_t0(input); ls_array1_acquire(value); ls_array1_drop(*(ls_t0 *)slot); *(ls_t0 *)slot=value;
}
static const ls_array_ops ls_array1_ops={sizeof(ls_t0),ls_array1_retain_slot,ls_array1_drop_slot,ls_array1_trace_slot,ls_array1_read_slot,ls_array1_write_slot};
static LS_NATIVE_UNUSED ls_array1 *ls_array1_new(size_t capacity) { return ls_array_new(&ls_array1_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array1_hole(ls_array1 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED ls_t0 ls_array1_absent(void) { ls_native_undefined_element(); return (ls_t0){0}; }
static LS_NATIVE_UNUSED ls_t0 ls_array1_get(ls_array1 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array1_absent();
if(array->ops==&ls_array1_ops) return ((ls_t0 *)array->items)[index];
return ls_value_to_t0(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array1_optional(ls_array1 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array1_push_owned(ls_array1 *array, ls_t0 value) {
if(array->ops==&ls_array1_ops) { *(ls_t0 *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array1_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array1_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array1_push(ls_array1 *array, ls_t0 value) {
ls_array1_acquire(value); return ls_array1_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array1_set(ls_array1 *array, int32_t index, ls_t0 value) {
if(array->ops==&ls_array1_ops) {
ls_array1_acquire(value); ls_t0 *slot=ls_array_store_slot(array,index); ls_array1_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array1_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED ls_t0 ls_array1_pop(ls_array1 *array) {
if(!array->length) return ls_array1_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array1_absent(); }
if(array->ops==&ls_array1_ops) { ls_t0 result=((ls_t0 *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
ls_t0 result=ls_array1_get(array,(int32_t)index,&ls_temps); ls_array1_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_slice(ls_array1 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_concat(ls_array1 *left, ls_array1 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_reverse(ls_array1 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_splice(ls_array1 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_copy_within(ls_array1 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array1 *ls_array1_fill(ls_array1 *array, ls_t0 value) {
for(size_t i=0;i<array->length;++i) { ls_array1_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array1_copy(ls_array1 **slot, ls_array1 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array1_take(ls_array1 **slot, ls_array1 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array1_clear(ls_array1 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED void ls_array2_acquire(int32_t value) { (void)value;  }
static LS_NATIVE_UNUSED void ls_array2_drop(int32_t value) { (void)value;  }
static LS_NATIVE_UNUSED void ls_array2_retain_slot(const void *slot) { int32_t value=*(int32_t const *)slot; ls_array2_acquire(value); }
static LS_NATIVE_UNUSED void ls_array2_drop_slot(void *slot) { int32_t value=*(int32_t *)slot; ls_array2_drop(value); }
static LS_NATIVE_UNUSED void ls_array2_trace_slot(const void *slot, ls_native_visit visit, void *context) {
int32_t value=*(int32_t const *)slot; (void)value; (void)visit; (void)context; 
}
static LS_NATIVE_UNUSED ls_value ls_array2_read_slot(ls_native_temporary **temps, const void *slot) {
(void)temps; int32_t value=*(int32_t const *)slot; return ls_value_int(value);
}
static LS_NATIVE_UNUSED void ls_array2_write_slot(void *slot, ls_value input) {
int32_t value=ls_value_to_int(input); ls_array2_acquire(value); ls_array2_drop(*(int32_t *)slot); *(int32_t *)slot=value;
}
static const ls_array_ops ls_array2_ops={sizeof(int32_t),ls_array2_retain_slot,ls_array2_drop_slot,ls_array2_trace_slot,ls_array2_read_slot,ls_array2_write_slot};
static LS_NATIVE_UNUSED ls_array2 *ls_array2_new(size_t capacity) { return ls_array_new(&ls_array2_ops,capacity); }
static LS_NATIVE_UNUSED void ls_array2_hole(ls_array2 *array) { ls_array_hole(array); }
static LS_NATIVE_UNUSED int32_t ls_array2_absent(void) { return 0; }
static LS_NATIVE_UNUSED int32_t ls_array2_get(ls_array2 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) ls_native_undefined_element();
if(!ls_array_has(array,(size_t)index)) return ls_array2_absent();
if(array->ops==&ls_array2_ops) return ((int32_t *)array->items)[index];
return ls_value_to_int(ls_array_read(array,(size_t)index,temps));
}
static LS_NATIVE_UNUSED ls_value ls_array2_optional(ls_array2 *array, int32_t index, ls_native_temporary **temps) {
if(index<0 || (size_t)index>=array->length) return (ls_value){0};
return ls_array_read(array,(size_t)index,temps);
}
static LS_NATIVE_UNUSED int32_t ls_array2_push_owned(ls_array2 *array, int32_t value) {
if(array->ops==&ls_array2_ops) { *(int32_t *)ls_array_append_slot(array)=value; }
else {
ls_native_temporary *ls_temps=NULL;
ls_value boxed=ls_array2_read_slot(&ls_temps,&value);
ls_array_write(array,(int32_t)array->length,boxed); ls_array2_drop(value);
ls_native_temporaries_clear(&ls_temps);
}
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array2_push(ls_array2 *array, int32_t value) {
ls_array2_acquire(value); return ls_array2_push_owned(array,value);
}
static LS_NATIVE_UNUSED void ls_array2_set(ls_array2 *array, int32_t index, int32_t value) {
if(array->ops==&ls_array2_ops) {
ls_array2_acquire(value); int32_t *slot=ls_array_store_slot(array,index); ls_array2_drop(*slot); *slot=value;
} else {
ls_native_temporary *ls_temps=NULL; ls_value boxed=ls_array2_read_slot(&ls_temps,&value);
ls_array_write(array,index,boxed); ls_native_temporaries_clear(&ls_temps);
}
}
static LS_NATIVE_UNUSED int32_t ls_array2_pop(ls_array2 *array) {
if(!array->length) return ls_array2_absent();
size_t index=array->length-1;
if(!ls_array_has(array,index)) { --array->length; return ls_array2_absent(); }
if(array->ops==&ls_array2_ops) { int32_t result=((int32_t *)array->items)[index]; --array->length; return result; }
ls_native_temporary *ls_temps=NULL;
int32_t result=ls_array2_get(array,(int32_t)index,&ls_temps); ls_array2_acquire(result);
ls_array_drop_last(array); ls_native_temporaries_clear(&ls_temps); return result;
}
static LS_NATIVE_UNUSED ls_array2 *ls_array2_slice(ls_array2 *array, size_t start, size_t end) { return ls_array_slice(array,start,end); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_concat(ls_array2 *left, ls_array2 *right) { return ls_array_concat(left,right); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_reverse(ls_array2 *array) { return ls_array_reverse(array); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_splice(ls_array2 *array, int32_t start, int32_t count) { return ls_array_splice(array,start,count); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_copy_within(ls_array2 *array, int32_t target, int32_t start, bool bounded, int32_t end) { return ls_array_copy_within(array,target,start,bounded,end); }
static LS_NATIVE_UNUSED ls_array2 *ls_array2_fill(ls_array2 *array, int32_t value) {
for(size_t i=0;i<array->length;++i) { ls_array2_set(array,(int32_t)i,value); }
return array;
}
static LS_NATIVE_UNUSED void ls_array2_copy(ls_array2 **slot, ls_array2 *value) { ls_native_retain(value); ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array2_take(ls_array2 **slot, ls_array2 *value) { ls_native_release(*slot); *slot=value; }
static LS_NATIVE_UNUSED void ls_array2_clear(ls_array2 **slot) { ls_native_release(*slot); *slot=NULL; }

static LS_NATIVE_UNUSED int32_t ls_array2_index_of(ls_array2 *array, int32_t right) {
ls_native_temporary *ls_temps=NULL;
for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) {
int32_t left=ls_array2_get(array,(int32_t)i,&ls_temps); bool same=(left==right);
ls_native_temporaries_clear(&ls_temps); if(same) return (int32_t)i;
}
return -1;
}
static LS_NATIVE_UNUSED bool ls_array2_includes(ls_array2 *array, int32_t right, int32_t start) {
size_t from=start>=0 ? (size_t)start : ls_array_relative(start,array->length);
ls_native_temporary *ls_temps=NULL;
for(size_t i=from;i<array->length;++i) if(ls_array_has(array,i)) {
int32_t left=ls_array2_get(array,(int32_t)i,&ls_temps); bool same=(left==right);
ls_native_temporaries_clear(&ls_temps); if(same) return true;
}
return false;
}
static LS_NATIVE_UNUSED ls_array0 *ls_string_split(ls_string text, ls_string separator) {
ls_array0 *parts = ls_array0_new(0);
if (!separator.length) {
for (size_t index = 0; index < text.length; index++) ls_array0_push_owned(parts, ls_string_view(text, index, index + 1));
return parts;
}
if (!text.length) { ls_array0_push(parts, text); return parts; }
size_t start = 0;
for (size_t at = 0; at + separator.length <= text.length;) {
if (ls_string_matches(text, at, separator)) { ls_array0_push_owned(parts, ls_string_view(text, start, at)); at += separator.length; start = at; }
else at++;
}
ls_array0_push_owned(parts, ls_string_view(text, start, text.length));
return parts;
}
static LS_NATIVE_UNUSED void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef struct ls_object0 {
ls_native_object owner;
ls_value ls_m0;
ls_value ls_m1;
ls_string ls_m2;
} ls_object0;
static LS_NATIVE_UNUSED void ls_object0_clear_fields(ls_object0 *object) {
(void)object;
ls_value_clear(&object->ls_m0);
ls_value_clear(&object->ls_m1);
ls_string_clear(&object->ls_m2);
}
static LS_NATIVE_UNUSED void ls_object0_destroy(ls_native_object *owner) { ls_object0_clear_fields((ls_object0 *)owner); }
static LS_NATIVE_UNUSED void ls_object0_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_object0 *object = (ls_object0 *)owner; (void)object; (void)visit; (void)context;
ls_value_trace(object->ls_m0, visit, context);
ls_value_trace(object->ls_m1, visit, context);
visit(object->ls_m2.owner, context);
}
typedef int32_t host_assertNativeCount_arg0;
typedef void host_assertNativeCount_result;
void host_assertNativeCount(int32_t);
typedef void host_collectNative_result;
void host_collectNative(void);
typedef ls_string host_makeHostText_result;
ls_string host_makeHostText(void);
typedef struct { ls_native_object owner; int32_t value; bool ready; } ls_box26;
static LS_NATIVE_UNUSED ls_box26 *ls_box_allocate26(void) { return ls_native_allocate(sizeof(ls_box26),NULL,NULL); }
static LS_NATIVE_UNUSED void ls_box_initialize26(ls_box26 *box, int32_t value) {
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box26 *ls_box_new26(int32_t value) { ls_box26 *box = ls_box_allocate26(); ls_box_initialize26(box, value); return box; }
static LS_NATIVE_UNUSED int32_t *ls_box_value26(ls_box26 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct { ls_native_object owner; ls_callable5 value; bool ready; } ls_box27;
static LS_NATIVE_UNUSED void ls_box_destroy27(ls_native_object *owner) { ls_callable5_clear(&((ls_box27 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_box_trace27(ls_native_object *owner, ls_native_visit visit, void *context) { visit(((ls_box27 *)owner)->value.environment, context);
 }
static LS_NATIVE_UNUSED ls_box27 *ls_box_allocate27(void) { return ls_native_allocate(sizeof(ls_box27),ls_box_destroy27,ls_box_trace27); }
static LS_NATIVE_UNUSED void ls_box_initialize27(ls_box27 *box, ls_callable5 value) {
ls_native_retain(value.environment);
ls_native_release(box->value.environment);
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box27 *ls_box_new27(ls_callable5 value) { ls_box27 *box = ls_box_allocate27(); ls_box_initialize27(box, value); return box; }
static LS_NATIVE_UNUSED ls_callable5 *ls_box_value27(ls_box27 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct { ls_native_object owner; ls_callable8 value; bool ready; } ls_box32;
static LS_NATIVE_UNUSED void ls_box_destroy32(ls_native_object *owner) { ls_callable8_clear(&((ls_box32 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_box_trace32(ls_native_object *owner, ls_native_visit visit, void *context) { visit(((ls_box32 *)owner)->value.environment, context);
 }
static LS_NATIVE_UNUSED ls_box32 *ls_box_allocate32(void) { return ls_native_allocate(sizeof(ls_box32),ls_box_destroy32,ls_box_trace32); }
static LS_NATIVE_UNUSED void ls_box_initialize32(ls_box32 *box, ls_callable8 value) {
ls_native_retain(value.environment);
ls_native_release(box->value.environment);
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box32 *ls_box_new32(ls_callable8 value) { ls_box32 *box = ls_box_allocate32(); ls_box_initialize32(box, value); return box; }
static LS_NATIVE_UNUSED ls_callable8 *ls_box_value32(ls_box32 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct {
ls_native_object owner;
ls_box26 *ls_e0;
} ls_env9;
static LS_NATIVE_UNUSED void ls_env_destroy9(ls_native_object *owner) {
ls_env9 *environment = (ls_env9 *)owner;
ls_native_release(environment->ls_e0);
}
static LS_NATIVE_UNUSED void ls_env_trace9(ls_native_object *owner, ls_native_visit visit, void *context) {
ls_env9 *environment = (ls_env9 *)owner;
visit(environment->ls_e0, context);
}
typedef struct {
ls_native_object owner;
ls_box27 *ls_e0;
} ls_env10;
static LS_NATIVE_UNUSED void ls_env_destroy10(ls_native_object *owner) {
ls_env10 *environment = (ls_env10 *)owner;
ls_native_release(environment->ls_e0);
}
static LS_NATIVE_UNUSED void ls_env_trace10(ls_native_object *owner, ls_native_visit visit, void *context) {
ls_env10 *environment = (ls_env10 *)owner;
visit(environment->ls_e0, context);
}
typedef struct {
ls_native_object owner;
ls_box32 *ls_e0;
} ls_env11;
static LS_NATIVE_UNUSED void ls_env_destroy11(ls_native_object *owner) {
ls_env11 *environment = (ls_env11 *)owner;
ls_native_release(environment->ls_e0);
}
static LS_NATIVE_UNUSED void ls_env_trace11(ls_native_object *owner, ls_native_visit visit, void *context) {
ls_env11 *environment = (ls_env11 *)owner;
visit(environment->ls_e0, context);
}
static LS_NATIVE_UNUSED const uint16_t ls_s11[] = {115,101,101,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s12[] = {58,};
static LS_NATIVE_UNUSED const uint16_t ls_s13[] = {103,111,110,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s14[] = {112,114,101,102,105,120,};
static LS_NATIVE_UNUSED const uint16_t ls_s15[] = {97,};
static LS_NATIVE_UNUSED const uint16_t ls_s17[] = {98,};
static LS_NATIVE_UNUSED const uint16_t ls_s19[] = {98,111,120,101,100,};
static LS_NATIVE_UNUSED const uint16_t ls_s20[] = {100,111,110,101,};
static LS_NATIVE_UNUSED void ls_init0(void);
static LS_NATIVE_UNUSED ls_t0 ls_fn1(ls_t0 ls_c12 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn2(void);
static LS_NATIVE_UNUSED void ls_fn3(ls_native_object * ls_c22 LS_NATIVE_UNUSED,ls_string ls_c23 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn4(void);
static LS_NATIVE_UNUSED ls_callable5 ls_fn5(void);
static LS_NATIVE_UNUSED ls_t1 ls_fn6(void);
static LS_NATIVE_UNUSED void ls_fn7(void);
static LS_NATIVE_UNUSED int32_t ls_fn8(int32_t ls_c31 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED int32_t ls_fn9(void *ls_env LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED int32_t ls_fn10(void *ls_env LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED int32_t ls_fn11(void *ls_env LS_NATIVE_UNUSED,int32_t ls_c33 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_callable5 ls_closure9(ls_box26 *ls_e0) {
ls_env9 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy9,ls_env_trace9);
ls_native_retain(ls_e0);
environment->ls_e0 = ls_e0;
return (ls_callable5){ls_fn9,environment,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED ls_callable5 ls_closure10(ls_box27 *ls_e0) {
ls_env10 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy10,ls_env_trace10);
ls_native_retain(ls_e0);
environment->ls_e0 = ls_e0;
return (ls_callable5){ls_fn10,environment,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED ls_callable8 ls_closure11(ls_box32 *ls_e0) {
ls_env11 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy11,ls_env_trace11);
ls_native_retain(ls_e0);
environment->ls_e0 = ls_e0;
return (ls_callable8){ls_fn11,environment,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED void ls_init0(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_c34 LS_NATIVE_UNUSED;
int32_t ls_v13 LS_NATIVE_UNUSED;
int32_t ls_v14 LS_NATIVE_UNUSED;
int32_t ls_v19 LS_NATIVE_UNUSED;
int32_t ls_v21 LS_NATIVE_UNUSED;
int32_t ls_v22 LS_NATIVE_UNUSED;
int32_t ls_v23 LS_NATIVE_UNUSED;
bool ls_v24 LS_NATIVE_UNUSED;
int32_t ls_v27 LS_NATIVE_UNUSED;
int32_t ls_v28 LS_NATIVE_UNUSED;
int32_t ls_v29 LS_NATIVE_UNUSED;
int32_t ls_v33 LS_NATIVE_UNUSED;
ls_string ls_v35 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn2();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn7();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v13 = ls_from_u32(UINT32_C(4));
ls_native_temporaries_clear(&ls_temps);
ls_v14 = ls_fn8(ls_v13);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v14);
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
ls_v21 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_c34 = ls_v21;
ls_native_temporaries_clear(&ls_temps);
ls_test47: ;
ls_native_temporaries_clear(&ls_temps);
ls_v22 = ls_c34;
ls_native_temporaries_clear(&ls_temps);
ls_v23 = ls_from_u32(UINT32_C(100));
ls_native_temporaries_clear(&ls_temps);
ls_v24 = ls_v22 < ls_v23;
ls_native_temporaries_clear(&ls_temps);
if (!ls_v24) goto ls_end47;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn4();
ls_native_temporaries_clear(&ls_temps);
ls_update47: LS_NATIVE_UNUSED;
ls_v27 = ls_c34;
ls_native_temporaries_clear(&ls_temps);
ls_v28 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v29 = ls_from_u32((uint32_t)((uint32_t)ls_v27 + (uint32_t)ls_v28));
ls_native_temporaries_clear(&ls_temps);
ls_c34 = ls_v29;
ls_native_temporaries_clear(&ls_temps);
goto ls_test47;
ls_end47: ;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
host_collectNative();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v33 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
host_assertNativeCount(ls_v33);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v35),(ls_string){ls_s20,sizeof ls_s20/sizeof *ls_s20,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v35);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v35);
}
static LS_NATIVE_UNUSED ls_t0 ls_fn1(ls_t0 ls_c12 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_t0_retain(ls_c12);
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
ls_array2 * ls_v3 LS_NATIVE_UNUSED = NULL;
int32_t ls_v4 LS_NATIVE_UNUSED;
int32_t ls_v5 LS_NATIVE_UNUSED;
ls_t0 ls_v6 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v7 LS_NATIVE_UNUSED = {0};
(void)(ls_c12);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v0),ls_c12.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v2),ls_string_slice(ls_v0,ls_v1,false,0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c12.ls_f0),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_v3),ls_c12.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v4 = ls_from_u32(UINT32_C(9));
ls_native_temporaries_clear(&ls_temps);
ls_v5 = ls_array2_push(ls_v3,ls_v4);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v6),ls_c12);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v7),ls_v6);
ls_native_temporaries_clear(&ls_temps);
{
ls_t0 ls_return = ls_v7;
ls_t0_retain(ls_return);
ls_t0_clear(&ls_v7);
ls_t0_clear(&ls_v6);
ls_array2_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v0);
ls_t0_clear(&ls_c12);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v7);
ls_t0_clear(&ls_v6);
ls_array2_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v0);
ls_t0_clear(&ls_c12);
}
static LS_NATIVE_UNUSED void ls_fn2(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_string ls_c13 LS_NATIVE_UNUSED = {0};
ls_string ls_c14 LS_NATIVE_UNUSED = {0};
int32_t ls_c15 LS_NATIVE_UNUSED;
ls_array0 * ls_c16 LS_NATIVE_UNUSED = NULL;
ls_string ls_c17 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c18 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c19 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_c20 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_c21 LS_NATIVE_UNUSED = {0};
ls_string ls_v1 LS_NATIVE_UNUSED = {0};
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_string ls_v4 LS_NATIVE_UNUSED = {0};
ls_string ls_v6 LS_NATIVE_UNUSED = {0};
int32_t ls_v7 LS_NATIVE_UNUSED;
int32_t ls_v8 LS_NATIVE_UNUSED;
int32_t ls_v9 LS_NATIVE_UNUSED;
bool ls_v10 LS_NATIVE_UNUSED;
int32_t ls_v11 LS_NATIVE_UNUSED;
ls_string ls_v12 LS_NATIVE_UNUSED = {0};
ls_string ls_v13 LS_NATIVE_UNUSED = {0};
int32_t ls_v14 LS_NATIVE_UNUSED;
int32_t ls_v15 LS_NATIVE_UNUSED;
ls_string ls_v16 LS_NATIVE_UNUSED = {0};
ls_string ls_v17 LS_NATIVE_UNUSED = {0};
int32_t ls_v18 LS_NATIVE_UNUSED;
int32_t ls_v19 LS_NATIVE_UNUSED;
int32_t ls_v20 LS_NATIVE_UNUSED;
ls_string ls_v21 LS_NATIVE_UNUSED = {0};
ls_string ls_v22 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v23 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v24 LS_NATIVE_UNUSED = NULL;
int32_t ls_v25 LS_NATIVE_UNUSED;
ls_string ls_v26 LS_NATIVE_UNUSED = {0};
ls_string ls_v28 LS_NATIVE_UNUSED = {0};
int32_t ls_v29 LS_NATIVE_UNUSED;
int32_t ls_v30 LS_NATIVE_UNUSED;
ls_string ls_v31 LS_NATIVE_UNUSED = {0};
ls_string ls_v32 LS_NATIVE_UNUSED = {0};
ls_string ls_v33 LS_NATIVE_UNUSED = {0};
ls_string ls_v35 LS_NATIVE_UNUSED = {0};
int32_t ls_v36 LS_NATIVE_UNUSED;
ls_string ls_v37 LS_NATIVE_UNUSED = {0};
int32_t ls_v38 LS_NATIVE_UNUSED;
int32_t ls_v39 LS_NATIVE_UNUSED;
ls_array2 * ls_v40 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_v41 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v42 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v44 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v45 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v46 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v47 LS_NATIVE_UNUSED = {0};
ls_string ls_v48 LS_NATIVE_UNUSED = {0};
ls_string ls_v50 LS_NATIVE_UNUSED = {0};
ls_array2 * ls_v52 LS_NATIVE_UNUSED = NULL;
int32_t ls_v53 LS_NATIVE_UNUSED;
ls_t0 ls_v55 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v56 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v57 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v58 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v59 LS_NATIVE_UNUSED = NULL;
ls_array1 * ls_v60 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_v61 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v62 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v63 LS_NATIVE_UNUSED = NULL;
int32_t ls_v64 LS_NATIVE_UNUSED;
ls_t0 ls_v65 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v66 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v67 LS_NATIVE_UNUSED = NULL;
int32_t ls_v68 LS_NATIVE_UNUSED;
ls_string ls_v69 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v1),host_makeHostText());
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c13),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v2),ls_c13);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v4),ls_string_slice(ls_v2,ls_v3,false,0));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v4);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v6),(ls_string){ls_s11,sizeof ls_s11/sizeof *ls_s11,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c14),ls_v6);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v7 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_c15 = ls_v7;
ls_native_temporaries_clear(&ls_temps);
ls_test30: ;
ls_native_temporaries_clear(&ls_temps);
ls_v8 = ls_c15;
ls_native_temporaries_clear(&ls_temps);
ls_v9 = ls_from_u32(UINT32_C(100));
ls_native_temporaries_clear(&ls_temps);
ls_v10 = ls_v8 < ls_v9;
ls_native_temporaries_clear(&ls_temps);
if (!ls_v10) goto ls_end30;
ls_v11 = ls_c15;
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v12),(ls_string){ls_s12,sizeof ls_s12/sizeof *ls_s12,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v13),ls_c14);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v14 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v15 = ls_from_u32(UINT32_C(3));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v16),ls_string_slice(ls_v13,ls_v14,true,ls_v15));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v17),ls_string_join_owned(3,(ls_string[]){ls_int_to_string(ls_v11),ls_string_hold(ls_v12),ls_string_hold(ls_v16)}));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c14),ls_v17);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v17);
ls_string_clear(&ls_v16);
ls_string_clear(&ls_v13);
ls_string_clear(&ls_v12);
ls_update30: LS_NATIVE_UNUSED;
ls_v18 = ls_c15;
ls_native_temporaries_clear(&ls_temps);
ls_v19 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v20 = ls_from_u32((uint32_t)((uint32_t)ls_v18 + (uint32_t)ls_v19));
ls_native_temporaries_clear(&ls_temps);
ls_c15 = ls_v20;
ls_native_temporaries_clear(&ls_temps);
goto ls_test30;
ls_end30: ;
ls_string_copy(&(ls_v21),ls_c14);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v22),(ls_string){ls_s12,sizeof ls_s12/sizeof *ls_s12,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_array0_take(&(ls_v23),ls_string_split(ls_v21,ls_v22));
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_c16),ls_v23);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v24),ls_c16);
ls_native_temporaries_clear(&ls_temps);
ls_v25 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v26),ls_array0_get(ls_v24,ls_v25,&ls_temps));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v26);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v28),ls_c14);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v29 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v30 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v31),ls_string_slice(ls_v28,ls_v29,true,ls_v30));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c17),ls_v31);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v32),(ls_string){ls_s13,sizeof ls_s13/sizeof *ls_s13,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c14),ls_v32);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v33),ls_c17);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v33);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v35),(ls_string){ls_s14,sizeof ls_s14/sizeof *ls_s14,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v36 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v37),ls_string_repeat(ls_v35,ls_v36));
ls_native_temporaries_clear(&ls_temps);
ls_v38 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v39 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_array2_take(&(ls_v40),ls_array2_new(2));
ls_array2_push(ls_v40,ls_v38);
ls_array2_push(ls_v40,ls_v39);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v41),(ls_t0){ls_v37,ls_v40});
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v42),ls_v41);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c18),ls_v42);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v44),ls_c18);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v45),ls_v44);
ls_native_temporaries_clear(&ls_temps);
ls_t0_take(&(ls_v46),ls_fn1(ls_v45));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v47),ls_v46);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c19),ls_v47);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v48),ls_c18.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v48);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v50),ls_c19.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v50);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array2_copy(&(ls_v52),ls_c18.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_v53 = (int32_t)ls_v52->length;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v53);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v55),ls_c18);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v56),ls_v55);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v57),ls_c19);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v58),ls_v57);
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v59),ls_array1_new(2));
ls_array1_push(ls_v59,ls_v56);
ls_array1_push(ls_v59,ls_v58);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_c20),ls_v59);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v60),ls_c20);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_take(&(ls_v61),ls_array1_pop(ls_v60));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v62),ls_v61);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c21),ls_v62);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v63),ls_c20);
ls_native_temporaries_clear(&ls_temps);
ls_v64 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v65),ls_c21);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v66),ls_v65);
ls_native_temporaries_clear(&ls_temps);
ls_array1_set(ls_v63,ls_v64,ls_v66);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v67),ls_c20);
ls_native_temporaries_clear(&ls_temps);
ls_v68 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v69),ls_array1_get(ls_v67,ls_v68,&ls_temps).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v69);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v69);
ls_array1_clear(&ls_v67);
ls_t0_clear(&ls_v66);
ls_t0_clear(&ls_v65);
ls_array1_clear(&ls_v63);
ls_t0_clear(&ls_c21);
ls_t0_clear(&ls_v62);
ls_t0_clear(&ls_v61);
ls_array1_clear(&ls_v60);
ls_array1_clear(&ls_c20);
ls_array1_clear(&ls_v59);
ls_t0_clear(&ls_v58);
ls_t0_clear(&ls_v57);
ls_t0_clear(&ls_v56);
ls_t0_clear(&ls_v55);
ls_array2_clear(&ls_v52);
ls_string_clear(&ls_v50);
ls_string_clear(&ls_v48);
ls_t0_clear(&ls_c19);
ls_t0_clear(&ls_v47);
ls_t0_clear(&ls_v46);
ls_t0_clear(&ls_v45);
ls_t0_clear(&ls_v44);
ls_t0_clear(&ls_c18);
ls_t0_clear(&ls_v42);
ls_t0_clear(&ls_v41);
ls_array2_clear(&ls_v40);
ls_string_clear(&ls_v37);
ls_string_clear(&ls_v35);
ls_string_clear(&ls_v33);
ls_string_clear(&ls_v32);
ls_string_clear(&ls_c17);
ls_string_clear(&ls_v31);
ls_string_clear(&ls_v28);
ls_string_clear(&ls_v26);
ls_array0_clear(&ls_v24);
ls_array0_clear(&ls_c16);
ls_array0_clear(&ls_v23);
ls_string_clear(&ls_v22);
ls_string_clear(&ls_v21);
ls_string_clear(&ls_c14);
ls_string_clear(&ls_v6);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_c13);
ls_string_clear(&ls_v1);
}
static LS_NATIVE_UNUSED void ls_fn3(ls_native_object * ls_c22 LS_NATIVE_UNUSED,ls_string ls_c23 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_retain(ls_c22);
ls_native_retain(ls_c23.owner);
ls_native_object * ls_v0 LS_NATIVE_UNUSED = NULL;
ls_string ls_v1 LS_NATIVE_UNUSED = {0};
ls_object_copy(&(ls_v0),ls_c22);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v1),ls_c23);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(((ls_object0 *)ls_v0)->ls_m2),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v1);
ls_object_clear(&ls_v0);
ls_string_clear(&ls_c23);
ls_object_clear(&ls_c22);
}
static LS_NATIVE_UNUSED void ls_fn4(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c24 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_c25 LS_NATIVE_UNUSED = NULL;
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_value ls_v4 LS_NATIVE_UNUSED = {0};
ls_string ls_v5 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v6 LS_NATIVE_UNUSED = NULL;
ls_string ls_v9 LS_NATIVE_UNUSED = {0};
int32_t ls_v10 LS_NATIVE_UNUSED;
ls_string ls_v11 LS_NATIVE_UNUSED = {0};
ls_value ls_v12 LS_NATIVE_UNUSED = {0};
ls_value ls_v13 LS_NATIVE_UNUSED = {0};
ls_string ls_v14 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v15 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v18 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v19 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v20 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v21 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v22 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v23 LS_NATIVE_UNUSED = NULL;
ls_string_take(&(ls_v0),(ls_string){ls_s15,sizeof ls_s15/sizeof *ls_s15,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v2),ls_string_repeat(ls_v0,ls_v1));
ls_native_temporaries_clear(&ls_temps);
ls_v3 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_v4 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v5),(ls_string){0});
ls_native_temporaries_clear(&ls_temps);
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy, ls_object0_trace);
((ls_object0 *)ls_o)->ls_m0 = ls_v3;
ls_value_retain(((ls_object0 *)ls_o)->ls_m0);
((ls_object0 *)ls_o)->ls_m1 = ls_v4;
ls_value_retain(((ls_object0 *)ls_o)->ls_m1);
((ls_object0 *)ls_o)->ls_m2 = ls_v5;
ls_native_retain(((ls_object0 *)ls_o)->ls_m2.owner);
ls_object_take(&(ls_v6),(ls_native_object *)ls_o);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn3(ls_v6,ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_c24),ls_v6);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v9),(ls_string){ls_s17,sizeof ls_s17/sizeof *ls_s17,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v10 = ls_from_u32(UINT32_C(3));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v11),ls_string_repeat(ls_v9,ls_v10));
ls_native_temporaries_clear(&ls_temps);
ls_v12 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_v13 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v14),(ls_string){0});
ls_native_temporaries_clear(&ls_temps);
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy, ls_object0_trace);
((ls_object0 *)ls_o)->ls_m0 = ls_v12;
ls_value_retain(((ls_object0 *)ls_o)->ls_m0);
((ls_object0 *)ls_o)->ls_m1 = ls_v13;
ls_value_retain(((ls_object0 *)ls_o)->ls_m1);
((ls_object0 *)ls_o)->ls_m2 = ls_v14;
ls_native_retain(((ls_object0 *)ls_o)->ls_m2.owner);
ls_object_take(&(ls_v15),(ls_native_object *)ls_o);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn3(ls_v15,ls_v11);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_c25),ls_v15);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v18),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v19),ls_c25);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(((ls_object0 *)ls_v18)->ls_m0),ls_value_object(ls_v19));
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v20),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v21),ls_c25);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(((ls_object0 *)ls_v20)->ls_m1),ls_value_object(ls_v21));
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v22),ls_c25);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v23),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(((ls_object0 *)ls_v22)->ls_m0),ls_value_object(ls_v23));
ls_native_temporaries_clear(&ls_temps);
ls_object_clear(&ls_v23);
ls_object_clear(&ls_v22);
ls_object_clear(&ls_v21);
ls_object_clear(&ls_v20);
ls_object_clear(&ls_v19);
ls_object_clear(&ls_v18);
ls_object_clear(&ls_c25);
ls_object_clear(&ls_v15);
ls_string_clear(&ls_v14);
ls_string_clear(&ls_v11);
ls_string_clear(&ls_v9);
ls_object_clear(&ls_c24);
ls_object_clear(&ls_v6);
ls_string_clear(&ls_v5);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v0);
}
static LS_NATIVE_UNUSED ls_callable5 ls_fn5(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_box26 *ls_c26 = NULL;
ls_box27 *ls_c27 = NULL;
int32_t ls_v0 LS_NATIVE_UNUSED;
ls_callable5 ls_v1 LS_NATIVE_UNUSED = {0};
ls_callable5 ls_v2 LS_NATIVE_UNUSED = {0};
ls_c26 = ls_box_allocate26();
ls_c27 = ls_box_allocate27();
ls_v0 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize26(ls_c26,ls_v0);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_take(&(ls_v1),ls_closure9(ls_c26));
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize27(ls_c27,ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_take(&(ls_v2),ls_closure10(ls_c27));
ls_native_temporaries_clear(&ls_temps);
{
ls_callable5 ls_return = ls_v2;
ls_native_retain(ls_return.environment);
ls_callable5_clear(&ls_v2);
ls_native_release(ls_c27);
ls_c27 = NULL;
ls_callable5_clear(&ls_v1);
ls_native_release(ls_c26);
ls_c26 = NULL;
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_callable5_clear(&ls_v2);
ls_native_release(ls_c27);
ls_c27 = NULL;
ls_callable5_clear(&ls_v1);
ls_native_release(ls_c26);
ls_c26 = NULL;
}
static LS_NATIVE_UNUSED ls_t1 ls_fn6(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_callable5 ls_v1 LS_NATIVE_UNUSED = {0};
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_string ls_v4 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v5 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v6 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_take(&(ls_v1),ls_fn5());
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v2),(ls_string){ls_s19,sizeof ls_s19/sizeof *ls_s19,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v4),ls_string_repeat(ls_v2,ls_v3));
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v5),(ls_t1){ls_v1,ls_v4});
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v6),ls_v5);
ls_native_temporaries_clear(&ls_temps);
{
ls_t1 ls_return = ls_v6;
ls_t1_retain(ls_return);
ls_t1_clear(&ls_v6);
ls_t1_clear(&ls_v5);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v2);
ls_callable5_clear(&ls_v1);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t1_clear(&ls_v6);
ls_t1_clear(&ls_v5);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v2);
ls_callable5_clear(&ls_v1);
}
static LS_NATIVE_UNUSED void ls_fn7(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_callable5 ls_c28 LS_NATIVE_UNUSED = {0};
ls_t1 ls_c29 LS_NATIVE_UNUSED = {0};
ls_t1 ls_c30 LS_NATIVE_UNUSED = {0};
ls_callable5 ls_v1 LS_NATIVE_UNUSED = {0};
ls_callable5 ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_callable5 ls_v5 LS_NATIVE_UNUSED = {0};
int32_t ls_v6 LS_NATIVE_UNUSED;
ls_t1 ls_v9 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v10 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v11 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v12 LS_NATIVE_UNUSED = {0};
ls_callable5 ls_v13 LS_NATIVE_UNUSED = {0};
int32_t ls_v14 LS_NATIVE_UNUSED;
ls_callable5 ls_v16 LS_NATIVE_UNUSED = {0};
int32_t ls_v17 LS_NATIVE_UNUSED;
ls_string ls_v19 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_take(&(ls_v1),ls_fn5());
ls_native_temporaries_clear(&ls_temps);
ls_callable5_copy(&(ls_c28),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_copy(&(ls_v2),ls_c28);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_v2.code(ls_v2.environment);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v3);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_copy(&(ls_v5),ls_c28);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v6 = ls_v5.code(ls_v5.environment);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v6);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t1_take(&(ls_v9),ls_fn6());
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v10),ls_v9);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_c29),ls_v10);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v11),ls_c29);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v12),ls_v11);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_c30),ls_v12);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_copy(&(ls_v13),ls_c30.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v14 = ls_v13.code(ls_v13.environment);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v14);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable5_copy(&(ls_v16),ls_c29.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v17 = ls_v16.code(ls_v16.environment);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v17);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v19),ls_c30.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v19);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v19);
ls_callable5_clear(&ls_v16);
ls_callable5_clear(&ls_v13);
ls_t1_clear(&ls_c30);
ls_t1_clear(&ls_v12);
ls_t1_clear(&ls_v11);
ls_t1_clear(&ls_c29);
ls_t1_clear(&ls_v10);
ls_t1_clear(&ls_v9);
ls_callable5_clear(&ls_v5);
ls_callable5_clear(&ls_v2);
ls_callable5_clear(&ls_c28);
ls_callable5_clear(&ls_v1);
}
static LS_NATIVE_UNUSED int32_t ls_fn8(int32_t ls_c31 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_box32 *ls_c32 = NULL;
ls_callable8 ls_v0 LS_NATIVE_UNUSED = {0};
ls_callable8 ls_v1 LS_NATIVE_UNUSED = {0};
int32_t ls_v2 LS_NATIVE_UNUSED;
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_c32 = ls_box_allocate32();
ls_callable8_take(&(ls_v0),ls_closure11(ls_c32));
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize32(ls_c32,ls_v0);
ls_native_temporaries_clear(&ls_temps);
ls_callable8_copy(&(ls_v1),(*ls_box_value32(ls_c32)));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_c31;
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_v1.code(ls_v1.environment,ls_v2);
ls_native_temporaries_clear(&ls_temps);
{
int32_t ls_return = ls_v3;
ls_callable8_clear(&ls_v1);
ls_native_release(ls_c32);
ls_c32 = NULL;
ls_callable8_clear(&ls_v0);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_callable8_clear(&ls_v1);
ls_native_release(ls_c32);
ls_c32 = NULL;
ls_callable8_clear(&ls_v0);
}
static LS_NATIVE_UNUSED int32_t ls_fn9(void *ls_env LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_v0 LS_NATIVE_UNUSED;
int32_t ls_v1 LS_NATIVE_UNUSED;
int32_t ls_v2 LS_NATIVE_UNUSED;
int32_t ls_v3 LS_NATIVE_UNUSED;
ls_v0 = (*ls_box_value26(((ls_env9 *)ls_env)->ls_e0));
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_from_u32((uint32_t)((uint32_t)ls_v0 + (uint32_t)ls_v1));
ls_native_temporaries_clear(&ls_temps);
(*ls_box_value26(((ls_env9 *)ls_env)->ls_e0)) = ls_v2;
ls_native_temporaries_clear(&ls_temps);
ls_v3 = (*ls_box_value26(((ls_env9 *)ls_env)->ls_e0));
ls_native_temporaries_clear(&ls_temps);
{
int32_t ls_return = ls_v3;
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
}
static LS_NATIVE_UNUSED int32_t ls_fn10(void *ls_env LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_callable5 ls_v0 LS_NATIVE_UNUSED = {0};
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_callable5_copy(&(ls_v0),(*ls_box_value27(((ls_env10 *)ls_env)->ls_e0)));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_v0.code(ls_v0.environment);
ls_native_temporaries_clear(&ls_temps);
{
int32_t ls_return = ls_v1;
ls_callable5_clear(&ls_v0);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_callable5_clear(&ls_v0);
}
static LS_NATIVE_UNUSED int32_t ls_fn11(void *ls_env LS_NATIVE_UNUSED,int32_t ls_c33 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_v0 LS_NATIVE_UNUSED;
int32_t ls_v1 LS_NATIVE_UNUSED;
bool ls_v2 LS_NATIVE_UNUSED;
int32_t ls_v3 LS_NATIVE_UNUSED;
int32_t ls_v4 LS_NATIVE_UNUSED;
ls_callable8 ls_v5 LS_NATIVE_UNUSED = {0};
int32_t ls_v6 LS_NATIVE_UNUSED;
int32_t ls_v7 LS_NATIVE_UNUSED;
int32_t ls_v8 LS_NATIVE_UNUSED;
int32_t ls_v9 LS_NATIVE_UNUSED;
int32_t ls_v10 LS_NATIVE_UNUSED;
ls_v0 = ls_c33;
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_v0 <= ls_v1;
ls_native_temporaries_clear(&ls_temps);
if (ls_v2) {
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
{
int32_t ls_return = ls_v3;
ls_callable8_clear(&ls_v5);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
}
ls_v4 = ls_c33;
ls_native_temporaries_clear(&ls_temps);
ls_callable8_copy(&(ls_v5),(*ls_box_value32(((ls_env11 *)ls_env)->ls_e0)));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v6 = ls_c33;
ls_native_temporaries_clear(&ls_temps);
ls_v7 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v8 = ls_from_u32((uint32_t)((uint32_t)ls_v6 - (uint32_t)ls_v7));
ls_native_temporaries_clear(&ls_temps);
ls_v9 = ls_v5.code(ls_v5.environment,ls_v8);
ls_native_temporaries_clear(&ls_temps);
ls_v10 = ls_from_u32((uint32_t)((uint32_t)ls_v4 + (uint32_t)ls_v9));
ls_native_temporaries_clear(&ls_temps);
{
int32_t ls_return = ls_v10;
ls_callable8_clear(&ls_v5);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_callable8_clear(&ls_v5);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(12);
ls_init0();
fflush(stdout);
ls_native_collect_cycles();
return 0;
}
