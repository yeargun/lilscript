/* Place loads borrow their view; the destination acquires ownership. */
static LS_NATIVE_UNUSED inline ls_string ls_string_index(ls_string value, int32_t index) {
    if (index < 0 || (size_t)index >= value.length) {
        ls_native_raise_error("RangeError","LilScript native string index out of range");
        return (ls_string){0};
    }
    return (ls_string){value.data + (size_t)index, 1, value.owner};
}
