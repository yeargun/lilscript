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
typedef struct ls_t0 ls_t0;
typedef struct ls_t1 ls_t1;
typedef struct ls_t2 ls_t2;
typedef struct ls_array0 ls_array0;
typedef struct ls_array1 ls_array1;
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
typedef struct {
ls_value (*code)(void *,ls_value);
void *environment;
uint64_t identity;
} ls_callable3;
typedef struct {
ls_t0 (*code)(void *,ls_t0);
void *environment;
uint64_t identity;
} ls_callable5;
typedef struct {
bool (*code)(void *,ls_t0);
void *environment;
uint64_t identity;
} ls_callable13;
struct ls_t0 {
ls_string ls_f0;
ls_array1 * ls_f1;
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
ls_value ls_f0;
int32_t ls_f1;
};
static LS_NATIVE_UNUSED inline void ls_t1_retain(ls_t1 value) { (void)value;
ls_value_retain(value.ls_f0);
}
static LS_NATIVE_UNUSED inline void ls_t1_release(ls_t1 value) { (void)value;
ls_value_release(value.ls_f0);
}
static LS_NATIVE_UNUSED inline void ls_t1_trace(ls_t1 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
ls_value_trace(value.ls_f0, visit, context);
}
static LS_NATIVE_UNUSED inline void ls_t1_copy(ls_t1 *slot, ls_t1 value) { ls_t1_retain(value); ls_t1_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t1_take(ls_t1 *slot, ls_t1 value) { ls_t1_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED inline void ls_t1_clear(ls_t1 *slot) { ls_t1_release(*slot); *slot = (ls_t1){0}; }
struct ls_t2 {
ls_value ls_f0;
ls_value ls_f1;
};
static LS_NATIVE_UNUSED inline void ls_t2_retain(ls_t2 value) { (void)value;
ls_value_retain(value.ls_f0);
ls_value_retain(value.ls_f1);
}
static LS_NATIVE_UNUSED inline void ls_t2_release(ls_t2 value) { (void)value;
ls_value_release(value.ls_f0);
ls_value_release(value.ls_f1);
}
static LS_NATIVE_UNUSED inline void ls_t2_trace(ls_t2 value, ls_native_visit visit, void *context) { (void)value; (void)visit; (void)context;
ls_value_trace(value.ls_f0, visit, context);
ls_value_trace(value.ls_f1, visit, context);
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
static LS_NATIVE_UNUSED inline ls_value ls_callable3_call(ls_callable3 value,ls_value ls_p0) {
ls_native_retain(value.environment);
ls_value result = value.code(value.environment,ls_p0);
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
static LS_NATIVE_UNUSED inline ls_t0 ls_callable5_call(ls_callable5 value,ls_t0 ls_p0) {
ls_native_retain(value.environment);
ls_t0 result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable13 ls_callable13_retain(ls_callable13 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable13_release(ls_callable13 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable13_copy(ls_callable13 *destination, ls_callable13 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable13_take(ls_callable13 *destination, ls_callable13 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable13_clear(ls_callable13 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable13){0};
}
static LS_NATIVE_UNUSED inline bool ls_callable13_call(ls_callable13 value,ls_t0 ls_p0) {
ls_native_retain(value.environment);
bool result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
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
_Static_assert(sizeof(ls_callable13) == sizeof(((ls_value *)0)->as.c), "callable layout");
static LS_NATIVE_UNUSED ls_value ls_value_callable13(ls_callable13 value) { ls_value result = {.tag = LS_CALLABLE, .signature = 13}; memcpy(&result.as.c, &value, sizeof value); return result; }
static LS_NATIVE_UNUSED ls_callable13 ls_value_to_callable13(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable13){0};
if (value.tag != LS_CALLABLE || value.signature != 13) ls_value_mismatch();
ls_callable13 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
typedef struct { ls_native_object owner; ls_callable5 inner; } ls_adapter5_3;
static LS_NATIVE_UNUSED void ls_adapter5_3_destroy(ls_native_object *owner) { ls_callable5_clear(&((ls_adapter5_3 *)owner)->inner); }
static LS_NATIVE_UNUSED void ls_adapter5_3_trace(ls_native_object *owner, ls_native_visit visit, void *context) { visit(((ls_adapter5_3 *)owner)->inner.environment, context); }
static LS_NATIVE_UNUSED ls_value ls_adapter5_3_code(void *environment,ls_value ls_p0) {
ls_adapter5_3 *adapter = environment;
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_t0 ls_inner = adapter->inner.code(adapter->inner.environment,ls_value_to_t0(ls_p0));
ls_value ls_outer = ls_t0_box(&ls_temps,ls_inner);
ls_value_retain(ls_outer);
ls_t0_release(ls_inner);
ls_native_temporaries_clear(&ls_temps);
return ls_outer;
}
static LS_NATIVE_UNUSED ls_callable3 ls_adapt5_3(ls_callable5 inner) {
ls_adapter5_3 *adapter = ls_native_allocate(sizeof *adapter, ls_adapter5_3_destroy, ls_adapter5_3_trace);
ls_native_retain(inner.environment);
adapter->inner = inner;
return (ls_callable3){ls_adapter5_3_code, adapter, inner.identity};
}
static LS_NATIVE_UNUSED ls_value ls_adapt_value5_3(ls_value value) {
if (value.tag != LS_CALLABLE) { ls_value_retain(value); return value; }
return ls_value_callable3(ls_adapt5_3(ls_value_to_callable5(value)));
}
static LS_NATIVE_UNUSED void ls_native_undefined_element(void) {
fputs("LilScript native array element is undefined\n", stderr);
abort();
}
static LS_NATIVE_UNUSED size_t ls_array_relative(int32_t index, size_t length) {
if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
return (size_t)index < length ? (size_t)index : length;
}
struct ls_array0 { ls_native_object owner; size_t length; size_t capacity; ls_t0 *items; };
static LS_NATIVE_UNUSED void ls_array0_acquire(ls_t0 value) { (void)value; ls_t0_retain(value);
}
static LS_NATIVE_UNUSED void ls_array0_drop(ls_t0 value) { (void)value; ls_t0_release(value);
}
static LS_NATIVE_UNUSED void ls_array0_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_array0 *array = (ls_array0 *)owner; (void)visit; (void)context; for (size_t index = 0; index < array->length; ++index) { ls_t0_trace(array->items[index], visit, context);
 } }
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
ls_t0 *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static LS_NATIVE_UNUSED int32_t ls_array0_push_owned(ls_array0 *array, ls_t0 value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array0_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array0_push(ls_array0 *array, ls_t0 value) {
ls_array0_acquire(value);
return ls_array0_push_owned(array, value);
}
static LS_NATIVE_UNUSED void ls_array0_hole(ls_array0 *array) { (void)array; ls_native_undefined_element(); }
static LS_NATIVE_UNUSED ls_t0 ls_array0_get(ls_array0 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { ls_native_undefined_element(); }
return array->items[index];
}
static LS_NATIVE_UNUSED void ls_array0_set(ls_array0 *array, int32_t index, ls_t0 value) {
if (index >= 0 && (size_t)index < array->length) { ls_array0_acquire(value); ls_array0_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array0_push(array, value); return; }
ls_native_undefined_element();
}
static LS_NATIVE_UNUSED ls_t0 ls_array0_pop(ls_array0 *array) {
if (!array->length) { ls_native_undefined_element(); return array->items[0]; }
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
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { ls_t0 value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array0 *ls_array0_fill(ls_array0 *array, ls_t0 value) {
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
static LS_NATIVE_UNUSED ls_value ls_array0_optional(ls_array0 *array, int32_t index, ls_native_temporary **ls_temps) {
if (index < 0 || (size_t)index >= array->length) return (ls_value){0};
return ls_t0_box(ls_temps,array->items[index]);
}
struct ls_array1 { ls_native_object owner; size_t length; size_t capacity; int32_t *items; };
static LS_NATIVE_UNUSED void ls_array1_acquire(int32_t value) { (void)value; }
static LS_NATIVE_UNUSED void ls_array1_drop(int32_t value) { (void)value; }
static LS_NATIVE_UNUSED void ls_array1_trace(ls_native_object *owner, ls_native_visit visit, void *context) { ls_array1 *array = (ls_array1 *)owner; (void)visit; (void)context; for (size_t index = 0; index < array->length; ++index) {  } }
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
int32_t *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static LS_NATIVE_UNUSED int32_t ls_array1_push_owned(ls_array1 *array, int32_t value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array1_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static LS_NATIVE_UNUSED int32_t ls_array1_push(ls_array1 *array, int32_t value) {
ls_array1_acquire(value);
return ls_array1_push_owned(array, value);
}
static LS_NATIVE_UNUSED void ls_array1_hole(ls_array1 *array) { ls_array1_push(array, 0); }
static LS_NATIVE_UNUSED int32_t ls_array1_get(ls_array1 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { ls_native_undefined_element(); }
return array->items[index];
}
static LS_NATIVE_UNUSED void ls_array1_set(ls_array1 *array, int32_t index, int32_t value) {
if (index >= 0 && (size_t)index < array->length) { ls_array1_acquire(value); ls_array1_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array1_push(array, value); return; }
ls_native_undefined_element();
}
static LS_NATIVE_UNUSED int32_t ls_array1_pop(ls_array1 *array) {
if (!array->length) { return 0; }
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
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { int32_t value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static LS_NATIVE_UNUSED ls_array1 *ls_array1_fill(ls_array1 *array, int32_t value) {
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
return ls_value_int(array->items[index]);
}
static LS_NATIVE_UNUSED int32_t ls_array1_index_of(ls_array1 *array, int32_t right) {
for (size_t index = 0; index < array->length; index++) { int32_t left = array->items[index]; if (left == right) return (int32_t)index; }
return -1;
}
static LS_NATIVE_UNUSED bool ls_array1_includes(ls_array1 *array, int32_t right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { int32_t left = array->items[index]; if (left == right) return true; }
return false;
}
static LS_NATIVE_UNUSED void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static LS_NATIVE_UNUSED void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef int32_t host_assertNativeCount_arg0;
typedef void host_assertNativeCount_result;
void host_assertNativeCount(int32_t);
typedef void host_collectNative_result;
void host_collectNative(void);
typedef int32_t host_assertMaxNativeCount_arg0;
typedef void host_assertMaxNativeCount_result;
void host_assertMaxNativeCount(int32_t);
typedef struct { ls_native_object owner; ls_t0 value; bool ready; } ls_box21;
static LS_NATIVE_UNUSED void ls_box_destroy21(ls_native_object *owner) { ls_t0_clear(&((ls_box21 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_box_trace21(ls_native_object *owner, ls_native_visit visit, void *context) { ls_t0_trace(((ls_box21 *)owner)->value, visit, context);
 }
static LS_NATIVE_UNUSED ls_box21 *ls_box_allocate21(void) { return ls_native_allocate(sizeof(ls_box21),ls_box_destroy21,ls_box_trace21); }
static LS_NATIVE_UNUSED void ls_box_initialize21(ls_box21 *box, ls_t0 value) {
ls_t0_retain(value);
ls_t0_release(box->value);
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box21 *ls_box_new21(ls_t0 value) { ls_box21 *box = ls_box_allocate21(); ls_box_initialize21(box, value); return box; }
static LS_NATIVE_UNUSED ls_t0 *ls_box_value21(ls_box21 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct { ls_native_object owner; ls_array0 * value; bool ready; } ls_box39;
static LS_NATIVE_UNUSED void ls_box_destroy39(ls_native_object *owner) { ls_array0_clear(&((ls_box39 *)owner)->value); }
static LS_NATIVE_UNUSED void ls_box_trace39(ls_native_object *owner, ls_native_visit visit, void *context) { visit(((ls_box39 *)owner)->value, context);
 }
static LS_NATIVE_UNUSED ls_box39 *ls_box_allocate39(void) { return ls_native_allocate(sizeof(ls_box39),ls_box_destroy39,ls_box_trace39); }
static LS_NATIVE_UNUSED void ls_box_initialize39(ls_box39 *box, ls_array0 * value) {
ls_native_retain(value);
ls_native_release(box->value);
box->value = value; box->ready = true;
}
static LS_NATIVE_UNUSED ls_box39 *ls_box_new39(ls_array0 * value) { ls_box39 *box = ls_box_allocate39(); ls_box_initialize39(box, value); return box; }
static LS_NATIVE_UNUSED ls_array0 * *ls_box_value39(ls_box39 *box) { if (!box || !box->ready) { fputs("LilScript native captured binding used before initialization\n", stderr); abort(); } return &box->value; }
typedef struct {
ls_native_object owner;
ls_box21 *ls_e0;
ls_box39 *ls_e1;
} ls_env9;
static LS_NATIVE_UNUSED void ls_env_destroy9(ls_native_object *owner) {
ls_env9 *environment = (ls_env9 *)owner;
ls_native_release(environment->ls_e0);
ls_native_release(environment->ls_e1);
}
static LS_NATIVE_UNUSED void ls_env_trace9(ls_native_object *owner, ls_native_visit visit, void *context) {
ls_env9 *environment = (ls_env9 *)owner;
visit(environment->ls_e0, context);
visit(environment->ls_e1, context);
}
static LS_NATIVE_UNUSED const uint16_t ls_s0[] = {118,97,108,117,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s10[] = {33,};
static LS_NATIVE_UNUSED const uint16_t ls_s11[] = {63,};
static LS_NATIVE_UNUSED const uint16_t ls_s12[] = {43,};
static LS_NATIVE_UNUSED const uint16_t ls_s13[] = {98,97,115,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s14[] = {110,101,119,};
static LS_NATIVE_UNUSED const uint16_t ls_s15[] = {103,101,110,101,114,105,99,};
static LS_NATIVE_UNUSED const uint16_t ls_s16[] = {111,116,104,101,114,};
static LS_NATIVE_UNUSED const uint16_t ls_s17[] = {112,97,105,114,};
static LS_NATIVE_UNUSED const uint16_t ls_s18[] = {111,110,101,};
static LS_NATIVE_UNUSED const uint16_t ls_s19[] = {108,111,99,97,108,};
static LS_NATIVE_UNUSED const uint16_t ls_s20[] = {97,114,114,97,121,};
static LS_NATIVE_UNUSED const uint16_t ls_s21[] = {104,111,108,100,101,114,};
static LS_NATIVE_UNUSED const uint16_t ls_s23[] = {104,101,97,112,};
static LS_NATIVE_UNUSED const uint16_t ls_s24[] = {112,114,111,100,117,99,116,115,32,100,111,110,101,};
static LS_NATIVE_UNUSED void ls_init0(void);
static LS_NATIVE_UNUSED ls_value ls_fn1(ls_value ls_c13 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_value ls_fn2(ls_value ls_c14 LS_NATIVE_UNUSED,ls_callable3 ls_c15 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_t0 ls_fn3(ls_t0 ls_c16 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_value ls_fn4(ls_t0 ls_c17 LS_NATIVE_UNUSED,bool ls_c18 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_value ls_fn5(ls_value ls_c19 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED void ls_fn6(void);
static LS_NATIVE_UNUSED void ls_fn8(void);
static LS_NATIVE_UNUSED bool ls_fn9(void *ls_env LS_NATIVE_UNUSED,ls_t0 ls_c44 LS_NATIVE_UNUSED);
static LS_NATIVE_UNUSED ls_t0 ls_adapter3(void *environment,ls_t0 ls_p0) {
(void)environment;
return ls_fn3(ls_p0);
}
static LS_NATIVE_UNUSED ls_callable13 ls_closure9(ls_box21 *ls_e0,ls_box39 *ls_e1) {
ls_env9 *environment = ls_native_allocate(sizeof *environment,ls_env_destroy9,ls_env_trace9);
ls_native_retain(ls_e0);
environment->ls_e0 = ls_e0;
ls_native_retain(ls_e1);
environment->ls_e1 = ls_e1;
return (ls_callable13){ls_fn9,environment,ls_native_fresh_identity()};
}
static LS_NATIVE_UNUSED void ls_init0(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_c49 LS_NATIVE_UNUSED;
int32_t ls_v12 LS_NATIVE_UNUSED;
int32_t ls_v19 LS_NATIVE_UNUSED;
int32_t ls_v21 LS_NATIVE_UNUSED;
int32_t ls_v22 LS_NATIVE_UNUSED;
int32_t ls_v23 LS_NATIVE_UNUSED;
bool ls_v24 LS_NATIVE_UNUSED;
int32_t ls_v25 LS_NATIVE_UNUSED;
int32_t ls_v26 LS_NATIVE_UNUSED;
int32_t ls_v27 LS_NATIVE_UNUSED;
int32_t ls_v31 LS_NATIVE_UNUSED;
ls_string ls_v33 LS_NATIVE_UNUSED = {0};
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn6();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
host_collectNative();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v12 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
host_assertNativeCount(ls_v12);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_fn8();
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
ls_c49 = ls_v21;
ls_native_temporaries_clear(&ls_temps);
ls_test43: ;
ls_native_temporaries_clear(&ls_temps);
ls_v22 = ls_c49;
ls_native_temporaries_clear(&ls_temps);
ls_v23 = ls_from_u32(UINT32_C(200));
ls_native_temporaries_clear(&ls_temps);
ls_v24 = ls_v22 < ls_v23;
ls_native_temporaries_clear(&ls_temps);
if (!ls_v24) goto ls_end43;
ls_update43: LS_NATIVE_UNUSED;
ls_v25 = ls_c49;
ls_native_temporaries_clear(&ls_temps);
ls_v26 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v27 = ls_from_u32((uint32_t)((uint32_t)ls_v25 + (uint32_t)ls_v26));
ls_native_temporaries_clear(&ls_temps);
ls_c49 = ls_v27;
ls_native_temporaries_clear(&ls_temps);
goto ls_test43;
ls_end43: ;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
host_collectNative();
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v31 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
host_assertNativeCount(ls_v31);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v33),(ls_string){ls_s24,sizeof ls_s24/sizeof *ls_s24,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v33);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v33);
}
static LS_NATIVE_UNUSED ls_value ls_fn1(ls_value ls_c13 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c13);
ls_value ls_v0 LS_NATIVE_UNUSED = {0};
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value_copy(&(ls_v0),ls_c13);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_v0);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_v1;
ls_value_retain(ls_return);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c13);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c13);
}
static LS_NATIVE_UNUSED ls_value ls_fn2(ls_value ls_c14 LS_NATIVE_UNUSED,ls_callable3 ls_c15 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c14);
ls_native_retain(ls_c15.environment);
ls_callable3 ls_v0 LS_NATIVE_UNUSED = {0};
ls_value ls_v1 LS_NATIVE_UNUSED = {0};
ls_value ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_value ls_v4 LS_NATIVE_UNUSED = {0};
ls_callable3_copy(&(ls_v0),ls_c15);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v1),ls_c14);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v2),ls_v1);
ls_native_temporaries_clear(&ls_temps);
ls_value_take(&(ls_v3),ls_v0.code(ls_v0.environment,ls_v2));
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
ls_callable3_clear(&ls_v0);
ls_callable3_clear(&ls_c15);
ls_value_clear(&ls_c14);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v4);
ls_value_clear(&ls_v3);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_callable3_clear(&ls_v0);
ls_callable3_clear(&ls_c15);
ls_value_clear(&ls_c14);
}
static LS_NATIVE_UNUSED ls_t0 ls_fn3(ls_t0 ls_c16 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_t0_retain(ls_c16);
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
ls_string ls_v1 LS_NATIVE_UNUSED = {0};
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v3 LS_NATIVE_UNUSED = NULL;
int32_t ls_v4 LS_NATIVE_UNUSED;
int32_t ls_v5 LS_NATIVE_UNUSED;
ls_t0 ls_v6 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v7 LS_NATIVE_UNUSED = {0};
ls_string_copy(&(ls_v0),ls_c16.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v1),(ls_string){ls_s10,sizeof ls_s10/sizeof *ls_s10,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v2),ls_string_join_owned(2,(ls_string[]){ls_string_hold(ls_v0),ls_string_hold(ls_v1)}));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c16.ls_f0),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v3),ls_c16.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v4 = ls_from_u32(UINT32_C(8));
ls_native_temporaries_clear(&ls_temps);
ls_v5 = ls_array1_push(ls_v3,ls_v4);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v6),ls_c16);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v7),ls_v6);
ls_native_temporaries_clear(&ls_temps);
{
ls_t0 ls_return = ls_v7;
ls_t0_retain(ls_return);
ls_t0_clear(&ls_v7);
ls_t0_clear(&ls_v6);
ls_array1_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v1);
ls_string_clear(&ls_v0);
ls_t0_clear(&ls_c16);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v7);
ls_t0_clear(&ls_v6);
ls_array1_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v1);
ls_string_clear(&ls_v0);
ls_t0_clear(&ls_c16);
}
static LS_NATIVE_UNUSED ls_value ls_fn4(ls_t0 ls_c17 LS_NATIVE_UNUSED,bool ls_c18 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_t0_retain(ls_c17);
bool ls_v0 LS_NATIVE_UNUSED;
ls_t0 ls_v1 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v2 LS_NATIVE_UNUSED = {0};
ls_value ls_v3 LS_NATIVE_UNUSED = {0};
ls_v0 = ls_c18;
ls_native_temporaries_clear(&ls_temps);
if (ls_v0) {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v1),ls_c17);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v2),ls_v1);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_t0_box(&ls_temps,ls_v2);
ls_value_retain(ls_return);
ls_t0_clear(&ls_v2);
ls_t0_clear(&ls_v1);
ls_t0_clear(&ls_c17);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v2);
ls_t0_clear(&ls_v1);
}
ls_v3 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_v3;
ls_value_retain(ls_return);
ls_t0_clear(&ls_c17);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_c17);
}
static LS_NATIVE_UNUSED ls_value ls_fn5(ls_value ls_c19 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_value_retain(ls_c19);
ls_t0 ls_c20 LS_NATIVE_UNUSED = {0};
ls_value ls_v0 LS_NATIVE_UNUSED = {0};
bool ls_v1 LS_NATIVE_UNUSED;
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
ls_string ls_v3 LS_NATIVE_UNUSED = {0};
ls_string ls_v4 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v5 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v6 LS_NATIVE_UNUSED = {0};
ls_string ls_v7 LS_NATIVE_UNUSED = {0};
ls_string ls_v8 LS_NATIVE_UNUSED = {0};
ls_string ls_v9 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v10 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v11 LS_NATIVE_UNUSED = {0};
ls_value_copy(&(ls_v0),ls_c19);
ls_native_temporaries_clear(&ls_temps);
{ uint8_t ls_t = ls_v0.tag; ls_v1 = ls_t == LS_STRING; }
ls_native_temporaries_clear(&ls_temps);
if (ls_v1) {
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v2),ls_value_to_string(ls_c19));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v3),(ls_string){ls_s11,sizeof ls_s11/sizeof *ls_s11,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v4),ls_string_join_owned(2,(ls_string[]){ls_string_hold(ls_v2),ls_string_hold(ls_v3)}));
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_value_string(ls_v4);
ls_value_retain(ls_return);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v3);
ls_string_clear(&ls_v2);
ls_t0_clear(&ls_v11);
ls_t0_clear(&ls_v10);
ls_string_clear(&ls_v9);
ls_string_clear(&ls_v8);
ls_string_clear(&ls_v7);
ls_t0_clear(&ls_c20);
ls_t0_clear(&ls_v6);
ls_t0_clear(&ls_v5);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c19);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v4);
ls_string_clear(&ls_v3);
ls_string_clear(&ls_v2);
}
ls_t0_copy(&(ls_v5),ls_value_to_t0(ls_c19));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v6),ls_v5);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c20),ls_v6);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v7),ls_c20.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v8),(ls_string){ls_s12,sizeof ls_s12/sizeof *ls_s12,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v9),ls_string_join_owned(2,(ls_string[]){ls_string_hold(ls_v7),ls_string_hold(ls_v8)}));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c20.ls_f0),ls_v9);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v10),ls_c20);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v11),ls_v10);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_return = ls_t0_box(&ls_temps,ls_v11);
ls_value_retain(ls_return);
ls_t0_clear(&ls_v11);
ls_t0_clear(&ls_v10);
ls_string_clear(&ls_v9);
ls_string_clear(&ls_v8);
ls_string_clear(&ls_v7);
ls_t0_clear(&ls_c20);
ls_t0_clear(&ls_v6);
ls_t0_clear(&ls_v5);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c19);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v11);
ls_t0_clear(&ls_v10);
ls_string_clear(&ls_v9);
ls_string_clear(&ls_v8);
ls_string_clear(&ls_v7);
ls_t0_clear(&ls_c20);
ls_t0_clear(&ls_v6);
ls_t0_clear(&ls_v5);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c19);
}
static LS_NATIVE_UNUSED void ls_fn6(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_box21 *ls_c21 = NULL;
ls_t0 ls_c22 LS_NATIVE_UNUSED = {0};
ls_value ls_c23 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c24 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c25 LS_NATIVE_UNUSED = {0};
ls_t1 ls_c26 LS_NATIVE_UNUSED = {0};
ls_t1 ls_c27 LS_NATIVE_UNUSED = {0};
ls_t1 ls_c28 LS_NATIVE_UNUSED = {0};
ls_t1 ls_c29 LS_NATIVE_UNUSED = {0};
ls_t2 ls_c30 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c31 LS_NATIVE_UNUSED = {0};
ls_value ls_c32 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c33 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_c34 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_c35 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c36 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_c37 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_c38 LS_NATIVE_UNUSED = {0};
ls_box39 *ls_c39 = NULL;
ls_value ls_c40 LS_NATIVE_UNUSED = {0};
ls_t0 ls_c41 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_c42 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_c43 LS_NATIVE_UNUSED = NULL;
ls_string ls_v0 LS_NATIVE_UNUSED = {0};
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_string ls_v2 LS_NATIVE_UNUSED = {0};
int32_t ls_v3 LS_NATIVE_UNUSED;
int32_t ls_v4 LS_NATIVE_UNUSED;
ls_array1 * ls_v5 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_v6 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v7 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v10 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v11 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v12 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v13 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v14 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v15 LS_NATIVE_UNUSED = {0};
ls_string ls_v16 LS_NATIVE_UNUSED = {0};
ls_string ls_v18 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v20 LS_NATIVE_UNUSED = NULL;
int32_t ls_v21 LS_NATIVE_UNUSED;
ls_t0 ls_v23 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v24 LS_NATIVE_UNUSED = {0};
ls_string ls_v25 LS_NATIVE_UNUSED = {0};
ls_value ls_v26 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v27 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v28 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v29 LS_NATIVE_UNUSED = {0};
ls_string ls_v30 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v33 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v34 LS_NATIVE_UNUSED = {0};
bool ls_v35 LS_NATIVE_UNUSED;
ls_value ls_v36 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v37 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v38 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v39 LS_NATIVE_UNUSED = {0};
ls_string ls_v40 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v43 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v44 LS_NATIVE_UNUSED = {0};
bool ls_v45 LS_NATIVE_UNUSED;
ls_value ls_v46 LS_NATIVE_UNUSED = {0};
ls_value ls_v47 LS_NATIVE_UNUSED = {0};
bool ls_v48 LS_NATIVE_UNUSED;
ls_string ls_v50 LS_NATIVE_UNUSED = {0};
int32_t ls_v51 LS_NATIVE_UNUSED;
ls_string ls_v52 LS_NATIVE_UNUSED = {0};
int32_t ls_v53 LS_NATIVE_UNUSED;
ls_t1 ls_v54 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v55 LS_NATIVE_UNUSED = {0};
int32_t ls_v56 LS_NATIVE_UNUSED;
int32_t ls_v57 LS_NATIVE_UNUSED;
ls_t1 ls_v58 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v59 LS_NATIVE_UNUSED = {0};
ls_string ls_v60 LS_NATIVE_UNUSED = {0};
int32_t ls_v62 LS_NATIVE_UNUSED;
ls_t0 ls_v64 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v65 LS_NATIVE_UNUSED = {0};
int32_t ls_v66 LS_NATIVE_UNUSED;
ls_t1 ls_v67 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v68 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v69 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v70 LS_NATIVE_UNUSED = {0};
ls_string ls_v71 LS_NATIVE_UNUSED = {0};
ls_string ls_v72 LS_NATIVE_UNUSED = {0};
ls_string ls_v74 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v76 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v77 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v78 LS_NATIVE_UNUSED = {0};
ls_t1 ls_v79 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v80 LS_NATIVE_UNUSED = {0};
ls_t2 ls_v81 LS_NATIVE_UNUSED = {0};
ls_string ls_v82 LS_NATIVE_UNUSED = {0};
ls_string ls_v83 LS_NATIVE_UNUSED = {0};
ls_string ls_v85 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v88 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v89 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v91 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v92 LS_NATIVE_UNUSED = {0};
ls_string ls_v93 LS_NATIVE_UNUSED = {0};
ls_array1 * ls_v95 LS_NATIVE_UNUSED = NULL;
int32_t ls_v96 LS_NATIVE_UNUSED;
ls_t0 ls_v99 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v100 LS_NATIVE_UNUSED = {0};
ls_value ls_v101 LS_NATIVE_UNUSED = {0};
ls_value ls_v102 LS_NATIVE_UNUSED = {0};
ls_value ls_v103 LS_NATIVE_UNUSED = {0};
bool ls_v104 LS_NATIVE_UNUSED;
ls_string ls_v105 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v107 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v108 LS_NATIVE_UNUSED = {0};
ls_string ls_v109 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v111 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v112 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v113 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v114 LS_NATIVE_UNUSED = NULL;
ls_value ls_v115 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v116 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v117 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v118 LS_NATIVE_UNUSED = {0};
ls_string ls_v119 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v120 LS_NATIVE_UNUSED = NULL;
ls_value ls_v121 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v122 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v123 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v124 LS_NATIVE_UNUSED = {0};
ls_string ls_v125 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v127 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v128 LS_NATIVE_UNUSED = NULL;
int32_t ls_v129 LS_NATIVE_UNUSED;
ls_string ls_v130 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v132 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v133 LS_NATIVE_UNUSED = NULL;
int32_t ls_v134 LS_NATIVE_UNUSED;
ls_t0 ls_v135 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v136 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v137 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v138 LS_NATIVE_UNUSED = NULL;
int32_t ls_v139 LS_NATIVE_UNUSED;
ls_value ls_v140 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v141 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v142 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v143 LS_NATIVE_UNUSED = {0};
ls_string ls_v144 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v146 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v147 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v148 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v149 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v150 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v151 LS_NATIVE_UNUSED = NULL;
int32_t ls_v152 LS_NATIVE_UNUSED;
int32_t ls_v153 LS_NATIVE_UNUSED;
int32_t ls_v154 LS_NATIVE_UNUSED;
bool ls_v155 LS_NATIVE_UNUSED;
bool ls_v156 LS_NATIVE_UNUSED;
bool ls_v157 LS_NATIVE_UNUSED;
ls_t0 ls_v158 LS_NATIVE_UNUSED = {0};
ls_value ls_v159 LS_NATIVE_UNUSED = {0};
ls_value ls_v160 LS_NATIVE_UNUSED = {0};
ls_value ls_v161 LS_NATIVE_UNUSED = {0};
ls_value ls_v162 LS_NATIVE_UNUSED = {0};
ls_value ls_v163 LS_NATIVE_UNUSED = {0};
bool ls_v164 LS_NATIVE_UNUSED;
ls_array0 * ls_v166 LS_NATIVE_UNUSED = NULL;
int32_t ls_v167 LS_NATIVE_UNUSED;
int32_t ls_v168 LS_NATIVE_UNUSED;
int32_t ls_v169 LS_NATIVE_UNUSED;
bool ls_v170 LS_NATIVE_UNUSED;
bool ls_v171 LS_NATIVE_UNUSED;
bool ls_v172 LS_NATIVE_UNUSED;
ls_t0 ls_v173 LS_NATIVE_UNUSED = {0};
ls_value ls_v174 LS_NATIVE_UNUSED = {0};
ls_value ls_v175 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v176 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v177 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v178 LS_NATIVE_UNUSED = {0};
ls_string ls_v179 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v181 LS_NATIVE_UNUSED = NULL;
int32_t ls_v182 LS_NATIVE_UNUSED;
ls_string ls_v183 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v184 LS_NATIVE_UNUSED = NULL;
int32_t ls_v185 LS_NATIVE_UNUSED;
ls_string ls_v186 LS_NATIVE_UNUSED = {0};
ls_string ls_v188 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v190 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v191 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v192 LS_NATIVE_UNUSED = NULL;
ls_native_object * ls_v193 LS_NATIVE_UNUSED = NULL;
ls_string ls_v194 LS_NATIVE_UNUSED = {0};
ls_native_object * ls_v195 LS_NATIVE_UNUSED = NULL;
ls_string ls_v196 LS_NATIVE_UNUSED = {0};
ls_string ls_v198 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v200 LS_NATIVE_UNUSED = NULL;
ls_callable13 ls_v201 LS_NATIVE_UNUSED = {0};
ls_array0 * ls_v202 LS_NATIVE_UNUSED = NULL;
ls_array0 * ls_v203 LS_NATIVE_UNUSED = NULL;
int32_t ls_v204 LS_NATIVE_UNUSED;
ls_string ls_v205 LS_NATIVE_UNUSED = {0};
ls_c21 = ls_box_allocate21();
ls_c39 = ls_box_allocate39();
ls_string_take(&(ls_v0),(ls_string){ls_s13,sizeof ls_s13/sizeof *ls_s13,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v2),ls_string_repeat(ls_v0,ls_v1));
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v4 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v5),ls_array1_new(2));
ls_array1_push(ls_v5,ls_v3);
ls_array1_push(ls_v5,ls_v4);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v6),(ls_t0){ls_v2,ls_v5});
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v7),ls_v6);
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize21(ls_c21,ls_v7);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v10),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v11),ls_v10);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_call_result = ls_fn1(ls_t0_box(&ls_temps,ls_v11));
ls_t0_copy(&(ls_v12),ls_value_to_t0(ls_call_result));
ls_value_release(ls_call_result);
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v13),ls_v12);
ls_native_temporaries_clear(&ls_temps);
ls_t0_take(&(ls_v14),ls_fn3(ls_v13));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v15),ls_v14);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c22),ls_v15);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v16),(*ls_box_value21(ls_c21)).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v16);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v18),ls_c22.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v18);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v20),(*ls_box_value21(ls_c21)).ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_v21 = (int32_t)ls_v20->length;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v21);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v23),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v24),ls_v23);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_c23),ls_t0_box(&ls_temps,ls_v24));
ls_native_temporaries_clear(&ls_temps);
(void)((*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v25),(ls_string){ls_s14,sizeof ls_s14/sizeof *ls_s14,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&((*ls_box_value21(ls_c21)).ls_f0),ls_v25);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v26),ls_c23);
ls_native_temporaries_clear(&ls_temps);
if (ls_v26.tag != LS_NULL) {
ls_t0_copy(&(ls_v28),ls_value_to_t0(ls_v26));
} else {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v27),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v28),ls_v27);
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v27);
}
ls_t0_copy(&(ls_v29),ls_v28);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c24),ls_v29);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v30),ls_c24.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v30);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v33),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v34),ls_v33);
ls_native_temporaries_clear(&ls_temps);
ls_v35 = true;
ls_native_temporaries_clear(&ls_temps);
ls_value_take(&(ls_v36),ls_fn4(ls_v34,ls_v35));
ls_native_temporaries_clear(&ls_temps);
if (ls_v36.tag != LS_NULL) {
ls_t0_copy(&(ls_v38),ls_value_to_t0(ls_v36));
} else {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v37),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v38),ls_v37);
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v37);
}
ls_t0_copy(&(ls_v39),ls_v38);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c25),ls_v39);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v40),ls_c25.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v40);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v43),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v44),ls_v43);
ls_native_temporaries_clear(&ls_temps);
ls_v45 = false;
ls_native_temporaries_clear(&ls_temps);
ls_value_take(&(ls_v46),ls_fn4(ls_v44,ls_v45));
ls_native_temporaries_clear(&ls_temps);
ls_v47 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_v48 = ls_value_equal(ls_v46,ls_v47);
ls_native_temporaries_clear(&ls_temps);
puts(ls_v48 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v50),(ls_string){ls_s15,sizeof ls_s15/sizeof *ls_s15,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v51 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v52),ls_string_repeat(ls_v50,ls_v51));
ls_native_temporaries_clear(&ls_temps);
ls_v53 = ls_from_u32(UINT32_C(3));
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v54),(ls_t1){ls_value_string(ls_v52),ls_v53});
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v55),ls_v54);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_c26),ls_v55);
ls_native_temporaries_clear(&ls_temps);
ls_v56 = ls_from_u32(UINT32_C(42));
ls_native_temporaries_clear(&ls_temps);
ls_v57 = ls_from_u32(UINT32_C(4));
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v58),(ls_t1){ls_value_int(ls_v56),ls_v57});
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v59),ls_v58);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_c27),ls_v59);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v60),ls_value_to_string(ls_c26.ls_f0));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v60);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v62 = ls_value_to_int(ls_c27.ls_f0);
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v62);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v64),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v65),ls_v64);
ls_native_temporaries_clear(&ls_temps);
ls_v66 = ls_from_u32(UINT32_C(7));
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v67),(ls_t1){ls_t0_box(&ls_temps,ls_v65),ls_v66});
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v68),ls_v67);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_c28),ls_v68);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v69),ls_c28);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v70),ls_v69);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_c29),ls_v70);
ls_native_temporaries_clear(&ls_temps);
(void)(ls_c29.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v71),(ls_string){ls_s16,sizeof ls_s16/sizeof *ls_s16,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_t1 ls_wb0 = ls_c29;
ls_t1_retain(ls_wb0);
ls_t0 ls_wb1 = ls_value_to_t0(ls_wb0.ls_f0);
ls_t0_retain(ls_wb1);
ls_string_copy(&ls_wb1.ls_f0,ls_v71);
ls_value_copy(&ls_wb0.ls_f0,ls_t0_box(&ls_temps,ls_wb1));
ls_t0_release(ls_wb1);
ls_t1_copy(&(ls_c29),ls_wb0);
ls_t1_release(ls_wb0);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v72),ls_value_to_t0(ls_c28.ls_f0).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v72);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v74),ls_value_to_t0(ls_c29.ls_f0).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v74);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v76),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v77),ls_v76);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v78),ls_c26);
ls_native_temporaries_clear(&ls_temps);
ls_t1_copy(&(ls_v79),ls_v78);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v80),(ls_t2){ls_t0_box(&ls_temps,ls_v77),ls_t1_box(&ls_temps,ls_v79)});
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_v81),ls_v80);
ls_native_temporaries_clear(&ls_temps);
ls_t2_copy(&(ls_c30),ls_v81);
ls_native_temporaries_clear(&ls_temps);
(void)(ls_c30.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v82),(ls_string){ls_s17,sizeof ls_s17/sizeof *ls_s17,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_t2 ls_wb0 = ls_c30;
ls_t2_retain(ls_wb0);
ls_t1 ls_wb1 = ls_value_to_t1(ls_wb0.ls_f1);
ls_t1_retain(ls_wb1);
ls_value_copy(&ls_wb1.ls_f0,ls_value_string(ls_v82));
ls_value_copy(&ls_wb0.ls_f1,ls_t1_box(&ls_temps,ls_wb1));
ls_t1_release(ls_wb1);
ls_t2_copy(&(ls_c30),ls_wb0);
ls_t2_release(ls_wb0);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v83),ls_value_to_string(ls_value_to_t1(ls_c30.ls_f1).ls_f0));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v83);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v85),ls_value_to_string(ls_c26.ls_f0));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v85);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v88),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v89),ls_v88);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
{
ls_callable3 ls_adapted1 = ls_adapt5_3((ls_callable5){ls_adapter3,NULL,UINT64_C(4)});
{
ls_value ls_call_result = ls_fn2(ls_t0_box(&ls_temps,ls_v89),ls_adapted1);
ls_t0_copy(&(ls_v91),ls_value_to_t0(ls_call_result));
ls_value_release(ls_call_result);
}
ls_callable3_clear(&ls_adapted1);
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v92),ls_v91);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c31),ls_v92);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v93),ls_c31.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v93);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v95),ls_c24.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_v96 = (int32_t)ls_v95->length;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v96);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v99),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v100),ls_v99);
ls_native_temporaries_clear(&ls_temps);
ls_value_take(&(ls_v101),ls_fn5(ls_t0_box(&ls_temps,ls_v100)));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v102),ls_v101);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_c32),ls_v102);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v103),ls_c32);
ls_native_temporaries_clear(&ls_temps);
{ uint8_t ls_t = ls_v103.tag; ls_v104 = ls_t == LS_STRING; }
ls_native_temporaries_clear(&ls_temps);
if (ls_v104) {
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v105),ls_value_to_string(ls_c32));
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v105);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v105);
} else {
ls_t0_copy(&(ls_v107),ls_value_to_t0(ls_c32));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v108),ls_v107);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c33),ls_v108);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v109),ls_c33.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v109);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v109);
ls_t0_clear(&ls_c33);
ls_t0_clear(&ls_v108);
ls_t0_clear(&ls_v107);
}
ls_t0_copy(&(ls_v111),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v112),ls_v111);
ls_native_temporaries_clear(&ls_temps);
ls_object_take(&(ls_v113),ls_record_new());
ls_record_set(ls_v113,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL},ls_t0_box(&ls_temps,ls_v112));
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_c34),ls_v113);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v114),ls_c34);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v115),ls_record_get(ls_v114,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL}));
ls_native_temporaries_clear(&ls_temps);
if (ls_v115.tag != LS_NULL) {
ls_t0_copy(&(ls_v117),ls_value_to_t0(ls_v115));
} else {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v116),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v117),ls_v116);
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v116);
}
ls_t0_copy(&(ls_v118),ls_v117);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c35),ls_v118);
ls_native_temporaries_clear(&ls_temps);
(void)(ls_c35);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v119),(ls_string){ls_s19,sizeof ls_s19/sizeof *ls_s19,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_c35.ls_f0),ls_v119);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v120),ls_c34);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v121),ls_record_get(ls_v120,(ls_string){ls_s18,sizeof ls_s18/sizeof *ls_s18,NULL}));
ls_native_temporaries_clear(&ls_temps);
if (ls_v121.tag != LS_NULL) {
ls_t0_copy(&(ls_v123),ls_value_to_t0(ls_v121));
} else {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v122),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v123),ls_v122);
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v122);
}
ls_t0_copy(&(ls_v124),ls_v123);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c36),ls_v124);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v125),ls_c36.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v125);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v127),ls_c34);
ls_native_temporaries_clear(&ls_temps);
{
ls_map *ls_record = (ls_map *)ls_v127;
ls_record_key *ls_order = ls_record_order(ls_v127);
ls_array0_take(&(ls_v128),ls_array0_new(ls_record->size));
for (size_t ls_i = 0; ls_i < ls_record->size; ++ls_i) {
ls_array0_push(ls_v128,ls_value_to_t0(ls_record->entries[ls_order[ls_i].position].value));
}
free(ls_order);
}
ls_native_temporaries_clear(&ls_temps);
ls_v129 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v130),ls_array0_get(ls_v128,ls_v129).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v130);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_object_take(&(ls_v132),ls_map_new());
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_c37),ls_v132);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v133),ls_c37);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v134 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v135),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v136),ls_v135);
ls_native_temporaries_clear(&ls_temps);
ls_map_set(ls_v133,ls_value_int(ls_v134),ls_t0_box(&ls_temps,ls_v136));
ls_object_copy(&(ls_v137),ls_v133);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v138),ls_c37);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v139 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v140),ls_map_get(ls_v138,ls_value_int(ls_v139)));
ls_native_temporaries_clear(&ls_temps);
if (ls_v140.tag != LS_NULL) {
ls_t0_copy(&(ls_v142),ls_value_to_t0(ls_v140));
} else {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v141),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v142),ls_v141);
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v141);
}
ls_t0_copy(&(ls_v143),ls_v142);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c38),ls_v143);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v144),ls_c38.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v144);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v146),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v147),ls_v146);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v148),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v149),ls_v148);
ls_native_temporaries_clear(&ls_temps);
ls_array0_take(&(ls_v150),ls_array0_new(2));
ls_array0_push(ls_v150,ls_v147);
ls_array0_push(ls_v150,ls_v149);
ls_native_temporaries_clear(&ls_temps);
ls_box_initialize39(ls_c39,ls_v150);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v151),(*ls_box_value39(ls_c39)));
ls_native_temporaries_clear(&ls_temps);
ls_v152 = ls_from_u32(UINT32_C(9));
ls_native_temporaries_clear(&ls_temps);
ls_v153 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v154 = (int32_t)ls_v151->length;
ls_native_temporaries_clear(&ls_temps);
ls_v155 = ls_v152 >= ls_v153;
ls_native_temporaries_clear(&ls_temps);
ls_v157 = ls_v155;
if (ls_v155) {
ls_native_temporaries_clear(&ls_temps);
ls_v156 = ls_v152 < ls_v154;
ls_native_temporaries_clear(&ls_temps);
ls_v157 = ls_v156;
ls_native_temporaries_clear(&ls_temps);
}
if (ls_v157) {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v158),ls_array0_get(ls_v151,ls_v152));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v160),ls_t0_box(&ls_temps,ls_v158));
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v158);
} else {
ls_value_take(&(ls_v159),(ls_value){0});
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v160),ls_v159);
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v159);
}
ls_value_copy(&(ls_v161),ls_v160);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_c40),ls_v161);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v162),ls_c40);
ls_native_temporaries_clear(&ls_temps);
ls_v163 = (ls_value){0};
ls_native_temporaries_clear(&ls_temps);
ls_v164 = ls_value_equal(ls_v162,ls_v163);
ls_native_temporaries_clear(&ls_temps);
puts(ls_v164 ? "true" : "false");
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v166),(*ls_box_value39(ls_c39)));
ls_native_temporaries_clear(&ls_temps);
ls_v167 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v168 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v169 = (int32_t)ls_v166->length;
ls_native_temporaries_clear(&ls_temps);
ls_v170 = ls_v167 >= ls_v168;
ls_native_temporaries_clear(&ls_temps);
ls_v172 = ls_v170;
if (ls_v170) {
ls_native_temporaries_clear(&ls_temps);
ls_v171 = ls_v167 < ls_v169;
ls_native_temporaries_clear(&ls_temps);
ls_v172 = ls_v171;
ls_native_temporaries_clear(&ls_temps);
}
if (ls_v172) {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v173),ls_array0_get(ls_v166,ls_v167));
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v175),ls_t0_box(&ls_temps,ls_v173));
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v173);
} else {
ls_value_take(&(ls_v174),(ls_value){0});
ls_native_temporaries_clear(&ls_temps);
ls_value_copy(&(ls_v175),ls_v174);
ls_native_temporaries_clear(&ls_temps);
ls_value_clear(&ls_v174);
}
if (ls_v175.tag != LS_NULL) {
ls_t0_copy(&(ls_v177),ls_value_to_t0(ls_v175));
} else {
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v176),(*ls_box_value21(ls_c21)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v177),ls_v176);
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v176);
}
ls_t0_copy(&(ls_v178),ls_v177);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c41),ls_v178);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v179),ls_c41.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v179);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v181),(*ls_box_value39(ls_c39)));
ls_native_temporaries_clear(&ls_temps);
ls_v182 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
(void)(ls_array0_get(ls_v181,ls_v182));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v183),(ls_string){ls_s20,sizeof ls_s20/sizeof *ls_s20,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_t0 ls_wb0 = ls_array0_get(ls_v181,ls_v182);
ls_t0_retain(ls_wb0);
ls_string_copy(&ls_wb0.ls_f0,ls_v183);
ls_array0_set(ls_v181,ls_v182,ls_wb0);
ls_t0_release(ls_wb0);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v184),(*ls_box_value39(ls_c39)));
ls_native_temporaries_clear(&ls_temps);
ls_v185 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v186),ls_array0_get(ls_v184,ls_v185).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v186);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v188),ls_c24.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v188);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v190),ls_c24);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v191),ls_v190);
ls_native_temporaries_clear(&ls_temps);
ls_object_take(&(ls_v192),ls_record_new());
ls_shape_set(ls_v192,(ls_string){ls_s0,sizeof ls_s0/sizeof *ls_s0,NULL},ls_t0_box(&ls_temps,ls_v191),false);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_c42),ls_v192);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v193),ls_c42);
ls_native_temporaries_clear(&ls_temps);
(void)(ls_record_get(ls_v193,(ls_string){ls_s0,sizeof ls_s0/sizeof *ls_s0,NULL}));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v194),(ls_string){ls_s21,sizeof ls_s21/sizeof *ls_s21,NULL});
ls_native_temporaries_clear(&ls_temps);
{
ls_t0 ls_wb0 = ls_value_to_t0(ls_record_get(ls_v193,(ls_string){ls_s0,sizeof ls_s0/sizeof *ls_s0,NULL}));
ls_t0_retain(ls_wb0);
ls_string_copy(&ls_wb0.ls_f0,ls_v194);
ls_shape_set(ls_v193,(ls_string){ls_s0,sizeof ls_s0/sizeof *ls_s0,NULL},ls_t0_box(&ls_temps,ls_wb0),false);
ls_t0_release(ls_wb0);
}
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_object_copy(&(ls_v195),ls_c42);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v196),ls_value_to_t0(ls_record_get(ls_v195,(ls_string){ls_s0,sizeof ls_s0/sizeof *ls_s0,NULL})).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v196);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v198),ls_c24.ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v198);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v200),(*ls_box_value39(ls_c39)));
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_callable13_take(&(ls_v201),ls_closure9(ls_c21,ls_c39));
ls_native_temporaries_clear(&ls_temps);
{
ls_array0 *ls_src = ls_v200;
size_t ls_len = ls_src->length;
ls_array0 *ls_out = ls_array0_new(0);
for (size_t ls_k = 0; ls_k < ls_len; ls_k++) {
if (ls_k >= ls_src->length) continue;
ls_t0 ls_item = ls_src->items[ls_k];
ls_t0_retain(ls_item);
if (ls_v201.code(ls_v201.environment,ls_item)) ls_array0_push(ls_out, ls_item);
ls_t0_release(ls_item);
ls_native_temporaries_clear(&ls_temps);
}
ls_array0_take(&(ls_v202),ls_out);
}
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_c43),ls_v202);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_array0_copy(&(ls_v203),ls_c43);
ls_native_temporaries_clear(&ls_temps);
ls_v204 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_string_copy(&(ls_v205),ls_array0_get(ls_v203,ls_v204).ls_f0);
ls_native_temporaries_clear(&ls_temps);
ls_print_string(ls_v205);
ls_native_temporaries_clear(&ls_temps);
ls_string_clear(&ls_v205);
ls_array0_clear(&ls_v203);
ls_array0_clear(&ls_c43);
ls_array0_clear(&ls_v202);
ls_callable13_clear(&ls_v201);
ls_array0_clear(&ls_v200);
ls_string_clear(&ls_v198);
ls_string_clear(&ls_v196);
ls_object_clear(&ls_v195);
ls_string_clear(&ls_v194);
ls_object_clear(&ls_v193);
ls_object_clear(&ls_c42);
ls_object_clear(&ls_v192);
ls_t0_clear(&ls_v191);
ls_t0_clear(&ls_v190);
ls_string_clear(&ls_v188);
ls_string_clear(&ls_v186);
ls_array0_clear(&ls_v184);
ls_string_clear(&ls_v183);
ls_array0_clear(&ls_v181);
ls_string_clear(&ls_v179);
ls_t0_clear(&ls_c41);
ls_t0_clear(&ls_v178);
ls_t0_clear(&ls_v177);
ls_value_clear(&ls_v175);
ls_array0_clear(&ls_v166);
ls_value_clear(&ls_v162);
ls_value_clear(&ls_c40);
ls_value_clear(&ls_v161);
ls_value_clear(&ls_v160);
ls_array0_clear(&ls_v151);
ls_native_release(ls_c39);
ls_c39 = NULL;
ls_array0_clear(&ls_v150);
ls_t0_clear(&ls_v149);
ls_t0_clear(&ls_v148);
ls_t0_clear(&ls_v147);
ls_t0_clear(&ls_v146);
ls_string_clear(&ls_v144);
ls_t0_clear(&ls_c38);
ls_t0_clear(&ls_v143);
ls_t0_clear(&ls_v142);
ls_value_clear(&ls_v140);
ls_object_clear(&ls_v138);
ls_object_clear(&ls_v137);
ls_t0_clear(&ls_v136);
ls_t0_clear(&ls_v135);
ls_object_clear(&ls_v133);
ls_object_clear(&ls_c37);
ls_object_clear(&ls_v132);
ls_string_clear(&ls_v130);
ls_array0_clear(&ls_v128);
ls_object_clear(&ls_v127);
ls_string_clear(&ls_v125);
ls_t0_clear(&ls_c36);
ls_t0_clear(&ls_v124);
ls_t0_clear(&ls_v123);
ls_value_clear(&ls_v121);
ls_object_clear(&ls_v120);
ls_string_clear(&ls_v119);
ls_t0_clear(&ls_c35);
ls_t0_clear(&ls_v118);
ls_t0_clear(&ls_v117);
ls_value_clear(&ls_v115);
ls_object_clear(&ls_v114);
ls_object_clear(&ls_c34);
ls_object_clear(&ls_v113);
ls_t0_clear(&ls_v112);
ls_t0_clear(&ls_v111);
ls_value_clear(&ls_v103);
ls_value_clear(&ls_c32);
ls_value_clear(&ls_v102);
ls_value_clear(&ls_v101);
ls_t0_clear(&ls_v100);
ls_t0_clear(&ls_v99);
ls_array1_clear(&ls_v95);
ls_string_clear(&ls_v93);
ls_t0_clear(&ls_c31);
ls_t0_clear(&ls_v92);
ls_t0_clear(&ls_v91);
ls_t0_clear(&ls_v89);
ls_t0_clear(&ls_v88);
ls_string_clear(&ls_v85);
ls_string_clear(&ls_v83);
ls_string_clear(&ls_v82);
ls_t2_clear(&ls_c30);
ls_t2_clear(&ls_v81);
ls_t2_clear(&ls_v80);
ls_t1_clear(&ls_v79);
ls_t1_clear(&ls_v78);
ls_t0_clear(&ls_v77);
ls_t0_clear(&ls_v76);
ls_string_clear(&ls_v74);
ls_string_clear(&ls_v72);
ls_string_clear(&ls_v71);
ls_t1_clear(&ls_c29);
ls_t1_clear(&ls_v70);
ls_t1_clear(&ls_v69);
ls_t1_clear(&ls_c28);
ls_t1_clear(&ls_v68);
ls_t1_clear(&ls_v67);
ls_t0_clear(&ls_v65);
ls_t0_clear(&ls_v64);
ls_string_clear(&ls_v60);
ls_t1_clear(&ls_c27);
ls_t1_clear(&ls_v59);
ls_t1_clear(&ls_v58);
ls_t1_clear(&ls_c26);
ls_t1_clear(&ls_v55);
ls_t1_clear(&ls_v54);
ls_string_clear(&ls_v52);
ls_string_clear(&ls_v50);
ls_value_clear(&ls_v46);
ls_t0_clear(&ls_v44);
ls_t0_clear(&ls_v43);
ls_string_clear(&ls_v40);
ls_t0_clear(&ls_c25);
ls_t0_clear(&ls_v39);
ls_t0_clear(&ls_v38);
ls_value_clear(&ls_v36);
ls_t0_clear(&ls_v34);
ls_t0_clear(&ls_v33);
ls_string_clear(&ls_v30);
ls_t0_clear(&ls_c24);
ls_t0_clear(&ls_v29);
ls_t0_clear(&ls_v28);
ls_value_clear(&ls_v26);
ls_string_clear(&ls_v25);
ls_value_clear(&ls_c23);
ls_t0_clear(&ls_v24);
ls_t0_clear(&ls_v23);
ls_array1_clear(&ls_v20);
ls_string_clear(&ls_v18);
ls_string_clear(&ls_v16);
ls_t0_clear(&ls_c22);
ls_t0_clear(&ls_v15);
ls_t0_clear(&ls_v14);
ls_t0_clear(&ls_v13);
ls_t0_clear(&ls_v12);
ls_t0_clear(&ls_v11);
ls_t0_clear(&ls_v10);
ls_native_release(ls_c21);
ls_c21 = NULL;
ls_t0_clear(&ls_v7);
ls_t0_clear(&ls_v6);
ls_array1_clear(&ls_v5);
ls_string_clear(&ls_v2);
ls_string_clear(&ls_v0);
}
static LS_NATIVE_UNUSED void ls_fn8(void) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
int32_t ls_c46 LS_NATIVE_UNUSED;
int32_t ls_c47 LS_NATIVE_UNUSED;
ls_t0 ls_c48 LS_NATIVE_UNUSED = {0};
int32_t ls_v0 LS_NATIVE_UNUSED;
int32_t ls_v1 LS_NATIVE_UNUSED;
int32_t ls_v2 LS_NATIVE_UNUSED;
int32_t ls_v3 LS_NATIVE_UNUSED;
bool ls_v4 LS_NATIVE_UNUSED;
ls_string ls_v6 LS_NATIVE_UNUSED = {0};
int32_t ls_v7 LS_NATIVE_UNUSED;
ls_string ls_v8 LS_NATIVE_UNUSED = {0};
int32_t ls_v9 LS_NATIVE_UNUSED;
ls_array1 * ls_v10 LS_NATIVE_UNUSED = NULL;
ls_t0 ls_v11 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v12 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v13 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v14 LS_NATIVE_UNUSED = {0};
int32_t ls_v15 LS_NATIVE_UNUSED;
ls_array1 * ls_v16 LS_NATIVE_UNUSED = NULL;
int32_t ls_v17 LS_NATIVE_UNUSED;
int32_t ls_v18 LS_NATIVE_UNUSED;
int32_t ls_v19 LS_NATIVE_UNUSED;
int32_t ls_v21 LS_NATIVE_UNUSED;
int32_t ls_v23 LS_NATIVE_UNUSED;
int32_t ls_v24 LS_NATIVE_UNUSED;
int32_t ls_v25 LS_NATIVE_UNUSED;
int32_t ls_v26 LS_NATIVE_UNUSED;
ls_v0 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_c46 = ls_v0;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_c47 = ls_v1;
ls_native_temporaries_clear(&ls_temps);
ls_test34: ;
ls_native_temporaries_clear(&ls_temps);
ls_v2 = ls_c47;
ls_native_temporaries_clear(&ls_temps);
ls_v3 = ls_from_u32(UINT32_C(2000));
ls_native_temporaries_clear(&ls_temps);
ls_v4 = ls_v2 < ls_v3;
ls_native_temporaries_clear(&ls_temps);
if (!ls_v4) goto ls_end34;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v6),(ls_string){ls_s23,sizeof ls_s23/sizeof *ls_s23,NULL});
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v7 = ls_from_u32(UINT32_C(2));
ls_native_temporaries_clear(&ls_temps);
ls_string_take(&(ls_v8),ls_string_repeat(ls_v6,ls_v7));
ls_native_temporaries_clear(&ls_temps);
ls_v9 = ls_c47;
ls_native_temporaries_clear(&ls_temps);
ls_array1_take(&(ls_v10),ls_array1_new(1));
ls_array1_push(ls_v10,ls_v9);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v11),(ls_t0){ls_v8,ls_v10});
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v12),ls_v11);
ls_native_temporaries_clear(&ls_temps);
{
ls_value ls_call_result = ls_fn1(ls_t0_box(&ls_temps,ls_v12));
ls_t0_copy(&(ls_v13),ls_value_to_t0(ls_call_result));
ls_value_release(ls_call_result);
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v14),ls_v13);
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_c48),ls_v14);
ls_native_temporaries_clear(&ls_temps);
ls_v15 = ls_c46;
ls_native_temporaries_clear(&ls_temps);
ls_array1_copy(&(ls_v16),ls_c48.ls_f1);
ls_native_temporaries_clear(&ls_temps);
ls_v17 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_v18 = ls_array1_get(ls_v16,ls_v17);
ls_native_temporaries_clear(&ls_temps);
ls_v19 = ls_from_u32((uint32_t)((uint32_t)ls_v15 + (uint32_t)ls_v18));
ls_native_temporaries_clear(&ls_temps);
ls_c46 = ls_v19;
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_native_temporaries_clear(&ls_temps);
ls_v21 = ls_from_u32(UINT32_C(40));
ls_native_temporaries_clear(&ls_temps);
host_assertMaxNativeCount(ls_v21);
ls_native_temporaries_clear(&ls_temps);
ls_array1_clear(&ls_v16);
ls_t0_clear(&ls_c48);
ls_t0_clear(&ls_v14);
ls_t0_clear(&ls_v13);
ls_t0_clear(&ls_v12);
ls_t0_clear(&ls_v11);
ls_array1_clear(&ls_v10);
ls_string_clear(&ls_v8);
ls_string_clear(&ls_v6);
ls_update34: LS_NATIVE_UNUSED;
ls_v23 = ls_c47;
ls_native_temporaries_clear(&ls_temps);
ls_v24 = ls_from_u32(UINT32_C(1));
ls_native_temporaries_clear(&ls_temps);
ls_v25 = ls_from_u32((uint32_t)((uint32_t)ls_v23 + (uint32_t)ls_v24));
ls_native_temporaries_clear(&ls_temps);
ls_c47 = ls_v25;
ls_native_temporaries_clear(&ls_temps);
goto ls_test34;
ls_end34: ;
ls_native_temporaries_clear(&ls_temps);
ls_v26 = ls_c46;
ls_native_temporaries_clear(&ls_temps);
printf("%ld\n",(long)ls_v26);
ls_native_temporaries_clear(&ls_temps);
}
static LS_NATIVE_UNUSED bool ls_fn9(void *ls_env LS_NATIVE_UNUSED,ls_t0 ls_c44 LS_NATIVE_UNUSED) {
ls_native_temporary *ls_temps LS_NATIVE_UNUSED = NULL;
ls_t0_retain(ls_c44);
ls_array0 * ls_v0 LS_NATIVE_UNUSED = NULL;
int32_t ls_v1 LS_NATIVE_UNUSED;
ls_t0 ls_v2 LS_NATIVE_UNUSED = {0};
ls_t0 ls_v3 LS_NATIVE_UNUSED = {0};
bool ls_v4 LS_NATIVE_UNUSED;
ls_array0_copy(&(ls_v0),(*ls_box_value39(((ls_env9 *)ls_env)->ls_e1)));
ls_native_temporaries_clear(&ls_temps);
ls_v1 = ls_from_u32(UINT32_C(0));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v2),(*ls_box_value21(((ls_env9 *)ls_env)->ls_e0)));
ls_native_temporaries_clear(&ls_temps);
ls_t0_copy(&(ls_v3),ls_v2);
ls_native_temporaries_clear(&ls_temps);
ls_array0_set(ls_v0,ls_v1,ls_v3);
ls_native_temporaries_clear(&ls_temps);
ls_v4 = true;
ls_native_temporaries_clear(&ls_temps);
{
bool ls_return = ls_v4;
ls_t0_clear(&ls_v3);
ls_t0_clear(&ls_v2);
ls_array0_clear(&ls_v0);
ls_t0_clear(&ls_c44);
ls_native_temporaries_clear(&ls_temps);
return ls_return;
}
ls_native_temporaries_clear(&ls_temps);
ls_t0_clear(&ls_v3);
ls_t0_clear(&ls_v2);
ls_array0_clear(&ls_v0);
ls_t0_clear(&ls_c44);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(10);
ls_init0();
fflush(stdout);
ls_native_collect_cycles();
return 0;
}
