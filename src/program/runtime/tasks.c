/* Typed tasks and FIFO microtasks. Jobs own their context and payload; pending
   observers are traced edges of their task. There is no source interpreter. */
typedef struct ls_task ls_task;
typedef void (*ls_task_notify)(ls_native_object *,bool,ls_value);
typedef struct ls_task_observer {
    struct ls_task_observer *next;
    ls_task_notify notify;
    ls_native_object *context;
    ls_value value;
    bool rejected;
} ls_task_observer;
struct ls_task {
    ls_native_object owner;
    ls_value value;
    ls_task_observer *head,*tail;
    ls_task *unhandled_next,*unhandled_previous;
    unsigned state; /* 0 pending, 1 fulfilled, 2 rejected */
    bool resolved,handled,tracked;
};
static _Thread_local ls_task_observer *ls_jobs_head,*ls_jobs_tail;
static _Thread_local ls_task *ls_unhandled_tasks;
static LS_NATIVE_UNUSED void ls_task_observer_drop(ls_task_observer *observer) {
    ls_native_release(observer->context);ls_value_release(observer->value);free(observer);
}
static LS_NATIVE_UNUSED void ls_task_destroy(ls_native_object *owner) {
    ls_task *task=(ls_task *)owner;ls_value_clear(&task->value);
    while(task->head) {ls_task_observer *observer=task->head;task->head=observer->next;ls_task_observer_drop(observer);}
}
static LS_NATIVE_UNUSED void ls_task_trace(ls_native_object *owner,ls_native_visit visit,void *context) {
    ls_task *task=(ls_task *)owner;ls_value_trace(task->value,visit,context);
    for(ls_task_observer *observer=task->head;observer;observer=observer->next) visit(observer->context,context);
}
static LS_NATIVE_UNUSED ls_native_object *ls_value_to_task(ls_value value) {
    return ls_value_checked_object(value,ls_task_destroy);
}
static LS_NATIVE_UNUSED ls_task *ls_task_new(void) {return ls_native_allocate(sizeof(ls_task),ls_task_destroy,ls_task_trace);}
static LS_NATIVE_UNUSED bool ls_value_is_task(ls_value value) {
    return value.tag==LS_OBJECT && value.as.o && value.as.o->destroy==ls_task_destroy;
}
static LS_NATIVE_UNUSED void ls_task_enqueue(ls_task_observer *observer,bool rejected,ls_value value) {
    observer->next=NULL;observer->rejected=rejected;observer->value=value;ls_value_retain(value);
    if(ls_jobs_tail) ls_jobs_tail->next=observer;else ls_jobs_head=observer;
    ls_jobs_tail=observer;
}
static LS_NATIVE_UNUSED ls_task_observer *ls_task_observer_new(ls_task_notify notify,ls_native_object *context) {
    ls_task_observer *observer=calloc(1,sizeof *observer);if(!observer) ls_native_resource_failure();
    observer->notify=notify;observer->context=context;ls_native_retain(context);return observer;
}
static LS_NATIVE_UNUSED void ls_task_untrack(ls_task *task) {
    if(!task->tracked) return;
    if(task->unhandled_previous) task->unhandled_previous->unhandled_next=task->unhandled_next;else ls_unhandled_tasks=task->unhandled_next;
    if(task->unhandled_next) task->unhandled_next->unhandled_previous=task->unhandled_previous;
    task->unhandled_previous=task->unhandled_next=NULL;task->tracked=false;ls_native_release(task);
}
static LS_NATIVE_UNUSED void ls_task_observe(ls_task *task,ls_task_notify notify,ls_native_object *context) {
    ls_task_observer *observer=ls_task_observer_new(notify,context);task->handled=true;
    if(task->state) ls_task_enqueue(observer,task->state==2,task->value);
    else {if(task->tail) task->tail->next=observer;else task->head=observer;task->tail=observer;}
    ls_task_untrack(task);
}
static LS_NATIVE_UNUSED void ls_task_settle(ls_task *task,bool rejected,ls_value value) {
    if(task->state) return;
    task->resolved=true;task->state=rejected?2:1;ls_value_copy(&task->value,value);
    if(rejected && !task->handled) {ls_native_retain(task);task->unhandled_next=ls_unhandled_tasks;if(ls_unhandled_tasks) ls_unhandled_tasks->unhandled_previous=task;ls_unhandled_tasks=task;task->tracked=true;}
    ls_task_observer *observer=task->head;task->head=task->tail=NULL;
    while(observer) {ls_task_observer *next=observer->next;ls_task_enqueue(observer,rejected,value);observer=next;}
}
static LS_NATIVE_UNUSED void ls_task_forward(ls_native_object *context,bool rejected,ls_value value) {ls_task_settle((ls_task *)context,rejected,value);}
static LS_NATIVE_UNUSED void ls_task_adopt(ls_native_object *context,bool rejected,ls_value value) {
    (void)rejected;ls_task_observe((ls_task *)value.as.o,ls_task_forward,context);
}
static LS_NATIVE_UNUSED void ls_task_resolve(ls_task *task,ls_value value) {
    if(task->resolved) return;
    task->resolved=true;
    if(ls_value_is_task(value)) {
        if(value.as.o==&task->owner) {
            ls_native_raise_error("TypeError","task cannot resolve to itself");ls_value error=ls_exception_catch();ls_task_settle(task,true,error);ls_value_release(error);
        } else ls_task_enqueue(ls_task_observer_new(ls_task_adopt,&task->owner),false,value);
    } else ls_task_settle(task,false,value);
}
static LS_NATIVE_UNUSED ls_native_object *ls_task_resolved(ls_value value) {
    if(ls_value_is_task(value)) {ls_native_retain(value.as.o);return value.as.o;}
    ls_task *task=ls_task_new();ls_task_resolve(task,value);return &task->owner;
}
static LS_NATIVE_UNUSED ls_native_object *ls_task_rejected(ls_value reason) {
    ls_task *task=ls_task_new();ls_task_settle(task,true,reason);return &task->owner;
}
static LS_NATIVE_UNUSED void ls_task_reject_exception(ls_task *task) {
    ls_value reason=ls_exception_catch();ls_task_settle(task,true,reason);ls_value_release(reason);
}
/* Finally is precisely a then handler returning
   Task.resolve(callback()).then(() => saved completion). Its adoption jobs
   remain observable relative to unrelated tasks. */
