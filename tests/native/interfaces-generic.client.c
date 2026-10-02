#include "generic.h"
#include <assert.h>
#include <stdio.h>
int main(void) {
    assert(generic_initialize(0,NULL));
    generic_ls_native_object *object=generic_e_interfaces_2dgeneric__make();
    generic_ls_value value=generic_e_interfaces_2dgeneric__Base_method_pick(object,(generic_ls_value){.tag=generic_LS_INT,.as.i=8},false,(generic_ls_native_arguments){2,NULL});
    assert(!generic_ls_native_exception_pending());assert(value.tag==generic_LS_INT && value.as.i==48);
    generic_ls_value_release(value);generic_ls_native_release(object);
    assert(generic_e_interfaces_2dgeneric__Mode_variant_Last()==1);
    assert(generic_e_interfaces_2dgeneric__Code_variant_Okay()==65);
    generic_ls_string text=generic_e_interfaces_2dgeneric__Kind_variant_Text();
    assert(text.length==4 && text.data[0]=='t');generic_ls_string_release(text);
    text=generic_e_interfaces_2dgeneric__Kind_variant_Empty();assert(text.length==0);generic_ls_string_release(text);
    generic_shutdown();assert(generic_ls_native_owned_objects()==0);puts("generic override and enum interfaces done");
}
