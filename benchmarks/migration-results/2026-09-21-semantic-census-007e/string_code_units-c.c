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
static inline double ls_f64_bits(uint64_t bits) {
    double value;
    memcpy(&value, &bits, sizeof value);
    return value;
}
static inline int32_t ls_mul(int32_t left, int32_t right) {
    return ls_to_i32(ls_f64((double)left * (double)right));
}
static inline int32_t ls_string_length(ls_string value) {
    return ls_from_u32((uint32_t)value.length);
}
static inline int32_t ls_char_code_at(ls_string value, int32_t index) {
    return index < 0 || (size_t)index >= value.length ? 0 : (int32_t)value.data[(size_t)index];
}
static inline ls_string ls_char_at(ls_string value, int32_t index) {
    if (index < 0 || (size_t)index >= value.length) return (ls_string){NULL, 0};
    return (ls_string){value.data + (size_t)index, 1};
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
static const uint16_t ls_s1[] = {65,55357,56832,90,};
static const uint16_t ls_s3[] = {76,105,108,83,99,114,105,112,116,};
static void ls_init0(void);
static int32_t ls_fn1(ls_string ls_c6);
static void ls_init0(void) {
ls_string ls_c1;
ls_string ls_c2;
int32_t ls_c3;
int32_t ls_c4;
double ls_c5;
ls_string ls_v1;
ls_string ls_v2;
int32_t ls_v3;
ls_string ls_v5;
int32_t ls_v6;
int32_t ls_v7;
ls_string ls_v9;
int32_t ls_v10;
int32_t ls_v11;
ls_string ls_v13;
int32_t ls_v14;
int32_t ls_v15;
ls_string ls_v17;
int32_t ls_v18;
int32_t ls_v19;
ls_string ls_v21;
int32_t ls_v22;
int32_t ls_v23;
int32_t ls_v24;
ls_string ls_v26;
int32_t ls_v27;
int32_t ls_v28;
ls_string ls_v30;
int32_t ls_v31;
ls_string ls_v32;
ls_string ls_v34;
int32_t ls_v35;
ls_string ls_v36;
int32_t ls_v37;
int32_t ls_v38;
ls_string ls_v40;
int32_t ls_v41;
ls_string ls_v42;
int32_t ls_v43;
int32_t ls_v44;
ls_string ls_v46;
int32_t ls_v47;
ls_string ls_v48;
ls_string ls_v50;
int32_t ls_v51;
ls_string ls_v52;
ls_string ls_v53;
bool ls_v54;
ls_string ls_v56;
int32_t ls_v57;
ls_string ls_v58;
ls_string ls_v59;
int32_t ls_v60;
ls_string ls_v61;
ls_string ls_v62;
ls_string ls_v63;
int32_t ls_v64;
ls_string ls_v66;
int32_t ls_v67;
int32_t ls_v68;
ls_string ls_v70;
int32_t ls_v71;
int32_t ls_v72;
int32_t ls_v74;
int32_t ls_v75;
int32_t ls_v76;
int32_t ls_v77;
int32_t ls_v79;
int32_t ls_v80;
int32_t ls_v81;
int32_t ls_v82;
ls_string ls_v85;
int32_t ls_v86;
int32_t ls_v87;
int32_t ls_v88;
int32_t ls_v89;
bool ls_v90;
double ls_v91;
double ls_v92;
double ls_v93;
double ls_v94;
ls_v1 = (ls_string){ls_s1,sizeof ls_s1/sizeof *ls_s1};
ls_c1 = ls_v1;
ls_v2 = ls_c1;
ls_v3 = ls_string_length(ls_v2);
printf("%ld\n",(long)ls_v3);
ls_v5 = ls_c1;
ls_v6 = ls_from_u32(UINT32_C(0));
ls_v7 = ls_char_code_at(ls_v5,ls_v6);
printf("%ld\n",(long)ls_v7);
ls_v9 = ls_c1;
ls_v10 = ls_from_u32(UINT32_C(1));
ls_v11 = ls_char_code_at(ls_v9,ls_v10);
printf("%ld\n",(long)ls_v11);
ls_v13 = ls_c1;
ls_v14 = ls_from_u32(UINT32_C(2));
ls_v15 = ls_char_code_at(ls_v13,ls_v14);
printf("%ld\n",(long)ls_v15);
ls_v17 = ls_c1;
ls_v18 = ls_from_u32(UINT32_C(3));
ls_v19 = ls_char_code_at(ls_v17,ls_v18);
printf("%ld\n",(long)ls_v19);
ls_v21 = ls_c1;
ls_v22 = ls_from_u32(UINT32_C(1));
ls_v23 = ls_from_u32((uint32_t)(UINT32_C(0) - (uint32_t)ls_v22));
ls_v24 = ls_char_code_at(ls_v21,ls_v23);
printf("%ld\n",(long)ls_v24);
ls_v26 = ls_c1;
ls_v27 = ls_from_u32(UINT32_C(9));
ls_v28 = ls_char_code_at(ls_v26,ls_v27);
printf("%ld\n",(long)ls_v28);
ls_v30 = ls_c1;
ls_v31 = ls_from_u32(UINT32_C(0));
ls_v32 = ls_char_at(ls_v30,ls_v31);
ls_print_string(ls_v32);
ls_v34 = ls_c1;
ls_v35 = ls_from_u32(UINT32_C(1));
ls_v36 = ls_char_at(ls_v34,ls_v35);
ls_v37 = ls_from_u32(UINT32_C(0));
ls_v38 = ls_char_code_at(ls_v36,ls_v37);
printf("%ld\n",(long)ls_v38);
ls_v40 = ls_c1;
ls_v41 = ls_from_u32(UINT32_C(2));
ls_v42 = ls_char_at(ls_v40,ls_v41);
ls_v43 = ls_from_u32(UINT32_C(0));
ls_v44 = ls_char_code_at(ls_v42,ls_v43);
printf("%ld\n",(long)ls_v44);
ls_v46 = ls_c1;
ls_v47 = ls_from_u32(UINT32_C(3));
ls_v48 = ls_char_at(ls_v46,ls_v47);
ls_print_string(ls_v48);
ls_v50 = ls_c1;
ls_v51 = ls_from_u32(UINT32_C(9));
ls_v52 = ls_char_at(ls_v50,ls_v51);
ls_v53 = (ls_string){NULL,0};
ls_v54 = ls_string_equal(ls_v52,ls_v53);
puts(ls_v54 ? "true" : "false");
ls_v56 = ls_c1;
ls_v57 = ls_from_u32(UINT32_C(1));
ls_v58 = ls_char_at(ls_v56,ls_v57);
ls_v59 = ls_c1;
ls_v60 = ls_from_u32(UINT32_C(2));
ls_v61 = ls_char_at(ls_v59,ls_v60);
ls_v62 = ls_string_concat(ls_v58,ls_v61);
ls_c2 = ls_v62;
ls_v63 = ls_c2;
ls_v64 = ls_string_length(ls_v63);
printf("%ld\n",(long)ls_v64);
ls_v66 = ls_c2;
ls_v67 = ls_from_u32(UINT32_C(0));
ls_v68 = ls_char_code_at(ls_v66,ls_v67);
printf("%ld\n",(long)ls_v68);
ls_v70 = ls_c2;
ls_v71 = ls_from_u32(UINT32_C(1));
ls_v72 = ls_char_code_at(ls_v70,ls_v71);
printf("%ld\n",(long)ls_v72);
ls_v74 = ls_from_u32(UINT32_C(5));
ls_v75 = ls_from_u32(UINT32_C(3));
ls_v76 = ls_from_u32((uint32_t)ls_v74 ^ (uint32_t)ls_v75);
ls_c3 = ls_v76;
ls_v77 = ls_c3;
printf("%ld\n",(long)ls_v77);
ls_v79 = ls_c3;
ls_v80 = ls_from_u32(UINT32_C(12));
ls_v81 = ls_from_u32((uint32_t)ls_v79 ^ (uint32_t)ls_v80);
ls_c3 = ls_v81;
ls_v82 = ls_c3;
printf("%ld\n",(long)ls_v82);
ls_v85 = (ls_string){ls_s3,sizeof ls_s3/sizeof *ls_s3};
ls_v86 = ls_fn1(ls_v85);
ls_c4 = ls_v86;
ls_v87 = ls_c4;
ls_c5 = (double)(ls_v87);
ls_v88 = ls_c4;
ls_v89 = ls_from_u32(UINT32_C(0));
ls_v90 = ls_v88 < ls_v89;
if (ls_v90) {
ls_v91 = ls_c5;
ls_v92 = ls_f64_bits(UINT64_C(4751297606875873280));
ls_v93 = ls_f64((double)ls_v91 + (double)ls_v92);
ls_c5 = ls_v93;
}
ls_v94 = ls_c5;
ls_print_number(ls_v94);
}
static int32_t ls_fn1(ls_string ls_c6) {
int32_t ls_c7;
int32_t ls_c8;
int32_t ls_v0;
ls_string ls_v1;
int32_t ls_v2;
int32_t ls_v3;
int32_t ls_v4;
bool ls_v5;
int32_t ls_v6;
int32_t ls_v7;
int32_t ls_v8;
int32_t ls_v9;
int32_t ls_v10;
int32_t ls_v11;
ls_string ls_v12;
int32_t ls_v13;
int32_t ls_v14;
int32_t ls_v15;
int32_t ls_v16;
ls_v0 = ls_from_u32(UINT32_C(5381));
ls_c7 = ls_v0;
ls_v1 = ls_c6;
ls_v2 = ls_string_length(ls_v1);
ls_c8 = ls_v2;
ls_test21: ;
ls_v3 = ls_c8;
ls_v4 = ls_from_u32(UINT32_C(0));
ls_v5 = ls_v3 > ls_v4;
if (!ls_v5) goto ls_end21;
ls_v6 = ls_c8;
ls_v7 = ls_from_u32(UINT32_C(1));
ls_v8 = ls_from_u32((uint32_t)((uint32_t)ls_v6 - (uint32_t)ls_v7));
ls_c8 = ls_v8;
ls_v9 = ls_c7;
ls_v10 = ls_from_u32(UINT32_C(33));
ls_v11 = ls_mul(ls_v9,ls_v10);
ls_v12 = ls_c6;
ls_v13 = ls_c8;
ls_v14 = ls_char_code_at(ls_v12,ls_v13);
ls_v15 = ls_from_u32((uint32_t)ls_v11 ^ (uint32_t)ls_v14);
ls_c7 = ls_v15;
ls_update21: ;
goto ls_test21;
ls_end21: ;
ls_v16 = ls_c7;
return ls_v16;
}
int main(void) {
if (!ls_runtime_init()) return 1;
ls_init0();
fflush(stdout);
ls_strings_release();
return 0;
}