typedef struct {ls_native_object owner;ls_task *result;ls_value saved;bool rejected;} ls_task_finally;
static LS_NATIVE_UNUSED void ls_task_finally_destroy(ls_native_object *owner) {
    ls_task_finally *state=(ls_task_finally *)owner;ls_native_release(state->result);ls_value_release(state->saved);
}
static LS_NATIVE_UNUSED void ls_task_finally_trace(ls_native_object *owner,ls_native_visit visit,void *context) {
    ls_task_finally *state=(ls_task_finally *)owner;visit((ls_native_object *)state->result,context);ls_value_trace(state->saved,visit,context);
}
static LS_NATIVE_UNUSED void ls_task_finally_notify(ls_native_object *owner,bool rejected,ls_value value) {
    ls_task_finally *state=(ls_task_finally *)owner;
    ls_task_settle(state->result,rejected||state->rejected,rejected?value:state->saved);
}
typedef ls_value (*ls_task_invoke)(ls_value,ls_value);
typedef struct {ls_native_object owner;ls_task *result;ls_value callback;ls_task_invoke invoke;unsigned mode;} ls_task_chain_state;
static LS_NATIVE_UNUSED void ls_task_chain_destroy(ls_native_object *owner) {
    ls_task_chain_state *state=(ls_task_chain_state *)owner;ls_native_release(state->result);ls_value_release(state->callback);
}
static LS_NATIVE_UNUSED void ls_task_chain_trace(ls_native_object *owner,ls_native_visit visit,void *context) {
    ls_task_chain_state *state=(ls_task_chain_state *)owner;visit((ls_native_object *)state->result,context);ls_value_trace(state->callback,visit,context);
}
static LS_NATIVE_UNUSED void ls_task_chain_notify(ls_native_object *owner,bool rejected,ls_value value) {
    ls_task_chain_state *state=(ls_task_chain_state *)owner;
    if((state->mode==0 && rejected)||(state->mode==1 && !rejected)) {ls_task_settle(state->result,rejected,value);return;}
    ls_value result=state->invoke(state->callback,value);
    if(ls_native_raised) {ls_task_reject_exception(state->result);ls_value_release(result);return;}
    if(state->mode==2) {
        ls_task *wait=(ls_task *)ls_task_resolved(result),*inner=ls_task_new();
        ls_task_finally *saved=ls_native_allocate(sizeof *saved,ls_task_finally_destroy,ls_task_finally_trace);
        saved->result=inner;saved->saved=value;ls_value_retain(value);saved->rejected=rejected;
        ls_task_observe(wait,ls_task_finally_notify,&saved->owner);
        ls_task_resolve(state->result,ls_value_object(&inner->owner));
        ls_native_release(wait);ls_native_release(saved);
    } else ls_task_resolve(state->result,result);
    ls_value_release(result);
}
static LS_NATIVE_UNUSED ls_native_object *ls_task_chain(ls_native_object *owner,unsigned mode,ls_value callback,ls_task_invoke invoke) {
    if(!owner || owner->destroy!=ls_task_destroy) {ls_value_mismatch();return NULL;}
    ls_task *result=ls_task_new();
    ls_task_chain_state *state=ls_native_allocate(sizeof *state,ls_task_chain_destroy,ls_task_chain_trace);
    state->result=result;ls_native_retain(result);state->callback=callback;ls_value_retain(callback);state->invoke=invoke;state->mode=mode;
    ls_task_observe((ls_task *)owner,ls_task_chain_notify,&state->owner);ls_native_release(state);return &result->owner;
}
/* all stores runtime-produced values in the shared tagged array descriptor;
   typed readers use the normal collection view without copying identities. */
