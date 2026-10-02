static LS_NATIVE_UNUSED inline int32_t ls_mul(int32_t left, int32_t right) {
    int64_t product = (int64_t)left * (int64_t)right;
    if (product >= -INT64_C(9007199254740992) && product <= INT64_C(9007199254740992)) {
        return ls_from_u32((uint32_t)(uint64_t)product);
    }
    return ls_to_i32((double)left * (double)right);
}
