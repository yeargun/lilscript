static LS_NATIVE_UNUSED inline ls_string ls_char_at(ls_string value, int32_t index) {
    if (index < 0 || (size_t)index >= value.length) return (ls_string){0};
    ls_native_retain(value.owner);
    return (ls_string){value.data + (size_t)index, 1, value.owner};
}