typedef struct {ls_native_object owner;ls_task *result;ls_native_array *values;size_t remaining;} ls_task_all_state;
typedef struct {ls_native_object owner;ls_task_all_state *all;size_t index;} ls_task_all_item;
static LS_NATIVE_UNUSED void ls_task_all_destroy(ls_native_object *owner) {
    ls_task_all_state *state=(ls_task_all_state *)owner;ls_native_release(state->result);ls_native_release(state->values);
}
static LS_NATIVE_UNUSED void ls_task_all_trace(ls_native_object *owner,ls_native_visit visit,void *context) {
    ls_task_all_state *state=(ls_task_all_state *)owner;visit((ls_native_object *)state->result,context);visit((ls_native_object *)state->values,context);
}
static LS_NATIVE_UNUSED void ls_task_all_item_destroy(ls_native_object *owner) {ls_native_release(((ls_task_all_item *)owner)->all);}
static LS_NATIVE_UNUSED void ls_task_all_item_trace(ls_native_object *owner,ls_native_visit visit,void *context) {visit(&((ls_task_all_item *)owner)->all->owner,context);}
static LS_NATIVE_UNUSED void ls_task_all_notify(ls_native_object *owner,bool rejected,ls_value value) {
    ls_task_all_item *item=(ls_task_all_item *)owner;ls_task_all_state *all=item->all;
    if(all->result->state) return;
    if(rejected) {ls_task_settle(all->result,true,value);return;}
    ls_array_write(all->values,(int32_t)item->index,value);
    if(!--all->remaining) ls_task_settle(all->result,false,ls_value_array(&all->values->owner));
}
static LS_NATIVE_UNUSED ls_native_object *ls_task_all(ls_native_array *tasks) {
    if(!tasks) {ls_value_mismatch();return NULL;}
    ls_task *result=ls_task_new();ls_native_array *values=ls_array_new(&ls_tagged_array_ops,tasks->length);
    ls_task_all_state *all=ls_native_allocate(sizeof *all,ls_task_all_destroy,ls_task_all_trace);
    all->result=result;ls_native_retain(result);all->values=values;all->remaining=tasks->length;
    for(size_t i=0;i<tasks->length;++i) ls_array_append_slot(values);
    for(size_t i=0;i<tasks->length;++i) {
        ls_native_temporary *temps=NULL;ls_value value=ls_array_read(tasks,i,&temps);
        ls_task *task=(ls_task *)ls_task_resolved(value);
        ls_task_all_item *item=ls_native_allocate(sizeof *item,ls_task_all_item_destroy,ls_task_all_item_trace);
        item->all=all;ls_native_retain(all);item->index=i;
        ls_task_observe(task,ls_task_all_notify,&item->owner);ls_native_release(task);ls_native_release(item);ls_native_temporaries_clear(&temps);
    }
    if(!all->remaining) ls_task_settle(result,false,ls_value_array(&values->owner));
    ls_native_release(all);return &result->owner;
}
/* The frame writer supplies typed slot cleanup/tracing and compiled resume
   labels. The only frame kept by the queue is the one waiting for that task. */
