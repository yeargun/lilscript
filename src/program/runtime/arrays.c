/* One array identity and buffer across concrete and generic views. Concrete
   accessors retain their typed fast path; the descriptor converts only when
   the requested element representation differs from the physical buffer. */
typedef struct ls_array_ops {
    size_t width;
    void (*retain)(const void *);
    void (*drop)(void *);
    void (*trace)(const void *, ls_native_visit, void *);
    ls_value (*read)(ls_native_temporary **, const void *);
    void (*write)(void *, ls_value);
} ls_array_ops;
typedef struct ls_native_array {
    ls_native_object owner;
    const ls_array_ops *ops;
    size_t length, capacity;
    void *items;
    /* Null for a dense array. Sparse map results allocate presence lazily. */
    uint8_t *present;
} ls_native_array;
static LS_NATIVE_UNUSED void ls_native_undefined_element(void) {
    fputs("LilScript native array element is undefined\n",stderr); abort();
}
static LS_NATIVE_UNUSED inline void *ls_array_item(ls_native_array *array, size_t index) {
    return (unsigned char *)array->items + index*array->ops->width;
}
static LS_NATIVE_UNUSED inline bool ls_array_has(ls_native_array *array, size_t index) {
    return index < array->length && (!array->present || array->present[index]);
}
static LS_NATIVE_UNUSED void ls_array_destroy(ls_native_object *owner) {
    ls_native_array *array=(ls_native_array *)owner;
    for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) array->ops->drop(ls_array_item(array,i));
    free(array->items); free(array->present);
}
static LS_NATIVE_UNUSED void ls_array_trace(ls_native_object *owner, ls_native_visit visit, void *context) {
    ls_native_array *array=(ls_native_array *)owner;
    for(size_t i=0;i<array->length;++i) if(ls_array_has(array,i)) array->ops->trace(ls_array_item(array,i),visit,context);
}
static LS_NATIVE_UNUSED inline void ls_array_reserve(ls_native_array *array, size_t needed) {
    if(needed <= array->capacity) return;
    if(needed > (size_t)INT32_MAX) ls_native_resource_failure();
    size_t capacity=array->capacity ? array->capacity : 4;
    while(capacity < needed) {
        if(capacity > (size_t)INT32_MAX/2) { capacity=INT32_MAX; break; }
        capacity*=2;
    }
    if(capacity > SIZE_MAX/array->ops->width) ls_native_resource_failure();
    void *items=realloc(array->items,capacity*array->ops->width);
    if(!items) ls_native_resource_failure();
    array->items=items;
    if(array->present) {
        uint8_t *present=realloc(array->present,capacity);
        if(!present) ls_native_resource_failure();
        memset(present+array->capacity,0,capacity-array->capacity);
        array->present=present;
    }
    array->capacity=capacity;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_new(const ls_array_ops *ops, size_t capacity) {
    ls_native_array *array=ls_native_allocate(sizeof *array,ls_array_destroy,ls_array_trace);
    array->ops=ops; ls_array_reserve(array,capacity); return array;
}
static LS_NATIVE_UNUSED inline void ls_array_presence(ls_native_array *array) {
    if(array->present) return;
    array->present=calloc(array->capacity ? array->capacity : 1,1);
    if(!array->present) ls_native_resource_failure();
    memset(array->present,1,array->length);
}
static LS_NATIVE_UNUSED inline void *ls_array_append_slot(ls_native_array *array) {
    ls_array_reserve(array,array->length+1);
    void *slot=ls_array_item(array,array->length);
    memset(slot,0,array->ops->width);
    if(array->present) array->present[array->length]=1;
    ++array->length; return slot;
}
static LS_NATIVE_UNUSED inline void *ls_array_store_slot(ls_native_array *array, int32_t index) {
    if(index<0 || (size_t)index>array->length) ls_native_undefined_element();
    if((size_t)index==array->length) return ls_array_append_slot(array);
    if(array->present) array->present[index]=1;
    return ls_array_item(array,(size_t)index);
}
static LS_NATIVE_UNUSED inline void ls_array_hole(ls_native_array *array) {
    ls_array_append_slot(array); ls_array_presence(array); array->present[array->length-1]=0;
}
static LS_NATIVE_UNUSED inline ls_value ls_array_read(ls_native_array *array, size_t index, ls_native_temporary **temps) {
    if(!ls_array_has(array,index)) return (ls_value){0};
    return array->ops->read(temps,ls_array_item(array,index));
}
static LS_NATIVE_UNUSED inline void ls_array_write(ls_native_array *array, int32_t index, ls_value value) {
    array->ops->write(ls_array_store_slot(array,index),value);
}
static LS_NATIVE_UNUSED inline void ls_array_drop_last(ls_native_array *array) {
    if(!array->length) return;
    size_t index=array->length-1;
    if(ls_array_has(array,index)) array->ops->drop(ls_array_item(array,index));
    memset(ls_array_item(array,index),0,array->ops->width);
    array->length=index;
}
static LS_NATIVE_UNUSED inline size_t ls_array_relative(int32_t index, size_t length) {
    if(index<0) return (size_t)-(int64_t)index>=length ? 0 : length-(size_t)-(int64_t)index;
    return (size_t)index<length ? (size_t)index : length;
}
/* The caller selects separate destination storage. Equal schemas copy directly;
   different schemas preserve aliases through a short-lived tagged view. */
static LS_NATIVE_UNUSED inline void ls_array_append_from(ls_native_array *out, ls_native_array *source, size_t index) {
    if(!ls_array_has(source,index)) { ls_array_hole(out); return; }
    if(out->ops==source->ops) {
        void *slot=ls_array_append_slot(out);
        const void *input=ls_array_item(source,index);
        source->ops->retain(input); memcpy(slot,input,source->ops->width);
    } else {
        ls_native_temporary *temps=NULL;
        ls_value value=ls_array_read(source,index,&temps);
        out->ops->write(ls_array_append_slot(out),value);
        ls_native_temporaries_clear(&temps);
    }
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_slice(ls_native_array *array, size_t start, size_t end) {
    if(end>array->length) end=array->length;
    if(end<start) end=start;
    ls_native_array *out=ls_array_new(array->ops,end-start);
    for(size_t i=start;i<end;++i) ls_array_append_from(out,array,i);
    return out;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_concat(ls_native_array *left, ls_native_array *right) {
    if(left->length > (size_t)INT32_MAX-right->length) ls_native_resource_failure();
    ls_native_array *out=ls_array_new(left->ops,left->length+right->length);
    for(size_t i=0;i<left->length;++i) ls_array_append_from(out,left,i);
    for(size_t i=0;i<right->length;++i) ls_array_append_from(out,right,i);
    return out;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_splice(ls_native_array *array, int32_t start, int32_t count) {
    size_t from=ls_array_relative(start,array->length),removed=count>0?(size_t)count:0;
    if(removed>array->length-from) removed=array->length-from;
    ls_native_array *out=ls_array_slice(array,from,from+removed);
    for(size_t i=0;i<removed;++i) if(ls_array_has(array,from+i)) array->ops->drop(ls_array_item(array,from+i));
    size_t remaining=array->length-from-removed;
    if(remaining) memmove(ls_array_item(array,from),ls_array_item(array,from+removed),remaining*array->ops->width);
    if(array->present && remaining) memmove(array->present+from,array->present+from+removed,remaining);
    array->length-=removed; return out;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_reverse(ls_native_array *array) {
    if(array->length<2) return array;
    void *temporary=malloc(array->ops->width);
    if(!temporary) ls_native_resource_failure();
    for(size_t low=0,high=array->length-1;low<high;++low,--high) {
        void *a=ls_array_item(array,low),*b=ls_array_item(array,high);
        memcpy(temporary,a,array->ops->width); memcpy(a,b,array->ops->width); memcpy(b,temporary,array->ops->width);
        if(array->present) { uint8_t flag=array->present[low]; array->present[low]=array->present[high]; array->present[high]=flag; }
    }
    free(temporary); return array;
}
static LS_NATIVE_UNUSED inline ls_native_array *ls_array_copy_within(ls_native_array *array, int32_t target, int32_t start, bool bounded, int32_t end) {
    size_t to=ls_array_relative(target,array->length),from=ls_array_relative(start,array->length);
    size_t final=bounded?ls_array_relative(end,array->length):array->length;
    if(final<=from) return array;
    size_t count=final-from;
    if(count>array->length-to) count=array->length-to;
    for(size_t i=0;i<count;++i) if(ls_array_has(array,from+i)) array->ops->retain(ls_array_item(array,from+i));
    for(size_t i=0;i<count;++i) if(ls_array_has(array,to+i)) array->ops->drop(ls_array_item(array,to+i));
    if(count) {
        memmove(ls_array_item(array,to),ls_array_item(array,from),count*array->ops->width);
        if(array->present) memmove(array->present+to,array->present+from,count);
    }
    return array;
}
