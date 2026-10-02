/* Source regions are compiled into each frame's step function. This runtime
   owns handles and iteration protocol only; it executes no source bytecode. */
typedef struct ls_generator ls_generator;
struct ls_generator {
    ls_native_object owner;
    void (*step)(ls_generator *, bool);
    void (*clear)(ls_generator *);
    void (*trace_slots)(ls_generator *,ls_native_visit,void *);
    void *environment;
    size_t pc;
    bool started,running,done;
    ls_value yielded;
};
static LS_NATIVE_UNUSED void ls_generator_finish(ls_generator *frame) {
    if(frame->done) return;
    frame->done=true;
    frame->clear(frame);
    ls_native_release(frame->environment);frame->environment=NULL;
    ls_value_clear(&frame->yielded);
}
static LS_NATIVE_UNUSED void ls_generator_destroy(ls_native_object *owner) {
    ls_generator_finish((ls_generator *)owner);
}
static LS_NATIVE_UNUSED void ls_generator_trace(ls_native_object *owner,ls_native_visit visit,void *context) {
    ls_generator *frame=(ls_generator *)owner;
    visit(frame->environment,context);ls_value_trace(frame->yielded,visit,context);
    if(!frame->done) frame->trace_slots(frame,visit,context);
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_value_to_generator(ls_value value) {
    return ls_value_checked_object(value,ls_generator_destroy);
}
static LS_NATIVE_UNUSED bool ls_generator_next(ls_native_object *owner) {
    if(!owner || owner->destroy!=ls_generator_destroy) {ls_value_mismatch();return false;}
    ls_generator *frame=(ls_generator *)owner;
    if(frame->running) {ls_native_raise_error("TypeError","generator is already running");return false;}
    if(frame->done) return false;
    frame->running=true;frame->started=true;ls_value_clear(&frame->yielded);
    frame->step(frame,false);frame->running=false;
    return !frame->done && !ls_native_raised;
}
static LS_NATIVE_UNUSED void ls_generator_close(ls_native_object *owner) {
    ls_generator *frame=(ls_generator *)owner;
    if(frame->running) {ls_native_raise_error("TypeError","generator is already running");return;}
    if(frame->done) return;
    if(!frame->started) {ls_generator_finish(frame);return;}
    frame->running=true;ls_value_clear(&frame->yielded);
    frame->step(frame,true);frame->running=false;
}

typedef struct ls_iteration ls_iteration;
typedef bool (*ls_iteration_next_fn)(ls_iteration *,ls_value *,ls_native_temporary **);
struct ls_iteration {ls_native_object *source;size_t index;ls_iteration_next_fn next;};
static LS_NATIVE_UNUSED bool ls_iteration_generator(ls_iteration *iterator,ls_value *value,ls_native_temporary **temps) {
    (void)temps;
    if(!ls_generator_next(iterator->source)) return false;
    *value=((ls_generator *)iterator->source)->yielded;return true;
}
static LS_NATIVE_UNUSED bool ls_iteration_array(ls_iteration *iterator,ls_value *value,ls_native_temporary **temps) {
    ls_native_array *array=(ls_native_array *)iterator->source;
    if(iterator->index>=array->length) return false;
    *value=ls_array_read(array,iterator->index++,temps);return true;
}
static LS_NATIVE_UNUSED bool ls_iteration_set(ls_iteration *iterator,ls_value *value,ls_native_temporary **temps) {
    (void)temps;ls_map *set=(ls_map *)iterator->source;
    while(iterator->index<set->used) {
        ls_map_entry *entry=&set->entries[iterator->index++];
        if(entry->live) {*value=entry->key;return true;}
    }
    return false;
}
/* Discarding an unreachable iterator never executes a user finalizer. */
static LS_NATIVE_UNUSED void ls_iteration_dispose(ls_iteration *iterator) {
    if(!iterator->source) return;
    if(iterator->next==ls_iteration_set) {ls_map *set=(ls_map *)iterator->source;if(!set->iterators) ls_native_resource_failure();--set->iterators;}
    ls_native_release(iterator->source);*iterator=(ls_iteration){0};
}
static LS_NATIVE_UNUSED void ls_iteration_start(ls_iteration *iterator,ls_native_object *source,ls_iteration_next_fn next) {
    ls_iteration_dispose(iterator);
    if(!source) {ls_value_mismatch();return;}
    ls_native_retain(source);*iterator=(ls_iteration){source,0,next};
    if(next==ls_iteration_set) {ls_map *set=(ls_map *)source;if(set->iterators==SIZE_MAX) ls_native_resource_failure();++set->iterators;}
}
static LS_NATIVE_UNUSED bool ls_iteration_next(ls_iteration *iterator,ls_value *value,ls_native_temporary **temps) {
    *value=(ls_value){0};
    return iterator->source && iterator->next(iterator,value,temps);
}
static LS_NATIVE_UNUSED void ls_iteration_close(ls_iteration *iterator) {
    if(!iterator->source) return;
    ls_iteration held=*iterator;*iterator=(ls_iteration){0};
    /* An existing throw wins over a failing IteratorClose. Other abrupt
       completions are replaced by a close failure in the generated caller. */
    ls_exception_state pending=ls_exception_save();
    if(held.next==ls_iteration_generator) ls_generator_close(held.source);
    ls_iteration_dispose(&held);
    if(pending.raised) {ls_exception_state failure=ls_exception_save();ls_exception_clear(&failure);ls_exception_restore(&pending);}
    else ls_exception_clear(&pending);
}
