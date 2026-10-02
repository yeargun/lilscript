static LS_NATIVE_UNUSED inline double ls_char_code_at_number(ls_string value, int32_t index) {
    return index < 0 || (size_t)index >= value.length ? NAN : (double)value.data[(size_t)index];
}
