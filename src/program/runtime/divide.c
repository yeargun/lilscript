static LS_NATIVE_UNUSED inline int32_t ls_div(int32_t left, int32_t right) {
    if (right == 0) return 0;
    if (left == INT32_MIN && right == -1) return INT32_MIN;
    return left / right;
}
