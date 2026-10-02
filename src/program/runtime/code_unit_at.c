static LS_NATIVE_UNUSED inline int32_t ls_code_unit_at(ls_string value, int32_t index) {
    if (index < 0 || (size_t)index >= value.length) {
        fputs("LilScript native string index out of range\n", stderr);
        abort();
    }
    return (int32_t)value.data[(size_t)index];
}
