/* Authored callback provider, compiled separately against the generated header.
 * Its retained handles and live-object assertions exercise callback ABI v1.
 * No generated C is parsed, included, or altered by this provider. */
#include "program.h"
#include <stdio.h>
#include <stdlib.h>

#if LILSCRIPT_NATIVE_CALLBACK_ABI_VERSION != 1
#error unexpected callback ABI
#endif
#ifndef LS_NATIVE_QUALIFICATION
#error lifetime cohort requires the actual allocation counter
#endif

static host_keep_arg1 slots[8];
static int booted;

static void fail(const char *reason) {
    fprintf(stderr, "native closure host: %s\n", reason);
    abort();
}
static void at_exit(void) {
    if (ls_native_owned_objects() != 0) fail("owned objects remain after execution");
}
static void boot(void) {
    if (!booted) {
        booted = 1;
        if (atexit(at_exit) != 0) fail("atexit registration");
    }
}
static void valid_slot(int32_t slot) {
    boot();
    if (slot < 0 || slot >= 8) fail("slot out of range");
}
void host_keep(int32_t slot, host_keep_arg1 callback) {
    valid_slot(slot);
    /* Retain new before dropping old, including replacement by the same value. */
    host_keep_arg1 next = host_keep_arg1_retain(callback);
    host_keep_arg1 old = slots[slot];
    slots[slot] = next;
    host_keep_arg1_release(old);
}
int32_t host_invoke(int32_t slot, int32_t delta) {
    valid_slot(slot);
    host_keep_arg1 current = slots[slot];
    if (!current.code) fail("invoking empty slot");
    /* The generated typed call wrapper must retain across callback reentry,
     * including a callback that clears this slot while it is still executing. */
    return host_keep_arg1_call(current, delta);
}
void host_clear(int32_t slot) {
    valid_slot(slot);
    host_keep_arg1 old = slots[slot];
    slots[slot] = (host_keep_arg1){0};
    host_keep_arg1_release(old);
}
void host_clear_all(void) {
    boot();
    for (int32_t slot = 0; slot < 8; ++slot) host_clear(slot);
}
host_take_result host_take(int32_t slot) {
    valid_slot(slot);
    host_take_result result = slots[slot];
    if (!result.code) fail("taking empty slot");
    slots[slot] = (host_keep_arg1){0};
    /* Return transfers the existing retained owner to the source caller. */
    return result;
}
void host_expect_live(int32_t count) {
    boot();
    if (count < 0 || ls_native_owned_objects() != (size_t)count) {
        fprintf(stderr, "expected %d owned objects, found %zu\n", count, ls_native_owned_objects());
        fail("live owner count");
    }
}
void host_assert_empty(void) {
    host_expect_live(0);
}
