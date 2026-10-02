static LS_NATIVE_UNUSED inline int32_t ls_shr(int32_t value, int32_t count) {
    return value >> ((uint32_t)count & UINT32_C(31));
}
