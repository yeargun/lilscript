#include <assert.h>
#include <stdio.h>
#include "static-data.h"
#ifndef LS_EXPECT_STATIC
#error qualify both static and owned storage explicitly
#endif
void host_observeNativeStartup(void) {
    size_t count=ls_native_owned_objects();
    fprintf(stderr,"startup owned objects: %zu\n",count);
    assert(LS_EXPECT_STATIC ? count==0 : count>0);
}
void host_collectNative(void) { ls_native_collect_cycles(); }
void host_assertMaxNativeCount(int32_t maximum) { assert(ls_native_owned_objects()<=(size_t)maximum); }
