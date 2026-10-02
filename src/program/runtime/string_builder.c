/* Shared bounded UTF-16 builder for portable data/text recipes. */
typedef struct { uint16_t *units; size_t length,capacity; } ls_string_builder;
static LS_NATIVE_UNUSED inline void ls_string_builder_unit(ls_string_builder *out, uint16_t unit) {
    if(out->length >= (size_t)INT32_MAX) ls_string_failure("LilScript native builder exceeds the string limit");
    if(out->length==out->capacity) {
        size_t capacity=out->capacity ? out->capacity+out->capacity/2+1 : 64;
        if(capacity>(size_t)INT32_MAX) capacity=INT32_MAX;
        if(capacity>SIZE_MAX/sizeof *out->units) ls_native_resource_failure();
        uint16_t *units=realloc(out->units,capacity*sizeof *units);
        if(!units) ls_native_resource_failure();
        out->units=units; out->capacity=capacity;
    }
    out->units[out->length++]=unit;
}
static LS_NATIVE_UNUSED inline void ls_string_builder_text(ls_string_builder *out, ls_string text) {
    for(size_t i=0;i<text.length;i++) ls_string_builder_unit(out,text.data[i]);
}
static LS_NATIVE_UNUSED inline void ls_string_builder_ascii(ls_string_builder *out, const char *text) {
    while(*text) ls_string_builder_unit(out,(unsigned char)*text++);
}
static LS_NATIVE_UNUSED inline ls_string ls_string_builder_finish(ls_string_builder *out) {
    ls_string result=ls_string_from_utf16(out->units,out->length);
    free(out->units); *out=(ls_string_builder){0}; return result;
}
