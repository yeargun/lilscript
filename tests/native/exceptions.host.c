#include <assert.h>
#include "ownership.h"
void host_assertNativeCount(int32_t expected) { assert(ls_native_owned_objects() == (size_t)expected); }
void host_collectNative(void) { ls_native_collect_cycles(); }
void host_assertMaxNativeCount(int32_t maximum) { assert(ls_native_owned_objects() <= (size_t)maximum); }

ls_string host_relay(host_relay_arg0 action) {
    ls_string result=host_relay_arg0_call(action);
    if(ls_native_exception_pending()) { ls_string_release(result); return (ls_string){0}; }
    return result;
}
ls_string host_recover(host_recover_arg0 action) {
    ls_string result=host_recover_arg0_call(action);
    if(!ls_native_exception_pending()) return result;
    ls_string_release(result);
    ls_value error=ls_native_exception_take();
    assert(error.tag==LS_STRING); ls_value_release(error);
    static const uint16_t message[]={'h','o','s','t',' ','r','e','c','o','v','e','r','e','d'};
    return ls_string_from_utf16(message,sizeof message/sizeof *message);
}
void host_raiseHost(void) {
    static const uint16_t message[]={'h','o','s','t',' ','r','a','i','s','e','d'};
    ls_string text=ls_string_from_utf16(message,sizeof message/sizeof *message);
    ls_value error={.tag=LS_STRING}; error.as.s=text;
    ls_native_exception_raise(error); ls_string_release(text);
}
