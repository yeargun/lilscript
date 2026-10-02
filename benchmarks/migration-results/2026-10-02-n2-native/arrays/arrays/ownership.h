#ifndef LILSCRIPT_NATIVE_CALLBACK_ABI_V2_H
#define LILSCRIPT_NATIVE_CALLBACK_ABI_V2_H
#define LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION 2
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
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
#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
void ls_native_collect_cycles(void);
typedef struct ls_native_object ls_native_object;
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
#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void);
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
typedef int32_t host_assertNativeCount_arg0;
typedef void host_assertNativeCount_result;
void host_assertNativeCount(int32_t);
typedef void host_collectNative_result;
void host_collectNative(void);
typedef int32_t host_assertMaxNativeCount_arg0;
typedef void host_assertMaxNativeCount_result;
void host_assertMaxNativeCount(int32_t);
#endif
