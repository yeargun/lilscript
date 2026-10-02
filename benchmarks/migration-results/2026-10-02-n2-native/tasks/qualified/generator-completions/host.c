#include <assert.h>
#include "tasks.h"
void host_assertNativeCount(int32_t expected) { assert(ls_native_owned_objects() == (size_t)expected); }
void host_collectNative(void) { ls_native_collect_cycles(); }
