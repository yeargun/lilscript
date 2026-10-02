#include <assert.h>
#include "ownership.h"
void host_assertNativeCount(int32_t expected) { assert(ls_native_owned_objects() == (size_t)expected); }
void host_collectNative(void) { ls_native_collect_cycles(); }
void host_assertMaxNativeCount(int32_t maximum) { assert(ls_native_owned_objects() <= (size_t)maximum); }
