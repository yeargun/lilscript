#include "initialization.h"
#include <assert.h>
#include <stdio.h>
static int calls;
static void clear(void) {
    assert(failed_ls_native_exception_pending());
    failed_ls_value error=failed_ls_native_exception_take();failed_ls_value_release(error);
}
void host_observe(void) {
    ++calls;
    assert(!failed_initialize(0,NULL));clear();
    assert(failed_e_interfaces_2dinit__initialized_get()==0);clear();
    failed_shutdown();clear();
}
int main(void) {
    assert(!failed_initialize(0,NULL));assert(calls==1);
    failed_ls_value first=failed_ls_native_exception_take();
    assert(first.tag==failed_LS_STRING && first.as.s.length==11);
    assert(!failed_initialize(0,NULL));assert(calls==1);
    failed_ls_value second=failed_ls_native_exception_take();
    assert(second.tag==first.tag && second.as.s.length==first.as.s.length);
    for(size_t i=0;i<first.as.s.length;++i) assert(first.as.s.data[i]==second.as.s.data[i]);
    failed_ls_value_release(first);failed_ls_value_release(second);
    failed_shutdown();assert(failed_ls_native_owned_objects()==0);puts("initialization failure cached");
}
