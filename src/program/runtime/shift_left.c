static LS_NATIVE_UNUSED inline int32_t ls_shl(int32_t value, int32_t count) {
    uint32_t shift = (uint32_t)count & UINT32_C(31);
    return ls_from_u32((uint32_t)((uint32_t)value << shift));
}
