#include <assert.h>
#include <stdlib.h>
#include "tasks.h"
static void verify_shutdown(void) { assert(ls_native_owned_objects()==0); }
void host_watchNativeExit(void) { assert(atexit(verify_shutdown)==0); }
void host_collectNative(void) { ls_native_collect_cycles(); }
void host_assertMaxNativeCount(int32_t maximum) { assert(ls_native_owned_objects()<=(size_t)maximum); }
