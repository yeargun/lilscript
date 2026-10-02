#include <assert.h>
#include "ownership.h"
void host_assertNativeCount(int32_t expected) { assert(ls_native_owned_objects() == (size_t)expected); }
void host_collectNative(void) { ls_native_collect_cycles(); }
ls_string host_makeHostText(void) {
    uint16_t units[] = {65, 66, 67};
    return ls_string_from_utf16(units, 3);
}
