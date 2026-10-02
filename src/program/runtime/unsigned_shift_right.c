static LS_NATIVE_UNUSED inline int32_t ls_ushr(int32_t value, int32_t count) {
    return ls_from_u32((uint32_t)value >> ((uint32_t)count & UINT32_C(31)));
}
