#include "interfaces.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static demo_ls_string text(const char *s) {
    uint16_t units[128];size_t n=strlen(s);assert(n<128);
    for(size_t i=0;i<n;++i) units[i]=(unsigned char)s[i];
    return demo_ls_string_from_utf16(units,n);
}
static void equal(demo_ls_string value,const char *expected) {
    assert(value.length==strlen(expected));
    for(size_t i=0;i<value.length;++i) assert(value.data[i]==(unsigned char)expected[i]);
}
static void clear_error(void) {
    assert(demo_ls_native_exception_pending());
    demo_ls_value error=demo_ls_native_exception_take();demo_ls_value_release(error);
    assert(!demo_ls_native_exception_pending());
}
static int32_t host_callback(void *environment,int32_t value) {
    demo_ls_string key=text("offset");
    demo_ls_value offset=demo_ls_native_record_get(environment,key);
    demo_ls_string_release(key);assert(offset.tag==demo_LS_INT);return value+offset.as.i;
}
int main(void) {
    assert(demo_e_interfaces__total_get()==0);clear_error();
    assert(demo_initialize(0,NULL));assert(demo_initialize(0,NULL));
    assert(demo_e_interfaces__total_get()==3);
    assert(demo_e_interfaces__add(4,0,(demo_ls_native_arguments){1,NULL})==9);
    assert(demo_e_interfaces__add(1,5,(demo_ls_native_arguments){2,NULL})==15);
    bool absent[]={false,true};
    assert(demo_e_interfaces__add(1,99,(demo_ls_native_arguments){2,absent})==18);
    demo_e_interfaces__current_binding_result saved=demo_e_interfaces__current_get();
    assert(demo_e_interfaces__current(5)==6);demo_e_interfaces__replace();
    assert(demo_e_interfaces__current(5)==15);
    assert(demo_e_interfaces__current_binding_result_call(saved,5)==6);
    demo_e_interfaces__current_binding_result_release(saved);
    demo_ls_string who=text("world"),greeting=demo_e_interfaces__greeting(who);
    equal(greeting,"hello world");demo_ls_string_release(greeting);
    demo_e_interfaces__Payload_type payload={0};
    demo_e_interfaces__Payload_field_text_set(&payload,who);
    demo_e_interfaces__Payload_field_count_set(&payload,5);
    demo_e_interfaces__decorate_result decorated=demo_e_interfaces__decorate(payload);
    demo_ls_value boxed=demo_e_interfaces__Payload_box(payload);
    demo_ls_value boxed_again=demo_e_interfaces__identity(boxed);
    demo_e_interfaces__Payload_type unboxed=demo_e_interfaces__Payload_unbox(boxed_again);
    assert(demo_e_interfaces__Payload_field_count_get(unboxed)==5);
    demo_e_interfaces__decorate_result_release(unboxed);demo_ls_value_release(boxed);demo_ls_value_release(boxed_again);
    assert(demo_e_interfaces__Payload_field_count_get(payload)==5);
    assert(demo_e_interfaces__Payload_field_count_get(decorated)==6);
    demo_ls_string original=demo_e_interfaces__Payload_field_text_get(payload);
    demo_ls_string changed=demo_e_interfaces__Payload_field_text_get(decorated);
    equal(original,"world");equal(changed,"world!");
    demo_ls_string_release(original);demo_ls_string_release(changed);
    demo_e_interfaces__decorate_result_release(payload);demo_e_interfaces__decorate_result_release(decorated);
    demo_ls_value generic=demo_e_interfaces__identity((demo_ls_value){.tag=demo_LS_STRING,.as.s=who});
    equal(generic.as.s,"world");demo_ls_value_release(generic);demo_ls_string_release(who);
    assert(demo_e_interfaces__callProvider(7)==21);
    assert(demo_e_interfaces__sameProvider());
    demo_e_interfaces__closure_result callback=demo_e_interfaces__closure(10);
    assert(demo_e_interfaces__closure_result_call(callback,2)==12);
    assert(demo_e_interfaces__closure_result_call(callback,3)==15);
    demo_e_interfaces__closure_result_release(callback);
    demo_e_interfaces__Counter_type counter=demo_e_interfaces__Counter_new(0,(demo_ls_native_arguments){0,NULL});
    assert(demo_e_interfaces__Counter_field_value_get(counter)==4);
    assert(demo_e_interfaces__Counter_method_bump(counter,0,(demo_ls_native_arguments){1,NULL})==5);
    demo_e_interfaces__Counter_method_reader_result reader=demo_e_interfaces__Counter_method_reader(counter);
    demo_e_interfaces__Counter_field_value_set(counter,20);
    assert(demo_e_interfaces__Counter_method_reader_result_call(reader)==20);
    demo_ls_native_release(counter);assert(demo_e_interfaces__Counter_method_reader_result_call(reader)==20);
    demo_e_interfaces__Counter_method_reader_result_release(reader);
    counter=demo_e_interfaces__doubled(10);
    assert(demo_e_interfaces__Counter_method_bump(counter,3,(demo_ls_native_arguments){2,NULL})==16);
    demo_ls_native_release(counter);
    demo_ls_native_object *generator=demo_e_interfaces__numbers(30);demo_ls_value item;
    assert(demo_ls_native_generator_next(generator,&item));assert(item.tag==demo_LS_INT && item.as.i==30);demo_ls_value_release(item);
    demo_ls_native_generator_close(generator);demo_ls_native_release(generator);
    assert(demo_e_interfaces__total_get()==118);
    demo_ls_native_object *input=demo_ls_native_task_new(),*task=demo_e_interfaces__waitFor(input);
    assert(demo_ls_native_task_state(task)==0);
    demo_ls_native_task_resolve(input,(demo_ls_value){.tag=demo_LS_INT,.as.i=35});
    demo_ls_native_task_reject(input,(demo_ls_value){.tag=demo_LS_INT,.as.i=0});
    assert(demo_ls_native_task_state(task)==0);assert(demo_drain());
    assert(demo_ls_native_task_state(task)==1);item=demo_ls_native_task_result(task);assert(item.as.i==40);demo_ls_value_release(item);
    demo_ls_native_release(task);demo_ls_native_release(input);
    demo_ls_native_array *array=demo_e_interfaces__values_get();
    assert(demo_e_interfaces__arrayTotal(array)==6);
    demo_ls_native_array_set(array,1,(demo_ls_value){.tag=demo_LS_INT,.as.i=8});
    assert(demo_e_interfaces__arrayTotal(array)==10);demo_ls_native_release(array);
    array=demo_ls_native_array_new();demo_ls_native_array_set(array,0,(demo_ls_value){.tag=demo_LS_INT,.as.i=22});
    assert(demo_e_interfaces__arrayTotal(array)==22);demo_ls_native_release(array);
    demo_ls_native_object *record=demo_e_interfaces__labels_get();demo_ls_string key=text("first");
    item=demo_ls_native_record_get(record,key);equal(item.as.s,"one");demo_ls_value_release(item);demo_ls_string_release(key);demo_ls_native_release(record);
    demo_e_interfaces__fail();assert(demo_ls_native_exception_pending());
    item=demo_ls_native_exception_take();assert(item.tag==demo_LS_STRING);equal(item.as.s,"export failure");demo_ls_value_release(item);
    demo_e_interfaces__callProvider(88);clear_error();assert(demo_e_interfaces__total_get()==118);
    record=demo_ls_native_record_new();key=text("offset");
    demo_ls_native_record_set(record,key,(demo_ls_value){.tag=demo_LS_INT,.as.i=50});demo_ls_string_release(key);
    demo_e_interfaces__remember((demo_e_interfaces__remember_arg0){host_callback,record,demo_ls_native_new_identity()});
    demo_ls_native_release(record);assert(demo_e_interfaces__current(2)==52);
    demo_shutdown();demo_shutdown();assert(demo_ls_native_owned_objects()==0);
    assert(!demo_initialize(0,NULL));clear_error();
    assert(demo_ls_native_owned_objects()==0);puts("native interfaces done");
}
