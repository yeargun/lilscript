/* Place loads borrow their view; the destination acquires ownership. */
static LS_NATIVE_UNUSED inline ls_string ls_string_index(ls_string value, int32_t index) {
    (void)ls_code_unit_at(value, index);
    return (ls_string){value.data + (size_t)index, 1, value.owner};
}
