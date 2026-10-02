static LS_NATIVE_UNUSED inline int32_t ls_rem(int32_t left, int32_t right) {
    if (right == 0 || (left == INT32_MIN && right == -1)) return 0;
    return left % right;
}
