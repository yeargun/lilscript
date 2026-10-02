static LS_NATIVE_UNUSED inline int32_t ls_imul(int32_t left, int32_t right) {
    uint64_t product = (uint64_t)(uint32_t)left * (uint64_t)(uint32_t)right;
    return ls_from_u32((uint32_t)product);
}
