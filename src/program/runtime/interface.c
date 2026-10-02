uint64_t ls_native_new_identity(void) {return ls_native_fresh_identity();}
ls_native_object *ls_native_record_new(void) { return ls_native_raised ? NULL : ls_record_new(); }
ls_value ls_native_record_get(ls_native_object *object,ls_string key) {
    if(ls_native_raised) return (ls_value){0};
    if(!object || object->destroy!=ls_record_destroy) {ls_value_mismatch();return (ls_value){0};}
    ls_value value=ls_record_get(object,key);ls_value_retain(value);return value;
}
void ls_native_record_set(ls_native_object *object,ls_string key,ls_value value) {
    if(ls_native_raised) return;
    if(!object || object->destroy!=ls_record_destroy) {ls_value_mismatch();return;}
    ls_record_set(object,key,value);
}
#ifdef LS_NATIVE_API_ARRAYS
ls_native_array *ls_native_array_new(void) { return ls_native_raised ? NULL : ls_array_new(&ls_tagged_array_ops,0); }
size_t ls_native_array_length(ls_native_array *array) {
    if(ls_native_raised) return 0;
    if(!array || array->owner.destroy!=ls_array_destroy) {ls_value_mismatch();return 0;}
    return array->length;
}
ls_value ls_native_array_get(ls_native_array *array,int32_t index) {
    if(ls_native_raised) return (ls_value){0};
    if(!array || array->owner.destroy!=ls_array_destroy) {ls_value_mismatch();return (ls_value){0};}
    ls_native_temporary *temps=NULL;
    ls_value value=index<0 ? (ls_value){0} : ls_array_read(array,(size_t)index,&temps);
    ls_value_retain(value);ls_native_temporaries_clear(&temps);return value;
}
void ls_native_array_set(ls_native_array *array,int32_t index,ls_value value) {
    if(ls_native_raised) return;
    if(!array || array->owner.destroy!=ls_array_destroy) {ls_value_mismatch();return;}
    if(index<0) {ls_native_raise_error("RangeError","negative native array index");return;}
    ls_array_write(array,index,value);
}
#endif
#ifdef LS_NATIVE_API_TASKS
static LS_NATIVE_UNUSED ls_task *ls_native_task_check(ls_native_object *object) {
    if(ls_native_raised) return NULL;
    if(!object || object->destroy!=ls_task_destroy) {ls_value_mismatch();return NULL;}
    return (ls_task *)object;
}
ls_native_object *ls_native_task_new(void) { return ls_native_raised ? NULL : &ls_task_new()->owner; }
void ls_native_task_resolve(ls_native_object *object,ls_value value) {
    ls_task *task=ls_native_task_check(object);if(task) ls_task_resolve(task,value);
}
void ls_native_task_reject(ls_native_object *object,ls_value value) {
    ls_task *task=ls_native_task_check(object);
    if(task && !task->resolved) {task->resolved=true;ls_task_settle(task,true,value);}
}
unsigned ls_native_task_state(ls_native_object *object) {
    ls_task *task=ls_native_task_check(object);return task ? task->state : 0;
}
ls_value ls_native_task_result(ls_native_object *object) {
    ls_task *task=ls_native_task_check(object);
    if(!task) return (ls_value){0};
    if(!task->state) {ls_native_raise_error("TypeError","native task is still pending");return (ls_value){0};}
    task->handled=true;ls_task_untrack(task);ls_value_retain(task->value);return task->value;
}
#endif
#ifdef LS_NATIVE_API_ITERATORS
bool ls_native_generator_next(ls_native_object *object,ls_value *value) {
    if(value) *value=(ls_value){0};
    if(ls_native_raised) return false;
    if(!value) {ls_value_mismatch();return false;}
    if(!ls_generator_next(object)) return false;
    *value=((ls_generator *)object)->yielded;ls_value_retain(*value);return true;
}
void ls_native_generator_close(ls_native_object *object) {
    if(ls_native_raised) return;
    if(!object || object->destroy!=ls_generator_destroy) {ls_value_mismatch();return;}
    ls_generator_close(object);
}
#endif
