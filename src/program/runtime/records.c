static LS_NATIVE_UNUSED inline void ls_record_destroy(ls_native_object *owner) { ls_map_destroy(owner); }
static LS_NATIVE_UNUSED inline ls_native_object *ls_record_new(void) {
    return ls_native_allocate(sizeof(ls_map),ls_record_destroy,ls_map_trace);
}
static LS_NATIVE_UNUSED inline void ls_shape_set(ls_native_object *owner, ls_string key, ls_value value, bool omit_absent) {
    if (omit_absent && value.tag == LS_NULL) ls_map_delete(owner,ls_value_string(key));
    else ls_map_set(owner,ls_value_string(key),value);
}
static LS_NATIVE_UNUSED inline bool ls_shape_tag(ls_value value, ls_string key, ls_value tag) {
    return value.tag == LS_OBJECT && value.as.o && value.as.o->destroy == ls_record_destroy
        && ls_value_equal(ls_map_get(value.as.o,ls_value_string(key)),tag);
}
/* A portable Record owns string keys and tagged values in the shared map.
   Enumeration follows ECMAScript own-key order: canonical array indices first
   in numeric order, then other strings in insertion order. No prototype keys. */
typedef struct { size_t position; uint32_t index; bool numeric; } ls_record_key;
static LS_NATIVE_UNUSED inline bool ls_record_index(ls_string key, uint32_t *result) {
    if (!key.length || key.length > 10 || (key.length > 1 && key.data[0] == '0')) return false;
    uint64_t value=0;
    for (size_t i=0;i<key.length;i++) {
        if (key.data[i] < '0' || key.data[i] > '9') return false;
        value=value*10+(uint32_t)(key.data[i]-'0');
    }
    if (value >= UINT32_MAX) return false;
    *result=(uint32_t)value; return true;
}
static LS_NATIVE_UNUSED int ls_record_key_compare(const void *left, const void *right) {
    const ls_record_key *a=left,*b=right;
    if (a->numeric != b->numeric) return a->numeric ? -1 : 1;
    if (a->numeric) return a->index < b->index ? -1 : a->index > b->index;
    return a->position < b->position ? -1 : a->position > b->position;
}
static LS_NATIVE_UNUSED inline ls_record_key *ls_record_order(ls_native_object *owner) {
    ls_map *record=(ls_map *)owner;
    if (!record->size) return NULL;
    if (record->size > SIZE_MAX/sizeof(ls_record_key)) ls_native_resource_failure();
    ls_record_key *keys=malloc(record->size*sizeof *keys);
    if (!keys) ls_native_resource_failure();
    size_t count=0;
    for (size_t i=0;i<record->used;i++) if(record->entries[i].live) {
        keys[count]=(ls_record_key){.position=i};
        keys[count].numeric=ls_record_index(record->entries[i].key.as.s,&keys[count].index);
        ++count;
    }
    qsort(keys,count,sizeof *keys,ls_record_key_compare);
    return keys;
}
static LS_NATIVE_UNUSED inline ls_value ls_record_get(ls_native_object *record, ls_string key) {
    return ls_map_get(record,ls_value_string(key));
}
static LS_NATIVE_UNUSED inline void ls_record_set(ls_native_object *record, ls_string key, ls_value value) {
    ls_map_set(record,ls_value_string(key),value);
}
static LS_NATIVE_UNUSED inline bool ls_record_has(ls_native_object *record, ls_string key) {
    return ls_map_has(record,ls_value_string(key));
}
static LS_NATIVE_UNUSED inline ls_native_object *ls_record_assign(ls_native_object *target, ls_native_object *source) {
    if (target==source) return target;
    ls_map *record=(ls_map *)source;
    ls_record_key *order=ls_record_order(source);
    for(size_t i=0;i<record->size;i++) {
        ls_map_entry *entry=&record->entries[order[i].position];
        ls_map_set(target,entry->key,entry->value);
    }
    free(order); return target;
}
