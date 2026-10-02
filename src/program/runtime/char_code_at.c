static LS_NATIVE_UNUSED inline int32_t ls_char_code_at(ls_string value, int32_t index) {
    return index < 0 || (size_t)index >= value.length ? 0 : (int32_t)value.data[(size_t)index];
}
