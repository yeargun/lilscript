#ifndef LILSCRIPT_NATIVE_CALLBACK_ABI_V1_H
#define LILSCRIPT_NATIVE_CALLBACK_ABI_V1_H
#define LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION 1
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
typedef struct { const uint16_t *data; size_t length; } ls_string;
#include <stddef.h>
#include <stdint.h>
void ls_native_retain(void *environment);
void ls_native_release(void *environment);
typedef struct ls_native_object ls_native_object;
#ifdef LS_NATIVE_QUALIFICATION
size_t ls_native_owned_objects(void);
#endif
typedef struct {
int32_t (*code)(void *);
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
static inline int32_t ls_callable0_call(ls_callable0 value) {
ls_native_retain(value.environment);
int32_t result = value.code(value.environment);
ls_native_release(value.environment);
return result;
}
typedef struct {
int32_t (*code)(void *,ls_callable0);
void *environment;
uint64_t identity;
} ls_callable1;
static inline ls_callable1 ls_callable1_retain(ls_callable1 value) {
ls_native_retain(value.environment);
return value;
}
static inline void ls_callable1_release(ls_callable1 value) { ls_native_release(value.environment); }
static inline void ls_callable1_copy(ls_callable1 *destination, ls_callable1 value) {
ls_native_retain(value.environment);
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable1_take(ls_callable1 *destination, ls_callable1 value) {
ls_native_release(destination->environment);
*destination = value;
}
static inline void ls_callable1_clear(ls_callable1 *destination) {
ls_native_release(destination->environment);
*destination = (ls_callable1){0};
}
static inline int32_t ls_callable1_call(ls_callable1 value,ls_callable0 ls_p0) {
ls_native_retain(value.environment);
int32_t result = value.code(value.environment,ls_p0);
ls_native_release(value.environment);
return result;
}
typedef ls_callable0 host_install_arg0;
static inline ls_callable0 host_install_arg0_retain(ls_callable0 value) { return ls_callable0_retain(value); }
static inline void host_install_arg0_release(ls_callable0 value) { ls_callable0_release(value); }
static inline int32_t host_install_arg0_call(ls_callable0 value) { return ls_callable0_call(value); }
typedef int32_t host_install_result;
int32_t host_install(ls_callable0);
#endif
