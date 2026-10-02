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

#include <stdbool.h>
bool ls_native_exception_pending(void);
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
void ls_native_exception_raise(ls_value value);
ls_value ls_native_exception_take(void);
typedef struct ls_native_array ls_array0;
typedef struct ls_native_array ls_array1;
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
ls_string (*code)(void *);
void *environment;
uint64_t identity;
} ls_callable2;
typedef struct {
ls_string (*code)(void *,ls_callable2);
void *environment;
uint64_t identity;
} ls_callable3;
typedef struct {
ls_string (*code)(void *,ls_string);
void *environment;
uint64_t identity;
} ls_callable7;
typedef struct {
bool (*code)(void *,ls_string);
void *environment;
uint64_t identity;
} ls_callable10;
typedef struct {
ls_string (*code)(void *,ls_string,ls_string);
void *environment;
uint64_t identity;
} ls_callable12;
typedef struct {
void (*code)(void *,ls_string);
void *environment;
uint64_t identity;
} ls_callable15;
#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void);
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
static LS_NATIVE_UNUSED inline ls_string ls_callable2_call(ls_callable2 value) {
ls_native_retain(value.environment);
ls_string result = value.code(value.environment);
ls_native_release(value.environment);
return result;
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
static LS_NATIVE_UNUSED inline ls_string ls_callable3_call(ls_callable3 value,ls_callable2 ls_p0) {
ls_native_retain(value.environment);
ls_string result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable7 ls_callable7_retain(ls_callable7 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable7_release(ls_callable7 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable7_copy(ls_callable7 *destination, ls_callable7 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable7_take(ls_callable7 *destination, ls_callable7 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable7_clear(ls_callable7 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable7){0};
}
static LS_NATIVE_UNUSED inline ls_string ls_callable7_call(ls_callable7 value,ls_string ls_p0) {
ls_native_retain(value.environment);
ls_string result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable10 ls_callable10_retain(ls_callable10 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable10_release(ls_callable10 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable10_copy(ls_callable10 *destination, ls_callable10 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable10_take(ls_callable10 *destination, ls_callable10 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable10_clear(ls_callable10 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable10){0};
}
static LS_NATIVE_UNUSED inline bool ls_callable10_call(ls_callable10 value,ls_string ls_p0) {
ls_native_retain(value.environment);
bool result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
static LS_NATIVE_UNUSED inline ls_callable12 ls_callable12_retain(ls_callable12 value) {
ls_native_retain(value.environment);
return value;
}
static LS_NATIVE_UNUSED inline void ls_callable12_release(ls_callable12 value) { ls_native_release(value.environment); }
static LS_NATIVE_UNUSED inline void ls_callable12_copy(ls_callable12 *destination, ls_callable12 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable12_take(ls_callable12 *destination, ls_callable12 value) {
ls_native_release(destination->environment);
*destination = value;
}
static LS_NATIVE_UNUSED inline void ls_callable12_clear(ls_callable12 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable12){0};
}
static LS_NATIVE_UNUSED inline ls_string ls_callable12_call(ls_callable12 value,ls_string ls_p0,ls_string ls_p1) {
ls_native_retain(value.environment);
ls_string result = value.code(value.environment,ls_p0,ls_p1);
ls_native_release(value.environment);
return result;
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
static LS_NATIVE_UNUSED inline void ls_callable15_call(ls_callable15 value,ls_string ls_p0) {
ls_native_retain(value.environment);
value.code(value.environment,ls_p0);
ls_native_release(value.environment);
}
typedef int32_t host_assertNativeCount_arg0;
typedef void host_assertNativeCount_result;
void host_assertNativeCount(int32_t);
typedef void host_collectNative_result;
void host_collectNative(void);
typedef int32_t host_assertMaxNativeCount_arg0;
typedef void host_assertMaxNativeCount_result;
void host_assertMaxNativeCount(int32_t);
typedef ls_callable2 host_relay_arg0;
static LS_NATIVE_UNUSED inline ls_callable2 host_relay_arg0_retain(ls_callable2 value) { return ls_callable2_retain(value); }
static LS_NATIVE_UNUSED inline void host_relay_arg0_release(ls_callable2 value) { ls_callable2_release(value); }
static LS_NATIVE_UNUSED inline ls_string host_relay_arg0_call(ls_callable2 value) { return ls_callable2_call(value); }
typedef ls_string host_relay_result;
ls_string host_relay(ls_callable2);
typedef ls_callable2 host_recover_arg0;
static LS_NATIVE_UNUSED inline ls_callable2 host_recover_arg0_retain(ls_callable2 value) { return ls_callable2_retain(value); }
static LS_NATIVE_UNUSED inline void host_recover_arg0_release(ls_callable2 value) { ls_callable2_release(value); }
static LS_NATIVE_UNUSED inline ls_string host_recover_arg0_call(ls_callable2 value) { return ls_callable2_call(value); }
typedef ls_string host_recover_result;
ls_string host_recover(ls_callable2);
typedef void host_raiseHost_result;
void host_raiseHost(void);
#endif
