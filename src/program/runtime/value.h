/* Shared native value layout and ownership operations for C and host headers. */
typedef struct ls_value {
    uint8_t tag;
    /* A callable payload's physical signature. */
    uint32_t signature;
    union {
        int32_t i;
        double f;
        bool b;
        ls_string s;
        ls_native_object *o;
        struct { void (*code)(void); void *environment; uint64_t identity; } c;
    } as;
} ls_value;
enum { LS_NULL, LS_INT, LS_FLOAT, LS_BOOL, LS_STRING, LS_OBJECT, LS_ARRAY, LS_CALLABLE, LS_SYMBOL };
static LS_NATIVE_UNUSED inline void ls_value_retain(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_retain(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_retain(value.as.c.environment);
    else if (value.tag == LS_STRING) ls_native_retain(value.as.s.owner);
}
static LS_NATIVE_UNUSED inline void ls_value_release(ls_value value) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) ls_native_release(value.as.o);
    else if (value.tag == LS_CALLABLE) ls_native_release(value.as.c.environment);
    else if (value.tag == LS_STRING) ls_native_release(value.as.s.owner);
}
static LS_NATIVE_UNUSED inline void ls_value_trace(ls_value value, ls_native_visit visit, void *context) {
    if (value.tag == LS_OBJECT || value.tag == LS_ARRAY || value.tag == LS_SYMBOL) visit(value.as.o, context);
    else if (value.tag == LS_CALLABLE) visit(value.as.c.environment, context);
    else if (value.tag == LS_STRING) visit(value.as.s.owner, context);
}