typedef struct ls_async_frame ls_async_frame;
struct ls_async_frame {
    ls_native_object owner;
    void (*step)(ls_async_frame *);
    void (*clear)(ls_async_frame *);
    void (*trace_slots)(ls_async_frame *,ls_native_visit,void *);
    void *environment;
    size_t pc;
    bool done,rejected;
    ls_task *result;
    ls_value awaited;
};
static LS_NATIVE_UNUSED void ls_async_finish(ls_async_frame *frame) {
    if(frame->done) return;
    frame->done=true;frame->clear(frame);ls_native_release(frame->environment);frame->environment=NULL;
    ls_native_release(frame->result);frame->result=NULL;ls_value_clear(&frame->awaited);
}
static LS_NATIVE_UNUSED void ls_async_destroy(ls_native_object *owner) {ls_async_finish((ls_async_frame *)owner);}
static LS_NATIVE_UNUSED void ls_async_trace(ls_native_object *owner,ls_native_visit visit,void *context) {
    ls_async_frame *frame=(ls_async_frame *)owner;visit(frame->environment,context);visit((ls_native_object *)frame->result,context);ls_value_trace(frame->awaited,visit,context);
    if(!frame->done) frame->trace_slots(frame,visit,context);
}
static LS_NATIVE_UNUSED void ls_async_resume(ls_native_object *owner,bool rejected,ls_value value) {
    ls_async_frame *frame=(ls_async_frame *)owner;if(frame->done) return;
    frame->rejected=rejected;ls_value_copy(&frame->awaited,value);frame->step(frame);
}
static LS_NATIVE_UNUSED void ls_async_await(ls_async_frame *frame,ls_native_object *owner) {
    if(!owner || owner->destroy!=ls_task_destroy) {ls_value_mismatch();return;}
    ls_task_observe((ls_task *)owner,ls_async_resume,&frame->owner);
}
static LS_NATIVE_UNUSED void ls_task_discard_jobs(void) {
    while(ls_jobs_head) {ls_task_observer *job=ls_jobs_head;ls_jobs_head=job->next;ls_task_observer_drop(job);}ls_jobs_tail=NULL;
    while(ls_unhandled_tasks) ls_task_untrack(ls_unhandled_tasks);
}
static LS_NATIVE_UNUSED void ls_task_drain(void) {
    while(ls_jobs_head && !ls_native_raised) {
        ls_task_observer *job=ls_jobs_head;ls_jobs_head=job->next;if(!ls_jobs_head) ls_jobs_tail=NULL;
        job->notify(job->context,job->rejected,job->value);ls_task_observer_drop(job);
    }
    while(ls_unhandled_tasks) {
        ls_task *task=ls_unhandled_tasks;
        if(!task->handled && !ls_native_raised) ls_native_throw(task->value);
        ls_task_untrack(task);
    }
}
