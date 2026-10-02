/* One traced layout for physical callable bridges. A reverse conversion can
   recover the original callable without allocating a chain of wrappers. */
typedef struct {
    ls_native_object owner;
    ls_native_temporary temporary;
    ls_value inner;
} ls_callable_bridge;
static LS_NATIVE_UNUSED void ls_callable_bridge_destroy(ls_native_object *owner) {
    ls_value_release(((ls_callable_bridge *)owner)->inner);
}
static LS_NATIVE_UNUSED void ls_callable_bridge_trace(ls_native_object *owner, ls_native_visit visit, void *context) {
    ls_value_trace(((ls_callable_bridge *)owner)->inner,visit,context);
}
