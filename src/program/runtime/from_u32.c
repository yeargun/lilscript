static LS_NATIVE_UNUSED inline int32_t ls_from_u32(uint32_t value) {
    return value <= INT32_MAX ? (int32_t)value : (int32_t)((int64_t)value - INT64_C(4294967296));
}
