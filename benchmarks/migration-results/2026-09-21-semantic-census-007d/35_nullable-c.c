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
static inline double ls_f64(double value) {
    volatile double rounded = value;
    return rounded;
}
static inline double ls_f64_bits(uint64_t bits) {
    double value;
    memcpy(&value, &bits, sizeof value);
    return value;
}
static inline bool ls_string_equal(ls_string left, ls_string right) {
    return left.length == right.length &&
           (left.length == 0 || memcmp(left.data, right.data, left.length * sizeof *left.data) == 0);
}
#include <stdlib.h>
typedef struct ls_string_block { struct ls_string_block *next; uint16_t units[]; } ls_string_block;
static ls_string_block *ls_string_blocks;
static void ls_string_failure(const char *message) {
    fputs(message, stderr);
    fputc('\n', stderr);
    abort();
}
static uint16_t *ls_string_allocate(size_t length) {
    if (length > (size_t)INT32_MAX) ls_string_failure("LilScript native string length exceeds the runtime limit");
    ls_string_block *block = malloc(sizeof *block + length * sizeof(uint16_t));
    if (!block) ls_string_failure("LilScript native runtime resource exhaustion");
    block->next = ls_string_blocks;
    ls_string_blocks = block;
    return block->units;
}
static void ls_strings_release(void) {
    while (ls_string_blocks) {
        ls_string_block *next = ls_string_blocks->next;
        free(ls_string_blocks);
        ls_string_blocks = next;
    }
}
static ls_string ls_string_ascii(const char *text, size_t length) {
    if (!length) return (ls_string){NULL, 0};
    uint16_t *units = ls_string_allocate(length);
    for (size_t index = 0; index < length; index++) units[index] = (unsigned char)text[index];
    return (ls_string){units, length};
}
static ls_string ls_string_concat(ls_string left, ls_string right) {
    if (!left.length) return right;
    if (!right.length) return left;
    if (right.length > (size_t)INT32_MAX - left.length)
        ls_string_failure("LilScript native string length exceeds the runtime limit");
    uint16_t *units = ls_string_allocate(left.length + right.length);
    memcpy(units, left.data, left.length * sizeof *units);
    memcpy(units + left.length, right.data, right.length * sizeof *units);
    return (ls_string){units, left.length + right.length};
}
static int ls_string_compare(ls_string left, ls_string right) {
    size_t shared = left.length < right.length ? left.length : right.length;
    for (size_t index = 0; index < shared; index++) {
        if (left.data[index] != right.data[index]) return left.data[index] < right.data[index] ? -1 : 1;
    }
    return left.length == right.length ? 0 : left.length < right.length ? -1 : 1;
}
static const uint16_t ls_true_units[] = {116, 114, 117, 101};
static const uint16_t ls_false_units[] = {102, 97, 108, 115, 101};
static ls_string ls_bool_to_string(bool value) {
    return value ? (ls_string){ls_true_units, 4} : (ls_string){ls_false_units, 5};
}
static ls_string ls_uint_to_radix(uint32_t magnitude, bool negative, int32_t radix) {
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
static ls_string ls_int_to_radix(int32_t value, int32_t radix) {
    uint32_t magnitude = value < 0 ? UINT32_C(0) - (uint32_t)value : (uint32_t)value;
    return ls_uint_to_radix(magnitude, value < 0, radix);
}
static ls_string ls_int_to_string(int32_t value) {
    return ls_int_to_radix(value, 10);
}
/* The k significant digits (no trailing zeros) and exponent n of the
   shortest decimal 0.d1...dk x 10^n that reads back as `value` (> 0,
   finite). printf rounds correctly, so each precision's candidate is the
   nearest decimal of that length; below a power of two the rounding
   interval is narrower, so the next decimal up may be the one that reads
   back when the nearest does not. */
static void ls_shortest_decimal(double value, char *digits, int *count, int *exponent) {
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
static ls_string ls_number_to_string(double value) {
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
static void ls_write_string(ls_string value) {
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
static void ls_print_string(ls_string value) {
    ls_write_string(value);
    putchar('\n');
}
static void ls_print_number(double value) {
    if (value == 0 && signbit(value)) {
        puts("-0");
        return;
    }
    ls_print_string(ls_number_to_string(value));
}
static size_t ls_string_clamp(int32_t index, size_t length) {
    return index < 0 ? 0 : (size_t)index > length ? length : (size_t)index;
}
static size_t ls_string_relative(int32_t index, size_t length) {
    if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
    return (size_t)index < length ? (size_t)index : length;
}
static bool ls_string_matches(ls_string text, size_t at, ls_string search) {
    return at + search.length <= text.length &&
           (search.length == 0 || memcmp(text.data + at, search.data, search.length * sizeof *search.data) == 0);
}
static int32_t ls_string_index_of(ls_string text, ls_string search, int32_t position) {
    for (size_t at = ls_string_clamp(position, text.length); at + search.length <= text.length; at++) {
        if (ls_string_matches(text, at, search)) return (int32_t)at;
    }
    return -1;
}
static int32_t ls_string_last_index_of(ls_string text, ls_string search, int32_t position) {
    if (search.length > text.length) return -1;
    size_t at = ls_string_clamp(position, text.length);
    if (at > text.length - search.length) at = text.length - search.length;
    for (;; at--) {
        if (ls_string_matches(text, at, search)) return (int32_t)at;
        if (at == 0) return -1;
    }
}
static bool ls_string_starts_with(ls_string text, ls_string search) {
    return ls_string_matches(text, 0, search);
}
static bool ls_string_ends_with(ls_string text, ls_string search) {
    return search.length <= text.length && ls_string_matches(text, text.length - search.length, search);
}
static ls_string ls_string_view(ls_string text, size_t start, size_t end) {
    if (end <= start) return (ls_string){NULL, 0};
    return (ls_string){text.data + start, end - start};
}
static ls_string ls_string_slice(ls_string text, int32_t start, bool bounded, int32_t end) {
    size_t from = ls_string_relative(start, text.length);
    size_t to = bounded ? ls_string_relative(end, text.length) : text.length;
    return ls_string_view(text, from, to);
}
static bool ls_string_space(uint16_t unit) {
    return (unit >= 9 && unit <= 13) || unit == 32 || unit == 0xA0 || unit == 0x1680 ||
           (unit >= 0x2000 && unit <= 0x200A) || unit == 0x2028 || unit == 0x2029 ||
           unit == 0x202F || unit == 0x205F || unit == 0x3000 || unit == 0xFEFF;
}
static ls_string ls_string_trim(ls_string text, bool start, bool end) {
    size_t from = 0, to = text.length;
    while (start && from < to && ls_string_space(text.data[from])) from++;
    while (end && to > from && ls_string_space(text.data[to - 1])) to--;
    return ls_string_view(text, from, to);
}
static ls_string ls_string_repeat(ls_string text, int32_t count) {
    if (count < 0) ls_string_failure("LilScript native repeat count must not be negative");
    if (count == 0 || text.length == 0) return (ls_string){NULL, 0};
    if (text.length > (size_t)INT32_MAX / (size_t)count)
        ls_string_failure("LilScript native string length exceeds the runtime limit");
    uint16_t *units = ls_string_allocate(text.length * (size_t)count);
    for (int32_t index = 0; index < count; index++) memcpy(units + (size_t)index * text.length, text.data, text.length * sizeof *units);
    return (ls_string){units, text.length * (size_t)count};
}
static ls_string ls_string_case(ls_string text, bool upper) {
    if (!text.length) return text;
    uint16_t *units = ls_string_allocate(text.length);
    for (size_t index = 0; index < text.length; index++) {
        uint16_t unit = text.data[index];
        if (unit >= 0x80) ls_string_failure("LilScript native case mapping supports ASCII text only");
        if (upper && unit >= 'a' && unit <= 'z') unit = (uint16_t)(unit - 32);
        if (!upper && unit >= 'A' && unit <= 'Z') unit = (uint16_t)(unit + 32);
        units[index] = unit;
    }
    return (ls_string){units, text.length};
}
static int32_t ls_string_code_points(ls_string text) {
    int32_t count = 0;
    for (size_t index = 0; index < text.length; index++, count++) {
        if (text.data[index] >= 0xD800 && text.data[index] <= 0xDBFF && index + 1 < text.length &&
            text.data[index + 1] >= 0xDC00 && text.data[index + 1] <= 0xDFFF) index++;
    }
    return count;
}
static double ls_round(double value) {
    if (!isfinite(value) || value == 0) return value;
    if (value > 0 && value < 0.5) return 0.0;
    if (value < 0 && value >= -0.5) return -0.0;
    double floor_value = floor(value);
    return value - floor_value >= 0.5 ? floor_value + 1.0 : floor_value;
}
static double ls_min(double left, double right) {
    if (left != left || right != right) return NAN;
    if (left == 0 && right == 0) return signbit(left) ? left : right;
    return left < right ? left : right;
}
static double ls_max(double left, double right) {
    if (left != left || right != right) return NAN;
    if (left == 0 && right == 0) return signbit(left) ? right : left;
    return left > right ? left : right;
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
static void ls_value_mismatch(void) {
    fputs("LilScript native value has an unexpected type\n", stderr);
    abort();
}
static void ls_value_retain(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_retain(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_retain(value.as.c.environment);
}
static void ls_value_release(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_release(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_release(value.as.c.environment);
}
static void ls_value_copy(ls_value *slot, ls_value value) { ls_value_retain(value); ls_value_release(*slot); *slot = value; }
static void ls_value_take(ls_value *slot, ls_value value) { ls_value_release(*slot); *slot = value; }
static void ls_value_clear(ls_value *slot) { ls_value_release(*slot); *slot = (ls_value){0}; }
static ls_value ls_value_int(int32_t value) { ls_value result = {LS_INT}; result.as.i = value; return result; }
static ls_value ls_value_float(double value) { ls_value result = {LS_FLOAT}; result.as.f = value; return result; }
static ls_value ls_value_bool(bool value) { ls_value result = {LS_BOOL}; result.as.b = value; return result; }
static ls_value ls_value_string(ls_string value) { ls_value result = {LS_STRING}; result.as.s = value; return result; }
static ls_value ls_value_object(ls_native_object *value) { ls_value result = {LS_OBJECT}; result.as.o = value; return result; }
static ls_value ls_value_array(ls_native_object *value) { ls_value result = {LS_ARRAY}; result.as.o = value; return result; }
static ls_value ls_value_symbol(ls_native_object *value) { ls_value result = {LS_SYMBOL}; result.as.o = value; return result; }
static int32_t ls_value_to_int(ls_value value) { if (value.tag != LS_INT) ls_value_mismatch(); return value.as.i; }
static double ls_value_to_number(ls_value value) {
    if (value.tag == LS_INT) return (double)value.as.i;
    if (value.tag != LS_FLOAT) ls_value_mismatch();
    return value.as.f;
}
static bool ls_value_to_bool(ls_value value) { if (value.tag != LS_BOOL) ls_value_mismatch(); return value.as.b; }
static ls_string ls_value_to_string(ls_value value) { if (value.tag != LS_STRING) ls_value_mismatch(); return value.as.s; }
/* A reference slot of a class instance holds null until `init` stores it,
   as in JavaScript: null unboxes to the empty slot there. */
static ls_native_object *ls_value_to_object(ls_value value) {
    if (value.tag == LS_NULL) return NULL;
    if (value.tag != LS_OBJECT) ls_value_mismatch();
    return value.as.o;
}
static ls_native_object *ls_value_to_array(ls_value value) {
    if (value.tag == LS_NULL) return NULL;
    if (value.tag != LS_ARRAY) ls_value_mismatch();
    return value.as.o;
}
static ls_native_object *ls_value_to_symbol(ls_value value) { if (value.tag != LS_SYMBOL) ls_value_mismatch(); return value.as.o; }
static bool ls_value_number(ls_value value) { return value.tag == LS_INT || value.tag == LS_FLOAT; }
/* JavaScript strict equality: numbers by value, strings by code units,
   everything else by identity. */
static bool ls_value_equal(ls_value left, ls_value right) {
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
static bool ls_value_same_zero(ls_value left, ls_value right) {
    if (ls_value_number(left) && ls_value_number(right)) {
        double x = ls_value_to_number(left), y = ls_value_to_number(right);
        return x == y || (x != x && y != y);
    }
    return ls_value_equal(left, right);
}
static void ls_print_value(ls_value value) {
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
typedef struct {
ls_value ls_f0;
ls_value ls_f1;
} ls_t0;
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
typedef struct {
ls_native_object * (*code)(void *,ls_value);
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
static inline ls_native_object * ls_callable0_call(ls_callable0 value,ls_value ls_p0) {
ls_native_retain(value.environment);
ls_native_object * result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
typedef struct {
ls_value (*code)(void *,ls_value);
void *environment;
uint64_t identity;
} ls_callable5;
static inline ls_callable5 ls_callable5_retain(ls_callable5 value) {
ls_native_retain(value.environment);
return value;
}
static inline void ls_callable5_release(ls_callable5 value) { ls_native_release(value.environment); }
static inline void ls_callable5_copy(ls_callable5 *destination, ls_callable5 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable5_take(ls_callable5 *destination, ls_callable5 value) {
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable5_clear(ls_callable5 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable5){0};
}
static inline ls_value ls_callable5_call(ls_callable5 value,ls_value ls_p0) {
ls_native_retain(value.environment);
ls_value result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
typedef struct {
ls_value (*code)(void *,ls_value);
void *environment;
uint64_t identity;
} ls_callable6;
static inline ls_callable6 ls_callable6_retain(ls_callable6 value) {
ls_native_retain(value.environment);
return value;
}
static inline void ls_callable6_release(ls_callable6 value) { ls_native_release(value.environment); }
static inline void ls_callable6_copy(ls_callable6 *destination, ls_callable6 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable6_take(ls_callable6 *destination, ls_callable6 value) {
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable6_clear(ls_callable6 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable6){0};
}
static inline ls_value ls_callable6_call(ls_callable6 value,ls_value ls_p0) {
ls_native_retain(value.environment);
ls_value result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
_Static_assert(sizeof(ls_callable0) == sizeof(((ls_value *)0)->as.c), "callable layout");
static ls_value ls_value_callable0(ls_callable0 value) { ls_value result = {LS_CALLABLE, 0}; memcpy(&result.as.c, &value, sizeof value); return result; }
static ls_callable0 ls_value_to_callable0(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable0){0};
if (value.tag != LS_CALLABLE || value.signature != 0) ls_value_mismatch();
ls_callable0 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable5) == sizeof(((ls_value *)0)->as.c), "callable layout");
static ls_value ls_value_callable5(ls_callable5 value) { ls_value result = {LS_CALLABLE, 5}; memcpy(&result.as.c, &value, sizeof value); return result; }
static ls_callable5 ls_value_to_callable5(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable5){0};
if (value.tag != LS_CALLABLE || value.signature != 5) ls_value_mismatch();
ls_callable5 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
_Static_assert(sizeof(ls_callable6) == sizeof(((ls_value *)0)->as.c), "callable layout");
static ls_value ls_value_callable6(ls_callable6 value) { ls_value result = {LS_CALLABLE, 6}; memcpy(&result.as.c, &value, sizeof value); return result; }
static ls_callable6 ls_value_to_callable6(ls_value value) {
if (value.tag == LS_NULL) return (ls_callable6){0};
if (value.tag != LS_CALLABLE || value.signature != 6) ls_value_mismatch();
ls_callable6 result; memcpy(&result, &value.as.c, sizeof result); return result;
}
static void ls_native_undefined_element(void) {
fputs("LilScript native array element is undefined\n", stderr);
abort();
}
static size_t ls_array_relative(int32_t index, size_t length) {
if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
return (size_t)index < length ? (size_t)index : length;
}
struct ls_array0 { ls_native_object owner; size_t length; size_t capacity; ls_value *items; };
static void ls_array0_acquire(ls_value value) { (void)value; }
static void ls_array0_drop(ls_value value) { (void)value; }
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
ls_value *items = realloc(array->items, capacity * sizeof *items);
if (!items) ls_native_resource_failure();
array->items = items; array->capacity = capacity;
}
static int32_t ls_array0_push_owned(ls_array0 *array, ls_value value) {
if (array->length >= (size_t)INT32_MAX) ls_native_resource_failure();
ls_array0_reserve(array, array->length + 1);
array->items[array->length++] = value;
return (int32_t)array->length;
}
static int32_t ls_array0_push(ls_array0 *array, ls_value value) {
ls_array0_acquire(value);
return ls_array0_push_owned(array, value);
}
static void ls_array0_hole(ls_array0 *array) { ls_array0_push(array, (ls_value){0}); }
static ls_value ls_array0_get(ls_array0 *array, int32_t index) {
if (index < 0 || (size_t)index >= array->length) { return (ls_value){0}; }
return array->items[index];
}
static void ls_array0_set(ls_array0 *array, int32_t index, ls_value value) {
if (index >= 0 && (size_t)index < array->length) { ls_array0_acquire(value); ls_array0_drop(array->items[index]); array->items[index] = value; return; }
if (index >= 0 && (size_t)index == array->length) { ls_array0_push(array, value); return; }
ls_native_undefined_element();
}
static ls_value ls_array0_pop(ls_array0 *array) {
if (!array->length) { return (ls_value){0}; }
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
for (size_t low = 0, high = array->length; low + 1 < high; low++, high--) { ls_value value = array->items[low]; array->items[low] = array->items[high - 1]; array->items[high - 1] = value; }
return array;
}
static ls_array0 *ls_array0_fill(ls_array0 *array, ls_value value) {
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
static int32_t ls_array0_index_of(ls_array0 *array, ls_value right) {
for (size_t index = 0; index < array->length; index++) { ls_value left = array->items[index]; if (ls_value_equal(left, right)) return (int32_t)index; }
return -1;
}
static bool ls_array0_includes(ls_array0 *array, ls_value right, int32_t start) {
size_t length = array->length;
size_t from = start >= 0 ? (size_t)start : ls_array_relative(start, length);
for (size_t index = from; index < length; index++) { ls_value left = array->items[index]; if (ls_value_same_zero(left, right)) return true; }
return false;
}
static void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
typedef struct ls_object0 {
ls_native_object owner;
ls_value ls_m0;
} ls_object0;
static void ls_object0_clear_fields(ls_object0 *object) {
(void)object;
}
static void ls_object0_destroy(ls_native_object *owner) { ls_object0_clear_fields((ls_object0 *)owner); }
static const uint16_t ls_s7[] = {108,105,108,115,99,114,105,112,116,};
static const uint16_t ls_s8[] = {117,110,117,115,101,100,};
static void ls_init0(void);
static ls_value ls_fn1(bool ls_c22,ls_value ls_c23);
static ls_value ls_fn2(bool ls_c24);
static ls_value ls_fn3(bool ls_c26);
static void ls_fn4(ls_native_object * ls_c28,ls_value ls_c29);
static bool ls_fn5(ls_value ls_c30,int32_t ls_c31);
static bool ls_fn6(ls_value ls_c32,int32_t ls_c33);
static ls_value ls_fn7(void *ls_env,ls_value ls_c34);
static ls_value ls_fn8(void *ls_env,ls_value ls_c35);
static ls_callable5 ls_closure7(void) {
return (ls_callable5){ls_fn7,NULL,ls_native_fresh_identity()};
}
static ls_callable6 ls_closure8(void) {
return (ls_callable6){ls_fn8,NULL,ls_native_fresh_identity()};
}
static void ls_init0(void) {
ls_value ls_c6 = {0};
ls_value ls_c7 = {0};
ls_value ls_c8 = {0};
ls_value ls_c9 = {0};
ls_value ls_c10 = {0};
ls_array0 * ls_c11 = NULL;
ls_t0 ls_c12;
ls_native_object * ls_c13 = NULL;
ls_callable5 ls_c14 = {0};
double ls_c15;
ls_value ls_c16 = {0};
ls_value ls_c17 = {0};
ls_value ls_c18 = {0};
ls_callable6 ls_c19 = {0};
ls_value ls_c20 = {0};
ls_value ls_c21 = {0};
bool ls_v7;
int32_t ls_v8;
ls_value ls_v9 = {0};
bool ls_v11;
int32_t ls_v12;
ls_value ls_v13 = {0};
bool ls_v15;
ls_string ls_v16;
ls_value ls_v17 = {0};
bool ls_v19;
ls_string ls_v20;
ls_value ls_v21 = {0};
int32_t ls_v22;
ls_value ls_v23 = {0};
ls_value ls_v24 = {0};
bool ls_v25;
ls_value ls_v27 = {0};
ls_value ls_v28 = {0};
bool ls_v29;
ls_value ls_v31 = {0};
ls_value ls_v32 = {0};
bool ls_v33;
ls_value ls_v35 = {0};
ls_value ls_v36 = {0};
bool ls_v37;
ls_value ls_v39 = {0};
int32_t ls_v40;
bool ls_v41;
ls_value ls_v43 = {0};
int32_t ls_v44;
bool ls_v45;
ls_value ls_v47 = {0};
int32_t ls_v48;
bool ls_v49;
ls_value ls_v51 = {0};
ls_value ls_v52 = {0};
bool ls_v53;
ls_value ls_v55 = {0};
ls_value ls_v56 = {0};
int32_t ls_v57;
ls_array0 * ls_v58 = NULL;
ls_array0 * ls_v59 = NULL;
int32_t ls_v60;
ls_value ls_v61 = {0};
int32_t ls_v62;
bool ls_v63;
ls_array0 * ls_v65 = NULL;
int32_t ls_v66;
ls_value ls_v67 = {0};
ls_value ls_v68 = {0};
bool ls_v69;
ls_array0 * ls_v71 = NULL;
int32_t ls_v72;
int32_t ls_v73;
ls_array0 * ls_v74 = NULL;
int32_t ls_v75;
ls_value ls_v76 = {0};
int32_t ls_v77;
bool ls_v78;
int32_t ls_v80;
ls_value ls_v81 = {0};
ls_t0 ls_v82;
ls_t0 ls_v83;
ls_value ls_v84 = {0};
int32_t ls_v85;
bool ls_v86;
ls_value ls_v88 = {0};
ls_value ls_v89 = {0};
bool ls_v90;
ls_value ls_v92 = {0};
ls_native_object * ls_v93 = NULL;
int32_t ls_v95;
ls_native_object * ls_v97 = NULL;
ls_value ls_v98 = {0};
int32_t ls_v99;
bool ls_v100;
ls_native_object * ls_v102 = NULL;
ls_value ls_v103 = {0};
ls_native_object * ls_v104 = NULL;
ls_value ls_v105 = {0};
ls_value ls_v106 = {0};
bool ls_v107;
bool ls_v110;
ls_value ls_v111 = {0};
int32_t ls_v112;
bool ls_v113;
bool ls_v116;
ls_value ls_v117 = {0};
ls_value ls_v118 = {0};
bool ls_v119;
ls_callable5 ls_v121 = {0};
ls_callable5 ls_v122 = {0};
int32_t ls_v123;
ls_value ls_v124 = {0};
int32_t ls_v125;
bool ls_v126;
ls_callable5 ls_v128 = {0};
ls_value ls_v129 = {0};
ls_value ls_v130 = {0};
ls_value ls_v131 = {0};
bool ls_v132;
int32_t ls_v134;
int32_t ls_v135;
int32_t ls_v136;
ls_value ls_v137 = {0};
ls_callable6 ls_v138 = {0};
double ls_v139;
double ls_v140;
bool ls_v141;
ls_value ls_v143 = {0};
double ls_v144;
bool ls_v145;
ls_value ls_v147 = {0};
double ls_v148;
bool ls_v149;
ls_callable6 ls_v151 = {0};
int32_t ls_v152;
ls_value ls_v153 = {0};
double ls_v154;
bool ls_v155;
bool ls_v158;
ls_value ls_v159 = {0};
double ls_v160;
bool ls_v161;
bool ls_v164;
ls_value ls_v165 = {0};
ls_value ls_v166 = {0};
bool ls_v167;
ls_value ls_v169 = {0};
ls_native_object * ls_v170 = NULL;
int32_t ls_v172;
ls_value ls_v174 = {0};
ls_value ls_v176 = {0};
int32_t ls_v177;
bool ls_v178;
ls_value ls_v181 = {0};
int32_t ls_v182;
bool ls_v183;
bool ls_v184;
ls_value ls_v187 = {0};
int32_t ls_v188;
bool ls_v189;
ls_value ls_v192 = {0};
int32_t ls_v193;
bool ls_v194;
bool ls_v195;
ls_v7 = true;
ls_v8 = ls_from_u32(UINT32_C(7));
ls_v9 = ls_fn1(ls_v7,ls_value_int(ls_v8));
ls_c6 = ls_v9;
ls_v11 = false;
ls_v12 = ls_from_u32(UINT32_C(9));
ls_v13 = ls_fn1(ls_v11,ls_value_int(ls_v12));
ls_c7 = ls_v13;
ls_v15 = true;
ls_v16 = (ls_string){ls_s7,sizeof ls_s7/sizeof *ls_s7};
ls_v17 = ls_fn1(ls_v15,ls_value_string(ls_v16));
ls_c8 = ls_v17;
ls_v19 = false;
ls_v20 = (ls_string){ls_s8,sizeof ls_s8/sizeof *ls_s8};
ls_v21 = ls_fn1(ls_v19,ls_value_string(ls_v20));
ls_c9 = ls_v21;
ls_v22 = ls_from_u32(UINT32_C(7));
ls_c10 = ls_value_int(ls_v22);
ls_v23 = ls_c6;
ls_v24 = (ls_value){0};
ls_v25 = !ls_value_equal(ls_v23,ls_v24);
puts(ls_v25 ? "true" : "false");
ls_v27 = ls_c7;
ls_v28 = (ls_value){0};
ls_v29 = ls_value_equal(ls_v27,ls_v28);
puts(ls_v29 ? "true" : "false");
ls_v31 = ls_c8;
ls_v32 = (ls_value){0};
ls_v33 = !ls_value_equal(ls_v31,ls_v32);
puts(ls_v33 ? "true" : "false");
ls_v35 = ls_c9;
ls_v36 = (ls_value){0};
ls_v37 = ls_value_equal(ls_v35,ls_v36);
puts(ls_v37 ? "true" : "false");
ls_v39 = ls_c6;
ls_v40 = ls_from_u32(UINT32_C(7));
ls_v41 = ls_value_equal(ls_v39,ls_value_int(ls_v40));
puts(ls_v41 ? "true" : "false");
ls_v43 = ls_c7;
ls_v44 = ls_from_u32(UINT32_C(9));
ls_v45 = !ls_value_equal(ls_v43,ls_value_int(ls_v44));
puts(ls_v45 ? "true" : "false");
ls_v47 = ls_c7;
ls_v48 = ls_from_u32(UINT32_C(9));
ls_v49 = ls_value_equal(ls_v47,ls_value_int(ls_v48));
puts(ls_v49 ? "true" : "false");
ls_v51 = ls_c6;
ls_v52 = ls_c10;
ls_v53 = ls_value_equal(ls_v51,ls_v52);
puts(ls_v53 ? "true" : "false");
ls_v55 = ls_c6;
ls_v56 = (ls_value){0};
ls_v57 = ls_from_u32(UINT32_C(3));
ls_array0_take(&(ls_v58),ls_array0_new(3));
ls_array0_push(ls_v58,ls_v55);
ls_array0_push(ls_v58,ls_v56);
ls_array0_push(ls_v58,ls_value_int(ls_v57));
ls_array0_copy(&(ls_c11),ls_v58);
ls_array0_copy(&(ls_v59),ls_c11);
ls_v60 = ls_from_u32(UINT32_C(0));
ls_v61 = ls_array0_get(ls_v59,ls_v60);
ls_v62 = ls_from_u32(UINT32_C(7));
ls_v63 = ls_value_equal(ls_v61,ls_value_int(ls_v62));
puts(ls_v63 ? "true" : "false");
ls_array0_copy(&(ls_v65),ls_c11);
ls_v66 = ls_from_u32(UINT32_C(1));
ls_v67 = ls_array0_get(ls_v65,ls_v66);
ls_v68 = (ls_value){0};
ls_v69 = ls_value_equal(ls_v67,ls_v68);
puts(ls_v69 ? "true" : "false");
ls_array0_copy(&(ls_v71),ls_c11);
ls_v72 = ls_from_u32(UINT32_C(1));
ls_v73 = ls_from_u32(UINT32_C(4));
ls_array0_set(ls_v71,ls_v72,ls_value_int(ls_v73));
ls_array0_copy(&(ls_v74),ls_c11);
ls_v75 = ls_from_u32(UINT32_C(1));
ls_v76 = ls_array0_get(ls_v74,ls_v75);
ls_v77 = ls_from_u32(UINT32_C(4));
ls_v78 = ls_value_equal(ls_v76,ls_value_int(ls_v77));
puts(ls_v78 ? "true" : "false");
ls_v80 = ls_from_u32(UINT32_C(5));
ls_v81 = (ls_value){0};
ls_v82 = (ls_t0){ls_value_int(ls_v80),ls_v81};
ls_v83 = ls_v82;
ls_c12 = ls_v83;
ls_v84 = ls_c12.ls_f0;
ls_v85 = ls_from_u32(UINT32_C(5));
ls_v86 = ls_value_equal(ls_v84,ls_value_int(ls_v85));
puts(ls_v86 ? "true" : "false");
ls_v88 = ls_c12.ls_f1;
ls_v89 = (ls_value){0};
ls_v90 = ls_value_equal(ls_v88,ls_v89);
puts(ls_v90 ? "true" : "false");
ls_v92 = (ls_value){0};
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy);
((ls_object0 *)ls_o)->ls_m0 = ls_v92;
ls_object_take(&(ls_v93),(ls_native_object *)ls_o);
}
ls_v95 = ls_from_u32(UINT32_C(6));
ls_fn4(ls_v93,ls_value_int(ls_v95));
ls_object_copy(&(ls_c13),ls_v93);
ls_object_copy(&(ls_v97),ls_c13);
ls_v98 = ((ls_object0 *)ls_v97)->ls_m0;
ls_v99 = ls_from_u32(UINT32_C(6));
ls_v100 = ls_value_equal(ls_v98,ls_value_int(ls_v99));
puts(ls_v100 ? "true" : "false");
ls_object_copy(&(ls_v102),ls_c13);
ls_v103 = (ls_value){0};
((ls_object0 *)ls_v102)->ls_m0 = ls_v103;
ls_object_copy(&(ls_v104),ls_c13);
ls_v105 = ((ls_object0 *)ls_v104)->ls_m0;
ls_v106 = (ls_value){0};
ls_v107 = ls_value_equal(ls_v105,ls_v106);
puts(ls_v107 ? "true" : "false");
ls_v110 = true;
ls_v111 = ls_fn2(ls_v110);
ls_v112 = ls_from_u32(UINT32_C(11));
ls_v113 = ls_value_equal(ls_v111,ls_value_int(ls_v112));
puts(ls_v113 ? "true" : "false");
ls_v116 = false;
ls_v117 = ls_fn2(ls_v116);
ls_v118 = (ls_value){0};
ls_v119 = ls_value_equal(ls_v117,ls_v118);
puts(ls_v119 ? "true" : "false");
ls_callable5_take(&(ls_v121),ls_closure7());
ls_callable5_copy(&(ls_c14),ls_v121);
ls_callable5_copy(&(ls_v122),ls_c14);
ls_v123 = ls_from_u32(UINT32_C(12));
ls_v124 = ls_v122.code(ls_v122.environment,ls_value_int(ls_v123));
ls_v125 = ls_from_u32(UINT32_C(12));
ls_v126 = ls_value_equal(ls_v124,ls_value_int(ls_v125));
puts(ls_v126 ? "true" : "false");
ls_callable5_copy(&(ls_v128),ls_c14);
ls_v129 = (ls_value){0};
ls_v130 = ls_v128.code(ls_v128.environment,ls_v129);
ls_v131 = (ls_value){0};
ls_v132 = ls_value_equal(ls_v130,ls_v131);
puts(ls_v132 ? "true" : "false");
ls_v134 = ls_from_u32(UINT32_C(1));
ls_c15 = (double)(ls_v134);
ls_v135 = ls_from_u32(UINT32_C(8));
ls_c16 = ls_value_int(ls_v135);
ls_v136 = ls_from_u32(UINT32_C(9));
ls_c17 = ls_value_int(ls_v136);
ls_v137 = ls_c17;
ls_c18 = ls_v137;
ls_callable6_take(&(ls_v138),ls_closure8());
ls_callable6_copy(&(ls_c19),ls_v138);
ls_v139 = ls_c15;
ls_v140 = ls_f64_bits(UINT64_C(4607182418800017408));
ls_v141 = ls_v139 == ls_v140;
puts(ls_v141 ? "true" : "false");
ls_v143 = ls_c16;
ls_v144 = ls_f64_bits(UINT64_C(4620693217682128896));
ls_v145 = ls_value_equal(ls_v143,ls_value_float(ls_v144));
puts(ls_v145 ? "true" : "false");
ls_v147 = ls_c18;
ls_v148 = ls_f64_bits(UINT64_C(4621256167635550208));
ls_v149 = ls_value_equal(ls_v147,ls_value_float(ls_v148));
puts(ls_v149 ? "true" : "false");
ls_callable6_copy(&(ls_v151),ls_c19);
ls_v152 = ls_from_u32(UINT32_C(13));
ls_v153 = ls_v151.code(ls_v151.environment,ls_value_int(ls_v152));
ls_v154 = ls_f64_bits(UINT64_C(4623507967449235456));
ls_v155 = ls_value_equal(ls_v153,ls_value_float(ls_v154));
puts(ls_v155 ? "true" : "false");
ls_v158 = true;
ls_v159 = ls_fn3(ls_v158);
ls_v160 = ls_f64_bits(UINT64_C(4624070917402656768));
ls_v161 = ls_value_equal(ls_v159,ls_value_float(ls_v160));
puts(ls_v161 ? "true" : "false");
ls_v164 = false;
ls_v165 = ls_fn3(ls_v164);
ls_v166 = (ls_value){0};
ls_v167 = ls_value_equal(ls_v165,ls_v166);
puts(ls_v167 ? "true" : "false");
ls_v169 = (ls_value){0};
{
ls_object0 *ls_o = ls_native_allocate(sizeof *ls_o, ls_object0_destroy);
((ls_object0 *)ls_o)->ls_m0 = ls_v169;
ls_object_take(&(ls_v170),(ls_native_object *)ls_o);
}
ls_v172 = ls_from_u32(UINT32_C(15));
ls_fn4(ls_v170,ls_value_int(ls_v172));
ls_value_copy(&(ls_c20),ls_value_object(ls_v170));
ls_v174 = (ls_value){0};
ls_value_copy(&(ls_c21),ls_v174);
ls_value_copy(&(ls_v176),ls_c20);
ls_v177 = ls_from_u32(UINT32_C(15));
ls_v178 = ls_fn5(ls_v176,ls_v177);
puts(ls_v178 ? "true" : "false");
ls_value_copy(&(ls_v181),ls_c21);
ls_v182 = ls_from_u32(UINT32_C(15));
ls_v183 = ls_fn5(ls_v181,ls_v182);
ls_v184 = !ls_v183;
puts(ls_v184 ? "true" : "false");
ls_value_copy(&(ls_v187),ls_c20);
ls_v188 = ls_from_u32(UINT32_C(15));
ls_v189 = ls_fn6(ls_v187,ls_v188);
puts(ls_v189 ? "true" : "false");
ls_value_copy(&(ls_v192),ls_c21);
ls_v193 = ls_from_u32(UINT32_C(15));
ls_v194 = ls_fn6(ls_v192,ls_v193);
ls_v195 = !ls_v194;
puts(ls_v195 ? "true" : "false");
ls_value_clear(&ls_v192);
ls_value_clear(&ls_v187);
ls_value_clear(&ls_v181);
ls_value_clear(&ls_v176);
ls_value_clear(&ls_c21);
ls_value_clear(&ls_c20);
ls_object_clear(&ls_v170);
ls_callable6_clear(&ls_v151);
ls_callable6_clear(&ls_c19);
ls_callable6_clear(&ls_v138);
ls_callable5_clear(&ls_v128);
ls_callable5_clear(&ls_v122);
ls_callable5_clear(&ls_c14);
ls_callable5_clear(&ls_v121);
ls_object_clear(&ls_v104);
ls_object_clear(&ls_v102);
ls_object_clear(&ls_v97);
ls_object_clear(&ls_c13);
ls_object_clear(&ls_v93);
ls_array0_clear(&ls_v74);
ls_array0_clear(&ls_v71);
ls_array0_clear(&ls_v65);
ls_array0_clear(&ls_v59);
ls_array0_clear(&ls_c11);
ls_array0_clear(&ls_v58);
}
static ls_value ls_fn1(bool ls_c22,ls_value ls_c23) {
ls_value_retain(ls_c23);
bool ls_v0;
ls_value ls_v1 = {0};
ls_value ls_v2 = {0};
ls_value ls_v3 = {0};
ls_v0 = ls_c22;
if (ls_v0) {
ls_value_copy(&(ls_v1),ls_c23);
ls_value_copy(&(ls_v2),ls_v1);
{
ls_value ls_return = ls_v2;
ls_value_retain(ls_return);
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
ls_value_clear(&ls_c23);
return ls_return;
}
ls_value_clear(&ls_v2);
ls_value_clear(&ls_v1);
}
ls_v3 = (ls_value){0};
{
ls_value ls_return = ls_v3;
ls_value_retain(ls_return);
ls_value_clear(&ls_c23);
return ls_return;
}
ls_value_clear(&ls_c23);
}
static ls_value ls_fn2(bool ls_c24) {
ls_value ls_c25 = {0};
ls_value ls_v0 = {0};
bool ls_v1;
int32_t ls_v2;
ls_value ls_v3 = {0};
ls_v0 = (ls_value){0};
ls_c25 = ls_v0;
ls_v1 = ls_c24;
if (ls_v1) {
ls_v2 = ls_from_u32(UINT32_C(11));
ls_c25 = ls_value_int(ls_v2);
}
ls_v3 = ls_c25;
{
ls_value ls_return = ls_v3;
return ls_return;
}
}
static ls_value ls_fn3(bool ls_c26) {
ls_value ls_c27 = {0};
bool ls_v1;
int32_t ls_v2;
ls_value ls_v3 = {0};
ls_value ls_v4 = {0};
ls_v1 = ls_c26;
ls_v2 = ls_from_u32(UINT32_C(14));
ls_v3 = ls_fn1(ls_v1,ls_value_int(ls_v2));
ls_c27 = ls_v3;
ls_v4 = ls_c27;
{
ls_value ls_return = ls_v4;
return ls_return;
}
}
static void ls_fn4(ls_native_object * ls_c28,ls_value ls_c29) {
ls_native_retain(ls_c28);
ls_native_object * ls_v0 = NULL;
ls_value ls_v1 = {0};
ls_object_copy(&(ls_v0),ls_c28);
ls_v1 = ls_c29;
((ls_object0 *)ls_v0)->ls_m0 = ls_v1;
ls_object_clear(&ls_v0);
ls_object_clear(&ls_c28);
}
static bool ls_fn5(ls_value ls_c30,int32_t ls_c31) {
ls_value_retain(ls_c30);
ls_value ls_v0 = {0};
ls_value ls_v1 = {0};
bool ls_v2;
ls_native_object * ls_v3 = NULL;
ls_value ls_v4 = {0};
int32_t ls_v5;
bool ls_v6;
bool ls_v7;
ls_value_copy(&(ls_v0),ls_c30);
ls_v1 = (ls_value){0};
ls_v2 = !ls_value_equal(ls_v0,ls_v1);
if (ls_v2) {
ls_object_copy(&(ls_v3),ls_value_to_object(ls_c30));
ls_v4 = ((ls_object0 *)ls_v3)->ls_m0;
ls_v5 = ls_c31;
ls_v6 = ls_value_equal(ls_v4,ls_value_int(ls_v5));
{
bool ls_return = ls_v6;
ls_object_clear(&ls_v3);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c30);
return ls_return;
}
ls_object_clear(&ls_v3);
}
ls_v7 = false;
{
bool ls_return = ls_v7;
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c30);
return ls_return;
}
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c30);
}
static bool ls_fn6(ls_value ls_c32,int32_t ls_c33) {
ls_value_retain(ls_c32);
ls_value ls_v0 = {0};
ls_value ls_v1 = {0};
bool ls_v2;
bool ls_v3;
ls_native_object * ls_v4 = NULL;
ls_value ls_v5 = {0};
int32_t ls_v6;
bool ls_v7;
ls_value_copy(&(ls_v0),ls_c32);
ls_v1 = (ls_value){0};
ls_v2 = ls_value_equal(ls_v0,ls_v1);
if (ls_v2) {
ls_v3 = false;
{
bool ls_return = ls_v3;
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c32);
return ls_return;
}
} else {
ls_object_copy(&(ls_v4),ls_value_to_object(ls_c32));
ls_v5 = ((ls_object0 *)ls_v4)->ls_m0;
ls_v6 = ls_c33;
ls_v7 = ls_value_equal(ls_v5,ls_value_int(ls_v6));
{
bool ls_return = ls_v7;
ls_object_clear(&ls_v4);
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c32);
return ls_return;
}
ls_object_clear(&ls_v4);
}
ls_value_clear(&ls_v0);
ls_value_clear(&ls_c32);
}
static ls_value ls_fn7(void *ls_env,ls_value ls_c34) {
ls_value ls_v0 = {0};
ls_v0 = ls_c34;
{
ls_value ls_return = ls_v0;
return ls_return;
}
}
static ls_value ls_fn8(void *ls_env,ls_value ls_c35) {
ls_value ls_v0 = {0};
ls_v0 = ls_c35;
{
ls_value ls_return = ls_v0;
return ls_return;
}
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(9);
ls_init0();
fflush(stdout);
ls_strings_release();
return 0;
}
