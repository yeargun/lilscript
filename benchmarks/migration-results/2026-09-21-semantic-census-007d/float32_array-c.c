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
typedef struct { ls_native_object owner; size_t length; uint8_t *bytes; } ls_buffer;
typedef struct { ls_native_object owner; ls_native_object *buffer; size_t offset; size_t length; } ls_typed;
static void ls_binary_failure(const char *message) {
    fputs(message, stderr);
    fputc('\n', stderr);
    abort();
}
static void ls_buffer_destroy(ls_native_object *owner) { free(((ls_buffer *)owner)->bytes); }
static ls_native_object *ls_buffer_new(int32_t length) {
    if (length < 0) ls_binary_failure("LilScript native buffer length is negative");
    ls_buffer *buffer = ls_native_allocate(sizeof *buffer, ls_buffer_destroy);
    buffer->length = (size_t)length;
    buffer->bytes = calloc(length ? (size_t)length : 1, 1);
    if (!buffer->bytes) ls_native_resource_failure();
    return &buffer->owner;
}
static int32_t ls_buffer_byte_length(ls_native_object *owner) { return (int32_t)((ls_buffer *)owner)->length; }
static size_t ls_binary_relative(int32_t index, size_t length) {
    if (index < 0) return (size_t)-(int64_t)index >= length ? 0 : length - (size_t)-(int64_t)index;
    return (size_t)index < length ? (size_t)index : length;
}
static ls_native_object *ls_buffer_slice(ls_native_object *owner, int32_t start, int32_t end) {
    ls_buffer *buffer = (ls_buffer *)owner;
    size_t from = ls_binary_relative(start, buffer->length), to = ls_binary_relative(end, buffer->length);
    size_t length = to > from ? to - from : 0;
    ls_native_object *result = ls_buffer_new((int32_t)length);
    if (length) memcpy(((ls_buffer *)result)->bytes, buffer->bytes + from, length);
    return result;
}
static void ls_typed_destroy(ls_native_object *owner) { ls_native_release(((ls_typed *)owner)->buffer); }
static ls_native_object *ls_typed_view(ls_native_object *buffer, size_t offset, size_t length) {
    ls_typed *view = ls_native_allocate(sizeof *view, ls_typed_destroy);
    ls_native_retain(buffer);
    view->buffer = buffer;
    view->offset = offset;
    view->length = length;
    return &view->owner;
}
static ls_native_object *ls_typed_new(int32_t length, size_t size) {
    if (length < 0) ls_binary_failure("LilScript native typed array length is negative");
    if ((size_t)length > (size_t)INT32_MAX / size) ls_native_resource_failure();
    ls_native_object *buffer = ls_buffer_new((int32_t)((size_t)length * size));
    ls_native_object *view = ls_typed_view(buffer, 0, (size_t)length);
    ls_native_release(buffer);
    return view;
}
static ls_native_object *ls_typed_over(ls_native_object *buffer, size_t size) {
    size_t length = ((ls_buffer *)buffer)->length;
    if (length % size) ls_binary_failure("LilScript native buffer length is not a multiple of the element size");
    return ls_typed_view(buffer, 0, length / size);
}
static int32_t ls_typed_length(ls_native_object *owner) { return (int32_t)((ls_typed *)owner)->length; }
static int32_t ls_typed_byte_offset(ls_native_object *owner) { return (int32_t)((ls_typed *)owner)->offset; }
static ls_native_object *ls_typed_buffer(ls_native_object *owner) { return ((ls_typed *)owner)->buffer; }
static uint8_t *ls_typed_at(ls_native_object *owner, int32_t index, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    if (index < 0 || (size_t)index >= view->length) return NULL;
    return ((ls_buffer *)view->buffer)->bytes + view->offset + (size_t)index * size;
}
static ls_native_object *ls_typed_subarray(ls_native_object *owner, int32_t start, int32_t end, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    size_t from = ls_binary_relative(start, view->length), to = ls_binary_relative(end, view->length);
    return ls_typed_view(view->buffer, view->offset + from * size, to > from ? to - from : 0);
}
static ls_native_object *ls_typed_slice(ls_native_object *owner, int32_t start, int32_t end, size_t size) {
    ls_typed *view = (ls_typed *)owner;
    size_t from = ls_binary_relative(start, view->length), to = ls_binary_relative(end, view->length);
    size_t length = to > from ? to - from : 0;
    ls_native_object *result = ls_typed_new((int32_t)length, size);
    if (length) memcpy(((ls_buffer *)((ls_typed *)result)->buffer)->bytes, ((ls_buffer *)view->buffer)->bytes + view->offset + from * size, length * size);
    return result;
}
static void ls_typed_undefined(void) { ls_binary_failure("LilScript native typed array element is undefined"); }
#define LS_TYPED_INT(name, type, read, write) \
static int32_t ls_typed_get_##name(ls_native_object *owner, int32_t index) { \
    uint8_t *at = ls_typed_at(owner, index, sizeof(type)); \
    if (!at) return 0; \
    type value; memcpy(&value, at, sizeof value); return read; \
} \
static void ls_typed_set_##name(ls_native_object *owner, int32_t index, int32_t input) { \
    uint8_t *at = ls_typed_at(owner, index, sizeof(type)); \
    if (!at) return; \
    type value = write; memcpy(at, &value, sizeof value); \
}
LS_TYPED_INT(int8, int8_t, (int32_t)value, (int8_t)(uint8_t)(uint32_t)input)
LS_TYPED_INT(uint8, uint8_t, (int32_t)value, (uint8_t)(uint32_t)input)
LS_TYPED_INT(uint8c, uint8_t, (int32_t)value, (uint8_t)(input < 0 ? 0 : input > 255 ? 255 : input))
LS_TYPED_INT(int16, int16_t, (int32_t)value, (int16_t)(uint16_t)(uint32_t)input)
LS_TYPED_INT(uint16, uint16_t, (int32_t)value, (uint16_t)(uint32_t)input)
LS_TYPED_INT(int32, int32_t, value, input)
LS_TYPED_INT(uint32, uint32_t, ls_from_u32(value), (uint32_t)input)
static double ls_typed_get_float32(ls_native_object *owner, int32_t index) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(float));
    if (!at) ls_typed_undefined();
    float value; memcpy(&value, at, sizeof value); return (double)value;
}
static void ls_typed_set_float32(ls_native_object *owner, int32_t index, double input) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(float));
    if (!at) return;
    float value = (float)input; memcpy(at, &value, sizeof value);
}
static double ls_typed_get_float64(ls_native_object *owner, int32_t index) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(double));
    if (!at) ls_typed_undefined();
    double value; memcpy(&value, at, sizeof value); return value;
}
static void ls_typed_set_float64(ls_native_object *owner, int32_t index, double input) {
    uint8_t *at = ls_typed_at(owner, index, sizeof(double));
    if (!at) return;
    memcpy(at, &input, sizeof input);
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
static void ls_object_copy(ls_native_object **slot, ls_native_object *value) { ls_native_retain(value); ls_native_release(*slot); *slot = value; }
static void ls_object_take(ls_native_object **slot, ls_native_object *value) { ls_native_release(*slot); *slot = value; }
static void ls_object_clear(ls_native_object **slot) { ls_native_release(*slot); *slot = NULL; }
static void ls_init0(void);
static void ls_init0(void) {
ls_native_object * ls_c0 = NULL;
ls_native_object * ls_c1 = NULL;
ls_native_object * ls_c2 = NULL;
ls_native_object * ls_c3 = NULL;
ls_native_object * ls_c4 = NULL;
ls_native_object * ls_c5 = NULL;
int32_t ls_v0;
ls_native_object * ls_v1 = NULL;
ls_native_object * ls_v2 = NULL;
ls_native_object * ls_v3 = NULL;
ls_native_object * ls_v4 = NULL;
int32_t ls_v5;
ls_native_object * ls_v7 = NULL;
int32_t ls_v8;
ls_native_object * ls_v10 = NULL;
int32_t ls_v11;
ls_native_object * ls_v13 = NULL;
int32_t ls_v14;
ls_native_object * ls_v16 = NULL;
ls_value ls_v17 = {0};
ls_native_object * ls_v18 = NULL;
bool ls_v19;
ls_native_object * ls_v21 = NULL;
int32_t ls_v22;
double ls_v23;
ls_native_object * ls_v24 = NULL;
int32_t ls_v25;
double ls_v26;
double ls_v27;
double ls_v28;
ls_native_object * ls_v29 = NULL;
int32_t ls_v30;
double ls_v31;
ls_native_object * ls_v33 = NULL;
int32_t ls_v34;
double ls_v35;
double ls_v36;
double ls_v37;
double ls_v38;
bool ls_v39;
ls_native_object * ls_v41 = NULL;
int32_t ls_v42;
int32_t ls_v43;
ls_native_object * ls_v44 = NULL;
ls_native_object * ls_v45 = NULL;
int32_t ls_v46;
ls_native_object * ls_v48 = NULL;
int32_t ls_v49;
ls_native_object * ls_v51 = NULL;
int32_t ls_v52;
double ls_v53;
ls_native_object * ls_v54 = NULL;
int32_t ls_v55;
double ls_v56;
ls_native_object * ls_v58 = NULL;
int32_t ls_v59;
int32_t ls_v60;
ls_native_object * ls_v61 = NULL;
ls_native_object * ls_v62 = NULL;
int32_t ls_v63;
ls_native_object * ls_v65 = NULL;
int32_t ls_v66;
double ls_v67;
ls_native_object * ls_v68 = NULL;
int32_t ls_v69;
double ls_v70;
ls_native_object * ls_v72 = NULL;
int32_t ls_v73;
double ls_v74;
int32_t ls_v76;
ls_native_object * ls_v77 = NULL;
ls_native_object * ls_v78 = NULL;
ls_native_object * ls_v79 = NULL;
ls_native_object * ls_v80 = NULL;
int32_t ls_v81;
double ls_v82;
ls_native_object * ls_v83 = NULL;
int32_t ls_v84;
ls_native_object * ls_v86 = NULL;
int32_t ls_v87;
double ls_v88;
ls_v0 = ls_from_u32(UINT32_C(16));
ls_object_take(&(ls_v1),ls_buffer_new(ls_v0));
ls_object_copy(&(ls_c0),ls_v1);
ls_object_copy(&(ls_v2),ls_c0);
ls_object_take(&(ls_v3),ls_typed_over(ls_v2, 4));
ls_object_copy(&(ls_c1),ls_v3);
ls_object_copy(&(ls_v4),ls_c0);
ls_v5 = ls_buffer_byte_length(ls_v4);
printf("%ld\n",(long)ls_v5);
ls_object_copy(&(ls_v7),ls_c1);
ls_v8 = ls_typed_length(ls_v7);
printf("%ld\n",(long)ls_v8);
ls_object_copy(&(ls_v10),ls_c1);
ls_v11 = (int32_t)(ls_typed_length(ls_v10) * 4);
printf("%ld\n",(long)ls_v11);
ls_object_copy(&(ls_v13),ls_c1);
ls_v14 = ls_typed_byte_offset(ls_v13);
printf("%ld\n",(long)ls_v14);
ls_object_copy(&(ls_v16),ls_c1);
ls_value_copy(&(ls_v17),ls_value_object(ls_typed_buffer(ls_v16)));
ls_object_copy(&(ls_v18),ls_c0);
ls_v19 = ls_value_equal(ls_v17,ls_value_object(ls_v18));
puts(ls_v19 ? "true" : "false");
ls_object_copy(&(ls_v21),ls_c1);
ls_v22 = ls_from_u32(UINT32_C(0));
ls_v23 = ls_f64_bits(UINT64_C(4608308318706860032));
ls_typed_set_float32(ls_v21,ls_v22,ls_v23);
ls_object_copy(&(ls_v24),ls_c1);
ls_v25 = ls_from_u32(UINT32_C(1));
ls_v26 = ls_f64_bits(UINT64_C(4607182418800017408));
ls_v27 = ls_f64_bits(UINT64_C(4613937818241073152));
ls_v28 = ls_f64((double)ls_v26 / (double)ls_v27);
ls_typed_set_float32(ls_v24,ls_v25,ls_v28);
ls_object_copy(&(ls_v29),ls_c1);
ls_v30 = ls_from_u32(UINT32_C(0));
ls_v31 = ls_typed_get_float32(ls_v29,ls_v30);
ls_print_number(ls_v31);
ls_object_copy(&(ls_v33),ls_c1);
ls_v34 = ls_from_u32(UINT32_C(1));
ls_v35 = ls_typed_get_float32(ls_v33,ls_v34);
ls_v36 = ls_f64_bits(UINT64_C(4607182418800017408));
ls_v37 = ls_f64_bits(UINT64_C(4613937818241073152));
ls_v38 = ls_f64((double)ls_v36 / (double)ls_v37);
ls_v39 = ls_v35 != ls_v38;
puts(ls_v39 ? "true" : "false");
ls_object_copy(&(ls_v41),ls_c1);
ls_v42 = ls_from_u32(UINT32_C(1));
ls_v43 = ls_from_u32(UINT32_C(3));
ls_object_take(&(ls_v44),ls_typed_subarray(ls_v41, ls_v42, ls_v43, 4));
ls_object_copy(&(ls_c2),ls_v44);
ls_object_copy(&(ls_v45),ls_c2);
ls_v46 = ls_typed_length(ls_v45);
printf("%ld\n",(long)ls_v46);
ls_object_copy(&(ls_v48),ls_c2);
ls_v49 = ls_typed_byte_offset(ls_v48);
printf("%ld\n",(long)ls_v49);
ls_object_copy(&(ls_v51),ls_c2);
ls_v52 = ls_from_u32(UINT32_C(0));
ls_v53 = ls_f64_bits(UINT64_C(4612811918334230528));
ls_typed_set_float32(ls_v51,ls_v52,ls_v53);
ls_object_copy(&(ls_v54),ls_c1);
ls_v55 = ls_from_u32(UINT32_C(1));
ls_v56 = ls_typed_get_float32(ls_v54,ls_v55);
ls_print_number(ls_v56);
ls_object_copy(&(ls_v58),ls_c1);
ls_v59 = ls_from_u32(UINT32_C(1));
ls_v60 = ls_from_u32(UINT32_C(3));
ls_object_take(&(ls_v61),ls_typed_slice(ls_v58, ls_v59, ls_v60, 4));
ls_object_copy(&(ls_c3),ls_v61);
ls_object_copy(&(ls_v62),ls_c3);
ls_v63 = ls_typed_length(ls_v62);
printf("%ld\n",(long)ls_v63);
ls_object_copy(&(ls_v65),ls_c3);
ls_v66 = ls_from_u32(UINT32_C(0));
ls_v67 = ls_f64_bits(UINT64_C(4615063718147915776));
ls_typed_set_float32(ls_v65,ls_v66,ls_v67);
ls_object_copy(&(ls_v68),ls_c3);
ls_v69 = ls_from_u32(UINT32_C(0));
ls_v70 = ls_typed_get_float32(ls_v68,ls_v69);
ls_print_number(ls_v70);
ls_object_copy(&(ls_v72),ls_c1);
ls_v73 = ls_from_u32(UINT32_C(1));
ls_v74 = ls_typed_get_float32(ls_v72,ls_v73);
ls_print_number(ls_v74);
ls_v76 = ls_from_u32(UINT32_C(8));
ls_object_take(&(ls_v77),ls_buffer_new(ls_v76));
ls_object_copy(&(ls_c4),ls_v77);
ls_object_copy(&(ls_v78),ls_c4);
ls_object_take(&(ls_v79),ls_typed_over(ls_v78, 4));
ls_object_copy(&(ls_c5),ls_v79);
ls_object_copy(&(ls_v80),ls_c5);
ls_v81 = ls_from_u32(UINT32_C(1));
ls_v82 = ls_f64_bits(UINT64_C(4616752568008179712));
ls_typed_set_float32(ls_v80,ls_v81,ls_v82);
ls_object_copy(&(ls_v83),ls_c5);
ls_v84 = ls_typed_length(ls_v83);
printf("%ld\n",(long)ls_v84);
ls_object_copy(&(ls_v86),ls_c5);
ls_v87 = ls_from_u32(UINT32_C(1));
ls_v88 = ls_typed_get_float32(ls_v86,ls_v87);
ls_print_number(ls_v88);
ls_object_clear(&ls_v86);
ls_object_clear(&ls_v83);
ls_object_clear(&ls_v80);
ls_object_clear(&ls_c5);
ls_object_clear(&ls_v79);
ls_object_clear(&ls_v78);
ls_object_clear(&ls_c4);
ls_object_clear(&ls_v77);
ls_object_clear(&ls_v72);
ls_object_clear(&ls_v68);
ls_object_clear(&ls_v65);
ls_object_clear(&ls_v62);
ls_object_clear(&ls_c3);
ls_object_clear(&ls_v61);
ls_object_clear(&ls_v58);
ls_object_clear(&ls_v54);
ls_object_clear(&ls_v51);
ls_object_clear(&ls_v48);
ls_object_clear(&ls_v45);
ls_object_clear(&ls_c2);
ls_object_clear(&ls_v44);
ls_object_clear(&ls_v41);
ls_object_clear(&ls_v33);
ls_object_clear(&ls_v29);
ls_object_clear(&ls_v24);
ls_object_clear(&ls_v21);
ls_object_clear(&ls_v18);
ls_value_clear(&ls_v17);
ls_object_clear(&ls_v16);
ls_object_clear(&ls_v13);
ls_object_clear(&ls_v10);
ls_object_clear(&ls_v7);
ls_object_clear(&ls_v4);
ls_object_clear(&ls_c1);
ls_object_clear(&ls_v3);
ls_object_clear(&ls_v2);
ls_object_clear(&ls_c0);
ls_object_clear(&ls_v1);
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_native_identity_counter = UINT64_C(1);
ls_init0();
fflush(stdout);
ls_strings_release();
return 0;
}
