static LS_NATIVE_UNUSED inline bool ls_string_equal(ls_string left, ls_string right) {
    return left.length == right.length &&
           (left.length == 0 || memcmp(left.data, right.data, left.length * sizeof *left.data) == 0);
}
